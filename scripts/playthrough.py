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


def walkable(map_name, x, y):
    """Tile-resolution passability: blockset tile whitelist (see
    pokered-data/src/collision.rs — anything not whitelisted blocks)."""
    m = MAPS[map_name]
    if not (0 <= x < m["width"] * 2 and 0 <= y < m["height"] * 2):
        return False
    block = m["blocks"][(y // 2) * m["width"] + (x // 2)]
    tiles = BLOCKSETS[m["tileset_id"]][block]
    tile = tiles[((y % 2) * 2 + 1) * 4 + (x % 2) * 2]
    return tile in m["passable_tiles"]


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


class NavError(RuntimeError):
    pass


class Game:
    def __init__(self, port):
        self.run_dir = Path(tempfile.mkdtemp(prefix="pokered-run-"))
        self.log = open(self.run_dir / "game.log", "w")
        self.proc = subprocess.Popen(
            [str(BIN), "run", "--headless", "--debug-port", str(port),
             "--save", str(self.run_dir / "play.sav")],
            cwd=str(ROOT), stdout=subprocess.DEVNULL, stderr=self.log)
        self.d = DebugClient(port)
        self.frame0 = None

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

    def npc_blocked(self, map_name):
        data = self.d.cmd(cmd="get_npcs")["data"]
        npcs = data.get("npcs", data) if isinstance(data, dict) else data
        return {(n["x"], n["y"]) for n in npcs
                if n.get("visible", True) and (n["x"], n["y"]) != (-1, -1)}

    def evidence(self, tag):
        s = self.st()
        print(f"[{tag}] frame={s['frame_count']} screen={s['screen']} "
              f"map={s['map_name']} pos=({s['player_x']},{s['player_y']}) "
              f"party={s['party_count']} money={s['money']} "
              f"effect={s['active_script_effect']}")
        return s

    # ── movement ────────────────────────────────────────────────────────
    def nav_to(self, x, y, map_name=None, tries=30):
        """Closed-loop BFS walk: re-localize after each straight segment so
        drift, ledges and NPC shuffles self-correct."""
        for _ in range(tries):
            cm, cx, cy = self.pos()
            if map_name is not None and cm != map_name:
                raise NavError(f"warped out of {map_name} -> {cm}")
            if (cx, cy) == (x, y):
                return
            path = bfs(cm, (cx, cy), (x, y), blocked=self.npc_blocked(cm))
            if not path:
                raise NavError(f"no path in {cm}: ({cx},{cy})->({x},{y})")
            dirs = [d for _, d in path[1:]]
            i = 0
            while i < len(dirs):
                j = i
                while j + 1 < len(dirs) and dirs[j + 1] == dirs[i]:
                    j += 1
                tiles = j - i + 1
                self.d.drive([dirs[i]] * (tiles * FRAMES_PER_TILE),
                             frames=tiles * FRAMES_PER_TILE + 4)
                i = j + 1
            self.step(8)
        raise NavError(f"nav_to({x},{y}) did not converge")

    def face(self, direction):
        """Turn in place: one held frame turns, walking needs more."""
        self.d.drive([direction], frames=1 + 12)

    def nav_warp(self, x, y, from_map, to_map=None, tries=30,
                 approach="walk"):
        """Walk onto the warp tile at (x, y) and wait for the map change.

        approach="down": bottom-edge exit mats only fire when the player
        faces the map edge with the direction still held (engine mirrors
        the original CheckWarps extra_warp_check), so path to (x, y-1)
        first and walk down through the mat."""
        if approach == "down":
            self.nav_to(x, y - 1, map_name=from_map, tries=tries)
            self.d.drive(["down"] * (2 * FRAMES_PER_TILE),
                         frames=2 * FRAMES_PER_TILE + 8)
        else:
            self.nav_to(x, y, map_name=from_map, tries=tries)
        for _ in range(40):
            cm, _, _ = self.pos()
            if cm != from_map:
                if to_map is not None:
                    assert cm == to_map, f"unexpected warp target {cm}"
                self.wait("control_ready", 600)
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
        The menu wraps both ways, so take the shorter arc."""
        ch = self.st()["choice"]
        assert ch is not None, "no choice open"
        idx = ch["options"].index(label)
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

    # ── battle ──────────────────────────────────────────────────────────
    def battle_loop(self, max_iters=400):
        """Generic battle driver: FIGHT + first move, A through text.
        Assumes control starts at (or before) the battle's PlayerMenu."""
        for _ in range(max_iters):
            s = self.st()
            if s["screen"] != "battle":
                break
            ph = s["battle_phase"]
            if ph == "PlayerMenu":
                self.tap("a", 4)               # FIGHT (cursor 0, untouched)
                if not self._await_phase("MoveSelect", 120):
                    continue                    # text popped up first
                self.tap("a", 4)               # first move
                self.step(30)
            elif ph == "MoveSelect":
                self.tap("a", 4)
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
            self.proc.wait(timeout=10)
            self.log.close()
            shutil.rmtree(self.run_dir, ignore_errors=True)


# ── milestones ──────────────────────────────────────────────────────────
def m01_boot(g):
    """Power-on → main menu → NEW GAME → Oak speech starts."""
    g.wait("screen=language-select", 1800)
    g.tap("a", 10)
    for _ in range(60):                      # intro auto-plays to title
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


MILESTONES = [
    ("m01", "boot to NEW GAME / Oak speech", m01_boot),
    ("m02", "Oak speech + default names → bedroom", m02_oak_speech),
    ("m03", "leave home → Pallet Town", m03_leave_house),
    ("m04", "Oak interception → OaksLab", m04_oak_intercept),
    ("m05", "take starter (bulbasaur)", m05_take_starter),
    ("m06", "first rival battle", m06_rival_battle),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=9020)
    ap.add_argument("--until", default=None, help="stop after this milestone")
    ap.add_argument("--starter", default="bulbasaur",
                    choices=["bulbasaur", "squirtle", "charmander"])
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    if args.list:
        for mid, desc, _ in MILESTONES:
            print(f"{mid}: {desc}")
        return

    g = Game(args.port)
    t0 = time.time()
    try:
        for mid, desc, fn in MILESTONES:
            print(f"== {mid}: {desc}")
            if mid == "m05":
                fn(g, args.starter)
            else:
                fn(g)
            print(f"   done ({time.time()-t0:.1f}s wall)")
            if args.until == mid:
                break
        print("PLAYTHROUGH REACHED REQUESTED MILESTONE")
    finally:
        g.close()


if __name__ == "__main__":
    main()
