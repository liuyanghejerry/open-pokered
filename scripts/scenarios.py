#!/usr/bin/env python3
"""Scenario-driven subsystem tests for open-pokered.

Complements the milestone playthrough (playthrough.py) with non-linear
tests: instead of walking m01→m10, each scenario constructs a minimal
state through the debug protocol's seeding commands (warp / give_pokemon /
give_item / start_wild_battle / save) and then drives ONE subsystem with
real button input, asserting only what is read back over the protocol
(state / party / bag / flags). Black-box on the running game — nothing is
asserted from engine internals.

A milestone proves "the game can be played this far"; a scenario proves
"this subsystem behaves as specified". Use milestones for progression
regressions (a fresh full run is the final verdict); use scenarios when a
change touches battle / items / save / menus and you want a verdict in
seconds instead of minutes.

Usage:
    python3 scripts/scenarios.py [--list] [--only s01,s05] [--skip s06]
"""
import argparse
import shutil
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
from playthrough import (  # noqa: E402
    Game,
    NavError,
    m01_boot,
    m02_oak_speech,
    m03_leave_house,
    resume_reentry,
)

SCENARIOS = []


def scenario(sid, desc):
    def deco(fn):
        SCENARIOS.append((sid, desc, fn))
        return fn
    return deco


# ── shared boot helpers ─────────────────────────────────────────────────
def boot_starter(g, species="Bulbasaur", level=5):
    """Power-on into the bedroom overworld, then seed a party member.

    Skips Oak's lab entirely: give_pokemon writes straight into the
    save-data party (the same path starter scripts use), so battle /
    bag / save scenarios never re-walk m04/m05."""
    m01_boot(g)
    m02_oak_speech(g)
    r = g.d.cmd(cmd="give_pokemon", species=species, level=level)
    assert r["ok"], r
    return g.st()


def snapshot(g):
    """Everything a save roundtrip must reproduce."""
    s = g.st()
    party = g.d.cmd(cmd="get_party")["data"]
    bag = g.d.cmd(cmd="get_bag")["data"]
    return {"map": s["map_name"], "x": s["player_x"], "y": s["player_y"],
            "money": s["money"], "party": party, "bag": bag}


def bag_qty(g, name):
    for it in g.d.cmd(cmd="get_bag")["data"]:
        if it["item"] == name:
            return it["qty"]
    return 0


def throw_balls_until_caught(g, max_balls):
    """Throw Poké Balls from the battle BAG until the wild battle ends in
    a catch, then return the number of balls spent. Full-HP catch odds
    are ~1/3 per ball, so callers should budget >=20 (weakening first
    risks a KO and ending the battle the wrong way). Asserts if the
    budget runs out; the caller must dismiss the dex-registration
    screen afterwards (`dismiss_dex_screen`)."""
    throws = 0
    for _ in range(max_balls * 8):
        s = g.st()
        if s["screen"] != "battle":
            return throws
        ph = s["battle_phase"]
        if ph == "PlayerMenu":
            # 2x2 menu: FIGHT TL / PKMN TR / BAG BL / RUN BR — battle_loop's
            # up+left and down+right pin the other two corners.
            g.d.drive(["down", "left"], frames=10)  # -> BAG
            g.tap("a", 4)
            g.step(10)
        elif ph == "BagSelect":
            g.tap("a", 4)               # slot 0 is the only item: the ball
            throws += 1
            g.step(10)
        else:
            g.tap("a", 10)              # "used POKé BALL" / miss text
            g.step(10)
        if throws >= max_balls:
            break
    raise AssertionError(f"catch failed: still battling after {throws} balls")


def dismiss_dex_screen(g):
    """A catch parks the dex-registration entry screen over the
    overworld; b/a taps dismiss it."""
    for _ in range(20):
        if g.st()["screen"] == "overworld":
            return
        g.tap("b", 10)
        g.tap("a", 10)
    raise AssertionError(f"never returned to the overworld: "
                         f"{g.st()['screen']}")


