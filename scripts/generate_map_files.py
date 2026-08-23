#!/usr/bin/env python3
"""
Generate map data files from existing Rust data.

This script reads the existing npc_data_part*.rs, sign_data_part*.rs, and map_objects.rs
and generates individual map files in maps/ directory.
"""

import os
import re
from pathlib import Path

# Resolved relative to this script so the tool works from any checkout.
REPO_ROOT = Path(__file__).resolve().parent.parent
BASE_DIR = REPO_ROOT / "crates/pokered-data/src"

def snake_case(name):
    """Convert PascalCase to snake_case"""
    s1 = re.sub('(.)([A-Z][a-z]+)', r'\1_\2', name)
    return re.sub('([a-z0-9])([A-Z])', r'\1_\2', s1).lower()

def read_npc_data():
    """Read all NPC data from part files"""
    npcs = {}
    for i in range(1, 6):
        filepath = BASE_DIR / f"npc_data_part{i}.rs"
        if filepath.exists():
            content = filepath.read_text()
            # Find all NPC array definitions
            pattern = r'pub static NPCS_(\w+):\s*\[NpcEntry;\s*(\d+)\]\s*=\s*\[([\s\S]*?)\];'
            for match in re.finditer(pattern, content):
                map_name = match.group(1)
                count = int(match.group(2))
                data = match.group(3).strip()
                npcs[map_name] = (count, data)
    return npcs

def read_sign_data():
    """Read all sign data from part files"""
    signs = {}
    for i in range(1, 6):
        filepath = BASE_DIR / f"sign_data_part{i}.rs"
        if filepath.exists():
            content = filepath.read_text()
            pattern = r'pub static SIGNS_(\w+):\s*\[SignEntry;\s*(\d+)\]\s*=\s*\[([\s\S]*?)\];'
            for match in re.finditer(pattern, content):
                map_name = match.group(1)
                count = int(match.group(2))
                data = match.group(3).strip()
                signs[map_name] = (count, data)
    return signs

def read_warp_data():
    """Read all warp data from map_objects.rs"""
    warps = {}
    filepath = BASE_DIR / "map_objects.rs"
    if filepath.exists():
        content = filepath.read_text()
        pattern = r'static WARPS_(\w+):\s*\[WarpData;\s*(\d+)\]\s*=\s*\[([\s\S]*?)\];'
        for match in re.finditer(pattern, content):
            map_name = match.group(1)
            count = int(match.group(2))
            data = match.group(3).strip()
            warps[map_name] = (count, data)
    return warps

def generate_map_file(map_name, npc_data, sign_data, warp_data):
    """Generate a single map file"""
    snake_name = snake_case(map_name)
    
    lines = [
        "use crate::maps::MapId;",
        "use crate::npc_data::{NpcEntry, NpcFacing, NpcMovement};",
        "use crate::sign_data::SignEntry;",
        "use crate::map_objects::WarpData;",
        "",
    ]
    
    # NPC data
    if map_name in npc_data:
        count, data = npc_data[map_name]
        lines.append(f"pub const NPCS: [NpcEntry; {count}] = [")
        lines.append(data)
        lines.append("];")
    else:
        lines.append("pub const NPCS: [NpcEntry; 0] = [];")
    
    lines.append("")
    
    # Sign data
    if map_name in sign_data:
        count, data = sign_data[map_name]
        lines.append(f"pub const SIGNS: [SignEntry; {count}] = [")
        lines.append(data)
        lines.append("];")
    else:
        lines.append("pub const SIGNS: [SignEntry; 0] = [];")
    
    lines.append("")
    
    # Warp data
    if map_name in warp_data:
        count, data = warp_data[map_name]
        lines.append(f"pub const WARPS: [WarpData; {count}] = [")
        lines.append(data)
        lines.append("];")
    else:
        lines.append("pub const WARPS: [WarpData; 0] = [];")
    
    return "\n".join(lines)

def main():
    print("Reading existing data...")
    npcs = read_npc_data()
    signs = read_sign_data()
    warps = read_warp_data()
    
    # Get all unique map names
    all_maps = set(npcs.keys()) | set(signs.keys()) | set(warps.keys())
    print(f"Found {len(all_maps)} maps")
    
    # Create maps directory if needed
    maps_dir = BASE_DIR / "maps"
    maps_dir.mkdir(exist_ok=True)
    
    # Generate each map file
    generated = []
    for map_name in sorted(all_maps):
        snake_name = snake_case(map_name)
        content = generate_map_file(map_name, npcs, signs, warps)
        filepath = maps_dir / f"{snake_name}.rs"
        filepath.write_text(content)
        generated.append(snake_name)
    
    print(f"Generated {len(generated)} map files")
    
    # Generate mod.rs
    mod_content = [
        "// Auto-generated map module",
        "// Each map file contains NPCs, Signs, and Warps",
        "",
    ]
    
    for snake_name in sorted(generated):
        mod_content.append(f"mod {snake_name};")
    
    mod_content.extend([
        "",
        "use crate::maps::MapId;",
        "use crate::npc_data::NpcEntry;",
        "use crate::sign_data::SignEntry;",
        "use crate::map_objects::WarpData;",
        "",
        "pub fn get_npcs(map: MapId) -> &'static [NpcEntry] {",
        "    match map {",
    ])
    
    # This needs MapId enum mapping - placeholder for now
    mod_content.extend([
        "        _ => &[]",
        "    }",
        "}",
        "",
        "pub fn get_signs(map: MapId) -> &'static [SignEntry] {",
        "    match map {",
        "        _ => &[]",
        "    }",
        "}",
        "",
        "pub fn get_warps(map: MapId) -> &'static [WarpData] {",
        "    match map {",
        "        _ => &[]",
        "    }",
        "}",
    ])
    
    mod_path = maps_dir / "mod_generated.rs"
    mod_path.write_text("\n".join(mod_content))
    print(f"Generated mod_generated.rs")

if __name__ == "__main__":
    main()