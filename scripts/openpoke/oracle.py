"""Rule-based oracle planner for openpoke (M6).

The oracle plays the "intent" role the design doc assigns to the LLM —
decompose a task goal into semantic steps over the event graph, world
route, and script semantics — while every step below it stays
deterministic (debug commands / skills). Decomposition (kept explicit):

  map goal      → travel_to(destination)
  flag goal     → strategy lookup (table below): travel → enter →
                  interact → resolve (battle/choice) → poll flag
  item goal     → travel to the map holding it → get_nearby →
                  interact_with the hidden/ball entity
  battle_won    → pace grass (win_wild_battle skill)

Battle strategies reuse the event graph + script semantics rather than
hardcoding ids: Brock's storyline is found by scanning PewterGym's
semantics for a `battle_started` effect and reading its trigger npc id
(`script:...` semantics give `npc:{i}` binding? no — the storyline's
trigger list carries the npc text id; the runtime entity id comes from
get_nearby by matching text-id position — see `_battle_npc_entity`).
"""
from . import skills


class OracleError(RuntimeError):
    pass


def _wait_for_flag(client, flag, max_rounds=80):
    """Wait until a flag is set: advance dialogue when open, otherwise
    step frames (skip_dialogue is a no-op when no box is open — it does
    NOT advance the cutscene on its own)."""
    for _ in range(max_rounds):
        if client.flags().get(flag):
            return True
        s = client.state()
        if s["dialogue_state"] is not None:
            client.skip_dialogue()
        else:
            client.step(10)
    return False


def _entity_for_npc_text_id(client, text_id):
    """Map a storyline's npc text id to a live get_nearby entity id.
    Runtime npc_states are indexed by load order; their text_id matches
    the script_config binding. We resolve via the live NPC list."""
    npcs = client.cmd(cmd="get_npcs")
    for n in npcs:
        if n.get("text_id") == text_id and n.get("visible", True):
            return f"npc:{n['npc_index']}"
    return None


