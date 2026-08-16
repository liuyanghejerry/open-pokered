use pokered_core::battle::state::{status2, status3};
use pokered_core::battle::state::StatusCondition as CoreStatus;
use pokered_core::battle::{
    BattleAnimEvent, BallAnimOutcome, BattlePhase, BattleScreen, BattleTransition as CoreTransition,
    IntroPhase,
};
use pokered_audio::sfx_data::SfxId;
use pokered_data::items::ItemId;
use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_renderer::battle_anim::{
    AnimEffect, AnimTickResult, AnimationPlayer, AnimationType, BattleEffects, MonRect, MonSide,
    ANIM_BASE_TILE_ID,
};
use pokered_renderer::battle_scene::{
    EnemyHud, PlayerHud, BallIndicators, BallStatus, StatusCondition,
};
use pokered_renderer::battle_transition::{BattleTransitionKind, BattleTransitionState};
use pokered_renderer::palette::GRAYSCALE_PALETTE;
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::sprite::SpriteLayer;
use pokered_renderer::text_renderer::{write_tiles_at, ScreenTileBuffer};
use pokered_renderer::textbox::TextBoxFrame;
use pokered_renderer::tile::{Tile, TileSet, TILE_PIXELS};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::{blit_tileset, species_to_sprite_name};

#[derive(Debug, Clone, Copy)]
struct AttackLunge {
    attacker_is_player: bool,
    frame: u8,
}

/// `AnimationMoveMonHorizontally` (Tackle/Body Slam): shift the mon 1 tile
/// toward the opponent for 3 frames.
#[derive(Debug, Clone, Copy)]
struct MoveMonH {
    side: MonSide,
    frame: u8,
}

/// Slide animation kinds. `Legacy` covers battle-flow slides (faint, switch,
/// trainer send-out); the `Se*` kinds are the faithful `_AnimationSlideMonOff`
/// / `AnimationSlideMonDown` / `_AnimationSlideMonUp` ports (one tile per
/// `wSlideMonDelay` frames).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlideKind {
    Legacy,
    /// SlideDownFaintedMonPic: BOTH sides slide DOWN 7 rows, DelayFrames 2
    /// per row (~14 frames total). The original has no horizontal faint slide.
    Faint,
    /// SE_SLIDE_MON_OFF / SE_SLIDE_ENEMY_MON_OFF: 8 tiles, 3 frames/tile.
    SeOff,
    /// SE_SLIDE_MON_HALF_OFF (Softboiled): 4 tiles, 4 frames/tile; the mon
    /// stays half-off afterwards.
    SeHalfOff,
    /// SE_SLIDE_MON_DOWN / SE_SLIDE_MON_DOWN_AND_HIDE: slide down and hide.
    SeDown,
    /// SE_SLIDE_MON_UP (Dig): rise from below.
    SeUp,
}

#[derive(Debug, Clone, Copy)]
struct SlideAnim {
    frame: u8,
    kind: SlideKind,
}

#[derive(Debug, Clone, Copy)]
struct PendingApplying {
    anim_type: AnimationType,
    attacker_is_player: bool,
}

