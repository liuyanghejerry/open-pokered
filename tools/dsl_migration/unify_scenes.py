#!/usr/bin/env python3
"""Unify existing .scene files onto the fused DSL design.

For every map under pokered-data/maps/:

  1. Replace `@speaker("System")` -> `@speaker("")` so prefix-less narrator
     dialogue compiles to bare `game.showText("...")` (kills the `System: `
     regression — the engine's empty-speaker codegen now emits no prefix).

  2. Regenerate each storyline's `@trigger(...)` line *deterministically from
     `script_config.json`* (matched by storyline name == talk/trigger fn), so
     the `.scene` carries the full, correct binding data:
        npc  -> @trigger(map=, npc=<id>[, toggle=][, script=][, hidden=true])
        sign -> @trigger(map=, sign=<id>)
        coord-> @trigger(map=, coords=[[x,y],...])   (ALL positions for that fn)
     This also fixes lossy/incoherent triggers in the stories migration
     (object-name-as-npc, and dropped coord tiles like PalletTown's (11,1)).

  3. Inject a storyline for each toggled object that has no talk handler
     (`no_talk = true`) so its binding survives regeneration.

The original `script_config.json` is the INPUT here (source of binding truth);
afterwards `gen_map_config` regenerates it FROM the fixed `.scene`, making the
DSL the single source of truth.
"""
import json
import re
import sys
from pathlib import Path

MAPS_DIR = Path(__file__).resolve().parents[2] / "crates/pokered-data/maps"

STORYLINE_RE = re.compile(r'^\s*@storyline\(\s*"([^"]+)"\s*\)')
TRIGGER_RE = re.compile(r'^(\s*)@trigger\(')
SCENE_CLOSE_RE = re.compile(r'^\}\s*$')


def build_lookups(cfg):
    npc_by_talk = {}  # talk fn -> [npc, ...]  (a fn may serve several objects)
    object_only = []
    for n in cfg.get("npcs", []):
        talk = n.get("talk")
        if talk:
            npc_by_talk.setdefault(talk, []).append(n)
        elif n.get("toggleId") or n.get("scriptId") or n.get("defaultHidden"):
            object_only.append(n)
    sign_by_talk = {s["talk"]: s["id"] for s in cfg.get("signs", []) if s.get("talk")}
    coords_by_trigger = {}
    for c in cfg.get("coordEvents", []):
        pos = c["position"]
        coords_by_trigger.setdefault(c["trigger"], []).append((pos[0], pos[1]))
    return npc_by_talk, sign_by_talk, coords_by_trigger, object_only


def gen_trigger(map_name, name, npc_by_talk, sign_by_talk, coords_by_trigger, indent):
    if name in npc_by_talk:
        # One @trigger line per object that routes to this handler (shared-talk
        # fns like OaksLab's talkPokedex bind several objects, each with its own
        # id/toggle).
        lines = []
        for n in npc_by_talk[name]:
            parts = [f'map = "{map_name}"', f'npc = {n["id"]}']
            if n.get("toggleId"):
                parts.append(f'toggle = "{n["toggleId"]}"')
            if n.get("scriptId"):
                parts.append(f'script = "{n["scriptId"]}"')
            if n.get("defaultHidden"):
                parts.append("hidden = true")
            lines.append(f'{indent}@trigger({", ".join(parts)})')
        return "\n".join(lines)
    if name in sign_by_talk:
        return f'{indent}@trigger(map = "{map_name}", sign = {sign_by_talk[name]})'
    if name in coords_by_trigger:
        coords = ", ".join(f"[{x}, {y}]" for x, y in coords_by_trigger[name])
        return f'{indent}@trigger(map = "{map_name}", coords = [{coords}])'
    return None  # unknown storyline — leave its trigger untouched


def object_storyline(map_name, n):
    parts = [f'map = "{map_name}"', f'npc = {n["id"]}']
    if n.get("toggleId"):
        parts.append(f'toggle = "{n["toggleId"]}"')
    if n.get("scriptId"):
        parts.append(f'script = "{n["scriptId"]}"')
    if n.get("defaultHidden"):
        parts.append("hidden = true")
    parts.append("no_talk = true")
    sid = f'{map_name}_obj{n["id"]}'
    return [
        f'  @storyline("{sid}") {{',
        f'    @trigger({", ".join(parts)})',
        f'  }}',
    ]


def unify_map(map_dir):
    scene_path = map_dir / "script.scene"
    cfg_path = map_dir / "script_config.json"
    if not scene_path.is_file() or not cfg_path.is_file():
        return None
    map_name = map_dir.name
    cfg = json.loads(cfg_path.read_text())
    npc_by_talk, sign_by_talk, coords_by_trigger, object_only = build_lookups(cfg)

    text = scene_path.read_text()
    lines = text.splitlines()
    out = []
    current = None
    stats = {"speaker_fixed": 0, "triggers_regen": 0, "objects_injected": 0}

    # injected object storylines need the toggle ids that are NOT already
    # represented by a talk storyline; emit them just before the scene close.
    injected = []
    for n in object_only:
        sid = f'{map_name}_obj{n["id"]}'
        if f'@storyline("{sid}")' in text:  # idempotent: already injected
            continue
        injected.extend(object_storyline(map_name, n))
        stats["objects_injected"] += 1

    for line in lines:
        m = STORYLINE_RE.match(line)
        if m:
            current = m.group(1)

        tm = TRIGGER_RE.match(line)
        if tm and current:
            new = gen_trigger(map_name, current, npc_by_talk, sign_by_talk,
                              coords_by_trigger, tm.group(1))
            if new is not None and new != line:
                line = new
                stats["triggers_regen"] += 1

        if '@speaker("System")' in line:
            line = line.replace('@speaker("System")', '@speaker("")')
            stats["speaker_fixed"] += 1

        # inject object-only storylines right before the final scene close brace
        if injected and SCENE_CLOSE_RE.match(line):
            out.extend(injected)
            injected = []
        out.append(line)

    # safety: if we never matched a scene-close (shouldn't happen), append
    if injected:
        out.extend(injected)

    new_text = "\n".join(out) + "\n"
    if new_text != text:
        scene_path.write_text(new_text)
    return stats


def main():
    totals = {"speaker_fixed": 0, "triggers_regen": 0, "objects_injected": 0, "maps": 0}
    for map_dir in sorted(MAPS_DIR.iterdir()):
        if not map_dir.is_dir():
            continue
        st = unify_map(map_dir)
        if st is None:
            continue
        totals["maps"] += 1
        for k in ("speaker_fixed", "triggers_regen", "objects_injected"):
            totals[k] += st[k]
    print(f"unified {totals['maps']} maps: "
          f"{totals['speaker_fixed']} @speaker('System')->'' lines, "
          f"{totals['triggers_regen']} @trigger regenerated, "
          f"{totals['objects_injected']} object-only storylines injected")


if __name__ == "__main__":
    sys.exit(main())
