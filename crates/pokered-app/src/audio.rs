//! Audio for the app frontend: the shared [`AudioOutput`] (device glue lives
//! in `pokered_audio::output`, on top of the generic `dotzuki_audio::output`
//! backends) plus the species-cry helper.

use pokered_audio::sfx_data::SfxId;
use pokered_data::species::Species;

pub use pokered_audio::output::AudioOutput;

/// Play a species' cry with its pitch/length modifiers (`PlayCry` in
/// home/pokemon.asm: `GetCryData` + `PlaySound`).
pub fn play_species_cry(audio: &AudioOutput, species: Species) {
    let c = pokered_data::cries::cry_data(species);
    if let Some(id) = SfxId::from_u8(c.sfx) {
        audio.play_cry(id, c.pitch, c.length);
    }
}

/// A one-update view of the existing FIGHT caller, not retained playback state.
/// Game's usual sound gate sees the resulting phase, while DirectBattle's sees
/// the entering phase. Both must beep for selection/manual ACK, never Delay65.
#[derive(Clone, Copy)]
pub(crate) struct FightNoticeSound {
    player_menu: bool,
    move_select: bool,
    notice: Option<pokered_core::battle::FightNoticeWait>,
}

impl FightNoticeSound {
    #[inline]
    pub(crate) fn before(phase: &pokered_core::battle::BattlePhase) -> Self {
        use pokered_core::battle::{BattlePhase, BattleTextBackdrop};
        Self {
            player_menu: matches!(phase, BattlePhase::PlayerMenu),
            move_select: matches!(phase, BattlePhase::MoveSelect),
            notice: match phase {
                BattlePhase::ShowingText { backdrop: BattleTextBackdrop::FightMoveNotice { wait }, .. } => Some(*wait),
                _ => None,
            },
        }
    }

    /// Preserve each frontend's other menu callers verbatim. A manual notice
    /// acknowledges only when update actually leaves it (early keys are silent).
    #[inline]
    pub(crate) fn after(self, phase: &pokered_core::battle::BattlePhase, other_menu: bool) -> bool {
        use pokered_core::battle::{BattlePhase, BattleTextBackdrop, FightNoticeWait};
        match self.notice {
            Some(FightNoticeWait::LinkDelay) => false,
            Some(FightNoticeWait::Source(_)) => false,
            Some(FightNoticeWait::Ack) => !matches!(phase, BattlePhase::ShowingText {
                backdrop: BattleTextBackdrop::FightMoveNotice { .. }, .. }),
            None => other_menu
                || (self.player_menu && matches!(phase, BattlePhase::ForcedStruggle { .. }))
                || (self.move_select && matches!(phase, BattlePhase::ShowingText {
                backdrop: BattleTextBackdrop::FightMoveNotice { .. }, .. })),
        }
    }
}

#[cfg(test)]
mod fight_notice_sound_tests {
    use super::*;
    use crate::alloc_prelude::*;
    use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
    use pokered_core::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::moves::MoveId;

    #[test]
    fn fight_caller_beeps_only_for_selection_and_actual_manual_ack() {
        let make=|species,moves| create_pokemon_with_moves(species,50,[0x9a,0x78],moves).unwrap();
        let valid_selection=FightNoticeSound::before(&BattlePhase::MoveSelect);
        assert!(!valid_selection.after(&BattlePhase::LinkWaiting,false), "retain Game's existing valid Link-action gate");
        let ordinary=BattlePhase::ShowingText { messages:vec!["An ordinary turn.".into()],current:0,
            wait_frames:10,next_phase:alloc::boxed::Box::new(BattlePhase::PlayerMenu),
            backdrop:pokered_core::battle::BattleTextBackdrop::Battle };
        assert!(!valid_selection.after(&ordinary,false), "do not broaden Game's valid-move SFX scope");
        assert!(valid_selection.after(&ordinary,true), "DirectBattle's pre-update menu gate is unchanged");
        for link in [false,true] {
            let mut player=make(Species::Snorlax,[MoveId::Headbutt,MoveId::Amnesia,MoveId::Rest,MoveId::BodySlam]);
            player.pp[0]=0;
            let enemy=make(Species::Magikarp,[MoveId::Splash,MoveId::None,MoveId::None,MoveId::None]);
            let mut screen=BattleScreen::from_parties(true,&[player],&[enemy],None);
            screen.phase=BattlePhase::PlayerMenu; screen.link_mode=link;
            let a=BattleInput { a:true,..BattleInput::none() };
            screen.update_frame(a);
            let mut accepted=Vec::new();
            // A at entry, during each wait frame and at manual readiness.
            // A single caller gate serves both frontends' pre/post menu policy.
            for frame in 0..=if link {65}else{11} {
                let sound=FightNoticeSound::before(&screen.phase);
                screen.update_frame(a);
                if sound.after(&screen.phase,matches!(screen.phase,BattlePhase::MoveSelect)) {
                    accepted.push(frame);
                }
            }
            assert_eq!(accepted,if link {vec![0]}else{vec![0,11]});
            assert!(matches!(screen.phase,BattlePhase::MoveSelect));
        }
    }

