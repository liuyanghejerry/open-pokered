# 保留项目字体后的 UI-preview 验证

按用户要求恢复并保留项目原有 Fusion Pixel 字体。14个固定图像哈希已用当前实际预览重新导出、逐张查看后更新，边框、光标、命名专用下划线及命名字段修复继续保留。英文命名键盘框为 `(0,4,20,12)`，大小写切换行在16，给现有字体较高的标点留出空间。命名JSON测试继续移动可编辑的文本region，保留像素必须变化的断言。

Battle Bag图对已重新从真正的 `72ff719` 冻结库生成before，与当前项目字体的after比较：POTION×3、SUPER POTION×1、ANTIDOTE×2、cursor0、English、frame0。**原项目字体基线中，SUPER POTION及数量已经完整位于框内，未观察到数量溢出。** 当前变化是数量独立右对齐并保留边框/光标修复，不再将中间8px字体引入的回归描述成原基线缺陷。

[同mock/同frame图对与精确来源](../../screenshots/2026-10-02-full-fidelity/ui-preview/README.md)记录before72库、after实际构建HEAD及未提交布局差异hash、依赖库和PNG checksum。after捕获时HEAD为 `2c6c941`，含记录的布局工作树修改；没有追溯标成后来完整构建。全部14张当前预览、Cargo JSON artifacts、hash打印与最终日志保存在 `/tmp/pokered-font-preserved-preview`。

标准 `cargo test --offline --locked -p pokered-ui-preview`（`CARGO_INCREMENTAL=0`）通过全部58项，doc target0项，退出码0。测试既验证14个golden，也保留命名metadata及可编辑region、布局JSON变化产生像素差异等行为断言。数量的生产pixel回归以现有Fusion Pixel绘制和测量为oracle，覆盖EN/ZH、SUPER POTION/THUNDERSTONE、1/99与右边框；此前使用临时8px字库的before失败日志属于历史记录，不作为项目字体基线缺陷证据。新图对变化1184像素，包含边框、光标与数量位置调整。

另外用冻结72库及真实 `PokemonRenderData` 验证了中文模式默认mock，以及英/中文模式的 SUPER POTION、THUNDERSTONE×99。最长量宽为80px，从x56到x136，右边框为x152，三张图均逐张核验，无数量溢出。72版真实provider在中文模式仍返回英文道具名；本次证据不声称生产环境已出现中文道具名溢出。额外图和日志位于上述临时目录，manifest记录其checksum。

对话行距修复 `d7291ea` 后再次导出并逐张核验14张：只有DIALOG变化364像素，golden更新为 `599168bcd2b2e67c`，另13张完全相同，Battle Bag图对及其实际来源不变。比例绘制的对话/战斗文本默认行距为12px，英文两行起点y112/y124，原有10px字体的下伸笔画与下边框分离；字形、字号和tile mock坐标均未改变。额外EN/ZH两行 `gyp` 图也已核验。当前58项preview测试及doc0项再次通过，精确d729 capture/库hash/日志见manifest。
