# Mist before the primary accuracy roll

> 归档说明（2026-11-11）：原始录制包、逐帧日志和运行报告已从当前源码树移除。下文保留历史结论与验证范围；旧附件可在清理前的 Git 历史中查阅。复测请使用保留的脚本，将生成文件写到临时目录。

Original `MoveHitTest` rejects primary stat-down effects when the target has
Mist before checking X Accuracy or drawing the accuracy byte. Production
applied the Mist veto later, after consuming that byte. The stat protection
looked correct, but subsequent battle random choices used the wrong byte.

Move the primary Mist check to the original position. Damaging stat-drop
side effects still bypass this check, as verified by the preceding fix.
The existing invulnerability and X Accuracy ordering remains intact.

The actual production turn caller reproduced 36 extra draws across all nine
primary stat-drop moves and both sides. The 144-case regression also covers
no-Mist controls, successful and missed accuracy bytes, and X Accuracy.
All 3,686 core tests pass, with one existing ignored test. Source, failure
and passing log hashes are recorded in `verification.json`.

This is prepared-state battle-logic evidence, exempt from screenshots under
AGENTS.md. The committed combined source `b4db6d24` also passes the original
seven GBA performance scenarios and all 13 memory scenarios, including all
330 move animations. Minimum contiguous heap is 7,832 bytes and stack margin
is 8,724 bytes. All scenarios, thresholds, timeouts and workflows are unchanged.
Final remote CI/runtime gates remain prerequisites before merge. NPC stat-drop
miss rolls and other early-return cases remain separate audits.
