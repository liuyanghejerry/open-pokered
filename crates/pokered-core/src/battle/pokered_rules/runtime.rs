//! # P6 production runtime — drive a real battle through the stack engine.
//!
//! The PRODUCTION-side glue the live battle loop uses to route a turn through
//! [`StackDriver`](dotzuki_engine::battle::stack::StackDriver): a `rand`-backed
//! [`RandBattleRng`], the [`TurnEvent`] → battle-text translator, and the
//! legacy `Pokemon` → engine `BattlerState` adapter.
//!
//! The translator here is the canonical, production copy; the differential
//! `translate_*` tests in [`tests`](super) exercise the same logic test-side.

use dotzuki_engine::battle::rng::BattleRng;
use dotzuki_engine::battle::stack::{HpChangeCause, TurnEvent, TurnLog};
use dotzuki_engine::battle::{
    BattlerState as EngineBattler, BattlerRef, EnumMap, BattleState as EngineState,
};

use pokered_data::lang_data::{move_name, species_name};
use pokered_data::move_data::MoveData;
use pokered_data::moves::MoveId;
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::types::{Effectiveness, PokemonType};

use dotzuki_engine::battle::stack::{EffectId, EffectState};

use crate::battle::stat_stages::StatIndex;
use crate::battle::state::{
    status1, status2, status3, BattleState as LegacyBattleState, BattlerState as LegacyParty, Pokemon,
    StatusCondition as LegacyStatus,
};

use super::{PokeVolatile, PokeredRules};

/// A `rand`-backed [`BattleRng`] for live play (the engine never links `rand`, so
/// the game supplies it). Each draw is a fresh `rand::random::<u8>()`; the stack's
/// lazy draws are still correct Gen-1 distributions (byte-exact replay vs the
/// legacy pre-roll only mattered for the differential parity tests).
pub struct RandBattleRng;

impl BattleRng for RandBattleRng {
    fn next_u8(&mut self) -> u8 {
        rand::random::<u8>()
    }
}

/// The `MoveData` for a move logged in a `MoveUsed` event (always a real move).
fn move_data(m: MoveId) -> MoveData {
    *MoveData::get(m).unwrap_or_else(|| MoveData::get(MoveId::Tackle).unwrap())
}

/// Build an engine battler from a leveled, stat-computed legacy `Pokemon` (a fresh
/// battler — no mid-battle volatiles; the stack maintains its own arena).
pub fn engine_from_pokemon(p: &Pokemon) -> EngineBattler<PokeredRules> {
    let mut stats = EnumMap::new();
    stats.set(StatIndex::Attack, p.attack);
    stats.set(StatIndex::Defense, p.defense);
    stats.set(StatIndex::Speed, p.speed);
    stats.set(StatIndex::Special, p.special);
    let moves: Vec<MoveId> = p.moves.iter().copied().filter(|m| *m != MoveId::None).collect();
    EngineBattler::new(p.species, p.hp, p.max_hp, stats, moves).with_level(p.level)
}

// ── translator (canonical production copy) ──────────────────────────────────

/// The combined type-effectiveness category of `move_type` vs a (possibly
/// dual-type) defender, via the chart's integer discriminants (×10 units; exact,
/// float-free; a single-type mon has `def2 == def1`).
pub fn effectiveness_category(move_type: PokemonType, def1: PokemonType, def2: PokemonType) -> Effectiveness {
    use pokered_data::type_chart::get_effectiveness;
    let e1 = get_effectiveness(move_type, def1) as u32;
    let e2 = if def2 == def1 { 10 } else { get_effectiveness(move_type, def2) as u32 };
    match e1 * e2 / 10 {
        0 => Effectiveness::NoEffect,
        10 => Effectiveness::Normal,
        x if x > 10 => Effectiveness::SuperEffective,
        _ => Effectiveness::NotVeryEffective,
    }
}

/// The move-announcement block for one mover, matching `format_move_outcome`.
pub fn move_announcement(
    side_name: &str,
    move_name: &str,
    crit: bool,
    missed: bool,
    eff: Effectiveness,
    blocked_reason: Option<&str>,
) -> Vec<String> {
    if let Some(reason) = blocked_reason {
        return vec![format!("{} {}", side_name, reason)];
    }
    let mut msgs = vec![format!("{} used {}!", side_name, move_name)];
    if missed {
        msgs.push(format!("{}'s attack missed!", side_name));
        return msgs;
    }
    if crit {
        msgs.push("Critical hit!".to_string());
    }
    match eff {
        Effectiveness::SuperEffective => msgs.push("It's super effective!".to_string()),
        Effectiveness::NotVeryEffective => msgs.push("It's not very effective...".to_string()),
        Effectiveness::NoEffect => msgs.push("It doesn't affect the enemy!".to_string()),
        Effectiveness::Normal => {}
    }
    msgs
}

