"""Unit checks for the TypeSafe System One layer — mock HTTP only, no real
API calls, no game spawns, no key required.

Follows the scripts/test_openpokered_llm.py pattern (ScriptedOpener /
FakeResponse / http_error), so the two adapters are tested the same way.

Run: python3 -m unittest scripts.test_openpokered_typesafe
"""
import json
import sys
import unittest
import urllib.error
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from openpokered import policies, semantics
from openpokered import typesafe as ts


# ── mock HTTP ─────────────────────────────────────────────────────────
class FakeResponse:
    def __init__(self, payload):
        self.payload = payload

    def read(self):
        return json.dumps(self.payload).encode()

    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False


class ScriptedOpener:
    """Returns queued outcomes (payloads or exceptions) per call."""

    def __init__(self, outcomes):
        self.outcomes = list(outcomes)
        self.calls = []

    def __call__(self, req, timeout=None):
        self.calls.append(req)
        item = self.outcomes.pop(0)
        if isinstance(item, Exception):
            raise item
        return FakeResponse(item)


def http_error(code):
    body = b"boom"
    fp = type("B", (), {"read": lambda self, n=-1: body,
                        "close": lambda self: None})()
    return urllib.error.HTTPError("http://x", code, "err", hdrs=None, fp=fp)


def answers_payload(answers, model="jev-1.13.0", in_tok=10, out_tok=4):
    return {"model": model, "answers": answers,
            "usage": {"input_tokens": in_tok, "output_tokens": out_tok}}


def client(opener, **kw):
    return ts.TypeSafeClient("https://api.example", "k", "jev-latest",
                             opener=opener, **kw)


# ── credentials and .env ──────────────────────────────────────────────
class CredentialTests(unittest.TestCase):
    def test_key_required(self):
        with self.assertRaises(ts.CredentialError):
            ts.resolve_credentials({})

    def test_defaults(self):
        base, key, source = ts.resolve_credentials({ts.API_KEY_ENV: "k"})
        self.assertEqual((base, key, source),
                         (ts.DEFAULT_BASE_URL, "k", "typesafe"))

    def test_base_and_model_overrides(self):
        env = {ts.API_KEY_ENV: "k", ts.BASE_URL_ENV: "http://v/",
               ts.MODEL_ENV: "jev-x"}
        self.assertEqual(ts.resolve_credentials(env)[0], "http://v")
        self.assertEqual(ts.default_model(env), "jev-x")

    def test_not_configured_without_key(self):
        self.assertFalse(ts.configured({}))

    def test_env_file_fills_gaps_only(self):
        path = Path(self.tmp()) / ".env"
        path.write_text("# comment\n\nA=1\nexport B='two'\nA=ignored\n")
        env = {"A": "real"}
        self.assertEqual(ts.load_env_file(path, env=env), 1)
        self.assertEqual(env, {"A": "real", "B": "two"})

    def test_env_file_missing_is_not_an_error(self):
        self.assertEqual(ts.load_env_file(Path("/nonexistent/.env"), env={}), 0)

    def tmp(self):
        import tempfile
        d = tempfile.mkdtemp(prefix="typesafe-test-")
        self.addCleanup(lambda: __import__("shutil").rmtree(d, ignore_errors=True))
        return d


# ── question builders ─────────────────────────────────────────────────
class QuestionTests(unittest.TestCase):
    def test_noul_without_criteria_omits_the_key(self):
        self.assertEqual(ts.Noul("is it?").to_json(),
                         {"type": "noul", "instructions": "is it?"})

    def test_noul_criteria_kept_when_given(self):
        q = ts.Noul("is it?", true="yes it is", false="no it is not")
        self.assertEqual(q.to_json()["criteria"],
                         {"true": "yes it is", "false": "no it is not"})

    def test_choice_needs_options(self):
        with self.assertRaises(ValueError):
            ts.Choice("pick", {})

    def test_score_needs_two_levels(self):
        with self.assertRaises(ValueError):
            ts.Score("rate", ["only one"])
        self.assertEqual(ts.Score("rate", ["a", "b"]).to_json()["criteria"],
                         ["a", "b"])

    def test_choice_null_description_is_preserved(self):
        q = ts.Choice("pick", {"a": None, "b": "the other one"})
        self.assertIsNone(q.to_json()["criteria"]["a"])


