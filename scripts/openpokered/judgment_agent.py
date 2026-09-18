"""A policy that decides from the goal text instead of a script.

The scripted tiers encode "where does this goal point?" as data a
maintainer typed: `oracle.py` keeps a `FLAG_STRATEGIES` dispatch table
keyed on exact flag names with a hardcoded `travel_to:<map>` inside each
entry, `policies.NPC_HINTS` maps a task id to a name substring, and the
milestone playthrough is coordinates all the way down. Adding a goal means
writing another entry.

This policy derives the same answer instead. Per decision, code enumerates
the actions that are actually executable from where the agent stands —
walk to a place the world graph says is reachable, or interact with an
entity the observation layer says is here — and one Choice question picks
among them. The model never invents an action; it selects from a list code
built, and every key in that list is a string `env.step()` already knows
how to run.

Code keeps everything that is not a judgment:

- the world-graph walk that bounds the candidate places (a graph with 222
  maps is both too large to offer and mostly unreachable, so the hop
  budget is what makes the question answerable at all);
- the action vocabulary and its execution;
- the battle loop and the dialogue settling;
- the step budget, and the deterministic fallback when a judgment fails.

The fallback matters: a judgment returns None on any transport failure, and
this policy then takes a deterministic action rather than stalling, so a
network outage degrades speed instead of correctness.
"""
import json
import random
from pathlib import Path

from . import skills
from .semantics import render_candidate

# A single Choice with 222 options is neither answerable nor cheap. The
# cap is generous for the hop budgets in use (budget 1 from a typical town
# yields 3-8 places) and exists to bound the pathological case.
MAX_CANDIDATE_PLACES = 24

# The curated story objectives. A committed data file rather than an API:
# it is what the event-graph tooling validates against, and reading it
# costs nothing.
OBJECTIVES_PATH = (Path(__file__).resolve().parents[2]
                   / "crates" / "pokered-data" / "story" / "objectives.json")


def load_objectives(path=OBJECTIVES_PATH):
    """The curated objectives as `[{id, name, satisfied_when:{flag}}]`."""
    try:
        data = json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return []
    if isinstance(data, dict):
        data = data.get("objectives") or list(data.values())
    if not isinstance(data, list):
        return []
    return [o for o in data if isinstance(o, dict) and o.get("id")]


def outstanding(objectives, flags):
    """Objectives whose flag is not set yet — what is left of the story.

    A flag missing from the table counts as unset, so an objective is
    dropped only when the game positively reports it done.
    """
    left = []
    for objective in objectives:
        flag = (objective.get("satisfied_when") or {}).get("flag")
        if flag and not flags.get(flag):
            left.append(dict(objective, flag=flag))
    return left

# How the graph says a place is reached, in words a reader would use.
_VIA = {"warp": ("building", "indoors, enter by its door"),
        "connection": ("route", "a way out of the current area")}


def hop_distances(edges, origin, hop_budget):
    """{map: (hops, edge kind)} for maps within `hop_budget` of `origin`."""
    hops = {origin: (0, None)}
    frontier = {origin}
    for depth in range(hop_budget):
        # Level by level: only the previous depth's maps may expand, or a
        # single sweep would reach two hops inside a budget of one
        # whenever the edge order happened to allow it.
        discovered = set()
        for edge in edges:
            src, dst = edge.get("from_map"), edge.get("to_map")
            if dst and src in frontier and dst not in hops:
                hops[dst] = (depth + 1, edge.get("kind"))
                discovered.add(dst)
        if not discovered:
            break
        frontier = discovered
    hops.pop(origin, None)
    return hops


def _describe_place(map_name, distance, kind, visited):
    via_kind, via_note = _VIA.get(kind, ("place", "reachable from here"))
    return {
        "id": map_name,
        "kind": via_kind,
        "note": (f"{via_note}, {distance} map(s) away, "
                 f"{'already visited' if map_name in visited else 'not visited yet'}"),
    }


def reachable_places(edges, origin, hop_budget, visited=None,
                     limit=MAX_CANDIDATE_PLACES):
    """Maps within `hop_budget` graph hops of `origin`, nearest first.

    This is the candidate set, and it is where recall is won or lost: the
    model cannot choose a map that was never offered. So the entries carry
    facts only — how the place is reached, how far, and whether the agent
    has been there — never a guess at relevance, which is the judgment's
    job. `visited` is a fact the agent has and the model does not.
    """
    seen = set(visited or ())
    # Sorting before the cap means a wide search keeps the *nearest*
    # places; the ordering never decides which one is chosen, only which
    # survive to be offered.
    return [_describe_place(name, distance, kind, seen)
            for name, (distance, kind) in sorted(
                hop_distances(edges, origin, hop_budget).items(),
                key=lambda kv: (kv[1][0], kv[0]))[:limit]]


