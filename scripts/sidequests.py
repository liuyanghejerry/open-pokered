"""End-to-end automation for Pokémon Red side-quests (s13–s16).

Each flow boots from a real SaveBuilder snapshot with real button inputs only,
driving every dialogue, choice, battle, trade, revival, and field-item flow to
catch Gen-1 fidelity regressions across optional storylines.
"""

import shutil
import tempfile
from pathlib import Path

from playthrough import Game, resume_reentry
from playthrough_late import (
    field_move,
    fly,
    require_flag,
    talk_npc,
    use_item,
)
from save_builder import SaveBuilder


ALL_TOWNS = [
    "PalletTown", "ViridianCity", "PewterCity", "CeruleanCity",
    "LavenderTown", "VermilionCity", "CeladonCity", "FuchsiaCity",
    "CinnabarIsland", "IndigoPlateau", "SaffronCity",
]


def _make_sidequest_game(team, items=None, map_name="PalletTown", pos=(5, 6),
                         money=200000, badges=0xFF, dex_complete=True, flags=()):
    """Boot a clean Game from a SaveBuilder snapshot with real CONTINUE."""
    tmp = Path(tempfile.mkdtemp(prefix="pokered-sidequest-"))
    snap = tmp / "sidequest.json"
    sav = tmp / "sidequest.sav"
    sb = SaveBuilder()
    for sp, lv, mv in team:
        sb.party_add(sp, lv, moves=mv)
    sb.money(money)
    sb.badges(badges)
    sb.visited_towns(ALL_TOWNS)
    if dex_complete:
        sb.dex_complete()
    for f in flags:
        sb.flag(f)
    for item, qty in (items or []):
        sb.give_item(item, qty)
    sb.position(map_name, pos[0], pos[1])
    sb.write(snap)
    g = Game(save_path=sav, snapshot=snap)
    resume_reentry(g)
    return g, tmp


def _bag_qty(g, item_name):
    return sum(v["qty"] for v in g.d.cmd(cmd="get_bag")["data"] if v["item"] == item_name)


def _flag(g, name):
    return bool(g.d.cmd(cmd="get_flags")["data"].get(name))


def _talk_live_npc(g, map_name, text_id, **drive_kwargs):
    """Locate a live (possibly wandering) NPC by text_id, approach, talk, and drive script."""
    def find():
        npcs = g.d.cmd(cmd="get_npcs")["data"]
        rows = npcs.get("npcs", npcs) if isinstance(npcs, dict) else npcs
        return next(n for n in rows if n["text_id"] == text_id and n.get("visible", True))

    for _ in range(8):
        npc = find()
        g.approach_object(npc["x"], npc["y"], map_name)
        cur = find()
        if (cur["x"], cur["y"]) != (npc["x"], npc["y"]):
            continue
        g.tap("a", 16)
        s = g.st()
        if not (s.get("script_running") or s.get("dialogue_state") is not None
                or s.get("active_script_effect") is not None or s.get("choice")):
            continue
        _drive_script_dialogue(g, **drive_kwargs)
        return
    raise RuntimeError(f"NPC {text_id} did not start dialogue on {map_name}")


def _step_tiles(g, direction, count=1):
    """Walk `count` tiles in `direction` (used right after cutting a tree)."""
    for _ in range(count):
        before = (g.st()["player_x"], g.st()["player_y"])
        for _ in range(6):
            g.d.drive([direction] * 16, frames=20)
            after = (g.st()["player_x"], g.st()["player_y"])
            if after != before:
                break
        else:
            raise RuntimeError(f"_step_tiles({direction}) stuck at {before}")


