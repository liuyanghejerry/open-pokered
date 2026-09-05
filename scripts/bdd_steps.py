#!/usr/bin/env python3
"""Step vocabulary for scripts/bdd.py — regex-matched Gherkin steps.

The vocabulary deliberately reuses the validated primitives of
scenarios.py / playthrough.py (boot_starter, pause_menu,
throw_balls_until_caught, battle_loop, …), so the BDD layer adds only
authoring ergonomics, never a second implementation of driver behavior.
Steps assert only what the debug protocol reads back — black-box, like
the rest of the test stack.

Cross-step state (recorded baselines, save paths) travels in the
Context the runner provides; every scenario gets a fresh one.
"""
import re
import shutil
import tempfile
from pathlib import Path

from playthrough import (
    Game,
    m01_boot,
    m02_oak_speech,
    m03_leave_house,
    resume_reentry,
)
from scenarios import (
    bag_qty,
    dismiss_dex_screen,
    pause_menu,
    snapshot,
    throw_balls_until_caught,
)
from save_builder import SaveBuilder, event_flag_bit

BUTTONS = {"up", "down", "left", "right", "a", "b", "start", "select"}


class Context:
    """One scenario's world: its game instance, throwaway save dir, and
    step-to-step memory (recorded baselines)."""

    def __init__(self):
        self.tmpdir = Path(tempfile.mkdtemp(prefix="pokered-bdd-"))
        self.game = None
        self.builder = None
        self.memory = {}

    def g(self):
        if self.game is None:
            self.fresh_game()
        return self.game

    def fresh_game(self):
        # The save lives OUTSIDE the game's run dir: close() rmtree's the
        # run dir, but save-roundtrip steps reboot from this file.
        self.game = Game(save_path=self.tmpdir / "bdd.sav")
        return self.game

    def close(self):
        if self.game is not None:
            self.game.close()
            self.game = None
        shutil.rmtree(self.tmpdir, ignore_errors=True)


STEPS = {"given": [], "when": [], "then": []}


def _register(kind, pattern):
    rx = re.compile(pattern)

    def deco(fn):
        STEPS[kind].append((rx, fn))
        return fn
    return deco


def given(pattern):
    return _register("given", pattern)


def when(pattern):
    return _register("when", pattern)


def then(pattern):
    return _register("then", pattern)


def match(step):
    for rx, fn in STEPS[step.kind]:
        m = rx.fullmatch(step.text)
        if m:
            kwargs = {k: (int(v) if isinstance(v, str) and v.isdigit() else v)
                      for k, v in m.groupdict().items()}
            return fn, kwargs
    raise AssertionError(f"no '{step.kind}' step matches: {step.text!r}")


def _to_const(item):
    """POKE BALL / poke ball -> POKE_BALL (give_item's const names)."""
    return item.strip().upper().replace(" ", "_")


def _to_debug(item):
    """POKE_BALL -> PokeBall (get_bag reports the Debug form)."""
    return "".join(w.capitalize() for w in _to_const(item).split("_"))


# ── Given: seed state ───────────────────────────────────────────────────
@given(r"a fresh game")
def _(ctx):
    ctx.fresh_game()


@given(r"a booted game")
def _(ctx):
    g = ctx.fresh_game()
    m01_boot(g)
    m02_oak_speech(g)
    s = g.st()
    ctx.memory["start_pos"] = (s["map_name"], s["player_x"], s["player_y"])


@given(r"the player has an? (?P<species>[A-Za-z]+) at level (?P<level>\d+)")
def _(ctx, species, level):
    r = ctx.g().d.cmd(cmd="give_pokemon", species=species, level=level)
    assert r["ok"], r


@given(r"the player has (?P<qty>\d+) (?P<item>[A-Za-z_ ]+)")
def _(ctx, qty, item):
    r = ctx.g().d.cmd(cmd="give_item", item=_to_const(item), qty=qty)
    assert r["ok"], r


@given(r"the leader's experience is recorded")
def _(ctx):
    party = ctx.g().d.cmd(cmd="get_party")["data"]
    ctx.memory["exp0"] = party[0]["experience"]


@given(r"the pause menu is open")
def _(ctx):
    pause_menu(ctx.g())


# ── Given: construct a save offline (fast full-state setup) ────────────
# `the save has …` steps mutate a SaveBuilder; the built snapshot boots
# via `When the game boots from the save`. The money step MUST stay
# ahead of the item step: its literal " money" tail is what keeps
# "65000 money" from matching the item pattern.
@given(r"a constructed save")
def _(ctx):
    ctx.builder = SaveBuilder()


@given(r"the save has an? (?P<species>[A-Za-z]+) at level (?P<level>\d+)")
def _(ctx, species, level):
    ctx.builder.party_add(species, int(level))


@given(r"the save has (?P<amount>\d+) money")
def _(ctx, amount):
    ctx.builder.money(int(amount))