# Deep enough to reach every map in the world graph (248 at the time of
# writing), so a goal map is found wherever it is.
ALL_HOPS = 64
MAX_GOAL_PLACES = 6


def goal_places(edges, origin, index, goal, visited=None):
    """Maps the index says satisfy `goal`, however far away.

    Distance is deliberately not a filter. The index answers "which map
    sets this flag" globally, and the agent should be able to act on that
    from anywhere — `travel_to` walks the route. The first cut of this
    filtered them to the local hop radius, and an ablation showed that
    changed no decision at all: the maps that satisfy a goal are usually
    several hops out, so their facts never reached the model.
    """
    if index is None or not goal:
        return []
    table = index.table_for(goal)
    want = goal.get("id")
    if not want or table is None:
        return []
    hops = hop_distances(edges, origin, ALL_HOPS)
    seen = set(visited or ())
    found = []
    for map_name in sorted(table.get(want, ())):
        if map_name not in hops:
            continue    # not on the connected graph from here
        distance, kind = hops[map_name]
        found.append(_describe_place(map_name, distance, kind, seen))
    return found[:MAX_GOAL_PLACES]


def build_actions(entities, places):
    """The executable action vocabulary for one decision.

    Code decides what is *possible*; the judgment decides what is
    *useful*. Keys are the exact strings `env.step()` runs, and the
    descriptions are what a reader would use to choose between them.
    """
    actions = {}
    for e in entities:
        # A warp is a destination, not a thing to interact with.
        # `interact_with` on one only resolves from its own tile, so
        # offering it produces a no-op that burns a decision and then
        # gets retired — which is exactly how a run ends up shuttling
        # between a gym and the town outside it. Leaving a map is already
        # expressible as `travel_to`.
        if e.get("kind") == "warp" or not e.get("position"):
            continue
        actions[f"interact_with:{e['id']}"] = \
            f"walk over and interact with: {render_candidate(e)}"
    for p in places:
        actions[f"travel_to:{p['id']}"] = \
            f"travel to: {render_candidate(p)}"
    return actions


class ScriptIndex:
    """Which maps set which flags and give which items.

    The oracle carries this knowledge by hand — `FLAG_STRATEGIES` maps a
    goal flag to a strategy with a hardcoded `travel_to` inside. But it is
    not judgment: a storyline records the flags it sets and the items it
    gives, so "which map satisfies this goal" is a lookup. Code does the
    lookup and puts the answer in the candidate's description; the
    judgment weighs it against distance, what is already visited, and
    what else is on the map.

    Built lazily, once, and cached for the life of the policy — the
    semantics are static and a pass over the map list costs a few seconds.
    A failed build is not retried: a run should not pay that cost twice.
    """

    def __init__(self, client):
        self.client = client
        self._by_flag = {}
        self._by_item = {}
        self._built = False
        self.error = None
        self.maps_indexed = 0

    def build(self):
        if self._built:
            return
        self._built = True   # set first: a failed build must not retry
        try:
            maps = self.client.script_semantics().get("maps") or []
        except Exception as e:
            self.error = f"{type(e).__name__}: {e}"
            return
        for name in maps:
            try:
                data = self.client.script_semantics(name)
            except Exception:
                continue      # one unreadable map must not lose the index
            self.maps_indexed += 1
            for story in data.get("storylines") or []:
                for effect in story.get("effects") or []:
                    kind = effect.get("kind")
                    if kind == "flag_set" and effect.get("flag"):
                        self._by_flag.setdefault(effect["flag"], set()).add(name)
                    elif kind == "item_given" and effect.get("item"):
                        self._by_item.setdefault(effect["item"], set()).add(name)

    def table_for(self, goal):
        """(goal id -> maps) for this goal kind, or None when the index
        has nothing to say about this kind of goal.

        Builds on demand: this is a public accessor, and an empty table
        because nobody happened to call `facts()` first is the kind of
        ordering bug that reads as "the index found nothing".
        """
        self.build()
        return {"flag": self._by_flag, "item": self._by_item}.get(
            (goal or {}).get("type"))

    def facts(self, map_name, goal):
        """What this place is known to do about the goal, in words."""
        want = (goal or {}).get("id")
        table = self.table_for(goal)
        if not want or table is None or map_name not in table.get(want, ()):
            return []
        return [f"a script here {'sets' if goal['type'] == 'flag' else 'gives'} "
                f"{want}"]


