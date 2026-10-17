# 卧室脚本与存档回归

原作 `RedsHouse2FDefaultScript` 只在脚本字节为0时朝上，随后把 `wRedsHouse2FCurScript` 写成1（NOOP）。该字节位于原 SRAM 的 `wGameProgressFlags[28]`；Continue 默认朝下，已经执行过的卧室脚本不应再次改变朝向。

截图使用同一份32KiB实体SAV（脚本字节1、卧室(3,6)）、相同seed42和679帧普通输入，均在第615帧通过正常Continue进入场景。前图来自 master `3ada1c5272bce1a402d020380fec40bf092e6af5`，后图来自修复源码 `93dd7057b9538f09a66075f784e706551d696e86`。第679帧前者朝上，后者保持朝下；位置、金钱和存档文件一致。运行期间没有修改游戏状态、传送或跳过对话。

`bedroom-script1.sav` 是构造的测试存档，`fixed-inputs.json` 每项对应一帧（null表示无按键）。可在相应版本构建 `pokered-app` 后，将SAV复制到临时目录，用 `run --headless --speed 0 --seed 42 --debug-port 19511 --lang en --no-audio --save <SAV路径>` 启动。通过调试端口逐项发送 `press_timeline`（`start_at_frame` 从0开始、`advance:true`），第679帧使用 `capture_frame` 取得对照图。构建需启用 `debug-server`。

验证覆盖原生/Boa脚本、原SRAM读写、旧companion别名优先级、snapshot、图谱及App默认/调试入口。新增App回归保留同一个已完成脚本的旧场景，经真实OakSpeech清空SaveData后调用host/GBA共用的新游戏脚本入口，确保重新初始化。具体源码版本与计数见 `validation.json`。

这两张原生截图验证Continue场景；新游戏复用入口由组件测试验证，并非一次真实GBA软重置运行。
