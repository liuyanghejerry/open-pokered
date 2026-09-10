#!/usr/bin/env python3
"""Construct engine-valid save snapshots for fast game-state setup.

The engine boots from a JSON snapshot (`run --snapshot state.json`, see
`Game(snapshot=...)` in playthrough.py); there is no JSON→.sav
write-back. SaveBuilder produces such snapshots by mutating a canonical
template — a real save exported from a freshly booted game — so every
SaveData field the builder doesn't touch keeps engine-produced values.
Partial snapshots would not deserialize: SaveData's fields carry no
serde defaults.

Everything the protocol's write commands can't set is constructible
here: money, badges, event flags, Pokédex bits, spawn position, and
party members with exact level/moves (stats are computed with the Gen-1
formula, verified field-for-field against the engine's create_pokemon).

Usage (module):
    sb = SaveBuilder()
    sb.party_add("Charizard", 36)
    sb.money(65000)
    sb.flag("EVENT_BEAT_BROCK")
    sb.position("PalletTown", 5, 6)
    sb.write("/tmp/state.json")

Usage (CLI):
    python3 scripts/save_builder.py -o /tmp/state.json \
        --party Charizard:36 --money 65000 \
        --flag EVENT_BEAT_BROCK --item POKE_BALL:20 --map PalletTown:5,6
    python3 scripts/save_builder.py -o /tmp/champion.json --preset champion
"""
import argparse
import copy
import json
import re
import shutil
import subprocess
import sys
import tempfile
from functools import lru_cache
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
from playthrough import BIN, Game, m01_boot, m02_oak_speech  # noqa: E402

DATA = ROOT / "crates" / "pokered-data"

# ── static tables (parsed from the repo's own data sources) ────────────
@lru_cache(maxsize=None)
def species_order():
    """Species names in dex order (Bulbasaur=1..Mew=151) — the same
    order build.rs' SPECIES_ORDER bakes into the Species enum, so
    dex-bit indexes and base-stats rows line up with the engine."""
    text = (DATA / "build.rs").read_text()
    m = re.search(r"const SPECIES_ORDER: &\[&str\] = &\[(.*?)\];", text, re.S)
    if not m:
        raise RuntimeError("SPECIES_ORDER not found in pokered-data/build.rs")
    return re.findall(r'"(\w+)"', m.group(1))


@lru_cache(maxsize=None)
def species_id(name):
    order = species_order()
    if name not in order:
        raise KeyError(f"unknown species {name!r}")
    return order.index(name) + 1


@lru_cache(maxsize=None)
def species_data(name):
    path = DATA / "pokemon" / f"{name}.json"
    if not path.exists():
        raise KeyError(f"no species data at {path}")
    return json.load(open(path))


@lru_cache(maxsize=None)
def move_pp(move):
    if move == "None":
        return 0
    return json.load(open(DATA / "moves" / f"{move}.json"))["pp"]


@lru_cache(maxsize=None)
def map_id(name):
    path = DATA / "maps" / name / "map.json"
    if not path.exists():
        raise KeyError(f"no map data at {path}")
    d = json.loads(path.read_text())
    if d.get("name") != name:
        raise RuntimeError(f"map name mismatch in {path}: {d.get('name')}")
    return d["id"]


@lru_cache(maxsize=None)
def event_flag_bit(name):
    """EventFlag enum values ARE bit indexes (byte = bit >> 3,
    mask = 1 << (bit & 7)) — event_flags.rs is the source of truth."""
    text = (DATA / "src" / "event_flags.rs").read_text()
    m = re.search(rf"^\s*{name} = 0x([0-9A-Fa-f]+),", text, re.M)
    if not m:
        raise KeyError(f"unknown event flag {name!r} (event_flags.rs)")
    return int(m.group(1), 16)


def item_id(name):
    """Numeric bag id from item_list.json order (NoItem=0 → offset 1).
    Self-checked against the ids the save-editor skill documents."""
    order = json.load(open(DATA / "data" / "items" / "item_list.json"))["items"]
    if name not in order:
        raise KeyError(f"unknown item {name!r} (item_list.json)")
    got = order.index(name) + 1
    assert order.index("MasterBall") + 1 == 1 and order.index("PokeBall") + 1 == 4
    return got


