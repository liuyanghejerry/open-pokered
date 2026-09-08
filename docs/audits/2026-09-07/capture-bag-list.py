#!/usr/bin/env python3
"""A10 修复验证：注入 25 件物品后打开背包，截屏长列表。

修复前 Auto 高度随条目数增长——底边框出屏、光标行不可见；修复后高度封顶并
按窗口滚动（光标可见、框不出屏）。
"""
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
from debug_drive import DebugClient  # noqa: E402

OUT = Path(__file__).resolve().parent / "bag-long-list.png"

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

    wait_screen(d, "overworld")
    items = [
        "POTION", "ANTIDOTE", "BURN_HEAL", "ICE_HEAL", "AWAKENING",
        "PARLYZ_HEAL", "FULL_RESTORE", "MAX_POTION", "HYPER_POTION",
        "SUPER_POTION", "REVIVE", "MAX_REVIVE", "FULL_HEAL", "ESCAPE_ROPE",
        "POKE_DOLL", "ETHER", "MAX_ETHER", "ELIXER", "MAX_ELIXER",
        "DIRE_HIT", "X_ATTACK", "X_DEFEND", "X_SPEED", "X_SPECIAL",
        "GUARD_SPEC",
    ]
    for it in items:
        r = d.cmd(cmd="give_item", item=it, qty=1)
        assert r["ok"], f"give_item {it} failed: {r}"

    d.cmd(cmd="press", button="start")
    d.cmd(cmd="step_frames", frames=5)
    d.cmd(cmd="press", button="a")   # ITEM (cursor 0)
    d.cmd(cmd="step_frames", frames=12)
    st = d.cmd(cmd="get_state")["data"]
    print("screen:", st.get("screen"))
    r = d.cmd(cmd="capture_frame", path=str(OUT))
    assert r["ok"], f"capture failed: {r}"
    print("OK:", OUT)
finally:
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
