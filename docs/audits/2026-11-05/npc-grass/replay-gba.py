import argparse,ctypes as c,hashlib,json,traceback
from pathlib import Path
import struct,zlib
a=argparse.ArgumentParser();a.add_argument('variant');a.add_argument('repeat',type=int,choices=[1,2]);a.add_argument('--rom',type=Path,required=True);a.add_argument('--sha256',required=True);a.add_argument('--fixture',type=Path,required=True);a.add_argument('--bridge',type=Path,required=True);a.add_argument('--output',type=Path,required=True);args=a.parse_args()
out=args.output/f'{args.variant}-{args.repeat}';out.mkdir(parents=True);(out/'frames').mkdir()
rom=args.rom;fixture=args.fixture
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(rom)==args.sha256
identity={'rom':str(rom),'rom_sha256':args.sha256,'fixture':str(fixture)}
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
 prefix=[(300,0),(300,0),(10,8),(120,0),(6,1),(180,0),(6,8),(90,0),(6,8),(90,0),(8,8),(120,0),(8,1),(120,0),(8,1),(180,0)]
 for n,(count,keys) in enumerate(prefix):
  step(count,keys)
  if n in [11,13,15]:stages.append(capture(out/f'boot-{n:02}.png'))
 step(400,0)
 record['before_walk']=capture(out/'before-walk.png')
 assert lib.audit_export_sram(h,str(out/'initial-cartridge.sav').encode())==32768
 assert (out/'initial-cartridge.sav').read_bytes()==saved
 record['origin']=lib.audit_frame(h)
 for tick in range(1000):
  keys=128 if 120<=tick<280 else 0
  step(1,keys)
  frames.append({'tick':tick,'keys':keys,**capture(out/'frames'/f'frame-{tick:04}.png')})
 record['after_walk']=capture(out/'after-walk.png')
 step(8,8);step(320,0);record['after_reopen']=capture(out/'after-reopen.png')
 assert lib.audit_export_sram(h,str(out/'final-cartridge.sav').encode())==32768
 assert (out/'final-cartridge.sav').read_bytes()==saved,'walking and START must not save'
 record['status']='recorded'
except BaseException:
 record['status']='failed';record['error']=traceback.format_exc();raise
finally:
 if h:lib.audit_close(h)
 assert sha(rom)==identity['rom_sha256'] and fixture.read_bytes()==saved
 assert save.read_bytes()==saved,'input fixture is temporary and stays immutable'
 record['fixture_and_rom_unchanged']=True
 (out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
 print(json.dumps({'repeat':args.repeat,'status':record['status'],'variant':args.variant,'origin':record.get('origin'),'frames':len(frames)}),flush=True)
