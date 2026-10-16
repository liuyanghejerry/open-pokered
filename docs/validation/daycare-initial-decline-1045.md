# 育屋首次拒绝台词验证

修复的是尚未寄养宝可梦时，首次询问中的“否”及 B 取消。原作从 `.ComeAgainText` 开始，复刻此前误用了包含 `.AllRightThenText` 的完整句子。选宝可梦时取消、拒绝支付领回费用仍使用完整句子。

原作依据为 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c` 的 [scripts/Daycare.asm:14–20](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/Daycare.asm#L14-L20) 与 [text/Daycare_2.asm:1–8](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/text/Daycare_2.asm#L1-L8)。本次实际下载三个 Daycare 原始文件，HTTP 200、完整字节均等于本地原作引用；没有原 ROM 逐帧验证的声明。

## 首次实际 Game 前后对照

基线为 master `e6150de4`，实际修复源码为 `f04254e0614afbaf62377892668d8ba6ffa1b6b1`。使用同一不可变 CLI `c9976465`，其 2699 项非文档文件、2181 项编译文件与该 master 等值；702 项 GFX 固定。两侧均用 `--scripts-dir` 的原生 AST 解释器，分别加载 master 及修复分支脚本。因此这是修复脚本的实际 Game 运行，未声称重新编译了修复版 CLI。

预先准备的双卡比兽 JSON 存档从育屋进入，实际第 0 帧出生点为 `(2,7)`，随后用普通方向键走到 `(1,3)` 与老人对话。两侧固定 seed 42、英文、无音频；各完整重放同一条 **7615 帧**按键序列，各有 **419 次 RPC、204 个状态端点及 12 张截图**。首轮走位中的单帧按键未命中与碰撞也完整保留，之后用普通持续方向键完成导航。未使用运行中改状态、跳过对话、RNG 读取／改写或恢复存档。

| 场景 | 相同帧 | 前 | 后 |
| --- | --- | --- | --- |
| 首次询问选择“否” | 2627 | All right then, come again. | come again. |
| 首次询问按 B | 3519 | All right then, come again. | come again. |
| 进入队伍选择后按 B | 4742 | 完整句子 | 完整句子，截图字节相同 |
| 实际寄养后拒绝领回报价 | 7446 | 完整句子及金钱框 | 完整句子及金钱框，截图字节相同 |

204 对完整公开状态中，只有首次拒绝的两个端点的 `dialogue`、`dialogue_state`、`script_effect` 文本字段不同，其他字段与端点完全相同。首次拒绝及队伍取消保留两只队员；实际寄养后拒绝领回仍保留一只队员及 ¥3000。最后第 7615 帧已关闭对话并交还控制。Game 进程仅在验证结束后由驱动关闭，两侧进程组均已退出。

本次未重新运行全主线、SRAM SAVE／独立进程 CONTINUE 或 GBA；编译嵌入脚本与跨平台构建以 PR 的官方 CI 为准。此次只改首次拒绝台词，没有对话框布局变更。

本地证据位于 `.task-tmp/fidelity-39-plus/`：

- `audit-1045-daycare-pair-review-v1.json`：`7eb1291ad8bc3aaabcd749a2dd90d5b2c98cf4a5b354f25a945d3c35e6f3b227`
- `audit-1045-daycare-pair-peer-review.json`：`85623fe9c3014bd3da95cd744e75936ab939b503a86811e9b464b55341fb0c60`
- `audit-1045-daycare-fixed-actions-v1.json`：`81f921f324cb8c217d3b9002c3d4aa834f01dd557b75210aa57393cb5060ff82`
- `audit-1045-daycare-primary-http-v1/result.json`：`7fbec88843fcb202258a2a07b3ace9d75c74eda93bfb830544dfb7dd4cad2063`

## 整合 PR279 后的最终对照

在首次记录之后，将 PR279 固定发布版本 `1d2911cfe94c960385961e02ac2e9f5739e904df` 合入本分支，实际整合源码为 `2a5f213383c8db3cd0f753a2cce2fdd054224c57`。换用实际编译于 `a1efdac02` 的不可变 CLI（SHA-256 `a8747aac9cd73ea64e855782e40353699b41070a69e7ca8f300e235c4e7154a2`），其 2705 项非文档文件和 2187 项编译文件与 `1d2911cf` 等值；两侧继续用同一 CLI 加载对应脚本。完整重放原先冻结的同一条 7615 帧输入，两侧仍各 204 状态、419 RPC、12 截图，没有重新选路线或调整按键。

最终对照仍仅在第 2627、3519 帧的初问 NO/B 出现三项文本字段差异，其余 202 对完整状态相同。所有 24 张截图分别与首次运行字节相同，四张公开截图也逐一等于最终运行的实际 PNG。两个取消控制、队员、金钱与最后交还控制均再次通过；相关驱动和 Game 进程组已退出。

最终运行发生时基线仍明确标为发布版本 `1d2911cf`。随后 PR279 实际合入 master `e323f16e1de75fdd8b94dc347f70cfbd82fa7b4e`，其整个 Git 文件树与 `1d2911cf` 相同。本分支再合入实际 master，整个文件树没有变化；之后只补充本验证文档。因此当前实际 PR 基线为 `e323f16e`，运行记录仍保留实际 CLI `a1efdac02` 和修复源码 `2a5f2133` 的身份。

- `audit-1045-daycare-pair-review-v2.json`：`cbbb5fab31e4ce6894d08993014579d34b57e2266edb59724b36dc44471ae8c8`
- `audit-1045-daycare-pair-peer-review-v2.json`：`a1216a4d0a561d9596a50b492f1d10a5077696510d292ec9d4f9f147ade93274`
- 最终完整源码逐项清单 `audit-1045-daycare-v2-candidate-full-source-peer-manifest.json`：`6970cbbf5c95fecd5847a34d11e47e78d2aa0bd70e78bedc008df3e113013aac`
