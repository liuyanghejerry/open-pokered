# 剧情奖励与早期演出修复

基线 `72ff719`，原作 `pret/pokered@fbcf7d0`。早期逐段20项证据和原作行号见根审计目录的 `early-story.md`。本分支覆盖 E01–E05、E07、E10–E20；E06/E08/E09 与全部地面球由 world 分支修复。E20 的普通存读桥接及 OLD/GOOD/SUPER_ROD 的原作 status_flags1 bits3/4/5桥接已由systems补齐并集成；原作状态字节恢复与普通SRAM round-trip已有生产逻辑测试。下面812项及两个独立binary的结果保留为早期分支历史，不作为最终冻结构建的统计。

21处NPC `giveItem` 分支现在只在成功时置领取旗标、移除兑换券或隐藏赠送者。包括自行车、小霞/夏伯TM、华蓝盗贼、常磐TM42、代币盒、屋顶饮料换TM、Metronome、两位图鉴助手、TM39、SUPER_ROD、Safari HM03、园长HM04。Oak包裹和5球原作未检查 carry，保留原行为；屋顶女孩先喝掉饮料、园长先收金牙再GiveItem也保留原作顺序。

本阶段扩展确认的NPC失败分支（表中当前行号均为72ff719）：

| 地图/奖励 | 原作依据 | 当前未检查调用 | 后果 |
|---|---|---|---|
| CeladonDiner COIN_CASE | scripts/CeladonDiner.asm:36–38 | scene:51–52 | 满包后永久置已领 |
| CeladonMartRoof TM13/48/49 | scripts/CeladonMartRoof.asm:93–125 | scene:45–46,63–64,81–82 | 原作也喝掉饮料，但本作还错误置领取旗标 |
| CinnabarGym TM38 | scripts/CinnabarGym.asm:152–153 | scene:68–69,93–94 | 满包后永久置已领 |
| CinnabarLabMetronomeRoom TM35 | scripts/CinnabarLabMetronomeRoom.asm:19–23 | scene:18–19 | 满包后永久置已领 |
| Route11Gate2F ITEMFINDER | scripts/Route11Gate2F.asm:34–38；engine/events/oaks_aide.asm:24–34 | scene:71–72 | 满包后永久置已领 |
| Route12Gate2F TM39 | scripts/Route12Gate2F.asm:17–21 | scene:21–22 | 满包后永久置已领 |
| Route12SuperRodHouse SUPER_ROD | scripts/Route12SuperRodHouse.asm:20–23 | scene:12,23 | 满包显示虚假成功；hasItem代理允许PC存竿后重复领 |
| Route15Gate2F EXP_ALL | scripts/Route15Gate2F.asm:23–27；engine/events/oaks_aide.asm:24–34 | scene:32–33 | 满包后永久置已领 |
| SafariZoneSecretHouse HM03 | scripts/SafariZoneSecretHouse.asm:15–19 | scene:18–22 | 错误用hasItem模拟满包，真实满包反而永久置已领 |
| WardensHouse HM04 | scripts/WardensHouse.asm:43–47 | scene:26–27,35–36 | 收牙后满包仍置领取旗标，不能补领 |

早期分支真实边界验证（历史记录）：

- `cargo test --offline --locked -p pokered-core --lib overworld::`：812通过，0失败。新增12种NPC赠礼失败→重试、盗贼失败不隐藏、满队鲤鱼王只按给宠结果扣钱、垃圾桶原作mask/DEC下溢及恢复第1锁分支、Daisy立即重入、音乐等待真实完成等测试。
- `cargo build --offline --locked --bin pokered-app --features debug-server` 成功。保存独立binary，使用 seed 42驱动真实按键/交互。
- `story-runtime.py` 的满20类自行车/小霞/盗贼、满队空箱鲤鱼王、Daisy即时重入5个fixture通过断言。鲤鱼王使用会追踪走动NPC的 `interact_with npc:3`，避免固定坐标未真正对话。SRAM导出 `magikarp-box-after.json` 验证当前箱有Lv5 Magikarp，金钱2500；队伍仍6只。
- `story-captures.py` 在同seed、同输入、同相对帧生成5组截图。人工检查确认对应差异，PNG保存在 `docs/screenshots/fidelity-story/`：daisy-reentry、bill-walk-around-player、bill-exits-machine、oak-player-exclamation、cerulean-rival-right，各有 `-before.png` / `-after.png` 与JSON命令记录。

截图基线binary固定为 `/workspace/onboarding/pokered-audit-base-app`，修复binary固定为 `/workspace/onboarding/pokered-story-fixed-app`；两者都不依赖随后其他分支覆盖公共target。

最终源码状态与验证边界：ROM侧仍为原作汇编静态对照，本文的定点结果没有从NEW GAME连续跑完全流程。E20普通Save/Continue现在通过 `OverworldScreen::write_system_save_state` / `restore_loaded_save_flags` / `restore_system_save_state` 读写垃圾桶索引、Safari计数和三种钓竿位，Native App与TUI调用相同桥接；`fidelity_systems.rs::safari_and_original_status_aliases_resume_allowances` 已覆盖原作status0x38、普通SRAM导出/导入和剩余步数/球数，systems报告另记录实际START→SAVE→普通CONTINUE入口。不能把这些桥接与单fixture验证称为全部地图存读穷举。

旧版“headless无音频设备时等待立即完成”的说明不正确。`--no-audio` 仍使用PCM-only output并按游戏帧推进真实sequencer；当前Native App/TUI的waitMusic读取音乐channel0实际播放状态。Captain原作等待PKMNHEALED第一声道结束：真实曲目probe在136帧结束时，旧global music_playing latch仍true。生产App回归用真实healed字节流证明等待不会立即完成，声道结束后才恢复脚本，见 [captain-wait-repair.md](captain-wait-repair.md) 与 `final-captain-tests.log`。最终冻结eebde2f的全workspace/跨平台/连续流程结果以统一最终验证记录为准，本文不提前声明其完成。
