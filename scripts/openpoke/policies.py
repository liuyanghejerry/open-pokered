"""RQ1 calibration policies — tier-restricted baseline agents (T2/T1).

T2 `LocalExplorer` (symbolic observation + LOCAL navigation only):
  allowed: get_agent_state / get_nearby / move_to / interact /
  interact_with / battle skills. Forbidden: travel_to, world graph,
  find_world_route, script semantics. It does NOT know the route, which
  building holds the goal, or any storyline binding. Navigation is a
  greedy compass walk with wall-bump recovery and visited-tile memory;
  entity targets (items, named NPCs) are acquired from get_nearby
  scans; when the target is not visible, unexplored building warps are
  tried systematically (nearest-first, each once). The explorer does
  NOT read the warp entities' destination names even though the
  observation layer exposes them (`get_nearby` warp `name` field) —
  that field is an abstraction leak noted for the RQ1 report.

T1 `ButtonRandomWalk` (controller buttons only): observes position/mode
via get_agent_state, acts with press_sequence + step_frames bursts.
Seeded random walk with a north bias + occasional A presses; battles
are mashed through with A. Hard frame cap.

Both are seeded from the task seed: same (task, tier, seed) → same
action stream (up to the engine's own seeded determinism, M5).

Battle accounting in both policies is by observation-mode transitions
(overworld→battle / battle→overworld), so T1 (which never calls
move_to) and T2 share one definition.
"""
import random

from . import skills

COMPASS_DELTA = {"north": (0, -1), "south": (0, 1), "west": (-1, 0), "east": (1, 0)}

# Entity hints the GOAL text itself implies (not world knowledge):
# "talk to Oak" → look for an npc whose name contains "oak".
NPC_HINTS = {"talk-to-oak": "oak"}


def _norm(text):
    return (text or "").replace(" ", "").replace("_", "").lower()


def _manhattan(ax, ay, bx, by):
    return abs(ax - bx) + abs(ay - by)


