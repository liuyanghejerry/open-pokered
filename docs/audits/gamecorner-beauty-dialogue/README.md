# Game Corner Beauty dialogue

> 归档说明（2026-11-11）：原始录制包、逐帧日志和运行报告已从当前源码树移除。下文保留历史结论与验证范围；旧附件可在清理前的 Git 历史中查阅。复测请使用保留的脚本，将生成文件写到临时目录。

Original `scripts/GameCorner.asm` GameCornerBeauty2Text only invokes `_GameCornerBeauty2Text` and ends. The scene appended COIN CASE checks, an invitation, PLAY/QUIT choice, and openSlots. The machine interactions already implement the original slot-machine entrances.

The added native scene regression covers with/without COIN CASE and 0/50 coins: Beauty2 produces only the original text; slotMachines still requests openSlots. All 63 `fidelity_` core tests and native debug-server build pass.

Actual input on master and fix starts adjacent to NPC4 at (2,14), with COIN CASE and 50 coins. Face UP, interact, wait for the original text, close it, and wait160 frames. Same-frame525 screenshots show master's added invitation and fixed player's restored control. A subsequent warp to (18,16), face UP, A, and neutral200 frames enters Slots on the fix. See verification.json. This is seeded input verification, not a fresh playthrough or every machine's odds validation.
