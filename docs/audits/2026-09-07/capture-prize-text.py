#!/usr/bin/env python3
"""A17 修复验证：跑到 m06 劲敌战胜利，在奖金文本页 capture_frame。

修复前画面（审计证据 prize-player-name.png）：`Player got $...` + 小费/合计分页；
修复后应为原版 `_MoneyForWinningText`（text_2.asm:867）单页 `<名字> got $X for winning!`。
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import playthrough as pt  # noqa: E402

OUT = Path(__file__).resolve().parent / "prize-after-fixed.png"

g = pt.Game()
try:
    pt.m01_boot(g)
    pt.m02_oak_speech(g)
    pt.m03_leave_house(g)
    pt.m04_oak_intercept(g)
    pt.m05_take_starter(g, which="bulbasaur")
    g.nav_to(5, 6, "OaksLab")
    assert g.cutscene(), "rival challenge cutscene never finished"
    g.wait("screen=battle", 900)

    captured = False
    for _ in range(400):
        s = g.st()
        if s["screen"] != "battle":
            break
        ph = s["battle_phase"]
        msg = s.get("battle_message") or ""
        if not captured and "got $" in msg:
            r = g.d.cmd(cmd="capture_frame", path=str(OUT))
            assert r["ok"], f"capture_frame failed: {r}"
            print("captured prize page:", msg.replace("\n", " / "), flush=True)
            captured = True
        if ph == "PlayerMenu":
            g.d.drive(["up", "left"], frames=10)
            g.tap("a", 4)
            if g._await_phase("MoveSelect", 120):
                g._select_move()
            g.step(30)
        elif ph == "MoveSelect":
            g._select_move()
            g.step(30)
        elif ph == "ShiftPrompt":
            g.tap("a", 8)
            g.step(30)
        else:
            g.tap("a", 10)
    assert captured, "money text page never observed"
    g.wait("not_battle", 1800)
    print("OK:", OUT)
finally:
    g.close()
