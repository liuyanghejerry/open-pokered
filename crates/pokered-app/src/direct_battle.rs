use crate::alloc_prelude::*;
use pokered_audio::music_data::MusicId;
use pokered_audio::sfx_data::SfxId;
use pokered_core::battle::state::{BattleType, Pokemon};
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::game_state::{GameScreen, Lang, ScreenAction};
use pokered_data::species::Species;
use pokered_data::trainer_data::TrainerClass;
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::resource::{AssetRoot, ResourceManager};
use pokered_renderer::window::GameLoop;
use pokered_renderer::{FrameBuffer, Rgba};

use crate::audio::{play_species_cry, AudioOutput};
use crate::render::{draw_battle, BattleVisualEffects};

fn phase_tag(phase: &BattlePhase) -> u8 {
    match phase {
        BattlePhase::Intro { .. } => 1,
        BattlePhase::PlayerMenu => 2,
        BattlePhase::MoveSelect => 3,
        // Ether's per-move pick renders through the move-menu path (tag 3).
        BattlePhase::ItemMoveSelect { .. } => 3,
        BattlePhase::BagSelect => 11,
        BattlePhase::ItemTargetSelect { .. } => 12,
        BattlePhase::ItemMedicineResult { .. } => 18,
        BattlePhase::ShowingText { .. } | BattlePhase::EnemyFreeTurnAfterItem => 4,
        BattlePhase::PartySelect => 5,
        BattlePhase::PartySubMenu { .. } => 9,
        BattlePhase::PartyStats { .. } => 10,
        BattlePhase::EnemySendingNext { .. } => 6,
        BattlePhase::ShiftPrompt => 14,
        // The learn-move chain renders through the shift-prompt (YES/NO) and
        // move-menu (forget list) paths.
        BattlePhase::LearnMoveAsk { .. } | BattlePhase::LearnMoveGiveUpConfirm { .. } => 14,
        BattlePhase::LearnMoveChoose { .. } => 3,
        BattlePhase::ShiftSwitchSelect => 15,
        // Forced-struggle countdown behaves like a text wait (enemy still visible).
        BattlePhase::ForcedStruggle { .. } => 17,
        BattlePhase::PlayerFaintSwitch => 7,
        BattlePhase::TrainerVictory { .. } => 13,
        BattlePhase::BattleOver { .. } => 8,
        // Never reached in direct battles (link battles run through the main
        // game); a distinct tag keeps the debug log unambiguous if it ever is.
        BattlePhase::LinkWaiting => 16,
    }
}

pub struct DirectBattleGame {
    pub battle: BattleScreen,
    pub resources: Option<ResourceManager>,
    pub battle_vfx: BattleVisualEffects,
    pub exit_requested: bool,
    /// Text language for battle menus/messages (see `--lang`).
    lang: Lang,

    #[cfg(not(target_arch = "wasm32"))]
    audio: Option<AudioOutput>,
    prev_message: Option<String>,
    prev_phase_tag: u8,
    music_started: bool,
    prev_enemy_species: Species,
    prev_player_species: Species,
    end_music_played: bool,
    cry_played_for_msg: bool,
    /// SFX_FAINT_FALL has played for an enemy faint in a trainer battle;
    /// SFX_FAINT_THUD follows once the fall finishes (engine/battle/core.asm:782-791).
    faint_thud_pending: bool,
}

