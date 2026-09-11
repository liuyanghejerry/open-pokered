use pokered_audio::sfx_data::SfxId;
use pokered_core::battle::state::StatusCondition as CoreStatus;
use pokered_core::battle::state::{status2, status3};
use pokered_core::battle::{
    BallAnimOutcome, BattleAnimEvent, BattlePhase, BattleScreen,
    BattleTransition as CoreTransition, IntroPhase,
};
use pokered_data::items::ItemId;
use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_data::ui_layout::schema::BATTLE_TEXT_DEFAULT_LAYOUT;
use pokered_renderer::battle_anim::{
    AnimEffect, AnimationType, BattleEffects, MonRect, MonSide, ANIM_BASE_TILE_ID,
};
use pokered_renderer::battle_scene::{
    BallIndicators, BallStatus, EnemyHud, PlayerHud, StatusCondition,
};
use pokered_renderer::battle_transition::{BattleTransitionKind, BattleTransitionState};
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::gen1_battle_anim::{
    draw_mon_pic_clipped, move_short_flash_timing, render_gen1_oam,
    render_gen1_oam_palette_split, render_gen1_slide_up, render_gen1_squish, AnimTickResult,
    AnimationPlayer, BallFrameEvent, BgPaletteState, BlinkMon, LongFlashTiming, LongScreenFlash,
    MonTilemapAnimation, RockSlideShake, ShakeBackAndForth, ShortFlashTiming, ShortScreenFlash,
};
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
/// toward the opponent until `AnimationResetMonPosition` restores it.
#[derive(Debug, Clone, Copy)]
struct MoveMonH {
    side: MonSide,
    frame: u8,
    /// `false` while `AnimationMoveMonHorizontally` is hiding/redrawing the
    /// shifted pic; `true` while `AnimationResetMonPosition` redraws it back.
    resetting: bool,
    reset_profile: MoveMonResetProfile,
    /// Tail Whip/Metronome's second forward copy is scanned bottom-first:
    /// the lower six tile rows move one VBlank before the top row.
    bottom_first_entry: bool,
}

#[derive(Debug, Clone, Copy)]
struct SquishRaster {
    side: MonSide,
    frame: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoveMonResetProfile {
    Normal,
    AfterSingleFlash,
    AfterDoubleFlash,
}

impl MoveMonH {
    fn base_is_shifted(self) -> bool {
        if !self.resetting {
            return if self.bottom_first_entry {
                self.frame >= 2
            } else {
                self.frame > 3
            };
        }
        match self.reset_profile {
            MoveMonResetProfile::Normal => self.frame < 4,
            MoveMonResetProfile::AfterSingleFlash => self.frame < 3,
            MoveMonResetProfile::AfterDoubleFlash => self.frame == 1,
        }
    }

