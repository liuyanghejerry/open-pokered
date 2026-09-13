"""RQ2 hierarchical planner: event-graph goal decomposition → T2 execution.

Loads the M4 event graph (`crates/pokered-data/story/graph.json`, 3213
edges over storyline/flag/map/trainer/item nodes) and plans a task goal
backward from it:

1. **Goal translation.** flag goals target `flag:X`, item goals `item:X`
   (case-normalized). `map` and `battle_won` goals are not graph
   representable (map reachability is world-graph territory; wild
   encounters are map tables, not storylines) — they use small built-in
   strategies, documented as such (travel plan / grass-walk plan).
2. **Producer search.** Candidate producer storylines = edges of kind
   `sets` (flag) or `gives` (item) INTO the target. Candidates are
   ordered by world-graph route length from the current map to the
   storyline's `triggered_at` map, then by fewest `requires` reads.
3. **Prerequisites.** `requires` edges are storyline flag READS, which
   carry no polarity (the Oak lab speech requires
   EVENT_OAK_APPEARED_IN_PALLET *set* AND EVENT_FOLLOWED_OAK_INTO_LAB
   *unset* — neither direction is recoverable from the graph). v1
   therefore treats them as annotations, not gates: self-set/given flags
   (self-loops like talkBrock reading EVENT_BEAT_BROCK) are dropped, the
   rest are recorded on the plan, and the runtime *goal verification*
   after execution is the authoritative check. Recursive prerequisite
   expansion with polarity is v2.
4. **Plan execution (world-model condition: travel_to allowed).** Per
   candidate: `travel_to(triggered_at map)` → fire the trigger
   (`@load` storylines fire on entry; npc-bound storylines resolve the
   semantics npc text_id to a live entity and `interact_with`, with
   bounded re-attempts through interception battles/dialogue) →
   auto-resolve started battles with `skills.fight_current_battle` →
   verify the goal. Candidate failure falls through to the next
   candidate; total failure is a graceful `no_path:<stage>`, never a
   crash.

Ablation support: `delete_edges(rng, fraction)` returns a graph copy
with a seeded share of edges removed; the same planner runs on it and
success/no-path decay is measured per cell.

Interface matches policies.py: `run(env, task, frame_budget) ->
(success, reason)`. Planning itself makes no game calls except world
graph routes and script semantics (the T3/world-model information
class); execution uses T2 skill actions.
"""
import json
import random
from pathlib import Path

from . import skills
from .oracle import _entity_for_npc_text_id, Oracle
from .tasks import goal_satisfied

STORY_DIR = (Path(__file__).resolve().parent.parent.parent
             / "crates" / "pokered-data" / "story")

MAX_INTERACT_ATTEMPTS = 4  # interceptions (sight trainers, greeters)


class EventGraph:
    """The M4 event graph, indexed for producer/prerequisite queries."""

    def __init__(self, edges):
        self.edges = list(edges)
        self._into = {}
        for e in self.edges:
            self._into.setdefault((e["kind"], e["to"]), []).append(e)

    @classmethod
    def load(cls, story_dir=None):
        path = (Path(story_dir) if story_dir else STORY_DIR) / "graph.json"
        return cls(json.loads(path.read_text())["edges"])

    def edges_into(self, kind, to):
        return self._into.get((kind, to), [])

    def edges_from(self, kind, from_node):
        return [e for e in self.edges
                if e["kind"] == kind and e["from"] == from_node]

    def producers(self, target_kind, target):
        return [e["from"] for e in self.edges_into(target_kind, target)]

    def storyline_info(self, node):
        """node 'script:Map:storyline' → (map, storyline) + graph facts."""
        _, map_name, storyline = node.split(":", 2)
        triggered = [e["to"].split(":", 1)[1]
                     for e in self.edges_from("triggered_at", node)]
        requires = [e["to"].split(":", 1)[1]
                    for e in self.edges_from("requires", node)]
        self_produced = {e["to"].split(":", 1)[1]
                         for e in self.edges_from("sets", node)}
        self_produced |= {e["to"].split(":", 1)[1]
                          for e in self.edges_from("gives", node)}
        battles = [e["to"] for e in self.edges_from("starts_battle", node)]
        return {
            "node": node,
            "map": map_name,
            "storyline": storyline,
            "triggered_at": triggered[0] if triggered else map_name,
            "requires": [f for f in requires if f not in self_produced],
            "battles": battles,
        }

    def delete_edges(self, rng, fraction):
        """A copy with a seeded `fraction` of edges removed (ablation)."""
        kept = [e for e in self.edges if rng.random() >= fraction]
        return EventGraph(kept)


