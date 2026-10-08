#!/usr/bin/env python3
"""Real-input escape-move regression and same-frame before/after captures.

Usage: fidelity_escape_moves.py APP_TEST_DRIVER ARTIFACT_DIR [--before]
The initial fixture is constructed from an actual fresh-game save; each battle
and all menu/text input use the shared production runtime via fidelity_stdio.
"""
import argparse
import json
from pathlib import Path

import debug_drive
import fidelity_stdio
import playthrough as pt
import save_builder as sb


def enter_player_menu(game):
    for _ in range(200):
        if game.st()["battle_phase"] == "PlayerMenu":
            return
        game.tap("a", 10)
    raise AssertionError(game.st())


def case(output, seed, level, foe, foe_level, trainer=False, capture=None):
    fixture = sb.SaveBuilder().party_add("Abra", level, moves=["Teleport"])
    map_name, x, y = ("PewterGym", 4, 2) if trainer else ("PalletTown", 5, 6)
    assert pt.walkable(map_name, x, y)
    fixture.position(map_name, x, y)
    game = pt.Game(snapshot=fixture.write(output / "fixture.json"), seed=seed)
    try:
        pt.resume_reentry(game)
        if trainer:
            game.face("up")
            game.tap("a", 1)
            for _ in range(100):
                if game.st()["screen"] == "battle":
                    break
                if game.st()["dialogue"]:
                    game.skip()
                else:
                    game.step(2)
        else:
            game.d.cmd(cmd="start_wild_battle", species=foe, level=foe_level)
        game.wait("screen=battle", 600)
        enter_player_menu(game)
        initial_hp = game.st()["battle_live"]["player"]["hp"]
        game.d.drive(["up", "left"], frames=10)
        game.tap("a", 4)
        assert game._await_phase("MoveSelect", 120)
        game.tap("a", 4)
        if capture:
            for _ in range(250):
                if "ABRA used TELEPORT!" in (game.st()["battle_message"] or ""):
                    break
                game.tap("a", 10)
            else:
                raise AssertionError(game.st())
            game.step(400)
            game.tap("a", 4)
            assert game.st()["frame_count"] < 4000
            game.step(4000 - game.st()["frame_count"])
            game.d.cmd(cmd="capture_frame", path=str(output / f"{capture}.png"))
            return game.st()

        states = []
        for _ in range(400):
            state = game.st()
            states.append(state)
            if state["screen"] != "battle" or state["battle_phase"] == "PlayerMenu":
                break
            game.tap("a", 10)
        else:
            raise AssertionError(game.st())
        final_hp = next(s["battle_live"]["player"]["hp"]
                        for s in reversed(states) if s["battle_live"])
        messages = list(dict.fromkeys(s["battle_message"] for s in states
                                      if s["battle_message"]))
        return {"seed": seed, "level": level, "foe": foe, "foe_level": foe_level,
                "escaped": game.st()["screen"] != "battle", "hp_before": initial_hp,
                "hp_after": final_hp, "messages": messages}
    finally:
        game.close()


def run(args):
    fidelity_stdio.install(pt, debug_drive, args.driver.resolve())
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    game = pt.Game()
    try:
        pt.m01_boot(game)
        pt.m02_oak_speech(game)
        game.d.cmd(cmd="save")
        reloaded = pt.Game(save_path=game.save_path)
        try:
            sb.SaveBuilder._tpl = reloaded.d.cmd(cmd="export_fixture")["data"]
        finally:
            reloaded.close()
    finally:
        game.close()

    records = [case(output, seed, 5, "Metapod", 50) for seed in range(64)]
    fast = [case(output, seed, 50, "Caterpie", 5) for seed in range(8)]
    trainer = case(output, 0, 50, "Brock", 12, trainer=True)
    escaped = sum(r["escaped"] for r in records)
    if not args.before:
        # Exact distribution/byte consumption is pinned by ScriptedRng unit
        # tests; this checks that the production path reaches both outcomes.
        assert 0 < escaped < 64, escaped
        for record in fast:
            assert record["escaped"] and record["hp_before"] == record["hp_after"], record
            assert all(not text.startswith("Enemy ") for text in record["messages"]), record
            assert "ABRA ran from battle!" in record["messages"], record
        assert not trainer["escaped"] and "But it failed!" in trainer["messages"], trainer
    captures = {}
    for name, seed, level, foe, foe_level, is_trainer in [
        ("success", 2, 50, "Caterpie", 5, False),
        ("trainer-failure", 0, 50, "Brock", 12, True),
        ("weak-failure", 5, 5, "Metapod", 50, False),
    ]:
        captures[name] = case(output, seed, level, foe, foe_level, is_trainer, name)
    (output / "results.json").write_text(json.dumps(records + fast + [trainer], indent=2))
    (output / "states.json").write_text(json.dumps(captures, indent=2))
    print(f"Weak-user escapes: {escaped}/64; fast-user/trainer cases and three captures completed")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("driver", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--before", action="store_true", help="capture unfixed base behavior")
    run(parser.parse_args())