/// A request to play one animation-command sound, surfaced to the frontend
/// (which owns the audio device). Resolved through
/// `pokered_data::move_sfx::get_move_sound` — the `GetMoveSound` port.
#[derive(Debug, Clone, Copy)]
pub struct AnimSfxRequest {
    /// The command's sound byte: a move id (0 = NO_MOVE is never emitted).
    pub sound_move: u8,
    /// `wAnimationID`: the move whose animation is playing (IsCryMove check).
    pub anim_move: MoveId,
    /// Species of the whose-turn mon (used for GROWL/ROAR cries).
    pub attacker_species: pokered_data::species::Species,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntroAnimState {
    None,
    ScreenFlash { remaining: u8 },
    SilhouetteSlide { remaining: u8, offset: i32 },
    /// Player send-out (AnimateSendingOutMon): POOF (stage 0), ball tile
    /// (stage 1, Delay3), 3×3 growth (stage 2, DelayFrames 4), 5×5 growth
    /// (stage 3, DelayFrames 5), then the full 7×7 pic + cry.
    PlayerSendOut { stage: u8, frames: u8 },
}

/// Non-move animation ids (data/moves/animations.asm) as 0-based
/// MOVE_ANIM_DATA indices (the 1-based animation id − 1).
mod non_move_anim {
    /// SHOWPIC_ANIM ($A6): SE_SHOW_ENEMY_MON_PIC — mon reappears.
    pub const SHOWPIC: usize = 0xA5;
    /// STATUS_AFFECTED_ANIM ($A7): the original flashes the whose-turn mon
    /// pic (`AnimationFlashMonPic`, engine/battle/animations.asm:1378-1387) —
    /// itself a pic redraw with no visible blink; approximated by anim 0xA7's
    /// SE_FLASH_MON_PIC ($F5) blink. SE $DD (anim 0xA6, ShowMonPic) is the
    /// old no-op mapping kept for reference.
    pub const STATUS_AFFECTED: usize = 0xA6;
    /// XSTATITEM_ANIM ($AE): light palette + spiral balls + reset palette.
    pub const XSTATITEM: usize = 0xAD;
    /// XSTATITEM_DUPLICATE_ANIM ($AF): same, for the enemy side (trainer-AI
    /// X items).
    pub const XSTATITEM_DUP: usize = 0xAE;
    /// TOSS_ANIM ($C1): SUBANIM_0_BALL_TOSS_LOW (Poké Ball).
    pub const BALL_TOSS: usize = 0xC0;
    /// SHAKE_ANIM ($C2): SUBANIM_0_BALL_SHAKE_ENEMY.
    pub const BALL_SHAKE: usize = 0xC1;
    /// POOF_ANIM ($C3): SUBANIM_0_BALL_POOF_ENEMY.
    pub const BALL_POOF: usize = 0xC2;
    /// GREATTOSS_ANIM ($C5): SUBANIM_0_BALL_TOSS_MIDDLE (Great Ball).
    pub const GREAT_TOSS: usize = 0xC4;
    /// ULTRATOSS_ANIM ($C6): SUBANIM_0_BALL_TOSS_HIGH (Ultra/Safari Ball).
    pub const ULTRA_TOSS: usize = 0xC5;
    /// HIDEPIC_ANIM ($C8): SE_HIDE_ENEMY_MON_PIC.
    pub const HIDEPIC: usize = 0xC7;
    /// ROCK_ANIM ($C9): Safari Zone rock throw.
    pub const SAFARI_ROCK: usize = 0xC8;
    /// BAIT_ANIM ($CA): Safari Zone bait throw.
    pub const SAFARI_BAIT: usize = 0xC9;
}

/// One step of the ball-throw choreography (`TossBallAnimation` +
/// `.PokeBallAnimations`, engine/battle/animations.asm:2581-2628): play one
/// non-move animation for at least `min_frames` frames. `shake` steps get
/// the `DoBallShakeSpecialEffects` treatment: SFX_TINK at the start and a
/// 40-frame hold on the first frame block before the wobble plays out.
#[derive(Debug, Clone, Copy)]
struct BallStep {
    anim: usize,
    min_frames: u8,
    shake: bool,
    sfx: Option<SfxId>,
}

/// The capture/ball-throw sequence currently playing (see
/// [`BattleAnimEvent`]). Drives `anim_player` through each [`BallStep`].
#[derive(Debug, Clone)]
struct BallChoreo {
    steps: Vec<BallStep>,
    step: usize,
    frames: u8,
    started: bool,
}

/// Build the step list for a thrown ball, mirroring `TossBallAnimation`:
/// the toss variant is chosen by the ball kind, then N entries of the
/// PokeBallAnimations table play (upper nybble of `wPokeBallAnimData`):
///   $10 dodged   → TOSS
///   $20 missed   → TOSS, POOF
///   $43 caught   → TOSS, POOF, HIDEPIC, SHAKE×3
///   $61-63 broke → TOSS, POOF, HIDEPIC, SHAKE×N, POOF, SHOWPIC
fn build_ball_choreo(ball: ItemId, shakes: u8, outcome: BallAnimOutcome) -> BallChoreo {
    let toss = BallStep {
        anim: match ball {
            ItemId::PokeBall => non_move_anim::BALL_TOSS,
            ItemId::GreatBall => non_move_anim::GREAT_TOSS,
            _ => non_move_anim::ULTRA_TOSS,
        },
        // Subanim_0BallToss*: 11 frame blocks × delay 3.
        min_frames: 33,
        shake: false,
        sfx: Some(SfxId::BallToss),
    };
    let poof = BallStep {
        anim: non_move_anim::BALL_POOF,
        // Subanim_0BallPoofEnemy: 6 frame blocks × delay 4.
        min_frames: 24,
        shake: false,
        sfx: Some(SfxId::BallPoof),
    };
    let hide = BallStep {
        anim: non_move_anim::HIDEPIC,
        min_frames: 3,
        shake: false,
        sfx: None,
    };
    let shake = BallStep {
        anim: non_move_anim::BALL_SHAKE,
        // SFX_TINK + 40-frame hold (DoBallShakeSpecialEffects) + the
        // 4-block wobble (delay 4).
        min_frames: 56,
        shake: true,
        sfx: Some(SfxId::Tink),
    };
    let show = BallStep {
        anim: non_move_anim::SHOWPIC,
        min_frames: 3,
        shake: false,
        sfx: None,
    };
    let steps = match outcome {
        BallAnimOutcome::Dodged => vec![toss],
        BallAnimOutcome::Caught => {
            let mut v = vec![toss, poof, hide];
            v.extend(std::iter::repeat(shake).take(shakes as usize));
            v
        }
        BallAnimOutcome::BrokeFree => {
            if shakes == 0 {
                vec![toss, poof]
            } else {
                let mut v = vec![toss, poof, hide];
                v.extend(std::iter::repeat(shake).take(shakes as usize));
                v.push(poof);
                v.push(show);
                v
            }
        }
    };
    BallChoreo {
        steps,
        step: 0,
        frames: 0,
        started: false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BattlePhaseKind {
    Intro,
    PlayerMenu,
    MoveSelect,
    BagSelect,
    ItemTargetSelect,
    ShowingText,
    PartySelect,
    PartySubMenu,
    PartyStats,
    EnemySendingNext,
    ShiftPrompt,
    ShiftSwitchSelect,
    PlayerFaintSwitch,
    /// Link battle: local action sent, waiting for the remote player's.
    LinkWaiting,
    TrainerVictory,
    BattleOver,
}

#[derive(Debug, Clone)]
pub struct BattleVisualEffects {
    last_phase_kind: Option<BattlePhaseKind>,
    last_intro_phase: Option<IntroPhase>,
    last_message: Option<String>,
    player_visible: bool,
    enemy_visible: bool,
    player_entry: Option<SlideAnim>,
    enemy_entry: Option<SlideAnim>,
    player_exit: Option<SlideAnim>,
    enemy_exit: Option<SlideAnim>,
    /// SE_SLIDE_MON_HALF_OFF latch: the mon stays 4 tiles off until reset.
    player_half_off: bool,
    enemy_half_off: bool,
    attack_lunge: Option<AttackLunge>,
    move_mon_h: Option<MoveMonH>,
    anim_player: AnimationPlayer,
    current_attacker_is_player: bool,
    /// `wAnimationID` for the running animation (IsCryMove checks this, not
    /// the command's sound byte).
    current_move: MoveId,
    /// Species of the whose-turn mon, captured when the animation starts.
    current_attacker_species: pokered_data::species::Species,
    anim_wait: u8,
    anim_tileset: u8,
    anim_layer: SpriteLayer,
    pending_applying: Option<PendingApplying>,
    /// Sound of the latest animation command, waiting for the frontend to
    /// play it (PlayAnimation/PlaySubanimation call GetMoveSound+PlaySound
    /// once per command).
    pending_move_sfx: Option<AnimSfxRequest>,
    /// wOptions BIT_BATTLE_ANIMATION: set by the frontend every frame.
    /// When false, move animations (and their per-command sounds) are
    /// skipped; MoveAnimation instead waits 30 frames and runs only the
    /// applying-attack feedback.
    pub animations_enabled: bool,
    /// Remaining frames of the 30-frame DelayFrames in the
    /// `.animationsDisabled` path of MoveAnimation.
    anim_disabled_wait: u8,
    /// Set by the frontend every frame: whether an SFX is still playing.
    /// MoveAnimation opens with WaitForSoundToFinish — the animation does
    /// not start until the previous sound (e.g. the send-out cry) ends.
    pub sfx_playing: bool,
    /// Animation start deferred by WaitForSoundToFinish
    /// ((animation id, player_is_attacker)).
    pending_anim_start: Option<(usize, bool)>,
    suppress_hit_flash: bool,
    /// Previous-frame HAS_SUBSTITUTE_UP flags, used to clear the doll latch
    /// when a substitute breaks (the SE itself latches the doll on).
    player_sub_flag: bool,
    enemy_sub_flag: bool,
    /// Shared framebuffer special effects (dotzuki-renderer battle_anim::effects).
    fx: BattleEffects,
    intro_anim: IntroAnimState,
    /// Active screen-wipe transition (engine/battle/battle_transitions.asm),
    /// rendered over the pre-battle overworld snapshot. `None` outside the
    /// BattleTransitionWipe intro phase.
    transition_state: Option<BattleTransitionState>,
    /// The overworld frame captured when the wipe begins; the wipe eats it
    /// tile by tile instead of flashing black (mirrors pokered-app).
    pub overworld_snapshot: Option<FrameBuffer>,
    is_wild_intro: bool,
    cry_pending: Option<pokered_data::species::Species>,
    /// SFX_SILPH_SCOPE request for the trainer-appear sound
    /// (`PrintBeginningBattleText`'s `.trainerBattle` → `.playSFX`; the
    /// wTempoModifier write is dead for non-cries, so the plain SFX plays).
    trainer_appear_sfx_pending: bool,
    /// Active ball-throw choreography (capture / ghost dodge / old man).
    ball_choreo: Option<BallChoreo>,
    /// Ball-flow SFX (BallToss / Tink per shake / BallPoof) queued for the
    /// frontend, which owns the audio device.
    pending_ball_sfx: std::collections::VecDeque<SfxId>,
}

impl BattleVisualEffects {
    pub fn take_cry_pending(&mut self) -> Option<pokered_data::species::Species> {
        self.cry_pending.take()
    }

    /// Take the pending animation-command sound request, if any.
    pub fn take_move_sfx(&mut self) -> Option<AnimSfxRequest> {
        self.pending_move_sfx.take()
    }

    /// Take the pending trainer-appear SFX request (SFX_SILPH_SCOPE,
    /// `PrintBeginningBattleText` `.trainerBattle`).
    pub fn take_trainer_appear_sfx_pending(&mut self) -> bool {
        std::mem::take(&mut self.trainer_appear_sfx_pending)
    }

    /// Take one queued ball-flow SFX (BallToss / Tink / BallPoof).
    pub fn take_ball_sfx(&mut self) -> Option<SfxId> {
        self.pending_ball_sfx.pop_front()
    }

    /// Whether a screen-wipe transition is currently active.
    pub fn has_transition(&self) -> bool {
        self.transition_state.is_some()
    }

    /// Render the active wipe over `source` (the overworld snapshot) into
    /// `dest`. Returns true once the screen is fully black.
    pub fn render_transition(&self, source: &FrameBuffer, dest: &mut FrameBuffer) -> bool {
        if let Some(ref ts) = self.transition_state {
            ts.render(source, dest)
        } else {
            false
        }
    }

    /// Drop the overworld snapshot once the wipe is over.
    pub fn clear_snapshot(&mut self) {
        self.overworld_snapshot = None;
    }

    /// Handle a core non-move animation request (see
    /// `pokered_core::battle::BattleAnimEvent`): ball throws start the
    /// staged toss/poof/shake choreography; an X-stat item plays
    /// XSTATITEM_ANIM on the player's mon.
    pub fn on_anim_event(&mut self, event: BattleAnimEvent) {
        match event {
            BattleAnimEvent::Ball {
                ball,
                shakes,
                outcome,
            } => {
                self.ball_choreo = Some(build_ball_choreo(ball, shakes, outcome));
            }
            BattleAnimEvent::XStatItem => {
                self.start_non_move_anim(non_move_anim::XSTATITEM, true);
            }
        }
    }

    /// Start one non-move animation (ids $A6+, indexed into MOVE_ANIM_DATA)
    /// through the regular animation player.
    fn start_non_move_anim(&mut self, anim_index: usize, player_is_attacker: bool) {
        self.current_attacker_is_player = player_is_attacker;
        self.current_move = MoveId::None;
        self.anim_player.start(anim_index, player_is_attacker);
        self.anim_wait = 0;
        self.anim_layer.clear();
    }
}

impl Default for BattleVisualEffects {
    fn default() -> Self {
        Self {
            last_phase_kind: None,
            last_intro_phase: None,
            last_message: None,
            player_visible: true,
            enemy_visible: true,
            player_entry: None,
            enemy_entry: None,
            player_exit: None,
            enemy_exit: None,
            player_half_off: false,
            enemy_half_off: false,
            attack_lunge: None,
            move_mon_h: None,
            anim_player: AnimationPlayer::new(),
            current_attacker_is_player: true,
            current_move: MoveId::None,
            current_attacker_species: pokered_data::species::Species::None,
            anim_wait: 0,
            anim_tileset: 0,
            anim_layer: SpriteLayer::new(),
            pending_applying: None,
            pending_move_sfx: None,
            animations_enabled: true,
            anim_disabled_wait: 0,
            sfx_playing: false,
            pending_anim_start: None,
            suppress_hit_flash: false,
            player_sub_flag: false,
            enemy_sub_flag: false,
            fx: BattleEffects::new(),
            intro_anim: IntroAnimState::None,
            transition_state: None,
            overworld_snapshot: None,
            is_wild_intro: false,
            cry_pending: None,
            trainer_appear_sfx_pending: false,
            ball_choreo: None,
            pending_ball_sfx: std::collections::VecDeque::new(),
        }
    }
}

impl BattleVisualEffects {
    fn phase_kind(phase: &BattlePhase) -> BattlePhaseKind {
        match phase {
            BattlePhase::Intro { .. } => BattlePhaseKind::Intro,
            BattlePhase::PlayerMenu => BattlePhaseKind::PlayerMenu,
            BattlePhase::MoveSelect => BattlePhaseKind::MoveSelect,
            BattlePhase::BagSelect => BattlePhaseKind::BagSelect,
            BattlePhase::ItemTargetSelect { .. } => BattlePhaseKind::ItemTargetSelect,
            BattlePhase::ShowingText { .. } => BattlePhaseKind::ShowingText,
            BattlePhase::PartySelect => BattlePhaseKind::PartySelect,
            BattlePhase::PartySubMenu { .. } => BattlePhaseKind::PartySubMenu,
            BattlePhase::PartyStats { .. } => BattlePhaseKind::PartyStats,
            BattlePhase::EnemySendingNext { .. } => BattlePhaseKind::EnemySendingNext,
            BattlePhase::ShiftPrompt => BattlePhaseKind::ShiftPrompt,
            BattlePhase::ShiftSwitchSelect => BattlePhaseKind::ShiftSwitchSelect,
            BattlePhase::ForcedStruggle { .. } => BattlePhaseKind::ShowingText,
            BattlePhase::PlayerFaintSwitch => BattlePhaseKind::PlayerFaintSwitch,
            BattlePhase::LinkWaiting => BattlePhaseKind::LinkWaiting,
            BattlePhase::TrainerVictory { .. } => BattlePhaseKind::TrainerVictory,
            BattlePhase::BattleOver { .. } => BattlePhaseKind::BattleOver,
        }
    }

    fn on_phase_change(&mut self, phase: &BattlePhase) {
        match phase {
            BattlePhase::Intro { .. } => {
                self.is_wild_intro = true;
            }
            BattlePhase::EnemySendingNext { .. } => {
                self.enemy_visible = true;
                self.enemy_entry = Some(SlideAnim { frame: 0, kind: SlideKind::Legacy });
                self.enemy_exit = None;
                self.enemy_half_off = false;
                self.fx.clear_side(MonSide::Enemy);
            }
            BattlePhase::PlayerFaintSwitch => {
                self.player_visible = true;
                self.player_entry = Some(SlideAnim { frame: 0, kind: SlideKind::Legacy });
                self.player_exit = None;
                self.player_half_off = false;
                self.fx.clear_side(MonSide::Player);
            }
            _ => {}
        }
    }

    fn on_intro_phase_change(
        &mut self,
        intro_phase: &IntroPhase,
        player_species: pokered_data::species::Species,
        enemy_species: pokered_data::species::Species,
    ) {
        match intro_phase {
            IntroPhase::BattleTransitionWipe(transition) => {
                // Real screen wipe (engine/battle/battle_transitions.asm):
                // the dotzuki-renderer port eats the overworld snapshot tile by
                // tile (circle/spiral/stripes/shrink/split), instead of the
                // previous 8-frame black flash approximation.
                let kind = match transition {
                    CoreTransition::DoubleCircle => BattleTransitionKind::DoubleCircle,
                    CoreTransition::Spiral { outward } => BattleTransitionKind::Spiral { outward: *outward },
                    CoreTransition::Circle => BattleTransitionKind::Circle,
                    CoreTransition::SpiralTrainerStronger => BattleTransitionKind::Spiral { outward: true },
                    CoreTransition::HorizontalStripes => BattleTransitionKind::HorizontalStripes,
                    CoreTransition::Shrink => BattleTransitionKind::Shrink,
                    CoreTransition::VerticalStripes => BattleTransitionKind::VerticalStripes,
                    CoreTransition::Split => BattleTransitionKind::Split,
                };
                self.transition_state = Some(BattleTransitionState::new(kind, 20, 18));
                self.intro_anim = IntroAnimState::None;
                self.player_visible = false;
                self.enemy_visible = false;
            }
            IntroPhase::TransitionFlash => {
                self.intro_anim = IntroAnimState::ScreenFlash { remaining: 8 };
                self.player_visible = false;
                self.enemy_visible = false;
            }
            IntroPhase::SilhouetteSlide => {
                self.intro_anim = IntroAnimState::SilhouetteSlide {
                    remaining: 72,
                    offset: 144,
                };
                self.player_visible = true;
                self.enemy_visible = true;
                self.player_entry = None;
                self.enemy_entry = None;
            }
            IntroPhase::WildReveal => {
                self.intro_anim = IntroAnimState::None;
                self.player_visible = false;
                self.enemy_visible = true;
                self.player_entry = None;
                self.enemy_entry = Some(SlideAnim { frame: 0, kind: SlideKind::Legacy });
                // Play wild Pokémon cry when "Wild X appeared!" is shown
                // Matches PlayCry in engine/battle/core.asm after the enemy
                // pic is loaded and before the "appeared!" text prints.
                self.cry_pending = Some(enemy_species);
            }
            // Ghost intro phases: no TUI reveal animation — static text phases.
            IntroPhase::GhostCantID | IntroPhase::GhostUnveil => {
                self.intro_anim = IntroAnimState::None;
                self.player_visible = false;
                self.enemy_visible = true;
                self.player_entry = None;
                self.enemy_entry = None;
            }
            IntroPhase::TrainerReveal => {
                self.intro_anim = IntroAnimState::None;
                self.player_visible = true;
                self.enemy_visible = true;
                self.player_entry = None;
                self.enemy_entry = None;
                // PrintBeginningBattleText's .trainerBattle plays the
                // trainer-appear SFX (SFX_SILPH_SCOPE) before
                // "X wants to fight!".
                self.trainer_appear_sfx_pending = true;
            }
            IntroPhase::TrainerSendOut => {
                self.intro_anim = IntroAnimState::None;
                self.player_visible = true;
                self.enemy_visible = true;
                self.enemy_exit = Some(SlideAnim { frame: 0, kind: SlideKind::Legacy });
                // The original has NO enemy-mon entry animation: the trainer
                // pic slides off right, then the mon simply appears.
                self.cry_pending = Some(enemy_species);
            }
            IntroPhase::PlayerSendOut => {
                // SendOutMon (engine/battle/core.asm:1723): POOF_ANIM at the
                // player's side, then AnimateSendingOutMon grows the pic
                // 3×3 → 5×5 → 7×7; the cry fires when the growth completes.
                self.intro_anim = IntroAnimState::PlayerSendOut { stage: 0, frames: 0 };
                self.player_visible = true;
                self.enemy_visible = true;
                self.player_entry = None;
                self.enemy_entry = None;
                self.start_non_move_anim(non_move_anim::BALL_POOF, false);
                let _ = player_species;
            }
        }
        // Only reset exit animations for phases that don't use them.
        // TrainerSendOut sets enemy_exit above — don't clobber it.
        if *intro_phase != IntroPhase::TrainerSendOut {
            self.player_exit = None;
            self.enemy_exit = None;
        }
    }

    fn resolve_message_move(screen: &BattleScreen, message: &str) -> Option<(usize, bool, MoveId)> {
        if !(message.contains(" used ") && message.ends_with('!')) {
            return None;
        }

        let bs = screen.battle_state.as_ref()?;
        if message.starts_with("Enemy ") {
            let move_id = bs.enemy.selected_move;
            let id = move_id as usize;
            if id > 0 {
                Some((id - 1, false, move_id))
            } else {
                None
            }
        } else {
            let move_id = bs.player.selected_move;
            let id = move_id as usize;
            if id > 0 {
                Some((id - 1, true, move_id))
            } else {
                None
            }
        }
    }

    fn classify_applying_attack(move_id: MoveId, attacker_is_player: bool) -> AnimationType {
        let Some(data) = MoveData::get(move_id) else {
            return AnimationType::None;
        };

        if data.power == 0 {
            return if attacker_is_player {
                AnimationType::ShakeScreenHorizontallySlow2
            } else {
                AnimationType::ShakeScreenHorizontallySlow
            };
        }

        if data.effect == MoveEffect::NoAdditionalEffect {
            if attacker_is_player {
                AnimationType::BlinkEnemyMonSprite
            } else {
                AnimationType::ShakeScreenVertically
            }
        } else if attacker_is_player {
            AnimationType::ShakeScreenHorizontallyLight
        } else {
            AnimationType::ShakeScreenHorizontallyHeavy
        }
    }

    fn run_applying_attack_feedback(&mut self, anim_type: AnimationType, attacker_is_player: bool) {
        // Match PlayApplyingAttackAnimation in engine/battle/animations.asm:
        // 1/2/3/5/6 are shake variants, only 4 is blink-target-sprite.
        match anim_type {
            AnimationType::None => {}
            AnimationType::ShakeScreenVertically => {
                self.apply_anim_effect(AnimEffect::ShakeScreenV {
                    pixels: 1,
                    frames: 16,
                });
            }
            AnimationType::ShakeScreenHorizontallyHeavy => {
                self.apply_anim_effect(AnimEffect::ShakeScreenH {
                    pixels: 1,
                    frames: 16,
                });
            }
            AnimationType::ShakeScreenHorizontallySlow => {
                self.apply_anim_effect(AnimEffect::ShakeScreenH {
                    pixels: 1,
                    frames: 48,
                });
            }
            AnimationType::BlinkEnemyMonSprite => {
                if attacker_is_player {
                    self.apply_anim_effect(AnimEffect::BlinkEnemyMon { times: 6 });
                } else {
                    self.apply_anim_effect(AnimEffect::BlinkPlayerMon { times: 6 });
                }
            }
            AnimationType::ShakeScreenHorizontallyLight => {
                self.apply_anim_effect(AnimEffect::ShakeScreenH {
                    pixels: 1,
                    frames: 4,
                });
            }
            AnimationType::ShakeScreenHorizontallySlow2 => {
                self.apply_anim_effect(AnimEffect::ShakeScreenH {
                    pixels: 1,
                    frames: 24,
                });
            }
        }
    }

    fn is_no_hit_feedback_message(message: &str) -> bool {
        let msg = message.to_ascii_lowercase();
        msg.contains("missed")
            || msg.contains("avoided")
            || msg.contains("no effect")
            || msg.contains("had no effect")
            || msg.contains("doesn't affect")
            || msg.contains("does not affect")
            || msg.contains("unaffected")
    }

    /// If the message is "{NAME} used {ITEM}!" where ITEM is an item
    /// display name (ball / X-stat / potion…), return the item id. Item-use
    /// lines are NOT move uses: they must not fire the attack lunge or the
    /// selected-move animation.
    fn used_item_id(message: &str) -> Option<ItemId> {
        let arg = message.split(" used ").nth(1)?.strip_suffix('!')?;
        (0..=pokered_data::items::MAX_ITEM_ID).find_map(|i| {
            let id = ItemId::from_id(i);
            pokered_data::item_data::get_item_data(id)
                .filter(|d| d.name == arg)
                .map(|_| id)
        })
    }

    /// Two-turn charge-turn narration (`charge_message` in pokered-core):
    /// the original plays STATUS_AFFECTED_ANIM (flash the whose-turn mon
    /// pic) on the charge turn — engine/battle/core.asm:3196/3475/5598/5851.
    fn is_charge_message(message: &str) -> bool {
        message.ends_with("flew up high!")
            || message.ends_with("dug a hole!")
            || message.ends_with("took in sunlight!")
            || message.ends_with("made a whirlwind!")
            || message.ends_with("lowered its head!")
            || message.ends_with("is glowing!")
            || message.ends_with("began charging!")
    }

    fn trigger_from_message(&mut self, screen: &BattleScreen, message: &str) {
        let normalized = message.replace('\n', " ");

        if Self::is_no_hit_feedback_message(&normalized) {
            // Miss / no-effect messages should not produce hit flash feedback.
            self.pending_applying = None;
            self.suppress_hit_flash = true;
        }

        // Safari Zone BAIT/ROCK (ItemUseBait/ItemUseRock,
        // engine/items/item_effects.asm:1431/1447).
        if normalized == "Threw some BAIT!" {
            self.start_non_move_anim(non_move_anim::SAFARI_BAIT, true);
        } else if normalized == "Threw a ROCK!" {
            self.start_non_move_anim(non_move_anim::SAFARI_ROCK, true);
        }

        // Two-turn charge turn: STATUS_AFFECTED_ANIM flashes the
        // whose-turn mon's pic.
        if Self::is_charge_message(&normalized) {
            let player_charging = !normalized.starts_with("Enemy ");
            self.start_non_move_anim(non_move_anim::STATUS_AFFECTED, player_charging);
        }

        if normalized.contains(" used ") && normalized.ends_with('!') {
            if let Some(item_id) = Self::used_item_id(&normalized) {
                // Item-use line — no lunge, no move animation. The trainer
                // AI's X-stat items play XSTATITEM_DUPLICATE_ANIM on the
                // enemy mon (player-side X items arrive via
                // BattleAnimEvent::XStatItem instead).
                use pokered_core::battle::menu::ItemCategory;
                if ItemCategory::from_item(item_id) == ItemCategory::BattleStat {
                    self.start_non_move_anim(non_move_anim::XSTATITEM_DUP, false);
                }
            } else {
                self.suppress_hit_flash = false;
                let enemy_attacker = normalized.starts_with("Enemy ");
                self.attack_lunge = Some(AttackLunge {
                    attacker_is_player: !enemy_attacker,
                    frame: 0,
                });

                if self.ball_choreo.is_none() {
                    if let Some((anim_id, player_is_attacker, move_id)) =
                        Self::resolve_message_move(screen, &normalized)
                    {
                        self.current_attacker_is_player = player_is_attacker;
                        self.current_move = move_id;
                        self.current_attacker_species = if player_is_attacker {
                            screen.player_species
                        } else {
                            screen.enemy_species
                        };
                        if self.animations_enabled {
                            if self.sfx_playing {
                                // MoveAnimation: WaitForSoundToFinish first — start
                                // the animation once the previous SFX has ended.
                                self.pending_anim_start = Some((anim_id, player_is_attacker));
                            } else {
                                self.anim_player.start(anim_id, player_is_attacker);
                                self.anim_wait = 0;
                                self.anim_layer.clear();
                            }
                        } else {
                            // MoveAnimation .animationsDisabled: no animation (and no
                            // per-command sounds), just DelayFrames 30 before the
                            // applying-attack feedback.
                            self.anim_disabled_wait = 30;
                        }
                        self.pending_applying = Some(PendingApplying {
                            anim_type: Self::classify_applying_attack(move_id, player_is_attacker),
                            attacker_is_player: player_is_attacker,
                        });
                    }
                }
            }
        }

        if normalized.starts_with("Go! ") {
            self.player_visible = true;
            self.player_entry = Some(SlideAnim { frame: 0, kind: SlideKind::Legacy });
            self.player_exit = None;
            self.player_half_off = false;
            self.fx.clear_side(MonSide::Player);
        }

        if normalized.contains("come back!") {
            self.player_exit = Some(SlideAnim { frame: 0, kind: SlideKind::Legacy });
            self.player_entry = None;
            self.player_half_off = false;
            self.fx.clear_side(MonSide::Player);
        }

        if normalized.ends_with("fainted!") {
            if normalized.starts_with("Enemy ") {
                // SlideDownFaintedMonPic: enemy mon slides DOWN, not right.
                self.enemy_exit = Some(SlideAnim { frame: 0, kind: SlideKind::Faint });
                self.enemy_entry = None;
                self.enemy_half_off = false;
                self.fx.clear_side(MonSide::Enemy);
            } else {
                self.player_exit = Some(SlideAnim { frame: 0, kind: SlideKind::Faint });
                self.player_entry = None;
                self.player_half_off = false;
                self.fx.clear_side(MonSide::Player);
                // RemoveFaintedPlayerMon (engine/battle/core.asm:1040-1043):
                // the player mon's own cry plays before "X fainted!" prints.
                self.cry_pending = Some(screen.player_species);
            }
        }
    }

    /// Side whose turn it is (`hWhoseTurn` in the original).
    fn attacker_side(&self) -> MonSide {
        if self.current_attacker_is_player {
            MonSide::Player
        } else {
            MonSide::Enemy
        }
    }

    fn slide_slot(&mut self, side: MonSide, entry: bool) -> &mut Option<SlideAnim> {
        match (side, entry) {
            (MonSide::Player, false) => &mut self.player_exit,
            (MonSide::Enemy, false) => &mut self.enemy_exit,
            (MonSide::Player, true) => &mut self.player_entry,
            (MonSide::Enemy, true) => &mut self.enemy_entry,
        }
    }

    fn set_visible(&mut self, side: MonSide, visible: bool) {
        match side {
            MonSide::Player => self.player_visible = visible,
            MonSide::Enemy => self.enemy_visible = visible,
        }
    }

    fn apply_anim_effect(&mut self, effect: AnimEffect) {
        let attacker = self.attacker_side();
        let defender = attacker.other();

        // Miss / no-effect messages suppress the blink feedback.
        if self.suppress_hit_flash
            && matches!(
                effect,
                AnimEffect::BlinkEnemyMon { .. }
                    | AnimEffect::BlinkPlayerMon { .. }
                    | AnimEffect::FlashEnemyMonPic
                    | AnimEffect::FlashPlayerMonPic
            )
        {
            return;
        }

        // Shared framebuffer effects (dotzuki-renderer). The returned frame
        // count is how long the original routine blocks the command stream.
        let wait = self.fx.apply(&effect, attacker);
        if wait > 0 {
            self.anim_wait = self.anim_wait.max(wait);
        }

        // Frontend-side flows: visibility, mon slides and lunges.
        match effect {
            // SE_HIDE/SHOW_MON_PIC act on whose-turn mon; the "Enemy"
            // variants use CallWithTurnFlipped. The forced-hidden latch
            // lives in `self.fx`; these flags cover the battle flow.
            AnimEffect::ShowPlayerMon | AnimEffect::SubstituteMon | AnimEffect::MinimizeMon => {
                self.set_visible(attacker, true);
            }
            AnimEffect::ShowEnemyMon => {
                self.set_visible(defender, true);
            }
            AnimEffect::SlideEnemyMonOff => {
                // AnimationSlideMonOff: e = 8 tiles, wSlideMonDelay = 3.
                *self.slide_slot(defender, false) = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::SeOff,
                });
                self.anim_wait = self.anim_wait.max(24);
            }
            AnimEffect::SlidePlayerMonOff => {
                *self.slide_slot(attacker, false) = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::SeOff,
                });
                self.anim_wait = self.anim_wait.max(24);
            }
            AnimEffect::SlidePlayerMonHalfOff => {
                // AnimationSlideMonHalfOff: e = 4 tiles, wSlideMonDelay = 4;
                // the mon stays half-off (Softboiled).
                *self.slide_slot(attacker, false) = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::SeHalfOff,
                });
                self.anim_wait = self.anim_wait.max(16);
            }
            AnimEffect::SlidePlayerMonDown => {
                *self.slide_slot(attacker, false) = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::SeDown,
                });
                self.anim_wait = self.anim_wait.max(21);
            }
            AnimEffect::SlidePlayerMonUp => {
                self.set_visible(attacker, true);
                *self.slide_slot(attacker, true) = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::SeUp,
                });
                self.anim_wait = self.anim_wait.max(21);
            }
            AnimEffect::ResetPlayerMonPosition => {
                // AnimationResetMonPosition: instantly redraw the pic at the
                // normal coordinates (no slide).
                self.move_mon_h = None;
                match attacker {
                    MonSide::Player => self.player_half_off = false,
                    MonSide::Enemy => self.enemy_half_off = false,
                }
                self.set_visible(attacker, true);
            }
            AnimEffect::MovePlayerMonH => {
                // AnimationMoveMonHorizontally: hold the mon 1 tile toward
                // the opponent for 3 frames (Tackle/Body Slam).
                self.move_mon_h = Some(MoveMonH {
                    side: attacker,
                    frame: 0,
                });
                self.anim_wait = self.anim_wait.max(3);
            }
            AnimEffect::Delay10 => {
                self.anim_wait = self.anim_wait.max(10);
            }
            _ => {}
        }
    }

    /// Queue one animation-command sound for the frontend to play
    /// (GetMoveSound + PlaySound in PlayAnimation/PlaySubanimation).
    fn emit_move_sfx(&mut self, sound_move: u8) {
        self.pending_move_sfx = Some(AnimSfxRequest {
            sound_move,
            anim_move: self.current_move,
            attacker_species: self.current_attacker_species,
        });
    }

    fn advance_move_animation(&mut self) {
        // MoveAnimation .animationsDisabled: DelayFrames 30, then the
        // applying-attack feedback (no animation, no command sounds).
        if self.anim_disabled_wait > 0 {
            self.anim_disabled_wait -= 1;
            if self.anim_disabled_wait == 0 {
                if !self.suppress_hit_flash {
                    if let Some(pending) = self.pending_applying.take() {
                        self.run_applying_attack_feedback(
                            pending.anim_type,
                            pending.attacker_is_player,
                        );
                    }
                } else {
                    self.pending_applying = None;
                }
            }
            return;
        }

        if self.anim_player.is_finished() {
            self.anim_layer.clear();
            return;
        }

        if self.anim_wait > 0 {
            self.anim_wait -= 1;
            return;
        }

        match self.anim_player.tick() {
            AnimTickResult::Playing { sound, hook } => {
                if let Some(sound_move) = sound {
                    self.emit_move_sfx(sound_move);
                }
                self.anim_layer.clear();
                for entry in self.anim_player.oam_entries() {
                    // OAM tile ids are absolute VRAM ids (raw + $31, matching
                    // DrawFrameBlock); the loaded move-anim tilesets are
                    // indexed from 0 (they are loaded at vSprites tile $31 in
                    // the original), so subtract the base for rendering.
                    let mut e = *entry;
                    e.tile_id = e.tile_id.wrapping_sub(ANIM_BASE_TILE_ID);
                    self.anim_layer.add(e);
                }
                if let Some(ts) = self.anim_player.current_tileset() {
                    self.anim_tileset = ts;
                }
                if let Some(hook) = hook {
                    self.apply_anim_effect(hook);
                }
            }
            AnimTickResult::WaitDelay {
                frames,
                sound,
                hook,
            } => {
                if let Some(sound_move) = sound {
                    self.emit_move_sfx(sound_move);
                }
                // AnimationPlayer already decrements subanimation delays internally.
                // Do not add extra renderer delay, otherwise pacing is effectively doubled.
                let _ = frames;
                self.anim_wait = 0;
                if let Some(hook) = hook {
                    self.apply_anim_effect(hook);
                }
            }
            AnimTickResult::Effect { sound, effect } => {
                if let Some(sound_move) = sound {
                    self.emit_move_sfx(sound_move);
                }
                self.apply_anim_effect(AnimationPlayer::apply_effect(effect));
            }
            AnimTickResult::Done => {
                self.anim_layer.clear();
                if !self.suppress_hit_flash {
                    if let Some(pending) = self.pending_applying.take() {
                        self.run_applying_attack_feedback(
                            pending.anim_type,
                            pending.attacker_is_player,
                        );
                    }
                } else {
                    self.pending_applying = None;
                }
            }
        }
    }

    /// Advance the ball-throw choreography (capture / ghost dodge / old man)
    /// by one frame: start each step's animation in turn, hold shake steps
    /// on their first frame block for 40 frames
    /// (`DoBallShakeSpecialEffects`: SFX_TINK + DelayFrames 40), and keep
    /// each step up for at least its `min_frames`.
    fn advance_ball_choreo(&mut self) {
        let Some(choreo) = self.ball_choreo.as_mut() else {
            return;
        };
        if !choreo.started {
            let step = choreo.steps[choreo.step];
            self.anim_player.start(step.anim, true);
            self.anim_wait = 0;
            self.anim_layer.clear();
            self.current_move = MoveId::None;
            if let Some(sfx) = step.sfx {
                self.pending_ball_sfx.push_back(sfx);
            }
            choreo.started = true;
            return;
        }
        choreo.frames = choreo.frames.saturating_add(1);
        let step = choreo.steps[choreo.step];
        // The first frame block is on screen now (ticked by
        // advance_move_animation above): hold it for 40 frames before the
        // wobble plays out.
        if step.shake && choreo.frames == 1 {
            self.anim_wait = self.anim_wait.max(40);
        }
        if self.anim_player.is_finished() && choreo.frames >= step.min_frames {
            choreo.step += 1;
            choreo.frames = 0;
            choreo.started = false;
            if choreo.step >= choreo.steps.len() {
                self.ball_choreo = None;
            }
        }
    }

    /// Duration of a slide animation in frames, per the original routines
    /// (entry/exit Legacy slides are battle-flow specific).
    fn slide_frames(kind: SlideKind, entry: bool) -> u8 {
        match kind {
            SlideKind::Legacy => {
                if entry {
                    12
                } else {
                    10
                }
            }
            // SlideDownFaintedMonPic: 7 rows × DelayFrames 2 ≈ 14 frames.
            SlideKind::Faint => 14,
            // _AnimationSlideMonOff: 8 tiles × wSlideMonDelay(3).
            SlideKind::SeOff => 24,
            // AnimationSlideMonHalfOff: 4 tiles × delay 4.
            SlideKind::SeHalfOff => 16,
            // AnimationSlideMonDown: 7 rows × Delay3.
            SlideKind::SeDown => 21,
            // _AnimationSlideMonUp: 7 rows × Delay3.
            SlideKind::SeUp => 21,
        }
    }

    /// Pixel offset contributed by an exit slide. Player slides left, enemy
    /// slides right (`_AnimationSlideMonOff` shifts player tile ids +7 / enemy
    /// −7, i.e. the pics move off their respective screen edges).
    fn exit_slide_offset(kind: SlideKind, frame: u8, is_player: bool) -> (i32, i32) {
        let f = frame as i32;
        match kind {
            SlideKind::Legacy => {
                if is_player {
                    (0, f * 2)
                } else {
                    // Trainer pic sliding off during TrainerSendOut
                    // (SlideTrainerPicOffScreen slides the enemy trainer right).
                    (f * 2, 0)
                }
            }
            // SlideDownFaintedMonPic: one 8px row every 2 frames, straight down.
            SlideKind::Faint => (0, (f / 2 + 1) * 8),
            SlideKind::SeOff => {
                let d = (f / 3 + 1) * 8;
                (if is_player { -d } else { d }, 0)
            }
            SlideKind::SeHalfOff => {
                let d = (f / 4 + 1) * 8;
                (if is_player { -d } else { d }, 0)
            }
            SlideKind::SeDown => (0, (f / 3 + 1) * 8),
            SlideKind::SeUp => (0, 0),
        }
    }

    pub fn update(&mut self, screen: &BattleScreen) {
        let kind = Self::phase_kind(&screen.phase);
        if self.last_phase_kind != Some(kind) {
            self.on_phase_change(&screen.phase);
            self.last_phase_kind = Some(kind);
        }

        if let BattlePhase::Intro {
            phase: intro_phase, ..
        } = &screen.phase
        {
            if self.last_intro_phase != Some(*intro_phase) {
                self.on_intro_phase_change(
                    intro_phase,
                    screen.player_species,
                    screen.enemy_species,
                );
                self.last_intro_phase = Some(*intro_phase);
            }
        }

        if self.last_message.as_ref() != screen.current_message.as_ref() {
            if let Some(ref msg) = screen.current_message {
                self.trigger_from_message(screen, msg);
            }
            self.last_message = screen.current_message.clone();
        }

        // WaitForSoundToFinish: start the deferred animation once the
        // previous SFX has finished playing.
        if !self.sfx_playing && self.ball_choreo.is_none() {
            if let Some((anim_id, player_is_attacker)) = self.pending_anim_start.take() {
                self.anim_player.start(anim_id, player_is_attacker);
                self.anim_wait = 0;
                self.anim_layer.clear();
            }
        }

        self.advance_move_animation();
        self.advance_ball_choreo();

        // Substitute doll lifecycle: SE_SUBSTITUTE_MON latches the doll on
        // (inside `fx`); here we only clear the latch when the core
        // HAS_SUBSTITUTE_UP flag drops (substitute broke).
        if let Some(bs) = screen.battle_state.as_ref() {
            let p = bs.player.battle_status2 & status2::HAS_SUBSTITUTE_UP != 0;
            let e = bs.enemy.battle_status2 & status2::HAS_SUBSTITUTE_UP != 0;
            if self.player_sub_flag && !p {
                self.fx.set_substitute(MonSide::Player, false);
            }
            if self.enemy_sub_flag && !e {
                self.fx.set_substitute(MonSide::Enemy, false);
            }
            self.player_sub_flag = p;
            self.enemy_sub_flag = e;
        }
        self.fx.tick();

        if let Some(anim) = self.player_entry.as_mut() {
            anim.frame = anim.frame.saturating_add(1);
            if anim.frame >= Self::slide_frames(anim.kind, true) {
                self.player_entry = None;
            }
        }
        if let Some(anim) = self.enemy_entry.as_mut() {
            anim.frame = anim.frame.saturating_add(1);
            if anim.frame >= Self::slide_frames(anim.kind, true) {
                self.enemy_entry = None;
            }
        }
        if let Some(anim) = self.player_exit.as_mut() {
            anim.frame = anim.frame.saturating_add(1);
            if anim.frame >= Self::slide_frames(anim.kind, false) {
                let kind = anim.kind;
                self.player_exit = None;
                match kind {
                    // AnimationSlideMonHalfOff leaves the mon half-off screen.
                    SlideKind::SeHalfOff => self.player_half_off = true,
                    _ => self.player_visible = false,
                }
            }
        }
        if let Some(anim) = self.enemy_exit.as_mut() {
            anim.frame = anim.frame.saturating_add(1);
            if anim.frame >= Self::slide_frames(anim.kind, false) {
                let kind = anim.kind;
                self.enemy_exit = None;
                match kind {
                    SlideKind::SeHalfOff => self.enemy_half_off = true,
                    SlideKind::Legacy => {
                        let in_trainer_send_out = matches!(
                            &screen.phase,
                            BattlePhase::Intro {
                                phase: IntroPhase::TrainerSendOut,
                                ..
                            }
                        );
                        if in_trainer_send_out {
                            // The trainer pic has slid off: the enemy mon
                            // simply appears (the original has no enemy-mon
                            // entry animation).
                        } else {
                            self.enemy_visible = false;
                        }
                    }
                    _ => self.enemy_visible = false,
                }
            }
        }
        if let Some(anim) = self.attack_lunge.as_mut() {
            anim.frame = anim.frame.saturating_add(1);
            if anim.frame >= 8 {
                self.attack_lunge = None;
            }
        }
        if let Some(anim) = self.move_mon_h.as_mut() {
            anim.frame = anim.frame.saturating_add(1);
            if anim.frame >= 3 {
                self.move_mon_h = None;
            }
        }

        // Tick the screen-wipe transition (mirrors pokered-app: the state
        // machine advances once per frame until all tiles are black).
        if let Some(ref mut ts) = self.transition_state {
            ts.tick();
            if ts.is_done() {
                self.transition_state = None;
            }
        }

        match self.intro_anim {
            IntroAnimState::ScreenFlash { remaining } => {
                if remaining > 0 {
                    self.intro_anim = IntroAnimState::ScreenFlash {
                        remaining: remaining - 1,
                    };
                } else {
                    self.intro_anim = IntroAnimState::None;
                }
            }
            IntroAnimState::SilhouetteSlide { remaining, offset } => {
                if remaining > 0 {
                    self.intro_anim = IntroAnimState::SilhouetteSlide {
                        remaining: remaining - 1,
                        offset: (offset - 2).max(0),
                    };
                } else {
                    self.intro_anim = IntroAnimState::None;
                }
            }
            IntroAnimState::PlayerSendOut { stage, frames } => {
                // AnimateSendingOutMon: the POOF plays (stage 0), then the
                // ball tile (stage 1, Delay3 = 3), then 3×3 (stage 2,
                // DelayFrames 4), then 5×5 (stage 3, DelayFrames 5); the
                // full 7×7 pic + cry follow (intro_anim → None).
                const STAGE_FRAMES: [u8; 4] = [0, 3, 4, 5];
                let stage_done = if stage == 0 {
                    // Wait for the POOF_ANIM to finish.
                    self.anim_player.is_finished()
                } else {
                    frames + 1 >= STAGE_FRAMES[stage as usize]
                };
                if stage_done {
                    if stage >= 3 {
                        self.intro_anim = IntroAnimState::None;
                        // SendOutMon: PlayCry AFTER the growth.
                        self.cry_pending = Some(screen.player_species);
                    } else {
                        self.intro_anim = IntroAnimState::PlayerSendOut {
                            stage: stage + 1,
                            frames: 0,
                        };
                    }
                } else {
                    self.intro_anim = IntroAnimState::PlayerSendOut {
                        stage,
                        frames: frames + 1,
                    };
                }
            }
            IntroAnimState::None => {}
        }
    }

    fn player_offset(&self) -> (i32, i32) {
        let mut dx = 0;
        let mut dy = 0;

        if let Some(entry) = self.player_entry {
            if entry.kind == SlideKind::SeUp {
                // AnimationSlideMonUp (Dig): rise from one row below.
                dy += 56 - (entry.frame as i32 / 3 + 1) * 8;
            } else {
                dy += (12i32 - entry.frame as i32).max(0) * 4;
            }
        }
        if let Some(exit) = self.player_exit {
            if exit.kind == SlideKind::Legacy {
                dy += exit.frame as i32 * 5;
            } else {
                let (ox, oy) = Self::exit_slide_offset(exit.kind, exit.frame, true);
                dx += ox;
                dy += oy;
            }
        }
        if self.player_half_off {
            // SE_SLIDE_MON_HALF_OFF latch (Softboiled): stays 4 tiles off.
            dx -= 32;
        }
        if let Some(lunge) = self.attack_lunge {
            if lunge.attacker_is_player {
                let f = lunge.frame as i32;
                let peak = if f < 4 { f } else { 8 - f };
                dx += peak * 2;
                dy -= peak;
            }
        }
        if let Some(mh) = self.move_mon_h {
            if mh.side == MonSide::Player {
                // AnimationMoveMonHorizontally: hlcoord(2,5) vs (1,5).
                dx += 8;
            }
        }
        dx += self.fx.mon_dx(MonSide::Player);
        // AnimationBoundUpAndDown (Splash): vertical slide-down cycles.
        dy += self.fx.mon_dy(MonSide::Player);

        if let IntroAnimState::SilhouetteSlide { offset, .. } = self.intro_anim {
            dx -= offset;
        }

        (dx, dy)
    }

    fn enemy_offset(&self) -> (i32, i32) {
        let mut dx = 0;
        let mut dy = 0;

        if let Some(entry) = self.enemy_entry {
            if entry.kind == SlideKind::SeUp {
                dy += 56 - (entry.frame as i32 / 3 + 1) * 8;
            } else {
                dx += (12i32 - entry.frame as i32).max(0) * 6;
            }
        }
        if let Some(exit) = self.enemy_exit {
            if exit.kind == SlideKind::Legacy {
                dx += exit.frame as i32 * 6;
            } else {
                let (ox, oy) = Self::exit_slide_offset(exit.kind, exit.frame, false);
                dx += ox;
                dy += oy;
            }
        }
        if self.enemy_half_off {
            dx += 32;
        }
        if let Some(lunge) = self.attack_lunge {
            if !lunge.attacker_is_player {
                let f = lunge.frame as i32;
                let peak = if f < 4 { f } else { 8 - f };
                dx -= peak * 2;
                dy += peak / 2;
            }
        }
        if let Some(mh) = self.move_mon_h {
            if mh.side == MonSide::Enemy {
                // AnimationMoveMonHorizontally: hlcoord(11,0) vs (12,0).
                dx -= 8;
            }
        }
        dx += self.fx.mon_dx(MonSide::Enemy);
        dy += self.fx.mon_dy(MonSide::Enemy);
        // AnimationShakeEnemyHUD scrolls the BG (SCX); the enemy mon is part
        // of the BG in the original, so it shakes along with the HUD strip.
        dx += self.fx.enemy_hud_shake_offset();

        if let IntroAnimState::SilhouetteSlide { offset, .. } = self.intro_anim {
            dx += offset;
        }

        (dx, dy)
    }

    fn player_visible_now(&self) -> bool {
        self.player_visible && !self.fx.mon_hidden(MonSide::Player)
    }

    fn enemy_visible_now(&self) -> bool {
        self.enemy_visible && !self.fx.mon_hidden(MonSide::Enemy)
    }

    fn apply_post_effects(&self, fb: &mut FrameBuffer) {
        self.fx.apply_screen_effects(fb);
        self.apply_intro_effects(fb);
    }

    fn apply_intro_effects(&self, fb: &mut FrameBuffer) {
        match self.intro_anim {
            IntroAnimState::ScreenFlash { .. } => {
                // All black (matches the per-pixel black-out the RGBA loop
                // produced); indices untouched, palette remap instead.
                fb.remap_shades(&[3, 3, 3, 3]);
            }
            IntroAnimState::SilhouetteSlide { .. } => {
                // Luminance inversion of the 4 grayscale shades (white↔black,
                // 0xAA↔0x55), as a display-palette remap.
                fb.remap_shades(&[3, 2, 1, 0]);
            }
            _ => {}
        }
    }
}