# ── seeding / observation ───────────────────────────────────────────────
@scenario("s01-bag-seed", "give_item lands in the bag; unknown items rejected")
def s01_bag_seed(g):
    for item, qty in (("POKE_BALL", 10), ("POTION", 3)):
        r = g.d.cmd(cmd="give_item", item=item, qty=qty)
        assert r["ok"], r
    bag = {i["item"]: i["qty"] for i in g.d.cmd(cmd="get_bag")["data"]}
    assert bag.get("PokeBall") == 10, bag
    assert bag.get("Potion") == 3, bag
    r = g.d.cmd(cmd="give_item", item="NOT_AN_ITEM", qty=1)
    assert not r["ok"] and "unknown item" in r.get("error", ""), r
    g.evidence("s01")


@scenario("s02-party-seed", "give_pokemon appends in order; leader is first")
def s02_party_seed(g):
    for sp, lv in (("Bulbasaur", 5), ("Pidgey", 4), ("Rattata", 3)):
        r = g.d.cmd(cmd="give_pokemon", species=sp, level=lv)
        assert r["ok"], r
    s = g.st()
    assert s["party_count"] == 3, s["party_count"]
    party = g.d.cmd(cmd="get_party")["data"]
    assert [p["species"] for p in party] == \
        ["Bulbasaur", "Pidgey", "Rattata"], party
    assert [p["level"] for p in party] == [5, 4, 3], party
    g.evidence("s02")


# ── battle flow ─────────────────────────────────────────────────────────
@scenario("s03-wild-run", "wild battle: RUN escapes without moving the player")
def s03_wild_run(g):
    st = boot_starter(g, "Bulbasaur", 5)
    here = (st["map_name"], st["player_x"], st["player_y"])
    r = g.d.cmd(cmd="start_wild_battle", species="Pidgey", level=3)
    assert r["ok"], r
    g.wait("screen=battle", 600)
    g.battle_loop(prefer="run")
    g.wait("not_battle", 1800)
    s = g.st()
    assert s["screen"] == "overworld", s["screen"]
    assert (s["map_name"], s["player_x"], s["player_y"]) == here, (here, s)
    g.evidence("s03")


@scenario("s04-wild-win", "wild battle: winning grants experience")
def s04_wild_win(g):
    boot_starter(g, "Bulbasaur", 10)
    exp0 = g.d.cmd(cmd="get_party")["data"][0]["experience"]
    r = g.d.cmd(cmd="start_wild_battle", species="Caterpie", level=2)
    assert r["ok"], r
    g.wait("screen=battle", 600)
    g.battle_loop(prefer="fight")
    g.wait("not_battle", 1800)
    s = g.st()
    assert s["screen"] == "overworld", s["screen"]
    exp1 = g.d.cmd(cmd="get_party")["data"][0]["experience"]
    assert exp1 > exp0, (exp0, exp1)
    g.evidence("s04")


@scenario("s05-wild-catch", "throwing Poké Balls from the battle bag catches")
def s05_wild_catch(g):
    boot_starter(g, "Bulbasaur", 8)
    # Weakening risks a KO (L8 vs L3) and a weak one-hit fight on the
    # wrong side of the catch formula — brute force instead: at full HP
    # each ball lands ~1/3, so 20 balls miss together ~0.03% of runs.
    r = g.d.cmd(cmd="give_item", item="POKE_BALL", qty=20)
    assert r["ok"], r
    r = g.d.cmd(cmd="start_wild_battle", species="Caterpie", level=3)
    assert r["ok"], r
    g.wait("screen=battle", 600)
    throw_balls_until_caught(g, 20)
    dismiss_dex_screen(g)
    s = g.st()
    assert s["party_count"] == 2, s
    party = g.d.cmd(cmd="get_party")["data"]
    assert party[1]["species"] == "Caterpie", party[1]["species"]
    left = bag_qty(g, "PokeBall")
    assert 0 <= left < 20, left
    g.evidence("s05")


