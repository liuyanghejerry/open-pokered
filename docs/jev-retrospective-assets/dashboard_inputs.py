"""Display summaries derived only from recorded application-level Jev inputs."""
import json


def parse(value):
    try:return json.loads(value)
    except (TypeError, ValueError):return value


def summarize(record, place, goal, moves, species):
    state=record['state'];criteria={k:parse(v) for k,v in record['question']['criteria'].items()}
    chosen=criteria.get(record['answer']['choice']);chosen=chosen if isinstance(chosen,dict) else {}
    world=state.get('world',state.get('local_state',{}))
    party=world.get('party',[])
    context=chosen.get('context') or state.get('strategy_context') or {}
    rows=[]
    if world:
        rows.append(['位置',f"{place(world['map'])} ({world['x']},{world['y']}) · {world['badges']}/8 徽章"])
        rows.append(['队伍',' / '.join(f"{species.get(m['species'],m['species'])} {m['level']}级 {m['hp']}/{m['max_hp']}HP" for m in party) or '尚无宝可梦'])
        if party:
            m=party[0]
            rows.append(['主力 PP',' · '.join(f"{moves.get(move,move)} {pp}" for move,pp in zip(m.get('moves',[]),m.get('pp',[])) if move!='None')])
        rows.append(['资源',f"背包 {len(world.get('bag',{}))} 类 · 金钱 {world.get('money',0)} · 剧情标志 {len(world.get('flags',{}))} 项"])
        routes=context.get('trigger_navigation',[])
        nav=chosen.get('navigation',{})
        reset=context.get('route_resets_won_battles')
        if reset:
            rows.append(['代价','治疗路线会重置已获胜战斗；完整列表可展开'])
        elif routes:
            rows.append(['选中路线',' / '.join(place(r['map'])+(' '+str(r['steps'])+'步' if r.get('tile_route_found') else '无已知格点路径') for r in routes[:2])])
        elif nav:
            rows.append(['选中路线',f"跨 {nav.get('map_hops','?')} 张地图 · 路线由导航代码提供"])
        elif context.get('move'):
            rows.append(['地形',moves.get(context['move'],context['move'])+' · '+place(context.get('map',world['map']))])
        else:
            rows.append(['导航',f"{len(state.get('navigation',{}))} 个地图目标 · {len(state.get('known_navigation_failures',{}))} 条已知失败" if 'navigation' in state else '局部状态与候选效果；完整字段可展开'])
        if 'remaining_objectives' in state:
            rows.append(['目标',f"完成 {len(state['completed_objectives'])} · 待办 {len(state['remaining_objectives'])} · 近期结果 {len(state.get('recent_outcomes',[]))} 条"])
        else:rows.append(['子目标',goal(state.get('subgoal'))])
        kind='剧情策略' if record['layer']=='strategy' else '操作选择'
    elif 'menu' in state:
        kind='对话选项' if 'dialogue' in state else '菜单选择'
        rows=[['子目标',goal(state.get('subgoal'))],['对白',(state.get('dialogue') or '此次未提供对白').replace('\n',' ')],['选项',' / '.join(state['menu'])]]
        if state.get('script'):rows.append(['脚本事实','确认选项：'+', '.join(state['script'].get('confirmation_options',[]))])
        rows.append(['位置',place(state.get('current_map','此次未提供'))])
        rows.append(['输入边界','此类调用未重复传入完整队伍与地形'])
    elif 'player_hp_band' in state:
        kind='战斗选招';band={'hurt':'受伤','healthy':'健康','low':'低血量'}
        rows=[['对战',f"{species.get(state['player'],state['player'])} {state['player_level']}级 → {species.get(state['enemy'],state['enemy'])} {state['enemy_level']}级"],
              ['血量分档',f"我方 {band.get(state['player_hp_band'],state['player_hp_band'])} / 对手 {band.get(state['enemy_hp_band'],state['enemy_hp_band'])}"],
              ['可用攻击',' / '.join(moves.get(v['move'],v['move']) for v in state['moves'].values())],
              ['计算信息','威力、命中、属性倍率、本系、暴击、PP 档位'],
              ['数值来源','基础种族值与代码计算的期望威力'],
              ['输入边界','HP/PP 为分档；不含整张地图或逐帧图像']]
    elif 'new_move' in state:
        kind='学习招式';m=state['pokemon']
        rows=[['宝可梦',species.get(m['species'],m['species'])+f" {m['level']}级"],['新招式',moves.get(state['new_move'],state['new_move'])],['已有招式',' / '.join(moves.get(v,v) for v in m['moves'] if v!='None')],['效果','新旧招式的威力、属性、命中、PP 与效果'],['候选','保留原招式，或替换允许遗忘的招式']]
    elif 'battle' in state:
        kind='受限战斗';rows=[['对手',state['battle']['enemy']['species']],['约束',state['constraint']],['输入','双方当前战斗状态与合法操作候选']]
    else:
        kind='战斗换人';enemy=state.get('enemy',{})
        rows=[['对手',str(enemy.get('species','未知'))+f" {enemy.get('level','?')}级 {enemy.get('hp','?')}HP"],['队伍','候选携带可出战成员的等级、HP、招式、PP'],['比较','候选还包含对该对手可用的有效攻击']]
    return {'kind':kind,'rows':rows,'state_fields':list(state),'candidate_count':len(criteria),'non_abstain_candidates':sum(k!='none' for k in criteria)}


def command_label(request):
    cmd=request['cmd']
    if cmd=='press_timeline':
        sequence=request['buttons'];groups=[]
        for button in sequence:
            name={'a':'A','b':'B','up':'↑','down':'↓','left':'←','right':'→',None:'松开'}.get(button,button)
            if groups and groups[-1][0]==name:groups[-1][1]+=1
            else:groups.append([name,1])
        if not any(sequence):return None
        return ' → '.join(str(n)+(f'×{count}' if count>1 else '') for n,count in groups)
    if cmd=='skip_dialogue':return '推进对白：skip_dialogue（调试命令）'
    if cmd=='move_to':return f"导航命令：走到 ({request['x']},{request['y']})"
    if cmd=='interact_with':return f"交互命令：{request['id']}"
    return None