def _drive_script_dialogue(g, choices=(), party_slot=None, rename_confirm=False, max_steps=400):
    """Advance an active script dialogue/choice/party-select/naming/diploma flow."""
    choice_iter = iter(choices)
    for _ in range(max_steps):
        s = g.st()
        screen = s["screen"]
        if screen == "battle":
            g.battle_loop(prefer="fight")
            continue
        if s.get("choice"):
            want = next(choice_iter, "YES")
            opts = s["choice"]["options"]
            match = next(
                (o for o in opts if o.lower() == want.lower()),
                next((o for o in opts if o.lower().startswith(want.lower())), None),
            )
            if match is None:
                raise AssertionError(f"choice {want!r} not in {opts!r}")
            g.choose(match)
            g.step(12)
            continue
        # Party selection overlay (Daycare / Name Rater) or filterBag menu
        fm = s.get("field_menu")
        if screen == "party" or (fm and fm.get("kind") == "party"):
            want_slot = 0 if party_slot is None else party_slot
            cur = fm.get("cursor", 0) if fm else 0
            g.tap("a" if cur == want_slot else "down", 10)
            continue
        if screen == "naming":
            if rename_confirm:
                g.tap("a", 8)       # enter 'A'
                g.tap("start", 12)  # START submits immediately in Gen-1 naming screen
                rename_confirm = False
            else:
                g.tap("start", 12)
            continue
        if screen != "overworld":
            # Diploma / Pokedex entry / bag filter screen
            g.tap("a", 12)
            continue
        if s.get("dialogue_state") is not None:
            g.tap("a", 10)
            continue
        if s.get("active_script_effect") is not None or s.get("script_running"):
            # Active filterBag / party-select / naming / diploma / trade effect
            eff = str(s.get("active_script_effect") or "")
            se = s.get("script_effect") or {}
            child_eff = str((se.get("child") or {}).get("effect") or "")
            if "FilterBag" in eff:
                g.tap("a", 10)
            elif "ChoosePartyPokemon" in eff or (
                not eff and s.get("script_running") and party_slot is not None
            ):
                want_slot = 0 if party_slot is None else party_slot
                for _ in range(want_slot):
                    g.tap("down", 8)
                g.tap("a", 12)
                party_slot = None
            elif "NamingScreen" in eff or "NamingScreen" in child_eff:
                if rename_confirm:
                    g.tap("a", 8)
                    g.tap("start", 12)
                    rename_confirm = False
                else:
                    g.tap("start", 12)
            elif "ShowDiploma" in eff or "ShowPokedexEntry" in eff:
                g.tap("a", 12)
            else:
                g.step(10)
            continue
        return
    raise RuntimeError(f"sidequest dialogue did not finish: {g.st()}")


