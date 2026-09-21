#!/usr/bin/env python3
"""Align recorded Jev choices with simulated video time; no generated reasoning."""
import argparse
import bisect
import json
import statistics
from pathlib import Path

from analyze_full_playthrough import rows
from dashboard_inputs import summarize, command_label

MAPS = {'PalletTown':'真新镇','RedsHouse2F':'主角房间','RedsHouse1F':'主角家',
        'OaksLab':'大木研究所','ViridianCity':'常青市','ViridianForest':'常青森林',
        'PewterCity':'尼比市','CeruleanCity':'华蓝市','VermilionCity':'枯叶市',
        'LavenderTown':'紫苑镇','CeladonCity':'玉虹市','FuchsiaCity':'浅红市',
        'SaffronCity':'金黄市','CinnabarIsland':'红莲岛','IndigoPlateau':'石英高原',
        'IndigoPlateauLobby':'联盟大厅','LoreleisRoom':'科拿房间','BrunosRoom':'希巴房间',
        'AgathasRoom':'菊子房间','LancesRoom':'渡房间','ChampionsRoom':'冠军房间',
        'HallOfFame':'名人堂','BillsHouse':'正辉家','MrFujisHouse':'富士老人家',
        'SafariZoneSecretHouse':'狩猎地带秘密小屋'}
FAMILIES = {'VictoryRoad':'冠军之路','MtMoon':'月见山','RockTunnel':'岩山隧道',
            'PokemonTower':'宝可梦塔','SilphCo':'西尔佛公司','SeafoamIslands':'双子岛',
            'PokemonMansion':'宝可梦屋','RocketHideout':'火箭队基地','SSAnne':'圣安奴号'}
CITIES = {'Viridian':'常青','Pewter':'尼比','Cerulean':'华蓝','Vermilion':'枯叶',
          'Celadon':'玉虹','Fuchsia':'浅红','Saffron':'金黄','Cinnabar':'红莲','Lavender':'紫苑'}
ITEMS = {'OAKSPARCEL':'大木博士的包裹','DOMEFOSSIL':'贝壳化石','BIKEVOUCHER':'自行车兑换券',
         'HM01':'居合斩秘传机','HM03':'冲浪秘传机','HM04':'怪力秘传机','POKEFLUTE':'宝可梦笛',
         'GOLDTEETH':'金牙','SECRETKEY':'秘密钥匙','CARDKEY':'钥匙卡','COINCASE':'代币盒',
         'FRESHWATER':'淡水','ELIXER':'全招式 PP 恢复道具','FullRestore':'全满药','MaxPotion':'全满回复药'}
MOVES = {'Slash':'劈开','Cut':'居合斩','Flamethrower':'喷射火焰','Dig':'挖洞','Scratch':'抓',
         'Ember':'火花','Growl':'叫声','Leer':'瞪眼','WaterGun':'水枪','Surf':'冲浪','Strength':'怪力',
         'Rage':'愤怒','Smokescreen':'烟幕','FireSpin':'火焰旋涡','DragonRage':'龙之怒','Bite':'咬住'}
SPECIES = {'Charmander':'小火龙','Charmeleon':'火恐龙','Charizard':'喷火龙','Lapras':'拉普拉斯'}
FLAGS = {'EVENT_OAK_APPEARED_IN_PALLET':'触发博士出场','EVENT_GOT_STARTER':'选择初始宝可梦',
         'EVENT_BATTLED_RIVAL_IN_OAKS_LAB':'完成研究所劲敌战','EVENT_GOT_POKEDEX':'领取图鉴',
         'EVENT_BEAT_BROCK':'挑战小刚','EVENT_BEAT_MISTY':'挑战小霞','EVENT_BEAT_LT_SURGE':'挑战马志士',
         'EVENT_BEAT_ERIKA':'挑战莉佳','EVENT_BEAT_KOGA':'挑战阿桔','EVENT_BEAT_SABRINA':'挑战娜姿',
         'EVENT_BEAT_BLAINE':'挑战夏伯','EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI':'挑战坂木',
         'EVENT_BEAT_LORELEIS_ROOM_TRAINER_0':'挑战科拿','EVENT_BEAT_BRUNOS_ROOM_TRAINER_0':'挑战希巴',
         'EVENT_BEAT_AGATHAS_ROOM_TRAINER_0':'挑战菊子','EVENT_BEAT_LANCE':'挑战渡',
         'EVENT_BEAT_CHAMPION_RIVAL':'击败冠军，进入名人堂','EVENT_BEAT_GHOST_MAROWAK':'解决幽灵嘎啦嘎啦阻碍',
         'EVENT_GAVE_GOLD_TEETH':'交还金牙，准备怪力','EVENT_FIGHT_ROUTE12_SNORLAX':'唤醒挡路的卡比兽',
         'EVENT_BILL_SAID_USE_CELL_SEPARATOR':'协助正辉恢复原状'}