@scenario("s06-blackout", "total party KO: whiteout respawns at home, party healed")
def s06_blackout(g):
    boot_starter(g, "Rattata", 2)
    money0 = g.st()["money"]
    r = g.d.cmd(cmd="start_wild_battle", species="Beedrill", level=30)
    assert r["ok"], r
    g.wait("screen=battle", 600)
    g.battle_loop(prefer="fight")       # a L2 cannot win; we mean to lose
    g.wait("not_battle", 1800)
    g.cutscene()
    g.wait("screen=overworld", 1800)
    s = g.st()
    assert s["screen"] == "overworld", s["screen"]
    assert s["map_name"] in ("RedsHouse1F", "RedsHouse2F", "PalletTown"), \
        s["map_name"]
    party = g.d.cmd(cmd="get_party")["data"]
    assert all(p["current_hp"] == p["max_hp"] for p in party), party
    # The engine starts a new game at ¥0, so the loss penalty can't be
    # observed here — respawn + full heal is the assertable contract.
    print(f"   blackout: money {money0} -> {s['money']}, "
          f"respawn {s['map_name']} ({s['player_x']},{s['player_y']})")
    g.evidence("s06")


# ── save / menus ────────────────────────────────────────────────────────
@scenario("s07-save-roundtrip", "save → fresh boot → CONTINUE restores everything")
def s07_save_roundtrip():
    save_path = Path(tempfile.mkdtemp(prefix="pokered-scenario-")) / "rt.sav"
    try:
        g1 = Game(save_path=save_path)
        try:
            m01_boot(g1)
            m02_oak_speech(g1)
            r = g1.d.cmd(cmd="give_pokemon", species="Pidgey", level=6)
            assert r["ok"], r
            r = g1.d.cmd(cmd="give_item", item="POTION", qty=5)
            assert r["ok"], r
            before = snapshot(g1)
            r = g1.d.cmd(cmd="save")
            assert r["ok"], r
            assert save_path.exists() and save_path.stat().st_size > 0
        finally:
            g1.close()
        g2 = Game(save_path=save_path)
        try:
            resume_reentry(g2)
            after = snapshot(g2)
            assert after == before, (before, after)
        finally:
            g2.close()
        print(f"   roundtrip restored {before['map']} ({before['x']},{before['y']}) "
              f"party={len(before['party'])} bag={len(before['bag'])}")
    finally:
        shutil.rmtree(save_path.parent, ignore_errors=True)


def pause_menu(g):
    """Open the START menu and return its screen name."""
    for _ in range(20):
        g.tap("start", 20)
        s = g.st()
        if s["screen"] != "overworld":
            return s["screen"]
    raise AssertionError("START did not open the pause menu")


@scenario("s08-start-menu", "START menu: party submenu opens; EXIT returns control")
def s08_start_menu(g):
    boot_starter(g, "Bulbasaur", 5)
    r = g.d.cmd(cmd="give_item", item="POTION", qty=2)
    assert r["ok"], r
    menu = pause_menu(g)
    print(f"   pause menu screen: {menu}")
    # Cursor starts on the first entry (POKéMON when the dex is missing);
    # open it, then back out to the menu.
    g.tap("a", 10)
    g.step(20)
    sub = g.st()["screen"]
    print(f"   first-entry screen: {sub}")
    assert sub not in ("overworld", menu), sub
    for _ in range(20):
        g.tap("b", 10)
        if g.st()["screen"] == menu:
            break
    else:
        raise AssertionError(f"never returned to {menu}")
    # Walk down the entries until one of them closes the menu for good
    # (EXIT). Any other entry only opens a one-level submenu; B backs out
    # of it (and cancels SAVE's prompt) before we move the cursor on.
    for _ in range(7):
        g.tap("a", 10)
        g.step(20)
        if g.st()["screen"] == "overworld":
            break
        g.tap("b", 10)
        g.step(10)
        g.d.drive(["down"], frames=10)
        g.step(6)
    else:
        raise AssertionError("EXIT never returned to the overworld")
    g.evidence("s08")


