# RQ1（Abstraction）+ RQ2（Planning）实验报告

分支 `exp/rq1-rq2`；提交 `442f200`（WP1 适配器）、`5779765`（WP2 RQ1 矩阵）、
`6d41a7d`（WP3 RQ2 planner+消融）。全部运行产物在 gitignored 的
`target/agent/runs/` 下（RQ1 总表 `rq1/SUMMARY.md`，RQ2 总表
`rq2/SUMMARY.md`，逐单元 JSONL 同级目录）。实验基础设施 M1–M7 见
PR #87（已合入 master）。

## 1. 背景与 RQ 定义

引用设计简报《open-pokered AI World Model — Agent Environment — Initial
Design Brief》839–872 行：

- **RQ1 — Abstraction**：观测与动作抽象层级如何影响长时程 agent 表现？
  比较 pixels / symbolic state / world model 与 buttons / navigation /
  semantic actions 的组合。
- **RQ2 — Planning**：显式世界模型与层级动作能否提升长时程规划的
  效率与可靠性？比较 button-level agent / skill-level agent /
  hierarchical planner / oracle planner。

本文覆盖：RQ1 的 LLM 三 tier 矩阵（配额受阻）与 RQ2 的
hierarchical planner + 世界模型消融（全脚本、完整执行）。

## 2. 方法

- **驱动模式**：所有运行一律 `--speed 0`（driven-only）——游戏帧只在
  同步 debug 命令内推进，帧计数与墙钟解耦；与校准矩阵完全可比。
- **种子**：1 / 42 / 777（与校准一致）。
- **任务子集**（6 个，校准 8 任务中的小任务）：reach-viridian-city、
  reach-pewter-city、talk-to-oak、acquire-potion、win-wild-battle、
  beat-brock。get-starter / beat-one-trainer 作为次一级难度裁掉；
  get-pokedex 为 oracle:false 排除。
- **预算惯例**：T2/T3 帧预算 = 2× oracle 中位帧（floor 2000）；
  T1 = 20000 帧。模型调用封顶 T3 40 / T2 60 / T1 400。
- **模型**：`Qwen/Qwen2.5-Coder-3B-Instruct`（HF Inference Providers
  探针可用的最小 instruct 模型），temperature 0，`OPEN_POKERED_MODEL`
  环境变量可覆盖；凭据解析顺序 OPENAI_BASE_URL+OPENAI_API_KEY →
  HF_TOKEN。回复严格全串解析，失败计次 + 一次纠错重试 + 有界降级。
- **tier 语义对齐决定**：RQ1 的 T2 **禁用 travel_to**（WP1 冒烟中模型
  直接 travel_to 两步取胜、与 oracle 同帧，使 T2 坍缩成 T3）。禁用后
  与校准期 T2 LocalExplorer 的"无世界模型导航"设定一致；T3 保留
  世界图 route 信息。

## 3. RQ1 结果（配额受阻，完整披露）

### 3.1 对比表（`target/agent/runs/rq1/SUMMARY.md`）

| tier | success | median frames (wins) | median model calls | prompt tokens | completion tokens |
|---|---|---|---|---|---|
| T1 | **0/2** | — | 7 | 4052 | 56 |
| T2 | **0/1** | — | 9 | 7487 | 64 |
| T3 | **0/1** | — | 0 | 0 | 0 |
| oracle (scripted, ref) | **16/16** | 1203 | 0 | 0 | 0 |

### 3.2 配额受阻披露

HuggingFace Inference Providers 账户**月度包含额度耗尽（HTTP 402，
非瞬时限流）**。执行顺序与单元状态：

| 单元 | 状态 | 撞墙前数据 |
|---|---|---|
| T1 × seed 42（试跑 1） | attempted → blocked | 9 调用 / 46 帧 / 2872+36 tok |
| T1 × seed 42（试跑 2） | attempted → blocked | 5 调用 / 25 帧 / 1180+20 tok |
| T3 × seed 42 | attempted → blocked | 0 调用即 402 |
| T2 × seed 42（禁 travel_to） | attempted → blocked | 9 调用 / 93 帧 / 7487+64 tok，pf=0 |
| 其余全部单元 | **not run**（按停止规则收缩，不记为失败） | — |

停止规则执行：T1 试跑撞 402 → 探针确认端点仅对极小请求放行 →
收缩到最小可行矩阵（每 tier 各 1 单元）→ 最小单元仍全部撞墙 →
如实记录为配额负结果，未继续消耗。

### 3.3 关键发现

- **T1（按钮层）在该模型/端点下成本结构性不可行**（独立于配额）：
  部分轨迹中每模型调用仅覆盖约 5 帧（模型倾向 `up x4`–`x8` 小步
  drive），2 万帧预算意味着数千次调用/运行。即使配额充足，T1 全矩阵
  的调用量也不现实。这是 T1 失败模式的主因，与校准期 scripted T1
  （零调用）形成结构性成本差。
- **T2 禁 travel_to 后未坍缩但更慢且 token 密集**：9 次决策全部格式
  合规（pf=0，严格全串解析 + 纠错重试有效）；prompt 含 nearby 实体使
  T2 prompt 强度约 830 tokens/调用（T1 约 320）。
- 链路的端到端可行性已被 WP1 冒烟证实（travel_to 开启的 T2 曾以
  3 调用 / 873 帧成功，与 oracle 同帧）——WP2 的规则化矩阵恰逢月度
  配额见底，是配额约束而非设计约束。