# Bounds on the state handed to the judgment: enough for a decision,
# not so much that every request re-sends the whole save file.
MAX_PARTY_SHOWN = 6
MAX_BAG_SHOWN = 12


def story_state(client, obs, goal, goal_spec, visited, failed_here):
    """Everything the agent knows, as one object for the judgment.

    A judgment is only as good as the state behind it. Asked to choose the
    next action with nothing but a map name and a party *count*, a model
    has to guess at the things a player would actually look at: how hurt
    the party is, what is in the bag, how many badges are held, where it
    has already been, and what it has already tried and failed here. This
    assembles those.

    Fields are top-level and named so a question can refer to them
    directly (`party[0].hp`, `actions_already_failed_here`). The bag is
    not in the observation snapshot, so it is read separately — one cheap
    local protocol call, against one model request per decision.
    """
    pos = obs.get("position") or {}
    state = {
        "goal": goal,
        "goal_spec": goal_spec or "not specified",
        "where": {"map": obs["map"]["name"], "x": pos.get("x"),
                  "y": pos.get("y"), "facing": obs.get("facing")},
        "party": [{"species": m.get("species"), "level": m.get("level"),
                   "hp": m.get("hp"), "max_hp": m.get("max_hp"),
                   "status": m.get("status")}
                  for m in (obs.get("party") or [])[:MAX_PARTY_SHOWN]],
        "badges": (obs.get("badges") or {}).get("count", 0),
        "maps_already_visited": sorted(visited),
        "actions_already_failed_here": sorted(failed_here),
    }
    try:
        bag = client.bag()
    except Exception:
        bag = None
    entries = []
    if isinstance(bag, dict):
        entries = [{"item": k, "qty": v} for k, v in bag.items()]
    elif isinstance(bag, list):
        entries = [{"item": b.get("item") or b.get("name"),
                    "qty": b.get("qty", 1)}
                   for b in bag if isinstance(b, dict)]
    if entries:
        state["bag"] = entries[:MAX_BAG_SHOWN]
    return state


class Decision:
    """The judgment's answer, with the distribution behind it.

    The model is asked for a *judgment*, and how the probability mass is
    spread is part of the answer — not only which option edged ahead.
    `probabilities` keeps the whole distribution, `margin` is the gap
    between the top two, and `decisive` is the caller's threshold on that
    gap, so the policy can decline to act on a near-tie instead of
    committing to whichever option happened to win by a hair.
    """

    def __init__(self, selection, act_margin=0.0):
        self.action = selection.choice
        self.probabilities = dict(selection.probabilities)
        self.confidence = selection.confidence
        self.margin = selection.margin
        self.runner_up = selection.runner_up
        self.decisive = True if act_margin <= 0 else self.margin >= act_margin

    def __repr__(self):
        p = self.probabilities.get(self.action, 0.0)
        return (f"Decision({self.action!r}, p={p:.2f}, margin={self.margin:.2f}, "
                f"decisive={self.decisive})")


