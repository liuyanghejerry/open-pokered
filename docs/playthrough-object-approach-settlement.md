# 对象接近期间的训练家触发与结算

这是主线控制工具修复，未改变游戏引擎或剧情。

## Fresh 失败证据

以冻结生产二进制 SHA256
`9ff93ef98a2cd19d01d7792b0868180a3a0fd92e208ae180e0a0daf4e2c7254b`
运行未恢复存档的 m01–m49，导航工具版本为 `b232f87e`。
m01–m21 已实际完成；进程在 m22 以 exit 1 终止。

调用链为 `m22_lift_key` → `talk_npc` → `approach_object` → `face`。
导航到 RocketHideoutB4F (11,3) 后，(11,2) 朝下的训练家启动视线事件。
`nav_to` 保留“请求目的地上的事件由调用者负责”的契约，先返回；
`approach_object` 随即发送 UP，但正常战前对话已锁住方向输入。
frame 535768 的状态仍朝 Left，正在打印
“The elevator doesn't work? Who has the LIFT KEY?”，字符 13/47。
原作该句是 `RocketHideoutB4FRocket3BattleText`，不是战后台词；
`EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2` 尚未设置。
调试字段里的 TrainerVictory 属于先前 GameCorner 战斗，不能用它推断当前战斗阶段。

日志、状态、flags、NPC 和原始视频保留在私有审计目录
`audit-844-fresh-main-flow-retry2`。视频完整解码通过：133947 个录制帧，
160×144、60fps、2232.45 秒。录制抽样不能当作每个 runtime frame 的逐帧证明。
没有重启该 fresh 流程，也没有把后面的构造场景算为 fresh m22 或通关成功。

## 控制工具变更

`approach_object` 在邻格导航返回后先用中性输入等待 64 帧。
该上限来自现有训练家感叹号的 32 个 field ticks，以及普通地图每两个渲染帧
推进一次 field tick。感叹号/走近阶段已经阻止控制，但 `get_state` 和
`control_ready` 尚未暴露该阶段；只检查已出现的 dialogue 不足以处理它。

随后观察真正出现的 dialogue、script、choice 或 battle。现有 `cutscene`
负责确认文本，现有 `battle_loop` 负责实际战斗；文本关闭后还推进一个正常更新，
再观察可能的延迟战斗交接。普通选择框仍明确失败；没有新增自动 YES 或命名输入。

结算后重新检查地图、玩家目标邻格和 NPC。若已观察到的 NPC 消失或移动，
返回交互调用者，由其验证完成事件或获取 NPC 新位置。地图改变仍失败；
玩家被位移则重新规划。原有四次接近次数、`face` 的朝向/坐标断言和故事事件
断言均保留。没有修改 `nav_to` 对请求目的地事件的所有权。

## 加速验证与局限

同一份初始 JSON 存档和同一冻结二进制，分别执行原 helper 和修复 helper。
这是一次构造 B4F 诊断，不是历史 fresh 存档的恢复：SaveBuilder 提供完整模板，
把初始位置设为 (12,3)，使用已知 canonical flags、Venusaur 37、原失败场景的
招式/PP、金额和徽章。默认 DVs/经验/统计与原 fresh 状态不同，模板的姓名、
图鉴和其他字段保留。构造时写入 Left，实际启动画面为 Down；真实左移建立了
失败位置上的 Left 朝向。初始训练家未打败、钥匙未掉落且钥匙对象隐藏均已核对。
启动后只使用真实输入和只读观察，没有 warp 或 flags/状态注入命令。

原 helper 在静默训练家 intro 阶段的 frame 47 发生同一朝向断言失败。
只加可见 dialogue guard 的中间版本在 frame 51 仍失败，证据保留。
最终版本连续两次完成真实视线台词、训练家战斗、二次对话掉落钥匙和拾取：
每次最终为 B4F (10,3) Up/Idle，frame 2190，战胜/掉落事件均为 true，
背包含 LiftKey×1，无待处理对话或脚本。

私有证据 `audit-858-m22-settlement` 保存初始存档、构造说明、每次命令/响应、
结果和全部 PNG。五次保留实验共 6668 个 PNG 均可完整解码、160×144，
录制编号连续；其中最终版本两次各 2190 帧。
该证据不声称渲染原作逐帧一致或 m49 全流程成功。

交互单测 17 个通过，覆盖目的地上的台词/静默 intro/延迟战斗、script-only、
choice/超时、败退地图变化、等待期间地图变化、NPC 消失/移位与玩家位移。
既有导航 46 个和恢复 32 个单测通过，共 95 个。没有运行 Cargo。
