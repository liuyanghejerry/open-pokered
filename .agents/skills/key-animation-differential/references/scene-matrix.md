# 标准关键动效场景矩阵

这些场景用于复用录制路径和语义锚点。坐标是 Pokémon Red/Blue 的 2D 地图坐标；如果地图数据或当前实现改变，先重新验证坐标和面向方向。

| ID | 场景与固定起点 | 触发与停止条件 | 必测通道 | 原版重点检查 |
| --- | --- | --- | --- | --- |
| `battle-entry` | 官方 DEBUG 战斗入口，或固定野生战斗 | 触发战斗；停止在 `Wild ... appeared!` 或正式战斗菜单出现 | phase 首帧/时长、fade 曲线、双方 sprite bbox/可见性、输入锁定 | 淡入/闪白、双方精灵出现、入场滑动、出现文字；不同队伍时只比较进场层，不比较精灵差异 |
| `fly-departure-arrival` | `PalletTown (5,6)` → `ViridianCity (23,26)` | 在 Town Map 确认目的地；分别记录离场和到达，至玩家稳定落地 | 每段原始时长、飞鸟中心点/拍翅帧、背景/地图切换、fade 曲线 | 离场：原地拍翅、两段飞行坐标、白屏；到达：飞鸟从右上方进入并落地，拍翅节拍 |
| `surf-entry` | `PalletTown (5,13)`，面向前方水面（通常为水 tile `0x14`） | 使用 SURF；停止在上水文字完成并可移动 | 文字首帧、运输状态切换帧、玩家/冲浪 sprite bbox、推进轨迹 | 确认文字、玩家推进到水面、冲浪精灵/运输状态、后续水面移动 |
| `cut-tree` | `VermilionCity (15,17)`，面向有效 CUT 树（前方 tile `0x3d`） | 使用 CUT；停止在砍树文字完成、地图稳定 | whiteout/fade 时长、CUT OAM bbox/帧序列、树块变化帧、文字首帧 | 白屏/重载时序、树块替换、CUT OAM/覆盖层、音效与文字顺序 |
| `ledge-down` | `Route1 (10,4)`，面向下方台阶/ledge | 按住下方向直到 `(10,6)`；单独截取第一跳 | trigger→stable 时长、player Y、背景累计 Y、最大单帧背景位移、落点提交帧 | 角色弧线和背景滚动必须组合一致；已知 reference 窗口为 37 帧，背景分 16 次 × 2px 平滑滚动，不能在落地时一次跳 32px |
| `overworld-origin` | 同一原作实机 SAVE，`ViridianPokecenter (13,4)`，实际标题菜单 Continue；方向输入前朝下，先空走时钟 120 帧 | 左方向按住 16 帧，第 16 帧释放；录制 `t=-1..99` 至站稳 | 连续 PNG、玩家 OAM / 背景、坐标提交与 walk_counter、实际输入、两轮重现；几何和行走时序分开量化 | 玩家 sprite-state `(64,60)`；背景地面格 `(64,64)`；硬件 OAM `+8/+16` 在 LCD 抵消。普通 OverworldLoop 两次 DelayFrame，不能把八次推进误称八个硬件帧。 |
| `stats-pc-transition` | `docs/screenshots/fidelity-82-85/fixture.sav`，实际 Continue→面向上→PC→Bill PC→DEPOSIT / WITHDRAW→STATS | 各按键按住 2 帧，第三帧释放；分别连续记录进入、翻页、退出至稳定操作菜单 | palette/NR50、画面阶段、species/level/moves/数据来源、鸣叫起止、输入边界、两轮重现 | 源码 `StatusScreen` / `StatusScreen2` / `GetMaxPP`。不能仅改 loadedMoves 推算耗时：GetMaxPP 读取队伍/箱子实际招式。箱子需 CalcStats；此特定 PC 夹具返回完整清屏帧 1..6、菜单输入帧 6。 |

## 原版实现锚点

当需要解释“为什么应该有这一段”时，优先对照固定的原版 routine，而不是只凭截图猜测：

- FLY：`engine/overworld/player_animations.asm` 的 `EnterMapAnim`、`_LeaveMapAnim`、`FlyAnimationEnterScreenCoords`、`FlyAnimationScreenCoords1/2`、`DoFlyAnimation`。
- CUT：`engine/overworld/cut.asm` 的 `UsedCut`、`InitCutAnimOAM`、`AnimCut` 调用和树块替换流程。
- 台阶：`engine/overworld/player_animations.asm` 的 `_HandleMidJump` 与 `PlayerJumpingYScreenCoords`。
- SURF：`engine/overworld/surf.asm` / `engine/items/item_effects.asm` 的上水流程和运输状态。
- 战斗进场：`engine/battle/` 中的战斗初始化、精灵入场和出现文字流程。

报告中应注明实际 reference ROM 的 source commit/path。上述路径假定 reference source clone 位于仓库外，例如 `/tmp/pokered-reference`；不存在时不要伪造源码链接。

## 录制元数据最小字段

```json
{
  "scenario": "ledge-down",
  "reference": {"rom": "...", "source_commit": "...", "variant": "Red or DEBUG"},
  "current": {"commit": "...", "binary": "..."},
  "start": {"map": "Route1", "x": 10, "y": 4, "facing": "Down"},
  "trigger": {"input": "down", "frame": 0},
  "end_expected": {"map": "Route1", "x": 10, "y": 6},
  "capture_fps": 60,
  "frame_mapping": "one PNG per emulated frame",
  "phases": [{"name": "jump", "first": 0, "last": 36}],
  "measurements": {
    "actor_y": [],
    "background_y": [],
    "movement_state": []
  }
}
```