class JudgmentAgent:
    """T2J: goal-driven decisions over a code-built action vocabulary."""

    POLICY_NAME = "judgment_agent"
    TIER = "T2J"

    def __init__(self, judge, seed=0, hop_budget=1, max_hop_budget=3,
                 max_judgments=60, entity_radius=99, place_facts=True,
                 rich_state=True, act_margin=0.0, explore=False,
                 objectives=None):
        self.judge = judge
        # Exploration mode: the goal is not given by the task spec, it is
        # chosen from what is left of the story. See `choose_objective`.
        self.explore = explore
        self.objectives = (load_objectives() if objectives is None
                           else objectives)
        self.chosen_objective = None
        self.rng = random.Random(seed)
        self.hop_budget = hop_budget
        self.max_hop_budget = max_hop_budget
        self.max_judgments = max_judgments
        self.entity_radius = entity_radius
        # Annotating candidates with what their scripts do about the goal
        # is also the ablation switch: same policy, same model, facts on
        # or off, so the facts' contribution is attributable.
        self.place_facts = place_facts
        # `rich_state` likewise: off sends the thin map/position/count
        # observation the earlier measurements used.
        self.rich_state = rich_state
        # Below this margin the top two options are treated as a tie and
        # the policy declines to act on the judgment (0 disables the gate).
        self.act_margin = act_margin
        self.undecided = 0     # decisions the margin gate declined to act on
        self.answer = None     # last Decision, for run reports
        self.judgments = 0
        self.fallbacks = 0
        self.escalations = 0   # decisions that had to widen the candidate set
        self.battles = 0
        self.battles_won = 0
        self.visited = set()   # maps stood on — a fact the model is not told
        self.goal_spec = {}    # the task's typed goal, when it has one
        self.index = None      # ScriptIndex, built on first use
        self._in_battle = False
        self._failed = set()   # actions this map already proved useless
        self._map = None

    # ── candidate construction (code's half) ────────────────────────
    def _entities(self, client):
        """Everything the observation layer can see, not just what is in
        reach.

        `interactable` means *adjacent* (distance <= 2 step units), so
        filtering on it hides every trainer across a room — which is how a
        gym leader stops being a candidate at all. `interact_with`
        navigates to an id, so distance is not a reason to withhold one.
        """
        return client.nearby(radius=self.entity_radius).get("entities", [])

    def _script_index(self, client):
        if self.index is None:
            self.index = ScriptIndex(client)
        return self.index

    def _places(self, client, obs, budget=None):
        """Candidates: what is reachable, plus what the index says works.

        The second group is not distance-filtered, because the index
        answers globally and `travel_to` walks the route.
        """
        edges = client.world_graph()["edges"]
        origin = obs["map"]["name"]
        places = reachable_places(
            edges, origin,
            self.hop_budget if budget is None else budget,
            visited=self.visited)
        if not (self.place_facts and self.goal_spec):
            return places
        index = self._script_index(client)
        have = {p["id"] for p in places}
        places.extend(p for p in goal_places(edges, origin, index,
                                             self.goal_spec,
                                             visited=self.visited)
                      if p["id"] not in have)
        for place in places:
            extra = index.facts(place["id"], self.goal_spec)
            if extra:
                place["note"] = f"{place['note']}; {'; '.join(extra)}"
        return places

    def actions_for(self, client, obs):
        """Everything executable right now, minus what already failed.

        Places widen only when the narrow set runs out: the first pass
        offers the adjacent maps, and once every one of those has been
        ruled out the next pass reaches a hop further. Escalating here
        rather than starting wide keeps the common case cheap — the
        candidate list is re-sent with every request, so its size is the
        main cost — and it is what stops a run ending in
        `no_actions_left` while unvisited maps sit two hops away.
        """
        here = obs["map"]["name"]
        entities = self._entities(client)
        for budget in range(self.hop_budget, self.max_hop_budget + 1):
            places = self._places(client, obs, budget)
            actions = {a: d for a, d in build_actions(entities, places).items()
                       if (here, a) not in self._failed}
            if actions:
                if budget > self.hop_budget:
                    self.escalations += 1
                return actions
        return {}

    # ── exploration: choosing what to pursue ────────────────────────
    def choose_objective(self, client, obs):
        """Pick the next story thread, or None when the story is done.

        The task specs fix a goal; this is the other mode. Code narrows to
        the objectives whose flag is still unset and reports what the
        script index knows can satisfy each; the judgment chooses which is
        worth pulling.

        The state matters here as much as anywhere: offered nothing but a
        list of the eleven objectives, the judgment declines all of them —
        it has no way to tell whether the agent is standing in Pallet Town
        at the start or outside the eighth gym. So who and where goes in
        with the list.
        """
        left = outstanding(self.objectives, client.flags())
        if not left:
            return None
        index = self._script_index(client)
        candidates = []
        for objective in left:
            table = index.table_for({"type": "flag", "id": objective["flag"]})
            maps = sorted((table or {}).get(objective["flag"], ()))
            name = objective.get("name") or objective["id"]
            candidates.append({
                "id": objective["id"],
                "kind": "objective",
                "note": (f"{name}: a script in {', '.join(maps)} sets "
                         f"{objective['flag']}" if maps else
                         f"{name}: nothing in the index sets "
                         f"{objective['flag']}"),
            })
        chosen = self.judge.choose_objective(
            {"where": obs["map"]["name"],
             "badges": (obs.get("badges") or {}).get("count", 0),
             "party": [{"species": m.get("species"), "level": m.get("level"),
                        "hp": m.get("hp"), "max_hp": m.get("max_hp")}
                       for m in (obs.get("party") or [])],
             "what_is_left_of_the_story": [c["note"] for c in candidates]},
            candidates)
        if chosen is None:
            return None
        return next(o for o in left if o["id"] == chosen)

    # ── decision (the model's half) ─────────────────────────────────
    def decide(self, client, obs, goal):
        """One action string, or None when nothing is left to try.

        Falls back to a deterministic pick when the judgment is
        unavailable, so a transport failure costs a decision's quality
        rather than the whole run.
        """
        actions = self.actions_for(client, obs)
        if not actions:
            return None
        if self.judgments >= self.max_judgments:
            return None
        selection = None
        if self.judge is not None and self.judge.enabled:
            self.judgments += 1
            selection = self.judge.route_action(
                goal, actions, self.observation_for(client, obs, goal))
        if selection is None or selection.choice not in actions:
            self.fallbacks += 1
            return self._fallback(actions)
        answer = Decision(selection, self.act_margin)
        self.answer = answer
        if not answer.decisive:
            # The spread says the options are a toss-up. Acting on the
            # argmax would be committing on a coin-flip, so take the
            # deterministic action instead and let the next observation
            # break the tie.
            self.undecided += 1
            return self._fallback(actions)
        return answer.action

    def observation_for(self, client, obs, goal):
        """The state the judgment sees. Thin by default in the ablation."""
        if not self.rich_state:
            return {"map": obs["map"]["name"],
                    "position": obs.get("position"),
                    "mode": obs["mode"],
                    "party_size": len(obs.get("party") or [])}
        here = obs["map"]["name"]
        return story_state(client, obs, goal, self.goal_spec, self.visited,
                           (a for (m, a) in self._failed if m == here))

    def _fallback(self, actions):
        """Deterministic, and deliberately the conservative one: talk to
        whatever is here before wandering off. Sorted so the same state
        always yields the same action."""
        interact = sorted(a for a in actions if a.startswith("interact_with:"))
        return interact[0] if interact else sorted(actions)[0]

    # ── execution ───────────────────────────────────────────────────
    def _observe_key(self, obs):
        pos = obs.get("position") or {}
        return (obs["map"]["name"], pos.get("x"), pos.get("y"), obs["mode"])

    def run(self, env, task, frame_budget):
        start = env.frame_count()
        obs = env.client.observe()
        goal = task.get("name") or task["id"]
        # The goal text drives the judgment; the typed goal is what the
        # script index can look up. The specs carry both.
        self.goal_spec = task.get("goal") or {}
        while True:
            if env.frame_count() - start > frame_budget:
                return False, "frame_budget"
            mode = obs["mode"]
            if mode == "battle":
                if not self._in_battle:
                    self.battles += 1
                    self._in_battle = True
                skills.fight_current_battle(env.client, prefer="fight")
                self.battles_won += 1
                self._in_battle = False
                obs = env.client.observe()
                continue
            if mode in ("dialogue", "menu", "transition"):
                env.client.skip_dialogue()
                obs, outcome, _ = env.step("step_frames:10")
                if outcome["done"]:
                    return outcome["success"], "" if outcome["success"] else "max_steps"
                continue

            if self.explore:
                flags = env.client.flags()
                settled = (self.chosen_objective is not None
                           and flags.get(self.chosen_objective["flag"]))
                if self.chosen_objective is None or settled:
                    nxt = self.choose_objective(env.client, obs)
                    if nxt is None:
                        # Nothing left of the story: that is the goal,
                        # reached. Report it as the run's success.
                        return True, ""
                    self.chosen_objective = nxt
                    self.goal_spec = {"type": "flag", "id": nxt["flag"]}
                    goal = nxt.get("name") or nxt["id"]

            if obs["map"]["name"] != self._map:
                self._map = obs["map"]["name"]
                self.visited.add(self._map)
                # Failures are keyed by map and never cleared. Clearing on
                # every arrival re-armed them for an agent that oscillates
                # between two maps, so it retried the same dead action
                # forever — which is exactly what it did between Pewter
                # Gym and Pewter City.

            action = self.decide(env.client, obs, goal)
            if action is None:
                # Two dead ends that a run report must not conflate: an
                # empty action space is a policy problem (widen it), a
                # spent judgment budget is a budget problem (raise it).
                if self.judgments >= self.max_judgments:
                    return False, "judgment_cap"
                return False, "no_actions_left"
            before = self._observe_key(obs)
            obs, outcome, info = env.step(action)
            if outcome["done"]:
                return outcome["success"], "" if outcome["success"] else "max_steps"
            # An action that moved nothing and changed nothing will not
            # do so on a retry either; retire it, for this map only —
            # the same action string elsewhere is a different situation.
            if info.get("invalid") or self._observe_key(obs) == before:
                self._failed.add((self._map, action))
