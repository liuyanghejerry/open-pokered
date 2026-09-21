# Open-Pokered 模型 Benchmark

这个 benchmark 比较模型在同一个游戏控制器下的判断质量与自主剧情推进能力。首版将 Jev/Laya 实验中的计时、观察指标和证据留档封装成配置驱动的工具；增加模型不需要修改游戏或复制评测脚本。

入口：`python3 scripts/openpokered/benchmark.py`。配置：[manifest.example.json](manifest.example.json)。历史结果：[首次 Jev/Laya 比较](../../docs/laya-jev-evaluation-results/interpretation.md)、[Laya 排查](../../docs/laya-jev-evaluation-results/laya-diagnosis.md)。历史成绩保留在原目录，不自动当作新版 benchmark 成绩。

实现验证：[20 秒 smoke 报告与原始日志](evidence/smoke-20260921/report/README.md)。这里使用 Laya 和协议示例测试桩验证工具链，不构成模型能力排名。

## 两个评测轨道

| 轨道 | 测什么 | 固定条件与产物 |
|---|---|---|
| `decisions` | 策略/动作判断正确率、回答覆盖率、选项顺序一致性、概率评分、调用耗时、Token | 相同 state/questions；每题按原顺序及反序各问一次；完整请求、回答与评分 |
| `story` | 从新游戏自主探索：剧情节点时间与进度、徽章、图鉴、支线、练度、失败、金钱 | 默认 1,200 秒有效时限；同引擎、地图、技能、控制器、种子和剧情目标；游戏观察、操作审计、结束截图 |

初始固定场景集有 **16 个独立 development 场景**，包含 8 个已公开的 Laya 排查样本，以及对话、战斗和策略前置条件题。它用于开发检查，不是隐藏测试集。反转选项和重复运行不增加独立场景数量。可以增加独立、人工审阅标签的案例文件；数据哈希变化后单独报告，不能与旧数据集混算。

首版 `native-controller-v1` 保留当前控制器：策略层选择子目标，动作层选择操作，通用技能执行导航、交互和战斗。模型不直接决定每一个 A/B 按键。候选来自脚本条件和现场状态，不使用已通关路线，也不通过写标记、传送、赠送道具等方式推进评测。

模型拒选可能让现有控制器较早结束；已知的“拒选计入执行失败”行为也保留在这个基线中。短输入改造、拒选恢复、改变执行技能或给某个模型额外提示，都属于另一套控制器/输入配置，应独立评测。当前工具没有自动修复 Laya 的剧情策略。

## 开始使用

在仓库根目录执行。通用工具与命令适配器仅依赖 Python **3.11+** 标准库；Laya 使用 `~/develop/laya-mlx/howto.md` 的 Python 环境。绘图需要 `matplotlib`，没有时仍会生成 Markdown、JSON 和 CSV。

```bash
# 检查配置，不加载模型、不发请求、不启动游戏。
python3 scripts/openpokered/benchmark.py validate benchmarks/autonomy/manifest.example.json

# 构建带评测只读遥测的游戏；先按仓库说明准备 gfx/。
cargo build --release --bin pokered-app --features debug-server

# 预览冻结条件、哈希和串行运行计划，不调用模型。
python3 scripts/openpokered/benchmark.py plan benchmarks/autonomy/manifest.example.json \
  --binary target/release/pokered-app

# 短预检：标记为 smoke、只用首个种子/一次重复，剧情上限 20 秒。
~/develop/laya-mlx/.venv/bin/python scripts/openpokered/benchmark.py run \
  benchmarks/autonomy/manifest.example.json --binary target/release/pokered-app \
  --smoke-seconds 20 --output .artifacts/benchmark-smoke-001 --env-file .env

# 正式执行配置中的全部模型、种子、重复和轨道。
~/develop/laya-mlx/.venv/bin/python scripts/openpokered/benchmark.py run \
  benchmarks/autonomy/manifest.example.json --binary target/release/pokered-app \
  --output .artifacts/benchmark-001 --env-file .env
```

