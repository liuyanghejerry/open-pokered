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


@scenario("s13-full-party-catch", "full-party catch appears in PC and survives CONTINUE")
def s13_full_party_catch():
    with tempfile.TemporaryDirectory(prefix="pokered-catch-box-") as private:
        save_path = Path(private) / "catch.sav"
        g = Game(save_path=save_path)
        try:
            boot_starter(g, "Bulbasaur", 8)
            for _ in range(5):
                reply = g.d.cmd(cmd="give_pokemon", species="Rattata", level=5)
                assert reply["ok"], reply
            assert g.st()["party_count"] == 6
            reply = g.d.cmd(cmd="give_item", item="POKE_BALL", qty=20)
            assert reply["ok"], reply
            reply = g.d.cmd(cmd="start_wild_battle", species="Caterpie", level=3)
            assert reply["ok"], reply
            g.wait("screen=battle", 600)
            throw_balls_until_caught(g, 20)
            dismiss_dex_screen(g)
            before = g.st()
            assert before["party_count"] == 6
            assert before["box_counts"][0] == 1, before["box_counts"]
            assert before["stored_pokemon"][0]["species"] == "Caterpie"
            assert 0 <= bag_qty(g, "PokeBall") < 20
            assert g.d.cmd(cmd="save")["ok"]
        finally:
            g.close()
        g = Game(save_path=save_path)
        try:
            resume_reentry(g)
            after = g.st()
            assert after["party_count"] == 6
            assert after["box_counts"][0] == 1, after["box_counts"]
            assert after["stored_pokemon"][0]["species"] == "Caterpie"
            g.evidence("s13")
        finally:
            g.close()


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


@scenario("s11-forced-switch", "fainted leader → live replacement → battle victory")
def s11_forced_switch(g):
    boot_starter(g, "Magikarp", 1)
    assert g.d.cmd(cmd="give_pokemon", species="Wartortle", level=50)["ok"]
    assert g.d.cmd(cmd="start_wild_battle", species="Rattata", level=10)["ok"]
    g.smart_moves = True
    observed = set()
    original_state = g.st

    def observe():
        state = original_state()
        observed.add(state["battle_phase"])
        return state

    g.st = observe
    try:
        g.battle_loop()
    finally:
        g.st = original_state
    state = g.st()
    assert "PlayerFaintSwitch" in observed, observed
    assert state["screen"] == "overworld", state["screen"]
    assert state["party"][0]["hp"] == 0, state["party"]
    assert state["party"][1]["hp"] > 0, state["party"]
    assert state["battle_live"]["enemy"]["hp"] == 0, state["battle_live"]
    g.evidence("s11")


@scenario("s12-item-evolution", "real ITEM menus consume a stone and register evolution")
def s12_item_evolution():
    # JevGame contributes only the observed-menu input driver here.  The debug
    # protocol seeds the precondition; evolution and Pokédex registration must
    # result from the same START -> ITEM -> party menus a player uses.
    from types import SimpleNamespace
    from openpokered.playthrough_judgments import JevGame

    g = Game()
    try:
        m01_boot(g)
        m02_oak_speech(g)
        assert g.d.cmd(cmd="give_pokemon", species="Growlithe", level=20)["ok"]
        assert g.d.cmd(cmd="give_item", item="FIRE_STONE", qty=1)["ok"]
        g.judgments = SimpleNamespace(check_budget=lambda: None)
        JevGame.use_consumable(g, "FireStone", 0)
        for _ in range(30):
            state = g.st()
            if (state["party"][0]["species"] == "Arcanine"
                    and not state.get("evolution_phase")):
                break
            # Evolution is time-driven.  A only advances its intro/result
            # text; never press B, which would cancel the morph.
            g.step(120)
            g.tap("a", 4)
        else:
            raise AssertionError(f"evolution did not finish: {g.st()}")
        party = g.d.cmd(cmd="get_party")["data"]
        state = g.st()
        assert party[0]["species"] == "Arcanine", party[0]
        assert bag_qty(g, "FireStone") == 0, g.d.cmd(cmd="get_bag")["data"]
        assert "Arcanine" in state["pokedex"]["owned_species"], state["pokedex"]
        g.evidence("s12")
    finally:
        g.close()


