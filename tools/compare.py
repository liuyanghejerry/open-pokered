#!/usr/bin/env python3
"""
compare.py — Compare pokered output against PyBoy (reference GB emulator).

Runs both sides for a given scenario, then:
  1. Pixel-diffs screenshots (grayscale, tolerance 50/255).
  2. Compares game state (Rust JSON dump vs PyBoy WRAM reads).

Usage:
    python tools/compare.py --rom path/to/pokemon_red.gb --scenario overworld --frames 300
    python tools/compare.py --rom pokemon_red.gb --scenario all

Requirements:
    pip install pyboy pillow numpy

WRAM addresses (from pret/pokered ram/wram.asm):
    wCurMap     0xD35E   current map ID
    wYCoord     0xD361   player Y tile
    wXCoord     0xD362   player X tile
    wIsInBattle 0xD057   0 = not in battle, non-zero = in battle
"""

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    import numpy as np
    from PIL import Image, ImageChops
except ImportError:
    print("ERROR: Missing dependencies. Run: pip install pyboy pillow numpy", file=sys.stderr)
    sys.exit(1)

try:
    from pyboy import PyBoy
except ImportError:
    print("ERROR: PyBoy not installed. Run: pip install pyboy", file=sys.stderr)
    sys.exit(1)


WRAM_CUR_MAP      = 0xD35E
WRAM_Y_COORD      = 0xD361
WRAM_X_COORD      = 0xD362
WRAM_IS_IN_BATTLE = 0xD057
WRAM_MAX_MENU_ITEM = 0xCC28

WRAM_GAME_PROGRESS   = 0xD5A3
WRAM_PLAYER_NAME     = 0xD158
WRAM_SPRITE_FACING   = 0xC109
WRAM_CURRENT_MENU    = 0xCC26
WRAM_NAMING_SUBMIT   = 0xCC26
WRAM_PLAYER_DIRECTION = 0xD119

SCENARIO_SCREEN_MAP = {
    "copyright":   "copyright",
    "title":       "title",
    "main_menu":   "main-menu",
    "oak":         "oak",
    "overworld":   "overworld",
    "battle":      "battle",
    "start_menu":  "start-menu",
    "options":     "options",
    "save":        "save",
}

ALL_SCENARIOS = list(SCENARIO_SCREEN_MAP.keys())


def _find_binary() -> Path | None:
    base = Path(__file__).parent.parent
    for profile in ("debug", "release"):
        for name in ("pokered-app", "pokered"):
            p = base / "target" / profile / name
            if p.exists():
                return p
    return None


def run_rust_screenshot(screen: str, frames: int, output_path: Path) -> bool:
    rust_binary = _find_binary()
    if rust_binary is None:
        print(f"  ERROR: pokered-app binary not found. Run 'cargo build -p pokered-app' first.")
        return False

    cmd = [
        str(rust_binary),
        "screenshot",
        "--screen", screen,
        "--output", str(output_path),
        "--frames", str(frames),
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"  ERROR running Rust screenshot: {result.stderr.strip()}")
        return False
    return True


def run_rust_dump_state(screen: str, frames: int) -> dict | None:
    rust_binary = _find_binary()
    if rust_binary is None:
        print(f"  ERROR: pokered-app binary not found. Run 'cargo build -p pokered-app' first.")
        return None

    cmd = [
        str(rust_binary),
        "dump-state",
        "--screen", screen,
        "--frames", str(frames),
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"  ERROR running Rust dump-state: {result.stderr.strip()}")
        return None

    for line in result.stdout.splitlines():
        line = line.strip()
        if line.startswith("{"):
            try:
                return json.loads(line)
            except json.JSONDecodeError:
                pass

    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError:
        print(f"  ERROR parsing Rust JSON output: {result.stdout[:200]}")
        return None


def run_pyboy_scenario(rom_path: Path, scenario: str, frames: int, screenshot_path: Path) -> dict | None:
    """
    Boot PyBoy, skip to the logical state matching the given scenario, advance
    `frames` more ticks, then capture screen + WRAM.

    For scenarios like 'overworld', we fast-forward through the real boot
    sequence (title → intro → name entry → overworld) using button inputs
    rather than WRAM patches, so the state reflects what actually happens in
    the real ROM at that point.
    """
    pyboy = PyBoy(str(rom_path), window="null")
    try:
        if scenario == "overworld":
            _pyboy_reach_overworld(pyboy)
        elif scenario == "copyright":
            pass
        elif scenario == "title":
            pyboy.tick(1500, render=False)
        elif scenario == "main_menu":
            _pyboy_skip_to_title(pyboy)
            pyboy.button("start")
            pyboy.tick(30, render=False)
        else:
            pass

        pyboy.tick(frames, render=True)

        screen_arr = pyboy.screen.ndarray
        img = Image.fromarray(screen_arr, mode="RGBA")
        img.save(str(screenshot_path))

        state = {
            "map_id":      pyboy.memory[WRAM_CUR_MAP],
            "player_y":    pyboy.memory[WRAM_Y_COORD],
            "player_x":    pyboy.memory[WRAM_X_COORD],
            "in_battle":   pyboy.memory[WRAM_IS_IN_BATTLE] != 0,
            "frame_count": frames,
        }
        return state
    finally:
        pyboy.stop()


