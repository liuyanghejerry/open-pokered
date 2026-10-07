"""Matched seeded captures: --binary PATH --label before|after --output DIR.

This exercises real dialogue, menus, and battles; only initial saves are seeded.
The Rocket image is eight neutral frames after his last dialogue closes.
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
from debug_drive import DebugClient

parser = argparse.ArgumentParser()
parser.add_argument("--binary", type=Path, required=True)
parser.add_argument("--label", choices=["before", "after"], required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
pt.BIN = sb.BIN = args.binary.resolve()
original_init = pt.Game.__init__
def init(self, *a, **kw):
    kw.setdefault("seed", 0)
    kw.setdefault("speed", 0)
    original_init(self, *a, **kw)
pt.Game.__init__ = init

def cmd(self, **kw):
    self.f.write(json.dumps(kw) + "\n")
    self.f.flush()
    if hasattr(socket, "TCP_QUICKACK"):
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_QUICKACK, 1)
    response = json.loads(self.f.readline())
    assert response["ok"], response
    return response
DebugClient.cmd = cmd

def drive(self, buttons, frames=None):
    buttons = list(buttons)
    count = len(buttons) if frames is None else frames
    return self.cmd(cmd="press_timeline", buttons=buttons + [None] * count, advance=True)
DebugClient.drive = drive

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

with tempfile.TemporaryDirectory(prefix="fidelity-sidequests-") as tmp:
    tmp = Path(tmp)
    # Export the canonical fresh save once, then let SaveBuilder reset its state.
    sb.SaveBuilder._tpl = sb.fresh_template()
    def boot(builder, name):
        path = builder.write(tmp / f"{name}.json")
        g = pt.Game(snapshot=path)
        pt.resume_reentry(g)
        neutral(g, 10)
        return g

    builder = sb.SaveBuilder().party_add("MrMime", 20).position("NameRatersHouse", 5, 4)
    builder.data["game_data"]["player_direction"] = 4
    builder.data["party"][0]["ot_id"] = 456
    builder.data["party"][0]["ot_name"] = [0x81, 0x8B, 0x94, 0x84] + [0x50] * 7
    g = boot(builder, "name-rater")
    try:
        g.tap("a", 1)
        wait(g, lambda s: s.get("choice") is not None, True)
        g.tap("a", 1)
        wait(g, lambda s: s.get("active_script_effect") == "ChoosePartyPokemon" and not s.get("dialogue"), True)
        g.tap("a", 1)
        wait(g, lambda s: s["screen"] == "overworld" and s.get("dialogue") is not None)
        neutral(g, 300)
        capture(g, "name-rater")
    finally:
        g.close()

    for name, position, coins, buy in [("coin-gift", (14, 12), 9990, False), ("coin-purchase", (5, 7), 9950, True)]:
        builder = sb.SaveBuilder().party_add("Bulbasaur", 5).money(5000).give_item("COIN_CASE", 1).position("GameCorner", *position)
        builder.data["game_data"]["player_direction"] = 4
        builder.data["game_data"]["player_coins"] = coins
        g = boot(builder, name)
        try:
            g.tap("a", 1)
            wait(g, lambda s: s.get("dialogue") is not None)
            if buy:
                wait(g, lambda s: s.get("choice") is not None, True)
                g.tap("a", 1)
            else:
                close_text(g)
            wait(g, lambda s: s.get("dialogue") is not None)
            neutral(g, 300)
            capture(g, name)
        finally:
            g.close()

    builder = sb.SaveBuilder().party_add("Bulbasaur", 5).flag("EVENT_GOT_POKEDEX").position("IndigoPlateauLobby", 11, 3)
    # Receptionist actual position comes from the map's NPC table.
    npc = json.loads((ROOT / "crates/pokered-data/maps/IndigoPlateauLobby/map.json").read_text())["npcs"][4]
    builder.position("IndigoPlateauLobby", npc["x"], npc["y"] + 1)
    builder.data["game_data"]["player_direction"] = 4
    g = boot(builder, "link-reception")
    try:
        g.tap("a", 1)
        wait(g, lambda s: s.get("dialogue") is not None)
        close_text(g)
        wait(g, lambda s: s.get("dialogue") is not None)
        neutral(g, 300)
        capture(g, "link-reception")
    finally:
        g.close()

    builder = sb.SaveBuilder().party_add("Mewtwo", 100, ["PsychicM"]).position("GameCorner", 10, 6)
    builder.data["game_data"]["player_direction"] = 4
    g = boot(builder, "rocket-exit")
    try:
        # Walk through real collision from the floor; never seed onto the
        # poster at (9,4), which is a solid wall cell.
        g.nav_to(10, 5, "GameCorner")
        g.face("left")
        assert (g.st()["player_x"], g.st()["player_y"]) == (10, 5)
        g.tap("a", 1)
        wait(g, lambda s: s["screen"] == "battle", True)
        g.battle_loop()
        # Close the end-battle and escape dialogue; don't advance the walk.
        for _ in range(20):
            st = g.st()
            if "hideout" in (st.get("script_effect") or {}).get("text", ""):
                g.d.cmd(cmd="skip_dialogue")
                neutral(g, 8)
                capture(g, "rocket-exit")
                break
            if st.get("dialogue"):
                close_text(g)
            else:
                neutral(g, 2)
        else:
            raise AssertionError(g.st())
    finally:
        g.close()
(args.output / f"states-{args.label}.json").write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
