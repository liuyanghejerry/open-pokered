# OpenPokeRed as an AI Agent Research Environment

## 1. 核心设想

`open-pokered` 不应该仅仅被看作一个 Pokémon Red 的 Rust 重制项目。

由于它拥有：

- 完整、可修改的 Rust 游戏运行时
- 结构化地图、NPC、Trainer、Item、Pokémon 等游戏数据
- `.scene` 描述的剧情和事件逻辑
- Headless execution
- Debug / state inspection API
- Deterministic frame stepping（同步步进，不依赖 wall-clock；分支回放见第 13 节）
- 可直接访问的游戏内部状态
- 可选的 framebuffer / visual observation

它天然适合进一步发展成一个：

> **面向长期自主 Agent 的、具有完整语义 Ground Truth 的 RPG Research Environment。**

核心研究问题不再是：

> AI 能不能通关 Pokémon Red？

而是：

> **一个 Agent 在完成长期复杂任务时，感知、状态表示、动作抽象、世界建模、规划和执行分别贡献了多少能力？失败又发生在哪一层？**

---

# 2. 与传统 Pokémon AI 环境的区别

传统 Pokémon AI 环境通常采用：

```text
ROM
 ↓
Emulator
 ↓
Pixels / RAM
 ↓
Agent
 ↓
Controller buttons
```

Agent 最终只能通过：

```text
Up
Down
Left
Right
A
B
Start
Select
```

操作游戏。

即使能够读取 RAM，也主要是在观察：

> 游戏现在是什么状态？

而 `open-pokered` 可以进一步提供：

```text
                 open-pokered
                       │
        ┌──────────────┴──────────────┐
        │                             │
  Runtime State                 Static World
        │                             │
  position                       maps
  party                          warps
  bag                            NPCs
  flags                          trainers
  battle                         items
  dialogue                       scripts
        │                             │
        └──────────────┬──────────────┘
                       ↓
                 World Model
                       ↓
                 Agent API
                       ↓
             Planner / RL / LLM
```

因此不仅可以回答：

> 当前发生了什么？

理论上还可以回答：

> 世界由什么组成？

> 哪些状态转换是可能的？

> 一个事件依赖哪些条件？

> 某个动作可能改变什么？

> 从当前状态到目标状态需要经过哪些步骤？

---

# 3. 多层 Observation

一个核心设计原则是：

> **同一个游戏，允许研究者控制 Agent 能看到多少信息。**

可以定义不同 Observation Level。

## Level 0 — Vision

```text
framebuffer
```

Agent 只能看到游戏画面。

---

## Level 1 — Vision + Runtime State

```text
framebuffer
+
position
party
bag
battle
flags
```

---

## Level 2 — Symbolic State

```text
current map
position
nearby entities
party
bag
flags
battle
dialogue
```

不需要依赖视觉识别游戏状态。

---

## Level 3 — Local World Model

额外提供：

```text
collision
nearby NPCs
nearby items
warps
local map topology
```

---

## Level 4 — Global World Model

进一步提供：

```text
map graph
known entities
world topology
event structure
```

---

## Level 5 — Oracle

Agent 获得完整的 Ground Truth World Model。

包括：

```text
maps
warps
entities
scripts
event dependencies
hidden state
```

Oracle 可以作为理论性能上界。

---

# 4. 多层 Action Space

Observation 可以分层，Action 同样可以。

## Level 0 — Controller

```text
press(Up)
press(Down)
press(A)
```

---

## Level 1 — Navigation

```text
move_to(x, y)
```

系统负责：

```text
pathfinding
↓
controller input
↓
closed-loop verification
```

---

## Level 2 — Interaction

```text
interact(npc)
interact(item)
enter(warp)
```

---

## Level 3 — World Navigation

```text
travel_to("PewterCity")
travel_to("PewterGym")
```

系统自动完成：

```text
world graph search
↓
local navigation
↓
warp traversal
```

---

## Level 4 — Skills

```text
heal_at_pokemon_center()
buy_item(...)
catch_pokemon(...)
defeat_trainer(...)
```

---

## Level 5 — Goals

```text
achieve("GetPokedex")
achieve("BeatBrock")
achieve("BecomeChampion")
```

