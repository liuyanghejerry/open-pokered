"""Read-only game audit: fixture states, normal A interaction, observed mutations.

Run from repository root after building pokered-app with debug-server.
The original baseline is reviewed separately; this drives only the Rust port.
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
from debug_drive import DebugClient

OUT = Path(__file__).parent
PORT = 19002
ITEMS = [
    "POTION", "ANTIDOTE", "PARLYZ_HEAL", "BURN_HEAL", "ICE_HEAL", "AWAKENING",
    "FULL_HEAL", "SUPER_POTION", "HYPER_POTION", "MAX_POTION", "FULL_RESTORE",
    "REVIVE", "MAX_REVIVE", "ETHER", "MAX_ETHER", "ELIXER", "MAX_ELIXER",
    "X_ATTACK", "X_DEFEND", "X_SPEED",
]

log = []

def cmd(**kw):
    result = client.cmd(**kw)
    log.append({"command": kw, "response": result})
    if not result.get("ok"):
        raise RuntimeError(result)
    return result.get("data")

def observe():
    return {key: cmd(cmd=key) for key in ["get_state", "get_bag", "get_flags", "get_npcs"]}

def drain(limit=80):
    for _ in range(limit):
        state = client.state()
        if state["screen"] == "battle":
            raise RuntimeError("Unexpected battle")
        if state["dialogue"] is not None:
            cmd(cmd="skip_dialogue")
        elif state["choice"] is not None:
            cmd(cmd="press_timeline", buttons=["a", None], advance=True)
        elif state["script_running"] or state["active_script_effect"]:
            cmd(cmd="step_frames", count=20)
        else:
            return
    raise RuntimeError("Script did not finish")

def warp(map_name, x, y):
    cmd(cmd="warp", map=map_name, x=x, y=y)
    cmd(cmd="step_frames", count=60)
    drain()

def talk_up():
    cmd(cmd="press_timeline", buttons=["up", None, "a", None], advance=True)
    drain()

def fill_bag(voucher=False):
    items = ["BIKE_VOUCHER"] + ITEMS[:19] if voucher else ITEMS
    for item in items:
        cmd(cmd="give_item", item=item, qty=1)
    cmd(cmd="step_frames", count=2)

def flags(**values):
    for name, value in values.items():
        cmd(cmd="set_flag", name=name, value=value)

def fixture(name, setup, action=talk_up):
    global log
    log = []
    cmd(cmd="restore_state", slot=0)
    setup()
    before = observe()
    action()
    after = observe()
    data = {"case": name, "port_commit": "72ff719", "seed": 42,
            "before": before, "after": after, "transcript": log}
    (OUT / (name + ".json")).write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    print(name, "bag", len(before["get_bag"]), "->", len(after["get_bag"]),
          "flags", after["get_flags"], flush=True)

def bike():
    fill_bag(voucher=True)
    warp("BikeShop", 6, 4)

def misty():
    fill_bag()
    flags(EVENT_BEAT_MISTY=True, EVENT_BEAT_CERULEAN_GYM_TRAINER_0=True,
          EVENT_BEAT_CERULEAN_GYM_TRAINER_1=True)
    warp("CeruleanGym", 4, 3)

def thief():
    fill_bag()
    flags(EVENT_BEAT_CERULEAN_RIVAL=True, EVENT_BEAT_CERULEAN_ROCKET_THIEF=True)
    warp("CeruleanCity", 30, 9)

def tm45():
    fill_bag()
    for i in range(6):
        cmd(cmd="set_flag", name=f"EVENT_BEAT_ROUTE_24_TRAINER_{i}", value=True)
    flags(EVENT_GOT_NUGGET=True)
    warp("Route24", 10, 6)

def moonstone():
    fill_bag()
    for i in range(7):
        cmd(cmd="set_flag", name=f"EVENT_BEAT_MT_MOON_1_TRAINER_{i}", value=True)
    warp("MtMoon1F", 2, 3)

def hp_up():
    flags(EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD=True)
    for i in range(4):
        cmd(cmd="set_flag", name=f"EVENT_BEAT_MT_MOON_3_TRAINER_{i}", value=True)
    warp("MtMoonB2F", 25, 22)

def repeated_talk():
    talk_up()
    talk_up()

def magikarp():
    for _ in range(6):
        cmd(cmd="give_pokemon", species="Pidgey", level=5)
    warp("MtMoonPokecenter", 10, 7)

def daisy():
    flags(EVENT_FOLLOWED_OAK_INTO_LAB=True, EVENT_GOT_TOWN_MAP=True,
          EVENT_ENTERED_BLUES_HOUSE=True)
    warp("PalletTown", 15, 6)

def reenter_daisy():
    warp("BluesHouse", 3, 7)

process = subprocess.Popen(
    [str(ROOT / "target/debug/pokered-app"), "run", "--headless", "--debug-port", str(PORT),
     "--skip-intro", "--seed", "42", "--speed", "0", "--warp", "BikeShop"],
    cwd=ROOT, stdout=(OUT / "early-runtime-server.log").open("w"), stderr=subprocess.STDOUT)
try:
    client = DebugClient(PORT)
    cmd(cmd="step_frames", count=2)
    cmd(cmd="save_state", slot=0)
    fixture("early-bike-full-bag", bike)
    fixture("early-misty-full-bag", misty)
    fixture("early-thief-full-bag", thief)
    fixture("early-route24-tm45-full-bag", tm45)
    fixture("early-mtmoon-moonstone-full-bag", moonstone)
    fixture("early-mtmoon-hp-up-repeat", hp_up, repeated_talk)
    fixture("early-magikarp-party-full", magikarp, lambda: (cmd(cmd="interact_with", id="npc:3"), drain()))
    fixture("early-daisy-reentry", daisy, reenter_daisy)
    client.close()
finally:
    process.terminate()
    process.wait(timeout=10)
