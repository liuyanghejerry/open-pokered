# Primary stat-drop prechecks

> 归档说明（2026-11-11）：原始录制包、逐帧日志和运行报告已从当前源码树移除。下文保留历史结论与验证范围；旧附件可在清理前的 Git 历史中查阅。复测请使用保留的脚本，将生成文件写到临时目录。

Original `StatModifierDownEffect` first draws an extra miss byte for the enemy
outside link battles: bytes below `25 percent + 1` (64/256) fail. It then checks
Substitute before entering `MoveHitTest`, which handles Mist and accuracy.
Production omitted the ordinary enemy roll and reached Substitute only after
accuracy, changing both stat results and subsequent random choices.

Carry the ordinary-enemy flag into the production rule adapter and perform
these primary-effect prechecks in their original order. Player attacks and
link peers have no extra miss roll. The primary Mist gate remains before
X Accuracy; damaging stat-down side effects remain a separate follow-up.

The production turn caller reproduced 27 wrong stat results and 162 wrong
random draw counts across 432 cases: nine moves, both sides, ordinary/link
modes, no protection/Substitute/Mist, and bytes 0/63/64/255. The deterministic
random source is supplied via the LinkRng slot independently of battle mode.
Both mode flags are asserted; two existing Mist fixtures now initialize both
flags just as the real link driver does. All 3,687 core tests pass, with one
existing ignored test. Exact failure, fixture and passing hashes are retained.

Pure battle logic is exempt from screenshots under AGENTS.md. This is prepared
production-turn evidence, not a fresh playthrough. Committed source `595060da`
also passes the seven original GBA performance scenarios and all 13 memory
scenarios, including 330 move animations, with 7,832 bytes minimum contiguous
heap and 8,708 bytes stack margin. No gates or workloads were changed. All
required remote workflows/runtime gates remain prerequisites before merge.
