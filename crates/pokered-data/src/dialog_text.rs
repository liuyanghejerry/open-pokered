//! Overworld-dialog localization (English → Chinese).
//!
//! Core-generated overworld messages (field moves, field items, fishing,
//! hidden items, map.json sign/NPC fallback text, …) are authored in English
//! and normally rendered verbatim. When Chinese is selected
//! (`OverworldScreen::set_script_lang("zh")`), the dialogue construction sites
//! run the message through [`localize`] **before** it is split into 2-line
//! pages, so template matching sees the full English text and the Chinese
//! output re-paginates naturally.
//!
//! Map-script (`@t`) dialogues resolve to Chinese upstream and pass through
//! unchanged here. Unknown texts pass through unchanged as well, so data-driven
//! English degrades gracefully instead of breaking.

/// Exact (static) message → Chinese, keyed on the full dialog text (all lines
/// joined with `\n`, matching the page split used by `BedroomDialogue`).
const EXACT: &[(&str, &str)] = &[
    // ── Field moves (field_moves.rs) ────────────────────────────────
    ("No! A new BADGE\nis required.", "不行！需要新的\n徽章。"),
    ("There isn't\nanything to CUT!", "这里没有可以砍的\n东西！"),
    ("Cycling is fun!\nForget SURFing!", "骑自行车很开心！\n不冲浪了！"),
    ("The current is\nmuch too fast!", "水流太急了！"),
    ("There's no place\nto get off!", "这里没有地方\n下去！"),
    ("A blinding FLASH\nlights the area!", "耀眼的闪光\n照亮了四周！"),
    ("Warp to the last\n#MON CENTER.", "传送回最后的\n宝可梦中心。"),
    ("Not healthy\nenough.", "体力不够。"),
    // ── Field items (overworld/screen.rs use_field_item) ────────────
    ("You played the\nPOKe FLUTE!\n\nThe SNORLAX\nwoke up!", "你吹响了宝可梦笛！\n\n卡比兽\n醒了过来！"),
    ("You played the\nPOKe FLUTE.\n\nNothing happened.", "你吹响了宝可梦笛。\n\n什么都没有发生。"),
    ("You can't get off\nhere.", "这里不能下去。"),
    ("You can't\nBICYCLE here!", "这里不能骑自行车！"),
    ("You got off the\nBICYCLE.", "你下了自行车。"),
    ("You got on the\nBICYCLE!", "你骑上了自行车！"),
    ("You checked the\nTOWN MAP.", "查看了城镇地图。"),
    ("Can't use that\nhere.", "这里不能使用。"),
    ("This isn't the\ntime to use that!", "现在不能使用！"),
    ("REPEL's effect\nwore off.", "驱虫剂的效果\n消失了。"),
    // ── Fishing (fishing.rs) ────────────────────────────────────────
    ("Not even a nibble!", "连一点动静都没有！"),
    ("Looks like there's\nnothing here.", "看来这里\n什么都没有。"),
    ("Oh!\nIt's a bite!", "哦！\n有鱼上钩了！"),
    // ── Hidden items / coins (hidden_items.rs) ──────────────────────
    ("Yes! ITEMFINDER\nindicates there's\nan item nearby.", "有了！寻宝机\n显示附近有道具。"),
    ("Nope! ITEMFINDER\nisn't responding.", "没有！寻宝机\n没有反应。"),
    ("Oops! Dropped\nsome coins!", "哎呀！硬币\n掉了一些！"),
    // ── Bag use on party Pokémon (items/bag_use.rs) ─────────────────
    ("It won't have any\neffect.", "不会有效果的。"),
    ("PP was\nrestored!", "PP回复了！"),
    ("HM techniques\ncan't be deleted!", "秘传招式\n不能忘掉！"),
    // ── Cable club / link (link/cable_club.rs) ──────────────────────
    ("Just a moment.", "请稍等。"),
    ("Waiting...!", "等待中……！"),
    ("PLEASE WAIT!", "请稍等！"),
    ("Trade completed!", "交换完成了！"),
    ("Too bad! The trade\nwas canceled!", "真遗憾！交换\n被取消了！"),
    // ── Safari game over (overworld/update.rs) ──────────────────────
    ("PA: Ding-ding!\nYour SAFARI GAME is over!", "广播：叮叮！\n你的狩猎游戏结束了！"),
    // ── Bedroom intro (overworld/screen.rs) ─────────────────────────
    ("...Okay!\nIt's time to go!", "……好了！\n该出发了！"),
    // ── map.json sign/NPC fallback texts ────────────────────────────
    // Pallet Town
    ("OAK: It's unsafe!\nWild POKeMON live\nin tall grass!\n\nYou need your own\nPOKeMON for your\nprotection.\nI know!\nHere, come with\nme!", "大木：太危险了！\n野生宝可梦\n就住在草丛里！\n\n为了保护自己，\n你需要自己的\n宝可梦。\n对了！跟我来！"),
    ("I'm raising\nPOKeMON too!\nWhen they get\nstrong, they can\nprotect me!", "我也在养宝可梦！\n等它们变强了，\n就能保护我了！"),
    ("Technology is\nincredible!\nYou can now store\nand recall items\nand POKeMON as\ndata via PC!", "科技真是了不起！\n现在可以把道具\n和宝可梦变成\n数据存进电脑，\n随时取出！"),
    ("OAK POKeMON\nRESEARCH LAB", "大木宝可梦\n研究所"),
    ("PALLET TOWN\nShades of your\njourney await!", "真新镇\n宛如你旅程的\n预兆！"),
    ("<PLAYER>'s house", "<PLAYER>的家"),
    ("<RIVAL>'s house", "<RIVAL>的家"),
    // Oak's Lab
    ("<RIVAL>: Yo\n<PLAYER>! Gramps\nisn't around!", "<RIVAL>：哟，\n<PLAYER>！爷爷\n不在啊！"),
    ("Those are POKe\nBALLs. They\ncontain POKeMON!", "那是精灵球。\n里面装着宝可梦！"),
    ("OAK: Now, <PLAYER>,\nwhich POKeMON do\nyou want?", "大木：<PLAYER>，\n你想要哪只\n宝可梦？"),
    ("It's encyclopedia-\nlike, but the\npages are blank!", "像是图鉴的东西，\n可是里面\n全是空白页！"),
    ("PROF.OAK is the\nauthority on\nPOKeMON!\n\nMany POKeMON\ntrainers hold him\nin high regard!", "大木博士是\n宝可梦研究的\n权威！\n\n许多宝可梦\n训练家都\n非常敬重他！"),
    ("Push START to\nopen the MENU!", "按 START 键\n打开菜单！"),
    ("The SAVE option is\non the MENU screen.", "保存选项在\n菜单画面里。"),
    ("There's an e-mail message here!\n\n...\n\nCalling all POKeMON trainers!\nThe elite trainers of POKeMON LEAGUE are ready to take on all comers!\nBring your best POKeMON and see how you rate as a trainer!\n\nPOKeMON LEAGUE HQ INDIGO PLATEAU\n\nPS: PROF.OAK, please visit us! ...", "这里有一封电子邮件！\n\n……\n\n召集所有宝可梦训练家！\n宝可梦联盟的精英训练家\n随时恭候各位挑战者！\n带上你最强的宝可梦，\n来看看你有几斤几两吧！\n\n宝可梦联盟总部 石英高原\n\n又及：大木博士，请来我们这里做客！……"),
    // Blue's house
    ("Hi <PLAYER>!\n<RIVAL> is out at\nGrandpa's lab.", "你好，<PLAYER>！\n<RIVAL>去爷爷的\n研究所了。"),
    ("POKeMON are living\nthings! If they\nget tired, give\nthem a rest!", "宝可梦是活的！\n它们累了的话，\n要让它们休息！"),
    ("It's a big map!\nThis is useful!", "好大的地图！\n这个真有用！"),
    ("Crammed full of\nPOKeMON books!", "塞满了宝可梦\n相关的书！"),
    // Bike shop
    ("A shiny new\nBICYCLE!", "崭新闪亮的\n自行车！"),
    // Museum
    ("It's ¥50 for a\nchild's ticket.\nWould you like to\ncome in?", "儿童票 50 元。\n要进来参观吗？"),
    ("That is one\nmagnificent\nfossil!", "这可真是\n一件了不起的\n化石！"),
    ("Ssh! I think that\nthis chunk of\nAMBER contains\n#MON DNA!\nIt would be great\nif #MON could\nbe resurrected\nfrom it!\nBut, my colleagues\njust ignore me!\nSo I have a favor\nto ask!\nTake this to a\n#MON LAB and\nget it examined!", "嘘！我觉得这块\n琥珀里含有\n宝可梦的DNA！\n如果能从里面\n复活出宝可梦，\n那就太棒了！\n可是同事们\n都不理我！\n所以想拜托你\n一件事！\n把这个带到宝可梦\n研究所去\n检查一下吧！"),
    ("We are proud of 2\nfossils of very\nrare, prehistoric\n#MON!", "我们引以为豪的\n是两具非常稀有\n的史前宝可梦\n化石！"),
    ("The AMBER is\nclear and gold!", "这琥珀又清澈\n又金黄！"),
    ("AERODACTYL Fossil\nA primitive and\nrare POKeMON.", "化石翼龙化石\n原始而稀有的\n宝可梦。"),
    ("KABUTOPS Fossil\nA primitive and\nrare POKeMON.", "镰刀盔化石\n原始而稀有的\n宝可梦。"),
    ("MOON STONE?\n\nWhat's so special\nabout it?", "月之石？\n\n它有什么特别的\n地方吗？"),
    ("July 20, 1969!\n\nThe 1st lunar\nlanding!\nI bought a color\nTV to watch it!", "1969年7月20日！\n\n人类第一次\n登上月球！\n我买了台彩色电视\n专门看直播！"),
    ("We have a space\nexhibit now.", "我们现在有太空\n主题的展览。"),
    ("I want a PIKACHU!\nIt's so cute!\nI asked my Daddy\nto catch me one!", "我想要皮卡丘！\n好可爱！\n我求爸爸\n帮我抓一只！"),
    ("Yeah, a PIKACHU\nsoon, I promise!", "嗯，很快就有\n皮卡丘了，我保证！"),
    ("SPACE SHUTTLE\nCOLUMBIA", "哥伦比亚号\n航天飞机"),
    ("Meteorite that\nfell on MT.MOON.\n(MOON STONE?)", "落在月见山上的\n陨石。\n（月之石？）"),
    // Fighting dojo
    ("FIGHTING DOJO", "格斗道馆"),
    ("Enemies on every\nside!", "到处都是\n敌人！"),
    ("What goes around\ncomes around!", "善恶到头\n终有报！"),
    // Game corner
    ("OUT OF ORDER\nThis is broken.", "故障中\n这个坏了。"),
    ("OUT TO LUNCH\nThis is reserved.", "外出用餐\n这个已被预约。"),
    ("Someone's keys!\nThey'll be back.", "是谁掉的钥匙！\n主人会回来的。"),
    // Indigo Plateau
    ("INDIGO PLATEAU\nPOKeMON LEAGUE HQ", "石英高原\n宝可梦联盟总部"),
    // Mr. Fuji's house
    ("That's odd, MR.FUJI\nisn't here.\nWhere'd he go?", "真奇怪，富士老人\n不在这里。\n他去哪儿了？"),
    ("This is really\nMR.FUJI's house.\n\nHe's really kind!\n\nHe looks after\nabandoned and\norphaned #MON!", "这里真的是\n富士老人的家。\n\n他人真的很亲切！\n\n他照顾着被遗弃的\n和失去双亲的\n宝可梦！"),
    ("PSYDUCK: Gwappa!", "可达鸭：嘎啪！"),
    ("NIDORINO: Gaoo!", "尼多朗：嘎呜！"),
    ("MR.FUJI: <PLAYER>.\n\n\nYour #DEX quest\nmay fail without\nlove for your\n#MON.\n\nI think this may\nhelp your quest.", "富士老人：<PLAYER>。\n\n\n如果没有对宝可梦\n的爱，你的图鉴\n之旅可能\n无法完成。\n\n我觉得这个\n能帮上你。"),
    ("#MON Monthly\nGrand Prize\nDrawing!\n\n\nThe application\nform is...\n\nGone! It's been\nclipped out!", "宝可梦月刊\n大奖\n抽奖！\n\n\n申请表\n是……\n\n不见了！被人\n剪走了！"),
    ("POKeMON magazines!\nPOKeMON notebooks!\nPOKeMON graphs!", "宝可梦杂志！\n宝可梦笔记本！\n宝可梦图表！"),
    // S.S. Anne kitchen
    ("You, mon petit!\nWe're busy here!\nOut of the way!", "说你呢，小家伙！\n我们忙着呢！\n别挡路！"),
    ("I saw an odd ball\nin the trash.", "我在垃圾桶里\n看到个怪东西。"),
    ("I'm so busy I'm\ngetting dizzy!", "忙得我\n头都晕了！"),
    ("Hum-de-hum-de-\nho...\n\nI peel spuds\nevery day!\nHum-hum...", "哼呐哼呐\n呵……\n\n我每天削土豆皮！\n哼哼……"),
    ("Did you hear about\nSNORLAX?\n\nAll it does is\neat and sleep!", "你听说卡比兽了吗？\n\n它整天就知道\n吃了睡！"),
    ("Snivel...Sniff...\n\n\nI only get to\npeel onions...\nSnivel...", "抽泣……吸鼻子……\n\n\n我只能\n削洋葱……\n抽泣……"),
    ("Er-hem! Indeed I\nam le CHEF!\n\nLe main course is\n[random dish]", "咳咳！本人正是\n主厨！\n\n今日主菜是\n[random dish]"),
    ("Nope, there's\nonly trash here.", "没有，这里\n只有垃圾。"),
    // Vermilion gym NPCs
    ("Hey, kid! What do\nyou think you're\ndoing here?\n\n\nYou won't live\nlong in combat!\nThat's for sure!\n\n\nI tell you kid,\nelectric #MON\nsaved me during\nthe war!\n\nThey zapped my\nenemies into\nparalysis!\n\n\nThe same as I'll\ndo to you!", "喂，小子！你觉得\n你在这里\n干什么？\n\n\n打起仗来你\n可活不长！\n这是肯定的！\n\n\n告诉你，小子，\n电系宝可梦\n在战争中\n救过我的命！\n\n它们把敌人\n电得麻痹\n动弹不得！\n\n\n我也要让你\n尝尝这个滋味！"),
    ("When I was in the\nArmy, LT.SURGE\nwas my strict CO!", "我当兵的时候，\n马志士是我\n严格的上司！"),
    ("I'm a lightweight,\nbut I'm good with\nelectricity!", "我虽然瘦小，\n但玩电很在行！"),
    ("This is no place\nfor kids!", "这里可不是\n小孩子来的地方！"),
    ("Yo! Champ in\nmaking!\n\nLT.SURGE has a\nnickname. People\nrefer to him as\nthe Lightning\nAmerican!\n\n\nHe's an expert on\nelectric #MON!\n\nBirds and water\n#MON are at\nrisk! Beware of\nparalysis too!\n\nLT.SURGE is very\ncautious!\n\nYou'll have to\nbreak a code to\nget to him!", "哟！未来的冠军！\n\n马志士有个外号。\n人们都叫他\n闪电\n美国人！\n\n\n他是电系宝可梦\n的专家！\n\n鸟系和水系\n宝可梦都危险！\n小心麻痹！\n\n马志士非常\n谨慎！\n\n你得破译密码\n才能见到他！"),
    // School
    ("Whew! I'm trying\nto memorize all\nmy notes.", "呼！我在努力背\n我的笔记。"),
    ("Okay!\nBe sure to read\nthe blackboard\ncarefully!", "好的！\n一定要仔细读\n黑板上的内容！"),
    ("Looked at the notebook!\n\nFirst page...\n\n# BALLs are used to catch POKeMON.\nUp to 6 POKeMON can be carried.\nPeople who raise and make POKeMON fight are called POKeMON trainers.\n\nSecond page...\n\nA healthy POKeMON may be hard to catch, so weaken it first!\nPoison, burns and other damage are effective!\n\nThird page...\n\nPOKeMON trainers seek others to engage in POKeMON fights.\nBattles are constantly fought at POKeMON GYMs.\n\nFourth page...\n\nThe goal for POKeMON trainers is to beat the top 8 POKeMON GYM LEADERs.\nDo so to earn the right to face...\nThe ELITE FOUR of POKeMON LEAGUE!\n\nGIRL: Hey! Don't look at my notes!", "看了笔记！\n\n第一页……\n\n精灵球用来捕捉宝可梦。\n最多可以携带6只宝可梦。\n培养宝可梦并让它们对战的人，被称为宝可梦训练家。\n\n第二页……\n\n健康的宝可梦可能很难捕捉，所以要先削弱它！\n中毒、灼伤等异常状态伤害很有效！\n\n第三页……\n\n宝可梦训练家会互相切磋，进行宝可梦对战。\n宝可梦道馆里时刻都在进行对战。\n\n第四页……\n\n宝可梦训练家的目标是击败8位道馆馆主。\n做到这一点，就有资格挑战……\n宝可梦联盟的四天王！\n\n女孩：喂！不许看我的笔记！"),
    // Hotel / mansion roof house
    ("My sis brought me", "我姐姐带我来的"),
    ("It's a pamphlet on TMs.\n\n...\n\nThere are 50 TMs in all.\n\nThere are also 5 HMs that can be used repeatedly.\n\nSILPH CO.", "这是一本招式学习器手册。\n\n……\n\n招式学习器共有50种。\n\n另外还有5种可以\n反复使用的秘传学习器。\n\n西尔佛公司"),
    // Route 15 gate
    ("Looked into the\nbinoculars.\n\nIt looks like a\nsmall island!", "用望远镜看了看。\n\n看起来像一座\n小岛！"),
    ("Looked into the\nbinoculars...\nA large, shining\nbird is flying\ntoward the sea.", "用望远镜看了看……\n一只巨大的、\n闪闪发光的鸟\n正朝大海飞去。"),
    // Pokémon centers (BILL gossip + signs)
    ("BILL has lots of\nPOKeMON!\nHe collects rare", "正辉有很多\n宝可梦！\n他专门收集稀有的"),
    ("Yawn!\nWhen JIGGLYPUFF\nsings, POKeMON\n...Me too...", "哈欠！\n胖丁一唱歌，\n宝可梦就……\n我也是……"),
    ("That BILL!\nI heard that\nhe'll do whatever\nit takes to get\nrare #MON!", "那个正辉！\n听说为了得到\n稀有的宝可梦，\n他什么都\n干得出来！"),
    ("Have you heard\nabout BILL?\nEveryone calls\nhim a #MANIAC!\nI think people\nare just jealous\nof BILL, though.\nWho wouldn't want\nto boast about\ntheir #MON?", "你听说过正辉吗？\n大家都叫他\n宝可梦狂！\n不过我觉得\n那些人只是\n嫉妒正辉罢了。\n谁不想炫耀\n自己的宝可梦呢？"),
    ("You can use that\nPC in the corner.\nThe receptionist\ntold me. So kind!", "你可以用角落里的\n那台电脑。\n是前台告诉我的，\n人真好！"),
    ("There's a POKeMON\nCENTER in every\ntown ahead.\nThey don't charge\nany money either!", "前面的每个镇子\n都有宝可梦中心。\n而且一分钱\n都不收！"),
    ("POKeMON CENTERs\nheal your tired,", "宝可梦中心能治愈\n你疲惫的，"),
    // Pewter center
    ("What!?\nTEAM ROCKET is\nat MT.MOON? Huh?\nI'm on the phone!\nScram!", "什么！？\n火箭队在月见山？\n呃？我正在打电话！\n走开走开！"),
    ("JIGGLYPUFF: Puu\npupuu!", "胖丁：噗——\n噗噗噗！"),
    // Cerulean player's house / city misc kept minimal above.
    ("Everyone calls\nhim a #MANIAC!", "大家都叫他\n宝可梦狂！"),
    // Route / town signs & frequently hit NPC fallbacks.
    ("ROUTE 1\nPALLET TOWN -\nVIRIDIAN CITY", "1号道路\n真新镇 -\n常磐市"),
    ("ROUTE 3\nMT.MOON AHEAD", "3号道路\n前方是月见山"),
    ("Now leaving\nVIRIDIAN FOREST", "前方出口\n常磐森林"),
    ("VIRIDIAN FOREST\nPEWTER CITY -\nVIRIDIAN CITY", "常磐森林\n尼比市 -\n常磐市"),
    ("CERULEAN CITY\nA Mysterious,\nBlue Aura\nSurrounds It", "华蓝市\n神秘的蓝色\n气息笼罩着\n这座城市"),
    ("TRAINER TIPS\nPressing B Button\nduring evolution\ncancels the whole\nprocess.", "训练家须知\n在宝可梦进化的过程中\n按下 B 键，\n整个进化就会\n被取消。"),
    ("Grass and caves\nhandled easily!\nBIKE SHOP", "草丛和洞窟\n都不在话下！\n自行车店"),
    ("Heal Your #MON!\n#MON CENTER", "治愈你的宝可梦！\n宝可梦中心"),
    ("All your item\nneeds fulfilled!\n#MON MART", "满足你所有的\n道具需求！\n宝可梦商店"),
    ("CERULEAN CITY\n#MON GYM\nLEADER: MISTY\n\nThe Tomboyish\nMermaid!", "华蓝市\n宝可梦道馆\n馆主：小霞\n\n假小子\n人鱼小姐！"),
    ("TRAINER TIPS\nIf you want to\navoid battles,\nstay away from\ngrassy areas!", "训练家须知\n如果不想\n进行对战，\n就远离\n草丛！"),
    ("TRAINER TIPS\nIf a POKeMON is\npoisoned, it loses\nHP even after the\nbattle.\nUse ANTIDOTE to\ncure it!", "训练家须知\n宝可梦中毒后，\n即使对战结束\n也会损失HP。\n使用解毒药\n可以治疗！"),
    ("TRAINER TIPS\nContact with the\noutside world has\nbeen made through\nthe Pc system.", "训练家须知\n通过电脑系统，\n可以与外面的\n世界取得联系。"),
    ("TRAINER TIPS\nPOKeMON attacks\nare physical or\nspecial based on\ntheir type.", "训练家须知\n宝可梦的攻击\n根据其属性，\n分为物理攻击\n和特殊攻击。"),
    // Pokémon Tower
    ("#MON TOWER was\nerected in the\nmemory of #MON\nthat had died.", "宝可梦塔是为了\n纪念已经故去的\n宝可梦\n而建造的。"),
    ("Did you come to\npay respects?\nBless you!", "你是来\n祭奠的吗？\n愿神保佑你！"),
    ("I came to pray\nfor my CLEFAIRY.\n\nSniff! I can't\nstop crying...", "我是来为我的\n皮皮祈祷的。\n\n呜呜！我止不住\n眼泪……"),
    ("My GROWLITHE...\nWhy did you die?", "我的卡蒂狗……\n你为什么要死啊？"),
    ("I am a CHANNELER!\nThere are spirits\nup to mischief!", "我是通灵者！\n这里有幽灵\n在捣乱！"),
    // Route 1 NPCs
    ("Hi! I work at a\nPOKeMON MART.\nIt's a convenient\nshop, so please\nvisit us in\nVIRIDIAN CITY.\nI know, I'll give\nyou a sample!\nHere you go!", "你好！我在宝可梦\n商店工作。\n我们的店很方便，\n请一定来常磐市\n光顾本店。\n对了，送你\n一份赠品！\n请收下！"),
    ("See those ledges\nalong the road?\nIt's a bit scary,\nbut you can jump\nfrom them.\nYou can get back\nto PALLET TOWN\nquicker that way.", "看到路上的\n那些台阶了吗？\n虽然有点吓人，\n但可以从上面\n跳下去。\n那样回真新镇\n会快一些。"),
];

