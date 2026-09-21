#!/usr/bin/env python3
"""Check fresh start, natural ending, independent save reload, and video evidence."""
import argparse
import hashlib
import json
from pathlib import Path

from analyze_full_playthrough import analyze, rows


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(folder):
    capture = json.loads((folder / 'capture.json').read_text())
    analysis = analyze(folder)
    assert capture['success'] and capture['scope'] == 'full clear and independent CONTINUE', capture
    assert len(capture['processes']) == 2
    assert not capture['processes'][0]['initial_save_exists']
    assert capture['processes'][1]['initial_save_exists']
    assert analysis['empty_new_game_observation'] is not None
    assert not analysis['forbidden_progress_commands']
    assert len(analysis['badges']) == 8 and analysis['badges'][-1]['badges'] == 255
    verification = analysis['first_clear_verification']
    phases = verification['phases']
    assert any(p['phase'][1] for p in phases), 'Hall of Fame phase not observed'
    assert any(p['phase'][2] == 'TheEnd' for p in phases), 'TheEnd not observed'
    assert any(p['phase'][0] == 'title' for p in phases), 'Return to title not observed'
    resumed = verification['separate_process_continue']
    assert resumed['screen'] == 'overworld' and resumed['map_name'] == 'PalletTown'
    assert resumed['badges'] == 255 and resumed['hall_of_fame_count'] == 1
    save = folder / 'process-01/output.sav'
    independent_input = folder / 'process-02/input.sav'
    assert save.stat().st_size == independent_input.stat().st_size == 32768
    digest = sha(save)
    assert digest == sha(independent_input) == capture['processes'][1]['input_save_sha256']
    if 'autosave_sha256' in verification:
        assert digest == verification['autosave_sha256']
    observed = list(rows(folder / 'observations.jsonl'))
    video_checks = []
    for process in capture['processes']:
        stream = process['video_probe']['streams'][0]
        assert stream['r_frame_rate'] == '60/1'
        assert (stream['width'], stream['height']) == (160, 144)
        assert int(stream['nb_frames']) > 0
        last = max(r['frame_count'] for r in observed if r['process'] == process['index'])
        # Report the recorder/observer boundary explicitly instead of hiding it.
        delta = int(stream['nb_frames']) - last
        assert delta >= -1, f'Video ends before the last observed state: {delta}'
        video_checks.append({'process': process['index'], 'frames': int(stream['nb_frames']),
                             'last_observed_frame': last, 'recorded_minus_observed_frames': delta,
                             'duration_s': float(process['video_probe']['format']['duration']),
                             'sha256': process['video_sha256']})
    if capture['controller'] == 'script':
        assert len(analysis['milestones']) == 49
    else:
        summary = json.loads(next((folder / 'run').glob('*/summary.json')).read_text())
        assert summary['success'] and not summary['uses_milestone_handlers']
        assert summary['final_observations_valid']
        assert not summary.get('development_checkpoint'), 'Development checkpoint present'
    return {'controller': capture['controller'], 'attempt': folder.name, 'passed': True,
            'source_commit': capture['source_commit'], 'source_patch_sha256': capture.get('source_patch_sha256'),
            'binary_sha256': capture['binary_sha256'], 'fresh_new_game': True,
            'prohibited_progress_commands': [], 'hall_of_fame': True, 'the_end': True,
            'independent_continue': {'map': resumed['map_name'], 'badges': resumed['badges'],
                                     'hall_of_fame_count': resumed['hall_of_fame_count']},
            'natural_autosave_sha256': digest, 'videos': video_checks}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('folders', type=Path, nargs='+')
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    checks = [verify(folder.resolve()) for folder in args.folders]
    assert len({c['binary_sha256'] for c in checks}) == 1
    args.output.write_text(json.dumps(checks, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps([{'attempt': c['attempt'], 'passed': c['passed']} for c in checks]))


if __name__ == '__main__':
    main()
