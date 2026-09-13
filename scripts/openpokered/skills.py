"""Skill actions for openpokered (M6): reusable compositions of the semantic
debug commands. Behavior reference: `scripts/playthrough.py`
(`battle_loop`, `heal_pokecenter`, `train_until`) — ported onto the
typed `AgentClient`, NOT reimplemented in Rust.
"""

# Preferred damaging moves, STAB/typed damage first (playthrough.py's order).
PREFERRED_MOVES = ["VineWhip", "Ember", "Bubble", "WaterGun",
                   "ThunderShock", "Absorb", "RazorLeaf", "Tackle",
                   "Scratch", "Pound", "PsychicM", "Psychic", "Swift"]


def tap(client, btn, gap=8):
    """One held frame + idle frames (menus are edge-triggered)."""
    client.drive([btn], frames=1 + gap)


def skip_dialogue(client):
    return client.skip_dialogue()


def _preferred_slot(moves):
    for want in PREFERRED_MOVES:
        for i, m in enumerate(moves):
            if m["move"] == want and m["pp"] > 0 and not m["disabled"]:
                return i
    for i, m in enumerate(moves):
        if m["pp"] > 0 and not m["disabled"]:
            return i
    return None


def _select_move(client):
    """Closed-loop FIGHT-menu selection (playthrough.py _select_move)."""
    for _ in range(24):
        s = client.state()
        if s["screen"] != "battle" or s["battle_phase"] != "MoveSelect":
            return
        menu = s.get("battle_moves")
        if not menu:
            client.step(4)
            continue
        moves = menu["moves"]
        want = _preferred_slot(moves)
        if want is None:
            tap(client, "b")
            client.step(10)
            return
        n = len(moves)
        cur = menu["cursor"]
        if cur != want:
            delta = (want - cur) % n
            up = delta * 2 > n
            for _ in range(n - delta if up else delta):
                client.drive(["up" if up else "down"], frames=10)
            continue
        tap(client, "a", 4)
        client.step(10)


def battle_loop(client, prefer="fight", max_iters=400, on_round=None):
    """Generic battle driver (playthrough.py battle_loop port).
    prefer="run" picks RUN from the menu and falls back to FIGHT if
    escape keeps failing. Returns True when the battle ends."""
    fight = prefer == "fight"
    iters_in_mode = 0
    for it in range(max_iters):
        s = client.state()
        if on_round is not None:
            on_round(it, s)
        if not fight and "Trainer" in s["battle_phase"]:
            fight = True
        if s["screen"] != "battle":
            break
        ph = s["battle_phase"]
        if ph == "PlayerMenu":
            if not fight and iters_in_mode > 3:
                fight = True
            iters_in_mode += 1
            if fight:
                client.drive(["up", "left"], frames=10)   # -> FIGHT
                tap(client, "a", 4)
                # Confirm phase moved on (text may have eaten the press).
                for _ in range(30):
                    s2 = client.state()
                    if s2["battle_phase"] == "MoveSelect" or s2["screen"] != "battle":
                        break
                    client.step(4)
                _select_move(client)
            else:
                client.drive(["down", "right"], frames=10)  # -> RUN
                tap(client, "a", 4)
            client.step(30)
        elif ph == "MoveSelect":
            _select_move(client)
            client.step(30)
        elif ph == "ShiftPrompt":
            tap(client, "a")   # default cursor is NO
            client.step(30)
        else:
            # Intro/ShowingText/TrainerVictory pages.
            tap(client, "a", 10)
    client.wait_until("not_battle", 1800)
    return True


def heal_at_pokemon_center(client, city_map, pc_map="Pokecenter"):
    """Travel to a city's Pokécenter, heal at the nurse (YES), exit.
    `pc_map` is the interior map name as used by the world graph warps
    (e.g. ViridianPokecenter, PewterPokecenter)."""
    client.travel_to(city_map)
    # Enter the center via the world graph's warp edge.
    edges = client.world_graph(maps=[city_map])["edges"]
    warp = next(e for e in edges
                if e["kind"] == "warp" and e["to_map"] == pc_map)
    out = client.move_to(warp["from_pos"]["x"], warp["from_pos"]["y"])
    if out["result"] != "map_changed":
        raise RuntimeError(f"pokecenter warp did not fire: {out['result']}")
    # Nurse: stand across the counter and talk up (playthrough pattern).
    client.move_to(3, 3)
    # Face the counter and press A; answer YES to the heal prompt.
    client.drive(["up"], frames=10)
    out = client.interact()
    for _ in range(40):
        s = client.state()
        if s["choice"] is not None:
            # Default cursor is YES — confirm.
            tap(client, "a", 10)
            break
        if s["dialogue_state"] is not None:
            client.skip_dialogue()
        else:
            client.step(20)
    client.wait_until("control_ready", 900)
    party = client.state()["party"]
    assert all(m["hp"] == m["max_hp"] for m in party), party
    # Exit the center (south door warp).
    for w in client.world_graph(maps=[pc_map])["edges"]:
        if w["kind"] == "warp" and w["to_map"] == city_map:
            client.move_to(w["from_pos"]["x"], w["from_pos"]["y"])
            break
    client.wait_until("control_ready", 600)


def win_wild_battle(client, grass_walk, prefer="fight", max_rounds=60):
    """Wander grass until a wild battle starts, then win it.
    `grass_walk` is a list of (x, y) tiles to pace through (task/oracle
    picks them per route). Returns after the battle is resolved."""
    for _ in range(max_rounds):
        obs = client.observe()
        if obs["mode"] == "battle":
            battle_loop(client, prefer=prefer)
            return True
        for (x, y) in grass_walk:
            out = client.move_to(x, y)
            if out["result"] == "entered_battle":
                battle_loop(client, prefer=prefer)
                return True
            if out["result"] != "reached":
                break
    raise RuntimeError("no wild encounter within the round budget")


def fight_current_battle(client, prefer="fight"):
    """Resolve whatever battle/dialogue is currently in flight."""
    for _ in range(30):
        obs = client.observe()
        if obs["mode"] == "battle":
            battle_loop(client, prefer=prefer)
            return True
        if obs["mode"] == "dialogue":
            client.skip_dialogue()
        else:
            client.step(10)
    return False
