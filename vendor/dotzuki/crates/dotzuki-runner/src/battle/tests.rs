//! Unit tests for the generic battle: the formula (exact numbers under a
//! scripted rng), the turn loop (win/lose paths, speed order, MP gate), and
//! v2-b parties (switching, forced replacement, persistent state) and items.

use super::*;

// ── builders ────────────────────────────────────────────────────────────────

fn skill(name: &str, power: u32, accuracy: u32, category: SkillCategory, cost: u32) -> Skill {
    Skill {
        id: name.to_lowercase(),
        name: name.to_string(),
        power,
        accuracy,
        element: None,
        category,
        stat: "attack".to_string(),
        cost,
        ron: false,
    }
}

fn combatant(
    name: &str,
    hp: u32,
    atk: u32,
    def: u32,
    spd: u32,
    mp: u32,
    skills: Vec<Skill>,
) -> Combatant {
    Combatant {
        id: name.to_lowercase(),
        name: name.to_string(),
        element: None,
        hp,
        max_hp: hp,
        attack: atk,
        defense: def,
        speed: spd,
        level: 1,
        base: BaseStats {
            max_hp: hp,
            max_mp: mp,
            attack: atk,
            defense: def,
            speed: spd,
        },
        exp: 0,
        exp_reward: 0,
        stages: Stages::default(),
        mp,
        max_mp: mp,
        skills,
        status: None,
        ability: None,
        held_item: None,
    }
}

/// The canonical win-path pair: Aria (fast) vs Slime.
fn aria_vs_slime() -> (Combatant, Combatant) {
    let aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
    );
    let slime = combatant(
        "Slime",
        90,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 100, SkillCategory::Damage, 0)],
    );
    (aria, slime)
}

/// Aria's mage teammate (same move list, distinct stats).
fn bryn() -> Combatant {
    combatant(
        "Bryn",
        80,
        14,
        12,
        10,
        60,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
    )
}

/// A two-member party battle (Aria leads) against the Slime.
fn party_battle(bytes: &[u8]) -> Battle {
    let (aria, slime) = aria_vs_slime();
    Battle::full(
        vec![aria, bryn()],
        0,
        slime,
        Vec::new(),
        HashMap::new(),
        TypeChart::default(),
        scripted(bytes),
        None,
    )
}

fn scripted(bytes: &[u8]) -> Box<dyn BattleRng> {
    Box::new(ScriptedRng::new(bytes.to_vec()))
}

// ── input drivers ───────────────────────────────────────────────────────────

fn press(b: &mut Battle, mask: u8) {
    let mut input = InputState::new();
    input.set_from_bitmask(mask);
    b.update(&input);
}

/// A single A press (fresh InputState ⇒ just-pressed).
fn confirm(b: &mut Battle) {
    press(b, GbButton::A.bit_mask());
}

/// Root → Fight, then confirm skill `pick`.
fn fight(b: &mut Battle, pick: usize) {
    confirm(b); // Root: Fight
    for _ in 0..pick {
        press(b, GbButton::Down.bit_mask());
    }
    confirm(b);
}

/// Root → Party, then confirm member `idx` (a voluntary switch).
fn party_switch(b: &mut Battle, idx: usize) {
    press(b, GbButton::Down.bit_mask()); // Root: Party
    confirm(b);
    // The party cursor starts on the first switchable member.
    let start = b.first_switchable();
    for _ in 0..(idx + b.party().len() - start) % b.party().len().max(1) {
        press(b, GbButton::Down.bit_mask());
    }
    confirm(b);
}

/// Root → Item, then confirm usable item `pick`.
fn item_use(b: &mut Battle, pick: usize) {
    press(b, GbButton::Down.bit_mask());
    press(b, GbButton::Down.bit_mask()); // Root: Item
    confirm(b);
    for _ in 0..pick {
        press(b, GbButton::Down.bit_mask());
    }
    confirm(b);
}

/// Advance narration until a menu returns or the battle ends (bounded).
fn settle(b: &mut Battle) {
    for _ in 0..40 {
        if b.outcome().is_some() || b.in_menu() {
            return;
        }
        confirm(b);
    }
    panic!("battle did not settle after 40 A presses");
}

/// Play rounds (always picking skill `pick`) until the battle ends (bounded).
fn play_to_end(b: &mut Battle, pick: usize) {
    for _ in 0..40 {
        if b.outcome().is_some() {
            return;
        }
        fight(b, pick);
        settle(b);
    }
    panic!("battle did not end after 40 rounds");
}

// ── formula ─────────────────────────────────────────────────────────────────

#[test]
fn stage_multipliers() {
    assert_eq!(stage_multiplier(12, 0), 12);
    assert_eq!(stage_multiplier(12, 1), 15, "+1 = ×1.25");
    assert_eq!(stage_multiplier(12, 4), 24, "+4 = ×2");
    assert_eq!(stage_multiplier(12, -1), 9, "-1 = ×0.8 (12×4/5)");
    assert_eq!(stage_multiplier(12, -4), 6, "-4 = ×0.5");
    // Out-of-range stages clamp.
    assert_eq!(stage_multiplier(12, 9), 24);
    assert_eq!(stage_multiplier(12, -9), 6);
}

#[test]
fn damage_roll_exact_numbers() {
    // base = 40×12/8 = 60; variance byte 100 ⇒ ×(85+100%16)/100 = ×89/100 = 53.
    let roll = damage_roll(40, 12, 8, (1, 1), &mut *scripted(&[100, 1]));
    assert_eq!(roll.damage, 53);
    assert!(!roll.crit);

    // Crit byte 0 ⇒ ×3/2: 53×3/2 = 79.
    let roll = damage_roll(40, 12, 8, (1, 1), &mut *scripted(&[100, 0]));
    assert_eq!(roll.damage, 79);
    assert!(roll.crit);

    // Super-effective ×2: 106; resisted ×1/2: 26.
    let roll = damage_roll(40, 12, 8, (2, 1), &mut *scripted(&[100, 1]));
    assert_eq!(roll.damage, 106);
    let roll = damage_roll(40, 12, 8, (1, 2), &mut *scripted(&[100, 1]));
    assert_eq!(roll.damage, 26);

    // Min variance (byte 0 ⇒ ×85/100): 60×85/100 = 51; crit also (0%16==0):
    // 51×3/2 = 76 — one byte stream, byte 0 plays both roles.
    let roll = damage_roll(40, 12, 8, (1, 1), &mut *scripted(&[0]));
    assert_eq!(roll.damage, 76);
    assert!(roll.crit);

    // Zero power still floors at 1.
    let roll = damage_roll(0, 12, 8, (1, 1), &mut *scripted(&[100, 1]));
    assert_eq!(roll.damage, 1);
}

#[test]
fn accuracy_roll_threshold() {
    // byte % 100 < accuracy.
    assert!(accuracy_roll(100, &mut *scripted(&[99])));
    assert!(accuracy_roll(50, &mut *scripted(&[149]))); // 149%100=49 < 50
    assert!(!accuracy_roll(50, &mut *scripted(&[50])));
    assert!(!accuracy_roll(0, &mut *scripted(&[0])));
}

#[test]
fn type_chart_from_ruleset() {
    let ruleset = dotzuki_rules::Ruleset::from_ron(
        r#"Ruleset(
            types: ["fire", "grass"],
            type_chart: [
                (atk: "fire", def: "grass", mult: [2, 1]),
                (atk: "grass", def: "fire", mult: [1, 2]),
            ],
        )"#,
    )
    .expect("parse ruleset");
    let chart = TypeChart::from_ruleset(&ruleset);
    assert_eq!(chart.mult(Some("fire"), Some("grass")), (2, 1));
    assert_eq!(chart.mult(Some("grass"), Some("fire")), (1, 2));
    assert_eq!(
        chart.mult(Some("Fire"), Some("GRASS")),
        (2, 1),
        "case-insensitive"
    );
    assert_eq!(
        chart.mult(Some("water"), Some("fire")),
        (1, 1),
        "no edge = neutral"
    );
    assert_eq!(
        chart.mult(None, Some("fire")),
        (1, 1),
        "untyped attack = neutral"
    );
    assert_eq!(
        chart.mult(Some("fire"), None),
        (1, 1),
        "untyped defender = neutral"
    );
}