def _pyboy_skip_to_title(pyboy: "PyBoy") -> None:
    pyboy.tick(180, render=False)


def _pyboy_is_at_title_screen(pyboy: "PyBoy") -> bool:
    try:
        tile_at_2_1 = pyboy.memory[0x9800 + 2 + 1 * 32]
        return tile_at_2_1 >= 0x80
    except Exception:
        return False


def _dbg_shot(pyboy: "PyBoy", tag: str) -> None:
    from PIL import Image as _Image
    arr = pyboy.screen.ndarray
    _Image.fromarray(arr, mode="RGBA").save(f"/tmp/pyboy_{tag}.png")


def _pyboy_reach_overworld(pyboy: "PyBoy") -> None:
    def press(btn: str, hold: int = 3, wait: int = 10) -> None:
        pyboy.button_press(btn)
        pyboy.tick(hold, render=False)
        pyboy.button_release(btn)
        pyboy.tick(wait, render=False)

    pyboy.tick(1800, render=False)
    _dbg_shot(pyboy, "01_title")

    press("start", wait=60)
    _dbg_shot(pyboy, "02_main_menu")

    press("a", wait=120)
    _dbg_shot(pyboy, "03_new_game")

    for _ in range(300):
        if pyboy.memory[WRAM_MAX_MENU_ITEM] == 3:
            break
        press("a", hold=3, wait=8)
        pyboy.tick(30, render=False)
    _dbg_shot(pyboy, "04_after_oak_speech")

    press("down", hold=3, wait=15)
    _dbg_shot(pyboy, "05_player_cursor_down")
    press("a", hold=3, wait=250)
    _dbg_shot(pyboy, "06_player_red_chosen")

    press("a", hold=3, wait=60)
    _dbg_shot(pyboy, "07_your_name_is_red")

    for _ in range(300):
        if pyboy.memory[WRAM_MAX_MENU_ITEM] == 3:
            break
        press("a", hold=3, wait=8)
        pyboy.tick(30, render=False)
    _dbg_shot(pyboy, "08_rival_intro_done")

    press("down", hold=3, wait=15)
    _dbg_shot(pyboy, "09_rival_cursor_down")
    press("a", hold=3, wait=250)
    _dbg_shot(pyboy, "10_rival_blue_chosen")

    press("a", hold=3, wait=60)
    _dbg_shot(pyboy, "11_his_name_is_blue")

    for _ in range(600):
        pyboy.tick(30, render=False)
        press("a", hold=3, wait=5)
        if pyboy.memory[WRAM_PLAYER_DIRECTION] != 0:
            break

    pyboy.tick(180, render=False)


def pixel_diff(img_path_a: Path, img_path_b: Path, tolerance: int = 50) -> dict:
    img_a = Image.open(img_path_a).convert("L")
    img_b = Image.open(img_path_b).convert("L")

    if img_a.size != img_b.size:
        return {
            "match": False,
            "error": f"Size mismatch: {img_a.size} vs {img_b.size}",
            "differing_pixels": -1,
            "total_pixels": -1,
            "diff_pct": 100.0,
        }

    arr_a = np.array(img_a, dtype=np.int16)
    arr_b = np.array(img_b, dtype=np.int16)
    diff = np.abs(arr_a - arr_b)

    total = diff.size
    differing = int(np.sum(diff > tolerance))
    diff_pct = differing / total * 100

    return {
        "match": differing == 0,
        "differing_pixels": differing,
        "total_pixels": total,
        "diff_pct": round(diff_pct, 2),
    }


def compare_state(rust: dict, pyboy: dict) -> list[dict]:
    mismatches = []

    rust_map = rust.get("map_id")
    pyboy_map = pyboy.get("map_id")
    if rust_map != pyboy_map:
        mismatches.append({
            "field": "map_id",
            "wram_addr": hex(WRAM_CUR_MAP),
            "rust": rust_map,
            "pyboy": pyboy_map,
        })

    rust_x = rust.get("player_x")
    pyboy_x = pyboy.get("player_x")
    if rust_x != pyboy_x:
        mismatches.append({
            "field": "player_x",
            "wram_addr": hex(WRAM_X_COORD),
            "rust": rust_x,
            "pyboy": pyboy_x,
        })

    rust_y = rust.get("player_y")
    pyboy_y = pyboy.get("player_y")
    if rust_y != pyboy_y:
        mismatches.append({
            "field": "player_y",
            "wram_addr": hex(WRAM_Y_COORD),
            "rust": rust_y,
            "pyboy": pyboy_y,
        })

    rust_battle = rust.get("in_battle", False)
    pyboy_battle = pyboy.get("in_battle", False)
    if rust_battle != pyboy_battle:
        mismatches.append({
            "field": "in_battle",
            "wram_addr": hex(WRAM_IS_IN_BATTLE),
            "rust": rust_battle,
            "pyboy": pyboy_battle,
        })

    return mismatches