# ── the champion preset ─────────────────────────────────────────────────
# Story-completion flags for a finished first playthrough. Sources: every
# `@if (!getFlag(...))` gate across maps/*/script.scene (a negated gate is
# the "story not done → intercept/block" branch), plus positively-tested
# story beats (rival fights, follower hand-offs). Deliberately NOT set:
#   - EVENT_LANCES_ROOM_LOCK_DOOR — @load replaces the door tiles with a
#     closed block when set; post-Lance the door must stay open
#   - EVENT_IN_PURIFIED_ZONE — dedupe latch for Pokémon Tower's heal
#     zone; setting it would disable the heal
#   - EVENT_DAISY_WALKING — cosmetic NPC position
#   - EVENT_BEAT_ARTICUNO/ZAPDOS/MOLTRES/MEWTWO, POWER_PLANT_VOLTORB_* —
#     optional legendaries stay catchable post-game
#   - overworld GOT_TM*/GOT_*_POTION/... item pickups — 通关 ≠ 100%,
#     pickups remain collectable (story gifts like SS_TICKET are set)
#   - EVENT_BEAT_SS_ANNE_RIVAL / EVENT_GOT_EEVEE / EVENT_GOT_LAPRAS /
#     EVENT_GAVE_SAFFRON_GUARDS_DRINK — runtime sidecar flags, NOT part
#     of the SRAM bitset a snapshot can carry; their gates (gift
#     re-offers, the Saffron gate guards' drink check) stay in the
#     "not done" state until set live via the set_flag debug command
CHAMPION_FLAGS = [
    # Pallet / Oak intro chain — FOLLOWED_OAK_INTO_LAB is what stops the
    # "Hey! Wait! Don't go out!" grass interception at the north exit
    "EVENT_OAK_ASKED_TO_CHOOSE_MON", "EVENT_GOT_STARTER",
    "EVENT_BATTLED_RIVAL_IN_OAKS_LAB", "EVENT_FOLLOWED_OAK_INTO_LAB",
    "EVENT_FOLLOWED_OAK_INTO_LAB_2", "EVENT_GOT_POKEDEX",
    "EVENT_GOT_POKEBALLS_FROM_OAK", "EVENT_GOT_TOWN_MAP",
    "EVENT_ENTERED_BLUES_HOUSE",
    # Rival
    "EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE", "EVENT_BEAT_CERULEAN_RIVAL",
    "EVENT_BEAT_POKEMON_TOWER_RIVAL", "EVENT_BEAT_SILPH_CO_RIVAL",
    # SS Anne
    "EVENT_GOT_SS_TICKET", "EVENT_SS_ANNE_LEFT", "EVENT_WALKED_OUT_OF_DOCK",
    # Mt Moon / Cerulean / Rocket hideout
    "EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD", "EVENT_GOT_HELIX_FOSSIL",
    "EVENT_GAVE_FOSSIL_TO_LAB", "EVENT_BEAT_CERULEAN_ROCKET_THIEF",
    "EVENT_ENTERED_ROCKET_HIDEOUT", "EVENT_ROCKET_DROPPED_LIFT_KEY",
    # Gym leaders + their badge-check gates
    "EVENT_BEAT_BROCK", "EVENT_BEAT_MISTY", "EVENT_BEAT_LT_SURGE",
    "EVENT_BEAT_ERIKA", "EVENT_BEAT_KOGA", "EVENT_BEAT_SABRINA",
    "EVENT_BEAT_BLAINE", "EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI",
    "EVENT_PASSED_CASCADEBADGE_CHECK", "EVENT_PASSED_THUNDERBADGE_CHECK",
    "EVENT_PASSED_RAINBOWBADGE_CHECK", "EVENT_PASSED_SOULBADGE_CHECK",
    "EVENT_PASSED_MARSHBADGE_CHECK", "EVENT_PASSED_VOLCANOBADGE_CHECK",
    "EVENT_PASSED_EARTHBADGE_CHECK",
    # Routes / towns
    "EVENT_FIGHT_ROUTE16_SNORLAX", "EVENT_FIGHT_ROUTE12_SNORLAX",
    "EVENT_BOUGHT_MUSEUM_TICKET",
    # Pokémon Tower / Silph / Mansion
    "EVENT_BEAT_GHOST_MAROWAK", "EVENT_RESCUED_MR_FUJI",
    "EVENT_BEAT_SILPH_CO_GIOVANNI",
    "EVENT_SILPH_CO_RECEPTIONIST_AT_DESK", "EVENT_MANSION_SWITCH_ON",
    # Silph card-key doors — leave the building traversable post-game
    "EVENT_SILPH_CO_2_UNLOCKED_DOOR1", "EVENT_SILPH_CO_2_UNLOCKED_DOOR2",
    "EVENT_SILPH_CO_3_UNLOCKED_DOOR1", "EVENT_SILPH_CO_3_UNLOCKED_DOOR2",
    "EVENT_SILPH_CO_4_UNLOCKED_DOOR1", "EVENT_SILPH_CO_4_UNLOCKED_DOOR2",
    "EVENT_SILPH_CO_5_UNLOCKED_DOOR1", "EVENT_SILPH_CO_5_UNLOCKED_DOOR2",
    "EVENT_SILPH_CO_5_UNLOCKED_DOOR3", "EVENT_SILPH_CO_6_UNLOCKED_DOOR",
    "EVENT_SILPH_CO_7_UNLOCKED_DOOR1", "EVENT_SILPH_CO_7_UNLOCKED_DOOR2",
    "EVENT_SILPH_CO_7_UNLOCKED_DOOR3", "EVENT_SILPH_CO_8_UNLOCKED_DOOR",
    "EVENT_SILPH_CO_9_UNLOCKED_DOOR1", "EVENT_SILPH_CO_9_UNLOCKED_DOOR2",
    "EVENT_SILPH_CO_9_UNLOCKED_DOOR3", "EVENT_SILPH_CO_9_UNLOCKED_DOOR4",
    "EVENT_SILPH_CO_10_UNLOCKED_DOOR", "EVENT_SILPH_CO_11_UNLOCKED_DOOR",
    # Seafoam / Victory Road passage boulders
    "EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE", "EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE",
    "EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE", "EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE",
    "EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1",
    "EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2",
    "EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH1",
    # Elite Four + champion (BEAT_LANCE is separate from the room trainer
    # flag; AUTOWALKED skips the entry cutscenes on re-entry)
    "EVENT_BEAT_LORELEIS_ROOM_TRAINER_0", "EVENT_AUTOWALKED_INTO_LORELEIS_ROOM",
    "EVENT_BEAT_BRUNOS_ROOM_TRAINER_0", "EVENT_AUTOWALKED_INTO_BRUNOS_ROOM",
    "EVENT_BEAT_AGATHAS_ROOM_TRAINER_0", "EVENT_AUTOWALKED_INTO_AGATHAS_ROOM",
    "EVENT_BEAT_LANCES_ROOM_TRAINER_0", "EVENT_BEAT_LANCE",
    "EVENT_BEAT_CHAMPION_RIVAL", "EVENT_HALL_OF_FAME_DEX_RATING",
]

