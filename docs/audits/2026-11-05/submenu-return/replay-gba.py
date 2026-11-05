import argparse,ctypes as c,hashlib,json,traceback
from pathlib import Path
import struct,zlib

a=argparse.ArgumentParser();a.add_argument('--rom',type=Path,required=True);a.add_argument('--sha256',required=True);a.add_argument('--output',type=Path,required=True);a.add_argument('--bridge',type=Path,required=True);a.add_argument('--fixture',type=Path,default=Path(__file__).resolve().parent.parent/'start-menu-close/fixture.sav');a.add_argument('--case',choices=['bag-neutral','bag-short','option-neutral','option-short'],required=True);args=a.parse_args()
out=args.output;out.mkdir(exist_ok=False);(out/'frames').mkdir()
rom=args.rom;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(rom)==args.sha256
identity={'rom':str(rom),'rom_sha256':sha(rom)}
fixture=args.fixture
assert sha(fixture)=='8e69cf22e2ed9c1a940512a135b0dae52bad662f43206978d13b85bfad777ced'
save=out/'play.sav';save.write_bytes(fixture.read_bytes());saved=save.read_bytes()
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
record={'status':'running','identity':{k:v for k,v in identity.items() if k!='source_files'},'fixture_sha256':sha(fixture),'commands':commands,'stages':stages,'frames':frames,'scope':'Unmodified default release ROM, ordinary SRAM at cold boot, physical emulator keys only; no game-state setters, no autopilot, full render/audio, no frameskip.'}
seen_frames={}
def capture(path):
 rgba=c.string_at(lib.audit_pixels(h),240*160*4);rgb=bytes(b for i,b in enumerate(rgba) if i%4!=3);pixels_sha256=hashlib.sha256(rgb).hexdigest()
 if path.parent==out/'frames' and pixels_sha256 in seen_frames:path=seen_frames[pixels_sha256]
 else:
  def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
  raw=b''.join(b'\x00'+rgb[y*720:(y+1)*720] for y in range(160))
  path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',240,160,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b''))
  if path.parent==out/'frames':seen_frames[pixels_sha256]=path
 return {'physical_frame':lib.audit_frame(h),'sha256':sha(path),'pixels_sha256':pixels_sha256,'file':str(path.relative_to(out))}
def step(frames,keys):
 commands.append({'frames':frames,'keys':keys});lib.audit_step(h,frames,keys)
try:
 record['frequency']=lib.audit_frequency(h);record['cycles_per_frame']=lib.audit_frame_cycles(h)
 prefix=[{'frames': 300, 'keys': 0, 'capture': 'boot-0300'}, {'frames': 300, 'keys': 0, 'capture': 'boot-0600'}, {'frames': 10, 'keys': 8}, {'frames': 120, 'keys': 0, 'capture': 'after-start'}, {'frames': 6, 'keys': 1}, {'frames': 180, 'keys': 0, 'capture': 'after-language'}, {'frames': 6, 'keys': 8}, {'frames': 90, 'keys': 0, 'capture': 'title-or-menu'}, {'frames': 6, 'keys': 8}, {'frames': 90, 'keys': 0, 'capture': 'main-menu'}, {'frames': 8, 'keys': 8}, {'frames': 120, 'keys': 0, 'capture': 'menu-ready'}, {'frames': 8, 'keys': 1}, {'frames': 120, 'keys': 0, 'capture': 'continue-result'}, {'frames': 8, 'keys': 1}, {'frames': 180, 'keys': 0, 'capture': 'loaded-map'}, {'frames': 8, 'keys': 8}, {'frames': 90, 'keys': 0, 'capture': 'start-menu'}]
 def menu_roi():
  rgba=c.string_at(lib.audit_pixels(h),240*160*4)
  return b''.join(rgba[y*960+123*4:y*960+199*4]for y in range(9,151))
 for request in prefix[:18]:step(request['frames'],request['keys'])
 for _ in range(2 if args.case.startswith('bag') else 5):step(8,128);step(64,0)
 target=menu_roi();record['selected_menu']=capture(out/'selected-menu.png')
 step(8,1);step(320,0)
 record['before_b']=capture(out/'before-b.png')
 record['case']=args.case
 assert lib.audit_export_sram(h,str(out/'initial-cartridge.sav').encode())==32768
 assert (out/'initial-cartridge.sav').read_bytes()==saved
 record['origin']=lib.audit_frame(h)
 for tick in range(800):
  keys=2 if tick<8 else 128 if args.case.endswith('short') and tick in [20,21] else 0
  step(1,keys);frames.append({'tick':tick,'keys':keys,**capture(out/'frames'/f'frame-{tick:04}.png')})
 record['final']=capture(out/'after-cancel.png')
 assert menu_roi()==target,'restored START cursor or menu differs from the selected entry'
 step(8,1);step(320,0);record['after_reopen']=capture(out/'after-reopen.png')
 assert record['after_reopen']['pixels_sha256']==record['before_b']['pixels_sha256'],'A did not reopen the same submenu'
 assert lib.audit_export_sram(h,str(out/'final-cartridge.sav').encode())==32768
 committed=(out/'final-cartridge.sav').read_bytes();assert committed==saved
 record['committed_sha256']=sha(out/'final-cartridge.sav')
 record['status']='passed'
except BaseException:
 record['status']='failed';record['error']=traceback.format_exc();raise
finally:
 if h:lib.audit_close(h)
 assert sha(rom)==identity['rom_sha256'] and fixture.read_bytes()==saved
 assert save.read_bytes()==saved,'input fixture is temporary and stays immutable'
 record['fixture_and_rom_unchanged']=True
 (out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
 print(json.dumps({'status':record['status'],'origin':record.get('origin'),'frames':len(frames)}),flush=True)
