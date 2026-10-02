# 连续流程发现的转场硬锁

FLOW01 / P1：森林训练家等选择向外螺旋转场的战斗，在第一招（复现为String Shot）停住。
核心已结束入场，但front-end的 `transition_state` 始终未结束，
`is_frame_stable=false` 使 `complete_move_presentation` 无法被调用。

基线NEW GAME链在m01–m08通过后，m09两次卡在同一森林训练家；第一个失败状态位于
`first-clear/failure-state.json`，第二次调试亦为Weedle9对Bulbasaur5的首回合。
测试驱动的输入队列存在壁钟时序差异，因此不把两次总帧号作为完全一致的replay。
随后独立unit regression从完整trainer intro开始，固定seed42与两只精灵，5000帧仍不能
回到PlayerMenu，直接确认游戏问题；旧330个招式测试从PlayerMenu开始，没有覆盖入場轉場。

原作 `pret/pokered@fbcf7d0` 的 `engine/battle/battle_transitions.asm:186–205,262–326`
先探测当前方向左侧格，空格就转向并写入；已经黑的格则向当前方向前进。
它执行120批×3写入，再整体清黑。当前引擎却探测前方格，并没有固定结束条件，
会绕着已填格永久循环。修复位于依赖 [dotzuki #78](https://github.com/liuyanghejerry/dotzuki/pull/78)。

新增测试 `forest_string_shot_turn_finishes_after_real_trainer_intro` 在旧实现失败，
固定依赖到修复提交后通过。引擎另有首两批坐标、两种向外variant在120批结束的回归；
13项转场测试和全部476 renderer unit tests通过。

`docs/screenshots/fidelity-flow/outward-spiral-120-{before,after}.png`
来自同一公开renderer入口、同一160×144通用网格、同120次更新。
图在120时从“边缘已黑、中间仍空、永不done”变成全黑并done；fixture源码、
1/12/60/120四对PNG在引擎PR的 `docs/screenshots/fidelity-spiral/`。
这是实际转场层的通用渲染捕获，不是原版ROM截图。

最终集成NEW GAME连续流程的结果另见总审计README；单项unit通过不能替代通关验证。
