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
        PoisonSideEffect1 => status_effects::apply_poison_side(state, move_data, randoms, 51),
        PoisonSideEffect2 => status_effects::apply_poison_side(state, move_data, randoms, 102),
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

        // Stat modifications — opponent (primary)
        AttackDown1Effect => stat_effects::apply_stat_down(state, 0, 1),
        DefenseDown1Effect => stat_effects::apply_stat_down(state, 1, 1),
        SpeedDown1Effect => stat_effects::apply_stat_down(state, 2, 1),
        SpecialDown1Effect => stat_effects::apply_stat_down(state, 3, 1),
        AccuracyDown1Effect => stat_effects::apply_stat_down(state, 4, 1),
        EvasionDown1Effect => stat_effects::apply_stat_down(state, 5, 1),
        AttackDown2Effect => stat_effects::apply_stat_down(state, 0, 2),
        DefenseDown2Effect => stat_effects::apply_stat_down(state, 1, 2),
        SpeedDown2Effect => stat_effects::apply_stat_down(state, 2, 2),
        SpecialDown2Effect => stat_effects::apply_stat_down(state, 3, 2),
        AccuracyDown2Effect => stat_effects::apply_stat_down(state, 4, 2),
        EvasionDown2Effect => stat_effects::apply_stat_down(state, 5, 2),

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
        ConfusionSideEffect => special_effects::apply_confusion_side(state, randoms, 26),

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