Agent 或 Planner 自动进行任务分解。

---

# 5. Hierarchical Agency

最终可以形成一个完整的动作层级：

```text
Goal
 │
 ▼
achieve("BeatBrock")
 │
 ▼
Task
 │
 ▼
travel_to("PewterGym")
 │
 ▼
Skill
 │
 ▼
interact(Brock)
 │
 ▼
Navigation
 │
 ▼
move_to(x, y)
 │
 ▼
Motor Action
 │
 ▼
Up / Down / Left / Right / A / B
```

这样可以系统研究：

> **Agent 应该在哪一层进行 reasoning？**

例如：

| Agent | Decision Level           |
| ----- | ------------------------ |
| A     | 每一步 controller button |
| B     | `move_to()`              |
| C     | `travel_to()`            |
| D     | reusable skills          |
| E     | high-level objectives    |

然后比较：

- Success Rate
- Environment Steps
- Token Cost
- Planning Errors
- Recovery Ability
- Wall-clock Time

---

# 6. 研究方向一：Abstraction

第一个核心研究问题：

> **Observation 和 Action abstraction 对长期 Agent 性能有多大影响？**

可以进行标准 Ablation：

| Observation            | Action           |
| ---------------------- | ---------------- |
| Pixels                 | Buttons          |
| Pixels + State         | Buttons          |
| Symbolic               | Buttons          |
| Symbolic               | `move_to`        |
| Symbolic + World Graph | Semantic Actions |
| Oracle World Model     | Semantic Actions |

然后测量：

```text
Beat Brock success
Beat Misty success
Champion success
environment steps
token usage
wall-clock cost
error rate
```

这样可以把 Agent 的能力分解开。

---

# 7. 研究方向二：Long-Horizon Planning

Pokémon Red 天然具有非常长的任务 Horizon。

例如：

```text
Start
 ↓
Get Starter
 ↓
Get Pokédex
 ↓
Brock
 ↓
Misty
 ↓
...
 ↓
Elite Four
 ↓
Champion
```

中间同时涉及：

```text
navigation
NPC interaction
inventory
party management
level progression
battle
HM requirements
story flags
optional objectives
```

因此可以研究：

> **Agent 能否在数千甚至数万次 environment interactions 中保持 coherent long-term plan？**

指标可以包括：

```text
goal completion
planning efficiency
backtracking
repeated mistakes
unnecessary interactions
resource efficiency
failure recovery
```

---

# 8. 研究方向三：World Model Learning

这是 `open-pokered` 最特殊的优势之一。

由于我们拥有完整源码，可以构建机器可验证的：

```text
Ground Truth World Model
│
├── map graph
├── collision
├── NPC identity
├── trainer identity
├── item locations
├── scripts
├── event flags
├── event transitions
└── battle state
```

然后让 Agent 在不知道完整世界的情况下探索游戏并自己构建 World Model。

最终比较：

```text
Agent Inferred World Model
            ↓
          versus
            ↓
Ground Truth World Model
```

可以测：

```text
map graph accuracy
entity grounding accuracy
event prediction accuracy
precondition accuracy
transition prediction accuracy
```

这使 World Model Learning 可以得到自动化、客观的评分。

其中 event dependencies / event transitions 的 Ground Truth 可以直接从 `.scene` AST 静态分析提取（flag reads/writes、item grants、warps、battle triggers），无需人工标注，也无需完美的符号执行——未知构造显式标记即可。

---

# 9. 研究方向四：Action Consequence Prediction

还可以把游戏转化成 World Model Prediction Benchmark。

给 Agent：

```text
State S_t
+
Action A_t
```

要求预测：

```text
State S_(t+1)
```

例如：

> 如果现在与这个 NPC 对话，会发生什么？

Agent 预测：

```text
dialogue starts
flag X becomes true
item Y acquired
NPC disappears
```

然后真正执行游戏：

```text
predicted state
      ↓
    compare
      ↓
actual state
```

可以自动评价 Agent 对环境因果结构的理解程度。

---

# 10. 研究方向五：Partial Observability

环境拥有完整 Ground Truth，但不代表 Agent 必须看到它。

这是一个非常重要的区别：

