"""Unit checks for openpoke LLM adapter (WP1) — mock HTTP only, no real
API calls, no game spawns. Follows the scripts/test_openpoke_*.py pattern.
"""
import json
import sys
import unittest
import urllib.error
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from openpoke import llm_agent as la


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


def chat_payload(text, prompt=11, completion=3):
    return {"choices": [{"message": {"content": text}}],
            "usage": {"prompt_tokens": prompt, "completion_tokens": completion}}


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


# ── credentials ───────────────────────────────────────────────────────
class CredentialTests(unittest.TestCase):
    def test_openai_first(self):
        base, key, source = la.resolve_credentials(
            {"OPENAI_BASE_URL": "http://v.example/v1/",
             "OPENAI_API_KEY": "k1", "HF_TOKEN": "hf2"})
        self.assertEqual((base, key, source),
                         ("http://v.example/v1", "k1", "openai"))

    def test_hf_fallback(self):
        base, key, source = la.resolve_credentials({"HF_TOKEN": "hf2"})
        self.assertEqual((base, key, source),
                         (la.HF_BASE_URL, "hf2", "huggingface"))

    def test_openai_requires_both(self):
        base, key, source = la.resolve_credentials(
            {"OPENAI_BASE_URL": "http://v.example/v1", "HF_TOKEN": "hf2"})
        self.assertEqual(source, "huggingface")

    def test_missing_raises(self):
        with self.assertRaises(la.CredentialError):
            la.resolve_credentials({})

    def test_model_env_override(self):
        self.assertEqual(la.default_model({}), la.DEFAULT_MODEL)
        self.assertEqual(la.default_model({la.MODEL_ENV: "x/y"}), "x/y")


# ── ChatClient ────────────────────────────────────────────────────────
class ChatClientTests(unittest.TestCase):
    def client(self, opener, **kw):
        return la.ChatClient("http://v", "k", "m", opener=opener, **kw)

    def test_success_parses_text_and_usage(self):
        opener = ScriptedOpener([chat_payload("hello", 17, 5)])
        result = self.client(opener).chat([{"role": "user", "content": "hi"}])
        self.assertEqual(result.text, "hello")
        self.assertEqual((result.prompt_tokens, result.completion_tokens), (17, 5))
        self.assertEqual(len(opener.calls), 1)
        body = json.loads(opener.calls[0].data)
        self.assertEqual(body["model"], "m")

    def test_429_retried_then_success(self):
        opener = ScriptedOpener([http_error(429), chat_payload("up")])
        result = self.client(opener).chat([])
        self.assertEqual(result.text, "up")
        self.assertEqual(len(opener.calls), 2)

    def test_400_not_retried(self):
        opener = ScriptedOpener([http_error(400)])
        with self.assertRaises(la.LlmError):
            self.client(opener).chat([])
        self.assertEqual(len(opener.calls), 1)

    def test_network_error_exhausts_retries(self):
        opener = ScriptedOpener([TimeoutError("t"), TimeoutError("t"),
                                 TimeoutError("t")])
        with self.assertRaises(la.LlmError):
            self.client(opener, max_retries=1).chat([])
        self.assertEqual(len(opener.calls), 2)


# ── action parsing ────────────────────────────────────────────────────
class ParseTests(unittest.TestCase):
    def test_t1_forms(self):
        self.assertEqual(la.parse_t1_action("up"), "drive:up,4")
        self.assertEqual(la.parse_t1_action("ACTION: up x8"), "drive:up,8")
        self.assertEqual(la.parse_t1_action("left x3"), "drive:left,4")
        self.assertEqual(la.parse_t1_action("a"), "press:a")
        self.assertEqual(la.parse_t1_action("ACTION: A"), "press:a")
        self.assertEqual(la.parse_t1_action("start"), "press:start")
        self.assertEqual(la.parse_t1_action("b x20"), "drive:b,16")  # clamped

    def test_t1_rejects_prose(self):
        for bad in ("", "I think we should go up", "move north please"):
            with self.assertRaises(la.ParseFailure):
                la.parse_t1_action(bad)

    def test_t2_forms(self):
        self.assertEqual(la.parse_t2_action("move_to:10,6"), "move_to:10,6")
        self.assertEqual(la.parse_t2_action("ACTION: move_to: 10, 6"), "move_to:10,6")
        self.assertEqual(la.parse_t2_action("travel_to:ViridianCity"),
                         "travel_to:ViridianCity")
        self.assertEqual(la.parse_t2_action("interact_with:npc:0"),
                         "interact_with:npc:0")
        self.assertEqual(la.parse_t2_action("interact"), "interact")
        self.assertEqual(la.parse_t2_action("press:a"), "press:a")
        self.assertEqual(la.parse_t2_action("step_frames:30"), "step_frames:30")
        self.assertEqual(la.parse_t2_action("step_frames:9999"), "step_frames:600")

    def test_t2_rejects_prose_and_bad_verbs(self):
        for bad in ("", "go north", "warp_to:Oz", "press:q"):
            with self.assertRaises(la.ParseFailure):
                la.parse_t2_action(bad)


