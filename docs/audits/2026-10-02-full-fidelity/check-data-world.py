#!/usr/bin/env python3
"""Read-only independent data audit. Usage: python3 check-data-world.py REF [ROOT].

REF is a pret/pokered tree with data/, constants/, maps/ exported from fbcf7d0.
No game parser or runtime is used as an oracle. Numeric accuracy is compared
before RGBDS `percent` encoding; enum spelling/underscores are normalized.
"""
import argparse
import json
import re
import subprocess
from pathlib import Path
from collections import Counter

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('reference', type=Path)
parser.add_argument('root', nargs='?', type=Path, default=Path(__file__).resolve().parents[3])
parser.add_argument('--output', type=Path, default=Path(__file__).with_name('data-world-check.json'))
parser.add_argument('--implementation', help='Explicit label for a frozen exported source tree')
options = parser.parse_args()
REF, ROOT = options.reference, options.root
DATA = ROOT / 'crates/pokered-data'
implementation = options.implementation
if implementation is None:
    revision = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
    dirty = subprocess.check_output(['git', '-C', str(ROOT), 'status', '--porcelain', '--', str(DATA)], text=True).strip()
    implementation = 'open-pokered@' + revision + ('+worktree' if dirty else '')
out = {'reference': 'pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c', 'implementation': implementation,
       'counts': {}, 'diffs': [], 'notes': []}

def norm(s):
    s = re.sub('[^a-zA-Z0-9]', '', str(s)).lower()
    return {'nomove': 'none', 'psychictype': 'psychic', 'psychicm': 'psychicm',
            'unusedmaped': 'unusedmaped', 'psychictr': 'psychic'}.get(s, s)

def num(s):
    s = s.strip()
    if s.startswith('$'): return int(s[1:], 16)
    return int(s)

def text(path): return path.read_text()
def clean(path):
    return '\n'.join(x.split(';')[0].rstrip() for x in text(path).splitlines())
def args(line): return [x.strip() for x in line.split(',')]
def check(kind, name, field, actual, expected, ref):
    if actual != expected:
        out['diffs'].append({'kind': kind, 'name': name, 'field': field,
                             'actual': actual, 'expected': expected, 'reference': str(ref.relative_to(REF))})
def count(kind, amount=1): out['counts'][kind] = out['counts'].get(kind, 0) + amount

itemconst = clean(REF / 'constants/item_constants.asm')
tms = re.findall(r'^\s*add_tm\s+(\w+)', itemconst, re.M)
hms = re.findall(r'^\s*add_hm\s+(\w+)', itemconst, re.M)
tmnum = {s: i for i, s in enumerate(tms + hms)}
tmnum['UNUSED'] = 55