// ── turn loop ───────────────────────────────────────────────────────────────

#[test]
fn win_path_narrates_and_ends() {
    let (aria, slime) = aria_vs_slime();
    // Per action: [acc=50 (hit), var=100 (89%), crit=1 (no)].
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50, 100, 1]));

    // Round 1: Aria 53 dmg (90→37), Slime 28 dmg (60→32).
    fight(&mut b, 0);
    assert_eq!(b.current_line(), Some("Aria used Slash!"));
    settle(&mut b);
    assert_eq!(b.enemy().hp, 37);
    assert_eq!(b.player().hp, 32);
    assert!(b.in_menu(), "round over, back to the menu");
    assert!(b.outcome().is_none());

    // Round 2: Aria 53 dmg → Slime faints → win.
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.outcome(), Some(BattleOutcome::Win));
    assert_eq!(
        b.log(),
        &[
            "Aria used Slash!",
            "53 damage!",
            "Slime used Tackle!",
            "28 damage!",
            "Aria used Slash!",
            "53 damage!",
            "Slime fainted!",
            "You won the battle!",
        ]
    );
}

#[test]
fn lose_path_narrates_and_ends() {
    let weak = combatant(
        "Aria",
        20,
        5,
        5,
        15,
        0,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
    );
    let strong = combatant(
        "Golem",
        200,
        30,
        10,
        5,
        0,
        vec![skill("Smash", 40, 100, SkillCategory::Damage, 0)],
    );
    let mut b = Battle::new(weak, strong, TypeChart::default(), scripted(&[50, 100, 1]));

    play_to_end(&mut b, 0);
    assert_eq!(b.outcome(), Some(BattleOutcome::Lose));
    assert!(b.log().contains(&"Aria fainted!".to_string()));
    assert_eq!(b.log().last(), Some(&"You lost the battle…".to_string()));
}

#[test]
fn faster_enemy_acts_first() {
    let (aria, mut slime) = aria_vs_slime();
    slime.speed = 20; // 20 > 15: the enemy moves first.
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50, 100, 1]));
    fight(&mut b, 0);
    assert_eq!(b.current_line(), Some("Slime used Tackle!"));
}

#[test]
fn speed_tie_goes_to_the_player() {
    let (aria, mut slime) = aria_vs_slime();
    slime.speed = 15; // tie → player first.
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50, 100, 1]));
    fight(&mut b, 0);
    assert_eq!(b.current_line(), Some("Aria used Slash!"));
}

#[test]
fn mp_gate_blocks_unaffordable_player_skill() {
    let mut aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        3, // only 3 MP: Fire Bolt costs 5
        vec![
            skill("Slash", 40, 100, SkillCategory::Damage, 0),
            skill("Fire Bolt", 50, 100, SkillCategory::Damage, 5),
        ],
    );
    aria.max_mp = 3;
    let slime = combatant(
        "Slime",
        90,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 100, SkillCategory::Damage, 0)],
    );
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50, 100, 1]));

    // The root menu opens first; Fight reveals the skills.
    assert_eq!(
        b.menu_items(),
        vec!["Fight".to_string(), "Party".to_string(), "Run".to_string()]
    );
    confirm(&mut b);
    assert_eq!(
        b.menu_items(),
        vec!["Slash".to_string(), "× Fire Bolt 5MP".to_string()]
    );

    // Cursor onto Fire Bolt; A must NOT start a round.
    press(&mut b, GbButton::Down.bit_mask());
    confirm(&mut b);
    assert!(b.in_menu(), "unaffordable skill is unselectable");
    assert!(b.log().is_empty());

    // Back to Slash: the round runs and spends no MP.
    press(&mut b, GbButton::Up.bit_mask());
    confirm(&mut b);
    settle(&mut b);
    assert_eq!(b.player().mp, 3);
}

#[test]
fn mp_is_spent_per_use() {
    let mut aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        12,
        vec![skill("Fire Bolt", 50, 100, SkillCategory::Damage, 5)],
    );
    aria.max_mp = 12;
    let slime = combatant(
        "Slime",
        900,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 100, SkillCategory::Damage, 0)],
    );
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50, 100, 1]));
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.player().mp, 7);
}

#[test]
fn enemy_falls_back_to_basic_attack_when_broke() {
    let (aria, mut slime) = aria_vs_slime();
    slime.skills = vec![skill("Fire Bolt", 50, 100, SkillCategory::Damage, 5)];
    slime.mp = 0; // can't afford its only skill
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50, 100, 1]));
    play_to_end(&mut b, 0);
    assert!(
        b.log().contains(&"Slime used Attack!".to_string()),
        "broke enemy uses the built-in Attack: {:?}",
        b.log()
    );
}

#[test]
fn buff_raises_attack_stage_and_damage() {
    let aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![
            skill("Slash", 40, 100, SkillCategory::Damage, 0),
            skill("Focus", 0, 100, SkillCategory::Buff, 0),
        ],
    );
    let slime = combatant(
        "Slime",
        900,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 100, SkillCategory::Damage, 0)],
    );
    // Focus consumes 1 byte (acc), Tackle 3, Slash 3.
    let mut b = Battle::new(
        aria,
        slime,
        TypeChart::default(),
        scripted(&[50, 50, 100, 1, 50, 100, 1]),
    );

    // Round 1: Focus (buff) → "Aria's Attack rose!"; Tackle hits back.
    fight(&mut b, 1);
    settle(&mut b);
    assert_eq!(b.player().stages.attack, 1);
    assert!(b.log().contains(&"Aria's Attack rose!".to_string()));

    // Round 2: Slash at stage +1: eff atk = 12×5/4 = 15 → base 40×15/8 = 75
    // → ×89/100 = 66.
    fight(&mut b, 0);
    settle(&mut b);
    assert!(b.log().contains(&"66 damage!".to_string()), "{:?}", b.log());
}

#[test]
fn debuff_lowers_defense_stage() {
    let mut weaken = skill("Weaken", 0, 100, SkillCategory::Debuff, 0);
    weaken.stat = "defense".to_string();
    let aria = combatant("Aria", 60, 12, 10, 15, 20, vec![weaken]);
    let slime = combatant(
        "Slime",
        90,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 100, SkillCategory::Damage, 0)],
    );
    let mut b = Battle::new(
        aria,
        slime,
        TypeChart::default(),
        scripted(&[50, 50, 100, 1]),
    );
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.enemy().stages.defense, -1);
    assert!(b.log().contains(&"Slime's Defense fell!".to_string()));
    // eff def at −1: 8×4/5 = 6.
    assert_eq!(b.enemy().eff_defense(), 6);
}

#[test]
fn heal_restores_up_to_the_cap() {
    let mut aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![skill("Heal", 25, 100, SkillCategory::Heal, 0)],
    );
    aria.hp = 30; // mid-battle damage
    let slime = combatant(
        "Slime",
        90,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 0, SkillCategory::Damage, 0)],
    );
    // Slime has 0 accuracy: it always misses (acc byte 50).
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50]));

    // Heal 25: 30 → 55. Then the slime misses.
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.player().hp, 55);
    assert!(b.log().contains(&"Aria recovered 25 HP!".to_string()));
    assert!(b.log().contains(&"But it missed!".to_string()));

    // Heal again: capped at max (55 → 60, recovers 5).
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.player().hp, 60);
    assert!(b.log().contains(&"Aria recovered 5 HP!".to_string()));
}

