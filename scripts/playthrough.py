#!/usr/bin/env python3
"""Milestone-driven playthrough driver for open-pokered.

Drives a headless instance purely with button input (press/press_sequence/
step_frames) plus the observation/synchronization commands of the debug
protocol (get_state / wait_until / skip_dialogue), so every milestone is
reproducible end-to-end from a real power-on — no --skip-intro, --warp or
state seeding. Save goes to a throwaway path so the main menu always offers
a clean NEW GAME.

Pathfinding is local: tools/map_data.json carries each map's blocks and
passable-tile whitelist, crates/pokered-data/maps/*/map.json the
connections; BFS runs at tile resolution with live NPC blocking.

Usage:
    python3 scripts/playthrough.py [--port 9020] [--until m06] [--list]
"""
import argparse
import json
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from collections import deque
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
from debug_drive import DebugClient  # noqa: E402

BIN = ROOT / "target/debug/pokered-app"
FRAMES_PER_TILE = 8  # held frames to cross one tile (smoke-tested)
TAP_GAP = 6          # idle frames after a 1-frame tap (edge-trigger safety)

# ── static map data ─────────────────────────────────────────────────────
_MD = json.load(open(ROOT / "tools/map_data.json"))
BLOCKSETS = {b["tileset_id"]: b["blocks"] for b in _MD["blocksets"]}
MAPS = {m["name"]: m for m in _MD["maps"]}
CONNS = {}
for _p in sorted((ROOT / "crates/pokered-data/maps").iterdir()):
    _f = _p / "map.json"
    if _f.exists():
        _j = json.load(open(_f))
        CONNS[_j["name"]] = _j.get("connections") or {}

DELTA = {"up": (0, -1), "down": (0, 1), "left": (-1, 0), "right": (1, 0)}


