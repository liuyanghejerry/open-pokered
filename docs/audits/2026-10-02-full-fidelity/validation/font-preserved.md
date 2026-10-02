# 保留项目原有字体

用户明确要求不修改字体。本 PR 的普通英文、中文及数字文本继续通过
`dotzuki_renderer::embedded_font` 使用原有 Fusion Pixel；native、Web、GBA、TUI
共享相同绘字、缩放、字宽和测宽路径。未更改引擎字体资源。

原作 alphabet、extra-font、number-symbol 字库替换已撤回。
仅保留修复菜单所需的六个边框图块（48字节）、两个 PK/MN 图块（16字节）
及命名槽图块；这些是专用 UI 图形，不替换普通文本提供器。
任意 GB tile 的文字 fallback 也回到原项目提供器。

纯为 8px 替换字体增加的中文 stats 编号／主人堆叠行已经撤回，
标签和值恢复原有并排行与真实字宽对齐。Town Map 给两种语言的原有 10px
字体留足高度；PC 的 PK/MN 拼接按真实像素坐标放置，避免 5px Latin 前进
被误当作 8px tile 后发生错位。数量列仍根据真实字宽保护右框。

独立字体提供器逐像素回归覆盖 English、中文、混排与通用 tile fallback。
UI 聚焦正式 Cargo 回归共 84 次执行通过（14 lib、4 dialog、2 mart、57 menus、
4 stats、3 transparency）；包括 EN/ZH 长物品名及 1/99 数量的实际 framebuffer
回归。`font-preserved-ui-tests.log` 保存命令结果。新的预览与截图按各自构建
哈希记录；此前 8px 字体实验截图不作为最终画面证据。

实际 EXP `points!` 截图进一步发现原字体下伸笔画会与原作边框 y=138 相接。
两行对话在真实比例字体路径改为 12px 间距（第一行 y=112，第二行 y=124），
保留原有字体及中文基线；独立提供器逐像素检查 `gyp points!` 完整字形，
并确认 y=136..137 与边框间留白。5 个 dialog 像素测试与 57 个菜单测试通过，
见 `font-preserved-descenders.log`。此修正调整行距，不修改字体资源或字号。

最终生产源码 `eebde2fc3a4f78c38b4c7b5d49e0db2b830cb235` 的全部16个workspace包
通过123个测试目标、4596次执行（0失败、9跳过），包含上述绘字与行距回归。
Web三包编译通过，最终普通文本均保留同一提供器。最终截图的实际源码、冻结依赖及
二进制哈希见 [验收记录](final-validation.md) 与 [最终构建清单](final-checked-build-manifest.json)。