# All base-stat records, initial moves, and independently recomputed TM/HM bits.
statsfiles = {norm(p.stem): p for p in (REF / 'data/pokemon/base_stats').glob('*.asm')}
for p in sorted((DATA / 'pokemon').glob('*.json')):
    v = json.loads(text(p)); ref = statsfiles[norm(v['species'])]; src = clean(ref)
    dbs = [args(x) for x in re.findall(r'^\s*db\s+(.+)$', src, re.M)]
    check('pokemon', p.stem, 'baseStats', list(v['baseStats'].values()), [num(x) for x in dbs[1]], ref)
    check('pokemon', p.stem, 'types', [norm(v['type1']), norm(v['type2'])], [norm(x) for x in dbs[2]], ref)
    check('pokemon', p.stem, 'catchRate', v['catchRate'], num(dbs[3][0]), ref)
    check('pokemon', p.stem, 'baseExp', v['baseExp'], num(dbs[4][0]), ref)
    check('pokemon', p.stem, 'initialMoves', list(map(norm,v['initialMoves'])), list(map(norm, dbs[5])), ref)
    growth = dbs[6][0].removeprefix('GROWTH_')
    check('pokemon', p.stem, 'growthRate', norm(v['growthRate']), norm(growth), ref)
    tmline = re.search(r'^[ \t]*tmhm\b([\s\S]*?)\n\s*db', src, re.M).group(1).replace('\\', '')
    bits = [0]*7
    for move in args(tmline.replace('\n', ' ')):
        if move:
            n = tmnum[move]; bits[n//8] |= 1 << (n%8)
    check('pokemon', p.stem, 'tmHmFlags', v['tmHmFlags'], bits, ref)
    count('pokemon')

# All 165 actual moves (animation identifiers above 165 are not new moves).
ref = REF / 'data/moves/moves.asm'
refmoves = {norm(a[0]): a for a in [args(x) for x in re.findall(r'^\s*move\s+(.+)$', clean(ref), re.M)]}
for p in sorted((DATA / 'moves').glob('*.json')):
    v = json.loads(text(p)); a=refmoves[norm(v['id'])]
    for key, n in [('power',2), ('accuracy',4), ('pp',5)]: check('move',p.stem,key,v[key],num(a[n]),ref)
    check('move',p.stem,'effect',norm(v['effect']),norm(a[1]),ref)
    check('move',p.stem,'type',norm(v['type']),norm(a[3]),ref)
    count('moves')

# All 151 evolution records and ordered level-up moves.
ref = REF / 'data/pokemon/evos_moves.asm'
evos = {norm(name):body for name,body in re.findall(r'^(\w+)EvosMoves:\s*([\s\S]*?)(?=^\w+EvosMoves:|\Z)', clean(ref), re.M)}
for p in sorted((DATA / 'pokemon').glob('*.json')):
    v = json.loads(text(p)); dbs = [args(x) for x in re.findall(r'^\s*db\s+(.+)$', evos[norm(v['species'])], re.M)]
    expected=[]; learn=[]; mode='evolutions'
    for a in dbs:
        if a == ['0']:
            if mode=='evolutions': mode='learnset'; continue
            break
        if mode=='learnset': learn.append([num(a[0]),norm(a[1])]); continue
        if a[0]=='EVOLVE_LEVEL': expected.append(['level',num(a[1]),norm(a[2])])
        elif a[0]=='EVOLVE_ITEM': expected.append(['item',norm(a[1]),num(a[2]),norm(a[3])])
        elif a[0]=='EVOLVE_TRADE': expected.append(['trade',num(a[1]),norm(a[2])])
        else: raise ValueError(a)
    actual=[]
    for e in v['evolutions']:
        if e['method']=='level': actual.append(['level',e['level'],norm(e['species'])])
        elif e['method']=='item': actual.append(['item',norm(e['item']),e.get('minLevel',1),norm(e['species'])])
        else: actual.append(['trade',e.get('minLevel',1),norm(e['species'])])
    check('evolution',p.stem,'evolutions',actual,expected,ref)
    check('evolution',p.stem,'learnset',[[m['level'],norm(m['moveId'])] for m in v['learnset']],learn,ref)
    count('evolution_records')

# All trainer class parties, expanding shared-level and per-mon-level formats.
ref = REF / 'data/trainers/parties.asm'
parties={norm(name):body for name,body in re.findall(r'^(\w+)Data:\s*([\s\S]*?)(?=^\w+Data:|\Z)',clean(ref),re.M)}
for p in sorted((DATA / 'trainers').glob('*.json')):
    v=json.loads(text(p)); key=norm(v['class']); key={'psychictr':'psychic'}.get(key,key)
    expected=[]
    for a in [args(x) for x in re.findall(r'^\s*db\s+(.+)$',parties[key],re.M)]:
        assert a[-1]=='0', a
        if a[0]=='$FF': expected.append([[num(a[i]),norm(a[i+1])] for i in range(1,len(a)-1,2)])
        else: expected.append([[num(a[0]),norm(s)] for s in a[1:-1]])
    actual=[[[m['level'],norm(m['species'])] for m in party['pokemon']] for party in v['parties']]
    check('trainers',p.stem,'parties',actual,expected,ref)
    count('trainer_classes'); count('trainer_parties',len(actual)); count('trainer_pokemon',sum(map(len,actual)))

# Complete Pokédex numeric metadata and original English text pages.
ref=REF/'data/pokemon/dex_entries.asm'
dexentries={norm(name):body for name,body in re.findall(r'^(\w+)DexEntry:\s*([\s\S]*?)(?=^\w+DexEntry:|\Z)',clean(ref),re.M)}
dextext={norm(name):body for name,body in re.findall(r'^_(\w+)DexEntry::\s*([\s\S]*?)(?=^_\w+DexEntry::|\Z)',clean(REF/'data/pokemon/dex_text.asm'),re.M)}
for p in sorted((DATA/'pokemon').glob('*.json')):
    v=json.loads(text(p)); d=v['pokedex']; body=dexentries[norm(v['species'])]
    category=re.search(r'db\s+"([^"]+)@"',body).group(1)
    heights=re.search(r'db\s+(\d+),\s*(\d+)',body).groups()
    weight=int(re.search(r'dw\s+(\d+)',body).group(1))
    check('pokedex',p.stem,'category',d['category'],category,ref)
    check('pokedex',p.stem,'height',[d['heightFeet'],d['heightInches']],list(map(int,heights)),ref)
    check('pokedex',p.stem,'weight',d['weightDecipounds'],weight,ref)
    pages=[]; lines=[]
    for token,line in re.findall(r'^\s*(text|next|page)\s+"([^"]*)"',dextext[norm(v['species'])],re.M):
        if token=='page': pages.append('\n'.join(lines)); lines=[]
        lines.append(line)
    if lines: pages.append('\n'.join(lines))
    check('pokedex',p.stem,'flavorTextPages',d['flavorTextPages'],pages,REF/'data/pokemon/dex_text.asm')
    count('pokedex_records')

mapconst={}
for name,w,h,idhex in re.findall(r'^\s*map_const\s+(\w+),\s*(\d+),\s*(\d+)\s*;\s*\$(\w+)',text(REF/'constants/map_constants.asm'),re.M):
    mapconst[name]=(int(w),int(h),int(idhex,16))
headers={}
for ref in (REF/'data/maps/headers').glob('*.asm'):
    a=args(re.search(r'map_header\s+(.+)',clean(ref)).group(1)); headers[a[1]]=a[0]
mapnames={norm(k):v for k,v in headers.items()}
def mapname(s): return norm(headers.get(s,s))

# All 248 implemented map metadata/block maps/warps/objects, with additions
# retained in the report rather than silently treated as oracle content.
for p in sorted((DATA/'maps').glob('*/map.json')):
    v=json.loads(text(p)); name=v['name']; count('maps')
    ref=REF/'data/maps/headers'/f'{name}.asm'
    if not ref.exists():
        out['notes'].append({'extra_map':name}); continue
    src=clean(ref); a=args(re.search(r'map_header\s+(.+)',src).group(1))
    check('map',name,'dimensions',[v['header']['width'],v['header']['height']],list(mapconst[a[1]][:2]),ref)
    idconst=next((k for k in mapconst if norm(k)==norm(name)),a[1])
    check('map',name,'id',v['id'],mapconst[idconst][2],ref)
    check('map',name,'tileset',norm(v['header']['tileset']),norm(a[2]),ref)
    connections={d:{'targetMap':n,'offset':int(offset)} for d,n,c,offset in re.findall(r'connection\s+(\w+),\s*(\w+),\s*(\w+),\s*(-?\d+)',src)}
    check('map',name,'connections',v.get('connections',{}),connections,ref)
    blocks=REF/'maps'/f'{name}.blk'
    if blocks.exists(): check('map',name,'map.blk',list(p.with_name('map.blk').read_bytes()),list(blocks.read_bytes()),blocks)
    ref=REF/'data/maps/objects'/f'{name}.asm'; src=clean(ref)
    border=num(re.search(r'^\s*db\s+(\S+)',src,re.M).group(1))
    check('map',name,'borderBlock',v['header']['borderBlock'],border,ref)
    expected=[]
    for x,y,m,n in re.findall(r'warp_event\s+(\d+),\s*(\d+),\s*(\w+),\s*(\d+)',src):
        expected.append([int(x),int(y),mapname(m),int(n)-1])
    actual=[[w['x'],w['y'],norm(w.get('destMap','LAST_MAP')),w['destWarpId']] for w in v['warps']]
    check('map',name,'warps',actual,expected,ref); count('warps',len(actual))
    obj=[args(s) for s in re.findall(r'^\s*object_event\s+(.+)$',src,re.M)]
    check('map',name,'npc_count',len(v['npcs']),len(obj),ref)
    for i,(n,a) in enumerate(zip(v['npcs'],obj)):
        count('npcs')
        check('npc',name,f'{i+1}:position',[n['x'],n['y']],[num(a[0]),num(a[1])],ref)
        check('npc',name,f'{i+1}:spriteName',norm(n['spriteName']),norm(a[2].removeprefix('SPRITE_')),ref)
        check('npc',name,f'{i+1}:movement',n['movement'],'Wander' if a[3]=='WALK' else 'Stationary',ref)
        if a[3]=='WALK':
            check('npc',name,f'{i+1}:wander_axis',n['range'],{'ANY_DIR':0,'UP_DOWN':1,'LEFT_RIGHT':2}[a[4]],ref)
            count('wandering_npcs')
        elif a[4] in ('DOWN','UP','LEFT','RIGHT','NONE'):
            check('npc',name,f'{i+1}:facing',n['facing'],{'DOWN':'Down','UP':'Up','LEFT':'Left','RIGHT':'Right','NONE':'Down'}[a[4]],ref)
        else:
            out['notes'].append({'stationary_variable_facing':name,'object':i+1,'direction':a[4]})
        trainer=len(a)==8 and a[6].startswith('OPP_')
        check('npc',name,f'{i+1}:isTrainer',n['isTrainer'],trainer,ref)
        if trainer:
            check('npc',name,f'{i+1}:trainerClass',norm(n.get('trainerClass','')),norm(a[6].removeprefix('OPP_')),ref)
            check('npc',name,f'{i+1}:trainerSet',n.get('trainerSet',0),num(a[7]),ref)
        if len(a)==7:
            token=a[6]
            if token.startswith('TM_'): expected_item=201+tms.index(token[3:])
            elif token.startswith('HM_'): expected_item=196+hms.index(token[3:])
            elif token.isdigit(): expected_item=int(token)
            else:
                match=re.search(r'const\s+'+token+r'\s*;\s*\$(\w+)',text(REF/'constants/item_constants.asm'))
                assert match, (name, token)
                expected_item=int(match.group(1),16)
            check('npc',name,f'{i+1}:itemId',n.get('itemId',0),expected_item,ref); count('item_objects')
    bg=[[int(x),int(y)] for x,y in re.findall(r'bg_event\s+(\d+),\s*(\d+),',src)]
    actual=[[s['x'],s['y']] for s in v.get('signs',[])]
    check('map',name,'bg_coordinates',actual,bg,ref); count('background_objects',len(actual))

    # Conditional wild files evaluated independently for each version.
    ref=REF/'data/wild/maps'/f'{"SeaRoutes" if name in ("Route19","Route20") else name}.asm'
    if ref.exists():
        for version in ('red','blue'):
            active=[True]; lines=[]
            for line in clean(ref).splitlines():
                m=re.match(r'\s*IF\s+DEF\(_(RED|BLUE)\)',line)
                if m: active.append(active[-1] and m.group(1).lower()==version); continue
                if line.strip()=='ELSE': active[-1]=active[-2] and not active[-1]; continue
                if line.strip()=='ENDC': active.pop(); continue
                if active[-1]: lines.append(line)
            src='\n'.join(lines)
            for terrain in ('grass','water'):
                match=re.search(r'def_'+terrain+r'_wildmons\s+(\d+)\s*([\s\S]*?)end_'+terrain+r'_wildmons',src)
                rate=int(match.group(1)); mons=[[int(l),norm(s)] for l,s in re.findall(r'\bdb\s+(\d+),\s*(\w+)',match.group(2))]
                actual=((v.get('wild') or {}).get(version) or {}).get(terrain) or {}
                check('wild',name,version+':'+terrain+':rate',actual.get('encounterRate',0),rate,ref)
                check('wild',name,version+':'+terrain+':mons',[[m['level'],norm(m['species'])] for m in actual.get('mons',[])],mons,ref)
                count('wild_tables')

# Map-pointer wild table coverage, including every zero-rate map and aliases.
wildpointers=re.findall(r'^\s*dw\s+(\w+)',clean(REF/'data/wild/grass_water.asm'),re.M)
for p in sorted((DATA/'maps').glob('*/map.json')):
    v=json.loads(text(p)); label=wildpointers[v['id']]
    if label=='NothingWildMons':
        for version in ('red','blue'):
            for terrain in ('grass','water'):
                table=((v.get('wild') or {}).get(version) or {}).get(terrain) or {}
                check('wild_zero_map',v['name'],version+':'+terrain+':rate',table.get('encounterRate',0),0,REF/'data/wild/grass_water.asm')
        count('zero_wild_maps')
count('wild_pointer_maps',len(wildpointers))

# Independent checks for every fishing group and both map->group encodings.
rust=text(DATA/'src/wild_data.rs')
fishing=lambda s:[[int(l),norm(n)] for l,n in re.findall(r'level:\s*(\d+),\s*species:\s*Species::(\w+)',s)]
good=re.search(r'pub fn good_rod_data\(\)[\s\S]*?(?=/// Super Rod)',rust).group(0)
ref=REF/'data/wild/good_rod.asm'
check('fishing','good_rod','mons',fishing(good),[[int(l),norm(n)] for l,n in re.findall(r'db\s+(\d+),\s*(\w+)',clean(ref))],ref)
count('good_rod_species',2)
ref=REF/'data/wild/super_rod.asm'; src=clean(ref)
refgroups=[[[int(l),norm(n)] for l,n in re.findall(r'db\s+(\d+),\s*(\w+)',s)] for s in re.split(r'\.Group\d+:',src)[1:]]
groups=re.search(r'pub fn super_rod_groups\(\)[\s\S]*?(?=/// Super Rod map)',rust).group(0)
actual=[fishing(s) for s in re.split(r'// Group\d+',groups)[1:]]
check('fishing','super_rod','groups',actual,refgroups,ref)
refentries={norm(n):int(g)-1 for n,g in re.findall(r'dbw\s+(\w+),\s*\.Group(\d+)',src)}
entries={norm(n):int(g) for n,g in re.findall(r'map_name:\s*"(\w+)",\s*group_index:\s*(\d+)',rust)}
check('fishing','super_rod','named_map_groups',entries,refentries,ref)
runtime=re.search(r'pub fn super_rod_group_index_for_map\([\s\S]*?\n}',rust).group(0)
runtimegroups={}
for match in re.finditer(r'((?:MapId::\w+[\s|]*)+)\s*=>\s*(\d+)',runtime):
    for n in re.findall(r'MapId::(\w+)',match.group(1)): runtimegroups[norm(n)]=int(match.group(2))
check('fishing','super_rod','runtime_map_groups',runtimegroups,refentries,ref)
count('super_rod_groups',len(actual)); count('super_rod_maps',len(refentries))

# Independent coordinate proof: RGBDS rate anchor is the bottom-right 8px
# tile inside this 16px player cell, not the next 16px cell's bottom-left.
anchorproof=[]; affected=Counter()
passable={'Overworld':{0x00,0x10,0x1b,0x20,0x21,0x23,0x2c,0x2d,0x2e,0x30,0x31,0x33,0x39,0x3c,0x3e,0x52,0x54,0x58,0x5b,0x14,0x32},
          'Forest':{0x1e,0x20,0x2e,0x30,0x34,0x37,0x39,0x3a,0x40,0x51,0x52,0x5a,0x5c,0x5e,0x5f,0x14,0x48}}
for p in sorted((DATA/'maps').glob('*/map.json')):
    v=json.loads(text(p)); ts=v['header']['tileset']
    if ts not in passable or not p.with_name('map.blk').exists(): continue
    bst=(ROOT/'gfx/blocksets'/f'{ts.lower()}.bst').read_bytes(); blocks=p.with_name('map.blk').read_bytes()
    w=v['header']['width']; h=v['header']['height']
    if not w or not h: continue
    wild=(v.get('wild') or {}).get('red') or {}
    if not wild: continue
    def tile(x,y,right=0):
        b=blocks[(y//2)*w+x//2]
        return bst[b*16+((y%2)*2+1)*4+(x%2)*2+right]
    def rate(t):
        terrain='grass' if t=={'Overworld':0x52,'Forest':0x20}[ts] else 'water' if t==0x14 else None
        if terrain: return (wild.get(terrain) or {}).get('encounterRate',0)
        return 0
    for y in range(h*2):
        for x in range(w*2):
            left=tile(x,y)
            if left not in passable[ts]: continue
            actual=tile(x+1,y) if x+1<w else left
            expected=tile(x,y,1)
            if rate(actual)!=rate(expected):
                affected[v['name']]+=1
                if len(anchorproof)<30: anchorproof.append({'map':v['name'],'x':x,'y':y,'standing':left,'actual_rate_tile':actual,'expected_rate_tile':expected,'actual_rate':rate(actual),'expected_rate':rate(expected)})
out['encounter_anchor_proof']={'affected_map_cell_counts':dict(affected),'sample_cells':anchorproof,
 'scope':'Static walk/surf-passable cells on Overworld/Forest maps with wild tables; excludes sprite occupancy, warp gating and reachability, so counts are candidate coordinates, not encounter frequency.'}

out['diff_counts']=dict(Counter(d['kind'] for d in out['diffs']))
out['diff_fields']=dict(Counter(d['field'].split(':')[-1] for d in out['diffs']))
target=options.output
target.write_text(json.dumps(out,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({k:v for k,v in out.items() if k not in ('diffs','notes','encounter_anchor_proof')},indent=2))
print('report:',target)