fn battler_at<'a>(state: &'a EngineState<PokeredRules>, who: BattlerRef) -> &'a EngineBattler<PokeredRules> {
    if who.side == 0 {
        &state.player_battlers[who.slot as usize]
    } else {
        &state.opponent_battlers[who.slot as usize]
    }
}

/// Display name: UPPERCASE species, "Enemy "-prefixed on side 1.
pub fn display_name(state: &EngineState<PokeredRules>, who: BattlerRef) -> String {
    let up = species_name(battler_at(state, who).species, false).to_uppercase();
    if who.side == 0 {
        up
    } else {
        format!("Enemy {}", up)
    }
}

fn opp_ref(who: BattlerRef) -> BattlerRef {
    BattlerRef::new(if who.side == 0 { 1 } else { 0 }, who.slot)
}

fn blocked_reason(state: &EngineState<PokeredRules>, log: &TurnLog<PokeredRules>, who: BattlerRef) -> &'static str {
    match battler_at(state, who).status {
        Some(LegacyStatus::Sleep(_)) => "is fast asleep!",
        Some(LegacyStatus::Freeze) => "is frozen solid!",
        Some(LegacyStatus::Paralysis) => "is fully paralyzed!",
        _ => {
            if log.events.iter().any(|e| matches!(e, TurnEvent::Damaged { target, .. } if *target == who)) {
                "hurt itself in confusion!"
            } else {
                "can't move!"
            }
        }
    }
}

/// The defender's EFFECTIVE types for the narration effectiveness line: a live
/// Conversion `TypeOverride` on `who` if present (so the "super effective!" text
/// agrees with the override-aware damage), else the species types. Mirrors
/// [`super::effective_types`] but off the raw `effects` slice (the narration layer
/// holds no `BattleCtx`).
fn narration_types(
    effects: &[EffectState<PokeredRules>],
    state: &EngineState<PokeredRules>,
    who: BattlerRef,
) -> (PokemonType, PokemonType) {
    if let Some(t) = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::TypeOverride { type1, type2 } if e.host == who => Some((*type1, *type2)),
        _ => None,
    }) {
        return t;
    }
    get_base_stats(battler_at(state, who).species)
        .map(|b| (b.type1, b.type2))
        .unwrap_or((PokemonType::Normal, PokemonType::Normal))
}