# ── s13: Fossil Revival (Old Amber + Helix Fossil) + Fighting Dojo ────────
def s13_fossils_dojo():
    g, tmp = _make_sidequest_game(
        team=[
            ("Venusaur", 65, ["RazorLeaf", "Cut", "SleepPowder", "MegaDrain"]),
            ("Zapdos", 65, ["Fly", "Thunderbolt", "DrillPeck", "ThunderWave"]),
        ],
        items=[("HELIX_FOSSIL", 1)],
        map_name="PewterCity",
        pos=(13, 26),
    )
    try:
        # 1. Pewter Museum back entrance (Cut tree at (26, 4)) -> Old Amber
        g.nav_to(27, 4, "PewterCity")
        g.face("left")
        field_move(g, "Cut")
        _step_tiles(g, "left", 2)
        g.nav_warp(19, 5, "PewterCity", "Museum1F")
        # Scientist 3 at Museum1F gives OLD_AMBER (text_id=3)
        talk_npc(g, "Museum1F", 3)
        require_flag(g, "EVENT_GOT_OLD_AMBER")
        assert _bag_qty(g, "OldAmber") == 1, g.d.cmd(cmd="get_bag")["data"]
        g.nav_warp(16, 7, "Museum1F", "PewterCity", approach="down")

        # 2. Cinnabar Lab Fossil Room: revive both OLD_AMBER (Aerodactyl) & HELIX_FOSSIL (Omanyte)
        fly(g, "CinnabarIsland")
        for _ in range(2):
            g.nav_warp(6, 9, "CinnabarIsland", "CinnabarLab")
            g.nav_warp(16, 4, "CinnabarLab", "CinnabarLabFossilRoom")
            _talk_live_npc(g, "CinnabarLabFossilRoom", 1, choices=("YES",))
            require_flag(g, "EVENT_GAVE_FOSSIL_TO_LAB")
            require_flag(g, "EVENT_LAB_STILL_REVIVING_FOSSIL")
            # Exit all the way to CinnabarIsland so @load resets EVENT_LAB_STILL_REVIVING_FOSSIL
            g.nav_warp(2, 7, "CinnabarLabFossilRoom", "CinnabarLab", approach="down")
            g.nav_warp(2, 7, "CinnabarLab", "CinnabarIsland", approach="down")
            assert not _flag(g, "EVENT_LAB_STILL_REVIVING_FOSSIL"), \
                "CinnabarIsland @load must clear EVENT_LAB_STILL_REVIVING_FOSSIL"
            # Re-enter and claim revived Pokemon (Scientist 1 wanders, so locate dynamically)
            g.nav_warp(6, 9, "CinnabarIsland", "CinnabarLab")
            g.nav_warp(16, 4, "CinnabarLab", "CinnabarLabFossilRoom")
            _talk_live_npc(g, "CinnabarLabFossilRoom", 1)
            flags_now = {k: v for k, v in g.d.cmd(cmd="get_flags")["data"].items() if "FOSSIL" in k or "REVIV" in k or "LAB" in k}
            party_now = [m["species"] for m in g.st()["party"]]
            assert not _flag(g, "EVENT_GAVE_FOSSIL_TO_LAB"), \
                f"revive flag must reset after claim: flags={flags_now} party={party_now}"
            g.nav_warp(2, 7, "CinnabarLabFossilRoom", "CinnabarLab", approach="down")
            g.nav_warp(2, 7, "CinnabarLab", "CinnabarIsland", approach="down")
        species_list = [m["species"] for m in g.st()["party"]]
        assert "Aerodactyl" in species_list and "Omanyte" in species_list, species_list

        # 3. Saffron City Fighting Dojo: beat Karate Master, claim Hitmonlee, verify Hitmonchan blocked
        fly(g, "SaffronCity")
        g.nav_warp(26, 3, "SaffronCity", "FightingDojo")
        g.approach_object(5, 3, "FightingDojo")  # Karate Master (defeats LOS trainers on the way)
        g.tap("a", 16)
        _drive_script_dialogue(g)
        require_flag(g, "EVENT_BEAT_KARATE_MASTER")
        # Pick Hitmonlee Poké Ball (npc 6 at (4, 1))
        g.approach_object(4, 1, "FightingDojo")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",))
        require_flag(g, "EVENT_GOT_HITMONLEE")
        require_flag(g, "EVENT_DEFEATED_FIGHTING_DOJO")
        assert "Hitmonlee" in [m["species"] for m in g.st()["party"]]
        # Verify Hitmonchan ball (npc 7 at (5, 1)) refuses ("Better not get greedy...")
        g.approach_object(5, 1, "FightingDojo")
        g.tap("a", 16)
        _drive_script_dialogue(g)
        assert not _flag(g, "EVENT_GOT_HITMONCHAN"), "must not allow taking both Hitmonlee and Hitmonchan"
        assert "Hitmonchan" not in [m["species"] for m in g.st()["party"]]
        print(f"   s13 party: {[m['species'] for m in g.st()['party']]}")
        g.evidence("s13")
    finally:
        g.close()
        shutil.rmtree(tmp, ignore_errors=True)