CHAMPION_TEAM = [
    ("Charizard", 62, ["Flamethrower", "Slash", "Earthquake", "FireBlast"]),
    ("Lapras", 58, ["Surf", "IceBeam", "BodySlam", "Thunderbolt"]),
    ("Snorlax", 56, ["BodySlam", "Earthquake", "HyperBeam", "Rest"]),
    ("Jolteon", 57, ["Thunderbolt", "PinMissile", "ThunderWave", "DoubleKick"]),
    ("Dugtrio", 53, ["Earthquake", "Slash", "Dig", "SandAttack"]),
    ("Nidoking", 55, ["Earthquake", "IceBeam", "Thunderbolt", "HornDrill"]),
]

# A deliberately over-provisioned, but story-accurate, start state for
# checkpoint exploration.  It represents the state immediately after m26:
# the player has reached Fuchsia, but Koga and all later milestones are still
# untouched.  The high-level starter has the early field move needed by the
# later canonical route, while later story gifts/captures remain available.
EXPLORATION_M26_FLAGS = [
    "EVENT_OAK_ASKED_TO_CHOOSE_MON", "EVENT_GOT_STARTER",
    "EVENT_BATTLED_RIVAL_IN_OAKS_LAB", "EVENT_FOLLOWED_OAK_INTO_LAB",
    "EVENT_FOLLOWED_OAK_INTO_LAB_2", "EVENT_GOT_POKEDEX",
    "EVENT_GOT_POKEBALLS_FROM_OAK", "EVENT_GOT_TOWN_MAP",
    "EVENT_ENTERED_BLUES_HOUSE", "EVENT_OAK_GOT_PARCEL",
    "EVENT_OAK_APPEARED_IN_PALLET", "EVENT_GOT_OAKS_PARCEL",
    "EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE", "EVENT_BEAT_BROCK",
    "EVENT_BEAT_MISTY", "EVENT_BEAT_CERULEAN_RIVAL",
    "EVENT_BEAT_CERULEAN_ROCKET_THIEF", "EVENT_BEAT_LT_SURGE",
    "EVENT_BEAT_ERIKA", "EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD",
    "EVENT_GOT_HELIX_FOSSIL", "EVENT_GOT_SS_TICKET",
    "EVENT_SS_ANNE_LEFT", "EVENT_STARTED_WALKING_OUT_OF_DOCK",
    "EVENT_WALKED_OUT_OF_DOCK", "EVENT_RUBBED_CAPTAINS_BACK",
    "EVENT_ENTERED_ROCKET_HIDEOUT", "EVENT_FOUND_ROCKET_HIDEOUT",
    "EVENT_ROCKET_DROPPED_LIFT_KEY", "EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED",
    "EVENT_BEAT_ROCKET_HIDEOUT_GIOVANNI", "EVENT_BEAT_GHOST_MAROWAK",
    "EVENT_RESCUED_MR_FUJI", "EVENT_RESCUED_MR_FUJI_2",
    "EVENT_GOT_POKE_FLUTE", "EVENT_FIGHT_ROUTE12_SNORLAX",
    "EVENT_BEAT_ROUTE12_SNORLAX", "EVENT_MET_BILL", "EVENT_MET_BILL_2",
    "EVENT_USED_CELL_SEPARATOR_ON_BILL", "EVENT_BILL_SAID_USE_CELL_SEPARATOR",
    "EVENT_GOT_NUGGET", "EVENT_GOT_HM01", "EVENT_GOT_TM11",
    "EVENT_GOT_TM21", "EVENT_GOT_TM24", "EVENT_GOT_TM34",
    "EVENT_BEAT_CELADON_GYM_TRAINER_0", "EVENT_BEAT_CELADON_GYM_TRAINER_1",
    "EVENT_BEAT_CELADON_GYM_TRAINER_2", "EVENT_BEAT_CELADON_GYM_TRAINER_3",
    "EVENT_BEAT_CELADON_GYM_TRAINER_4", "EVENT_BEAT_CELADON_GYM_TRAINER_5",
    "EVENT_BEAT_CELADON_GYM_TRAINER_6",
]

