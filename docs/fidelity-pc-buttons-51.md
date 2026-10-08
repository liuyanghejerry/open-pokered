# 电脑菜单按键音差异 51

对照 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`：
`engine/menus/pc.asm` 的 PCMainMenu、`engine/menus/players_pc.asm` 的 PlayerPCMenu
设置 BIT_NO_MENU_BUTTON_SOUND，`home/window.asm` 的 HandleMenuInput 据此跳过 SFX_PRESS_AB。

修复前，共享运行时在所有电脑 A 输入时都播放 PressAB，造成原作无声菜单多出提示音。
修复后，宝可梦中心电脑和玩家电脑遵守该规则，操作本身的 EnterPC、WithdrawDeposit、
Save 和物种叫声仍由其语义事件播放。这是纯音频修复，没有屏幕输出变化。

## 验证

- 共享 app debug 全套 161 项通过，原有 3 项忽略不变；核心逻辑没有改动。
- `scripts/fidelity_pc_buttons.py DRIVER OUTPUT [--before]` 实际输入及原生 PCM 共 6 条路径。
  四条菜单路径：BILL 的存储列表、宝可梦动作菜单、宝可梦中心的玩家物品电脑列表、卧室物品列表。
  对齐帧 3003：基线均有 PressAB 的单声道 SFX，修复后四条都无活动 SFX 声道。
- 正向验证存入宝可梦和取出道具：两版活动 SFX 声道及其寄存器一致。
  皮卡丘叫声对照生产叫声入口，持续 48 tick；存入等待后显示回执并正确移动队伍/盒子。
  Potion 按数量取出后背包 +1、电脑 -1，音效仍播放。
- 每次从实际开场及 Oak 流程获得构造模板，保留两个宿主相同合法存档和 seed 0。
  不替换音序器、APU 或输入状态机，不用假音频断言。

## 联机状态页叫声差异 55

原作 `engine/link/cable_club.asm` 的 TradeCenter_DisplayStats 也依次调用
StatusScreen / StatusScreen2；`engine/pokemon/status_screen.asm` 的 StatusScreen
先 PlayCry，再等待 A/B。复刻联机状态是覆盖层，没有进入共享状态页，遗漏了叫声。
现在只在联机覆盖层首次打开状态页时播放当前选中宝可梦的叫声；翻页与退出不重复。
该修复也只有声音变化，没有屏幕输出变化。

`game::link_stats_cry_fidelity_tests::actual_trade_room_peer_stats_plays_selected_species_cry_once`
建立两个实际 PokemonGame，在交换中心合法位置以 ChannelTransport 连接；从桌上
游戏机的 A 互动、对方接受、切换对方列表、A 查看状态，全程调用实际 update 输入。
修复前观察到四个 SFX 通道全部关闭；修复后皮卡丘的活动通道为
`[true, true, false, true]`，叫声通道 APU 寄存器和 48 帧时长均匹配独立生产叫声入口。
随后 B 翻页、A 退出仍留在对方列表，且不会重播叫声。没有注入联机阶段或状态页。

本地保留修复前失败证据与修复后日志。默认功能完整 app 测试 418 项通过（基于 PR #128 修正联机集成测试后的提交）；
联机自己的 STATS / TRADE 操作菜单差异 56 正在独立审计，不能由本次叫声修复判为已完成。
