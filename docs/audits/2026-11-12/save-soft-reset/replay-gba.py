import argparse,ctypes as c,hashlib,json,traceback
from pathlib import Path
import struct,zlib

a=argparse.ArgumentParser();a.add_argument('repeat',type=int,choices=[1,2]);a.add_argument('--rom',type=Path,required=True);a.add_argument('--sha256',required=True);a.add_argument('--bridge',type=Path,required=True);a.add_argument('--output',type=Path,required=True);a.add_argument('--fixture',type=Path,default=Path(__file__).resolve().parents[4]/'docs/audits/2026-11-05/party-return/indigo/fixture.sav');a.add_argument('--probe',choices=['start','save'],required=True);a.add_argument('--choice',choices=['no','yes'],default='no');a.add_argument('--expect-reset',action='store_true');args=a.parse_args()
out=args.output;out.mkdir();(out/'frames').mkdir()
rom=args.rom;identity={'rom':str(rom),'rom_sha256':args.sha256}
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(rom)==identity['rom_sha256']
fixture=args.fixture
assert sha(fixture)=='becce9837f96157312392254d058afbecd0a13ec7f85915563e1093ece51f1fa'
save=out/'play.sav';save.write_bytes(fixture.read_bytes());saved=save.read_bytes()
identity.update(bridge_sha256=sha(args.bridge),script_sha256=sha(Path(__file__)))
lib=c.CDLL(str(args.bridge))
lib.audit_open.argtypes=[c.c_char_p]*3;lib.audit_open.restype=c.c_void_p
lib.audit_step.argtypes=[c.c_void_p,c.c_uint,c.c_uint]
lib.audit_pixels.argtypes=[c.c_void_p];lib.audit_pixels.restype=c.c_void_p
for name in ['audit_frame','audit_frequency','audit_frame_cycles']:
 f=getattr(lib,name);f.argtypes=[c.c_void_p];f.restype=c.c_uint
lib.audit_close.argtypes=[c.c_void_p]
lib.audit_export_sram.argtypes=[c.c_void_p,c.c_char_p];lib.audit_export_sram.restype=c.c_size_t
h=lib.audit_open(str(rom).encode(),str(save).encode(),str(out/'emulator.log').encode());assert h
commands=[];stages=[];frames=[]
record={'status':'running','identity':{k:v for k,v in identity.items() if k!='source_files'},'preamble':'none','fixture_sha256':sha(fixture),'commands':commands,'stages':stages,'frames':frames,'scope':'Unmodified default release ROM, ordinary SRAM at cold boot, physical emulator keys only; no game-state setters, no autopilot, full render/audio, no frameskip.'}
seen_frames={}
def capture(path):
 rgba=c.string_at(lib.audit_pixels(h),240*160*4);rgb=bytes(b for i,b in enumerate(rgba) if i%4!=3);pixels_sha256=hashlib.sha256(rgb).hexdigest()
 if path.parent==out/'frames' and pixels_sha256 in seen_frames:path=seen_frames[pixels_sha256]
 else:
  def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
  raw=b''.join(b'\x00'+rgb[y*720:(y+1)*720] for y in range(160))
  path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',240,160,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b''))
  if path.parent==out/'frames':seen_frames[pixels_sha256]=path
 # Compare START labels independently of its remembered cursor.
 menu_body=b''.join(rgb[y*720+136*3:y*720+192*3] for y in range(16,136))
 return {'menu_body_sha256':hashlib.sha256(menu_body).hexdigest(),'physical_frame':lib.audit_frame(h),'sha256':sha(path),'pixels_sha256':pixels_sha256,'file':str(path.relative_to(out))}
def step(frames,keys):
 commands.append({'frames':frames,'keys':keys});lib.audit_step(h,frames,keys)
