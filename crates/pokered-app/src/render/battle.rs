use pokered_audio::sfx_data::SfxId;
use pokered_core::battle::state::StatusCondition as CoreStatus;
use pokered_core::battle::state::{status2, status3};
use pokered_core::battle::{
    BallAnimOutcome, BattleAnimEvent, BattlePhase, BattleScreen,
    BattleTransition as CoreTransition, IntroPhase,
};
use pokered_core::game_state::Lang;
use pokered_data::impl_traits::PokemonRenderData;
use pokered_data::items::ItemId;
use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_data::ui_layout::schema::{
    BATTLE_BAG_DEFAULT_LAYOUT, BATTLE_MAIN_DEFAULT_LAYOUT, BATTLE_MOVE_DEFAULT_LAYOUT,
    BATTLE_PARTY_DEFAULT_LAYOUT, BATTLE_TEXT_DEFAULT_LAYOUT, YES_NO_DEFAULT_LAYOUT,
};
use pokered_renderer::battle_anim::{
    AnimEffect, AnimationType, BattleEffects, MonRect, MonSide, ANIM_BASE_TILE_ID,
};
use pokered_renderer::battle_scene::{
    BallIndicators, BallStatus, EnemyHud, PlayerHud, StatusCondition,
};
use pokered_renderer::battle_transition::{BattleTransitionKind, BattleTransitionState};
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::gen1_battle_anim::{
    draw_mon_pic_clipped, move_short_flash_timing, render_gen1_oam, render_gen1_slide_up,
    render_gen1_squish, AnimTickResult, AnimationPlayer, BgPaletteState, BlinkMon, LongFlashTiming,
    LongScreenFlash, MonTilemapAnimation, RockSlideShake, ShakeBackAndForth, ShortFlashTiming,
    ShortScreenFlash,
};
use pokered_renderer::palette::{GRAYSCALE_PALETTE, GRAYSCALE_SPRITE_PALETTE};
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::sprite::SpriteLayer;
use pokered_renderer::text_renderer::{write_tiles_at, ScreenTileBuffer};
use pokered_renderer::textbox::TextBoxFrame;
use pokered_renderer::tile::{Tile, TileSet, TILE_PIXELS};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, Ui};

use super::{blit_tileset, species_to_sprite_name};

// FlashScreen palette sequence from engine/battle/battle_transitions.asm.
// Each entry maps shade N to brightness[entry[N]]. The sequence goes:
// darken → black → flash white → back to normal. In the original the whole
// 12-step sequence repeats 3 times (`ld b, $3` in BattleTransition_FlashScreen),
// strobing the OVERWORLD screen before the Circle/DoubleCircle wipe begins.
const FLASH_SCREEN_PALETTE: [[u8; 4]; 12] = [
    [3, 3, 2, 1], // Step 0:  dc 3,3,2,1  (darken)
    [3, 3, 3, 2], // Step 1:  dc 3,3,3,2  (darker)
    [3, 3, 3, 3], // Step 2:  dc 3,3,3,3  (all black)
    [3, 3, 3, 2], // Step 3:  dc 3,3,3,2  (slightly lighter)
    [3, 3, 2, 1], // Step 4:  dc 3,3,2,1  (darken) ← was missing
    [3, 2, 1, 0], // Step 5:  dc 3,2,1,0  (normal-ish)
    [2, 1, 0, 0], // Step 6:  dc 2,1,0,0  (bright)
    [1, 0, 0, 0], // Step 7:  dc 1,0,0,0  (brighter)
    [0, 0, 0, 0], // Step 8:  dc 0,0,0,0  (all white / flash peak)
    [1, 0, 0, 0], // Step 9:  dc 1,0,0,0  (brighter fading)
    [2, 1, 0, 0], // Step 10: dc 2,1,0,0  (bright fading)
    [3, 2, 1, 0], // Step 11: dc 3,2,1,0  (normal) ← was [3,3,3,3]
];

/// The party-ball strip appears for the last 25 frames of WildReveal.  Text
/// begins three frames later and advances in the original's three-glyph /
/// three-frame transfer cadence.
const WILD_REVEAL_BALL_FRAMES: u16 = 25;
const WILD_REVEAL_TEXT_START_WAIT: u16 = 21;

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
    /// Use the shifted tilemap as the base during reset. On its last frame the
    /// top tile row has already returned; `apply_move_mon_h_raster_edge`
    /// corrects that row after the mon is rendered.
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

    /// `Some(true)` means the first tile row has moved toward the opponent;
    /// `Some(false)` means that row has moved back to the normal position.
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

/// Slide animation kinds. `Legacy` covers battle-flow slides (switch,
/// trainer send-out); `Faint` is the `SlideDownFaintedMonPic` port used for
/// BOTH sides (engine/battle/core.asm:1181-1225); the `Se*` kinds are the
/// faithful `_AnimationSlideMonOff` / `AnimationSlideMonDown` /
/// `_AnimationSlideMonUp` ports (one tile per `wSlideMonDelay` frames).
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

