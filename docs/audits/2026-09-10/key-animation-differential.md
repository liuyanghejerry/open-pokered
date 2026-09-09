# 关键动效原版 / 当前版帧差异审计

日期：2026-09-10

## 结论

| 动效 | 结果 | 结论 |
| --- | --- | --- |
| 战斗进场 | 部分通过 | 淡入、双方入场、野生宝可梦出现和出现文字均存在；两侧使用的触发路径不同，未把物种/训练家精灵差异判为动效差异。 |
| FLY 到达 | 通过 | 当前版保留了右上方飞鸟进入、拍翅和落地段，12 个坐标点 × 3 帧的节奏与原版一致。 |
| FLY 起飞 | **不通过** | 原版有“原地拍翅 → 向右下飞行 → 向左上飞行 → 白屏”的离场段；当前版从选择目的地后直接进入白色淡出，没有起飞飞鸟。 |
| SURF | 通过 | 两版都在确认文字后切换到冲浪运输状态，水面显示冲浪精灵，并能继续在水面移动。 |
| CUT | **不通过** | 原版有白屏/重载/`AnimCut` 的视觉时序；当前版直接替换树块、播放音效并打印文字，没有可见砍树动画阶段。 |
| 台阶跳跃 | 通过 | 两版均完成 16 段上抛—下落轨迹；角色位移、阴影和 `(10,4) → (10,6)` 落点一致。 |

优先级上，建议先补 FLY 起飞状态机和 CUT 的覆盖层/OAM 动画；这两项是录制中可以直接确认的视觉缺口。台阶跳跃、冲浪和 FLY 到达暂未发现需要修正的动效差异。

## 录制方法

- 原版使用 pret/pokered 固定提交 `fbcf7d0` 构建的官方 `pokeblue_debug.gbc`。使用 DEBUG 测试入口准备完整队伍和场景；动画例程与 Gen-1 原版一致，但战斗 HUD 会显示 Blue/DEBUG 路径。
- 当前版使用仓库现有 `target/debug/pokered-app`，提交为 `e54a435`。各场景使用可重复的字段快照，分辨率均为 160×144。
- 帧率按 60fps 导出 MP4；对比图中的帧号是各录制文件内部帧号，不是跨实现的绝对模拟器时钟。
- 场景坐标：FLY `PalletTown (5,6) → ViridianCity (23,26)`；CUT `VermilionCity (15,17)` 面向树；SURF `PalletTown (5,13)` 面向水；台阶 `Route1 (10,4) → (10,6)`。

构建备注：本次尝试重新构建 debug-server 时被本机 Cargo 缓存中的 dotzuki 模板包名 `{{project-name}}` 解析错误阻断，因此使用已存在且对应当前提交的 debug binary 完成录制；未修改该无关缓存。

## 帧证据

### FLY

起飞对比最清楚：原版在地图淡出前持续出现飞鸟，当前版未出现离场飞鸟，目的地选择界面随后直接进入白色淡出。

![FLY 起飞对比](../../screenshots/visual-key-animations/fly-departure-compare.png)

到达段两侧均能看到飞鸟从右上方进入并在玩家位置落地。

![FLY 到达对比](../../screenshots/visual-key-animations/fly-arrival-compare.png)

录制： [原版 MP4](../../screenshots/visual-key-animations/fly-reference.mp4) · [当前版 MP4](../../screenshots/visual-key-animations/fly-current.mp4)

### CUT

原版触发后先出现白屏，再回到地图并进入文字阶段；当前版从动作菜单退出后直接在地图上开始文字打印。

![CUT 触发对比](../../screenshots/visual-key-animations/cut-trigger-compare.png)

录制： [原版 MP4](../../screenshots/visual-key-animations/cut-reference.mp4) · [当前版 MP4](../../screenshots/visual-key-animations/cut-current.mp4)

### SURF

两侧的持续帧都显示玩家已经处于水面运输状态；文字内容中的训练家名不同是测试存档差异，不是动效差异。

![SURF 持续状态对比](../../screenshots/visual-key-animations/surf-compare.png)

录制： [原版 MP4](../../screenshots/visual-key-animations/surf-reference.mp4) · [当前版 MP4](../../screenshots/visual-key-animations/surf-current.mp4)

### 台阶跳跃

逐阶段帧对齐显示，当前版复用了原版的垂直弧线和移动方向，并绘制了跳跃中的阴影。

![台阶跳跃对比](../../screenshots/visual-key-animations/ledge-compare.png)

录制： [原版 MP4](../../screenshots/visual-key-animations/ledge-reference.mp4) · [当前版 MP4](../../screenshots/visual-key-animations/ledge-current.mp4)

### 战斗进场

两侧都经过过渡、精灵出现和 `Wild RHYDON appeared!` 阶段。原版录制走官方 FIGHT/DEBUG 入口；当前版使用 debug server 的 `start_wild_battle Rhydon level20`，所以玩家精灵、训练家名和前置菜单时序不作像素级结论。

![战斗进场对比](../../screenshots/visual-key-animations/battle-entry-compare.png)

录制： [原版 MP4](../../screenshots/visual-key-animations/battle-reference.mp4) · [当前版 MP4](../../screenshots/visual-key-animations/battle-current.mp4)

## 实现侧交叉检查

- 当前 FLY 入口 [`field_moves.rs`](../../../crates/pokered-core/src/overworld/field_moves.rs#L393-L411) 只设置 `pending_fly_arrival`、目的地和白色淡出；当前实现没有离场飞鸟状态。到达坐标表位于 [`presentation.rs`](../../../crates/pokered-core/src/overworld/presentation.rs#L176-L231)，与原版 `FlyAnimationEnterScreenCoords` 一致。
- 原版 `_LeaveMapAnim` 在 `player_animations.asm` 中包含原地拍翅、两段坐标列表和最终白屏；这与 FLY 起飞录制差异吻合。
- 当前 CUT [`field_moves.rs`](../../../crates/pokered-core/src/overworld/field_moves.rs#L121-L148) 直接 `set_block`、发 `SFX_CUT` 并进入文字，没有原版 `InitCutAnimOAM` / `AnimCut` 对应的渲染阶段。
- 当前台阶渲染 [`overworld.rs`](../../../crates/pokered-app/src/render/overworld.rs#L596-L640) 使用原版 16 项 `PlayerJumpingYScreenCoords` 偏移表；录制结果与此实现一致。
- 当前冲浪入口 [`field_moves.rs`](../../../crates/pokered-core/src/overworld/field_moves.rs#L221-L238) 设置 `TransportMode::Surfing` 并将玩家推进到水面；录制结果与该状态转换一致。

本次仅新增审计文档和录制证据，没有修改游戏实现代码。
