#!/usr/bin/env python3
"""Add the recorded Jev dashboard to a Pages tree without replacing the game."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess


ASSETS = Path('docs/jev-retrospective-assets')
LFS_PREFIX = b'version https://git-lfs.github.com/spec/v1'
REQUIRED = (
    'jev-dashboard.css', 'jev-dashboard.js', 'jev-dashboard-i18n.js',
    'full-run/jev-player.html', 'full-run/player.html',
    'full-run/jev-dashboard-data.js', 'full-run/jev-inputs-data.js',
    'full-run/jev-dashboard.json', 'full-run/jev-inputs.json',
    'full-run/manifest.json', 'full-run/comparison-analysis.json',
    'full-run/jev-full.mp4', 'full-run/script-full.mp4',
    'full-run/script-vs-jev-full.mp4',
)
DEX_REQUIRED = (
    'dex-run/jev-dex-player.html', 'dex-run/jev-dex-dashboard-data.js',
    'dex-run/jev-dex-dashboard.json', 'dex-run/manifest.json',
    'dex-run/jev-dex-full.mp4',
)


def selected_files(source, include_dex=False):
    # Chapter encodes are render inputs; the players use the continuous originals.
    files = []
    for path in sorted(source.rglob('*')):
        rel = path.relative_to(source)
        if not path.is_file() or len(rel.parts) > 2:
            continue
        folders = {'full-run', 'dex-run'} if include_dex else {'full-run'}
        if len(rel.parts) == 2 and rel.parts[0] not in folders:
            continue
        if re.fullmatch(r'(jev|script)-\d\d-x\d+\.mp4', path.name):
            continue
        if path.suffix not in {'.css', '.js', '.json', '.png', '.svg', '.mp4', '.patch'}:
            if str(rel) not in REQUIRED + DEX_REQUIRED:
                continue
        if path.name in {'hyperframes.json', 'package.json'}:
            continue
        files.append(path)
    return files


def verify_dex_completion(data):
    """The requested full-run publication is not a partial-run preview."""
    target = data.get('target') or {}
    audit = data.get('collection_audit') or {}
    progress = data.get('progress') or []
    if (data.get('schema', 0) < 3 or (data.get('run') or {}).get('success') is not True
            or target.get('solo_ceiling') != 124 or target.get('owned') != 124
            or target.get('validated_owned') != 124
            or target.get('pending_source_validation') != []
            or audit.get('pending_species') != [] or not progress):
        raise ValueError('Pokédex run is not a verified 124-species completion')
    species = data.get('species') or []
    owned = {mon['name'] for mon in species if mon.get('status') == 'owned'}
    last = progress[-1]
    names = last.get('owned_species') or []
    proof = (data.get('run') or {}).get('collection_continue_verification') or {}
    restored = (proof.get('restored') or {}).get('dex') or {}
    snapshot = proof.get('restored') or {}
    saved_state = snapshot.get('state')
    safari = saved_state.get('safari_game') if isinstance(saved_state, dict) else None
    party, pp = snapshot.get('party'), snapshot.get('party_pp')
    party_pp_complete = (isinstance(party, list) and 1 <= len(party) <= 6
                         and isinstance(pp, list) and len(pp) == len(party)
                         and all(isinstance(row, list) and len(row) == 4
                                 and all(type(value) is int and 0 <= value <= 255 for value in row)
                                 for row in pp))
    if (proof.get('schema') != 4 or proof.get('verified') is not True
            or not {'dex', 'state', 'party', 'party_pp', 'bag', 'flags', 'stored_pokemon'} <= snapshot.keys()
            or not party_pp_complete
            or not isinstance(snapshot.get('stored_pokemon'), list)
            or not isinstance(safari, dict) or type(safari.get('active')) is not bool
            or not all(type(safari.get(key)) is int and 0 <= safari[key] <= limit
                       for key, limit in (('balls_remaining', 30), ('steps_remaining', 500)))
            or proof.get('expected') != proof.get('restored')
            or not re.fullmatch(r'[0-9a-f]{64}', proof.get('save_sha256') or '')
            or restored.get('owned') != 124 or set(restored.get('owned_species') or []) != owned):
        raise ValueError('Pokédex completion lacks matching separate-process CONTINUE evidence')
    if (len(species) != 151 or {mon.get('number') for mon in species} != set(range(1, 152))
            or len({mon['name'] for mon in species}) != 151 or len(owned) != 124
            or len(names) != 124 or set(names) != owned
            or last.get('owned') != 124 or last.get('validated_owned') != 124
            or last.get('pending_source_validation') != []
            or any(mon.get('status') not in ('owned', 'unreachable') for mon in species)):
        raise ValueError('Pokédex completion disagrees with catalog or final progress evidence')


def stage(repo, site, revision):
    source = repo / ASSETS
    for name in REQUIRED:
        if not (source / name).is_file():
            raise ValueError(f'Missing dashboard dependency: {name}')
    # The player template exists before a run is complete. Only publish it
    # together with generated data and recording; partial delivery is an error.
    include_dex = any((source / name).exists() for name in DEX_REQUIRED[1:])
    if include_dex:
        for name in DEX_REQUIRED:
            if not (source / name).is_file():
                raise ValueError(f'Missing Pokédex dashboard dependency: {name}')
    files = selected_files(source, include_dex)
    for path in files:
        with path.open('rb') as stream:
            if stream.read(len(LFS_PREFIX)) == LFS_PREFIX:
                raise ValueError(f'Unresolved Git LFS pointer: {path}; run git lfs pull')
    manifest = json.loads((source / 'full-run/manifest.json').read_text())
    for record in manifest['recordings']:
        path = source / 'full-run' / record['file']
        with path.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        if digest != record['sha256']:
            raise ValueError(f'Recording checksum mismatch: {path}')
    if include_dex:
        dex_manifest = json.loads((source / 'dex-run/manifest.json').read_text())
        dex_data = json.loads((source / 'dex-run/jev-dex-dashboard.json').read_text())
        path = source / 'dex-run/jev-dex-full.mp4'
        record = dex_manifest['files']['jev-dex-full.mp4']
        with path.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        if digest != record['sha256'] or digest != dex_data['run']['video_sha256']:
            raise ValueError(f'Recording checksum mismatch: {path}')
        if path.stat().st_size != record['bytes']:
            raise ValueError(f'Recording size mismatch: {path}')
        verify_dex_completion(dex_data)
        runtime = (source / 'dex-run/jev-dex-dashboard-data.js').read_text().strip()
        payload = re.fullmatch(r'window\.JEV_DEX_DASHBOARD=(.*);', runtime, re.S)
        if not payload or json.loads(payload[1]) != dex_data:
            raise ValueError('Pokédex runtime data disagrees with audited JSON')
    target = site / 'jev-dashboard'
    existing_size = sum(p.stat().st_size for p in site.rglob('*')
                        if p.is_file() and not p.is_relative_to(target))
    # Leave room below GitHub Pages' 1 GB published-site limit.
    if existing_size + sum(p.stat().st_size for p in files) > 950_000_000:
        raise ValueError('Combined Pages site exceeds the 950 MB deployment budget')
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    for path in files:
        dest = target / path.relative_to(source)
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, dest)
    retrospective = ('https://github.com/liuyanghejerry/open-pokered/blob/'
                     f'{revision}/docs/jev-autonomous-retrospective.md')
    for name in ['jev-player.html', 'player.html']:
        dest = target / 'full-run' / name
        content = dest.read_text().replace('../../jev-autonomous-retrospective.md', retrospective)
        # The shared locale script supplies navigation for both hosted and local pages.
        dest.write_text(content)
    (target / 'index.html').write_text('''<!doctype html><html lang="zh-CN"><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Jev 完整通关 · 同步决策大盘</title>
<meta http-equiv="refresh" content="0;url=full-run/jev-player.html">
<a href="full-run/jev-player.html">打开 Jev 同步决策大盘</a>
<script>location.replace('full-run/jev-player.html'+location.search+location.hash)</script>
</html>\n''')
    videos = {}
    for path in files:
        if path.suffix == '.mp4':
            with path.open('rb') as stream:
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
            videos[str(path.relative_to(source))] = {'bytes': path.stat().st_size, 'sha256': digest}
    report = {'source_commit': revision, 'path': 'jev-dashboard/',
              'videos': videos, 'bytes': sum(p.stat().st_size for p in target.rglob('*') if p.is_file()),
              'existing_site_bytes': existing_size}
    (target / 'deployment.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path('.'))
    parser.add_argument('--site', type=Path, required=True)
    args = parser.parse_args()
    revision = subprocess.check_output(['git', '-C', str(args.repo), 'rev-parse', 'HEAD'], text=True).strip()
    stage(args.repo.resolve(), args.site.resolve(), revision)