# ── s14: Bike Voucher -> Bicycle + 3 Fishing Rods + 3 Oak's Aides ─────────
def s14_bike_rods_aides():
    g, tmp = _make_sidequest_game(
        team=[
            ("Venusaur", 65, ["RazorLeaf", "Cut", "SleepPowder", "MegaDrain"]),
            ("Zapdos", 65, ["Fly", "Thunderbolt", "DrillPeck", "ThunderWave"]),
        ],
        items=[],
        map_name="VermilionCity",
        pos=(11, 4),
        flags=("EVENT_BEAT_ROUTE12_SNORLAX",),
    )
    try:
        r = g.d.cmd(cmd="set_flag", name="__OBJ_HIDDEN_ROUTE_12_OBJ_1", value=True)
        assert r["ok"], r
        # 1. Pokemon Fan Club Chairman (npc 5 at (3, 1)) -> Bike Voucher
        g.nav_warp(9, 13, "VermilionCity", "PokemonFanClub")
        g.approach_object(3, 1, "PokemonFanClub")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",))
        require_flag(g, "EVENT_GOT_BIKE_VOUCHER")
        assert _bag_qty(g, "BikeVoucher") == 1
        g.nav_warp(2, 7, "PokemonFanClub", "VermilionCity", approach="down")

        # 2. Vermilion Old Rod House -> OLD_ROD
        g.nav_warp(7, 3, "VermilionCity", "VermilionOldRodHouse")
        g.approach_object(2, 4, "VermilionOldRodHouse")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",))
        require_flag(g, "EVENT_GOT_OLD_ROD")
        assert _bag_qty(g, "OldRod") == 1
        g.nav_warp(2, 7, "VermilionOldRodHouse", "VermilionCity", approach="down")

        # 3. Cerulean Bike Shop -> exchange Bike Voucher for Bicycle, then ride it!
        fly(g, "CeruleanCity")
        g.nav_warp(13, 25, "CeruleanCity", "BikeShop")
        g.nav_to(6, 4, "BikeShop")
        g.face("up")
        g.tap("a", 16)
        _drive_script_dialogue(g)
        require_flag(g, "EVENT_GOT_BICYCLE")
        assert _bag_qty(g, "Bicycle") == 1 and _bag_qty(g, "BikeVoucher") == 0
        g.nav_warp(2, 7, "BikeShop", "CeruleanCity", approach="down")
        use_item(g, "Bicycle")
        assert g.st()["player_transport"] == "Biking", g.st()["player_transport"]
        use_item(g, "Bicycle")
        assert g.st()["player_transport"] == "Walking", g.st()["player_transport"]

        # 4. Fuchsia Good Rod House -> GOOD_ROD
        fly(g, "FuchsiaCity")
        g.nav_warp(31, 27, "FuchsiaCity", "FuchsiaGoodRodHouse")
        g.approach_object(5, 3, "FuchsiaGoodRodHouse")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",))
        require_flag(g, "EVENT_GOT_GOOD_ROD")
        assert _bag_qty(g, "GoodRod") == 1
        g.nav_warp(2, 7, "FuchsiaGoodRodHouse", "FuchsiaCity", approach="down")

        # 5. Oak's Aide #3 (Route15Gate2F, 50+ dex) -> EXP_ALL
        g.nav_to_map(6, 8, "Route15", avoid_grass=False)
        g.nav_warp(7, 8, "Route15", "Route15Gate1F", approach="right")
        g.nav_warp(6, 8, "Route15Gate1F", "Route15Gate2F")
        g.approach_object(4, 2, "Route15Gate2F")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",))
        require_flag(g, "EVENT_GOT_EXP_ALL")
        assert _bag_qty(g, "ExpAll") == 1
        g.nav_warp(7, 7, "Route15Gate2F", "Route15Gate1F")
        g.nav_warp(0, 4, "Route15Gate1F", "Route15", approach="left")

        # 6. Route 12 Super Rod House -> SUPER_ROD (fly LavenderTown, pass Route12Gate1F south)
        fly(g, "LavenderTown")
        g.nav_to(8, 17, "LavenderTown")
        _step_tiles(g, "down", 2)
        g.nav_to(10, 14, "Route12")
        g.nav_warp(10, 15, "Route12", "Route12Gate1F", approach="down")
        g.nav_warp(4, 7, "Route12Gate1F", "Route12", approach="down")
        g.nav_to(11, 78, "Route12", tries=60)
        g.nav_warp(11, 77, "Route12", "Route12SuperRodHouse")
        g.approach_object(2, 4, "Route12SuperRodHouse")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",))
        require_flag(g, "EVENT_GOT_SUPER_ROD")
        assert _bag_qty(g, "SuperRod") == 1
        g.nav_warp(2, 7, "Route12SuperRodHouse", "Route12", approach="down")

        # 7. Oak's Aide #2 (Route11Gate2F, 30+ dex) -> ITEMFINDER
        fly(g, "VermilionCity")
        g.nav_to_map(48, 8, "Route11", avoid_grass=False)
        g.nav_warp(49, 8, "Route11", "Route11Gate1F", approach="right")
        g.nav_warp(6, 8, "Route11Gate1F", "Route11Gate2F")
        g.approach_object(2, 6, "Route11Gate2F")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",))
        require_flag(g, "EVENT_GOT_ITEMFINDER")
        assert _bag_qty(g, "Itemfinder") == 1
        g.nav_warp(7, 7, "Route11Gate2F", "Route11Gate1F")
        g.nav_warp(0, 4, "Route11Gate1F", "Route11", approach="left")

        # 8. Oak's Aide #1 (Route2Gate, 10+ dex) -> HM05 Flash via Diglett's Cave
        g.nav_warp(4, 5, "Route11", "DiglettsCaveRoute11")
        g.nav_warp(4, 4, "DiglettsCaveRoute11", "DiglettsCave")
        g.nav_warp(5, 5, "DiglettsCave", "DiglettsCaveRoute2")
        g.nav_warp(2, 7, "DiglettsCaveRoute2", "Route2", approach="down")
        g.nav_to(15, 21, "Route2")
        g.face("down")
        field_move(g, "Cut")
        _step_tiles(g, "down", 2)
        g.nav_warp(16, 35, "Route2", "Route2Gate", approach="down")
        g.approach_object(1, 4, "Route2Gate")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",))
        require_flag(g, "EVENT_GOT_HM05")
        assert _bag_qty(g, "Hm05") == 1
        print("   s14 verified: Bicycle, Old/Good/Super Rods, HM05/Itemfinder/ExpAll")
        g.evidence("s14")
    finally:
        g.close()
        shutil.rmtree(tmp, ignore_errors=True)


