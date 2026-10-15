"""Verify and restore the original paths in the SHA256-deduplicated capture archive."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("output", type=Path)
parser.add_argument("--archive", type=Path, default=Path(__file__).with_name("evidence.zip"))
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
with zipfile.ZipFile(args.archive) as archive:
    manifest = json.loads(archive.read("archive-entry-manifest.json"))
    for entry in manifest["entries"]:
        path = Path(entry["path"])
        assert not path.is_absolute() and ".." not in path.parts
        data = archive.read("blobs/" + entry["sha256"])
        assert len(data) == entry["bytes"]
        assert hashlib.sha256(data).hexdigest() == entry["sha256"]
        target = args.output / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
print(f"Verified and restored {len(manifest['entries'])} original paths")
