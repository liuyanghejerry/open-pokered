pub mod damage_effects;
pub mod field_effects;
pub mod multi_hit_effects;
pub mod multi_turn_effects;
pub mod special_effects;
pub mod stat_effects;
pub mod status_effects;

use pokered_data::move_data::MoveData;
use pokered_data::moves::MoveEffect;

use super::state::BattleState;

/// Random values needed by effect handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectRandoms {
    /// 0-255: used for side-effect chance rolls
    pub side_effect_roll: u8,
    /// 0-255: used for sleep turns (& 0x7), confusion turns (& 0x3), etc.
    pub duration_roll: u8,
    /// 0-255: used for multi-hit count determination
    pub multi_hit_roll: u8,
    /// 0-255: used for the primary stat-down moves' extra 25% miss roll in
    /// regular battles (engine/battle/effects.asm:553, `cp 25 percent + 1`).
    pub stat_down_miss_roll: u8,
}

/// Result of applying a move effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectResult {
    /// No additional effect (or effect didn't trigger)
    NoEffect,
    /// Status was inflicted on target
    StatusInflicted(StatusEffectType),
    /// Status failed (immune, already has status, etc.)
    StatusFailed,
    /// Stat stage was modified
    StatModified { stat: u8, stages: i8 },
    /// Stat modification was blocked (Mist, already at cap)
    StatBlocked,
    /// HP was drained from target and healed to attacker
    HpDrained { drained: u16 },
    /// Recoil damage to attacker
    RecoilDamage { recoil: u16 },
    /// One-hit KO succeeded
    OhkoSuccess,
    /// One-hit KO failed (level check)
    OhkoFailed,
    /// User exploded (HP set to 0)
    Exploded,
    /// Field effect was set up
    FieldEffectSet,
    /// Field effect already active
    FieldEffectAlreadyActive,
    /// Flinch was applied
    FlinchApplied,
    /// Confusion was applied
    ConfusionApplied,
    /// Target was seeded
    Seeded,
    /// Substitute was created
    SubstituteCreated { hp_cost: u16 },
    /// Substitute failed (not enough HP or already has one)
    SubstituteFailed,
    /// Pay Day — coins scattered
    PayDay { coins: u16 },
    /// Conversion — types changed
    TypesChanged,
    /// Haze — all stats reset
    HazeReset,
    /// Heal effect
    Healed { amount: u16 },
    /// Transform succeeded
    Transformed,
    /// Mimic — move copied
    MoveCopied,
    /// Disable applied
    Disabled,
    /// Switch/Teleport — battle ended
    SwitchedOut,
    /// Splash — nothing happened
    NothingHappened,
    /// Multi-turn move started charging / continuing
    MultiTurnContinue,
    /// Rage activated
    RageActivated,
    /// Hyper Beam recharge needed
    MustRecharge,
    /// Jump Kick crash damage
    CrashDamage { damage: u16 },
    /// Special damage (fixed: Seismic Toss, Night Shade, Dragon Rage, Sonic Boom, Psywave)
    SpecialDamageDealt { damage: u16 },
    /// Super Fang — half HP
    SuperFangDamage { damage: u16 },
    /// Dream Eater healed attacker
    DreamEaterHealed { drained: u16 },
    /// Dream Eater failed — target not asleep
    DreamEaterFailed,
    /// Mirror Move — needs to re-execute the mirrored move
    MirrorMove {
        mirrored_move: pokered_data::moves::MoveId,
    },
    /// Metronome — picked a random move to execute
    MetronomeMove {
        picked_move: pokered_data::moves::MoveId,
    },
    /// The whole move missed during the effect phase (Gen-1 quirk: primary
    /// stat-down moves have an extra 25% miss roll in regular battles,
    /// engine/battle/effects.asm:553).
    Missed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusEffectType {
    Sleep,
    Poison,
    BadlyPoisoned,
    Burn,
    Freeze,
    Paralysis,
}