class Oracle:
    """Executes task intents deterministically. Not an LLM."""

    def __init__(self, env):
        self.env = env
        self.client = env.client

    # ── top-level decomposition ─────────────────────────────────────
    def run(self, task):
        goal = task["goal"]
        gtype = goal["type"]
        if not task.get("oracle", True):
            raise OracleError(f"task {task['id']} is marked oracle:false (needs a v2 strategy)")
        if gtype == "map":
            return self._reach_map(goal["id"])
        if gtype == "item":
            return self._acquire_item(goal["id"])
        if gtype == "battle_won":
            return self._win_wild_battle()
        if gtype == "party_count":
            return self._ensure_party_count(goal["min"])
        if gtype == "flag":
            return self._achieve_flag(task, goal["id"])
        raise OracleError(f"no oracle strategy for goal type {gtype}")

    # ── map ─────────────────────────────────────────────────────────
    def _reach_map(self, map_name):
        out = self.env.step(f"travel_to:{map_name}")
        return out

    # ── party count (trivial setup path) ────────────────────────────
    def _ensure_party_count(self, minimum):
        have = len(self.client.party())
        while have < minimum:
            self.client.give_pokemon("Charmander", 5)
            have += 1
        return None, {"done": True, "success": True}, {"strategy": "give_pokemon"}

    # ── item acquisition ────────────────────────────────────────────
    # Maps known (from world data) to hold each item as a hidden item or
    # a Poké Ball; the oracle verifies with get_nearby at run time.
    ITEM_SPOTS = {
        "Potion": [("ViridianCity", "hidden_item"), ("ViridianForest", "item")],
    }

    def _acquire_item(self, item_id):
        for (map_name, kind) in self.ITEM_SPOTS.get(item_id, []):
            out, *_ = self.env.step(f"travel_to:{map_name}")
            entities = self.client.nearby(radius=99)["entities"]
            for e in entities:
                if e["kind"] == kind and (e.get("name") or "").replace(" ", "") == item_id.replace("_", ""):
                    self.env.step(f"interact_with:{e['id']}")
                    return None, {"done": True, "success": True}, {"strategy": "item_spot", "entity": e["id"]}
        raise OracleError(f"item {item_id} not found at any known spot")

    # ── wild battle ─────────────────────────────────────────────────
    # Grass pacing spots per map (tiles inside the patches).
    GRASS_SPOTS = {
        "Route1": [(10, 30), (10, 32), (8, 30), (8, 32)],
        "Route2": [(4, 46), (4, 48), (6, 46), (6, 48)],
    }

    def _win_wild_battle(self, map_name="Route1"):
        self.env.step(f"travel_to:{map_name}")
        skills.win_wild_battle(self.client, self.GRASS_SPOTS[map_name])
        self.env.battles += 1
        self.env.battles_won += 1
        # Settle the post-battle fade-in tail so the goal check observes
        # mode == "overworld" (not "transition").
        self.client.wait_until("control_ready", 900)
        return None, {"done": True, "success": True}, {"strategy": "wild_grass", "map": map_name}

    # ── flag strategies ─────────────────────────────────────────────
    def _achieve_flag(self, task, flag):
        strategy = FLAG_STRATEGIES.get(flag) or FLAG_STRATEGIES.get(task["id"])
        if strategy is None:
            raise OracleError(f"no flag strategy for {flag} (task {task['id']})")
        return strategy(self)

    def _talk_to_oak(self):
        # EVENT_OAK_ASKED_TO_CHOOSE_MON is set by the lab's on-entry
        # OakChooseMonSpeech (requires EVENT_OAK_APPEARED_IN_PALLET set
        # and EVENT_FOLLOWED_OAK_INTO_LAB unset — the task setup pins
        # that post-escort, pre-speech state). Walk in and listen.
        self.env.step("travel_to:OaksLab")
        _wait_for_flag(self.client, "EVENT_OAK_ASKED_TO_CHOOSE_MON")
        return None, {"done": True, "success": True}, {"strategy": "oak_entry_speech"}

    def _get_starter(self):
        self.env.step("travel_to:OaksLab")
        # The choice only opens after the entry speech sets the ask flag.
        _wait_for_flag(self.client, "EVENT_OAK_ASKED_TO_CHOOSE_MON")
        # The speech's tail (Oak/rival movement) still runs after the flag
        # — walking now would read as a script interruption. Settle it.
        self.client.wait_until("control_ready", 1200)
        # The starter balls are scripted NPCs (no item_id), so nearby
        # classifies them as npc, not item. Find a ball storyline by its
        # pokemon_given effect (script semantics, not hardcode) and
        # resolve its bound npc to a live entity.
        sem = self.client.script_semantics("OaksLab")
        ball_line = None
        for sl in sem["storylines"]:
            for eff in sl.get("effects", []):
                if eff.get("kind") == "pokemon_given":
                    ball_line = sl
                    break
            if ball_line:
                break
        if ball_line is None:
            raise OracleError("no starter ball storyline in OaksLab semantics")
        npc_text_id = None
        for trig in ball_line["triggers"]:
            if trig.startswith("npc:"):
                npc_text_id = int(trig.split(":", 1)[1])
        if npc_text_id is None:
            raise OracleError(f"ball storyline has no npc trigger: {ball_line['id']}")
        entity = _entity_for_npc_text_id(self.client, npc_text_id)
        if entity is None:
            raise OracleError(f"starter ball text_id {npc_text_id} not live in OaksLab")
        self.env.step(f"interact_with:{entity}")
        # Confirm the YES/NO choice (cursor defaults to YES) and listen
        # through the receive text + Pokédex entry screen until the flag
        # lands (the dex entry closes on A, not via skip_dialogue).
        for _ in range(80):
            if self.client.flags().get("EVENT_GOT_STARTER"):
                break
            s = self.client.state()
            if s["choice"] is not None:
                skills.tap(self.client, "a", 10)
            elif s["dialogue_state"] is not None:
                self.client.skip_dialogue()
            elif s["active_script_effect"] is not None or s["screen"] != "overworld":
                skills.tap(self.client, "a", 6)
            else:
                self.client.step(10)
        return None, {"done": True, "success": True}, {
            "strategy": "starter_ball", "entity": entity, "storyline": ball_line["id"]}

    def _beat_forest_trainer(self):
        self.env.step("travel_to:ViridianForest")
        # The forest has three sight trainers (ordinals 0/1/2 mapping to
        # EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_{0,1,2}); the goal flag
        # tracks ONE of them, and arrival side decides who's nearest.
        # Fight them nearest-first, retrying each through the wild
        # battles the grass keeps rolling — every defeat is permanent,
        # so the loop always terminates.
        goal_flag = "EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0"
        fought = set()
        for _ in range(3):
            if self.client.flags().get(goal_flag):
                break
            entities = self.client.nearby(radius=99)["entities"]
            trainer = next(
                (e for e in entities
                 if e["kind"] == "trainer" and e["id"] not in fought),
                None)
            if trainer is None:
                break
            fought.add(trainer["id"])
            engaged = False
            for _attempt in range(6):
                out = self.client.interact_with(trainer["id"])
                result = out.get("result")
                if result in ("entered_battle", "entered_dialogue"):
                    # A wild battle (or his own sight intro) interrupted
                    # the approach — resolve it and re-approach.
                    skills.fight_current_battle(self.client, prefer="fight")
                    self.env.battles += 1
                    self.env.battles_won += 1
                    continue
                if result in ("dialogue", "battle"):
                    skills.fight_current_battle(self.client, prefer="fight")
                    self.env.battles += 1
                    self.env.battles_won += 1
                    engaged = True
                    break
                if result == "nothing":
                    # Defeated already (his talk is empty) — done here.
                    engaged = True
                    break
                raise OracleError(f"trainer approach failed: {result}")
            if not engaged:
                raise OracleError(f"could not engage trainer {trainer['id']}")
        if not self.client.flags().get(goal_flag):
            raise OracleError(f"goal flag {goal_flag} not set after forest trainers")
        return None, {"done": True, "success": True}, {"strategy": "forest_trainers"}

    def _beat_brock(self):
        # 1. Party check: the task setup guarantees a capable lead
        #    (documented in the task spec); otherwise train first (v2).
        # 2. Travel to Pewter City.
        self.env.step("travel_to:PewterCity")
        # 3. Enter the gym via the world graph's warp edge.
        edges = self.client.world_graph(maps=["PewterCity"])["edges"]
        gym = next((e for e in edges if e["kind"] == "warp" and e["to_map"] == "PewterGym"), None)
        if gym is None:
            raise OracleError("no PewterGym warp edge on PewterCity")
        out = self.client.move_to(gym["from_pos"]["x"], gym["from_pos"]["y"])
        if out["result"] != "map_changed":
            raise OracleError(f"gym warp did not fire: {out['result']}")
        # 4. Find Brock's storyline by its battle effect and resolve the
        #    bound npc to a live entity (script semantics, not hardcode).
        #    The gym also has a junior trainer with his own battle
        #    storyline — pick the one whose trainer id/class is the
        #    leader's, not just the first battle effect in file order.
        sem = self.client.script_semantics("PewterGym")
        brock_line = None
        for sl in sem["storylines"]:
            for eff in sl.get("effects", []):
                if eff.get("kind") != "battle_started":
                    continue
                battle = eff.get("battle", {})
                if battle.get("kind") != "trainer":
                    continue
                trainer_id = battle.get("trainer_id") or ""
                tclass = battle.get("class") or ""
                if "BROCK" in trainer_id.upper() or tclass == "Brock":
                    brock_line = sl
                    break
            if brock_line:
                break
        if brock_line is None:
            raise OracleError("no leader battle storyline in PewterGym semantics")
        npc_text_id = None
        for trig in brock_line["triggers"]:
            if trig.startswith("npc:"):
                npc_text_id = int(trig.split(":", 1)[1])
        if npc_text_id is None:
            raise OracleError(f"battle storyline has no npc trigger: {brock_line['id']}")
        entity = _entity_for_npc_text_id(self.client, npc_text_id)
        if entity is None:
            raise OracleError(f"battle npc text_id {npc_text_id} not live in PewterGym")
        # 5. Talk → his challenge text → the gym battle → win. PewterGym
        #    has a junior trainer with a proximity sight line on the way
        #    in: he intercepts the approach and must be fought first
        #    (his defeat flag is not the goal). The gym guide's
        #    auto-greeting is cleared the same way. Loop: clear dialogue
        #    and auto-resolve whatever battle shows up with FIGHT (the
        #    cursor is explicitly corrected — the menu may remember RUN
        #    from earlier wild escapes), then re-attempt the talk.
        for _ in range(4):
            state = self.client.state()
            if state["dialogue_state"] is not None:
                self.client.skip_dialogue()
                continue
            out = self.client.interact_with(entity)
            result = out.get("result")
            if result in ("dialogue", "battle"):
                # Brock's challenge line opened (or the battle is already
                # running): fight through it with FIGHT.
                skills.fight_current_battle(self.client, prefer="fight")
                self.env.battles += 1
                self.env.battles_won += 1
                break
            if result in ("entered_dialogue", "entered_battle"):
                # Intercepted by the junior trainer (sight intro → his
                # battle). Win it, then re-attempt the approach — he is
                # defeated afterwards and will not re-engage.
                skills.fight_current_battle(self.client, prefer="fight")
                self.env.battles += 1
                self.env.battles_won += 1
                continue
            raise OracleError(f"unexpected interact_with result: {result}")
        else:
            raise OracleError("could not reach Brock through the intercepts")
        # 6. Victory text/badge ceremony then verify the flag.
        _wait_for_flag(self.client, "EVENT_BEAT_BROCK")
        return None, {"done": True, "success": True}, {
            "strategy": "gym_leader", "entity": entity, "storyline": brock_line["id"]}


def _strategies():
    return {
        "EVENT_OAK_ASKED_TO_CHOOSE_MON": Oracle._talk_to_oak,
        "EVENT_GOT_STARTER": Oracle._get_starter,
        "EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0": Oracle._beat_forest_trainer,
        "EVENT_BEAT_BROCK": Oracle._beat_brock,
    }


FLAG_STRATEGIES = _strategies()
