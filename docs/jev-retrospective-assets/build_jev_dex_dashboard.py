#!/usr/bin/env python3
"""Build the frame-aligned solo-Pokédex dashboard from an autonomous run."""
import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from openpokered.collection_planner import (  # noqa: E402
    SUPER_ROD_MAP_GROUP, complete_acquisition_graph, infer_solo_choices, solo_plan,
)
from openpokered.story_rules import MAPS_DIR  # noqa: E402


def rows(path):
    with path.open() as stream:
        for line in stream:
            if line.strip():
                yield json.loads(line)


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def method(event):
    explicit = event.get('acquisition_method')
    if explicit:
        return explicit
    # Legacy traces did not label every capture. Absence of evidence is not
    # evidence of a gift (and a resumed initial snapshot is not an acquisition).
    return 'unknown'


def load_chain(run, follow_parents=False):
    """Only follow the checkpoint lineage, never unrelated failed attempts."""
    segments, visited = [], set()
    while True:
        run = run.resolve()
        if run in visited:
            raise ValueError(f'cyclic checkpoint lineage: {run}')
        visited.add(run)
        summary = json.loads((run / 'summary.json').read_text())
        segments.append({'run': run, 'summary': summary,
                         'trace': list(rows(run / 'trace.jsonl'))})
        parent = summary.get('resumed_from')
        if not follow_parents or not parent:
            break
        run = Path(parent)
        if not run.is_absolute():
            run = ROOT / run
    segments.reverse()
    for segment in segments[:-1]:
        if not segment['summary'].get('development_checkpoint'):
            raise ValueError(f'parent is not a checkpoint: {segment["run"]}')
    return segments


def video_duration(path):
    return float(subprocess.check_output([
        'ffprobe', '-v', 'error', '-show_entries', 'format=duration',
        '-of', 'default=noprint_wrappers=1:nokey=1', str(path)], text=True))


def merge_traces(segments):
    """Offsets use clip durations, not wall time or the last observed frame."""
    merged, boundaries = [], []
    offset, registered, milestones = 0.0, set(), set()
    for index, segment in enumerate(segments):
        trace, fps, duration = segment['trace'], segment['fps'], segment['duration_s']
        if fps <= 0 or duration < 0:
            raise ValueError('invalid recording clock')
        boundaries.append({'segment': index, 'source_run': str(segment['run']),
                           'source_s': round(offset, 6), 'duration_s': duration,
                           'video_fps': fps})
        next_frames = [None] * len(trace)
        following = None
        for position in range(len(trace) - 1, -1, -1):
            next_frames[position] = following
            if trace[position].get('frame') is not None:
                following = trace[position]['frame']
        previous = 0
        for position, original in enumerate(trace):
            event = dict(original)
            frame = event.get('frame')
            if frame is not None:
                if frame < previous or frame / fps > duration + 1 / 60:
                    raise ValueError(f'trace frame outside recording clock: {segment["run"]}')
                previous = frame
                event['timing_basis'] = 'engine_frame'
            else:
                frame = previous
                upper = next_frames[position]
                event['timing_basis'] = 'preceding_frame_estimate'
                event['source_interval_s'] = [round(offset + frame / fps, 6),
                    round(offset + (upper / fps if upper is not None else duration), 6)]
            event['source_s'] = round(offset + frame / fps, 6)
            event['segment'] = index
            if event.get('kind') == 'dex_progress':
                owned = set(event.get('owned_species') or [])
                if event.get('owned', len(owned)) != len(owned):
                    raise ValueError('dex count disagrees with species list')
                if not registered <= owned:
                    raise ValueError('registered species lost across checkpoint lineage')
                event['acquired'] = sorted(owned - registered)
                registered = owned
            elif event.get('kind') == 'milestone':
                key = (event.get('objective'), event.get('flag'))
                if key in milestones:
                    continue
                milestones.add(key)
            merged.append(event)
        final = (segment.get('summary') or {}).get('final_dex')
        if final and set(final.get('owned_species', [])) != registered:
            raise ValueError('final dex disagrees with trace')
        offset += duration
    return merged, boundaries


def total_metrics(segments, field):
    counts = Counter()
    for segment in segments:
        counts.update(segment['summary'].get(field) or {})
    return dict(counts)


