#!/usr/bin/env python3
"""Check dashboard evidence against the completed recording's original logs."""
import argparse
import collections
import json
from pathlib import Path
from analyze_full_playthrough import rows


def verify(folder, output):
    data=json.loads((output/'jev-dashboard.json').read_text())
    raw=json.loads((output/'jev-inputs.json').read_text())
    trace=list(rows(next((folder/'run').glob('*/trace.jsonl'))))
    judgments=[r for r in trace if r['kind']=='judgment']
    assert len(raw)==len(judgments)==len(data['strategy_inputs'])+len(data['action_inputs'])
    for i,r in enumerate(judgments,1):
        saved=raw[str(i)]
        for key in ('state','question','answer','model','layer','elapsed_s','input_tokens','output_tokens'):
            assert saved[key]==r[key], (i,key)
    for layer in ('strategy','action'):
        for stamp,summary in data[layer+'_inputs']:
            record=raw[str(summary['id'])]
            assert stamp==record['source_s'] and record['layer']==layer
            assert summary['candidate_count']==len(record['question']['criteria'])
            assert summary['state_fields']==list(record['state'])
    for key in ('states','strategies','actions','outcomes','counts','strategy_inputs','action_inputs','commands'):
        assert all(a[0]<=b[0] for a,b in zip(data[key],data[key][1:])), key
    command_map={(r['process'],r['start_s']):r for r in rows(folder/'commands.jsonl')}
    for stamp,record in data['commands']:
        matches=[r for p in (1,2) if (r:=command_map.get((p,record['start_wall_s']))) is not None]
        assert len(matches)==1 and matches[0]['request']==record['request']
        if record['cmd']=='skip_dialogue':assert '调试命令' in record['label']
    assert data['clock_alignment']['battle_anchor_frame_mismatches']==0
    counts=collections.Counter(e['kind'] for key in ('strategy_inputs','action_inputs') for _,e in data[key])
    report={'passed':True,'judgments_matched':len(judgments),'input_types':dict(counts),'commands_matched':len(data['commands']),
            'battle_anchor_frame_mismatches':0,'scope':'Application input/output parity, chronology and recorded command provenance; not exact button-down frame timing.'}
    (output/'dashboard-verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(report,ensure_ascii=False))

if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('folder',type=Path);ap.add_argument('--output',type=Path,required=True)
    a=ap.parse_args();verify(a.folder,a.output)
