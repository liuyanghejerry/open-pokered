#!/usr/bin/env python3
"""Same-frame font captures using a preserved base or final integrated binary.

CLI captures use frame 10; seeded stats uses frame 512; seeded shop frame 1024.
Every debug input timeline executes synchronously, without wall-clock pacing.
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
from save_builder import event_flag_bit, make_mon

ROOT = Path(__file__).resolve().parent.parent


def capture(binary, template, output, label, case):
    data = json.loads(template.read_text())
    data["party"] = [make_mon("Venusaur", 100, dv_bytes=(255, 255))]
    if case == "stats-zh":
        data["party"][0].update(ot_id=65535, ot_name=list(range(128, 135)) + [80] * 4)
        flags, language, warp, frame = ["EVENT_GOT_POKEDEX"], "zh", "PalletTown,5,5", 512
    else:
        data["game_data"]["player_money"] = 999999
        flags = ["EVENT_GOT_OAKS_PARCEL", "EVENT_OAK_GOT_PARCEL"]
        language, warp, frame = "en", "ViridianMart,2,5", 1024
    for flag in flags:
        bit = event_flag_bit(flag)
        data["game_data"]["event_flags"][bit >> 3] |= 1 << (bit & 7)
    with tempfile.TemporaryDirectory(prefix="pokered-font-capture-") as temp:
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
                "--save", str(directory / "save.sav"), "--skip-intro", "--warp", warp,
                "--headless", "--debug-port", str(port), "--speed", "0", "--no-audio",
                "--seed", "49", "--lang", language], cwd=ROOT, stdout=log, stderr=log)
            client = None
            events = []
            try:
                client = DebugClient(port)
                def cmd(verb, **arguments):
                    result = client.cmd(cmd=verb, **arguments)
                    events.append({"command": {"cmd": verb, **arguments}, "result": result})
                    if not result.get("ok", True):
                        raise RuntimeError(result)
                    return result
                cmd("step_frames", count=100)
                if case == "stats-zh":
                    for button in ["start", "down", "a", "a", "a"]:
                        cmd("press_timeline", buttons=[button, None, None], advance=True)
                    state = cmd("get_state")["data"]
                    if state["screen"] != "stats":
                        raise RuntimeError(state)
                else:
                    cmd("press_timeline", buttons=["left", None, None, "a", None, None], advance=True)
                    cmd("step_frames", count=8)
                    cmd("skip_dialogue")
                    cmd("step_frames", count=20)
                    cmd("press_timeline", buttons=["a", None, None], advance=True)
                    state = cmd("get_state")["data"]
                    if "SelectItem" not in str(state["shop_phase"]):
                        raise RuntimeError(state)
                cmd("step_frames", count=frame - state["frame_count"])
                cmd("capture_frame", path=str((output / f"{case}-{label}.png").resolve()))
                (output / f"{case}-{label}.json").write_text(json.dumps({
                    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                    "frame": frame, "events": events}, indent=2, ensure_ascii=False))
                shutil.copy2(snapshot, output / f"{case}-{label}-input.json")
            finally:
                if client is not None:
                    client.close()
                proc.terminate()
                proc.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--label", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--town-map-only", action="store_true")
    parser.add_argument("--template", type=Path, default=ROOT / "docs/screenshots/2026-10-02-original-font/stats-zh-before-input.json")
    args = parser.parse_args()
    args.binary = args.binary.resolve()
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="pokered-font-cli-") as temp:
        local = Path(temp) / "pokered-app"
        shutil.copy2(args.binary, local)
        cases = [(screen, screen, "en") for screen in ["main-menu", "options", "naming"]]
        if args.town_map_only:
            cases = [("town-map-en", "town-map", "en"), ("town-map-zh", "town-map", "zh")]
        for name, screen, language in cases:
            command = [str(local), "screenshot", "--screen", screen, "--frames", "10",
                "--lang", language, "-o", str(args.output / f"{name}-{args.label}.png")]
            result = subprocess.run(command, cwd=ROOT, check=True, capture_output=True, text=True)
            (args.output / f"{name}-{args.label}.json").write_text(json.dumps({
                "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                "frame": 10, "language": "zh" if screen == "naming" else language,
                "note": "Naming explicitly seeds the Chinese pinyin IME; Town Map explicitly starts on Pallet Town with no adjacent save.",
                "command": command, "stdout": result.stdout}, indent=2))
    if args.town_map_only:
        return
    for case in ["stats-zh", "mart-buy"]:
        capture(args.binary, args.template, args.output, args.label, case)


if __name__ == "__main__":
    main()