fn apply_offset(base: u32, delta: i32) -> u32 {
    if delta >= 0 {
        base.saturating_add(delta as u32)
    } else {
        base.saturating_sub((-delta) as u32)
    }
}

// ---------------------------------------------------------------------------
// ScaleSpriteByTwo — faithful port of engine/battle/scale_sprites.asm
// ---------------------------------------------------------------------------

/// Scale a 4×4-tile (32×32 px) sprite to 7×7 tiles (56×56 px).
///
/// Matches the original `ScaleSpriteByTwo` algorithm:
///   1. Take only the top-left 28×28 pixels (ignore last 4 rows & cols).
///   2. Double every pixel in both X and Y → 56×56 pixels.
///   3. Pack the result into 7×7 = 49 tiles.
pub(crate) fn scale_sprite_by_two(src: &TileSet, src_tpr: usize) -> TileSet {
    const SRC_USED: usize = 28; // 32 - 4 = 28 pixels used per axis
    const DST_SIZE: usize = 56; // 28 * 2 = 56 pixels output per axis
    const DST_TILES: usize = 7; // 56 / 8 = 7 tiles per axis

    // 1. Extract 28×28 pixel grid from the source tileset
    let mut src_px = [[0u8; SRC_USED]; SRC_USED];
    for py in 0..SRC_USED {
        for px in 0..SRC_USED {
            let tile_col = px / TILE_PIXELS;
            let tile_row = py / TILE_PIXELS;
            let tile_idx = tile_row * src_tpr + tile_col;
            let local_col = px % TILE_PIXELS;
            let local_row = py % TILE_PIXELS;
            src_px[py][px] = src.get(tile_idx).pixels[local_row][local_col];
        }
    }

    // 2. Double each pixel in both X and Y → 56×56
    let mut dst_px = [[0u8; DST_SIZE]; DST_SIZE];
    for sy in 0..SRC_USED {
        for sx in 0..SRC_USED {
            let c = src_px[sy][sx];
            let dx = sx * 2;
            let dy = sy * 2;
            dst_px[dy][dx] = c;
            dst_px[dy][dx + 1] = c;
            dst_px[dy + 1][dx] = c;
            dst_px[dy + 1][dx + 1] = c;
        }
    }

    // 3. Pack 56×56 pixel grid into 7×7 tiles
    let mut out = TileSet::blank(DST_TILES * DST_TILES);
    for ty in 0..DST_TILES {
        for tx in 0..DST_TILES {
            let mut pixels = [[0u8; TILE_PIXELS]; TILE_PIXELS];
            for row in 0..TILE_PIXELS {
                for col in 0..TILE_PIXELS {
                    pixels[row][col] = dst_px[ty * TILE_PIXELS + row][tx * TILE_PIXELS + col];
                }
            }
            out.set(ty * DST_TILES + tx, Tile { pixels });
        }
    }

    out
}

