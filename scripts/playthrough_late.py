"""Post-Brock milestones, driven through the same real-input Game API.

No state seeding: every badge, item and level must be earned in game.
"""
import json
import re
from functools import lru_cache
from pathlib import Path

DATA = Path(__file__).resolve().parent.parent / "crates/pokered-data"


@lru_cache(None)
def species_data(name):
    return json.loads((DATA / "pokemon" / f"{name}.json").read_text())


@lru_cache(None)
def move_data(name):
    return json.loads((DATA / "moves" / f"{name}.json").read_text())


@lru_cache(None)
def type_chart():
    source = (DATA / "src/type_chart.rs").read_text()
    factors = {"SuperEffective": 2, "NotVeryEffective": 0.5, "NoEffect": 0}
    return {(a, d): factors[e] for a, d, e in re.findall(
        r"attacker: PokemonType::(\w+),\s*defender: PokemonType::(\w+),\s*effectiveness: Effectiveness::(\w+)", source)}


def damage_slot(moves, state):
    """Choose with public species/type data and the observed live opponent.

    This is a strategy heuristic, not a reimplementation of damage math.
    The old fixed VineWhip-first order wastes all 10 PP against Oddish and
    Zubat, leaving the Mt Moon solo starter unable to finish the crossing.
    """
    live = state.get("battle_live")
    if not live:
        return None
    player = species_data(live["player"]["species"])
    enemy = species_data(live["enemy"]["species"])
    ranked = []
    special = {"Fire", "Water", "Grass", "Electric", "Psychic", "Ice", "Dragon"}
    for i, slot in enumerate(moves):
        if slot["pp"] <= 0 or slot["disabled"]:
            continue
        move = move_data(slot["move"])
        power = move["power"]
        if power <= 0:
            continue
        typ = move["type"]
        score = power * move["accuracy"] / 100
        if slot["move"] in {"RazorLeaf", "Slash", "Crabhammer", "KarateChop"}:
            score *= 1 + min(player["baseStats"]["speed"] * 8, 255) / 256
        if slot["move"] in {"MegaDrain", "Absorb"} and live["player"]["hp"] < live["player"]["max_hp"] * 0.65:
            score *= 2.5  # Prefer a damaging recovery move when hurt.
        if typ in {player["type1"], player["type2"]}:
            score *= 1.5
        for defense in {enemy["type1"], enemy["type2"]}:
            score *= type_chart().get((typ, defense), 1)
        attack = "special" if typ in special else "attack"
        defense = "special" if typ in special else "defense"
        score *= player["baseStats"][attack] / enemy["baseStats"][defense]
        ranked.append((score, slot["pp"], -i))
    return -max(ranked)[2] if ranked else None


def battle_party_target(state):
    party = state["battle_live"]["player_party"]
    enemy = species_data(state["battle_live"]["enemy"]["species"])
    preferred = "Venusaur" if "Ground" in {enemy["type1"], enemy["type2"]} else "Zapdos"
    return next((i for i, mon in enumerate(party) if mon["species"] == preferred and mon["hp"] > 0),
                max((i for i, mon in enumerate(party) if mon["hp"] > 0), key=lambda i: party[i]["level"]))


def battle_medicine(state):
    live = state["battle_live"]["player"]
    bag = {v["item"]: v["qty"] for v in state["battle_inventory"]}
    hurt = live["hp"] < live["max_hp"] * 0.75
    status = live["status"] != "None"
    if status and bag.get("FullRestore"):
        return "FullRestore"
    if hurt:
        return next((item for item in ["HyperPotion", "FullRestore", "MaxPotion"] if bag.get(item)), None)
    return None


def battle_recovery_plan(state):
    party = state["battle_live"]["player_party"]
    active = next(i for i, mon in enumerate(party) if mon["species"] == state["battle_live"]["player"]["species"])
    medicine = battle_medicine(state)
    if medicine:
        return medicine, active
    enemy = species_data(state["battle_live"]["enemy"]["species"])
    preferred = "Venusaur" if "Ground" in {enemy["type1"], enemy["type2"]} else "Zapdos"
    target = next((i for i, mon in enumerate(party) if mon["species"] == preferred), active)
    bag = {v["item"]: v["qty"] for v in state["battle_inventory"]}
    if party[target]["hp"] == 0 and bag.get("Revive"):
        return "Revive", target
    if target != active and party[target]["hp"] < party[target]["max_hp"] * 0.75:
        medicine = next((item for item in ["HyperPotion", "FullRestore", "MaxPotion"] if bag.get(item)), None)
        if medicine:
            return medicine, target
    return None


def learn_move(g, state):
    """Answer full-moveset prompts deliberately; default NO + repeated A
    cycles Ask/GiveUp forever (first exposed by Lv22 Poisonpowder).
    """
    phase = state["battle_phase"]
    name = re.search(r"move_id: (\w+)", phase)[1]
    if phase.startswith("LearnMoveAsk"):
        if move_data(name)["power"] > 0:
            g.tap("up", 8)  # Default NO -> YES.
            g.tap("a", 8)
        else:
            g.tap("b", 8)
    elif phase.startswith("LearnMoveGiveUpConfirm"):
        g.tap("up", 8)  # Default NO -> YES, abandon the status move.
        g.tap("a", 8)
    else:
        idx = int(re.search(r"party_index: (\d+)", phase)[1])
        cursor = int(re.search(r"cursor: (\d+)", phase)[1])
        moves = state["party"][idx]["moves"]
        replaceable = [(move_data(m)["power"], i) for i, m in enumerate(moves)
                       if m not in {"Cut", "Fly", "Surf", "Strength", "Flash"}]
        target = min(replaceable)[1]
        if cursor != target:
            g.tap("down", 8)
        else:
            g.tap("a", 8)


def require_flag(g, name):
    flags = g.d.cmd(cmd="get_flags")["data"]
    assert flags.get(name), f"missing story flag {name}"


def open_start(g, entry):
    g.tap("start", 12)
    for _ in range(20):
        state = g.st()
        menu = state.get("field_menu")
        if menu is None and state["screen"] == "overworld":
            # A warp's final input lock can consume the first START tap.
            g.step(20)
            g.tap("start", 12)
            continue
        assert menu and menu["kind"] == "start", menu
        if menu["items"][menu["cursor"]] == entry:
            g.tap("a", 12)
            return
        g.tap("down", 8)
    raise RuntimeError(f"start-menu entry not found: {entry}")


def use_item(g, name, party_index=None, forget=None):
    open_start(g, "Item")
    for _ in range(25):
        menu = g.st()["field_menu"]
        assert menu and menu["kind"] == "bag", menu
        target = next(i for i, item in enumerate(menu["items"]) if item["item"] == name)
        if menu["cursor"] == target:
            break
        g.tap("down", 8)
    else:
        raise RuntimeError(f"bag item not selected: {name}")
    g.tap("a", 8)  # USE / TOSS / CANCEL, default USE.
    g.tap("a", 12)
    if party_index is not None:
        for _ in range(8):
            menu = g.st()["field_menu"]
            assert menu and menu["kind"] == "party", menu
            if menu["cursor"] == party_index:
                g.tap("a", 12)
                break
            g.tap("down", 8)
        if forget is not None:
            for _ in range(8):
                menu = g.st()["field_menu"]
                if menu is None:
                    break  # A free slot did not require replacement.
                assert menu["phase"].startswith("ChooseMove"), menu
                target = menu["known_moves"].index(forget)
                cur = int(re.search(r"cursor: (\d+)", menu["phase"])[1])
                if cur == target:
                    g.tap("a", 12)
                    break
                g.tap("down", 8)
    assert g.cutscene()


def field_move(g, name, party_index=0):
    open_start(g, "Pokemon")
    for _ in range(8):
        menu = g.st()["field_menu"]
        assert menu and menu["kind"] == "party", menu
        if menu["cursor"] == party_index:
            break
        g.tap("down", 8)
    g.tap("a", 8)
    for _ in range(12):
        menu = g.st()["field_menu"]
        target = menu["field_moves"].index(name)
        cur = int(re.search(r"cursor: (\d+)", menu["phase"])[1])
        if cur == target:
            g.tap("a", 12)
            if name != "Fly":
                assert g.cutscene()
            return
        g.tap("down", 8)
    raise RuntimeError(f"field move not selected: {name}")


def m11_mt_moon_entrance(g):
    g.nav_warp(4, 13, "PewterGym", "PewterCity", approach="down")
    g.heal_pokecenter((13, 25), "PewterCity", "PewterPokecenter")
    g.nav_to_map(11, 6, "Route4")
    g.heal_pokecenter((11, 5), "Route4", "MtMoonPokecenter")
    s = g.evidence("m11")
    assert s["map_name"] == "Route4"


def m12_mt_moon(g):
    for attempt in range(3):
        try:
            return cross_mt_moon(g)
        except (AssertionError, RuntimeError):
            s = g.st()
            if (attempt == 2 or s["map_name"] != "Route4"
                    or (s["player_x"], s["player_y"]) != (11, 6)
                    or s.get("battle_live", {}).get("player", {}).get("hp") != 0):
                raise
            # Real blackout recovery preserves XP and defeated trainers.
            # Do not reload the checkpoint to reroll individual battles.
            print(f"[m12] blackout {attempt + 1}: healed at Mt Moon, retrying")


