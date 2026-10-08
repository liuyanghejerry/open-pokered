#!/usr/bin/env python3
"""Real-input NPC receipt/payment regression and same-frame screenshots.

Usage: fidelity_npc_receipts.py APP_TEST_DRIVER ARTIFACT_DIR [--before]
Fixtures originate from a new-game save. Only initial state is constructed;
all interactions use production debug handlers and actual button input.
"""
import argparse
import json
from pathlib import Path

import debug_drive
import fidelity_stdio
import playthrough as pt
import save_builder as sb


CASES = [
    ("museum-money", "Museum1F", 9, 5, []),
    ("casino-coins", "GameCorner", 5, 8, ["COIN_CASE"]),
    ("daycare-cancel", "Daycare", 2, 4, []),
    ("tm36-first", "SilphCo2F", 10, 2, []),
    ("magikarp-money", "MtMoonPokecenter", 10, 7, []),
    ("daycare-money", "Daycare", 2, 4, []),
]


def until(game, predicate):
    for _ in range(500):
        if predicate(game.st()):
            return
        if game.st()["dialogue"]:
            game.skip()
        else:
            game.step(2)
    raise AssertionError(game.st())


def finish(game):
    states = []
    for _ in range(500):
        state = game.st()
        effect = state["script_effect"]
        if effect and (not states or effect != states[-1]["script_effect"]):
            states.append(state)
        if state["choice"]:
            game.choose("NO")  # Decline the optional nickname after Magikarp.
        elif state["dialogue"]:
            game.skip()
        elif state["active_script_effect"] == "ShowPokedexEntry":
            game.tap("b", 20)
        else:
            game.step(2)
        state = game.st()
        if (not state["script_running"] and state["screen"] == "overworld"
                and not state["dialogue"] and not state["choice"]):
            return states
    raise AssertionError(game.st())


def talk_salesman(game):
    # This NPC wanders. Approach its live location and verify it hasn't moved.
    for _ in range(5):
        npc = next(n for n in game.d.cmd(cmd="get_npcs")["data"]
                   if n["text_id"] == 4)
        game.approach_object(npc["x"], npc["y"], "MtMoonPokecenter")
        current = next(n for n in game.d.cmd(cmd="get_npcs")["data"]
                       if n["text_id"] == 4)
        if (npc["x"], npc["y"]) != (current["x"], current["y"]):
            continue
        game.tap("a", 16)
        if game.st()["dialogue"]:
            return
    raise AssertionError("could not talk to Magikarp salesman")


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

    records = {}
    for name, map_name, x, y, items in CASES:
        assert pt.walkable(map_name, x, y), (map_name, x, y)
        fixture = sb.SaveBuilder().party_add("Bulbasaur", 20).position(map_name, x, y)
        if name.startswith("daycare"):
            fixture.party_add("Pidgey", 10)
        for item in items:
            fixture.give_item(item, 1)
        game = pt.Game(snapshot=fixture.write(output / f"{name}.json"))
        try:
            pt.resume_reentry(game)
            game.step(10)
            if name == "daycare-money":
                game.face("up")
                game.tap("a", 1)
                until(game, lambda s: s["choice"])
                game.choose("YES")
                until(game, lambda s: s["active_script_effect"] == "ChoosePartyPokemon")
                game.step(20)
                game.tap("a", 12)
                records["daycare-deposit"] = finish(game)
                assert game.st()["party_count"] == 1

            if name == "magikarp-money":
                talk_salesman(game)
            elif name == "museum-money":
                game.tap("up", 20)
            else:
                game.face("up")
                game.tap("a", 1)

            if name == "tm36-first":
                game.skip()
                game.step(2)
                game.skip()  # Close the receipt; its fanfare is awaited.
                game.step(200)
            else:
                until(game, lambda s: s["choice"])
                if name == "daycare-cancel":
                    game.choose("YES")
                    until(game, lambda s: s["active_script_effect"] == "ChoosePartyPokemon")
                    game.step(20)
                    game.tap("b", 12)
                game.step(200)

            target = 2400 if name == "daycare-money" else 1800
            assert game.st()["frame_count"] < target, game.st()
            game.step(target - game.st()["frame_count"])
            records[name] = game.st()
            game.d.cmd(cmd="capture_frame", path=str(output / f"{name}.png"))
            if name == "tm36-first":
                if not args.before:
                    assert not game.st()["script_running"], game.st()
                    assert "Tm36" in str(game.d.cmd(cmd="get_bag")["data"])
                    game.tap("a", 1)
                    assert "SELFDESTRUCT" in str(finish(game))
            elif name == "daycare-cancel":
                if not args.before:
                    assert "All right then, come again." in str(game.st()["script_effect"])
                assert game.st()["party_count"] == 2
                finish(game)
            else:
                game.choose("YES")
                records[name + "-completion"] = finish(game)
                final = game.st()
                records[name + "-paid"] = final
                cost = {"museum-money": 50, "casino-coins": 1000,
                        "magikarp-money": 500, "daycare-money": 100}[name]
                assert final["money"] == 3000 - cost, final
                if name == "casino-coins":
                    assert final["coins"] == 50
                if name in ["magikarp-money", "daycare-money"]:
                    assert final["party_count"] == 2
        finally:
            game.close()
    (output / "states.json").write_text(json.dumps(records, indent=2))
    print("NPC receipt/payment scenarios passed")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("driver", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--before", action="store_true", help="capture the unfixed base")
    run(parser.parse_args())