/// Walk the whole `TurnLog` → the production per-turn text lines. `effects` is the
/// post-turn arena, consulted so the effectiveness message honours a Conversion
/// `TypeOverride` on the defender (the damage already does); pass `&[]` when no
/// override can be live.
pub fn translate_turn(
    log: &TurnLog<PokeredRules>,
    state: &EngineState<PokeredRules>,
    effects: &[EffectState<PokeredRules>],
) -> Vec<String> {
    let evs = &log.events;
    let mut msgs = Vec::new();
    // The move whose effect events are currently being walked — Haze's own
    // cure/stat events narrate through its single "eliminated" line.
    let mut current_move: Option<MoveId> = None;
    for (i, e) in evs.iter().enumerate() {
        match e {
            TurnEvent::MoveUsed { actor, move_ } => {
                current_move = Some(*move_);
                let actor = *actor;
                let (mut crit, mut missed) = (false, false);
                for follow in &evs[i + 1..] {
                    match follow {
                        TurnEvent::MoveUsed { .. } | TurnEvent::Blocked { .. } => break,
                        TurnEvent::Crit { actor: a } if *a == actor => crit = true,
                        TurnEvent::Missed { actor: a } if *a == actor => missed = true,
                        _ => {}
                    }
                }
                let md = move_data(*move_);
                let (d1, d2) = narration_types(effects, state, opp_ref(actor));
                let eff = effectiveness_category(md.move_type, d1, d2);
                msgs.extend(move_announcement(&display_name(state, actor), move_name(*move_, false), crit, missed, eff, None));
                // HazeEffect (haze.asm:1-49): a landed Haze narrates ONLY
                // StatusChangesEliminatedText ("All STATUS changes are
                // eliminated!", text_3.asm:269-271) — no per-stat or
                // per-status-cure lines follow.
                if *move_ == MoveId::Haze && !missed {
                    msgs.push("All STATUS changes\nare eliminated!".to_string());
                }
            }
            TurnEvent::Blocked { actor } => {
                msgs.push(format!("{} {}", display_name(state, *actor), blocked_reason(state, log, *actor)));
            }
            TurnEvent::Fainted { who } => msgs.push(format!("{} fainted!", display_name(state, *who))),
            TurnEvent::StatusInflicted { target, status } => {
                let name = display_name(state, *target);
                let line = match status {
                    LegacyStatus::Poison => format!("{name} was poisoned!"),
                    LegacyStatus::Burn => format!("{name} was burned!"),
                    LegacyStatus::Paralysis => {
                        format!("{name} is paralyzed!\nIt may be unable\nto move!")
                    }
                    LegacyStatus::Freeze => format!("{name} was frozen solid!"),
                    LegacyStatus::Sleep(_) => format!("{name} fell asleep!"),
                    LegacyStatus::None => continue,
                };
                msgs.push(line);
            }
            TurnEvent::StatusCured { target, status } => {
                // Haze's cure narrates through its own "eliminated" line
                // (haze.asm prints StatusChangesEliminatedText — NOT
                // CheckDefrost's "Fire defrosted <TARGET>!", text_3.asm:91).
                if current_move == Some(MoveId::Haze) {
                    continue;
                }
                // Gen-1 narrates waking from sleep mid-turn, and CheckDefrost's
                // "Fire defrosted <TARGET>!" when a Fire-type burn-family move
                // thaws a frozen target.
                if matches!(status, LegacyStatus::Sleep(_)) {
                    msgs.push(format!("{} woke up!", display_name(state, *target)));
                } else if matches!(status, LegacyStatus::Freeze) {
                    msgs.push(format!("Fire defrosted\n{}!", display_name(state, *target)));
                }
            }
            TurnEvent::StatChanged { target, stat, delta } => {
                // Haze's stat reset draws no per-stat lines (the original
                // prints only StatusChangesEliminatedText).
                if current_move == Some(MoveId::Haze) {
                    continue;
                }
                if *delta != 0 {
                    let name = display_name(state, *target);
                    let verb = match *delta {
                        d if d >= 2 => "greatly rose",
                        1 => "rose",
                        -1 => "fell",
                        _ => "greatly fell",
                    };
                    msgs.push(format!("{name}'s {} {verb}!", stat_display(*stat)));
                }
            }
            // Residual HP loss from a NON-VOLATILE status (burn/poison chip) — the
            // driver tags it `Status(s)`. `cause: None` (move damage) stays silent (the
            // move announcement narrates it).
            TurnEvent::Damaged { target, cause: Some(HpChangeCause::Status(status)), .. } => {
                let name = display_name(state, *target);
                let line = match status {
                    LegacyStatus::Poison => format!("{name} is hurt by POISON!"),
                    LegacyStatus::Burn => format!("{name} is hurt by its BURN!"),
                    _ => continue, // no other Gen-1 status chips
                };
                msgs.push(line);
            }
            // VOLATILE residual — the driver now tags it `Volatile(kind)`, the game's
            // opaque per-volatile token. A badly-poisoned mon's ramp chips via the Toxic
            // VOLATILE (the plain-Poison status residual SKIPS when Toxic is live — "one
            // chip, not two", see `effect_for_status`), so its "hurt by POISON!" text lives
            // HERE, not on the Status arm. Leech Seed's sap narrates the DRAINED mon (the
            // `Damaged` target); the paired `Healed` on the seeder stays silent (Gen-1
            // prints one line). Bide's cross-battler unleash and any other volatile residual
            // narrate via their own move flow → silent here.
            TurnEvent::Damaged { target, cause: Some(HpChangeCause::Volatile(kind)), .. } => {
                let name = display_name(state, *target);
                let line = match kind {
                    PokeVolatile::Toxic { .. } => format!("{name} is hurt by POISON!"),
                    PokeVolatile::LeechSeed => format!("{name}'s\nHEALTH is sapped\nby LEECH SEED!"),
                    _ => continue,
                };
                msgs.push(line);
            }
            _ => {}
        }
    }
    msgs
}

