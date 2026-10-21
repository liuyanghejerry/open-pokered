#!/usr/bin/env python3
"""Import original object-event movement selectors without rewriting map JSON.

Usage: python3 scripts/sync_npc_motion.py /path/to/pret-pokered [--check]
The reference checkout is external; unknown constants or mismatched objects fail
before any file is written. Trainer sight ranges and initial facings are retained.
"""
import argparse
import json
import re
from pathlib import Path

SELECTORS = {
    'ANY_DIR': 0, 'UP_DOWN': 1, 'LEFT_RIGHT': 2,
    'BOULDER_MOVEMENT_BYTE_2': 16,
    'DOWN': 208, 'UP': 209, 'LEFT': 210, 'RIGHT': 211, 'NONE': 255,
}


def synchronize(reference, root, check=False):
    plans = []
    receipt = {}
    for source in sorted((reference / 'data/maps/objects').glob('*.asm')):
        rows = []
        for line in source.read_text().splitlines():
            match = re.match(r'\s*object_event\s+(.*)', line)
            if not match:
                continue
            args = [value.strip() for value in match[1].split(';')[0].split(',')]
            assert args[3] in ('WALK', 'STAY'), (source, args)
            rows.append([int(args[0]), int(args[1]), args[3] == 'STAY', SELECTORS[args[4]]])
        path = root / 'crates/pokered-data/maps' / source.stem / 'map.json'
        text = path.read_text()
        data = json.loads(text)
        assert len(data['npcs']) == len(rows), source
        for npc, row in zip(data['npcs'], rows):
            assert [npc['x'], npc['y']] == row[:2], (source, npc, row)
            assert npc['movement'] == ('Stationary' if row[2] else 'Wander') or (row[2] and row[3] == 0), (source, npc, row)
        # Decode each object span to preserve all unrelated formatting verbatim.
        cursor = re.search(r'"npcs"\s*:\s*\[', text).end()
        edits = []
        for row in rows:
            while text[cursor].isspace() or text[cursor] == ',':
                cursor += 1
            npc, end = json.JSONDecoder().raw_decode(text, cursor)
            block = text[cursor:end]
            if 'movementByte2' in npc:
                block = re.sub(r'("movementByte2"\s*:\s*)\d+', lambda m: m[1] + str(row[3]), block)
            else:
                block, count = re.subn(r'(\n\s*)"range"(\s*:\s*\d+,)', lambda m: m[1] + '"movementByte2": ' + str(row[3]) + ',' + m[0], block)
                assert count == 1, path
            if row[2]:
                block = re.sub(r'("movement"\s*:\s*)"Wander"', r'\1"Stationary"', block)
            edits.append((cursor, end, block))
            cursor = end
        for start, end, block in reversed(edits):
            text = text[:start] + block + text[end:]
        if rows:
            receipt[source.stem] = rows
        plans.append((path, text))
    assert len(plans) == 223, len(plans)
    assert sum(map(len, receipt.values())) == 918
    if check:
        for path, text in plans:
            assert path.read_text() == text, f'Outdated motion selectors: {path}'
    else:
        for path, text in plans:
            if path.read_text() != text:
                path.write_text(text)
    return receipt


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference', type=Path)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    receipt = synchronize(args.reference, Path(__file__).resolve().parents[1], args.check)
    print(f'Validated {sum(map(len, receipt.values()))} NPCs in 223 original object maps')