/// Non-move animation ids (data/moves/animations.asm) as 0-based
/// MOVE_ANIM_DATA indices (the 1-based animation id − 1).
mod non_move_anim {
    /// SHOWPIC_ANIM ($A6): SE_SHOW_ENEMY_MON_PIC — mon reappears.
    pub const SHOWPIC: usize = 0xA5;
    /// STATUS_AFFECTED_ANIM ($A7): the original flashes the whose-turn mon
    /// pic (`AnimationFlashMonPic`, engine/battle/animations.asm:1378-1387).
    /// Approximation: anim index 0xA7 plays SE_FLASH_MON_PIC ($F5), whose
    /// handler is a blink of the whose-turn mon. Note the original's
    /// "flash" is itself just a pic REDRAW (no visible blink for a normal
    /// mon), so the blink is only a stand-in. The charge-turn trigger fires
    /// via `is_charge_message` below; SE $DD (anim 0xA6, ShowMonPic) is the
    /// old no-op mapping kept for reference.
    pub const STATUS_AFFECTED: usize = 0xA6;
    /// XSTATITEM_ANIM ($AE): light palette + spiral balls + reset palette.
    pub const XSTATITEM: usize = 0xAD;
    /// XSTATITEM_DUPLICATE_ANIM ($AF): same, played for the enemy side
    /// (trainer-AI X items, `ldh a, [hWhoseTurn]; add XSTATITEM_ANIM`).
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
/// [`BallAnimEvent`]). Drives `anim_player` through each [`BallStep`].
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
enum IntroAnimState {
    None,
    /// Screen wipe transition (8 types selected by 3-bit flags)
    BattleTransition {
        step: u8,
    },
    /// FlashScreen with 12 palette steps × 2 frames each.
    /// Matches BattleTransition_FlashScreenPalettes from engine/battle/battle_transitions.asm:
    /// each step applies a DMG palette that shifts all 4 shades toward darker/lighter values.
    /// step index 0-11 maps to the 12 palette entries, cycling from normal → black → flash white → normal.
    ScreenFlash {
        step: u8,
        step_frames: u8,
    },
    SilhouetteSlide {
        remaining: u8,
        offset: i32,
    },
    /// Player send-out (AnimateSendingOutMon + SendOutMon,
    /// engine/battle/core.asm:6801 / :1723): POOF_ANIM plays (stage 0),
    /// then the ball tile sits at hlcoord(4,11) (stage 1, Delay3), then the
    /// mon pic grows 3×3 at (3,9) (stage 2, DelayFrames 4) → 5×5 at (2,7)
    /// (stage 3, DelayFrames 5) → full 7×7 + cry.
    PlayerSendOut {
        stage: u8,
        frames: u8,
    },
    /// Ghost Marowak reveal: flash ghost → fade out → fade in Marowak
    GhostMarowakReveal {
        phase: u8,
        counter: u8,
    },
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
    TrainerVictory,
    BattleOver,
    /// Link battle: local action sent, waiting for the remote action
    /// (no input accepted; field renders as-is).
    LinkWaiting,
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
    /// Select the native tile-font oracle scene used only by the isolated
    /// move-animation differential recorder.
    move_animation_capture_scene: bool,
    intro_anim: IntroAnimState,
    is_wild_intro: bool,
    cry_pending: Option<pokered_data::species::Species>,
    transition_state: Option<BattleTransitionState>,
    ghost_marowak_palette: u8,
    /// Set when the GhostMarowakReveal anim completes (or is A-skipped): the
    /// enemy sprite switches from ghost.png to the Marowak front sprite.
    ghost_marowak_revealed: bool,
    /// SFX_SILPH_SCOPE request, fired as the reveal completes (consumed by the
    /// frontend, which owns the audio device).
    silph_scope_sfx_pending: bool,
    /// SFX_SILPH_SCOPE request for the trainer-appear sound
    /// (`PrintBeginningBattleText`'s `.trainerBattle` → `.playSFX`,
    /// engine/battle/common_text.asm:21+62 — the `wTempoModifier = $80`
    /// write is dead for non-cries, so it is the plain SFX).
    trainer_appear_sfx_pending: bool,
    /// Active ball-throw choreography (capture / ghost dodge / old man).
    ball_choreo: Option<BallChoreo>,
    /// Ball-flow SFX (BallToss / Tink per shake / BallPoof) queued for the
    /// frontend, which owns the audio device.
    pending_ball_sfx: std::collections::VecDeque<SfxId>,
    pub overworld_snapshot: Option<FrameBuffer>,
    pub victory_music_played: bool,
}

/// Machine-readable state emitted beside isolated move-animation frames.
/// Coordinates use the renderer's screen-space convention (not raw OAM's
/// +8/+16 hardware offsets).
#[derive(Debug, serde::Serialize)]
pub(crate) struct MoveAnimationCaptureState {
    pub animation_finished: bool,
    pub command_wait: u8,
    pub tileset: u8,
    pub player_visible: bool,
    pub enemy_visible: bool,
    pub player_offset: (i32, i32),
    pub enemy_offset: (i32, i32),
    pub move_mon_h_frame: Option<u8>,
    pub move_mon_h_resetting: bool,
    pub objects_active: bool,
    /// Screen-space objects emitted by AnimationPlayer before the frontend
    /// copies them into its render layer.
    pub source_oam: Vec<MoveAnimationCaptureOam>,
    /// Objects that the production frontend actually renders this frame.
    pub oam: Vec<MoveAnimationCaptureOam>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct MoveAnimationCaptureOam {
    pub x: i32,
    pub y: i32,
    pub tile: u8,
    pub attributes: u8,
}

impl BattleVisualEffects {
    pub fn has_transition(&self) -> bool {
        self.transition_state.is_some()
    }

    pub fn render_transition(&self, source: &FrameBuffer, dest: &mut FrameBuffer) -> bool {
        if let Some(ref ts) = self.transition_state {
            ts.render(source, dest)
        } else {
            false
        }
    }

    pub fn clear_snapshot(&mut self) {
        self.overworld_snapshot = None;
    }

    /// Mark the static scene used by the differential recorder as already
    /// observed, so its synthetic attack text is not mistaken for a live
    /// battle event on the first sampled frame.
    pub(crate) fn prime_move_animation_capture_scene(&mut self, screen: &BattleScreen) {
        self.last_phase_kind = Some(Self::phase_kind(&screen.phase));
        self.last_message = screen.current_message.clone();
        self.move_animation_capture_scene = true;
    }

    /// Start a move's visual command stream without executing battle logic.
    /// Used only by the frame-differential CLI so misses, charge turns, RNG,
    /// and move effects cannot change which animation is sampled.
    pub(crate) fn start_move_animation_capture(
        &mut self,
        move_id: MoveId,
        player_is_attacker: bool,
    ) {
        debug_assert!(move_id != MoveId::None);
        self.current_attacker_is_player = player_is_attacker;
        self.current_move = move_id;
        self.attack_lunge = None;
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
        self.current_attacker_species = pokered_data::species::Species::Rhydon;
        self.pending_applying = None;
        self.pending_anim_start = None;
        self.suppress_hit_flash = false;
        self.anim_player
            .start(move_id as usize - 1, player_is_attacker);
        self.anim_wait = 0;
        self.anim_layer.clear();
        self.anim_layer_pending.clear();
    }

    pub(crate) fn move_animation_capture_finished(&self) -> bool {
        self.anim_player.is_finished() && self.anim_wait == 0
    }

    pub(crate) fn move_animation_capture_state(&self) -> MoveAnimationCaptureState {
        MoveAnimationCaptureState {
            animation_finished: self.anim_player.is_finished(),
            command_wait: self.anim_wait,
            tileset: self.anim_tileset,
            player_visible: self.player_visible_now(),
            enemy_visible: self.enemy_visible_now(),
            player_offset: self.player_offset(),
            enemy_offset: self.enemy_offset(),
            move_mon_h_frame: self.move_mon_h.map(|state| state.frame),
            move_mon_h_resetting: self.move_mon_h.is_some_and(|state| state.resetting),
            objects_active: self.fx.objects_active(),
            source_oam: self
                .anim_player
                .oam_entries()
                .iter()
                .map(|entry| MoveAnimationCaptureOam {
                    x: entry.x,
                    y: entry.y,
                    tile: entry.tile_id.wrapping_sub(ANIM_BASE_TILE_ID),
                    attributes: entry.attributes,
                })
                .collect(),
            oam: self
                .anim_layer_pending
                .entries
                .iter()
                .map(|entry| MoveAnimationCaptureOam {
                    x: entry.x,
                    y: entry.y,
                    // anim_layer stores a tileset-relative id after
                    // advance_move_animation subtracts ANIM_BASE_TILE_ID.
                    tile: entry.tile_id,
                    attributes: entry.attributes,
                })
                .collect(),
        }
    }
}

impl BattleVisualEffects {
    pub fn take_cry_pending(&mut self) -> Option<pokered_data::species::Species> {
        self.cry_pending.take()
    }