/// Downscale a `src_tiles`×`src_tiles`-tile pic to `dst_tiles`×`dst_tiles`
/// tiles by nearest-neighbor decimation — the `CopyDownscaledMonTiles` port
/// (home/copy2.asm) used by `AnimateSendingOutMon` to grow the send-out pic
/// 3×3 → 5×5 → 7×7.
fn downscale_mon_tiles(src: &TileSet, src_tiles: usize, dst_tiles: usize) -> TileSet {
    let src_px_len = src_tiles * TILE_PIXELS;
    let dst_px_len = dst_tiles * TILE_PIXELS;
    let mut out = TileSet::blank(dst_tiles * dst_tiles);
    for ty in 0..dst_tiles {
        for tx in 0..dst_tiles {
            let mut pixels = [[0u8; TILE_PIXELS]; TILE_PIXELS];
            for row in 0..TILE_PIXELS {
                for col in 0..TILE_PIXELS {
                    let dy = ty * TILE_PIXELS + row;
                    let dx = tx * TILE_PIXELS + col;
                    let sy = dy * src_px_len / dst_px_len;
                    let sx = dx * src_px_len / dst_px_len;
                    let tile_idx = (sy / TILE_PIXELS) * src_tiles + (sx / TILE_PIXELS);
                    pixels[row][col] =
                        src.get(tile_idx).pixels[sy % TILE_PIXELS][sx % TILE_PIXELS];
                }
            }
            out.set(ty * dst_tiles + tx, Tile { pixels });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// ASCII → Pokémon charmap conversion
// ---------------------------------------------------------------------------

/// Convert an ASCII string to a vector of Pokémon Red tile IDs.
///
/// Matches the charmap in constants/charmap.asm:
///   'A'-'Z' → $80-$99, 'a'-'z' → $A0-$B9,
///   '0'-'9' → $F6-$FF, ' ' → $7F, ':' → $9C, '/' → $F3, etc.
fn ascii_to_tiles(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            'A'..='Z' => 0x80 + (c as u8 - b'A'),
            'a'..='z' => 0xA0 + (c as u8 - b'a'),
            '0'..='9' => 0xF6 + (c as u8 - b'0'),
            ' ' => 0x7F,
            ':' => 0x9C,
            '/' => 0xF3,
            '(' => 0x9A,
            ')' => 0x9B,
            '-' => 0xE3,
            '.' => 0xE8,
            '\'' => 0xE0,
            '!' => 0xE7,
            '?' => 0xE6,
            '>' => 0xED, // used as cursor arrow
            '×' => 0xF1, // multiplication sign (charmap.asm:181)
            _ => 0x7F,   // space for unknown
        })
        .collect()
}

fn core_status_to_tiles(status: &CoreStatus) -> Option<StatusCondition> {
    match status {
        CoreStatus::None => None,
        CoreStatus::Sleep(_) => Some(StatusCondition::Sleep),
        CoreStatus::Poison => Some(StatusCondition::Poison),
        CoreStatus::Burn => Some(StatusCondition::Burn),
        CoreStatus::Freeze => Some(StatusCondition::Freeze),
        CoreStatus::Paralysis => Some(StatusCondition::Paralysis),
    }
}

fn slot_status_to_pokeball(slot: pokered_core::battle::PokeballSlotStatus) -> BallStatus {
    use pokered_core::battle::PokeballSlotStatus as S;
    match slot {
        S::Normal => BallStatus::Normal,
        S::StatusAilment => BallStatus::StatusAilment,
        S::Fainted => BallStatus::Fainted,
        S::Empty => BallStatus::Empty,
    }
}

// ---------------------------------------------------------------------------
// Combined VRAM tileset construction
// ---------------------------------------------------------------------------

/// Build the combined 256-tile VRAM tileset that mirrors the Game Boy's
/// VRAM layout during battle.
///
/// Tile ID mapping (from home/load_font.asm):
///   $80-$FF: font.png (1bpp, 128 tiles) — A-Z, a-z, digits, punctuation
///   $60-$7F: font_extra.png (2bpp, 32 tiles) — textbox borders, then
///   $62-$7F: font_battle_extra.png (2bpp, 30 tiles) — HP bar tiles (OVERWRITES $62+)
///   $6D+: battle_hud_1.png (1bpp, 3 tiles) — end cap, Lv, triangle
///   $73+: battle_hud_2.png + battle_hud_3.png (1bpp, 3+3=6 tiles) — HUD borders
fn build_battle_tileset(rm: &mut ResourceManager) -> TileSet {
    let mut ts = TileSet::blank(256);

    // 1. Font tiles at $80-$FF (128 tiles from font.png, loaded as 1bpp)
    if let Ok(cached) = rm.load_font("font") {
        let font_ts = cached.tileset.clone();
        for i in 0..font_ts.len().min(128) {
            ts.set(0x80 + i, font_ts.get(i).clone());
        }
    }

    // 2. TextBox tiles at $60-$7F (from font_extra.png, 2bpp)
    //    Must load as 2bpp — can't use load_font() which forces 1bpp.
    if let Ok(cached) = rm.load_asset_2bpp(AssetCategory::Font, "font_extra.png") {
        let extra_ts = cached.tileset.clone();
        for i in 0..extra_ts.len().min(32) {
            ts.set(0x60 + i, extra_ts.get(i).clone());
        }
    }

    // 3. HP bar + status tiles at $62+ (from font_battle_extra.png, 2bpp)
    //    OVERWRITES $62+ from step 2.
    if let Ok(cached) = rm.load_asset_2bpp(AssetCategory::Font, "font_battle_extra.png") {
        let hp_ts = cached.tileset.clone();
        for i in 0..hp_ts.len() {
            ts.set(0x62 + i, hp_ts.get(i).clone());
        }
    }

    // 4. Battle HUD tiles — loaded as **1bpp** (matching ASM's FarCopyDataDouble)
    //    The PNGs are 2-bit grayscale but the original game INCBINs them as .1bpp
    //    and loads via CopyVideoDataDouble which doubles each byte (1bpp→2bpp).
    //    battle_hud_1.png (1bpp, 3 tiles) → $6D
    if let Ok(cached) = rm.load_asset_1bpp(AssetCategory::Battle, "battle_hud_1.png") {
        let hud1 = cached.tileset.clone();
        for i in 0..hud1.len() {
            ts.set(0x6D + i, hud1.get(i).clone());
        }
    }

    //    battle_hud_2.png (1bpp, 3 tiles) → $73
    //    battle_hud_3.png (1bpp, 3 tiles) → concatenated after hud_2 at $73+3
    if let Ok(cached) = rm.load_asset_1bpp(AssetCategory::Battle, "battle_hud_2.png") {
        let hud2 = cached.tileset.clone();
        let hud2_len = hud2.len();
        for i in 0..hud2_len {
            ts.set(0x73 + i, hud2.get(i).clone());
        }
        if let Ok(cached3) = rm.load_asset_1bpp(AssetCategory::Battle, "battle_hud_3.png") {
            let hud3 = cached3.tileset.clone();
            for i in 0..hud3.len() {
                ts.set(0x73 + hud2_len + i, hud3.get(i).clone());
            }
        }
    }

    // 5. Pokeball indicator tiles at $31 (from balls.png, 2bpp)
    //    Original loads via CopyVideoData into vSprites tile $31 (OAM).
    //    We render them in the background tilemap instead.
    if let Ok(cached) = rm.load_asset_2bpp(AssetCategory::Battle, "balls.png") {
        let balls_ts = cached.tileset.clone();
        for i in 0..balls_ts.len().min(5) {
            ts.set(0x31 + i, balls_ts.get(i).clone());
        }
    }

    ts
}

// ---------------------------------------------------------------------------
// Battle menu text (tile-encoded)
// ---------------------------------------------------------------------------

/// Draw the 2×2 battle menu items into the tile buffer.
///
/// Original layout (from DisplayBattleMenu in engine/battle/core.asm):
///   The battle menu is in the right half of the bottom text box.
///   In this Rust port, action mapping is:
///   Row 14: "FIGHT" at left, "PKMN" at right
///   Row 16: "ITEM" at left, "RUN" at right
fn draw_battle_menu(buf: &mut ScreenTileBuffer, selected_row: usize, selected_col: usize) {
    // Battle menu inner box border (right half of dialog area)
    // From DrawPlayerBattleMenu: a 2-column wide inner box at (8,12) 12×6
    // We draw a sub-box on the right side
    let menu_box = TextBoxFrame::new(8, 12, 12, 6);
    menu_box.draw_frame(buf);

    let fight_tiles = ascii_to_tiles("FIGHT");
    let pkmn_tiles: Vec<u8> = vec![0xE1, 0xE2]; // <PK><MN> charmap tiles
    let item_tiles = ascii_to_tiles("ITEM");
    let run_tiles = ascii_to_tiles("RUN");

    write_tiles_at(buf, 10, 14, &fight_tiles);
    write_tiles_at(buf, 16, 14, &pkmn_tiles);
    write_tiles_at(buf, 10, 16, &item_tiles);
    write_tiles_at(buf, 16, 16, &run_tiles);

    // Selection cursor (▶ = $ED in charmap)
    // row=0/1 (top/bottom), col=0/1 (left/right)
    let cursor_x = if selected_col == 0 { 9 } else { 15 };
    let cursor_y = if selected_row == 0 { 14 } else { 16 };
    buf.set(cursor_x, cursor_y, 0xED);
}

/// Draw the 2×2 Safari battle action menu (BALL / BAIT / ROCK / RUN) into
/// the tile buffer — the Safari battle replaces the FIGHT menu with this
/// grid (app: `battle_safari.gui` overlay; original: DisplayBattleMenu's
/// Safari branch). Same inner-box geometry and cursor grid as
/// [`draw_battle_menu`]; the 2×2 mapping is Ball/Bait (top) Rock/Run
/// (bottom), driven by `SafariBattleMenuState`.
fn draw_safari_menu(buf: &mut ScreenTileBuffer, selected_row: usize, selected_col: usize, balls: u8) {
    let menu_box = TextBoxFrame::new(8, 12, 12, 6);
    menu_box.draw_frame(buf);

    write_tiles_at(buf, 10, 14, &ascii_to_tiles("BALL"));
    // SAFARI BALL count after "BALL×" (core.asm:2077-2081; BAIT shifted one
    // tile right to fit the 2-digit number at columns 15-16).
    write_tiles_at(buf, 14, 14, &ascii_to_tiles(&format!("×{:02}", balls)));
    write_tiles_at(buf, 17, 14, &ascii_to_tiles("BAIT"));
    write_tiles_at(buf, 10, 16, &ascii_to_tiles("ROCK"));
    write_tiles_at(buf, 16, 16, &ascii_to_tiles("RUN"));

    // Selection cursor (▶ = $ED in charmap), same grid as battle_main.
    let cursor_x = if selected_col == 0 { 9 } else { 15 };
    let cursor_y = if selected_row == 0 { 14 } else { 16 };
    buf.set(cursor_x, cursor_y, 0xED);
}

/// Draw battle dialog text into the text box area.
fn draw_battle_text(buf: &mut ScreenTileBuffer, text: &str) {
    const LINE_WIDTH: usize = 18;

    let mut wrapped: Vec<String> = Vec::new();

    for raw_line in text.split('\n') {
        let words: Vec<&str> = raw_line.split_whitespace().collect();
        if words.is_empty() {
            wrapped.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in words {
            let word_chars: Vec<char> = word.chars().collect();
            let mut start = 0;
            while start < word_chars.len() {
                let end = (start + LINE_WIDTH).min(word_chars.len());
                let part: String = word_chars[start..end].iter().collect();

                if current.is_empty() {
                    current.push_str(&part);
                } else if current.chars().count() + 1 + part.chars().count() <= LINE_WIDTH {
                    current.push(' ');
                    current.push_str(&part);
                } else {
                    wrapped.push(current);
                    current = part;
                }

                start = end;
            }
        }

        if !current.is_empty() {
            wrapped.push(current);
        }
    }

    if let Some(line1) = wrapped.first() {
        write_tiles_at(buf, 1, 14, &ascii_to_tiles(line1));
    }
    if let Some(line2) = wrapped.get(1) {
        write_tiles_at(buf, 1, 16, &ascii_to_tiles(line2));
    }
}

fn move_display_name(move_id: pokered_data::moves::MoveId) -> String {
    let raw = format!("{:?}", move_id);
    let mut result = String::with_capacity(raw.len() + 4);
    for (i, c) in raw.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            let prev = raw.as_bytes()[i - 1] as char;
            if prev.is_lowercase() {
                result.push(' ');
            }
        }
        result.push(c);
    }
    result.to_uppercase()
}

