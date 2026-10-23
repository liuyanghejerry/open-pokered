"""Normal CONTINUE, gate entry and exit; observe CPU hooks without rendering fonts."""
import argparse
import hashlib
import io
import json
import os
import re
from pathlib import Path

os.environ["SDL_AUDIODRIVER"] = "dummy"
from pyboy import PyBoy

parser = argparse.ArgumentParser()
for name in ["rom", "symbols", "save", "out"]:
    parser.add_argument("--" + name, type=Path, required=True)
args = parser.parse_args()
sha = lambda b: hashlib.sha256(b).hexdigest()
assert sha(args.rom.read_bytes()) == "5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b"
symbols = {}
for line in args.symbols.read_text().splitlines():
    match = re.match(r"([0-9a-f]+):([0-9a-f]+) (\S+)", line)
    if match:
        symbols[match[3]] = (int(match[1], 16), int(match[2], 16))

results = []
for lane in [4, 5]:
    p = PyBoy(str(args.rom), ram_file=io.BytesIO(args.save.read_bytes()),
              window="null", sound_emulated=True, sound_volume=0)
    p.set_emulation_speed(0)
    held = None
    inputs, events = [], []
    field = [False]

    def read(name):
        return int(p.memory[symbols[name][1]])

    def state():
        return {"frame": p.frame_count, "map": read("wCurMap"),
                "x": read("wXCoord"), "y": read("wYCoord"),
                "walk": read("wWalkCounter"),
                "facing": read("wSpritePlayerStateData1FacingDirection")}

    def in_field(_):
        field[0] = True

    def door_hook(_):
        events.append(state())

    p.hook_register(*symbols["OverworldLoop"], in_field, None)
    p.hook_register(*symbols["PlayerStepOutFromDoor"], door_hook, None)

    def tick(key=None):
        global held
        if held != key:
            if held:
                p.button_release(held)
            if key:
                p.button_press(key)
            held = key
        p.tick(1, False)
        inputs.append(key)

    def wait(frames):
        for _ in range(frames):
            tick()

    def travel(key, destination):
        for _ in range(500):
            if read("wCurMap") == destination:
                wait(100)
                return
            tick(key)
        raise AssertionError(("map transition", state()))

    def step(key):
        start = (read("wXCoord"), read("wYCoord"))
        for _ in range(100):
            tick(key)
            if (read("wXCoord"), read("wYCoord")) != start:
                wait(40)
                return
        raise AssertionError(("step", key, state()))

    try:
        for frame in range(2400):
            if field[0] and (read("wCurMap"), read("wXCoord"), read("wYCoord")) == (33, 8, 6):
                break
            tick("start" if frame % 60 < 2 else "a" if frame % 60 in [20, 21] else None)
        assert field[0] and read("wCurMap") == 33, state()
        wait(64)
        travel("up", 193)
        assert (read("wXCoord"), read("wYCoord")) == (4, 7), state()
        step("up")
        if lane == 5:
            step("right")
        start = state()
        assert (start["x"], start["y"]) == (lane, 6), start
        travel("down", 33)
        end = state()
        assert (end["x"], end["y"], end["walk"]) == (8, 6, 0), end
        assert any(e["map"] == 33 and (e["x"], e["y"]) == (8, 5) for e in events), events
        results.append({"lane": lane, "start": start, "end": end,
                        "door_calls": events, "inputs": inputs})
    finally:
        p.stop(save=False)

args.out.parent.mkdir(parents=True, exist_ok=True)
args.out.write_text(json.dumps({"rom_sha256": sha(args.rom.read_bytes()),
    "save_sha256": sha(args.save.read_bytes()), "symbols_sha256": sha(args.symbols.read_bytes()),
    "prepared_save": True, "runtime_memory_writes": False, "render_original_font": False,
    "cases": results}, indent=2) + "\n")
print(json.dumps([{k: row[k] for k in ["lane", "start", "end", "door_calls"]} for row in results]))