EXPLORATION_M26_TEAM = [
    # Keep later story gifts/captures at their canonical party indexes:
    # m32 adds Lapras at index 1 and m36 catches Zapdos at index 2.
    ("Venusaur", 50, ["RazorLeaf", "Cut", "Tackle", "SleepPowder"]),
]

# Runtime object state is separate from the persistent event bitset. The m26
# snapshot starts after the Route 12 Snorlax fight, so the exploration driver
# must re-apply this sidecar flag after booting a copied snapshot.
EXPLORATION_M26_RUNTIME_FLAGS = [
    "__OBJ_HIDDEN_ROUTE_12_OBJ_1",
]

# Towns visited on the canonical route through m26. Keeping this bitset
# accurate makes FLY available without unlocking future destinations.
EXPLORATION_M26_VISITED_TOWNS = [
    "PalletTown", "ViridianCity", "PewterCity", "CeruleanCity",
    "LavenderTown", "VermilionCity", "CeladonCity", "FuchsiaCity",
]

EXPLORATION_M26_ITEMS = [
    # Four free slots remain for Master Ball, HM02, Secret Key, and a later
    # optional reward encountered during exploration.
    "POKE_BALL", "FULL_RESTORE", "REVIVE", "HM01", "POKE_FLUTE",
    "TOWN_MAP", "SS_TICKET", "HELIX_FOSSIL", "TM21", "TM24", "TM28",
    "TM34",
]


