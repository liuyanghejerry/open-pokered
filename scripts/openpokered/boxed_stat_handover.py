"""Read-only witness for a legacy boxed-stat cache migration.

This is NOT a schema4 CONTINUE proof or an acquisition-legality certificate.
The original 33-byte box record has no derived stats. A separate, unchanged
schema4 check must certify the new native baseline; this witness only links
that baseline to legacy observations and the byte-identical source SRAM.
"""
import hashlib
import json
import math
from pathlib import Path
import re

import playthrough as pt
from .collection_verification import collection_snapshot


def _enum_names(source, name):
    match = re.search(rf'const {name}: &\[&str\] = &\[(.*?)\];', source, re.S)
    if not match:
        raise ValueError(f'Missing native enum table: {name}')
    return re.findall(r'"(\w+)"', match[1])


def _status(byte):
    names = {0: 'None', 8: 'Poison', 16: 'Burn', 32: 'Freeze', 64: 'Paralysis'}
    if 1 <= byte <= 7:
        return f'Sleep({byte})'
    if byte not in names:
        raise ValueError('Noncanonical saved status byte')
    return names[byte]


def _equal(left, right):
    # JSON observation types matter too (False must not compare equal to 0).
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def _derived_stats(record, base):
    level = record[3]
    if not 1 <= level <= 100:
        raise ValueError('Invalid saved box level')
    attack, defense = record[27] >> 4, record[27] & 15
    speed, special = record[28] >> 4, record[28] & 15
    hp = ((attack & 1) << 3) | ((defense & 1) << 2) | ((speed & 1) << 1) | (special & 1)
    values = []
    for index, (name, dv) in enumerate(zip(
            ('hp', 'attack', 'defense', 'speed', 'special'), (hp, attack, defense, speed, special))):
        ev = int.from_bytes(record[17 + index * 2:19 + index * 2], 'big')
        root = math.isqrt(ev)
        # Original CalcStat's 8-bit loop saturates at 255, before division.
        bonus = min(255, root + (root * root < ev)) // 4
        value = ((base[name] + dv) * 2 + bonus) * level // 100
        values.append(min(999, value + (level + 10 if index == 0 else 5)))
    return values


def validate_boxed_stat_handover(saved, legacy_observations, restored_observations, data_root=None):
    """Allow ONLY independently rebuilt boxed max_hp, checking every raw slot.

    No source writes, native commands, healing, choice overrides or observation
    rewriting. Even an unchanged boxed max_hp must agree with the raw record's
    original formula. Hidden legacy OT/XP/DVs/EVs/PP-Ups are not observable and
    cannot be retrospectively certified; their saved bytes are bound here.
    """
    if not isinstance(saved, bytes) or len(saved) != 32768:
        raise ValueError('Native SRAM must contain 32768 bytes')
    before = collection_snapshot(legacy_observations)
    after = collection_snapshot(restored_observations)
    for key in before.keys() - {'stored_pokemon'}:
        if not _equal(before[key], after[key]):
            raise ValueError(f'Handover changed persisted facts: {key}')
    data_root = Path(data_root or pt.ROOT / 'crates/pokered-data')
    build_bytes = (data_root / 'build.rs').read_bytes()
    species = ['None', *_enum_names(build_bytes.decode(), 'SPECIES_ORDER')]
    moves = ['None', *_enum_names(build_bytes.decode(), 'MOVE_ORDER')]
    old_slots = {(mon['box'], mon['index']): mon for mon in before['stored_pokemon']}
    new_slots = {(mon['box'], mon['index']): mon for mon in after['stored_pokemon']}
    if old_slots.keys() != new_slots.keys():
        raise ValueError('Handover changed stored slots')
    records, changes, data_hashes = [], [], {}
    for bank in (2, 3):
        content = saved[bank * 8192:(bank + 1) * 8192]
        total = 6 * 1122
        checksum = lambda data: (~sum(data)) & 255
        if content[total] != checksum(content[:total]) or any(
                content[total + 1 + box] != checksum(content[box * 1122:(box + 1) * 1122])
                for box in range(6)):
            raise ValueError('Saved box-bank checksum disagrees')
    for box, count in enumerate(before['state']['box_counts']):
        start = (2 + box // 6) * 8192 + (box % 6) * 1122
        if saved[start] != count or saved[start + count + 1] != 255:
            raise ValueError('Saved box count or sentinel disagrees')
        for index in range(count):
            old, new = old_slots[box, index], new_slots[box, index]
            if not _equal({key: value for key, value in old.items() if key != 'max_hp'},
                          {key: value for key, value in new.items() if key != 'max_hp'}):
                raise ValueError('Handover changed persisted stored fields')
            if type(old['max_hp']) is not int or not 1 <= old['max_hp'] <= 999:
                raise ValueError('Invalid legacy derived max_hp')
            offset = start + 22 + index * 33
            record = saved[offset:offset + 33]
            if not 0 < record[0] < len(species) or record[0] != saved[start + 1 + index]:
                raise ValueError('Saved species/header disagrees')
            if any(move >= len(moves) for move in record[8:12]):
                raise ValueError('Invalid saved move ID')
            name = species[record[0]]
            path = data_root / 'pokemon' / f'{name}.json'
            content = path.read_bytes()
            data_hashes[str(path.relative_to(data_root))] = hashlib.sha256(content).hexdigest()
            derived = _derived_stats(record, json.loads(content)['baseStats'])
            persisted = {'species': name, 'level': record[3],
                'hp': int.from_bytes(record[1:3], 'big'), 'status': _status(record[4]),
                'moves': [moves[move] for move in record[8:12]],
                'pp': [value & 63 for value in record[29:33]]}
            if any(not _equal(new[field], value)
                   for field, value in persisted.items()):
                raise ValueError('Saved box record disagrees with observations')
            if type(new['max_hp']) is not int or new['max_hp'] != derived[0]:
                raise ValueError('Restored boxed max_hp disagrees with original formula')
            witness = {'box': box, 'index': index, 'offset': offset, 'species': name,
                'record_hex': record.hex(), 'record_sha256': hashlib.sha256(record).hexdigest(),
                'derived_stats': derived}
            records.append(witness)
            if old['max_hp'] != new['max_hp']:
                changes.append({**witness, 'field': 'max_hp', 'legacy': old['max_hp'],
                                'restored': new['max_hp'], 'current_hp': new['hp']})
    if not changes:
        raise ValueError('No boxed-stat cache migration; use strict CONTINUE instead')
    return {'schema': 1, 'kind': 'serialized_boxed_stat_cache_handover',
        'cache_handover_verified': True, 'historical_strict_continue_verified': False,
        'save_sha256': hashlib.sha256(saved).hexdigest(), 'changes': changes,
        'saved_box_records': records, 'build_enum_sha256': hashlib.sha256(build_bytes).hexdigest(),
        'species_data_sha256': data_hashes,
        'scope': 'Only boxed derived max_hp is migrated. All other exposed schema4 facts '
            'are exact; all saved boxed records and original stat formulas independently '
            'checked. Not a native replay, historical schema4 pass, hidden legacy-field '
            'certificate, acquisition/route legality proof or final124 certification.'}
