#!/usr/bin/env python3
"""Verify or restore audit evidence; never needs the game or original ROM."""
from pathlib import Path, PurePosixPath
import argparse, hashlib, json, zipfile

p = argparse.ArgumentParser()
p.add_argument('--verify-only', action='store_true')
p.add_argument('--output', type=Path)
a = p.parse_args()
root = Path(__file__).resolve().parent
metadata = json.loads((root / 'archive.json').read_text())
archive = root / metadata['archive']
sha = lambda b: hashlib.sha256(b).hexdigest()
assert sha(archive.read_bytes()) == metadata['sha256']
with zipfile.ZipFile(archive) as z:
    manifest = json.loads(z.read('manifest.json'))
    assert manifest['schema'] == 1
    assert len(manifest['files']) == metadata['original_path_count']
    blobs = {}
    for name, info in manifest['files'].items():
        path = PurePosixPath(name)
        assert not path.is_absolute() and '..' not in path.parts
        digest = info['sha256']
        if digest not in blobs:
            data = z.read('blobs/' + digest)
            assert sha(data) == digest
            blobs[digest] = data
        data = blobs[digest]
        assert len(data) == info['bytes']
        if not a.verify_only:
            if a.output is None: p.error('--output is required without --verify-only')
            target = a.output.joinpath(*path.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            assert sha(target.read_bytes()) == digest
    assert len(blobs) == metadata['unique_blob_count']
    assert set(z.namelist()) == {'manifest.json'} | {'blobs/' + key for key in blobs}
print(f"Verified {len(manifest['files'])} original paths / {len(blobs)} blobs")