@scenario("s09-options", "OPTIONS: text speed toggle takes effect and persists")
def s09_options(g):
    boot_starter(g, "Bulbasaur", 5)
    ts0 = g.st()["text_speed_delay_frames"]
    menu = pause_menu(g)
    # Entries: POKéMON ITEM RED SAVE OPTION EXIT — cursor 0 + 4 downs
    # reaches OPTION (5 downs is EXIT, which would just close the menu).
    for _ in range(4):
        g.d.drive(["down"], frames=10)
    g.tap("a", 10)
    g.step(20)
    opt = g.st()["screen"]
    assert opt not in ("overworld", menu), f"OPTION never opened: {opt}"
    print(f"   options screen: {opt} text_speed {ts0}")
    # Text Speed is the first row: left/right cycles FAST/MEDIUM/SLOW.
    seen = {}
    for _ in range(30):
        ts = g.st()["text_speed_delay_frames"]
        seen[ts] = seen.get(ts, 0) + 1
        if ts != ts0:
            break
        g.tap("left", 8)
    else:
        raise AssertionError(f"text speed never moved: {seen}")
    for _ in range(20):
        g.tap("b", 10)
        if g.st()["screen"] == "overworld":
            break
    else:
        raise AssertionError("OPTIONS never returned to the overworld")
    # Persistence: close and reopen the menu — the setting survives.
    for _ in range(20):
        g.tap("b", 10)
        if g.st()["screen"] == "overworld":
            break
    else:
        raise AssertionError("OPTIONS never returned to the overworld")
    menu = pause_menu(g)
    for _ in range(4):
        g.d.drive(["down"], frames=10)
    g.tap("a", 10)
    g.step(20)
    ts1 = g.st()["text_speed_delay_frames"]
    assert ts1 != ts0, (ts0, ts1)
    print(f"   text_speed {ts0} -> {ts1} (persisted)")
    for _ in range(20):
        g.tap("b", 10)
        if g.st()["screen"] == "overworld":
            break
    g.evidence("s09")


@scenario("s10-npcs", "get_npcs: live NPCs report sane fields on the current map")
def s10_npcs(g):
    st = boot_starter(g, "Bulbasaur", 5)
    m03_leave_house(g)               # stairs → 1F → PalletTown
    npcs = g.d.cmd(cmd="get_npcs")["data"]
    npcs = npcs.get("npcs", npcs) if isinstance(npcs, dict) else npcs
    assert npcs, "no NPCs reported on PalletTown"
    for n in npcs:
        for f in ("x", "y"):
            assert f in n, n
    live = {(n["x"], n["y"]) for n in npcs
            if n.get("visible", True) and (n["x"], n["y"]) != (-1, -1)}
    assert live, npcs
    print(f"   PalletTown NPCs: {sorted(live)}")
    g.evidence("s10")


# ── runner ──────────────────────────────────────────────────────────────
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--only", default=None,
                    help="comma-separated scenario ids to run")
    ap.add_argument("--skip", default=None,
                    help="comma-separated scenario ids to skip")
    args = ap.parse_args()

    if args.list:
        for sid, desc, _ in SCENARIOS:
            print(f"{sid}: {desc}")
        return

    only = args.only.split(",") if args.only else None
    skip = set(args.skip.split(",")) if args.skip else set()
    picked = [(sid, d, fn) for sid, d, fn in SCENARIOS
              if (only is None or any(sid.startswith(p) for p in only))
              and not any(sid.startswith(p) for p in skip)]
    if not picked:
        print("no scenarios selected", file=sys.stderr)
        return sys.exit(1)

    fails = []
    for sid, desc, fn in picked:
        print(f"== {sid}: {desc}", flush=True)
        t0 = time.time()
        try:
            if fn.__code__.co_argcount == 0:
                fn()
            else:
                g = Game()
                try:
                    fn(g)
                finally:
                    g.close()
            print(f"   PASS ({time.time()-t0:.1f}s)", flush=True)
        except Exception as e:
            print(f"   FAIL: {type(e).__name__}: {e}", flush=True)
            fails.append(sid)
    n = len(picked)
    print(f"SCENARIOS {n - len(fails)}/{n} PASS"
          + (f" (failed: {', '.join(fails)})" if fails else ""))
    return sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
