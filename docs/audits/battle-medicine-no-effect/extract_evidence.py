#!/usr/bin/env python3
"""Verify/extract battle-medicine negative audit content-addressed evidence, without emulators."""
from pathlib import Path, PurePosixPath
import argparse,hashlib,json,zipfile
p=argparse.ArgumentParser();p.add_argument('--archive',type=Path,default=Path(__file__).with_name('evidence.zip'));p.add_argument('--output',type=Path);p.add_argument('--verify-only',action='store_true');a=p.parse_args()
if not a.verify_only and a.output is None:p.error('choose --verify-only or --output DIRECTORY')
with zipfile.ZipFile(a.archive)as z:
 m=json.loads(z.read('manifest.json'));assert m['schema']==1
 for name,info in m['files'].items():
  rel=PurePosixPath(name);assert not rel.is_absolute()and '..'not in rel.parts,name;data=z.read('blobs/'+info['sha256']);assert len(data)==info['bytes']and hashlib.sha256(data).hexdigest()==info['sha256'],name
  if not a.verify_only:
   dest=a.output.joinpath(*rel.parts);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data);assert hashlib.sha256(dest.read_bytes()).hexdigest()==info['sha256']
print(f"Verified {len(m['files'])} original paths")