#[test]
fn effectiveness_is_narrated() {
    let mut bolt = skill("Fire Bolt", 50, 100, SkillCategory::Damage, 0);
    bolt.element = Some("fire".to_string());
    let mut aria = combatant("Aria", 60, 12, 10, 15, 20, vec![bolt]);
    aria.element = Some("grass".to_string());
    let mut slime = combatant(
        "Slime",
        900,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 100, SkillCategory::Damage, 0)],
    );
    slime.element = Some("grass".to_string());

    let ruleset = dotzuki_rules::Ruleset::from_ron(
        r#"Ruleset(
            types: ["fire", "grass"],
            type_chart: [(atk: "fire", def: "grass", mult: [2, 1])],
        )"#,
    )
    .unwrap();
    let chart = TypeChart::from_ruleset(&ruleset);
    let mut b = Battle::new(aria, slime, chart, scripted(&[50, 100, 1]));

    // Fire Bolt vs grass Slime: base 50×12/8 = 75 → ×89/100 = 66 → ×2 = 132.
    fight(&mut b, 0);
    settle(&mut b);
    assert!(b.log().contains(&"It's super effective!".to_string()));
    assert!(
        b.log().contains(&"132 damage!".to_string()),
        "{:?}",
        b.log()
    );
}

// ── parties: switching (v2-b) ───────────────────────────────────────────────

#[test]
fn switch_consumes_the_turn_and_resets_stages() {
    let aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![
            skill("Slash", 40, 100, SkillCategory::Damage, 0),
            skill("Focus", 0, 100, SkillCategory::Buff, 0),
        ],
    );
    let slime = combatant(
        "Slime",
        900,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 100, SkillCategory::Damage, 0)],
    );
    let mut b = Battle::full(
        vec![aria, bryn()],
        0,
        slime,
        Vec::new(),
        HashMap::new(),
        TypeChart::default(),
        // Focus 1 byte, Tackle 3; then Tackle 3 per switch round; Slash 3.
        scripted(&[50, 50, 100, 1, 50, 100, 1, 50, 100, 1, 50, 100, 1]),
        None,
    );

    // Round 1: Focus → Aria +1 Attack.
    fight(&mut b, 1);
    settle(&mut b);
    assert_eq!(b.player().stages.attack, 1);
    let aria_hp = b.player().hp;

    // Round 2: Party → Bryn. The switch consumes the turn: the Slime hits
    // BRYN (the new member), Aria is untouched.
    party_switch(&mut b, 1);
    assert_eq!(b.current_line(), Some("Come back, Aria!"));
    settle(&mut b);
    assert_eq!(b.active_index(), 1);
    assert_eq!(b.player().name, "Bryn");
    // Tackle on Bryn: 40×8/12 = 26 → ×89/100 = 23 (80→57).
    assert_eq!(b.party()[1].hp, 57, "{:?}", b.log());
    assert_eq!(b.party()[0].hp, aria_hp, "benched Aria took no damage");
    assert!(b.log().contains(&"Go, Bryn!".to_string()));

    // Round 3: switch back — Aria's stages reset on switch-in.
    party_switch(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.active_index(), 0);
    assert_eq!(b.player().stages.attack, 0, "stages reset on switch-in");
    assert_eq!(b.player().eff_attack(), 12);
}

#[test]
fn forced_switch_on_faint_and_win_still_possible() {
    // Aria at 1 HP faints to the first Tackle; Bryn avenges her.
    let mut aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
    );
    aria.hp = 1;
    let mut strong_bryn = bryn();
    strong_bryn.attack = 200; // one-hit KO
    let slime = combatant(
        "Slime",
        90,
        8,
        8,
        20,
        0,
        vec![skill("Tackle", 40, 100, SkillCategory::Damage, 0)],
    );
    // Slime (spd 20) outspeeds Aria → Tackle first: 40×8/10 = 32 → ×89/100 = 28 ≥ 1.
    let mut b = Battle::full(
        vec![aria, strong_bryn],
        0,
        slime,
        Vec::new(),
        HashMap::new(),
        TypeChart::default(),
        scripted(&[50, 100, 1]),
        None,
    );

    // Aria's pick is moot — the Slime strikes first and she faints; the
    // forced-switch phase replaces the menu.
    fight(&mut b, 0);
    settle(&mut b);
    assert!(b.log().contains(&"Aria fainted!".to_string()));
    assert!(b.outcome().is_none(), "Bryn still stands");
    assert!(matches!(b.phase, Phase::ForcedSwitch));
    // The active and fainted members are unselectable.
    assert_eq!(
        b.menu_items(),
        vec!["× Aria 0/60".to_string(), "Bryn 80/80".to_string()]
    );

    // Pick Bryn (the cursor starts on him): free action, then the round is
    // over (the Slime already acted).
    confirm(&mut b);
    settle(&mut b);
    assert_eq!(b.player().name, "Bryn");
    assert_eq!(b.player().hp, 80, "the Slime had already acted");

    // Bryn one-shots the Slime (200 atk ⇒ damage ≥ 90).
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.outcome(), Some(BattleOutcome::Win), "{:?}", b.log());
}

#[test]
fn all_members_fainted_is_a_loss() {
    let weak = |name: &str| {
        combatant(
            name,
            10,
            5,
            5,
            15,
            0,
            vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
        )
    };
    let golem = combatant(
        "Golem",
        500,
        30,
        10,
        20,
        0,
        vec![skill("Smash", 40, 100, SkillCategory::Damage, 0)],
    );
    let mut b = Battle::full(
        vec![weak("Aria"), weak("Bryn")],
        0,
        golem,
        Vec::new(),
        HashMap::new(),
        TypeChart::default(),
        scripted(&[50, 100, 1]),
        None,
    );

    // Aria faints → forced switch to Bryn → Bryn faints → lose (no menu).
    fight(&mut b, 0);
    settle(&mut b);
    assert!(matches!(b.phase, Phase::ForcedSwitch));
    confirm(&mut b); // the only legal pick: Bryn
    settle(&mut b);
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.outcome(), Some(BattleOutcome::Lose), "{:?}", b.log());
    assert_eq!(b.log().last(), Some(&"You lost the battle…".to_string()));
}

#[test]
fn party_state_reflects_the_battle_for_write_back() {
    let mut b = party_battle(&[50, 100, 1]);
    fight(&mut b, 0);
    settle(&mut b);
    let state = b.party_state();
    assert_eq!(state.len(), 2);
    // Aria took the Slime's 28 damage; Bryn is untouched at full HP.
    assert_eq!((state[0].id.as_str(), state[0].hp), ("aria", 32));
    assert_eq!(
        (state[1].id.as_str(), state[1].hp, state[1].mp),
        ("bryn", 80, 60)
    );
    assert_eq!(state[0].status, None);
}

// ── items (v2-b) ────────────────────────────────────────────────────────────

fn potion() -> BattleItem {
    BattleItem {
        id: "potion".to_string(),
        name: "Potion".to_string(),
        heal: 50,
    }
}

/// Aria (hurt) vs a always-missing Slime, with `count` Potions in the bag.
fn item_battle(hp: u32, count: u32) -> Battle {
    let mut aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
    );
    aria.hp = hp;
    let slime = combatant(
        "Slime",
        900,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 0, SkillCategory::Damage, 0)],
    );
    Battle::full(
        vec![aria],
        0,
        slime,
        vec![potion()],
        HashMap::from([("potion".to_string(), count)]),
        TypeChart::default(),
        scripted(&[50]),
        None,
    )
}

#[test]
fn item_heals_caps_decrements_and_consumes_the_turn() {
    let mut b = item_battle(30, 2);
    // The root menu gains Item when usable items exist.
    assert_eq!(
        b.menu_items(),
        vec![
            "Fight".to_string(),
            "Party".to_string(),
            "Item".to_string(),
            "Run".to_string()
        ]
    );

    // Potion on 30/60: capped at max ⇒ recovers 30; count 2 → 1; the Slime
    // still gets its turn (it misses, accuracy 0).
    item_use(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.player().hp, 60);
    assert_eq!(b.inventory().get("potion"), Some(&1));
    assert_eq!(
        b.log(),
        &[
            "Aria used Potion!",
            "Aria recovered 30 HP!",
            "Slime used Tackle!",
            "But it missed!",
        ]
    );
}

