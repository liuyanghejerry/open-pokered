# 入图脚本：避免漏跑与重复执行

Continue 后，地图加载器已执行完首轮 `@load`，自动 OnEnter 又排入同一脚本，导致红莲道馆六门效果重复并吞掉紧随其后的输入。正常 New Game 则没有调用这段初始化，卧室初始朝向错误。

地图加载器现在统一直接调用 `@load`，不再额外注册自动 OnEnter。New Game 在名称、存档、队伍及语言上下文就绪后，通过同一入口执行一次；正常 warp、连接及重入各执行一次。`load_map_script_ex(false)` 只重建函数和触发器，不把脚本延迟到下一次 OnEnter；显式战后重跑仍保留。

旧快照只删除当前地图中 type、自动 ID 和 handler 都精确匹配的历史自动绑定，原 TriggerManager 的触发器顺序、已触发状态、前一地图/位置及其他 OneShot/OnceOnInteract 状态均保留。native 与 Boa 共用入口；另修正了该加载器在既存 Boa feature 下的非穷尽匹配编译问题。

本次对比实际主线 `762fc5a32602d7dcee4da23a687ad42a7751384d` 与运行候选 `60b1a191972f73e633d994200716a8c798658ef2`，使用各自实际构建的 CLI 和内嵌地图，无地图覆盖。后续发布提交只增加本文和四张截图，代码、测试与运行候选逐字相同。

- 24 次定向测试执行通过（16 个不同测试名）：9 native、8 Boa、2 地图绑定、3 App 菜单/Continue 转换，以及 2 项主线红莲测验集成回归。候选 debug CLI 实际重新构建。
- 正常冷启动 New Game 使用同一份 1445 帧输入，正常经过 Oak 开场及默认命名，到达卧室 `(3,6)`：主线朝下，修复后朝上。双方独立 SAVE 路径及 sidecar 均不存在。
- Continue 使用同一物理 SRAM 和 525 帧输入：首轮六门结束后固定 neutral2、Up2、neutral12，修复后在红莲道馆 `(15,8)` 正常转向，主线重复效果吞掉输入。双方 SRAM 字节未变。
- 两组共 4 个 Game，26 张原尺寸同帧 PNG 逐对检查；源文件、engine、GFX、地图前后未变，全部进程组退出。未使用运行时 setter、warp、跳过对话或快照恢复。

正常 New Game，第 1445 帧：

![前](screenshots/onload-once-newgame-before.png)
![后](screenshots/onload-once-newgame-after.png)

Continue，第 525 帧：

![前](screenshots/onload-once-continue-before.png)
![后](screenshots/onload-once-continue-after.png)

Continue 的公开朝向状态在锚点后第 4 帧已改变，但早期 PNG 仍可能保留展示缓存；这里展示第 525 帧，不把状态变化时刻当成首个可见帧，也不声称原作逐硬件帧一致。

广泛回归仍有明确边界：历史 `fb6c886` 候选的全新 m01–m49 原 driver 运行只完成 m01–m11；m12 经原有两次黑屏恢复后第三次战败，最终以离开月见山的导航错误结束。原首败与全部记录保留，未重跑 fresh49。随后从同一战败后满血 Route4 持久状态，分别以历史 `077034d` / `fb6c886`、普通 Continue、seed42 和原 m12 策略做存档加速诊断，两侧均抵达华蓝市；两侧自适应路径与按键不同，不能据此排除 OnLoad 的因果影响，也不构成 fresh49 通过。

卧室脚本首次执行后永久变为 NOOP 的原作存档语义是另一项独立修复，本次不包含。