@scenario("s14-capture-sleep", "Jev capture move selection applies native sleep and consumes PP")
def s14_capture_sleep():
    from types import SimpleNamespace
    from openpokered.playthrough_judgments import JevGame, capture_move_question

    def choose(_axis, _state, candidates, _instruction):
        return next(key for key, move in candidates.items() if move == "SleepPowder")

    g = Game(seed=42)
    try:
        boot_starter(g, "Gloom", 21)
        assert g.d.cmd(cmd="give_item", item="POKE_BALL", qty=20)["ok"]
        g.judgments = SimpleNamespace(collects_dex=True, active=None,
            choose=choose, record=lambda *args, **kwargs: None)
        g.move_cache, g.move_cache_hits, g.active_milestone = {}, 0, None
        assert g.d.cmd(cmd="start_wild_battle", species="Caterpie", level=3)["ok"]

        def settle_turn():
            for _ in range(200):
                state = g.st()
                if state["battle_phase"] == "PlayerMenu":
                    return state
                g.tap("a", 8)
                g.step(30)
            raise AssertionError("native turn did not return to PlayerMenu")

        settle_turn()
        for _ in range(5):
            for button in ("up", "left", "a"):
                g.tap(button, 8)
            state = g.st()
            assert state["battle_phase"] == "MoveSelect", state["battle_phase"]
            before = next(m["pp"] for m in state["battle_moves"]["moves"]
                          if m["move"] == "SleepPowder")
            JevGame._select_move(g)
            state = settle_turn()
            if state["battle_live"]["enemy"]["status"].startswith("Sleep"):
                break
        else:
            raise AssertionError("five real SleepPowder turns failed to apply sleep")
        for button in ("up", "left", "a"):
            g.tap(button, 8)
        state = g.st()
        after = next(m["pp"] for m in state["battle_moves"]["moves"]
                     if m["move"] == "SleepPowder")
        assert after == before-1, (before, after)
        _, candidates = capture_move_question(state, state["battle_moves"])
        assert "SleepPowder" not in candidates.values(), candidates
        g.evidence("s14")
    finally:
        g.close()


@scenario("s15-static-retreat", "Jev menu RUN preserves the native Zapdos source for a retry")
def s15_static_retreat():
    _static_retreat(ball_qty=20)


@scenario("s19-selected-move-pp", "The first nonzero move slot spends its own PP, not slot zero")
def s19_selected_move_pp():
    from types import SimpleNamespace
    from openpokered.playthrough_judgments import JevGame

    def choose(_axis, _state, candidates, _instruction):
        return next(key for key, move in candidates.items() if move == "SleepPowder")

    g = Game(seed=42)
    try:
        boot_starter(g, "Gloom", 21)
        assert g.d.cmd(cmd="give_item", item="POKE_BALL", qty=20)["ok"]
        g.judgments = SimpleNamespace(collects_dex=True, active=None,
            choose=choose, record=lambda *args, **kwargs: None)
        g.move_cache, g.move_cache_hits, g.active_milestone = {}, 0, None
        assert g.d.cmd(cmd="start_wild_battle", species="Caterpie", level=3)["ok"]

        def settle_turn():
            for _ in range(200):
                state = g.st()
                if state["battle_phase"] == "PlayerMenu":
                    return state
                g.tap("a", 8)
                g.step(30)
            raise AssertionError("native turn did not return to PlayerMenu")

        def open_moves():
            for button in ("up", "left", "a"):
                g.tap(button, 8)
            state = g.st()
            assert state["battle_phase"] == "MoveSelect"
            return state["battle_moves"]["moves"]

        settle_turn()
        before = open_moves()
        selected = next(i for i, move in enumerate(before) if move["move"] == "SleepPowder")
        assert selected != 0, before
        JevGame._select_move(g)
        settle_turn()
        after = open_moves()
        expected = [move["pp"] - (i == selected) for i, move in enumerate(before)]
        actual = [move["pp"] for move in after]
        assert actual == expected, {"selected": selected, "before": before,
                                    "expected": expected, "actual": actual}
        g.evidence("s19")
    finally:
        g.close()