# ── Gen-1 stat math (verified against create_pokemon output) ───────────
def _dv_pair(dv_bytes):
    """dv_bytes = [atk_def, spd_spc]: high nybble = Atk/Spd IV, low =
    Def/Spc IV; HP IV is derived from the low bits (engine/battle/state.rs)."""
    atk, dfn = dv_bytes[0] >> 4, dv_bytes[0] & 0xF
    spd, spc = dv_bytes[1] >> 4, dv_bytes[1] & 0xF
    hp = ((atk & 1) << 3) | ((dfn & 1) << 2) | ((spd & 1) << 1) | (spc & 1)
    return hp, atk, dfn, spd, spc


def _stat(base, dv, level):
    return ((base + dv) * 2) * level // 100 + 5


def compute_stats(species, level, dv_bytes=(0x9A, 0x78)):
    """Gen-1 stats with zero stat experience. Matches the engine's
    create_pokemon (default DVs 0x9A,0x78 → Bulbasaur L5 = 20/10/10/10/12)."""
    bs = species_data(species)["baseStats"]
    hp_dv, atk, dfn, spd, spc = _dv_pair(dv_bytes)
    hp = ((bs["hp"] + hp_dv) * 2) * level // 100 + level + 10
    return {
        "max_hp": hp, "hp": hp,
        "attack": _stat(bs["attack"], atk, level),
        "defense": _stat(bs["defense"], dfn, level),
        "speed": _stat(bs["speed"], spd, level),
        "special": _stat(bs["special"], spc, level),
    }


def exp_for_level(level, growth_rate):
    """Total experience at `level` for the species' growth rate."""
    n3 = level ** 3
    if growth_rate == "MediumSlow":
        return (6 * n3) // 5 - 15 * level * level + 100 * level - 140
    if growth_rate == "MediumFast":
        return n3
    if growth_rate == "Fast":
        return (4 * n3) // 5
    if growth_rate == "Slow":
        return (5 * n3) // 4
    return n3


def make_mon(species, level, moves=None, dv_bytes=(0x9A, 0x78)):
    """A snapshot-ready party entry, mirroring the engine's give_pokemon
    output (initial moveset unless `moves` overrides; live PP)."""
    sd = species_data(species)
    stats = compute_stats(species, level, dv_bytes)
    if moves is None:
        moves = sd["initialMoves"]
    moves = list(moves[:4]) + ["None"] * (4 - min(4, len(moves)))
    return {
        "species": species,
        "level": level,
        **stats,
        "type1": sd["type1"],
        "type2": sd["type2"],
        "moves": moves,
        "pp": [move_pp(m) for m in moves],
        "pp_ups": [0, 0, 0, 0],
        "status": "None",
        "dv_bytes": list(dv_bytes),
        "stat_exp": [0, 0, 0, 0, 0],
        "total_exp": exp_for_level(level, sd["growthRate"]),
        "is_traded": False,
        "ot_id": 0,
    }


# ── the builder ─────────────────────────────────────────────────────────
def fresh_template():
    """Export a canonical snapshot from a freshly booted game (~4s): the
    only source for engine-valid values of every SaveData field."""
    tmp = Path(tempfile.mkdtemp(prefix="pokered-savetpl-"))
    try:
        sav = tmp / "template.sav"
        g = Game(save_path=sav)
        try:
            m01_boot(g)
            m02_oak_speech(g)
            r = g.d.cmd(cmd="save")
            assert r["ok"], r
        finally:
            g.close()
        out = tmp / "template.json"
        r = subprocess.run([str(BIN), "import-snapshot", "--input", str(sav),
                            "-o", str(out)], capture_output=True, timeout=60)
        assert r.returncode == 0, r.stderr.decode()[-400:]
        return json.load(open(out))
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


