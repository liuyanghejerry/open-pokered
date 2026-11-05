from pathlib import Path
import gzip,hashlib,json
p=Path(__file__).resolve().parent
v=json.loads((p/'verification.json').read_text())
for case in v['cases']:
 name=case['case']
 before=json.loads(gzip.decompress((p/f'{name}-before.json.gz').read_bytes()))
 after=json.loads(gzip.decompress((p/f'{name}-after.json.gz').read_bytes()))
 original=json.loads(gzip.decompress((p/f'{name}-original.json.gz').read_bytes()))
 gba=json.loads(gzip.decompress((p/f'{name}-gba.json.gz').read_bytes()))
 assert before['images']['initial']==after['images']['initial']
 for data in [before,after]:
  fs=data['frames'];assert len(fs)==80
  assert [f['frame_count']for f in fs]==list(range(fs[0]['frame_count'],fs[0]['frame_count']+80))
  assert all((f['player_x'],f['player_y'],f['player_facing'])==(fs[0]['player_x'],fs[0]['player_y'],fs[0]['player_facing'])for f in fs)
 assert next(i for i,f in enumerate(after['frames'])if (f['field_menu']or{}).get('ready_for_input'))==9
 assert after['final']['field_menu']['cursor']==after['selected_start']['cursor']
 assert [f['tick']for f in original['frames']]==list(range(80))
 assert next(e['tick']for e in original['events']if e['routine']=='Joypad'and e['tick']>5)==9
 assert len(gba['frames'])==800 and gba['status']=='passed'
 assert [f['physical_frame']for f in gba['frames']]==list(range(gba['origin']+1,gba['origin']+801))
 assert gba['committed_sha256']==gba['fixture_sha256']
 assert gba['before_b']['pixels_sha256']==gba['after_reopen']['pixels_sha256']
print('PASS: committed raw-time input, frame, cursor, storage and completion evidence')