@scenario("s20-transformed-capture", "A transformed wild Ditto is caught and registered as Ditto")
def s20_transformed_capture():
    from types import SimpleNamespace
    from openpokered.playthrough_judgments import JevGame

    g = Game(seed=42)
    try:
        boot_starter(g, "Gloom", 21)
        assert g.d.cmd(cmd="give_item", item="MASTER_BALL", qty=2)["ok"]
        # Register the copied form first, so this covers the exact real-run
        # failure: an unregistered Ditto copies an already-owned Gloom.
        assert g.d.cmd(cmd="start_wild_battle", species="Gloom", level=1)["ok"]
        throw_balls_until_caught(g, max_balls=5)
        dismiss_dex_screen(g)
        assert "Gloom" in g.st()["pokedex"]["owned_species"]
        g.judgments = SimpleNamespace(collects_dex=True, active=None,
            choose=lambda _axis, _state, candidates, _instruction:
                ('ball:MasterBall' if 'ball:MasterBall' in candidates else
                 next(key for key, move in candidates.items() if move == "StunSpore")),
            record=lambda *args, **kwargs: None)
        g.move_cache, g.move_cache_hits, g.active_milestone = {}, 0, None
        assert g.d.cmd(cmd="start_wild_battle", species="Ditto", level=26)["ok"]
        def settle_turn():
            for _ in range(200):
                state = g.st()
                if state["battle_phase"] == "PlayerMenu":
                    return state
                g.tap("a", 8)
                g.step(30)
            raise AssertionError("Transform turn did not settle")

        settle_turn()
        for button in ("up", "left", "a"):
            g.tap(button, 8)
        JevGame._select_move(g)
        state = settle_turn()
        assert state["battle_live"]["enemy"]["species"] == "Gloom", state["battle_live"]
        assert state["battle_live"]["enemy"]["capture_species"] == "Ditto", state["battle_live"]
        assert state["battle_live"]["enemy"]["capture_catch_rate"] == 35, state["battle_live"]
        assert JevGame.battle_recovery_plan(g, state) == ("MasterBall", None)
        throws = throw_balls_until_caught(g, max_balls=5)
        dismiss_dex_screen(g)
        party = g.d.cmd(cmd="get_party")["data"]
        assert throws == 1, throws
        assert party[-1]["species"] == "Ditto", party[-1]
        assert "Ditto" in g.st()["pokedex"]["owned_species"], g.st()["pokedex"]
        g.evidence("s20")
    finally:
        g.close()


@scenario("s21-capture-support-training", "A capture support shares real victory XP after switching to a finisher")
def s21_capture_support_training():
    from types import SimpleNamespace
    from openpokered.playthrough_judgments import JevGame
    from openpokered.story_rules import StoryIndex

    def choose(_axis, _state, candidates, _instruction):
        if 'switch:1' in candidates:
            return 'switch:1'
        if 'skip' in candidates:
            return 'skip'
        return next(key for key, move in candidates.items() if move == 'Flamethrower')

    g = Game(seed=42)
    try:
        boot_starter(g, 'Gloom', 5)
        assert g.d.cmd(cmd='give_pokemon', species='Charizard', level=70)['ok']
        g.judgments = SimpleNamespace(collects_dex=True,
            active={'target': ('level', 'Gloom', 6), 'context': {
                'capture_support_training': True, 'trigger': 'level', 'from_species': 'Gloom'}},
            choose=choose, record=lambda *args, **kwargs: None)
        g.move_cache, g.move_cache_hits, g.active_milestone = {}, 0, None
        g.battles_driven, g.smart_moves = 0, True
        g.battle_recovery_plan = lambda state: JevGame.battle_recovery_plan(g, state)
        g._select_move = lambda: JevGame._select_move(g)
        g.battle_party_target = lambda state: JevGame.battle_party_target(g, state)
        before = g.st()['party']
        index = StoryIndex.__new__(StoryIndex)
        assert not index.satisfied(('level', 'Gloom', 6), {'party': before})
        assert g.d.cmd(cmd='start_wild_battle', species='Metapod', level=40)['ok']
        for _ in range(200):
            state = g.st()
            if state['battle_phase'] == 'PlayerMenu':
                break
            g.tap('a', 8)
            g.step(30)
        else:
            raise AssertionError('support training battle did not reach the first menu')
        assert g.battle_recovery_plan(state) == ('switch', 1), 'capture_support_finisher_not_offered'
        for button in ('up', 'right', 'a'):
            g.tap(button, 8)
        assert g.st()['battle_phase'] == 'PartySelect'
        while g.st()['battle_party_cursor'] != 1:
            g.tap('down', 8)
        g.tap('a', 8)
        g.battle_loop(max_iters=400)
        after = g.st()
        assert after['screen'] == 'overworld'
        assert after['party'][0]['species'] == 'Gloom'
        assert after['party'][0]['hp'] > 0
        assert after['party'][0]['level'] > before[0]['level'], (before, after['party'])
        assert index.satisfied(('level', 'Gloom', 6), {'party': after['party']})
        assert after['party'][1]['hp'] > 0
        print(f"   support Gloom Lv{before[0]['level']} → Lv{after['party'][0]['level']}, "
              f"HP={after['party'][0]['hp']}; finisher survived", flush=True)
        g.evidence('s21')
    finally:
        g.close()


