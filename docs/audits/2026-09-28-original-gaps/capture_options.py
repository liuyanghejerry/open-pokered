"""Static options comparison, not a timing/animation verdict. Requires PyBoy."""
import argparse, hashlib, json, shutil, sys, tempfile
from pathlib import Path
from pyboy import PyBoy

ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'scripts'))
from playthrough import Game,m01_boot,m02_oak_speech

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rom',type=Path,required=True)
parser.add_argument('--output',type=Path)
args=parser.parse_args()
out=args.output or Path(tempfile.mkdtemp(prefix='pokered-options-audit-'))
out.mkdir(parents=True,exist_ok=True)
sha1=hashlib.sha1(args.rom.read_bytes()).hexdigest()
assert sha1=='ea9bcae617fdf159b045185467ae58b2e4a48b9a',sha1
with tempfile.TemporaryDirectory(prefix='pokered-reference-') as tmp:
    rom=Path(tmp)/'red.gb'
    shutil.copyfile(args.rom,rom)
    p=PyBoy(str(rom),window='null')
    p.set_emulation_speed(0)
    def reference_tap(button,wait=60):
        p.button_press(button);p.tick(8);p.button_release(button);p.tick(wait)
    try:
        p.tick(1200)
        reference_tap('start',240);reference_tap('start',120)
        reference_tap('down',20);reference_tap('a',60)
        p.screen.image.save(out/'options-original.png')
    finally:p.stop(save=False)

g=Game(speed=0)
def tap(button):
    response=g.d.cmd(cmd='press_timeline',buttons=[button]+[None]*20,advance=True)
    assert response['ok'],response
try:
    m01_boot(g);m02_oak_speech(g);tap('start')
    for _ in range(8):
        field=g.st().get('field_menu') or {}
        items=field.get('items',[])
        if items and items[field['cursor']]=='Option':break
        tap('down')
    tap('a')
    assert g.st()['screen']=='options',g.st()
    for label in ('text','animation','style','cancel'):
        response=g.d.cmd(cmd='capture_frame',path=str((out/f'options-live-{label}.png').resolve()))
        assert response['ok'],response
        tap('down')
finally:g.close()
(out/'reference.json').write_text(json.dumps({'rom_sha1':sha1,'resolution':[160,144],'kind':'static screen comparison'},indent=2))
print('Evidence:',out)
