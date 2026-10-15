# 捕获后的图鉴、取名和入箱流程

原作 `pret/pokered` 的 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`，
`engine/items/item_effects.asm:518–568` 在成功文字之后检查已拥有位：新种类先显示
“New POKéDEX data will be added…” 和图鉴条目，然后调用 AddPartyMon 或 SendNewMonToBox。
两条路径均调用 AskName；入箱还依据 EVENT_MET_BILL 显示正辉或某人的电脑。
成功文字的 `ItemUseBallText05` 带 `sound_caught_mon` 和确认等待；
`ItemUseBallText06` 带 `sound_dex_page_added` 和确认等待。
原作 AddPartyMon 和 SendNewMonToBox 都从玩家记录复制 OT 名字和 ID。

修复前，新种类已有图鉴条目，但普通和 Safari 捕获都缺少取名及入箱提示。
普通成功文字只有 “Caught!”，Safari 为 “Gotcha!”；Safari 保存的新宝可梦 OT ID 为 0、
OT 名字为空。已有图鉴条目从未缺失，本次不再将它列为缺陷。

修复后，成功文字使用实际捕获种类，依次处理新图鉴说明、已有条目、YES/NO、现有名字输入框
及入箱文字；已有种类直接进入取名。新图鉴提示和成功提示保留各自声音等待及新的确认输入。
取名写回捕获插入操作返回的实际队伍或当前箱槽位，兼容新宝可梦插入箱首；其他记录保持原样。
共同结算入口写入玩家原始 OT 名字和 ID；未第一次 SAVE 的正常大木博士名字选择也立即同步
到该原始名字记录。Safari 最后一球成功后，取名和入箱提示结束才继续公告及离场。

流程借用现有 NamingScreen，结束、重置、读档或恢复快照会清理继续点。
流程进行中拒绝保存调试快照，避免将未结束的旧 BattleOver 写回新存档。
原作清除精灵时保留的玩家 HUD，以及 YES 取名返回后的战斗背景，按流程阶段绘制。
本次没有修改字体、中文、拼音输入法或通用对话框布局。

## 真实输入和保存验证

所有存档构造均在启动和验证输入前完成；验证期间没有写战斗 RAM、替换遇敌种类或跳转地图。
普通捕获采用合法 ViridianForest 草地存档、自然遭遇 WEEDLE L4、MASTER BALL、普通 START→SAVE。
控制已拥有/未拥有 × 队伍 5/6 只 × YES:A/NO，每种重复两次；另测 metBill=true 的入箱两种回答各两次。

| 证据 | 完成的普通 SAVE | 版本 |
| --- | ---: | --- |
| 原作普通捕获及 BILL 分支 | 16 + 4 | fbcf7d0e 原作构造控制 |
| 当前基线普通捕获及 BILL 分支 | 16 + 4 | 干净 master 72912bc1，SHA c7f51b32… |
| 最终修复普通捕获及 BILL 分支 | 16 + 4 | b98bd9eb，SHA f574dbe4… |
| 未 SAVE 的实际大木博士 ASH 名字→自然捕获→首次 SAVE | 2 | 最终修复同源测试构建 |
| 原作 / 修复前 / 修复后 Safari 成功 | 各 4 | 原作 / db612ae6、SHA 996d61cc… / b98bd9eb |

40 个新鲜普通捕获前后对照均核对每帧状态、PNG、实际 YES/NO、昵称、图鉴拥有门槛、一次球消耗、
新 OT 原始字节、原有队伍和盒子完整记录及 SAVE 校验和。两次重复的动作、PNG、最终 SRAM 相同。
未 SAVE 的 ASH 两次控制保留 2727 帧输入/状态，保存捕获 OT ID 22373 和原始 ASH 字节；
该控制预置初始宝可梦、道具和草地起点，不能称为正常领取初始宝可梦、包裹任务或全主线回归。

Safari 的 4 次修复后成功与修复前使用完全相同的前置存档；入箱最后一球两次均为
NewDex→AskName→YES:A→Transfer→PA→Gate，旧队伍、OT/昵称/PP、旧盒记录、道具和金钱保持。
原作自然 NIDORAN♂ 与复刻自然 VENONAT 的随机种类不同，未声称新宝可梦所有原始数据相等。
9 次自然首投失败保留且不计成功。旧版并未重现 PA 覆盖图鉴，缺失的是图鉴之后的名字和电脑提示。

声音验证记录静音设备下仍运行的 PCM 状态：成功 SFX 101、新图鉴 SFX 100 和结束后的新确认输入。
不声称原作音频波形或采样时钟一致。普通原作与复刻的 RNG、DVs、帧耗时及捕获动画像素也未匹配。
Silph 离场动画和原生 IRQ 原型与本次无关，没有宣称修复。

## 截图与可复查文件

`docs/screenshots/fidelity-post-capture-880/` 保存同一前置存档、seed 12、相同真实输入的同帧前后截图。
基线没有新增模态对应的画面，因此取名和输入名字的“前”是已经返回的草地画面；BILL 帧的同样 START/A 输入会在基线打开图鉴列表。
帧 2181 为成功文字，2498 为新图鉴提示最后一页，2709 为取名选择，2721 为输入 A；
BILL 提示配对在帧 2567。`manifest.json` 记录文件 SHA、准确版本和捕获帧。

`docs/audits/post-capture-flow/evidence.zip` 以 SHA256 去重保存 19557 个原文件路径（655 个内容块）：合法现代 JSON 前置存档、输入动作、逐帧记录、PNG、普通 SRAM、校验报告和
测试日志；不含游戏 ROM 或构建可执行文件。同目录的 `unpack_evidence.py` 校验并还原所有原路径。`fixtures/` 单独提供两个可直接使用 `--snapshot` 的存档。
所有冻结证据维持原版本标签，不因后续合并 master 而改称新版本验证。

核心全套 2879 项通过；最终 app 8 项针对捕获继续点的测试通过，另一个真实输入记录器按需显式运行，
两次首次 SAVE 均通过。单测覆盖实际槽位、旧记录保护、B/空名字、名字提交、BILL、Safari 暂停、声音确认、
重置和读档清理；不同变身显示种类与实际捕获种类也有成功文字测试（未做实际变身捕获录制）。

后续例行合并 master 64ef04af 后，捕获相关全部 10 个哈希和本次全部 8 个 Rust 文件保持一致；
专项 8 项（库和二进制各运行一次）及合并后的核心全套 2883 项通过，详情见
`docs/audits/post-capture-flow/integration-verification.json`。历史录制仍为 72912bc1/b98bd9eb，不改标为合并后录制。

例行接入 master 4cd8fc36（TM/HM 完整学习流程）后，再次通过捕获 8 项、TM/HM 10 项和 Rare Candy 11 项；证据见 `post-tmhm-integration-verification.json`。历史录制身份保持原样。