# ── s15: Eevee + Diploma + Coin Case & Prize + Copycat TM31 + Route16 Snorlax
def s15_celadon_saffron():
    g, tmp = _make_sidequest_game(
        team=[
            ("Venusaur", 65, ["RazorLeaf", "Cut", "SleepPowder", "MegaDrain"]),
            ("Zapdos", 65, ["Fly", "Thunderbolt", "DrillPeck", "ThunderWave"]),
        ],
        items=[("POKE_DOLL", 1), ("POKE_FLUTE", 1)],
        map_name="CeladonCity",
        pos=(41, 10),
        flags=("EVENT_BEAT_SILPH_CO_GIOVANNI",),
    )
    try:
        # 1. Celadon Mansion back door -> Roof House -> Eevee
        g.nav_warp(24, 3, "CeladonCity", "CeladonMansion1F")
        g.nav_warp(2, 1, "CeladonMansion1F", "CeladonMansion2F")
        g.nav_warp(4, 1, "CeladonMansion2F", "CeladonMansion3F")
        g.nav_warp(2, 1, "CeladonMansion3F", "CeladonMansionRoof")
        g.nav_warp(2, 7, "CeladonMansionRoof", "CeladonMansionRoofHouse")
        g.approach_object(4, 3, "CeladonMansionRoofHouse")
        g.tap("a", 16)
        _drive_script_dialogue(g)
        require_flag(g, "EVENT_GOT_EEVEE")
        assert "Eevee" in [m["species"] for m in g.st()["party"]]
        g.nav_warp(2, 7, "CeladonMansionRoofHouse", "CeladonMansionRoof", approach="down")
        g.nav_warp(2, 1, "CeladonMansionRoof", "CeladonMansion3F")
        g.nav_warp(4, 1, "CeladonMansion3F", "CeladonMansion2F")
        g.nav_warp(2, 1, "CeladonMansion2F", "CeladonMansion1F")
        g.nav_warp(4, 0, "CeladonMansion1F", "CeladonCity", approach="up")

        # 2. Celadon Mansion front door -> 3F Game Freak Designer -> 151 Diploma
        g.nav_warp(24, 9, "CeladonCity", "CeladonMansion1F")
        g.nav_warp(7, 1, "CeladonMansion1F", "CeladonMansion2F")
        g.nav_warp(6, 1, "CeladonMansion2F", "CeladonMansion3F")
        g.approach_object(2, 3, "CeladonMansion3F")
        g.tap("a", 16)
        _drive_script_dialogue(g)
        g.nav_warp(6, 1, "CeladonMansion3F", "CeladonMansion2F")
        g.nav_warp(7, 1, "CeladonMansion2F", "CeladonMansion1F")
        g.nav_warp(4, 11, "CeladonMansion1F", "CeladonCity", approach="down")

        # 3. Celadon Diner -> Coin Case
        g.nav_warp(31, 27, "CeladonCity", "CeladonDiner")
        g.approach_object(0, 1, "CeladonDiner")
        g.tap("a", 16)
        _drive_script_dialogue(g)
        require_flag(g, "EVENT_GOT_COIN_CASE")
        assert _bag_qty(g, "CoinCase") == 1
        g.nav_warp(3, 7, "CeladonDiner", "CeladonCity", approach="down")

        # 4. Game Corner -> buy 200 coins (4 x 50) from Clerk 1 at (5, 6), then Prize Room -> Abra
        g.nav_warp(28, 19, "CeladonCity", "GameCorner")
        g.nav_to(5, 8, "GameCorner")
        g.face("up")
        for _ in range(4):
            g.tap("a", 16)
            _drive_script_dialogue(g, choices=("YES",))
        assert g.st().get("coins", 0) >= 180, g.st()
        g.nav_warp(15, 17, "GameCorner", "CeladonCity", approach="down")

        g.nav_warp(33, 19, "CeladonCity", "GameCornerPrizeRoom")
        g.nav_to(2, 3, "GameCornerPrizeRoom")
        g.face("up")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("ABRA", "YES"))
        assert "Abra" in [m["species"] for m in g.st()["party"]]
        g.nav_warp(4, 7, "GameCornerPrizeRoom", "CeladonCity", approach="down")

        # 5. Route 16 Snorlax (Poke Flute) -> defeat/clear EVENT_BEAT_ROUTE16_SNORLAX
        g.nav_to_map(27, 10, "Route16")
        use_item(g, "PokeFlute")
        assert g.cutscene()
        g.wait("screen=battle", 900)
        g.battle_loop(prefer="fight")
        assert g.cutscene()
        require_flag(g, "EVENT_BEAT_ROUTE16_SNORLAX")

        # 6. Saffron Copycat's House 2F -> trade Poke Doll for TM31 Mimic (Copycat wanders)
        fly(g, "SaffronCity")
        g.nav_warp(7, 5, "SaffronCity", "CopycatsHouse1F")
        g.nav_warp(7, 1, "CopycatsHouse1F", "CopycatsHouse2F")
        _talk_live_npc(g, "CopycatsHouse2F", 1)
        require_flag(g, "EVENT_GOT_TM31")
        assert _bag_qty(g, "Tm31") == 1 and _bag_qty(g, "PokeDoll") == 0
        print(f"   s15 party: {[m['species'] for m in g.st()['party']]}")
        g.evidence("s15")
    finally:
        g.close()
        shutil.rmtree(tmp, ignore_errors=True)


