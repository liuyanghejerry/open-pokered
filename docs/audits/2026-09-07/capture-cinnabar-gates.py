#!/usr/bin/env python3
"""A13 修复验证：warp 到红莲道馆门口，截图门前区域。

A03 让 @load 把六个门块写入运行时 map_data；A13 修复前渲染器读静态 .blk
（门不显示、玩家被隐形墙挡住），修复后读运行时块（门可见）。
"""
import socket
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
from debug_drive import DebugClient  # noqa: E402

OUT = Path(__file__).resolve().parent / "cinnabar-gates-render.png"

s = socket.socket()
s.bind(("127.0.0.1", 0))
port = s.getsockname()[1]
s.close()

cmd = [
    str(ROOT / "target/debug/pokered-app"),
    "run", "--headless", "--debug-port", str(port),
    "--skip-intro", "--warp", "CinnabarGym,17,7", "--no-audio",
]
proc = subprocess.Popen(cmd, cwd=str(ROOT),
                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
import time
try:
    d = None
    for _ in range(30):
        try:
            d = DebugClient(port)
            break
        except OSError:
            time.sleep(0.5)
    assert d is not None, "app never opened the debug port"
    d.cmd(cmd="step_frames", frames=90)
    r = d.cmd(cmd="capture_frame", path=str(OUT))
    assert r["ok"], f"capture failed: {r}"
    st = d.cmd(cmd="get_state")["data"]
    print("screen:", st.get("screen"), "map:", st.get("map_id") or st.get("map"))
    print("OK:", OUT)
finally:
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
