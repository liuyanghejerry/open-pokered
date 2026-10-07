# 原作差异 18–27：复现和回归证据

基线：master `4ba17701dfa5561e3e5a73d8b169fd6f95e339a8`。
原作核对：pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`，对应地图脚本、文本、`HandlePlayerBlackOut`、`BillsHousePC` 和 `_DisplayPokedex`。

## 截图

每组来自相同初始存档、相同输入和相同采样帧；`states-before.json` / `states-after.json` 保存完整观察。初始夹具可以设定背包、徽章和剧情进度，所有操作随后走实际游戏输入；所有玩家落点经过地图可行走检查。

| 场景 | 帧 | 检查点 |
| --- | ---: | --- |
| bike-menu | 1200 | 完整 ¥1000000 价格和 CANCEL |
| badge-menu | 1200 | 仅灰色、橙色徽章及 CANCEL |
| safari-admission | 1200 | 右移一格，显示金钱框 |
| aide-count | 1600 | 拥有 80 种时报告 80，非固定 30 |
| rod-full | 1600 | 满包直接拒绝，不先承诺赠送 |
| bill-pc | 1600 | 普通寄存电脑改为伊布进化系图鉴菜单 |
| vermilion-return | 1400 | 船离港后第一次返回，静默上移两格 |
| bill-return | 1800 | 离开再返回后换成第三个正辉 |
| rival-loss | 1901 | 真实战败后先显示劲敌台词 |
| fly-first | 2200 | 首次赠送后结束，说明保留到再次对话 |

`capture.py` 还验证：四种图鉴都能打开、返回时保留菜单位置，浏览只增加“见过”且保存后保持；正辉换位和金珠 NPC 隐藏标记持久化；败给劲敌不设置战胜标记。

## 复现

构建应用的测试可执行文件：

```sh
cargo test -p pokered-app --lib --features debug-server --no-run
```

使用 Cargo 输出的 `Executable unittests src/lib.rs (...)` 路径作为 `APP_TEST_BINARY`。截图运行：

```sh
python3 docs/screenshots/fidelity-18-27/capture.py --binary APP_TEST_BINARY --label after --output docs/screenshots/fidelity-18-27
python3 scripts/fidelity_stdio.py APP_TEST_BINARY --until m49 --artifacts target/fidelity-playthrough
```

前图使用独立的 master 检出。仅加入相同 `fidelity_stdio.rs` 测试模块和对应 `#[cfg(all(test, feature = "debug-server"))]` 声明，游戏代码保持 master；保存基线测试可执行文件后，再构建修复分支，分别运行相同截图脚本。两套二进制校验值见 `verification.json`。

stdio 驱动是测试专用适配器，直接调用生产 `PokemonGame` 的调试命令处理器、逐帧输入和截图渲染。无需 TCP；未增加发行版功能。完整主线从真实 NEW GAME 开始，不使用 warp、赠送宝可梦或进度种子。

## 验证

- 核心 2683、数据 262、剧情分析 53 项单测通过。
- 应用 160 项单测通过（串行运行；2 项原有忽略项，交互式驱动不参与单测）。
- 数据层 `script-boa` 273 项通过，包含新菜单及指定奖励音效的实际 Boa 挂起/恢复测试。
- 新鲜 m01–m49 全线通过：冠军、名人堂、片尾、结束存档和独立进程 CONTINUE。见 `playthrough.log` / `m49.json`。
- 10 组截图实际帧号相同，已检查布局与完整文字。

额外检查发现：完整核心 `script-boa` 测试在 master 已因 `load_map_script_ex` 对 `OverworldScriptEngine::Native` 的非穷尽匹配而无法编译。基线检出复现同一 E0004；本次没有修改该既存问题。常用原生解释器的核心测试和数据层 Boa API 测试均已通过。