#[test]
fn item_disappears_at_zero_count() {
    let mut b = item_battle(10, 1);
    item_use(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.player().hp, 60);
    assert_eq!(
        b.inventory().get("potion"),
        None,
        "count 0 removes the entry"
    );

    // The Item menu is now empty (the root Item entry remains, harmlessly).
    press(&mut b, GbButton::Down.bit_mask());
    press(&mut b, GbButton::Down.bit_mask());
    confirm(&mut b);
    assert!(b.menu_items().is_empty(), "count-0 items are not listed");
}

// ── RON effect hooks (v2-a) ─────────────────────────────────────────────────

/// A RON-taken-over skill (id matches the record id).
fn ron_skill(name: &str, power: u32, cost: u32) -> Skill {
    let mut s = skill(name, power, 100, SkillCategory::Damage, cost);
    s.ron = true;
    s
}

/// The canonical poison ruleset: `venom-sting` (15 power, 30% poison on hit)
/// + the `poison` status (residual chip of 1/8 max HP).
const POISON_RULES: &str = r#"Ruleset(
    stats: ["hp", "attack", "defense", "speed"],
    types: [],
    resources: ["mp"],
    effects: [
        Effect(id: "venom-sting", kind: Move, power: 15, accuracy: 100, hooks: [
            Hook(on: "DamagingHit", chance: [30, 100], do: [
                InflictStatus(status: "poison", target: Target),
            ]),
        ]),
        Effect(id: "poison", kind: Status, hooks: [
            Hook(on: "Residual", do: [
                DamageFraction(num: 1, den: 8, of: MaxHp, target: Target),
            ]),
        ]),
    ],
)"#;

/// Build a battle with the hook machinery installed from a ruleset text.
/// Both combatants get an MP pool (`has_resource`).
fn ron_battle(player: Combatant, enemy: Combatant, rules: &str, bytes: &[u8]) -> Battle {
    let ruleset = dotzuki_rules::Ruleset::from_ron(rules).expect("parse ruleset");
    let compiled = hooks::compile_ruleset(&ruleset).expect("compile ruleset");
    hooks::install_compiled(compiled.clone());
    let registry = compiled.build_effects::<GenericProvider>();
    let stat_names = ruleset.stats.clone();
    let status_names = hooks::status_names(&ruleset);
    let state = HookState {
        state: BattleState::new(
            vec![hooks::mirror_of(&player, &stat_names, &status_names, true)],
            vec![hooks::mirror_of(&enemy, &stat_names, &status_names, true)],
        ),
        effects: Vec::new(),
        mv: dotzuki_engine::battle::stack::MoveContext::default(),
        registry,
        move_records: hooks::ron_moves(&ruleset, Some("mp")),
        status_names,
        stat_names,
        has_resource: true,
    };
    let chart = TypeChart::from_ruleset(&ruleset);
    Battle::with_hooks(player, enemy, chart, scripted(bytes), Some(state))
}

fn venom_aria() -> Combatant {
    combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![ron_skill("venom-sting", 15, 0)],
    )
}

fn tackle_slime(hp: u32) -> Combatant {
    combatant(
        "Slime",
        hp,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 100, SkillCategory::Damage, 0)],
    )
}

#[test]
fn ron_poison_inflicts_and_chips_exact_hp() {
    // Per round: Aria [acc, var, crit, poison-chance], Slime [acc, var, crit].
    let mut b = ron_battle(
        venom_aria(),
        tackle_slime(96),
        POISON_RULES,
        &[50, 100, 1, 29, 50, 100, 1],
    );

    // Round 1: venom-sting 15×12/8 = 22 → ×89/100 = 19 dmg (96→77); chance
    // byte 29 < 30 ⇒ poison; Tackle 40×8/10 = 32 → ×89/100 = 28 (60→32);
    // the chip follows Slime's action: 96/8 = 12 (77→65).
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.enemy().hp, 65, "{:?}", b.log());
    assert_eq!(b.player().hp, 32);
    assert_eq!(
        b.hooks().unwrap().battler(Side::Enemy).status,
        Some(hooks::StatusId(0)),
    );
    assert_eq!(
        b.log(),
        &[
            "Aria used venom-sting!",
            "19 damage!",
            "Slime was afflicted with poison!",
            "Slime used Tackle!",
            "28 damage!",
            "Slime is hurt by poison!",
        ]
    );

    // Round 2: same numbers again — 65−19−12 = 34; 32−28 = 4. Re-inflicting
    // an already-held status narrates nothing new.
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.enemy().hp, 34);
    assert_eq!(b.player().hp, 4);
    assert_eq!(
        b.log()
            .iter()
            .filter(|l| l.as_str() == "Slime was afflicted with poison!")
            .count(),
        1,
        "{:?}",
        b.log()
    );
}

#[test]
fn ron_chance_gate_compares_the_rng_byte() {
    // chance [30, 100]: byte 30 ⇒ 30 % 100 = 30, not < 30 ⇒ no poison…
    let mut b = ron_battle(
        venom_aria(),
        tackle_slime(96),
        POISON_RULES,
        &[50, 100, 1, 30, 50, 100, 1],
    );
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.hooks().unwrap().battler(Side::Enemy).status, None);
    assert!(
        !b.log().iter().any(|l| l.contains("afflicted")),
        "{:?}",
        b.log()
    );

    // …byte 29 < 30 ⇒ the poison lands.
    let mut b = ron_battle(
        venom_aria(),
        tackle_slime(96),
        POISON_RULES,
        &[50, 100, 1, 29, 50, 100, 1],
    );
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(
        b.hooks().unwrap().battler(Side::Enemy).status,
        Some(hooks::StatusId(0)),
    );
}

#[test]
fn ron_boost_matches_the_builtin_buff() {
    let rules = r#"Ruleset(
        stats: ["hp", "attack", "defense", "speed"],
        resources: ["mp"],
        effects: [
            Effect(id: "focus", kind: Move, power: 0, hooks: [
                Hook(on: "DamagingHit", do: [Boost(stat: "attack", stages: 1, target: Source)]),
            ]),
        ],
    )"#;
    let mut focus = skill("Focus", 0, 100, SkillCategory::Buff, 0);
    focus.ron = true; // the RON record takes over; the table category is ignored
    let aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0), focus],
    );
    // The exact byte script of the built-in `buff_raises_attack_stage_and_damage`.
    let mut b = ron_battle(
        aria,
        tackle_slime(900),
        rules,
        &[50, 50, 100, 1, 50, 100, 1],
    );

    // Round 1: Focus (RON Boost hook) → "Aria's Attack rose!"; Tackle back.
    fight(&mut b, 1);
    settle(&mut b);
    assert_eq!(b.player().stages.attack, 1);
    assert!(
        b.log().contains(&"Aria's Attack rose!".to_string()),
        "{:?}",
        b.log()
    );

    // Round 2: Slash at stage +1 — the same 66 damage the built-in buff deals.
    fight(&mut b, 0);
    settle(&mut b);
    assert!(b.log().contains(&"66 damage!".to_string()), "{:?}", b.log());
}

