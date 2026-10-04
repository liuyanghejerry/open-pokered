# NPC交换来源与捕捉辅助练级的边际收益

本次仅修改Python策略因子和只读记录，不改变原生画面或游戏资源。验证链仍不是最终完整合法NEW GAME，旧幽灵通行、来源标签和失败证据保留。

## 已验证能力

`RegistrationEvidence`现在消费现有`settle_special`拿到的原生NPC交易动画快照。一次动画只记录一个`npc_trade_started`，原有每次`step(10)`输入保持不变，不增加RPC或模型请求。

只有同时匹配以下证据才记录`npc_trade`：

- 已有操作前完整事实中的未完成NPC交换旗标、实际原生交易动画阶段与有序帧号；不把精简策略世界中省略的旗标当作False。
- 同一原生地图、单个图鉴登记增加、实际队伍/PC中恰好一个给出物种替换为一个收到物种、唯一队伍个体和保留等级。
- 原版使用的9组NPC交换目录中对应的地图/物种/新完成旗标。

未知阶段、Done阶段、拒绝、旧旗标、重复或额外个体、异地、等级不一致、陈旧观察、仅图鉴/计划目标变化都保持unknown。动画观察不会继承到后续登记或对战。不声称已穷尽认证原版菜单、随机属性或路线fidelity。

封存67的独立副本通过普通CONTINUE、实际PC取出22级毛球、正常移动及NPC交换，观察到`SlideInGiveMon`和22级蔓藤怪登记。交换前后现金11871、背包和物种库存守恒；零模型调用、零存档编辑/warp/seed、零正式收集新增。这是隔离能力回归，答案和获得结果不进入真实收集。

## 新决策因子

`capture_support_level_reference`区分下一等级的自然学习/进化提议、当前队伍有PP的非伤害催眠/麻痹工具，以及纯等级/属性投资。已登记进化和外部连接交换进化不计作新登记提议。提议不是获得，当前招式不是安全切换、命中或生存保证。

固定野生目标HP/状态、物种捕获率和球种时，辅助成员等级不进入原生投球公式；已有招式基础命中率也不随等级改变。实际命中仍受现场命中/闪避阶段及免疫影响。练级可能改善属性、出手顺序或生存，但本因子不编造下一等级属性、命中或生存概率，不强制练级、等级持平或任何收集顺序。

原生依据：`battle/capture.rs::CaptureContext`、`battle/accuracy.rs::accuracy_check`；原版固定参考：[ItemUseBall](https://github.com/pret/pokered/blob/d2704a63c26f9ba046ade877445216b3de0519a4/engine/items/item_effects.asm)、[InGameTrade_DoTrade](https://github.com/pret/pokered/blob/d2704a63c26f9ba046ade877445216b3de0519a4/engine/events/in_game_trades.asm)、[TradeMons](https://github.com/pret/pokered/blob/d2704a63c26f9ba046ade877445216b3de0519a4/data/events/trades.asm)。遵循[TypeSafe state](https://docs.typesafe.ai/concepts/state)的原则：程序提供确定事实，Jev比较完整选项，不代替程序计算已知规则。

## 回归

全部1231项scripts Python测试通过，含新增11项NPC记录测试、4项边际收益测试。同一封存67/同一原生/完整12目标的114候选前后逐项比较：所有事实、规则、目标、既有上下文相同，仅3个辅助练级候选新增边际收益因子；无裁剪、硬编码下一选择或模型答案复用。

第28段真实链的蔓藤怪原始日志仍为unknown，缺失的原生动画快照不会从隔离测试补写。保存核验和策略采用另有不可变证据记录；最终独立合法124、整片MP4及图鉴大盘仍待完成。