/// Gen-1 battle-text name for a stat stage.
fn stat_display(stat: StatIndex) -> &'static str {
    match stat {
        StatIndex::Attack => "ATTACK",
        StatIndex::Defense => "DEFENSE",
        StatIndex::Speed => "SPEED",
        StatIndex::Special => "SPECIAL",
        StatIndex::Accuracy => "ACCURACY",
        StatIndex::Evasion => "EVASIVENESS",
    }
}

// ── legacy BattleState ↔ engine EngineState adapter ─────────────────────────
//
// The live loop's source of truth is the legacy `BattleState` (active mons +
// status1/2/3 flags). Each turn it is converted to an `EngineState` + the volatile
// arena, run through `StackDriver`, then written back. Lock-in volatiles
// (Bide/Thrash/Fly/…) are NOT yet in `PokeVolatile`, so those moves don't lock
// across turns on the stack path (a known gap — see docs/17).

const ADAPTER_ID_BASE: u32 = 0x70_000;

fn legacy_status_opt(p: &Pokemon) -> Option<LegacyStatus> {
    match p.status {
        LegacyStatus::None => None,
        s => Some(s),
    }
}

/// One side's active mon → an engine battler (hp/max_hp/level/stats/moves + status
/// + stat stages).
fn engine_active(party: &LegacyParty) -> EngineBattler<PokeredRules> {
    let mon = party.active_mon();
    let mut b = engine_from_pokemon(mon);
    b.status = legacy_status_opt(mon);
    let ss = &party.stat_stages;
    b.stat_stages.set(StatIndex::Attack, ss.attack);
    b.stat_stages.set(StatIndex::Defense, ss.defense);
    b.stat_stages.set(StatIndex::Speed, ss.speed);
    b.stat_stages.set(StatIndex::Special, ss.special);
    b.stat_stages.set(StatIndex::Accuracy, ss.accuracy);
    b.stat_stages.set(StatIndex::Evasion, ss.evasion);
    b
}

/// Legacy status1/2/3 flags + counters → `PokeVolatile` arena entries on `host`.
fn build_volatiles(party: &LegacyParty, host: BattlerRef, out: &mut Vec<EffectState<PokeredRules>>, next: &mut u32) {
    let mut push = |out: &mut Vec<EffectState<PokeredRules>>, kind: PokeVolatile| {
        out.push(EffectState { id: EffectId(*next), host, effect_order: *next as u64, kind });
        *next += 1;
    };
    if party.has_status2(status2::GETTING_PUMPED) { push(out, PokeVolatile::FocusEnergy); }
    if party.has_status2(status2::HAS_SUBSTITUTE_UP) { push(out, PokeVolatile::SubstituteHp { hp: party.substitute_hp as u16 }); }
    if party.has_status3(status3::HAS_LIGHT_SCREEN_UP) { push(out, PokeVolatile::LightScreen); }
    if party.has_status3(status3::HAS_REFLECT_UP) { push(out, PokeVolatile::Reflect); }
    if party.has_status2(status2::PROTECTED_BY_MIST) { push(out, PokeVolatile::Mist); }
    if party.has_status2(status2::SEEDED) { push(out, PokeVolatile::LeechSeed); }
    if party.has_status1(status1::CONFUSED) { push(out, PokeVolatile::Confused { turns: party.confused_turns_left }); }
    if party.has_status1(status1::FLINCHED) { push(out, PokeVolatile::Flinched); }
    if party.has_status3(status3::BADLY_POISONED) { push(out, PokeVolatile::Toxic { counter: party.toxic_counter }); }
    if party.has_status2(status2::NEEDS_TO_RECHARGE) { push(out, PokeVolatile::Recharge); }
    if party.has_status1(status1::CHARGING_UP) {
        // The charging move is recovered from the persisted `selected_move` (the flag
        // itself stores no move); invulnerability from the INVULNERABLE bit.
        push(out, PokeVolatile::Charging {
            move_: party.selected_move,
            invulnerable: party.has_status1(status1::INVULNERABLE),
        });
    }
    if party.has_status1(status1::THRASHING_ABOUT) {
        // Rampage move recovered from `selected_move`; remaining uses from num_attacks_left.
        push(out, PokeVolatile::LockedMove {
            move_: party.selected_move,
            turns_left: party.num_attacks_left,
            confuse_on_end: true,
        });
    }
    if party.has_status2(status2::USING_RAGE) { push(out, PokeVolatile::Rage); }
    if party.has_status1(status1::USING_TRAPPING_MOVE) {
        push(out, PokeVolatile::Trapping {
            move_: party.selected_move,
            turns_left: party.num_attacks_left,
        });
    }
    if party.has_status1(status1::STORING_ENERGY) {
        push(out, PokeVolatile::Bide {
            turns_left: party.num_attacks_left,
            accumulated: party.bide_accumulated_damage,
        });
    }
    // Conversion: the battle-only type override (never on the persistent Pokémon).
    if let (Some(type1), Some(type2)) = (party.conversion_type1, party.conversion_type2) {
        push(out, PokeVolatile::TypeOverride { type1, type2 });
    }
    // Disable: the disabled move slot (1-based, mirrors the compacted engine `moves`
    // for a gapless moveset — the Gen-1 norm) + its countdown.
    if party.disabled_move > 0 {
        push(out, PokeVolatile::Disable {
            slot: party.disabled_move,
            turns: party.disabled_turns_left,
        });
    }
}

