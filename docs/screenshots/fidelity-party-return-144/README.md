# 队伍退出恢复：输入门禁已接入，时钟/扫描线待补齐

原作恢复图形时忽略 B退出后的短按 DOWN/A；主分支立即返回 START，第16帧打开背包。本批将精灵重载周期和原作 Delay3、玩家/字体/文本框传输接入队伍退出恢复流程，期间不接收菜单输入。旧 JSON 缺少 `submenu_reload` 时仍表示原来的关窗恢复。其他菜单保留各自恢复路径。

前端 CPU 相位暂为0。此常青市场景实测第37帧重显、第40帧首次读取，与原作一致；不能据此声称其他地图同步。传入原作实测相位的周期模型可以重现五场景重显时间，但全局时钟还未接入。

## 同存档、输入与帧的主分支前后对比

主分支为 `31b1eda`，只在测试模块添加助手，生产前缀已逐字验证不变。两个版本均实际 Continue 同一32KiB原作 SRAM，常青市(20,30)，实际打开 START/队伍，再输入 B0..1、DOWN10..11、A16..17。无 warp、队伍或 NPC 修改。

第16帧，前（背包）/ 后（白色恢复）/ 原作：

![前，第16帧](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-party-return-144/probe-16-before.png)
![后，第16帧](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-party-return-144/probe-16-after.png)
![原作，第16帧](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-party-return-144/probe-16-original.png)

第40帧，前仍在背包，后与原作回到 POKéMON：

![前，第40帧](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-party-return-144/probe-40-before.png)
![后，第40帧](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-party-return-144/probe-40-after.png)
![原作，第40帧](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-party-return-144/probe-40-original.png)

原作与修复分支两个场景的白色帧1..36，解码 RGB共72帧相同；不同编码器的 PNG字节不作跨引擎比较。各场景重复两次；原生/主分支只在重复比较时排除两个独立初始化但未使用的 RNG元数据，原始 JSON全部保留，其他字段及 PNG字节须相同。原作独立长按 DOWN录制确认第40帧切到 ITEM；实际存档回归验证短按丢弃、长按生效、不重播 START音效。82帧缓存绘制和完整绘制相同，且命中帧复用。

核心单元测试2734项、应用单元测试201项通过，22项专项/录制测试保持忽略；录制助手另行实际执行。GBA发布构建和未修改基准的31项性能比较通过。原生与 GBA的10个对应源文件哈希一致。

## 剩余工作

首帧原作保留上方15/16扫描线，当前全白；全局 CPU/PPU相位、背景分段传输和 NPC重载/更新时机仍需验证。不声称重显后的完整 RGB相同，字体是排除项，地图人物等仍待审计。

冠军之路2F第二机关、脚本/跟随/连接移动时钟、Safari/对白、最新完整主线与独立 Continue及广泛覆盖仍未完成。PR保持草稿。