def collection_audit(trace, summary):
    """Annotate native registrations; preserve invalid events and later remedies.

    Never interpret a resumed initial snapshot as an acquisition. A known bad
    source cannot be cleared just because a later snapshot still owns it.
    """
    pending, history, previous = {}, [], None
    for event in trace:
        if event.get('kind') == 'dex_progress':
            owned = set(event.get('owned_species', []))
            if (previous is not None and 'Marowak' not in previous
                    and 'Marowak' in event.get('acquired', [])
                    and event.get('map') == 'PokemonTower6F'):
                evidence = {'species': 'Marowak', 'reason': 'uncatchable_restless_soul',
                    'source_s': event['source_s'], 'segment': event['segment'],
                    'map': event['map'], 'status': 'invalid_source'}
                pending['Marowak'] = evidence
                history.append(evidence)
            previous = owned
            event['validated_owned'] = len(owned - set(pending))
            event['pending_source_validation'] = sorted(pending)
        elif event.get('kind') == 'collection_audit_resolved':
            species = event.get('species')
            if species not in pending:
                raise ValueError('audit resolution has no preceding invalid acquisition')
            before, after = event.get('before') or {}, event.get('after') or {}
            normalize = lambda name: str(name).replace('_', '').upper()
            if (species != 'Marowak' or event.get('acquisition_method') != 'evolution'
                    or normalize(before.get('species')) != 'CUBONE'
                    or normalize(after.get('species')) != 'MAROWAK'
                    or not isinstance(before.get('level'), int)
                    or not isinstance(after.get('level'), int)
                    or not before['level'] < after['level'] or after['level'] < 28):
                raise ValueError('audit resolution lacks valid native evolution evidence')
            pending.pop(species)
            history.append({'species': species, 'source_s': event['source_s'],
                'segment': event['segment'], 'status': 'resolved',
                'acquisition_method': 'evolution', 'before': before, 'after': after})
            # This is a real acquisition even though the native owned bit was
            # already set. Give the player a progress waypoint at the remedy.
            event['validated_owned'] = len((previous or set()) - set(pending))
            event['pending_source_validation'] = sorted(pending)
    if summary.get('collection_audit_schema') == 1 and set(
            summary.get('collection_audit_pending', {})) != set(pending):
        raise ValueError('final source audit disagrees with trace lineage')
    return {'pending_species': sorted(pending), 'history': history,
            'validity_scope': 'Native registrations minus evidenced invalid sources; unknown legacy methods remain unknown.'}