/// Build a fresh engine state + volatile arena FROM the legacy battle state.
pub fn engine_state_from_legacy(
    ls: &LegacyBattleState,
) -> (EngineState<PokeredRules>, Vec<EffectState<PokeredRules>>) {
    let mut player_b = engine_active(&ls.player);
    // Badge stat boosts: the player battler's working stats are the boosted
    // `wBattleMon*` copy persisted on the legacy side (see badge_boosts), and
    // the badge context (bits + unmodified stats) rides the battler's resource
    // pool so the stat-stage funnel can re-apply the stat-up glitch in-turn.
    if let Some(boosted) = ls.player.badge_boosted_stats {
        crate::battle::badge_boosts::engine_stats_overlay(&mut player_b, boosted);
    }
    crate::battle::badge_boosts::seed_badge_context(
        &mut player_b,
        ls.player_badges,
        [
            ls.player.unmodified_attack,
            ls.player.unmodified_defense,
            ls.player.unmodified_speed,
            ls.player.unmodified_special,
        ],
    );
    let state = EngineState::new(vec![player_b], vec![engine_active(&ls.enemy)]);
    let mut effects = Vec::new();
    let mut next = ADAPTER_ID_BASE;
    build_volatiles(&ls.player, BattlerRef::PLAYER, &mut effects, &mut next);
    build_volatiles(&ls.enemy, BattlerRef::OPPONENT, &mut effects, &mut next);
    (state, effects)
}

