#!/usr/bin/env python3
"""Inject Simplified-Chinese Pokédex entries (category + flavor text) into
crates/pokered-data/pokemon/*.json.

For every one of the 151 canonical species this writes
`pokedex.categoryZh` and `pokedex.flavorTextPagesZh` (one ZH page per EN
page, same order). All other JSON content is preserved byte-for-byte
(key order + 2-space indent round-trip). Follows the pattern of
tools/gen_battle_quips.py: full-coverage validation, missing → error.

Style: official Simplified-Chinese translations — species names follow
pokered_data::lang_data::SPECIES_ZH; categories carry the 宝可梦 suffix
(≤ 7 full-width chars so the entry label fits the 18-tile box when
right-aligned). Flavor pages are stored as one continuous string per page;
the renderer wraps CJK text to the entry box width (≤ 18 tile units/line).

Usage: python3 tools/gen_pokedex_zh.py   (from the repo root)
"""
import json
import sys
from collections import OrderedDict
from pathlib import Path

POKEMON_DIR = Path(__file__).resolve().parent.parent / "crates/pokered-data/pokemon"

# species → (category_zh, [page0_zh, page1_zh]); pages match flavorTextPages 1:1.
ZH = {
    'Abra': ('念力宝可梦', ['拥有读取心声的能力，能察觉迫近的危险。', '一旦感到危险，就会用瞬间移动逃到安全的地方。']),
    'Aerodactyl': ('化石宝可梦', ['性格凶暴的远古宝可梦，会用锯齿般的利牙', '直取敌人的咽喉。']),
    'Alakazam': ('念力宝可梦', ['它的大脑比超级计算机还要出色。', '据说智商高达5000。']),
    'Arbok': ('眼镜蛇宝可梦', ['据说它腹部吓人的警告花纹，', '因地区不同而各不相同。']),
    'Arcanine': ('传说宝可梦', ['从古代起就因美丽而受人喜爱的宝可梦。', '奔跑起来轻盈敏捷，仿佛长了翅膀。']),
    'Articuno': ('急冻宝可梦', ['传说中的鸟宝可梦，据说会在冰封的山中迷路、', '濒临绝境的人面前出现。']),
    'Beedrill': ('毒蜂宝可梦', ['飞行速度极快，用前肢和尾部巨大的毒针', '发起攻击。']),
    'Bellsprout': ('花宝可梦', ['会设陷阱捕食虫子的肉食宝可梦。', '用根一样的脚吸取生长所需的水分。']),
    'Blastoise': ('贝壳宝可梦', ['性情凶暴的宝可梦，甲壳上装有加压水炮。', '它会借着水炮的推力发起高速撞击。']),
    'Bulbasaur': ('种子宝可梦', ['出生时，背上就种着一颗奇妙的种子。', '种子发芽长大，和它一起成长。']),
    'Butterfree': ('蝴蝶宝可梦', ['战斗时高速扇动翅膀，', '向空中撒布剧毒的鳞粉。']),
    'Caterpie': ('毛毛虫宝可梦', ['短脚的尖端有吸盘，能不知疲倦地', '爬上陡坡和墙壁。']),
    'Chansey': ('蛋宝可梦', ['非常罕见的宝可梦，据说能得到它的人', '就能获得幸福。']),
    'Charizard': ('火焰宝可梦', ['喷出的火焰炽热得能熔化岩石。', '据说有时会无意间引发森林火灾。']),
    'Charmander': ('蜥蜴宝可梦', ['显然喜欢炎热的地方。据说下雨时，', '它的尾巴尖端会喷出热气。']),
    'Charmeleon': ('火焰宝可梦', ['当它甩动燃烧的尾巴时，', '周围的温度会高得让人难以忍受。']),
    'Clefable': ('妖精宝可梦', ['胆小的妖精宝可梦，很少露面。', '一察觉到人的气息就会逃走躲起来。']),
    'Clefairy': ('妖精宝可梦', ['既神奇又可爱，仰慕者众多。', '非常稀有，只在特定的地方栖息。']),
    'Cloyster': ('双壳贝宝可梦', ['受到攻击时，会把尖刺快速连发射出。', '谁也没见过它身体的内部。']),
    'Cubone': ('孤独宝可梦', ['它从不摘下头骨头盔，所以谁也没见过', '这张真面目。']),
    'Dewgong': ('海狮宝可梦', ['体内储存着热量。即使在寒冷的海里，', '也能以8节的速度稳定地游动。']),
    'Diglett': ('鼹鼠宝可梦', ['住在地下约1米深处，以植物根为食。', '有时会钻出地面。']),
    'Ditto': ('变身宝可梦', ['能复制敌人的基因信息，瞬间变身成', '和敌人一模一样的样子。']),
    'Dodrio': ('三头鸟宝可梦', ['用三个脑袋执行复杂的计划。', '两个头睡觉时，另一个头保持清醒。']),
    'Doduo': ('双头鸟宝可梦', ['不擅长飞，却跑得飞快来弥补的鸟。', '它留下的脚印非常巨大。']),
    'Dragonair': ('龙宝可梦', ['浑身散发着温柔气息的神秘宝可梦。', '拥有改变天气的能力。']),
    'Dragonite': ('龙宝可梦', ['极其罕见的海中宝可梦。', '据说智慧不输给人类。']),
    'Dratini': ('龙宝可梦', ['长期以来被视为梦幻般的宝可梦，', '直到最近才发现水下栖息着一小群。']),
    'Drowzee': ('催眠宝可梦', ['先让敌人睡着，然后吃掉它的梦。', '有时吃了噩梦会闹肚子。']),
    'Dugtrio': ('鼹鼠宝可梦', ['三只地鼠组成的小队。钻到地下100公里', '深处时会引发大地震。']),
    'Eevee': ('进化宝可梦', ['基因构成不稳定。受到进化之石的', '辐射影响时可能会变异。']),
    'Ekans': ('蛇宝可梦', ['悄无声息地悄悄移动。会把波波和烈雀', '这样的鸟蛋整个吞下。']),
    'Electabuzz': ('电气宝可梦', ['通常在发电厂附近活动，一旦走失', '就会造成城市大停电。']),
    'Electrode': ('圆球宝可梦', ['在体内以极高的压力储存电能。', '稍受刺激就会爆炸。']),
    'Exeggcute': ('蛋宝可梦', ['常被误认为是蛋。受到惊扰时会', '迅速聚集起来蜂拥攻击。']),
    'Exeggutor': ('椰子宝可梦', ['传说偶尔会有脑袋掉下来，', '变成蛋蛋继续活下去。']),
    'Farfetchd': ('野鸭宝可梦', ['手里拿着的大葱是它的武器。', '用法和铁剑一样。']),
    'Fearow': ('嘴巴宝可梦', ['用巨大而华美的翅膀，能够一直', '停在空中，不需要落地休息。']),
    'Flareon': ('火焰宝可梦', ['在体内储存热量时，体温可以', '飙升到1600度以上。']),
    'Gastly': ('气体宝可梦', ['这种气状宝可梦几乎隐形，会悄悄', '裹住对手让它不知不觉睡着。']),
    'Gengar': ('影子宝可梦', ['在满月之夜，它喜欢模仿人的影子，', '看到人吓坏的样子就发笑。']),
    'Geodude': ('岩石宝可梦', ['栖息在原野和山地。常被误认为是石块，', '被人踩到或绊倒。']),
    'Gloom': ('杂草宝可梦', ['从嘴里流出的液体不是口水。', '是用来引诱猎物的蜜汁。']),
    'Golbat': ('蝙蝠宝可梦', ['一旦开始吸食，就算重得飞不起来，', '也不会停止吸取猎物的能量。']),
    'Goldeen': ('金鱼宝可梦', ['尾鳍舒展如优雅的舞裙，', '因此被称为水中女王。']),
    'Golduck': ('鸭宝可梦', ['常在湖边优雅地游泳，', '经常被误认成河童。']),
    'Golem': ('百万吨宝可梦', ['岩石般的身体极其坚硬，', '就算被炸药爆破也毫发无伤。']),
    'Graveler': ('岩石宝可梦', ['靠滚下斜坡来移动。不管遇到什么障碍', '都不会减速或改变方向，径直碾过。']),
    'Grimer': ('污泥宝可梦', ['出现在肮脏的地方，靠吸食工厂', '排出的污染污泥为生。']),
    'Growlithe': ('小狗宝可梦', ['领地意识很强。会对入侵自己地盘的', '家伙又咬又叫，毫不留情。']),
    'Gyarados': ('凶恶宝可梦', ['很少出现在野外。巨大而凶暴，', '暴怒时能将整个城市夷为平地。']),
    'Haunter': ('气体宝可梦', ['能穿墙而过，据说它是来自', '异次元的存在。']),
    'Hitmonchan': ('拳击宝可梦', ['看似一动不动，实则打出了肉眼', '无法看清的闪电般连环拳。']),
    'Hitmonlee': ('踢腿宝可梦', ['着急时腿会不断伸长，迈开超长的', '步伐轻快地奔跑。']),
    'Horsea': ('龙宝可梦', ['据说能在水面上精准地喷出墨汁，', '击落飞行中的虫子。']),
    'Hypno': ('催眠宝可梦', ['和敌人四目相对时，会混合使用', '催眠术和念力等超能力招式。']),
    'Ivysaur': ('种子宝可梦', ['当背上的花苞长大后，', '似乎就无法用后腿站立了。']),
    'Jigglypuff': ('气球宝可梦', ['大眼睛一亮，就会唱起不可思议的', '摇篮曲，让敌人沉沉睡去。']),
    'Jolteon': ('闪电宝可梦', ['聚集大气中的负离子，', '放出1万伏的雷电。']),
    'Jynx': ('人形宝可梦', ['走路时妖艳地扭动腰肢，', '能让人不由自主地和它共舞。']),
    'Kabuto': ('贝壳宝可梦', ['从远古海底地层出土的化石中', '复活的宝可梦。']),
    'Kabutops': ('贝壳宝可梦', ['流线型的身体非常适合游泳。', '用利爪撕裂猎物，吸食体液。']),
    'Kadabra': ('念力宝可梦', ['身体会发出特殊的阿尔法波，', '靠近它就会头痛。']),
    'Kakuna': ('蛹宝可梦', ['几乎无法动弹的宝可梦，只能', '硬壳自保，躲避天敌。']),
    'Kangaskhan': ('亲子宝可梦', ['幼崽在3岁之前，几乎从不离开', '妈妈温暖的育儿袋。']),
    'Kingler': ('铁钳宝可梦', ['大钳子有1万马力的夹碎力。', '不过太大太重，用起来并不灵活。']),
    'Koffing': ('毒气宝可梦', ['体内储存着好几种毒气，', '随时可能毫无预兆地爆炸。']),
    'Krabby': ('河蟹宝可梦', ['钳子不仅是强力武器，', '横着走路时还能用来保持平衡。']),
    'Lapras': ('搭载宝可梦', ['因被滥捕而濒临灭绝的宝可梦。', '它能载着人渡过水面。']),
    'Lickitung': ('舔舐宝可梦', ['舌头能像变色龙一样伸长。', '被它舔到会又麻又痒。']),
    'Machamp': ('怪力宝可梦', ['挥动结实的肌肉打出重拳，', '能把对手打飞到地平线之外。']),
    'Machoke': ('怪力宝可梦', ['肌肉发达得惊人，必须戴着力量腰带', '才能控制自己的动作。']),
    'Machop': ('怪力宝可梦', ['热衷于锻炼肌肉。为了变得更强，', '修习各种流派的武术。']),
    'Magikarp': ('鱼宝可梦', ['在遥远的过去，它比如今这些弱得', '不像话的后代要强一些。']),
    'Magmar': ('喷火宝可梦', ['全身总是燃烧着橙色的光，', '藏身火焰中时完全无法分辨。']),
    'Magnemite': ('磁铁宝可梦', ['靠反重力悬浮在空中。会突然出现，', '使用电磁波之类的招式。']),
    'Magneton': ('磁铁宝可梦', ['由几只小磁怪连在一起构成。', '太阳黑子活跃时经常出现。']),
    'Mankey': ('猪猴宝可梦', ['极易发怒。前一秒还温顺得很，', '下一秒就暴跳如雷。']),
    'Marowak': ('爱护宝可梦', ['手中的骨头是它的主武器。它会像', '回力镖一样娴熟地掷出击倒对手。']),
    'Meowth': ('妖怪猫宝可梦', ['痴迷圆形的东西。每晚在街上', '游荡，寻找掉落的硬币。']),
    'Metapod': ('蛹宝可梦', ['外壳还软的时候，柔弱的身体', '暴露在外，最怕受到攻击。']),
    'Mew': ('新种宝可梦', ['太过稀有，许多专家仍说它只是幻影。', '全世界亲眼见过它的人寥寥无几。']),
    'Mewtwo': ('遗传宝可梦', ['科学家经过多年可怕的基因重组', '和DNA改造实验创造出的宝可梦。']),
    'Moltres': ('火焰宝可梦', ['传说中的火之鸟。每次扇动翅膀，', '都会迸发出耀眼的火焰。']),
    'MrMime': ('屏障宝可梦', ['演哑剧时被打断的话，', '会用宽大的手掌猛扇打扰者。']),
    'Muk': ('污泥宝可梦', ['全身裹着肮脏恶心的浓稠污泥，', '毒性极强，连脚印都带毒。']),
    'Nidoking': ('钻鼬宝可梦', ['战斗时用有力的尾巴猛击、缠住猎物，', '再将其骨头折断。']),
    'Nidoqueen': ('钻鼬宝可梦', ['坚硬的鳞片提供强有力的防护。', '凭借厚重的身躯施展出强力招式。']),
    'NidoranF': ('毒针宝可梦', ['体型虽小，但带毒的棘刺让它十分危险。', '雌性的角比较小。']),
    'NidoranM': ('毒针宝可梦', ['竖起耳朵监听危险。角越大，', '分泌的毒液就越强。']),
    'Nidorina': ('毒针宝可梦', ['雌性的角长得慢。更擅长爪击和', '撕咬这样的物理攻击。']),
    'Nidorino': ('毒针宝可梦', ['好斗的宝可梦，出手迅捷。', '头上的角分泌着剧毒。']),
    'Ninetales': ('狐狸宝可梦', ['非常聪明，而且记仇。要是抓住它的', '尾巴，可能会被诅咒1000年。']),
    'Oddish': ('杂草宝可梦', ['白天把脸埋进土里。夜里四处游荡，', '播撒自己的种子。']),
    'Omanyte': ('螺旋宝可梦', ['早已灭绝，但在极少数情况下，', '能从化石中基因复活。']),
    'Omastar': ('螺旋宝可梦', ['史前宝可梦，因沉重的壳让它', '抓不到猎物而灭绝。']),
    'Onix': ('岩蛇宝可梦', ['随着成长，身体的岩石部分会越来越硬，', '变得如钻石一般，只是呈黑色。']),
    'Paras': ('蘑菇宝可梦', ['钻进土里吸食树根。背上的蘑菇靠着', '吸取宿主虫子的养分生长。']),
    'Parasect': ('蘑菇宝可梦', ['宿主与寄生者的组合，寄生蘑菇已经', '控制了宿主虫子。喜欢潮湿的地方。']),
    'Persian': ('高贵猫宝可梦', ['皮毛仰慕者众多，但它任性又记仇，', '想当宠物养可不容易。']),
    'Pidgeot': ('鸟宝可梦', ['捕猎时高速掠过水面，', '叼走大意的鲤鱼王等猎物。']),
    'Pidgeotto': ('鸟宝可梦', ['领地意识极强，对闯入领地的', '家伙会毫不留情地猛啄。']),
    'Pidgey': ('小鸟宝可梦', ['森林里常见的鸟。会贴着地面扇动', '翅膀，掀起迷眼的沙尘。']),
    'Pikachu': ('鼠宝可梦', ['几只皮卡丘聚在一起时，电荷会', '积蓄起来，引发雷电交加的风暴。']),
    'Pinsir': ('锹形虫宝可梦', ['如果没能用大颚夹碎猎物，', '就会把它抡起来狠狠摔出去。']),
    'Poliwag': ('蝌蚪宝可梦', ['刚长出的腿还跑不快。比起站立，', '它似乎更喜欢游泳。']),
    'Poliwhirl': ('蝌蚪宝可梦', ['既能在水里也能在陆地生活。', '在陆地上时会出汗保持身体湿润。']),
    'Poliwrath': ('蝌蚪宝可梦', ['自由泳和蛙泳都很在行，', '轻松超越人类顶尖游泳选手。']),
    'Ponyta': ('火马宝可梦', ['蹄子的硬度是钻石的10倍，', '眨眼间就能把任何东西踩扁。']),
    'Porygon': ('虚拟宝可梦', ['完全由程序代码构成的宝可梦，', '能在赛博空间里自由移动。']),
    'Primeape': ('猪猴宝可梦', ['永远怒气冲冲，而且异常执着。', '不追到猎物绝不罢休。']),
    'Psyduck': ('鸭宝可梦', ['用呆滞的眼神让敌人放松警惕，', '趁机使出意念力。']),
    'Raichu': ('鼠宝可梦', ['长尾巴充当接地线，', '防止被自己的高压电所伤。']),
    'Rapidash': ('火马宝可梦', ['胜负心极强，只要看到跑得快的东西', '就会追上去，想和它比个高低。']),
    'Raticate': ('鼠宝可梦', ['靠胡须保持平衡。据说胡须被剪掉', '后速度就会变慢。']),
    'Rattata': ('鼠宝可梦', ['攻击时逮什么咬什么。体型小、速度', '快，在许多地方都很常见。']),
    'Rhydon': ('钻角宝可梦', ['表皮如铠甲般坚固，', '能栖身在3600度的熔岩里。']),
    'Rhyhorn': ('尖刺宝可梦', ['粗壮的骨骼硬度是人类骨头的1000倍。', '能轻松把拖车撞飞。']),
    'Sandshrew': ('鼠宝可梦', ['在远离水源的干旱地带深挖地洞。', '只有觅食时才会钻出地面。']),
    'Sandslash': ('鼠宝可梦', ['遇到危险就蜷成刺球。还能保持着', '蜷缩的姿态滚动，攻击或逃跑。']),
    'Scyther': ('螳螂宝可梦', ['身手如忍者般敏捷，能让人产生', '它分身有术的错觉。']),
    'Seadra': ('龙宝可梦', ['高速扇动翅膀般的胸鳍和粗壮的尾巴，', '能够倒退着游泳。']),
    'Seaking': ('金鱼宝可梦', ['秋天的产卵季，能看到它们奋力', '溯流而上的身影。']),
    'Seel': ('海狮宝可梦', ['头上突出的角非常坚硬，', '用来撞穿厚厚的冰层。']),
    'Shellder': ('双壳贝宝可梦', ['坚硬的外壳能弹开任何攻击。', '只有张壳的瞬间才有破绽。']),
    'Slowbro': ('寄居蟹宝可梦', ['咬住呆呆兽尾巴的刺甲贝，据说靠', '吸食宿主吃剩的食物为生。']),
    'Slowpoke': ('迟钝宝可梦', ['迟钝得不可思议。受到攻击后，', '要过5秒才感觉到疼。']),
    'Snorlax': ('摄眠宝可梦', ['非常懒惰，只知道吃和睡。肚子越', '大，就变得越不爱动。']),
    'Spearow': ('小鸟宝可梦', ['在草丛里捕食虫子。翅膀短小，', '必须高速扇动才能待在空中。']),
    'Squirtle': ('小龟宝可梦', ['出生后背部隆起变硬，形成龟壳。', '能从嘴里有力地喷出泡沫。']),
    'Starmie': ('神秘宝可梦', ['中心的核心闪烁着彩虹七色。', '有人把它当作宝石看待。']),
    'Staryu': ('星形宝可梦', ['充满谜团的宝可梦。战斗中失去的', '肢体能毫不费力地再生。']),
    'Tangela': ('藤蔓宝可梦', ['全身缠满海带般宽大的藤蔓。', '走动时藤蔓会随之摇动。']),
    'Tauros': ('野牛宝可梦', ['锁定敌人后就狂暴冲锋，', '同时用长鞭似的尾巴抽打自己。']),
    'Tentacool': ('水母宝可梦', ['漂流在浅海。不小心钓到它的钓鱼人，', '常被它刺人的毒液惩罚。']),
    'Tentacruel': ('水母宝可梦', ['触手平时收得很短。捕猎时会伸长，', '缠住猎物使其动弹不得。']),
    'Vaporeon': ('泡沫宝可梦', ['栖息在水边。长尾巴上长着鳍状的', '突起，常被误认为美人鱼。']),
    'Venomoth': ('毒蛾宝可梦', ['翅膀上覆盖的鳞粉颜色不同，', '代表的毒性也不同。']),
    'Venonat': ('昆虫宝可梦', ['住在高大树木的树荫下，捕食虫子。', '夜里会被灯光吸引。']),
    'Venusaur': ('种子宝可梦', ['吸收太阳能时背上的花会绽放。', '为了寻找阳光而不断迁徙。']),
    'Victreebel': ('捕蝇草宝可梦', ['据说栖息在丛林深处的大群落里。', '但从没有人从那里活着回来。']),
    'Vileplume': ('花宝可梦', ['花瓣越大，花粉的毒性就越强。', '大脑袋沉甸甸的，抬起来很吃力。']),
    'Voltorb': ('圆球宝可梦', ['通常出现在发电厂。常被误认为', '精灵球，电到过许多人。']),
    'Vulpix': ('狐狸宝可梦', ['刚出生时只有一条尾巴。随着长大，', '尾巴会从尖端开始分叉。']),
    'Wartortle': ('海龟宝可梦', ['常藏在水里偷袭大意的猎物。', '游泳时会摆动耳朵来保持平衡。']),
    'Weedle': ('毛毛虫宝可梦', ['常在森林里啃食树叶。', '头顶的毒针尖锐有毒。']),
    'Weepinbell': ('捕蝇草宝可梦', ['先喷出毒粉让敌人动弹不得，', '再用溶解液收尾。']),
    'Weezing': ('毒气宝可梦', ['两种毒气相遇的地方，两只瓦斯弹', '经过长年累月会融合成双弹瓦斯。']),
    'Wigglytuff': ('气球宝可梦', ['身体柔软富有弹性。一旦被惹怒，', '就会吸气把自己胀得巨大。']),
    'Zapdos': ('电气宝可梦', ['传说中的鸟宝可梦，据说会伴随', '巨大的落雷从云中出现。']),
    'Zubat': ('蝙蝠宝可梦', ['在常年黑暗的地方群居。用超声波', '锁定并接近目标。']),
}


