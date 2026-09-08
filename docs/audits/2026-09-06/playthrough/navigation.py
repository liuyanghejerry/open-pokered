"""Navigation helper used during this audit; no game-state seeding.

Adds land tile-pair restrictions and overworld ledge jumps to local BFS.
This is an exploratory helper, not a complete navigation engine.
"""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts"))
from playthrough import Game,NavError,warp_tiles,MAPS,tile_at,walkable,DELTA,ROOT
from collections import deque
import re
source=(ROOT/'crates/pokered-data/src/collision.rs').read_text().split('pub const TILE_PAIR_COLLISIONS_LAND:')[1].split('];',1)[0]
pairs={(int(a),frozenset((int(b,16),int(c,16)))) for a,b,c in re.findall(r'tileset:\s*(\d+),\s*tile1:\s*(0x[0-9A-Fa-f]+),\s*tile2:\s*(0x[0-9A-Fa-f]+)',source)}
ledge_source=(ROOT/'crates/pokered-data/src/collision.rs').read_text().split('pub const LEDGE_TILES:')[1].split('];',1)[0]
ledges={(a.lower(),int(b,16),int(c,16)) for a,b,c in re.findall(r'direction: SPRITE_FACING_(\w+),\s*standing_tile: (0x[0-9A-Fa-f]+),\s*ledge_tile: (0x[0-9A-Fa-f]+)',ledge_source)}
spinner_source=(ROOT/'crates/pokered-core/src/overworld/spinner_paths.rs').read_text()
spinners={}
for name,body in re.findall(r'"(\w+)" => &\[(.*?)\n        \],',spinner_source,re.S):
 mapping={}
 for sx,sy,steps in re.findall(r'\((\d+), (\d+), &\[(.*?)\]\)',body):
  x,y=int(sx),int(sy)
  for direction,count in re.findall(r'dir: Direction::(\w+), steps: (\d+)',steps):
   dx,dy=DELTA[direction.lower()];x+=dx*int(count);y+=dy*int(count)
  mapping[(int(sx),int(sy))]=(x,y)
 spinners[name]=mapping
def bfs(name,start,goal,blocked):
 q=deque([start]);prev={start:None}
 while q:
  pos=q.popleft()
  if pos==goal:
   out=[]
   while prev[pos] is not None:
    old,d=prev[pos];out.append((pos,d));pos=old
   return [(start,None)]+out[::-1]
  for d,(dx,dy) in DELTA.items():
   nxt=(pos[0]+dx,pos[1]+dy)
   if MAPS[name]['tileset_id']==0 and (d,tile_at(name,*pos),tile_at(name,*nxt)) in ledges:
    nxt=(pos[0]+2*dx,pos[1]+2*dy)
   nxt=spinners.get(name,{}).get(nxt,nxt)
   if nxt in prev or nxt in blocked or not walkable(name,*nxt):continue
   if (MAPS[name]['tileset_id'],frozenset((tile_at(name,*pos),tile_at(name,*nxt)))) in pairs:continue
   prev[nxt]=(pos,d);q.append(nxt)
 return None
class AuditGame(Game):
 def nav_to(self,x,y,map_name=None,tries=60):
  initial=map_name or self.st()['map_name']
  for it in range(max(tries*12,600)):
   s=self.st()
   if s['screen']=='battle':
    self.battle_loop(prefer='fight' if s['script_awaiting_battle'] else 'run');self.cutscene();continue
   if s['screen']=='evolution':self.tap('a',30);continue
   cm=s['map_name'];px=s['player_x'];py=s['player_y']
   if cm!=initial:
    if (x,y) in warp_tiles(initial):return
    raise NavError(f'changed map {initial}->{cm}')
   if (px,py)==(x,y):return
   if s['script_running'] or s['dialogue_state']:
    self.cutscene();continue
   path=bfs(cm,(px,py),(x,y),blocked=self.live_npcs(cm)|(warp_tiles(cm)-{(x,y)}))
   if not path:raise NavError(f'no path {cm} {(px,py)} -> {(x,y)}')
   direction=path[1][1]
   jump=abs(path[1][0][0]-px)+abs(path[1][0][1]-py)>1
   self.d.drive([direction]*8,frames=48 if jump else 16)
   dx,dy=DELTA[direction]
   if (px+dx,py+dy) in spinners.get(cm,{}):
    self.wait('control_ready',max_frames=1200)
  raise NavError(f'single-step navigation budget exhausted {initial} {(x,y)}')