impl DirectBattleGame {
    pub fn new(
        battle_type: BattleType,
        player_party: Vec<Pokemon>,
        enemy_party: Vec<Pokemon>,
        trainer_class: Option<TrainerClass>,
    ) -> Self {
        let is_wild = battle_type == BattleType::Wild;
        let battle =
            BattleScreen::from_parties(is_wild, &player_party, &enemy_party, trainer_class);

        let resources = match AssetRoot::auto_detect() {
            Ok(root) => {
                eprintln!("Asset root found: {:?}", root.gfx_dir());
                Some(ResourceManager::new(root))
            }
            Err(e) => {
                eprintln!("Warning: Could not find gfx/ directory: {}", e);
                eprintln!("Falling back to text-only placeholder rendering.");
                None
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        let audio = match AudioOutput::new() {
            Some(ao) => {
                eprintln!("Audio output initialized (cpal 44100 Hz stereo)");
                Some(ao)
            }
            None => {
                eprintln!("Warning: Could not initialize audio output.");
                None
            }
        };

        let player_species = battle.player_species;
        let enemy_species = battle.enemy_species;

        Self {
            battle,
            resources,
            battle_vfx: BattleVisualEffects::default(),
            exit_requested: false,
            lang: Lang::default(),
            #[cfg(not(target_arch = "wasm32"))]
            audio,
            prev_message: None,
            prev_phase_tag: 0,
            music_started: false,
            prev_enemy_species: enemy_species,
            prev_player_species: player_species,
            end_music_played: false,
            cry_played_for_msg: false,
            faint_thud_pending: false,
        }
    }

    /// Set the text language used by the battle renderer (`--lang`).
    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
        self.battle.is_zh = lang == Lang::Zh;
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn play_sfx(&self, id: SfxId) {
        if let Some(ref audio) = self.audio {
            audio.play_sfx(id);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn play_music(&self, id: MusicId) {
        if let Some(ref audio) = self.audio {
            audio.play_music(id);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn play_cry(&self, species: Species) {
        if species != Species::None {
            if let Some(ref audio) = self.audio {
                play_species_cry(audio, species);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn update_audio(&mut self) {
        if let Some(ref audio) = self.audio {
            audio.update_frame();
            // wLowHealthAlarm: re-evaluated every frame from the player
            // mon's HP (DrawPlayerHUDAndHPBar, engine/battle/core.asm:1851-1875).
            audio.set_low_health_alarm(self.battle.low_health_alarm());
        }

        // In-battle POKé FLUTE jingle (Music_PokeFluteInBattle,
        // audio/poke_flute.asm) — requested by use_poke_flute when the
        // flute wakes at least one sleeping Pokémon
        // (engine/items/item_effects.asm:1732-1739).
        if self.battle.take_poke_flute_sfx_pending() {
            if let Some(ref audio) = self.audio {
                audio.play_flute_in_battle();
            }
        }
        if let Some(sfx) = self.battle.take_item_sfx_pending() {
            self.play_sfx(match sfx {
                pokered_core::battle::BattleItemSfx::HealHp => SfxId::HealHP,
                pokered_core::battle::BattleItemSfx::HealAilment => SfxId::HealAilment,
            });
        }

        // Non-move battle animation requests (ball throws, X-stat items)
        // queued by the core this frame.
        while let Some(event) = self.battle.take_anim_event() {
            self.battle_vfx.on_anim_event(event);
        }
        // Trainer-appear SFX (plain SFX_SILPH_SCOPE) + ball-flow SFX
        // (BallToss / Tink / BallPoof).
        if self.battle_vfx.take_trainer_appear_sfx_pending() {
            self.play_sfx(SfxId::SilphScope);
        }
        while let Some(sfx) = self.battle_vfx.take_ball_sfx() {
            self.play_sfx(sfx);
        }

        // Pokémon cries queued by the visual-effects layer (send-out cries,
        // the player faint cry from RemoveFaintedPlayerMon).
        if let Some(species) = self.battle_vfx.take_cry_pending() {
            self.play_cry(species);
        }

        // SFX_FAINT_THUD follows SFX_FAINT_FALL once the fall has finished
        // (PlaySoundWaitForCurrent → wait → PlaySound, core.asm:782-791).
        if self.faint_thud_pending {
            let playing = self
                .audio
                .as_ref()
                .is_some_and(|a| a.is_sfx_playing());
            if !playing {
                self.play_sfx(SfxId::FaintThud);
                self.faint_thud_pending = false;
            }
        }

        if let Some(effectiveness) = self.battle_vfx.take_applying_attack_sound() {
            if let Some(audio) = &self.audio { audio.play_applying_attack_sound(effectiveness as u8); }
        }

        // Per-command move animation SFX (GetMoveSound in
        // PlayAnimation/PlaySubanimation — one play per command).
        if let Some(req) = self.battle_vfx.take_move_sfx() {
            use pokered_data::move_sfx::{get_move_sound, MoveSound};
            match get_move_sound(req.anim_move, req.sound_move, req.attacker_species) {
                Some(MoveSound::Sfx(row)) => {
                    if let (Some(audio), Some(id)) = (&self.audio, SfxId::from_u8(row.sfx)) {
                        audio.play_move_sfx(id, row.pitch_mod, row.tempo_mod);
                    }
                }
                Some(MoveSound::Cry {
                    species,
                    pitch_mod,
                    tempo_mod,
                }) => {
                    // GetCryData sets the cry's modifiers, then GetMoveSound
                    // adds the command's table bytes.
                    if let Some(ref audio) = self.audio {
                        let c = pokered_data::cries::cry_data(species);
                        if let Some(id) = SfxId::from_u8(c.sfx) {
                            audio.play_cry(
                                id,
                                c.pitch.wrapping_add(pitch_mod),
                                c.length.wrapping_add(tempo_mod),
                            );
                        }
                    }
                }
                None => {}
            }
        }

        if !self.audio.as_ref().is_some_and(|audio| audio.is_sfx_playing() && !audio.low_health_alarm_active()) {
            if let Some(sound) = self.battle_vfx.take_faint_sound() {
                use pokered_core::battle::FaintSound;
                match sound {
                    FaintSound::OwnCry => self.play_cry(self.battle.player_species),
                    FaintSound::EnemyFall => {
                        self.play_sfx(SfxId::FaintFall);
                        self.faint_thud_pending = true;
                    }
                    FaintSound::WildVictory => {
                        if let Some(id) = MusicId::from_u8(self.battle.victory_music_id()) {
                            self.play_music(id);
                        }
                        self.battle_vfx.victory_music_played = true;
                    }
                }
            }
        }

        let cur_phase_tag = phase_tag(&self.battle.phase);
        let phase_changed = cur_phase_tag != self.prev_phase_tag;
        let cur_message = self.battle.current_message.clone();
        let message_changed = cur_message != self.prev_message;

        if phase_changed {
            match self.battle.phase {
                BattlePhase::Intro { .. } => {
                    if !self.music_started {
                        if let Some(id) = MusicId::from_u8(self.battle.battle_music_id()) {
                            self.play_music(id);
                        }
                        self.music_started = true;
                    }
                }
                BattlePhase::EnemySendingNext { .. } => {
                    let new_species = self.battle.enemy_species;
                    if new_species != self.prev_enemy_species {
                        self.play_cry(new_species);
                        self.prev_enemy_species = new_species;
                    }
                }
                BattlePhase::BattleOver { won, .. } => {
                    if !self.end_music_played {
                        if won {
                            if !self.battle_vfx.victory_music_played {
                                if let Some(id) = MusicId::from_u8(self.battle.victory_music_id()) {
                                    self.play_music(id);
                                }
                            }
                        } else {
                            if let Some(ref audio) = self.audio {
                                audio.stop_music();
                            }
                        }
                        self.end_music_played = true;
                    }
                }
                _ => {}
            }
        }

        if message_changed {
            self.cry_played_for_msg = false;

            if let Some(ref msg) = cur_message {
                let msg_lower = msg.to_lowercase();

                // Move-use SFX are per-command animation sounds
                // (take_move_sfx below), not message triggers.
                if self.battle.presentation.current_faint().is_none() && msg_lower.ends_with("fainted!") {
                    // HandleEnemyMonFainted: trainer battles play
                    // SFX_FAINT_FALL then SFX_FAINT_THUD; wild battles play
                    // the victory music instead. A PLAYER faint plays the
                    // mon's own cry (queued by battle_vfx cry_pending).
                    if msg_lower.starts_with("enemy ") && !self.battle.is_wild {
                        self.play_sfx(SfxId::FaintFall);
                        self.faint_thud_pending = true;
                    }
                } else if msg.starts_with("Go! ") || msg.starts_with("Go, ") {
                    if !self.cry_played_for_msg {
                        self.play_cry(self.battle.player_species);
                        self.cry_played_for_msg = true;
                    }
                    self.prev_player_species = self.battle.player_species;
                } else if msg_lower.contains("come back") || msg_lower.contains("enough") {
                    self.play_sfx(SfxId::WithdrawDeposit);
                } else if msg_lower.contains("appeared!") || msg_lower.contains("attacked!") {
                    // "Wild X appeared!" / "The hooked X attacked!" (fishing).
                    if !self.cry_played_for_msg {
                        self.play_cry(self.battle.enemy_species);
                        self.cry_played_for_msg = true;
                    }
                } else if msg_lower.contains("wants to fight") {
                    if !self.cry_played_for_msg {
                        self.play_cry(self.battle.enemy_species);
                        self.cry_played_for_msg = true;
                    }
                }
            }
        }

        if self.battle.player_species != self.prev_player_species {
            self.prev_player_species = self.battle.player_species;
        }
        if self.battle.enemy_species != self.prev_enemy_species {
            self.prev_enemy_species = self.battle.enemy_species;
        }

        self.prev_phase_tag = cur_phase_tag;
        self.prev_message = cur_message;
    }
}

impl GameLoop for DirectBattleGame {
    type Fb = FrameBuffer;

    fn update(&mut self, input: &InputState) {
        let fight_notice_sound = crate::audio::FightNoticeSound::before(&self.battle.phase);
        let ordinary_menu_sound = matches!(phase_tag(&self.battle.phase), 2 | 3 | 5 | 7);
        #[cfg(not(target_arch = "wasm32"))]
        if ordinary_menu_sound {
            if input.is_just_pressed(GbButton::A) { self.play_sfx(SfxId::PressAB); }
            if input.is_just_pressed(GbButton::B) { self.play_sfx(SfxId::PressAB); }
        }

        let battle_input = BattleInput {
            up: input.is_just_pressed(GbButton::Up),
            down: input.is_just_pressed(GbButton::Down),
            left: input.is_just_pressed(GbButton::Left),
            right: input.is_just_pressed(GbButton::Right),
            a: input.is_just_pressed(GbButton::A),
            b: input.is_just_pressed(GbButton::B),
        };
        self.battle.set_presentation_enabled(true);
        let faint_audio_finished = {
            #[cfg(not(target_arch = "wasm32"))]
            { !self.faint_thud_pending && !self.audio.as_ref().is_some_and(|audio|
                audio.is_sfx_playing() && !audio.low_health_alarm_active()) }
            #[cfg(target_arch = "wasm32")]
            { true }
        };
        if self.battle.presentation.waiting && self.battle_vfx.is_frame_stable()
            && (self.battle.presentation.current_faint().is_none() || faint_audio_finished)
        {
            self.battle.complete_move_presentation();
        }
        // DirectBattle has no save/options owner: this synthetic entry uses
        // the frontend's explicit Medium default, not an inferred SRAM flag.
        let text_frame = crate::battle_text_input::frame(input, Some(
            pokered_core::battle::BattleTextSettings::fight_menu(3, self.battle.link_mode),
        ));
        let mut text_receipt = pokered_core::battle::FightTextReceipt::default();
        let action = self.battle.update_frame_with_text_input(battle_input, text_frame, &mut text_receipt);
        #[cfg(not(target_arch = "wasm32"))]
        if !ordinary_menu_sound && (text_receipt.press_ab || (fight_notice_sound.after(&self.battle.phase, false)
            && (input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::B)))) {
            self.play_sfx(SfxId::PressAB);
        }
        #[cfg(not(target_arch = "wasm32"))]
        { self.battle_vfx.audio_available = self.audio.is_some();
          self.battle_vfx.sfx_playing = self.audio.as_ref()
            .is_some_and(|audio| audio.is_sfx_playing() && !audio.low_health_alarm_active()); }
        self.battle_vfx.update(&self.battle);
        #[cfg(target_arch = "wasm32")]
        {
            let _ = self.battle_vfx.take_faint_sound();
            let _ = self.battle_vfx.take_move_sfx();
            let _ = self.battle_vfx.take_applying_attack_sound();
        }

        #[cfg(not(target_arch = "wasm32"))]
        self.update_audio();

        if let ScreenAction::Transition(GameScreen::Overworld) = action {
            self.exit_requested = true;
        }
    }

    fn draw(&mut self, frame_buffer: &mut FrameBuffer) {
        frame_buffer.clear(Rgba::WHITE);
        draw_battle(
            &self.battle,
            &mut self.resources,
            frame_buffer,
            &mut self.battle_vfx,
            self.lang,
        );
    }

    fn should_exit(&self) -> bool {
        self.exit_requested
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), not(target_os = "none")))]
mod applying_sound_tests {
    use super::*;
    use pokered_core::{pokemon::stats::create_pokemon_with_moves, battle::pokered_rules::runtime::StdBattleRng};
    use pokered_data::moves::MoveId;

    #[test]
    fn direct_battle_preserves_the_animation_command_sound_modifiers() {
        std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
            let mon = |species, level, move_id| create_pokemon_with_moves(
                species, level, [0x99, 0x88],
                [move_id, MoveId::None, MoveId::None, MoveId::None],
            ).unwrap();
            let mut battle = BattleScreen::from_parties(true,
                &[mon(Species::Mew, 30, MoveId::HornAttack)],
                &[mon(Species::Rattata, 5, MoveId::Splash)], None);
            battle.phase = BattlePhase::PlayerMenu;
            battle.rng = StdBattleRng::from_seed(42);
            let mut game = DirectBattleGame {
                battle, resources: None, battle_vfx: BattleVisualEffects::default(),
                exit_requested: false, lang: Lang::En, audio: Some(AudioOutput::new_pcm()),
                prev_message: None, prev_phase_tag: 0, music_started: true,
                prev_enemy_species: Species::Rattata, prev_player_species: Species::Mew,
                end_music_played: false, cry_played_for_msg: false, faint_thud_pending: false,
            };
            let mut input = InputState::new();
            let mut pcm = vec![0.0; 1470];
            let mut heard = false;
            for tick in 0..1000 {
                input.begin_frame();
                if tick % 16 == 0 { input.press(GbButton::A); }
                else { input.release(GbButton::A); }
                game.update(&input);
                let audio = game.audio.as_ref().unwrap();
                let command = {
                    let manager = audio.manager.lock().unwrap();
                    manager.sequencer.current_sfx_id == SfxId::Battle1E as u8
                        && manager.sequencer.sfx_tempo == 0xe0
                };
                audio.render_pcm(&mut pcm);
                heard |= command && pcm.iter().any(|value| value.abs() > 0.00001);
                if game.battle.enemy_hp == 0 { break; }
            }
            assert!(heard, "the real DirectBattle owner must play the command with HornAttack's tempo");
        }).unwrap().join().unwrap();
    }

    #[test]
    fn direct_battle_uses_the_same_early_typed_sound_without_a_text_duplicate() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(|| {
            let mon=|species,level,move_id|create_pokemon_with_moves(species,level,[0x99,0x88],
                [move_id,MoveId::None,MoveId::None,MoveId::None]).unwrap();
            let mut battle=BattleScreen::from_parties(true,&[mon(Species::Mew,30,MoveId::IceBeam)],
                &[mon(Species::Gyarados,5,MoveId::Splash)],None);
            battle.phase=BattlePhase::PlayerMenu;
            battle.rng=StdBattleRng::from_seed(42);
            let mut game=DirectBattleGame {
                battle,resources:None,battle_vfx:BattleVisualEffects::default(),exit_requested:false,
                lang:Lang::En,audio:Some(AudioOutput::new_pcm()),prev_message:None,prev_phase_tag:0,
                music_started:true,prev_enemy_species:Species::Gyarados,prev_player_species:Species::Mew,
                end_music_played:false,cry_played_for_msg:false,faint_thud_pending:false,
            };
            let mut input=InputState::new();let mut cue=None;let mut hp=None;let mut caption=false;
            let mut starts=0;let mut previous=(0,0);let mut pcm=vec![0.0;1470];let mut heard=false;
            for tick in 0..1400 {
                input.begin_frame();if tick%16==0 {input.press(GbButton::A);}else{input.release(GbButton::A);}
                game.update(&input);
                let audio=game.audio.as_ref().unwrap();
                let current={let manager=audio.manager.lock().unwrap();(manager.sequencer.current_sfx_id,manager.sequencer.sfx_tempo)};
                assert_ne!(current.0,SfxId::Damage as u8,"super-effective attack must not get generic drain sound");
                if current==(SfxId::SuperEffective as u8,0x17f) {
                    if current!=previous {starts+=1;cue.get_or_insert(tick);}
                    audio.render_pcm(&mut pcm);heard|=pcm.iter().any(|v|v.abs()>0.00001);
                } else {audio.render_pcm(&mut pcm);}
                previous=current;
                if game.battle.enemy_hp<game.battle.enemy_max_hp {hp.get_or_insert(tick);}
                if game.battle.current_message.as_deref()==Some("It's super effective!") {caption=true;break;}
            }
            assert!(caption && heard);assert_eq!(starts,1);assert!(cue.unwrap()<hp.unwrap());
        }).unwrap().join().unwrap();
    }
}