# ── HTTP client ───────────────────────────────────────────────────────
class ClientTests(unittest.TestCase):
    def test_request_shape(self):
        opener = ScriptedOpener([answers_payload({"q": {"type": "noul",
                                                        "noul": 0.9}})])
        client(opener).system_one({"a": 1}, {"q": ts.Noul("well?")})
        body = json.loads(opener.calls[0].data)
        self.assertEqual(body["state"], {"a": 1})
        self.assertEqual(body["model"], "jev-latest")
        self.assertEqual(body["questions"],
                         {"q": {"type": "noul", "instructions": "well?"}})
        self.assertEqual(opener.calls[0].get_header("Authorization"),
                         "Bearer k")
        self.assertTrue(opener.calls[0].full_url.endswith("/v1/systemone"))

    def test_all_three_answer_types_parse(self):
        opener = ScriptedOpener([answers_payload({
            "a": {"type": "noul", "noul": 0.25},
            "b": {"type": "choice", "choice": "x",
                  "probabilities": {"x": 0.7, "y": 0.3}, "confidence": 0.4},
            "c": {"type": "score", "score": 1.5,
                  "legend": {"0": "lo", "1": "hi"},
                  "probabilities": {"0": 0.5, "1": 0.5}, "confidence": 0.0},
        })])
        result = client(opener).system_one("s", {"q": ts.Noul("?")})
        self.assertAlmostEqual(result.answers["a"].noul, 0.25)
        self.assertEqual(result.answers["b"].choice, "x")
        self.assertEqual(result.answers["c"].legend, {"0": "lo", "1": "hi"})
        self.assertEqual((result.input_tokens, result.output_tokens), (10, 4))

    def test_unknown_answer_type_is_an_error(self):
        opener = ScriptedOpener([answers_payload({"q": {"type": "wat"}})])
        with self.assertRaises(ts.TypeSafeError):
            client(opener).system_one("s", {"q": ts.Noul("?")})

    def test_429_and_529_are_retried(self):
        for code in (429, 529):
            opener = ScriptedOpener([http_error(code),
                                     answers_payload({"q": {"type": "noul",
                                                            "noul": 0.1}})])
            result = client(opener).system_one("s", {"q": ts.Noul("?")})
            self.assertAlmostEqual(result.answers["q"].noul, 0.1)
            self.assertEqual(len(opener.calls), 2, code)

    def test_other_4xx_is_not_retried(self):
        for code in (400, 401, 422):
            opener = ScriptedOpener([http_error(code)])
            with self.assertRaises(ts.TypeSafeError):
                client(opener).system_one("s", {"q": ts.Noul("?")})
            self.assertEqual(len(opener.calls), 1, code)

    def test_empty_questions_rejected(self):
        with self.assertRaises(ValueError):
            client(ScriptedOpener([])).system_one("s", {})


# ── semantic judgments ────────────────────────────────────────────────
class StubClient:
    """Records requests, replays queued answer dicts."""

    def __init__(self, *queued):
        self.queued = list(queued)
        self.requests = []

    def system_one(self, state, questions, model=None):
        self.requests.append({"state": state, "questions": questions})
        item = self.queued.pop(0)
        if isinstance(item, Exception):
            raise item
        return ts.SystemOneResult("jev-stub", item, 7, 3)


def choice_answer(pick, probs):
    return ts.ChoiceAnswer(pick, probs, max(probs.values()))


CANDIDATES = [
    {"id": "npc:0", "kind": "npc", "name": "Oak",
     "position": {"x": 5, "y": 3}, "distance": 4, "interactable": True},
    {"id": "npc:1", "kind": "npc", "name": "Rival",
     "position": {"x": 8, "y": 6}, "distance": 7, "interactable": True},
]


class DisabledJudgeTests(unittest.TestCase):
    """No key means no judgment — every call returns None so callers keep
    their deterministic path."""

    def test_disabled_by_default(self):
        judge = semantics.SemanticJudge()
        self.assertFalse(judge.enabled)
        self.assertIsNone(judge.select_entity("talk to Oak", CANDIDATES))
        self.assertIsNone(judge.route_action("go", {"a": "b"}, {}))
        self.assertIsNone(judge.conveys("hi", "a greeting"))
        self.assertIsNone(judge.same_meaning("a", "b"))
        self.assertEqual(judge.calls, 0)

    def test_from_env_without_key_is_disabled(self):
        self.assertFalse(semantics.SemanticJudge.from_env({}).enabled)


class JudgmentTests(unittest.TestCase):
    def test_threshold_and_band(self):
        self.assertTrue(semantics.Judgment(0.9, "c").holds)
        self.assertTrue(semantics.Judgment(0.9, "c").resolved)
        self.assertFalse(semantics.Judgment(0.1, "c").holds)
        self.assertFalse(semantics.Judgment(0.5, "c").resolved)
        self.assertFalse(semantics.Judgment(0.45, "c").resolved)

    def test_threshold_is_configurable(self):
        j = semantics.Judgment(0.65, "c", threshold=0.9)
        self.assertFalse(j.holds)