def cross_mt_moon(g):
    g.nav_warp(18, 5, "Route4", "MtMoon1F")
    g.nav_warp(5, 5, "MtMoon1F", "MtMoonB1F")
    # The B1F ladder pockets are disconnected; the northwest 1F ladder
    # connects to (21,17), not the tempting (17,11) side pocket.
    g.nav_warp(21, 17, "MtMoonB1F", "MtMoonB2F")
    g.nav_to(13, 8, "MtMoonB2F")
    assert g.cutscene()
    if g.st()["screen"] == "battle":
        g.battle_loop()
        assert g.cutscene()
    require_flag(g, "EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD")
    g.nav_to(13, 7, "MtMoonB2F")
    g.face("up")
    g.tap("a", 20)
    g.dialogue_then_choice()
    g.choose("YES")
    assert g.cutscene()
    require_flag(g, "EVENT_GOT_HELIX_FOSSIL")
    g.nav_warp(5, 7, "MtMoonB2F", "MtMoonB1F")
    g.nav_warp(27, 3, "MtMoonB1F", "Route4")
    # Route4 requires an east-facing jump before the south-facing jump.
    # Jumping south beside the cave strands us west of the rock column.
    g.nav_to(44, 8, "Route4")
    g.d.drive(["right"] * 24, frames=40)
    g.nav_to(64, 8, "Route4")
    g.d.drive(["down"] * 24, frames=40)
    assert g.pos()[0] == "Route4" and g.pos()[2] >= 10, g.pos()
    g.nav_to_map(19, 18, "CeruleanCity")
    g.heal_pokecenter((19, 17), "CeruleanCity", "CeruleanPokecenter")
    s = g.evidence("m12")
    assert s["map_name"] == "CeruleanCity"


def challenge(g, map_name, x, y, direction, flag):
    """Talk to a story trainer and require the actual victory reward."""
    g.nav_to(x, y, map_name)
    g.face(direction)
    g.tap("a", 20)
    assert g.cutscene()
    g.wait("screen=battle", 900)
    g.battle_loop()
    assert g.cutscene()
    require_flag(g, flag)


def m13_misty(g):
    g.nav_warp(30, 19, "CeruleanCity", "CeruleanGym")
    # The sight trainer walks onto (4,3), blocking Misty's south side.
    challenge(g, "CeruleanGym", 5, 2, "left", "EVENT_BEAT_MISTY")
    g.evidence("m13")
    g.nav_warp(4, 13, "CeruleanGym", "CeruleanCity", approach="down")
    g.heal_pokecenter((19, 17), "CeruleanCity", "CeruleanPokecenter")


def m14_bill(g):
    g.nav_to_map(45, 4, "Route25")
    g.nav_warp(45, 3, "Route25", "BillsHouse")
    g.nav_to(6, 6, "BillsHouse")
    g.face("up")
    g.tap("a", 20)
    g.dialogue_then_choice()
    g.choose("YES")
    assert g.cutscene()
    require_flag(g, "EVENT_BILL_SAID_USE_CELL_SEPARATOR")
    g.nav_to(1, 5, "BillsHouse")
    g.face("up")
    g.tap("a", 20)
    assert g.cutscene()
    require_flag(g, "EVENT_MET_BILL")
    g.nav_to(4, 5, "BillsHouse")
    g.face("up")
    g.tap("a", 20)
    assert g.cutscene()
    require_flag(g, "EVENT_GOT_SS_TICKET")
    g.evidence("m14")


def m15_vermilion(g):
    g.nav_warp(2, 7, "BillsHouse", "Route25", approach="down")
    g.nav_to_map(27, 12, "CeruleanCity")
    g.nav_warp(27, 11, "CeruleanCity", "CeruleanTrashedHouse")
    g.nav_warp(3, 0, "CeruleanTrashedHouse", "CeruleanCity", approach="up")
    g.nav_to_map(17, 28, "Route5")
    g.nav_warp(17, 27, "Route5", "UndergroundPathRoute5")
    g.nav_warp(4, 4, "UndergroundPathRoute5", "UndergroundPathNorthSouth")
    g.nav_warp(2, 41, "UndergroundPathNorthSouth", "UndergroundPathRoute6")
    g.nav_warp(3, 7, "UndergroundPathRoute6", "Route6", approach="down")
    g.nav_to_map(11, 4, "VermilionCity")
    g.heal_pokecenter((11, 3), "VermilionCity", "VermilionPokecenter")
    s = g.evidence("m15")
    assert s["map_name"] == "VermilionCity"


def m16_ss_anne(g):
    for attempt in range(3):
        try:
            return board_ss_anne(g)
        except (AssertionError, RuntimeError):
            s = g.st()
            if (attempt == 2 or s["map_name"] != "VermilionCity"
                    or s.get("battle_live", {}).get("player", {}).get("hp") != 0):
                raise
            print(f"[m16] rival blackout {attempt + 1}: boarding again")
            # A fresh party can reach the ship below Razor Leaf's level.
            # Earn it in nearby grass after a loss, rather than spending
            # all remaining attempts on a burned Tackle user.
            if s["party"][0]["level"] < 30:
                heal = ((11, 3), "VermilionCity", "VermilionPokecenter")
                assert g.train_until(30, "Route6", (12, 25), heal), "ship preparation stalled"
                g.heal_pokecenter(*heal)


def board_ss_anne(g):
    # The ticket inspection owns the tile immediately before the warp.
    g.nav_to(18, 30, "VermilionCity")
    assert g.cutscene()
    g.nav_warp(18, 31, "VermilionCity", "VermilionDock", approach="down")
    g.nav_warp(14, 2, "VermilionDock", "SSAnne1F", approach="down")
    g.nav_warp(2, 6, "SSAnne1F", "SSAnne2F")
    g.nav_to(36, 8, "SSAnne2F")
    assert g.cutscene()
    if g.st()["screen"] == "battle":
        g.battle_loop()
        assert g.cutscene()
    require_flag(g, "EVENT_BEAT_SS_ANNE_RIVAL")
    g.nav_warp(36, 4, "SSAnne2F", "SSAnneCaptainsRoom")
    g.nav_to(4, 3, "SSAnneCaptainsRoom")
    g.face("up")
    g.tap("a", 20)
    assert g.cutscene()
    require_flag(g, "EVENT_GOT_HM01")
    g.evidence("m16")


def m17_surge(g):
    use_item(g, "Hm01", party_index=0, forget="LeechSeed")
    assert "Cut" in g.st()["party"][0]["moves"]
    g.nav_warp(0, 7, "SSAnneCaptainsRoom", "SSAnne2F", approach="down")
    g.nav_warp(2, 4, "SSAnne2F", "SSAnne1F")
    g.nav_warp(27, 0, "SSAnne1F", "VermilionDock", approach="up")
    g.nav_warp(14, 0, "VermilionDock", "VermilionCity", approach="up")
    g.heal_pokecenter((11, 3), "VermilionCity", "VermilionPokecenter")
    g.nav_to(15, 17, "VermilionCity")
    g.face("down")
    field_move(g, "Cut")
    g.nav_warp(12, 19, "VermilionCity", "VermilionGym")
    solve_surge_switches(g)
    challenge(g, "VermilionGym", 5, 2, "up", "EVENT_BEAT_LT_SURGE")
    g.evidence("m17")


