# 状态页按键顺序差异 46

对照 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`：
`engine/menus/start_sub_menus.asm` 的 `.choseStats`、
`engine/pokemon/bills_pc.asm` 的 `.viewStats` 及 `engine/link/cable_club.asm` 的
TradeCenter_DisplayStats 都依次调用 StatusScreen 和 StatusScreen2。
两页各自使用 WaitForTextScrollButtonPress，A/B 都结束当前页，然后返回调用者。

修复前：A 在两页之间循环；B 在能力页直接退出，在招式页返回能力页。
修复后：能力页按 A/B 进入招式页，招式页按 A/B 返回队伍或电脑。
进入时的叫声仍只播放一次，翻页不会再次进入状态页或重复播放叫声。

## 验证

- 基线为 PR #127 合入后的 master（提交号在 PR 描述中）。基线与修复版使用相同的
  只读 `stats_state` 测试观测入口，实际输入仍由共享 PokemonGame 处理。
- 联机交换集成测试按同样的两页顺序检查对方状态，A/B 组合退出后仍停留在对方列表，
  再切换回己方、选择宝可梦、确认并完成交换。
- 核心全套 3118 项、共享 app debug 全套 160 项及默认功能完整 app 测试 416 项通过（含联机交换集成测试）。测试覆盖 AA/AB/BA/BB，保留无输入时不翻页检查。
- `scripts/fidelity_stats_navigation.py DRIVER OUTPUT [--before]` 从合法构造存档实际打开
  队伍/电脑状态页，两个入口分别验证四种按键组合。修复后均返回正确调用者；电脑保留 MonAction。
- 同一存档、seed 0、输入：第一下 B 的截图对齐绝对帧 3003，第二下 A 的截图对齐帧 3303。
- 独立全新 m01–m49、结局自动存档、独立进程 CONTINUE 通过。
- 首次完整回归在玉虹道馆连续三次战败后停止：玩家妙蛙花 L36、HP 0、麻痹，结束状态为
  TrainerVictory/player_won=false，回到玉虹市 41,10；日志与失败状态保留在本地
  `.task-tmp/fidelity-39-plus/stats-46-full-run/`。再次完整回归未修改路线、断言或战败重试次数。
  该战斗失败的机制与驱动诊断仍在长期审计中，不能凭主线另一次通过判定所有战斗行为一致。

同一 PR 还修复队伍光标记忆、上下循环、CANCEL 去向及单只队伍 SWITCH，见
`docs/fidelity-party-menus-47-50.md`。
