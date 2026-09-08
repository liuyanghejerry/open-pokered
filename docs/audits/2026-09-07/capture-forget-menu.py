#!/usr/bin/env python3
"""A14 修复验证：对 4 招式宝可梦使用 HM01，截屏遗忘招式菜单。

原版 learn_move.asm:123：招式框在第 4 列、内部宽 14——长招式名（LEECH SEED /
POISONPOWDER）不越界。修复前沿用窄的动作菜单框，长名被截断。
"""
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
from debug_drive import DebugClient  # noqa: E402

OUT = Path(__file__).resolve().parent / "forget-menu.png"

s = socket.socket()
s.bind(("127.0.0.1", 0))
port = s.getsockname()[1]
s.close()

cmd = [
    str(ROOT / "target/debug/pokered-app"),
    "run", "--headless", "--debug-port", str(port),
    "--skip-intro", "--warp", "PalletTown,5,6", "--no-audio",
]
proc = subprocess.Popen(cmd, cwd=str(ROOT),
                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def wait_screen(client, want, tries=40, step=10):
    for _ in range(tries):
        st = client.cmd(cmd="get_state")["data"]
        if st.get("screen") == want and st.get("map_id") is not None:
            return st
        client.cmd(cmd="step_frames", frames=step)
    return st


try:
    d = None
    for _ in range(30):
        try:
            d = DebugClient(port)
            break
        except OSError:
            time.sleep(0.5)
    assert d is not None, "app never opened the debug port"

    st = wait_screen(d, "overworld")
    print("[1] booted:", st.get("screen"), st.get("map_id"), flush=True)
    r = d.cmd(cmd="give_pokemon", species="Venusaur", level=55)
    print("[2] give_pokemon:", r.get("ok"), flush=True)
    r = d.cmd(cmd="give_item", item="HM_01", qty=1)
    print("[3] give_item:", r.get("ok"), flush=True)

    party = d.cmd(cmd="get_state")["data"].get("party")
    print("[4] party:", party, flush=True)

    r = d.cmd(cmd="give_item", item="HM01", qty=1)
    print("[5] give_item HM01:", r.get("ok"), flush=True)

    d.cmd(cmd="press", button="start")
    d.cmd(cmd="step_frames", frames=5)
    print("[6] after start:", d.cmd(cmd="get_state")["data"].get("screen"), flush=True)
    d.cmd(cmd="press", button="down")  # skip POKéMON -> ITEM
    d.cmd(cmd="step_frames", frames=5)
    d.cmd(cmd="press", button="a")     # ITEM -> bag
    d.cmd(cmd="step_frames", frames=10)
    print("[7] after a1:", d.cmd(cmd="get_state")["data"].get("screen"), flush=True)
    d.cmd(cmd="press", button="a")     # HM01 (cursor 0)
    d.cmd(cmd="step_frames", frames=10)
    print("[8] after a2:", d.cmd(cmd="get_state")["data"].get("screen"), flush=True)
    d.cmd(cmd="press", button="a")     # USE
    d.cmd(cmd="step_frames", frames=10)
    print("[9] after a3:", d.cmd(cmd="get_state")["data"].get("screen"), flush=True)
    d.cmd(cmd="press", button="a")     # choose the mon
    d.cmd(cmd="step_frames", frames=12)
    st = d.cmd(cmd="get_state")["data"]
    print("[10] screen:", st.get("screen"), "| dialogue:", st.get("dialogue"), flush=True)
    r = d.cmd(cmd="capture_frame", path=str(OUT))
    assert r["ok"], f"capture failed: {r}"
    print("OK:", OUT)
finally:
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