class SaveBuilder:
    """Mutates a canonical snapshot template. Chaining-friendly."""

    def __init__(self, template=None):
        # One template export shared by all builders of a process: every
        # builder deep-copies it, so mutations never leak between saves.
        SaveBuilder._tpl = SaveBuilder._tpl if hasattr(SaveBuilder, "_tpl") \
            else fresh_template()
        self.data = copy.deepcopy(SaveBuilder._tpl)
        self.template = SaveBuilder._tpl

    # ── party ──
    def party_add(self, species, level, moves=None, dv_bytes=(0x9A, 0x78)):
        if len(self.data["party"]) >= 6:
            raise AssertionError("party already has 6 members")
        self.data["party"].append(
            make_mon(species, level, moves, dv_bytes))
        return self

    # ── world state ──
    def money(self, amount):
        self.data["game_data"]["player_money"] = amount
        return self

    def badges(self, value):
        """Bitfield: bit0=Boulder .. bit7=Earth; or a name dict
        like {"Boulder": True, "Cascade": True}."""
        if isinstance(value, dict):
            order = ["Boulder", "Cascade", "Thunder", "Rainbow",
                     "Soul", "Marsh", "Volcano", "Earth"]
            bits = 0
            for i, badge in enumerate(order):
                if value.get(badge):
                    bits |= 1 << i
            value = bits
        self.data["game_data"]["obtained_badges"] = value
        return self

    def give_item(self, item, qty):
        """`item` is the Debug name (PokeBall) or the const (POKE_BALL)."""
        name = "".join(w.capitalize()
                       for w in item.strip().upper().replace(" ", "_").split("_"))
        items = self.data["game_data"]["bag"]["items"]
        for entry in items:
            if entry[0] == name:
                entry[1] += qty
                break
        else:
            if len(items) >= 20:
                raise ValueError("bag has no free item slots")
            items.append([name, qty])
        return self

    def visited_towns(self, towns):
        """Mark the named city maps as visited for the FLY destination list."""
        flags = bytearray(self.data["game_data"]["town_visited_flags"])
        for town in towns:
            town_id = map_id(town)
            if town_id >= 0x0B:
                raise ValueError(f"{town!r} is not a town map")
            flags[town_id >> 3] |= 1 << (town_id & 7)
        self.data["game_data"]["town_visited_flags"] = list(flags)
        return self

    def last_map(self, map_name):
        """Set the saved outside map used by LAST_MAP exit warps."""
        self.data["game_data"]["last_map"] = map_id(map_name)
        return self

    def flag(self, name, value=True):
        bit = event_flag_bit(name)
        byte, mask = bit >> 3, 1 << (bit & 7)
        flags = bytearray(self.data["game_data"]["event_flags"])
        if value:
            flags[byte] |= mask
        else:
            flags[byte] &= ~mask & 0xFF
        self.data["game_data"]["event_flags"] = list(flags)
        return self

    def position(self, map_name, x, y):
        self.data["game_data"]["position"] = {
            "map_id": map_id(map_name), "x": x, "y": y,
            "x_block": x & 1, "y_block": y & 1,
        }
        return self

    def dex(self, species, seen=True, owned=True):
        """Flip the species' bits in the Pokédex seen/owned bitsets
        (19 bytes; bit n of the flat bitstream = dex id n)."""
        sid = species_id(species)
        byte, mask = sid >> 3, 1 << (sid & 7)
        dex = self.data["game_data"]["pokedex"]
        for key, want in (("seen", seen), ("owned", owned)):
            bits = bytearray(dex[key])
            if want:
                bits[byte] |= mask
            else:
                bits[byte] &= ~mask & 0xFF
            dex[key] = list(bits)
        return self

    def dex_complete(self):
        """Fill the whole dex (151/151 seen+owned; the trailing bits of
        the 19-byte bitsets beyond dex 151 stay zero)."""
        dex = self.data["game_data"]["pokedex"]
        for key in ("seen", "owned"):
            bits = bytearray(dex[key])
            for sid in range(1, 152):
                bits[sid >> 3] |= 1 << (sid & 7)
            dex[key] = list(bits)
        return self

    # ── presets ──
    def champion(self, team=CHAMPION_TEAM):
        """A finished first playthrough: champion team, all 8 badges,
        story-complete flags (see CHAMPION_FLAGS), full dex, home spawn.
        Legendaries and overworld pickups are deliberately left for
        post-game play."""
        for sp, lv, mv in team:
            self.party_add(sp, lv, moves=mv)
        self.money(483700)
        self.badges(0xFF)
        for f in CHAMPION_FLAGS:
            self.flag(f)
        self.dex_complete()
        self.position("RedsHouse2F", 3, 6)
        self.data["game_data"]["play_time"] = {
            "hours": 39, "minutes": 24, "seconds": 17,
            "frames": 0, "maxed": False}
        self.data["game_data"]["player_starter"] = 4   # Charmander
        self.data["game_data"]["rival_starter"] = 7    # Squirtle
        return self

    def exploration(self, milestone):
        """Build a fast, independent checkpoint-exploration start state.

        Unlike ``champion()``, this keeps the story just after the requested
        milestone and leaves later gates, gifts, and optional pickups live.
        The first supported state is m26 because it is the useful hand-off
        point for the long late-game exploration tail.
        """
        if milestone != "m26":
            raise ValueError(f"unsupported exploration milestone {milestone!r}")
        for species, level, moves in EXPLORATION_M26_TEAM:
            self.party_add(species, level, moves=moves)
        self.money(999999)
        self.badges(0x0F)  # Boulder, Cascade, Thunder, Rainbow
        self.visited_towns(EXPLORATION_M26_VISITED_TOWNS)
        for flag in EXPLORATION_M26_FLAGS:
            self.flag(flag)
        for item in EXPLORATION_M26_ITEMS:
            self.give_item(item, 99 if item in {
                "POKE_BALL", "GREAT_BALL", "ULTRA_BALL", "FULL_RESTORE", "REVIVE"
            } else 1)
        self.position("FuchsiaCity", 19, 27)
        self.last_map("FuchsiaCity")
        self.data["game_data"]["player_starter"] = species_id("Bulbasaur")
        self.data["game_data"]["rival_starter"] = species_id("Charmander")
        self.data["game_data"]["play_time"] = {
            "hours": 18, "minutes": 0, "seconds": 0, "frames": 0,
            "maxed": False,
        }
        return self

    def write(self, path):
        Path(path).write_text(json.dumps(self.data))
        return path


