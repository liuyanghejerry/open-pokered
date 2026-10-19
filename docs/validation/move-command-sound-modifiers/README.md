# 招式命令音效参数

动画命令的 sound_move 决定音效表行。get_move_sound 现在返回完整 MoveSfx，不再只返回音效 ID；普通游戏、DirectBattle 和录制元数据均保留 pitch_mod / tempo_mod。聚宝功先使用拍击行，再使用聚宝功行，不能把整段动画统一成同一个声音。

Audio2 的战斗音效支持音高和速度修饰。音调通道采用 frequency + pitch 和 $80 + tempo；噪声 CHAN8 保留噪声参数及固定 $100 音符速度。音频引擎依赖 8a6e0dd 包含实际执行音高修饰的修复，测试检查音符和 APU，避免只验证保存了字段。

verification.json 保留 7 个原作例程控制，以及当前原生测试范围和限制。原作 ROM 不包含在仓库中；这些例程控制不等于整场战斗逐周期或声波对齐。

可运行验证：

```sh
cargo test -p pokered-audio --lib
cargo test -p pokered-app --features desktop,debug-server --lib actual_move_commands_keep
cargo test -p pokered-app --features desktop,debug-server --lib direct_battle_preserves
```
