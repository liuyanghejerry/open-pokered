//! Trainer AI system — Gen 1 faithful implementation.
//!
//! Two independent subsystems:
//! 1. **Move Choice** (`move_choice.rs`): Selects which move(s) the AI considers,
//!    using a buffer-scoring system with per-class modification layers.
//! 2. **AI Action** (`ai_action.rs`): Per-class AI routines that may use items,
//!    switch Pokémon, or use X items during battle.

pub mod ai_action;
pub mod engine_ai;
pub mod move_choice;

use pokered_data::trainer_data::TrainerClass;

/// Which modification layers to apply when scoring moves.
/// Matches `data/trainers/move_choices.asm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveChoiceLayer {
    /// Discourage status moves if player already has a status.
    Layer1,
    /// Slightly encourage stat-up / bide-range moves (only when wAILayer2Encouragement == 1).
    Layer2,
    /// Type-effectiveness based: encourage super-effective, discourage not-effective
    /// (only if a "better move" exists).
    Layer3,
}

/// Returns the ordered list of move-choice modification layers for a trainer class.
/// Matches `data/trainers/move_choices.asm` — TrainerClassMoveChoiceModifications.
pub fn move_choice_layers(class: TrainerClass) -> &'static [MoveChoiceLayer] {
    use MoveChoiceLayer::*;
    use TrainerClass::*;

    match class {
        // No modifications
        Youngster | CueBall | Nobody => &[],

        // Layer 1 only
        BugCatcher | Lass | JrTrainerM | JrTrainerF | Hiker | Biker | Engineer | Juggler
        | Tamer | BirdKeeper | Blackbelt | Rival1 | Rocket | Bruno | Brock | Channeler
        | Agatha | Gambler => {
            &[Layer1]
        }

        // Layer 1 + Layer 3
        Sailor | Burglar | Fisher | Swimmer | Beauty | Rocker | Giovanni | CooltrainerM
        | CooltrainerF | Misty | LtSurge | Erika | Koga | Blaine | Sabrina | Rival2 | Rival3
        | Lance | ProfOak => &[Layer1, Layer3],

        // Layer 1 + Layer 2
        SuperNerd | UnusedJuggler | PsychicTr | Chief | Scientist | Gentleman => &[Layer1, Layer2],

        // Layer 1 + Layer 2 + Layer 3
        Pokemaniac | Lorelei => &[Layer1, Layer2, Layer3],
    }
}

/// Per-class AI action configuration.
/// Matches `data/trainers/ai_pointers.asm` — TrainerAIPointers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainerAiConfig {
    /// How many times the AI action can fire per Pokémon (wAICount).
    pub ai_count: u8,
    /// Which AI routine to use.
    pub routine: AiRoutine,
}

/// Identifies which per-class AI action routine to run.
/// Matches the label names in `engine/battle/trainer_ai.asm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiRoutine {
    /// No-op — most trainer classes.
    Generic,
    /// 25% chance to switch Pokémon.
    Juggler,
    /// ~13% chance to use X Attack.
    Blackbelt,
    /// 25% chance to use Guard Spec.
    Giovanni,
    /// 25% chance to use X Attack.
    CooltrainerM,
    /// BUG: Always runs HP check; 10% Hyper Potion if HP < 20%, else 20% switch.
    /// (The `ret nc` after the random check is missing.)
    CooltrainerF,
    /// 25% chance to use X Defend.
    Bruno,
    /// Full Heal if active Pokémon has a status condition (up to 5 times).
    Brock,
    /// 25% chance to use X Defend.
    Misty,
    /// 25% chance to use X Speed.
    LtSurge,
    /// 50% chance, then Super Potion if HP < 10%.
    Erika,
    /// 25% chance to use X Attack.
    Koga,
    /// 25% chance to use Super Potion (no HP check!).
    Blaine,
    /// 50% chance, then Hyper Potion if HP < 10%.
    Sabrina,
    /// ~13% chance, then Potion if HP < 20%.
    Rival2,
    /// ~13% chance, then Full Restore if HP < 20%.
    Rival3,
    /// 50% chance, then Super Potion if HP < 20%.
    Lorelei,
    /// ~8% chance to switch, else 50% chance + Super Potion if HP < 25%.
    Agatha,
    /// 50% chance, then Hyper Potion if HP < 20%.
    Lance,
}

