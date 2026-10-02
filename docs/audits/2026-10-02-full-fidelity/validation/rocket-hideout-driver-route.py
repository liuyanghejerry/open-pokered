"""Controlled, real-input Rocket/Lift Key regression; not a fresh clear."""
import argparse
import json
from pathlib import Path
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--fixture", required=True, type=Path)
parser.add_argument("--binary", required=True, type=Path)
parser.add_argument("--baseline", action="store_true",
                    help="Exercise the previous helper and verify its premature return.")
args = parser.parse_args()
ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "scripts"))
import playthrough as p
from debug_drive import DebugClient
from playthrough_late import talk_npc, talk_object, require_flag

commands = []
original_command = DebugClient.cmd
def record(self, **kwargs):
    if kwargs.get("cmd") not in {"get_state", "get_npcs", "get_flags", "get_bag",
                                  "step_frames", "wait_until", "skip_dialogue",
                                  "press_timeline"}:
        raise RuntimeError("non-input command prohibited: " + str(kwargs))
    commands.append(kwargs)
    return original_command(self, **kwargs)
DebugClient.cmd = record
def atomic_drive(self, buttons, frames=None):
    timeline = list(buttons)
    count = len(timeline) if frames is None else frames
    timeline.extend([None] * (count - len(timeline)))
    return self.cmd(cmd="press_timeline", buttons=timeline, advance=True)
DebugClient.drive = atomic_drive

g = p.Game(binary=args.binary, snapshot=args.fixture, seed=42, speed=0)
g.smart_moves = True  # Same battle/navigation strategy as the fresh m22 driver.
observations = []
def observe(tag):
    state = g.st()
    value = {key: state.get(key) for key in (
        "frame_count", "screen", "map_name", "player_x", "player_y",
        "player_facing", "player_movement_state", "dialogue_state",
        "script_running", "active_script_effect", "battle_phase")}
    value["tag"] = tag
    observations.append(value)
    print(tag, json.dumps(value), flush=True)
    return state

try:
    p.resume_reentry(g)
    observe("start")
    assert g.pos() == ("RocketHideoutB4F", 19, 10), g.pos()
    if args.baseline:
        npc = next(n for n in g.d.cmd(cmd="get_npcs")["data"] if n["text_id"] == 4)
        g.approach_object(npc["x"], npc["y"], "RocketHideoutB4F")
        g.tap("a", 16)
        assert g.cutscene()
        old_return = observe("old-helper-return")
        assert old_return["screen"] == "overworld"
        flags = g.d.cmd(cmd="get_flags")["data"]
        assert not flags.get("EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2")
        g.step(1)
        promoted = observe("one-normal-update-later")
        assert promoted["screen"] == "battle"
        assert promoted["frame_count"] == old_return["frame_count"] + 1
    else:
        original_battle = g.battle_loop
        def battle(*a, **kw):
            state = observe("actual-battle-handoff")
            enemy = state["battle_live"]["enemy_party"]
            assert [(m["species"], m["level"]) for m in enemy] == [
                ("Koffing", 21), ("Zubat", 21)], enemy
            return original_battle(*a, **kw)
        g.battle_loop = battle
        talk_npc(g, "RocketHideoutB4F", 4,
                 completion_flag="EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2")
        require_flag(g, "EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2")
        observe("rocket-won")
        talk_npc(g, "RocketHideoutB4F", 4,
                 completion_flag="EVENT_ROCKET_DROPPED_LIFT_KEY")
        require_flag(g, "EVENT_ROCKET_DROPPED_LIFT_KEY")
        observe("lift-key-dropped")
        talk_object(g, "RocketHideoutB4F", 10, 2)
        bag = g.d.cmd(cmd="get_bag")["data"]
        assert any(i["item"] == "LiftKey" and i["qty"] == 1 for i in bag), bag
        key = next(n for n in g.d.cmd(cmd="get_npcs")["data"] if n["text_id"] == 9)
        assert not key["visible"], key
        observe("lift-key-collected")
    result = {"scope": "Controlled seeded real-input regression only; not a fresh playthrough.",
              "baseline": args.baseline, "binary": str(args.binary),
              "fixture": str(args.fixture), "observations": observations,
              "final_flags": g.d.cmd(cmd="get_flags")["data"],
              "final_bag": g.d.cmd(cmd="get_bag")["data"],
              "final_key_object": next(n for n in g.d.cmd(cmd="get_npcs")["data"]
                                       if n["text_id"] == 9),
              "commands": commands}
    suffix = "before" if args.baseline else "after"
    output = Path(__file__).with_name("rocket-hideout-driver-" + suffix + ".json")
    output.write_text(json.dumps(result, indent=2) + "\n")
    print("PASS", suffix, "commands", len(commands), "evidence", output, flush=True)
finally:
    g.close()
