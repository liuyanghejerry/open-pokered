# TM/HM 队员兼容提示

原作 `engine/menus/party_menu.asm:83` 调用 CanLearnTM，替换 HP/状态区域并从第 12 列印出 ABLE 或 NOT ABLE。复刻的兼容位图与原作一致，但选择队员时只有 HP；这次让队员选择使用现有兼容规则显示提示。药品与普通队伍仍显示 HP 和状态。

原作实际 TM21/HM01 菜单各重复两次：合法妙蛙种子可学，等级 12 绿毛虫不可学。编译 ROM 的 7 字节 TM/HM 位图与原生 JSON 完全相同；菜单图块也实际读出两种提示。通过正常取消与 SAVE 确认没有学习、物品或队伍变化。完整原作记录在私有 `audit-861-original-tmhm-hints-bb8dn765`；这里保留输入、原始队员数据、菜单图块解码和保存哈希。没有原作/复刻逐帧一致声明。

发布对照另从当前 master `5941a7f1` 构建 before，以及本修复 `56b167f2` 构建 after。两个独立 debug-server 二进制使用相同初始 JSON、seed42 和完整输入，TM21/HM01 各重复两次，共 8 次实际菜单及普通 SAVE；没有输入后的状态修改，也不调用 debug SAVE。第 436 帧前后截图相同输入、相同菜单，仅队员行的提示变化。机器开机、教学确认、取消与保存画面字节相同；每一配对的完整 32 KiB SRAM、队伍、物品、金钱和事件标志也相同。新提示没有改变可学习规则。

58 项菜单渲染测试通过，覆盖昏倒/中毒队员仍按物种显示机器兼容性，机器不显示 HP/状态，以及普通药品保留原显示。两次客户端构建通过。此处未按 A 选择不兼容队员，实际拒绝或满招学习流程由独立审计处理；不把取消验证称为学习验证。源与原生的旧捕获率字节差异另由 #235 处理，本修复不改存档格式。

可重放输入（两个程序都须编译 debug-server）：

```sh
python3 docs/audits/machine-compatibility-hints/native_capture.py before /abs/base-debug /abs/new-before-evidence
python3 docs/audits/machine-compatibility-hints/native_capture.py after /abs/fixed-debug /abs/new-after-evidence
```

`verification.json` 保存构建、来源、输入、截图与保存身份；截图位于 `docs/screenshots/machine-compatibility-hints/`。