@scenario("s22-overlevel-evolution", "A captured overlevel Tentacool evolves only after a new real XP level-up")
def s22_overlevel_evolution():
    from types import SimpleNamespace
    from openpokered.playthrough_judgments import JevGame

    def choose(_axis, _state, candidates, _instruction):
        if 'switch:1' in candidates:
            return 'switch:1'
        if 'skip' in candidates:
            return 'skip'
        return next(key for key, move in candidates.items() if move == 'Flamethrower')

    g = Game(seed=42)
    try:
        boot_starter(g, 'Tentacool', 40)
        assert g.d.cmd(cmd='give_pokemon', species='Charizard', level=100)['ok']
        g.judgments = SimpleNamespace(collects_dex=False,
            active={'target': ('register', 'Tentacruel', True), 'context': {
                'acquisition_method': 'evolution', 'trigger': 'level', 'from_species': 'Tentacool'}},
            choose=choose, record=lambda *args, **kwargs: None)
        g.move_cache, g.move_cache_hits, g.active_milestone = {}, 0, None
        g.smart_moves = True
        g.battle_recovery_plan = lambda state: JevGame.battle_recovery_plan(g, state)
        g._select_move = lambda: JevGame._select_move(g)
        g.battle_party_target = lambda state: JevGame.battle_party_target(g, state)
        assert 'Tentacruel' not in g.st()['pokedex']['owned_species']
        for victory in range(12):
            assert g.d.cmd(cmd='start_wild_battle', species='Metapod', level=100)['ok']
            g.battle_loop(max_iters=400)
            # Finish the real post-battle evolution presentation with A only.
            for _ in range(180):
                state = g.st()
                if not state.get('evolution_phase') and not state.get('evolution'):
                    break
                g.tap('a', 8)
                g.step(30)
            state = g.st()
            if state['party'][0]['species'] == 'Tentacruel':
                break
            assert state['party'][0]['level'] == 40, state['party'][0]
        else:
            raise AssertionError('twelve real shared-XP victories did not trigger evolution')
        assert state['party'][0]['level'] > 40, state['party'][0]
        assert 'Tentacruel' in state['pokedex']['owned_species']
        assert state['party'][0]['hp'] > 0 and state['party'][1]['hp'] > 0
        print(f"   Tentacool Lv40 → Tentacruel Lv{state['party'][0]['level']} "
              f"after {victory+1} real victories", flush=True)
        g.evidence('s22')
    finally:
        g.close()


