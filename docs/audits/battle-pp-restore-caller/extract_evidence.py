#!/usr/bin/env python3
"""Verify every deduplicated evidence path; optionally extract its original bytes."""
from pathlib import Path, PurePosixPath
import argparse,hashlib,json,zipfile
p=argparse.ArgumentParser();p.add_argument('--archive',type=Path,default=Path(__file__).with_name('evidence.zip'));p.add_argument('--output',type=Path);p.add_argument('--verify-only',action='store_true');a=p.parse_args()
if not a.verify_only and a.output is None:p.error('use --verify-only or --output DIR')
with zipfile.ZipFile(a.archive)as z:
 index=json.loads(z.read('index.json'));paths=index['paths'];assert index['format']=='sha256-blob-deduplicated-v1'
 expected={'index.json'}|{'blobs/'+row['sha256']for row in paths.values()};assert set(z.namelist())==expected
 for key,row in paths.items():
  name=PurePosixPath(key);assert not name.is_absolute()and'..'not in name.parts
  assert name.name!='pokered-app'and name.suffix not in ['.gbc','.state','.gba']
  data=z.read('blobs/'+row['sha256']);assert len(data)==row['bytes']and hashlib.sha256(data).hexdigest()==row['sha256']
  if a.output is not None and not a.verify_only:
   dest=a.output.joinpath(*name.parts);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
print(f'PASS: {len(paths)} paths, {len(expected)-1} unique SHA256 blobs')
