#!/usr/bin/env python3
"""
migrate_coord_events.py — Generate coordEvent name mappings and migrate all maps.

For each map with coordEvents in script_config.json:
  1. Generate a camelCase name from the trigger function (stripping "coord" prefix)
  2. Add positional disambiguation suffix when multiple coords share the same trigger
  3. Insert `name` as the first field in each coordEvent object
  4. Update .scene files: add `name = "xxx"` to @trigger declarations with coords

Usage:
  python3 migrate_coord_events.py           # Execute migration
  python3 migrate_coord_events.py --dry-run # Preview changes only
"""

import json
import os
import re
import sys
from collections import defaultdict


def camel_case_first_lower(s: str) -> str:
    """Convert a string like 'coordNorthExit' to 'northExit'."""
    # Strip 'coord' prefix if present
    if s.startswith("coord"):
        s = s[len("coord"):]
    # Lowercase the first character
    if s:
        s = s[0].lower() + s[1:]
    return s


def generate_names(coord_events):
    """Generate names for coordEvents entries.

    Returns a list of name strings, one per entry.
    """
    # Build groups by base name (trigger → list of indices)
    groups = defaultdict(list)
    for i, event in enumerate(coord_events):
        trigger = event["trigger"]
        base_name = camel_case_first_lower(trigger)
        groups[base_name].append(i)

    names = [None] * len(coord_events)

    for base_name, indices in groups.items():
        if len(indices) == 1:
            names[indices[0]] = base_name
        else:
            for j, idx in enumerate(indices):
                names[idx] = f"{base_name}{j + 1}"

    return names


def update_json_config(map_dir, names, dry_run=False):
    """Update script_config.json with name fields."""
    config_path = os.path.join(map_dir, "script_config.json")
    with open(config_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    coord_events = data.get("coordEvents", [])
    if not coord_events:
        return False

    modified = False
    new_events = []
    for i, event in enumerate(coord_events):
        name = names[i]
        if "name" not in event:
            # Rebuild dict with name first
            new_event = {"name": name}
            new_event.update(event)
            new_events.append(new_event)
            modified = True
        else:
            new_events.append(event)

    if not modified:
        return False

    data["coordEvents"] = new_events

    if dry_run:
        print(f"  [DRY-RUN] Would update {config_path}")
        return True

    with open(config_path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"  Updated {config_path}")
    return True


def update_scene_file(map_dir, coord_events, names, dry_run=False):
    """Update script.scene with name parameter in @trigger coords."""
    scene_path = os.path.join(map_dir, "script.scene")
    if not os.path.exists(scene_path):
        return False

    with open(scene_path, "r", encoding="utf-8") as f:
        content = f.read()

    # Build a mapping from coordinate (as tuple) → name
    coord_to_name = {}
    for i, event in enumerate(coord_events):
        pos = tuple(event["position"])
        coord_to_name[pos] = names[i]

    # Pattern: @trigger(... coords = [[...], ...] ...)
    # or @trigger(... coord = [...] ...)
    # We need to find coords/coord patterns inside @trigger parens
    modified = False

    def replace_trigger(match):
        nonlocal modified
        full_match = match.group(0)

        # Extract the coords array
        coords_match = re.search(r'\b(coord)s?\s*=\s*\[\[(.*?)\]\]', full_match)
        if not coords_match:
            # Try singular coord = [x, y]
            coords_match = re.search(r'\bcoord\s*=\s*\[(\d+)\s*,\s*(\d+)\]', full_match)
            if coords_match:
                x, y = int(coords_match.group(1)), int(coords_match.group(2))
                first_name = coord_to_name.get((x, y))
                if first_name:
                    modified = True
                    # Add name before closing )
                    return re.sub(r'\)$', f', name = "{first_name}")', full_match)
            return full_match

        # Parse multiple coords: [[x1, y1], [x2, y2], ...]
        coords_str = coords_match.group(0)
        coord_pairs = re.findall(r'\[(\d+)\s*,\s*(\d+)\]', coords_str)
        if not coord_pairs:
            return full_match

        # Find the first coord that has a mapping
        first_name = None
        for pair in coord_pairs:
            x, y = int(pair[0]), int(pair[1])
            name = coord_to_name.get((x, y))
            if name:
                first_name = name
                break

        if not first_name:
            return full_match

        # Check if name already exists
        if re.search(r'\bname\s*=\s*"', full_match):
            return full_match

        modified = True
        return re.sub(r'\)$', f', name = "{first_name}")', full_match)

    # Find all @trigger lines with coords
    new_content = re.sub(
        r'@trigger\([^)]*(?:coord)s?\s*=\s*\[.*?\)',
        replace_trigger,
        content,
        flags=re.DOTALL
    )

    if not modified:
        return False

    if dry_run:
        print(f"  [DRY-RUN] Would update {scene_path}")
        return True

    with open(scene_path, "w", encoding="utf-8") as f:
        f.write(new_content)
    print(f"  Updated {scene_path}")
    return True


def process_map(map_dir, dry_run=False):
    """Process a single map directory."""
    config_path = os.path.join(map_dir, "script_config.json")
    if not os.path.exists(config_path):
        return False

    with open(config_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    coord_events = data.get("coordEvents", [])
    if not coord_events:
        return False

    # Check if all events already have names
    if all("name" in e for e in coord_events):
        return False

    map_name = os.path.basename(map_dir)
    print(f"\nProcessing {map_name} ({len(coord_events)} coordEvents)...")

    names = generate_names(coord_events)

    # Print name mappings
    for i, event in enumerate(coord_events):
        pos = event["position"]
        trigger = event["trigger"]
        print(f"  [{pos[0]},{pos[1]}] {trigger} → \"{names[i]}\"")

    updated_json = update_json_config(map_dir, names, dry_run)
    updated_scene = update_scene_file(map_dir, coord_events, names, dry_run)

    return updated_json or updated_scene


def main():
    dry_run = "--dry-run" in sys.argv

    if dry_run:
        print("=== DRY RUN MODE — no files will be modified ===\n")

    script_dir = os.path.dirname(os.path.abspath(__file__))
    maps_dir = os.path.join(
        script_dir,
        "..", "examples", "pokered", "crates", "pokered-data", "maps"
    )
    maps_dir = os.path.normpath(maps_dir)

    if not os.path.isdir(maps_dir):
        print(f"Error: Maps directory not found: {maps_dir}", file=sys.stderr)
        sys.exit(1)

    map_dirs = sorted(
        d for d in os.listdir(maps_dir)
        if os.path.isdir(os.path.join(maps_dir, d)) and d != "shared"
    )

    processed = 0
    for map_name in map_dirs:
        map_dir = os.path.join(maps_dir, map_name)
        if process_map(map_dir, dry_run):
            processed += 1

    print(f"\n{'Would process' if dry_run else 'Processed'} {processed} map(s).")
    if dry_run:
        print("Run without --dry-run to apply changes.")


if __name__ == "__main__":
    main()