fn draw_move_menu(buf: &mut ScreenTileBuffer, screen: &BattleScreen) {
    if let Some(ref mm) = screen.move_menu {
        // Original: TextBoxBorder(4, 12, 14, 4), moves at hlcoord(6, 13), cursor at col 5
        let move_box = TextBoxFrame::new(4, 12, 16, 6);
        move_box.draw_frame(buf);

        // Match engine/battle/core.asm MoveSelectionMenu:
        // after drawing the move box, top border is patched at (4,12)='─' and (10,12)='┘'
        // to join the left TYPE/PP panel cleanly.
        buf.set(4, 12, 0x7A);
        buf.set(10, 12, 0x7E);

        let moves = mm.moves();
        for (i, slot) in moves.iter().enumerate() {
            let name = move_display_name(slot.move_id);
            let truncated: String = name.chars().take(12).collect();
            let name_tiles = ascii_to_tiles(&truncated);
            let y = 13 + i as u32;
            write_tiles_at(buf, 6, y, &name_tiles);
        }

        let cursor_y = 13 + mm.cursor() as u32;
        buf.set(5, cursor_y, 0xED);

        // Original: TextBoxBorder(0, 8, 3, 9) — TYPE/PP info for highlighted move
        let pp_box = TextBoxFrame::new(0, 8, 11, 5);
        pp_box.draw_frame(buf);

        let cursor_idx = mm.cursor();
        if cursor_idx < moves.len() {
            let slot = &moves[cursor_idx];
            let type_label = ascii_to_tiles("TYPE/");
            write_tiles_at(buf, 1, 9, &type_label);

            if let Some(move_data) = pokered_data::move_data::MoveData::get(slot.move_id) {
                let type_name = format!("{:?}", move_data.move_type).to_uppercase();
                let type_tiles = ascii_to_tiles(&type_name);
                write_tiles_at(buf, 1, 10, &type_tiles);
            }

            // Match PrintMenuItem in engine/battle/core.asm:
            // (5,9)='/', (7,11)='/', current PP at (5,11), max PP at (8,11), plus "PP" label.
            let pp_label = ascii_to_tiles("PP");
            write_tiles_at(buf, 2, 11, &pp_label);

            let pp_text = format!("{:>2}/{:>2}", slot.current_pp.min(99), slot.max_pp.min(99));
            let pp_tiles = ascii_to_tiles(&pp_text);
            write_tiles_at(buf, 5, 11, &pp_tiles);
        }
    }

    if let Some(ref msg) = screen.current_message {
        let tiles = ascii_to_tiles(msg);
        write_tiles_at(buf, 1, 14, &tiles);
    }
}