/// Main dispatcher: apply the move's effect after damage has been dealt.
///
/// This follows the ASM's JumpMoveEffect pattern — each MoveEffect value
/// routes to the appropriate handler.
///
/// Some effects (like OHKO, SuperFang, SpecialDamage, Explode) are handled
/// BEFORE/DURING damage calculation in move_execution.rs. This dispatcher
/// handles the POST-damage effects (status infliction, stat changes, etc.)
/// and primary-only effects (Sleep, Confusion, stat-up/down, field effects).
pub fn apply_move_effect(
    state: &mut BattleState,
    move_data: &MoveData,
    randoms: &EffectRandoms,
    damage_dealt: u16,
) -> EffectResult {
    use MoveEffect::*;
    match move_data.effect {
        NoAdditionalEffect | Effect01 | Effect1E => EffectResult::NoEffect,

        // Status infliction — primary (guaranteed, with accuracy check)
        SleepEffect => status_effects::apply_sleep(state, randoms),
        PoisonEffect => status_effects::apply_poison_primary(state, move_data),
        ParalyzeEffect => status_effects::apply_paralyze_primary(state, move_data),

        // Status infliction — side effects (chance-based, after damage)
        // Thresholds match engine/battle/effects.asm: `cp 20 percent + 1` (52),
        // `cp 40 percent + 1` (103) — roll < threshold applies the status.
        PoisonSideEffect1 => status_effects::apply_poison_side(state, move_data, randoms, 52),
        PoisonSideEffect2 => status_effects::apply_poison_side(state, move_data, randoms, 103),
        BurnSideEffect1 => status_effects::apply_burn_side(state, move_data, randoms, 26),
        BurnSideEffect2 => status_effects::apply_burn_side(state, move_data, randoms, 77),
        FreezeSideEffect1 => status_effects::apply_freeze_side(state, move_data, randoms, 26),
        FreezeSideEffect2 => status_effects::apply_freeze_side(state, move_data, randoms, 77),
        ParalyzeSideEffect1 => status_effects::apply_paralyze_side(state, move_data, randoms, 26),
        ParalyzeSideEffect2 => status_effects::apply_paralyze_side(state, move_data, randoms, 77),

        // Stat modifications — self (primary)
        AttackUp1Effect => stat_effects::apply_stat_up(state, 0, 1),
        DefenseUp1Effect => stat_effects::apply_stat_up(state, 1, 1),
        SpeedUp1Effect => stat_effects::apply_stat_up(state, 2, 1),
        SpecialUp1Effect => stat_effects::apply_stat_up(state, 3, 1),
        AccuracyUp1Effect => stat_effects::apply_stat_up(state, 4, 1),
        EvasionUp1Effect => stat_effects::apply_stat_up(state, 5, 1),
        AttackUp2Effect => stat_effects::apply_stat_up(state, 0, 2),
        DefenseUp2Effect => stat_effects::apply_stat_up(state, 1, 2),
        SpeedUp2Effect => stat_effects::apply_stat_up(state, 2, 2),
        SpecialUp2Effect => stat_effects::apply_stat_up(state, 3, 2),
        AccuracyUp2Effect => stat_effects::apply_stat_up(state, 4, 2),
        EvasionUp2Effect => stat_effects::apply_stat_up(state, 5, 2),

        // Stat modifications — opponent (primary).
        // Gen-1 quirk: in a regular battle these moves get an extra 25% miss
        // roll (effects.asm:553) before the Mist/substitute checks.
        AttackDown1Effect => stat_effects::apply_stat_down_primary(state, 0, 1, randoms),
        DefenseDown1Effect => stat_effects::apply_stat_down_primary(state, 1, 1, randoms),
        SpeedDown1Effect => stat_effects::apply_stat_down_primary(state, 2, 1, randoms),
        SpecialDown1Effect => stat_effects::apply_stat_down_primary(state, 3, 1, randoms),
        AccuracyDown1Effect => stat_effects::apply_stat_down_primary(state, 4, 1, randoms),
        EvasionDown1Effect => stat_effects::apply_stat_down_primary(state, 5, 1, randoms),
        AttackDown2Effect => stat_effects::apply_stat_down_primary(state, 0, 2, randoms),
        DefenseDown2Effect => stat_effects::apply_stat_down_primary(state, 1, 2, randoms),
        SpeedDown2Effect => stat_effects::apply_stat_down_primary(state, 2, 2, randoms),
        SpecialDown2Effect => stat_effects::apply_stat_down_primary(state, 3, 2, randoms),
        AccuracyDown2Effect => stat_effects::apply_stat_down_primary(state, 4, 2, randoms),
        EvasionDown2Effect => stat_effects::apply_stat_down_primary(state, 5, 2, randoms),

        // Stat-down side effects (33% chance, after damage)
        AttackDownSideEffect => stat_effects::apply_stat_down_side(state, 0, randoms),
        DefenseDownSideEffect => stat_effects::apply_stat_down_side(state, 1, randoms),
        SpeedDownSideEffect => stat_effects::apply_stat_down_side(state, 2, randoms),
        SpecialDownSideEffect => stat_effects::apply_stat_down_side(state, 3, randoms),

        // Damage variant effects
        DrainHpEffect => damage_effects::apply_drain(state, damage_dealt),
        DreamEaterEffect => damage_effects::apply_dream_eater(state, damage_dealt),
        RecoilEffect => damage_effects::apply_recoil(state, damage_dealt, move_data.id),
        ExplodeEffect => damage_effects::apply_explode(state),
        OhkoEffect => EffectResult::NoEffect, // handled in move_execution
        SuperFangEffect => EffectResult::NoEffect, // handled in damage calc
        SpecialDamageEffect => EffectResult::NoEffect, // handled in damage calc
        JumpKickEffect => EffectResult::NoEffect, // crash handled in move_execution

        // Flinch side effects
        FlinchSideEffect1 => special_effects::apply_flinch_side(state, randoms, 26),
        FlinchSideEffect2 => special_effects::apply_flinch_side(state, randoms, 77),

        // Confusion
        ConfusionEffect => special_effects::apply_confusion_primary(state, randoms),
        // Ref: `cp 10 percent; ret nc` (effects.asm:1116) — no `+1`, so 25.
        ConfusionSideEffect => special_effects::apply_confusion_side(state, randoms, 25),

        // Multi-hit effects
        TwoToFiveAttacksEffect => multi_hit_effects::apply_two_to_five(state, randoms),
        AttackTwiceEffect => multi_hit_effects::apply_attack_twice(state),
        TwineedleEffect => multi_hit_effects::apply_twineedle(state, randoms),

        // Multi-turn effects
        ChargeEffect => multi_turn_effects::apply_charge(state, move_data),
        FlyEffect => multi_turn_effects::apply_fly(state, move_data),
        TrappingEffect => multi_turn_effects::apply_trapping(state, randoms),
        BideEffect => multi_turn_effects::apply_bide(state, randoms),
        ThrashPetalDanceEffect => multi_turn_effects::apply_thrash(state, randoms),
        RageEffect => multi_turn_effects::apply_rage(state),
        HyperBeamEffect => multi_turn_effects::apply_hyper_beam(state),

        // Field effects
        MistEffect => field_effects::apply_mist(state),
        FocusEnergyEffect => field_effects::apply_focus_energy(state),
        LightScreenEffect => field_effects::apply_light_screen(state),
        ReflectEffect => field_effects::apply_reflect(state),
        LeechSeedEffect => field_effects::apply_leech_seed(state, move_data),
        HazeEffect => field_effects::apply_haze(state),
        SubstituteEffect => field_effects::apply_substitute(state),
        ConversionEffect => field_effects::apply_conversion(state, move_data),
        HealEffect => field_effects::apply_heal(state),

        // Special effects
        TransformEffect => special_effects::apply_transform(state),
        MimicEffect => special_effects::apply_mimic(state),
        MetronomeEffect => special_effects::apply_metronome(randoms),
        MirrorMoveEffect => special_effects::apply_mirror_move(state),
        DisableEffect => special_effects::apply_disable(state, randoms),
        SplashEffect => EffectResult::NothingHappened,
        PayDayEffect => special_effects::apply_pay_day(state, damage_dealt),
        SwitchAndTeleportEffect => special_effects::apply_switch_teleport(state),
        SwiftEffect => EffectResult::NoEffect, // handled in accuracy check
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::*;
    use pokered_data::moves::{MoveEffect, MoveId};
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn make_pokemon() -> Pokemon {
        Pokemon {
            species: Species::Pikachu,
            nickname: [0x50; 11],
            level: 50,
            hp: 200,
            max_hp: 200,
            attack: 100,
            defense: 80,
            speed: 110,
            special: 80,
            type1: PokemonType::Normal,
            type2: PokemonType::Normal,
            moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            pp: [35, 0, 0, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: [0x50; 11],
        }
    }

    fn make_state() -> BattleState {
        new_battle_state(BattleType::Wild, vec![make_pokemon()], vec![make_pokemon()])
    }

    fn move_data(effect: MoveEffect) -> MoveData {
        MoveData {
            id: MoveId::Tackle,
            effect,
            power: 0,
            move_type: PokemonType::Normal,
            accuracy: 255,
            pp: 35,
        }
    }

    fn randoms(side_effect_roll: u8, stat_down_miss_roll: u8) -> EffectRandoms {
        EffectRandoms {
            side_effect_roll,
            duration_roll: 0,
            multi_hit_roll: 0,
            stat_down_miss_roll,
        }
    }

    #[test]
    fn poison_side_thresholds_match_asm() {
        // effects.asm:101 `cp 20 percent + 1` → 52: roll 51 fires, 52 does not.
        let mut state = make_state();
        let r = randoms(51, 255);
        let res = apply_move_effect(&mut state, &move_data(MoveEffect::PoisonSideEffect1), &r, 10);
        assert_eq!(res, EffectResult::StatusInflicted(StatusEffectType::Poison));

        let mut state = make_state();
        let r = randoms(52, 255);
        let res = apply_move_effect(&mut state, &move_data(MoveEffect::PoisonSideEffect1), &r, 10);
        assert_eq!(res, EffectResult::NoEffect);

        // effects.asm:104 `cp 40 percent + 1` → 103: roll 102 fires, 103 does not.
        let mut state = make_state();
        let r = randoms(102, 255);
        let res = apply_move_effect(&mut state, &move_data(MoveEffect::PoisonSideEffect2), &r, 10);
        assert_eq!(res, EffectResult::StatusInflicted(StatusEffectType::Poison));

        let mut state = make_state();
        let r = randoms(103, 255);
        let res = apply_move_effect(&mut state, &move_data(MoveEffect::PoisonSideEffect2), &r, 10);
        assert_eq!(res, EffectResult::NoEffect);
    }

    #[test]
    fn confusion_side_threshold_matches_asm() {
        // effects.asm:1116 `cp 10 percent` (no +1) → 25: roll 24 fires, 25 not.
        let mut state = make_state();
        let r = randoms(24, 255);
        let res = apply_move_effect(&mut state, &move_data(MoveEffect::ConfusionSideEffect), &r, 10);
        assert_eq!(res, EffectResult::ConfusionApplied);

        let mut state = make_state();
        let r = randoms(25, 255);
        let res = apply_move_effect(&mut state, &move_data(MoveEffect::ConfusionSideEffect), &r, 10);
        assert_eq!(res, EffectResult::NoEffect);
    }

    #[test]
    fn stat_down_primary_extra_miss_quirk() {
        // effects.asm:553 `cp 25 percent + 1` → 64: roll 63 misses, 64 lands.
        let mut state = make_state();
        let r = randoms(0, 63);
        let res = apply_move_effect(&mut state, &move_data(MoveEffect::AttackDown1Effect), &r, 0);
        assert_eq!(res, EffectResult::Missed);
        assert!(state.move_missed);

        let mut state = make_state();
        let r = randoms(0, 64);
        let res = apply_move_effect(&mut state, &move_data(MoveEffect::AttackDown1Effect), &r, 0);
        assert_eq!(res, EffectResult::StatModified { stat: 0, stages: -1 });

        // Link battles skip the quirk roll (effects.asm:549-551).
        let mut state = make_state();
        state.link_battle = true;
        let r = randoms(0, 0);
        let res = apply_move_effect(&mut state, &move_data(MoveEffect::AttackDown1Effect), &r, 0);
        assert_eq!(res, EffectResult::StatModified { stat: 0, stages: -1 });
    }
}