def place(name):
    if name in MAPS:return MAPS[name]
    if name.startswith('Route'):return name.replace('Route','')+'号道路'
    for k,v in FAMILIES.items():
        if name.startswith(k):return v+' '+name[len(k):]
    for k,v in CITIES.items():
        if name.startswith(k):return v+name[len(k):].replace('Pokecenter','宝可梦中心').replace('Gym','道馆').replace('Mart','商店')
    if name.startswith('SafariZone'):return '狩猎地带 '+name[10:]
    return name


def goal(target):
    if not target:return '等待首次策略选择'
    kind,key,value=target
    if kind=='heal':return '恢复队伍 HP、状态与 PP'
    if kind=='level':return f'训练主力至 {value} 级'
    if kind=='item':return '取得'+ITEMS.get(str(key).replace('_',''),str(key))
    if kind=='move':return '学会'+MOVES.get(key,key)
    if kind=='pokemon':return '取得'+SPECIES.get(str(key).title(),str(key))
    if kind=='location':return '前往'+place(key[0])
    if kind=='transport':return '调整移动方式并通过目标位置'
    if kind=='terrain':return '清除'+place(key.split(',')[0])+'的地形阻碍'
    if kind=='block':return '打通'+place(key.split(',')[0])+'的前进通道'
    if kind=='supply':return f'补给：{ITEMS.get(key,key)} × {value}'
    if kind=='visibility':return '解决可见人物或挡路对象的条件'
    if kind=='flag':
        if key in FLAGS:return FLAGS[key]
        if 'VICTORY_ROAD' in key:return '推动石块，开启冠军之路机关'
        if 'MANSION_SWITCH' in key:return '调整宝可梦屋开关'
        if 'UNLOCKED' in key or 'LOCK_DOOR' in key:return '解除当前门禁或机关'
        if 'TRAINER' in key:return '击败前置路线上的训练家'
        if 'AUTOWALK' in key:return '完成房间入口剧情'
        return '推进当前剧情前置条件'
    return '完成当前准备目标'


def operation(value):
    prefix,_,detail=str(value).partition(':')
    if prefix=='travel_to':return '前往'+place(detail)
    if prefix=='train_encounter':return '在野外寻找遭遇并训练'
    if prefix in ('interact_with','interact_counter'):return '与目标人物交谈'
    if prefix=='interact_tile':return '调查目标位置'
    if prefix=='move_to':return '移动到目标站位'
    if prefix=='push_puzzle':return '按已规划步骤推石解谜'
    if prefix=='learn':return '学习'+MOVES.get(detail.split(',')[0],detail.split(',')[0])
    if prefix=='teach_tm':return '使用招式机器学习招式'
    if prefix=='wait_for_control':return '等待剧情交还控制'
    if prefix in ('surf','cut'):return '使用'+MOVES.get(prefix.title(),prefix)+'通过地形'
    return str(value)


def candidate(value, strategy):
    try:data=json.loads(value)
    except (ValueError,TypeError):data=value
    if isinstance(data,dict):
        if 'establish' in data:return goal(data['establish'])
        if 'operation' in data:return operation(data['operation'])
        if 'move' in data:return '使用'+MOVES.get(data['move'],data['move'])
    if isinstance(data,str):
        if data in MOVES:return '使用'+MOVES[data]
        if data.startswith('None of'):return '暂不选择'
        if 'Attack this turn' in data:return '本回合攻击，保留药品'
        if data=='Keep the current moves and decline the new move':return '保留现有招式'
        if 'Learn ' in data:return '调整学习的新招式'
        if data.upper() in ('YES','NO'):return '确认' if data.upper()=='YES' else '取消'
        return data[:65]
    return '选择可用操作'