def run_comparison(rom_path: Path, scenario: str, frames: int, output_dir: Path) -> dict:
    screen = SCENARIO_SCREEN_MAP.get(scenario, scenario)
    print(f"\n{'='*60}")
    print(f"Scenario: {scenario}  |  screen={screen}  |  frames={frames}")
    print(f"{'='*60}")

    rust_png  = output_dir / f"{scenario}_rust.png"
    pyboy_png = output_dir / f"{scenario}_pyboy.png"

    # Frame synchronization for title screen:
    # PyBoy skips 1500 frames (boot + intro) to reach stable title screen
    # Rust starts directly at title screen, needs ~104 frames for initial animations
    # (logo bounce: 32, pause: 36, version scroll: 36)
    # Then match the remaining stable frames
    rust_frames = frames
    if scenario == "title":
        # PyBoy captures at frame 300 after 1500 skip = total 1800 frames from boot
        # Rust needs: 104 (initial animations) + 300 (stable) = 404 frames
        rust_frames = 404

    print("  [Rust] capturing screenshot...")
    rust_ok = run_rust_screenshot(screen, rust_frames, rust_png)

    print("  [Rust] dumping state...")
    rust_state = run_rust_dump_state(screen, rust_frames)

    print("  [PyBoy] running scenario...")
    pyboy_state = run_pyboy_scenario(rom_path, scenario, frames, pyboy_png)

    result = {
        "scenario": scenario,
        "frames": frames,
        "rust_screenshot": str(rust_png) if rust_ok else None,
        "pyboy_screenshot": str(pyboy_png) if pyboy_state else None,
        "pixel_diff": None,
        "state_mismatches": [],
        "rust_state": rust_state,
        "pyboy_state": pyboy_state,
    }

    if rust_ok and pyboy_state:
        print("  Diffing screenshots...")
        result["pixel_diff"] = pixel_diff(rust_png, pyboy_png)

    if rust_state and pyboy_state:
        print("  Comparing state...")
        result["state_mismatches"] = compare_state(rust_state, pyboy_state)

    return result


def print_report(results: list[dict]) -> None:
    print("\n" + "=" * 60)
    print("COMPARISON REPORT")
    print("=" * 60)

    for r in results:
        scenario = r["scenario"]
        pixel = r.get("pixel_diff")
        mismatches = r.get("state_mismatches", [])

        pixel_ok = pixel is not None and pixel["match"]
        state_ok = len(mismatches) == 0

        status = "PASS" if (pixel_ok and state_ok) else "FAIL"
        print(f"\n[{status}] {scenario}")

        if pixel:
            if pixel.get("error"):
                print(f"  Pixel diff: ERROR — {pixel['error']}")
            else:
                print(f"  Pixel diff: {pixel['differing_pixels']}/{pixel['total_pixels']} px differ "
                      f"({pixel['diff_pct']}%) — {'MATCH' if pixel['match'] else 'MISMATCH'}")
        else:
            print("  Pixel diff: skipped (screenshot failed)")

        if mismatches:
            print("  State mismatches:")
            for m in mismatches:
                print(f"    {m['field']} [{m['wram_addr']}]: rust={m['rust']}  pyboy={m['pyboy']}")
        else:
            if r.get("rust_state") and r.get("pyboy_state"):
                print("  State: all fields match")
            else:
                print("  State: skipped (state dump failed)")

    pass_count = sum(
        1 for r in results
        if r.get("pixel_diff") and r["pixel_diff"]["match"]
        and len(r.get("state_mismatches", [])) == 0
    )
    print(f"\nSummary: {pass_count}/{len(results)} scenarios passed")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Compare pokered output vs PyBoy reference emulator"
    )
    parser.add_argument("--rom", required=True, help="Path to Pokemon Red .gb ROM file")
    parser.add_argument(
        "--scenario",
        default="overworld",
        choices=ALL_SCENARIOS + ["all"],
        help="Which scenario to compare (default: overworld)",
    )
    parser.add_argument(
        "--frames",
        type=int,
        default=300,
        help="Frames to advance before comparing (default: 300)",
    )
    parser.add_argument(
        "--output-dir",
        default="compare_output",
        help="Directory to save screenshots and report (default: compare_output)",
    )
    args = parser.parse_args()

    rom_path = Path(args.rom)
    if not rom_path.exists():
        print(f"ERROR: ROM file not found: {rom_path}", file=sys.stderr)
        sys.exit(1)

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    scenarios = ALL_SCENARIOS if args.scenario == "all" else [args.scenario]

    results = []
    for scenario in scenarios:
        result = run_comparison(rom_path, scenario, args.frames, output_dir)
        results.append(result)

    report_path = output_dir / "report.json"
    with open(report_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nFull report saved to: {report_path}")

    print_report(results)


if __name__ == "__main__":
    main()