# ── CLI ─────────────────────────────────────────────────────────────────
def main():
    ap = argparse.ArgumentParser(
        description="Construct a bootable game snapshot JSON.")
    ap.add_argument("-o", "--out", required=True, help="output .json path")
    ap.add_argument("--preset", choices=["champion", "m26-exploration"],
                    help="start from a built-in preset (champion = story "
                         "complete; m26-exploration = fast late-game "
                         "checkpoint hand-off)")
    ap.add_argument("--party", action="append", default=[],
                    metavar="SPECIES:LEVEL",
                    help="add a party member (repeatable)")
    ap.add_argument("--move", dest="moves", action="append", default=[],
                    metavar="MOVE",
                    help="moveset for the NEXT --party member")
    ap.add_argument("--money", type=int, default=None)
    ap.add_argument("--badges", type=lambda s: int(s, 0), default=None,
                    help="badge bitfield (e.g. 0x0F)")
    ap.add_argument("--item", action="append", default=[],
                    metavar="ITEM:QTY", help="bag item (repeatable)")
    ap.add_argument("--flag", action="append", default=[],
                    metavar="EVENT_*", help="set an event flag (repeatable)")
    ap.add_argument("--map", metavar="MAP:X,Y", default=None,
                    help="spawn position")
    args = ap.parse_args()

    sb = SaveBuilder()
    if args.preset == "champion":
        sb.champion()
    elif args.preset == "m26-exploration":
        sb.exploration("m26")
    pending_moves = None
    for spec in args.party:
        species, _, level = spec.partition(":")
        sb.party_add(species, int(level),
                     moves=pending_moves if pending_moves else None)
        pending_moves = None
    for mv in args.moves:
        pending_moves = (pending_moves or []) + [mv]
    if args.money is not None:
        sb.money(args.money)
    if args.badges is not None:
        sb.badges(args.badges)
    for spec in args.item:
        item, _, qty = spec.partition(":")
        sb.give_item(item, int(qty or 1))
    for name in args.flag:
        sb.flag(name)
    if args.map:
        where, _, xy = args.map.partition(":")
        x, _, y = xy.partition(",")
        sb.position(where, int(x), int(y))
    sb.write(args.out)
    party = [(m["species"], m["level"]) for m in sb.data["party"]]
    print(f"wrote {args.out}: party={party} money="
          f"{sb.data['game_data']['player_money']} "
          f"bag={sb.data['game_data']['bag']['items']}")


if __name__ == "__main__":
    main()