# ── policy decision loop ──────────────────────────────────────────────
def obs(mode="overworld", x=10, y=10, map_name="PalletTown"):
    return {"mode": mode, "position": {"x": x, "y": y},
            "map": {"name": map_name}, "facing": "Up"}


class FakeEnvClient:
    def observe(self):
        return obs()

    def nearby(self, radius=None):
        return {"entities": []}

    def skip_dialogue(self):
        return {}

    def route(self, a, b):
        return {"legs": [{"to_map": "Route1"}, {"to_map": "ViridianCity"}]}


class FakeEnv:
    """One decision then done+success."""

    def __init__(self):
        self.client = FakeEnvClient()
        self.actions = []
        self.env_steps = 0
        self.invalid_actions = 0
        self.frames = 0

    def frame_count(self):
        return self.frames

    def step(self, action):
        self.actions.append(action)
        self.env_steps += 1
        self.frames += 10
        done = self.env_steps >= 1
        return (self.client.observe(), {"done": done, "success": done},
                {"invalid": False, "result": {"result": "reached"}})


class StubChat:
    """Pre-canned replies; same accounting shape as ChatClient."""

    def __init__(self, texts):
        self.texts = list(texts)
        self.calls = 0

    def chat(self, messages, max_tokens=64):
        self.calls += 1
        return la.ChatResult(self.texts.pop(0), 5, 2)


TASK = {"id": "reach-viridian-city", "name": "Reach Viridian City",
        "goal": {"type": "map", "id": "ViridianCity"}}


class PolicyDecisionTests(unittest.TestCase):
    def test_valid_first_reply_no_retry(self):
        env = FakeEnv()
        chat = StubChat(["move_to:10,6"])
        agent = la.SkillLlmAgent(chat, 42)
        ok, reason = agent.run(env, TASK, frame_budget=100)
        self.assertTrue(ok)
        self.assertEqual(env.actions, ["move_to:10,6"])
        self.assertEqual(agent.parse_failures, 0)
        self.assertEqual(agent.degraded_actions, 0)
        self.assertEqual(agent.model_calls, 1)
        self.assertEqual((agent.prompt_tokens, agent.completion_tokens), (5, 2))

    def test_invalid_then_corrective_retry(self):
        env = FakeEnv()
        chat = StubChat(["let me think about this", "travel_to:ViridianCity"])
        agent = la.SkillLlmAgent(chat, 42)
        ok, _ = agent.run(env, TASK, frame_budget=100)
        self.assertTrue(ok)
        self.assertEqual(env.actions, ["travel_to:ViridianCity"])
        self.assertEqual(agent.parse_failures, 1)
        self.assertEqual(agent.degraded_actions, 0)
        self.assertEqual(agent.model_calls, 2)

    def test_double_invalid_degrades_to_fallback(self):
        env = FakeEnv()
        chat = StubChat(["hmm", "still thinking"])
        agent = la.SkillLlmAgent(chat, 42)
        ok, _ = agent.run(env, TASK, frame_budget=100)
        self.assertTrue(ok)  # the fallback action itself executed
        self.assertEqual(agent.parse_failures, 2)
        self.assertEqual(agent.degraded_actions, 1)
        self.assertEqual(env.actions, ["move_to:10,6"])  # T2 fallback: north
        self.assertEqual(agent.model_calls, 2)

    def test_t1_buttons_only(self):
        env = FakeEnv()
        chat = StubChat(["up x6"])
        agent = la.ButtonLlmAgent(chat, 42)
        ok, _ = agent.run(env, TASK, frame_budget=100)
        self.assertTrue(ok)
        self.assertEqual(env.actions, ["drive:up,6"])

    def test_model_call_cap(self):
        env = FakeEnv()
        env.step = lambda action: (  # never done → loop until cap
            env.client.observe(), {"done": False, "success": False},
            {"invalid": False, "result": {"result": "reached"}})
        chat = StubChat(["interact"] * 10)
        agent = la.SkillLlmAgent(chat, 42, max_model_calls=2)
        ok, reason = agent.run(env, TASK, frame_budget=10000)
        self.assertFalse(ok)
        self.assertEqual(reason, "LlmError: model_call_cap")
        self.assertEqual(agent.model_calls, 2)

    def test_t3_prompt_includes_world_route(self):
        env = FakeEnv()
        chat = StubChat(["travel_to:Route1"])
        agent = la.WorldModelLlmAgent(chat, 42)
        agent.run(env, TASK, frame_budget=100)
        user_msg = chat and agent.messages[1]["content"]
        self.assertIn("World graph route: PalletTown → Route1 → ViridianCity",
                      user_msg)


if __name__ == "__main__":
    unittest.main()
