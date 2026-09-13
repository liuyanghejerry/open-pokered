"""Typed client for the pokered agent debug API (M6 experiment layer).

Wraps the raw JSON-line `DebugClient` with one method per agent command,
returning the response `data` (raising on `ok: false`). Plain stdlib.

    from openpoke.client import AgentClient
    c = AgentClient(9000)
    print(c.observe()["mode"], c.position())
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from debug_drive import DebugClient  # noqa: E402


class AgentError(RuntimeError):
    """A debug command returned ok:false (validation, boundary, unknown)."""


class AgentClient:
    def __init__(self, port=9000, host="127.0.0.1"):
        self.d = DebugClient(port, host)

    def close(self):
        self.d.close()

    # ── raw escape hatch ────────────────────────────────────────────
    def cmd(self, **kw):
        r = self.d.cmd(**kw)
        if not r.get("ok"):
            raise AgentError(f"{kw.get('cmd')}: {r.get('error')}")
        return r.get("data")

    # ── observation (M1) ────────────────────────────────────────────
    def observe(self, level=None, profile=None):
        kw = {"cmd": "get_agent_state"}
        if level is not None:
            kw["level"] = level
        if profile is not None:
            kw["profile"] = profile
        return self.cmd(**kw)

    def nearby(self, radius=None):
        kw = {"cmd": "get_nearby"}
        if radius is not None:
            kw["radius"] = radius
        return self.cmd(**kw)

    def state(self):
        return self.cmd(cmd="get_state")

    def position(self):
        s = self.cmd(cmd="get_position")
        return (s["map_name"], s["x"], s["y"])

    def flags(self):
        return self.cmd(cmd="get_flags")

    def party(self):
        return self.cmd(cmd="get_party")

    def bag(self):
        return self.cmd(cmd="get_bag")

    # ── navigation + interaction (M2) ───────────────────────────────
    def move_to(self, x, y):
        return self.cmd(cmd="move_to", x=x, y=y)

    def interact(self):
        return self.cmd(cmd="interact")

    def interact_with(self, entity_id):
        return self.cmd(cmd="interact_with", id=entity_id)

    # ── world + travel (M3) ─────────────────────────────────────────
    def world_graph(self, maps=None):
        kw = {"cmd": "get_world_graph"}
        if maps is not None:
            kw["maps"] = list(maps)
        return self.cmd(**kw)

    def route(self, from_map, to_map):
        return self.cmd(cmd="find_world_route", **{"from": from_map, "to": to_map})

    def travel_to(self, map_name):
        return self.cmd(cmd="travel_to", map=map_name)

    # ── semantics (M4) ──────────────────────────────────────────────
    def script_semantics(self, map_name=None):
        kw = {"cmd": "get_script_semantics"}
        if map_name is not None:
            kw["map"] = map_name
        return self.cmd(**kw)

    # ── determinism (M5) ────────────────────────────────────────────
    def set_seed(self, seed):
        return self.cmd(cmd="set_seed", seed=seed)

    def save_state(self, slot):
        return self.cmd(cmd="save_state", slot=slot)

    def restore_state(self, slot):
        return self.cmd(cmd="restore_state", slot=slot)

    # ── core driving ────────────────────────────────────────────────
    def press(self, button):
        return self.cmd(cmd="press", button=button)

    def press_sequence(self, buttons):
        return self.cmd(cmd="press_sequence", buttons=list(buttons))

    def step(self, count):
        return self.cmd(cmd="step_frames", count=count)

    def drive(self, buttons, frames=None):
        self.press_sequence(buttons)
        return self.step(frames if frames is not None else len(buttons))

    def wait_until(self, condition, max_frames=600):
        return self.cmd(cmd="wait_until", condition=condition, max_frames=max_frames)

    def skip_dialogue(self):
        return self.cmd(cmd="skip_dialogue")

    def set_flag(self, name, value=True):
        return self.cmd(cmd="set_flag", name=name, value=value)

    def give_item(self, item, qty=1):
        return self.cmd(cmd="give_item", item=item, qty=qty)

    def give_pokemon(self, species, level):
        return self.cmd(cmd="give_pokemon", species=species, level=level)

    def warp(self, map_name, x=None, y=None):
        kw = {"cmd": "warp", "map": map_name}
        if x is not None:
            kw["x"] = x
        if y is not None:
            kw["y"] = y
        return self.cmd(**kw)
