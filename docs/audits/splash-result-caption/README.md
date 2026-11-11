# Splash result caption

> 归档说明（2026-11-11）：原始录制包、逐帧日志和运行报告已从当前源码树移除。下文保留历史结论与验证范围；旧附件可在清理前的 Git 历史中查阅。复测请使用保留的脚本，将生成文件写到临时目录。

Original SplashEffect plays the move animation and jumps to PrintNoEffectText:
**No effect!** follows the move announcement. See original revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/effects.asm:1282-1284,1395-1401` and
`data/text/text_3.asm:200-202`.

The production translator now appends that result for an executed SplashEffect.
A blocked action has no MoveUsed event, so it does not gain a spurious result.
The existing localization entry is reused. No move mechanics or RNG rules change.

Fourteen prepared public-FIGHT turns cover both sides, ordinary/Substitute
conditions, sleep, freeze, failed and successful paralysis gates, plus two
consecutive turns where both sides use Splash. Tests check the result immediately
after each actual announcement, animation receipt counts, unchanged HP and exact
random consumption. On the parent, six executing matrix cases omit the caption,
and the both-Splash case omits both result pages. Blocked controls already pass.
The nicknamed animation issue found during fixture development is recorded
separately and is outside this patch.

All 3,700 core tests pass, with one existing ignored test. Fresh native NEW GAME
through m06 and all 11 subsystem scenarios pass with source and executable held
fixed. Seven original GBA performance and 13 memory scenarios pass with unchanged
workloads, gates and timeouts; minimum contiguous heap is 7,832 bytes and stack
is 8,708 bytes. The completed memory log was losslessly compressed and verified.
These are scoped regressions, not another complete first-clear run.

Before and after captures use identical public FIGHT inputs and frame 139:
before proceeds directly to the opponent, after shows the missing result.
The exact capture.rs drives actual BattleVisualEffects and draw_battle. It does
not drive the outer PokemonGame/audio clock or compare original-ROM timing.
Hashes and validation identities are in verification.json. All three remote
workflows and six actual GBA runtime steps must pass before merge.