#[test]
fn ron_scale_relay_scales_the_precomputed_damage() {
    let rules = r#"Ruleset(
        stats: ["hp", "attack", "defense", "speed"],
        resources: ["mp"],
        effects: [
            Effect(id: "slash", kind: Move, hooks: [
                Hook(on: "ModifyDamage", do: [DealMoveDamage, ScaleRelay(num: 3, den: 2)]),
            ]),
        ],
    )"#;
    let aria = combatant("Aria", 60, 12, 10, 15, 20, vec![ron_skill("Slash", 40, 0)]);
    let mut b = ron_battle(aria, tackle_slime(900), rules, &[50, 100, 1]);
    fight(&mut b, 0);
    settle(&mut b);
    // 40×12/8 = 60 → ×89/100 = 53 → ScaleRelay ×3/2 = 79.
    assert!(b.log().contains(&"79 damage!".to_string()), "{:?}", b.log());
}

#[test]
fn ron_before_move_veto_blocks_the_action() {
    let rules = r#"Ruleset(
        stats: ["hp", "attack", "defense", "speed"],
        resources: ["mp"],
        effects: [
            Effect(id: "venom-sting", kind: Move, power: 15, hooks: [
                Hook(on: "BeforeMove", do: [VetoIf(cond: SourceHasStatus("poison"))]),
            ]),
            Effect(id: "poison", kind: Status, hooks: []),
        ],
    )"#;
    // Aria starts poisoned (the Combatant carries it; the mirror picks it up).
    let mut aria = venom_aria();
    aria.status = Some("poison".to_string());
    let mut b = ron_battle(aria, tackle_slime(96), rules, &[50, 100, 1, 50, 100, 1]);
    fight(&mut b, 0);
    settle(&mut b);
    assert!(
        b.log().contains(&"But it failed!".to_string()),
        "{:?}",
        b.log()
    );
    assert_eq!(b.enemy().hp, 96, "the vetoed sting dealt nothing");
}

#[test]
fn ron_residual_chip_can_knock_out() {
    // Slime (already poisoned) at 60 HP: Aria's v1 Slash deals 53 (60→7),
    // Tackle answers, then the 96/8 = 12 chip KOs — the win lands mid-round.
    let mut slime = tackle_slime(96);
    slime.hp = 60;
    slime.status = Some("poison".to_string());
    let aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
    );
    let mut b = ron_battle(aria, slime, POISON_RULES, &[50, 100, 1]);
    play_to_end(&mut b, 0);
    assert_eq!(b.outcome(), Some(BattleOutcome::Win), "{:?}", b.log());
    let log = b.log();
    let chip = log
        .iter()
        .position(|l| l == "Slime is hurt by poison!")
        .unwrap();
    let faint = log.iter().position(|l| l == "Slime fainted!").unwrap();
    assert!(chip < faint, "{log:?}");
}

#[test]
fn ron_poison_switch_keeps_status_and_resolves_the_deferred_enemy_action() {
    // Aria (poisoned, 7 HP) outspeeds the Slime: her Slash lands, the poison
    // chip then KOs her — the forced switch brings Bryn in and the Slime's
    // DEFERRED Tackle resolves against him. Aria keeps her poison on the
    // bench; Bryn's stages are reset.
    let mut aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
    );
    aria.hp = 7; // poison chip: 60/8 = 7 ⇒ exactly 0
    aria.status = Some("poison".to_string());
    let slime = tackle_slime(900);
    let player = vec![aria, bryn()];
    let ruleset = dotzuki_rules::Ruleset::from_ron(POISON_RULES).expect("parse ruleset");
    let compiled = hooks::compile_ruleset(&ruleset).expect("compile ruleset");
    hooks::install_compiled(compiled.clone());
    let stat_names = ruleset.stats.clone();
    let status_names = hooks::status_names(&ruleset);
    let state = HookState {
        state: BattleState::new(
            vec![hooks::mirror_of(
                &player[0],
                &stat_names,
                &status_names,
                true,
            )],
            vec![hooks::mirror_of(&slime, &stat_names, &status_names, true)],
        ),
        effects: Vec::new(),
        mv: dotzuki_engine::battle::stack::MoveContext::default(),
        registry: compiled.build_effects::<GenericProvider>(),
        move_records: hooks::ron_moves(&ruleset, Some("mp")),
        status_names,
        stat_names,
        has_resource: true,
    };
    let mut b = Battle::full(
        player,
        0,
        slime,
        Vec::new(),
        HashMap::new(),
        TypeChart::default(),
        scripted(&[50, 100, 1]),
        Some(state),
    );

    // Aria (spd 15) acts first: Slash 40×12/8 = 60 → ×89/100 = 53; the chip
    // KOs her; the Slime's Tackle waits for the replacement.
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.enemy().hp, 900 - 53);
    assert_eq!(b.party()[0].hp, 0);
    assert!(matches!(b.phase, Phase::ForcedSwitch), "{:?}", b.log());
    assert!(b.log().contains(&"Aria is hurt by poison!".to_string()));
    assert!(b.log().contains(&"Aria fainted!".to_string()));

    // Bryn comes in (free action) — and the Slime's deferred Tackle hits HIM:
    // 40×8/12 = 26 → ×89/100 = 23 (80→57).
    confirm(&mut b);
    settle(&mut b);
    assert_eq!(b.player().name, "Bryn");
    assert_eq!(b.party()[1].hp, 57, "{:?}", b.log());
    assert_eq!(b.party()[1].stages.attack, 0, "stages reset on switch-in");
    // The status persists on the BENCHED member.
    assert_eq!(b.party()[0].status.as_deref(), Some("poison"));
    assert_eq!(b.player().status, None, "Bryn was never poisoned");
    // The mirror tracks the new active member.
    assert_eq!(b.hooks().unwrap().battler(Side::Player).hp, 57);
}

#[test]
fn validate_ruleset_reports_unknown_names() {
    assert!(hooks::validate_ruleset(POISON_RULES).is_empty());

    let bad_stat = r#"Ruleset(
        stats: ["attack"],
        effects: [Effect(id: "x", kind: Move, hooks: [
            Hook(on: "DamagingHit", do: [Boost(stat: "speed", stages: 1, target: Source)]),
        ])],
    )"#;
    let diags = hooks::validate_ruleset(bad_stat);
    assert!(diags.iter().any(|d| d.contains("speed")), "{diags:?}");

    let bad_event =
        r#"Ruleset(effects: [Effect(id: "x", kind: Move, hooks: [Hook(on: "OnHit")])])"#;
    let diags = hooks::validate_ruleset(bad_event);
    assert!(diags.iter().any(|d| d.contains("OnHit")), "{diags:?}");

    let bad_status = r#"Ruleset(effects: [Effect(id: "x", kind: Move, hooks: [
        Hook(on: "DamagingHit", do: [InflictStatus(status: "burn", target: Target)]),
    ])])"#;
    let diags = hooks::validate_ruleset(bad_status);
    assert!(diags.iter().any(|d| d.contains("burn")), "{diags:?}");

    assert!(!hooks::validate_ruleset("Ruleset(types: [").is_empty());
}

// ── abilities, held items & weather (v2-e) ──────────────────────────────────

/// A party battle with the hook machinery installed from a ruleset text.
fn ron_party_battle(party: Vec<Combatant>, enemy: Combatant, rules: &str, bytes: &[u8]) -> Battle {
    let ruleset = dotzuki_rules::Ruleset::from_ron(rules).expect("parse ruleset");
    let compiled = hooks::compile_ruleset(&ruleset).expect("compile ruleset");
    hooks::install_compiled(compiled.clone());
    let stat_names = ruleset.stats.clone();
    let status_names = hooks::status_names(&ruleset);
    let state = HookState {
        state: BattleState::new(
            vec![hooks::mirror_of(
                &party[0],
                &stat_names,
                &status_names,
                true,
            )],
            vec![hooks::mirror_of(&enemy, &stat_names, &status_names, true)],
        ),
        effects: Vec::new(),
        mv: dotzuki_engine::battle::stack::MoveContext::default(),
        registry: compiled.build_effects::<GenericProvider>(),
        move_records: hooks::ron_moves(&ruleset, Some("mp")),
        status_names,
        stat_names,
        has_resource: true,
    };
    let chart = TypeChart::from_ruleset(&ruleset);
    Battle::full(
        party,
        0,
        enemy,
        Vec::new(),
        HashMap::new(),
        chart,
        scripted(bytes),
        Some(state),
    )
}

