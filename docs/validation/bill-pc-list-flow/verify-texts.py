"""Compare every ordered bilingual literal, including typed-command arguments."""
import argparse
import json
from pathlib import Path
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--base', default='a79508ccaf5ef416235718688d76dad24ec8a417')
args = parser.parse_args()
path = 'crates/pokered-data/maps/BillsHouse/script.scene'
pairs = re.compile(r'@t\(\s*("(?:[^"\\]|\\.)*")\s*,\s*("(?:[^"\\]|\\.)*")\s*\)')

def literals(source):
    return [(json.loads(en), json.loads(zh)) for en, zh in pairs.findall(source)]

before = literals(subprocess.check_output(['git', 'show', f'{args.base}:{path}'], text=True))
after = literals(Path(path).read_text())
assert len(before) == 20 and before == after, 'Bilingual literal sequence changed'
print(json.dumps({'base': args.base, 'exactly_preserved_pairs': len(before)}))