def has_cjk(s: str) -> bool:
    return any(0x2E80 <= ord(c) <= 0x9FFF for c in s)


def validate():
    """Full-coverage validation: 151/151 species, one ZH page per EN page."""
    files = sorted(POKEMON_DIR.glob('*.json'))
    names = [f.stem for f in files]
    missing = sorted(set(ZH) - set(names))
    extra = sorted(set(names) - set(ZH))
    if missing:
        print('MISSING ZH ENTRIES:')
        for m in missing:
            print(' ', m)
    if extra:
        print('SPECIES WITHOUT ZH DATA (151 snapshot mismatch?):')
        for e in extra:
            print(' ', e)
    problems = []
    for name in sorted(set(ZH) & set(names)):
        cat, pages = ZH[name]
        if not 1 <= len(cat) <= 7 or not has_cjk(cat):
            problems.append(f'{name}: bad category {cat!r}')
        data = json.loads((POKEMON_DIR / f'{name}.json').read_text())
        en_pages = data['pokedex']['flavorTextPages']
        if len(pages) != len(en_pages):
            problems.append(
                f'{name}: {len(pages)} zh pages vs {len(en_pages)} en pages')
        for i, p in enumerate(pages):
            if not p.strip() or not has_cjk(p):
                problems.append(f'{name}: zh page {i} empty or not Chinese')
    if problems:
        print('DATA PROBLEMS:')
        for p in problems:
            print(' ', p)
    if missing or extra or problems:
        raise SystemExit(1)


def inject():
    for name, (cat, pages) in sorted(ZH.items()):
        path = POKEMON_DIR / f'{name}.json'
        data = json.loads(path.read_text(), object_pairs_hook=OrderedDict)
        dex = data['pokedex']
        if len(pages) != len(dex['flavorTextPages']):
            raise SystemExit(f'{name}: page count changed under our feet')
        # Rebuild the pokedex object so the ZH fields sit right after their
        # EN counterparts (categoryZh after category, flavorTextPagesZh
        # after flavorTextPages).
        new_dex = OrderedDict()
        for k, v in dex.items():
            new_dex[k] = v
            if k == 'category':
                new_dex['categoryZh'] = cat
            elif k == 'flavorTextPages':
                new_dex['flavorTextPagesZh'] = pages
        if 'categoryZh' not in new_dex or 'flavorTextPagesZh' not in new_dex:
            raise SystemExit(f'{name}: pokedex block missing expected keys')
        data['pokedex'] = new_dex
        path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')
    print(f'Injected zh pokedex data for {len(ZH)}/151 species.')


def main():
    validate()
    inject()


if __name__ == '__main__':
    main()