```text
WorldModel
=
environment ground truth

AgentObservation
=
information exposed to agent
```

因此可以人为控制信息量：

```text
Oracle
████████████████

Full Symbolic
██████████████

Local Symbolic
██████████

Vision + Memory
██████

Pixels Only
██
```

并研究：

> 世界知识减少时，Agent 的性能如何下降？

也可以逐项增加信息：

```text
+ coordinates
+ NPC identity
+ inventory
+ collision
+ map graph
+ event flags
+ event dependencies
```

从而测量每一种信息带来的 marginal benefit。

这同时构成一条 API 设计约束：Observation API 必须支持按 Level 裁剪信息——同一份 Ground Truth，按配置暴露不同子集；omniscience 不应该被硬编码进接口。

---

# 11. Oracle Gap

一个很有价值的 Benchmark 概念是：

> **Oracle Gap**

建立拥有完整 World Model 和 Planner 的 Oracle Agent。

然后比较：

```text
Vision Agent
Symbolic Agent
LLM Agent
RL Agent
Hierarchical Agent
Oracle Agent
```

最终尝试把总性能差距拆解为：

```text
Perception Gap
      +
Grounding Gap
      +
Navigation Gap
      +
Planning Gap
      +
Execution Gap
      =
Total Agent Gap
```

这可能成为整个 Benchmark 最有辨识度的设计之一。

因为它不仅告诉我们：

> Agent 失败了。

而是进一步回答：

> **Agent 为什么失败？**

---

# 12. Research Direction: Generalization

如果游戏的数据层足够独立，可以进一步生成 Pokémon Red variants。

例如改变：

```text
map topology
warp topology
NPC locations
trainer teams
item locations
wild encounters
event prerequisites
quest structure
```

然后：

```text
Train / Develop
        ↓
Original Pokémon Red

Test
        ↓
Unseen World Variant
```

研究：

> Agent 是记住了 Pokémon Red 的攻略，还是学会了真正的探索、世界建模和规划？

这是区分：

```text
memorization
```

和：

```text
generalizable agency
```

非常重要的一步。

实现上，`open-pokered` 的数据层已经支持运行时数据覆盖（runtime overrides / 外部 maps 数据目录），因此 world variants 可以作为纯数据生成，不需要修改引擎——这使 RQ3 从设想变成可执行的实验。

---

# 13. Counterfactual Simulation

`open-pokered` 的 frame stepping 已经是确定性的。在此基础上，未来还可以支持完整的 state fork / restore：

```text
save state
↓
try action A
↓
observe future
↓
restore
↓
try action B
↓
observe future
```

也就是：

```rust
let checkpoint = env.save_state();

for action in candidate_actions {
    env.restore(&checkpoint);
    let outcome = env.step(action);
    evaluate(outcome);
}

env.restore(&checkpoint);
```

这允许研究：

- Search
- MCTS
- Model-Based Planning
- Counterfactual Reasoning
- Battle Lookahead

传统 emulator 环境也可以尝试 save-state，但原生 runtime + semantic state 可以让这种研究更加干净。

前置工程：RNG 种子化 + 运行时状态（overworld / battle / script engine）的完整序列化。目前的快照粒度为存档级（save data round-trip），帧级 fork/restore 是 planned capability，而非现有能力。

---

# 14. Benchmark Tasks

不需要一开始就要求通关整个游戏。

可以设计分层任务。

## Navigation

```text
Reach Viridian City
Reach Pewter City
Reach Pokémon Center
```

## Interaction

```text
Talk to Oak
Acquire Potion
Buy Poké Ball
```

## Battle

```text
Win one wild battle
Beat one trainer
Beat Brock
```

## Story

```text
Get Starter
Get Pokédex
Beat Brock
Beat Misty
Acquire Surf
```

## Long Horizon

```text
Reach Elite Four
Become Champion
```

---

# 15. Benchmark Metrics

基础指标：

```text
Success Rate
Environment Steps
Wall-clock Time
Token Usage
Model Calls
```

规划指标：

```text
Plan Length
Backtracking
Repeated Actions
Invalid Actions
Recovery Rate
```

World Model 指标：