/// The intimidate ability: −1 attack stage to the foe on switch-in.
const INTIMIDATE_RULES: &str = r#"Ruleset(
    stats: ["hp", "attack", "defense", "speed"],
    resources: ["mp"],
    effects: [
        Effect(id: "intimidate", kind: Ability, hooks: [
            Hook(on: "SwitchIn", do: [Boost(stat: "attack", stages: -1, target: Foe)]),
        ]),
    ],
)"#;

fn intimidate_aria() -> Combatant {
    let mut aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
    );
    aria.ability = Some("intimidate".to_string());
    aria
}

#[test]
fn ability_fires_on_begin_and_re_fires_on_switch_benched_inert() {
    // Both party members carry intimidate; only the ACTIVE one's fires.
    let mut bryn = bryn();
    bryn.ability = Some("intimidate".to_string());
    let slime = tackle_slime(900);
    let mut b = ron_party_battle(
        vec![intimidate_aria(), bryn],
        slime,
        INTIMIDATE_RULES,
        &[50, 100, 1],
    );

    // Battle start: Aria's intimidate drops the Slime's attack to −1; Bryn's
    // (benched) stays inert — exactly one intro line.
    b.begin();
    assert_eq!(b.enemy().stages.attack, -1, "{:?}", b.log());
    assert_eq!(b.player().stages.attack, 0, "the foe has no ability");
    assert_eq!(
        b.log(),
        &["Aria's Intimidate!", "Slime's Attack fell!"],
        "{:?}",
        b.log()
    );
    settle(&mut b);

    // Voluntary switch to Bryn: his intimidate fires on switch-in (−2), and
    // the Slime's answer is weakened (eff atk 8×4/6 = 5: 40×5/12 = 16 → 14).
    party_switch(&mut b, 1);
    settle(&mut b);
    assert_eq!(b.enemy().stages.attack, -2, "{:?}", b.log());
    assert_eq!(b.player().name, "Bryn");
    assert_eq!(b.player().hp, 80 - 14, "{:?}", b.log());
    assert!(
        b.log().contains(&"Bryn's Intimidate!".to_string()),
        "{:?}",
        b.log()
    );

    // Switch back to Aria: her intimidate fires AGAIN (−3) — once per switch-in.
    party_switch(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.enemy().stages.attack, -3, "{:?}", b.log());
    assert_eq!(
        b.log()
            .iter()
            .filter(|l| l.as_str() == "Aria's Intimidate!")
            .count(),
        2,
        "{:?}",
        b.log()
    );
}

#[test]
fn ability_enemy_fires_at_battle_start_and_softens_its_damage_taken() {
    // The SLIME carries intimidate: Aria's attack drops at battle start, and
    // her Slash weakens accordingly (eff atk 12×4/5 = 9: 40×9/8 = 45 → 40).
    let mut slime = tackle_slime(900);
    slime.ability = Some("intimidate".to_string());
    let aria = intimidate_aria(); // Aria's own intimidate cancels the race: both fire.
    let mut b = ron_battle(aria, slime, INTIMIDATE_RULES, &[50, 100, 1]);

    b.begin();
    // Player fires first, then the enemy.
    assert_eq!(
        b.log(),
        &[
            "Aria's Intimidate!",
            "Slime's Attack fell!",
            "Slime's Intimidate!",
            "Aria's Attack fell!",
        ],
        "{:?}",
        b.log()
    );
    assert_eq!(b.player().stages.attack, -1);
    assert_eq!(b.enemy().stages.attack, -1);
    settle(&mut b);

    fight(&mut b, 0);
    settle(&mut b);
    assert!(b.log().contains(&"40 damage!".to_string()), "{:?}", b.log());
}

#[test]
fn ability_fires_when_the_encounter_sends_out_the_next_enemy() {
    // Two queued slimes, the SECOND with intimidate: when the first faints
    // the replacement comes in and its ability fires on send-out.
    let first = tackle_slime(50);
    let mut second = tackle_slime(900);
    second.ability = Some("intimidate".to_string());
    let mut b = ron_party_battle(
        vec![intimidate_aria()],
        first,
        INTIMIDATE_RULES,
        &[50, 100, 1],
    );
    b.set_enemy_party(vec![second], false, 0);
    b.begin(); // Aria's intimidate → the first Slime's attack −1.
    settle(&mut b);

    // Slash 53 KOs the 50-HP slime; the queue sends out the second, whose
    // intimidate drops ARIA's attack on switch-in.
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.enemy().hp, 900, "fresh replacement");
    assert_eq!(b.player().stages.attack, -1, "{:?}", b.log());
    assert!(
        b.log().contains(&"Foe sent out Slime!".to_string()),
        "{:?}",
        b.log()
    );
    assert!(
        b.log().contains(&"Slime's Intimidate!".to_string()),
        "{:?}",
        b.log()
    );
}

#[test]
fn ability_modify_damage_hook_joins_the_action_sequence() {
    // warrior-spirit scales the acting combatant's damage ×3/2 — an ability
    // hooking a per-action event fires alongside the skill's own hooks.
    let rules = r#"Ruleset(
        stats: ["hp", "attack", "defense", "speed"],
        resources: ["mp"],
        effects: [
            Effect(id: "slash", kind: Move, hooks: []),
            Effect(id: "warrior-spirit", kind: Ability, hooks: [
                Hook(on: "ModifyDamage", do: [ScaleRelay(num: 3, den: 2)]),
            ]),
        ],
    )"#;
    let mut aria = combatant("Aria", 60, 12, 10, 15, 20, vec![ron_skill("Slash", 40, 0)]);
    aria.ability = Some("warrior-spirit".to_string());
    let mut b = ron_battle(aria, tackle_slime(900), rules, &[50, 100, 1]);
    b.begin(); // no SwitchIn subscription: no intro, root menu straight away.
    assert!(b.in_menu(), "{:?}", b.log());

    // 40×12/8 = 60 → ×89/100 = 53 → warrior-spirit ×3/2 = 79.
    fight(&mut b, 0);
    settle(&mut b);
    assert!(b.log().contains(&"79 damage!".to_string()), "{:?}", b.log());
    // The ability-less Slime's Tackle is untouched: 40×8/10 = 32 → 28.
    assert!(b.log().contains(&"28 damage!".to_string()), "{:?}", b.log());
}

#[test]
fn held_item_leftovers_heals_after_the_holders_action_only() {
    let rules = r#"Ruleset(
        stats: ["hp", "attack", "defense", "speed"],
        resources: ["mp"],
        effects: [
            Effect(id: "leftovers", kind: Item, hooks: [
                Hook(on: "Residual", do: [HealFraction(num: 1, den: 16, of: MaxHp, target: Target)]),
            ]),
        ],
    )"#;
    let mut bryn = bryn(); // 80 HP — leftovers heal 80/16 = 5.
    bryn.held_item = Some("leftovers".to_string());
    bryn.hp = 50;
    let mut b = ron_battle(bryn, tackle_slime(90), rules, &[50, 100, 1]);

    // Bryn (spd 10) acts first: Slash 40×14/8 = 70 → 62 (90→28); his
    // leftovers then heal 5 (50→55). The Slime (no heldItem) tackles back
    // 40×8/12 = 26 → 23 (55→32) and heals NOTHING after its own action.
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.enemy().hp, 28, "{:?}", b.log());
    assert_eq!(b.player().hp, 32, "{:?}", b.log());
    assert!(
        b.log().contains(&"Bryn recovered 5 HP!".to_string()),
        "{:?}",
        b.log()
    );
    assert!(
        !b.log().iter().any(|l| l.starts_with("Slime recovered")),
        "the enemy without heldItem heals nothing: {:?}",
        b.log()
    );
}