class SelectionTests(unittest.TestCase):
    def test_margin_and_runner_up(self):
        s = semantics.Selection("a", {"a": 0.6, "b": 0.3, "c": 0.1}, 0.3)
        self.assertEqual(s.runner_up, "b")
        self.assertAlmostEqual(s.margin, 0.3)

    def test_single_option_margin_is_total(self):
        self.assertEqual(semantics.Selection("a", {"a": 1.0}, 1.0).margin, 1.0)


class CandidateRenderingTests(unittest.TestCase):
    def test_description_carries_what_tells_candidates_apart(self):
        text = semantics.render_candidate(CANDIDATES[0])
        self.assertIn("npc", text)
        self.assertIn("Oak", text)
        self.assertIn("(5,3)", text)
        self.assertIn("4 tiles", text)

    def test_sparse_candidate_does_not_crash(self):
        self.assertEqual(semantics.render_candidate({"id": "x"}), "entity")


class JudgeTests(unittest.TestCase):
    def judge(self, *queued):
        return semantics.SemanticJudge(client=StubClient(*queued))

    def test_select_entity_returns_the_chosen_id(self):
        judge = self.judge({"target": choice_answer("npc:1",
                                                    {"npc:0": 0.1, "npc:1": 0.9,
                                                     semantics.NO_MATCH: 0.0})})
        self.assertEqual(judge.select_entity("the rival", CANDIDATES), "npc:1")
        self.assertEqual(judge.calls, 1)
        self.assertEqual((judge.input_tokens, judge.output_tokens), (7, 3))

    def test_select_entity_none_on_no_match(self):
        judge = self.judge({"target": choice_answer(
            semantics.NO_MATCH,
            {"npc:0": 0.1, "npc:1": 0.1, semantics.NO_MATCH: 0.8})})
        self.assertIsNone(judge.select_entity("buy a bicycle", CANDIDATES))

    def test_every_candidate_is_offered_plus_no_match(self):
        stub = StubClient({"target": choice_answer("npc:0",
                                                   {"npc:0": 1.0})})
        semantics.SemanticJudge(client=stub).select_entity("oak", CANDIDATES)
        criteria = stub.requests[0]["questions"]["target"].criteria
        self.assertEqual(set(criteria),
                         {"npc:0", "npc:1", semantics.NO_MATCH})

    def test_no_candidates_skips_the_call(self):
        stub = StubClient()
        judge = semantics.SemanticJudge(client=stub)
        self.assertIsNone(judge.select_entity("oak", []))
        self.assertEqual(stub.requests, [])

    def test_conveys_returns_a_judgment(self):
        judge = self.judge({"conveys": ts.NoulAnswer(0.97)})
        verdict = judge.conveys("I came here with some friends!",
                                "the speaker arrived with others")
        self.assertTrue(verdict.holds)
        self.assertTrue(verdict.resolved)

    def test_conveys_sends_bare_text_without_context(self):
        stub = StubClient({"conveys": ts.NoulAnswer(0.9)})
        semantics.SemanticJudge(client=stub).conveys("hello", "a greeting")
        self.assertEqual(stub.requests[0]["state"], "hello")

    def test_conveys_carries_context_as_state(self):
        """A claim that depends on where the speaker is needs that fact in
        the state; the line alone does not imply it."""
        stub = StubClient({"conveys": ts.NoulAnswer(0.9)})
        semantics.SemanticJudge(client=stub).conveys(
            "I came here with some friends!", "the speaker arrived with others",
            context="the speaker is standing in ViridianForest")
        self.assertEqual(stub.requests[0]["state"],
                         {"text": "I came here with some friends!",
                          "context": "the speaker is standing in ViridianForest"})

    def test_decide_asks_both_questions_in_one_request(self):
        stub = StubClient({
            "target": choice_answer("npc:0", {"npc:0": 0.9,
                                              semantics.NO_MATCH: 0.1}),
            "action": choice_answer("interact_with:npc:0",
                                    {"interact_with:npc:0": 0.8,
                                     "move_to:1,1": 0.2}),
        })
        judge = semantics.SemanticJudge(client=stub)
        target, action = judge.decide(
            "talk to Oak", CANDIDATES,
            {"interact_with:npc:0": "talk to Oak", "move_to:1,1": "walk"},
            {"mode": "overworld"})
        self.assertEqual(len(stub.requests), 1)  # one call, not two
        self.assertEqual(target.choice, "npc:0")
        self.assertEqual(action.choice, "interact_with:npc:0")

    def test_decide_omits_absent_question_kinds(self):
        stub = StubClient({"target": choice_answer("npc:0", {"npc:0": 1.0})})
        judge = semantics.SemanticJudge(client=stub)
        judge.decide("talk to Oak", CANDIDATES, {}, {"mode": "overworld"})
        self.assertEqual(set(stub.requests[0]["questions"]), {"target"})

    def test_api_failure_degrades_to_none_and_is_recorded(self):
        judge = self.judge(ts.TypeSafeError("HTTP 500: boom"))
        self.assertIsNone(judge.conveys("hi", "a greeting"))
        self.assertEqual(judge.calls, 0)
        self.assertEqual(len(judge.errors), 1)
        self.assertIn("TypeSafeError", judge.errors[0])

    def test_credential_failure_degrades_to_none(self):
        judge = self.judge(ts.CredentialError("no key"))
        self.assertIsNone(judge.select_entity("oak", CANDIDATES))
        self.assertEqual(len(judge.errors), 1)

    def test_missing_answer_key_degrades(self):
        judge = self.judge({})
        self.assertIsNone(judge.conveys("hi", "a greeting"))