class LocalExplorer:
    """T2: greedy compass exploration + local entity acquisition."""

    POLICY_NAME = "local_explorer"

    def __init__(self, seed, compass="north", scan_radius=99, npc_hints=None):
        self.rng = random.Random(seed)
        self.compass = compass
        self.scan_radius = scan_radius
        self.npc_hints = npc_hints or NPC_HINTS
        self.visited = set()       # (map, x, y) tiles stood on
        self.blocked = set()       # (map, x, y) move_to targets that failed
        self.tried_warps = set()   # (map, x, y) building warps already explored
        self.battles = 0
        self.battles_won = 0
        self._in_battle = False
        self._edge_slide = 0     # committed wall-follow direction (±1), 0 = none
        self._npc_waits = {}     # (map, x, y) → waits already spent on a camper
        self._npc_blocked = {}   # (map, x, y) → actions left before forgiving
                                   # a transient NPC camp (not a static wall)
        self._bumped_once = set()  # tiles bumped once with no NPC in sight —
                                   # static-blocked only on the SECOND bump

    # ── target scanning ─────────────────────────────────────────────
    def _want(self, task):
        goal = task["goal"]
        if goal["type"] == "item":
            want_id = _norm(goal["id"])
            return lambda e: (e.get("kind") in ("item", "hidden_item")
                              and _norm(e.get("name")) == want_id)
        hint = self.npc_hints.get(task["id"])
        if hint:
            return lambda e: (e.get("kind") == "npc"
                              and hint in (e.get("name") or "").lower())
        return None

    def _scan_target(self, env, task):
        want = self._want(task)
        if want is None:
            return None
        entities = env.client.nearby(radius=self.scan_radius).get("entities", [])
        for e in entities:
            if want(e) and e.get("interactable", True):
                return e
        return None

    # ── compass walking ─────────────────────────────────────────────
    def _candidates(self, x, y):
        dx, dy = COMPASS_DELTA[self.compass]
        px, py = dy, dx  # perpendicular
        return [
            (x + dx * 4, y + dy * 4),   # primary: 4 ahead
            (x + px * 3 + dx * 2, y + py * 3 + dy * 2),
            (x - px * 3 + dx * 2, y - py * 3 + dy * 2),
            (x + px * 5, y + py * 5),   # lateral detours
            (x - px * 5, y - py * 5),
            (x - dx * 3, y - dy * 3),   # backtrack (last resort)
        ]

    def _npc_at(self, env, x, y):
        """A live NPC/trainer entity standing on tile (x, y), if any."""
        entities = env.client.nearby(radius=self.scan_radius).get("entities", [])
        for e in entities:
            if e.get("kind") in ("npc", "trainer"):
                pos = e.get("position") or {}
                if pos.get("x") == x and pos.get("y") == y:
                    return e
        return None

    def _compass_action(self, env, obs):
        x, y = obs["position"]["x"], obs["position"]["y"]
        map_name = obs["map"]["name"]
        dx, dy = COMPASS_DELTA[self.compass]
        # Edge-crossing burst: about to leave the map bounds — drive the
        # compass button instead of move_to (off-map targets are invalid).
        # North/west edges are detectable via small coordinates; south/
        # east rely on blocked-failure feedback (not needed by RQ1 tasks).
        # A burst that previously failed here marked the ahead tile
        # blocked: then commit to a wall-follow slide along the edge
        # instead of dithering between laterals.
        if dy < 0 and y <= 3:
            for tile in [t for t, ttl in self._npc_blocked.items() if ttl <= 1]:
                del self._npc_blocked[tile]
            for tile in self._npc_blocked:
                self._npc_blocked[tile] -= 1
            ahead = (map_name, x, y - 1)
            if ahead not in self.blocked and self._npc_blocked.get(ahead, 0) <= 0:
                # A wandering NPC camped on the crossing tile reads as a
                # wall to the bump check — wait for them to drift off
                # (bounded) before committing to a slide.
                if (self._npc_at(env, x, y - 1) or self._npc_at(env, x, y - 2)) \
                        and self._npc_waits.get(ahead, 0) < 2:
                    self._npc_waits[ahead] = self._npc_waits.get(ahead, 0) + 1
                    return "step_frames:30"
                return "drive:up,12"
            if self._edge_slide == 0:
                self._edge_slide = self.rng.choice((-1, 1))
            for _ in range(2):
                cx = x + self._edge_slide * 3
                if (map_name, cx, y) not in self.blocked:
                    return f"move_to:{cx},{y}"
                self._edge_slide = -self._edge_slide  # flip once, then score
        if dx < 0 and x <= 3 and (map_name, x - 1, y) not in self.blocked:
            return "drive:left,12"
        best, best_score = None, None
        for cx, cy in self._candidates(x, y):
            if (map_name, cx, cy) in self.blocked:
                continue
            if dy < 0 and y <= 3 and cy < 0:
                continue  # off-map target would only score an invalid action
            if dx < 0 and x <= 3 and cx < 0:
                continue
            gain = (cx - x) * dx + (cy - y) * dy
            revisits = sum(1 for (m, vx, vy) in self.visited
                           if m == map_name and _manhattan(vx, vy, cx, cy) <= 1)
            score = gain - 3 * revisits + self.rng.random() * 0.5
            if best_score is None or score > best_score:
                best, best_score = (cx, cy), score
        if best is None:
            return "drive:up,12"  # everything blocked: push the compass
        return f"move_to:{best[0]},{best[1]}"

    # ── building warps ──────────────────────────────────────────────
    def _untried_warp_action(self, env, obs):
        map_name = obs["map"]["name"]
        px, py = obs["position"]["x"], obs["position"]["y"]
        entities = env.client.nearby(radius=self.scan_radius).get("entities", [])
        warps = [e for e in entities if e.get("kind") == "warp"]
        warps.sort(key=lambda e: _manhattan(
            px, py, e["position"]["x"], e["position"]["y"]))
        for e in warps:
            key = (map_name, e["position"]["x"], e["position"]["y"])
            if key not in self.tried_warps:
                self.tried_warps.add(key)
                return f"move_to:{e['position']['x']},{e['position']['y']}"
        return None

    def _settle(self, env):
        """After a warp/edge crossing: drain cutscene/dialogue so the
        goal check observes the settled state (skip_dialogue is a no-op
        when no box is open, so step frames too)."""
        for _ in range(8):
            env.client.skip_dialogue()
            obs, outcome, _ = env.step("step_frames:10")
            if outcome["done"]:
                return obs, outcome, True
        return obs, outcome, False

    def _exit_building(self, env, obs):
        """Leave via the nearest untried warp (about-face presses when the
        first approach doesn't fire the carpet)."""
        building = obs["map"]["name"]
        action = self._untried_warp_action(env, obs)
        if action is None:
            return None
        obs, outcome, _ = env.step(action)
        if outcome["done"]:
            return obs, outcome
        for btn in ("down", "up", "left", "right"):
            if obs["map"]["name"] != building:
                break
            obs, outcome, _ = env.step(f"drive:{btn},10")
            if outcome["done"]:
                return obs, outcome
        return obs, {"done": False, "success": False}

    # ── main loop ───────────────────────────────────────────────────
    def run(self, env, task, frame_budget):
        start = env.frame_count()
        obs = env.client.observe()
        entered_building = False
        while True:
            if env.frame_count() - start > frame_budget:
                return False, "step_budget"
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
            if mode in ("dialogue", "transition", "menu"):
                # Cutscene / text owns input: advance it (starter speech,
                # warp fades, bumped-into sign).
                env.client.skip_dialogue()
                obs, outcome, _ = env.step("step_frames:10")
                if outcome["done"]:
                    return outcome["success"], "" if outcome["success"] else "max_steps"
                continue

            self.visited.add((obs["map"]["name"], obs["position"]["x"], obs["position"]["y"]))
            goal = task["goal"]

            # 1. Entity target visible? Acquire it.
            target = self._scan_target(env, task)
            if target is not None:
                obs, outcome, info = env.step(f"interact_with:{target['id']}")
                if outcome["done"]:
                    return outcome["success"], "" if outcome["success"] else "max_steps"
                continue

            # 2. Inside a building we only entered to search? Leave.
            if entered_building:
                result = self._exit_building(env, obs)
                if result is None:
                    return False, "stuck_in_building"
                obs, outcome = result
                entered_building = False
                continue

            # 3. Entity goal with unseen target: try the nearest
            #    unexplored building warp (target may be indoors).
            if goal["type"] in ("item", "flag"):
                action = self._untried_warp_action(env, obs)
                if action is not None:
                    before = obs["map"]["name"]
                    obs, outcome, _ = env.step(action)
                    if outcome["done"]:
                        return outcome["success"], "" if outcome["success"] else "max_steps"
                    if obs["map"]["name"] != before:
                        obs, outcome, done = self._settle(env)
                        if done:
                            return outcome["success"], "" if outcome["success"] else "max_steps"
                        if goal["type"] == "flag":
                            # The flag speech may still be playing; keep
                            # settling in the main loop (dialogue branch).
                            entered_building = False
                        else:
                            entered_building = True
                    continue

            # 4. Compass walk.
            action = self._compass_action(env, obs)
            x0, y0 = obs["position"]["x"], obs["position"]["y"]
            map0 = obs["map"]["name"]
            obs, outcome, info = env.step(action)
            if outcome["done"]:
                return outcome["success"], "" if outcome["success"] else "max_steps"
            dx, dy = COMPASS_DELTA[self.compass]
            progressed = ((obs["position"]["x"] - x0) * dx
                          + (obs["position"]["y"] - y0) * dy) > 0
            if progressed or obs["map"]["name"] != map0:
                self._edge_slide = 0
            unmoved = (obs["position"]["x"] == x0 and obs["position"]["y"] == y0
                       and obs["map"]["name"] == map0)
            if action.startswith("move_to") and (
                    info.get("invalid")
                    or (info.get("result", {}).get("result") not in
                        ("reached", "map_changed", "entered_battle", "entered_dialogue")
                        and unmoved)):
                target_xy = action.split(":", 1)[1].split(",")
                self.blocked.add((map0, int(target_xy[0]), int(target_xy[1])))
            elif action.startswith("drive:") and unmoved:
                # Edge burst against something solid. An NPC visible on
                # the tile ahead means a transient camp: park the tile in
                # the forgive-after-a-few-actions set. With no NPC in
                # sight, give the benefit of the doubt once (the camper
                # may have drifted off mid-burst) and only seal the tile
                # as a static wall on the SECOND bump.
                btn = action.split(":", 1)[1].split(",", 1)[0]
                bdx, bdy = {"up": (0, -1), "down": (0, 1),
                            "left": (-1, 0), "right": (1, 0)}[btn]
                tile = (map0, x0 + bdx, y0 + bdy)
                if self._npc_at(env, x0 + bdx, y0 + bdy) \
                        or self._npc_at(env, x0 + bdx * 2, y0 + bdy * 2):
                    self._npc_blocked[tile] = 6
                elif tile in self._bumped_once:
                    self._bumped_once.discard(tile)
                    self.blocked.add(tile)
                else:
                    self._bumped_once.add(tile)
                    self._npc_blocked[tile] = 6