def _static_retreat(ball_qty):
    from types import SimpleNamespace
    from openpokered.playthrough_judgments import JevGame
    from openpokered.story_rules import StoryIndex

    g = Game(seed=42)
    try:
        boot_starter(g, "Nidoqueen", 70)
        if ball_qty:
            assert g.d.cmd(cmd="give_item", item="POKE_BALL", qty=ball_qty)["ok"]
        semantics = lambda name=None: (g.d.cmd(cmd="get_script_semantics", map=name)["data"]
                                     if name else {"maps": ["PowerPlant"]})
        index = StoryIndex(SimpleNamespace(script_semantics=semantics))
        assert not index.errors, index.errors
        choices = []
        def choose(_axis, _state, candidates, _instruction):
            assert "run" in candidates, candidates
            choices.append("run")
            return "run"
        g.judgments = SimpleNamespace(index=index, collects_dex=True, active=None,
            choose=choose, record=lambda *args, **kwargs: None)
        g.smart_moves = True
        g.battle_recovery_plan = lambda state: JevGame.battle_recovery_plan(g, state)
        g.move_cache, g.move_cache_hits, g.active_milestone = {}, 0, None
        assert g.d.cmd(cmd="warp", map="PowerPlant", x=4, y=10)["ok"]
        g.step(180)

        def encounter():
            g.tap("up", 8)
            g.tap("a", 8)
            for _ in range(100):
                state = g.st()
                if state["screen"] == "battle":
                    assert state["battle_live"]["enemy"]["species"] == "Zapdos"
                    assert state["script_awaiting_battle"]
                    return
                g.d.cmd(cmd="skip_dialogue")
                g.step(20)
            raise AssertionError("static Zapdos encounter did not start")

        encounter()
        for _ in range(100):
            if g.st()['battle_phase'] == 'PlayerMenu':
                break
            g.tap('a', 8)
            g.step(20)
        else:
            raise AssertionError('native PlayerMenu did not appear')
        for button in ('up', 'left', 'a'):
            g.tap(button, 8)
        assert g.st()['battle_phase'] == 'MoveSelect'
        from openpokered.story_agent import StoryStopped
        def abstain(*args):
            raise StoryStopped('action:no_selection')
        g.judgments.choose = abstain
        JevGame._select_move(g)
        assert g.st()['battle_phase'] == 'PlayerMenu', g.st()['battle_phase']
        g.judgments.choose = choose
        g.battle_loop()
        g.step(180)
        assert choices, "production escape hook was not used"
        assert g.st()["screen"] == "overworld"
        bird = next(n for n in g.d.cmd(cmd="get_npcs")["data"] if n["text_id"] == 9)
        assert bird["visible"], bird
        assert "Zapdos" not in g.st()["pokedex"]["owned_species"]
        assert not g.d.cmd(cmd="get_flags")["data"].get("EVENT_BEAT_ZAPDOS", False)
        # Load-time scripts must not silently spend the source either.
        assert g.d.cmd(cmd="warp", map="Route10", x=10, y=10)["ok"]
        g.step(180)
        assert g.d.cmd(cmd="warp", map="PowerPlant", x=4, y=10)["ok"]
        g.step(180)
        # Start the same native source again: visibility alone is not enough.
        encounter()
        g.evidence("s15")
    finally:
        g.close()


@scenario("s18-empty-ball-static-retreat", "No balls preserves static capture intent and native Zapdos after RUN/re-entry")
def s18_empty_ball_static_retreat():
    _static_retreat(ball_qty=0)


# ── runner ──────────────────────────────────────────────────────────────
@scenario("s16-restless-soul", "Revealed Tower spirit rejects Master Ball without consuming it")
def s16_restless_soul():
    from openpokered.playthrough_judgments import ball_options
    g = Game(seed=42)
    try:
        boot_starter(g, "Charizard", 60)
        assert g.d.cmd(cmd="give_item", item="SILPH_SCOPE", qty=1)["ok"]
        assert g.d.cmd(cmd="give_item", item="MASTER_BALL", qty=1)["ok"]
        assert g.d.cmd(cmd="warp", map="PokemonTower6F", x=10, y=14)["ok"]
        assert g.d.cmd(cmd="start_wild_battle", species="Marowak", level=30)["ok"]
        for _ in range(200):
            state = g.st()
            if state["battle_phase"] == "PlayerMenu":
                break
            g.tap("a", 8)
            g.step(30)
        else:
            raise AssertionError("ghost reveal did not reach battle menu")
        live = state["battle_live"]
        assert live["capture_blocked_reason"] == "restless_soul", live
        assert not live["is_ghost"], live
        assert not list(ball_options(live, {"MasterBall": 1})), live
        owned = state["pokedex"]["owned_species"]
        # Real input deliberately attempts the forbidden throw, testing the
        # engine boundary independently of the controller's legal-option filter.
        for button in ("down", "left", "a"):
            g.tap(button, 8)
        state = g.st()
        assert state["battle_phase"].startswith("BagSelect"), state["battle_phase"]
        # Silph Scope is slot zero, Master Ball slot one.
        g.tap("down", 8)
        g.tap("a", 8)
        for _ in range(200):
            state = g.st()
            if state["battle_phase"] == "PlayerMenu":
                break
            g.tap("a", 8)
            g.step(30)
        else:
            raise AssertionError("rejected throw did not return to menu")
        assert bag_qty(g, "MasterBall") == 1
        assert state["pokedex"]["owned_species"] == owned
        g.evidence("s16")
    finally:
        g.close()