/// Returns the AI action configuration for a trainer class.
/// Matches `data/trainers/ai_pointers.asm`.
pub fn trainer_ai_config(class: TrainerClass) -> TrainerAiConfig {
    use TrainerClass::*;

    match class {
        UnusedJuggler | Juggler => TrainerAiConfig {
            ai_count: 3,
            routine: AiRoutine::Juggler,
        },
        Blackbelt => TrainerAiConfig {
            ai_count: 2,
            routine: AiRoutine::Blackbelt,
        },
        Giovanni => TrainerAiConfig {
            ai_count: 1,
            routine: AiRoutine::Giovanni,
        },
        CooltrainerM => TrainerAiConfig {
            ai_count: 2,
            routine: AiRoutine::CooltrainerM,
        },
        CooltrainerF => TrainerAiConfig {
            ai_count: 1,
            routine: AiRoutine::CooltrainerF,
        },
        Bruno => TrainerAiConfig {
            ai_count: 2,
            routine: AiRoutine::Bruno,
        },
        Brock => TrainerAiConfig {
            ai_count: 5,
            routine: AiRoutine::Brock,
        },
        Misty => TrainerAiConfig {
            ai_count: 1,
            routine: AiRoutine::Misty,
        },
        LtSurge => TrainerAiConfig {
            ai_count: 1,
            routine: AiRoutine::LtSurge,
        },
        Erika => TrainerAiConfig {
            ai_count: 1,
            routine: AiRoutine::Erika,
        },
        Koga => TrainerAiConfig {
            ai_count: 2,
            routine: AiRoutine::Koga,
        },
        Blaine => TrainerAiConfig {
            ai_count: 2,
            routine: AiRoutine::Blaine,
        },
        Sabrina => TrainerAiConfig {
            ai_count: 1,
            routine: AiRoutine::Sabrina,
        },
        Rival2 => TrainerAiConfig {
            ai_count: 1,
            routine: AiRoutine::Rival2,
        },
        Rival3 => TrainerAiConfig {
            ai_count: 1,
            routine: AiRoutine::Rival3,
        },
        Lorelei => TrainerAiConfig {
            ai_count: 2,
            routine: AiRoutine::Lorelei,
        },
        Agatha => TrainerAiConfig {
            ai_count: 2,
            routine: AiRoutine::Agatha,
        },
        Lance => TrainerAiConfig {
            ai_count: 1,
            routine: AiRoutine::Lance,
        },
        // All other classes: GenericAI (no-op), count=3
        _ => TrainerAiConfig {
            ai_count: 3,
            routine: AiRoutine::Generic,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::trainer_data::TrainerClass;

    #[test]
    fn youngster_has_no_move_layers() {
        assert!(move_choice_layers(TrainerClass::Youngster).is_empty());
    }

    #[test]
    fn bug_catcher_has_layer1_only() {
        let layers = move_choice_layers(TrainerClass::BugCatcher);
        assert_eq!(layers, &[MoveChoiceLayer::Layer1]);
    }

    #[test]
    fn sailor_has_layer1_and_layer3() {
        let layers = move_choice_layers(TrainerClass::Sailor);
        assert_eq!(layers, &[MoveChoiceLayer::Layer1, MoveChoiceLayer::Layer3]);
    }

    #[test]
    fn super_nerd_has_layer1_and_layer2() {
        let layers = move_choice_layers(TrainerClass::SuperNerd);
        assert_eq!(layers, &[MoveChoiceLayer::Layer1, MoveChoiceLayer::Layer2]);
    }

    #[test]
    fn pokemaniac_has_all_three_layers() {
        let layers = move_choice_layers(TrainerClass::Pokemaniac);
        assert_eq!(
            layers,
            &[
                MoveChoiceLayer::Layer1,
                MoveChoiceLayer::Layer2,
                MoveChoiceLayer::Layer3
            ]
        );
    }

    #[test]
    fn lorelei_has_all_three_layers() {
        let layers = move_choice_layers(TrainerClass::Lorelei);
        assert_eq!(
            layers,
            &[
                MoveChoiceLayer::Layer1,
                MoveChoiceLayer::Layer2,
                MoveChoiceLayer::Layer3
            ]
        );
    }

    #[test]
    fn juggler_ai_config() {
        let cfg = trainer_ai_config(TrainerClass::Juggler);
        assert_eq!(cfg.ai_count, 3);
        assert_eq!(cfg.routine, AiRoutine::Juggler);
    }

    #[test]
    fn unused_juggler_same_as_juggler() {
        let cfg = trainer_ai_config(TrainerClass::UnusedJuggler);
        assert_eq!(cfg.ai_count, 3);
        assert_eq!(cfg.routine, AiRoutine::Juggler);
    }

    #[test]
    fn brock_ai_config() {
        let cfg = trainer_ai_config(TrainerClass::Brock);
        assert_eq!(cfg.ai_count, 5);
        assert_eq!(cfg.routine, AiRoutine::Brock);
    }

    #[test]
    fn cooltrainer_f_ai_config() {
        let cfg = trainer_ai_config(TrainerClass::CooltrainerF);
        assert_eq!(cfg.ai_count, 1);
        assert_eq!(cfg.routine, AiRoutine::CooltrainerF);
    }

    #[test]
    fn generic_class_defaults() {
        let cfg = trainer_ai_config(TrainerClass::Youngster);
        assert_eq!(cfg.ai_count, 3);
        assert_eq!(cfg.routine, AiRoutine::Generic);
    }

    #[test]
    fn lance_ai_config() {
        let cfg = trainer_ai_config(TrainerClass::Lance);
        assert_eq!(cfg.ai_count, 1);
        assert_eq!(cfg.routine, AiRoutine::Lance);
    }

    #[test]
    fn agatha_ai_config() {
        let cfg = trainer_ai_config(TrainerClass::Agatha);
        assert_eq!(cfg.ai_count, 2);
        assert_eq!(cfg.routine, AiRoutine::Agatha);
    }

    #[test]
    fn rival3_ai_config() {
        let cfg = trainer_ai_config(TrainerClass::Rival3);
        assert_eq!(cfg.ai_count, 1);
        assert_eq!(cfg.routine, AiRoutine::Rival3);
    }
}

#[cfg(test)]
mod move_choice_layer_regression_tests {
    use super::*;

    /// data/trainers/move_choices.asm lists BIKER and GAMBLER with
    /// `move_choices 1` — Layer1 only, not the default empty set.
    #[test]
    fn biker_and_gambler_use_layer1() {
        assert_eq!(move_choice_layers(TrainerClass::Biker), &[MoveChoiceLayer::Layer1]);
        assert_eq!(move_choice_layers(TrainerClass::Gambler), &[MoveChoiceLayer::Layer1]);
    }
}