fn draw_bag_menu(buf: &mut ScreenTileBuffer, screen: &BattleScreen) {
    use pokered_data::item_data::get_item_data;

    if let Some(ref bm) = screen.bag_menu {
        let bag_box = TextBoxFrame::new(4, 12, 16, 6);
        bag_box.draw_frame(buf);

        buf.set(4, 12, 0x7A);

        let items = bm.items();
        for (i, (item_id, qty)) in items.iter().enumerate() {
            let item_name = get_item_data(*item_id).map(|d| d.name).unwrap_or("???");
            let truncated: String = item_name.chars().take(12).collect();
            let line = format!("{} x{}", truncated, qty);
            let name_tiles = ascii_to_tiles(&line);
            let y = 13 + i as u32;
            write_tiles_at(buf, 6, y, &name_tiles);
        }

        let cancel_y = 13 + items.len() as u32;
        let cancel_tiles = ascii_to_tiles("CANCEL");
        write_tiles_at(buf, 6, cancel_y, &cancel_tiles);

        let cursor_y = 13 + bm.cursor() as u32;
        buf.set(5, cursor_y, 0xED);
    }

    if let Some(ref msg) = screen.current_message {
        let tiles = ascii_to_tiles(msg);
        write_tiles_at(buf, 1, 14, &tiles);
    }
}

