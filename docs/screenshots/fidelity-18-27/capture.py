"""Matched seeded captures: --binary PATH --label before|after --output DIR.

This exercises real dialogue, menus, and battles; only initial saves are seeded.
Menus use fixed sampling frames; rival loss uses the first blackout dialogue.
"""
import argparse
import json
import socket
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import playthrough as pt
import save_builder as sb
import debug_drive
import fidelity_stdio
parser = argparse.ArgumentParser()
parser.add_argument("--binary", type=Path, required=True)
parser.add_argument("--label", choices=["before", "after"], required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
fidelity_stdio.install(pt, debug_drive, args.binary.resolve())
def fresh_template():
    g = pt.Game()
    try:
        pt.m01_boot(g)
        pt.m02_oak_speech(g)
        assert g.d.cmd(cmd="save")["ok"]
        reloaded = pt.Game(save_path=g.save_path)
        try: return reloaded.d.cmd(cmd="export_fixture")["data"]
        finally: reloaded.close()
    finally:
        g.close()
sb.fresh_template = fresh_template

def neutral(g, frames):
    g.d.cmd(cmd="step_frames", count=frames)

def close_text(g):
    g.d.cmd(cmd="skip_dialogue")
    neutral(g, 2)

def wait(g, predicate, allow_text=False):
    for _ in range(500):
        st = g.st()
        if predicate(st):
            return st
        if allow_text and st.get("dialogue"):
            close_text(g)
        else:
            neutral(g, 2)
    raise AssertionError(g.st())

evidence = {}
def capture(g, name):
    if name == "coin-purchase":
        # The two settlement commands add two frames on the fixed branch;
        # hold both waiting textboxes to the same absolute sampling frame.
        neutral(g, 1000 - g.st()["frame_count"])
    path = args.output / f"{name}-{args.label}.png"
    response = g.d.cmd(cmd="capture_frame", path=str(path.resolve()))
    evidence[name] = response["data"]["state"]
    print(name, json.dumps({k: evidence[name].get(k) for k in ["screen", "dialogue", "coins", "money", "active_script_effect"]}))


with tempfile.TemporaryDirectory(prefix="fidelity-18-27-") as tmp:
    tmp = Path(tmp)
    sb.SaveBuilder._tpl = sb.fresh_template()
    def boot(builder, name):
        position = builder.data["game_data"]["position"]
        map_name = next(m for m, data in pt.MAPS.items() if data["id"] == position["map_id"])
        assert pt.walkable(map_name, position["x"], position["y"]), (map_name, position)
        path = builder.write(tmp / f"{name}.json")
        g = pt.Game(snapshot=path)
        pt.resume_reentry(g)
        neutral(g, 10)
        return g
    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).position("BikeShop", 6, 4)
    builder.data["game_data"]["player_direction"] = 4
    g = boot(builder, "bike-menu")
    try:
        g.tap("a", 1)
        wait(g, lambda s: s.get("choice") is not None, True)
        neutral(g, 1200 - g.st()["frame_count"])
        capture(g, "bike-menu")
    finally:
        g.close()

    # Owned badges: identical sparse mask and dialogue/menu stage.
    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).badges(5).position("CeruleanBadgeHouse", 5, 4)
    builder.data["game_data"]["player_direction"] = 8
    builder.data["game_data"]["player_direction"] = 4
    g = boot(builder, "badge-menu")
    try:
        g.tap("a", 1)
        wait(g, lambda s: s.get("choice") is not None, True)
        neutral(g, 1200 - g.st()["frame_count"])
        capture(g, "badge-menu")
    finally: g.close()

    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).money(3000).position("SafariZoneGate", 3, 3).last_map("FuchsiaCity")
    g = boot(builder, "safari-admission")
    try:
        g.tap("up", 20)
        wait(g, lambda s: s.get("choice") is not None, True)
        neutral(g, 1200 - g.st()["frame_count"])
        capture(g, "safari-admission")
    finally: g.close()

    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).position("Route11Gate2F", 2, 7)
    for name in sb.species_order()[:80]: builder.dex(name)
    builder.data["game_data"]["player_direction"] = 4
    g = boot(builder, "aide-count")
    try:
        g.tap("a", 1)
        wait(g, lambda s: s.get("choice") is not None, True)
        g.choose("YES")
        wait(g, lambda s: s.get("dialogue") is not None)
        neutral(g, 1600 - g.st()["frame_count"])
        capture(g, "aide-count")
    finally: g.close()

    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).position("FuchsiaGoodRodHouse", 5, 4)
    for item in ["PokeBall", "GreatBall", "UltraBall", "Potion", "SuperPotion", "HyperPotion", "MaxPotion", "FullRestore", "Antidote", "BurnHeal", "IceHeal", "Awakening", "ParlyzHeal", "FullHeal", "Revive", "MaxRevive", "Repel", "SuperRepel", "MaxRepel", "EscapeRope"]:
        builder.give_item(__import__("re").sub(r"(?<!^)(?=[A-Z])", "_", item).upper(), 1)
    builder.data["game_data"]["player_direction"] = 8
    builder.data["game_data"]["player_direction"] = 4
    g = boot(builder, "rod-full")
    try:
        g.tap("a", 1)
        wait(g, lambda s: s.get("choice") is not None, True)
        g.choose("YES")
        wait(g, lambda s: s.get("dialogue") is not None)
        neutral(g, 1600 - g.st()["frame_count"])
        capture(g, "rod-full")
    finally: g.close()

    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).position("BillsHouse", 1, 5).last_map("Route25")
    for flag in ["EVENT_USED_CELL_SEPARATOR_ON_BILL", "EVENT_MET_BILL", "EVENT_MET_BILL_2", "EVENT_GOT_SS_TICKET", "EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING"]: builder.flag(flag)
    builder.data["game_data"]["player_direction"] = 4
    g = boot(builder, "bill-pc")
    try:
        g.tap("a", 1)
        wait(g, lambda s: s.get("choice") is not None or s.get("pc_phase") is not None, True)
        neutral(g, 1600 - g.st()["frame_count"])
        capture(g, "bill-pc")
        if args.label == "after":
            for species in ["EEVEE", "FLAREON", "JOLTEON", "VAPOREON"]:
                g.choose(species)
                wait(g, lambda s: (s.get("script_effect") or {}).get("child", {}).get("effect") == "ShowPokedexEntry")
                neutral(g, 30)
                g.tap("b", 20)
                wait(g, lambda s: s.get("choice") is not None)
                assert g.st()["choice"]["options"][g.st()["choice"]["selected"]] == species
            g.choose("CANCEL")
            wait(g, lambda s: s.get("active_script_effect") is None)
            assert g.st()["evaluation"]["pokedex"]["seen"] == 4
            assert g.st()["evaluation"]["pokedex"]["owned"] == 0
            assert g.d.cmd(cmd="save")["ok"]
            reloaded = pt.Game(save_path=g.save_path)
            try:
                pt.resume_reentry(reloaded)
                assert reloaded.st()["evaluation"]["pokedex"]["seen"] == 4
                assert reloaded.st()["evaluation"]["pokedex"]["owned"] == 0
            finally: reloaded.close()
    finally: g.close()

    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).position("VermilionDock", 14, 1).last_map("VermilionCity").flag("EVENT_SS_ANNE_LEFT")
    g = boot(builder, "vermilion-return")
    try:
        g.nav_warp(14, 0, "VermilionDock", "VermilionCity", approach="up")
        neutral(g, 1400 - g.st()["frame_count"])
        capture(g, "vermilion-return")
        if args.label == "after":
            assert g.d.cmd(cmd="get_flags")["data"].get("EVENT_WALKED_PAST_GUARD_AFTER_SS_ANNE_LEFT")
    finally: g.close()

    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).position("BillsHouse", 2, 6).last_map("Route25")
    for flag in ["EVENT_USED_CELL_SEPARATOR_ON_BILL", "EVENT_MET_BILL", "EVENT_MET_BILL_2", "EVENT_GOT_SS_TICKET"]: builder.flag(flag)
    g = boot(builder, "bill-return")
    try:
        g.d.cmd(cmd="set_flag", name="__OBJ_SHOWN_BILLS_HOUSE_OBJ_2", value=True)
        g.d.cmd(cmd="set_flag", name="__OBJ_HIDDEN_BILLS_HOUSE_OBJ_1", value=True)
        g.nav_warp(2, 7, "BillsHouse", "Route25", approach="down")
        g.nav_to(45, 4, "Route25")
        g.nav_warp(45, 3, "Route25", "BillsHouse")
        neutral(g, 1800 - g.st()["frame_count"])
        capture(g, "bill-return")
        evidence["bill-return"]["npcs"] = g.d.cmd(cmd="get_npcs")["data"]
        if args.label == "after":
            npcs = evidence["bill-return"]["npcs"]
            assert npcs[2]["visible"] and not npcs[1]["visible"]
            assert g.d.cmd(cmd="get_flags")["data"]["__OBJ_HIDDEN_ROUTE_24_OBJ_1"]
            assert g.d.cmd(cmd="save")["ok"]
            reloaded = pt.Game(save_path=g.save_path)
            try:
                pt.resume_reentry(reloaded)
                assert reloaded.d.cmd(cmd="get_npcs")["data"][2]["visible"]
            finally: reloaded.close()
    finally: g.close()

    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).position("Route22", 30, 5)
    builder.flag("EVENT_1ST_ROUTE22_RIVAL_BATTLE").flag("EVENT_ROUTE22_RIVAL_WANTS_BATTLE")
    builder.data["party"][0]["hp"] = 1
    g = boot(builder, "rival-loss")
    try:
        g.tap("left", 20)
        wait(g, lambda s: s["screen"] == "battle", True)
        for _ in range(500):
            st = g.st()
            if st.get("battle_message") and ("great or what" in " ".join(st["battle_message"].split()) or "blacked out" in st["battle_message"]): break
            if st["battle_phase"] == "PlayerMenu":
                g.tap("up", 1); g.tap("left", 1); g.tap("a", 10)
            elif st["battle_phase"] == "MoveSelect": g.tap("a", 10)
            else: g.tap("a", 10)
        else: raise AssertionError(g.st())
        neutral(g, 120)
        capture(g, "rival-loss")
        g.battle_loop()
        assert not g.d.cmd(cmd="get_flags")["data"].get("EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE", False)
    finally: g.close()

    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).position("Route16FlyHouse", 2, 4)
    builder.data["game_data"]["player_direction"] = 8
    builder.data["game_data"]["player_direction"] = 4
    g = boot(builder, "fly-first")
    try:
        g.tap("a", 1)
        wait(g, lambda s: "received" in (s.get("script_effect") or {}).get("text", ""), True)
        g.d.cmd(cmd="skip_dialogue")
        neutral(g, 2200 - g.st()["frame_count"])
        capture(g, "fly-first")
    finally: g.close()
(args.output / f"states-{args.label}.json").write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
