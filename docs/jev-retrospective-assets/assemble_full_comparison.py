#!/usr/bin/env python3
"""Publish complete recordings and a chronological, milestone-aligned overview.

The 60 fps originals are concatenated without re-encoding. Derived overview
clips cover every source interval in order, with explicit constant acceleration.
"""
import argparse
import hashlib
import json
import math
import subprocess
from pathlib import Path

from analyze_full_playthrough import rows
from compose_dashboard_comparison import compose


def run(*args):
    subprocess.run([str(a) for a in args], check=True)


def probe(path):
    return json.loads(subprocess.check_output([
        'ffprobe', '-v', 'error', '-show_entries',
        'format=duration,size:stream=width,height,r_frame_rate,nb_frames',
        '-of', 'json', str(path)], text=True))


def seconds(path):
    return float(probe(path)['format']['duration'])


def checksum(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def readable_end_frame(video, end):
    """Freeze the last legible frame near a chapter boundary, not a fade."""
    start = max(0, end - 2)
    raw = subprocess.check_output([
        'ffmpeg', '-hide_banner', '-loglevel', 'error', '-nostdin',
        '-ss', f'{start:.9f}', '-t', f'{end-start:.9f}', '-i', str(video),
        '-vf', 'format=gray', '-f', 'rawvideo', 'pipe:1'])
    size = 160 * 144
    for index in range(len(raw) // size - 1, -1, -1):
        pixels = raw[index * size:(index + 1) * size]
        if sum(v < 80 for v in pixels) > 128 and sum(v > 200 for v in pixels) > 128:
            return min(end - 1 / 60, start + index / 60)
    return end - 1 / 60


def anchors(record):
    folder = Path(record['source'])
    meta = json.loads((folder / 'capture.json').read_text())
    assert meta['success'] and len(meta['processes']) == 2, folder
    observations = [r for r in rows(folder / 'observations.jsonl') if r['process'] == 1]
    process_frames = [int(p['video_probe']['streams'][0]['nb_frames']) for p in meta['processes']]
    dex = next(m for m in record['milestones']
               if m.get('milestone') == 'm08' or m.get('objective') == 'get-pokedex')['frame']
    points = [('boot', '真实 NEW GAME', 0), ('pokedex', '领取图鉴', dex)]
    for count, label in [(1, '第一枚徽章'), (2, '第二枚徽章'), (3, '三枚徽章'),
                         (5, '五枚徽章'), (8, '八枚徽章')]:
        row = next(r for r in record['badges'] if int(r['badges']).bit_count() == count)
        points.append((f'badge{count}', label, row['frame_count']))
    league = next(r for r in observations if r['map_name'] == 'LoreleisRoom' and r['badges'] == 255)
    hall = next(p for p in record['first_clear_verification']['phases']
                if p.get('hall_of_fame_count', 0) > 0 and p['phase'][1])
    points += [('league', '进入四天王挑战', league['frame_count']),
               ('hall', '冠军胜利与名人堂', hall['frame']),
               ('main_end', '字幕、自动存档与回到城镇', process_frames[0]),
               ('continue_end', '独立进程 CONTINUE 验证', sum(process_frames))]
    assert all(a[2] < b[2] for a, b in zip(points, points[1:])), points
    return points, meta, process_frames


def assemble_record(record, target):
    name = record['controller']
    points, meta, frames = anchors(record)
    folder = Path(record['source'])
    source_identity = [{'index': p['index'], 'sha256': p['video_sha256']}
                       for p in meta['processes']]
    identity_file = target / f'{name}-sources.json'
    if identity_file.exists():
        assert json.loads(identity_file.read_text()) == source_identity, 'Output belongs to different source recordings'
    else:
        identity_file.write_text(json.dumps(source_identity, indent=2) + '\n')
    concat = target / f'{name}-concat.txt'
    # Paths are emitted into ffconcat syntax, not interpolated into a shell.
    paths = [str((folder / p['video']).resolve()).replace("'", "'\\''") for p in meta['processes']]
    concat.write_text(''.join(f"file '{p}'\n" for p in paths))
    chapters = target / f'{name}-chapters.ffmeta'
    body = [';FFMETADATA1', f'title={name} — NEW GAME to independent CONTINUE',
            'comment=Complete simulated frames at 60 fps. Model wall waits are logged separately.']
    for i, (key, label, frame) in enumerate(points[:-1]):
        title = '独立进程 CONTINUE（新进程）' if key == 'main_end' else label
        body += ['[CHAPTER]', 'TIMEBASE=1/60', f'START={frame}',
                 f'END={points[i + 1][2]}', f'title={title}']
    chapters.write_text('\n'.join(body) + '\n')
    video = target / f'{name}-full.mp4'
    if not video.exists():
        run('ffmpeg', '-hide_banner', '-loglevel', 'warning', '-nostdin',
            '-f', 'concat', '-safe', '0', '-i', concat, '-i', chapters,
            '-map', '0:v:0', '-map_metadata', '1', '-map_chapters', '1',
            '-c', 'copy', '-movflags', '+faststart', video)
    info = probe(video)
    assert int(info['streams'][0]['nb_frames']) == sum(frames), (name, info, frames)
    observations = list(rows(folder / 'observations.jsonl'))
    state_timeline = []
    offset = 0
    for process, frame_count in enumerate(frames, 1):
        previous = None
        for row in observations:
            if row['process'] != process:
                continue
            state = [row['screen'], row['map_name'], row['badges'],
                     [[m['species'], m['level']] for m in row['party']], row['hall_of_fame_count']]
            if state != previous:
                state_timeline.append([(offset + row['frame_count']) / 60, *state])
                previous = state
        offset += frame_count
    return {'controller': name, 'file': video.name, 'sha256': checksum(video),
            'source_commit': meta['source_commit'], 'source_patch_sha256': meta.get('source_patch_sha256'),
            'source_segments': source_identity,
            'state_timeline': state_timeline,
            'probe': info, 'process_frames': frames,
            'anchors': [{'id': k, 'label': label, 'source_s': f / 60, 'frame': f}
                        for k, label, f in points]}


def derive_chapters(records, target, speed):
    chapters = []
    start = 0
    for i in range(len(records[0]['anchors']) - 1):
        a = records[0]['anchors'][i]
        b = records[0]['anchors'][i + 1]
        # Keep the ending and the independent persistence check legible.
        rate = 2 if a['id'] in ('hall', 'main_end') else speed
        chapter = {'id': a['id'], 'label': a['label'] + ' → ' + b['label'],
                   'start': start, 'rate': rate, 'sides': {}}
        for record in records:
            name = record['controller']
            left, right = record['anchors'][i:i + 2]
            source_start, source_end = left['source_s'], right['source_s']
            clip = target / f'{name}-{i:02}-x{rate}.mp4'
            still = target / f'{name}-{i:02}-end.png'
            if not clip.exists() or probe(clip)['streams'][0]['width'] != 480:
                run('ffmpeg', '-hide_banner', '-loglevel', 'warning', '-nostdin',
                    '-y', '-ss', f'{source_start:.9f}', '-t', f'{source_end - source_start:.9f}',
                    '-i', target / record['file'], '-an',
                    '-vf', f'setpts=(PTS-STARTPTS)/{rate},fps=30,scale=480:432:flags=neighbor',
                    '-c:v', 'libx264', '-threads', '2', '-preset', 'fast', '-crf', '18',
                    '-pix_fmt', 'yuv420p', '-movflags', '+faststart', clip)
            still_source_s = readable_end_frame(target / record['file'], source_end)
            run('ffmpeg', '-hide_banner', '-loglevel', 'error', '-nostdin', '-y',
                '-ss', f'{still_source_s:.9f}', '-i', target / record['file'],
                '-frames:v', '1', '-update', '1', still)
            chapter['sides'][name] = {'source_start': source_start, 'source_end': source_end,
                                      'file': clip.name, 'still': still.name,
                                      'still_source_s': still_source_s,
                                      'duration': seconds(clip)}
        # Equal speed within a chapter; freeze the side that reaches its endpoint first.
        chapter['duration'] = math.ceil(max(s['duration'] for s in chapter['sides'].values()) * 30) / 30 + 2
        chapters.append(chapter)
        start += chapter['duration']
    return chapters


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--analysis', type=Path, required=True)
    ap.add_argument('--project', type=Path, default=Path(__file__).resolve().parent)
    ap.add_argument('--speed', type=int, default=32)
    args = ap.parse_args()
    source = json.loads(args.analysis.read_text())
    assert {r['controller'] for r in source} == {'script', 'jev'}
    assert all(r['success'] and r['finished'] and not r['forbidden_progress_commands'] for r in source)
    assert len({r['binary_sha256'] for r in source}) == 1
    target = args.project / 'full-run'
    target.mkdir(exist_ok=True)
    recordings = [assemble_record(r, target) for r in source]
    chapters = derive_chapters(recordings, target, args.speed)
    duration, parts = compose(chapters, recordings, args.project)
    manifest = {'raw_fps': 60, 'overview_fps': 30, 'overview_duration': duration,
                'recordings': recordings, 'chapters': chapters, 'overview_parts': parts,
                'dashboard': 'jev-dashboard.json', 'reading_holds_s': sum(p['duration'] for p in parts if p['hold']),
                'continuity': 'All source intervals partitioned in chronological order; no route interval omitted.',
                'clock': 'Simulated frame time; model wall waits logged separately.',
                'alignment': 'Same rate per chapter. Earlier finisher freezes; raw source order never changes.'}
    (target / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    template = (args.project / 'full-comparison-player.template.html').read_text()
    (target / 'player.html').write_text(template.replace('__MANIFEST__', json.dumps(manifest, ensure_ascii=False).replace('</', '<\\/')))
    (target / 'comparison-analysis.json').write_text(args.analysis.read_text())
    print(json.dumps({'overview_s': duration, 'raw_s': [r['probe']['format']['duration'] for r in recordings]}))


if __name__ == '__main__':
    main()