@given(r"the save has (?P<qty>\d+) (?P<item>[A-Za-z_ ]+)")
def _(ctx, qty, item):
    ctx.builder.give_item(_to_const(item), qty)


@given(r"the save has the flag (?P<name>EVENT_[A-Z_0-9]+)")
def _(ctx, name):
    ctx.builder.flag(name)


@given(r"the save starts on (?P<map_name>[A-Za-z0-9]+) "
       r"at \((?P<x>\d+),(?P<y>\d+)\)")
def _(ctx, map_name, x, y):
    ctx.builder.position(map_name, int(x), int(y))


@when(r"the game boots from the save")
def _(ctx):
    assert ctx.builder is not None, "no constructed save (Given a constructed save)"
    path = ctx.builder.write(ctx.tmpdir / "constructed.json")
    if ctx.game is not None:
        ctx.game.close()
        ctx.game = None
    ctx.game = Game(save_path=ctx.tmpdir / "constructed.sav", snapshot=path)
    resume_reentry(ctx.game)


# ── When: drive ─────────────────────────────────────────────────────────
@when(r"a wild (?P<species>[A-Za-z]+) at level (?P<level>\d+) attacks")
def _(ctx, species, level):
    g = ctx.g()
    r = g.d.cmd(cmd="start_wild_battle", species=species, level=level)
    assert r["ok"], r
    g.wait("screen=battle", 600)


@when(r"the player runs from the battle")
def _(ctx):
    g = ctx.g()
    g.battle_loop(prefer="run")
    g.wait("not_battle", 1800)


@when(r"the player fights until the battle ends")
def _(ctx):
    g = ctx.g()
    g.battle_loop(prefer="fight")
    g.wait("not_battle", 1800)


@when(r"the player throws up to (?P<max_balls>\d+) Pok[eé] ?Balls")
def _(ctx, max_balls):
    throw_balls_until_caught(ctx.g(), max_balls)
    dismiss_dex_screen(ctx.g())


@when(r"the whiteout settles")
def _(ctx):
    g = ctx.g()
    g.cutscene()
    g.wait("screen=overworld", 1800)


@when(r"the player walks out of the house")
def _(ctx):
    m03_leave_house(ctx.g())


@when(r"the player presses (?P<keys>[A-Za-z, ]+)")
def _(ctx, keys):
    g = ctx.g()
    for k in re.split(r"[,\s]+", keys.strip()):
        assert k in BUTTONS, f"unknown button {k!r}"
        g.tap(k, 10)
    g.step(10)


@when(r"the player saves the game")
def _(ctx):
    g = ctx.g()
    r = g.d.cmd(cmd="save")
    assert r["ok"], r
    ctx.memory["pre_save"] = snapshot(g)


@when(r"the game is rebooted from the save")
def _(ctx):
    g = ctx.g()
    save_path = g.save_path
    g.close()
    ctx.game = None
    ctx.game = Game(save_path=save_path)
    resume_reentry(ctx.game)


@when(r"the player opens OPTION")
def _(ctx):
    g = ctx.g()
    menu = pause_menu(g)
    # Entries: POKéMON ITEM RED SAVE OPTION EXIT — cursor 0 + 4 downs
    # reaches OPTION (5 downs is EXIT, which would just close the menu).
    for _ in range(4):
        g.d.drive(["down"], frames=10)
    g.tap("a", 10)
    g.step(20)
    s = g.st()["screen"]
    assert s not in ("overworld", menu), f"OPTION never opened: {s}"


@when(r"the player toggles the text speed")
def _(ctx):
    g = ctx.g()
    ctx.memory["ts0"] = g.st()["text_speed_delay_frames"]
    for _ in range(30):
        g.tap("left", 8)
        if g.st()["text_speed_delay_frames"] != ctx.memory["ts0"]:
            return
    raise AssertionError("text speed never moved")


@when(r"the player leaves the options screen")
def _(ctx):
    g = ctx.g()
    for _ in range(20):
        g.tap("b", 10)
        if g.st()["screen"] == "overworld":
            return
    raise AssertionError("OPTIONS never returned to the overworld")


@when(r"the player closes the menu via EXIT")
def _(ctx):
    g = ctx.g()
    # Walk down the entries until one closes the menu for good (EXIT).
    # Any other entry only opens a one-level submenu; B backs out of it
    # (and cancels SAVE's prompt) before the cursor moves on.
    for _ in range(7):
        g.tap("a", 10)
        g.step(20)
        if g.st()["screen"] == "overworld":
            return
        g.tap("b", 10)
        g.step(10)
        g.d.drive(["down"], frames=10)
        g.step(6)
    raise AssertionError("EXIT never returned to the overworld")


# ── Then: assert over the protocol ──────────────────────────────────────
@then(r"the screen is (?P<screen>[a-z-]+)")
def _(ctx, screen):
    s = ctx.g().st()
    assert s["screen"] == screen, s["screen"]