    #[test]
    fn real_game_refusal_selection_and_ack_reach_existing_pcm_sfx_owner() {
        use crate::PokemonGame;
        use dotzuki_app::{GbButton,InputState};
        use pokered_core::{data::wild_data::GameVersion,game_state::GameScreen};
        let make=|species,moves| create_pokemon_with_moves(species,50,[0x9a,0x78],moves).unwrap();
        let mut player=make(Species::Snorlax,[MoveId::Headbutt,MoveId::Amnesia,MoveId::Rest,MoveId::BodySlam]);
        player.pp[0]=0;
        let enemy=make(Species::Magikarp,[MoveId::Splash,MoveId::None,MoveId::None,MoveId::None]);
        let mut game=PokemonGame::new(GameVersion::Red);
        game.battle=BattleScreen::from_parties(true,&[player],&[enemy],None);
        game.battle.phase=BattlePhase::PlayerMenu; game.state.screen=GameScreen::Battle;
        game.audio=Some(AudioOutput::new_pcm());
        let mut a=InputState::new();a.press(GbButton::A);
        let mut b=InputState::new();b.press(GbButton::B);
        game.update(&a);game.update(&InputState::new());
        game.audio.as_ref().unwrap().stop_all();
        game.update(&a);
        assert!(matches!(game.battle.phase,BattlePhase::ShowingText{..}));
        assert_eq!(game.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id,SfxId::PressAB as u8);
        game.audio.as_ref().unwrap().stop_all();
        game.update(&b); // early ACK ignored, without another sound
        assert_eq!(game.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id,0);
        for _ in 0..10 {game.update(&InputState::new());}
        game.update(&b);
        assert!(matches!(game.battle.phase,BattlePhase::MoveSelect));
        assert_eq!(game.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id,SfxId::PressAB as u8);
    }

    #[test]
    fn real_game_no_moves_fight_beeps_once_and_ignores_waiting_buttons() {
        use crate::PokemonGame;
        use dotzuki_app::{GbButton, InputState};
        use pokered_core::{data::wild_data::GameVersion, game_state::GameScreen};

        // Only the FIGHT -> no-moves caller is new. An ordinary text or link
        // transition from PlayerMenu keeps its existing sound policy.
        let entry = FightNoticeSound::before(&BattlePhase::PlayerMenu);
        assert!(!entry.after(&BattlePhase::LinkWaiting, false));

        let make = |species, moves| {
            create_pokemon_with_moves(species, 50, [0x9a, 0x78], moves).unwrap()
        };
        let mut player = make(
            Species::Snorlax,
            [MoveId::Headbutt, MoveId::Amnesia, MoveId::Rest, MoveId::BodySlam],
        );
        player.pp = [0; 4];
        let enemy = make(
            Species::Magikarp,
            [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
        );
        let mut game = PokemonGame::new(GameVersion::Red);
        game.battle = BattleScreen::from_parties(true, &[player], &[enemy], None);
        game.battle.phase = BattlePhase::PlayerMenu;
        game.state.screen = GameScreen::Battle;
        game.audio = Some(AudioOutput::new_pcm());

        let mut a = InputState::new();
        a.press(GbButton::A);
        let mut b = InputState::new();
        b.press(GbButton::B);
        game.update(&a);
        assert!(matches!(game.battle.phase, BattlePhase::ForcedStruggle { wait_frames: 60 }));
        assert_eq!(
            game.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id,
            SfxId::PressAB as u8,
            "the initial FIGHT press must reach the real sound owner",
        );
        let state = serde_json::to_value(&game.battle.battle_state).unwrap();
        let rng = serde_json::to_value(&game.battle.rng).unwrap();
        let mut finished = false;
        let mut audible = false;
        let mut pcm = [0.0f32; 1470];
        for frame in 1..60 {
            game.update(if frame % 2 == 0 { &a } else { &b });
            assert!(matches!(game.battle.phase, BattlePhase::ForcedStruggle {
                wait_frames
            } if wait_frames == 60 - frame));
            assert_eq!(serde_json::to_value(&game.battle.battle_state).unwrap(), state);
            assert_eq!(serde_json::to_value(&game.battle.rng).unwrap(), rng);
            let audio = game.audio.as_ref().unwrap();
            let id = audio.manager.lock().unwrap().sequencer.current_sfx_id;
            if id == 0 {
                finished = true;
            } else {
                assert_eq!(id, SfxId::PressAB as u8);
                assert!(!finished, "waiting A/B restarted the completed menu sound");
            }
            audio.render_pcm(&mut pcm);
            audible |= pcm.iter().any(|sample| sample.abs() > 0.00001);
        }
        assert!(finished, "the one menu sound must finish within the automatic wait");
        assert!(audible, "the real PCM owner must produce a nonzero menu sound");
        game.update(&a);
        assert!(!matches!(game.battle.phase, BattlePhase::ForcedStruggle { .. }));
        assert_ne!(serde_json::to_value(&game.battle.rng).unwrap(), rng);
        assert_eq!(game.battle.battle_state.as_ref().unwrap().player.active_mon().pp, [0; 4]);
        let after_turn = serde_json::to_value(&game.battle.rng).unwrap();
        game.update(&InputState::new());
        assert_eq!(serde_json::to_value(&game.battle.rng).unwrap(), after_turn);
    }
}