    fn top_row_transition(self) -> Option<bool> {
        if !self.resetting {
            return if self.bottom_first_entry && matches!(self.frame, 2 | 3) {
                Some(false)
            } else {
                (self.frame == 3).then_some(true)
            };
        }
        match (self.reset_profile, self.frame) {
            (MoveMonResetProfile::Normal, 3) | (MoveMonResetProfile::AfterSingleFlash, 2) => {
                Some(false)
            }
            (MoveMonResetProfile::AfterDoubleFlash, 2 | 3) => Some(true),
            _ => None,
        }
    }
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
struct MonRasterTransfer {
    side: MonSide,
    frame: u8,
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
    ScreenFlash {
        remaining: u8,
    },
    SilhouetteSlide {
        remaining: u8,
        offset: i32,
    },
    /// Player send-out (AnimateSendingOutMon): POOF (stage 0), ball tile
    /// (stage 1, Delay3), 3×3 growth (stage 2, DelayFrames 4), 5×5 growth
    /// (stage 3, DelayFrames 5), then the full 7×7 pic + cry.
    PlayerSendOut {
        stage: u8,
        frames: u8,
    },
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
    /// TOSS_ANIM ($C1): SUBANIM_0_BALL_TOSS_HIGH (Poké Ball).
    pub const BALL_TOSS: usize = 0xC0;
    /// SHAKE_ANIM ($C2): SUBANIM_0_BALL_SHAKE_ENEMY.
    pub const BALL_SHAKE: usize = 0xC1;
    /// POOF_ANIM ($C3): SUBANIM_0_BALL_POOF_ENEMY.
    pub const BALL_POOF: usize = 0xC2;
    /// BLOCKBALL_ANIM ($C4): trainer knocks the thrown ball away.
    pub const BLOCK_BALL: usize = 0xC3;
    /// GREATTOSS_ANIM ($C5): SUBANIM_0_BALL_TOSS_MIDDLE (Great Ball).
    pub const GREAT_TOSS: usize = 0xC4;
    /// ULTRATOSS_ANIM ($C6): SUBANIM_0_BALL_TOSS_LOW
    /// (Ultra/Master/Safari Ball).
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
/// non-move animation. Per-frame special handling uses the raw subanimation
/// counter surfaced by [`AnimationPlayer`].
#[derive(Debug, Clone, Copy)]
struct BallStep {
    anim: usize,
}

/// The capture/ball-throw sequence currently playing (see
/// [`BattleAnimEvent`]). Drives `anim_player` through each [`BallStep`].
#[derive(Debug, Clone)]
struct BallChoreo {
    steps: Vec<BallStep>,
    step: usize,
    started: bool,
    shakes_remaining: u8,
    flash_toss: bool,
    ghost_dodge: bool,
    trainer_block: bool,
    hide_top_only: bool,
    show_pattern: BallShowPattern,
    ghost_transition_start: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BallShowPattern {
    StandardTop,
    BottomTwice,
    HiddenThenTop,
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
    };
    let poof = BallStep {
        anim: non_move_anim::BALL_POOF,
    };
    let hide = BallStep {
        anim: non_move_anim::HIDEPIC,
    };
    let shake = BallStep {
        anim: non_move_anim::BALL_SHAKE,
    };
    let show = BallStep {
        anim: non_move_anim::SHOWPIC,
    };
    let block = BallStep {
        anim: non_move_anim::BLOCK_BALL,
    };
    let low_toss = toss.anim == non_move_anim::ULTRA_TOSS;
    let show_pattern = match (low_toss, shakes) {
        (true, 1) | (false, 3) => BallShowPattern::BottomTwice,
        (true, 3) | (false, 2) => BallShowPattern::HiddenThenTop,
        _ => BallShowPattern::StandardTop,
    };
    let steps = match outcome {
        BallAnimOutcome::Dodged => vec![toss],
        // The trainer branch hard-codes TOSS_ANIM regardless of which ball
        // was selected, then plays BLOCKBALL_ANIM.
        BallAnimOutcome::Blocked => vec![
            BallStep {
                anim: non_move_anim::BALL_TOSS,
            },
            block,
        ],
        BallAnimOutcome::Caught => {
            let mut v = vec![toss, poof, hide];
            if shakes > 0 {
                v.push(shake);
            }
            v
        }
        BallAnimOutcome::BrokeFree => {
            if shakes == 0 {
                vec![toss, poof]
            } else {
                let mut v = vec![toss, poof, hide];
                v.push(shake);
                v.push(poof);
                v.push(show);
                v
            }
        }
    };
    BallChoreo {
        steps,
        step: 0,
        started: false,
        shakes_remaining: shakes,
        flash_toss: matches!(ball, ItemId::MasterBall | ItemId::UltraBall),
        ghost_dodge: outcome == BallAnimOutcome::Dodged,
        trainer_block: outcome == BallAnimOutcome::Blocked,
        hide_top_only: !low_toss,
        show_pattern,
        ghost_transition_start: if low_toss { 5 } else { 6 },
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
    move_mon_h_count: u8,
    anim_player: AnimationPlayer,
    current_attacker_is_player: bool,
    /// `wAnimationID` for the running animation (IsCryMove checks this, not
    /// the command's sound byte).
    current_move: MoveId,
    /// Species of the whose-turn mon, captured when the animation starts.
    current_attacker_species: pokered_data::species::Species,
    anim_wait: u8,
    anim_tileset: u8,
    /// OAM visible in the scanout being drawn this update.
    anim_layer: SpriteLayer,
    /// Shadow OAM submitted on this VBlank, visible on the next scanout.
    anim_layer_pending: SpriteLayer,
    /// Tile banks are latched with their corresponding visible/shadow OAM.
    anim_layer_tileset: u8,
    anim_layer_pending_tileset: u8,
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
    /// Exact partial tilemap transfers used by Splash and Acid Armor.
    mon_tilemap: MonTilemapAnimation,
    /// Tile-column copies and partial VBlank rows for SquishMonPic.
    squish_raster: Option<SquishRaster>,
    /// BG-map copy and scanout edges used by Double Team's mon shake.
    shake_back_and_forth: ShakeBackAndForth,
    /// Sequential WX/WY mutation used by Rock Slide's four hooks.
    rock_slide_shake: RockSlideShake,
    /// BG-map clear/restore and scanout edges used by `AnimationBlinkMon`.
    blink_mon: BlinkMon,
    /// Raster-accurate four-frame `AnimationFlashScreen` state.
    short_flash: ShortScreenFlash,
    /// Raster-accurate 48-frame `AnimationFlashScreenLong` state.
    long_flash: LongScreenFlash,
    /// Zero-based `AnimationFlashScreen` call index within the current move.
    short_flash_count: u8,
    /// Persistent BGP register plus the writes visible during this scanout.
    bg_palette: BgPaletteState,
    /// Three-VBlank BG-map transfer performed by AnimationShowMonPic.
    show_reveal: [Option<u8>; 2],
    /// Enemy-side `AnimationMinimizeMon` keeps the old BG picture for four
    /// scanouts while `CopyTempPicToMonPic` finishes.
    minimize_reveal_delay: [u8; 2],
    /// Delay the Substitute doll until its synchronous copy routine returns.
    substitute_reveal_delay: [u8; 2],
    /// Two visible thirds of the mon tilemap while Seismic Toss hides it.
    seismic_hide_raster: Option<MonRasterTransfer>,
    /// Player-side tilemap edge and hidden tail of `AnimationTransformMon`.
    transform_raster: Option<MonRasterTransfer>,
    /// Preserve the user's picture on the scanout where Selfdestruct or
    /// Explosion dispatches HideMonPic; the hidden tilemap appears next frame.
    hide_mon_one_frame: Option<MonSide>,
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
    /// Horizontal tilemap displacement accumulated by Ghost Marowak's dodge.
    ball_enemy_offset_x: i32,
    ball_enemy_top_offset_x: Option<i32>,
    ball_ghost_frame: Option<u8>,
    ball_ghost_transition_start: u8,
    /// OBP0's middle shades are swapped after each Master/Ultra toss frame.
    ball_obj_palette_flipped: bool,
    ball_obj_palette_frame_initial: bool,
    ball_obj_palette_write_scanline: Option<u32>,
    /// Three-VBlank BG-map clear performed by HIDEPIC_ANIM in a ball flow.
    ball_hide_raster: Option<u8>,
    ball_hide_top_only: bool,
    ball_show_pattern: Option<BallShowPattern>,
    /// Ball-flow SFX (BallToss / Tink per shake / BallPoof) queued for the
    /// frontend, which owns the audio device.
    pending_ball_sfx: std::collections::VecDeque<SfxId>,
    scheduled_ball_sfx: Vec<(u8, SfxId)>,
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
                let choreo = build_ball_choreo(ball, shakes, outcome);
                self.ball_enemy_offset_x = 0;
                self.ball_enemy_top_offset_x = None;
                self.ball_ghost_frame = None;
                self.ball_ghost_transition_start = choreo.ghost_transition_start;
                self.ball_obj_palette_flipped = false;
                self.ball_hide_raster = None;
                self.ball_hide_top_only = choreo.hide_top_only;
                self.ball_show_pattern = None;
                self.scheduled_ball_sfx.clear();
                self.ball_choreo = Some(choreo);
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
        self.mon_tilemap = MonTilemapAnimation::default();
        self.squish_raster = None;
        self.shake_back_and_forth = ShakeBackAndForth::default();
        self.rock_slide_shake.reset();
        self.blink_mon = BlinkMon::default();
        self.short_flash = ShortScreenFlash::default();
        self.long_flash = LongScreenFlash::default();
        self.short_flash_count = 0;
        self.move_mon_h_count = 0;
        self.show_reveal = [None; 2];
        self.minimize_reveal_delay = [0; 2];
        self.substitute_reveal_delay = [0; 2];
        self.seismic_hide_raster = None;
        self.transform_raster = None;
        self.hide_mon_one_frame = None;
        self.bg_palette.reset();
        self.anim_player.start(anim_index, player_is_attacker);
        self.anim_wait = 0;
        self.anim_layer.clear();
        self.anim_layer_pending.clear();
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
            move_mon_h_count: 0,
            anim_player: AnimationPlayer::new(),
            current_attacker_is_player: true,
            current_move: MoveId::None,
            current_attacker_species: pokered_data::species::Species::None,
            anim_wait: 0,
            anim_tileset: 0,
            anim_layer: SpriteLayer::new(),
            anim_layer_pending: SpriteLayer::new(),
            anim_layer_tileset: 0,
            anim_layer_pending_tileset: 0,
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
            mon_tilemap: MonTilemapAnimation::default(),
            squish_raster: None,
            shake_back_and_forth: ShakeBackAndForth::default(),
            rock_slide_shake: RockSlideShake::default(),
            blink_mon: BlinkMon::default(),
            short_flash: ShortScreenFlash::default(),
            long_flash: LongScreenFlash::default(),
            short_flash_count: 0,
            bg_palette: BgPaletteState::new(),
            show_reveal: [None; 2],
            minimize_reveal_delay: [0; 2],
            substitute_reveal_delay: [0; 2],
            seismic_hide_raster: None,
            transform_raster: None,
            hide_mon_one_frame: None,
            intro_anim: IntroAnimState::None,
            transition_state: None,
            overworld_snapshot: None,
            is_wild_intro: false,
            cry_pending: None,
            trainer_appear_sfx_pending: false,
            ball_choreo: None,
            ball_enemy_offset_x: 0,
            ball_enemy_top_offset_x: None,
            ball_ghost_frame: None,
            ball_ghost_transition_start: 0,
            ball_obj_palette_flipped: false,
            ball_obj_palette_frame_initial: false,
            ball_obj_palette_write_scanline: None,
            ball_hide_raster: None,
            ball_hide_top_only: false,
            ball_show_pattern: None,
            pending_ball_sfx: std::collections::VecDeque::new(),
            scheduled_ball_sfx: Vec::new(),
        }
    }
}

impl BattleVisualEffects {
    fn phase_kind(phase: &BattlePhase) -> BattlePhaseKind {
        match phase {
            BattlePhase::Intro { .. } => BattlePhaseKind::Intro,
            BattlePhase::PlayerMenu => BattlePhaseKind::PlayerMenu,
            BattlePhase::MoveSelect => BattlePhaseKind::MoveSelect,
            // Ether's per-move pick reuses the FIGHT move-menu view.
            BattlePhase::ItemMoveSelect { .. } => BattlePhaseKind::MoveSelect,
            BattlePhase::BagSelect => BattlePhaseKind::BagSelect,
            BattlePhase::ItemTargetSelect { .. } => BattlePhaseKind::ItemTargetSelect,
            BattlePhase::ShowingText { .. } | BattlePhase::EnemyFreeTurnAfterItem => {
                BattlePhaseKind::ShowingText
            }
            BattlePhase::PartySelect => BattlePhaseKind::PartySelect,
            BattlePhase::PartySubMenu { .. } => BattlePhaseKind::PartySubMenu,
            BattlePhase::PartyStats { .. } => BattlePhaseKind::PartyStats,
            BattlePhase::EnemySendingNext { .. } => BattlePhaseKind::EnemySendingNext,
            BattlePhase::ShiftPrompt => BattlePhaseKind::ShiftPrompt,
            BattlePhase::LearnMoveAsk { .. } | BattlePhase::LearnMoveGiveUpConfirm { .. } => {
                BattlePhaseKind::ShiftPrompt
            }
            BattlePhase::LearnMoveChoose { .. } => BattlePhaseKind::MoveSelect,
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
                self.enemy_entry = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::Legacy,
                });
                self.enemy_exit = None;
                self.enemy_half_off = false;
                self.fx.clear_side(MonSide::Enemy);
            }
            BattlePhase::PlayerFaintSwitch => {
                self.player_visible = true;
                self.player_entry = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::Legacy,
                });
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
                    CoreTransition::Spiral { outward } => {
                        BattleTransitionKind::Spiral { outward: *outward }
                    }
                    CoreTransition::Circle => BattleTransitionKind::Circle,
                    CoreTransition::SpiralTrainerStronger => {
                        BattleTransitionKind::Spiral { outward: true }
                    }
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
                self.enemy_entry = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::Legacy,
                });
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
                self.enemy_exit = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::Legacy,
                });
                // The original has NO enemy-mon entry animation: the trainer
                // pic slides off right, then the mon simply appears.
                self.cry_pending = Some(enemy_species);
            }
            IntroPhase::PlayerSendOut => {
                // SendOutMon (engine/battle/core.asm:1723): POOF_ANIM at the
                // player's side, then AnimateSendingOutMon grows the pic
                // 3×3 → 5×5 → 7×7; the cry fires when the growth completes.
                self.intro_anim = IntroAnimState::PlayerSendOut {
                    stage: 0,
                    frames: 0,
                };
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
                        self.mon_tilemap = MonTilemapAnimation::default();
                        self.squish_raster = None;
                        self.shake_back_and_forth = ShakeBackAndForth::default();
                        self.rock_slide_shake.reset();
                        self.blink_mon = BlinkMon::default();
                        self.short_flash = ShortScreenFlash::default();
                        self.long_flash = LongScreenFlash::default();
                        self.short_flash_count = 0;
                        self.move_mon_h_count = 0;
                        self.show_reveal = [None; 2];
                        self.minimize_reveal_delay = [0; 2];
                        self.substitute_reveal_delay = [0; 2];
                        self.seismic_hide_raster = None;
                        self.transform_raster = None;
                        self.hide_mon_one_frame = None;
                        self.bg_palette.reset();
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
                                self.anim_layer_pending.clear();
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
            self.player_entry = Some(SlideAnim {
                frame: 0,
                kind: SlideKind::Legacy,
            });
            self.player_exit = None;
            self.player_half_off = false;
            self.fx.clear_side(MonSide::Player);
        }

        if normalized.contains("come back!") {
            self.player_exit = Some(SlideAnim {
                frame: 0,
                kind: SlideKind::Legacy,
            });
            self.player_entry = None;
            self.player_half_off = false;
            self.fx.clear_side(MonSide::Player);
        }

        if normalized.ends_with("fainted!") {
            if normalized.starts_with("Enemy ") {
                // SlideDownFaintedMonPic: enemy mon slides DOWN, not right.
                self.enemy_exit = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::Faint,
                });
                self.enemy_entry = None;
                self.enemy_half_off = false;
                self.fx.clear_side(MonSide::Enemy);
            } else {
                self.player_exit = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::Faint,
                });
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

    fn side_index(side: MonSide) -> usize {
        match side {
            MonSide::Player => 0,
            MonSide::Enemy => 1,
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

        if self.current_move == MoveId::DoubleTeam
            && matches!(effect, AnimEffect::ResetScreenPalette)
        {
            self.shake_back_and_forth.prime(attacker);
        }

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

        self.mon_tilemap.start(&effect, attacker);
        if matches!(effect, AnimEffect::SquishMonPic) {
            self.squish_raster = Some(SquishRaster {
                side: attacker,
                frame: 0,
            });
        }
        let palette_bgp = match effect {
            AnimEffect::DarkScreenPalette => Some(0x6f),
            AnimEffect::LightScreenPalette => Some(0x90),
            AnimEffect::DarkenMonPalette => Some(0xf9),
            AnimEffect::ResetScreenPalette => Some(0xe4),
            _ => None,
        };
        let generic_wait = match effect {
            _ if palette_bgp.is_some() => {
                self.bg_palette.write(
                    palette_bgp.expect("palette effect has a BGP value"),
                    self.palette_write_scanline(&effect),
                );
                0
            }
            AnimEffect::FlashScreen { frames } if frames <= 4 => {
                let timing = self.short_flash_timing();
                self.short_flash
                    .start(self.bg_palette.current_bgp(), timing);
                self.short_flash_count = self.short_flash_count.saturating_add(1);
                frames
            }
            AnimEffect::FlashScreen { frames } => {
                let timing = self.long_flash_timing();
                self.long_flash.start(self.bg_palette.current_bgp(), timing);
                frames
            }
            AnimEffect::ShakeBackAndForth => {
                self.shake_back_and_forth.start(attacker);
                96
            }
            AnimEffect::ShakeScreenHV { .. } if self.current_move == MoveId::RockSlide => {
                self.rock_slide_shake.start(self.current_attacker_is_player);
                0
            }
            AnimEffect::TransformMon => {
                self.transform_raster = Some(MonRasterTransfer {
                    side: attacker,
                    frame: 0,
                });
                0
            }
            AnimEffect::BlinkPlayerMon { .. } => {
                self.blink_mon.start(attacker);
                0
            }
            AnimEffect::BlinkEnemyMon { .. } => {
                self.blink_mon.start(defender);
                0
            }
            AnimEffect::HideEnemyMon if self.ball_choreo.is_some() => {
                self.ball_hide_raster = Some(0);
                0
            }
            _ => self.fx.apply(&effect, attacker),
        };
        let wait = AnimationPlayer::effect_duration(&effect, attacker).unwrap_or(generic_wait);
        if matches!(effect, AnimEffect::SubstituteMon) {
            self.substitute_reveal_delay[Self::side_index(attacker)] =
                if attacker == MonSide::Enemy { 10 } else { 11 };
        }
        if wait > 0 {
            // The effect is applied before this update is rendered, so the
            // current display frame is already the first blocked VBlank.
            self.anim_wait = self.anim_wait.max(wait.saturating_sub(1));
        }

        // Frontend-side flows: visibility, mon slides and lunges.
        match effect {
            // SE_HIDE/SHOW_MON_PIC act on whose-turn mon; the "Enemy"
            // variants use CallWithTurnFlipped. The forced-hidden latch
            // lives in `self.fx`; these flags cover the battle flow.
            AnimEffect::ShowPlayerMon | AnimEffect::SubstituteMon | AnimEffect::MinimizeMon => {
                self.set_visible(attacker, true);
                if matches!(effect, AnimEffect::MinimizeMon) {
                    self.minimize_reveal_delay[Self::side_index(attacker)] =
                        if attacker == MonSide::Enemy { 5 } else { 0 };
                }
                if matches!(effect, AnimEffect::ShowPlayerMon)
                    && self.current_move != MoveId::DoubleTeam
                {
                    self.show_reveal[Self::side_index(attacker)] = Some(0);
                    self.anim_wait = self.anim_wait.max(2);
                }
            }
            AnimEffect::ShowEnemyMon => {
                self.set_visible(defender, true);
                if self.current_move != MoveId::DoubleTeam {
                    if let Some(pattern) = self.ball_choreo.as_ref().map(|c| c.show_pattern) {
                        self.ball_show_pattern = Some(pattern);
                        self.show_reveal[Self::side_index(defender)] = Some(
                            if pattern == BallShowPattern::StandardTop {
                                1
                            } else {
                                0
                            },
                        );
                    } else {
                        self.show_reveal[Self::side_index(defender)] = Some(0);
                    }
                    self.anim_wait = self.anim_wait.max(2);
                }
            }
            AnimEffect::HidePlayerMon => {
                self.show_reveal[Self::side_index(attacker)] = None;
                if matches!(self.current_move, MoveId::Selfdestruct | MoveId::Explosion) {
                    self.hide_mon_one_frame = Some(attacker);
                }
            }
            AnimEffect::HideEnemyMon => {
                self.show_reveal[Self::side_index(defender)] = None;
                if self.current_move == MoveId::SeismicToss {
                    self.seismic_hide_raster = Some(MonRasterTransfer {
                        side: defender,
                        frame: 0,
                    });
                }
                if matches!(self.current_move, MoveId::Selfdestruct | MoveId::Explosion) {
                    self.hide_mon_one_frame = Some(defender);
                }
            }
            AnimEffect::SlideEnemyMonOff => {
                // AnimationSlideMonOff: e = 8 tiles, wSlideMonDelay = 3.
                *self.slide_slot(defender, false) = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::SeOff,
                });
                self.anim_wait = self.anim_wait.max(23);
            }
            AnimEffect::SlidePlayerMonOff => {
                *self.slide_slot(attacker, false) = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::SeOff,
                });
                self.anim_wait = self.anim_wait.max(23);
            }
            AnimEffect::SlidePlayerMonHalfOff => {
                // AnimationSlideMonHalfOff: e = 4 tiles, wSlideMonDelay = 4;
                // the mon stays half-off (Softboiled).
                *self.slide_slot(attacker, false) = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::SeHalfOff,
                });
                self.anim_wait = self.anim_wait.max(18);
            }
            AnimEffect::SlidePlayerMonDown => {
                *self.slide_slot(attacker, false) = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::SeDown,
                });
                self.anim_wait = self.anim_wait.max(20);
            }
            AnimEffect::SlidePlayerMonUp => {
                self.set_visible(attacker, true);
                *self.slide_slot(attacker, true) = Some(SlideAnim {
                    frame: 0,
                    kind: SlideKind::SeUp,
                });
                self.anim_wait = self.anim_wait.max(13);
            }
            AnimEffect::ResetPlayerMonPosition => {
                // AnimationResetMonPosition redraws the normal pic through
                // AnimationShowMonPic, whose final Delay3 blocks here.
                let reset_profile = match self.current_move {
                    MoveId::BodySlam => MoveMonResetProfile::AfterDoubleFlash,
                    MoveId::TakeDown | MoveId::DoubleEdge => MoveMonResetProfile::AfterSingleFlash,
                    MoveId::TailWhip | MoveId::Metronome if self.move_mon_h_count == 1 => {
                        MoveMonResetProfile::AfterSingleFlash
                    }
                    _ => MoveMonResetProfile::Normal,
                };
                if let Some(anim) = self.move_mon_h.as_mut() {
                    anim.frame = 0;
                    anim.resetting = true;
                    anim.reset_profile = reset_profile;
                }
                match attacker {
                    MonSide::Player => self.player_half_off = false,
                    MonSide::Enemy => self.enemy_half_off = false,
                }
                self.set_visible(attacker, true);
                self.anim_wait = self.anim_wait.max(2);
            }
            AnimEffect::MovePlayerMonH => {
                // AnimationMoveMonHorizontally: hold the mon 1 tile toward
                // the opponent for 3 frames (Tackle/Body Slam).
                self.move_mon_h = Some(MoveMonH {
                    side: attacker,
                    frame: 0,
                    resetting: false,
                    reset_profile: MoveMonResetProfile::Normal,
                    bottom_first_entry: matches!(
                        self.current_move,
                        MoveId::TailWhip | MoveId::Metronome
                    ) && self.move_mon_h_count == 1,
                });
                self.move_mon_h_count = self.move_mon_h_count.saturating_add(1);
                self.anim_wait = self.anim_wait.max(2);
            }
            AnimEffect::Delay10 => {
                self.anim_wait = self.anim_wait.max(9);
            }
            _ => {}
        }
    }

    fn short_flash_timing(&self) -> ShortFlashTiming {
        if matches!(self.anim_player.animation_id(), 0xAE | 0xAF) {
            return if self.current_attacker_is_player {
                ShortFlashTiming {
                    entry_scanline: 17,
                    white_scanline: 8,
                    restore_scanline: 8,
                }
            } else {
                ShortFlashTiming {
                    entry_scanline: 17,
                    white_scanline: 8,
                    restore_scanline: 8,
                }
            };
        }
        if let Some(timing) = move_short_flash_timing(
            self.current_move as u8,
            self.current_attacker_is_player,
            usize::from(self.short_flash_count),
        ) {
            return timing;
        }
        match (self.current_move, self.current_attacker_is_player) {
            (MoveId::Growth, _) => ShortFlashTiming {
                entry_scanline: 17,
                white_scanline: 8,
                restore_scanline: 9,
            },
            (MoveId::Leer | MoveId::Disable, true) if self.short_flash_count == 0 => {
                ShortFlashTiming {
                    entry_scanline: 26,
                    white_scanline: 9,
                    restore_scanline: 15,
                }
            }
            (MoveId::Leer | MoveId::Disable, false) if self.short_flash_count == 0 => {
                ShortFlashTiming {
                    entry_scanline: 27,
                    white_scanline: 9,
                    restore_scanline: 16,
                }
            }
            (MoveId::Leer | MoveId::Disable, _) => ShortFlashTiming {
                entry_scanline: 25,
                white_scanline: 9,
                restore_scanline: 9,
            },
            (MoveId::Glare, true) if self.short_flash_count == 0 => ShortFlashTiming {
                entry_scanline: 24,
                white_scanline: 9,
                restore_scanline: 9,
            },
            (MoveId::Glare, false) if self.short_flash_count == 0 => ShortFlashTiming {
                entry_scanline: 25,
                white_scanline: 9,
                restore_scanline: 9,
            },
            (MoveId::Flash, true) if self.short_flash_count == 0 => ShortFlashTiming {
                entry_scanline: 26,
                white_scanline: 9,
                restore_scanline: 9,
            },
            (MoveId::Flash, false) if self.short_flash_count == 0 => ShortFlashTiming {
                entry_scanline: 27,
                white_scanline: 9,
                restore_scanline: 9,
            },
            (MoveId::Glare | MoveId::Flash, true) => ShortFlashTiming {
                entry_scanline: 9,
                white_scanline: 16,
                restore_scanline: 9,
            },
            (MoveId::Glare | MoveId::Flash, false) => ShortFlashTiming {
                entry_scanline: 10,
                white_scanline: 16,
                restore_scanline: 9,
            },
            (MoveId::DoubleEdge, _) => ShortFlashTiming {
                entry_scanline: 15,
                white_scanline: 9,
                restore_scanline: 9,
            },
            (MoveId::MegaDrain, true) if self.short_flash_count == 0 => ShortFlashTiming {
                entry_scanline: 22,
                white_scanline: 8,
                restore_scanline: 9,
            },
            (MoveId::MegaDrain, false) if self.short_flash_count == 0 => ShortFlashTiming {
                entry_scanline: 23,
                white_scanline: 9,
                restore_scanline: 9,
            },
            (MoveId::MegaDrain, true) => ShortFlashTiming {
                entry_scanline: 44,
                white_scanline: 8,
                restore_scanline: 8,
            },
            (MoveId::MegaDrain, false) => ShortFlashTiming {
                entry_scanline: 21,
                white_scanline: 9,
                restore_scanline: 8,
            },
            (MoveId::Thunder, true) if self.short_flash_count == 0 => ShortFlashTiming {
                entry_scanline: 18,
                white_scanline: 9,
                restore_scanline: 9,
            },
            (MoveId::Thunder, false) if self.short_flash_count == 0 => ShortFlashTiming {
                entry_scanline: 19,
                white_scanline: 9,
                restore_scanline: 9,
            },
            (MoveId::Thunder, true) => ShortFlashTiming {
                entry_scanline: 21,
                white_scanline: 8,
                restore_scanline: 9,
            },
            (MoveId::Thunder, false) => ShortFlashTiming {
                entry_scanline: 21,
                white_scanline: 8,
                restore_scanline: 17,
            },
            (MoveId::Meditate, true) => ShortFlashTiming {
                entry_scanline: 21,
                white_scanline: 9,
                restore_scanline: 8,
            },
            (MoveId::Meditate, false) => ShortFlashTiming {
                entry_scanline: 21,
                white_scanline: 8,
                restore_scanline: 9,
            },
            (MoveId::DoubleTeam, true) => ShortFlashTiming {
                entry_scanline: 9,
                white_scanline: 8,
                restore_scanline: 9,
            },
            (MoveId::DoubleTeam, false) if self.short_flash_count == 0 => ShortFlashTiming {
                entry_scanline: 9,
                white_scanline: 8,
                restore_scanline: 9,
            },
            (MoveId::DoubleTeam, false) => ShortFlashTiming {
                entry_scanline: 9,
                white_scanline: 8,
                restore_scanline: 8,
            },
            (MoveId::Recover, _) => ShortFlashTiming {
                entry_scanline: 17,
                white_scanline: 8,
                restore_scanline: 8,
            },
            (MoveId::Harden, _) => ShortFlashTiming {
                entry_scanline: 22,
                white_scanline: 9,
                restore_scanline: 9,
            },
            (MoveId::Minimize, _) => ShortFlashTiming {
                entry_scanline: 17,
                white_scanline: 8,
                restore_scanline: 9,
            },
            (MoveId::DefenseCurl, true) => ShortFlashTiming {
                entry_scanline: 21,
                white_scanline: 8,
                restore_scanline: 9,
            },
            (MoveId::DefenseCurl, false) => ShortFlashTiming {
                entry_scanline: 21,
                white_scanline: 8,
                restore_scanline: 8,
            },
            (MoveId::Softboiled, true) => ShortFlashTiming {
                entry_scanline: 17,
                white_scanline: 8,
                restore_scanline: 9,
            },
            (MoveId::Softboiled, false) => ShortFlashTiming {
                entry_scanline: 18,
                white_scanline: 9,
                restore_scanline: 8,
            },
            (MoveId::Sharpen, _) => ShortFlashTiming {
                entry_scanline: 21,
                white_scanline: 8,
                restore_scanline: 8,
            },
            (MoveId::HyperBeam, _) if self.short_flash_count == 0 => ShortFlashTiming {
                entry_scanline: 17,
                white_scanline: 8,
                restore_scanline: 9,
            },
            (MoveId::HyperBeam, true) if self.short_flash_count == 1 => ShortFlashTiming {
                entry_scanline: 16,
                white_scanline: 32,
                restore_scanline: 9,
            },
            (MoveId::HyperBeam, false) if self.short_flash_count == 1 => ShortFlashTiming {
                entry_scanline: 24,
                white_scanline: 24,
                restore_scanline: 9,
            },
            (MoveId::HyperBeam, true) if self.short_flash_count == 2 => ShortFlashTiming {
                entry_scanline: 31,
                white_scanline: 9,
                restore_scanline: 17,
            },
            (MoveId::HyperBeam, false) if self.short_flash_count == 2 => ShortFlashTiming {
                entry_scanline: 31,
                white_scanline: 17,
                restore_scanline: 9,
            },
            (MoveId::HyperBeam, true) if self.short_flash_count == 3 => ShortFlashTiming {
                entry_scanline: 16,
                white_scanline: 9,
                restore_scanline: 24,
            },
            (MoveId::HyperBeam, false) if self.short_flash_count == 3 => ShortFlashTiming {
                entry_scanline: 16,
                white_scanline: 9,
                restore_scanline: 32,
            },
            (MoveId::HyperBeam, true) if self.short_flash_count == 4 => ShortFlashTiming {
                entry_scanline: 27,
                white_scanline: 18,
                restore_scanline: 31,
            },
            (MoveId::HyperBeam, false) if self.short_flash_count == 4 => ShortFlashTiming {
                entry_scanline: 27,
                white_scanline: 18,
                restore_scanline: 38,
            },
            (MoveId::HyperBeam, true) => ShortFlashTiming {
                entry_scanline: 31,
                white_scanline: 18,
                restore_scanline: 16,
            },
            (MoveId::HyperBeam, false) => ShortFlashTiming {
                entry_scanline: 39,
                white_scanline: 18,
                restore_scanline: 16,
            },
            (MoveId::BodySlam, _) if self.short_flash_count > 0 => ShortFlashTiming::default(),
            (MoveId::BodySlam, true) => ShortFlashTiming {
                entry_scanline: 9,
                white_scanline: 9,
                restore_scanline: 18,
            },
            (MoveId::BodySlam, false) => ShortFlashTiming {
                entry_scanline: 9,
                white_scanline: 24,
                restore_scanline: 9,
            },
            (MoveId::TakeDown, true) => ShortFlashTiming {
                entry_scanline: 15,
                white_scanline: 9,
                restore_scanline: 18,
            },
            (MoveId::TakeDown, false) => ShortFlashTiming {
                entry_scanline: 16,
                white_scanline: 24,
                restore_scanline: 9,
            },
            _ => ShortFlashTiming::default(),
        }
    }

    fn long_flash_timing(&self) -> LongFlashTiming {
        let player = self.current_attacker_is_player;
        let write_scanlines = match (self.current_move, player) {
            (MoveId::Psybeam, true) => [30, 10, 27, 10, 24, 10, 16, 10, 17, 10, 16, 10],
            (MoveId::Psybeam, false) => [30, 10, 35, 17, 17, 10, 16, 10, 17, 17, 16, 10],
            (MoveId::PsychicM, true) => [23, 25, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9],
            (MoveId::PsychicM, false) => [24, 25, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9],
            (MoveId::Hypnosis, true) => [20, 8, 9, 9, 9, 9, 9, 9, 10, 9, 9, 9],
            (MoveId::Hypnosis, false) => [21, 8, 9, 9, 9, 9, 9, 9, 10, 9, 9, 9],
            (MoveId::DreamEater, true) => [20, 8, 9, 9, 9, 9, 9, 9, 10, 18, 9, 8],
            (MoveId::DreamEater, false) => [21, 8, 9, 9, 9, 9, 9, 9, 10, 18, 9, 8],
            (_, true) => LongFlashTiming::default().write_scanlines,
            (_, false) => [19, 15, 10, 9, 10, 8, 8, 9, 9, 8, 8, 8],
        };
        LongFlashTiming { write_scanlines }
    }

    fn palette_write_scanline(&self, effect: &AnimEffect) -> u32 {
        let player = self.current_attacker_is_player;
        let side = |player_line, enemy_line| if player { player_line } else { enemy_line };
        if matches!(self.anim_player.animation_id(), 0xAE | 0xAF)
            && matches!(effect, AnimEffect::LightScreenPalette)
        {
            return side(13, 14);
        }
        match (self.current_move, effect) {
            (MoveId::Smokescreen, AnimEffect::DarkenMonPalette)
                if self.bg_palette.current_bgp() == 0xe4 =>
            {
                23
            }
            (MoveId::Smokescreen, _) => 9,
            (MoveId::Agility, AnimEffect::ResetScreenPalette) => {
                if player {
                    19
                } else {
                    20
                }
            }
            (MoveId::Thunderpunch, AnimEffect::ResetScreenPalette) => side(21, 22),
            (MoveId::DoubleEdge, AnimEffect::ResetScreenPalette) => 22,
            (MoveId::Mist, AnimEffect::ResetScreenPalette) => side(10, 11),
            (MoveId::HyperBeam, AnimEffect::ResetScreenPalette) => 36,
            (MoveId::Absorb, AnimEffect::ResetScreenPalette) => side(23, 22),
            (MoveId::MegaDrain, AnimEffect::ResetScreenPalette) => 9,
            (MoveId::PetalDance, AnimEffect::ResetScreenPalette) => 18,
            (MoveId::Thunder, AnimEffect::ResetScreenPalette) => side(31, 23),
            (MoveId::Meditate, AnimEffect::ResetScreenPalette) => side(9, 10),
            (MoveId::DoubleTeam, AnimEffect::ResetScreenPalette) => side(10, 9),
            (MoveId::Recover, AnimEffect::ResetScreenPalette) => 9,
            (MoveId::Minimize, AnimEffect::ResetScreenPalette) => 9,
            (MoveId::ConfuseRay, AnimEffect::ResetScreenPalette) => 22,
            (MoveId::Withdraw, AnimEffect::ResetScreenPalette) => side(22, 44),
            (MoveId::DefenseCurl, AnimEffect::ResetScreenPalette) => side(10, 9),
            (MoveId::LightScreen, AnimEffect::ResetScreenPalette) => side(22, 21),
            (MoveId::Haze, AnimEffect::ResetScreenPalette) => side(10, 11),
            (MoveId::Reflect, AnimEffect::ResetScreenPalette) => side(10, 9),
            (MoveId::Smog, AnimEffect::ResetScreenPalette) => 22,
            (MoveId::Softboiled, AnimEffect::ResetScreenPalette) => side(10, 8),
            (MoveId::DreamEater, AnimEffect::DarkenMonPalette) => 18,
            (MoveId::DreamEater, AnimEffect::DarkScreenPalette) => 18,
            (MoveId::DreamEater, AnimEffect::ResetScreenPalette) => side(31, 23),
            (MoveId::Sharpen, AnimEffect::ResetScreenPalette) => 9,
            (MoveId::SuperFang, AnimEffect::ResetScreenPalette) => side(22, 21),
            (_, AnimEffect::ResetScreenPalette) => 10,
            (MoveId::Thunderpunch, AnimEffect::DarkScreenPalette) => 22,
            (
                MoveId::Leer
                | MoveId::Disable
                | MoveId::HyperBeam
                | MoveId::Thunder
                | MoveId::ConfuseRay
                | MoveId::SuperFang,
                AnimEffect::DarkScreenPalette,
            ) => {
                if player {
                    17
                } else {
                    18
                }
            }
            (MoveId::Glare, AnimEffect::DarkScreenPalette) => side(17, 18),
            (MoveId::DoubleTeam | MoveId::Reflect, AnimEffect::DarkScreenPalette) => side(11, 12),
            (MoveId::DoubleEdge | MoveId::PetalDance, AnimEffect::LightScreenPalette) => {
                side(19, 20)
            }
            (MoveId::Mist, AnimEffect::LightScreenPalette) => side(13, 14),
            (MoveId::Absorb | MoveId::MegaDrain, AnimEffect::LightScreenPalette) => side(21, 22),
            (MoveId::Meditate, AnimEffect::LightScreenPalette) => side(18, 19),
            (MoveId::Recover, AnimEffect::LightScreenPalette) => 11,
            (MoveId::Harden | MoveId::Minimize, AnimEffect::LightScreenPalette) => side(19, 20),
            (
                MoveId::Withdraw | MoveId::DefenseCurl | MoveId::Sharpen,
                AnimEffect::LightScreenPalette,
            ) => side(21, 22),
            (MoveId::Softboiled, AnimEffect::LightScreenPalette) => side(23, 24),
            (MoveId::Haze, AnimEffect::DarkenMonPalette) => side(12, 13),
            (MoveId::Smog, AnimEffect::DarkenMonPalette) => side(18, 19),
            (MoveId::Growth | MoveId::Flash, AnimEffect::LightScreenPalette) => {
                if player {
                    19
                } else {
                    20
                }
            }
            (MoveId::LightScreen, AnimEffect::LightScreenPalette) => {
                if player {
                    13
                } else {
                    14
                }
            }
            (MoveId::Agility, AnimEffect::LightScreenPalette) => {
                if player {
                    18
                } else {
                    19
                }
            }
            _ => 10,
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

    /// Copy the shared Gen-I driver's screen-space OAM into the render layer.
    fn commit_move_animation_frame(&mut self) {
        self.anim_layer_pending.clear();
        for entry in self.anim_player.oam_entries() {
            let mut entry = *entry;
            entry.tile_id = entry.tile_id.wrapping_sub(ANIM_BASE_TILE_ID);
            if entry.is_on_screen(160, 144) {
                self.anim_layer_pending.add(entry);
            }
        }
        if let Some(tileset) = self.anim_player.current_tileset() {
            self.anim_tileset = tileset;
            self.anim_layer_pending_tileset = tileset;
        }
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

        for _ in 0..1024 {
            match self.anim_player.tick() {
                AnimTickResult::Loading { sound } | AnimTickResult::Display { sound } => {
                    if let Some(sound_move) = sound {
                        self.emit_move_sfx(sound_move);
                    }
                    self.handle_ball_frame_event();
                    self.commit_move_animation_frame();
                    return;
                }
                AnimTickResult::Hook { sound, effect } => {
                    if let Some(sound_move) = sound {
                        self.emit_move_sfx(sound_move);
                    }
                    self.commit_move_animation_frame();
                    self.apply_anim_effect(effect);
                    if self.anim_wait > 0 {
                        return;
                    }
                }
                AnimTickResult::Effect { sound, effect } => {
                    if let Some(sound_move) = sound {
                        self.emit_move_sfx(sound_move);
                    }
                    self.commit_move_animation_frame();
                    self.apply_anim_effect(AnimationPlayer::apply_effect(effect));
                    if self.anim_wait > 0 {
                        return;
                    }
                }
                AnimTickResult::Done => {
                    if !self.anim_player.preserves_oam_when_finished() {
                        self.anim_layer_pending.clear();
                    }
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
                    return;
                }
            }
        }
        debug_assert!(false, "move animation executed too many zero-time commands");
    }

    fn handle_ball_frame_event(&mut self) {
        let Some(event) = self.anim_player.take_ball_frame_event() else {
            return;
        };
        let Some(choreo) = self.ball_choreo.as_mut() else {
            return;
        };
        match event {
            BallFrameEvent::Toss { counter } => {
                if choreo.flash_toss && counter < 11 {
                    self.ball_obj_palette_frame_initial = self.ball_obj_palette_flipped;
                    self.ball_obj_palette_flipped = !self.ball_obj_palette_flipped;
                    self.ball_obj_palette_write_scanline = Some(if counter == 2 {
                        if choreo.trainer_block { 28 } else { 44 }
                    } else {
                        0
                    });
                }
                if counter == 11 {
                    let delay = if self.anim_player.animation_id() == 0xC6 {
                        3
                    } else {
                        4
                    };
                    self.scheduled_ball_sfx.push((delay, SfxId::BallToss));
                }
                if choreo.ghost_dodge && counter == 3 {
                    self.ball_ghost_frame = Some(0);
                }
                if choreo.trainer_block && counter == 2 {
                    self.anim_player.skip_next_subanimation_frame();
                }
            }
            BallFrameEvent::Shake { counter } => {
                if counter == 4 {
                    self.scheduled_ball_sfx.push((4, SfxId::Tink));
                    self.anim_wait = self.anim_wait.max(40);
                } else if counter == 1 && choreo.shakes_remaining > 1 {
                    choreo.shakes_remaining -= 1;
                    self.anim_player.repeat_current_subanimation();
                }
            }
            BallFrameEvent::Poof { counter: 5 } => {
                self.scheduled_ball_sfx.push((5, SfxId::BallPoof));
            }
            BallFrameEvent::Poof { .. } => {}
        }
    }

    fn tick_scheduled_ball_sfx(&mut self) {
        for (remaining, _) in &mut self.scheduled_ball_sfx {
            *remaining = remaining.saturating_sub(1);
        }
        let mut index = 0;
        while index < self.scheduled_ball_sfx.len() {
            if self.scheduled_ball_sfx[index].0 == 0 {
                let (_, sfx) = self.scheduled_ball_sfx.remove(index);
                self.pending_ball_sfx.push_back(sfx);
            } else {
                index += 1;
            }
        }
    }

    /// Start and chain each raw animation used by `TossBallAnimation`.
    fn advance_ball_choreo(&mut self) -> bool {
        let Some(choreo) = self.ball_choreo.as_mut() else {
            return false;
        };
        if !choreo.started {
            let step = choreo.steps[choreo.step];
            if choreo.step == 0 {
                self.anim_player.start(step.anim, true);
            } else {
                self.anim_player.start_preserving_oam(step.anim, true);
            }
            self.anim_wait = 0;
            self.current_move = MoveId::None;
            if step.anim == non_move_anim::BLOCK_BALL {
                self.pending_ball_sfx.push_back(SfxId::FaintThud);
            }
            choreo.started = true;
            return true;
        }
        if self.anim_player.is_finished() && self.anim_wait == 0 {
            let finished_anim = choreo.steps[choreo.step].anim;
            if matches!(
                finished_anim,
                non_move_anim::BALL_TOSS | non_move_anim::GREAT_TOSS | non_move_anim::ULTRA_TOSS
            ) {
                self.ball_obj_palette_flipped = false;
                self.ball_obj_palette_frame_initial = false;
                self.ball_obj_palette_write_scanline = None;
            }
            choreo.step += 1;
            choreo.started = false;
            if choreo.step >= choreo.steps.len() {
                self.ball_choreo = None;
                self.ball_obj_palette_flipped = false;
            } else {
                let step = choreo.steps[choreo.step];
                self.anim_player.start_preserving_oam(step.anim, true);
                self.current_move = MoveId::None;
                if step.anim == non_move_anim::BLOCK_BALL {
                    self.pending_ball_sfx.push_back(SfxId::FaintThud);
                }
                choreo.started = true;
                return true;
            }
        }
        false
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
            SlideKind::SeDown => 22,
            // _AnimationSlideMonUp: 7 rows × Delay3.
            SlideKind::SeUp => 21,
        }
    }

    /// Pixel offset contributed by an exit slide. Player slides left, enemy
    /// slides right (`_AnimationSlideMonOff` shifts player tile ids +7 / enemy
    /// −7, i.e. the pics move off their respective screen edges).
    fn exit_slide_offset(&self, kind: SlideKind, frame: u8, is_player: bool) -> (i32, i32) {
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
                let d = if self.current_move == MoveId::SeismicToss {
                    (f / 3) * 8
                } else if self.current_move == MoveId::Whirlwind {
                    ((f + 1) / 3) * 8
                } else {
                    ((f - 1).max(0) / 3) * 8
                };
                (if is_player { -d } else { d }, 0)
            }
            SlideKind::SeHalfOff => {
                let d = match f {
                    0..=3 => 0,
                    4..=6 => 8,
                    7..=9 => 16,
                    10..=15 => 24,
                    _ => 32,
                };
                (if is_player { -d } else { d }, 0)
            }
            SlideKind::SeDown => {
                let d = if f < 7 { 0 } else { ((f - 4) / 3) * 8 };
                (0, if is_player { d } else { d.min(48) })
            }
            SlideKind::SeUp => (0, 0),
        }
    }

    fn horizontal_slide_top_dx(&self, side: MonSide) -> Option<i32> {
        if side == MonSide::Enemy {
            if let Some(dx) = self.ball_enemy_top_offset_x {
                return Some(dx);
            }
        }
        if let Some(dx) = self.shake_back_and_forth.top_dx(side) {
            return Some(dx);
        }
        if self.current_move == MoveId::Softboiled
            && matches!(self.show_reveal[Self::side_index(side)], Some(2 | 3))
        {
            return Some(match side {
                MonSide::Player => -32,
                MonSide::Enemy => 32,
            });
        }
        let anim = match side {
            MonSide::Player => self.player_exit?,
            MonSide::Enemy => self.enemy_exit?,
        };
        let toward_opponent = match side {
            MonSide::Player => -8,
            MonSide::Enemy => 8,
        };
        match anim.kind {
            SlideKind::SeOff if self.current_move == MoveId::SeismicToss => {
                (anim.frame % 3 == 2).then_some(toward_opponent)
            }
            SlideKind::SeOff
                if self.current_move == MoveId::Whirlwind
                    && anim.frame >= 2
                    && (anim.frame + 1) % 3 != 2 =>
            {
                Some(-toward_opponent)
            }
            SlideKind::SeOff if anim.frame != 0 && anim.frame % 3 == 0 => Some(toward_opponent),
            SlideKind::SeHalfOff => match anim.frame {
                3 | 6 | 15 => Some(toward_opponent),
                10 | 11 => Some(-toward_opponent),
                _ => None,
            },
            _ => None,
        }
    }

    fn horizontal_slide_top_split(&self, side: MonSide) -> u32 {
        if self.shake_back_and_forth.top_dx(side).is_some() {
            self.shake_back_and_forth.top_split(side)
        } else {
            48
        }
    }

    fn horizontal_slide_clip(&self, side: MonSide) -> (i32, i32) {
        if side == MonSide::Enemy && self.ball_ghost_frame.is_some() {
            return (96, 152);
        }
        let sliding = match side {
            MonSide::Player => {
                self.player_half_off
                    || self.player_exit.is_some_and(|anim| {
                        matches!(anim.kind, SlideKind::SeOff | SlideKind::SeHalfOff)
                    })
            }
            MonSide::Enemy => {
                self.enemy_half_off
                    || self.enemy_exit.is_some_and(|anim| {
                        matches!(anim.kind, SlideKind::SeOff | SlideKind::SeHalfOff)
                    })
            }
        };
        if sliding {
            match side {
                MonSide::Player => (8, 64),
                MonSide::Enemy => (96, 152),
            }
        } else {
            (0, 160)
        }
    }

    pub fn update(&mut self, screen: &BattleScreen) {
        self.tick_scheduled_ball_sfx();
        std::mem::swap(&mut self.anim_layer, &mut self.anim_layer_pending);
        std::mem::swap(
            &mut self.anim_layer_tileset,
            &mut self.anim_layer_pending_tileset,
        );
        self.anim_layer_pending
            .entries
            .clone_from(&self.anim_layer.entries);
        self.anim_layer_pending_tileset = self.anim_layer_tileset;
        self.fx.tick();
        self.mon_tilemap.tick();
        if let Some(squish) = self.squish_raster.as_mut() {
            if squish.frame == 24 {
                self.squish_raster = None;
            } else {
                squish.frame += 1;
            }
        }
        self.shake_back_and_forth.tick();
        self.rock_slide_shake.tick();
        self.blink_mon.tick();
        self.short_flash.tick();
        self.long_flash.tick();
        self.hide_mon_one_frame = None;
        if self.anim_player.is_finished() {
            self.transform_raster = None;
        }
        self.bg_palette.begin_frame();
        self.ball_obj_palette_frame_initial = self.ball_obj_palette_flipped;
        self.ball_obj_palette_write_scanline = None;
        if let Some(frame) = self.ball_hide_raster.as_mut() {
            if *frame >= 3 {
                self.ball_hide_raster = None;
                self.enemy_visible = false;
            } else {
                *frame += 1;
            }
        }
        if let Some(frame) = self.ball_ghost_frame.as_mut() {
            *frame = frame.saturating_add(1);
            let phase = *frame;
            let first = self.ball_ghost_transition_start;
            self.ball_enemy_top_offset_x = None;
            if phase < first {
                self.ball_enemy_offset_x = 0;
            } else if phase == first {
                self.ball_enemy_offset_x = 0;
                self.ball_enemy_top_offset_x = Some(8);
            } else if phase < first.saturating_add(3) {
                self.ball_enemy_offset_x = 8;
            } else if phase == first.saturating_add(3) {
                self.ball_enemy_offset_x = 8;
                self.ball_enemy_top_offset_x = Some(8);
            } else {
                self.ball_enemy_offset_x = 16;
            }
        }

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
                self.anim_layer_pending.clear();
            }
        }

        self.advance_ball_choreo();
        self.advance_move_animation();
        if self.advance_ball_choreo() {
            self.advance_move_animation();
        }

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
            let end_frame = if anim.kind == SlideKind::SeDown {
                35
            } else {
                Self::slide_frames(anim.kind, false)
            };
            if anim.frame >= end_frame {
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
            if anim.resetting && anim.frame >= 4 {
                self.move_mon_h = None;
            }
        }
        for (side, reveal) in self.show_reveal.iter_mut().enumerate() {
            if let Some(frame) = reveal.as_mut() {
                if *frame >= 3 {
                    *reveal = None;
                    if side == Self::side_index(MonSide::Enemy) {
                        self.ball_show_pattern = None;
                    }
                    if self.current_move == MoveId::Softboiled {
                        if side == Self::side_index(MonSide::Player) {
                            self.player_half_off = false;
                        } else {
                            self.enemy_half_off = false;
                        }
                    }
                } else {
                    *frame += 1;
                }
            }
        }
        for delay in &mut self.minimize_reveal_delay {
            *delay = delay.saturating_sub(1);
        }
        for delay in &mut self.substitute_reveal_delay {
            *delay = delay.saturating_sub(1);
        }
        if let Some(transfer) = self.seismic_hide_raster.as_mut() {
            if transfer.frame >= 2 {
                self.seismic_hide_raster = None;
            } else {
                transfer.frame += 1;
            }
        }
        if let Some(transfer) = self.transform_raster.as_mut() {
            transfer.frame = transfer.frame.saturating_add(1);
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
            } else {
                dy += (12i32 - entry.frame as i32).max(0) * 4;
            }
        }
        if let Some(exit) = self.player_exit {
            if exit.kind == SlideKind::Legacy {
                dy += exit.frame as i32 * 5;
            } else {
                let (ox, oy) = self.exit_slide_offset(exit.kind, exit.frame, true);
                dx += ox;
                dy += oy;
            }
        }
        if self.player_half_off {
            // SE_SLIDE_MON_HALF_OFF latch (Softboiled): stays 4 tiles off.
            if !matches!(
                self.show_reveal[Self::side_index(MonSide::Player)],
                Some(2 | 3)
            ) {
                dx -= 32;
            }
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
            if mh.side == MonSide::Player && mh.base_is_shifted() {
                // AnimationMoveMonHorizontally: hlcoord(2,5) vs (1,5).
                dx += 8;
            }
        }
        dx += self.fx.mon_dx(MonSide::Player);
        dx += self.shake_back_and_forth.dx(MonSide::Player);
        // AnimationBoundUpAndDown (Splash): vertical slide-down cycles.
        if !self.mon_tilemap.controls_side(MonSide::Player) {
            dy += self.fx.mon_dy(MonSide::Player);
        }

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
            } else {
                dx += (12i32 - entry.frame as i32).max(0) * 6;
            }
        }
        if let Some(exit) = self.enemy_exit {
            if exit.kind == SlideKind::Legacy {
                dx += exit.frame as i32 * 6;
            } else {
                let (ox, oy) = self.exit_slide_offset(exit.kind, exit.frame, false);
                dx += ox;
                dy += oy;
            }
        }
        if self.enemy_half_off {
            if !matches!(
                self.show_reveal[Self::side_index(MonSide::Enemy)],
                Some(2 | 3)
            ) {
                dx += 32;
            }
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
            if mh.side == MonSide::Enemy && mh.base_is_shifted() {
                // AnimationMoveMonHorizontally: hlcoord(11,0) vs (12,0).
                dx -= 8;
            }
        }
        dx += self.fx.mon_dx(MonSide::Enemy);
        dx += self.shake_back_and_forth.dx(MonSide::Enemy);
        dx += self.ball_enemy_offset_x;
        if !self.mon_tilemap.controls_side(MonSide::Enemy) {
            dy += self.fx.mon_dy(MonSide::Enemy);
        }
        // AnimationShakeEnemyHUD scrolls the BG (SCX); the enemy mon is part
        // of the BG in the original, so it shakes along with the HUD strip.
        dx += self.fx.enemy_hud_shake_offset();

        if let IntroAnimState::SilhouetteSlide { offset, .. } = self.intro_anim {
            dx += offset;
        }

        (dx, dy)
    }

    fn player_visible_now(&self) -> bool {
        let side = MonSide::Player;
        let reveal = self.show_reveal[Self::side_index(side)];
        let seismic_defender = self.current_move == MoveId::SeismicToss
            && side
                != if self.current_attacker_is_player {
                    MonSide::Player
                } else {
                    MonSide::Enemy
                };
        let reveal_hidden = (matches!(reveal, Some(1))
            && !matches!(self.current_move, MoveId::Softboiled | MoveId::Fly))
            || (matches!(reveal, Some(2))
                && !matches!(
                    self.current_move,
                    MoveId::Softboiled | MoveId::Submission | MoveId::Fly
                )
                && !seismic_defender);
        let seismic_transfer = self
            .seismic_hide_raster
            .is_some_and(|transfer| transfer.side == side);
        let transform_hidden = self.transform_raster.is_some_and(|transfer| {
            transfer.side == side && side == MonSide::Player && transfer.frame >= 4
        });
        self.player_visible
            && self.blink_mon.visible_band(side, 144).is_some()
            && !reveal_hidden
            && !transform_hidden
            && (self.mon_tilemap.keeps_side_visible(side)
                || seismic_transfer
                || !self.fx.mon_hidden(side))
    }

    fn enemy_visible_now(&self) -> bool {
        let side = MonSide::Enemy;
        let reveal = self.show_reveal[Self::side_index(side)];
        let bottom_first = self.ball_show_pattern == Some(BallShowPattern::BottomTwice);
        let seismic_defender = self.current_move == MoveId::SeismicToss
            && side
                != if self.current_attacker_is_player {
                    MonSide::Player
                } else {
                    MonSide::Enemy
                };
        let reveal_hidden = (matches!(reveal, Some(1))
            && !matches!(self.current_move, MoveId::Softboiled | MoveId::Fly))
            || (matches!(reveal, Some(2))
                && !bottom_first
                && !matches!(
                    self.current_move,
                    MoveId::Softboiled | MoveId::Submission | MoveId::Fly
                )
                && !seismic_defender);
        let seismic_transfer = self
            .seismic_hide_raster
            .is_some_and(|transfer| transfer.side == side);
        self.enemy_visible
            && self.blink_mon.visible_band(side, 144).is_some()
            && !reveal_hidden
            && self.ball_hide_raster != Some(3)
            && (self.mon_tilemap.keeps_side_visible(side)
                || seismic_transfer
                || !self.fx.mon_hidden(side))
    }

    fn vertical_mon_clip(&self, side: MonSide, screen_bottom: u32) -> (u32, u32) {
        let (mut top, mut bottom) = self
            .blink_mon
            .visible_band(side, screen_bottom)
            .unwrap_or((0, 0));
        if self.current_move == MoveId::Softboiled
            && matches!(self.show_reveal[Self::side_index(side)], Some(2 | 3))
        {
            return (top, bottom);
        }
        if self.current_move == MoveId::Fly && self.show_reveal[Self::side_index(side)].is_some() {
            return (top, bottom);
        }
        if self.current_move == MoveId::Submission
            && matches!(self.show_reveal[Self::side_index(side)], Some(2 | 3))
        {
            top = top.max(48);
            return (top, bottom);
        }
        if self.current_move == MoveId::SeismicToss {
            if self
                .seismic_hide_raster
                .is_some_and(|transfer| transfer.side == side && transfer.frame == 2)
            {
                top = top.max(48);
            }
            let defender = if self.current_attacker_is_player {
                MonSide::Enemy
            } else {
                MonSide::Player
            };
            if side == defender {
                match self.show_reveal[Self::side_index(side)] {
                    Some(2) => bottom = bottom.min(48),
                    Some(3) => return (top, bottom),
                    _ => {}
                }
            }
        }
        if self.transform_raster.is_some_and(|transfer| {
            transfer.side == side && side == MonSide::Player && transfer.frame == 3
        }) {
            top = top.max(48);
        }
        if side == MonSide::Enemy {
            match (self.ball_hide_top_only, self.ball_hide_raster) {
                (true, Some(1 | 2)) => bottom = bottom.min(48),
                (false, Some(2)) => top = top.max(48),
                _ => {}
            }
        }
        if self.ball_hide_raster == Some(3) && side == MonSide::Enemy {
            return (0, 0);
        }
        if side == MonSide::Enemy
            && self.ball_show_pattern == Some(BallShowPattern::BottomTwice)
            && matches!(self.show_reveal[Self::side_index(side)], Some(2 | 3))
        {
            top = top.max(48);
            return (top, bottom);
        }
        if self.show_reveal[Self::side_index(side)] == Some(3) {
            bottom = bottom.min(48);
        }
        let slide = match side {
            MonSide::Player => self.player_exit,
            MonSide::Enemy => self.enemy_exit,
        };
        if slide.is_some_and(|anim| anim.kind == SlideKind::SeDown) {
            bottom = bottom.min(match side {
                MonSide::Player => 96,
                MonSide::Enemy => 56,
            });
            if side == MonSide::Player && slide.is_some_and(|anim| anim.frame == 6) {
                top = top.max(48);
            }
        }
        (top, bottom)
    }

    /// `AnimationSlideMonDown` copies the enemy tilemap while the LCD is
    /// scanning it. On every third frame the middle band has advanced one
    /// tile row while the rows above and below still show the old map.
    fn slide_down_transition(&self, side: MonSide) -> Option<(u32, u32)> {
        if side != MonSide::Enemy {
            return None;
        }
        let anim = self.enemy_exit?;
        if anim.kind != SlideKind::SeDown || !(6..=21).contains(&anim.frame) {
            return None;
        }
        if (anim.frame - 6) % 3 != 0 {
            return None;
        }
        Some((3 + u32::from((anim.frame - 6) / 3) * 8, 48))
    }

    fn squish_visible(&self, side: MonSide) -> bool {
        self.squish_raster
            .is_some_and(|squish| squish.side == side && squish.frame <= 20)
    }

    fn draw_squish(
        &self,
        fb: &mut FrameBuffer,
        tileset: &TileSet,
        x: i32,
        y: i32,
        palette: &pokered_renderer::palette::Palette,
        side: MonSide,
    ) -> bool {
        let Some(squish) = self.squish_raster.filter(|squish| squish.side == side) else {
            return false;
        };
        render_gen1_squish(fb, tileset, x, y, palette, side, squish.frame);
        true
    }

    fn draw_slide_up(
        &self,
        fb: &mut FrameBuffer,
        tileset: &TileSet,
        x: i32,
        y: i32,
        tiles_per_row: u32,
        palette: &pokered_renderer::palette::Palette,
        side: MonSide,
    ) -> bool {
        let entry = match side {
            MonSide::Player => self.player_entry,
            MonSide::Enemy => self.enemy_entry,
        };
        let Some(entry) = entry.filter(|entry| entry.kind == SlideKind::SeUp) else {
            return false;
        };
        render_gen1_slide_up(
            fb,
            tileset,
            x,
            y,
            tiles_per_row,
            palette,
            entry.frame,
            self.current_move == MoveId::Waterfall,
        );
        true
    }

    fn player_slide_corner_blank(&self) -> bool {
        if self.current_move == MoveId::Softboiled
            && matches!(
                self.show_reveal[Self::side_index(MonSide::Player)],
                Some(2 | 3)
            )
        {
            return false;
        }
        if self.player_half_off {
            return true;
        }
        let Some(anim) = self.player_exit else {
            return false;
        };
        match anim.kind {
            SlideKind::SeOff if self.current_move == MoveId::Whirlwind => anim.frame >= 2,
            SlideKind::SeOff if self.current_move == MoveId::SeismicToss => anim.frame >= 3,
            SlideKind::SeOff => anim.frame >= 4,
            SlideKind::SeHalfOff => anim.frame >= 4,
            _ => false,
        }
    }

    fn apply_post_effects(&self, fb: &mut FrameBuffer) {
        self.fx.apply_screen_effects(fb);
        if !(self.anim_player.is_shake_restore_frame()
            && self.short_flash.is_active()
            && !self.short_flash.is_entry())
        {
            self.anim_player.apply_screen_effects(fb);
        }
        self.rock_slide_shake.apply(fb);
        self.apply_intro_effects(fb);
        if self.long_flash.is_active() {
            self.long_flash.apply_with_palette(&self.bg_palette, fb);
        } else {
            self.short_flash.apply_with_palette(&self.bg_palette, fb);
        }
        if self.current_move == MoveId::Softboiled
            && !self.current_attacker_is_player
            && self.short_flash.is_restoring()
        {
            // CopyTempPicToMonPic reaches the enemy picture's top tile row
            // during this scanout. Three transparent pixels still expose the
            // previous blank tile before the following frame is fully copied.
            for x in [138, 148, 150] {
                fb.set_pixel_index(x, 8, pokered_renderer::palette::GbColor::White);
            }
        }
    }

    fn apply_move_mon_h_raster_edge(&self, fb: &mut FrameBuffer) {
        let Some(anim) = self.move_mon_h else {
            return;
        };
        let Some(toward_opponent) = anim.top_row_transition() else {
            return;
        };

        let (normal_x, y, toward_dx, transition_height) = match anim.side {
            MonSide::Player => (8i32, 40u32, 8i32, 8u32),
            MonSide::Enemy => (96i32, 0u32, -8i32, 48u32),
        };
        let shifted_x = normal_x + toward_dx;
        let (source_x, dest_x) = if toward_opponent {
            (normal_x, shifted_x)
        } else {
            (shifted_x, normal_x)
        };

        let mut row = Vec::with_capacity(56 * transition_height as usize);
        for py in y..y + transition_height {
            for px in source_x..source_x + 56 {
                row.push(fb.get_pixel(px as u32, py).unwrap_or(Rgba::WHITE));
            }
        }

        let clear_x = normal_x.min(shifted_x) as u32;
        for py in y..y + transition_height {
            for px in clear_x..clear_x + 64 {
                fb.set_pixel(px, py, Rgba::WHITE);
            }
        }
        for py in 0..transition_height {
            for px in 0..56u32 {
                fb.set_pixel(
                    (dest_x + px as i32) as u32,
                    y + py,
                    row[(py * 56 + px) as usize],
                );
            }
        }
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
                    pixels[row][col] = src.get(tile_idx).pixels[sy % TILE_PIXELS][sx % TILE_PIXELS];
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
fn draw_safari_menu(
    buf: &mut ScreenTileBuffer,
    selected_row: usize,
    selected_col: usize,
    balls: u8,
) {
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
    language: pokered_core::game_state::Lang,
) {
    let is_zh = language == pokered_core::game_state::Lang::Zh;
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
    let enemy_name =
        if screen.is_ghost || (screen.ghost_marowak_reveal && !screen.ghost_marowak_unveiled) {
            if is_zh {
                "幽灵".to_string()
            } else {
                "GHOST".to_string()
            }
        } else if is_zh {
            pokered_data::lang_data::species_name(screen.enemy_species, true).to_string()
        } else {
            format!("{}", screen.enemy_species).to_uppercase()
        };
    // The catch tutorial shows the player as "OLD MAN" (Gen-1
    // BATTLE_TYPE_OLD_MAN), mirroring the native renderer.
    let player_name = if screen.is_old_man {
        if is_zh {
            "老头".to_string()
        } else {
            "OLD MAN".to_string()
        }
    } else if is_zh {
        pokered_data::lang_data::species_name(screen.player_species, true).to_string()
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
            // The tile HUD only renders charmap glyphs; CJK names are blanked
            // here and drawn with the pixel font after the tilemap blit.
            let enemy_name_tiles = if is_zh {
                Vec::new()
            } else {
                ascii_to_tiles(&enemy_name)
            };
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
            let player_name_tiles = if is_zh {
                Vec::new()
            } else {
                ascii_to_tiles(&player_name)
            };
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
        // Standard dialog box: full width, bottom 6 rows.
        // `zh_dialog`: a message page deferred to the pixel-font overlay —
        // the tile font has no CJK glyphs, so zh pages are drawn after the
        // tilemap blit (see the end of this function).
        let mut zh_dialog: Option<(String, bool)> = None;
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
        } else if matches!(
            screen.phase,
            BattlePhase::MoveSelect
                | BattlePhase::ItemMoveSelect { .. }
                | BattlePhase::LearnMoveChoose { .. }
        ) {
            draw_move_menu(&mut tile_buf, screen);
        } else if matches!(screen.phase, BattlePhase::BagSelect) {
            draw_bag_menu(&mut tile_buf, screen);
        } else if matches!(screen.phase, BattlePhase::ItemTargetSelect { .. }) {
            draw_party_menu(&mut tile_buf, screen);
        } else if matches!(
            screen.phase,
            BattlePhase::PartySelect
                | BattlePhase::ShiftSwitchSelect
                | BattlePhase::PlayerFaintSwitch
        ) {
            draw_party_menu(&mut tile_buf, screen);
        } else if matches!(screen.phase, BattlePhase::ShiftPrompt) {
            // "Will you change #MON?" — prompt text + YES/NO box (original
            // TWO_OPTION_MENU at hlcoord(0,7), cursor default NO).
            if let Some(ref text) = screen.current_message {
                if is_zh {
                    zh_dialog = Some((pokered_data::battle_text::localize(text, true), false));
                } else {
                    draw_battle_text(&mut tile_buf, text);
                }
            }
            let yn_box = TextBoxFrame::new(0, 7, 7, 5);
            yn_box.draw_frame(&mut tile_buf);
            if !is_zh {
                write_tiles_at(&mut tile_buf, 2, 9, &ascii_to_tiles("YES"));
                write_tiles_at(&mut tile_buf, 2, 11, &ascii_to_tiles("NO"));
            }
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
                    IntroPhase::GhostCantID => Some("Darn! The GHOST\ncan't be ID'd!".to_string()),
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
            // Down-arrow while waiting for user input / the next page —
            // shared by the tile path and the zh pixel-font overlay below.
            let dialog_arrow = match &screen.phase {
                BattlePhase::Intro {
                    phase: ref intro_p,
                    wait_frames,
                } => {
                    matches!(
                        intro_p,
                        IntroPhase::WildReveal
                            | IntroPhase::GhostCantID
                            | IntroPhase::GhostUnveil
                            | IntroPhase::TrainerReveal
                            | IntroPhase::TrainerSendOut
                            | IntroPhase::PlayerSendOut
                    ) && *wait_frames == 0
                }
                BattlePhase::ShowingText {
                    messages,
                    current,
                    wait_frames,
                    ..
                } => *current + 1 < messages.len() && *wait_frames == 0,
                _ => false,
            };
            if dialog_arrow {
                dialog_box.show_down_arrow(&mut tile_buf);
            }
            if let Some(ref text) = phase_text {
                if is_zh {
                    zh_dialog = Some((
                        pokered_data::battle_text::localize(text, true),
                        dialog_arrow,
                    ));
                } else {
                    draw_battle_text(&mut tile_buf, text);
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
        if effects.enemy_visible_now()
            || effects.hide_mon_one_frame == Some(MonSide::Enemy)
            || effects.squish_visible(MonSide::Enemy)
        {
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
                if effects.substitute_reveal_delay[BattleVisualEffects::side_index(MonSide::Enemy)]
                    == 0
                {
                    if let Ok(cached) = rm.load_sprite("monster") {
                        let doll = cached.tileset.clone();
                        let rect = MonRect {
                            x: 12 * TILE_SIZE as i32 + enemy_dx,
                            y: enemy_dy,
                        };
                        BattleEffects::draw_substitute(fb, rect, &doll, pal, MonSide::Enemy);
                    }
                }
            } else if effects.fx.is_minimized(MonSide::Enemy)
                && effects.minimize_reveal_delay[BattleVisualEffects::side_index(MonSide::Enemy)]
                    == 0
            {
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
                let ex = (12 * TILE_SIZE + x_off) as i32 + enemy_dx;
                let ey = y_off as i32 + enemy_dy;
                if effects.draw_slide_up(fb, &ts, ex, ey, w_tiles, pal, MonSide::Enemy) {
                } else if effects
                    .mon_tilemap
                    .draw(fb, &ts, ex, ey, w_tiles, pal, MonSide::Enemy)
                {
                } else if let Some((rows, yoff)) = effects.fx.slide_down_hide_params(MonSide::Enemy)
                {
                    // AnimationSlideMonDownAndHide (Acid Armor): crop to the
                    // top rows (7×5 then 7×3 tile-id lists), drawn lower.
                    BattleEffects::draw_mon_rows(fb, &ts, ex, ey + yoff, w_tiles, pal, rows);
                } else if effects.draw_squish(fb, &ts, ex, ey, pal, MonSide::Enemy) {
                } else if let Some((width, anchor_right)) = effects.fx.squish_params(MonSide::Enemy)
                {
                    // AnimationSquishMonPic: narrow the pic one tile per pass.
                    BattleEffects::draw_squished(
                        fb,
                        &ts,
                        ex,
                        ey,
                        w_tiles,
                        pal,
                        width,
                        anchor_right,
                    );
                } else if let Some((transition_top, transition_bottom)) =
                    effects.slide_down_transition(MonSide::Enemy)
                {
                    let (visible_top, visible_bottom) =
                        effects.vertical_mon_clip(MonSide::Enemy, fb.height());
                    for (draw_y, top, bottom) in [
                        (ey, visible_top, transition_top),
                        (ey + 8, transition_top, transition_bottom),
                        (ey, transition_bottom, visible_bottom),
                    ] {
                        if top < bottom {
                            draw_mon_pic_clipped(
                                fb,
                                &ts,
                                ex,
                                draw_y,
                                w_tiles,
                                pal,
                                false,
                                0,
                                fb.width() as i32,
                                top,
                                bottom,
                            );
                        }
                    }
                } else {
                    let (clip_left, clip_right) = effects.horizontal_slide_clip(MonSide::Enemy);
                    let (visible_top, visible_bottom) =
                        effects.vertical_mon_clip(MonSide::Enemy, fb.height());
                    if let Some(top_dx) = effects.horizontal_slide_top_dx(MonSide::Enemy) {
                        let top_split = effects.horizontal_slide_top_split(MonSide::Enemy);
                        draw_mon_pic_clipped(
                            fb,
                            &ts,
                            ex,
                            ey,
                            w_tiles,
                            pal,
                            false,
                            clip_left,
                            clip_right,
                            top_split.max(visible_top),
                            visible_bottom,
                        );
                        draw_mon_pic_clipped(
                            fb,
                            &ts,
                            ex + top_dx,
                            ey,
                            w_tiles,
                            pal,
                            false,
                            clip_left,
                            clip_right,
                            visible_top,
                            top_split.min(visible_bottom),
                        );
                    } else {
                        draw_mon_pic_clipped(
                            fb,
                            &ts,
                            ex,
                            ey,
                            w_tiles,
                            pal,
                            false,
                            clip_left,
                            clip_right,
                            visible_top,
                            visible_bottom,
                        );
                    }
                    if let Some((top, bottom)) = effects
                        .shake_back_and_forth
                        .stale_normal_right_band(MonSide::Enemy, visible_bottom)
                    {
                        let normal_x = ex - effects.shake_back_and_forth.dx(MonSide::Enemy);
                        draw_mon_pic_clipped(
                            fb,
                            &ts,
                            normal_x,
                            ey,
                            w_tiles,
                            pal,
                            false,
                            normal_x + 48,
                            normal_x + 56,
                            top.max(visible_top),
                            bottom.min(visible_bottom),
                        );
                    }
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
        if effects.player_visible_now()
            || effects.hide_mon_one_frame == Some(MonSide::Player)
            || effects.squish_visible(MonSide::Player)
        {
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
                if effects.substitute_reveal_delay[BattleVisualEffects::side_index(MonSide::Player)]
                    == 0
                {
                    if let Ok(cached) = rm.load_sprite("monster") {
                        let doll = cached.tileset.clone();
                        let rect = MonRect {
                            x: TILE_SIZE as i32 + player_dx,
                            y: 5 * TILE_SIZE as i32 + player_dy,
                        };
                        BattleEffects::draw_substitute(fb, rect, &doll, pal, MonSide::Player);
                    }
                }
            } else if effects.fx.is_minimized(MonSide::Player)
                && effects.minimize_reveal_delay[BattleVisualEffects::side_index(MonSide::Player)]
                    == 0
            {
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
                    let px = TILE_SIZE as i32 + player_dx;
                    let py = (5 * TILE_SIZE) as i32 + player_dy;
                    if effects.draw_slide_up(fb, &scaled, px, py, 7, pal, MonSide::Player) {
                    } else if effects
                        .mon_tilemap
                        .draw(fb, &scaled, px, py, 7, pal, MonSide::Player)
                    {
                    } else if let Some((rows, yoff)) =
                        effects.fx.slide_down_hide_params(MonSide::Player)
                    {
                        BattleEffects::draw_mon_rows(fb, &scaled, px, py + yoff, 7, pal, rows);
                    } else if effects.draw_squish(fb, &scaled, px, py, pal, MonSide::Player) {
                    } else if let Some((width, anchor_right)) =
                        effects.fx.squish_params(MonSide::Player)
                    {
                        BattleEffects::draw_squished(
                            fb,
                            &scaled,
                            px,
                            py,
                            7,
                            pal,
                            width,
                            anchor_right,
                        );
                    } else {
                        let (clip_left, clip_right) =
                            effects.horizontal_slide_clip(MonSide::Player);
                        let corner_blank = effects.player_slide_corner_blank();
                        let (visible_top, visible_bottom) =
                            effects.vertical_mon_clip(MonSide::Player, fb.height());
                        if let Some(top_dx) = effects.horizontal_slide_top_dx(MonSide::Player) {
                            let top_split = effects.horizontal_slide_top_split(MonSide::Player);
                            draw_mon_pic_clipped(
                                fb,
                                &scaled,
                                px,
                                py,
                                7,
                                pal,
                                corner_blank,
                                clip_left,
                                clip_right,
                                top_split.max(visible_top),
                                visible_bottom,
                            );
                            draw_mon_pic_clipped(
                                fb,
                                &scaled,
                                px + top_dx,
                                py,
                                7,
                                pal,
                                true,
                                clip_left,
                                clip_right,
                                visible_top,
                                top_split.min(visible_bottom),
                            );
                        } else {
                            draw_mon_pic_clipped(
                                fb,
                                &scaled,
                                px,
                                py,
                                7,
                                pal,
                                corner_blank,
                                clip_left,
                                clip_right,
                                visible_top,
                                visible_bottom,
                            );
                        }
                        if let Some((top, bottom)) = effects
                            .shake_back_and_forth
                            .stale_normal_right_band(MonSide::Player, visible_bottom)
                        {
                            let normal_x = px - effects.shake_back_and_forth.dx(MonSide::Player);
                            draw_mon_pic_clipped(
                                fb,
                                &scaled,
                                normal_x,
                                py,
                                7,
                                pal,
                                false,
                                normal_x + 48,
                                normal_x + 56,
                                top.max(visible_top),
                                bottom.min(visible_bottom),
                            );
                        }
                    }
                }
            }
        }

        // Reconstruct the move-select TYPE/PP window before the hardware OAM
        // layer is composited below.
        if matches!(
            screen.phase,
            BattlePhase::MoveSelect
                | BattlePhase::ItemMoveSelect { .. }
                | BattlePhase::LearnMoveChoose { .. }
        ) {
            tile_buf.render_region(fb, &battle_ts, pal, 0, 8, 11, 5);
        }

        // Reconstruct the bottom battle window before hardware OAM composition.
        tile_buf.render_region(fb, &battle_ts, pal, 0, 12, 20, 6);

        // Chinese overlays: the tile font has no CJK glyphs, so zh HUD names
        // and message pages are drawn with the embedded pixel font after the
        // tilemap/sprite blits (mirrors the app's zh HUD overlay).
        if is_zh {
            let text_color = Rgba::new(0, 0, 0, 255);
            // Left-aligned at the HUD name origin — centering pushes 3+ char
            // names right onto the "Lv" column below (CJK glyphs are 10px tall
            // and their lower edge grazes the level row).
            if !hide_enemy_hud {
                draw_text(&enemy_name, EnemyHud::NAME_X * TILE_SIZE, 0, text_color, fb);
            }
            if !hide_player_hud {
                draw_text(
                    &player_name,
                    PlayerHud::NAME_X * TILE_SIZE,
                    PlayerHud::NAME_Y * TILE_SIZE,
                    text_color,
                    fb,
                );
            }
            if matches!(screen.phase, BattlePhase::ShiftPrompt) {
                // 是/否 replaces the YES/NO tiles (same box, hlcoord(2,9)/(2,11)).
                draw_text("是", 2 * TILE_SIZE, 9 * TILE_SIZE, text_color, fb);
                draw_text("否", 2 * TILE_SIZE, 11 * TILE_SIZE, text_color, fb);
            }
        }
        if is_zh
            && matches!(
                screen.phase,
                BattlePhase::MoveSelect | BattlePhase::ItemMoveSelect { .. }
            )
        {
            if let Some(mm) = &screen.move_menu {
                let mut painter = pokered_ui::backends::framebuffer::FrameBufferPainter::new(fb)
                    .with_lang(language);
                let mut ui = pokered_ui::Ui::new(&mut painter);
                pokered_ui::menus::battle_move::draw(
                    mm,
                    &pokered_data::ui_layout::schema::BATTLE_MOVE_DEFAULT_LAYOUT,
                    &mut ui,
                    language,
                    &pokered_data::impl_traits::PokemonRenderData::new(true),
                );
            }
        }
        if let Some((text, arrow)) = zh_dialog {
            let mut painter =
                pokered_ui::backends::framebuffer::FrameBufferPainter::new(fb).with_lang(language);
            let mut ui = pokered_ui::Ui::new(&mut painter);
            pokered_ui::menus::battle_text::draw(
                &text,
                arrow,
                &BATTLE_TEXT_DEFAULT_LAYOUT,
                &mut ui,
                language,
            );
        }

        effects.apply_move_mon_h_raster_edge(fb);
        effects.apply_post_effects(fb);

        // Hardware OAM is composited after the window/background.
        if effects.fx.objects_active() {
            let ts0 = rm
                .load_battle("move_anim_0")
                .map(|c| c.tileset.clone())
                .ok();
            let ts1 = rm
                .load_battle("move_anim_1")
                .map(|c| c.tileset.clone())
                .ok();
            if let (Some(ts0), Some(ts1)) = (ts0, ts1) {
                effects.fx.render_objects(fb, &ts0, &ts1, pal);
            }
        }
        if !effects.anim_layer.entries.is_empty() {
            let anim_tileset_name = match effects.anim_layer_tileset {
                1 => "move_anim_1",
                2 => "move_anim_0",
                _ => "move_anim_0",
            };
            if let Ok(cached) = rm.load_battle(anim_tileset_name) {
                if effects.ball_obj_palette_flipped
                    || effects.ball_obj_palette_frame_initial
                    || effects.ball_obj_palette_write_scanline.is_some()
                {
                    let mut flipped = *pal;
                    flipped.colors.swap(1, 2);
                    let before = if effects.ball_obj_palette_frame_initial {
                        &flipped
                    } else {
                        pal
                    };
                    let after = if effects.ball_obj_palette_flipped {
                        &flipped
                    } else {
                        pal
                    };
                    render_gen1_oam_palette_split(
                        fb,
                        &effects.anim_layer.entries,
                        &cached.tileset,
                        before,
                        after,
                        effects.ball_obj_palette_write_scanline,
                    );
                } else {
                    render_gen1_oam(fb, &effects.anim_layer.entries, &cached.tileset, pal);
                }
            }
        }
    } else {
        // No resources — fallback: render tile buffer with blank tileset
        let blank_ts = TileSet::blank(256);
        tile_buf.render(fb, &blank_ts, pal);
    }
}

#[cfg(test)]
mod zh_render_tests {
    use super::*;
    use pokered_core::game_state::Lang;
    use pokered_renderer::resource::{AssetRoot, ResourceManager};

    /// Offscreen zh-HUD regression: with `is_zh` the tile HUD name rows are
    /// blanked, so dark pixels in those rows can only come from the
    /// pixel-font overlay. Renders the same frame in zh and en, asserts both
    /// carry name ink in the HUD rows, and (with
    /// `POKERED_TUI_DEBUG_SHOTS=1`) drops both frames into `target/` as
    /// PNGs for eyeballing.
    #[test]
    fn zh_battle_hud_overlays_cjk_names() {
        let root = match AssetRoot::auto_detect() {
            Ok(root) => root,
            Err(e) => panic!("gfx assets unavailable for render test: {e}"),
        };
        let mut res = Some(ResourceManager::new(root));

        let mut screen = BattleScreen::new(true);
        // Stable phase: both HUDs visible plus the FIGHT/PKMN menu box.
        screen.phase = BattlePhase::PlayerMenu;

        let mut render = |lang: Lang| {
            let mut fb = FrameBuffer::new(
                dotzuki_engine::render_config::RenderConfig::new(160, 144),
                Rgba::BLACK,
            );
            let mut effects = BattleVisualEffects::default();
            draw_battle(&screen, &mut res, &mut fb, &mut effects, lang);
            fb
        };
        let fb_zh = render(Lang::Zh);
        let fb_en = render(Lang::En);

        let band_has_ink = |fb: &FrameBuffer, x0: u32, x1: u32, y0: u32, y1: u32| {
            (y0..y1).any(|y| (x0..x1).any(|x| fb.get_pixel(x, y).map_or(false, |p| p.r < 128)))
        };
        // Enemy name row (EnemyHud NAME at tile (1,0), left of the front
        // sprite at tile 12): the zh overlay centers the CJK name there.
        assert!(
            band_has_ink(&fb_zh, 8, 88, 0, 8),
            "zh enemy HUD name row must be drawn by the pixel-font overlay"
        );
        assert!(
            band_has_ink(&fb_en, 8, 88, 0, 8),
            "en enemy HUD name row must show the tiled species name"
        );
        // The two languages must produce visibly different frames
        // (CJK overlay vs tiled ASCII names + ASCII message pages).
        let differs = (0..160 * 144usize).any(|i| {
            let (x, y) = ((i % 160) as u32, (i / 160) as u32);
            fb_zh.get_pixel(x, y) != fb_en.get_pixel(x, y)
        });
        assert!(differs, "zh and en battle frames must differ");

        if std::env::var("POKERED_TUI_DEBUG_SHOTS").as_deref() == Ok("1") {
            let out = |name: &str| {
                std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/"))
                    .join(name)
            };
            let _ = fb_zh.save_png(&out("tui-battle-zh.png"));
            let _ = fb_en.save_png(&out("tui-battle-en.png"));
        }
    }
}