def inspect_can(g, index):
    x, y = 1 + (index // 3) * 2, 7 + (index % 3) * 2
    g.approach_object(x, y, "VermilionGym")
    g.tap("a", 16)
    assert g.cutscene()
    return g.d.cmd(cmd="get_flags")["data"]


def solve_surge_switches(g):
    # Observe only opened-lock flags, never the hidden random can positions.
    for attempt in range(60):
        for first in range(15):
            flags = inspect_can(g, first)
            if flags.get("EVENT_2ND_LOCK_OPENED"):
                return
            if not flags.get("EVENT_1ST_LOCK_OPENED"):
                continue
            col, row = divmod(first, 3)
            adjacent = [n for n in (first - 1, first + 1, first - 3, first + 3)
                        if 0 <= n < 15
                        and abs(n // 3 - col) + abs(n % 3 - row) == 1]
            flags = inspect_can(g, adjacent[attempt % len(adjacent)])
            if flags.get("EVENT_2ND_LOCK_OPENED"):
                print(f"[m17] both switches found on search {attempt + 1}")
                return
            break  # A wrong neighbor re-randomizes the first switch.
    raise RuntimeError("Surge switches not solved in 60 searches")


def m18_rock_tunnel_entrance(g):
    g.nav_warp(4, 17, "VermilionGym", "VermilionCity", approach="down")
    # CUT changes the current map instance; the tree regrows on re-entry.
    g.nav_to(15, 19, "VermilionCity")
    g.face("up")
    field_move(g, "Cut")
    g.heal_pokecenter((11, 3), "VermilionCity", "VermilionPokecenter")
    g.nav_to_map(17, 14, "Route6")
    g.nav_warp(17, 13, "Route6", "UndergroundPathRoute6")
    g.nav_warp(4, 4, "UndergroundPathRoute6", "UndergroundPathNorthSouth")
    g.nav_warp(5, 4, "UndergroundPathNorthSouth", "UndergroundPathRoute5")
    g.nav_warp(3, 7, "UndergroundPathRoute5", "Route5", approach="down")
    # Return via the eastern lane and the house's broken back wall;
    # Route5's central ledges cannot be climbed northbound.
    g.nav_to_map(27, 8, "CeruleanCity")
    g.nav_warp(27, 9, "CeruleanCity", "CeruleanTrashedHouse", approach="down")
    g.nav_warp(2, 7, "CeruleanTrashedHouse", "CeruleanCity", approach="down")
    g.heal_pokecenter((19, 17), "CeruleanCity", "CeruleanPokecenter")
    g.nav_to_map(4, 8, "Route9")
    g.face("right")
    field_move(g, "Cut")
    g.nav_to_map(11, 20, "Route10")
    g.heal_pokecenter((11, 19), "Route10", "RockTunnelPokecenter")
    g.evidence("m18")


def m19_rock_tunnel(g):
    g.nav_warp(8, 17, "Route10", "RockTunnel1F")
    g.nav_warp(37, 3, "RockTunnel1F", "RockTunnelB1F")
    g.nav_warp(27, 3, "RockTunnelB1F", "RockTunnel1F")
    g.nav_warp(17, 11, "RockTunnel1F", "RockTunnelB1F")
    g.nav_warp(3, 3, "RockTunnelB1F", "RockTunnel1F")
    g.nav_warp(15, 33, "RockTunnel1F", "Route10", approach="down")
    g.nav_to_map(3, 6, "LavenderTown")
    g.heal_pokecenter((3, 5), "LavenderTown", "LavenderPokecenter")
    g.evidence("m19")


def m20_celadon(g):
    g.nav_to_map(13, 4, "Route8")
    g.nav_warp(13, 3, "Route8", "UndergroundPathRoute8")
    g.nav_warp(4, 4, "UndergroundPathRoute8", "UndergroundPathWestEast")
    g.nav_warp(2, 5, "UndergroundPathWestEast", "UndergroundPathRoute7")
    g.nav_warp(3, 7, "UndergroundPathRoute7", "Route7", approach="down")
    g.nav_to_map(41, 10, "CeladonCity")
    g.heal_pokecenter((41, 9), "CeladonCity", "CeladonPokecenter")
    g.evidence("m20")


def m21_erika(g):
    for attempt in range(3):
        try:
            return challenge_erika(g)
        except (AssertionError, RuntimeError):
            s = g.st()
            if (attempt == 2 or s["map_name"] != "CeladonCity"
                    or s.get("battle_live", {}).get("player", {}).get("hp") != 0):
                raise
            print(f"[m21] blackout {attempt + 1}: returning to the gym")


def challenge_erika(g):
    g.nav_to(35, 31, "CeladonCity")
    g.face("down")
    field_move(g, "Cut")
    g.nav_warp(12, 27, "CeladonCity", "CeladonGym")
    g.nav_to(5, 8, "CeladonGym")
    g.face("up")
    field_move(g, "Cut")
    challenge(g, "CeladonGym", 4, 4, "up", "EVENT_BEAT_ERIKA")
    g.evidence("m21")


def talk_object(g, map_name, x, y):
    g.approach_object(x, y, map_name)
    g.tap("a", 16)
    assert g.cutscene()
    if g.st()["screen"] == "battle":
        g.battle_loop()
        assert g.cutscene()


def talk_npc(g, map_name, text_id, completion_flag=None):
    def find():
        data = g.d.cmd(cmd="get_npcs")["data"]
        npcs = data.get("npcs", data) if isinstance(data, dict) else data
        return next(n for n in npcs if n["text_id"] == text_id and n.get("visible", True))
    for _ in range(4):
        npc = find()
        g.approach_object(npc["x"], npc["y"], map_name)
        if completion_flag and g.d.cmd(cmd="get_flags")["data"].get(completion_flag):
            return  # An on-step encounter completed and removed the NPC.
        current = find()
        if (current["x"], current["y"]) != (npc["x"], npc["y"]):
            continue  # LOS battle moved the trainer during the approach.
        g.tap("a", 16)
        assert g.cutscene()
        if g.st()["screen"] == "battle":
            g.battle_loop()
            assert g.cutscene()
        return
    raise RuntimeError(f"NPC {text_id} did not settle on {map_name}")


def m22_lift_key(g):
    # A checkpoint reload restores the gym's original CUT tree as well.
    g.nav_to(5, 6, "CeladonGym")
    g.face("down")
    field_move(g, "Cut")
    g.nav_warp(4, 17, "CeladonGym", "CeladonCity", approach="down")
    g.nav_to(35, 33, "CeladonCity")
    g.face("up")
    field_move(g, "Cut")
    g.heal_pokecenter((41, 9), "CeladonCity", "CeladonPokecenter")
    g.nav_warp(28, 19, "CeladonCity", "GameCorner")
    talk_object(g, "GameCorner", 9, 5)
    require_flag(g, "EVENT_BEAT_GAME_CORNER_ROCKET")
    talk_object(g, "GameCorner", 9, 4)
    require_flag(g, "EVENT_FOUND_ROCKET_HIDEOUT")
    g.nav_warp(17, 4, "GameCorner", "RocketHideoutB1F")
    g.nav_warp(23, 2, "RocketHideoutB1F", "RocketHideoutB2F")
    g.nav_warp(21, 8, "RocketHideoutB2F", "RocketHideoutB3F")
    g.nav_warp(19, 18, "RocketHideoutB3F", "RocketHideoutB4F")
    talk_npc(g, "RocketHideoutB4F", 4)
    require_flag(g, "EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2")
    talk_npc(g, "RocketHideoutB4F", 4)  # post-battle admission drops the key
    require_flag(g, "EVENT_ROCKET_DROPPED_LIFT_KEY")
    talk_object(g, "RocketHideoutB4F", 10, 2)
    bag = g.d.cmd(cmd="get_bag")["data"]
    assert any(item["item"] == "LiftKey" for item in bag), bag
    g.evidence("m22")


def elevator(g, destination):
    talk_started = False
    for _ in range(30):
        s = g.st()
        menu = s.get("field_menu")
        if menu and menu["kind"] == "elevator":
            if menu["items"][menu["cursor"]] == destination:
                g.tap("a", 16)
                assert g.cutscene()
                return
            g.tap("down", 8)
        elif s.get("dialogue_state"):
            g.skip()
        elif not talk_started:
            g.approach_object(1, 1, "RocketHideoutElevator")
            g.tap("a", 16)
            talk_started = True
        else:
            g.step(20)
    raise RuntimeError(f"elevator did not reach {destination}")


def m23_silph_scope(g):
    g.nav_warp(19, 10, "RocketHideoutB4F", "RocketHideoutB3F")
    g.nav_warp(25, 6, "RocketHideoutB3F", "RocketHideoutB2F")
    g.nav_warp(24, 19, "RocketHideoutB2F", "RocketHideoutElevator", approach="down")
    elevator(g, "B4F")
    assert g.pos()[0] == "RocketHideoutB4F", g.pos()
    talk_npc(g, "RocketHideoutB4F", 2)
    talk_npc(g, "RocketHideoutB4F", 3)
    require_flag(g, "EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0")
    require_flag(g, "EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_1")
    # Door state is applied by the floor's on-entry callback.
    g.nav_warp(24, 15, "RocketHideoutB4F", "RocketHideoutElevator", approach="down")
    elevator(g, "B4F")
    require_flag(g, "EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED")
    talk_object(g, "RocketHideoutB4F", 25, 3)
    require_flag(g, "EVENT_BEAT_ROCKET_HIDEOUT_GIOVANNI")
    talk_object(g, "RocketHideoutB4F", 25, 2)
    bag = g.d.cmd(cmd="get_bag")["data"]
    assert any(item["item"] == "SilphScope" for item in bag), bag
    g.evidence("m23")


def m24_tower(g):
    g.nav_warp(24, 15, "RocketHideoutB4F", "RocketHideoutElevator", approach="down")
    elevator(g, "B1F")
    g.nav_warp(21, 2, "RocketHideoutB1F", "GameCorner")
    g.nav_warp(15, 17, "GameCorner", "CeladonCity", approach="down")
    g.heal_pokecenter((41, 9), "CeladonCity", "CeladonPokecenter")
    g.nav_to_map(5, 14, "Route7")
    g.nav_warp(5, 13, "Route7", "UndergroundPathRoute7")
    g.nav_warp(4, 4, "UndergroundPathRoute7", "UndergroundPathWestEast")
    g.nav_warp(47, 2, "UndergroundPathWestEast", "UndergroundPathRoute8")
    g.nav_warp(3, 7, "UndergroundPathRoute8", "Route8", approach="down")
    g.nav_to_map(3, 6, "LavenderTown")
    g.heal_pokecenter((3, 5), "LavenderTown", "LavenderPokecenter")
    g.nav_warp(14, 5, "LavenderTown", "PokemonTower1F")
    g.nav_warp(18, 9, "PokemonTower1F", "PokemonTower2F")
    g.nav_warp(3, 9, "PokemonTower2F", "PokemonTower3F")
    g.nav_warp(18, 9, "PokemonTower3F", "PokemonTower4F")
    g.nav_warp(3, 9, "PokemonTower4F", "PokemonTower5F")
    g.nav_to(10, 8, "PokemonTower5F")
    assert g.cutscene()
    s = g.evidence("m24")
    assert all(m["hp"] == m["max_hp"] for m in s["party"])


def m25_poke_flute(g):
    for attempt in range(3):
        try:
            return rescue_fuji(g)
        except (AssertionError, RuntimeError):
            s = g.st()
            if (attempt == 2 or s["map_name"] != "LavenderTown"
                    or s.get("battle_live", {}).get("player", {}).get("hp") != 0):
                raise
            print(f"[m25] blackout {attempt + 1}: returning to the tower")
            g.nav_warp(14, 5, "LavenderTown", "PokemonTower1F")
            for floor in range(1, 5):
                x = 18 if floor % 2 else 3
                g.nav_warp(x, 9, f"PokemonTower{floor}F", f"PokemonTower{floor + 1}F")
            g.nav_to(10, 8, "PokemonTower5F")
            assert g.cutscene()


def rescue_fuji(g):
    g.nav_warp(18, 9, "PokemonTower5F", "PokemonTower6F")
    # The Rare Candy ball occupies the one-tile passage into the west half.
    if not g.d.cmd(cmd="get_flags")["data"].get("EVENT_GOT_RARE_CANDY_POKEMON_TOWER_6F"):
        talk_npc(g, "PokemonTower6F", 4)
    g.nav_to(10, 16, "PokemonTower6F")
    assert g.cutscene()
    if g.st()["screen"] == "battle":
        g.battle_loop()
        assert g.cutscene()
    require_flag(g, "EVENT_BEAT_GHOST_MAROWAK")
    g.nav_warp(9, 16, "PokemonTower6F", "PokemonTower7F")
    talk_npc(g, "PokemonTower7F", 4)
    require_flag(g, "EVENT_RESCUED_MR_FUJI")
    assert g.pos()[0] == "MrFujisHouse", g.pos()
    talk_npc(g, "MrFujisHouse", 5)
    require_flag(g, "EVENT_GOT_POKE_FLUTE")
    g.evidence("m25")


def m26_fuchsia(g):
    g.nav_warp(2, 7, "MrFujisHouse", "LavenderTown", approach="down")
    g.heal_pokecenter((3, 5), "LavenderTown", "LavenderPokecenter")
    g.nav_to_map(10, 14, "Route12")
    g.nav_warp(10, 15, "Route12", "Route12Gate1F", approach="down")
    g.nav_warp(4, 7, "Route12Gate1F", "Route12", approach="down")
    g.approach_object(10, 62, "Route12")
    use_item(g, "PokeFlute")
    talk_npc(g, "Route12", 1)
    require_flag(g, "EVENT_BEAT_ROUTE12_SNORLAX")
    # The grass-free search otherwise takes a long northern detour.
    g.nav_to_map(19, 28, "FuchsiaCity", avoid_grass=False)
    g.heal_pokecenter((19, 27), "FuchsiaCity", "FuchsiaPokecenter")
    g.evidence("m26")


def m27_koga(g):
    for attempt in range(3):
        try:
            g.nav_warp(5, 27, "FuchsiaCity", "FuchsiaGym")
            talk_npc(g, "FuchsiaGym", 1)
            require_flag(g, "EVENT_BEAT_KOGA")
            g.evidence("m27")
            return
        except (AssertionError, RuntimeError):
            s = g.st()
            if (attempt == 2 or s["map_name"] != "FuchsiaCity"
                    or s.get("battle_live", {}).get("player", {}).get("hp") != 0):
                raise
            print(f"[m27] blackout {attempt + 1}: healed, retrying Koga")


def m28_safari(g):
    # A final Selfdestruct can award the badge with our sole member at 0 HP.
    # The next overworld step performs the normal blackout; preserve its cost.
    if all(m["hp"] == 0 for m in g.st()["party"]):
        for direction in ["up", "down"] * 6:
            g.d.drive([direction] * 8, frames=16)
            assert g.cutscene()
            if g.pos()[0] != "FuchsiaGym":
                break
        assert g.pos()[0] == "FuchsiaCity", g.pos()
    else:
        g.nav_warp(4, 17, "FuchsiaGym", "FuchsiaCity", approach="down")
    g.heal_pokecenter((19, 27), "FuchsiaCity", "FuchsiaPokecenter")
    g.nav_warp(18, 3, "FuchsiaCity", "SafariZoneGate")
    g.nav_to(3, 3, "SafariZoneGate")
    g.d.drive(["up"] * 8, frames=16)  # automatic gate-row fee prompt
    g.dialogue_then_choice()
    g.choose("YES")
    assert g.cutscene()
    assert g.pos()[0] == "SafariZoneCenter", g.pos()
    g.nav_warp(29, 10, "SafariZoneCenter", "SafariZoneEast")
    g.nav_warp(0, 4, "SafariZoneEast", "SafariZoneNorth")
    g.nav_warp(2, 35, "SafariZoneNorth", "SafariZoneWest", approach="down")
    talk_npc(g, "SafariZoneWest", 4)
    assert any(v["item"] == "GoldTeeth" for v in g.d.cmd(cmd="get_bag")["data"])
    g.nav_warp(3, 3, "SafariZoneWest", "SafariZoneSecretHouse")
    talk_npc(g, "SafariZoneSecretHouse", 1)
    require_flag(g, "EVENT_GOT_HM03")
    g.evidence("m28")


def m29_strength(g):
    g.nav_warp(2, 7, "SafariZoneSecretHouse", "SafariZoneWest", approach="down")
    g.nav_to(3, 4, "SafariZoneWest")
    # Walk the remaining Safari allowance on this safe corridor (x=2..5).
    # Time is measured in steps; waiting frames alone cannot end the hunt.
    for step in range(200):
        s = g.st()
        if s["map_name"] == "SafariZoneGate":
            break
        assert s["map_name"] == "SafariZoneWest", s["map_name"]
        direction = "right" if step % 2 == 0 else "left"
        g.d.drive([direction] * 32, frames=40)
        assert g.cutscene()
    else:
        raise RuntimeError("Safari step allowance did not expire")
    # The gate script still asks to return the remaining Safari Balls.
    g.nav_to(3, 1, "SafariZoneGate")
    g.d.drive(["down"] * 8, frames=16)
    g.dialogue_then_choice()
    g.choose("YES")
    assert g.cutscene()
    g.nav_warp(3, 5, "SafariZoneGate", "FuchsiaCity", approach="down")
    g.nav_warp(27, 27, "FuchsiaCity", "WardensHouse")
    talk_npc(g, "WardensHouse", 1)
    require_flag(g, "EVENT_GOT_HM04")
    # Gen-I Venusaur can only learn HM01; reserve Strength for Lapras.
    g.nav_warp(4, 7, "WardensHouse", "FuchsiaCity", approach="down")
    g.heal_pokecenter((19, 27), "FuchsiaCity", "FuchsiaPokecenter")
    g.evidence("m29")


def m30_saffron(g):
    g.nav_to_map(51, 1, "Route13", avoid_grass=False)
    g.nav_to_map(10, 22, "Route12", avoid_grass=False)
    g.nav_warp(10, 21, "Route12", "Route12Gate1F")
    g.nav_warp(4, 0, "Route12Gate1F", "Route12")
    g.nav_to_map(3, 6, "LavenderTown")
    g.heal_pokecenter((3, 5), "LavenderTown", "LavenderPokecenter")
    m20_celadon(g)
    g.nav_warp(8, 13, "CeladonCity", "CeladonMart1F")
    for floor, x in [(1, 12), (2, 16), (3, 12), (4, 16), (5, 12)]:
        dest = "CeladonMartRoof" if floor == 5 else f"CeladonMart{floor + 1}F"
        g.nav_warp(x, 1, f"CeladonMart{floor}F", dest)
    g.nav_to(10, 2, "CeladonMartRoof")
    g.face("up")
    g.tap("a", 16)
    g.dialogue_then_choice()
    g.choose("FRESH WATER")
    assert g.cutscene()
    assert any(v["item"] == "FreshWater" for v in g.d.cmd(cmd="get_bag")["data"])
    g.nav_warp(15, 2, "CeladonMartRoof", "CeladonMart5F")
    for floor, x in [(5, 16), (4, 12), (3, 16), (2, 12)]:
        g.nav_warp(x, 1, f"CeladonMart{floor}F", f"CeladonMart{floor - 1}F")
    g.nav_warp(2, 7, "CeladonMart1F", "CeladonCity", approach="down")
    g.nav_to_map(10, 10, "Route7")
    g.nav_warp(11, 10, "Route7", "Route7Gate", approach="right")
    g.nav_warp(5, 3, "Route7Gate", "Route7")
    require_flag(g, "EVENT_GAVE_SAFFRON_GUARDS_DRINK")
    g.nav_to_map(9, 30, "SaffronCity")
    g.heal_pokecenter((9, 29), "SaffronCity", "SaffronPokecenter")
    g.evidence("m30")


def m31_card_key(g):
    g.nav_warp(18, 21, "SaffronCity", "SilphCo1F")
    for floor, x in [(1, 26), (2, 26), (3, 24), (4, 26)]:
        g.nav_warp(x, 0, f"SilphCo{floor}F", f"SilphCo{floor + 1}F")
    silph_fifth_floor_pad(g)
    talk_npc(g, "SilphCo5F", 8)
    require_flag(g, "EVENT_GOT_CARD_KEY_SILPH_CO_5F")
    assert any(v["item"] == "CardKey" for v in g.d.cmd(cmd="get_bag")["data"])
    g.evidence("m31")


def m32_lapras(g):
    silph_fifth_floor_pad(g)
    g.nav_warp(26, 0, "SilphCo5F", "SilphCo4F")
    g.nav_warp(24, 0, "SilphCo4F", "SilphCo3F")
    use_item(g, "Tm21", party_index=0, forget="Tackle")
    g.nav_warp(26, 0, "SilphCo3F", "SilphCo2F")
    g.nav_warp(24, 0, "SilphCo2F", "SilphCo1F")
    g.nav_warp(10, 17, "SilphCo1F", "SaffronCity", approach="down")
    g.nav_warp(25, 11, "SaffronCity", "SaffronMart")
    g.nav_to(2, 5, "SaffronMart")
    g.face("left")
    g.tap("a", 16)
    buy(g, "HyperPotion", 1, 8)
    g.nav_warp(3, 7, "SaffronMart", "SaffronCity", approach="down")
    g.heal_pokecenter((9, 29), "SaffronCity", "SaffronPokecenter")
    g.nav_warp(18, 21, "SaffronCity", "SilphCo1F")
    g.nav_warp(26, 0, "SilphCo1F", "SilphCo2F")
    g.nav_warp(26, 0, "SilphCo2F", "SilphCo3F")
    talk_object(g, "SilphCo3F", 17, 9)
    require_flag(g, "EVENT_SILPH_CO_3_UNLOCKED_DOOR2")
    g.nav_warp(11, 11, "SilphCo3F", "SilphCo7F")
    g.nav_to(3, 3, "SilphCo7F")
    assert g.cutscene()
    if g.st()["screen"] == "battle":
        g.battle_loop()
        assert g.cutscene()
    require_flag(g, "EVENT_BEAT_SILPH_CO_RIVAL")
    talk_npc(g, "SilphCo7F", 1)
    require_flag(g, "EVENT_GOT_LAPRAS")
    use_item(g, "Hm03", party_index=1)
    assert "Surf" in g.st()["party"][1]["moves"]
    use_item(g, "Hm04", party_index=1, forget="Growl")
    assert "Strength" in g.st()["party"][1]["moves"]
    g.evidence("m32")


def silph_fifth_floor_pad(g):
    # The pad fills the corridor. Returning onto it lets us leave on the
    # opposite side; crossing it directly would always send us to 9F.
    g.nav_warp(9, 15, "SilphCo5F", "SilphCo9F")
    g.nav_to(17, 14, "SilphCo9F")
    g.nav_warp(17, 15, "SilphCo9F", "SilphCo5F")


def m33_silph_giovanni(g):
    g.nav_warp(5, 7, "SilphCo7F", "SilphCo11F")
    talk_object(g, "SilphCo11F", 6, 13)
    require_flag(g, "EVENT_SILPH_CO_11_UNLOCKED_DOOR")
    talk_npc(g, "SilphCo11F", 3, completion_flag="EVENT_BEAT_SILPH_CO_GIOVANNI")
    require_flag(g, "EVENT_BEAT_SILPH_CO_GIOVANNI")
    # The left aisle ends on a listed warp tile; explicitly walk there
    # before approaching the president behind the desk.
    g.nav_to(5, 5, "SilphCo11F")
    talk_npc(g, "SilphCo11F", 1)
    require_flag(g, "EVENT_GOT_MASTER_BALL")
    g.evidence("m33")


def teleport(g, map_name, start, destination):
    g.nav_to(*start, map_name)
    for _ in range(60):
        s = g.st()
        if (s["player_x"], s["player_y"]) == destination:
            assert g.cutscene()
            return
        g.step(10)
    raise RuntimeError(f"teleport {start} did not reach {destination}: {g.pos()}")


def m34_sabrina(g):
    g.nav_to(5, 5, "SilphCo11F")
    g.nav_warp(3, 2, "SilphCo11F", "SilphCo7F")
    g.nav_warp(5, 3, "SilphCo7F", "SilphCo3F")
    g.nav_warp(26, 0, "SilphCo3F", "SilphCo2F")
    g.nav_warp(24, 0, "SilphCo2F", "SilphCo1F")
    g.nav_warp(10, 17, "SilphCo1F", "SaffronCity", approach="down")
    g.heal_pokecenter((9, 29), "SaffronCity", "SaffronPokecenter")
    g.nav_warp(34, 3, "SaffronCity", "SaffronGym")
    for source, dest in [((11, 15), (19, 17)), ((19, 15), (19, 9)),
                         ((19, 11), (1, 9)), ((1, 11), (5, 5)),
                         ((1, 5), (11, 11))]:
        teleport(g, "SaffronGym", source, dest)
    talk_npc(g, "SaffronGym", 1)
    require_flag(g, "EVENT_BEAT_SABRINA")
    g.evidence("m34")


def surf_to(g, destination):
    """Plan over connected water, allowing dismount only at the chosen shore."""
    from collections import deque
    from playthrough import MAPS, CONNS, DELTA, tile_at, walkable
    for _ in range(400):
        s = g.st()
        if s["screen"] == "battle":
            g.battle_loop(prefer="run")
            assert g.cutscene()
            continue
        if s.get("dialogue_state"):
            assert g.cutscene()
            continue
        root = (s["map_name"], s["player_x"], s["player_y"])
        if root == destination:
            return
        assert s["player_transport"] == "Surfing", s["player_transport"]
        queue, previous = deque([root]), {root: None}
        occupied = g.live_npcs(root[0])
        while queue and destination not in previous:
            current = queue.popleft()
            m, x, y = current
            for d, (dx, dy) in DELTA.items():
                nx, ny, nm = x + dx, y + dy, m
                if not (0 <= nx < MAPS[m]["width"] * 2 and 0 <= ny < MAPS[m]["height"] * 2):
                    conn = CONNS[m].get({"up": "north", "down": "south", "left": "west", "right": "east"}[d])
                    if not conn:
                        continue
                    nm, off = conn["targetMap"], conn["offset"] * 2
                    if d == "down": nx, ny = x - off, 0
                    elif d == "up": nx, ny = x - off, MAPS[nm]["height"] * 2 - 1
                    elif d == "left": nx, ny = MAPS[nm]["width"] * 2 - 1, y - off
                    else: nx, ny = 0, y - off
                node = (nm, nx, ny)
                water = tile_at(nm, nx, ny) in {0x14, 0x48, 0x32}
                if (node in previous or nm == root[0] and (nx, ny) in occupied
                        or not (water or node == destination and walkable(nm, nx, ny))):
                    continue
                previous[node] = (current, d)
                queue.append(node)
        assert destination in previous, (root, destination)
        path, node = [], destination
        while previous[node]:
            node, d = previous[node]
            path.append(d)
        path.reverse()
        count = 1
        while count < min(3, len(path)) and path[count] == path[0]:
            count += 1
        g.d.drive([path[0]] * (count * 8), frames=count * 8 + 16)
    raise RuntimeError(f"Surf did not reach {destination}")


def m35_fly_hm(g):
    for source, dest in [((11, 11), (1, 5)), ((5, 5), (1, 11)),
                         ((1, 9), (19, 11)), ((19, 9), (19, 15)),
                         ((19, 17), (11, 15))]:
        teleport(g, "SaffronGym", source, dest)
    g.nav_warp(8, 17, "SaffronGym", "SaffronCity", approach="down")
    g.heal_pokecenter((9, 29), "SaffronCity", "SaffronPokecenter")
    g.nav_to_map(19, 10, "Route7")
    g.nav_warp(18, 10, "Route7", "Route7Gate", approach="left")
    g.nav_warp(0, 3, "Route7Gate", "Route7", approach="left")
    g.nav_to_map(34, 10, "Route16")
    g.face("up")
    field_move(g, "Cut")
    g.nav_warp(24, 4, "Route16", "Route16Gate1F", approach="left")
    g.nav_warp(0, 2, "Route16Gate1F", "Route16", approach="left")
    g.nav_warp(7, 5, "Route16", "Route16FlyHouse")
    talk_npc(g, "Route16FlyHouse", 1)
    require_flag(g, "EVENT_GOT_HM02")
    g.evidence("m35")


def catch_master_ball(g):
    for _ in range(250):
        s = g.st()
        if s["screen"] != "battle":
            if s["screen"] == "overworld":
                assert g.cutscene()
                return
            g.tap("b", 10)  # dex registration
            continue
        ph = s["battle_phase"]
        if ph == "PlayerMenu":
            g.tap("down", 8)
            g.tap("left", 8)
            g.tap("a", 8)
        elif ph == "BagSelect":
            menu = s["battle_bag"]
            target = next(i for i, v in enumerate(menu["items"]) if v["item"] == "MasterBall")
            g.tap("a" if menu["cursor"] == target else "down", 8)
        else:
            g.tap("a", 10)
    raise RuntimeError("Master Ball capture did not finish")


def m36_zapdos(g):
    g.nav_warp(2, 7, "Route16FlyHouse", "Route16", approach="down")
    g.nav_warp(17, 4, "Route16", "Route16Gate1F", approach="right")
    g.nav_warp(7, 2, "Route16Gate1F", "Route16", approach="right")
    g.nav_to(34, 8, "Route16")
    g.face("down")
    field_move(g, "Cut")
    g.nav_to_map(10, 10, "Route7")
    g.nav_warp(11, 10, "Route7", "Route7Gate", approach="right")
    g.nav_warp(5, 3, "Route7Gate", "Route7", approach="right")
    g.nav_to_map(4, 8, "Route9", avoid_grass=False)
    g.face("right")
    field_move(g, "Cut")
    g.nav_to_map(15, 4, "Route10", avoid_grass=False)
    g.face("right")
    field_move(g, "Surf", party_index=1)
    surf_to(g, ("Route10", 10, 45))
    g.nav_warp(6, 39, "Route10", "PowerPlant")
    g.approach_object(4, 9, "PowerPlant")
    g.tap("a", 16)
    assert g.cutscene()
    g.wait("screen=battle", 900)
    catch_master_ball(g)
    party = g.st()["party"]
    assert party[2]["species"] == "Zapdos", party
    use_item(g, "Hm02", party_index=2)
    use_item(g, "Tm24", party_index=2)
    assert {"Fly", "Thunderbolt"}.issubset(g.st()["party"][2]["moves"])
    g.evidence("m36")


def fly(g, destination):
    index = next(i for i, mon in enumerate(g.st()["party"]) if "Fly" in mon["moves"])
    field_move(g, "Fly", party_index=index)
    for _ in range(30):
        menu = g.st().get("field_menu")
        assert menu and menu["kind"] == "town_map", menu
        if menu["selected_map"] == destination:
            g.tap("a", 16)
            assert g.cutscene()
            assert g.pos()[0] == destination, g.pos()
            return
        g.tap("down", 8)
    raise RuntimeError(f"Fly destination unavailable: {destination}")


def m37_cinnabar(g):
    g.nav_warp(0, 11, "PowerPlant", "Route10", approach="left")
    fly(g, "ViridianCity")
    g.heal_pokecenter((23, 25), "ViridianCity", "ViridianPokecenter")
    fly(g, "PalletTown")
    g.nav_to(5, 13, "PalletTown")
    g.face("down")
    field_move(g, "Surf", party_index=1)
    surf_to(g, ("CinnabarIsland", 4, 4))
    g.heal_pokecenter((11, 11), "CinnabarIsland", "CinnabarPokecenter")
    g.evidence("m37")


def buy(g, item, stock_index, quantity):
    before = sum(v["qty"] for v in g.d.cmd(cmd="get_bag")["data"] if v["item"] == item)
    bought = False
    for _ in range(120):
        s = g.st()
        phase = s.get("shop_phase")
        # Transaction Result lasts one engine frame and may be gone before
        # the next snapshot. Verify the committed bag delta instead.
        current = sum(v["qty"] for v in g.d.cmd(cmd="get_bag")["data"] if v["item"] == item)
        assert current <= before + quantity, (item, before, current, quantity)
        bought = bought or current == before + quantity
        if phase is None:
            if bought:
                after = sum(v["qty"] for v in g.d.cmd(cmd="get_bag")["data"] if v["item"] == item)
                assert after == before + quantity, (before, after, quantity)
                return
            if s.get("dialogue_state"):
                g.skip()
            else:
                g.step(10)
        elif bought:
            g.tap("a" if "Result" in phase else "b", 8)
        elif phase.startswith("MainMenu"):
            g.tap("a" if "Buy" in phase else "up", 8)
        elif "SelectItem" in phase:
            cursor = int(re.search(r"cursor: (\d+)", phase)[1])
            g.tap("a" if cursor == stock_index else "down", 8)
        elif "Quantity" in phase:
            q = int(re.search(r"quantity: (\d+)", phase)[1])
            g.tap("a" if q == quantity else "up", 8)
        elif "Confirm" in phase:
            g.tap("a" if "selected: Yes" in phase else "up", 8)
        elif "Result" in phase:
            assert "Success" in phase, phase
            bought = True
        else:
            raise RuntimeError(f"unknown shop phase {phase}")
    raise RuntimeError(f"could not buy {quantity} {item}")


def mansion_switch(g, map_name, x, y):
    for _ in range(6):
        g.nav_to(x, y + 1, map_name)
        g.step(30)  # a wild encounter may start just after the final step
        if g.st()["screen"] == "battle":
            g.battle_loop(prefer="run")
            assert g.cutscene()
            continue
        g.face("up")
        g.tap("a", 16)
        # A wild encounter can begin during the interaction tap itself, after
        # the pre-tap battle check above. Drain it before looking for the
        # statue's YES/NO menu, otherwise dialogue_then_choice reports a
        # misleading missing-choice failure.
        if g.st()["screen"] == "battle":
            g.battle_loop(prefer="run")
            assert g.cutscene()
            continue
        g.dialogue_then_choice()
        break
    else:
        raise RuntimeError("wild encounters prevented statue interaction")
    before = g.d.cmd(cmd="get_flags")["data"].get("EVENT_MANSION_SWITCH_ON", False)
    g.choose("YES")
    assert g.cutscene()
    assert g.d.cmd(cmd="get_flags")["data"].get("EVENT_MANSION_SWITCH_ON", False) != before


def m38_secret_key(g):
    from playthrough import NavError
    # Avoid the locked gym's automatic shove at (18,4) while walking west.
    g.nav_to(10, 5, "CinnabarIsland")
    g.nav_warp(6, 3, "CinnabarIsland", "PokemonMansion1F")
    g.nav_warp(5, 10, "PokemonMansion1F", "PokemonMansion2F")
    g.nav_warp(6, 1, "PokemonMansion2F", "PokemonMansion3F")
    mansion_switch(g, "PokemonMansion3F", 10, 5)
    # The hole changes maps while nav_to is finishing its final drive. The
    # requested destination is already reached when nav_to raises its
    # same-map guard, so accept only this explicitly expected landing.
    try:
        g.nav_to(16, 14, "PokemonMansion3F")
    except NavError:
        if g.pos() != ("PokemonMansion1F", 16, 14):
            raise
    assert g.cutscene()
    assert g.pos()[0] == "PokemonMansion1F", g.pos()
    g.nav_warp(21, 23, "PokemonMansion1F", "PokemonMansionB1F")
    mansion_switch(g, "PokemonMansionB1F", 18, 25)
    mansion_switch(g, "PokemonMansionB1F", 20, 3)
    talk_npc(g, "PokemonMansionB1F", 8)
    require_flag(g, "EVENT_GOT_SECRET_KEY_MANSION_B1F")
    assert any(v["item"] == "SecretKey" for v in g.d.cmd(cmd="get_bag")["data"])
    g.evidence("m38")


def lead_with(g, species):
    party = g.st()["party"]
    target = next(i for i, mon in enumerate(party) if mon["species"] == species)
    if target == 0:
        return
    open_start(g, "Pokemon")
    for _ in range(8):
        menu = g.st()["field_menu"]
        if menu["cursor"] == target:
            break
        g.tap("down", 8)
    g.tap("a", 8)
    for _ in range(10):
        menu = g.st()["field_menu"]
        cursor = int(re.search(r"cursor: (\d+)", menu["phase"])[1])
        if cursor == len(menu["field_moves"]) + 1:
            g.tap("a", 8)
            break
        g.tap("down", 8)
    for _ in range(8):
        menu = g.st()["field_menu"]
        if menu["cursor"] == 0:
            g.tap("a", 8)
            break
        g.tap("up", 8)
    g.tap("b", 8)
    g.tap("b", 8)
    assert g.cutscene()
    assert g.st()["party"][0]["species"] == species


def m39_blaine(g):
    mansion_switch(g, "PokemonMansionB1F", 20, 3)
    mansion_switch(g, "PokemonMansionB1F", 18, 25)
    g.nav_warp(23, 22, "PokemonMansionB1F", "PokemonMansion1F")
    g.nav_warp(26, 27, "PokemonMansion1F", "CinnabarIsland", approach="down")
    g.heal_pokecenter((11, 11), "CinnabarIsland", "CinnabarPokecenter")
    lead_with(g, "Zapdos")
    g.nav_warp(18, 3, "CinnabarIsland", "CinnabarGym")
    for i, (x, y, answer) in enumerate([(15, 7, "YES"), (10, 1, "NO"), (9, 7, "NO"),
                                       (9, 13, "NO"), (1, 13, "YES"), (1, 7, "NO")], 1):
        g.nav_to(x, y + 1, "CinnabarGym")
        g.face("up")
        g.tap("a", 16)
        g.dialogue_then_choice()
        g.choose(answer)
        assert g.cutscene()
        require_flag(g, f"EVENT_CINNABAR_GYM_GATE{i}_UNLOCKED")
    talk_npc(g, "CinnabarGym", 1)
    require_flag(g, "EVENT_BEAT_BLAINE")
    g.evidence("m39")


def m40_giovanni_badge(g):
    g.nav_warp(16, 17, "CinnabarGym", "CinnabarIsland", approach="down")
    fly(g, "ViridianCity")
    g.heal_pokecenter((23, 25), "ViridianCity", "ViridianPokecenter")
    lead_with(g, "Venusaur")
    g.nav_warp(32, 7, "ViridianCity", "ViridianGym")
    talk_npc(g, "ViridianGym", 1)
    require_flag(g, "EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI")
    s = g.evidence("m40")
    assert s["badges"] == 255, s["badges"]


def m41_victory_road_entrance(g):
    g.nav_warp(16, 17, "ViridianGym", "ViridianCity", approach="down")
    g.heal_pokecenter((23, 25), "ViridianCity", "ViridianPokecenter")
    g.nav_to_map(8, 6, "Route22")
    g.nav_warp(8, 5, "Route22", "Route22Gate")
    g.nav_warp(4, 0, "Route22Gate", "Route23")
    g.nav_to(10, 104, "Route23")
    g.face("up")
    index = next(i for i, mon in enumerate(g.st()["party"]) if "Surf" in mon["moves"])
    field_move(g, "Surf", party_index=index)
    surf_to(g, ("Route23", 10, 71))
    g.nav_warp(4, 31, "Route23", "VictoryRoad1F")
    g.evidence("m41")


def push_boulder(g, map_name, text_id, destination, flag):
    from collections import deque
    from playthrough import bfs, DELTA, MAPS, walkable_edge, tile_at, warp_tiles
    # A switch push can finish on the same frame that a wild encounter hands
    # control back to the overworld. Resolve that hand-off before opening the
    # party menu for the next Strength use.
    if g.st()["screen"] == "battle":
        g.battle_loop(prefer="run")
        assert g.cutscene()
    index = next(i for i, mon in enumerate(g.st()["party"]) if "Strength" in mon["moves"])
    field_move(g, "Strength", party_index=index)
    for _ in range(150):
        if flag and g.d.cmd(cmd="get_flags")["data"].get(flag):
            return
        s = g.st()
        data = g.d.cmd(cmd="get_npcs")["data"]
        npcs = data.get("npcs", data) if isinstance(data, dict) else data
        target = next(n for n in npcs if n["text_id"] == text_id and n.get("visible", True))
        target_npc_index = npcs.index(target)
        all_boulder_indices = [i for i, npc in enumerate(npcs)
                               if npc.get("visible", True)
                               and npc.get("sprite_id") == 63]
        # The 3F second switch is the one puzzle where another boulder must
        # be moved to open the route. Keep the state space tight elsewhere:
        # the normal single-boulder planner is both faster and avoids
        # inventing unnecessary moves for the already-solved puzzles.
        if map_name == "VictoryRoad3F" and text_id == 10:
            boulder_indices = [target_npc_index] + [i for i in all_boulder_indices
                               if npcs[i]["text_id"] == 8]
        else:
            boulder_indices = [target_npc_index]
        target_slot = boulder_indices.index(target_npc_index)
        boulders = tuple((npcs[i]["x"], npcs[i]["y"])
                         for i in boulder_indices)
        box = boulders[target_slot]
        if box == destination and (flag is None or
                                   g.d.cmd(cmd="get_flags")["data"].get(flag)):
            return
        # Other boulders are dynamic obstacles, not permanent walls. Static
        # NPCs remain hard obstacles; all boulder positions are part of the
        # search state and may be pushed when that is the only way to reach
        # the requested target.
        occupied = {(n["x"], n["y"]) for i, n in enumerate(npcs)
                    if n.get("visible", True)
                    and (n.get("sprite_id") != 63
                         or i not in boulder_indices)
                    and (n["x"], n["y"]) != (-1, -1)}
        exit_mats = {(w["x"], w["y"]) for w in MAPS[map_name]["warps"]
                     if w["y"] == MAPS[map_name]["height"] * 2 - 1}
        occupied |= warp_tiles(map_name) - exit_mats
        root = ((s["player_x"], s["player_y"]), boulders)
        queue, previous = deque([root]), {root: None}
        solved = None
        while queue and len(previous) <= 5000:
            player, positions = node = queue.popleft()
            if positions[target_slot] == destination:
                solved = node
                break
            blocked = occupied | set(positions)
            # Prefer the requested boulder, then use the others only when a
            # valid solution requires clearing their route.
            slots = [target_slot] + [slot for slot in range(len(positions))
                                     if slot != target_slot]
            for slot in slots:
                boulder = positions[slot]
                for d, (dx, dy) in DELTA.items():
                    behind = (boulder[0] - dx, boulder[1] - dy)
                    ahead = (boulder[0] + dx, boulder[1] + dy)
                    if (ahead in occupied or ahead in positions
                            or not walkable_edge(map_name, behind, ahead)
                            or tile_at(map_name, *ahead) == 0x15
                            or behind in occupied or behind in positions):
                        continue
                    if not bfs(map_name, player, behind, blocked=blocked):
                        continue
                    moved = list(positions)
                    moved[slot] = ahead
                    child = (boulder, tuple(moved))
                    if child in previous:
                        continue
                    previous[child] = (node, d, behind, slot)
                    queue.append(child)
        assert solved is not None, (map_name, box, destination)
        actions, node = [], solved
        while previous[node]:
            parent, d, behind, slot = previous[node]
            actions.append((d, behind, slot))
            node = parent
        actions.reverse()
        print(f"[boulder] {map_name} {box} → {destination}: {len(actions)} pushes",
              flush=True)
        # A plan involving another movable boulder is executed one push at a
        # time and re-planned from the live state. Single-boulder plans keep
        # the original batch execution, which is much faster and has no
        # dynamic obstacle state to invalidate.
        actions_to_execute = actions if len(boulder_indices) == 1 else actions[:1]
        for d, behind, slot in actions_to_execute:
            # Boulder coordinates are mutable NPC state. The general
            # navigator intentionally remembers observed NPC bands, but a
            # remembered old boulder tile is stale immediately after a push
            # and can seal the next valid route.
            g.observed_npcs[map_name] = g.live_npcs(map_name)
            if map_name == "VictoryRoad1F" and behind in {(8, 17), (9, 17)}:
                # Enter an exit mat sideways to stand south of the boulder.
                # Walking DOWN onto it would leave the cave.
                g.nav_to(7, 17, map_name)
                g.d.drive(["right"] * ((behind[0] - 7) * 8),
                          frames=(behind[0] - 7) * 8 + 16)
            else:
                g.nav_to(*behind, map_name)
            g.d.drive([d] * 8, frames=48)
            assert g.cutscene()
            if flag and g.d.cmd(cmd="get_flags")["data"].get(flag):
                return
    raise RuntimeError(f"boulder did not reach {destination}")


def m42_victory_first_switch(g):
    push_boulder(g, "VictoryRoad1F", 5, (17, 13), "EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH")
    g.nav_warp(1, 1, "VictoryRoad1F", "VictoryRoad2F")
    push_boulder(g, "VictoryRoad2F", 11, (1, 16), "EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1")
    g.evidence("m42")


def m43_victory_upper_switch(g):
    g.nav_warp(23, 7, "VictoryRoad2F", "VictoryRoad3F")
    push_boulder(g, "VictoryRoad3F", 7, (3, 5), "EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH1")
    push_boulder(g, "VictoryRoad3F", 10, (23, 15), "EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2")
    g.nav_to(23, 15, "VictoryRoad3F")
    assert g.cutscene()
    assert g.pos()[0] == "VictoryRoad2F", g.pos()
    push_boulder(g, "VictoryRoad2F", 13, (9, 16), "EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2")
    g.evidence("m43")


def sell(g, item):
    initial = g.d.cmd(cmd="get_bag")["data"]
    target = next(i for i, v in enumerate(initial) if v["item"] == item)
    quantity = initial[target]["qty"]
    for _ in range(120):
        s = g.st()
        phase = s.get("shop_phase")
        sold = not any(v["item"] == item for v in g.d.cmd(cmd="get_bag")["data"])
        if sold:
            if phase is None:
                return
            g.tap("b", 8)
        elif phase is None:
            if s.get("dialogue_state"):
                g.skip()
            else:
                g.step(10)
        elif phase.startswith("MainMenu"):
            g.tap("a" if "Sell" in phase else "down", 8)
        elif "SelectItem" in phase:
            cursor = int(re.search(r"cursor: (\d+)", phase)[1])
            g.tap("a" if cursor == target else "down", 8)
        elif "Quantity" in phase:
            q = int(re.search(r"quantity: (\d+)", phase)[1])
            g.tap("a" if q == quantity else "up", 8)
        elif "Confirm" in phase:
            g.tap("a" if "selected: Yes" in phase else "up", 8)
        else:
            g.step(10)
    raise RuntimeError(f"could not sell {item}")


def m44_indigo(g):
    g.nav_warp(25, 14, "VictoryRoad2F", "VictoryRoad3F")
    g.nav_warp(26, 8, "VictoryRoad3F", "VictoryRoad2F")
    g.nav_warp(29, 7, "VictoryRoad2F", "Route23", approach="right")
    g.nav_to_map(9, 6, "IndigoPlateau", avoid_grass=False)
    g.nav_warp(9, 5, "IndigoPlateau", "IndigoPlateauLobby")
    g.nav_to(7, 7, "IndigoPlateauLobby")
    g.face("up")
    g.tap("a", 16)
    g.dialogue_then_choice()
    g.choose("YES")
    assert g.cutscene()
    assert all(mon["hp"] == mon["max_hp"] for mon in g.st()["party"])
    g.nav_to(2, 5, "IndigoPlateauLobby")
    g.face("left")
    for item in ["Nugget", "Tm34", "Tm11"]:
        if any(v["item"] == item for v in g.d.cmd(cmd="get_bag")["data"]):
            g.tap("a", 16)
            sell(g, item)
    quantity = min(16, (g.st()["money"] - 6000) // 3000)
    assert quantity >= 8, g.st()["money"]
    # The m26 exploration preset already carries large stacks. Buy only the
    # amount that fits the current stack cap; crossing 99 would create a new
    # bag slot and can make the subsequent Revive purchase fail at the full
    # bag limit.
    full_restore = sum(v["qty"] for v in g.d.cmd(cmd="get_bag")["data"]
                       if v["item"] == "FullRestore")
    full_restore_buy = min(quantity, max(0, 99 - full_restore))
    if full_restore_buy:
        g.tap("a", 16)
        buy(g, "FullRestore", 2, full_restore_buy)
    revive = sum(v["qty"] for v in g.d.cmd(cmd="get_bag")["data"]
                 if v["item"] == "Revive")
    revive_buy = min(4, max(0, 99 - revive))
    if revive_buy:
        g.tap("a", 16)
        buy(g, "Revive", 5, revive_buy)
    lead_with(g, "Zapdos")
    g.evidence("m44")


def recover_party(g):
    for index, mon in enumerate(g.st()["party"]):
        if mon["hp"] == 0:
            use_item(g, "Revive", party_index=index)
        mon = g.st()["party"][index]
        if mon["hp"] < mon["max_hp"] or mon["status"] != "None":
            use_item(g, "FullRestore", party_index=index)


def elite_battle(g, map_name, text_id, flag, mid):
    talk_npc(g, map_name, text_id, completion_flag=flag)
    require_flag(g, flag)
    g.evidence(mid)


def m45_lorelei(g):
    g.nav_warp(8, 0, "IndigoPlateauLobby", "LoreleisRoom")
    elite_battle(g, "LoreleisRoom", 1, "EVENT_BEAT_LORELEIS_ROOM_TRAINER_0", "m45")


def m46_bruno(g):
    recover_party(g)
    g.nav_warp(4, 0, "LoreleisRoom", "BrunosRoom")
    elite_battle(g, "BrunosRoom", 1, "EVENT_BEAT_BRUNOS_ROOM_TRAINER_0", "m46")


def m47_agatha(g):
    recover_party(g)
    g.nav_warp(4, 0, "BrunosRoom", "AgathasRoom")
    elite_battle(g, "AgathasRoom", 1, "EVENT_BEAT_AGATHAS_ROOM_TRAINER_0", "m47")


def m48_lance(g):
    recover_party(g)
    g.nav_warp(4, 0, "AgathasRoom", "LancesRoom")
    elite_battle(g, "LancesRoom", 1, "EVENT_BEAT_LANCE", "m48")


def m49_first_clear(g):
    from playthrough import Game, resume_reentry
    recover_party(g)
    previous_count = g.st()["hall_of_fame_count"]
    g.nav_warp(5, 0, "LancesRoom", "ChampionsRoom")
    assert g.cutscene()
    g.wait("screen=battle", 900)
    g.battle_loop(max_iters=1600)
    seen_hof, seen_credits, final_button = False, False, False
    phases = []
    for _ in range(1200):
        s = g.st()
        if s.get("hof_phase"):
            seen_hof = True
        if s.get("credits_phase"):
            seen_credits = True
        signature = (s["screen"], s.get("hof_phase"), s.get("credits_phase"))
        if not phases or phases[-1]["phase"] != list(signature):
            phases.append({"frame": s["frame_count"], "phase": list(signature),
                           "hall_of_fame_count": s["hall_of_fame_count"]})
        if s.get("credits_final_button"):
            assert s["hall_of_fame_count"] == previous_count + 1
            final_button = True
            g.tap("a", 16)
        elif final_button and not s.get("credits_phase") and s["screen"] != "overworld":
            break
        elif s.get("dialogue_state"):
            g.skip()
        elif s["screen"] == "battle":
            raise RuntimeError("champion battle did not finish")
        else:
            g.step(120)
    else:
        raise RuntimeError("Hall of Fame / credits did not finish")
    assert seen_hof and seen_credits and final_button, (seen_hof, seen_credits, final_button)
    # Validate the game's own post-credits save in a separate fresh process.
    # No debug save is issued before this check.
    assert g.save_path.exists(), "credits did not write a save"
    check = Game(save_path=g.save_path)
    try:
        resume_reentry(check)
        restored = check.st()
        assert restored["map_name"] == "PalletTown", restored["map_name"]
        assert restored["badges"] == 255
        assert restored["hall_of_fame_count"] == previous_count + 1
    finally:
        check.close()
    g.first_clear_verification = {"phases": phases, "separate_process_continue": restored}
    resume_reentry(g)
    g.evidence("m49")


LATE_MILESTONES = [
    ("m11", "Route 3 trainers → Mt Moon entrance", m11_mt_moon_entrance),
    ("m12", "Mt Moon fossil → Cerulean City", m12_mt_moon),
    ("m13", "Cascade Badge (Misty)", m13_misty),
    ("m14", "Nugget Bridge → Bill → S.S. Ticket", m14_bill),
    ("m15", "Cerulean thief → Underground Path → Vermilion", m15_vermilion),
    ("m16", "S.S. Anne rival + captain → HM01", m16_ss_anne),
    ("m17", "CUT + trash switches → Thunder Badge", m17_surge),
    ("m18", "Route 9 CUT → Rock Tunnel entrance", m18_rock_tunnel_entrance),
    ("m19", "Rock Tunnel → Lavender Town", m19_rock_tunnel),
    ("m20", "Lavender → Underground Path → Celadon", m20_celadon),
    ("m21", "Rainbow Badge (Erika)", m21_erika),
    ("m22", "Game Corner poster → Rocket Hideout Lift Key", m22_lift_key),
    ("m23", "Rocket elevator + Giovanni → Silph Scope", m23_silph_scope),
    ("m24", "Pokemon Tower → purified healing zone", m24_tower),
    ("m25", "Marowak + Fuji rescue → Poke Flute", m25_poke_flute),
    ("m26", "Poke Flute + Route 12 Snorlax → Fuchsia", m26_fuchsia),
    ("m27", "Invisible wall maze → Soul Badge", m27_koga),
    ("m28", "Safari Zone fee + Gold Teeth + HM03", m28_safari),
    ("m29", "Safari timeout + Warden → HM04 Strength", m29_strength),
    ("m30", "Celadon roof drink → Saffron guards", m30_saffron),
    ("m31", "Silph Co fifth floor → Card Key", m31_card_key),
    ("m32", "Silph rival + Lapras gift + Surf", m32_lapras),
    ("m33", "Silph Giovanni + president → Master Ball", m33_silph_giovanni),
    ("m34", "Saffron teleport maze → Marsh Badge", m34_sabrina),
    ("m35", "Route 16 CUT + hidden house → HM02", m35_fly_hm),
    ("m36", "Surf to Power Plant + Master Ball Zapdos", m36_zapdos),
    ("m37", "Fly to Pallet + Route 21 Surf → Cinnabar", m37_cinnabar),
    ("m38", "Mansion statues + floor hole → Secret Key", m38_secret_key),
    ("m39", "Cinnabar quiz machines → Volcano Badge", m39_blaine),
    ("m40", "Viridian spinner gym → eighth badge", m40_giovanni_badge),
    ("m41", "Route 22 rival + eight badge gates → Victory Road", m41_victory_road_entrance),
    ("m42", "Victory Road Strength → first two switches", m42_victory_first_switch),
    ("m43", "Victory Road upper switch + boulder hole", m43_victory_upper_switch),
    ("m44", "Victory Road exit + Indigo healing and supplies", m44_indigo),
    ("m45", "Elite Four: Lorelei", m45_lorelei),
    ("m46", "Elite Four: Bruno", m46_bruno),
    ("m47", "Elite Four: Agatha", m47_agatha),
    ("m48", "Elite Four: Lance", m48_lance),
    ("m49", "Champion → Hall of Fame → credits → saved Continue", m49_first_clear),
]
