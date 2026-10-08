#!/usr/bin/env python3
"""Measure original StatusScreen entry after a real party-menu STATS selection.

Requires PyBoy and a retail Red ROM/symbols built from the pinned pret/pokered
source. Prepare a normal Continue and select STATS for party member 0 first;
save that stable emulator state. This tool changes only that member's species
and nickname in its controlled RAM fixture, then consumes actual A input and
records StatusScreen, palette, picture, PlayCry and input-wait routine times.
This is timing-table extraction, not a full 151-scenario playthrough verdict.
"""
import argparse, os, pathlib, re, io, json, hashlib
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
from pyboy import PyBoy
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--rom",type=pathlib.Path,required=True)
parser.add_argument("--symbols",type=pathlib.Path,required=True)
parser.add_argument("--constants",type=pathlib.Path,required=True)
parser.add_argument("--pretrigger-state",type=pathlib.Path,required=True)
parser.add_argument("--output",type=pathlib.Path,required=True)
args=parser.parse_args()
symbols={}
for line in args.symbols.read_text().splitlines():
 match=re.match(r"([0-9a-fA-F]+):([0-9a-fA-F]+) (\S+)",line)
 if match: symbols[match[3]]=(int(match[1],16),int(match[2],16))
monids=[]

for line in args.constants.read_text().splitlines():
 m=re.match(r'\s*const ([A-Z0-9_]+)\s*; \$([0-9A-Fa-f]{2})',line)
 if m and not m[1].startswith(('FOSSIL','MON_','MISSINGNO','NO_MON','GHOST')):monids.append((m[1],int(m[2],16)))
print('Species:',len(monids),flush=True)
p=PyBoy(str(args.rom),window='null',sound_emulated=True);p.set_emulation_speed(0)
index=[0];events=[]
def callback(name):events.append({'t':index[0],'routine':name})
for name in ['StatusScreen','GBPalNormal','LoadFlippedFrontSpriteByMonIndex','PlayCry','WaitForTextScrollButtonPress']:
 b,a=symbols[name];p.hook_register(b,a,callback,name)
pictures={}
results=[]
rom=args.rom.read_bytes();bank,addr=symbols['MonsterNames'];name_offset=bank*0x4000+(addr&0x3fff)
for name,id in monids:
 p.load_state(io.BytesIO(args.pretrigger_state.read_bytes()));events.clear();pictures.clear()
 for symbol in ['wPartySpecies','wPartyMon1Species','wCurPartySpecies']:p.memory[symbols[symbol][1]]=id
 for n,v in enumerate(rom[name_offset+(id-1)*10:name_offset+id*10]+bytes([0x50])):p.memory[symbols['wPartyMon1Nick'][1]+n]=v
 p.button_press('a')
 for t in range(300):
  index[0]=t
  if t==2:p.button_release('a')
  p.tick(1)
  pictures[t]=p.screen.ndarray.copy()
  if any(e['routine']=='WaitForTextScrollButtonPress' for e in events):break
 cue=next(e['t'] for e in events if e['routine']=='PlayCry')
 parts=[]
 for dt in range(4):
  im=pictures[cue+dt]
  top=bool((im[0:48,8:64,:3]<250).any());bottom=bool((im[48:56,8:64,:3]<250).any())
  parts.append(int(top)+2*int(bottom))
 result={'species':name,'id':id,'events':events.copy(),'picture_parts':parts};results.append(result)
 print(result,flush=True)
 args.output.write_text(json.dumps({'rom_sha1':hashlib.sha1(rom).hexdigest(),'state_sha256':hashlib.sha256(args.pretrigger_state.read_bytes()).hexdigest(),'input':'A at t0, release at t2; one emulator tick per observation','species':results},indent=2))
p.stop(save=False)
