/* Display translations only. Recorded state, requests, answers and video are immutable. */
(function () {
  const english = new URLSearchParams(location.search).get('lang') === 'en';
  const labels = {
    'Jev 完整通关 · 同步决策大盘': 'Jev full playthrough · synchronized decision dashboard',
    '脚本与 Jev：完整通关对比播放器': 'Script vs. Jev: full playthrough comparison',
    '脚本自动化 × Jev 自主探索：完整通关': 'Scripted automation × Jev exploration: full playthroughs',
    '从真实 NEW GAME 到名人堂、片尾及独立读档。右侧同步显示目标、候选、操作与执行器实际输入；视频下方分别展示策略层和动作层最近一次调用的输入快照，可展开完整原文；关键节点的文字是依据日志写成的复盘注解。可变速观看，暂停后仔细比较。': 'Follow a real NEW GAME through the Hall of Fame, ending, and a separate save reload. The dashboard shows goals, candidates, actions, and recorded executor inputs. Below the video, inspect the latest strategy and action input snapshots or expand the original requests. Commentary at key moments is based on the logs. Change playback speed or pause to compare the details.',
    '两边均从真实 NEW GAME 开始，直到名人堂、结局自动存档和独立进程 CONTINUE。按共同剧情节点观看；每章先完成的一侧使用章末最近的清晰帧定格等候。': 'Both runs start with a real NEW GAME and reach the Hall of Fame, ending autosave, and CONTINUE in a separate process. Chapters align shared story milestones; the side that finishes first waits on a clear frame near its chapter end.',
    '速度': 'Speed', '上一章': 'Previous chapter', '下一章': 'Next chapter', '剧情章节': 'Story chapter',
    '播放': 'Play', '暂停': 'Pause', '自动下一章': 'Auto-advance chapters', '本章模拟时间': 'Chapter simulation time',
    '关键解说': 'Key commentary', '选择一个节点并暂停': 'Jump to a moment and pause',
    '跳转关键节点': 'Jump to a key moment', '跳转关键节点并暂停': 'Jump to a key moment and pause',
    '回到 NEW GAME': 'Back to NEW GAME', '打开完整原片': 'Open full original recording',
    '脚本（适配与恢复修复）': 'Script (compatibility and recovery fixes)',
    '策略层 + 动作层 Jev': 'Jev at the strategy and action layers',
    '脚本完整录像': 'Full scripted recording', 'Jev 完整录像': 'Full Jev recording',
    '单独打开脚本完整原片': 'Open full scripted recording', '单独打开 Jev 完整原片': 'Open full Jev recording',
    '观看提示': 'Viewing guide', '将目标选择与实际结果放在一起看': 'Compare the selected goal with what happened',
    '对照策略与实际结果': 'Compare decisions with their outcomes',
    '右侧记录不包含模型的隐藏推理。选择分布表示模型如何分配候选概率，不是战斗胜率或通关成功率。': 'These records do not contain hidden model reasoning. The distribution shows probability assigned to each choice, not the chance of winning a battle or completing the game.',
    '右侧展示 Jev 的记录；候选分布不是成功率，文字解说不是模型自述。': 'The dashboard displays Jev records. Choice probabilities are not success rates, and the commentary is retrospective analysis, not a model explanation.',
    '时间轴与证据口径': 'Timeline and evidence', '录像与比较口径': 'Recordings and comparison methodology',
    '视频按 60 fps 保存全部模拟更新帧；未推进游戏帧的网络等待另见墙钟统计。日志起始时钟经 776 场战斗的共同帧记录校验对齐。同一游戏帧上的多次判断显示最近一次，完整墙钟顺序保留在日志中。策略与动作输入有各自的快照时间，不以当前游戏状态补齐缺失字段。底层输入按命令完成后的下一次观测帧对齐，并非精确的按键按下动画；skip_dialogue 是实际调试命令，未伪装成 A 键。动作包含模型选择、判断缓存和通用技能执行；只在游戏状态实际变化后确认结果。最后约 10 秒是独立进程读取结局自动存档。': 'The video preserves every simulation update frame at 60 fps. Network waits that do not advance the game are reported separately in wall-clock statistics. The log offset was checked against shared frame records from 776 battles. When several judgments occur on one game frame, the latest is shown; the logs preserve their full wall-clock order. Strategy and action inputs retain their own snapshot times; missing fields are not filled from the current game state. Executor inputs align to the next observation after command completion, not the exact button-down animation. skip_dialogue is a recorded debug command, not an inferred A press. Actions distinguish model decisions, cached judgments, and general-purpose skills; outcomes are confirmed from actual state changes. The final roughly 10 seconds show a separate process loading the ending autosave.',
    '本轮是白盒探索：Jev 读取文本状态与游戏脚本事实，不通过视频识别游戏画面。面板中文名称是对原始目标和操作的显示翻译，选择与概率来自实际日志。所有复盘注解都与模型实际输出分开。': 'This is white-box exploration: Jev reads text state and game-script facts, not video frames. Panel labels are display translations of the original goals and actions; choices and probabilities come from actual logs. Retrospective commentary is separate from model output. Expanded request and response records retain their original wording.',
    '原片保存每个模拟更新帧，以 60 fps 回放；这里的时钟是游戏模拟帧时间。Jev 的网络等待与 Python 规划耗时见实验日志和复盘表格。': 'Original recordings preserve every simulation update frame at 60 fps. The clocks here measure simulated game time. Jev network waits and Python planning time are reported in the experiment logs and retrospective.',
    '两边使用相同冻结版本和 seed 42。保留各自默认队伍选择和时间模式：脚本为默认实时循环，Jev 为 driven-only。': 'Both runs use the same frozen build and seed 42, retaining their default party choices and clock modes: the script uses the real-time loop; Jev advances frames only through commands.',
    '旧脚本曾停在新增 HM 菜单，并在西尔佛劲敌及科拿战败退后退出。最终版适配道具菜单，补齐西尔佛最多四次挑战及后续三座道馆最多六次挑战，将智能战斗治疗阈值调整为 40%，并允许最多五次联盟败退重试。森林败退时提前执行原定的 13 级训练，保留原剧情路线和队伍成员，从 NEW GAME 重新录制；原始失败及人工修复证据见复盘。': 'The earlier script stalled at a new HM menu and exited after losses to the Silph rival and Lorelei. The recorded version adapts to the item menu, allows up to four Silph-rival attempts and six attempts at each of three later Gyms, lowers the smart-battle healing threshold to 40%, and allows up to five League restarts after blackouts. A forest blackout brings forward the planned level-13 training. The story route and party members are retained, and the run was recorded again from NEW GAME. The retrospective documents the original failures and manual fixes.',
    '每章保持相同播放倍速；各自路线的时间顺序不变。中段按“五枚徽章”对齐，允许第四、第五道馆次序不同。': 'Both sides use the same playback speed within a chapter, preserving each route in chronological order. The middle section aligns at five badges, allowing different orders for the fourth and fifth Gyms.',
    '最后一章来自各自新启动的独立进程，用来验证结局自动存档可继续游戏；它不属于原游戏进程的连续画面。': 'The final chapter comes from a newly launched process for each run, verifying that the ending autosave can be continued. It is not continuous footage from the original game process.',
    '一对运行只能提供案例认知；不同初始精灵、模型随机性、录制开销和并行主机负载均影响结果。': 'One pair of runs supports a case study, not a general ranking. Different starters, model randomness, recording overhead, and concurrent host load can affect the results.',
    '整体复盘': 'Full retrospective (Chinese)', '阅读整体复盘': 'Full retrospective (Chinese)',
    '同步数据与时钟校验': 'Synchronized data and clock checks', '录像时间轴与 SHA-256': 'Recording timeline and SHA-256', '运行统计': 'Run statistics',
    '· 完整原片和本页应保留在同一目录，可直接用浏览器打开。': '· Keep this page and its original recording together to open them locally in a browser.',
    '无法载入原片。请将本页与 jev-full.mp4 保留在同一目录。': 'Could not load the recording. Keep this page and jev-full.mp4 in the same directory.',
    '原片加载失败。请保留本页与两个 full.mp4 文件在同一目录，或通过支持 Range 请求的本地 HTTP 服务打开。': 'Could not load the recordings. Keep this page and both full.mp4 files together, or serve them with a local HTTP server that supports Range requests.',
    'JEV 决策与执行记录': 'JEV DECISIONS & EXECUTION',
    '游戏当前状态 · 区别于下方决策输入快照': 'Current game state · separate from the decision input snapshots below',
    '策略层 · 最近一次选择': 'Strategy layer · latest choice',
    '候选前三项 · 选择分布，不是成功率': 'Top three candidates · choice probabilities, not success rates',
    '分布非成功率 · 本次由代码从非拒选项中选择': 'Not success rates · the controller selected a non-abstention option',
    '动作层 / 通用技能执行': 'Action layer / general-purpose skills',
    '执行器最近输入 · 按观测帧对齐': 'Latest executor input · aligned to an observed frame',
    '已观察到的执行结果': 'Observed execution result',
    '标题画面': 'Title screen', '主菜单': 'Main menu', '版权画面': 'Copyright screen',
    '名人堂已记录': 'Hall of Fame recorded', '独立读档验证': 'Separate-process save reload', '结局流程': 'Ending sequence',
    '从 NEW GAME 开始': 'Starting from NEW GAME', '尚未领取初始宝可梦': 'No starter Pokémon yet',
    '等待首次策略选择': 'Waiting for the first strategy decision', '尚无模型判断': 'No model judgment yet',
    '开机与新建游戏': 'Boot and NEW GAME', '动作层 · Jev 选择，由技能执行': 'Action layer · Jev choice, executed by a skill',
    '动作层 · 复用已缓存的 Jev 选择': 'Action layer · cached Jev choice', '通用技能 · 代码选择合法回合': 'General-purpose skill · legal turn selected by code',
    '尚无已完成输入': 'No completed input yet', '命令成功': 'Command succeeded', '命令失败': 'Command failed',
    '不代表持续按住': 'Not a continuous button hold', '通用技能负责按键、导航与推进对白': 'Skills handle buttons, navigation, and dialogue advancement',
    '冠军目标已完成': 'Champion objective complete', '等待首个操作结果': 'Waiting for the first action outcome',
    '新进程载入自然结局存档': 'A new process loaded the natural ending autosave', '以游戏状态变化确认进度': 'Progress is confirmed from changes in game state',
    '状态与选择均来自记录': 'State and decisions are recorded', '输入文件加载失败，请保留 jev-inputs-data.js。': 'Could not load the inputs. Keep jev-inputs-data.js alongside this page.',
    '展开当次完整输入、全部候选与输出': 'Expand the original input, all candidates, and output',
    '应用层调用原文：state 是状态，question 是指令与全部候选，answer 是模型输出；controller_selection 如存在，表示代码二次选择。此处不包含模型隐藏推理。': 'Original application request: state contains the input state, question contains instructions and all candidates, and answer contains model output. If present, controller_selection records a subsequent choice made by code. These records are not translated and do not contain hidden model reasoning.',
    '该层尚未发生模型调用。': 'No model call at this layer yet.', '该层尚未发生模型调用': 'No model call at this layer yet',
    '只展示实际传入信息，不以当前状态补齐': 'Only information actually sent; no fields filled from current state',
    '已暂停，可查看选择细节': 'Paused; inspect the decision details', '播放中': 'Playing', '已暂停': 'Paused',
    '章末清晰帧 · 定格等候': 'Clear chapter-end frame · waiting', '模型等待另计': 'Model waiting time is accounted for separately',
    '尚无队伍': 'No party yet',
    '位置': 'Location', '队伍': 'Party', '主力 PP': 'Lead Pokémon PP', '资源': 'Resources', '代价': 'Cost',
    '选中路线': 'Selected route', '地形': 'Terrain', '导航': 'Navigation', '目标': 'Objectives', '子目标': 'Subgoal',
    '对白': 'Dialogue', '选项': 'Options', '脚本事实': 'Script facts', '输入边界': 'Input scope', '对战': 'Matchup',
    '血量分档': 'HP bands', '可用攻击': 'Available attacks', '计算信息': 'Calculated data', '数值来源': 'Value sources',
    '宝可梦': 'Pokémon', '新招式': 'New move', '已有招式': 'Current moves', '效果': 'Effects', '候选': 'Candidates',
    '对手': 'Opponent', '约束': 'Constraint', '输入': 'Input', '比较': 'Comparison',
    '剧情策略': 'Story strategy', '操作选择': 'Action selection', '对话选项': 'Dialogue choice', '菜单选择': 'Menu choice',
    '战斗选招': 'Battle move selection', '学习招式': 'Move learning', '受限战斗': 'Constrained battle', '战斗换人': 'Battle switch',
    '尚无宝可梦': 'No Pokémon yet', '治疗路线会重置已获胜战斗；完整列表可展开': 'The healing route resets battles already won; expand for the full list',
    '局部状态与候选效果；完整字段可展开': 'Local state and candidate effects; expand for all fields',
    '此次未提供对白': 'No dialogue supplied in this call', '此次未提供': 'Not supplied in this call',
    '此类调用未重复传入完整队伍与地形': 'These calls do not resend the full party and terrain',
    '威力、命中、属性倍率、本系、暴击、PP 档位': 'Power, accuracy, type effectiveness, STAB, critical hits, and PP bands',
    '基础种族值与代码计算的期望威力': 'Base species stats and expected power calculated by code',
    'HP/PP 为分档；不含整张地图或逐帧图像': 'HP/PP are bands; no full map or frame-by-frame images',
    '新旧招式的威力、属性、命中、PP 与效果': 'Power, type, accuracy, PP, and effects of the new and current moves',
    '保留原招式，或替换允许遗忘的招式': 'Keep the current moves, or replace a move that can be forgotten',
    '双方当前战斗状态与合法操作候选': 'Current battle state for both sides and legal action candidates',
    '候选携带可出战成员的等级、HP、招式、PP': 'Candidates include each available member’s level, HP, moves, and PP',
    '候选还包含对该对手可用的有效攻击': 'Candidates also include effective attacks against this opponent',
    '未知': 'Unknown', '受伤': 'Hurt', '健康': 'Healthy', '低血量': 'Low HP',
    '恢复队伍 HP、状态与 PP': 'Restore party HP, status, and PP', '调整移动方式并通过目标位置': 'Change travel mode and pass the target location',
    '解决可见人物或挡路对象的条件': 'Meet the conditions for a visible character or obstruction',
    '触发博士出场': 'Trigger Professor Oak’s appearance', '选择初始宝可梦': 'Choose a starter Pokémon',
    '完成研究所劲敌战': 'Complete the rival battle in Oak’s Lab', '领取图鉴': 'Receive the Pokédex',
    '挑战小刚': 'Challenge Brock', '挑战小霞': 'Challenge Misty', '挑战马志士': 'Challenge Lt. Surge', '挑战莉佳': 'Challenge Erika',
    '挑战阿桔': 'Challenge Koga', '挑战娜姿': 'Challenge Sabrina', '挑战夏伯': 'Challenge Blaine', '挑战坂木': 'Challenge Giovanni',
    '挑战科拿': 'Challenge Lorelei', '挑战希巴': 'Challenge Bruno', '挑战菊子': 'Challenge Agatha', '挑战渡': 'Challenge Lance',
    '击败冠军，进入名人堂': 'Defeat the Champion and enter the Hall of Fame', '解决幽灵嘎啦嘎啦阻碍': 'Resolve the ghost Marowak encounter',
    '交还金牙，准备怪力': 'Return the Gold Teeth to obtain Strength', '唤醒挡路的卡比兽': 'Wake the blocking Snorlax',
    '协助正辉恢复原状': 'Help restore Bill to human form', '推动石块，开启冠军之路机关': 'Push boulders to activate Victory Road switches',
    '调整宝可梦屋开关': 'Operate the Pokémon Mansion switches', '解除当前门禁或机关': 'Unlock the current door or mechanism',
    '击败前置路线上的训练家': 'Defeat a prerequisite trainer', '完成房间入口剧情': 'Complete the room-entry sequence',
    '推进当前剧情前置条件': 'Advance a story prerequisite', '完成当前准备目标': 'Complete the current preparation goal',
    '在野外寻找遭遇并训练': 'Find wild encounters and train', '与目标人物交谈': 'Talk to the target character',
    '调查目标位置': 'Inspect the target location', '移动到目标站位': 'Move to the target position',
    '按已规划步骤推石解谜': 'Solve the boulder puzzle using the planned steps', '使用招式机器学习招式': 'Teach a move using a TM/HM',
    '等待剧情交还控制': 'Wait for the scripted sequence to return control', '暂不选择': 'Abstain',
    '本回合攻击，保留药品': 'Attack this turn and conserve medicine', '保留现有招式': 'Keep the current moves',
    '调整学习的新招式': 'Choose which move to learn', '确认': 'Confirm', '取消': 'Cancel', '选择可用操作': 'Choose an available action',
    '目标效果已确认': 'Intended effect confirmed', '操作完成，继续检查目标': 'Action complete; checking the goal',
    '本场获胜': 'Battle won', '本场败退': 'Battle lost',
    '推进对白：skip_dialogue（调试命令）': 'Advance dialogue: skip_dialogue (debug command)',
    '首个道馆：战败后重新准备': 'First Gym: preparing again after defeat',
    '小火龙 15 级挑战小刚失败。代码提出训练至 17 级的候选；Jev 随后选择训练并再次挑战。': 'The level-15 Charmander lost to Brock. Code offered training to level 17 as a candidate; Jev then chose to train and challenge him again.',
    '小霞阶段：能恢复，但准备成本较高': 'Misty: recovery works, but preparation is costly',
    '本轮已在小霞处败退三次。主要补救是提升主力等级；当前候选空间缺少完整的针对性捕获、组队和培养方案。': 'This run has lost to Misty three times. Recovery mainly raises the lead Pokémon’s level; the candidate set lacks a complete plan for targeted catching, team building, and training.',
    '八徽章之后，折返准备怪力': 'After eight badges: returning to prepare Strength',
    '此前已进过狩猎地带取得冲浪，这次再寻找金牙。可改进的是顺路合并前置准备，而不是将中间完成道馆的时间都视为浪费。': 'The run had already visited the Safari Zone for Surf and now returns for the Gold Teeth. Combining prerequisite errands would help; the intervening time spent completing Gyms should not all be counted as waste.',
    '三连胜后撤退：应往前追问准备': 'Retreat after three wins: examine the earlier preparation',
    '喷火龙仅剩 4/210 HP，喷射火焰 1 PP。模型已收到“治疗会重置三场胜利”的提示；此刻撤退有依据，问题在连战前的资源准备。': 'Charizard has only 4/210 HP and 1 PP left for Flamethrower. The model was told that healing would reset three wins. Retreat is defensible here; the issue is resource preparation before the battle sequence.',
    '购买候选存在，补给仍未落实': 'Supplies were offered, but never purchased',
    '低血量败退后，候选已有全满药和全满回复药，Jev 仍选择挑战。神奇糖果也被算进 medicine，影响了代码的“无恢复物资”判断。': 'After a low-HP defeat, Full Restore and Max Potion purchases were already candidates, but Jev still chose another challenge. Rare Candy was also counted as medicine, affecting the code’s check for missing recovery supplies.',
    '复盘注解 · 依据日志，非模型自述': 'Retrospective commentary · based on logs, not a model explanation',
    '真实 NEW GAME': 'Real NEW GAME', '第一枚徽章': 'First badge', '第二枚徽章': 'Second badge', '三枚徽章': 'Three badges',
    '五枚徽章': 'Five badges', '八枚徽章': 'Eight badges', '进入四天王挑战': 'Enter the Elite Four challenge',
    '冠军胜利与名人堂': 'Champion victory and Hall of Fame', '字幕、自动存档与回到城镇': 'Credits, autosave, and return to town',
    '独立进程 CONTINUE 验证': 'Separate-process CONTINUE verification'
  };
  const names = {
    '真新镇':'Pallet Town', '主角房间':'Red’s bedroom', '主角家':'Red’s house', '大木研究所':'Oak’s Lab',
    '常青森林':'Viridian Forest', '常青市':'Viridian City', '尼比市':'Pewter City', '华蓝市':'Cerulean City',
    '枯叶市':'Vermilion City', '紫苑镇':'Lavender Town', '玉虹市':'Celadon City', '浅红市':'Fuchsia City',
    '金黄市':'Saffron City', '红莲岛':'Cinnabar Island', '石英高原':'Indigo Plateau', '联盟大厅':'Indigo Plateau lobby',
    '科拿房间':'Lorelei’s room', '希巴房间':'Bruno’s room', '菊子房间':'Agatha’s room', '渡房间':'Lance’s room',
    '冠军房间':'Champion’s room', '名人堂':'Hall of Fame', '正辉家':'Bill’s house', '富士老人家':'Mr. Fuji’s house',
    '狩猎地带秘密小屋':'Safari Zone Secret House', '狩猎地带':'Safari Zone', '冠军之路':'Victory Road',
    '月见山':'Mt. Moon', '岩山隧道':'Rock Tunnel', '宝可梦塔':'Pokémon Tower', '西尔佛公司':'Silph Co.',
    '双子岛':'Seafoam Islands', '宝可梦屋':'Pokémon Mansion', '火箭队基地':'Rocket Hideout', '圣安奴号':'S.S. Anne',
    '常青':'Viridian ', '尼比':'Pewter ', '华蓝':'Cerulean ', '枯叶':'Vermilion ', '玉虹':'Celadon ',
    '浅红':'Fuchsia ', '金黄':'Saffron ', '红莲':'Cinnabar ', '紫苑':'Lavender ',
    '宝可梦中心':'Pokémon Center', '道馆':'Gym', '商店':'Mart',
    '大木博士的包裹':'Oak’s Parcel', '贝壳化石':'Dome Fossil', '自行车兑换券':'Bike Voucher',
    '居合斩秘传机':'HM01 (Cut)', '冲浪秘传机':'HM03 (Surf)', '怪力秘传机':'HM04 (Strength)',
    '宝可梦笛':'Poké Flute', '金牙':'Gold Teeth', '秘密钥匙':'Secret Key', '钥匙卡':'Card Key', '代币盒':'Coin Case',
    '淡水':'Fresh Water', '全招式 PP 恢复道具':'Elixir', '全满回复药':'Max Potion', '全满药':'Full Restore',
    '劈开':'Slash', '居合斩':'Cut', '喷射火焰':'Flamethrower', '挖洞':'Dig', '抓':'Scratch', '火花':'Ember',
    '叫声':'Growl', '瞪眼':'Leer', '水枪':'Water Gun', '冲浪':'Surf', '怪力':'Strength', '愤怒':'Rage',
    '烟幕':'Smokescreen', '火焰旋涡':'Fire Spin', '龙之怒':'Dragon Rage', '咬住':'Bite',
    '小火龙':'Charmander', '火恐龙':'Charmeleon', '喷火龙':'Charizard', '拉普拉斯':'Lapras',
    'CaptainsRoom':'Captain’s room', 'Pokecenter':'Pokémon Center', 'TrashedHouse':'Trashed House',
    'ForestNorthGate':'Forest North Gate', 'ForestSouthGate':'Forest South Gate', 'PokemonFanClub':'Pokémon Fan Club',
    'WardensHouse':'Warden’s house', 'UndergroundPathNorthSouth':'Underground Path (north–south)',
    'UndergroundPathWestEast':'Underground Path (west–east)', 'UndergroundPathRoute':'Underground Path, Route ',
    'HELIX_FOSSIL':'Helix Fossil', 'LEMONADE':'Lemonade', 'SODA_POP':'Soda Pop'
  };
  const nameKeys=Object.keys(names).sort((a,b)=>b.length-a.length);
  const namePattern=new RegExp(nameKeys.join('|'),'g');
  function translate(value) {
    const text=String(value);
    if(Object.hasOwn(labels,text))return labels[text];
    const rules=[
      [/^前往(.+)$/,m=>`Travel to ${translate(m[1])}`],
      [/^取得(.+)$/,m=>`Obtain ${translate(m[1])}`],
      [/^(?:学会|学习)(.+)$/,m=>`Learn ${translate(m[1])}`],
      [/^使用(.+)通过地形$/,m=>`Use ${translate(m[1])} to traverse the terrain`],
      [/^使用(.+)$/,m=>`Use ${translate(m[1])}`],
      [/^战斗：使用(.+)$/,m=>`Battle: use ${translate(m[1])}`],
      [/^合法回合：使用(.+)$/,m=>`Legal turn: use ${translate(m[1])}`],
      [/^训练主力至 (\d+) 级$/,m=>`Train the lead Pokémon to level ${m[1]}`],
      [/^清除(.+)的地形阻碍$/,m=>`Clear terrain obstacles in ${translate(m[1])}`],
      [/^打通(.+)的前进通道$/,m=>`Open the path through ${translate(m[1])}`],
      [/^补给：(.+) × (\d+)$/,m=>`Restock: ${translate(m[1])} × ${m[2]}`],
      [/^背包 (\d+) 类 · 金钱 (\d+) · 剧情标志 (\d+) 项$/,m=>`Bag: ${m[1]} item types · Money: ${m[2]} · Story flags: ${m[3]}`],
      [/^完成 (\d+) · 待办 (\d+) · 近期结果 (\d+) 条$/,m=>`Completed: ${m[1]} · Remaining: ${m[2]} · Recent outcomes: ${m[3]}`],
      [/^(\d+) 个地图目标 · (\d+) 条已知失败$/,m=>`${m[1]} map targets · ${m[2]} known failures`],
      [/^跨 ([\d?]+) 张地图 · 路线由导航代码提供$/,m=>`${m[1]} map hops · route supplied by navigation code`],
      [/^(.+)无已知格点路径$/,m=>`${translate(m[1])}: no known tile path`],
      [/^确认选项：(.+)$/,m=>`Confirmation options: ${m[1]}`],
      [/^我方 (.+) \/ 对手 (.+)$/,m=>`Player: ${translate(m[1])} / opponent: ${translate(m[2])}`],
      [/^导航命令：走到 (.+)$/,m=>`Navigation command: move to ${m[1]}`],
      [/^交互命令：(.+)$/,m=>`Interaction command: ${m[1]}`],
      [/^判断 (\d+) 策略 \/ (\d+) 动作　·　战败 (\d+)$/,m=>`Judgments: ${m[1]} strategy / ${m[2]} action · Defeats: ${m[3]}`],
      [/^(策略层|动作层) · 当次输入(.*)$/,m=>`${m[1]==='策略层'?'Strategy':'Action'} layer · input${translate(m[2])}`],
      [/^原片快照 (.+) · 距画面 (\d+) 秒 · (\d+) 个候选$/,m=>`Snapshot ${m[1]} · ${m[2]}s before this frame · ${m[3]} candidates`],
      [/^本章模拟时间 (.+) · 模型等待另计$/,m=>`Chapter simulation time ${m[1]} · model waits accounted for separately`],
      [/^(Jev )?原片 (.+)$/,m=>`${m[1]||''}Recording ${translate(m[2])}`],
    ];
    for(const [pattern,render] of rules){const match=text.match(pattern);if(match)return render(match);}
    // Structured summaries have stable separators; translate each display field.
    for(const separator of ['　',' · ',' / ',' → ']){
      if(text.includes(separator))return text.split(separator).map(translate).join(separator);
    }
    return text.replace(namePattern,key=>names[key])
      .replace(/(\d+)(Gate(?:\dF)?)?号道路/g,(_,n,gate)=>`Route ${n}${gate?' '+gate.replace(/Gate/,'Gate '):''}`)
      .replace(/(\d+)级/g,'Lv.$1').replace(/(\d+)步/g,'$1 steps')
      .replace(/(\d+\/8) 徽章/g,'$1 badges').replace(/(\d+) 枚徽章/g,'$1 badges')
      .replaceAll('松开','release').replace(/Mart(?=\dF|Roof)/g,'Department Store ')
      .replace(/(Celadon )Mart$/, '$1Department Store').replace(/(City|Town|Diner|Dock)(?=\d)/g,'$1 ');
  }
  const t=value=>english?translate(value):String(value);
  function translateTree(root){
    if(!english)return;
    const walker=document.createTreeWalker(root,NodeFilter.SHOW_TEXT);
    for(let node;node=walker.nextNode();){
      if(node.parentElement?.closest('script,style,pre'))continue;
      const text=node.textContent,trimmed=text.trim();
      if(trimmed)node.textContent=text.replace(trimmed,t(trimmed));
    }
    for(const element of root.querySelectorAll('[aria-label],[title]')){
      for(const attr of ['aria-label','title'])if(element.hasAttribute(attr))element.setAttribute(attr,t(element.getAttribute(attr)));
    }
  }
  function html(source){
    if(!english)return source;
    const template=document.createElement('template');template.innerHTML=source;translateTree(template.content);return template.innerHTML;
  }
  function installNavigation(){
    const nav=document.createElement('nav');nav.className='jd-navigation';nav.setAttribute('aria-label',english?'Replay and language navigation':'回放与语言导航');
    const links=english?[['jev-player.html','Jev dashboard'],['player.html','Script vs. Jev'],['script-vs-jev-full.mp4','Commentary video (Chinese)']]:[['jev-player.html','Jev 大盘'],['player.html','脚本与 Jev 对比'],['script-vs-jev-full.mp4','完整解说视频']];
    for(const [file,label] of links){const a=document.createElement('a');a.href=file+(english&&file.endsWith('.html')?'?lang=en':'');a.textContent=label;nav.append(a);}
    const languages=document.createElement('span');languages.className='jd-languages';
    for(const [language,label] of [['zh','中文'],['en','English']]){
      const a=document.createElement('a'),url=new URL(location.href);language==='en'?url.searchParams.set('lang','en'):url.searchParams.delete('lang');
      a.href=url.href;a.textContent=label;a.lang=language==='en'?'en':'zh-CN';a.hreflang=a.lang;a.dataset.language=language;
      if((language==='en')===english)a.setAttribute('aria-current','page');
      const refresh=()=>{const target=new URL(a.href);window.JevPlaybackState?.(target);a.href=target.href;};
      for(const event of ['click','pointerdown','focus','contextmenu'])a.addEventListener(event,refresh);
      languages.append(a);
    }
    nav.append(languages);document.querySelector('main')?.prepend(nav);
  }
  window.JevI18n={english,t,translate,html,translateTree};
  document.documentElement.lang=english?'en':'zh-CN';translateTree(document);installNavigation();
})();