def tile_at(map_name, x, y):
    """Sample the blockset tile id at tile coordinates (blockset_data
    half-block sampling, mirrors pokered-core collision.rs)."""
    m = MAPS[map_name]
    if not (0 <= x < m["width"] * 2 and 0 <= y < m["height"] * 2):
        return None
    block = m["blocks"][(y // 2) * m["width"] + (x // 2)]
    tiles = BLOCKSETS[m["tileset_id"]][block]
    return tiles[((y % 2) * 2 + 1) * 4 + (x % 2) * 2]


def walkable(map_name, x, y):
    """Tile-resolution passability: blockset tile whitelist (see
    pokered-data/src/collision.rs — anything not whitelisted blocks)."""
    m = MAPS[map_name]
    if not (0 <= x < m["width"] * 2 and 0 <= y < m["height"] * 2):
        return False
    return tile_at(map_name, x, y) in m["passable_tiles"]


# data/tilesets/tileset_headers.asm grass tiles per tileset name.
GRASS_TILES = {"Overworld": 0x52}

# special_terrain::is_outside_map (engine): only these tilesets flip last_map.
OUTSIDE_MAP_TILESETS = ("overworld", "plateau")


def is_grass(map_name, x, y):
    t = tile_at(map_name, x, y)
    return t is not None and t == GRASS_TILES.get(MAPS[map_name]["tileset_name"])


def find_grass(map_name, x0, y0, radius=12):
    """Nearest wild-encounter grass tile to (x0, y0)."""
    best = None
    for r in range(radius):
        for dy in range(-r, r + 1):
            for dx in range(-r, r + 1):
                if max(abs(dx), abs(dy)) != r:
                    continue
                x, y = x0 + dx, y0 + dy
                if is_grass(map_name, x, y):
                    return x, y
    return best


def warp_tiles(map_name):
    """All warp tiles on a map — stepping onto any of them warps, so
    pathfinding treats them as walls unless explicitly targeted."""
    return {(w["x"], w["y"]) for w in MAPS[map_name]["warps"]}


def grass_tiles(map_name):
    return {(x, y)
            for y in range(MAPS[map_name]["height"] * 2)
            for x in range(MAPS[map_name]["width"] * 2)
            if is_grass(map_name, x, y)}


def bfs(map_name, start, goal, blocked=frozenset()):
    """BFS over one map; `blocked` is a set of (x, y) tiles NPCs occupy.
    Returns [(tile, direction-of-arrival), …] from start to goal."""
    if start == goal:
        return [(start, None)]
    q = deque([start])
    prev = {start: None}  # node -> ((from_tile, direction taken))
    while q:
        cx, cy = q.popleft()
        for d, (dx, dy) in DELTA.items():
            n = (cx + dx, cy + dy)
            if n in prev or n in blocked or not walkable(map_name, *n):
                continue
            prev[n] = ((cx, cy), d)
            if n == goal:
                out = []
                c = n
                while prev[c] is not None:
                    p, dd = prev[c]
                    out.append((c, dd))
                    c = p
                return [(start, None)] + out[::-1]
            q.append(n)
    return None


_CONN_DIR = {"up": "north", "down": "south", "left": "west", "right": "east"}


def warp_edges_from(map_name, x, y, last_map=None):
    """Static warp edges for stepping onto tile (x, y) on map_name.

    Explicit-dest warps (dest_map set) resolve directly. Exit-mat warps
    (dest_map null) fire dynamically: in-game they warp to the map the
    player entered from (last_map — engine semantics: updated only when
    arriving at an outside tileset, see special_terrain
    is_outside_map). With faithful driving-side tracking there is
    exactly ONE candidate; parents are only a fallback before the first
    real transition."""
    out = []
    for w in MAPS[map_name]["warps"]:
        if w["x"] != x or w["y"] != y:
            continue
        if w.get("dest_map_name"):
            d = MAPS[w["dest_map_name"]]["warps"][w["dest_warp_id"]]
            out.append((w["dest_map_name"], d["x"], d["y"]))
        else:
            candidates = ([last_map] if last_map is not None
                          else PARENTS.get(map_name, ()))
            for parent in sorted(candidates):
                pm = MAPS[parent]
                if w["dest_warp_id"] < len(pm["warps"]):
                    d = pm["warps"][w["dest_warp_id"]]
                    out.append((parent, d["x"], d["y"]))
    return out


def _build_parents():
    """building name -> maps that have an entry warp into it."""
    parents = {}
    for m in MAPS.values():
        for w in m["warps"]:
            if w.get("dest_map_name"):
                parents.setdefault(w["dest_map_name"], set()).add(m["name"])
    return parents


PARENTS = _build_parents()

# Buildings the driver refuses to route through. The school house's mats
# are pair-collision/warp-tile mixups in the engine's warp semantics —
# entering it from the city is fine, but the exit's last_map resolution
# contradicts a static model, and the resulting plan-vs-engine mismatch
# creates an infinite city↔schoolhouse travel loop. Gameplay is
# unaffected; the driver simply treats it as a dead zone.
# The school house and the pokecenters are also excluded: the model's
# exit-mat edge (resolved via last_map) disagrees with the engine's
# runtime resolution often enough that the shortcut plan diverges and
# loops. Explicit nav_warp still enters them; cross-planning just never
# routes through.
NO_THROUGH = {"ViridianSchoolHouse", "ViridianPokecenter",
              "PewterPokecenter", "ViridianMart"}


def warps_at(map_name, x, y):
    return [w for w in MAPS[map_name]["warps"] if w["x"] == x and w["y"] == y]


def outward_dir(map_name, x, y, d):
    """True when walking direction `d` from (x, y) faces off the map edge
    — the engine's extra_warp_check (FacingEdge) for bottom/top/left/right
    exit mats: they only fire when the player steps toward the edge."""
    m = MAPS[map_name]
    dx, dy = DELTA[d]
    return ((dy < 0 and y == 0) or (dy > 0 and y == m["height"] * 2 - 1)
            or (dx < 0 and x == 0) or (dx > 0 and x == m["width"] * 2 - 1))


def cross_step(map_name, x, y, d):
    """One tile step, geometry only: the landed tile (may be a warp tile
    — taking the warp is modeled by bfs_cross expanding warp_edges_from
    for it). Mirrors dotzuki map_transitions for connection crossings:
    arrival coords shift by -2*offset (blocks)."""
    dx, dy = DELTA[d]
    nx, ny = x + dx, y + dy
    m = MAPS[map_name]
    w2, h2 = m["width"] * 2, m["height"] * 2
    if 0 <= nx < w2 and 0 <= ny < h2:
        if not walkable(map_name, nx, ny):
            return None
        return (map_name, nx, ny)
    conn = CONNS[map_name].get(_CONN_DIR[d])
    if conn is None:
        return None
    tgt, off = conn["targetMap"], conn["offset"] * 2
    tm = MAPS[tgt]
    if d == "up":
        tn = (max(x - off, 0), tm["height"] * 2 - 1)
    elif d == "down":
        tn = (max(x - off, 0), 0)
    elif d == "left":
        tn = (tm["width"] * 2 - 1, max(y - off, 0))
    else:
        tn = (0, max(y - off, 0))
    return (tgt, *tn) if walkable(tgt, *tn) else None


def _path_of(prev, start, goal):
    out = []
    c = goal
    while prev[c] is not None:
        par, d = prev[c]
        out.append((c, d))
        c = par
    return [start] + out[::-1]


def bfs_cross(map_name, start, goal_map, goal, blocked_maps=None,
              last_map=None):
    """BFS whose steps are plain directions. Stepping onto a warp tile
    takes the warp: the expansion replaces the landed tile with its warp
    destinations (doors fire immediately; bottom-edge exit mats fire via
    the extra edge-check). Exit mats resolve against `last_map` — the
    driver tracks it from real transitions, so the plan matches what the
    engine will actually do. Nodes are (map, x, y)."""
    blocked_maps = blocked_maps or {}
    s = (map_name, *start)
    t = (goal_map, *goal)
    if s == t:
        return [s]
    q = deque([s])
    prev = {s: None}
    while q:
        cm, cx, cy = q.popleft()
        for d in DELTA:
            landed = cross_step(cm, cx, cy, d)
            if landed is None:
                continue
            key = (landed[1], landed[2])
            if key in warp_tiles(landed[0]):
                mats = warps_at(*landed)
                # Exit mats (no dest_map) fire only when stepping TOWARD
                # the map edge — sideways steps onto them are plain tiles,
                # otherwise plans ping-pong on the mat (school house bug).
                if (all(not w.get("dest_map_name") for w in mats)
                        and not outward_dir(*landed, d)):
                    cands = [landed]
                else:
                    cands = [n for n in warp_edges_from(*landed, last_map)
                             if n != (cm, cx, cy)
                             and n[0] not in NO_THROUGH]
            else:
                cands = [landed]
            for n in cands:
                if n == s or n in prev:
                    continue
                if n[0] == cm and (n[1], n[2]) in blocked_maps.get(cm, ()):
                    continue
                prev[n] = ((cm, cx, cy), d)
                if n == t:
                    return _path_of(prev, s, n)
                q.append(n)
    return None


class NavError(RuntimeError):
    pass


class Game:
    def __init__(self, port=None, save_path=None, record_dir=None):
        self.run_dir = Path(tempfile.mkdtemp(prefix="pokered-run-"))
        self.log = open(self.run_dir / "game.log", "w")
        # Persistent save ONLY for --resume (Game(..., save_path=...));
        # a plain run must boot clean or the main menu offers CONTINUE.
        self.persistent = save_path is not None
        if save_path is None:
            save_path = self.run_dir / "play.sav"
        self.save_path = save_path
        # Port collisions with unrelated listening daemons (e.g. a proxy
        # bound to a wide range) silently answer TCP and then drop the
        # connection — which the driver reads as a game crash. Probe for
        # a free port instead of trusting a hardcoded number.
        if port is None:
            s = socket.socket()
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
            s.close()
        cmd = [str(BIN), "run", "--headless", "--debug-port", str(port),
               "--no-audio", "--save", str(save_path)]
        if record_dir is not None:
            Path(record_dir).mkdir(parents=True, exist_ok=True)
            cmd += ["--record-frames", str(record_dir)]
        self.proc = subprocess.Popen(
            cmd, cwd=str(ROOT), stdout=subprocess.DEVNULL, stderr=self.log)
        self.d = DebugClient(port)
        self.frame0 = None
        # Engine default at new game (screen.rs OverworldScreen::new).
        self.last_map = "PalletTown"
        self.observed_npcs = {}

    def checkpoint(self, mid):
        """Persist SaveData and record the completed milestone id. Only
        meaningful for resume runs; fresh runs keep throwaway saves."""
        assert self.persistent, "checkpoint outside a --resume run"
        r = self.d.cmd(cmd="save")
        assert r["ok"], r
        (ROOT / "scripts" / ".playthrough.marker").write_text(mid)

    def marker_at_least(self, mid):
        """True when the completed-milestone marker is >= `mid`."""
        m = (ROOT / "scripts" / ".playthrough.marker")
        if not m.exists():
            return False
        return self.milestone_index(m.read_text().strip()) >= \
            self.milestone_index(mid)

    @staticmethod
    def milestone_index(mid):
        for i, (m, _, _) in enumerate(MILESTONES):
            if m == mid:
                return i
        return -1

    def track_last_map(self, current_map):
        """Engine-faithful last_map tracking (screen.rs' PendingWarp
        save_last_map): updated only when the player arrives at a map
        with an outside tileset ("overworld" / "plateau" — see
        special_terrain::is_outside_map). Interior maps leave it as is,
        which is what makes the forest gate corridor work: last_map
        stays Route2 through SouthGate → Forest → NorthGate."""
        prev = getattr(self, "_prev_map", None)
        if prev is not None and current_map != prev:
            if MAPS[current_map]["tileset_name"] in OUTSIDE_MAP_TILESETS:
                self.last_map = current_map
        self._prev_map = current_map

    # ── protocol helpers ────────────────────────────────────────────────
    def st(self):
        return self.d.cmd(cmd="get_state")["data"]

    def wait(self, condition, max_frames=600, must=True):
        r = self.d.cmd(cmd="wait_until", condition=condition,
                       max_frames=max_frames)
        assert r["ok"], f"wait_until failed: {r}"
        if must:
            assert r["data"]["reached"], (
                f"condition '{condition}' not reached in {max_frames} frames")
        return r["data"]

    def tap(self, btn, gap=TAP_GAP):
        """One held frame then idle frames — menus are edge-triggered
        (a_just_pressed), so consecutive tap frames would read as a hold."""
        self.d.drive([btn], frames=1 + gap)

    def skip(self):
        r = self.d.cmd(cmd="skip_dialogue")
        assert r["ok"], r
        return r["data"]

    def step(self, n):
        return self.d.step(n)

    # ── observation ─────────────────────────────────────────────────────
    def pos(self):
        s = self.st()
        return s["map_name"], s["player_x"], s["player_y"]

    def live_npcs(self, map_name):
        data = self.d.cmd(cmd="get_npcs")["data"]
        npcs = data.get("npcs", data) if isinstance(data, dict) else data
        live = {(n["x"], n["y"]) for n in npcs
                if n.get("visible", True) and (n["x"], n["y"]) != (-1, -1)}
        # Remember every observed NPC tile: wandering NPCs patrol a
        # small band, and plans that thread it pinch indefinitely at
        # drive time. Learned bands are avoided by PREFERRED routes.
        self.observed_npcs.setdefault(map_name, set()).update(live)
        return live

    def npc_blocked(self, map_name):
        return self.live_npcs(map_name) | self.observed_npcs.get(map_name, set())

    def evidence(self, tag):
        s = self.st()
        print(f"[{tag}] frame={s['frame_count']} screen={s['screen']} "
              f"map={s['map_name']} pos=({s['player_x']},{s['player_y']}) "
              f"party={s['party_count']} money={s['money']} "
              f"effect={s['active_script_effect']}")
        return s

    # ── movement ────────────────────────────────────────────────────────
    def nav_to(self, x, y, map_name=None, tries=80):
        """Closed-loop BFS walk: re-localize after each straight segment so
        drift, ledges and NPC shuffles self-correct. Wild battles on the
        way (grass routes) are run from and the walk resumes."""
        for _ in range(tries):
            if self.st()["screen"] == "battle":
                s = self.st()
                prefer = ("fight" if s["script_awaiting_battle"]
                          else "run")
                self.battle_loop(prefer=prefer)
                self.cutscene()
            cm, cx, cy = self.pos()
            if map_name is not None and cm != map_name:
                raise NavError(f"warped out of {map_name} -> {cm}")
            if (cx, cy) == (x, y):
                return
            blocked = self.npc_blocked(cm) | (warp_tiles(cm) - {(x, y)})
            path = bfs(cm, (cx, cy), (x, y), blocked=blocked)
            if not path:
                # Learned NPC bands must never seal a map: retry with
                # live positions only.
                path = bfs(cm, (cx, cy), (x, y),
                           blocked=self.live_npcs(cm)
                           | (warp_tiles(cm) - {(x, y)}))
            if not path:
                raise NavError(f"no path in {cm}: ({cx},{cy})->({x},{y})")
            dirs = [d for _, d in path[1:]]
            i = 0
            import os as _os2
            while i < len(dirs):
                j = i
                while j + 1 < len(dirs) and dirs[j + 1] == dirs[i]:
                    j += 1
                tiles = j - i + 1
                if _os2.environ.get("PT_DEBUG"):
                    print(f"   [nto] {cm}({cx},{cy}) {dirs[i]}x{tiles} "
                          f"[-> {x},{y}]", flush=True)
                px0, py0 = self.pos()[1:]
                self.d.drive([dirs[i]] * (tiles * FRAMES_PER_TILE),
                             frames=tiles * FRAMES_PER_TILE + 4)
                i = j + 1
                s2 = self.st()
                if (s2["player_x"], s2["player_y"]) == (px0, py0):
                    print(f"   [ntoPINCH] at {cm}({px0},{py0})", flush=True)
                    self.step(60)
            self.step(8)
        raise NavError(f"nav_to({x},{y}) did not converge")

    def face(self, direction):
        """Turn in place: one held frame turns, walking needs more."""
        self.d.drive([direction], frames=1 + 12)

    def nav_to_map(self, x, y, map_name, tries=150, avoid_grass=True):
        """Cross-map closed-loop walk (connections included). Prefers a
        route that avoids wild-encounter grass when one exists (wilds
        interrupt the walk — sometimes fatally at low HP); falls back
        to any walkable path. Wild encounters that still happen are run
        from (or fought for trainers) and the walk re-localizes."""
        self.last_pinch = None
        self.pinch_count = 0
        for attempt in range(tries):
            if self.st()["screen"] == "battle":
                s = self.st()
                prefer = ("fight" if s["script_awaiting_battle"]
                          else "run")
                self.battle_loop(prefer=prefer)
                self.cutscene()
            cm, cx, cy = self.pos()
            self.track_last_map(cm)
            import os
            if os.environ.get("PT_DEBUG") and attempt % 10 == 0:
                print(f"   [nav {map_name}({x},{y}) try={attempt} "
                      f"at {cm}({cx},{cy}) last={self.last_map}]", flush=True)
            if cm == map_name and (cx, cy) == (x, y):
                return
            blocked = {cm: self.npc_blocked(cm)}
            path = None
            if avoid_grass:
                path = bfs_cross(cm, (cx, cy), map_name, (x, y),
                                 blocked_maps={
                                     cm: blocked[cm] | grass_tiles(cm)},
                                 last_map=self.last_map)
            if path is None:
                # Fallback: live NPC positions only — the learned bands
                # must never seal off a whole map (they patrol wide).
                path = bfs_cross(cm, (cx, cy), map_name, (x, y),
                                 blocked_maps={cm: self.live_npcs(cm)},
                                 last_map=self.last_map)
            if not path:
                raise NavError(f"no cross path: {cm}({cx},{cy}) "
                               f"-> {map_name}({x},{y}) "
                               f"blocked={sorted(blocked.get(cm, set()))}")
            import os as _os
            if _os.environ.get("PT_DEBUG") and attempt % 5 == 0:
                plan = []
                cur = None
                for node, how in path[1:10]:
                    m, px, py = node
                    if m != cur:
                        plan.append(f"[{m}]")
                        cur = m
                    plan.append(f"{how or 'START'}({px},{py})")
                print(f"   [navplan] {' '.join(plan)}", flush=True)
            steps = [how for _, how in path[1:]]
            # Block warp tiles of the current map for the walking phase,
            # except tiles the plan deliberately steps onto (sideways mat
            # steps) — otherwise straight drives can drift onto a door and
            # teleport into a building.
            plain_warp = {n[1:3] for n, how in path[1:]
                          if n[0] == cm and how in DELTA
                          and (n[1], n[2]) in warp_tiles(cm)}
            if cm in blocked:
                blocked[cm] |= (warp_tiles(cm) - plain_warp)
            # walk at most 3 tiles per segment, then re-check (a wild
            # battle or an unplanned warp may interrupt; the closed loop
            # re-localizes and re-plans either way)
            i = 0
            while i < len(steps):
                j = i
                while (j + 1 < len(steps) and steps[j + 1] == steps[i]
                       and j + 1 - i < 3):
                    j += 1
                tiles = j - i + 1
                held = tiles * FRAMES_PER_TILE
                # A blanket held tail turns 1-tile segments into 2-tile
                # strides: near doors that over-runs onto the warp tile
                # and re-warps (passing the Pewter PC door westbound did
                # exactly that). Idle-heavy tail instead when any tile of
                # the segment is adjacent to a warp tile.
                # path[0] is the bare start node; the rest are
                # (node, direction) pairs.
                seg_start = path[i] if i == 0 else path[i][0]
                seg_end = path[i + tiles][0]
                near_warp = False
                # n may be a (x, y) or (map, x, y) node depending on the
                # BFS flavour — coordinates are always the last two.
                for n in (seg_start, seg_end):
                    for w in warp_tiles(cm):
                        if abs(w[0] - n[-2]) <= 1 and abs(w[1] - n[-1]) <= 1:
                            near_warp = True
                if near_warp:
                    frames = held + 16          # all-idle tail: drift-safe
                else:
                    # Warp/connection-firing steps need the direction held
                    # at the step-completion frame; pad those only.
                    idx_after = i + tiles
                    if idx_after < len(path) and path[idx_after][0][0] != cm:
                        held += 32
                    frames = held + 8
                px0, py0 = self.pos()[1:]
                if _os.environ.get("PT_DEBUG"):
                    print(f"   [seg] {cm}({px0},{py0}) {steps[i]}x{tiles} "
                          f"held={held}", flush=True)
                self.d.drive([steps[i]] * held, frames=frames)
                i = j + 1
                s = self.st()
                if s["screen"] == "battle":
                    break
                # Pinch: the segment made no progress (a wandering NPC
                # holds the plan's next tile). Sidestep onto a free
                # perpendicular tile and re-plan — waiting alone does
                # not work when the NPC patrols inside the pinch column.
                if (s["player_x"], s["player_y"]) == (px0, py0):
                    if _os.environ.get("PT_DEBUG"):
                        data = self.d.cmd(cmd="get_npcs")["data"]
                        npcs = data.get("npcs", data) if isinstance(data, dict) else data
                        print("   [pinch] npcs:", [
                            (n.get("text_id"), n["x"], n["y"])
                            for n in npcs if n.get("visible", True)],
                            flush=True)
                        if self.pinch_count >= 3:
                            full = self.st()
                            print("   [pinch!]", {k: full[k] for k in (
                                "screen", "map_name", "player_x",
                                "player_y", "warp_fade", "script_running",
                                "player_movement_state")}, flush=True)
                    if self.last_pinch == (cm, cx, cy):
                        self.pinch_count += 1
                    else:
                        self.last_pinch = (cm, cx, cy)
                        self.pinch_count = 0
                    free = None
                    for d2 in ("left", "right", "up", "down"):
                        dx, dy = DELTA[d2]
                        n = (px0 + dx, py0 + dy)
                        if (walkable(cm, *n) and n not in
                                self.npc_blocked(cm)
                                and n not in warp_tiles(cm)):
                            free = d2
                            break
                    if self.pinch_count >= 3:
                        # Stuck at the same spot: flee along a long open
                        # corridor so the next plan cannot immediately
                        # drag the loop back (e.g. 5 tiles west of the
                        # Viridian fence band opens the south exit).
                        flee = None
                        for d2, (dx, dy) in DELTA.items():
                            ok = True
                            for k in range(1, 6):
                                n = (px0 + dx * k, py0 + dy * k)
                                if not walkable(cm, *n):
                                    ok = False
                                    break
                            if ok:
                                flee = d2
                                break
                        if flee:
                            self.d.drive([flee] * (5 * FRAMES_PER_TILE),
                                         frames=5 * FRAMES_PER_TILE + 8)
                            self.step(8)
                        else:
                            self.step(200)
                    elif free:
                        self.d.drive([free] * FRAMES_PER_TILE,
                                     frames=FRAMES_PER_TILE + 8)
                        self.step(6)
                    else:
                        # A wandering NPC pauses for hundreds of frames
                        # between steps; a short wait catches it mid-pause.
                        self.step(200)
                    break
            self.step(6)
        raise NavError(f"nav_to_map({map_name},{x},{y}) did not converge")

    def nav_warp(self, x, y, from_map, to_map=None, tries=30,
                 approach="walk"):
        """Walk onto the warp tile at (x, y) and wait for the map change.

        approach="down": bottom-edge exit mats only fire when the player
        faces the map edge with the direction still held (engine mirrors
        the original CheckWarps extra_warp_check), so path to (x, y-1)
        first and walk down through the mat."""
        if approach == "down":
            self.nav_to(x, y - 1, map_name=from_map, tries=tries)
            self.d.drive(["down"] * (2 * FRAMES_PER_TILE + 32),
                         frames=2 * FRAMES_PER_TILE + 40)
        else:
            try:
                self.nav_to(x, y, map_name=from_map, tries=tries)
            except NavError:
                # Model blocked (e.g. an NPC patrol sealed the single
                # approach): walk to an inward neighbor and long-hold
                # onto the warp instead.
                for d, (dx, dy) in DELTA.items():
                    inner = (x - dx, y - dy)
                    if walkable(from_map, *inner):
                        self.nav_to(*inner, map_name=from_map,
                                    tries=tries)
                        self.d.drive([d] * 40, frames=48)
                        break
                else:
                    raise
            # Landing on a warp tile fires only while a step completes
            # with the direction held (extra_warp_check); retry with a
            # long hold toward the map edge.
            for d, (dx, dy) in DELTA.items():
                if outward_dir(from_map, x, y, d):
                    self.d.drive([d] * 40, frames=48)
                    break
        for _ in range(40):
            cm, _, _ = self.pos()
            if cm != from_map:
                if to_map is not None:
                    assert cm == to_map, f"unexpected warp target {cm}"
                # Arrival may kick off an @load cutscene (e.g. the Mart
                # parcel hand-off) — settle it before returning control.
                assert self.cutscene(), f"{cm} on-enter cutscene stalled"
                return cm
            self.step(10)
        raise NavError(f"warp at ({x},{y}) never fired ({from_map})")

    # ── cutscenes & dialogue ────────────────────────────────────────────
    def cutscene(self, max_rounds=300):
        """Advance a scripted cutscene until control returns — or until the
        script hands off into battle (startBattle suspends the script, so
        control_ready never fires; that's a successful hand-off, not a
        stall). wait_until burns through non-dialogue effects; dialogue
        pages collapse via skip_dialogue. An open choice menu is NOT
        answered here — it needs a deliberate decision, so we fail
        loudly instead of spinning forever."""
        for _ in range(max_rounds):
            r = self.d.cmd(cmd="wait_until", condition="control_ready",
                           max_frames=240)
            if r["data"]["reached"]:
                return True
            state = r["data"]["state"]
            if state["screen"] == "battle":
                return True
            if state["choice"] is not None:
                raise NavError(f"cutscene blocked on choice "
                               f"{state['choice']['options']} "
                               f"(cursor {state['choice']['selected']})")
            if state["dialogue_state"] is not None:
                self.skip()
        return False

    def dialogue_then_choice(self, timeout_frames=3600):
        """After an interaction A-tap: collapse dialogue pages (and dismiss
        blocking dex-entry screens) until the option menu opens. The dex
        preview needs real A taps — wait_until steps with neutral input,
        so it would sit there forever."""
        for _ in range(timeout_frames // 60):
            s = self.st()
            if s["choice"] is not None:
                return s["choice"]
            if s["active_script_effect"] == "ShowPokedexEntry":
                self.tap("a", 10)
            elif s["dialogue_state"] is not None:
                self.skip()
            else:
                self.step(30)
        raise NavError("expected a choice menu, never opened")

    def choose(self, label):
        """Move the cursor to `label` in the open choice menu, press A.
        Matches case-insensitively (scripts use both YES/NO and Yes/No).
        The menu wraps both ways, so take the shorter arc."""
        ch = self.st()["choice"]
        assert ch is not None, "no choice open"
        opts = ch["options"]
        if label not in opts:
            label = next(o for o in opts if o.lower() == label.lower())
        idx = opts.index(label)
        n = len(ch["options"])
        delta = (idx - ch["selected"]) % n
        if delta:
            down = delta * 2 <= n
            for _ in range(delta if down else n - delta):
                self.d.drive(["down" if down else "up"], frames=12)
            s = self.st()["choice"]
            assert s["selected"] == idx, s
        self.tap("a", 10)

    def talk(self, post="control_ready", budget=600):
        """Interact with whatever we face, then settle the conversation."""
        self.tap("a", 20)
        if post == "choice_open":
            self.wait("choice_open", budget)
        else:
            self.cutscene()

    # ── party / training ────────────────────────────────────────────────
    def leader(self):
        return self.st()["party"][0]

    def heal_pokecenter(self, door, city, pc):
        """Enter the Pokecenter, heal at the nurse (YES), verify full HP,
        walk back out. `door` is the city-side warp tile."""
        dx, dy = door
        self.nav_to_map(dx, dy + 1, city)
        self.nav_warp(dx, dy, city, pc)
        self.nav_to(3, 3, map_name=pc)         # across the counter from
        self.face("up")                        # the nurse (3,1); the
        self.tap("a", 20)                      # counter row is solid
        for _ in range(40):
            s = self.st()
            if s["choice"] is not None:
                self.choose("YES")
                break
            if s["dialogue_state"] is not None:
                self.skip()
            else:
                self.step(20)
        assert self.cutscene(), "heal cutscene never finished"
        s = self.st()
        assert all(m["hp"] == m["max_hp"] for m in s["party"]), s["party"]
        if city == "PewterCity":
            # Go-out staged in reverse (the forest is the only link).
            self.nav_to(18, 34, map_name="PewterCity")
            self.d.drive(["down"] * 24, frames=28)
            self.step(8)
            self.nav_warp(3, 11, "Route2",
                          "ViridianForestNorthGate", approach="down")
            self.nav_to(5, 6, map_name="ViridianForestNorthGate")
            self.nav_warp(5, 7, "ViridianForestNorthGate",
                          "ViridianForest", approach="down")
            self.nav_to(17, 46, map_name="ViridianForest")
            self.nav_warp(17, 47, "ViridianForest",
                          "ViridianForestSouthGate")
            self.nav_to(4, 6, map_name="ViridianForestSouthGate")
            self.nav_warp(4, 7, "ViridianForestSouthGate",
                          "Route2", approach="down")
            self.nav_to(9, 49, map_name="Route2")
        self.nav_warp(3, 7, pc, city, approach="down")
        # We are standing ON the city door tile. Step EAST with a tail of
        # idle frames only (any held tail would drift back onto the door
        # and re-warp), then let normal navigation take over.
        self.d.drive(["right"] * FRAMES_PER_TILE, frames=24)
        self.step(6)
        # Step out of the door pocket; the city fence band can seal it.
        for _ in range(3):
            try:
                self.nav_to(26, 26, map_name=city)
                break
            except NavError:
                self.nav_warp(3, 7, pc, city, approach="down")
        else:
            self.nav_to(23, 26, map_name=city)

    PREFERRED_MOVES = ["VineWhip", "Ember", "Bubble", "WaterGun",
                       "ThunderShock", "Absorb", "RazorLeaf", "Tackle",
                       "Scratch", "Pound"]

    def _preferred_slot(self, moves):
        """Best usable slot in the LIVE fight menu, by preference order
        (STAB/typed damage first). Menu entries carry live PP — the
        save-data party is a battle-start snapshot, so mid-battle PP
        exists only here."""
        for want in self.PREFERRED_MOVES:
            for i, m in enumerate(moves):
                if m["move"] == want and m["pp"] > 0 and not m["disabled"]:
                    return i
        for i, m in enumerate(moves):
            if m["pp"] > 0 and not m["disabled"]:
                return i
        return None

    def _select_move(self):
        """Closed-loop FIGHT-menu selection: read the live menu (cursor +
        per-slot PP), walk the cursor to the best usable slot, press A.
        Re-reads after every cursor step, so a stale position or a
        rejected slot (No PP) self-corrects. Returns once the menu closes
        (turn executing) or the battle leaves MoveSelect."""
        for _ in range(24):
            s = self.st()
            if s["screen"] != "battle" or s["battle_phase"] != "MoveSelect":
                return
            menu = s.get("battle_moves")
            if not menu:
                self.step(4)
                continue
            moves = menu["moves"]
            want = self._preferred_slot(moves)
            if want is None:
                # No usable slot: the engine refuses to open this menu
                # (forced Struggle), so this is only a defensive exit.
                self.tap("b", 8)
                self.step(10)
                return
            n = len(moves)
            cur = menu["cursor"]
            if cur != want:
                delta = (want - cur) % n
                up = delta * 2 > n    # shorter arc; the menu wraps
                for _ in range(n - delta if up else delta):
                    self.d.drive(["up" if up else "down"], frames=10)
                continue
            self.tap("a", 4)
            self.step(10)

    def leave_grass(self, map_name):
        """BFS out of the grass patch to the nearest solid ground, then
        walk the short path (wandering NPCs and wild fights may occur
        mid-way; the caller's loop re-enters if the map changes)."""
        cm, cx, cy = self.pos()
        if cm != map_name or not is_grass(cm, cx, cy):
            return
        path = bfs(cm, (cx, cy), None) if False else None
        # nearest non-grass via BFS
        from collections import deque as _dq
        q = _dq([(cx, cy)])
        prev = {(cx, cy): None}
        goal = None
        while q:
            x0, y0 = q.popleft()
            for d, (dx, dy) in DELTA.items():
                n = (x0 + dx, y0 + dy)
                if n in prev or not walkable(cm, *n):
                    continue
                prev[n] = ((x0, y0), d)
                if not is_grass(cm, *n):
                    goal = n
                    q.clear()
                    break
                q.append(n)
        if goal is None:
            raise NavError(f"could not leave grass at {cm}({cx},{cy})")
        dirs = []
        cur = goal
        while prev[cur] is not None:
            p0, dd = prev[cur]
            dirs.append(dd)
            cur = p0
        dirs.reverse()
        for d in dirs:
            self.d.drive([d] * FRAMES_PER_TILE, frames=16)
        self.step(6)

    def train_until(self, level, map_name, spot, heal, max_cycles=400):
        """Wander over wild-encounter grass near `spot` fighting battles
        until the leader reaches `level`. Heals at `heal` (pokecenter
        door, city, pc) when HP drops below 40%; blackout self-heals and
        the loop re-navigates."""
        import time
        t0 = time.time()
        x, y = find_grass(map_name, *spot) or spot
        battles = 0
        print(f"[train] grass spot {map_name} ({x},{y}), "
              f"target L{level}", flush=True)
        for cyc in range(max_cycles):
            s = self.st()
            if s["screen"] == "battle":
                battles += 1
                self.battle_loop(prefer="fight")
                self.cutscene()
                continue
            mon = s["party"][0]
            if cyc % 25 == 0:
                print(f"[train] cyc={cyc} battles={battles} "
                      f"L{mon['level']} hp={mon['hp']}/{mon['max_hp']} "
                      f"({time.time()-t0:.0f}s)", flush=True)
            if mon["level"] >= level:
                print(f"[train] level {mon['level']} reached "
                      f"({time.time()-t0:.0f}s, {battles} battles)")
                return True
            if mon["hp"] < mon["max_hp"] * 0.6:
                print(f"[train] hp {mon['hp']}/{mon['max_hp']} -> heal",
                      flush=True)
                self.leave_grass(map_name)
                if map_name == "Route1":
                    # Outbound: grass -> Route1 north crossing -> city
                    # south lane -> PC door (proven m08 corridor).
                    self.nav_to(10, 1, map_name="Route1")
                    self.d.drive(["up"] * 24, frames=28)
                    self.step(8)
                    self.nav_to(23, 26, map_name="ViridianCity")
                elif heal[1] == "ViridianCity":
                    # Outbound staged: grass -> Route2 south edge ->
                    # crossing -> city north lane -> PC door.
                    self.nav_to(8, 70, map_name="Route2")
                    self.d.drive(["down"] * 24, frames=28)
                    self.step(8)
                    self.nav_to(20, 32, map_name="ViridianCity")
                self.heal_pokecenter(*heal)
                # Return leg staged through the patrol-free city lane
                # (same drift-resistance as m09's gate chain).
                if map_name == "Route1":
                    # Return: city south lane -> crossing -> grass.
                    self.nav_to(20, 32, map_name="ViridianCity")
                    self.nav_to(20, 33, map_name="ViridianCity")
                    self.d.drive(["down"] * 24, frames=28)
                    self.step(8)
                    self.nav_to(x, y, map_name, tries=120)
                elif heal[1] == "PewterCity":
                    # Return through the forest corridor in reverse — the
                    # only link between Route 2's sections. Staged gates
                    # (mirror of m09) instead of cross-BFS: the engine's
                    # last_map stays Route2 through the whole chain, so
                    # every mat fires deterministically.
                    self.nav_to(18, 34, map_name="PewterCity")
                    self.d.drive(["down"] * 24, frames=28)   # S connection
                    self.step(8)
                    self.nav_warp(3, 11, "Route2",
                                  "ViridianForestNorthGate",
                                  approach="down")
                    self.nav_to(5, 6, map_name="ViridianForestNorthGate")
                    self.nav_warp(5, 7, "ViridianForestNorthGate",
                                  "ViridianForest", approach="down")
                    self.nav_to(17, 46, map_name="ViridianForest")
                    self.nav_warp(17, 47, "ViridianForest",
                                  "ViridianForestSouthGate")
                    self.nav_to(4, 6, map_name="ViridianForestSouthGate")
                    self.nav_warp(4, 7, "ViridianForestSouthGate",
                                  "Route2", approach="down")
                    self.nav_to(x, y, map_name, tries=120)
                elif heal[1] == "ViridianCity":
                    self.nav_to(20, 32, map_name="ViridianCity")
                    self.nav_to(18, 1, map_name="ViridianCity")
                    self.d.drive(["up"] * 24, frames=28)   # N connection
                    self.step(8)
                    self.nav_to(x, y, map_name, tries=120)
                else:
                    try:
                        self.nav_to_map(x, y, map_name, tries=450)
                    except NavError:
                        self.step(300)
                        self.nav_to_map(x, y, map_name, tries=450)
                continue
            # wander: a vertical shuttle over the grass; wilds interrupt
            cm, cx, cy = self.pos()
            if cm != map_name:
                # Lost the map (blackout or a battle drift): stage the
                # return from wherever we are, same drift-proof pattern.
                if cm == "PewterCity":
                    self.nav_to(18, 34, map_name="PewterCity")
                    self.d.drive(["down"] * 24, frames=28)
                    self.step(8)
                    self.nav_warp(3, 11, "Route2",
                                  "ViridianForestNorthGate",
                                  approach="down")
                    self.nav_to(5, 6, map_name="ViridianForestNorthGate")
                    self.nav_warp(5, 7, "ViridianForestNorthGate",
                                  "ViridianForest", approach="down")
                    self.nav_to(17, 46, map_name="ViridianForest")
                    self.nav_warp(17, 47, "ViridianForest",
                                  "ViridianForestSouthGate")
                    self.nav_to(4, 6, map_name="ViridianForestSouthGate")
                    self.nav_warp(4, 7, "ViridianForestSouthGate",
                                  "Route2", approach="down")
                    self.nav_to(x, y, map_name, tries=120)
                elif cm == "ViridianCity":
                    self.nav_to(20, 32, map_name="ViridianCity")
                    self.nav_to(18, 1, map_name="ViridianCity")
                    self.d.drive(["up"] * 24, frames=28)
                    self.step(8)
                    self.nav_to(x, y, map_name, tries=120)
                else:
                    # Blackout at home / elsewhere: generic re-route with
                    # a generous budget (rare path; the closed loop wins
                    # eventually via the staged city legs).
                    try:
                        self.nav_to_map(x, y, map_name, tries=450)
                    except NavError:
                        self.step(300)
                        self.nav_to_map(x, y, map_name, tries=450)
                continue
            dy = 4 if cy <= y else -4
            self.d.drive(["down" if dy > 0 else "up"] * 32, frames=36)
            if self.st()["screen"] != "battle":
                self.d.drive(["up" if dy > 0 else "down"] * 32, frames=36)
        return False

    # ── battle ──────────────────────────────────────────────────────────
    def battle_loop(self, prefer="fight", max_iters=400):
        """Generic battle driver. prefer="run" picks RUN from the menu
        (wild encounters); falls back to FIGHT if escape keeps failing.
        The 2x2 menu clamps cursor movement, so up+left always lands on
        FIGHT and down+right on RUN regardless of saved cursor state."""
        fight = prefer == "fight"
        iters_in_mode = 0
        import os
        dbg = os.environ.get("PT_DEBUG")
        for it in range(max_iters):
            s = self.st()
            if dbg:
                print(f"   [battle it={it} fight={fight} mode_iters="
                      f"{iters_in_mode}] phase={s['battle_phase']!r} "
                      f"msg={s['battle_message']!r}", flush=True)
            # LOS trainer battles are not script-suspended, so the
            # caller's wild/run heuristic can't see them — detect the
            # trainer marker in the phase and commit to fighting.
            if not fight and "Trainer" in s["battle_phase"]:
                fight = True
            if s["screen"] != "battle":
                break
            ph = s["battle_phase"]
            if ph == "PlayerMenu":
                if not fight and iters_in_mode > 3:
                    fight = True          # escape keeps failing: brawl
                iters_in_mode += 1
                if fight:
                    self.d.drive(["up", "left"], frames=10)   # -> FIGHT
                    self.tap("a", 4)                          # open FIGHT
                    if not self._await_phase("MoveSelect", 120):
                        continue               # text ate the press; retry
                    self._select_move()
                else:
                    self.d.drive(["down", "right"], frames=10)  # -> RUN
                    self.tap("a", 4)                            # try escape
                self.step(30)
            elif ph == "MoveSelect":
                self._select_move()
                self.step(30)
            elif ph == "ShiftPrompt":
                self.tap("a", 8)               # default = switch in
                self.step(30)
            else:
                # Intro{..}/ShowingText{..}/TrainerVictory{..} pages
                self.tap("a", 10)
        self.wait("not_battle", 1800)

    def _await_phase(self, name, max_frames):
        for _ in range(max_frames // 4):
            s = self.st()
            if s["screen"] != "battle":
                return False
            if s["battle_phase"] == name:
                return True
            self.step(4)
        return False

    def close(self):
        try:
            self.d.close()
        finally:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=5)
            self.log.close()
            shutil.rmtree(self.run_dir, ignore_errors=True)


# ── milestones ──────────────────────────────────────────────────────────
def m01_boot(g):
    """Power-on → main menu → NEW GAME → Oak speech starts."""
    g.wait("screen=language-select", 1800)
    for _ in range(40):                      # A has an accept window; the
        g.tap("a", 30)                       # first press may be swallowed
        r = g.d.cmd(cmd="wait_until", condition="screen=title",
                    max_frames=120)
        if r["data"]["reached"]:
            break
    for _ in range(30):                      # title settles, then A
        g.tap("a", 10)
        if g.st()["screen"] == "main-menu":
            break
    assert g.st()["screen"] == "main-menu"
    for _ in range(20):                      # fresh save → NEW GAME (cursor 0)
        g.tap("a", 20)
        if g.st()["screen"] == "oak":
            break
    g.wait("screen=oak", 600)


def m02_oak_speech(g):
    """A through the speech; both name menus default (RED / BLUE)."""
    for _ in range(200):
        if g.st()["screen"] == "overworld":
            break
        g.tap("a", 18)
    g.wait("screen=overworld", 900)          # ShrinkPlayer
    s = g.evidence("m02")
    assert s["player_name"] == "RED", s["player_name"]
    assert s["map_name"] == "RedsHouse2F"


def m03_leave_house(g):
    g.nav_warp(7, 1, "RedsHouse2F", "RedsHouse1F")   # stairs down
    g.nav_warp(3, 7, "RedsHouse1F", "PalletTown",
               approach="down")                      # door mat: exit south
    g.evidence("m03")


def m04_oak_intercept(g):
    """North edge of Pallet Town → Oak interception → into the lab."""
    g.nav_to(11, 1, "PalletTown")
    assert g.cutscene(), "oak interception cutscene never finished"
    s = g.st()
    assert s["map_name"] == "OaksLab", s["map_name"]
    g.evidence("m04")


def m05_take_starter(g, which="bulbasaur"):
    balls = {"bulbasaur": (8, 3), "squirtle": (7, 3), "charmander": (6, 3)}
    bx, by = balls[which]
    g.nav_to(bx, by + 1)                     # stand below the ball
    g.face("up")
    g.tap("a", 20)                           # dex-preview dialogue
    ch = g.dialogue_then_choice()            # pages → "Do you want X?"
    assert ch["options"] == ["YES", "NO"], ch
    g.choose("YES")
    assert g.cutscene(), "starter cutscene never finished"
    s = g.evidence("m05")
    assert s["party_count"] == 1, s


def m06_rival_battle(g):
    g.nav_to(5, 6, "OaksLab")                # trigger row -> rival challenge
    assert g.cutscene(), "rival challenge cutscene never finished"
    g.wait("screen=battle", 900)
    g.battle_loop()
    g.wait("not_battle", 1800)
    assert g.cutscene(), "post-battle script never settled"
    g.evidence("m06")


def m07_mart_parcel(g):
    """Exit the lab, cross Route 1 to Viridian, collect Oak's parcel."""
    g.nav_warp(5, 11, "OaksLab", "PalletTown", approach="down")   # lab door
    g.nav_to_map(20, 26, "ViridianCity")         # west avenue (patrol-free)
    g.nav_to(20, 32, map_name="ViridianCity")    # south lane x=20-21
    g.nav_to_map(29, 20, "ViridianCity")         # below mart
    g.nav_warp(29, 19, "ViridianCity", "ViridianMart")            # @load: parcel
    g.evidence("m07-in-mart")
    g.nav_warp(4, 7, "ViridianMart", "ViridianCity", approach="down")
    g.evidence("m07-back-outside")


def m08_deliver_parcel(g):
    """Back to Pallet Town, hand the parcel to Oak, receive the POKéDEX.
    The Viridian return leg is staged through the patrol-free x=23
    corridor — the BFS-shortest route crosses the wandering youngster's
    patrol band and pinches indefinitely."""
    g.nav_to(20, 32, map_name="ViridianCity")    # south lane x=20-21
    g.evidence("m08-southlane")
    g.nav_to_map(12, 12, "PalletTown")           # Route1 crossing
    g.evidence("m08-back-home")
    g.nav_warp(12, 11, "PalletTown", "OaksLab")
    g.nav_to(5, 3, map_name="OaksLab")
    g.face("up")
    g.tap("a", 20)
    assert g.cutscene(), "parcel delivery cutscene never finished"
    g.evidence("m08-delivered")
    # Post-delivery, Oak's talk flips to the POKéDEX rating branch —
    # sample one line as evidence the flag actually flipped.
    g.tap("a", 30)
    g.d.cmd(cmd="wait_until", condition="dialogue_ready", max_frames=300)
    s = g.st()
    print(f"[m08] oak-after-delivery: {s.get('dialogue') or s['dialogue_state']}")
    g.skip()
    g.cutscene()


def m09_to_pewter(g):
    """North through Route 2, Viridian Forest (trainer LOS fights), to
    Pewter City — the first gym town. Gate houses are traversed by the
    warp-aware BFS automatically. Heal at Viridian's Pokecenter first so
    the forest trainers are faced at full HP (a loss means a blackout
    home and a full-journey retry)."""
    g.nav_warp(5, 11, "OaksLab", "PalletTown", approach="down")
    # Explicit gate chain: cross-BFS routing through the forest depends on
    # a last_map that drifts from the engine's, which can detour the plan
    # back through Pallet Town (no path to Pewter). Staged nav_warp calls
    # execute the verified corridor deterministically instead.
    g.nav_to_map(3, 44, "Route2")                     # below south gate
    g.nav_warp(3, 43, "Route2", "ViridianForestSouthGate")
    g.nav_to(5, 1, map_name="ViridianForestSouthGate")
    g.nav_warp(5, 0, "ViridianForestSouthGate", "ViridianForest")
    g.nav_to(1, 1, map_name="ViridianForest")
    g.nav_warp(1, 0, "ViridianForest", "ViridianForestNorthGate")
    g.nav_to(5, 1, map_name="ViridianForestNorthGate")
    g.nav_warp(5, 0, "ViridianForestNorthGate", "Route2")
    g.nav_to_map(16, 29, "PewterCity")
    g.evidence("m09")


def m10_brock(g):
    """Grind to Vine Whip (L13) in Route 2 grass, heal, then beat Brock
    for the BOULDERBADGE."""
    g.evidence("m10-arrived")
    # Heal at VIRIDIAN, not Pewter: Route 2's south section connects
    # straight to Viridian City (proven corridor, no forest gates), so
    # the training round trip stays short and deterministic.
    heal = ((23, 25), "ViridianCity", "ViridianPokecenter")
    # Relocate to the Route 1 training grass next to Viridian: the only
    # long walk is the reverse forest chain; training/heal round trips
    # stay on the proven Viridian corridor.
    forest_corridor_back(g)
    g.nav_to(8, 70, map_name="Route2")
    g.d.drive(["down"] * 24, frames=28)   # -> city north border (18,0)
    g.step(8)
    g.nav_to(18, 1, map_name="ViridianCity")
    g.nav_to(20, 32, map_name="ViridianCity")
    g.nav_to(20, 33, map_name="ViridianCity")
    g.d.drive(["down"] * 24, frames=28)
    g.step(8)
    g.nav_to(12, 7, map_name="Route1")
    assert g.train_until(13, "Route1", (12, 7), heal), "training stalled"
    g.evidence("m10-trained")
    # Gym attempts: a loss blacks out to the Viridian fly point (the last
    # heal), so each retry is the same heal -> corridor -> gym chain. The
    # badge flag — not the battle outcome — is the success check.
    for attempt in range(3):
        g.heal_pokecenter(*heal)
        # Stage the town hop: city north crossing -> forest corridor (m09
        # outbound chain) -> Pewter. Cross-BFS town hops drift; the chain
        # is the verified path.
        g.nav_to(20, 32, map_name="ViridianCity")
        g.nav_to(18, 1, map_name="ViridianCity")
        g.d.drive(["up"] * 24, frames=28)    # N connection -> Route2 (8,71)
        g.step(8)
        forest_corridor_out(g)
        g.nav_to_map(16, 18, "PewterCity")   # below the gym door (16,17)
        g.nav_warp(16, 17, "PewterCity", "PewterGym")
        g.nav_to(4, 2, map_name="PewterGym")  # below Brock (4,1)
        g.face("up")
        g.tap("a", 20)                        # Brock's challenge speech
        assert g.cutscene(), "Brock challenge cutscene never finished"
        g.wait("screen=battle", 900)
        g.battle_loop(prefer="fight")
        g.wait("not_battle", 1800)
        assert g.cutscene(), "badge ceremony never finished"
        flags = g.d.cmd(cmd="get_flags")["data"]
        if flags.get("EVENT_BEAT_BROCK"):
            g.evidence("m10")
            return
        print(f"[m10] attempt {attempt + 1} lost — blacked out, retrying",
              flush=True)
    raise NavError("Brock never beaten in 3 attempts")


def forest_corridor_out(g):
    """Overworld Route2-south -> forest gates -> Route2-north (m09's
    verified chain, shared by the m10 heal round trips)."""
    g.nav_to_map(3, 44, "Route2")
    g.nav_warp(3, 43, "Route2", "ViridianForestSouthGate")
    g.nav_to(5, 1, map_name="ViridianForestSouthGate")
    g.nav_warp(5, 0, "ViridianForestSouthGate", "ViridianForest")
    g.nav_to(1, 1, map_name="ViridianForest")
    g.nav_warp(1, 0, "ViridianForest", "ViridianForestNorthGate")
    g.nav_to(5, 1, map_name="ViridianForestNorthGate")
    g.nav_warp(5, 0, "ViridianForestNorthGate", "Route2")


def forest_corridor_back(g):
    """PewterCity -> south crossing -> NorthGate -> forest -> SouthGate ->
    Route2 south (the m09 chain in reverse; used by m10's relocation)."""
    g.nav_to(18, 34, map_name="PewterCity")
    g.d.drive(["down"] * 24, frames=28)
    g.step(8)
    g.nav_warp(3, 11, "Route2", "ViridianForestNorthGate", approach="down")
    g.nav_to(5, 6, map_name="ViridianForestNorthGate")
    g.nav_warp(5, 7, "ViridianForestNorthGate", "ViridianForest",
               approach="down")
    g.nav_to(17, 46, map_name="ViridianForest")
    g.nav_warp(17, 47, "ViridianForest", "ViridianForestSouthGate")
    g.nav_to(4, 6, map_name="ViridianForestSouthGate")
    g.nav_warp(4, 7, "ViridianForestSouthGate", "Route2", approach="down")


MILESTONES = [
    ("m01", "boot to NEW GAME / Oak speech", m01_boot),
    ("m02", "Oak speech + default names → bedroom", m02_oak_speech),
    ("m03", "leave home → Pallet Town", m03_leave_house),
    ("m04", "Oak interception → OaksLab", m04_oak_intercept),
    ("m05", "take starter (bulbasaur)", m05_take_starter),
    ("m06", "first rival battle", m06_rival_battle),
    ("m07", "Route 1 → Viridian Mart parcel", m07_mart_parcel),
    ("m08", "deliver parcel → POKéDEX", m08_deliver_parcel),
    ("m09", "Viridian Forest → Pewter City", m09_to_pewter),
    ("m10", "train + Boulder Badge (Brock)", m10_brock),
]


def state_done(mid, g):
    """Replay-safety: the checkpoint marker is the single authority —
    after a preserved save the game resumes mid-run, and state predicates
    (overworld+RED, OaksLab...) are satisfied by ANY later stage, so only
    the marker decides what already happened."""
    return g.marker_at_least(mid)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=9020)
    ap.add_argument("--until", default=None, help="stop after this milestone")
    ap.add_argument("--starter", default="bulbasaur",
                    choices=["bulbasaur", "squirtle", "charmander"])
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--resume", action="store_true",
                    help="use the persistent .playthrough.sav + marker: "
                         "skip milestones already satisfied")
    ap.add_argument("--record", default=None, metavar="DIR",
                    help="record every rendered frame to DIR (passes "
                         "--record-frames to the game); assemble with e.g. "
                         "ffmpeg -framerate 240 -i frame-%%06d.png -r 60 out.mp4")
    args = ap.parse_args()

    if args.list:
        for mid, desc, _ in MILESTONES:
            print(f"{mid}: {desc}")
        return

    g = Game(args.port, save_path=(ROOT / "scripts" / ".playthrough.sav")
             if args.resume else None, record_dir=args.record)
    t0 = time.time()
    try:
        for mid, desc, fn in MILESTONES:
            if args.resume and state_done(mid, g):
                print(f"== {mid}: {desc}")
                print(f"   skipped (state/marker already satisfied)")
                continue
            print(f"== {mid}: {desc}")
            if mid == "m05":
                fn(g, args.starter)
            else:
                fn(g)
            print(f"   done ({time.time()-t0:.1f}s wall)")
            if g.persistent:
                g.checkpoint(mid)
            if args.until == mid:
                break
        print("PLAYTHROUGH REACHED REQUESTED MILESTONE")
    finally:
        g.close()


if __name__ == "__main__":
    main()
