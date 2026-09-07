#!/usr/bin/env python3
"""Recheck the two save defects using the real playthrough checkpoints.

Build pokered-app with debug-server first, then run this file from any cwd.
Prints expected and observed results; never changes the supplied save files.
"""
import json
from pathlib import Path
import shutil
import sys
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import playthrough


def observe(save, exit_gym=False):
    g = playthrough.Game(save_path=save)
    try:
        playthrough.resume_reentry(g)
        if exit_gym:
            g.nav_warp(4, 13, "PewterGym", approach="down")
            g.step(60)
            s = g.st()
            return {"map": s["map_name"],
                    "position": [s["player_x"], s["player_y"]]}
        return {str(n["text_id"]): n["visible"]
                for n in g.d.cmd(cmd="get_npcs")["data"]
                if n["text_id"] in (2, 3, 4)}
    finally:
        g.close()


def main():
    original_bin = playthrough.BIN
    with tempfile.TemporaryDirectory(prefix="pokered-save-audit-") as tmp:
        # current_exe's directory owns the sidecar. A private binary copy
        # prevents touching the developer's normal game/sidecar files.
        playthrough.BIN = Path(tmp) / "pokered-app"
        shutil.copy2(original_bin, playthrough.BIN)
        sidecar = Path(tmp) / "pokered.script_flags.json"
        result = {"gym_exit": {
            "expected_map": "PewterCity",
            "observed": observe(HERE / "after-brock.sav", exit_gym=True),
        }}
        clean = observe(HERE / "before-starter.sav")
        shutil.copy2(HERE / "later-script-flags.json", sidecar)
        polluted = observe(HERE / "before-starter.sav")
        result["same_save_different_sidecar"] = {
            "expected_both": {"2": True, "3": True, "4": True},
            "without_sidecar": clean,
            "with_later_save_sidecar": polluted,
        }
        print(json.dumps(result, ensure_ascii=False, indent=2))
    playthrough.BIN = original_bin


if __name__ == "__main__":
    main()
