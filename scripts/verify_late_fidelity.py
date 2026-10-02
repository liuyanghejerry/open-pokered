#!/usr/bin/env python3
"""Seeded late-story regression probes and deterministic before/after captures.

Run with an audited --binary and the committed late-base.json template. Each
case copies that binary into its own temporary directory, isolating sidecars.
These are targeted probes, not evidence of an unmodified full playthrough.
"""
import argparse
import copy
import json
import shutil
import socket
import subprocess
import tempfile
from pathlib import Path
from debug_drive import DebugClient
from save_builder import make_mon, event_flag_bit

ROOT = Path(__file__).resolve().parent.parent

class Probe:
    def __init__(self, binary, snapshot, map_name, x, y):
        self.directory = Path(tempfile.mkdtemp(prefix="pokered-late-fidelity-"))
        local_binary = self.directory / "pokered-app"
        shutil.copy2(binary, local_binary)
        snapshot_path = self.directory / "input.json"
        snapshot_path.write_text(json.dumps(snapshot))
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        self.log = (self.directory / "game.log").open("w")
        self.proc = subprocess.Popen([str(local_binary), "run", "--headless", "--debug-port", str(port),
            "--no-audio", "--speed", "0", "--skip-intro", "--snapshot", str(snapshot_path),
            "--save", str(self.directory / "save.sav"), "--warp", f"{map_name},{x},{y}"],
            cwd=ROOT, stdout=self.log, stderr=self.log)
        self.client = DebugClient(port)
        self.events = []

    def cmd(self, verb, **kwargs):
        response = self.client.cmd(cmd=verb, **kwargs)
        self.events.append({"command": {"cmd": verb, **kwargs}, "result": response})
        if not response.get("ok", True):
            raise RuntimeError(response)
        return response

    def settle(self):
        for _ in range(50):
            state = self.cmd("get_state")["data"]
            if state.get("dialogue_state"):
                self.cmd("skip_dialogue")
            elif state.get("choice"):
                self.cmd("press", button="a")
                self.cmd("step_frames", count=8)
            elif state.get("active_script_effect") or state.get("script_running"):
                self.cmd("step_frames", count=40)
            else:
                return
        raise RuntimeError("script did not settle")

    def until_choice(self):
        for _ in range(20):
            state = self.cmd("get_state")["data"]
            if state.get("choice"):
                return
            if state.get("dialogue_state"):
                self.cmd("skip_dialogue")
            else:
                self.cmd("step_frames", count=8)
        raise RuntimeError("prize choice did not appear")

    def finish(self, output):
        for name in ["get_state", "get_flags", "get_bag", "get_party", "get_npcs"]:
            self.cmd(name)
        self.cmd("save")
        output.write_text(json.dumps({"setup": "Debug-seeded snapshot; real scene/game dispatch; isolated sidecar", "events": self.events}, indent=2))
        shutil.copy2(self.directory / "input.json", output.with_name(output.stem + "-input.json"))
        self.client.close()
        self.proc.terminate()
        self.proc.wait(timeout=5)
        self.log.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--template", type=Path, default=ROOT / "docs/audits/2026-10-02-full-fidelity/evidence/late-base.json")
    parser.add_argument("--label", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--captures-only", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    base = json.loads(args.template.read_text())
    def snapshot(full=False):
        data = copy.deepcopy(base)
        data["party"] = [make_mon("Charizard", 100, ["Slash", "Flamethrower", "Surf", "Strength"])]
        if full:
            data["party"] *= 6
            mons = [make_mon("Pidgey", 5) for _ in range(20)]
            data["current_box"] = mons
            data["pc_storage"]["boxes"][0] = mons
        return data
    if not args.captures_only:
        run = Probe(args.binary, snapshot(True), "SilphCo7F", 1, 6)
        run.cmd("step_frames", count=100)
        run.cmd("press", button="up")
        run.cmd("step_frames", count=8)
        run.cmd("press", button="a")
        run.cmd("step_frames", count=8)
        run.settle()
        run.finish(args.output / f"late-lapras-{args.label}.json")
        data = snapshot(True)
        bit = event_flag_bit("EVENT_GAVE_FOSSIL_TO_LAB")
        data["game_data"]["event_flags"][bit >> 3] |= 1 << (bit & 7)
        run = Probe(args.binary, data, "CinnabarLabFossilRoom", 5, 3)
        run.cmd("step_frames", count=100)
        run.cmd("set_flag", name="EVENT_REVIVING_KABUTO", value=True)
        run.cmd("interact_with", id="npc:0")
        run.settle()
        run.finish(args.output / f"late-fossil-{args.label}.json")
        data = snapshot(True)
        data["game_data"]["bag"]["items"] = [["CoinCase", 1]]
        data["game_data"]["player_coins"] = 9999
        run = Probe(args.binary, data, "GameCornerPrizeRoom", 2, 3)
        run.cmd("step_frames", count=100)
        run.cmd("interact_with", id="sign:0")
        run.until_choice()
        run.cmd("press", button="a")
        run.cmd("step_frames", count=80)
        run.cmd("press", button="a")
        run.cmd("step_frames", count=8)
        run.cmd("capture_frame", path=str((args.output / f"late-prize-confirm-{args.label}.png").resolve()))
        run.settle()
        run.finish(args.output / f"late-prize-{args.label}.json")
    run = Probe(args.binary, snapshot(), "LancesRoom", 24, 16)
    run.cmd("step_frames", count=600)
    run.cmd("capture_frame", path=str((args.output / f"late-lance-{args.label}.png").resolve()))
    run.finish(args.output / f"late-lance-{args.label}.json")
    data = snapshot()
    # Deterministic input reaches the zone before an incidental cemetery encounter.
    run = Probe(args.binary, data, "PokemonTower5F", 9, 8)
    run.cmd("step_frames", count=100)
    # Face the zone first, then walk one step; record all frame states so the
    # first post-entry heal frame and palette offset remain independently visible.
    run.cmd("press", button="right")
    run.cmd("step_frames", count=8)
    run.cmd("press", button="right")
    run.cmd("step_frames", count=8)
    run.cmd("step_frames", count=4)
    run.cmd("capture_frame", path=str((args.output / f"late-purified-{args.label}.png").resolve()))
    run.finish(args.output / f"late-purified-{args.label}.json")

if __name__ == "__main__":
    main()