try:
 record['frequency']=lib.audit_frequency(h);record['cycles_per_frame']=lib.audit_frame_cycles(h)
 prefix=[{'frames': 300, 'keys': 0, 'capture': 'boot-0300'}, {'frames': 300, 'keys': 0, 'capture': 'boot-0600'}, {'frames': 10, 'keys': 8}, {'frames': 120, 'keys': 0, 'capture': 'after-start'}, {'frames': 6, 'keys': 1}, {'frames': 180, 'keys': 0, 'capture': 'after-language'}, {'frames': 6, 'keys': 8}, {'frames': 90, 'keys': 0, 'capture': 'title-or-menu'}, {'frames': 6, 'keys': 8}, {'frames': 90, 'keys': 0, 'capture': 'main-menu'}, {'frames': 8, 'keys': 8}, {'frames': 120, 'keys': 0, 'capture': 'menu-ready'}, {'frames': 8, 'keys': 1}, {'frames': 120, 'keys': 0, 'capture': 'continue-result'}, {'frames': 8, 'keys': 1}, {'frames': 180, 'keys': 0, 'capture': 'loaded-map'}, {'frames': 8, 'keys': 8}, {'frames': 90, 'keys': 0, 'capture': 'start-menu'}]
 for index,request in enumerate(prefix[:18]):
  step(request['frames'],request['keys'])
  if 'capture' in request:stages.append(capture(out/(request['capture']+'.png')))
 if args.probe=='save':
  for _ in range(4):step(8,128);step(64,0)
  step(8,1);step(320,0)
  if args.choice=='no':step(8,128);step(80,0)
  step(2000 if args.choice=='yes' else 600,1)
 record['probe']=args.probe;record['choice']=args.choice
 assert lib.audit_export_sram(h,str(out/'before-combo.sav').encode())==32768
 committed=(out/'before-combo.sav').read_bytes()
 assert (committed!=saved)==(args.probe=='save' and args.choice=='yes'), 'only completed YES may save'
 record['before_combo']=capture(out/'before-combo.png');record['origin']=lib.audit_frame(h)
 for tick in range(512):
  step(1,15);frames.append({'tick':tick,'keys':15,**capture(out/'frames'/f'frame-{tick:04}.png')})
 record['after_combo']=capture(out/'after-combo.png')
 step(800,0);record['after_release']=capture(out/'after-release.png')
 assert lib.audit_export_sram(h,str(out/'final-cartridge.sav').encode())==32768
 assert (out/'final-cartridge.sav').read_bytes()==committed,'soft reset must not write SRAM'
 if args.expect_reset:
  language=next(s for s in stages if s['file']=='after-start.png')
  assert record['after_release']['pixels_sha256']==language['pixels_sha256'],'reset must actually return to language selection'
  for request in prefix[4:16]:step(request['frames'],request['keys'])
  record['after_continue']=capture(out/'after-continue.png')
  loaded=next(s for s in stages if s['file']=='loaded-map.png')
  assert record['after_continue']['pixels_sha256']==loaded['pixels_sha256'],'real CONTINUE after reset must restore the field'
  assert lib.audit_export_sram(h,str(out/'continued-cartridge.sav').encode())==32768
  assert (out/'continued-cartridge.sav').read_bytes()==committed,'CONTINUE must not save again'
 record['changed_during_combo']=sum(f['pixels_sha256']!=record['before_combo']['pixels_sha256'] for f in frames)
 record['status']='recorded'
except BaseException:
 record['status']='failed';record['error']=traceback.format_exc();raise
finally:
 if h:lib.audit_close(h)
 for name in ('emulator.log','continue-emulator.log'):
  log=out/name
  if log.exists() and any(marker in log.read_text(errors='replace').lower() for marker in ('panicked at','memory allocation of','[failed]')):
   record['status']='failed'
   record['error']=record.get('error','')+'\nEmulator failure in '+name
 assert sha(rom)==identity['rom_sha256'] and fixture.read_bytes()==saved
 assert save.read_bytes()==saved,'input fixture is temporary and stays immutable'
 record['fixture_and_rom_unchanged']=True
 (out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
 assert record['status']!='failed',record.get('error','capture failed')
 print(json.dumps({'repeat':args.repeat,'status':record['status'],'origin':record.get('origin'),'frames':len(frames)}),flush=True)