@then(r"the player has not moved")
def _(ctx):
    s = ctx.g().st()
    here = (s["map_name"], s["player_x"], s["player_y"])
    assert here == ctx.memory.get("start_pos"), (ctx.memory["start_pos"], s)


@then(r"the player is on (?P<map_name>[A-Za-z0-9]+)")
def _(ctx, map_name):
    s = ctx.g().st()
    assert s["map_name"] == map_name, s["map_name"]


@then(r"the player is at \((?P<x>\d+),(?P<y>\d+)\)")
def _(ctx, x, y):
    s = ctx.g().st()
    assert (s["player_x"], s["player_y"]) == (int(x), int(y)), \
        (s["player_x"], s["player_y"])


@then(r"the player has (?P<amount>\d+) money")
def _(ctx, amount):
    s = ctx.g().st()
    assert s["money"] == int(amount), s["money"]


@then(r"the party leader is level (?P<level>\d+)")
def _(ctx, level):
    party = ctx.g().d.cmd(cmd="get_party")["data"]
    assert party[0]["level"] == int(level), party[0]["level"]


@then(r"the party leader has (?P<n>\d+) max hp")
def _(ctx, n):
    party = ctx.g().d.cmd(cmd="get_party")["data"]
    assert party[0]["max_hp"] == int(n), party[0]["max_hp"]


@then(r"the flag (?P<name>EVENT_[A-Z_0-9]+) is set")
def _(ctx, name):
    event_flag_bit(name)  # reject unknown flag names before querying
    flags = ctx.g().d.cmd(cmd="get_flags")["data"]
    if isinstance(flags, dict) and "flags" in flags:
        flags = flags["flags"]
    assert flags.get(name) is True, f"{name} not set"


@then(r"the party has (?P<n>\d+) Pok[eé]mon")
def _(ctx, n):
    s = ctx.g().st()
    assert s["party_count"] == n, s["party_count"]


@then(r"the party contains an? (?P<species>[A-Za-z]+)")
def _(ctx, species):
    party = ctx.g().d.cmd(cmd="get_party")["data"]
    assert any(p["species"] == species for p in party), \
        [p["species"] for p in party]


@then(r"the party is fully healed")
def _(ctx):
    party = ctx.g().d.cmd(cmd="get_party")["data"]
    assert all(p["current_hp"] == p["max_hp"] for p in party), party


@then(r"the bag contains (?P<qty>\d+) (?P<item>[A-Za-z_ ]+)")
def _(ctx, qty, item):
    got = bag_qty(ctx.g(), _to_debug(item))
    assert got == qty, (item, got, qty)


@then(r"the bag has fewer than (?P<qty>\d+) (?P<item>[A-Za-z_ ]+)")
def _(ctx, qty, item):
    got = bag_qty(ctx.g(), _to_debug(item))
    assert got < qty, (item, got, qty)


@then(r"the leader has gained experience")
def _(ctx):
    party = ctx.g().d.cmd(cmd="get_party")["data"]
    assert party[0]["experience"] > ctx.memory["exp0"], \
        (ctx.memory["exp0"], party[0]["experience"])


@then(r"the text speed has changed")
def _(ctx):
    ts = ctx.g().st()["text_speed_delay_frames"]
    assert ts != ctx.memory["ts0"], (ctx.memory["ts0"], ts)
    ctx.memory["ts1"] = ts


@then(r"the text speed is still changed")
def _(ctx):
    ts = ctx.g().st()["text_speed_delay_frames"]
    assert ts == ctx.memory["ts1"] != ctx.memory["ts0"], \
        (ctx.memory["ts0"], ctx.memory["ts1"], ts)


@then(r"the saved state matches the pre-save snapshot")
def _(ctx):
    after = snapshot(ctx.g())
    assert after == ctx.memory["pre_save"], (ctx.memory["pre_save"], after)


@then(r"the engine reports NPCs on the current map")
def _(ctx):
    data = ctx.g().d.cmd(cmd="get_npcs")["data"]
    npcs = data.get("npcs", data) if isinstance(data, dict) else data
    assert npcs, "no NPCs reported"
    for n in npcs:
        assert "x" in n and "y" in n, n
    live = {(n["x"], n["y"]) for n in npcs
            if n.get("visible", True) and (n["x"], n["y"]) != (-1, -1)}
    assert live, npcs
    print(f"    npcs: {sorted(live)}", flush=True)


@then(r"the engine refuses the command")
def _(ctx):
    assert ctx.memory.get("last_cmd_ok") is False, ctx.memory


# Negative-protocol probe: recorded, then asserted with the step above
# (kept last so the vocabulary reads seed → drive → assert).
@when(r"the driver attempts to give (?P<item>[A-Za-z_ ]+)")
def _(ctx, item):
    r = ctx.g().d.cmd(cmd="give_item", item=_to_const(item), qty=1)
    ctx.memory["last_cmd_ok"] = r["ok"]