def build(run, output, video=None, chain=False):
    segments = load_chain(run, chain)
    summary = segments[-1]['summary']
    recording = summary.get('recording') or {}
    fps = int(recording.get('game_frames_per_video_second', 240))
    source_video = Path(video or recording.get('path') or run / 'jev-dex-full.mp4').resolve()
    if not source_video.is_file():
        raise ValueError(f'missing recording: {source_video}')
    if len(segments) > 1 and video is None:
        raise ValueError('--chain requires --video pointing to the assembled lineage recording')
    for segment in segments:
        metadata = segment['summary'].get('recording') or {}
        clip = Path(metadata.get('path') or segment['run'] / 'jev-dex-full.mp4')
        if not clip.is_absolute():
            clip = ROOT / clip
        segment['fps'] = int(metadata.get('game_frames_per_video_second', 240))
        segment['duration_s'] = video_duration(clip)
        segment['video'] = {'path': str(clip.resolve()), 'sha256': sha256(clip),
                            'bytes': clip.stat().st_size}
    duration = sum(segment['duration_s'] for segment in segments)
    if abs(video_duration(source_video) - duration) > max(0.1, len(segments) / 60):
        raise ValueError('assembled video duration disagrees with checkpoint lineage')
    trace, boundaries = merge_traces(segments)
    audit = collection_audit(trace, summary)

    maps = {path.parent.name: json.loads(path.read_text())
            for path in MAPS_DIR.glob('*/map.json')}
    graph = complete_acquisition_graph(maps, SUPER_ROD_MAP_GROUP)
    final_dex = summary.get('final_dex') or (summary.get('final_facts') or {}).get('dex') or {}
    owned = set(final_dex.get('owned_species', []))
    validated = owned - set(audit['pending_species'])
    plan = solo_plan(graph, owned, infer_solo_choices(owned))
    build_source = (ROOT / 'crates/pokered-data/build.rs').read_text()
    order_body = build_source.split('const SPECIES_ORDER:', 1)[1].split('];', 1)[0]
    catalog = list(enumerate(re.findall(r'"([A-Za-z0-9]+)"', order_body), 1))

    stamp = lambda event: event['source_s']
    progress, decisions, operations, milestones = [], [], [], []
    counts = Counter()
    for event in trace:
        kind = event.get('kind')
        if kind == 'dex_progress':
            acquired = event.get('acquired') or []
            acquisition = method(event)
            counts[acquisition] += len(acquired)
            progress.append({
                'source_s': stamp(event), 'frame': event.get('frame'),
                'owned': event.get('owned', len(event.get('owned_species', []))),
                'seen': event.get('seen', 0), 'acquired': acquired,
                'owned_species': event.get('owned_species', []), 'map': event.get('map'),
                'validated_owned': event['validated_owned'],
                'pending_source_validation': event['pending_source_validation'],
                'method': acquisition, 'party_count': event.get('party_count', 0),
                'stored_count': event.get('stored_count', 0),
                'method_counts': dict(counts),
            })
        elif kind == 'collection_audit_resolved':
            progress.append({'source_s': stamp(event), 'frame': event.get('frame'),
                'owned': progress[-1]['owned'], 'seen': progress[-1]['seen'],
                'owned_species': progress[-1]['owned_species'], 'acquired': [],
                'validated_owned': event['validated_owned'],
                'pending_source_validation': event['pending_source_validation'],
                'source_revalidated': [event['species']], 'method': 'evolution',
                'method_counts': dict(counts)})
        elif kind == 'judgment' and event.get('layer') == 'strategy':
            answer = event.get('answer') or {}
            probabilities = answer.get('probabilities') or {}
            criteria = (event.get('question') or {}).get('criteria') or {}
            decisions.append({
                'source_s': stamp(event), 'choice': answer.get('choice'),
                'confidence': answer.get('confidence'),
                'candidates': [
                    {'id': key, 'label': criteria.get(key, key), 'probability': value}
                    for key, value in sorted(probabilities.items(), key=lambda item: -item[1])[:5]
                ],
                'dex_progress': (event.get('state') or {}).get('dex_progress'),
                'shared_strategy_evidence': (event.get('state') or {}).get('shared_strategy_evidence', {}),
            })
        elif kind == 'operation':
            operations.append({'source_s': stamp(event), 'segment': event['segment'],
                               'timing_basis': event['timing_basis'],
                               'source_interval_s': event.get('source_interval_s'),
                               'operation': event.get('operation'),
                               'result': event.get('result'), 'map': event.get('map')})
        elif kind == 'milestone':
            milestones.append({'source_s': stamp(event), 'objective': event.get('objective'),
                               'flag': event.get('flag')})

    data = {
        'schema': 3,
        'run': {
            'success': summary.get('success'), 'reason': summary.get('reason'),
            'seed': summary.get('seed'), 'model': (summary.get('models') or ['jev-1.13.0'])[0],
            'actions': sum(segment['summary'].get('actions') or 0 for segment in segments),
            'calls': total_metrics(segments, 'calls'),
            'tokens': total_metrics(segments, 'tokens'), 'frames': summary.get('frames'),
            'wall_s': summary.get('wall_s'), 'video_fps': fps,
            'segment_count': len(segments), 'video_duration_s': duration,
            'metrics_scope': 'actions/calls/tokens: lineage total; frames/wall_s: final segment only',
            'video_file': 'jev-dex-full.mp4', 'video_sha256': sha256(source_video),
            'video_bytes': source_video.stat().st_size,
        },
        'target': {'owned': len(owned), 'validated_owned': len(validated),
                   'pending_source_validation': audit['pending_species'],
                   'solo_ceiling': plan['ceiling'], 'total': 151,
                   'choices': plan['choices'], 'choice_options': plan['optimal_choices'],
                   'unreachable_species': plan['unreachable_species'],
                   'always_unreachable_species': plan['always_unreachable_species']},
        'species': [{'number': number, 'name': name,
                     'status': 'owned' if name in owned else
                               'reachable' if name in plan['reachable_species'] else 'unreachable'}
                    for number, name in catalog],
        'progress': progress, 'decisions': decisions, 'operations': operations,
        'milestones': milestones,
        'resume_boundaries': boundaries,
        'collection_audit': audit,
    }
    output.mkdir(parents=True, exist_ok=True)
    text = json.dumps(data, ensure_ascii=False, separators=(',', ':'))
    (output / 'jev-dex-dashboard.json').write_text(text + '\n')
    (output / 'jev-dex-dashboard-data.js').write_text(
        'window.JEV_DEX_DASHBOARD=' + text.replace('</', '<\\/') + ';\n')
    target_video = output / 'jev-dex-full.mp4'
    if source_video != target_video.resolve():
        target_video.unlink(missing_ok=True)
        shutil.copy2(source_video, target_video)
    manifest = {
        'source_run': str(run.resolve()),
        # Old summaries lack recorded code revisions. The builder's revision
        # must never be presented as the revision used by every gameplay clip.
        'builder_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'files': {'jev-dex-full.mp4': {'bytes': source_video.stat().st_size,
                                      'sha256': sha256(source_video)}},
        'segments': [{**boundary, 'recording': segment['video'],
                      'source_commit': segment['summary'].get('source_commit'),
                      'reason': segment['summary'].get('reason'),
                      'development_checkpoint': segment['summary'].get('development_checkpoint', False)}
                     for boundary, segment in zip(boundaries, segments)],
        'clock': 'video seconds = preceding clip durations + segment engine frame / segment fps',
        'legacy_operation_clock': 'preceding-frame estimate; source_interval_s brackets uncertainty',
    }
    (output / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'progress_events': len(progress), 'decisions': len(decisions),
                      'operations': len(operations), 'owned': len(owned),
                      'video': str(target_video)}, ensure_ascii=False))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    parser.add_argument('--output', type=Path,
                        default=ROOT / 'docs/jev-retrospective-assets/dex-run')
    parser.add_argument('--video', type=Path)
    parser.add_argument('--chain', action='store_true',
                        help='include checkpoint ancestors; requires an assembled --video')
    args = parser.parse_args()
    build(args.run.resolve(), args.output.resolve(), args.video, args.chain)
