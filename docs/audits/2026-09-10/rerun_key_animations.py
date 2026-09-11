#!/usr/bin/env python3
"""Re-record the key-animation matrix with raw-frame manifests.

Reference capture uses PyBoy and an official DEBUG ROM state. Current capture
uses pokered-app's headless debug server plus ``press_timeline`` so the verdict
window never depends on network round-trip timing or recursive ``step_frames``.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import re
import socket
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))

from debug_drive import DebugClient  # noqa: E402
from playthrough import MAPS, bfs, bfs_cross, warp_tiles  # noqa: E402


REFERENCE_ROM_SHA1 = "5b1456177671b79b263c614ea0e7cc9ac542e9c4"


REFERENCE_LENGTHS = {
    "battle-entry": 700,
    "fly-departure-arrival": 420,
    "cut-tree": 480,
    "surf-entry": 420,
    "ledge-down": 80,
}

CURRENT_SNAPSHOTS = {
    "fly-departure-arrival": ROOT / "docs/audits/2026-09-10/fixtures/current/field.json",
    "cut-tree": ROOT / "docs/audits/2026-09-10/fixtures/current/cut.json",
    "surf-entry": ROOT / "docs/audits/2026-09-10/fixtures/current/surf.json",
    "ledge-down": ROOT / "docs/audits/2026-09-10/fixtures/current/ledge.json",
    "battle-entry": ROOT / "docs/audits/2026-09-10/fixtures/current/field.json",
}

CURRENT_SETUP_FRAME = 60
CURRENT_TRIGGER_FRAME = 300


def timeline(length: int, presses: dict[int, str]) -> list[str | None]:
    result: list[str | None] = [None] * length
    for frame, button in presses.items():
        result[frame] = button
    return result


TIMELINES = {
    "battle-entry": timeline(
        REFERENCE_LENGTHS["battle-entry"], dict.fromkeys(range(2), "b")
    ),
    "fly-departure-arrival": timeline(
        REFERENCE_LENGTHS["fly-departure-arrival"], {0: "a"}
    ),
    "cut-tree": timeline(REFERENCE_LENGTHS["cut-tree"], {0: "a", 299: "a", 359: "a"}),
    "surf-entry": timeline(REFERENCE_LENGTHS["surf-entry"], {0: "a", 239: "a", 299: "a"}),
    # The Game Boy overworld samples direction less often than menu input;
    # hold Down for four raw frames so the same physical action is observed.
    "ledge-down": timeline(
        REFERENCE_LENGTHS["ledge-down"], dict.fromkeys(range(4), "down")
    ),
}


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def parse_symbols(path: Path) -> dict[str, int]:
    result = {}
    for line in path.read_text().splitlines():
        match = re.match(r"00:([0-9a-fA-F]{4})\s+(\S+)$", line)
        if match:
            result[match.group(2)] = int(match.group(1), 16)
    required = {
        "wCurMap",
        "wXCoord",
        "wYCoord",
        "wPlayerDirection",
        "wWalkCounter",
        "wTileInFrontOfPlayer",
        "wIsInBattle",
        "wCurOpponent",
        "wBattleType",
        "wWalkBikeSurfState",
        "wPlayerJumpingYScreenCoordsIndex",
        "wStatusFlags5",
        "wStatusFlags6",
        "wRepelRemainingSteps",
        "wSpritePlayerStateData1",
    }
    missing = sorted(required - result.keys())
    if missing:
        raise RuntimeError(f"reference symbol file lacks: {missing}")
    return result


class ReferenceHarness:
    def __init__(self, rom: Path, world_state: Path, symbols: Path):
        from pyboy import PyBoy

        self.PyBoy = PyBoy
        self.rom = rom
        self.world_state = world_state
        self.sym = parse_symbols(symbols)
        self.map_names = {entry["id"]: name for name, entry in MAPS.items()}

    def new(self):
        return self.PyBoy(str(self.rom), window="null", sound_emulated=False)

    def load_world(self):
        pyboy = self.new()
        with self.world_state.open("rb") as state:
            pyboy.load_state(state)
        return pyboy

    @staticmethod
    def tap(pyboy, button: str, frames: int = 30, hold: int = 2) -> None:
        if frames < hold:
            raise ValueError("tap duration must include its hold frames")
        pyboy.button_press(button)
        pyboy.tick(hold)
        pyboy.button_release(button)
        pyboy.tick(frames - hold)

    @staticmethod
    def face(pyboy, button: str) -> None:
        """Hold a direction long enough for the overworld input poll to see it."""
        pyboy.button_press(button)
        pyboy.tick(5)
        pyboy.button_release(button)
        pyboy.tick(5)

    @staticmethod
    def save_state(pyboy) -> bytes:
        state = io.BytesIO()
        pyboy.save_state(state)
        return state.getvalue()

    @staticmethod
    def load_state(pyboy, state: bytes) -> None:
        pyboy.load_state(io.BytesIO(state))

    def position(self, pyboy) -> tuple[str, int, int]:
        memory = pyboy.memory
        map_id = memory[self.sym["wCurMap"]]
        return (
            self.map_names.get(map_id, f"Map{map_id}"),
            memory[self.sym["wXCoord"]],
            memory[self.sym["wYCoord"]],
        )

    def set_repel(self, pyboy) -> None:
        pyboy.memory[self.sym["wRepelRemainingSteps"]] = 0xFF

    def step_to(self, pyboy, button: str, target: tuple[str, int, int]) -> None:
        pyboy.button_press(button)
        for _ in range(100):
            pyboy.tick()
            if self.position(pyboy) == target:
                break
        else:
            raise RuntimeError(f"reference navigation {button} failed: {self.position(pyboy)} -> {target}")
        pyboy.button_release(button)
        pyboy.tick(2)

    def walk_same_map(self, pyboy, map_name: str, target: tuple[int, int]) -> None:
        start = self.position(pyboy)
        if start[0] != map_name:
            raise RuntimeError(f"expected {map_name}, got {start}")
        blocked = {(npc["x"], npc["y"]) for npc in MAPS[map_name]["npcs"]}
        blocked |= warp_tiles(map_name)
        path = bfs(map_name, start[1:], target, blocked)
        if not path:
            raise RuntimeError(f"no reference path: {start} -> {map_name} {target}")
        for tile, button in path[1:]:
            self.step_to(pyboy, button, (map_name, *tile))

    def walk_cross_map(self, pyboy, goal_map: str, target: tuple[int, int]) -> None:
        start = self.position(pyboy)
        blocked = {
            name: {(npc["x"], npc["y"]) for npc in MAPS[name]["npcs"]}
            for name in (start[0], goal_map)
        }
        path = bfs_cross(
            start[0],
            start[1:],
            goal_map,
            target,
            blocked_maps=blocked,
            excluded_maps=("ViridianForestSouthGate",),
        )
        if not path:
            raise RuntimeError(f"no cross-map reference path: {start} -> {goal_map} {target}")
        for node, button in path[1:]:
            self.step_to(pyboy, button.removeprefix("jump_"), node)

    def open_field_action(
        self, pyboy, move_index: int, *, pokemon_cursor_already_selected: bool = False
    ) -> None:
        sequence = [("start", 30)]
        if not pokemon_cursor_already_selected:
            sequence.append(("down", 20))
        sequence.extend((("a", 40), ("a", 40)))
        for button, frames in sequence:
            self.tap(pyboy, button, frames)
        for _ in range(move_index):
            self.tap(pyboy, "down", 20)

    def fly_to(self, pyboy, list_steps: int, expected: tuple[str, int, int]) -> None:
        self.open_field_action(pyboy, 0)
        self.tap(pyboy, "a", 100)
        for _ in range(list_steps):
            self.tap(pyboy, "up", 30)
        self.tap(pyboy, "a", 500)
        if self.position(pyboy) != expected:
            raise RuntimeError(f"reference FLY setup landed at {self.position(pyboy)}, expected {expected}")

    def prepare(self, scenario: str) -> bytes:
        if scenario == "battle-entry":
            pyboy = self.new()
            pyboy.tick(500)
            self.tap(pyboy, "start", 120)
            self.tap(pyboy, "start", 120)
            for _ in range(8):
                self.tap(pyboy, "select", 60)
            # FIGHT first creates the player's debug Rhydon and asks for a
            # nickname. Advance to the YES/NO choice and save there; B is the
            # actual battle trigger.
            self.tap(pyboy, "a", 180)
            self.tap(pyboy, "b", 60)
        else:
            pyboy = self.load_world()
            self.set_repel(pyboy)
            if scenario == "fly-departure-arrival":
                self.open_field_action(pyboy, 0)
                self.tap(pyboy, "a", 100)
                self.tap(pyboy, "up", 30)
            elif scenario == "surf-entry":
                self.walk_same_map(pyboy, "PalletTown", (5, 13))
                self.face(pyboy, "down")
                self.open_field_action(pyboy, 2)
            elif scenario == "cut-tree":
                self.fly_to(pyboy, 5, ("VermilionCity", 11, 4))
                self.set_repel(pyboy)
                self.walk_same_map(pyboy, "VermilionCity", (15, 17))
                self.face(pyboy, "down")
                if pyboy.memory[self.sym["wTileInFrontOfPlayer"]] != 0x3D:
                    raise RuntimeError("CUT setup is not facing tile 0x3d")
                # FLY leaves the start-menu cursor on POKEMON, so another
                # Down would enter the bag and accidentally use BICYCLE.
                self.open_field_action(
                    pyboy, 1, pokemon_cursor_already_selected=True
                )
            elif scenario == "ledge-down":
                self.fly_to(pyboy, 1, ("ViridianCity", 23, 26))
                self.set_repel(pyboy)
                self.walk_cross_map(pyboy, "Route1", (10, 4))
            else:
                raise ValueError(scenario)
        state = self.save_state(pyboy)
        pyboy.stop(save=False)
        return state

    def visible_oam(self, pyboy) -> list[dict[str, object]]:
        result = []
        for index in range(40):
            sprite = pyboy.get_sprite(index)
            if sprite.on_screen:
                result.append(
                    {
                        "index": index,
                        "x": sprite.x,
                        "y": sprite.y,
                        "tile": sprite.tile_identifier,
                        "x_flip": sprite.attr_x_flip,
                        "y_flip": sprite.attr_y_flip,
                    }
                )
        return result

    def frame_state(self, pyboy, capture_index: int, input_name: str | None) -> dict[str, object]:
        memory = pyboy.memory
        pixels = pyboy.screen.ndarray.tobytes()
        player_base = self.sym["wSpritePlayerStateData1"]
        return {
            "capture_index": capture_index,
            "emulator_frame": capture_index,
            "png": f"frame-{capture_index:06}.png",
            "input": input_name,
            "sha1": hashlib.sha1(pixels).hexdigest(),
            "map": self.position(pyboy)[0],
            "map_id": memory[self.sym["wCurMap"]],
            "player_x": memory[self.sym["wXCoord"]],
            "player_y": memory[self.sym["wYCoord"]],
            "player_facing": memory[self.sym["wPlayerDirection"]],
            "player_transport": memory[self.sym["wWalkBikeSurfState"]],
            "walk_counter": memory[self.sym["wWalkCounter"]],
            "jump_index": memory[self.sym["wPlayerJumpingYScreenCoordsIndex"]],
            "player_screen_y": memory[player_base + 4],
            "player_screen_x": memory[player_base + 6],
            "tile_in_front": memory[self.sym["wTileInFrontOfPlayer"]],
            "status5": memory[self.sym["wStatusFlags5"]],
            "status6": memory[self.sym["wStatusFlags6"]],
            "is_in_battle": memory[self.sym["wIsInBattle"]],
            "opponent": memory[self.sym["wCurOpponent"]],
            "battle_type": memory[self.sym["wBattleType"]],
            "oam": self.visible_oam(pyboy),
        }

    def capture(self, scenario: str, pretrigger: bytes, out: Path, trace: list[str | None]) -> None:
        frames_dir = out / "frames"
        frames_dir.mkdir(parents=True, exist_ok=True)
        pyboy = self.new()
        self.load_state(pyboy, pretrigger)
        rows = []

        pyboy.screen.image.save(frames_dir / "frame-000000.png")
        rows.append(self.frame_state(pyboy, 0, None))
        held = None
        for capture_index, input_name in enumerate(trace, 1):
            if held != input_name:
                if held is not None:
                    pyboy.button_release(held)
                if input_name is not None:
                    pyboy.button_press(input_name)
                held = input_name
            pyboy.tick()
            pyboy.screen.image.save(frames_dir / f"frame-{capture_index:06}.png")
            rows.append(self.frame_state(pyboy, capture_index, input_name))
        if held is not None:
            pyboy.button_release(held)
        pyboy.stop(save=False)
        write_json(
            out / "manifest.json",
            {
                "scenario": scenario,
                "implementation": "reference",
                "frame_mapping": "capture_index == emulator_frame; frame 0 is pre-trigger",
                "trigger": {"capture_index": 1, "timeline_offset": 0, "input": trace[0]},
                "timeline": trace,
                "frames": rows,
            },
        )


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def wait_state(client: DebugClient, predicate, label: str, timeout: float = 15.0):
    deadline = time.time() + timeout
    last = None
    while time.time() < deadline:
        last = client.state()
        if predicate(last):
            return last
        time.sleep(0.04)
    raise RuntimeError(f"current setup timeout at {label}: {last}")


def prepare_current(client: DebugClient, scenario: str) -> dict[str, object]:
    expected = {
        "fly-departure-arrival": ("PalletTown", 5, 6),
        "cut-tree": ("VermilionCity", 15, 17),
        "surf-entry": ("PalletTown", 5, 13),
        "ledge-down": ("Route1", 10, 4),
        "battle-entry": ("PalletTown", 5, 6),
    }[scenario]
    state = wait_state(
        client,
        lambda s: s["screen"] == "overworld"
        and (s["map_name"], s["player_x"], s["player_y"]) == expected,
        "initial position",
    )
    return state


def current_setup_timeline(scenario: str) -> list[str | None]:
    """Deterministically reach the field-move pre-trigger screen.

    The supplied trace begins at CURRENT_SETUP_FRAME and ends immediately
    before CURRENT_TRIGGER_FRAME. Keeping setup and verdict input in one
    queued timeline also fixes otherwise-hidden overworld animation phases.
    """
    length = CURRENT_TRIGGER_FRAME - CURRENT_SETUP_FRAME
    presses: dict[int, str] = {}
    if scenario in {"fly-departure-arrival", "cut-tree", "surf-entry"}:
        presses.update({0: "start", 30: "down", 60: "a", 100: "a"})
    if scenario == "fly-departure-arrival":
        presses.update({140: "a", 180: "up"})
    elif scenario == "cut-tree":
        presses[140] = "down"
    elif scenario == "surf-entry":
        presses.update({140: "down", 170: "down"})
    elif scenario not in {"ledge-down", "battle-entry"}:
        raise ValueError(scenario)
    return timeline(length, presses)


def capture_current(binary: Path, snapshot: Path, scenario: str, out: Path) -> None:
    port = free_port()
    frames = out / "frames"
    frames.mkdir(parents=True, exist_ok=True)
    log = (out / "game.log").open("w")
    process = subprocess.Popen(
        [
            str(binary),
            "run",
            "--headless",
            "--no-audio",
            "--debug-port",
            str(port),
            "--skip-intro",
            "--snapshot",
            str(snapshot),
            "--record-frames",
            str(frames),
        ],
        cwd=ROOT,
        stdout=log,
        stderr=subprocess.STDOUT,
    )
    client = None
    try:
        client = DebugClient(port)
        pretrigger = prepare_current(client, scenario)
        if scenario == "battle-entry":
            trigger_response = client.cmd(
                cmd="start_wild_battle",
                species="Rhydon",
                level=20,
                start_at_frame=CURRENT_TRIGGER_FRAME,
            )
            if not trigger_response.get("ok"):
                raise RuntimeError(trigger_response)
            start_frame = trigger_response["data"]["frame_count"]
            trace = [None] * REFERENCE_LENGTHS[scenario]
        else:
            trace = TIMELINES[scenario]
            setup_trace = current_setup_timeline(scenario)
            trigger_response = client.press_timeline(
                setup_trace + trace, start_at_frame=CURRENT_SETUP_FRAME
            )
            if not trigger_response.get("ok"):
                raise RuntimeError(trigger_response)
            start_frame = CURRENT_TRIGGER_FRAME
        if scenario == "battle-entry":
            timeline_response = client.press_timeline(trace)
            end_frame = timeline_response["data"]["end_frame"]
        else:
            timeline_response = trigger_response
            end_frame = trigger_response["data"]["end_frame"]
        print(
            f"current {scenario} verdict {start_frame}.."
            f"{start_frame + len(trace) - 1} "
            f"({len(trace)} frames)",
            flush=True,
        )
        wait_state(
            client,
            lambda state: state["frame_count"] >= end_frame,
            "timeline completion",
            # PNG recording is synchronous and can run below real-time on CI.
            # This timeout guards a stalled timeline, not encoder throughput.
            timeout=max(60.0, len(trace) / 10),
        )
        final_state = client.state()
        final_map = client.cmd(cmd="get_map")
        write_json(
            out / "run.json",
            {
                "scenario": scenario,
                "implementation": "current",
                "frame_mapping": "frame-NNNNNN.png has frame_count N+1; verified by frame-manifest.jsonl",
                "pretrigger": pretrigger,
                "trigger_frame_count": start_frame,
                "trigger_capture_index": start_frame - 1,
                "trigger_response": trigger_response,
                "timeline_response": timeline_response,
                "setup_timeline": (
                    current_setup_timeline(scenario)
                    if scenario != "battle-entry"
                    else None
                ),
                "timeline": trace,
                "final_state": final_state,
                "final_map": final_map,
            },
        )
    finally:
        if client is not None:
            client.close()
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
        log.close()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--side", choices=("both", "reference", "current"), default="both")
    parser.add_argument("--repeat", type=int, default=2)
    parser.add_argument(
        "--scenario",
        action="append",
        choices=tuple(REFERENCE_LENGTHS),
        help="capture only this scenario; repeat the flag to select several",
    )
    parser.add_argument("--reference-rom", type=Path)
    parser.add_argument("--reference-world-state", type=Path)
    parser.add_argument("--reference-symbols", type=Path)
    parser.add_argument("--current-binary", type=Path, default=ROOT / "target/debug/pokered-app")
    args = parser.parse_args()
    if args.repeat < 1:
        parser.error("--repeat must be at least 1")

    args.output.mkdir(parents=True, exist_ok=True)
    scenarios = args.scenario or list(REFERENCE_LENGTHS)
    if args.side in ("both", "reference"):
        required = {
            "--reference-rom": args.reference_rom,
            "--reference-world-state": args.reference_world_state,
            "--reference-symbols": args.reference_symbols,
        }
        missing = [name for name, path in required.items() if path is None]
        if missing:
            parser.error(f"reference capture requires: {', '.join(missing)}")
        actual_rom_sha1 = hashlib.sha1(args.reference_rom.read_bytes()).hexdigest()
        if actual_rom_sha1 != REFERENCE_ROM_SHA1:
            parser.error(
                "reference ROM SHA-1 mismatch: "
                f"expected {REFERENCE_ROM_SHA1}, got {actual_rom_sha1}"
            )
        reference = ReferenceHarness(
            args.reference_rom, args.reference_world_state, args.reference_symbols
        )
        prepared = {scenario: reference.prepare(scenario) for scenario in scenarios}
        for scenario in scenarios:
            for run in range(1, args.repeat + 1):
                out = args.output / "reference" / scenario / f"run-{run}"
                print(f"reference {scenario} run {run}", flush=True)
                reference.capture(scenario, prepared[scenario], out, TIMELINES[scenario])

    if args.side in ("both", "current"):
        for scenario in scenarios:
            snapshot = CURRENT_SNAPSHOTS[scenario]
            if not snapshot.exists():
                raise FileNotFoundError(snapshot)
            for run in range(1, args.repeat + 1):
                out = args.output / "current" / scenario / f"run-{run}"
                print(f"current {scenario} run {run}", flush=True)
                capture_current(args.current_binary, snapshot, scenario, out)


if __name__ == "__main__":
    main()