fn write_party(party: &mut LegacyParty, b: &EngineBattler<PokeredRules>, effects: &[EffectState<PokeredRules>], host: BattlerRef) {
    {
        let mon = party.active_mon_mut();
        mon.hp = b.hp.min(mon.max_hp);
        mon.status = b.status.unwrap_or(LegacyStatus::None);
    }
    // Persist the player's working stats (with any in-turn stat-up-glitch
    // re-applications) back onto the legacy badge-boost copy (badge_boosts).
    if host == BattlerRef::PLAYER {
        party.badge_boosted_stats = Some(crate::battle::badge_boosts::engine_stats_snapshot(b));
    }
    let g = |s: StatIndex| b.stat_stages.get(s).copied().unwrap_or(0);
    let ss = &mut party.stat_stages;
    ss.attack = g(StatIndex::Attack);
    ss.defense = g(StatIndex::Defense);
    ss.speed = g(StatIndex::Speed);
    ss.special = g(StatIndex::Special);
    ss.accuracy = g(StatIndex::Accuracy);
    ss.evasion = g(StatIndex::Evasion);
    // Transform: persist the copied identity into the legacy Pokémon exactly ONCE
    // (the Transformed marker is present only on the Transform turn — it is never
    // re-created by build_volatiles). Mirrors apply_transform: destructive, no
    // switch-out restore. On later turns species/stats/moves/pp are left alone, so
    // PP depletes normally and the copy sticks.
    if effects.iter().any(|e| e.host == host && matches!(e.kind, PokeVolatile::Transformed)) {
        let (t1, t2) = get_base_stats(b.species)
            .map(|bs| (bs.type1, bs.type2))
            .unwrap_or((PokemonType::Normal, PokemonType::Normal));
        let mut moves4 = [MoveId::None; 4];
        for (i, m) in b.moves.iter().take(4).enumerate() {
            moves4[i] = *m;
        }
        let mon = party.active_mon_mut();
        mon.species = b.species;
        mon.attack = b.stats.get(StatIndex::Attack).copied().unwrap_or(mon.attack);
        mon.defense = b.stats.get(StatIndex::Defense).copied().unwrap_or(mon.defense);
        mon.speed = b.stats.get(StatIndex::Speed).copied().unwrap_or(mon.speed);
        mon.special = b.stats.get(StatIndex::Special).copied().unwrap_or(mon.special);
        mon.type1 = t1;
        mon.type2 = t2;
        mon.moves = moves4;
        mon.pp = [5, 5, 5, 5];
        party.set_status3(status3::TRANSFORMED);
    }
    // Volatiles → flags: clear the adapter-managed flags, then re-derive from the arena.
    party.clear_status1(status1::CONFUSED | status1::FLINCHED | status1::CHARGING_UP | status1::INVULNERABLE | status1::THRASHING_ABOUT | status1::USING_TRAPPING_MOVE | status1::STORING_ENERGY);
    party.clear_status2(status2::GETTING_PUMPED | status2::HAS_SUBSTITUTE_UP | status2::PROTECTED_BY_MIST | status2::SEEDED | status2::NEEDS_TO_RECHARGE | status2::USING_RAGE);
    party.clear_status3(status3::HAS_LIGHT_SCREEN_UP | status3::HAS_REFLECT_UP | status3::BADLY_POISONED);
    // Conversion + Disable ride scalar fields, not status bits, so clear them here and
    // re-derive from the arena below — a removed/expired volatile then clears its field.
    party.conversion_type1 = None;
    party.conversion_type2 = None;
    party.disabled_move = 0;
    party.disabled_turns_left = 0;
    for e in effects.iter().filter(|e| e.host == host) {
        match e.kind {
            PokeVolatile::FocusEnergy => party.set_status2(status2::GETTING_PUMPED),
            PokeVolatile::Substitute => party.set_status2(status2::HAS_SUBSTITUTE_UP),
            PokeVolatile::SubstituteHp { hp } => {
                party.set_status2(status2::HAS_SUBSTITUTE_UP);
                party.substitute_hp = hp.min(255) as u8;
            }
            PokeVolatile::LightScreen => party.set_status3(status3::HAS_LIGHT_SCREEN_UP),
            PokeVolatile::Reflect => party.set_status3(status3::HAS_REFLECT_UP),
            PokeVolatile::Mist => party.set_status2(status2::PROTECTED_BY_MIST),
            PokeVolatile::LeechSeed => party.set_status2(status2::SEEDED),
            PokeVolatile::Confused { turns } => {
                party.set_status1(status1::CONFUSED);
                party.confused_turns_left = turns;
            }
            PokeVolatile::Flinched => party.set_status1(status1::FLINCHED),
            PokeVolatile::Toxic { counter } => {
                party.set_status3(status3::BADLY_POISONED);
                party.toxic_counter = counter;
            }
            PokeVolatile::Recharge => party.set_status2(status2::NEEDS_TO_RECHARGE),
            PokeVolatile::Charging { invulnerable, .. } => {
                party.set_status1(status1::CHARGING_UP);
                if invulnerable {
                    party.set_status1(status1::INVULNERABLE);
                }
            }
            PokeVolatile::LockedMove { turns_left, .. } => {
                party.set_status1(status1::THRASHING_ABOUT);
                party.num_attacks_left = turns_left;
            }
            PokeVolatile::Rage => party.set_status2(status2::USING_RAGE),
            PokeVolatile::Trapping { turns_left, .. } => {
                party.set_status1(status1::USING_TRAPPING_MOVE);
                party.num_attacks_left = turns_left;
            }
            PokeVolatile::Bide { turns_left, accumulated } => {
                party.set_status1(status1::STORING_ENERGY);
                party.num_attacks_left = turns_left;
                party.bide_accumulated_damage = accumulated;
            }
            PokeVolatile::TypeOverride { type1, type2 } => {
                party.conversion_type1 = Some(type1);
                party.conversion_type2 = Some(type2);
            }
            PokeVolatile::Disable { slot, turns } => {
                party.disabled_move = slot;
                party.disabled_turns_left = turns;
            }
            _ => {}
        }
    }
}

