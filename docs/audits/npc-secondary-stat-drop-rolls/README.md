# NPC damaging stat-drop prechecks

> 归档说明（2026-11-11）：原始录制包、逐帧日志和运行报告已从当前源码树移除。下文保留历史结论与验证范围；旧附件可在清理前的 Git 历史中查阅。复测请使用保留的脚本，将生成文件写到临时目录。

Original `StatModifierDownEffect` applies the ordinary enemy's `25 percent + 1`
miss check to damaging secondary effects as well as primary moves. This draws
before the Substitute check and the `33 percent + 1` secondary chance. A broken
Substitute has already cleared the effect before dispatch. `MoveMissed` returns
silently for secondaries, so the damage still lands without a failure message.
See `engine/battle/effects.asm:539-563,712-716` at original revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.

Production previously omitted that enemy miss roll. The bridge now applies it
after the broken-doll early return and before checking an intact doll. Player
attacks and link battles retain their existing secondary chance. Mist still
allows damaging secondary stat drops.

840 prepared production-turn cases cover all six moves, both sides, ordinary
and link modes, no protection / intact doll / broken doll / Mist / Mist with a
doll, and both roll boundaries. Before the fix, 144 stat outcomes and 288 random
draw counts differed. Afterwards, all 3,688 core tests pass (one existing ignored
test). The full run also exposed an old fixture that set `BattleScreen.link_mode`
without `BattleState.link_battle`; both flags now identify that fixture as a link
battle, matching the real link driver.

Product source `b57f89f1be20187bdbc1f3453420d56903d9b3b0` passes the original seven
GBA performance scenarios and 13 memory scenarios, including all 330 move
animations. Minimum contiguous heap is 7,832 bytes and untouched stack is 8,708
bytes. No workloads, gates or timeouts changed. Exact source and log hashes are
in `verification.json`; documentation-only commits preserve these product files.

This is prepared-state production-path evidence, not an original-ROM recording
or a new full playthrough. Pure game logic is exempt from screenshots under
AGENTS.md. All final remote workflows and six actual GBA runtime gates remain
required before merge. The canonical build target is reused without retaining
another ROM copy; the completed memory log is losslessly compressed.
