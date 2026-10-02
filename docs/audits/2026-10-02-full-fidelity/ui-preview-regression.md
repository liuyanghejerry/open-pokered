# 原字库集成后的 UI-preview 验证

实际 native UI-preview 结果为58项中43通过、15失败：14个固定图像哈希仍锁定旧Fusion ASCII/箭头/边框/命名坐标，另外一个命名JSON测试修改英文已固定为原作坐标的外框。现已逐一导出并查看14幅预览，恢复其哈希；命名测试改为移动仍可编辑的文本region，保留像素必须变化的断言。

预览检查同时确证一处实际回归：SUPER POTION整行按11字符加数量拼接，在8px原字库下将数量挤到右框外。`04286e4`将名字与数量分开绘制；数量固定右对齐，名字按实际glyph像素预算保留前缀并附原ellipsis。现有box位置保留，原作依据仅为`home/list_menu.asm:364,479–491`的独立名称/×/两位数量字段，不将本修复声称为完整原版背包坐标还原。

[同mock/同frame前后图与精确provenance](../../screenshots/2026-10-02-full-fidelity/ui-preview/README.md)已保留。真实生产pixel回归覆盖EN/ZH×SuperPotion/ThunderStone×1/99八种状态；每位数量按原font tile逐bit核验，同时确保一tile空隙和原右框纹理未被覆盖。原生产renderer复跑该回归失败，修复后通过；[after pixel日志](validation/battle-bag-quantity-pixel-tests.log)与[before预期失败](validation/battle-bag-quantity-before-expected-failure.log)分别记录结果。

最小修复之后，原生raw-rustc核验为57/57菜单测试、58/58 UI-preview测试；[完整preview日志](validation/ui-preview-original-font-tests.log)记录所有golden与layout fixture结果。该核验使用实际Cargo preview build的精确serde-feature extern图，临时源仅移除WASM export annotations；没有修改生产render bodies或占用Cargo。最终标准Cargo/CI结果由主集成验证记录更新。