#[test]
fn weather_chips_both_sides_per_round_until_cleared() {
    let rules = r#"Ruleset(
        stats: ["hp", "attack", "defense", "speed"],
        resources: ["mp"],
        effects: [
            Effect(id: "sandstorm", kind: Weather, hooks: [
                Hook(on: "FieldResidual", do: [DamageFraction(num: 1, den: 16, of: MaxHp, target: Target)]),
            ]),
        ],
    )"#;
    let aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
    );
    let slime = tackle_slime(900); // chip 900/16 = 56.
    let mut b = ron_battle(aria, slime, rules, &[50, 100, 1]);
    b.set_weather(Some("sandstorm".to_string()));
    b.begin();
    assert_eq!(b.log(), &["A sandstorm rages!"], "{:?}", b.log());
    settle(&mut b);

    // Round 1: Slash 53 (900→847); Aria's residual chips her 3 (60→57);
    // Tackle 28 (57→29); the Slime's residual chips it 56 (847→791).
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.player().hp, 29, "{:?}", b.log());
    assert_eq!(b.enemy().hp, 791, "{:?}", b.log());
    assert!(
        b.log().contains(&"Aria lost 3 HP!".to_string()),
        "{:?}",
        b.log()
    );
    assert!(
        b.log().contains(&"Slime lost 56 HP!".to_string()),
        "{:?}",
        b.log()
    );

    // clearWeather stops the chip: round 2 deals only the hits — Slash 53
    // (791→738), Tackle 28 (29→1), no residuals either side.
    b.set_weather(None);
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.player().hp, 1, "no chip after clear: {:?}", b.log());
    assert_eq!(b.enemy().hp, 738, "{:?}", b.log());
    assert_eq!(
        b.log().iter().filter(|l| l.contains("lost")).count(),
        2,
        "only round 1 chipped: {:?}",
        b.log()
    );
}

#[test]
fn unknown_weather_id_is_dropped_with_a_warning() {
    let (aria, slime) = aria_vs_slime();
    let mut b = ron_battle(aria, slime, POISON_RULES, &[50, 100, 1]);
    b.set_weather(Some("bogus".to_string()));
    b.begin();
    assert_eq!(b.weather(), None, "an id with no compiled hooks is ignored");
    assert!(
        b.log().is_empty(),
        "no intro line for unknown weather: {:?}",
        b.log()
    );
    assert!(b.in_menu());
}

#[test]
fn begin_is_a_noop_without_abilities_or_weather() {
    // Back-compat: a project with none of the v2-e fields sees byte-identical
    // behavior — begin() queues nothing and the battle opens on the menu.
    let (aria, slime) = aria_vs_slime();
    let mut b = ron_battle(aria, slime, POISON_RULES, &[50, 100, 1]);
    b.begin();
    assert!(b.log().is_empty(), "{:?}", b.log());
    assert!(b.in_menu());
}

#[test]
fn prettify_id_title_cases_record_ids() {
    assert_eq!(prettify_id("intimidate"), "Intimidate");
    assert_eq!(prettify_id("swift-swim"), "Swift Swim");
    assert_eq!(prettify_id("battle_armor"), "Battle Armor");
}

// ── EXP & levels (v2-c) ───────────────────────────────────────────────────────

/// A levels config (defaults: exp/level fields, 8·L³ curve, +5% growth).
fn levels(max_level: u8) -> LevelsSetup {
    LevelsSetup {
        exp_field: "exp".to_string(),
        level_field: "level".to_string(),
        curve_base: 8,
        curve_exponent: 3,
        growth: 0.05,
        max_level,
    }
}

#[test]
fn growth_multiplier_exact_integers() {
    // floor(raw × (1 + 0.05 × (level − 1))).
    assert_eq!(growth_stat(100, 1, 0.05), 100, "level 1 = ×1 (unchanged)");
    assert_eq!(growth_stat(100, 2, 0.05), 105);
    assert_eq!(growth_stat(100, 10, 0.05), 145, "×1.45");
    assert_eq!(growth_stat(20, 1, 0.05), 20);
    assert_eq!(growth_stat(20, 2, 0.05), 21);
    assert_eq!(growth_stat(20, 10, 0.05), 29);
    // Records without a level field read as level 1 ⇒ ×1 for ANY growth.
    assert_eq!(growth_stat(33, 1, 0.25), 33);
    assert_eq!(growth_stat(33, 0, 0.25), 33, "level 0 also clamps to ×1");
    // Exact products don't floor one short on float error (×2 of 100).
    assert_eq!(growth_stat(100, 5, 0.25), 200);
}

#[test]
fn exp_curve_exact_thresholds() {
    // exp_to_next(L) = 8 × L³.
    assert_eq!(exp_to_next(8, 3, 1), 8);
    assert_eq!(exp_to_next(8, 3, 2), 64);
    assert_eq!(exp_to_next(8, 3, 3), 216);
    assert_eq!(exp_to_next(8, 3, 100), 8_000_000);
}

/// Win a 2-member party battle with the levels block armed; the enemy
/// awards `reward` EXP. Returns the finished battle.
fn win_levels_battle(reward: u32, max_level: u8, lang: &str) -> Battle {
    let mut b = party_battle(&[50, 100, 1]);
    b.enemy.exp_reward = reward;
    b.set_levels(Some(levels(max_level)));
    b.set_lang(lang);
    play_to_end(&mut b, 0);
    assert_eq!(b.outcome(), Some(BattleOutcome::Win));
    b
}

#[test]
fn win_awards_exp_to_living_members_only_and_heals_the_delta() {
    let mut b = party_battle(&[50, 100, 1]);
    b.party[1].hp = 0; // Bryn benched-fainted before the win
    b.enemy.exp_reward = 8;
    b.set_levels(Some(levels(100)));
    play_to_end(&mut b, 0);
    assert_eq!(b.outcome(), Some(BattleOutcome::Win));

    // Aria (took one 28-dmg Tackle: 60 → 32) gains 8 EXP = the level-1
    // curve cost ⇒ level 2. Growth ×1.05: max HP 60 → 63 (heal-the-delta:
    // 32 → 35), max MP 20 → 21 (20 → 21); the small stats floor unchanged.
    let aria = &b.party()[0];
    assert_eq!(aria.level, 2);
    assert_eq!(aria.exp, 0, "8 gained − 8 spent on the level-up");
    assert_eq!((aria.hp, aria.max_hp), (35, 63));
    assert_eq!((aria.mp, aria.max_mp), (21, 21));
    assert_eq!(aria.attack, 12, "floor(12 × 1.05) = 12");
    // Bryn was fainted ⇒ no EXP, no level-up.
    let bryn = &b.party()[1];
    assert_eq!((bryn.level, bryn.exp), (1, 0));

    // Narration, after the win text, for Aria only.
    let log = b.log();
    let win = log.iter().position(|l| l == "You won the battle!").unwrap();
    assert_eq!(log[win + 1], "Aria gained 8 EXP!");
    assert_eq!(log[win + 2], "Aria grew to level 2!");
    assert!(!log.iter().any(|l| l.contains("Bryn gained")), "{log:?}");

    // …and the harvest carries level/exp for the runner.
    let state = b.party_state();
    assert_eq!((state[0].level, state[0].exp), (2, 0));
    assert_eq!((state[1].level, state[1].exp), (1, 0));
}

#[test]
fn multi_level_up_from_one_award() {
    let b = win_levels_battle(80, 100, "en");
    let aria = &b.party()[0];
    // 80 EXP: −8 → level 2 (72 left), −64 → level 3 (8 left), 8 < 216 stop.
    assert_eq!(aria.level, 3);
    assert_eq!(aria.exp, 8);
    assert_eq!(aria.max_hp, 66, "60 × 1.10");
    let ups: Vec<&str> = b
        .log()
        .iter()
        .filter(|l| l.starts_with("Aria grew to level"))
        .map(String::as_str)
        .collect();
    assert_eq!(ups, ["Aria grew to level 2!", "Aria grew to level 3!"]);
}

