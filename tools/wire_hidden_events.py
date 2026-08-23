#!/usr/bin/env python3
"""Wire the hidden-event signs added by fill_hidden_events.py into the scene DSL.

- gym statues  -> per-gym scene storyline with the badge-conditional two texts
- PCs          -> openPC() storyline
- slot machines-> one openSlots() storyline covering every machine sign

Usage:
    python3 tools/wire_hidden_events.py --ref <pret/pokered checkout> [--apply]
"""
import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from fill_hidden_events import (build_plans, pascal, read, GYM_STATUE_INFO)

ROOT = Path(__file__).resolve().parent.parent

GYM_STATUE_ZH = {
    "POKeMON GYM": "宝可梦道馆",
    "LEADER": "馆主",
    "WINNING TRAINERS": "获胜训练家",
}


def esc(s: str) -> str:
    return s.replace("\\", "\\\\").replace("\n", "\\n").replace('"', '\\"')


def statue_texts(city: str, leader: str, zh_city: str, zh_leader: str) -> tuple[str, str, str, str]:
    t1 = f"{city}\nPOKeMON GYM\nLEADER: {leader}\n\nWINNING TRAINERS:\n<RIVAL>"
    t2 = f"{city}\nPOKeMON GYM\nLEADER: {leader}\n\nWINNING TRAINERS:\n<RIVAL>\n<PLAYER>"
    z1 = f"{zh_city}宝可梦道馆\n馆主：{zh_leader}\n\n获胜训练家：\n劲敌"
    z2 = f"{zh_city}宝可梦道馆\n馆主：{zh_leader}\n\n获胜训练家：\n劲敌\n玩家"
    return t1, t2, z1, z2