# ── LocalExplorer integration ─────────────────────────────────────────
class FakeNearbyClient:
    def __init__(self, entities):
        self.entities = entities
        self.calls = 0

    def nearby(self, radius=None):
        self.calls += 1
        return {"entities": self.entities}


class FakeEnv:
    def __init__(self, entities):
        self.client = FakeNearbyClient(entities)


TASK_NO_HINT = {"id": "find-the-forest-npc", "name": "Hear the forest NPC's story",
                "goal": {"type": "flag", "id": "EVENT_X"}}


class ExplorerIntegrationTests(unittest.TestCase):
    def test_baseline_makes_no_extra_protocol_call(self):
        """Without a judge the policy must not even ask for nearby
        entities when no rule applies — that is the RQ1 baseline."""
        env = FakeEnv(CANDIDATES)
        self.assertIsNone(
            policies.LocalExplorer(seed=1)._scan_target(env, TASK_NO_HINT))
        self.assertEqual(env.client.calls, 0)

    def test_hint_rule_still_wins_and_skips_the_judgment(self):
        stub = StubClient()
        explorer = policies.LocalExplorer(seed=1,
                                          judge=semantics.SemanticJudge(client=stub))
        task = {"id": "talk-to-oak", "name": "Hear Oak's offer",
                "goal": {"type": "flag", "id": "E"}}
        self.assertEqual(explorer._scan_target(FakeEnv(CANDIDATES), task)["id"],
                         "npc:0")
        self.assertEqual(stub.requests, [], "hint hit must not call the model")
        self.assertEqual(explorer.semantic_calls, 0)

    def test_semantic_fallback_resolves_a_goal_the_table_never_had(self):
        stub = StubClient({"target": choice_answer("npc:1",
                                                   {"npc:0": 0.2, "npc:1": 0.7,
                                                    semantics.NO_MATCH: 0.1})})
        explorer = policies.LocalExplorer(seed=1,
                                          judge=semantics.SemanticJudge(client=stub))
        got = explorer._scan_target(FakeEnv(CANDIDATES), TASK_NO_HINT)
        self.assertEqual(got["id"], "npc:1")
        self.assertEqual(explorer.semantic_calls, 1)
        self.assertEqual(explorer.semantic_hits, 1)

    def test_semantic_fallback_reports_no_match_as_no_target(self):
        stub = StubClient({"target": choice_answer(
            semantics.NO_MATCH,
            {"npc:0": 0.1, "npc:1": 0.1, semantics.NO_MATCH: 0.8})})
        explorer = policies.LocalExplorer(seed=1,
                                          judge=semantics.SemanticJudge(client=stub))
        self.assertIsNone(
            explorer._scan_target(FakeEnv(CANDIDATES), TASK_NO_HINT))
        self.assertEqual(explorer.semantic_calls, 1)
        self.assertEqual(explorer.semantic_hits, 0)

    def test_uninteractable_entities_are_not_offered(self):
        stub = StubClient({"target": choice_answer("npc:0", {"npc:0": 1.0})})
        explorer = policies.LocalExplorer(seed=1,
                                          judge=semantics.SemanticJudge(client=stub))
        hidden = dict(CANDIDATES[0], interactable=False)
        explorer._scan_target(FakeEnv([hidden, CANDIDATES[1]]), TASK_NO_HINT)
        criteria = stub.requests[0]["questions"]["target"].criteria
        self.assertNotIn("npc:0", criteria)
        self.assertIn("npc:1", criteria)

    def test_task_id_used_when_there_is_no_name(self):
        """A task with no `name` still reaches the model — it falls back
        to the id rather than silently skipping the judgment."""
        stub = StubClient({"target": choice_answer("npc:0", {"npc:0": 1.0,
                                                             "npc:1": 0.0})})
        explorer = policies.LocalExplorer(seed=1,
                                          judge=semantics.SemanticJudge(client=stub))
        explorer._scan_target(FakeEnv(CANDIDATES),
                              {"id": "find-the-professor",
                               "goal": {"type": "flag", "id": "E"}})
        self.assertEqual(stub.requests[0]["state"], "find-the-professor")


if __name__ == "__main__":
    unittest.main()
