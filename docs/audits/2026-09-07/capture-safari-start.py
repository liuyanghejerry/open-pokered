#!/usr/bin/env python3
"""A11 修复验证：进入狩猎地带，按 START 打开菜单，截图信息框区域。

原版 PrintSafariZoneSteps（player_state.asm:225）：内部 7×3 信息框，
"NNN/500" 在屏格 (1,1)、"BALL×NN" 在 (1,3)。修复前 7×4 总框只有两行内部，
球数行落到下边框之外。
"""
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
from debug_drive import DebugClient  # noqa: E402

OUT = Path(__file__).resolve().parent / "safari-start-info.png"

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

    # Wait for boot to settle at the pre-warp map before issuing ours,
    # otherwise the app's own init races and clobbers the debug warp.
    booted = wait_screen(d, "overworld")
    print("booted map:", booted.get("map_id"))

    warp_disabled = len(sys.argv) < 2
    if not warp_disabled:
        r = d.cmd(cmd="warp", map="SafariZoneWest", x=14, y=7)
        assert r["ok"], f"warp failed: {r}"
        st = wait_screen(d, "overworld")
        print("warped map:", st.get("map_id"))

    d.cmd(cmd="step_frames", frames=30)
    d.cmd(cmd="press", button="start")
    for i in range(8):
        st = d.cmd(cmd="get_state")["data"]
        print(f"frame+{i}: screen={st.get('screen')}")
        d.cmd(cmd="step_frames", frames=1)
    r = d.cmd(cmd="capture_frame", path=str(OUT))
    assert r["ok"], f"capture failed: {r}"
    print("OK:", OUT)
finally:
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
