#!/usr/bin/env python3
"""Parse event_constants.asm to generate Rust EventFlag enum with correct bit indices.

Handles const_def, const, const_next, const_skip directives.
Event flags are bit-packed in wEventFlags array.
"""

import re
import os

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.join(SCRIPT_DIR, '..', '..')

EVENT_CONSTANTS_PATH = os.path.join(PROJECT_ROOT, 'constants', 'event_constants.asm')
OUTPUT_PATH = os.path.join(SCRIPT_DIR, '..', 'crates', 'pokered-data', 'src', 'event_flags.rs')


def parse_event_constants():
    """Parse event_constants.asm and return list of (name, bit_index) tuples."""
    flags = []
    const_value = 0

    with open(EVENT_CONSTANTS_PATH, 'r') as f:
        for line in f:
            line = line.strip()
            # Skip comments and empty lines
            if not line or line.startswith(';'):
                continue

            # const_def — reset counter to 0
            if line == 'const_def':
                const_value = 0
                continue

            # const_next $XXX — jump to specific value
            m = re.match(r'const_next\s+\$([0-9A-Fa-f]+)\s*(?:-\s*(\d+))?', line)
            if m:
                val = int(m.group(1), 16)
                if m.group(2):
                    val -= int(m.group(2))
                const_value = val
                continue

            # const_skip N or const_skip (default 1)
            m = re.match(r'const_skip(?:\s+(\d+))?$', line)
            if m:
                skip = int(m.group(1)) if m.group(1) else 1
                const_value += skip
                continue

            # const EVENT_NAME — define a flag at current const_value
            m = re.match(r'const\s+(EVENT_\w+)', line)
            if m:
                name = m.group(1)
                flags.append((name, const_value))
                const_value += 1
                continue

            # DEF lines (like NUM_EVENTS) - skip
            # Other lines we don't care about

    return flags


def to_pascal_case(name):
    """Convert EVENT_FOO_BAR to FooBar."""
    # Remove EVENT_ prefix
    name = name.replace('EVENT_', '')
    parts = name.split('_')
    result = []
    for part in parts:
        if part.isdigit() or re.match(r'^\d', part):
            # Keep numbers: "1ST" -> "1st" but capitalize first if starts with letter
            result.append(part.capitalize())
        else:
            result.append(part.capitalize())
    return ''.join(result)


def generate_rust_code(flags):
    """Generate the Rust event_flags module."""
    max_bit = max(bit for _, bit in flags) if flags else 0
    num_bytes = (max_bit // 8) + 1

    lines = []
    lines.append('//! Event flag definitions — auto-generated from event_constants.asm')
    lines.append('//!')
    lines.append('//! DO NOT EDIT MANUALLY. Run scripts/parse_event_flags.py to regenerate.')
    lines.append('')
    lines.append(f'/// Total number of event flag bytes needed (bit array storage).')
    lines.append(f'pub const EVENT_FLAGS_SIZE: usize = {num_bytes};')
    lines.append('')
    lines.append(f'/// Total number of defined event flags.')
    lines.append(f'pub const NUM_EVENT_FLAGS: usize = {len(flags)};')
    lines.append('')
    lines.append('/// Event flag identifiers with their bit indices.')
    lines.append('///')
    lines.append('/// Each variant maps to a specific bit position in the wEventFlags array.')
    lines.append('/// Use `EventFlag::bit_index()` to get the bit position.')
    lines.append('#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]')
    lines.append('#[repr(u16)]')
    lines.append('#[allow(non_camel_case_types)]')
    lines.append('pub enum EventFlag {')

    # Group by area comment
    current_comment = None
    for name, bit in flags:
        lines.append(f'    /// Bit index: 0x{bit:03X} ({bit})')
        lines.append(f'    {name} = 0x{bit:03X},')

    lines.append('}')
    lines.append('')
    lines.append('impl EventFlag {')
    lines.append('    /// Get the bit index of this event flag.')
    lines.append('    #[inline]')
    lines.append('    pub const fn bit_index(self) -> u16 {')
    lines.append('        self as u16')
    lines.append('    }')
    lines.append('')
    lines.append('    /// Get the byte offset in the event flags array.')
    lines.append('    #[inline]')
    lines.append('    pub const fn byte_offset(self) -> usize {')
    lines.append('        (self as u16 as usize) / 8')
    lines.append('    }')
    lines.append('')
    lines.append('    /// Get the bit within the byte.')
    lines.append('    #[inline]')
    lines.append('    pub const fn bit_mask(self) -> u8 {')
    lines.append('        1 << ((self as u16) % 8)')
    lines.append('    }')
    lines.append('')
    lines.append('    /// Get all event flags as a slice.')
    lines.append('    pub const ALL: &[EventFlag] = &[')
    for name, bit in flags:
        lines.append(f'        EventFlag::{name},')
    lines.append('    ];')
    lines.append('}')
    lines.append('')

    return '\n'.join(lines)


def main():
    flags = parse_event_constants()
    print(f'Parsed {len(flags)} event flags')
    if flags:
        max_bit = max(bit for _, bit in flags)
        print(f'Max bit index: 0x{max_bit:03X} ({max_bit})')
        print(f'Storage needed: {(max_bit // 8) + 1} bytes')

    # Verify no duplicates
    seen = {}
    for name, bit in flags:
        if bit in seen:
            print(f'WARNING: Duplicate bit index 0x{bit:03X}: {name} and {seen[bit]}')
        seen[bit] = name

    code = generate_rust_code(flags)
    with open(OUTPUT_PATH, 'w') as f:
        f.write(code)
    print(f'Generated {OUTPUT_PATH}')


if __name__ == '__main__':
    main()