示例配置包含 Jev 与 Laya、3 个种子、各 1 次重复；共 6 次剧情运行和 6 次决策集运行。剧情预算合计最多 120 分钟，另加初始化、RTT 减除及决策集时间。`run` 会实际调用配置中的模型；可复制配置并删去暂不运行的模型或轨道。只运行 `decisions` 时不需要游戏二进制。

输出目录必须不存在，以防覆盖原始结果。单个模型提前停止或报错会被记录，后续任务仍按计划执行；不会偷偷重试整个失败剧本。重复运行须事先用 `repeats` 登记。一个运行结束前不会启动下一个，模型顺序按种子/重复块轮换；样本很少时顺序影响仍需在分析中考虑。

`seeds` 控制游戏随机数和固定题呈现次序；远端模型自身的采样随机性不由这个字段控制。其他模型的温度、采样种子等应在适配器中固定并写入 metadata。

## 更换模型

每个模型条目有稳定的 `id`（报告中的名字）、`provider` 和请求用的 `model`。相同 id 不可对应不同配置。

### TypeSafe

```json
{
  "id": "another-jev-version",
  "provider": "typesafe",
  "model": "EXACT_MODEL_VERSION",
  "timeout_s": 10,
  "max_retries": 1,
  "api_key_env": "TYPESAFE_API_KEY",
  "base_url_env": "TYPESAFE_BASE_URL"
}
```

