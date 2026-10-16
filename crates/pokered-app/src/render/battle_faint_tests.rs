//! Structural faint actions are delivered once, before sound/text; no text parse.
use super::*;
use pokered_core::battle::{FaintSide, FaintSound};
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{species::Species, trainer_data::TrainerClass};

#[test]
fn canonical_selfdestruct_slides_both_pictures_once_and_only_then_queues_the_typed_enemy_sound() {
    for reserve in [false, true] {
        let make = |species, level, move_id| {
            create_pokemon_with_moves(
                species,
                level,
                [0x9a, 0x78],
                [move_id, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let mut own = make(Species::Electrode, 60, MoveId::Selfdestruct);
        own.hp = 1;
        let mut enemy = make(Species::Magikarp, 5, MoveId::Splash);
        enemy.hp = 1;
        let mut party = vec![own];
        if reserve {
            party.push(make(Species::Pidgey, 5, MoveId::Tackle));
        }
        let mut battle =
            BattleScreen::from_parties(false, &party, &[enemy], Some(TrainerClass::Youngster));
        battle.rng = pokered_core::battle::pokered_rules::runtime::StdBattleRng::from_seed(42);
        // Follow the actual core intro and FIGHT entry. Presentation starts on
        // the field turn, just as Game enables it before accepting that input.
        let mut vfx = BattleVisualEffects::default();
        for _ in 0..2000 {
            if battle.phase == BattlePhase::PlayerMenu {
                break;
            }
            battle.update_frame(pokered_core::battle::BattleInput {
                a: true,
                ..pokered_core::battle::BattleInput::none()
            });
            vfx.update(&battle);
            let _ = vfx.take_cry_pending(); // intro cries are outside this turn's assertion
        }
        assert_eq!(battle.phase, BattlePhase::PlayerMenu);
        battle.set_presentation_enabled(true);
        vfx.animations_enabled = false; // the normal options path still has a real 30-frame wait
        let mut cues = Vec::new();
        let mut sounds = Vec::new();
        let mut last = None;
        for _ in 0..3000 {
            if battle.presentation.waiting && vfx.is_frame_stable() {
                battle.complete_move_presentation();
            }
            battle.update_frame(pokered_core::battle::BattleInput {
                a: true,
                ..pokered_core::battle::BattleInput::none()
            });
            vfx.update(&battle);
            if let Some((serial, cue)) = battle.presentation.current_faint() {
                if last != Some(serial) {
                    cues.push(cue.side);
                    assert!(if cue.side == FaintSide::Enemy {
                        vfx.enemy_exit.is_some()
                    } else {
                        vfx.player_exit.is_some()
                    });
                    last = Some(serial);
                }
            }
            if battle
                .current_message
                .as_ref()
                .is_some_and(|text| text.ends_with("fainted!"))
            {
                assert!(reserve, "no reserve returns before either faint caption");
                assert!(
                    !vfx.player_visible && !vfx.enemy_visible,
                    "EnemyFaintedText must follow BOTH completed structural slides"
                );
                assert_eq!(cues, vec![FaintSide::Enemy, FaintSide::Player]);
                assert!(vfx.player_exit.is_none() && vfx.enemy_exit.is_none());
            }
            if let Some(sound) = vfx.take_faint_sound() {
                assert_eq!(sound, FaintSound::EnemyFall);
                assert!(!vfx.enemy_visible);
                assert!(
                    vfx.enemy_exit.is_none(),
                    "sound follows actual slide completion"
                );
                sounds.push(sound);
            }
            assert!(
                vfx.take_cry_pending().is_none(),
                "silent player handler must not queue a legacy cry"
            );
            if cues.len() == 2 && !matches!(battle.phase, BattlePhase::ShowingText { .. }) {
                break;
            }
        }
        assert_eq!(cues, vec![FaintSide::Enemy, FaintSide::Player]);
        assert_eq!(sounds, vec![FaintSound::EnemyFall]);
        assert!(!vfx.player_visible && !vfx.enemy_visible);
    }
}