    /// Take the pending SFX_SILPH_SCOPE request (ghost-Marowak reveal done).
    pub fn take_silph_scope_sfx_pending(&mut self) -> bool {
        std::mem::take(&mut self.silph_scope_sfx_pending)
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

    /// Take the pending animation-command sound request, if any.
    pub fn take_move_sfx(&mut self) -> Option<AnimSfxRequest> {
        self.pending_move_sfx.take()
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
            move_animation_capture_scene: false,
            intro_anim: IntroAnimState::None,
            is_wild_intro: false,
            cry_pending: None,
            transition_state: None,
            ghost_marowak_palette: 0xe4,
            ghost_marowak_revealed: false,
            silph_scope_sfx_pending: false,
            trainer_appear_sfx_pending: false,
            ball_choreo: None,
            pending_ball_sfx: std::collections::VecDeque::new(),
            overworld_snapshot: None,
            victory_music_played: false,
        }
    }
}

impl BattleVisualEffects {
    fn phase_kind(phase: &BattlePhase) -> BattlePhaseKind {
        match phase {
            BattlePhase::Intro { .. } => BattlePhaseKind::Intro,
            BattlePhase::PlayerMenu => BattlePhaseKind::PlayerMenu,
            BattlePhase::MoveSelect => BattlePhaseKind::MoveSelect,
            // Ether's per-move pick reuses the FIGHT move-menu layout.
            BattlePhase::ItemMoveSelect { .. } => BattlePhaseKind::MoveSelect,
            BattlePhase::BagSelect => BattlePhaseKind::BagSelect,
            BattlePhase::ItemTargetSelect { .. } => BattlePhaseKind::ItemTargetSelect,
            BattlePhase::ShowingText { .. } => BattlePhaseKind::ShowingText,
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
            BattlePhase::TrainerVictory { .. } => BattlePhaseKind::TrainerVictory,
            BattlePhase::BattleOver { .. } => BattlePhaseKind::BattleOver,
            BattlePhase::LinkWaiting => BattlePhaseKind::LinkWaiting,
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
        is_ghost: bool,
        ghost_marowak_reveal: bool,
    ) {
        match intro_phase {
            IntroPhase::BattleTransitionWipe(transition) => {
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
                // FlashScreen strobe: 12 palette steps × 2 frames, repeated
                // 3 times (`ld b, $3` in BattleTransition_FlashScreen) =
                // 72 frames, matching TRANSITION_FLASH_FRAMES in the core.
                self.intro_anim = IntroAnimState::ScreenFlash {
                    step: 0,
                    step_frames: 2,
                };
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
                // Finalize a skipped ghost-Marowak reveal (the player A-mashed
                // through the GhostUnveil phase): the Marowak stands revealed
                // and the scope SFX + cry fire, as if the anim had completed.
                if matches!(self.intro_anim, IntroAnimState::GhostMarowakReveal { .. }) {
                    self.ghost_marowak_revealed = true;
                    self.ghost_marowak_palette = 0xe4;
                    self.silph_scope_sfx_pending = true;
                    self.cry_pending = Some(enemy_species);
                }
                self.intro_anim = IntroAnimState::None;
                self.player_visible = true;
                self.enemy_visible = true;
                self.player_entry = None;
                self.enemy_entry = None;
                // A GHOST never cries (PrintBeginningBattleText's .pokemonTower
                // path skips PlayCry); the ghost-Marowak reveal cries Marowak
                // only when the reveal completes (above / in the anim tick).
                if !is_ghost && !ghost_marowak_reveal {
                    self.cry_pending = Some(enemy_species);
                }
            }
            IntroPhase::GhostCantID => {
                // No-scope ghost battle: the GHOST stays unidentified — static
                // text phase, no cry, no animation.
                self.intro_anim = IntroAnimState::None;
                self.player_visible = true;
                self.enemy_visible = true;
                self.player_entry = None;
                self.enemy_entry = None;
            }
            IntroPhase::GhostUnveil => {
                // SILPH SCOPE reveal: run the MarowakAnim (flash 8×, fade the
                // ghost out, fade the Marowak in — ghost_marowak_anim.asm).
                self.intro_anim = IntroAnimState::GhostMarowakReveal {
                    phase: 0,
                    counter: 0,
                };
                self.ghost_marowak_palette = 0xe4;
                self.player_visible = true;
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
                // trainer-appear SFX (SFX_SILPH_SCOPE; its wTempoModifier
                // write is dead for non-cries) before "X wants to fight!".
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
                // pic slides off right, then the mon simply appears with its
                // cry (no slide/drop/grow).
                self.cry_pending = Some(enemy_species);
            }
            IntroPhase::PlayerSendOut => {
                // SendOutMon (engine/battle/core.asm:1723): POOF_ANIM at the
                // player's side (hWhoseTurn = 1 → HVFLIP), then
                // AnimateSendingOutMon grows the pic 3×3 → 5×5 → 7×7.
                self.intro_anim = IntroAnimState::PlayerSendOut {
                    stage: 0,
                    frames: 0,
                };
                self.player_visible = true;
                self.enemy_visible = true;
                self.player_entry = None;
                self.enemy_entry = None;
                self.start_non_move_anim(non_move_anim::BALL_POOF, false);
                // The cry fires when the growth completes (original:
                // PlayCry AFTER AnimateSendingOutMon).
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

    /// Two-turn charge-turn narration (`charge_message` in pokered-core,
    /// matching the original's `PrintChargingText` lines). On the charge
    /// turn the original plays STATUS_AFFECTED_ANIM (flash the whose-turn
    /// mon pic) — engine/battle/core.asm:3196/3475/5598/5851.
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
        // engine/items/item_effects.asm:1431/1447): the text prints, then
        // BAIT_ANIM/ROCK_ANIM plays with hWhoseTurn = player.
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
                // enemy mon (hWhoseTurn = enemy; player-side X items arrive
                // via BattleAnimEvent::XStatItem instead).
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
                    // AnimationShowMonPic ends with Delay3.
                    self.anim_wait = self.anim_wait.max(2);
                }
            }
            AnimEffect::ShowEnemyMon => {
                self.set_visible(defender, true);
                if self.current_move != MoveId::DoubleTeam {
                    self.show_reveal[Self::side_index(defender)] = Some(0);
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

    /// First scanline that observes a zero-time BGP write. These boundaries
    /// are measured against the pinned retail-ROM oracle; the one-line side
    /// difference comes from the flipped-turn dispatch path.
    fn palette_write_scanline(&self, effect: &AnimEffect) -> u32 {
        let player = self.current_attacker_is_player;
        let side = |player_line, enemy_line| if player { player_line } else { enemy_line };
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
            // OAM tile ids are absolute VRAM ids (raw + $31, matching
            // DrawFrameBlock); renderer tilesets are indexed from zero.
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

        // Mode02 frame blocks and non-blocking special effects execute before
        // the next VBlank. Consume them until the shared driver reports a
        // display frame or a blocking effect.
        for _ in 0..1024 {
            match self.anim_player.tick() {
                AnimTickResult::Loading { sound } | AnimTickResult::Display { sound } => {
                    if let Some(sound_move) = sound {
                        self.emit_move_sfx(sound_move);
                    }
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
                    // A command-stream effect starts after the preceding
                    // subanimation has returned and cleared shadow OAM.
                    self.commit_move_animation_frame();
                    self.apply_anim_effect(AnimationPlayer::apply_effect(effect));
                    if self.anim_wait > 0 {
                        return;
                    }
                }
                AnimTickResult::Done => {
                    // Shadow OAM is empty now, but the just-finished scanout
                    // still contains the entries submitted one VBlank ago.
                    self.anim_layer_pending.clear();
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

    /// Advance the ball-throw choreography (capture / ghost dodge / old man)
    /// by one frame: start each step's animation in turn, hold shake steps
    /// on their first frame block for 40 frames
    /// (`DoBallShakeSpecialEffects`: SFX_TINK + DelayFrames 40), and keep
    /// each step up for at least its `min_frames` (the original's frame
    /// count × `wSubAnimFrameDelay`).
    fn advance_ball_choreo(&mut self) {
        let Some(choreo) = self.ball_choreo.as_mut() else {
            return;
        };
        if !choreo.started {
            let step = choreo.steps[choreo.step];
            self.anim_player.start(step.anim, true);
            self.anim_wait = 0;
            self.anim_layer.clear();
            self.anim_layer_pending.clear();
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
                // The shadow tilemap changes before DelayFrames(3), but the
                // BG map copier reaches the mon rows on the third VBlank.
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
                // The 7x8 tilemap rewrite itself spans scanouts, so the four
                // nominal Delay4 steps do not land at uniform frame offsets.
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

    /// On the last VBlank of each horizontal-slide delay, only the top third
    /// of the BG map contains the next tilemap column. The ordinary offset is
    /// still used below scanline 48.
    fn horizontal_slide_top_dx(&self, side: MonSide) -> Option<i32> {
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
                // The third tilemap copy reaches the lower mon rows first;
                // its upper band remains at the preceding offset for two
                // scanouts.
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
        // OAM written during the previous VBlank is what this scanout sees.
        std::mem::swap(&mut self.anim_layer, &mut self.anim_layer_pending);
        std::mem::swap(
            &mut self.anim_layer_tileset,
            &mut self.anim_layer_pending_tileset,
        );
        // If no interpreter frame is submitted this update (for example
        // while a frame hook blocks), shadow OAM is copied unchanged.
        self.anim_layer_pending
            .entries
            .clone_from(&self.anim_layer.entries);
        self.anim_layer_pending_tileset = self.anim_layer_tileset;
        // Advance effects that were visible last frame before any command can
        // start a new effect. Newly applied effects therefore render frame 0.
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
                    screen.is_ghost,
                    screen.ghost_marowak_reveal,
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

        // Tick battle transition state
        if let Some(ref mut ts) = self.transition_state {
            ts.tick();
            if ts.is_done() {
                self.transition_state = None;
            }
        }

        match self.intro_anim {
            IntroAnimState::GhostMarowakReveal { phase, counter } => {
                let (next_phase, next_counter) = match phase {
                    0 => {
                        // FlashSprite8Times: 8 flashes, counter tracks flash number
                        if counter >= 16 {
                            (1, 0)
                        } else {
                            (0, counter + 1)
                        }
                    }
                    1 => {
                        // Fade out ghost: dim palette each 10 frames
                        if counter >= 10 {
                            self.ghost_marowak_palette =
                                self.ghost_marowak_palette.saturating_sub(0x10);
                            if self.ghost_marowak_palette <= 0x40 {
                                // The ghost has faded out — the fade-in below
                                // brightens the REVEALED Marowak sprite.
                                self.ghost_marowak_revealed = true;
                                (2, 0)
                            } else {
                                (1, 0)
                            }
                        } else {
                            (1, counter + 1)
                        }
                    }
                    2 => {
                        // Fade in Marowak: brighten palette each 10 frames
                        if counter >= 10 {
                            self.ghost_marowak_palette =
                                (self.ghost_marowak_palette + 0x10).min(0xe4);
                            if self.ghost_marowak_palette >= 0xe4 {
                                self.intro_anim = IntroAnimState::None;
                                self.ghost_marowak_revealed = true;
                                self.silph_scope_sfx_pending = true;
                                self.cry_pending = Some(pokered_data::species::Species::Marowak);
                                return;
                            }
                            (2, 0)
                        } else {
                            (2, counter + 1)
                        }
                    }
                    _ => {
                        self.intro_anim = IntroAnimState::None;
                        return;
                    }
                };
                self.intro_anim = IntroAnimState::GhostMarowakReveal {
                    phase: next_phase,
                    counter: next_counter,
                };
            }
            IntroAnimState::ScreenFlash { step, step_frames } => {
                // 12-step palette sequence repeated 3 times = 36 steps
                // (BattleTransition_FlashScreen: `ld b, $3` outer loop).
                const FLASH_TOTAL_STEPS: u8 = 12 * 3;
                if step_frames > 1 {
                    self.intro_anim = IntroAnimState::ScreenFlash {
                        step,
                        step_frames: step_frames - 1,
                    };
                } else if step < FLASH_TOTAL_STEPS - 1 {
                    // Advance to next palette step (palette index = step % 12)
                    self.intro_anim = IntroAnimState::ScreenFlash {
                        step: step + 1,
                        step_frames: 2,
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
            IntroAnimState::BattleTransition { .. } | IntroAnimState::None => {}
        }
    }

    fn player_offset(&self) -> (i32, i32) {
        let mut dx = 0;
        let mut dy = 0;

        // Original: player back sprite is loaded directly at hlcoord(1,5)
        // via CopyUncompressedPicToTilemap — no slide animation on entry.
        // The sprite just appears in place after the trainer slides off.
        // Retreat uses AnimateRetreatingPlayerMon (shrink to pokeball), not a slide.

        if let Some(exit) = self.player_exit {
            let (ox, oy) = self.exit_slide_offset(exit.kind, exit.frame, true);
            dx += ox;
            dy += oy;
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
            match entry.kind {
                SlideKind::SeUp => {}
                _ => {
                    // Original ScrollTrainerPicAfterBattle scrolls 7 columns from the right,
                    // one column per step with 4-frame delay between steps.
                    // Pixel offset starts large (offscreen right) and decreases to 0.
                    dx += (12i32 - entry.frame as i32).max(0) * 2;
                }
            }
        }
        if let Some(exit) = self.enemy_exit {
            let (ox, oy) = self.exit_slide_offset(exit.kind, exit.frame, false);
            dx += ox;
            dy += oy;
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
        if !self.mon_tilemap.controls_side(MonSide::Enemy) {
            dy += self.fx.mon_dy(MonSide::Enemy);
        }
        // AnimationShakeEnemyHUD scrolls the BG (SCX); the enemy mon is part
        // of the BG in the original, so it shakes along with the HUD strip.
        dx += self.fx.enemy_hud_shake_offset();

        // In the original Game Boy, both player and enemy sprites slide from
        // RIGHT to center during the SilhouetteSlide intro. The SCX register
        // starts at $90 (144px) and decrements by 2 each frame, which shifts
        // BOTH sprite positions from right to left simultaneously.
        // The enemy uses the same scroll direction as the player.
        if let IntroAnimState::SilhouetteSlide { offset, .. } = self.intro_anim {
            dx -= offset;
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
        self.enemy_visible
            && self.blink_mon.visible_band(side, 144).is_some()
            && !reveal_hidden
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

    /// Reproduce the VBlank-edge tilemap tear in
    /// `AnimationMoveMonHorizontally` / `AnimationResetMonPosition`. The
    /// original's first 8-pixel row changes one scanout before the other six.
    fn apply_move_mon_h_raster_edge(&self, fb: &mut FrameBuffer) {
        let Some(anim) = self.move_mon_h else {
            return;
        };
        let Some(toward_opponent) = anim.top_row_transition() else {
            return;
        };

        // The player pic begins on tilemap row 5, so only its first row has
        // crossed the VBlank boundary. The enemy pic begins at row 0: six
        // rows are scanned after the transfer and the last row remains old.
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
            // 12-step FlashScreen matching BattleTransition_FlashScreenPalettes,
            // repeated 3 times (palette index = step % 12). The strobe plays
            // over the OVERWORLD frame before the Circle/DoubleCircle wipe,
            // exactly as the original calls BattleTransition_FlashScreen first.
            // Each entry is a shade→shade map — a BGP register write on the
            // indexed framebuffer (the pixel loop becomes a palette remap).
            IntroAnimState::ScreenFlash { step, .. } => {
                let idx = (step as usize) % FLASH_SCREEN_PALETTE.len();
                fb.remap_shades(&FLASH_SCREEN_PALETTE[idx]);
            }
            // Original GB uses BGP=%11100100 which maps: color0→3, color1→2, color2→1, color3→0
            // This creates a silhouette/negative effect within the 4-shade palette,
            // not a harsh full-RGB inversion. Bright areas become dark, dark become bright,
            // but within the GB's limited palette — producing the iconic white-on-black silhouettes.
            IntroAnimState::SilhouetteSlide { .. } => {
                // Invert the display palette for the white-on-black
                // silhouette effect. Do NOT clear to white first — the
                // rendered sprites' indices are preserved.
                fb.remap_shades(&[3, 2, 1, 0]);
            }
            IntroAnimState::GhostMarowakReveal { phase, counter } => {
                // Ghost Marowak palette manipulation - progressively brighten/darken.
                // Scales the display palette toward black (per-frame, applied
                // to the freshly drawn frame — same result as the old
                // per-pixel multiply, without touching the pixels).
                let palette_val = self.ghost_marowak_palette;
                let mut scale = (palette_val as f32 / 0xe4 as f32).min(1.0);
                if phase == 0 {
                    // FlashSprite8Times: "alternate between black and light gray
                    // 8 times" (ghost_marowak_anim.asm:19-22) — 16 counter frames
                    // = 8 on/off alternations.
                    if counter % 4 >= 2 {
                        scale *= 0.35;
                    }
                }
                fb.scale_shades(scale);
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
/// 3×3 → 5×5 → 7×7. The source pic is read from the top-left
/// `src_tiles`×`src_tiles` block of `src` (laid out `src_tiles` per row).
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
    fb.clear(Rgba::WHITE);
    let pal = &GRAYSCALE_PALETTE;
    let sprite_pal = &GRAYSCALE_SPRITE_PALETTE;
    // `GRAYSCALE_SPRITE_PALETTE` makes color 0 transparent for ordinary
    // sprite blits. Substitute replaces a BG mon picture, so its blank
    // pixels are opaque white just like `wTempPic` in the original.
    let mut substitute_pal = *sprite_pal;
    substitute_pal.colors[0] = Rgba::WHITE;

    // During BattleTransitionWipe and TransitionFlash, the screen should be
    // fully controlled by the transition/animation effects — no battle scene
    // elements (HUD, sprites, text box) should be visible yet.
    // This matches the ASM flow: DoBattleTransition → SET_PAL_BATTLE_BLACK →
    // SlideSilhouettes → only then do battle elements appear.
    let is_flash = matches!(
        &screen.phase,
        BattlePhase::Intro {
            phase: IntroPhase::TransitionFlash,
            ..
        }
    );
    let skip_battle_render = matches!(
        &screen.phase,
        BattlePhase::Intro { phase, .. }
        if matches!(
            phase,
            IntroPhase::BattleTransitionWipe(_)
        )
    );

    if is_flash {
        // FlashScreen strobes the OVERWORLD screen BEFORE the wipe begins
        // (BattleTransition_Circle/DoubleCircle call BattleTransition_FlashScreen
        // first). Draw the snapshot, then apply the palette strobe on top.
        if let Some(snap) = effects.overworld_snapshot.as_ref() {
            fb.copy_from(snap);
        } else {
            fb.clear(Rgba::BLACK);
        }
        effects.apply_post_effects(fb);
        return;
    }

    if skip_battle_render {
        // Start from the base palette: the wipe would otherwise display through
        // whatever display palette the previous frame left behind (today it is
        // only safe by accident of the flash strobe ending on the identity map).
        fb.reset_palette();
        // During transition wipe, render the wipe on top of the overworld snapshot.
        // If the wipe animation has already finished (transition_state cleared)
        // but the core is still in BattleTransitionWipe waiting out its
        // remaining frames, keep the screen fully black — matches the ASM
        // BattleTransition_BlackScreen + DelayFrames hold and avoids a
        // single-frame flash back to white between the wipe and the
        // SilhouetteSlide phase.
        {
            let snapshot = effects.overworld_snapshot.as_ref();
            if let Some(snap) = snapshot {
                if effects.has_transition() {
                    effects.render_transition(snap, fb);
                } else {
                    fb.clear(Rgba::BLACK);
                }
                return;
            }
        }
        // No snapshot → just black screen for flash sequence
        fb.clear(Rgba::BLACK);
        effects.apply_post_effects(fb);
        return;
    }

    // A Pokémon-Tower GHOST (no Silph Scope) shows as "GHOST", not the real species.
    // The ghost-Marowak battle (with scope) is also "GHOST" until the unveil phase
    // completes and the SILPH SCOPE reveals the Marowak.
    let is_zh = language == pokered_core::game_state::Lang::Zh;
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
    // The catch tutorial shows the player as "OLD MAN" (Gen-1 BATTLE_TYPE_OLD_MAN).
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

    // During BattleTransitionWipe and TransitionFlash, the screen should be
    // fully controlled by the transition/animation effects — no battle scene
    // elements (HUD, sprites, text box) should be visible yet.
    // This matches the ASM flow: DoBattleTransition → SET_PAL_BATTLE_BLACK →
    // SlideSilhouettes → only then do battle elements appear.
    let skip_battle_render = matches!(
        &screen.phase,
        BattlePhase::Intro { phase, .. }
        if matches!(
            phase,
            IntroPhase::BattleTransitionWipe(_) | IntroPhase::TransitionFlash
        )
    );

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
                    | IntroPhase::WildReveal
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

        let wild_reveal_balls = matches!(
            &screen.phase,
            BattlePhase::Intro {
                phase: IntroPhase::WildReveal,
                wait_frames,
            } if *wait_frames < WILD_REVEAL_BALL_FRAMES
        );
        if screen.show_player_pokeballs || wild_reveal_balls {
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
        let skip_dialog = matches!(
            &screen.phase,
            BattlePhase::Intro { phase, .. }
            if matches!(
                phase,
                IntroPhase::SilhouetteSlide
            )
        );

        // Pre-compute dialog text for non-menu phases.
        let (dialog_text, dialog_show_arrow) = if skip_dialog {
            (None, false)
        } else if matches!(
            screen.phase,
            BattlePhase::PlayerMenu
                | BattlePhase::MoveSelect
                | BattlePhase::ItemMoveSelect { .. }
                | BattlePhase::LearnMoveChoose { .. }
                | BattlePhase::BagSelect
                | BattlePhase::ItemTargetSelect { .. }
        ) || matches!(
            screen.phase,
            BattlePhase::PartySelect
                | BattlePhase::ShiftSwitchSelect
                | BattlePhase::PlayerFaintSwitch
        ) {
            (None, false)
        } else {
            let trainer_name = screen
                .trainer_name
                .clone()
                .or_else(|| screen.trainer_class.map(|tc| tc.display_name().to_string()))
                .unwrap_or_else(|| enemy_name.clone());
            let text = match &screen.phase {
                BattlePhase::Intro { phase, .. } => match phase {
                    IntroPhase::SilhouetteSlide => None,
                    IntroPhase::WildReveal => {
                        // PrintBeginningBattleText: a ghost battle opens with
                        // "Enemy GHOST appeared!" (EnemyAppearedText) — the
                        // ghost-Marowak battle only until the unveil completes,
                        // after which it's the normal "Wild MAROWAK appeared!".
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
                    // GhostCantBeIDdText (data/text/text_2.asm:1269-1272).
                    IntroPhase::GhostCantID => Some("Darn! The GHOST\ncan't be ID'd!".to_string()),
                    // UnveiledGhostText (data/text/text_2.asm:1263-1267).
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
                    _ => None,
                },
                BattlePhase::BattleOver { won, escaped, .. } => {
                    if *escaped {
                        None
                    } else if *won {
                        Some("You won!".to_string())
                    } else {
                        Some("You lost...".to_string())
                    }
                }
                _ => screen.current_message.clone(),
            };
            let arrow = match &screen.phase {
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
            // PrintBeginningBattleText does not expose the complete sentence
            // on the first WildReveal frame.  The reference keeps the empty
            // battle text box while the cry/setup path runs, then writes one
            // visible character per frame.  Preserve the explicit newline but
            // do not count it as a typed glyph.
            let text = match (&screen.phase, text) {
                (
                    BattlePhase::Intro {
                        phase: IntroPhase::WildReveal,
                        wait_frames,
                    },
                    Some(full),
                ) => {
                    if language == Lang::Zh {
                        Some(full)
                    } else {
                        let full = full.replacen(" appeared!", "\nappeared!", 1);
                        let visible = if *wait_frames <= WILD_REVEAL_TEXT_START_WAIT {
                            1 + 3 * ((WILD_REVEAL_TEXT_START_WAIT - *wait_frames) / 3)
                        } else {
                            0
                        };
                        let mut glyphs = 0u16;
                        Some(
                            full.chars()
                                .take_while(|ch| {
                                    if *ch == '\n' {
                                        return true;
                                    }
                                    glyphs += 1;
                                    glyphs <= visible
                                })
                                .collect(),
                        )
                    }
                }
                (_, text) => text,
            };
            (text, arrow)
        };

        // Phases whose dialog box / menu are drawn directly to the framebuffer
        // by pokered-ui at the end of this function (post-sprite, post-anim).
        // These intentionally bypass the `tile_buf` path so the same JSON layout
        // drives both the editor preview and the in-game render.
        // Phases routed through pokered-ui (FrameBufferPainter + JSON layouts).
        // Bag/Party variants are unified so the editor preview and in-game
        // render share the same code path. ItemTargetSelect reuses the party
        // list view (same data, target-pick semantics).
        let use_unified_ui = !effects.move_animation_capture_scene
            && (matches!(
                screen.phase,
                BattlePhase::PlayerMenu
                    | BattlePhase::MoveSelect
                    | BattlePhase::ItemMoveSelect { .. }
                    | BattlePhase::LearnMoveChoose { .. }
                    | BattlePhase::BagSelect
                    | BattlePhase::ItemTargetSelect { .. }
                    | BattlePhase::PartySelect
                    | BattlePhase::ShiftSwitchSelect
                    | BattlePhase::PlayerFaintSwitch
            ) || dialog_text.is_some());

        // ── Bottom area (text box + menu) ───────────────────────────
        // Palette animations transform the complete framebuffer. Keep the
        // recorder's static scene pixel-identical to the retail ROM by using
        // the native 8x8 battle font instead of the proportional frontend UI.
        if effects.move_animation_capture_scene {
            let frame = TextBoxFrame::standard_dialog();
            frame.draw_frame(&mut tile_buf);
            if let Some(text) = dialog_text.as_deref() {
                let mut lines = text.split('\n');
                if let Some(line) = lines.next() {
                    write_tiles_at(&mut tile_buf, 1, 14, &ascii_to_tiles(line));
                }
                if let Some(line) = lines.next() {
                    write_tiles_at(&mut tile_buf, 1, 16, &ascii_to_tiles(line));
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
                        blit_tileset(fb, &ts, ex, ey, w_tiles, sprite_pal);
                    }
                }
            } else if screen.is_ghost
                || (screen.ghost_marowak_reveal && !effects.ghost_marowak_revealed)
            {
                // An unidentified Pokémon-Tower GHOST → the gfx/battle/ghost.png sprite,
                // not the real species front sprite. The ghost-Marowak battle (with
                // scope) also opens on the ghost sprite; the reveal anim swaps it for
                // the Marowak front when the fade-in completes.
                if let Ok(cached) = rm.load_battle("ghost") {
                    let ts = cached.tileset.clone();
                    let w_tiles = cached.source_size.0 / TILE_SIZE;
                    let h_tiles = cached.source_size.1 / TILE_SIZE;
                    let x_off = ((8 - w_tiles) / 2) * TILE_SIZE;
                    let y_off = (7 - h_tiles) * TILE_SIZE;
                    let ex = apply_offset(12 * TILE_SIZE + x_off, enemy_dx);
                    let ey = apply_offset(y_off, enemy_dy);
                    blit_tileset(fb, &ts, ex, ey, w_tiles, sprite_pal);
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
                        BattleEffects::draw_substitute(
                            fb,
                            rect,
                            &doll,
                            &substitute_pal,
                            MonSide::Enemy,
                        );
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
                BattleEffects::draw_minimized(fb, rect, sprite_pal);
            } else if let Ok(cached) = rm.load_pokemon_front(&enemy_sprite) {
                let ts = cached.tileset.clone();
                let w_tiles = cached.source_size.0 / TILE_SIZE;
                let h_tiles = cached.source_size.1 / TILE_SIZE;
                let x_off = ((8 - w_tiles) / 2) * TILE_SIZE;
                let y_off = (7 - h_tiles) * TILE_SIZE;
                let ex = (12 * TILE_SIZE + x_off) as i32 + enemy_dx;
                let ey = y_off as i32 + enemy_dy;
                if effects.draw_slide_up(fb, &ts, ex, ey, w_tiles, sprite_pal, MonSide::Enemy) {
                } else if effects.mon_tilemap.draw(
                    fb,
                    &ts,
                    ex,
                    ey,
                    w_tiles,
                    sprite_pal,
                    MonSide::Enemy,
                ) {
                } else if let Some((rows, yoff)) = effects.fx.slide_down_hide_params(MonSide::Enemy)
                {
                    // AnimationSlideMonDownAndHide (Acid Armor): crop to the
                    // top rows (7×5 then 7×3 tile-id lists), drawn lower.
                    BattleEffects::draw_mon_rows(fb, &ts, ex, ey + yoff, w_tiles, sprite_pal, rows);
                } else if effects.draw_squish(fb, &ts, ex, ey, sprite_pal, MonSide::Enemy) {
                } else if let Some((width, anchor_right)) = effects.fx.squish_params(MonSide::Enemy)
                {
                    // AnimationSquishMonPic: narrow the pic one tile per pass.
                    BattleEffects::draw_squished(
                        fb,
                        &ts,
                        ex,
                        ey,
                        w_tiles,
                        sprite_pal,
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
                                sprite_pal,
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
                            sprite_pal,
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
                            sprite_pal,
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
                            sprite_pal,
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
                            sprite_pal,
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
                matches!(phase, IntroPhase::SilhouetteSlide | IntroPhase::WildReveal)
                    || (!screen.is_wild
                        && matches!(
                            phase,
                            IntroPhase::TrainerReveal | IntroPhase::TrainerSendOut
                        ))
            }
            _ => false,
        };

        let (player_dx, player_dy) = effects.player_offset();
        if effects.player_visible_now()
            || effects.hide_mon_one_frame == Some(MonSide::Player)
            || effects.squish_visible(MonSide::Player)
        {
            if let IntroAnimState::PlayerSendOut { stage, .. } = effects.intro_anim {
                // AnimateSendingOutMon stages (engine/battle/core.asm:6801):
                // stage 0 = POOF only (mon still in the ball), stage 1 = the
                // ball tile at hlcoord(4,11), then the pic grows 3×3 at
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
                                sprite_pal,
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
                    blit_tileset(fb, &scaled, px, py, 7, sprite_pal);
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
                        BattleEffects::draw_substitute(
                            fb,
                            rect,
                            &doll,
                            &substitute_pal,
                            MonSide::Player,
                        );
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
                BattleEffects::draw_minimized(fb, rect, sprite_pal);
            } else {
                let back_sprite_name = format!("{}b", player_sprite);
                if let Ok(cached) = rm.load_pokemon_back(&back_sprite_name) {
                    let ts = cached.tileset.clone();
                    let src_tpr = (cached.source_size.0 / TILE_SIZE) as usize;
                    let scaled = scale_sprite_by_two(&ts, src_tpr);
                    let px = TILE_SIZE as i32 + player_dx;
                    let py = (5 * TILE_SIZE) as i32 + player_dy;
                    if effects.draw_slide_up(fb, &scaled, px, py, 7, sprite_pal, MonSide::Player) {
                    } else if effects.mon_tilemap.draw(
                        fb,
                        &scaled,
                        px,
                        py,
                        7,
                        sprite_pal,
                        MonSide::Player,
                    ) {
                    } else if let Some((rows, yoff)) =
                        effects.fx.slide_down_hide_params(MonSide::Player)
                    {
                        BattleEffects::draw_mon_rows(
                            fb,
                            &scaled,
                            px,
                            py + yoff,
                            7,
                            sprite_pal,
                            rows,
                        );
                    } else if effects.draw_squish(fb, &scaled, px, py, sprite_pal, MonSide::Player)
                    {
                    } else if let Some((width, anchor_right)) =
                        effects.fx.squish_params(MonSide::Player)
                    {
                        BattleEffects::draw_squished(
                            fb,
                            &scaled,
                            px,
                            py,
                            7,
                            sprite_pal,
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
                                sprite_pal,
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
                                sprite_pal,
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
                                sprite_pal,
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
                                sprite_pal,
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

        // In Gen1 move-select, the TYPE/PP panel overlays the player sprite.
        // Our sprite blit happens after tilemap rendering, so redraw this panel
        // region last to keep it in the foreground.
        // (Skipped when `use_unified_ui` — pokered-ui renders this panel directly
        // to the framebuffer below, after sprites, so no tile_buf overlay is needed.)
        if !use_unified_ui
            && matches!(
                screen.phase,
                BattlePhase::MoveSelect | BattlePhase::ItemMoveSelect { .. }
            )
        {
            tile_buf.render_region(fb, &battle_ts, pal, 0, 8, 11, 5);
        }

        // Keep the bottom dialog/menu box in front of sprites and animation overlays.
        // Skipped for `use_unified_ui` phases — pokered-ui draws their box directly to fb.
        if !use_unified_ui {
            tile_buf.render_region(fb, &battle_ts, pal, 0, 12, 20, 6);
        }

        // Chinese HUD names: the tile-based HUD cannot render CJK glyphs, so
        // the names are drawn with the pixel font here, after the tilemap and
        // sprite/panel re-blits. Left-aligned at the HUD name origin — centering
        // pushes 3+ char names right onto the "Lv" column below (CJK glyphs are
        // 10px tall and their lower edge grazes the level row).
        if is_zh {
            let text_color = Rgba::new(0, 0, 0, 255);
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
        }

        // Unified UI render: draw battle dialog/menus directly to framebuffer
        // using the same code path the editor preview uses. Hardware OAM is
        // composited after this window/background layer below.
        if !skip_dialog && use_unified_ui {
            let mut painter = FrameBufferPainter::new(fb).with_lang(language);
            let mut ui = Ui::new(&mut painter);
            let rd = PokemonRenderData::new(is_zh);
            if matches!(screen.phase, BattlePhase::PlayerMenu) {
                if screen.is_safari {
                    menus::battle_safari::draw(&screen.safari_menu, &mut ui, language);
                    // SAFARI BALL count after "BALL": the original prints
                    // wNumSafariBalls as a 2-digit number (core.asm:2077-2081,
                    // PrintNumber at hlcoord 7,14 — "BALL×NN").
                    draw_text(
                        &format!("×{}", screen.safari_menu.safari_balls_remaining),
                        112,
                        112,
                        Rgba::new(0, 0, 0, 255),
                        fb,
                    );
                } else {
                    menus::battle_main::draw(
                        &screen.battle_menu,
                        &BATTLE_MAIN_DEFAULT_LAYOUT,
                        &mut ui,
                        language,
                    );
                }
            } else if matches!(
                screen.phase,
                BattlePhase::MoveSelect
                    | BattlePhase::ItemMoveSelect { .. }
                    | BattlePhase::LearnMoveChoose { .. }
            ) {
                if let Some(ref mm) = screen.move_menu {
                    menus::battle_move::draw(
                        mm,
                        &BATTLE_MOVE_DEFAULT_LAYOUT,
                        &mut ui,
                        language,
                        &rd,
                    );
                }
            } else if matches!(screen.phase, BattlePhase::BagSelect) {
                if let Some(ref bm) = screen.bag_menu {
                    menus::battle_bag::draw(bm, &BATTLE_BAG_DEFAULT_LAYOUT, &mut ui, &rd);
                }
            } else if matches!(
                screen.phase,
                BattlePhase::PartySelect
                    | BattlePhase::ItemTargetSelect { .. }
                    | BattlePhase::ShiftSwitchSelect
                    | BattlePhase::PlayerFaintSwitch
            ) {
                if let Some(ref bs) = screen.battle_state {
                    menus::battle_party::draw(
                        &bs.player.party,
                        screen.party_cursor,
                        &BATTLE_PARTY_DEFAULT_LAYOUT,
                        &mut ui,
                        language == Lang::Zh,
                    );
                }
            } else if matches!(
                screen.phase,
                BattlePhase::ShiftPrompt
                    | BattlePhase::LearnMoveAsk { .. }
                    | BattlePhase::LearnMoveGiveUpConfirm { .. }
            ) {
                // "Will you change #MON?" — prompt text + YES/NO box
                // (TWO_OPTION_MENU, cursor default NO). The learn-move chain
                // reuses the same TWO_OPTION_MENU rendering.
                if let Some(ref text) = dialog_text {
                    let shown = if language == Lang::Zh {
                        crate::render::zh_battle_dialog(text, true)
                    } else {
                        text.clone()
                    };
                    menus::battle_text::draw(
                        &shown,
                        false,
                        &BATTLE_TEXT_DEFAULT_LAYOUT,
                        &mut ui,
                        language,
                    );
                }
                let (yes, no) = if language == Lang::Zh {
                    ("是".to_string(), "否".to_string())
                } else {
                    ("YES".to_string(), "NO".to_string())
                };
                let opts = vec![yes, no];
                let selected = if screen.shift_prompt_yes { 0 } else { 1 };
                menus::yes_no::draw(&opts, selected, &YES_NO_DEFAULT_LAYOUT, &mut ui);
            } else if let Some(ref text) = dialog_text {
                let shown = if language == Lang::Zh {
                    crate::render::zh_battle_dialog(text, true)
                } else {
                    text.clone()
                };
                if matches!(
                    &screen.phase,
                    BattlePhase::Intro {
                        phase: IntroPhase::WildReveal,
                        ..
                    }
                ) {
                    menus::battle_text::draw_hard_lines(
                        &shown,
                        dialog_show_arrow,
                        &BATTLE_TEXT_DEFAULT_LAYOUT,
                        &mut ui,
                        language,
                    );
                } else {
                    menus::battle_text::draw(
                        &shown,
                        dialog_show_arrow,
                        &BATTLE_TEXT_DEFAULT_LAYOUT,
                        &mut ui,
                        language,
                    );
                }
            }
        }

        effects.apply_move_mon_h_raster_edge(fb);
        effects.apply_post_effects(fb);

        // Hardware OAM is composited after the window/background, so move
        // objects may cover the battle text box just as they do on the Game Boy.
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
                render_gen1_oam(fb, &effects.anim_layer.entries, &cached.tileset, pal);
            }
        }
    } else {
        // No resources — fallback: render tile buffer with blank tileset
        let blank_ts = TileSet::blank(256);
        tile_buf.render(fb, &blank_ts, pal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ball_choreo_caught_is_toss_poof_hide_shake3() {
        // $43: TOSS, POOF, HIDEPIC, SHAKE × 3 (TossBallAnimation c=4).
        let choreo = build_ball_choreo(ItemId::PokeBall, 3, BallAnimOutcome::Caught);
        let anims: Vec<usize> = choreo.steps.iter().map(|s| s.anim).collect();
        assert_eq!(
            anims,
            vec![
                non_move_anim::BALL_TOSS,
                non_move_anim::BALL_POOF,
                non_move_anim::HIDEPIC,
                non_move_anim::BALL_SHAKE,
                non_move_anim::BALL_SHAKE,
                non_move_anim::BALL_SHAKE,
            ]
        );
        // SFX: BallToss, BallPoof, then one Tink per shake.
        let sfx: Vec<Option<SfxId>> = choreo.steps.iter().map(|s| s.sfx).collect();
        assert_eq!(
            sfx,
            vec![
                Some(SfxId::BallToss),
                Some(SfxId::BallPoof),
                None,
                Some(SfxId::Tink),
                Some(SfxId::Tink),
                Some(SfxId::Tink),
            ]
        );
    }

    #[test]
    fn ball_choreo_broke_free_reopens_and_shows_mon() {
        // $61-$63: TOSS, POOF, HIDEPIC, SHAKE × N, POOF, SHOWPIC (c=6).
        let choreo = build_ball_choreo(ItemId::GreatBall, 2, BallAnimOutcome::BrokeFree);
        let anims: Vec<usize> = choreo.steps.iter().map(|s| s.anim).collect();
        assert_eq!(
            anims,
            vec![
                non_move_anim::GREAT_TOSS,
                non_move_anim::BALL_POOF,
                non_move_anim::HIDEPIC,
                non_move_anim::BALL_SHAKE,
                non_move_anim::BALL_SHAKE,
                non_move_anim::BALL_POOF,
                non_move_anim::SHOWPIC,
            ]
        );
    }

    #[test]
    fn ball_choreo_miss_and_dodge_are_toss_only_variants() {
        // $20 (0 shakes): TOSS, POOF — the mon is never hidden.
        let miss = build_ball_choreo(ItemId::UltraBall, 0, BallAnimOutcome::BrokeFree);
        let anims: Vec<usize> = miss.steps.iter().map(|s| s.anim).collect();
        assert_eq!(
            anims,
            vec![non_move_anim::ULTRA_TOSS, non_move_anim::BALL_POOF]
        );
        // $10 (ghost dodge): TOSS only. Safari Ball uses the HIGH toss.
        let dodge = build_ball_choreo(ItemId::SafariBall, 0, BallAnimOutcome::Dodged);
        let anims: Vec<usize> = dodge.steps.iter().map(|s| s.anim).collect();
        assert_eq!(anims, vec![non_move_anim::ULTRA_TOSS]);
    }

    /// Drive the visual-effects layer through a full choreography and check
    /// the deterministic SFX sequence + enemy visibility flow.
    fn run_choreo(vfx: &mut BattleVisualEffects, max_frames: usize) {
        for _ in 0..max_frames {
            vfx.advance_move_animation();
            vfx.advance_ball_choreo();
            vfx.fx.tick();
            if vfx.ball_choreo.is_none() {
                break;
            }
        }
    }

    #[test]
    fn caught_choreo_hides_enemy_and_plays_all_sfx() {
        let mut vfx = BattleVisualEffects::default();
        vfx.on_anim_event(BattleAnimEvent::Ball {
            ball: ItemId::PokeBall,
            shakes: 3,
            outcome: BallAnimOutcome::Caught,
        });
        run_choreo(&mut vfx, 1000);
        assert!(vfx.ball_choreo.is_none(), "choreography must terminate");
        let mut sfx = Vec::new();
        while let Some(s) = vfx.take_ball_sfx() {
            sfx.push(s);
        }
        assert_eq!(
            sfx,
            vec![
                SfxId::BallToss,
                SfxId::BallPoof,
                SfxId::Tink,
                SfxId::Tink,
                SfxId::Tink
            ]
        );
        // HIDEPIC ran and no SHOWPIC followed: the mon stays hidden.
        assert!(vfx.fx.mon_hidden(MonSide::Enemy));
    }

    #[test]
    fn broke_free_choreo_reshows_enemy() {
        let mut vfx = BattleVisualEffects::default();
        vfx.on_anim_event(BattleAnimEvent::Ball {
            ball: ItemId::PokeBall,
            shakes: 1,
            outcome: BallAnimOutcome::BrokeFree,
        });
        run_choreo(&mut vfx, 1000);
        assert!(vfx.ball_choreo.is_none());
        // SHOWPIC cleared the HIDEPIC latch: the mon is visible again.
        assert!(!vfx.fx.mon_hidden(MonSide::Enemy));
        let mut sfx = Vec::new();
        while let Some(s) = vfx.take_ball_sfx() {
            sfx.push(s);
        }
        assert_eq!(
            sfx,
            vec![
                SfxId::BallToss,
                SfxId::BallPoof,
                SfxId::Tink,
                SfxId::BallPoof
            ]
        );
    }

    #[test]
    fn item_use_messages_are_not_move_uses() {
        assert_eq!(
            BattleVisualEffects::used_item_id("RED used POKé BALL!"),
            Some(ItemId::PokeBall)
        );
        assert_eq!(
            BattleVisualEffects::used_item_id("BROCK used X ATTACK!"),
            Some(ItemId::XAttack)
        );
        assert_eq!(
            BattleVisualEffects::used_item_id("Enemy PIDGEY used GUST!"),
            None
        );
        assert_eq!(
            BattleVisualEffects::used_item_id("CHARMANDER used EMBER!"),
            None
        );
    }

    #[test]
    fn charge_messages_match_status_affected_trigger() {
        assert!(BattleVisualEffects::is_charge_message(
            "CHARMANDER flew up high!"
        ));
        assert!(BattleVisualEffects::is_charge_message(
            "Enemy PIDGEY dug a hole!"
        ));
        assert!(BattleVisualEffects::is_charge_message(
            "Enemy EXEGGUTOR took in sunlight!"
        ));
        assert!(!BattleVisualEffects::is_charge_message(
            "CHARMANDER used FLY!"
        ));
    }
}