def build(folder, output):
    capture=json.loads((folder/'capture.json').read_text())
    assert capture['success']
    observations=list(rows(folder/'observations.jsonl'))
    primary=[r for r in observations if r['process']==1]
    wall=[r['wall_s'] for r in primary]
    trace=next((folder/'run').glob('*/trace.jsonl'))
    trace_rows=list(rows(trace))
    first_frame_wall={}
    for r in primary:first_frame_wall.setdefault(r['frame_count'],r['wall_s'])
    anchors=[r for r in trace_rows if r['kind']=='battle_skill_completed' and r['frame'] in first_frame_wall]
    offsets=[first_frame_wall[r['frame']]-r['elapsed_s'] for r in anchors]
    clock_offset=statistics.median(offsets)
    def stamp(t):
        # Native trace and capture wrapper start their clocks at different moments.
        # One millisecond accommodates the trace's rounded millisecond timestamps.
        index=max(0,bisect.bisect_right(wall,t+clock_offset+.001)-1)
        return primary[index]['frame_count']/60
    result={'source_attempt':folder.name,'clock':'Video time is simulated frames / 60; decisions are attached to the last observed primary frame before their trace timestamp.',
            'clock_alignment':{'capture_minus_trace_s':clock_offset,'anchor_count':len(anchors),'anchor_offset_range_s':[min(offsets),max(offsets)],'timestamp_rounding_tolerance_s':.001,'same_frame_policy':'When several logged decisions share one simulated frame, the latest event at that frame is displayed; wall-clock waits remain in the trace.'},
            'model':'jev-1.13.0','states':[],'strategies':[],'actions':[],'outcomes':[],'counts':[], 'notes':[],
            'strategy_inputs':[],'action_inputs':[],'commands':[],
            'command_alignment':'Most press_timeline responses have no frame count. Completed input commands are displayed at the next recorded observation frame; this is observation-aligned, not an exact button-down animation.'}
    offset=[0,int(capture['processes'][0]['video_probe']['streams'][0]['nb_frames'])]
    previous=None
    for r in sorted(observations,key=lambda r:offset[r['process']-1]+r['frame_count']):
        live=r.get('battle_live') or {}
        party=[dict(m) for m in r['party']]
        if r['screen']=='battle':
            for m,l in zip(party,live.get('player_party',[])):m.update(l)
            for m in party:
                if m['species']==(live.get('player') or {}).get('species'):m.update(live['player'])
        compact={'place':place(r['map_name']),'screen':r['screen'],'badges':int(r['badges']).bit_count(),
                 'hof':r['hall_of_fame_count'],'party':[{'name':SPECIES.get(m['species'],m['species']), 'level':m['level'], 'hp':m['hp'],'max_hp':m['max_hp'],'status':m.get('status','None')} for m in party],
                 'wall_s':r['wall_s'],'process':r['process']}
        signature=json.dumps({k:v for k,v in compact.items() if k!='wall_s'},sort_keys=True)
        if signature!=previous:
            result['states'].append([ (offset[r['process']-1]+r['frame_count'])/60,compact]);previous=signature
    counts={'strategy':0,'action':0,'defeats':0,'tokens':0}
    trace=next((folder/'run').glob('*/trace.jsonl'))
    last_judgment={};raw_inputs={};attack_origin='recorded judgment'
    for r in trace_rows:
        t=stamp(r.get('elapsed_s',0));kind=r['kind']
        if kind=='judgment':
            layer=r['layer'];counts[layer]+=1;counts['tokens']+=r.get('input_tokens',0)+r.get('output_tokens',0)
            criteria=r['question']['criteria'];answer=r['answer'];choice=answer['choice']
            top=sorted(answer['probabilities'].items(),key=lambda x:-x[1])[:3]
            input_id=len(raw_inputs)+1
            raw_inputs[str(input_id)]={k:r[k] for k in ('layer','elapsed_s','model','state','question','answer','latency_s','input_tokens','output_tokens')}
            raw_inputs[str(input_id)]['source_s']=t
            summary=summarize(r,place,goal,MOVES,SPECIES)
            result[layer+'_inputs'].append([t,{'id':input_id,'source_s':t,'wall_s':r['elapsed_s'],**summary}])
            event={'title':candidate(criteria.get(choice,choice),layer=='strategy'),'choice':choice,'input_id':input_id,
                   'candidates':[{'id':k,'label':candidate(criteria.get(k,k),layer=='strategy'),'p':v,'selected':k==choice} for k,v in top],
                   'wall_s':r['elapsed_s'],'latency_s':r['latency_s'],'confidence':answer.get('confidence'),'origin':'recorded judgment'}
            result['strategies' if layer=='strategy' else 'actions'].append([t,event]);last_judgment[layer]=event
            if layer=='action':attack_origin='recorded judgment'
        elif kind=='conditional_choice':
            event=last_judgment[r['layer']];event['model_choice']=event['choice'];event['choice']=r['selected'];event['origin']='controller conditional selection after model abstention'
            raw=raw_inputs[str(event['input_id'])];raw['controller_selection']=r['selected']
            event['title']=candidate(raw['question']['criteria'][r['selected']],r['layer']=='strategy')
            for c in event['candidates']:c['selected']=c['id']==r['selected']
        elif kind=='strategy' and result['strategies']:
            result['strategies'][-1][1].update(title=goal(r['target']),target=r['target'])
        elif kind=='action_cache_hit':attack_origin='cached judgment'
        elif kind=='attack':
            result['actions'].append([t,{'title':'战斗：使用'+MOVES.get(r['move'],r['move']),'wall_s':r['elapsed_s'],'origin':attack_origin}])
        elif kind=='legal_turn_fallback':
            result['actions'].append([t,{'title':'合法回合：使用'+MOVES.get(r['move'],r['move']),'wall_s':r['elapsed_s'],'origin':'code fallback'}])
        elif kind=='outcome':
            label='目标效果已确认' if r.get('intended_effect_observed') else '操作完成，继续检查目标'
            result['outcomes'].append([t,{'title':label,'detail':operation(r['operation']),'wall_s':r['elapsed_s']}])
        elif kind=='battle_skill_completed':
            won='player_won: true' in r['result_phase'];lost='player_won: false' in r['result_phase']
            if won or lost:result['outcomes'].append([t,{'title':'本场获胜' if won else '本场败退','detail':place(r['map']),'wall_s':r['elapsed_s']}])
        elif kind=='battle_defeat':counts['defeats']+=1
        if kind in ('judgment','battle_defeat'):result['counts'].append([t,dict(counts)])
    # Editorial annotations are anchored to recorded events, not model thoughts.
    notes=[
        (488.415,'首个道馆：战败后重新准备','小火龙 15 级挑战小刚失败。代码提出训练至 17 级的候选；Jev 随后选择训练并再次挑战。'),
        (1899.708,'小霞阶段：能恢复，但准备成本较高','本轮已在小霞处败退三次。主要补救是提升主力等级；当前候选空间缺少完整的针对性捕获、组队和培养方案。'),
        (5919.941,'三连胜后撤退：应往前追问准备','喷火龙仅剩 4/210 HP，喷射火焰 1 PP。模型已收到“治疗会重置三场胜利”的提示；此刻撤退有依据，问题在连战前的资源准备。'),
        (6164.796,'购买候选存在，补给仍未落实','低血量败退后，候选已有全满药和全满回复药，Jev 仍选择挑战。神奇糖果也被算进 medicine，影响了代码的“无恢复物资”判断。'),
    ]
    for elapsed,title,body in notes:result['notes'].append({'source_s':stamp(elapsed),'wall_s':elapsed,'title':title,'body':body,'label':'复盘注解 · 依据日志，非模型自述','hold_s':8})
    gold=next((r for r in rows(trace) if r['kind']=='strategy' and r['target'][:2]==['item','GOLD_TEETH']),None)
    if gold:result['notes'].append({'source_s':stamp(gold['elapsed_s']),'wall_s':gold['elapsed_s'],'title':'八徽章之后，折返准备怪力','body':'此前已进过狩猎地带取得冲浪，这次再寻找金牙。可改进的是顺路合并前置准备，而不是将中间完成道馆的时间都视为浪费。','label':'复盘注解 · 依据日志，非模型自述','hold_s':8})
    result['notes'].sort(key=lambda x:x['source_s'])
    by_process={p:[r for r in observations if r['process']==p] for p in (1,2)}
    observation_times={p:[r['wall_s'] for r in v] for p,v in by_process.items()}
    for r in rows(folder/'commands.jsonl'):
        label=command_label(r['request'])
        if not label:continue
        p=r['process'];obs=by_process[p]
        index=min(len(obs)-1,bisect.bisect_left(observation_times[p],r['end_s']-.00001))
        t=(offset[p-1]+obs[index]['frame_count'])/60
        result['commands'].append([t,{'label':label,'cmd':r['request']['cmd'],'request':r['request'],'ok':r['ok'],'process':p,'start_wall_s':r['start_s'],'end_wall_s':r['end_s'],'source_s':t}])
    # The independent CONTINUE process briefly overlaps the original process.
    # Its video is concatenated after process 1, irrespective of wall ordering.
    for key in ['strategies','actions','outcomes','counts','commands']:result[key].sort(key=lambda x:x[0])
    result['clock_alignment']['battle_anchor_frame_mismatches']=sum(round(stamp(r['elapsed_s'])*60)!=r['frame'] for r in anchors)
    assert result['clock_alignment']['battle_anchor_frame_mismatches']==0, result['clock_alignment']
    output.mkdir(exist_ok=True)
    text=json.dumps(result,ensure_ascii=False,separators=(',',':'))
    (output/'jev-dashboard.json').write_text(text+'\n')
    (output/'jev-dashboard-data.js').write_text('window.JEV_DASHBOARD='+text.replace('</','<\\/')+';\n')
    raw_text=json.dumps(raw_inputs,ensure_ascii=False,separators=(',',':'))
    (output/'jev-inputs.json').write_text(raw_text+'\n')
    (output/'jev-inputs-data.js').write_text('window.JEV_INPUTS='+raw_text.replace('</','<\\/')+';\n')
    print({k:len(result[k]) for k in ['states','strategies','actions','outcomes','counts','notes','strategy_inputs','action_inputs','commands']})


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('folder',type=Path);ap.add_argument('--output',type=Path,required=True)
    a=ap.parse_args();build(a.folder,a.output)
