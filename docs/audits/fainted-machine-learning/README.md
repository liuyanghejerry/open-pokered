# 昏倒队员的 TM/HM 学习

原作 `engine/items/item_effects.asm:2154` 的 `ItemUseTMHM` 在选择队员后检查兼容性、是否已知招式，再调用 `LearnMove`；没有当前 HP 检查。复刻已实现机器开机、教学确认和队员选择，但 `bag_use::apply_item_to_pokemon` 在 HP 为 0 时提前返回无效。这次只移除 TM/HM 的这条限制。

`original-verification.json` 是原作实际菜单操作的四次采集：TM03、HM01 各重复两次。开机前构造等级 6 的昏倒妙蛙种子，合法招式为 Tackle/Growl，第三槽为空；另有一名健康的等级 6 妙蛙种子。开始输入后不改 RAM。学会 Swords Dance 或 Cut 后，两个完整 44 字节队员记录只改变第一名队员的第三招式及其 PP（30）；HP 仍为 0。TM03 从 2 份减至 1 份，HM01 的唯一一份保留。此原作采集没有保存到 SRAM 的声明。

复刻验证使用相同的开机前 JSON、种子 42、逐帧按键，通过 START/ITEM/USE、机器开机、教学确认、队员选择和普通 SAVE/YES；没有开机后的状态修改，也不调用 debug SAVE。修复前后截图对齐到结果消息的第 712 帧。完整记录还检查全部队员字段、健康伙伴、背包、金钱、剧情标志与 SRAM 校验和。`native_capture.py` 可复现两组操作，每组、每个构建各重复两次；传入独立的 debug-server 可执行文件及新的证据目录：

```sh
python3 docs/audits/fainted-machine-learning/native_capture.py before /abs/base-debug /abs/evidence
python3 docs/audits/fainted-machine-learning/native_capture.py after /abs/fixed-debug /abs/evidence
```

四个核心回归测试覆盖空槽学习及非招式字段保持、已知或不兼容招式拒绝、满槽替换及 PP Up 重置、昏倒队员选择直接进入应用。这里没有原作逐帧视觉或完整 TM/HM 满槽对话流程一致性的声明。

验证完成：四个针对性核心测试通过，基于 `72f9d593` 的全部 2856 个核心测试通过，debug-server 客户端构建通过。TM03、HM01 在修复前后均重复两次；每组输入、结果帧和保存哈希重复一致。`native-verification.json` 保留构建与脚本哈希、完整队员读回数据、逐次输入、SRAM 原始队伍记录和前后配对检查。截图位于 `docs/screenshots/fainted-machine-learning/`。原作记录仅作为对照断言；运行实现不读取测试记录。

发布前已合并 master `6a0ef88b`（维他命具体能力提示）。该更新不经过本次 TM/HM 用例的路径；采集二进制仍对应上面记录的 `72f9d593` 基线，不能称为发布头的完整二进制验证。