GYM_ZH_NAMES = {
    "VIRIDIAN CITY": "常磐市", "GIOVANNI": "坂木",
    "PEWTER CITY": "深灰市", "BROCK": "小刚",
    "CERULEAN CITY": "华蓝市", "MISTY": "小霞",
    "VERMILION CITY": "枯叶市", "LT.SURGE": "马志士",
    "CELADON CITY": "玉虹市", "ERIKA": "莉佳",
    "FUCHSIA CITY": "浅红市", "KOGA": "阿桔",
    "SAFFRON CITY": "金黄市", "SABRINA": "娜姿",
    "CINNABAR ISLAND": "红莲岛", "BLAINE": "夏伯",
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", required=True)
    ap.add_argument("--apply", action="store_true")
    args = ap.parse_args()
    ref = Path(args.ref)
    plans = build_plans(ref)
    report = []
    for map_const, items in sorted(plans.items()):
        mname = pascal(map_const)
        base = ROOT / "crates" / "pokered-data" / "maps" / mname
        map_path = base / "map.json"
        cfg_path = base / "script_config.json"
        scene_path = base / "script.scene"
        if not map_path.exists():
            continue
        map_json = json.loads(read(map_path))
        # find the sign ids for the newly-added signs
        sign_ids = {}
        for s in map_json.get("signs", []):
            key = (s["x"], s["y"])
            if key in sign_ids:
                continue
            sign_ids[key] = s["textId"]

        statue_items = [it for it in items if it[2] == "gym_statue"]
        pc_items = [it for it in items if it[2] == "pc"]
        slot_items = [it for it in items if it[2] == "slot_machine"]

        if not (statue_items or pc_items or slot_items):
            continue

        cfg = json.loads(read(cfg_path)) if cfg_path.exists() else {
            "$schema": "../../schemas/script_config.schema.json",
            "npcs": [], "signs": [], "coordEvents": [],
        }
        cfg_signs = cfg.setdefault("signs", [])
        scene_lines = []
        if scene_path.exists():
            scene_lines = read(scene_path).rstrip().splitlines()
            # Drop the trailing game_scene closing brace; storylines are
            # appended INSIDE the block and the brace is re-added on write.
            if scene_lines and scene_lines[-1].strip() == "}":
                scene_lines.pop()

        def cfg_has_sign(sid):
            return any(e.get("id") == sid for e in cfg_signs)

        def scene_has_talk(talk):
            return f'@storyline("{talk}")' in "\n".join(scene_lines)

        def add_cfg_sign(sid, talk):
            if not cfg_has_sign(sid):
                cfg_signs.append({"id": sid, "talk": talk})

        # ── gym statues ──────────────────────────────────────────────
        info = GYM_STATUE_INFO.get(map_const)
        for x, y, kind, data in statue_items:
            sid = sign_ids.get((x, y))
            if sid is None or not info:
                continue
            talk_probe = f"gymStatue{sid}"
            if scene_has_talk(talk_probe):
                continue  # storyline already present
            city, leader, badge = info
            zh_city = GYM_ZH_NAMES.get(city, city)
            zh_leader = GYM_ZH_NAMES.get(leader, leader)
            t1, t2, z1, z2 = statue_texts(city, leader, zh_city, zh_leader)
            talk = f"gymStatue{sid}"
            add_cfg_sign(sid, talk)
            scene_lines.append(f"")
            scene_lines.append(f"  // ── Gym statue (sign {sid} at {x},{y}) — badge-conditional ──")
            scene_lines.append(f'  @storyline("{talk}") {{')
            scene_lines.append(f'    @trigger(map = "{mname}", sign = {sid})')
            scene_lines.append(f'    @if (getFlag("{badge}")) {{')
            scene_lines.append(f'      @speaker("") {{ @t("{esc(t2)}", "{esc(z2)}") }}')
            scene_lines.append(f"    }} @else {{")
            scene_lines.append(f'      @speaker("") {{ @t("{esc(t1)}", "{esc(z1)}") }}')
            scene_lines.append(f"    }}")
            scene_lines.append(f"  }}")
            report.append(f"[{mname}] statue sign {sid} ({city} / {leader} / {badge})")

        # ── PCs ──────────────────────────────────────────────────────
        for x, y, kind, data in pc_items:
            sid = sign_ids.get((x, y))
            if sid is None:
                continue
            existing_talk = next(
                (e.get("talk") for e in cfg_signs if e.get("id") == sid), None)
            if existing_talk is not None and existing_talk != f"pcSign{sid}":
                continue  # already wired with a different handler (e.g. pcStorage)
            if scene_has_talk(f"pcSign{sid}"):
                continue
            talk = f"pcSign{sid}"
            add_cfg_sign(sid, talk)
            scene_lines.append(f"")
            scene_lines.append(f"  // ── PC (sign {sid} at {x},{y}) — OpenPokemonCenterPC ──")
            scene_lines.append(f'  @storyline("{talk}") {{')
            scene_lines.append(f'    @trigger(map = "{mname}", sign = {sid})')
            scene_lines.append(f"    openPC()")
            scene_lines.append(f"  }}")
            report.append(f"[{mname}] pc sign {sid}")

        # ── slot machines ────────────────────────────────────────────
        if slot_items:
            machine_ids = [sign_ids.get((x, y)) for x, y, k, d in slot_items]
            machine_ids = [s for s in machine_ids if s is not None]
            if scene_has_talk("slotMachines"):
                machine_ids = []
            talk = "slotMachines"
            for sid in machine_ids:
                add_cfg_sign(sid, talk)
            scene_lines.append(f"")
            scene_lines.append(f"  // ── Slot machines (signs {machine_ids}) — StartSlotMachine ──")
            scene_lines.append(f'  @storyline("{talk}") {{')
            for sid in machine_ids:
                scene_lines.append(f'    @trigger(map = "{mname}", sign = {sid})')
            scene_lines.append(f"    openSlots()")
            scene_lines.append(f"  }}")
            report.append(f"[{mname}] slots storyline over {len(machine_ids)} machines")

        if args.apply:
            with open(cfg_path, "w", encoding="utf-8") as f:
                json.dump(cfg, f, indent=2, ensure_ascii=False)
            if scene_lines:
                body = "\n".join(scene_lines).rstrip()
                with open(scene_path, "w", encoding="utf-8") as f:
                    f.write(body + "\n")
                    f.write("}\n")
    for r in report:
        print(r)
    if not args.apply:
        print("(dry run — pass --apply to write)")


if __name__ == "__main__":
    main()