/// Template-based translations for parametrized messages. Checked in order
/// after [`EXACT`]; each entry is `(suffix, zh_prefix_placeholder)` pairs
/// handled inline in [`localize`].
pub fn localize(text: &str) -> String {
    // 1. Exact table. Keys are compared with trailing blank lines trimmed so
    //    page padding doesn't break the lookup.
    let key = text.trim_matches('\n');
    if let Some(zh) = EXACT.iter().find(|(en, _)| *en == key).map(|(_, zh)| *zh) {
        return zh.to_string();
    }

    // 2. Templates. Order matters: more specific patterns first.
    let (zh, _hit) = localize_template(text);
    zh
}

/// Apply the template rules; returns `(translated, true)` when one matched.
fn localize_template(text: &str) -> (String, bool) {
    // Field moves: "{mon} hacked\naway with CUT!"
    if let Some(a) = text.strip_suffix(" hacked\naway with CUT!") {
        return (format!("{}使用了居合斩，\n把树木砍倒了！", crate::battle_text::zh_name(a)), true);
    }
    // "{mon} can't\nFLY here."
    if let Some(a) = text.strip_suffix(" can't\nFLY here.") {
        return (format!("{}不能在这里\n飞翔。", crate::battle_text::zh_name(a)), true);
    }
    // "No SURFing on\n{mon}\nhere!"
    if let Some(a) = text.strip_prefix("No SURFing on\n") {
        if let Some(a) = a.strip_suffix("\nhere!") {
            return (format!("不能在这里\n让{}冲浪！", crate::battle_text::zh_name(a)), true);
        }
    }
    // "{player} got on\n{mon}!"
    if let Some(idx) = text.find(" got on\n") {
        if let Some(mon) = text[idx + " got on\n".len()..].strip_suffix('!') {
            return (
                format!(
                    "{}骑上了\n{}！",
                    crate::battle_text::zh_name(&text[..idx]),
                    crate::battle_text::zh_name(mon)
                ),
                true,
            );
        }
    }
    // "{mon} used\nSTRENGTH.\n{mon} can\nmove boulders."
    if let Some(rest) = text.strip_suffix(" can\nmove boulders.") {
        if let Some(idx) = rest.find(" used\nSTRENGTH.\n") {
            let a = &rest[..idx];
            let b = &rest[idx + " used\nSTRENGTH.\n".len()..];
            if a == b {
                return (
                    format!("{}使用了怪力。\n{}可以推动\n岩石了。", crate::battle_text::zh_name(a), crate::battle_text::zh_name(b)),
                    true,
                );
            }
        }
    }
    // "{mon} can't\nuse TELEPORT\nnow."
    if let Some(a) = text.strip_suffix(" can't\nuse TELEPORT\nnow.") {
        return (format!("{}现在不能使用\n瞬间移动。", crate::battle_text::zh_name(a)), true);
    }
    // "You used the\n{item}!" (fishing rod / repel)
    if let Some(a) = text.strip_prefix("You used the\n") {
        if let Some(a) = a.strip_suffix('!') {
            return (format!("你使用了\n{}！", crate::battle_text::zh_name(a)), true);
        }
    }
    // "{player} found\n{item}!"
    if let Some(idx) = text.find(" found\n") {
        if text.ends_with('!') {
            let item = text[idx + " found\n".len()..].strip_suffix('!').unwrap_or("");
            return (
                format!(
                    "{}找到了\n{}！",
                    crate::battle_text::zh_name(&text[..idx]),
                    crate::battle_text::zh_name(item)
                ),
                true,
            );
        }
    }
    // "But, {player} has\nno more room for\nother items!"
    if let Some(a) = text.strip_suffix(" has\nno more room for\nother items!") {
        if let Some(a) = a.strip_prefix("But, ") {
            return (format!("可是，{}\n没有空位再装\n其他道具了！", crate::battle_text::zh_name(a)), true);
        }
    }
    // "{player} found\n@{n} coins!"
    if let Some(idx) = text.find(" found\n@") {
        if let Some(n) = text[idx + " found\n@".len()..].strip_suffix(" coins!") {
            return (format!("{}找到了\n@{}枚硬币！", crate::battle_text::zh_name(&text[..idx]), n), true);
        }
    }
    // Bag use on party Pokémon.
    // "{mon}'s HP was\nrestored by {n}!"
    if let Some(idx) = text.find("'s HP was\nrestored by ") {
        if let Some(n) = text[idx + "'s HP was\nrestored by ".len()..].strip_suffix('!') {
            return (format!("{}的HP回复了{}点！", crate::battle_text::zh_name(&text[..idx]), n), true);
        }
    }
    // "{mon} was\nrevitalized!"
    if let Some(a) = text.strip_suffix(" was\nrevitalized!") {
        return (format!("{}恢复活力了！", crate::battle_text::zh_name(a)), true);
    }
    // "{mon} was cured\nof its status!"
    if let Some(a) = text.strip_suffix(" was cured\nof its status!") {
        return (format!("{}的异常状态\n治好了！", crate::battle_text::zh_name(a)), true);
    }
    // "{move}'s PP\nincreased!"
    if let Some(a) = text.strip_suffix("'s PP\nincreased!") {
        return (format!("{}的PP增加了！", crate::battle_text::zh_name(a)), true);
    }
    // "{mon}'s stats\nrose!"
    if let Some(a) = text.strip_suffix("'s stats\nrose!") {
        return (format!("{}的能力提高了！", crate::battle_text::zh_name(a)), true);
    }
    // "{mon} grew to\nlevel {n}!"
    if let Some(idx) = text.find(" grew to\nlevel ") {
        if let Some(n) = text[idx + " grew to\nlevel ".len()..].strip_suffix('!') {
            return (format!("{}升到了{}级！", crate::battle_text::zh_name(&text[..idx]), n), true);
        }
    }
    // "{mon} forgot\n{m1}...\nand learned\n{m2}!" — MUST run before the
    // plainer " learned\n" rule, which would otherwise match inside
    // "and learned\n{m2}!".
    if let Some(idx) = text.find(" forgot\n") {
        if let Some(after) = text[idx + " forgot\n".len()..].strip_suffix('!') {
            if let Some(pos) = after.find("...\nand learned\n") {
                let b = &after[..pos];
                let c = &after[pos + "...\nand learned\n".len()..];
                return (
                    format!(
                        "{}忘掉了{}，\n学会了{}！",
                        crate::battle_text::zh_name(&text[..idx]),
                        crate::battle_text::zh_name(b),
                        crate::battle_text::zh_name(c)
                    ),
                    true,
                );
            }
        }
    }
    // "{mon} learned\n{move}!"
    if let Some(idx) = text.find(" learned\n") {
        if let Some(b) = text[idx + " learned\n".len()..].strip_suffix('!') {
            return (
                format!(
                    "{}学会了{}！",
                    crate::battle_text::zh_name(&text[..idx]),
                    crate::battle_text::zh_name(b)
                ),
                true,
            );
        }
    }
    // "{mon} already\nknows {move}!"
    if let Some(idx) = text.find(" already\nknows ") {
        if let Some(b) = text[idx + " already\nknows ".len()..].strip_suffix('!') {
            return (
                format!(
                    "{}已经会{}了！",
                    crate::battle_text::zh_name(&text[..idx]),
                    crate::battle_text::zh_name(b)
                ),
                true,
            );
        }
    }
    // "{mon} is not\ncompatible with\n{move}.\nIt can't learn\n{move}."
    if let Some(rest) = text.strip_suffix("\nIt can't learn\n") {
        if let Some(idx) = rest.find(" is not\ncompatible with\n") {
            let a = &rest[..idx];
            let b = rest[idx + " is not\ncompatible with\n".len()..].strip_suffix('.');
            if let Some(b) = b {
                return (
                    format!(
                        "{}与{}属性不合，\n学不会它。",
                        crate::battle_text::zh_name(a),
                        crate::battle_text::zh_name(b)
                    ),
                    true,
                );
            }
        }
    }
    // "{mon} recovered by\n{n}!"
    if let Some(idx) = text.find(" recovered by\n") {
        if let Some(n) = text[idx + " recovered by\n".len()..].strip_suffix('!') {
            return (format!("{}回复了{}点HP！", crate::battle_text::zh_name(&text[..idx]), n), true);
        }
    }
    // Bedroom intro (player name substituted at the call site).
    if let Some(a) = text.strip_suffix(" is\nplaying the SNES!\n...Okay!\nIt's time to go!") {
        return (
            format!("{}正在玩超级任天堂！\n……好了！\n该出发了！", crate::battle_text::zh_name(a)),
            true,
        );
    }
    (text.to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_passes_through_unknown() {
        assert_eq!(localize("Hello world"), "Hello world");
        // Scene-authored Chinese passes through untouched.
        assert_eq!(localize("你好！欢迎来到\n宝可梦的世界！"), "你好！欢迎来到\n宝可梦的世界！");
    }

    #[test]
    fn field_moves() {
        assert_eq!(localize("No! A new BADGE\nis required."), "不行！需要新的\n徽章。");
        assert_eq!(
            localize("SQUIRTLE hacked\naway with CUT!"),
            "杰尼龟使用了居合斩，\n把树木砍倒了！"
        );
        assert_eq!(
            localize("No SURFing on\nBLASTOISE\nhere!"),
            "不能在这里\n让水箭龟冲浪！"
        );
        assert_eq!(
            localize("RED got on\nSQUIRTLE!"),
            "RED骑上了\n杰尼龟！"
        );
        assert_eq!(localize("Not healthy\nenough."), "体力不够。");
    }

    #[test]
    fn field_items_and_fishing() {
        assert_eq!(
            localize("You used the\nGOOD ROD!"),
            "你使用了\n好钓竿！"
        );
        assert_eq!(localize("You checked the\nTOWN MAP."), "查看了城镇地图。");
        assert_eq!(localize("REPEL's effect\nwore off."), "驱虫剂的效果\n消失了。");
    }

    #[test]
    fn hidden_items_and_bag() {
        assert_eq!(localize("RED found\nNUGGET!"), "RED找到了\n金珠！");
        assert_eq!(
            localize("But, RED has\nno more room for\nother items!"),
            "可是，RED\n没有空位再装\n其他道具了！"
        );
        assert_eq!(
            localize("PIKACHU's HP was\nrestored by 20!"),
            "皮卡丘的HP回复了20点！"
        );
        assert_eq!(
            localize("PIKACHU grew to\nlevel 12!"),
            "皮卡丘升到了12级！"
        );
        assert_eq!(
            localize("PIKACHU forgot\nTHUNDER...\nand learned\nTHUNDERBOLT!"),
            "皮卡丘忘掉了打雷，\n学会了十万伏特！"
        );
    }

    #[test]
    fn safari_and_map_fallbacks() {
        assert_eq!(
            localize("PA: Ding-ding!\nYour SAFARI GAME is over!"),
            "广播：叮叮！\n你的狩猎游戏结束了！"
        );
        assert_eq!(localize("A shiny new\nBICYCLE!"), "崭新闪亮的\n自行车！");
        assert!(localize("CERULEAN CITY\n#MON GYM\nLEADER: MISTY\n\nThe Tomboyish\nMermaid!").contains("华蓝市"));
    }
}