fn draw_party_menu(buf: &mut ScreenTileBuffer, screen: &BattleScreen) {
    if let Some(ref bs) = screen.battle_state {
        for (i, mon) in bs.player.party.iter().enumerate() {
            let name = format!("{}", mon.species).to_uppercase();
            let line = if mon.hp == 0 {
                format!("{} FNT", name)
            } else {
                format!("{} {}/{}", name, mon.hp, mon.max_hp)
            };
            let tiles = ascii_to_tiles(&line);
            let y = 14 + (i.min(3)) as u32;
            write_tiles_at(buf, 2, y, &tiles);
        }
        let cursor_y = 14 + (screen.party_cursor.min(3)) as u32;
        buf.set(1, cursor_y, 0xED);
    }

    if let Some(ref msg) = screen.current_message {
        let msg_tiles = ascii_to_tiles(msg);
        write_tiles_at(buf, 1, 16, &msg_tiles);
    }
}

fn draw_pokeball_tile(
    fb: &mut FrameBuffer,
    x: u32,
    y: u32,
    pal: &pokered_renderer::palette::Palette,
) {
    const SIZE: u32 = TILE_SIZE;
    for row in 0..SIZE {
        for col in 0..SIZE {
            let px = x + col;
            let py = y + row;
            if px >= fb.width() as u32 || py >= fb.height() as u32 {
                continue;
            }
            let center_y = SIZE / 2;
            let idx = if row == center_y || row == center_y + 1 {
                0
            } else if col >= SIZE / 2 - 1
                && col <= SIZE / 2
                && row >= center_y - 1
                && row <= center_y + 2
            {
                0
            } else if (col as i32 - SIZE as i32 / 2).pow(2) + (row as i32 - SIZE as i32 / 2).pow(2)
                <= (SIZE as i32 / 2 - 1).pow(2)
            {
                if row < center_y {
                    3
                } else {
                    1
                }
            } else {
                2
            };
            let c = pal.colors[idx];
            fb.set_pixel(px, py, c);
        }
    }
}

// ---------------------------------------------------------------------------
// Main battle rendering
// ---------------------------------------------------------------------------

