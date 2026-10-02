#!/usr/bin/env python3
"""Drive a real Route 11 NPC trade and sample identical text-phase frames.

The base has shorter holds and omits two window slides. Absolute movie frames
therefore differ; each paired picture samples the named text phase at frame 40.
"""
import argparse
import hashlib
import json
import shutil
import socket
import subprocess
import tempfile
from pathlib import Path
from debug_drive import DebugClient
from save_builder import make_mon

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--label", choices=["before", "after"], required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--template", type=Path, default=ROOT / "docs/screenshots/2026-10-02-original-font/stats-zh-before-input.json")
    args = parser.parse_args()
    binary, output = args.binary.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    data = json.loads(args.template.read_text())
    data["party"] = [make_mon("Nidorino", 20), make_mon("Pikachu", 20)]
    fixed = args.label == "after"
    frames = [(379, "went"), (716 if fixed else 459, "for"),
        (876 if fixed else 539, "farewell")]
    duration = 1570 if fixed else 1016
    events = []
    with tempfile.TemporaryDirectory(prefix="pokered-trade-text-") as temp:
        directory = Path(temp)
        local = directory / "pokered-app"
        shutil.copy2(binary, local)
        snapshot = directory / "input.json"
        snapshot.write_text(json.dumps(data))
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        with (directory / "game.log").open("w") as log:
            proc = subprocess.Popen([str(local), "run", "--snapshot", str(snapshot),
                "--save", str(directory / "save.sav"), "--skip-intro", "--warp", "Route11Gate2F,3,3",
                "--headless", "--debug-port", str(port), "--speed", "0", "--no-audio",
                "--seed", "49", "--lang", "en"], cwd=ROOT, stdout=log, stderr=log)
            client = None
            try:
                client = DebugClient(port)
                def cmd(verb, **arguments):
                    result = client.cmd(cmd=verb, **arguments)
                    events.append({"command": {"cmd": verb, **arguments}, "result": result})
                    if not result.get("ok", True) and verb != "save_state":
                        raise RuntimeError(result)
                    return result
                cmd("step_frames", count=100)
                # The Youngster can walk out of the faced tile during a turn.
                for _ in range(10):
                    result = cmd("interact_with", id="npc:0")
                    if result["data"]["result"] == "dialogue":
                        break
                else:
                    raise RuntimeError("could not talk to the Route 11 Youngster")
                cmd("skip_dialogue")
                cmd("step_frames", count=1)
                state = cmd("get_state")["data"]
                assert state["choice"] and state["choice"]["selected"] == 0
                cmd("press_timeline", buttons=["a", None, None], advance=True)
                if fixed:
                    cmd("step_frames", count=10)
                    # Select the first actual party entry before ConnectCableText.
                    cmd("press_timeline", buttons=["a", None, None], advance=True)
                cmd("skip_dialogue")
                state = cmd("get_state")["data"]
                if fixed:
                    assert state["dialogue"] is None and state["script_running"]
                    origin = state["frame_count"]
                    cmd("step_frames", count=1)  # creates and ticks the movie once
                    movie_frame = 1
                else:
                    for _ in range(10):
                        if state["active_script_effect"] is None:
                            break
                        cmd("step_frames", count=1)
                        state = cmd("get_state")["data"]
                    origin, movie_frame = state["frame_count"], 0
                # The public snapshot API independently confirms movie takeover.
                result = cmd("save_state", slot=1)
                assert not result["ok"] and "trade" in str(result), result
                for target, name in frames:
                    cmd("step_frames", count=target - movie_frame)
                    movie_frame = target
                    cmd("capture_frame", path=str(output / f"trade-{name}-40-{args.label}.png"))
                cmd("step_frames", count=duration + 5 - movie_frame)
                state = cmd("get_state")["data"]
                assert sorted(mon["species"] for mon in state["party"]) == ["Nidorina", "Pikachu"], state
                flags = cmd("get_flags")["data"]
                assert flags.get("EVENT_TRADED_FOR_TERRY"), flags
                (output / f"trade-text-{args.label}.json").write_text(json.dumps({
                    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                    "movie_origin_frame": origin, "phase_frame": 40, "movie_frames": dict((name, frame) for frame, name in frames),
                    "events": events}, indent=2, ensure_ascii=False))
                shutil.copy2(snapshot, output / f"trade-text-{args.label}-input.json")
            finally:
                if client is not None:
                    client.close()
                proc.terminate()
                proc.wait(timeout=5)


if __name__ == "__main__":
    main()