## 4. RQ2 结果（完整执行）

### 4.1 hierarchical planner 设计（事件图 → 可执行目标序列）

加载 M4 事件图（`crates/pokered-data/story/graph.json`，3213 边，
 storyline/flag/map/trainer/item 节点）：

1. 目标翻译：flag/item 目标映射为图节点；`map`/`battle_won` 目标图
   不可表示，用明确标注的内建策略（travel 计划 / 草地行走计划）。
2. 生产者搜索：`sets`/`gives` 入边找候选 storyline，按当前地图到
   `triggered_at` 的世界图路由腿数 + `requires` 数排序候选。
3. 前提：`requires` 是 storyline 的 flag 读取，无极性（Oak 演讲要求
   APPEARED 置位且 FOLLOWED 未置位）→ v1 仅作注解；自环读取丢弃；
   运行时目标验证为唯一权威门。带极性的递归前提展开留 v2。
4. 执行（世界模型条件，travel_to 允许）：travel_to → 触发（@load
   入场结算；npc 绑定经 M4 语义 text_id → 活体实体 interact_with，
   ≤4 次拦截重试）→ starts_battle 战斗自动解决 → 胜后庆典结算 →
   验证。候选顺序回退；全败优雅 `no_path:<stage>`。

### 4.2 hierarchical vs oracle（18/18 全过）

| task | hierarchical | oracle (校准参考) |
|---|---|---|
| reach-viridian-city | 3/3（873/873/1503 帧） | 3/3（中位 873） |
| reach-pewter-city | 3/3（4042–5314 帧） | 3/3（中位 4672） |
| talk-to-oak | 3/3（386 帧） | 3/3（中位 376） |
| acquire-potion | 3/3（669 帧，4 步） | 3/3（中位 1208） |
| win-wild-battle | 3/3（496–1198 帧） | 3/3（中位 622） |
| beat-brock | 3/3（6138–7485 帧，2 战） | 1/1（6582 帧） |

**hierarchical 18/18（中位 873 帧） vs oracle 16/16（中位 1203 帧）**。
planner 在多任务上与 oracle 同帧；acquire-potion 显著更优
（669 vs 1208）——事件图给出 `Route1:talkYoungster1` 免费药水生产者
（距起点 1 路由腿），优于 oracle 的 ViridianCity 隐藏点，验证了
"图驱动发现优于人工标注点"的 hierarchical 性质。

### 4.3 世界模型消融（30% 种子化删边，3 抽样 × 3 种子 × 6 任务）

| condition | success | median frames (wins) | no_path rate |
|---|---|---|---|
| hierarchical（完整图） | **18/18** | 873 | 0/18 |
| ablation-30% | **42/54 (78%)** | 1503 | 12/54 |

分任务衰减（每单元 9 个样本）：

| task | ablation-30% | 分析 |
|---|---|---|
| reach-viridian-city | 9/9 | 对照组：旅行图未删，不受影响 ✓ |
| reach-pewter-city | 9/9 | 同上 ✓ |
| win-wild-battle | 9/9 | 内建策略，不依赖事件图 ✓ |
| talk-to-oak | 4/9 | 单生产者（@load）：5 样本删其 `sets` 边 → no_producer |
| acquire-potion | 8/9 | 多生产者回退韧性：首选被删后仍经 MtMoon/ViridianForest 成功；1 例 `no_path:unreachable_map:MtMoon1F` |
| beat-brock | 3/9 | 3 样本 no_producer；3 样本 goal_unverified |

beat-brock 的 goal_unverified 样本暴露了一个结构性依赖：
`starts_battle` 边被删时，planner 仍能找到 talkBrock（`sets` 边在），
但挑战对话无法被识别为"对话后接战斗"，Brock 之战不触发——planner
对**执行语义**（怎么打）的依赖与对**发现语义**（去哪打）同样真实。

## 5. 局限

- `requires` 无极性：v1 仅作注解 + 运行时验证；带极性的递归前提
  展开（真多级规划）留 v2。本矩阵任务均为 1–2 层分解。
- RQ1 LLM 单元受阻于 HF 月度配额（HTTP 402），T1/T2/T3 各仅 1–2 个
  attempted 单元，无成功率可言；LLM 非确定性虽已 temperature 0 +
  全串解析约束，端点输出仍非逐位确定。
- 单模型（Qwen2.5-Coder-3B-Instruct）、单端点（HF router）；结论不
  外推至更大模型或其他提供方。
- 消融仅删事件图边；旅行世界图、脚本语义（script_semantics）未纳入
  消融面（v2 可扩展）。

## 6. 续跑路径

配额恢复（或提供 `OPENAI_BASE_URL` + `OPENAI_API_KEY` 任意
OpenAI 兼容端点）后：

```bash
python3 scripts/openpokered/run_rq1.py            # 断点续跑，已有 JSONL 自动跳过
python3 scripts/openpokered/run_rq1.py --aggregate-only   # 仅重建总表（配额安全）
python3 scripts/openpokered/run_rq2.py            # RQ2 同理（全脚本，随时可重放）
python3 scripts/openpokered/run_rq2.py --aggregate-only
```

测试门禁：`python3 -m unittest scripts.test_openpokered
scripts.test_openpokered_variants scripts.test_openpokered_rq1
scripts.test_openpokered_llm scripts.test_openpokered_planner`（64 例）。
