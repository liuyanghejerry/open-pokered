# 地图与人物显示原点（90）

基线为 master `567dffd420d40d4cc22ce958850c0442ac2f49ec`。原作：pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`、零改动 retail Red，ROM SHA1 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`，PyBoy 2.8.1；显式 DMG RGB 255/170/85/0，160×144，一帧一图。

## 原作依据与修复

`home/reset_player_sprite.asm` 保存玩家精灵屏幕坐标 X=$40、Y=$3c，即 (64,60)。`engine/gfx/sprite_oam.asm PrepareOAMData` 加硬件 X=8、Y=16，LCD 显示时减回；这不是把玩家移到 (72,64) 的理由。背景地面格原点是 (64,64)，人物顶部比地面格高四像素。

原实现横向摄像机多偏一张 8px tile，玩家和普通 NPC 顶部也低四像素。修复背景窗口中心和人物投影，并将依附玩家的钓竿、气泡、CUT、飞翔、骑车等覆盖层统一到原作人物原点。FLY 的坐标表原本就是屏幕坐标，不能再次平移。治疗机器依附地图地面格而不是人物顶部，保留其 Y 偏移。

没有改变核心移动逻辑。现有测试中依赖旧错误位置的断言改用独立原作坐标；推石灰尘测试只确认可见性，没有把其尚未修复的原始 OAM 转换问题标为通过。

## 实际入口与量化

使用已归档的 [原作实际 SAVE](screenshots/fidelity-82-85/fixture.sav)，真正经过标题菜单 Continue，停在常青市中心 (13,4) 朝下。空走时钟 120 帧，然后 Left 按住 16 帧并释放，记录 t=-1..99。master 与修复检出使用字节相同的只读录制函数，基线没有生产修改；每个版本录制两遍，全部 101 张 PNG 和状态 JSON 的重复 SHA256 一致。原作额外逐帧保存 160 字节 OAM。完整图像和状态见截图目录的三个 raw.zip；哈希见 [manifest](screenshots/fidelity-90/manifest.json)。

| 静止首帧通道 | 区域 | master 不同像素 | 修复后不同像素 | 判定 |
| --- | --- | --- | --- | --- |
| 顶部静态地图 | (0,0)..(160,24)，3840 像素 | 766 | 0 | 此区域 PASS |
| 玩家精灵 | (64,60)..(80,76)，256 像素 | 176 | 0 | 此区域 PASS |
| 全画面 | 160×144 | 4620 | 306 | PARTIAL，NPC 姿态/时钟未对齐 |

修复前后 native 101 帧记录的地图、玩家位置/朝向、移动状态、walk_counter、队伍和输入完全相同。这不是全部游戏内存的一致性证明，记录没有覆盖 NPC 的所有内部状态。

## 治疗机器单帧检查

在同一受控状态渲染：中心 (3,3) 朝上，护士 (3,1)，六球可见，flash=false。master 和修复版使用同一测试夹具。新增独立像素 oracle 使用原作 `dbsprite` 的 LCD 坐标：显示屏 (44,20)，球 (40/48,27/32/37)，每个 OAM 项一张 8×8 tile。同时检查 `$e0` / `$c8` 闪烁调色板及偶数球 X 翻转。此夹具只证明布局和调色板；没有经过实际护士交互，不能判定治疗动画时间轴 PASS。修正了 visual-verify 技能中旧的宏参数顺序、坐标、尺寸和调色板说明。

## 仍需继续修复

整段行走为 **PARTIAL**。相同 16 帧 Left，原作最终 (12,4)，native 最终 (11,4)。原作 OverworldLoop 每次两次 DelayFrame，普通一步的八次推进占约 16 个硬件帧；复刻现在每帧推进，下一批需修复并核对转向、骑车、NPC 与脚本时序。灰尘原始 OAM 坐标转换、草丛优先级和其他覆盖层需要独立实际输入录制；不因本次原点修复宣告所有动画对齐。

字体、中文、拼音输入法、对话框布局按用户指定排除。

## 复现

构建 `cargo test -p pokered-app --features debug-server --lib --no-run`，对测试执行文件运行 `capture_actual_field_origin_raw_90 --ignored`，环境变量 `FIDELITY_FIELD_SRAM` 指向上述夹具，`FIDELITY_FIELD_CAPTURE` 指向输出目录。录制使用正常菜单和实际输入，不改写源夹具。单帧治疗图使用集成测试 `capture_healing_origin_90 --ignored`，`FIDELITY_HEAL_ORIGIN_CAPTURE` 指向目标 PNG。

验证：193 项 debug 应用测试通过；483 项未优化应用全目标测试、48 套全部通过。