class ButtonRandomWalk:
    """T1: seeded random button walk, north-biased, A-mash battles."""

    POLICY_NAME = "button_random_walk"

    def __init__(self, seed, weights=None, drive_frames=10):
        self.rng = random.Random(seed)
        self.buttons = ["up", "left", "right", "down", "a"]
        self.weights = weights or [0.45, 0.15, 0.15, 0.10, 0.15]
        self.drive_frames = drive_frames
        self.battles = 0
        self.battles_won = 0
        self._in_battle = False

    def run(self, env, task, frame_budget):
        start = env.frame_count()
        obs = env.client.observe()
        battle_iters = 0
        while True:
            if env.frame_count() - start > frame_budget:
                return False, "frame_cap"
            mode = obs["mode"]
            if mode == "battle":
                if not self._in_battle:
                    self.battles += 1
                    self._in_battle = True
                    battle_iters = 0
                battle_iters += 1
                if battle_iters > 400:  # ~2.4k frames of mashing: give up
                    return False, "battle_stuck"
                obs, outcome, _ = env.step("press:a")
                if outcome["done"]:
                    return outcome["success"], "" if outcome["success"] else "max_steps"
                obs, outcome, _ = env.step("step_frames:5")
                if outcome["done"]:
                    return outcome["success"], "" if outcome["success"] else "max_steps"
                if obs["mode"] != "battle":
                    self._in_battle = False
                    self.battles_won += 1
                continue
            if mode in ("dialogue", "menu"):
                obs, outcome, _ = env.step("press:a")
                if outcome["done"]:
                    return outcome["success"], "" if outcome["success"] else "max_steps"
                continue
            if mode == "transition":
                obs, outcome, _ = env.step("step_frames:10")
                if outcome["done"]:
                    return outcome["success"], "" if outcome["success"] else "max_steps"
                continue
            btn = self.rng.choices(self.buttons, self.weights)[0]
            obs, outcome, _ = env.step(f"drive:{btn},{self.drive_frames}")
            if outcome["done"]:
                return outcome["success"], "" if outcome["success"] else "max_steps"