@scenario("s17-pc-withdraw-capacity", "Full-box withdrawal immediately reopens native capture capacity")
def s17_pc_withdraw_capacity():
    from types import SimpleNamespace
    from playthrough_late import catch_master_ball
    from openpokered.autonomous_story import AutonomousStoryAgent
    g = Game(seed=42)
    try:
        boot_starter(g, "Charizard", 60)
        for _ in range(5):
            assert g.d.cmd(cmd="give_pokemon", species="Rattata", level=5)["ok"]
        assert g.d.cmd(cmd="give_item", item="MASTER_BALL", qty=21)["ok"]
        for _ in range(20):
            assert g.d.cmd(cmd="start_wild_battle", species="Cubone", level=24)["ok"]
            catch_master_ball(g)
        assert g.st()["box_counts"][0] == 20
        assert g.d.cmd(cmd="warp", map="FuchsiaPokecenter", x=12, y=3)["ok"]

        def command(**args):
            if args["cmd"] == "press_timeline":
                args["advance"] = True
            response = g.d.cmd(**args)
            assert response["ok"], response
            return response.get("data")

        def interact(sign):
            command(cmd="interact_with", id=sign)
            for _ in range(40):
                if g.st().get("pc_state"):
                    return
                g.tap("a", 4)
                g.step(13)
            raise AssertionError("PC did not open")

        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {"target": ("pokemon", "Cubone", None)}
        agent.check_budget = lambda: None
        agent.client = SimpleNamespace(state=g.st, step=g.step, cmd=command,
                                       interact_with=interact)
        agent.retrieve_from_pc(0, 0, 5, 0)
        state = g.st()
        assert state["box_counts"][:2] == [19, 1], state["box_counts"]
        assert len(state["party"]) == 6 and state["party"][-1]["species"] == "Cubone"
        assert g.d.cmd(cmd="start_wild_battle", species="Caterpie", level=3)["ok"]
        for _ in range(100):
            state = g.st()
            if state["battle_phase"] == "PlayerMenu":
                break
            g.tap("a", 8)
            g.step(30)
        assert state["battle_live"]["capture_blocked_reason"] is None
        catch_master_ball(g)
        assert g.st()["box_counts"][:2] == [20, 1]
        assert bag_qty(g, "MasterBall") == 0
        g.evidence("s17")
    finally:
        g.close()


@scenario("s23-battle-presentation-budget", "Native animation waits do not spend the 400 battle input iterations")
def s23_battle_presentation_budget():
    """Seeded diagnostic only; real five-opponent battle, no outcome injection."""
    from playthrough_late import use_item
    g = Game(seed=42, speed=0)
    try:
        g.smart_moves = True
        boot_starter(g, "Venusaur", 48)
        for item, qty in (("HM01", 1), ("TM21", 1), ("HYPER_POTION", 8)):
            assert g.d.cmd(cmd="give_item", item=item, qty=qty)["ok"]
        use_item(g, "Hm01", party_index=0, forget="Growth")
        use_item(g, "Tm21", party_index=0, forget="Poisonpowder")
        assert g.d.cmd(cmd="warp", map="SilphCo7F", x=3, y=1)["ok"]
        g.wait("control_ready", 240)
        assert g.d.cmd(cmd="set_seed", seed=42)["ok"]
        g.d.drive(["down"] * 16, frames=20)
        for _ in range(100):
            state = g.st()
            if state["screen"] == "battle":
                break
            if state.get("dialogue_state"):
                g.skip()
            else:
                g.tap("a", 30)
        else:
            raise AssertionError("Rival fixture did not enter battle")
        assert len(state["battle_live"]["enemy_party"]) == 5
        tap, step = g.tap, g.step
        waited = 0
        def ready_tap(*args, **kwargs):
            assert not g.st()["battle_presentation"]["waiting"], "input while native presentation is blocked"
            return tap(*args, **kwargs)
        def observe_step(frames):
            nonlocal waited
            if g.st()["battle_presentation"]["waiting"]:
                waited += frames
            return step(frames)
        g.tap, g.step = ready_tap, observe_step
        g.battle_loop(max_iters=400)
        final = g.st()
        assert final["screen"] == "overworld" and "player_won: true" in final["battle_phase"], final
        assert all(mon["hp"] == 0 for mon in final["battle_live"]["enemy_party"])
        assert waited > 0, "Fixture did not exercise native presentation waiting"
        g.evidence("s23")
    finally:
        g.close()