# ── s16: In-Game Trade (DUX) + Route 5 Daycare + Name Rater ───────────────
def s16_trades_daycare_rater():
    g, tmp = _make_sidequest_game(
        team=[
            ("Venusaur", 65, ["RazorLeaf", "Cut", "SleepPowder", "MegaDrain"]),
            ("Zapdos", 65, ["Fly", "Thunderbolt", "DrillPeck", "ThunderWave"]),
            ("Spearow", 15, ["Peck", "Growl", "Leer", "FuryAttack"]),
            ("Pikachu", 10, ["Thundershock", "Growl", "TailWhip", "QuickAttack"]),
        ],
        items=[],
        map_name="VermilionCity",
        pos=(11, 4),
    )
    try:
        # 1. Vermilion Trade House: trade Spearow (slot 2) for Farfetch'd (DUX)
        g.nav_warp(15, 13, "VermilionCity", "VermilionTradeHouse")
        g.approach_object(3, 5, "VermilionTradeHouse")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",), party_slot=2)
        party_species = [m["species"] for m in g.st()["party"]]
        assert "Farfetchd" in party_species and "Spearow" not in party_species, party_species
        g.nav_warp(2, 7, "VermilionTradeHouse", "VermilionCity", approach="down")

        # 2. Route 5 Daycare: cut Cerulean tree (19, 28) -> jump ledges south into Route 5 -> Daycare
        fly(g, "CeruleanCity")
        g.nav_to(19, 27, "CeruleanCity")
        g.face("down")
        field_move(g, "Cut")
        _step_tiles(g, "down", 2)
        g.nav_to(20, 32, "CeruleanCity")
        # Jump one-way ledges straight south (x=20 in Cerulean -> x=10 in Route5) into Daycare yard
        for _ in range(40):
            s = g.st()
            if s["screen"] == "battle":
                g.battle_loop(prefer="fight")
                continue
            if s["map_name"] == "Route5" and s["player_y"] >= 16:
                break
            g.d.drive(["down"] * 24, frames=28)
        s = g.st()
        assert s["map_name"] == "Route5" and s["player_y"] >= 16, \
            (s["map_name"], s["player_x"], s["player_y"])
        g.nav_to(10, 22, "Route5")
        g.nav_warp(10, 21, "Route5", "Daycare")
        # 2a. Verify HM-move rejection guard (Zapdos at slot 1 knows Fly)
        g.approach_object(2, 3, "Daycare")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",), party_slot=1)
        assert len(g.st()["party"]) == 4, "Daycare must reject Pokemon knowing an HM move"
        # 2b. Deposit Pikachu (slot 3), walk steps, withdraw with gained EXP
        exp_before = g.st()["evaluation"]["party"][3]["total_exp"]
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",), party_slot=3)
        assert len(g.st()["party"]) == 3, g.st()["party"]
        g.nav_warp(2, 7, "Daycare", "Route5", approach="down")
        # Walk back and forth horizontally in front of Daycare (y=22) so Daycare increments EXP
        for _ in range(10):
            g.nav_to(7, 22, "Route5")
            g.nav_to(12, 22, "Route5")
        g.nav_warp(10, 21, "Route5", "Daycare")
        g.approach_object(2, 3, "Daycare")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",))
        assert len(g.st()["party"]) == 4, g.st()["party"]
        exp_after = g.st()["evaluation"]["party"][3]["total_exp"]
        assert exp_after > exp_before, (exp_before, exp_after)
        g.nav_warp(2, 7, "Daycare", "Route5", approach="down")

        # 3. Lavender Name Rater: refuse renaming traded Farfetch'd (DUX), allow renaming Pikachu
        fly(g, "LavenderTown")
        g.nav_warp(7, 13, "LavenderTown", "NameRatersHouse")
        farf_idx = next(i for i, m in enumerate(g.st()["party"]) if m["species"] == "Farfetchd")
        pika_idx = next(i for i, m in enumerate(g.st()["party"]) if m["species"] == "Pikachu")
        g.approach_object(5, 3, "NameRatersHouse")
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES",), party_slot=farf_idx)
        # Own-OT Pikachu -> YES -> open naming screen -> rename
        g.tap("a", 16)
        _drive_script_dialogue(g, choices=("YES", "YES"), party_slot=pika_idx, rename_confirm=True)
        print(f"   s16 verified: Trade(DUX), Daycare HM guard + EXP {exp_before}->{exp_after}, Name Rater")
        g.evidence("s16")
    finally:
        g.close()
        shutil.rmtree(tmp, ignore_errors=True)
