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


def selected_files(source):
    # Chapter encodes are render inputs; the players use the continuous originals.
    files = []
    for path in sorted(source.rglob('*')):
        rel = path.relative_to(source)
        if not path.is_file() or len(rel.parts) > 2:
            continue
        if len(rel.parts) == 2 and rel.parts[0] != 'full-run':
            continue
        if re.fullmatch(r'(jev|script)-\d\d-x\d+\.mp4', path.name):
            continue
        if path.suffix not in {'.css', '.js', '.json', '.png', '.svg', '.mp4', '.patch'}:
            if str(rel) not in REQUIRED:
                continue
        if path.name in {'hyperframes.json', 'package.json'}:
            continue
        files.append(path)
    return files


def stage(repo, site, revision):
    source = repo / ASSETS
    for name in REQUIRED:
        if not (source / name).is_file():
            raise ValueError(f'Missing dashboard dependency: {name}')
    files = selected_files(source)
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
