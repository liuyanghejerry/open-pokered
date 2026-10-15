"""Verify or reconstruct the exact original paths in evidence.zip."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import sys
import zipfile

archive, destination = sys.argv[1:]
target = None if destination == '--verify-only' else Path(destination)
verified = {}
with zipfile.ZipFile(archive) as z:
    rows = json.loads(z.read('entry-manifest.json'))
    for row in rows:
        path = PurePosixPath(row['path'])
        assert not path.is_absolute() and '..' not in path.parts
        blob = row['blob']
        if blob not in verified or target is not None:
            content = z.read(blob)
            assert len(content) == row['bytes']
            assert hashlib.sha256(content).hexdigest() == row['sha256']
            verified[blob] = (len(content), row['sha256'])
        assert verified[blob] == (row['bytes'], row['sha256'])
        if target is not None:
            output = target / path
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(content)
print(f'Verified {len(rows)} original paths from {len(verified)} exact-content blobs.')
