# TM35 / HM03 领取确认

master `e084fc3f` 在领取文字前设置完成标记，并在领取音效后自动关闭文字。候选将原作的明确段落、内部音效等待、调用者设置标记及外层按键确认依次连接。现有地面拾取的自动关闭行为不变。

两张地图均用准备存档普通冷启动并 CONTINUE，随后只用正常按键与 NPC 交谈。master、候选和原作各录制两次；每组观测帧与截图哈希重复一致。候选确认后再次交谈不重复发奖，并通过显式 debug SAVE 后的独立进程 CONTINUE 核对物品和标记。原作采用单独准备的普通 SRAM；准备阶段的临时入口设置不计作正常路线，正式录制不改运行时状态。

截图在相同游戏帧对齐：TM35 为 998，HM03 为 1153；均为候选领取音效开始后第 150 帧。原作与候选的两处完成标记均首次出现在音效开始后的观测第 73 帧。这只支持该场景的顺序和确认行为，不代表精确 CPU 周期一致。

- 原作依据：`pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c` 的 `scripts/CinnabarLabMetronomeRoom.asm`、`scripts/SafariZoneSecretHouse.asm`、对应 `text/` 文件、`home/text.asm`、`home/text_script.asm` 和 `home/window.asm`。
- `verification.json` 保存构建身份、存档哈希、重复核验与范围限制。
- `recordings.json.gz` 保存各场景第一遍的输入及观测；第二遍哈希在清单中。
- 新游戏 m01–m49 已完成冠军战、名人堂、字幕及结局存档；独立进程 CONTINUE 返回真新镇，8 枚徽章、1 条名人堂记录保留。完整观测保存在 `fresh-first-clear.json.gz`。
- 核心库 3,290 项、核心集成 514 项、数据／Boa 479 项（另 1 项忽略）、应用完整测试、11 项 CLI 场景、库 Clippy correctness 与 ARM release 检查通过。原生录像后仅更新一条旧测试对新文字命令的识别；该测试以外的源码字节完全相同，证明见清单。
- 英中全部文字及顺序经逐项检查保留；原翻译检查器仅识别 speaker 块，无法直接比较这次改成的明确文字命令。
