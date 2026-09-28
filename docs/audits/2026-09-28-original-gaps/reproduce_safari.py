import sys, json, argparse, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT/'scripts'))
from playthrough import Game, m01_boot, m02_oak_speech

parser=argparse.ArgumentParser(description='Reproduce Safari allowance reset through normal doorway input; setup is seeded.')
parser.add_argument('--output',type=Path)
args=parser.parse_args()
OUT=args.output or Path(tempfile.mkdtemp(prefix='pokered-safari-audit-'))
OUT=OUT.resolve()
OUT.mkdir(parents=True,exist_ok=True)

results=[]
for repeat in range(2):
    g=Game(speed=0, seed=42)
    trace=[]
    def record(label):
        s=g.st()
        entry={'label':label,'state':s,'flags':g.d.cmd(cmd='get_flags'), 'bag':g.d.cmd(cmd='get_bag')}
        trace.append(entry)
        print(label, json.dumps({k:s.get(k) for k in ('map_name','player_x','player_y','money','choice','script_effect','dialogue_state')}), flush=True)
        return s
    def tap(button, frames=15):
        r=g.d.cmd(cmd='press_timeline',buttons=[button]+[None]*frames,advance=True)
        assert r['ok'],r
    def shot(label):
        path=OUT/f'safari-{label}-{repeat+1}.png'
        r=g.d.cmd(cmd='capture_frame',path=str(path))
        assert r['ok'],r
    def menu_shot(label):
        tap('start',10);shot(label);tap('b',10)
    try:
        m01_boot(g); m02_oak_speech(g)
        g.d.cmd(cmd='give_pokemon',species='Bulbasaur',level=5)
        g.d.cmd(cmd='warp',map='SafariZoneGate',x=4,y=3)
        g.step(60); tap('up',30)
        record('gate-entry-dialogue')
        for _ in range(70):
            s=g.st()
            if s['map_name']=='SafariZoneCenter': break
            tap('a',15)
        g.step(60)
        record('paid-entry')
        assert g.st()['map_name']=='SafariZoneCenter' and g.st()['money']==2500,g.st()
        for _ in range(12):
            if g.st()['map_name']!='SafariZoneCenter': break
            tap('up',10)
        record('walked-away-from-entrance')
        menu_shot('before-gate-return')
        for _ in range(24):
            if g.st()['map_name']=='SafariZoneGate': break
            tap('down',10)
        g.step(40)
        record('returned-through-normal-door')
        assert g.st()['map_name']=='SafariZoneGate',g.st()
        for _ in range(3):tap('down',10)
        record('return-gate-dialogue')
        for _ in range(20):
            if g.st().get('choice'):break
            tap('a',15)
        assert g.st().get('choice'),g.st()
        tap('down',10);tap('a',10)
        for _ in range(70):
            s=g.st()
            if s['map_name']=='SafariZoneCenter': break
            if s.get('dialogue_state'):tap('a',15)
            else:tap('up',15)
        g.step(40)
        record('rejoin-after-gate-return')
        assert g.st()['map_name']=='SafariZoneCenter',g.st()
        assert g.st()['money']==2500,g.st()
        menu_shot('after-gate-return')
        results.append({'repeat':repeat+1,'trace':trace})
    finally: g.close()
(OUT/'safari-observations.json').write_text(json.dumps(results,indent=2))
print('Evidence:',OUT)
