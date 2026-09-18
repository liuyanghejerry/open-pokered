"""Unit checks for the judgment-driven policy — stubs only, no network, no
key, no game spawns.

Run: python3 -m unittest scripts.test_openpokered_judgment
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from openpokered import judgment_agent as ja
from openpokered.semantics import NO_MATCH, Selection


# ── stubs ─────────────────────────────────────────────────────────────
class StubClient:
    def __init__(self, entities=(), edges=()):
        self.entities = list(entities)
        self.edges = list(edges)
        self.nearby_calls = 0
        self.skipped = 0

    def observe(self):
        return OBS

    def skip_dialogue(self):
        self.skipped += 1
        return {"skipped": True}

    def script_semantics(self, map_name=None):
        """Empty by default: an index build over no maps yields no facts,
        which is the state the annotation tests override explicitly."""
        return {"maps": []} if map_name is None else {"storylines": []}

    def bag(self):
        return [{"item": "POKE_BALL", "qty": 5}, {"item": "POTION", "qty": 2}]

    def nearby(self, radius=None):
        self.nearby_calls += 1
        return {"entities": self.entities}

    def world_graph(self):
        return {"edges": self.edges}


class StubJudge:
    enabled = True

    def __init__(self, pick=None):
        self.pick = pick
        self.requests = []

    def route_action(self, goal, actions, observation):
        self.requests.append({"goal": goal, "actions": dict(actions),
                              "observation": observation})
        if self.pick is None:
            return None
        return Selection(self.pick, {self.pick: 0.9}, 0.9)


def edge(src, dst, kind="warp"):
    return {"from_map": src, "to_map": dst, "kind": kind}


# Mirrors the real `get_agent_state` shape, including `party` as a list
# of members — a stub that omits a field cannot catch a wrong read of it.
OBS = {"map": {"name": "PalletTown"}, "position": {"x": 10, "y": 6},
       "mode": "overworld", "party": [], "badges": {"count": 0}}

ENTITIES = [
    {"id": "npc:0", "kind": "npc", "name": "Oak", "interactable": True,
     "position": {"x": 8, "y": 5}, "distance": 13},
    {"id": "sign:1", "kind": "sign", "interactable": True,
     "position": {"x": 7, "y": 9}, "distance": 16},
    {"id": "npc:9", "kind": "npc", "name": "Hidden", "interactable": False},
]

EDGES = [
    edge("PalletTown", "Route1", "connection"),
    edge("PalletTown", "OaksLab"),
    edge("PalletTown", "RedsHouse1F"),
    edge("RedsHouse1F", "RedsHouse2F"),          # 2 hops — out of budget 1
    edge("Route1", "ViridianCity", "connection"),
    {"from_map": "PalletTown", "to_map": None, "kind": "warp",
     "dynamic_destination": True},               # LAST_MAP: no resolvable dst
]


# ── candidate places ──────────────────────────────────────────────────
class ReachablePlacesTests(unittest.TestCase):
    def test_one_hop_from_origin(self):
        got = {p["id"] for p in ja.reachable_places(EDGES, "PalletTown", 1)}
        self.assertEqual(got, {"Route1", "OaksLab", "RedsHouse1F"})

    def test_origin_is_never_a_candidate(self):
        got = {p["id"] for p in ja.reachable_places(EDGES, "PalletTown", 2)}
        self.assertNotIn("PalletTown", got)

    def test_budget_widens_the_set(self):
        one = {p["id"] for p in ja.reachable_places(EDGES, "PalletTown", 1)}
        two = {p["id"] for p in ja.reachable_places(EDGES, "PalletTown", 2)}
        self.assertLess(one, two)
        self.assertIn("RedsHouse2F", two)
        self.assertIn("ViridianCity", two)

    def test_unresolved_destination_is_skipped(self):
        """A LAST_MAP warp has no static destination; offering a null id
        would let the model pick something unrunnable."""
        got = [p["id"] for p in ja.reachable_places(EDGES, "PalletTown", 2)]
        self.assertNotIn(None, got)

    def test_places_describe_how_they_are_reached(self):
        places = {p["id"]: p for p in ja.reachable_places(EDGES, "PalletTown", 1)}
        self.assertEqual(places["OaksLab"]["kind"], "building")
        self.assertEqual(places["Route1"]["kind"], "route")
        self.assertIn("1 map(s) away", places["Route1"]["note"])

    def test_result_is_capped(self):
        wide = [edge("A", f"M{i}") for i in range(ja.MAX_CANDIDATE_PLACES + 20)]
        self.assertEqual(len(ja.reachable_places(wide, "A", 1)),
                         ja.MAX_CANDIDATE_PLACES)

    def test_the_cap_keeps_the_nearest_places(self):
        """Widening the search must not push the close options out of the
        candidate set — the cap is applied after the distance sort."""
        got = {p["id"] for p in ja.reachable_places(EDGES, "PalletTown", 2,
                                                    limit=2)}
        self.assertEqual(got, {"OaksLab", "RedsHouse1F"})

    def test_visited_is_reported_as_a_fact(self):
        places = {p["id"]: p for p in ja.reachable_places(
            EDGES, "PalletTown", 1, visited={"OaksLab"})}
        self.assertIn("already visited", places["OaksLab"]["note"])
        self.assertIn("not visited yet", places["Route1"]["note"])

    def test_visited_defaults_to_nothing_known(self):
        places = ja.reachable_places(EDGES, "PalletTown", 1)
        self.assertTrue(all("not visited yet" in p["note"] for p in places))

    def test_empty_graph_yields_nothing(self):
        self.assertEqual(ja.reachable_places([], "Nowhere", 2), [])


# ── action vocabulary ─────────────────────────────────────────────────
class BuildActionsTests(unittest.TestCase):
    def test_keys_are_executable_action_strings(self):
        actions = ja.build_actions(ENTITIES, ja.reachable_places(EDGES, "PalletTown", 1))
        self.assertIn("interact_with:npc:0", actions)
        self.assertIn("travel_to:OaksLab", actions)

    def test_non_adjacent_entities_are_offered(self):
        """`interact_with` navigates to an id, so being far away is not a
        reason to withhold an entity. Filtering on adjacency is what hid
        the gym leader across the room."""
        actions = ja.build_actions(ENTITIES, [])
        self.assertIn("interact_with:npc:0", actions)
        self.assertIn("13 tiles away", actions["interact_with:npc:0"])

    def test_warps_are_not_interaction_targets(self):
        """A warp is a destination. `interact_with` on one only resolves
        from its own tile, so it is a no-op that costs a decision — the
        action that made the agent shuttle in and out of Pewter Gym."""
        warps = [{"id": "warp:0", "kind": "warp", "name": "PewterCity",
                  "position": {"x": 4, "y": 13}, "distance": 0,
                  "interactable": True}]
        self.assertEqual(ja.build_actions(warps, []), {})

    def test_an_entity_without_a_position_cannot_be_an_action(self):
        self.assertNotIn("interact_with:npc:9", ja.build_actions(ENTITIES, []))

    def test_a_far_trainer_is_still_a_candidate(self):
        """The concrete shape of the gym bug: a trainer twelve tiles away
        with `interactable` false must still be offered."""
        trainers = [{"id": "npc:3", "kind": "trainer", "name": "Brock",
                     "position": {"x": 4, "y": 2}, "distance": 11,
                     "interactable": False}]
        self.assertIn("interact_with:npc:3", ja.build_actions(trainers, []))

    def test_descriptions_are_human_readable(self):
        actions = ja.build_actions(ENTITIES, [])
        self.assertIn("Oak", actions["interact_with:npc:0"])

    def test_no_candidates_means_no_actions(self):
        self.assertEqual(ja.build_actions([], []), {})


# ── the policy ────────────────────────────────────────────────────────
def agent_with(pick, entities=ENTITIES, edges=EDGES, **kw):
    judge = StubJudge(pick)
    return ja.JudgmentAgent(judge, **kw), judge, StubClient(entities, edges)


def retire(agent, action, map_name="PalletTown"):
    """Mark one action as already-proven-useless on a map (OBS's map).

    Retirement is per map, so the key is the pair — the same action
    string on another map is a different situation and stays available.
    """
    agent._failed.add((map_name, action))


def retire_all(agent, actions, map_name="PalletTown"):
    for action in actions:
        retire(agent, action, map_name)


class DecideTests(unittest.TestCase):
    def test_the_judged_action_is_returned(self):
        agent, judge, client = agent_with("travel_to:OaksLab")
        self.assertEqual(agent.decide(client, OBS, "Hear Oak's offer"),
                         "travel_to:OaksLab")
        self.assertEqual(agent.judgments, 1)
        self.assertEqual(agent.fallbacks, 0)

    def test_the_goal_text_is_what_gets_asked(self):
        agent, judge, client = agent_with("travel_to:OaksLab")
        agent.decide(client, OBS, "Hear Oak's starter offer in his lab")
        self.assertEqual(judge.requests[0]["goal"],
                         "Hear Oak's starter offer in his lab")

    def test_a_populated_party_is_reported_as_a_size(self):
        """`get_agent_state` returns party as a list of members. Reading
        it as a dict crashes on every task that has a party, and silently
        passes on the ones that don't — so cover the populated case."""
        agent, judge, client = agent_with("travel_to:OaksLab")
        obs = dict(OBS, party=[{"species": "Charizard", "level": 100}])
        self.assertEqual(agent.decide(client, obs, "goal"), "travel_to:OaksLab")
        seen = judge.requests[0]["observation"]["party"]
        self.assertEqual(len(seen), 1)
        self.assertEqual(seen[0]["species"], "Charizard")

    def test_only_executable_actions_are_offered(self):
        agent, judge, client = agent_with("travel_to:OaksLab")
        agent.decide(client, OBS, "goal")
        offered = set(judge.requests[0]["actions"])
        self.assertTrue(offered <= set(ja.build_actions(ENTITIES, agent._places(client, OBS))))
        for action in offered:
            self.assertTrue(action.startswith(("interact_with:", "travel_to:")))

    def test_failed_actions_are_not_re_offered(self):
        agent, judge, client = agent_with("travel_to:OaksLab")
        agent.decide(client, OBS, "goal")
        retire(agent, "travel_to:OaksLab")
        agent.decide(client, OBS, "goal")
        self.assertNotIn("travel_to:OaksLab", judge.requests[1]["actions"])

    def test_a_failure_on_one_map_does_not_retire_it_elsewhere(self):
        """Retirement is keyed by map. Clearing it on every arrival
        re-armed every failure for an agent that oscillates between two
        maps, so it retried the same dead action forever — which is what
        it did between Pewter Gym and Pewter City."""
        agent, _judge, client = agent_with("travel_to:OaksLab")
        retire(agent, "travel_to:OaksLab", map_name="PewterGym")
        self.assertIn("travel_to:OaksLab", agent.actions_for(client, OBS))

    def test_a_transport_failure_falls_back_deterministically(self):
        agent, _judge, client = agent_with(None)
        first = agent.decide(client, OBS, "goal")
        self.assertEqual(agent.fallbacks, 1)
        self.assertEqual(first, "interact_with:npc:0")  # talks before wandering
        self.assertEqual(agent.decide(client, OBS, "goal"), first)

    def test_an_unoffered_answer_is_refused(self):
        """The model can only pick from the list — anything else is a bug
        or a hallucination, and must not reach env.step()."""
        agent, _judge, client = agent_with("travel_to:CeladonCity")
        self.assertEqual(agent.decide(client, OBS, "goal"), "interact_with:npc:0")
        self.assertEqual(agent.fallbacks, 1)

    def test_no_candidates_yields_no_action(self):
        agent, _judge, client = agent_with("x", entities=[], edges=[])
        self.assertIsNone(agent.decide(client, OBS, "goal"))
        self.assertEqual(agent.judgments, 0)

    def test_everything_failed_at_every_radius_yields_no_action(self):
        # Pinning max_hop_budget to 1 holds the radius still, so retiring
        # every action at that radius really does leave nothing to try. At
        # a wider cap the policy escalates instead — see the escalation
        # tests, which is the behaviour that replaced this dead end.
        agent, _judge, client = agent_with("x", max_hop_budget=1)
        retire_all(agent, ja.build_actions(ENTITIES, agent._places(client, OBS)))
        self.assertIsNone(agent.decide(client, OBS, "goal"))

    def test_judgment_cap_is_respected(self):
        agent, judge, client = agent_with("travel_to:OaksLab", max_judgments=2)
        agent.decide(client, OBS, "goal")
        agent.decide(client, OBS, "goal")
        self.assertIsNone(agent.decide(client, OBS, "goal"))
        self.assertEqual(agent.judgments, 2)

    def test_disabled_judge_uses_the_fallback_only(self):
        judge = StubJudge("travel_to:OaksLab")
        judge.enabled = False
        agent = ja.JudgmentAgent(judge)
        self.assertEqual(agent.decide(StubClient(ENTITIES, EDGES), OBS, "goal"),
                         "interact_with:npc:0")
        self.assertEqual(judge.requests, [])
        self.assertEqual(agent.judgments, 0)

    def test_hop_budget_widens_the_offered_places(self):
        agent, judge, client = agent_with("travel_to:OaksLab", hop_budget=2)
        agent.decide(client, OBS, "goal")
        self.assertIn("travel_to:ViridianCity", judge.requests[0]["actions"])


class ActionSpaceEscalationTests(unittest.TestCase):
    """Escalation is what stops a run ending in `no_actions_left` while
    unvisited maps sit two hops away."""

    def test_the_narrow_set_is_offered_first(self):
        agent, _judge, client = agent_with("x", entities=[], hop_budget=1,
                                           max_hop_budget=3)
        actions = set(agent.actions_for(client, OBS))
        self.assertIn("travel_to:OaksLab", actions)         # 1 hop
        self.assertNotIn("travel_to:ViridianCity", actions)  # 2 hops
        self.assertEqual(agent.escalations, 0)

    def test_the_set_widens_once_the_narrow_one_is_exhausted(self):
        agent, _judge, client = agent_with("x", entities=[], hop_budget=1,
                                           max_hop_budget=2)
        retire_all(agent, agent.actions_for(client, OBS))
        wide = set(agent.actions_for(client, OBS))
        self.assertIn("travel_to:ViridianCity", wide)
        self.assertEqual(agent.escalations, 1)

    def test_escalation_stops_at_the_cap(self):
        agent, _judge, client = agent_with("x", entities=[], hop_budget=1,
                                           max_hop_budget=1)
        retire_all(agent, agent.actions_for(client, OBS))
        self.assertEqual(agent.actions_for(client, OBS), {})

    def test_widening_does_not_resurrect_retired_actions(self):
        """The retired action stays retired at every radius — otherwise a
        failed `travel_to` would be re-offered forever."""
        agent, _judge, client = agent_with("x", entities=[], hop_budget=1,
                                           max_hop_budget=3)
        retire(agent, "travel_to:OaksLab")
        retire(agent, "travel_to:Route1")
        retire(agent, "travel_to:RedsHouse1F")
        wide = set(agent.actions_for(client, OBS))
        self.assertNotIn("travel_to:OaksLab", wide)
        self.assertIn("travel_to:ViridianCity", wide)


# ── script index: what a map's scripts do about the goal ──────────────
class StubSemanticsClient:
    """Serves one script-semantics payload per map."""

    def __init__(self, semantics):
        self.semantics = semantics
        self.calls = []

    def script_semantics(self, map_name=None):
        self.calls.append(map_name)
        if map_name is None:
            return {"maps": sorted(self.semantics)}
        return {"storylines": self.semantics.get(map_name, [])}


SEMANTICS = {
    "PalletTown": [{"storyline": "a", "effects": [
        {"kind": "flag_set", "flag": "EVENT_DAISY_WALKING"}]}],
    "OaksLab": [{"storyline": "b", "effects": [
        {"kind": "flag_set", "flag": "EVENT_OAK_ASKED_TO_CHOOSE_MON"},
        {"kind": "item_given", "item": "POKE_BALL", "qty": 5},
        {"kind": "player_moved"}]}],
}


class ScriptIndexTests(unittest.TestCase):
    def index(self, semantics=SEMANTICS):
        return ja.ScriptIndex(StubSemanticsClient(semantics))

    def test_indexes_flags_by_map(self):
        idx = self.index()
        self.assertEqual(
            idx.facts("OaksLab", {"type": "flag",
                                  "id": "EVENT_OAK_ASKED_TO_CHOOSE_MON"}),
            ["a script here sets EVENT_OAK_ASKED_TO_CHOOSE_MON"])

    def test_indexes_items_by_map(self):
        self.assertEqual(
            self.index().facts("OaksLab", {"type": "item", "id": "POKE_BALL"}),
            ["a script here gives POKE_BALL"])

    def test_a_map_that_does_nothing_about_the_goal_has_no_fact(self):
        self.assertEqual(
            self.index().facts("PalletTown",
                               {"type": "flag", "id": "EVENT_NOT_HERE"}), [])

    def test_goal_kinds_the_index_cannot_answer_have_no_facts(self):
        idx = self.index()
        self.assertEqual(idx.facts("OaksLab", {"type": "map", "id": "OaksLab"}), [])
        self.assertEqual(idx.facts("OaksLab", {}), [])
        self.assertEqual(idx.facts("OaksLab", None), [])

    def test_effects_that_are_not_flags_or_items_are_ignored(self):
        idx = self.index()
        self.assertIsNone(idx._by_flag.get("player_moved"))
        self.assertNotIn("player_moved", idx._by_flag)

    def test_the_index_is_built_once(self):
        client = StubSemanticsClient(SEMANTICS)
        idx = ja.ScriptIndex(client)
        idx.facts("OaksLab", {"type": "flag", "id": "EVENT_X"})
        after_first = len(client.calls)
        idx.facts("PalletTown", {"type": "item", "id": "POKE_BALL"})
        self.assertEqual(len(client.calls), after_first)

    def test_a_failed_build_is_not_retried(self):
        class Broken(StubSemanticsClient):
            def script_semantics(self, map_name=None):
                raise RuntimeError("boom")

        idx = ja.ScriptIndex(Broken(SEMANTICS))
        self.assertEqual(idx.facts("OaksLab", {"type": "flag", "id": "EVENT_X"}), [])
        self.assertIn("boom", idx.error)
        self.assertEqual(idx.facts("OaksLab", {"type": "flag", "id": "EVENT_X"}), [])

    def test_one_unreadable_map_does_not_lose_the_index(self):
        class Flaky(StubSemanticsClient):
            def script_semantics(self, map_name=None):
                if map_name == "PalletTown":
                    raise RuntimeError("unreadable")
                return super().script_semantics(map_name)

        idx = ja.ScriptIndex(Flaky(SEMANTICS))
        self.assertEqual(
            idx.facts("OaksLab", {"type": "item", "id": "POKE_BALL"}),
            ["a script here gives POKE_BALL"])


class PlaceAnnotationTests(unittest.TestCase):
    """The facts reach the model as candidate descriptions, and the switch
    that removes them is what makes the contribution measurable."""

    @staticmethod
    def client_with_lab():
        client = StubClient([], EDGES)

        def semantics(map_name=None):
            if map_name is None:
                return {"maps": ["OaksLab"]}
            return {"storylines": [{"effects": [
                {"kind": "flag_set", "flag": "EVENT_GOT_STARTER"}]}]}
        client.script_semantics = semantics
        return client

    def description_of(self, place_facts):
        judge = StubJudge("travel_to:OaksLab")
        agent = ja.JudgmentAgent(judge, place_facts=place_facts)
        agent.goal_spec = {"type": "flag", "id": "EVENT_GOT_STARTER"}
        agent.decide(self.client_with_lab(), OBS, "get a starter")
        return judge.requests[0]["actions"]["travel_to:OaksLab"]

    def test_the_fact_reaches_the_description(self):
        self.assertIn("a script here sets EVENT_GOT_STARTER",
                      self.description_of(True))

    def test_the_ablation_leaves_it_out(self):
        self.assertNotIn("EVENT_GOT_STARTER", self.description_of(False))

    def test_without_a_typed_goal_there_is_nothing_to_look_up(self):
        judge = StubJudge("travel_to:OaksLab")
        agent = ja.JudgmentAgent(judge, place_facts=True)
        agent.decide(self.client_with_lab(), OBS, "a goal in prose only")
        self.assertNotIn("EVENT_GOT_STARTER",
                         judge.requests[0]["actions"]["travel_to:OaksLab"])


# ── the state the judgment sees ───────────────────────────────────────
PARTY_OBS = dict(OBS, party=[
    {"species": "Charizard", "level": 36, "hp": 98, "max_hp": 120,
     "status": "Poison"},
    {"species": "Pikachu", "level": 12, "hp": 30, "max_hp": 30,
     "status": "None"},
])


class StoryStateTests(unittest.TestCase):
    def state(self, obs=PARTY_OBS, bag=None, **kw):
        client = StubClient()
        if bag is not None:
            client.bag = bag if callable(bag) else (lambda: bag)
        kw.setdefault("visited", {"PalletTown", "Route1"})
        kw.setdefault("failed_here", ["interact_with:npc:0"])
        return ja.story_state(client, obs, "Defeat Brock", {"type": "flag",
                                                            "id": "EVENT_BEAT_BROCK"},
                              **kw)

    def test_the_goal_and_its_typed_spec_are_both_present(self):
        s = self.state()
        self.assertEqual(s["goal"], "Defeat Brock")
        self.assertEqual(s["goal_spec"], {"type": "flag", "id": "EVENT_BEAT_BROCK"})

    def test_the_party_is_state_not_a_count(self):
        """A player deciding what to do next looks at who is hurt and how
        badly; a party *count* cannot answer that."""
        party = self.state()["party"]
        self.assertEqual(len(party), 2)
        self.assertEqual(party[0]["species"], "Charizard")
        self.assertEqual(party[0]["hp"], 98)
        self.assertEqual(party[0]["max_hp"], 120)
        self.assertEqual(party[0]["status"], "Poison")

    def test_the_bag_is_state(self):
        self.assertEqual(self.state()["bag"], [{"item": "POKE_BALL", "qty": 5},
                                               {"item": "POTION", "qty": 2}])

    def test_the_bag_accepts_a_mapping_shape_too(self):
        s = self.state(bag={"POTION": 2})
        self.assertEqual(s["bag"], [{"item": "POTION", "qty": 2}])

    def test_an_unreadable_bag_does_not_lose_the_state(self):
        def boom():
            raise RuntimeError("no bag")
        s = self.state(bag=boom)
        self.assertNotIn("bag", s)
        self.assertIn("party", s)

    def test_where_it_has_been_and_what_it_tried_are_state(self):
        s = self.state()
        self.assertEqual(s["maps_already_visited"], ["PalletTown", "Route1"])
        self.assertEqual(s["actions_already_failed_here"], ["interact_with:npc:0"])

    def test_long_lists_are_bounded(self):
        obs = dict(OBS, party=[{"species": f"M{i}", "level": 1, "hp": 1,
                                "max_hp": 1, "status": "None"} for i in range(20)])
        self.assertEqual(len(self.state(obs=obs)["party"]), ja.MAX_PARTY_SHOWN)
        big = [{"item": f"I{i}", "qty": 1} for i in range(40)]
        self.assertEqual(len(self.state(bag=big)["bag"]), ja.MAX_BAG_SHOWN)


class DecisionTests(unittest.TestCase):
    def selection(self, probs):
        pick = max(probs, key=probs.get)
        return Selection(pick, probs, max(probs.values()))

    def test_the_whole_distribution_is_kept(self):
        """The model is asked for a judgment, so how the mass is spread is
        part of the answer — not just the winner."""
        d = ja.Decision(self.selection({"a": 0.6, "b": 0.3, "c": 0.1}))
        self.assertEqual(d.probabilities, {"a": 0.6, "b": 0.3, "c": 0.1})
        self.assertEqual(d.action, "a")
        self.assertEqual(d.runner_up, "b")

    def test_a_wide_margin_is_decisive(self):
        self.assertTrue(ja.Decision(self.selection({"a": 0.9, "b": 0.1}),
                                    act_margin=0.2).decisive)

    def test_a_near_tie_is_not_decisive(self):
        self.assertFalse(ja.Decision(self.selection({"a": 0.51, "b": 0.49}),
                                     act_margin=0.2).decisive)

    def test_the_gate_is_off_by_default(self):
        self.assertTrue(ja.Decision(self.selection({"a": 0.51, "b": 0.49})).decisive)

    def test_the_agent_declines_to_act_on_a_tie(self):
        agent, _judge, client = agent_with("travel_to:OaksLab", act_margin=0.5)
        judge = StubJudge("travel_to:OaksLab")
        judge.route_action = lambda g, a, o: Selection(
            "travel_to:OaksLab", {"travel_to:OaksLab": 0.51,
                                  "travel_to:Route1": 0.49}, 0.02)
        agent.judge = judge
        got = agent.decide(client, OBS, "goal")
        self.assertEqual(agent.undecided, 1)
        self.assertNotEqual(got, "travel_to:OaksLab")   # deterministic instead
        self.assertEqual(got, "interact_with:npc:0")

    def test_the_agent_acts_when_the_margin_clears(self):
        judge = StubJudge("travel_to:OaksLab")
        agent = ja.JudgmentAgent(judge, act_margin=0.2)
        self.assertEqual(agent.decide(StubClient(ENTITIES, EDGES), OBS, "goal"),
                         "travel_to:OaksLab")
        self.assertEqual(agent.undecided, 0)


class ObservationShapeTests(unittest.TestCase):
    def test_the_rich_state_reaches_the_judgment(self):
        judge = StubJudge("travel_to:OaksLab")
        agent = ja.JudgmentAgent(judge, rich_state=True)
        agent.decide(StubClient(ENTITIES, EDGES), PARTY_OBS, "goal")
        obs = judge.requests[0]["observation"]
        self.assertEqual(obs["party"][0]["species"], "Charizard")
        self.assertIn("bag", obs)

    def test_the_thin_ablation_sends_what_it_used_to(self):
        judge = StubJudge("travel_to:OaksLab")
        agent = ja.JudgmentAgent(judge, rich_state=False)
        agent.decide(StubClient(ENTITIES, EDGES), PARTY_OBS, "goal")
        obs = judge.requests[0]["observation"]
        self.assertEqual(sorted(obs), ["map", "mode", "party_size", "position"])
        self.assertEqual(obs["party_size"], 2)


# A graph where the map that satisfies a goal is nowhere near the start.
DEEP_EDGES = EDGES + [
    edge("ViridianCity", "Route2", "connection"),
    edge("Route2", "PewterCity", "connection"),
    edge("PewterCity", "PewterGym"),
]
BROCK = {"EVENT_BEAT_BROCK": {"PewterGym"}}


def semantics_client(edges, mapping):
    """A client whose script index reports `mapping` (flag -> maps)."""
    client = StubClient([], edges)

    def script_semantics(map_name=None):
        if map_name is None:
            return {"maps": sorted({m for ms in mapping.values() for m in ms})}
        return {"storylines": [{"effects": [
            {"kind": "flag_set", "flag": flag} for flag, maps in mapping.items()
            if map_name in maps]}]}
    client.script_semantics = script_semantics
    return client


class GoalPlaceTests(unittest.TestCase):
    """The index answers globally, so the candidate set has to as well.

    The first cut of this only annotated candidates inside the local hop
    radius. An ablation showed it changed no decision, because the maps
    that satisfy a goal are usually several hops out — these tests pin the
    behaviour that replaced it.
    """

    def test_a_goal_map_beyond_the_hop_radius_is_still_offered(self):
        local = {p["id"] for p in ja.reachable_places(DEEP_EDGES, "PalletTown", 1)}
        self.assertNotIn("PewterGym", local)
        idx = ja.ScriptIndex(semantics_client(DEEP_EDGES, BROCK))
        far = ja.goal_places(DEEP_EDGES, "PalletTown", idx,
                             {"type": "flag", "id": "EVENT_BEAT_BROCK"})
        self.assertEqual([p["id"] for p in far], ["PewterGym"])
        self.assertIn("5 map(s) away", far[0]["note"])

    def test_nothing_is_offered_when_the_index_cannot_answer(self):
        idx = ja.ScriptIndex(semantics_client(DEEP_EDGES, {}))
        self.assertEqual(ja.goal_places(DEEP_EDGES, "PalletTown", idx, {}), [])
        self.assertEqual(
            ja.goal_places(DEEP_EDGES, "PalletTown", idx,
                           {"type": "map", "id": "PewterCity"}), [])

    def test_a_map_off_the_graph_is_skipped(self):
        idx = ja.ScriptIndex(semantics_client(DEEP_EDGES,
                                              {"EVENT_X": {"Nowhere"}}))
        self.assertEqual(
            ja.goal_places(DEEP_EDGES, "PalletTown", idx,
                           {"type": "flag", "id": "EVENT_X"}), [])

    def test_the_agent_offers_the_goal_map_from_across_the_world(self):
        judge = StubJudge("travel_to:PewterGym")
        agent = ja.JudgmentAgent(judge, place_facts=True)
        agent.goal_spec = {"type": "flag", "id": "EVENT_BEAT_BROCK"}
        agent.decide(semantics_client(DEEP_EDGES, BROCK), OBS, "beat Brock")
        actions = judge.requests[0]["actions"]
        self.assertIn("travel_to:PewterGym", actions)
        self.assertIn("sets EVENT_BEAT_BROCK", actions["travel_to:PewterGym"])

    def test_a_local_goal_map_is_not_offered_twice(self):
        judge = StubJudge("travel_to:OaksLab")
        agent = ja.JudgmentAgent(judge, place_facts=True)
        agent.goal_spec = {"type": "flag", "id": "EVENT_OAK"}
        agent.decide(semantics_client(EDGES, {"EVENT_OAK": {"OaksLab"}}),
                     OBS, "goal")
        keys = list(judge.requests[0]["actions"])
        self.assertEqual(keys.count("travel_to:OaksLab"), 1)

    def test_the_ablation_removes_goal_maps_too(self):
        """Facts off must mean the old policy exactly, or the ablation
        measures something other than the facts."""
        judge = StubJudge("travel_to:OaksLab")
        agent = ja.JudgmentAgent(judge, place_facts=False)
        agent.goal_spec = {"type": "flag", "id": "EVENT_BEAT_BROCK"}
        agent.decide(semantics_client(DEEP_EDGES, BROCK), OBS, "beat Brock")
        self.assertNotIn("travel_to:PewterGym", judge.requests[0]["actions"])


# ── the loop ──────────────────────────────────────────────────────────
class FakeEnv:
    """Serves scripted observations in order; records the actions taken.

    `run()` takes its first observation from `client.observe()`, so the
    stub's current observation is the head of the script and `step()`
    advances it — otherwise the loop would never see a mode other than the
    one it started in.
    """

    def __init__(self, observations, done_after=None):
        self.script = list(observations)
        self.current = self.script[0]
        self.client = StubClient(ENTITIES, EDGES)
        self.client.observe = lambda: self.current
        self.taken = []
        self.frames = 0
        self.done_after = done_after

    def frame_count(self):
        return self.frames

    def step(self, action):
        self.taken.append(action)
        self.frames += 10
        if len(self.script) > 1:
            self.script.pop(0)
        self.current = self.script[0]
        done = self.done_after is not None and len(self.taken) >= self.done_after
        return self.current, {"done": done, "success": done}, {}


class RunLoopTests(unittest.TestCase):
    def test_executes_the_judged_action_then_stops_on_success(self):
        env = FakeEnv([OBS, OBS], done_after=1)
        agent = ja.JudgmentAgent(StubJudge("travel_to:OaksLab"))
        ok, reason = agent.run(env, {"id": "t", "name": "Hear Oak's offer"}, 9999)
        self.assertTrue(ok)
        self.assertEqual(env.taken, ["travel_to:OaksLab"])

    def test_dialogue_is_settled_rather_than_judged(self):
        talking = dict(OBS, mode="dialogue")
        env = FakeEnv([talking, OBS], done_after=2)
        agent = ja.JudgmentAgent(StubJudge("travel_to:OaksLab"))
        agent.run(env, {"id": "t", "name": "goal"}, 9999)
        self.assertEqual(env.taken[0], "step_frames:10")   # no judgment wasted
        self.assertEqual(agent.judgments, 1)

    def test_frame_budget_stops_the_run(self):
        env = FakeEnv([OBS] * 50)
        agent = ja.JudgmentAgent(StubJudge("travel_to:OaksLab"))
        ok, reason = agent.run(env, {"id": "t", "name": "goal"}, 25)
        self.assertFalse(ok)
        self.assertEqual(reason, "frame_budget")

    def test_a_fruitless_action_is_retired(self):
        """Observed state never changes → the action is dropped and the
        next decision cannot pick it again."""
        stuck = [OBS] * 10
        env = FakeEnv(stuck)
        agent = ja.JudgmentAgent(StubJudge("travel_to:OaksLab"))
        agent.run(env, {"id": "t", "name": "goal"}, 30)
        self.assertIn(("PalletTown", "travel_to:OaksLab"), agent._failed)
        self.assertNotIn("travel_to:OaksLab", env.taken[1:])


if __name__ == "__main__":
    unittest.main()
