#!/usr/bin/env python3
"""Summarize full-recording evidence without inventing unobserved outcomes."""
import argparse
import collections
import json
import re
import statistics
from pathlib import Path


def rows(path):
    with path.open() as stream:
        for line in stream:
            try:
                yield json.loads(line)
            except json.JSONDecodeError:
                continue  # A live file can have one incomplete final line.


def analyze(folder):
    metadata_path = folder/'capture.json'
    metadata = json.loads((metadata_path if metadata_path.exists() else folder/'capture-start.json').read_text())
    observations = list(rows(folder/'observations.jsonl'))
    primary = [r for r in observations if r['process'] == 1]
    result = {'controller': metadata['controller'], 'source': str(folder),
              'finished': metadata_path.exists(), 'success': metadata.get('success', False),
              'seed': metadata['seed'], 'binary_sha256': metadata['binary_sha256'],
              'source_commit': metadata['source_commit'],
              'source_patch_sha256': metadata.get('source_patch_sha256'),
              'controller_clock': metadata['controller_clock'],
              'wall_s': metadata.get('wall_s'), 'observations': len(primary),
              'milestones': [], 'badges': [], 'observed_battle_entries': 0,
              'observed_party_wipe_episodes': [], 'maps_in_first_visit_order': [],
              'map_first_visits': [], 'battle_episodes': [], 'team_composition_changes': []}
    seen_maps = set()
    previous_badges = 0
    previous_screen = None
    episode = 0
    wiped = set()
    battle = None
    last_team = ()
    for r in primary:
        name = r.get('map_name')
        if name and r['screen'] in ('overworld', 'battle') and name not in seen_maps:
            result['maps_in_first_visit_order'].append(name)
            result['map_first_visits'].append({k:r[k] for k in ['map_name','wall_s','frame_count']})
            seen_maps.add(name)
        team = tuple(m['species'] for m in r.get('party', []))
        if team and team != last_team:
            result['team_composition_changes'].append({k:r[k] for k in ['wall_s','frame_count','map_name','party']})
        last_team = team
        if r['screen'] == 'battle' and previous_screen != 'battle':
            episode += 1
            battle = {'episode': episode, 'map': name, 'start_wall_s': r['wall_s'],
                      'start_frame': r['frame_count'], 'entry_party': r['party'],
                      'opponents': [], 'party_wipe_observed': False,
                      'trainer_phase_observed': False, 'observed_result': None}
            result['battle_episodes'].append(battle)
        if battle is not None:
            battle.update(end_wall_s=r['wall_s'], end_frame=r['frame_count'])
            if r['screen'] != 'battle':
                battle = None
        previous_screen = r['screen']
        live = r.get('battle_live') or {}
        party = live.get('player_party') or []
        if battle is not None:
            if live.get('enemy_party'):
                battle['opponents'] = [{'species':m['species'],'level':m['level']} for m in live['enemy_party']]
            phase = r.get('battle_phase') or ''
            battle['trainer_phase_observed'] |= 'Trainer' in phase
            if 'player_won: true' in phase:
                battle['observed_result'] = 'won'
            elif 'player_won: false' in phase:
                battle['observed_result'] = 'lost'
        if r['screen'] == 'battle' and party and all(m.get('hp', 1) == 0 for m in party) and episode not in wiped:
            wiped.add(episode)
            battle['party_wipe_observed'] = True
            result['observed_party_wipe_episodes'].append({k:r[k] for k in ['wall_s','frame_count','map_name','battle_phase']})
        if r.get('badges', 0) != previous_badges:
            result['badges'].append({k:r[k] for k in ['wall_s','frame_count','map_name','badges','party']})
            previous_badges = r.get('badges', 0)
    result['observed_battle_entries'] = episode
    result['last_observation'] = primary[-1] if primary else None
    result['empty_new_game_observation'] = next((r for r in primary if r['map_name']=='RedsHouse2F' and not r['party'] and not r['badges']),None)
    counts = collections.Counter()
    commands_wall = 0
    for r in rows(folder/'commands.jsonl'):
        counts[r['request']['cmd']] += 1
        commands_wall += r['end_s']-r['start_s']
    result['commands'] = dict(counts)
    result['protocol_roundtrip_wall_s'] = round(commands_wall,3)
    prohibited = {'warp','give_item','give_pokemon','set_flag','restore_state','start_wild_battle','start_trainer_battle','save'}
    result['forbidden_progress_commands'] = sorted(prohibited & counts.keys())
    if metadata['controller'] == 'jev':
        run = next((folder/'run').glob('*'))
        summary = json.loads((run/'summary.json').read_text()) if (run/'summary.json').exists() else {}
        judgments = collections.Counter()
        input_tokens = collections.Counter()
        output_tokens = collections.Counter()
        latency = collections.Counter()
        latency_samples = collections.defaultdict(list)
        strategies = collections.Counter()
        milestones = []
        defeats = []
        for r in rows(run/'trace.jsonl'):
            if r['kind']=='judgment':
                layer=r['layer'];judgments[layer]+=1
                input_tokens[layer]+=r.get('input_tokens',0);output_tokens[layer]+=r.get('output_tokens',0)
                latency[layer]+=r.get('latency_s',0)
                latency_samples[layer].append(r.get('latency_s',0))
            elif r['kind']=='strategy': strategies[r['target'][0]]+=1
            elif r['kind']=='milestone': milestones.append(r)
            elif r['kind']=='battle_defeat': defeats.append(r)
        result.update(model_judgments=dict(judgments), input_tokens=dict(input_tokens), output_tokens=dict(output_tokens),
                      judgment_latency_s={k:round(v,3) for k,v in latency.items()},strategy_target_types=dict(strategies),
                      milestones=milestones, reported_battle_defeats=defeats,
                      first_clear_verification=summary.get('first_clear_verification'),
                      policy_sha256=summary.get('policy_sha256'), final_facts=summary.get('final_facts'),
                      actions=summary.get('actions'), action_cache_hits=summary.get('action_cache_hits'))
        result['judgment_latency_distribution_s'] = {
            layer: {'median': round(statistics.median(values),3),
                    'p95': round(sorted(values)[min(len(values)-1,int(len(values)*.95))],3),
                    'max': round(max(values),3)}
            for layer,values in latency_samples.items() if values}
    else:
        for p in sorted((folder/'milestones').glob('m[0-9][0-9].json')):
            d=json.loads(p.read_text());s=d['get_state']['data']
            nearest=min(primary,key=lambda r:abs(r['frame_count']-s['frame_count'])) if primary else {}
            result['milestones'].append({'milestone':p.stem,'frame':s['frame_count'],
                                         'wall_s':nearest.get('wall_s'),'map':s['map_name'],
                                         'party':s['party'],'badges':s['badges']})
            if 'first_clear_verification' in d:result['first_clear_verification']=d['first_clear_verification']
    result['video_segments'] = [{k:v for k,v in p.items() if k in ['index','video','video_probe','video_sha256','input_save_sha256']} for p in metadata['processes']]
    result['measurement_notes'] = [
        'Battle entries and party wipes are counts observed at protocol boundaries, not engine-internal totals.',
        'Wall time includes recording overhead; concurrent capture runs share host resources.',
        'Model latency is summed from judgment trace; protocol roundtrip time excludes unobserved Python planning time.',
        'Raw recordings contain every simulated update at 60 fps. Stationary wall-clock model waits are in command/trace timestamps.',
    ]
    return result


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('folders',type=Path,nargs='+')
    ap.add_argument('--output',type=Path,required=True)
    args=ap.parse_args()
    result=[analyze(p.resolve()) for p in args.folders]
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    for r in result:
        print(r['controller'], 'finished=',r['finished'], 'success=',r['success'],
              'badges=',len(r['badges']), 'observed battles=',r['observed_battle_entries'],
              'wipes=',len(r['observed_party_wipe_episodes']))


if __name__=='__main__':
    main()