@scenario("s24-coordinate-falls", "Native coordinate holes match navigation; ordinary stairs remain usable")
def s24_coordinate_falls():
    import playthrough as nav
    g = Game(seed=42, speed=0)
    try:
        boot_starter(g, 'Charizard', 60)
        g.smart_moves = True
        # Isolated fixture: suppress the separate downstream current scripts.
        # These writes are not collection progress or a formal-run shortcut.
        for floor in (3, 4):
            for boulder in (1, 2):
                reply = g.d.cmd(cmd='set_flag', name=f'EVENT_SEAFOAM{floor}_BOULDER{boulder}_DOWN_HOLE',
                                value=True)
                assert reply['ok'], reply
        expected = {
            'SeafoamIslands1F': 2, 'SeafoamIslandsB1F': 2,
            'SeafoamIslandsB2F': 2, 'SeafoamIslandsB3F': 2,
            'PokemonMansion3F': 3,
        }
        for name, count in expected.items():
            falls = nav.COORDINATE_WARPS.get(name, {})
            assert len(falls) == count, (name, falls)
            for (x, y), destination in falls.items():
                approaches = [(x+dx, y+dy) for dx, dy in nav.DELTA.values()
                              if nav.walkable_edge(name, (x+dx, y+dy), (x, y))
                              and nav.walkable(name, x+dx, y+dy)
                              and (x+dx, y+dy) not in nav.warp_tiles(name)]
                assert approaches, (name, x, y)
                sx, sy = approaches[0]
                assert g.d.cmd(cmd='warp', map=name, x=sx, y=sy)['ok']
                g.step(120)
                plan = nav.bfs_cross(name, (sx, sy), destination[0], destination[1:])
                assert len(plan) == 2 and plan[1][1].startswith('fall_'), plan
                g.nav_to_map(*destination[1:], destination[0], tries=10, avoid_grass=False)
                assert g.pos() == destination, (name, (x, y), destination, g.pos())
                g.evidence(f's24-{name}-{x}-{y}')
        assert g.d.cmd(cmd='warp', map='SeafoamIslands1F', x=24, y=3)['ok']
        g.step(120)
        target = nav.warp_edges_from('SeafoamIslands1F', 25, 3)[0]
        g.nav_to_map(*target[1:], target[0], tries=10, avoid_grass=False)
        assert g.pos() == target, (target, g.pos())
        g.evidence('s24-stairs')
    finally:
        g.close()


@scenario("s25-boulder-coordinate-hole", "A boulder may enter its target hole; the player falls only on the next step")
def s25_boulder_coordinate_hole():
    from save_builder import SaveBuilder
    from playthrough_late import push_boulder
    with tempfile.TemporaryDirectory(prefix='pokered-boulder-hole-') as folder:
        folder = Path(folder)
        builder = (SaveBuilder().party_add('Nidoqueen', 50, moves=['Strength', 'BodySlam'])
                   .badges({'Rainbow': True}).position('VictoryRoad3F', 21, 15))
        path = builder.write(folder / 'fixture.json')
        g = Game(snapshot=path, save_path=folder / 'fixture.sav', seed=42, speed=0)
        try:
            resume_reentry(g)
            flag = 'EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2'
            assert not g.d.cmd(cmd='get_flags')['data'].get(flag)
            push_boulder(g, 'VictoryRoad3F', 10, (23, 15), flag)
            assert g.d.cmd(cmd='get_flags')['data'][flag]
            assert g.pos() == ('VictoryRoad3F', 22, 15), g.pos()
            g.nav_to(23, 15, 'VictoryRoad3F')
            assert g.cutscene()
            assert g.pos() == ('VictoryRoad2F', 22, 16), g.pos()
            g.evidence('s25')
        finally:
            g.close()


