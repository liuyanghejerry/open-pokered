use pokered_audio::music_data::MusicId;
use pokered_audio::sfx_data::SfxId;
use pokered_core::battle::state::{BattleType, Pokemon};
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::game_state::{GameScreen, ScreenAction};
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
        BattlePhase::BagSelect => 11,
        BattlePhase::ItemTargetSelect { .. } => 12,
        BattlePhase::ShowingText { .. } => 4,
        BattlePhase::PartySelect => 5,
        BattlePhase::PartySubMenu { .. } => 9,
        BattlePhase::PartyStats { .. } => 10,
        BattlePhase::EnemySendingNext { .. } => 6,
        BattlePhase::ShiftPrompt => 14,
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

        // Per-command move animation SFX (GetMoveSound in
        // PlayAnimation/PlaySubanimation — one play per command).
        if let Some(req) = self.battle_vfx.take_move_sfx() {
            use pokered_data::move_sfx::{get_move_sound, MoveSound};
            match get_move_sound(req.anim_move, req.sound_move, req.attacker_species) {
                Some(MoveSound::Sfx(raw)) => {
                    if let Some(id) = SfxId::from_u8(raw) {
                        self.play_sfx(id);
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
                            if let Some(id) = MusicId::from_u8(self.battle.victory_music_id()) {
                                self.play_music(id);
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
                if msg_lower.contains("super effective") {
                    self.play_sfx(SfxId::SuperEffective);
                } else if msg_lower.contains("not very effective") {
                    self.play_sfx(SfxId::NotVeryEffective);
                } else if msg_lower.ends_with("fainted!") {
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
                } else if msg_lower.contains("critical hit") {
                    self.play_sfx(SfxId::Damage);
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
        #[cfg(not(target_arch = "wasm32"))]
        {
            let tag = phase_tag(&self.battle.phase);
            if matches!(tag, 2 | 3 | 5 | 7) {
                if input.is_just_pressed(GbButton::A) {
                    self.play_sfx(SfxId::PressAB);
                }
                if input.is_just_pressed(GbButton::B) {
                    self.play_sfx(SfxId::PressAB);
                }
            }
        }

        let battle_input = BattleInput {
            up: input.is_just_pressed(GbButton::Up),
            down: input.is_just_pressed(GbButton::Down),
            left: input.is_just_pressed(GbButton::Left),
            right: input.is_just_pressed(GbButton::Right),
            a: input.is_just_pressed(GbButton::A),
            b: input.is_just_pressed(GbButton::B),
        };
        let action = self.battle.update_frame(battle_input);
        self.battle_vfx.update(&self.battle);

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
            pokered_core::game_state::Lang::default(),
        );
    }

    fn should_exit(&self) -> bool {
        self.exit_requested
    }
}