/// Write the post-turn engine state (hp / status / stat stages / volatiles) back to
/// the legacy battle state (the loop's source of truth for the UI + faint checks).
pub fn apply_engine_to_legacy(
    ls: &mut LegacyBattleState,
    state: &EngineState<PokeredRules>,
    effects: &[EffectState<PokeredRules>],
) {
    write_party(&mut ls.player, &state.player_battlers[0], effects, BattlerRef::PLAYER);
    write_party(&mut ls.enemy, &state.opponent_battlers[0], effects, BattlerRef::OPPONENT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_and_effectiveness_work() {
        let mut r = RandBattleRng;
        let _ = r.next_u8(); // does not panic
        use PokemonType as T;
        assert_eq!(effectiveness_category(T::Ground, T::Electric, T::Electric), Effectiveness::SuperEffective);
        assert_eq!(effectiveness_category(T::Normal, T::Normal, T::Normal), Effectiveness::Normal);
    }

    #[test]
    fn move_announcement_wording() {
        assert_eq!(
            move_announcement("PIKACHU", "TACKLE", true, false, Effectiveness::SuperEffective, None),
            vec!["PIKACHU used TACKLE!", "Critical hit!", "It's super effective!"]
        );
        assert_eq!(
            move_announcement("SNORLAX", "TACKLE", false, false, Effectiveness::Normal, Some("is fast asleep!")),
            vec!["SNORLAX is fast asleep!"]
        );
    }

    #[test]
    fn secondary_effect_text_lines() {
        use crate::battle::state::{new_battle_state, BattleType};
        use pokered_data::species::Species;
        let mk = |sp| {
            crate::pokemon::stats::create_pokemon_with_moves(
                sp,
                30,
                [0xFF, 0xFF],
                [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let ls =
            new_battle_state(BattleType::Wild, vec![mk(Species::Pikachu)], vec![mk(Species::Snorlax)]);
        let (state, _fx) = engine_state_from_legacy(&ls);

        let mut log = TurnLog::new();
        log.push(TurnEvent::StatusInflicted {
            target: BattlerRef::OPPONENT,
            status: LegacyStatus::Poison,
        });
        log.push(TurnEvent::StatChanged {
            target: BattlerRef::PLAYER,
            stat: StatIndex::Attack,
            delta: 2,
        });
        log.push(TurnEvent::StatChanged {
            target: BattlerRef::OPPONENT,
            stat: StatIndex::Speed,
            delta: -1,
        });
        log.push(TurnEvent::StatusCured {
            target: BattlerRef::PLAYER,
            status: LegacyStatus::Sleep(0),
        });

        let text = translate_turn(&log, &state, &[]);
        assert!(text.iter().any(|l| l.contains("was poisoned!")), "{text:?}");
        assert!(text.iter().any(|l| l.contains("ATTACK greatly rose!")), "{text:?}");
        assert!(text.iter().any(|l| l.contains("SPEED fell!")), "{text:?}");
        assert!(text.iter().any(|l| l.contains("woke up!")), "{text:?}");
    }

    #[test]
    fn adapter_round_trips_state_and_volatiles() {
        use crate::battle::state::{new_battle_state, BattleType};
        use pokered_data::species::Species;
        let mk = |sp| {
            crate::pokemon::stats::create_pokemon_with_moves(
                sp, 30, [0xFF, 0xFF],
                [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            ).unwrap()
        };
        let mut ls = new_battle_state(BattleType::Wild, vec![mk(Species::Pikachu)], vec![mk(Species::Snorlax)]);
        ls.player.active_mon_mut().hp = 40;
        ls.player.active_mon_mut().status = LegacyStatus::Burn;
        ls.player.stat_stages.attack = 2;
        ls.player.set_status1(status1::CONFUSED);
        ls.player.confused_turns_left = 3;
        ls.player.set_status2(status2::GETTING_PUMPED);
        ls.player.set_status2(status2::NEEDS_TO_RECHARGE);
        ls.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
        ls.enemy.substitute_hp = 22;
        ls.enemy.set_status3(status3::BADLY_POISONED);
        ls.enemy.toxic_counter = 2;
        ls.enemy.set_status1(status1::CHARGING_UP);
        ls.enemy.set_status1(status1::INVULNERABLE);
        ls.enemy.selected_move = MoveId::Fly;

        // legacy → engine reflects hp/status/stage + volatiles.
        let (state, effects) = engine_state_from_legacy(&ls);
        assert_eq!(state.player_battlers[0].hp, 40);
        assert_eq!(state.player_battlers[0].status, Some(LegacyStatus::Burn));
        assert_eq!(state.player_battlers[0].stat_stages.get(StatIndex::Attack).copied(), Some(2));
        assert!(effects.iter().any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Confused { turns: 3 })));
        assert!(effects.iter().any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::FocusEnergy)));
        assert!(effects.iter().any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Recharge)));
        assert!(effects.iter().any(|e| e.host == BattlerRef::OPPONENT && matches!(e.kind, PokeVolatile::SubstituteHp { hp: 22 })));
        assert!(effects.iter().any(|e| e.host == BattlerRef::OPPONENT && matches!(e.kind, PokeVolatile::Toxic { counter: 2 })));
        assert!(effects.iter().any(|e| e.host == BattlerRef::OPPONENT && matches!(e.kind, PokeVolatile::Charging { move_: MoveId::Fly, invulnerable: true })));

        // engine → a FRESH legacy state preserves everything (the round-trip).
        let mut ls2 = new_battle_state(BattleType::Wild, vec![mk(Species::Pikachu)], vec![mk(Species::Snorlax)]);
        apply_engine_to_legacy(&mut ls2, &state, &effects);
        assert_eq!(ls2.player.active_mon().hp, 40);
        assert_eq!(ls2.player.active_mon().status, LegacyStatus::Burn);
        assert_eq!(ls2.player.stat_stages.attack, 2);
        assert!(ls2.player.has_status1(status1::CONFUSED) && ls2.player.confused_turns_left == 3);
        assert!(ls2.player.has_status2(status2::GETTING_PUMPED));
        assert!(ls2.player.has_status2(status2::NEEDS_TO_RECHARGE));
        assert!(ls2.enemy.has_status2(status2::HAS_SUBSTITUTE_UP) && ls2.enemy.substitute_hp == 22);
        assert!(ls2.enemy.has_status3(status3::BADLY_POISONED) && ls2.enemy.toxic_counter == 2);
        assert!(ls2.enemy.has_status1(status1::CHARGING_UP) && ls2.enemy.has_status1(status1::INVULNERABLE));
    }

    /// Transform persists the copied identity into the legacy Pokémon exactly ONCE:
    /// species/moves/PP land on the Transform turn, and a later no-marker turn leaves
    /// them alone (PP depletes normally — the PP-reset trap is avoided).
    #[test]
    fn transform_persists_identity_once() {
        use crate::battle::state::{new_battle_state, status3, BattleType};
        use pokered_data::species::Species;
        let mk = |sp| {
            crate::pokemon::stats::create_pokemon_with_moves(
                sp, 30, [0xFF, 0xFF],
                [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            ).unwrap()
        };
        let mut ls = new_battle_state(BattleType::Wild, vec![mk(Species::Ditto)], vec![mk(Species::Tauros)]);

        // Simulate a post-Transform engine state: the player battler copied the foe's
        // identity (what transform_install does) and carries the marker.
        let mut state = EngineState::new(vec![engine_active(&ls.player)], vec![engine_active(&ls.enemy)]);
        let (sp, stats, moves) = {
            let d = &state.opponent_battlers[0];
            (d.species, d.stats.clone(), d.moves.clone())
        };
        state.player_battlers[0].species = sp;
        state.player_battlers[0].stats = stats;
        state.player_battlers[0].moves = moves;
        let effects = vec![EffectState {
            id: EffectId(1),
            host: BattlerRef::PLAYER,
            effect_order: 0,
            kind: PokeVolatile::Transformed,
        }];
        apply_engine_to_legacy(&mut ls, &state, &effects);
        assert_eq!(ls.player.active_mon().species, Species::Tauros, "species copied");
        assert_eq!(ls.player.active_mon().pp, [5, 5, 5, 5], "all PP set to 5");
        assert!(ls.player.has_status3(status3::TRANSFORMED), "TRANSFORMED bit set");

        // Deplete a PP; a later turn WITHOUT the marker must NOT reset it or the copy.
        ls.player.active_mon_mut().pp[0] = 3;
        let (state2, _fx) = engine_state_from_legacy(&ls);
        apply_engine_to_legacy(&mut ls, &state2, &[]);
        assert_eq!(ls.player.active_mon().pp[0], 3, "PP not reset on a later (no-marker) turn");
        assert_eq!(ls.player.active_mon().species, Species::Tauros, "transformed species persists");
    }
}
