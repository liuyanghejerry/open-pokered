import argparse,ctypes as c,hashlib,json,traceback
from pathlib import Path
import struct,zlib

a=argparse.ArgumentParser();a.add_argument('repeat',type=int,choices=[1,2]);a.add_argument('case',choices=['cancel','short','no-held','save-held']);a.add_argument('--rom',type=Path,required=True);a.add_argument('--sha256',required=True);a.add_argument('--bridge',type=Path,required=True);a.add_argument('--output',type=Path,required=True);a.add_argument('--fixture',type=Path,default=Path(__file__).resolve().parent.parent/'party-return/indigo/fixture.sav');args=a.parse_args()
out=args.output;out.mkdir();(out/'frames').mkdir()
rom=args.rom;identity={'rom':str(rom),'rom_sha256':args.sha256,'production_commit':'32ace488f18840ca0ae32221456c0cde92a8836f'}
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
 for index,request in enumerate(prefix[:18]):
  step(request['frames'],request['keys'])
  if 'capture' in request:stages.append(capture(out/(request['capture']+'.png')))
 for _ in range(4):step(8,128);step(64,0)
 record['selected_save']=capture(out/'selected-save.png')
 step(8,1);step(320,0)
 if args.case=='no-held':step(8,128);step(80,0)
 record['before_choice']=capture(out/'before-choice.png')
 assert lib.audit_export_sram(h,str(out/'initial-cartridge.sav').encode())==32768
 assert (out/'initial-cartridge.sav').read_bytes()==saved
 record['origin']=lib.audit_frame(h)
 hold_until=2000 if args.case=='save-held' else 600
 count=2800 if args.case=='save-held' else 1400 if args.case=='no-held' else 800
 for tick in range(count):
  keys=(2 if tick<8 else 16 if args.case=='short' and tick in [20,21] else 0) if args.case in ['cancel','short'] else (1 if tick<hold_until else 0)
  step(1,keys)
  frames.append({'tick':tick,'keys':keys,**capture(out/'frames'/f'frame-{tick:04}.png')})
  if args.case in ['no-held','save-held'] and tick==hold_until-1:record['while_held']=capture(out/'while-held.png')
 record['final']=capture(out/'final-field.png')
 assert lib.audit_export_sram(h,str(out/'final-cartridge.sav').encode())==32768
 committed=(out/'final-cartridge.sav').read_bytes()
 step(8,8);step(320,0);record['after_reopen']=capture(out/'after-reopen.png')
 assert lib.audit_export_sram(h,str(out/'committed.sav').encode())==32768
 assert (out/'committed.sav').read_bytes()==committed,'reopening START must not save again'
 lib.audit_close(h);h=None
 record['committed_sha256']=sha(out/'committed.sav')
 assert (committed!=saved)==(args.case=='save-held'),'only YES may write SRAM'
 h=lib.audit_open(str(rom).encode(),str(out/'committed.sav').encode(),str(out/'continue-emulator.log').encode());assert h
 record['continue_origin']=lib.audit_frame(h)
 for request in prefix[:16]:step(request['frames'],request['keys'])
 record['after_continue']=capture(out/'after-continue.png')
 assert lib.audit_export_sram(h,str(out/'continued-cartridge.sav').encode())==32768
 assert (out/'continued-cartridge.sav').read_bytes()==committed,'CONTINUE must not write SRAM'
 record['status']='recorded'
except BaseException:
 record['status']='failed';record['error']=traceback.format_exc();raise
finally:
 if h:lib.audit_close(h)
 assert sha(rom)==identity['rom_sha256'] and fixture.read_bytes()==saved
 assert save.read_bytes()==saved,'input fixture is temporary and stays immutable'
 record['fixture_and_rom_unchanged']=True
 (out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
 print(json.dumps({'repeat':args.repeat,'status':record['status'],'origin':record.get('origin'),'frames':len(frames)}),flush=True)