凭据只从环境变量或 `--env-file` 读取；配置文件不接受 API key 字段。尽量指定固定模型版本，实际返回的模型名另行记录。该接口沿用 [TypeSafe System One HTTP 协议](https://docs.typesafe.ai/api)。

### Laya MLX

```json
{
  "id": "laya-other-checkpoint",
  "provider": "laya",
  "model": "HUGGING_FACE_REPOSITORY",
  "revision": "PINNED_REVISION",
  "dtype": "float16",
  "compile": true,
  "cache_prompts": true,
  "pad_to_multiple": 16
}
```

必须提供 revision。初始化时加载模型，在计时前做一次共同的预热请求。使用原生编码预算；请求日志保留实际编码文本、指令/状态/选项截断量。不要仅扩大总窗口后把它当作相同模型设置。

### 其他本地模型或 API：JSONL 适配器

```json
{
  "id": "my-new-model-v1",
  "provider": "command",
  "model": "MY_PINNED_MODEL_VERSION",
  "command": ["/absolute/path/to/python", "/absolute/path/to/my_adapter.py"],
  "timeout_s": 30
}
```

这是持久化子进程：加载一次模型，逐行接收请求，再返回统一的判断结果。任何语言、推断框架或服务 SDK 都可在进程内使用。`command` 是 argv 数组，直接执行，不经过 shell。相对文件参数按配置文件所在目录解析；可执行文件名也可以从 PATH 查找。命令及文件哈希会登记到运行计划，因此请将 token 放到环境变量，而非 argv。

可直接运行的协议示例：[adapters/example.py](adapters/example.py)。替换其 `infer()` 并在握手前加载模型即可。该示例只选第一个选项，**是测试用桩，不是智能模型或可比能力基线**；它故意不伪造概率或 Token 消耗。

进程启动后，先向 stdout 写一行：

```json
{"protocol":"open-pokered-judge-v1","ready":true,"metadata":{"implementation":"my-adapter-v1","checkpoint":"pinned-version"}}
```

评测器发送：

```json
{"protocol":"open-pokered-judge-v1","id":1,"model":"my-model-v1","state":{"hp":1},"questions":{"action":{"type":"choice","instructions":"Which operation advances the stated goal?","criteria":{"heal":"Restore HP","none":"No useful operation"}}}}
```

适配器返回：

```json
{"id":1,"model":"actual-model-version","answers":{"action":{"type":"choice","choice":"heal","probabilities":{"heal":0.8,"none":0.2},"confidence":0.3}},"usage":{"input_tokens":120,"output_tokens":18}}
```

约定：

- stdout 只用于每行一个 JSON 对象，调试信息写 stderr；请求与回答 id 必须匹配。进程会收到预热请求，不能把问题 id 当作任务指令。
- `answers` 必须覆盖全部问题，类型一致，choice 标签必须在候选中。概率必须有限、非负且总和为 1（允许浮点/四位小数舍入误差）。也支持类型正确的 `noul` 和 `score` 回答，首版游戏及固定集使用 choice。
- 只返回 choice 标签也可运行，此时省略 `probabilities` 和 `confidence`；不会补造 one-hot 概率。模型自身预测的概率也不应当作经过验证的校准值。
- 未知 Token 用量返回 `null` 或省略，不能写 0。已知合计单独展示，并同时报告用量缺失调用数。
- 加载失败、服务错误等可返回 `{"id":1,"error":"..."}`。每次请求、握手和整轮评测均有超时边界；不允许不完整的一行无限等待。
- 不把标准答案、标签解释、测试集文件或游戏调试连接提供给模型。适配器负责真实模型推断，不能查阅案例标签、硬编码路线、操作游戏或添加未登记的解题规则。外部适配器是受信任插件，不是安全沙箱。
- 将 SDK/检查点版本、量化参数、解码参数等写到握手 metadata。模型文件与第三方依赖仍需使用可复现的安装/版本管理；工具不能自动冻结远端服务内部实现。

对于文本生成模型，适配器将 state/questions 作为提示词并解析模型 JSON 输出；固定提示词应写在适配器代码中，随适配器文件哈希记录。让模型返回选择标签即可，不要求暴露内部思维过程。

## 公平性与时间定义

`story` 计时从模型加载、共同预热及游戏进程初始化之后开始，包含实际 NEW GAME 流程、控制器计算、推断等待、技能执行和采样开销。游戏使用 driven-only 模式，只有输入命令推进模拟帧。

有效时间 = 原始墙钟 − 获准减除的 RTT。仅 TypeSafe 适配器读取实际 TCP socket 的 RTT，每次 HTTP 尝试最多减一个 RTT，整轮默认最多减 300 秒；推断、排队和完整请求时间都不能整体减掉。当前 socket 测量实现适用于 macOS；测不到记 0 减除。命令适配器没有可验证的 socket RTT，当前不给减除。报告同时展示原始墙钟、有效时间和 RTT，跨平台或 RTT 政策不同应分开分析。

独立监督进程在预算边界要求退出，宽限只用于收尾；预算之后的状态不再计分。初始化最长等待 180 秒。决策集另有默认 300 秒总预算以及逐请求超时，不给 RTT 减除。加载和一次预热仍单列。

比较指纹包含 benchmark/控制器版本、源文件实际内容哈希、地图/数据哈希、游戏二进制、Python/主机信息、时间规则与案例集哈希。它不包含模型列表，因此可在相同条件下稍后补测新模型。模型配置、适配器文件/可执行文件与实际模型标识另外留档。运行前已有未提交修改也会进入实际文件哈希，不能只凭 commit 名称假定条件相同。

原生配置固定候选和应用层输入，不保证不同模型实际读到同样多的 Token。Laya 编码截断可审计，未提供编码明细的后端视为“不可见”，不能声称没有截断。未来加入统一紧凑输入应登记为新的 profile，并同时应用于参评模型。

## 指标如何解释

| 指标 | 定义 / 限制 |
|---|---|
| 剧情进度与耗时 | 11 个主线目标、8 个徽章；记录每个目标首次预算内观测的原始/有效秒。未达到为缺失值，不以退出时间代替 |
| 图鉴 | 见过/拥有种类；进化带来的拥有记录不等同于多只队员 |
| 支线 | 预定义 11 项可选成就；去过某地图不代表完成支线；目前主任务仍是通关 |
| 队伍练度 | 最高/平均/总等级和队伍经验；保留完整最终队伍 |
| 失败 | 战败、采样看到的全队 HP=0、受阻操作、拒选、协议错误分列，不能相加当作独立失败总数 |
| 金钱 | 初始/最终/峰值、余额正/负变化累计；采样可能漏掉中间收支，因此不是精确毛收入 |
| 内部循环 | 连续 180 秒没有新标记/地图/图鉴/经验/等级，记录疑似停滞；报告标记之外仍由总预算强制截止 |
| Token | 实际已知输入/输出合计及用量缺失数；本地分类器输出为 0 不代表没有计算；不同 tokenizer 不直接等价 |
| 固定题正确率 | 有效答案上的正确比例，必须与覆盖率同时看；服务中断不能当作剩余题全错或全对 |
| 顺序一致率 | 同一个案例原顺序/反序的选择标签是否相同；一致并不代表正确 |
| Brier / NLL | 仅在模型返回完整概率分布时计算；多可接受答案使用可接受概率总质量算 NLL，Brier 仅用于单标签题 |

汇总按模型/轨道给出运行数、结束类别和指标的均值、中位数、范围、标准差。小样本不声称显著性；公开调试案例不声称泛化准确率。没有随意加权的“综合能力分”，也不将 API 额度耗尽、协议错误和模型主动拒选混成同一个结论。

TypeSafe SDK 的 usage 对应最终响应；发生 HTTP 重试时，失败尝试可能还有未报告用量。`transport_samples` 记录各次尝试，不能仅凭 `usage_missing_calls=0` 推断所有 HTTP 尝试的计费都已完整取得。这里不估算未返回的 Token 或价格。

## 结果、追加比较与验证

```text
<output>/
  plan.json                  冻结配置、运行顺序、条件、案例和文件哈希
  specs/                     每个模型及任务的机器可读配置
  campaign.json              当前/完成状态
  <track>-<model>-s<seed>-r<repeat>/
    summary.json             单次结果或明确标注的不完整 checkpoint
    requests.jsonl           输入、回答、时延、已知 usage、可用编码审计
    observations.jsonl       剧情轨道的预算内游戏观测
    commands.jsonl           剧情轨道的实际操作审计
    trace.jsonl              策略、动作、结果和退出原因
  report/
    README.md                人类可读比较
    results.json             每次运行与汇总
    metrics.csv              完整指标
    verification.json        条件核验与原始文件哈希
    progress.png / .svg      安装 matplotlib 后生成的剧情曲线
```

原始记录保留退出时的真实进度，曲线不会补画到 20 分钟。模型报错时仍保存可用 checkpoint；未取得游戏观测用缺失值，而不是伪造零分。watchdog 强制退出时，未结束请求的 Token 可能未知，报告保留这个限制。

同条件补测第三个模型后，可合并报告：

```bash
python3 scripts/openpokered/benchmark.py report \
  .artifacts/benchmark-001 .artifacts/benchmark-new-model \
  --output .artifacts/benchmark-combined-report
```

条件指纹不一致、同名模型配置不同、重复计入相同模型/种子/重复编号、模型之间种子覆盖不齐、成本日志不一致或越界计分都会报错。模型新增运行的 seed/repeat 集必须与已有比较一致。报告目录也要求新建，避免覆盖此前的分析结果。

维护验证：

```bash
PYTHONPATH=scripts python3 -m unittest \
  scripts.test_benchmark scripts.test_model_evaluation \
  scripts.test_openpokered_story scripts.test_openpokered_autonomous
```

本次实现通过 155 项离线契约/控制器测试，以及 Laya 和独立 JSONL 适配器的实机短预检。新增加的 GitHub Actions 在 Python 3.11/3.13 上运行无需凭据、权重和游戏二进制的契约测试；本地验证已执行，远端 CI 尚未运行。

后续可新增未参与调试的真实状态集、更多种子及策略/动作层混合模型实验。每次修改标签、压缩规则、拒选恢复或操作能力，都应以新的条件指纹保留成绩，不能覆盖历史运行。