class PlanError(Exception):
    """Graceful planning/execution failure; message is the run reason."""


class HierarchicalPlanner:
    POLICY_NAME = "hierarchical"
    TIER = "hierarchical"

    def __init__(self, graph, seed=0):
        self.graph = graph
        self.rng = random.Random(seed)
        self.battles = 0
        self.battles_won = 0
        self._in_battle = False
        self.plan_steps = []
        self.candidates_tried = 0

    # ── planning ─────────────────────────────────────────────────────
    def build_plan(self, env, task):
        """Ordered list of execution steps for the task goal."""
        goal = task["goal"]
        gtype = goal["type"]
        if gtype == "map":
            return [("travel", goal["id"])]
        if gtype == "battle_won":
            return [("wild_grass", "Route1")]
        if gtype == "flag":
            target = f"flag:{goal['id']}"
            return self._producer_plan(env, "sets", target)
        if gtype == "item":
            target = f"item:{goal['id'].upper()}"
            return self._producer_plan(env, "gives", target)
        if gtype == "party_count":
            raise PlanError("no_path:party_count_not_graph_representable")
        raise PlanError(f"no_path:unknown_goal_type:{gtype}")

    def _producer_plan(self, env, kind, target):
        nodes = self.graph.producers(kind, target)
        if not nodes:
            raise PlanError(f"no_path:no_producer:{target}")
        infos = [self.graph.storyline_info(n) for n in nodes]
        start_map = env.client.observe()["map"]["name"]

        def route_len(info):
            try:
                route = env.client.route(start_map, info["triggered_at"])
                legs = route.get("legs")
                return len(legs) if legs is not None else 999
            except Exception:
                return 999

        infos.sort(key=lambda i: (route_len(i), len(i["requires"]),
                                      i["node"]))
        steps = []
        for info in infos:
            steps.append(("storyline", info))
        return steps

    # ── execution ────────────────────────────────────────────────────
    def run(self, env, task, frame_budget):
        start = env.frame_count()
        try:
            steps = self.build_plan(env, task)
        except PlanError as e:
            return False, str(e)
        self.plan_steps = [s[0] for s in steps]
        obs = env.client.observe()
        try:
            for step in steps:
                kind, payload = step
                while True:
                    if env.frame_count() - start > frame_budget:
                        return False, "frame_budget"
                    mode = obs["mode"]
                    if mode == "battle":
                        self._fight(env)
                        obs = env.client.observe()
                        continue
                    if mode in ("dialogue", "transition", "menu"):
                        env.client.skip_dialogue()
                        obs, outcome, _ = env.step("step_frames:10")
                        if outcome["done"]:
                            return (outcome["success"],
                                    "" if outcome["success"] else "max_steps")
                        continue
                    break
                if kind == "travel":
                    obs, outcome, _ = env.step(f"travel_to:{payload}")
                    if outcome["done"]:
                        return (outcome["success"],
                                "" if outcome["success"] else "max_steps")
                    obs = env.client.observe()
                elif kind == "wild_grass":
                    env.step(f"travel_to:{payload}")
                    spots = Oracle.GRASS_SPOTS[payload]
                    skills.win_wild_battle(env.client, spots)
                    env.battles += 1
                    env.battles_won += 1
                    self.battles += 1
                    self.battles_won += 1
                    env.client.wait_until("control_ready", 900)
                    obs = env.client.observe()
                elif kind == "storyline":
                    self.candidates_tried += 1
                    try:
                        obs = self._execute_storyline(env, obs, payload)
                    except PlanError:
                        if payload is steps[-1][1]:
                            raise
                        continue  # next producer candidate
                    if goal_satisfied(task["goal"], env):
                        break
                    if payload is steps[-1][1]:
                        break  # all candidates tried; final check reports it
                    continue  # trigger fired but goal unverified: next candidate
                if env.frame_count() - start > frame_budget:
                    return False, "frame_budget"
        except PlanError as e:
            return False, str(e)
        ok = goal_satisfied(task["goal"], env)
        return ok, "" if ok else "no_path:goal_unverified"

    def _execute_storyline(self, env, obs, info):
        map_name = info["triggered_at"]
        if obs["map"]["name"] != map_name:
            env.client.travel_to(map_name)
            obs = env.client.observe()
            # Verify arrival by location, not by travel outcome: an
            # on-load script (@load speech, Oak escort, gym greeter)
            # legitimately interrupts travel control once we ARE there.
            if obs["map"]["name"] != map_name:
                raise PlanError(f"no_path:unreachable_map:{map_name}")
        if info["storyline"].startswith("@"):
            # @load / coord storylines fire on entry or while walking:
            # settle until the goal lands (entry speeches run long) or a
            # bounded round count elapses (mirrors the oracle's
            # _wait_for_flag pacing).
            for _ in range(40):
                if goal_satisfied_step(env):
                    return obs
                env.client.skip_dialogue()
                obs, outcome, _ = env.step("step_frames:10")
                if outcome["done"]:
                    return obs
            return obs
        # npc-bound storyline: resolve trigger entity via M4 semantics.
        sem = env.client.script_semantics(map_name)
        line = next((sl for sl in sem["storylines"]
                     if sl["id"] == f"{map_name}:{info['storyline']}"), None)
        if line is None:
            raise PlanError(f"no_path:storyline_not_in_semantics:"
                            f"{map_name}:{info['storyline']}")
        text_id = next((int(t.split(":", 1)[1])
                        for t in line.get("triggers", [])
                        if t.startswith("npc:")), None)
        if text_id is None:
            raise PlanError(f"no_path:no_npc_trigger:{line['id']}")
        entity = _entity_for_npc_text_id(env.client, text_id)
        if entity is None:
            raise PlanError(f"no_path:no_live_trigger:{line['id']}")
        for _ in range(MAX_INTERACT_ATTEMPTS):
            obs, outcome, info_out = env.step(f"interact_with:{entity}")
            if outcome["done"]:
                return obs
            if goal_satisfied_step(env):
                return obs
            result = (info_out.get("result") or {}).get("result")
            if result in ("entered_battle", "battle", "battle_started"):
                self._fight(env)
                obs = self._settle_for_goal(env, env.client.observe())
                continue
            if result == "dialogue" and info["battles"]:
                # The storyline's challenge text is open and its battle
                # is bound (starts_battle): the fight skill taps through
                # the text into the battle and wins it.
                self._fight(env)
                obs = self._settle_for_goal(env, env.client.observe())
                if goal_satisfied_step(env):
                    return obs
                continue
            if result in ("entered_dialogue", "dialogue"):
                env.client.skip_dialogue()
                obs, outcome, _ = env.step("step_frames:10")
                continue
            return obs
        return env.client.observe()

    def _settle_for_goal(self, env, obs, rounds=20):
        """After a fight: victory text / badge ceremonies run past the
        battle's last frame — advance (goal-aware) until the goal flag
        lands or the round budget ends."""
        for _ in range(rounds):
            if goal_satisfied_step(env):
                return env.client.observe()
            env.client.skip_dialogue()
            obs, outcome, _ = env.step("step_frames:10")
            if outcome["done"]:
                return obs
        return obs

    def _fight(self, env):
        if not self._in_battle:
            self.battles += 1
            self._in_battle = True
        skills.fight_current_battle(env.client, prefer="fight")
        self.battles_won += 1
        self._in_battle = False


def goal_satisfied_step(env):
    """Cheap mid-plan check: any flag/item goal already satisfied."""
    task = getattr(env, "task", None)
    if task is None:
        return False
    return goal_satisfied(task["goal"], env)


def load_graph(story_dir=None):
    return EventGraph.load(story_dir)


def ablate(graph, fraction, seed):
    """Seeded 30%-style edge deletion for the world-model ablation."""
    return graph.delete_edges(random.Random(seed), fraction)