```text
Map Accuracy
Entity Accuracy
Transition Accuracy
Precondition Accuracy
Effect Prediction Accuracy
```

资源指标：

```text
Money Usage
Item Usage
Party Health
Blackouts
Grinding Cost
```

这样可以避免把所有结果压缩成一个：

```text
reward = 12345
```

环境本身通过 language-agnostic 的 JSON/TCP 协议暴露（基于现有 debug server 扩展），Python / Gym adapter 只是薄壳；Token Usage、Model Calls 等指标在 adapter 层采集，不进入环境核心。

---

# 16. 三个核心 Research Questions

第一篇工作不应该试图回答所有问题。

可以集中在三个 Research Questions。

## RQ1 — Abstraction

> **How do observation and action abstractions affect long-horizon agent performance?**

比较：

```text
pixels
symbolic state
world model
```

以及：

```text
buttons
navigation
semantic actions
```

---

## RQ2 — Planning

> **Can explicit world models and hierarchical actions improve the efficiency and reliability of long-horizon planning?**

比较：

```text
button-level agent
skill-level agent
hierarchical planner
oracle planner
```

---

## RQ3 — Generalization

> **Do agents learn reusable world-modeling and planning capabilities, or merely memorize a fixed game?**

通过 unseen Pokémon world variants 测试。

---

# 17. 与现有研究环境的关系

这个项目与几个已有研究方向存在明显联系。

### MiniHack / NetHack

值得借鉴：

```text
pixel observation
symbolic observation
text observation
```

同一个游戏允许不同 Observation abstraction。

---

### AI2-THOR

值得借鉴：

```text
visual observation
+
structured metadata
+
semantic actions
```

尤其适合作为 Agent API 的设计参考。

---

### ALFWorld

值得借鉴：

```text
abstract symbolic reasoning
        ↓
low-level embodied execution
```

与我们：

```text
goal
 ↓
semantic action
 ↓
navigation
 ↓
controller
```

非常接近。

---

### Voyager

值得借鉴：

```text
high-level planning
+
reusable skills
+
long-horizon autonomous behavior
```

但 Voyager 更适合作为 Agent architecture，而不是环境层。

---

### Pokémon Agent / Pokémon RL Environments

现有 Pokémon 环境（如基于 PyBoy 的 PokemonRedExperiments 及同类 RAM-reading 环境）通常是：

```text
ROM
↓
Emulator
↓
Pixels / RAM
↓
Agent
```

`open-pokered` 的潜在区别在于：

```text
Semantic Source
↓
Native Runtime
↓
Ground Truth World Model
↓
Agent
```

因此不仅可以暴露当前状态，还可能暴露和分析游戏本身的状态转换语义。

---

# 18. Proposed Positioning

不建议把项目定位成：

> AI Plays Pokémon Red

更合适的是：

> **A Semantically Instrumented RPG Environment for Long-Horizon Agents**

或者：

> **Pokémon Red with First-Class Agent Semantics**

一个可能的论文标题（**OpenPoke** 作为该研究环境的简称，对应仓库 `open-pokered`）：

> **OpenPoke: A Semantically Instrumented RPG Environment for Studying Long-Horizon Agents**

副标题可以是：

> _Studying perception, abstraction, world modeling, hierarchical planning, and generalization in a deterministic RPG._

---

# 19. 最重要的学术贡献

如果最终能够做到：

```text
Same Game
Same Initial State
Same Objective
       │
       ├── Pixels + Buttons
       ├── Symbolic + Buttons
       ├── Symbolic + Navigation
       ├── World Model + Semantic Actions
       └── Oracle Planner
```

那么这个环境最大的价值不是证明：

> 某个模型可以通关 Pokémon Red。

而是建立一种方法，系统回答：

> **一个长期自主 Agent 的能力究竟来自哪里？**

并进一步把失败拆分成：

```text
Perception
Grounding
Navigation
World Modeling
Planning
Execution
```

---

# 20. One-Sentence Research Thesis

整个项目最终可以压缩成一句话：

> **Existing game benchmarks often tell us that an agent failed; OpenPoke should help us determine at which level of abstraction the agent failed, and why.**

这应该成为整个 Research Environment 最核心的设计原则。