@scenario("s26-safari-save-allowance", "Paid Safari allowance survives save and independent CONTINUE without a refill")
def s26_safari_save_allowance():
    import playthrough as pt
    from save_builder import SaveBuilder
    from openpokered.autonomous_story import training_tile
    from openpokered.collection_verification import verify_collection_continue
    from openpokered.playthrough_judgments import ObservedProtocol

    # Seed only this isolated subsystem fixture. Admission, walking, throwing
    # and persistence use native gameplay; this is not autonomous dex evidence.
    with tempfile.TemporaryDirectory(prefix='pokered-safari-save-') as folder:
        folder = Path(folder)
        binary = folder / 'pokered-app'
        shutil.copy2(pt.BIN, binary)
        fixture = (SaveBuilder().party_add('Bulbasaur', 5).money(2000)
                   .position('SafariZoneGate', 3, 3).write(folder / 'fixture.json'))
        saved = folder / 'safari.sav'
        g = Game(binary=binary, snapshot=fixture, save_path=saved, seed=42, speed=0)
        raw = g.d
        g.d = ObservedProtocol(raw, lambda *args, **kwargs: None, time.monotonic() + 120)
        g.smart_moves = True
        try:
            resume_reentry(g)
            assert not g.st()['safari_game']['active']
            g.d.drive(['up'] * 8, frames=16)
            g.dialogue_then_choice()
            g.choose('YES')
            assert g.cutscene()
            state = g.st()
            assert state['map_name'] == 'SafariZoneCenter' and state['money'] == 1500
            assert state['safari_game'] == {
                'active': True, 'steps_remaining': 500, 'balls_remaining': 30}

            # Use the native encounter terrain predicate, not Overworld-only
            # grass tiles: Safari maps use the Forest tileset.
            grass = {(x, y) for x in range(pt.MAPS['SafariZoneCenter']['width'] * 2)
                     for y in range(pt.MAPS['SafariZoneCenter']['height'] * 2)
                     if training_tile('SafariZoneCenter', x, y)}
            origin = (state['player_x'], state['player_y'])
            target = next(xy for xy in sorted(grass, key=lambda xy:
                abs(xy[0] - origin[0]) + abs(xy[1] - origin[1]))
                if pt.bfs('SafariZoneCenter', origin, xy))
            g.nav_to(*target, 'SafariZoneCenter')
            for step in range(200):
                state = g.st()
                if state['screen'] == 'battle':
                    break
                assert state['safari_game']['active'], state['safari_game']
                x, y = state['player_x'], state['player_y']
                choices = [direction for direction, (dx, dy) in pt.DELTA.items()
                           if (x + dx, y + dy) in grass
                           and pt.walkable_edge('SafariZoneCenter', (x, y), (x + dx, y + dy))]
                assert choices, (x, y)
                g.d.drive([choices[step % len(choices)]] * 8, frames=12)
            else:
                raise AssertionError('No natural Safari encounter')
            assert g.st()['battle_live']['is_safari']
            g.safari_battle_action = lambda state: (
                'ball' if state['battle_live']['safari']['balls'] == 30 else 'run')
            g.battle_loop(prefer='run')
            assert g.cutscene()
            safari = g.st()['safari_game']
            assert safari['active'] and safari['balls_remaining'] == 29, safari
            assert 0 < safari['steps_remaining'] < 500, safari
            observations = {cmd: g.d.cmd(cmd=cmd) for cmd in (
                'get_state', 'get_party', 'get_bag', 'get_flags')}
            assert raw.cmd(cmd='save')['ok']
        finally:
            g.close()
        proof = verify_collection_continue(saved, binary, observations,
                                            folder / 'pokered.script_flags.json')
        assert proof['verified'] and proof['restored']['state']['safari_game'] == safari
        print(f"   paid 500; spent one native ball; independently restored {safari}")


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
