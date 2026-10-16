# 混乱自伤随机字节边界验证

混乱仍未解除时，原作对玩家和对手都使用同一个边界：随机字节小于 128 时正常行动，大于等于 128 时自伤。复刻此前把两个分支写反；总自伤概率仍是 128/256，但同一原始字节会选择相反的行动，并改变后续随机数是否被读取。

## 原作依据与修复范围

依据为 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`：

- 玩家：[engine/battle/core.asm:3427–3434](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/core.asm#L3427-L3434) 比较 `50 percent + 1`，小于阈值跳到正常行动。
- 对手：[engine/battle/core.asm:5768–5775](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/core.asm#L5768-L5775) 比较 `$80`，同样在较小字节时跳过自伤。
- [macros/data.asm:3](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/macros/data.asm#L3) 将 `percent` 定义为乘 255 后整除 100，因此玩家阈值也是 128。[BattleRandom](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/core.asm#L6543-L6564) 直接返回联机列表字节，普通路径转至 [Random](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/random.asm#L1-L12)。检查到的 native 调用链也直接传递原始字节，没有取补数。

上述三个官方原始文件已重新读取，字节 SHA-256 与早先源码发现记录中的解包文件完全相同；`core.asm` 为 `928a66d1ac444fc913c7c658d9f7739eb6351e797b3276da64a5de9682a57a5f`。早先记录仍保留其“未版本化解包文件、按字节哈希固定”的原始身份。

| 原始字节 | 原作及修复后 | 修复前 |
| --- | --- | --- |
| 0、127 | 正常行动 | 自伤 |
| 128、255 | 自伤 | 正常行动 |

生产改动位于 `battle/pokered_rules/p5_native.rs::confusion_gate`，将自伤条件改为 `roll >= 128`。减回合、解除混乱时不取随机数、混乱先于麻痹、后续招式处理及 PP 收费政策保持原有调用顺序。legacy 状态检查、测试侧栈 parity gate、随机流消费预测器及有意行动／自伤的向量同步采用原作边界；它们之间的一致性本身不作为原作正确性的依据。

## 首次完整核心回归

基线为已合入畏缩修复的 `e6150de4`，实际完整测试版本为 `2e76fe7d6a34abbd232c966638cd6aa6d83f846e`。在该 WT 专属的 `fidelity-1044-confusion-host-target` 上，以 `opt-level=0`、`debug=0`、`CARGO_INCREMENTAL=0` 运行一次：

```bash
cargo test -p pokered-core --offline --locked --target-dir "$TARGET" -j1
```

全部 **3386 项通过，0 失败**：2974 个 lib 测试与 412 个 integration 测试；另有 1 个已有 doc test 被忽略。实际 session `73082` 退出 0；runner `2585469` 和 Cargo `2585509` 的进程组均已结束。完整 argv、环境及原始输出保留在本地证据目录。

新增五项实际逐名通过：

- `real_turn_caller_uses_uncomplemented_link_confusion_bytes_on_both_sides`：通过 `BattleScreen` 规范回合调用，双方分别检查 0、127、128、255，以及 HP、实际招式、PP 和 RNG 索引。
- `gate_confusion_original_raw_boundaries_on_both_sides`：双方原始边界、Blocked／MoveUsed 事件与准确取数数量。
- `gate_confusion_link_list_keeps_raw_polarity_and_exact_index`：原始 LinkRng 列表、索引与消费量。
- `gate_confusion_original_polarity_preserves_paralysis_read_order`：正常混乱分支才继续读取麻痹字节。
- `gate_confusion_snap_out_never_reads_the_high_self_hit_byte`：混乱解除时不读取混乱判定字节。

继承的 5 项畏缩生命周期、4 项捕获结果回归，以及 `confusion_uses_typeless_modified_own_stats_and_cancels_charge` 也逐名通过。在隔离回合用例中，自伤只读 1 字节，正常攻击读 4 字节；这不是对所有战斗状态的统一取数断言。

## PR279 固定版本本地集成回归

在远端 PR279 的分支与 pull/head 均确认为 `1d2911cfe94c960385961e02ac2e9f5739e904df`、实际主分支仍为 `e6150de4` 后，将该固定版本用普通 merge 合入 `6257efe2`。实际集成版本为 `06d80d3dde1c2d7c9c9c5b5985cebd41397ff439`；没有冲突或人工源码调整，merge tree 与 Git 独立重算结果完全相同。相对 PR279 的差异仍仅为此前混乱修复及文档：除 `battle/mod.rs` 的两行测试模块注册外，各个混乱源码／测试文件与 `6257efe2` 字节一致；去掉该注册后，`battle/mod.rs` 与 PR279 字节一致。FIGHT receipt/API、等待、输入处理与混乱 `>= 128` 条件均保留。

在同一混乱 WT 原有专属 target 上，使用同样离线、锁定依赖、单任务及 `opt-level=0`／`debug=0` 环境，只运行一次完整 `pokered-core`。结果为 **3406 项通过，0 失败**（2994 lib + 412 integration），另有 1 个已有 doc test ignored。实际 session `27214` 退出 0，runner `3227809` 与 Cargo `3228015` 的进程组均已结束。新增五项混乱、继承畏缩／捕获、FIGHT 文本／通知及 Link 空 receipt 等 35 项指定测试逐名通过；所有测试 summary 均为 0 failed。没有修改断言／fixture 或重跑同版本。

本次冻结覆盖 2706 个非文档文件（2188 个编译相关文件）、702 项 GFX，以及 dotzuki `e533a3d2` 的 981 个 tracked 文件；前后源码完全一致。该次测试来自本地私有集成，测试操作没有修改实际主分支，也没有新增 App、原生 Game、GBA 或截图执行；PR279 自身的历史验证保持原身份。

随后 PR279 已实际合入主线 `e323f16e1de75fdd8b94dc347f70cfbd82fa7b4e`，该主线的整个文件树与已发布 `1d2911cf` 完全相同。本分支再用普通 merge 合入这个实际主线，生成 `15435de06749263d5099b21ff4ebc3f5fd2b34ce`；整个文件树与合并前文档版本 `4f9c8b8a` 完全相同，编译源码继续逐项等于实际跑过 3406 项测试的 `06d80d3d`。因此没有重跑 Cargo。后续文档提交只补充这一事实；当前实际基线为 `e323f16e`，测试证据仍明确标记为实际测试版本 `06d80d3d`，不把它重标成新的运行。

| 本次证据（同一本地证据目录） | SHA-256 |
| --- | --- |
| `audit-1045-confusion-pr279-integration-checkpoint-v1.json` | `5777a8221579a7c5a232acb416f8d040df98e26f962f1a7aef0e0b003996491c` |
| `audit-1045-confusion-pr279-integrated-core-v1/result.json` | `7b8adbd7200cb5405c30e5a2363f41d1acb8e3eeb0f05dee02446834dc1e1095` |
| `audit-1045-confusion-pr279-integrated-core-v1/full-core.log` | `d9a97a86f35d5b821192e03999c78738175f00af5db5b9700fe08eff38f8ed49` |
| `audit-1045-confusion-pr279-integrated-core-v1/source-before.json`／`source-after.json` | 均为 `62089c4a9f0240a335de42854d25721abd20d8642593c333eda0b988afe83279` |
| `audit-1045-confusion-pr279-integrated-core-receipt-v1.json` | `b0454fd91b5ef7f726f9e96d7d2d6854499a68b08a5ca33d62fd0947e87300b4` |
| `audit-1045-confusion-actualmaster-merge-source-equivalence-v1.json` | `5784878eb81e8bda16229faf9e713c44ffbe2e1313664c69edf484799ce27864` |

## 保留的失败记录与证据身份

前一版本 `710a5d8b` 的 lib 2974 项已通过，但完整包测试在 `fidelity_battle` 的自伤用例失败：该用例仍用字节 0，却要求 HP 从 400 降至 381。随后 `2e76fe7d` 仅将该测试两处刻意自伤向量首字节改为 128，并说明原作阈值；HP 381／356、蓄力与 INVULNERABLE 断言全部保留，生产源码未因这次失败再改。新提交的完整包测试通过；旧失败记录及其部分测试范围没有改写。

本地 `.task-tmp/fidelity-39-plus/` 保存以下记录：

| 证据 | 文件及 SHA-256 |
| --- | --- |
| 最初只读源码发现 | `audit-1040-confusion-polarity-readonly-finding-v1.json`：`29e09cd2caed1d0329f9f28c30f265e003bc5e5b6a2e7d5c362ec5fc65575875` |
| 完整包测试结果 | `audit-1044-confusion-full-core-v2/result.json`：`37d29b91504bd0a7550dd86556134aaa3ef89c6879261161373e2421e59cccf2` |
| 原始测试输出 | `audit-1044-confusion-full-core-v2/full-core.log`：`556327091e8ff0b12f8110cf36d31bf71e0d67266b67bee937e6e8e91b462b7e` |
| 前后源码冻结 | `audit-1044-confusion-full-core-v2/source-before.json`／`source-after.json`：均为 `5bc1b036beca41776cb3ff928abdd89915e2497a7e6ae082671e801f6b819bae` |
| 独立最终复核 | `audit-1044-confusion-final-fullcore-receipt-v2.json`：`f9db39bd0cabb700f0de20e41fab323e5330fd7c93b69ea046f1fbda764893b2` |
| 旧版本首败 | `audit-1044-confusion-full-core-v1/result.json`：`4186ff95998166c6a1a9721d025e3696917ffe3d0ea357a33a02eaecc066cc4b` |

首次完整回归的冻结覆盖 2700 个非文档文件（其中 2182 个编译相关文件）、702 项 GFX、锁定的 dotzuki `e533a3d2` 及其 981 个 tracked 文件。原文档提交 `6257efe2` 仅新增本文件，编译源码与当时实际测试版本 `2e76fe7d` 逐项等值；本次补充集成结果的提交也只修改文档，编译源码与 `06d80d3d` 等值。

本次为纯游戏逻辑及测试修复，按 AGENTS.md 的 pure game logic 排除项无需截图。验证范围为 core；本次未新增原生 Game、GBA、真实 Link 联机或原 ROM 逐帧验证。最终官方 CI 以 PR 实际 head 的检查记录为准。
