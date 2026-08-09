#!/usr/bin/env python3
"""Parse trainer headers from scripts/*.asm files.
Generates pokered-data/src/trainer_headers.rs with per-map trainer header data.

Each trainer header provides: event_flag (EventFlag enum variant), sight_range.
Text IDs are handled by the NPC data system (text_id on NpcEntry).
"""
import os
import re
import glob

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.join(SCRIPT_DIR, '..', '..')
SCRIPTS_DIR = os.path.join(PROJECT_ROOT, 'scripts')
OUTPUT_PATH = os.path.join(SCRIPT_DIR, '..', 'crates', 'pokered-data', 'src', 'trainer_headers.rs')

def filename_to_map_id(filename):
    """Convert e.g. 'PewterGym.asm' to 'PewterGym'."""
    return os.path.splitext(os.path.basename(filename))[0]

def parse_trainer_headers():
    """Parse all script files for trainer headers. Returns {map_name: [(event_flag, sight_range), ...]}."""
    results = {}

    for filepath in sorted(glob.glob(os.path.join(SCRIPTS_DIR, '*.asm'))):
        map_name = filename_to_map_id(filepath)
        trainers = []

        with open(filepath, 'r') as f:
            in_trainers = False
            for line in f:
                stripped = line.strip()

                if 'def_trainers' in stripped and not stripped.startswith(';'):
                    in_trainers = True
                    continue

                if in_trainers:
                    m = re.match(r'\s*trainer\s+(EVENT_\w+)\s*,\s*(\d+)\s*,', stripped)
                    if m:
                        event_flag = m.group(1)
                        sight_range = int(m.group(2))
                        trainers.append((event_flag, sight_range))
                        continue

                    if stripped == 'db -1 ; end' or stripped == 'db -1':
                        in_trainers = False
                        continue

                    if stripped.endswith(':') or stripped == '':
                        continue

                    if not stripped.startswith('trainer') and not stripped.startswith(';'):
                        in_trainers = False

        if trainers:
            results[map_name] = trainers

    return results

def generate_rust_code(all_trainers):
    lines = []
    lines.append('use crate::event_flags::EventFlag;')
    lines.append('use crate::maps::MapId;')
    lines.append('')
    lines.append('#[derive(Debug, Clone, Copy)]')
    lines.append('pub struct TrainerHeaderData {')
    lines.append('    pub event_flag: EventFlag,')
    lines.append('    pub sight_range: u8,')
    lines.append('}')
    lines.append('')

    total_trainers = 0
    for map_name in sorted(all_trainers.keys()):
        trainers = all_trainers[map_name]
        total_trainers += len(trainers)
        const_name = f'TRAINERS_{camel_to_upper_snake(map_name)}'
        lines.append(f'pub static {const_name}: [TrainerHeaderData; {len(trainers)}] = [')
        for event_flag, sight_range in trainers:
            lines.append(f'    TrainerHeaderData {{ event_flag: EventFlag::{event_flag}, sight_range: {sight_range} }},')
        lines.append('];')
        lines.append('')

    lines.append(f'pub fn get_trainer_headers(map: MapId) -> &\'static [TrainerHeaderData] {{')
    lines.append('    match map {')
    for map_name in sorted(all_trainers.keys()):
        const_name = f'TRAINERS_{camel_to_upper_snake(map_name)}'
        lines.append(f'        MapId::{map_name} => &{const_name},')
    lines.append('        _ => &[],')
    lines.append('    }')
    lines.append('}')
    lines.append('')

    return '\n'.join(lines), total_trainers

def camel_to_upper_snake(name):
    """Convert PascalCase to UPPER_SNAKE_CASE. E.g. PewterGym -> PEWTER_GYM."""
    s = re.sub(r'([A-Z])', r'_\1', name).upper()
    if s.startswith('_'):
        s = s[1:]
    s = re.sub(r'(\d)([A-Z])', r'\1_\2', s)
    return s

def main():
    all_trainers = parse_trainer_headers()
    print(f'Found trainer headers in {len(all_trainers)} maps')

    total = sum(len(t) for t in all_trainers.values())
    print(f'Total trainer headers: {total}')

    for map_name, trainers in sorted(all_trainers.items()):
        print(f'  {map_name}: {len(trainers)} trainers')

    code, _ = generate_rust_code(all_trainers)
    with open(OUTPUT_PATH, 'w') as f:
        f.write(code)
    print(f'Generated {OUTPUT_PATH}')

if __name__ == '__main__':
    main()
