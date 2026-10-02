#!/usr/bin/env python3
"""Audit every map.json PokeBall item against the connected scene and SRAM toggle.

Read-only by default. --repair normalizes only broken ground-item storylines;
it preserves their triggers and existing collected flags, and adds handlers for
the six items that previously had none. Non-item NPC storylines are untouched.
"""
import argparse,json,re
from pathlib import Path
from collections import Counter

ap=argparse.ArgumentParser(); ap.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[3]); ap.add_argument('--repair',action='store_true'); ap.add_argument('--output',type=Path)
opt=ap.parse_args(); root=opt.root; data=root/'crates/pokered-data'; records=[]
items=json.loads((data/'data/items/item_list.json').read_text())['items']
const=lambda name: re.sub(r'(?<!^)([A-Z])',r'_\1',name).upper()
ids={re.sub(r'[^A-Z0-9]','',const(s)):i+1 for i,s in enumerate(items)}
def itemid(s):
    n=re.sub(r'[^A-Z0-9]','',s.upper())
    if n.startswith('TM') and n[2:].isdigit(): return 200+int(n[2:])
    if n.startswith('HM') and n[2:].isdigit(): return 195+int(n[2:])
    return ids.get(n)
def itemname(i):
    if i>=201:return f'TM{i-200:02d}'
    if i>=196:return f'HM{i-195:02d}'
    return const(items[i-1])
zhitems=dict(re.findall(r'ItemId::(\w+)\s*=>\s*"([^"]*)"',(data/'src/lang_data.rs').read_text()))

def blockend(s,start):
    depth=0; quote=False; escaped=False; comment=False
    for j in range(start,len(s)):
        c=s[j]
        if comment:
            if c=='\n':comment=False
            continue
        if quote:
            if escaped:escaped=False
            elif c=='\\':escaped=True
            elif c=='"':quote=False
            continue
        if c=='"':quote=True;continue
        if s[j:j+2]=='//':comment=True;continue
        if c=='{':depth+=1
        elif c=='}':
            depth-=1
            if depth==0:return j+1
    raise ValueError('unclosed scene block')

def stories(s):
    return {m.group(1):(m.start(),blockend(s,s.index('{',m.end()-1))) for m in re.finditer(r'^[ \t]*@storyline\("([^"]+)"\)\s*\{',s,re.M)}

for p in sorted((data/'maps').glob('*/map.json')):
    v=json.loads(p.read_text()); mapname=v['name']; scene=p.with_name('script.scene'); src=scene.read_text(); configpath=p.with_name('script_config.json'); cfg=json.loads(configpath.read_text()); replacements=[]; additions=[]; cfgchanged=False
    st=stories(src)
    for n in v['npcs']:
        if n.get('spriteName')!='PokeBall' or not n.get('itemId'):continue
        npc=n['textId']; expected=n['itemId']; binding=next((b for b in cfg['npcs'] if b['id']==npc),None)
        match=next(((name,span) for name,span in st.items() if re.search(r'@trigger\([^)]*\bnpc\s*=\s*'+str(npc)+r'\b',src[span[0]:span[1]])),None)
        if not match and binding and binding.get('talk') in st:match=(binding['talk'],st[binding['talk']])
        body=src[match[1][0]:match[1][1]] if match else ''
        trigger=re.search(r'@trigger\([^)]*\)',body)
        t=trigger.group(0) if trigger else None
        toggle=(re.search(r'toggle\s*=\s*"([^"]+)"',t).group(1) if t and 'toggle' in t else (binding or {}).get('toggleId'))
        if not toggle:
            prefix=re.sub(r'(?<=[a-z])(?=[A-Z])|(?<=[A-Za-z])(?=\d)', '_', mapname).upper()
            # Existing script IDs follow POWER_PLANT and VIRIDIAN_GYM.
            toggle=f'{prefix}_OBJ_{npc}'
        give=re.search(r'(?:(\w+)\s*=\s*)?giveItem\("([^"]+)",\s*(\d+)\)',body)
        token=give.group(2) if give else None
        given=give.group(1) if give else None
        safe=bool(given and re.search(r'@if\s*\(\s*'+given+r'\s*\)',body))
        hidden=bool(re.search(r'hideObject(?:ByName)?\("'+re.escape(toggle)+r'"\)',body))
        auto_dialogue='showItemDialogue(' in body
        defects=[]
        if give and not auto_dialogue:defects.append('missing_found_fanfare')
        if not give:defects.append('no_pickup_handler')
        elif itemid(token)!=expected:defects.append('wrong_reward')
        if give and not safe:defects.append('ignores_give_failure')
        if give and not hidden:defects.append('success_does_not_hide')
        records.append({'map':mapname,'npc':npc,'expected_id':expected,'expected_item':itemname(expected),'scene_item':token,'toggle':toggle,'storyline':match[0] if match else None,'line':src[:match[1][0]].count('\n')+1 if match else None,'defects':defects})
        if not opt.repair or not defects:continue
        name=match[0] if match else f'groundItem{npc}'
        if not t:t=f'@trigger(map = "{mapname}", npc = {npc}, toggle = "{toggle}")'
        flags=list(dict.fromkeys(re.findall(r'setFlag\("([^"]+)"\)',body)))
        guard=flags[0] if flags else None
        # Keep original successful bilingual text unless its item was wrong.
        found=next((m.group(0) for m in re.finditer(r'@t\("(?:\\.|[^"\\])*",\s*"(?:\\.|[^"\\])*"\)',body) if 'found ' in m.group(0)),None)
        token=itemname(expected)
        if not found or 'wrong_reward' in defects:
            en=f'<PLAYER> found {token.replace("_"," ")}!'
            zh=zhitems.get(items[expected-1],token) if expected<196 else token
            found=f'@t({json.dumps(en)}, {json.dumps("<PLAYER>找到了"+zh+"！",ensure_ascii=False)})'
        indent='    ' if not guard else '      '
        content=f'  @storyline("{name}") {{\n    {t}\n'
        if guard:content+=f'    @if (!getFlag("{guard}")) {{\n'
        content+=indent+f'given = giveItem("{token}", 1)\n'+indent+'@if (given) {\n'
        for flag in flags:content+=indent+f'  setFlag("{flag}")\n'
        content+=indent+f'  hideObjectByName("{toggle}")\n'+indent+f'  showItemDialogue(t({found[3:-1]}))\n'+indent+'} @else {\n'+indent+'  @speaker("") { @t("You have too much stuff\\nalready!", "你带的东西太多了！") }\n'+indent+'}\n'
        if guard:content+='    }\n'
        content+='  }'
        if match:replacements.append((match[1][0],match[1][1],content))
        else:
            additions.append(content)
            cfg['npcs'].append({'id':npc,'talk':name,'toggleId':toggle});cfgchanged=True
    if opt.repair:
        for a,b,new in sorted(replacements,reverse=True):src=src[:a]+new+src[b:]
        if additions:
            src=src[:src.rfind('}')]+'\n'+'\n\n'.join(additions)+'\n'+src[src.rfind('}'):]
        if replacements or additions:scene.write_text(src)
        if cfgchanged:configpath.write_text(json.dumps(cfg,ensure_ascii=False,indent=2)+'\n')

report={'count':len(records),'defect_counts':dict(Counter(d for r in records for d in r['defects'])),'items':records}
target=opt.output or Path(__file__).with_name('ground-pickup-check.json');target.parent.mkdir(parents=True,exist_ok=True);target.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({k:v for k,v in report.items() if k!='items'},indent=2))
