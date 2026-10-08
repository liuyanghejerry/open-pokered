# 通信游戏机角色与入场坐标（73、75）

原作版本：`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。

73：`engine/pokemon/bills_pc.asm` 的左右游戏机分别限制 INTERNAL_CLOCK 且朝右、EXTERNAL_CLOCK 且朝左。`engine/overworld/hidden_events.asm` 比较玩家前方坐标。因此 Host 只能在 `(3,4)` 朝右启动左游戏机，Guest 只能在 `(6,4)` 朝左启动右游戏机。复刻此前只检查地图，四种错误角色/从下方朝上操作均能启动。现在按角色、位置、朝向校验场景启动命令，错误操作保持双方 InRoom。

75：`data/maps/special_warps.asm` 的 TradeCenter/Colosseum 入场点是 `(3,4)` / `(6,4)`。原作房间脚本的 NPCMapX/NPCMapY 使用对象存储坐标；`macros/scripts/maps.asm` 的 object_event 明确存储 `coord + 4`，因此远端人物实际位置是 `(6,4)` / `(3,4)`。复刻此前双方入场 `(2,3)`，远端人物在 `(3,2)` / `(1,2)`。现在恢复原作位置和朝向；离线占位对象保持原有逻辑。

真实双游戏回归覆盖四种错误操作，并通过接待员对话、保存、选择交换、等待和地图切换验证双方入场及远端人物。导出已提交存档仍保留宝可梦中心接待员前的位置。

基于 master `fa6b8b0e4f7745ceec5c10f9f05caed628ecddeb`，debug-server 单元 185 通过；未优化、带调试信息的应用所有测试目标 466 通过。核心单元 2706 通过。独立 m01–m49 主线回归仍在运行，未作为完成证据。

三组截图采用同一只读测试模块、夹具和真实输入：错误朝向第 141 帧，双方入场第 549 帧；帧数、队伍相同。基线、模块 SHA-256 及状态见 `docs/screenshots/fidelity-73-75/manifest.json`。入场位置和远端人物差异属于本次修复预期。

交换动画后 serial 同步与 40 帧等待、状态页载入完整声画时间线仍在审计。

GBA 编译失败源于房间切换方法访问仅联网平台存在的 link_role；现已按调用方的平台条件限制该方法。
