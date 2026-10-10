> 历史证据：此目录原作参考和大分支录制保持原样；当前独立修复的实际master前/后、源码哈希和测试结果见相邻 `fidelity-safari-split-195`。历史native/GBA结果不作为独立PR最新提交的验证。

# Safari 结束后的门房返程

原作 `pret/pokered` fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c 的 `SafariZoneGameOver` 选择门房第四个 warp `(4,0)`。`SafariZoneGateLeavingSafariScript` 先让玩家面朝下告别，再自动向下走三格，停在 `(4,3)`。复刻原先从 `(3,1)` 开始，停在 `(3,4)`；此次修正入口和告别前朝向。

原作、实际 master `31b1eda`、修复分支各录制超时/球耗尽两种场景，每种重复两次，共 8,424 张 PNG、每次 702 个连续硬件帧（-1 至 700）。前/后截图均取相同 SRAM、输入、硬件帧 700；原作此帧也已稳定，另列参考。修复分支通过真实 CONTINUE 载入原作 SAVE 生成的 SRAM。

准备和限制：使用之前一步额度的受控原作 fixture，不证明正常付费进入过程。球耗尽场景在原作直接把球数设为 0；原生连续调用正常消耗球的接口，模拟战斗结束后的端点，**不证明实际最后一球的战斗流程**。主分支在该端点仍留在狩猎区，对应上一提交已修复的空球结束问题；不能把这全部归因于本次坐标修正。

原作的地图编号先变化、坐标稍后才提交，因此未将编号变化时残留的 `(14,23)` 判为入口。超时原作第 321 帧实际提交 `(4,0)`、第 331 帧面朝下告别，最后停在 `(4,3)`；空球分别为第 171、181 帧。原生相应超时入口/转向为第 226/251 帧、空球为第 136/161 帧，最终同为 `(4,3)`。原始逐帧数据是时刻的依据。

**仍有节奏差异：** 原作三次坐标完成间隔为 17 个硬件帧，原生为 8。公告、对话和载入时间也未对齐。本次只证明入口、告别朝向、三格路径及最终位置修复，不声明完整时序、像素或音频一致。

核心 2,735 项、应用 201 项单元测试通过（24 项录制 helper 默认忽略）；Safari 9 项回归通过。额外执行本场景四次原生及四次 master 捕获。GBA 发布构建和原阈值下 31 项性能比较通过，源码/ELF 哈希见 provenance。两次原作状态及 PNG 完全一致；原生/master JSON 仅排除两个未参与场景的初始随机种子，PNG 字节完全一致，原始 JSON 保留。

归档使用 SHA-256 对象去重，MANIFEST.json 记录 8,479 路径到 448 对象的映射，全部重新验哈希。包含逐帧状态/画面、事件、SRAM、工具、来源和测试日志；不包含 ROM 或 PyBoy 状态文件。原作断点只位于可执行例程，PrintText 参数用于识别文本，未在数据标签设断点。

## 超时返程

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-safari-return-148/timeout-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-safari-return-148/timeout-after.png)

![原作参考](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-safari-return-148/timeout-original.png)


## 球耗尽返程（受控端点）

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-safari-return-148/empty-balls-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-safari-return-148/empty-balls-after.png)

![原作参考](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-safari-return-148/empty-balls-original.png)