pub fn draw_battle(
    screen: &BattleScreen,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    effects: &mut BattleVisualEffects,
) {
    fb.clear(Rgba::WHITE);

    // During BattleTransitionWipe and TransitionFlash, skip all battle rendering
    let skip_battle_render = matches!(
        &screen.phase,
        BattlePhase::Intro { phase, .. }
        if matches!(
            phase,
            IntroPhase::BattleTransitionWipe(_) | IntroPhase::TransitionFlash
        )
    );

    if skip_battle_render {
        // During BattleTransitionWipe the wipe eats the overworld snapshot
        // tile by tile (engine/battle/battle_transitions.asm); once the wipe
        // finished but the core is still holding the black screen, keep it
        // black — matches the ASM DelayFrames hold and avoids a single-frame
        // flash back to white before the SilhouetteSlide phase.
        let snapshot = effects.overworld_snapshot.as_ref();
        if let Some(snap) = snapshot {
            if effects.has_transition() {
                effects.render_transition(snap, fb);
                return;
            }
        }
        fb.clear(Rgba::BLACK);
        effects.apply_post_effects(fb);
        return;
    }

    let pal = &GRAYSCALE_PALETTE;

    // A Pokémon-Tower GHOST (no Silph Scope) shows as "GHOST", not the real species.
    // The ghost-Marowak battle (with scope) is "GHOST" until the unveil completes.
    let enemy_name = if screen.is_ghost || (screen.ghost_marowak_reveal && !screen.ghost_marowak_unveiled) {
        "GHOST".to_string()
    } else {
        format!("{}", screen.enemy_species).to_uppercase()
    };
    // The catch tutorial shows the player as "OLD MAN" (Gen-1
    // BATTLE_TYPE_OLD_MAN), mirroring the native renderer.
    let player_name = if screen.is_old_man {
        "OLD MAN".to_string()
    } else {
        format!("{}", screen.player_species).to_uppercase()
    };
    let enemy_sprite = species_to_sprite_name(&format!("{}", screen.enemy_species));
    let player_sprite = species_to_sprite_name(&format!("{}", screen.player_species));

    // AnimationTransformMon (ChangeMonPic): a transformed mon is drawn
    // with the opposing mon's sprite — the front sprite on the enemy
    // side, the back sprite on the player side.
    let (enemy_transformed, player_transformed) = match screen.battle_state.as_ref() {
        Some(bs) => (
            bs.enemy.battle_status3 & status3::TRANSFORMED != 0,
            bs.player.battle_status3 & status3::TRANSFORMED != 0,
        ),
        None => (false, false),
    };
    let enemy_sprite = if enemy_transformed {
        player_sprite.clone()
    } else {
        enemy_sprite
    };
    let player_sprite = if player_transformed {
        species_to_sprite_name(&format!("{}", screen.enemy_species))
    } else {
        player_sprite
    };

    // Build combined VRAM tileset and tile buffer
    let mut tile_buf = ScreenTileBuffer::new(fb.width() / TILE_SIZE, fb.height() / TILE_SIZE); // filled with $7F (space)

    if let Some(ref mut rm) = res {
        // ── Build combined 256-tile VRAM tileset ─────────────────────
        let battle_ts = build_battle_tileset(rm);

        let hide_enemy_hud = matches!(
            &screen.phase,
            BattlePhase::Intro { phase, .. }
            if matches!(
                phase,
                IntroPhase::SilhouetteSlide
                    | IntroPhase::TrainerReveal
            )
        );
        let hide_player_hud = matches!(
            &screen.phase,
            BattlePhase::Intro { phase, .. }
            if matches!(
                phase,
                IntroPhase::SilhouetteSlide
                    | IntroPhase::WildReveal
                    | IntroPhase::GhostCantID
                    | IntroPhase::GhostUnveil
                    | IntroPhase::TrainerReveal
                    | IntroPhase::TrainerSendOut
            )
        );

        if !hide_enemy_hud {
            let enemy_name_tiles = ascii_to_tiles(&enemy_name);
            let enemy_status_tiles = core_status_to_tiles(&screen.enemy_status).map(|s| s.tiles());
            let _enemy_hp_color = EnemyHud::draw(
                &mut tile_buf,
                &enemy_name_tiles,
                screen.enemy_level,
                enemy_status_tiles.as_ref().map(|t| t.as_slice()),
                screen.enemy_hp,
                screen.enemy_max_hp,
            );
        }

        if !hide_player_hud {
            let player_name_tiles = ascii_to_tiles(&player_name);
            let player_status_tiles =
                core_status_to_tiles(&screen.player_status).map(|s| s.tiles());
            let _player_hp_color = PlayerHud::draw(
                &mut tile_buf,
                &player_name_tiles,
                screen.player_level,
                player_status_tiles.as_ref().map(|t| t.as_slice()),
                screen.player_hp,
                screen.player_max_hp,
            );
        }

        if screen.show_player_pokeballs {
            let player_balls: [BallStatus; 6] =
                screen.player_pokeball_status.map(slot_status_to_pokeball);
            BallIndicators::draw_player(&mut tile_buf, &player_balls);
        }

        if screen.show_enemy_pokeballs {
            let enemy_balls: [BallStatus; 6] =
                screen.enemy_pokeball_status.map(slot_status_to_pokeball);
            BallIndicators::draw_enemy(&mut tile_buf, &enemy_balls);
        }

        // ── Bottom area (text box + menu or message) ─────────────────
        // Standard dialog box: full width, bottom 6 rows
        let dialog_box = TextBoxFrame::standard_dialog();
        dialog_box.draw_frame(&mut tile_buf);

        if matches!(screen.phase, BattlePhase::PlayerMenu) {
            if screen.is_safari {
                // Safari battle: the FIGHT menu is replaced by the
                // BALL/BAIT/ROCK/RUN grid (battle_safari.gui in the app;
                // DisplayBattleMenu's Safari branch in the original).
                draw_safari_menu(
                    &mut tile_buf,
                    screen.safari_menu.row(),
                    screen.safari_menu.col(),
                    screen.safari_menu.safari_balls_remaining,
                );
            } else {
                draw_battle_menu(
                    &mut tile_buf,
                    screen.battle_menu.row(),
                    screen.battle_menu.col(),
                );
            }
        } else if matches!(screen.phase, BattlePhase::MoveSelect) {
            draw_move_menu(&mut tile_buf, screen);
        } else if matches!(screen.phase, BattlePhase::BagSelect) {
            draw_bag_menu(&mut tile_buf, screen);
        } else if matches!(screen.phase, BattlePhase::ItemTargetSelect { .. }) {
            draw_party_menu(&mut tile_buf, screen);
        } else if matches!(
            screen.phase,
            BattlePhase::PartySelect | BattlePhase::ShiftSwitchSelect | BattlePhase::PlayerFaintSwitch
        ) {
            draw_party_menu(&mut tile_buf, screen);
        } else if matches!(screen.phase, BattlePhase::ShiftPrompt) {
            // "Will you change #MON?" — prompt text + YES/NO box (original
            // TWO_OPTION_MENU at hlcoord(0,7), cursor default NO).
            if let Some(ref text) = screen.current_message {
                draw_battle_text(&mut tile_buf, text);
            }
            let yn_box = TextBoxFrame::new(0, 7, 7, 5);
            yn_box.draw_frame(&mut tile_buf);
            write_tiles_at(&mut tile_buf, 2, 9, &ascii_to_tiles("YES"));
            write_tiles_at(&mut tile_buf, 2, 11, &ascii_to_tiles("NO"));
            let cursor_y = if screen.shift_prompt_yes { 9 } else { 11 };
            tile_buf.set(1, cursor_y, 0xED);
        } else {
            let trainer_name = screen
                .trainer_name
                .clone()
                .or_else(|| screen.trainer_class.map(|tc| tc.display_name().to_string()))
                .unwrap_or_else(|| enemy_name.clone());
            let phase_text = match &screen.phase {
                BattlePhase::Intro { phase, .. } => match phase {
                    IntroPhase::BattleTransitionWipe(_)
                    | IntroPhase::TransitionFlash
                    | IntroPhase::SilhouetteSlide => None,
                    IntroPhase::WildReveal => {
                        if screen.is_ghost
                            || (screen.ghost_marowak_reveal && !screen.ghost_marowak_unveiled)
                        {
                            Some(format!("Enemy {} appeared!", enemy_name))
                        } else if screen.hooked {
                            // HookedMonAttackedText (data/text/text_2.asm:1243-1249),
                            // chosen over WildMonAppearedText when wMoveMissed != 0
                            // (engine/battle/common_text.asm:13-18).
                            Some(format!("The hooked {}\nattacked!", enemy_name))
                        } else {
                            Some(format!("Wild {} appeared!", enemy_name))
                        }
                    }
                    IntroPhase::GhostCantID => {
                        Some("Darn! The GHOST\ncan't be ID'd!".to_string())
                    }
                    IntroPhase::GhostUnveil => {
                        Some("SILPH SCOPE unveiled the\nGHOST's identity!".to_string())
                    }
                    IntroPhase::TrainerReveal => Some(format!("{} wants to fight!", trainer_name)),
                    IntroPhase::TrainerSendOut => {
                        Some(format!("{} sent out {}!", trainer_name, enemy_name))
                    }
                    IntroPhase::PlayerSendOut => {
                        let pname = format!("{}", screen.player_species).to_uppercase();
                        Some(format!("Go! {}!", pname))
                    }
                },
                BattlePhase::BattleOver { won, .. } => {
                    if *won {
                        Some("You won!".to_string())
                    } else {
                        Some("You lost...".to_string())
                    }
                }
                _ => screen.current_message.clone(),
            };
            if let Some(ref text) = phase_text {
                draw_battle_text(&mut tile_buf, text);
            }

            // Show down-arrow when waiting for user input
            if let BattlePhase::Intro {
                phase: ref intro_p,
                wait_frames,
            } = &screen.phase
            {
                let needs_input = matches!(
                    intro_p,
                    IntroPhase::WildReveal
                        | IntroPhase::GhostCantID
                        | IntroPhase::GhostUnveil
                        | IntroPhase::TrainerReveal
                        | IntroPhase::TrainerSendOut
                        | IntroPhase::PlayerSendOut
                );
                if needs_input && *wait_frames == 0 {
                    dialog_box.show_down_arrow(&mut tile_buf);
                }
            }

            if let BattlePhase::ShowingText {
                messages,
                current,
                wait_frames,
                ..
            } = &screen.phase
            {
                let has_next_page = *current + 1 < messages.len();
                if has_next_page && *wait_frames == 0 {
                    dialog_box.show_down_arrow(&mut tile_buf);
                }
            }
        }

        // ── Render tile buffer to framebuffer ────────────────────────
        tile_buf.render(fb, &battle_ts, pal);

        // AnimationShakeEnemyHUD: SCX-shake the enemy HUD strip. Applied
        // before the mon sprites are drawn — the original protects the
        // player back pic by copying it to OAM first.
        effects.fx.apply_enemy_hud_shake(fb);

        // DMG-original look: keep battle HUD (including HP bars) in grayscale.
        // Do not apply SGB-style green/yellow/red recolor overlays here.

        // ── Overlay Pokémon / trainer sprites on top ────────────────
        let show_trainer_sprite = match &screen.phase {
            BattlePhase::Intro { phase, .. } => {
                !screen.is_wild
                    && match phase {
                        IntroPhase::SilhouetteSlide => true,
                        IntroPhase::TrainerReveal => true,
                        IntroPhase::TrainerSendOut => effects.enemy_exit.is_some(),
                        _ => false,
                    }
            }
            _ => false,
        };

        let (enemy_dx, enemy_dy) = effects.enemy_offset();
        if effects.enemy_visible_now() {
            if show_trainer_sprite {
                if let Some(tc) = screen.trainer_class {
                    if let Ok(cached) = rm.load_trainer(tc.sprite_name()) {
                        let ts = cached.tileset.clone();
                        let w_tiles = cached.source_size.0 / TILE_SIZE;
                        let h_tiles = cached.source_size.1 / TILE_SIZE;
                        let x_off = ((8 - w_tiles) / 2) * TILE_SIZE;
                        let y_off = (7 - h_tiles) * TILE_SIZE;
                        let ex = apply_offset(12 * TILE_SIZE + x_off, enemy_dx);
                        let ey = apply_offset(y_off, enemy_dy);
                        blit_tileset(fb, &ts, ex, ey, w_tiles, pal);
                    }
                }
            } else if effects.fx.is_substitute(MonSide::Enemy) {
                // AnimationSubstitute: the mon pic is replaced by the
                // MonsterSprite mini doll (facing down on the enemy side).
                if let Ok(cached) = rm.load_sprite("monster") {
                    let doll = cached.tileset.clone();
                    let rect = MonRect {
                        x: 12 * TILE_SIZE as i32 + enemy_dx,
                        y: enemy_dy,
                    };
                    BattleEffects::draw_substitute(fb, rect, &doll, pal, MonSide::Enemy);
                }
            } else if effects.fx.is_minimized(MonSide::Enemy) {
                // AnimationMinimizeMon: the mon pic is replaced by the blob.
                let rect = MonRect {
                    x: 12 * TILE_SIZE as i32 + enemy_dx,
                    y: enemy_dy,
                };
                BattleEffects::draw_minimized(fb, rect, pal);
            } else if let Ok(cached) = rm.load_pokemon_front(&enemy_sprite) {
                let ts = cached.tileset.clone();
                let w_tiles = cached.source_size.0 / TILE_SIZE;
                let h_tiles = cached.source_size.1 / TILE_SIZE;
                let x_off = ((8 - w_tiles) / 2) * TILE_SIZE;
                let y_off = (7 - h_tiles) * TILE_SIZE;
                let ex = apply_offset(12 * TILE_SIZE + x_off, enemy_dx);
                let ey = apply_offset(y_off, enemy_dy);
                if let Some((rows, yoff)) = effects.fx.slide_down_hide_params(MonSide::Enemy) {
                    // AnimationSlideMonDownAndHide (Acid Armor): crop to the
                    // top rows (7×5 then 7×3 tile-id lists), drawn lower.
                    BattleEffects::draw_mon_rows(
                        fb,
                        &ts,
                        ex as i32,
                        ey as i32 + yoff,
                        w_tiles,
                        pal,
                        rows,
                    );
                } else if let Some((width, anchor_right)) = effects.fx.squish_params(MonSide::Enemy) {
                    // AnimationSquishMonPic: narrow the pic one tile per pass.
                    BattleEffects::draw_squished(
                        fb,
                        &ts,
                        ex as i32,
                        ey as i32,
                        w_tiles,
                        pal,
                        width,
                        anchor_right,
                    );
                } else {
                    blit_tileset(fb, &ts, ex, ey, w_tiles, pal);
                }
            }
        }

        let show_player_trainer_back = match &screen.phase {
            BattlePhase::Intro { phase, .. } => {
                !screen.is_wild
                    && matches!(
                        phase,
                        IntroPhase::SilhouetteSlide
                            | IntroPhase::TrainerReveal
                            | IntroPhase::TrainerSendOut
                    )
            }
            _ => false,
        };

        // Player back sprite: loaded as 4×4 tiles (32×32), scaled to 7×7 (56×56)
        // via ScaleSpriteByTwo, then blitted at tile (1, 5) = pixel (8, 40)
        let (player_dx, player_dy) = effects.player_offset();
        if effects.player_visible_now() {
            if let IntroAnimState::PlayerSendOut { stage, .. } = effects.intro_anim {
                // AnimateSendingOutMon stages: stage 0 = POOF only, stage 1 =
                // the ball tile at hlcoord(4,11), then the pic grows 3×3 at
                // (3,9) → 5×5 at (2,7); the full 7×7 pic draws once the
                // state clears.
                match stage {
                    0 => {}
                    1 => draw_pokeball_tile(fb, 4 * TILE_SIZE, 11 * TILE_SIZE, pal),
                    _ => {
                        let tiles: usize = if stage == 2 { 3 } else { 5 };
                        let (tx, ty) = if stage == 2 { (3, 9) } else { (2, 7) };
                        let back_sprite_name = format!("{}b", player_sprite);
                        if let Ok(cached) = rm.load_pokemon_back(&back_sprite_name) {
                            let ts = cached.tileset.clone();
                            let src_tpr = (cached.source_size.0 / TILE_SIZE) as usize;
                            let scaled = scale_sprite_by_two(&ts, src_tpr);
                            let small = downscale_mon_tiles(&scaled, 7, tiles);
                            blit_tileset(
                                fb,
                                &small,
                                tx * TILE_SIZE,
                                ty * TILE_SIZE,
                                tiles as u32,
                                pal,
                            );
                        }
                    }
                }
            } else if show_player_trainer_back {
                // LoadPlayerBackPic (engine/battle/core.asm:6202-6211): the
                // player's back silhouette in the intro is RED — except in
                // the Old-Man tutorial, where wBattleType = BATTLE_TYPE_OLD_MAN
                // swaps in OldManPicBack (gfx/battle/oldmanb.png).
                let back_asset = if screen.is_old_man {
                    rm.load(AssetCategory::Battle, "oldmanb")
                } else {
                    rm.load(AssetCategory::Player, "redb")
                };
                if let Ok(cached) = back_asset {
                    let ts = cached.tileset.clone();
                    let src_tpr = (cached.source_size.0 / TILE_SIZE) as usize;
                    let scaled = scale_sprite_by_two(&ts, src_tpr);
                    let px = apply_offset(1 * TILE_SIZE, player_dx);
                    let py = apply_offset(5 * TILE_SIZE, player_dy);
                    blit_tileset(fb, &scaled, px, py, 7, pal);
                }
            } else if effects.fx.is_substitute(MonSide::Player) {
                // AnimationSubstitute: MonsterSprite mini doll, facing up on
                // the player side.
                if let Ok(cached) = rm.load_sprite("monster") {
                    let doll = cached.tileset.clone();
                    let rect = MonRect {
                        x: TILE_SIZE as i32 + player_dx,
                        y: 5 * TILE_SIZE as i32 + player_dy,
                    };
                    BattleEffects::draw_substitute(fb, rect, &doll, pal, MonSide::Player);
                }
            } else if effects.fx.is_minimized(MonSide::Player) {
                let rect = MonRect {
                    x: TILE_SIZE as i32 + player_dx,
                    y: 5 * TILE_SIZE as i32 + player_dy,
                };
                BattleEffects::draw_minimized(fb, rect, pal);
            } else {
                let back_sprite_name = format!("{}b", player_sprite);
                if let Ok(cached) = rm.load_pokemon_back(&back_sprite_name) {
                    let ts = cached.tileset.clone();
                    let src_tpr = (cached.source_size.0 / TILE_SIZE) as usize;
                    let scaled = scale_sprite_by_two(&ts, src_tpr);
                    let px = apply_offset(1 * TILE_SIZE, player_dx);
                    let py = apply_offset(5 * TILE_SIZE, player_dy);
                    if let Some((rows, yoff)) = effects.fx.slide_down_hide_params(MonSide::Player) {
                        BattleEffects::draw_mon_rows(
                            fb,
                            &scaled,
                            px as i32,
                            py as i32 + yoff,
                            7,
                            pal,
                            rows,
                        );
                    } else if let Some((width, anchor_right)) = effects.fx.squish_params(MonSide::Player) {
                        BattleEffects::draw_squished(
                            fb,
                            &scaled,
                            px as i32,
                            py as i32,
                            7,
                            pal,
                            width,
                            anchor_right,
                        );
                    } else {
                        blit_tileset(fb, &scaled, px, py, 7, pal);
                    }
                }
            }
        }

        if effects.fx.objects_active() {
            // SE object effects (spiral/shoot balls, petals/leaves, water
            // droplets) drawn with the move-animation tilesets.
            let ts0 = rm.load_battle("move_anim_0").map(|c| c.tileset.clone()).ok();
            let ts1 = rm.load_battle("move_anim_1").map(|c| c.tileset.clone()).ok();
            if let (Some(ts0), Some(ts1)) = (ts0, ts1) {
                effects.fx.render_objects(fb, &ts0, &ts1, pal);
            }
        }

        if !effects.anim_layer.entries.is_empty() {
            let anim_tileset_name = match effects.anim_tileset {
                1 => "move_anim_1",
                2 => "move_anim_0",
                _ => "move_anim_0",
            };
            if let Ok(cached) = rm.load_battle(anim_tileset_name) {
                effects
                    .anim_layer
                    .render(fb, &cached.tileset, pal, pal, None);
            }
        }

        // In Gen1 move-select, the TYPE/PP panel overlays the player sprite.
        // Our sprite blit happens after tilemap rendering, so redraw this panel
        // region last to keep it in the foreground.
        if matches!(screen.phase, BattlePhase::MoveSelect) {
            tile_buf.render_region(fb, &battle_ts, pal, 0, 8, 11, 5);
        }

        // Keep the bottom dialog/menu box in front of sprites and animation overlays.
        tile_buf.render_region(fb, &battle_ts, pal, 0, 12, 20, 6);

        effects.apply_post_effects(fb);
    } else {
        // No resources — fallback: render tile buffer with blank tileset
        let blank_ts = TileSet::blank(256);
        tile_buf.render(fb, &blank_ts, pal);
    }
}