#[test]
fn max_level_caps_level_ups() {
    let b = win_levels_battle(80, 2, "en");
    let aria = &b.party()[0];
    assert_eq!(aria.level, 2, "level-ups stop at the cap");
    assert_eq!(aria.exp, 72, "unspent progress stays (80 − 8)");
}

#[test]
fn zh_narration_for_exp_and_level_up() {
    let b = win_levels_battle(8, 100, "zh");
    let log = b.log();
    assert!(
        log.contains(&"Aria 获得了 8 点经验！".to_string()),
        "{log:?}"
    );
    assert!(log.contains(&"Aria 升到了 2 级！".to_string()), "{log:?}");
}

#[test]
fn no_levels_block_no_exp_narration() {
    // Back-compat: even with an exp_reward on the enemy, a battle without
    // the levels config awards nothing — the log is byte-identical to v1.
    let mut b = party_battle(&[50, 100, 1]);
    b.enemy.exp_reward = 8;
    play_to_end(&mut b, 0);
    assert_eq!(b.outcome(), Some(BattleOutcome::Win));
    assert_eq!(b.log().last(), Some(&"You won the battle!".to_string()));
    assert!(!b.log().iter().any(|l| l.contains("EXP")), "{:?}", b.log());
    assert_eq!((b.party()[0].level, b.party()[0].exp), (1, 0));
}

// ── encounters, trainer battles & Run (v2-d) ──────────────────────────────────

/// A fast but durable Bat (the queued second enemy): Aria's 106-dmg Slash
/// (40×12/4 ×89%) does NOT one-shot its 200 HP, so it gets to fight back.
fn bat() -> Combatant {
    combatant(
        "Bat",
        200,
        6,
        4,
        12,
        0,
        vec![skill("Bite", 20, 100, SkillCategory::Damage, 0)],
    )
}

/// Aria vs a two-enemy wild encounter (the Slime active — fragile enough
/// for a one-shot — the durable Bat queued behind it).
fn encounter_battle(bytes: &[u8]) -> Battle {
    let aria = combatant(
        "Aria",
        60,
        12,
        10,
        15,
        20,
        vec![skill("Slash", 40, 100, SkillCategory::Damage, 0)],
    );
    let slime = combatant(
        "Slime",
        40,
        8,
        8,
        5,
        0,
        vec![skill("Tackle", 40, 100, SkillCategory::Damage, 0)],
    );
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(bytes));
    b.set_enemy_party(vec![bat()], false, 0);
    b
}

/// Root → Run (the last root entry; no items in these battles).
fn run(b: &mut Battle) {
    press(b, GbButton::Down.bit_mask());
    press(b, GbButton::Down.bit_mask());
    confirm(b);
}

#[test]
fn encounter_queue_sends_out_next_enemy_and_exp_sums() {
    // [acc hit, var 89%, no crit] per action.
    let mut b = encounter_battle(&[50, 100, 1]);
    b.enemy.exp_reward = 8;
    b.enemies[0].exp_reward = 5;
    b.set_levels(Some(levels(100)));
    assert_eq!(b.enemies_remaining(), 1);

    // Round 1: Aria 53 dmg KOs the Slime; the queue sends out the Bat and
    // the round ends (the replacement never acts the turn it comes in).
    fight(&mut b, 0);
    settle(&mut b);
    assert!(b.outcome().is_none());
    assert!(b.in_menu(), "round over after the send-out");
    assert_eq!(b.enemy().name, "Bat");
    assert_eq!(b.enemy().hp, 200, "a fresh combatant at full HP");
    assert_eq!(b.enemies_remaining(), 0);
    let log = b.log();
    let faint = log.iter().position(|l| l == "Slime fainted!").unwrap();
    assert_eq!(log[faint + 1], "Foe sent out Bat!");
    assert!(!log.iter().any(|l| l == "Bat used Bite!"), "{log:?}");

    // Rounds 2-3: the Bat fights back (survives one 53-dmg hit at 60 HP),
    // then faints — the queue is empty, so the battle is won and the EXP
    // award is the SUM (8 + 5 = 13).
    fight(&mut b, 0);
    settle(&mut b);
    assert!(
        b.log().contains(&"Bat used Bite!".to_string()),
        "{:?}",
        b.log()
    );
    fight(&mut b, 0);
    settle(&mut b);
    assert_eq!(b.outcome(), Some(BattleOutcome::Win));
    let log = b.log();
    let win = log.iter().position(|l| l == "You won the battle!").unwrap();
    assert_eq!(log[win - 1], "Bat fainted!");
    assert_eq!(log[win + 1], "Aria gained 13 EXP!", "{log:?}");
    assert_eq!(b.party()[0].exp, 5, "13 gained − 8 spent on the level-up");
    assert_eq!(b.party()[0].level, 2);
}

#[test]
fn encounter_send_out_narrates_in_zh() {
    let mut b = encounter_battle(&[50, 100, 1]);
    b.set_lang("zh");
    fight(&mut b, 0);
    settle(&mut b);
    assert!(
        b.log().contains(&"对方派出了 Bat！".to_string()),
        "{:?}",
        b.log()
    );
}

#[test]
fn trainer_battle_blocks_run_without_consuming_the_turn() {
    let (aria, slime) = aria_vs_slime();
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50, 100, 1]));
    b.set_enemy_party(Vec::new(), true, 80);
    assert!(b.is_trainer());

    run(&mut b);
    settle(&mut b);
    assert!(b.outcome().is_none(), "a blocked Run never ends the battle");
    assert!(
        b.in_menu(),
        "back at the root menu — the turn was NOT consumed"
    );
    assert_eq!(
        b.log(),
        &["Can't escape from a trainer battle!".to_string()]
    );
    assert_eq!(b.player().hp, 60, "the enemy never acted");

    // Fighting on wins and pays the trainer money (narrated; the runner
    // reads trainer_money()).
    play_to_end(&mut b, 0);
    assert_eq!(b.outcome(), Some(BattleOutcome::Win));
    assert_eq!(b.trainer_money(), 80);
    let log = b.log();
    let win = log.iter().position(|l| l == "You won the battle!").unwrap();
    assert_eq!(log[win + 1], "Got 80 G for winning!");
}

#[test]
fn wild_run_ends_the_battle_with_the_run_outcome() {
    let (aria, slime) = aria_vs_slime();
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50, 100, 1]));
    b.enemy.exp_reward = 8;
    b.set_levels(Some(levels(100)));
    assert!(!b.is_trainer(), "a plain battle is wild");

    run(&mut b);
    assert_eq!(b.current_line(), Some("Got away safely!"));
    settle(&mut b);
    assert_eq!(b.outcome(), Some(BattleOutcome::Run));
    assert_eq!(
        b.log(),
        &["Got away safely!".to_string()],
        "no EXP/money, no faint lines"
    );
    assert_eq!(b.party()[0].exp, 0, "a run awards no EXP");
    assert_eq!(
        b.party()[0].hp,
        60,
        "the party state carries over untouched"
    );
}

#[test]
fn run_lines_narrate_in_zh() {
    let (aria, slime) = aria_vs_slime();
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50, 100, 1]));
    b.set_lang("zh");
    run(&mut b);
    settle(&mut b);
    assert_eq!(b.outcome(), Some(BattleOutcome::Run));
    assert_eq!(b.log(), &["顺利逃走了！".to_string()]);

    let (aria, slime) = aria_vs_slime();
    let mut b = Battle::new(aria, slime, TypeChart::default(), scripted(&[50, 100, 1]));
    b.set_enemy_party(Vec::new(), true, 0);
    b.set_lang("zh");
    run(&mut b);
    settle(&mut b);
    assert_eq!(b.log(), &["无法从训练家的战斗中逃走！".to_string()]);
}
