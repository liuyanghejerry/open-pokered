//! Gengar vs Nidorino intro battle animation state machine.
//!
//! Replicates `PlayIntroScene` from `engine/movie/intro.asm`:
//! Nidorino slides in from the left, exchanges blows with Gengar,
//! then lunges — followed by a fade-out to the title screen.
//!
//! This module handles the logical state (animation phase, timers, positions).
//! Rendering is handled by `pokered-app/src/render/intro.rs`.

use crate::game_state::{GameScreen, ScreenAction};

// ============================================================================
// Animation tables
// ============================================================================

/// Each entry is (dy, dx) — **absolute offset** applied to all OAM sprite positions
/// each keyframe. NOT cumulative; each keyframe replaces the previous offset.
/// Each keyframe is held for `ANIM_FRAME_DELAY` frames (5).
///
/// From `IntroNidorinoAnimation1` through `IntroNidorinoAnimation7` in intro.asm.
/// (Animation0 is just `(0,0)` idle — not used in choreography.)

/// IntroNidorinoAnimation1: hip — small hop right+up then down.
pub const ANIM_1: &[(i8, i8)] = &[(0, 0), (-2, 2), (-1, 2), (1, 2), (2, 2)];
/// IntroNidorinoAnimation2: hop — small hop left+up then down.
pub const ANIM_2: &[(i8, i8)] = &[(0, 0), (-2, -2), (-1, -2), (1, -2), (2, -2)];
/// IntroNidorinoAnimation3: recoil — big knockback right after Gengar slash.
pub const ANIM_3: &[(i8, i8)] = &[(0, 0), (-12, 6), (-8, 6), (8, 6), (12, 6)];
/// IntroNidorinoAnimation4: hip — medium hop left+up then down.
pub const ANIM_4: &[(i8, i8)] = &[(0, 0), (-8, -4), (-4, -4), (4, -4), (8, -4)];
/// IntroNidorinoAnimation5: hop — medium hop right+up then down.
pub const ANIM_5: &[(i8, i8)] = &[(0, 0), (-8, 4), (-4, 4), (4, 4), (8, 4)];
/// IntroNidorinoAnimation6: crouch — slight downward bob.
pub const ANIM_6: &[(i8, i8)] = &[(0, 0), (2, 0), (2, 0), (0, 0)];
/// IntroNidorinoAnimation7: lunge — jump up-left toward Gengar.
pub const ANIM_7: &[(i8, i8)] = &[(-8, -16), (-7, -14), (-6, -12), (-4, -10)];

/// Frames each keyframe is held before advancing to the next.
/// From `ld c, 5` in `AnimateIntroNidorino`.
pub const ANIM_FRAME_DELAY: u32 = 5;

// ============================================================================
// Choreography constants (from intro.asm)
// ============================================================================

/// Phase 1: `MoveNidorinoRight` — 40 iterations (80/2), each moves +2px.
/// ASM: `lb de, 80 / 2, MOVE_NIDORINO_RIGHT` → d=40 steps.
/// Each step: Nidorino OAM += 2, scroll_x += 2, then 2-frame delay.
pub const MOVE_RIGHT_STEPS: u32 = 40;
pub const MOVE_RIGHT_SPEED: i32 = 2;

/// After phase 1 completes, nidorino_base_x should be this value.
pub const NIDORINO_FINAL_BASE_X: i32 = 80;

/// Wait frames after various hip-hop sequences.
pub const WAIT_AFTER_HIPHOP1: u32 = 10;
pub const WAIT_AFTER_HIPHOP2: u32 = 30;
pub const WAIT_AFTER_GENGAR_RAISE: u32 = 30;
pub const WAIT_AFTER_GENGAR_SLASH: u32 = 30;
pub const WAIT_AFTER_GENGAR_RETREAT: u32 = 60;
pub const WAIT_AFTER_HIPHOP3: u32 = 20;
pub const WAIT_AFTER_CROUCH: u32 = 30;

/// Gengar movement: scroll_x delta per step.
pub const GENGAR_SCROLL_SPEED: i32 = 2;
/// Frames per movement step in IntroMoveMon (ASM: `ld c, 2; call CheckForUserInterruption`).
pub const MOVE_STEP_DELAY: u32 = 2;
/// Gengar raise: 4 steps moving left (scroll_x += 2 each). ASM: `lb de, 8 / 2, MOVE_GENGAR_LEFT`.
pub const GENGAR_RAISE_STEPS: u32 = 4;
/// Gengar slash: 8 steps moving right (scroll_x -= 2 each). ASM: `lb de, 16 / 2, MOVE_GENGAR_RIGHT`.
pub const GENGAR_SLASH_STEPS: u32 = 8;
/// Gengar retreat: 4 steps moving left (scroll_x += 2 each). ASM: `lb de, 8 / 2, MOVE_GENGAR_LEFT`.
pub const GENGAR_RETREAT_STEPS: u32 = 4;

/// Fade-out: 24 frames total (3 fade steps × 8 frames each).
pub const FADE_OUT_FRAMES: u32 = 24;

// ============================================================================
// Enums
// ============================================================================

/// Animation phases of the intro scene, played in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroPhase {
    /// Nidorino slides in from the left: 40 steps, +2px each.
    MoveNidorinoRight,
    /// First hip-hop: SFX Hip → Anim1, SFX Hop → Anim2, wait 10.
    HipHop1,
    /// Second hip-hop: SFX Hip → Anim1, SFX Hop → Anim2, wait 30.
    HipHop2,
    /// Gengar raises arm: pose=RaiseArm, SFX Raise, scroll left 4 steps, wait 30.
    GengarRaise,
    /// Gengar slashes: pose=Slash, SFX Crash, scroll right 8 steps,
    /// then SFX Hip, sprite_set=1, Anim3 (recoil), wait 30.
    GengarSlash,
    /// Gengar retreats: scroll left 4 steps, pose=Idle, wait 60.
    GengarRetreat,
    /// Third hip-hop: sprite_set=0, SFX Hip → Anim4, SFX Hop → Anim5, wait 20.
    HipHop3,
    /// Crouch: sprite_set=1, Anim6, wait 30.
    Crouch,
    /// Lunge: SFX Lunge, sprite_set=2, Anim7.
    Lunge,
    /// Fade to white over 24 frames, then transition.
    FadeOut,
    /// Done — transition to title screen.
    Done,
}

/// Gengar's visual pose (which tilemap to display).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GengarPose {
    /// Default standing pose (gengar_1.tilemap).
    Idle,
    /// Arm raised (gengar_2.tilemap).
    RaiseArm,
    /// Slashing (gengar_3.tilemap).
    Slash,
}

/// SFX events emitted by the state machine for the game loop to play.
/// Consumed (read then cleared) each frame by game.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroSfxEvent {
    None,
    IntroHip,
    IntroHop,
    IntroRaise,
    IntroCrash,
    IntroLunge,
}

/// Sub-state within a phase that plays a Nidorino animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimState {
    /// Not playing any animation.
    Idle,
    /// Playing an animation: current keyframe index + frames remaining at this keyframe.
    Playing { keyframe: usize, frames_left: u32 },
    /// Animation complete, waiting for `wait_frames` before phase transition.
    Waiting { frames_left: u32 },
}

/// Sub-state for phases that combine Gengar movement + optional Nidorino animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GengarMoveState {
    /// Moving Gengar (scroll_x changes). `delay` counts down each frame before stepping.
    Moving { steps_left: u32, delay: u32 },
    /// Movement done, playing a Nidorino animation.
    Animating,
    /// Waiting after everything in this phase.
    Waiting { frames_left: u32 },
}

// ============================================================================
// State struct
// ============================================================================

/// Complete state for the intro scene animation.
#[derive(Debug, Clone)]
pub struct IntroSceneState {
    /// Current choreography phase.
    pub phase: IntroPhase,
    /// Frame counter within the current phase.
    pub frame_counter: u32,

    /// Gengar background horizontal scroll (equivalent to hSCX).
    /// Positive = Gengar shifted left, Negative = shifted right.
    pub scroll_x: i32,
    /// Gengar's visual pose.
    pub gengar_pose: GengarPose,

    /// Nidorino base X (moves during MoveNidorinoRight).
    pub nidorino_base_x: i32,
    /// Nidorino base Y (constant at 80).
    pub nidorino_base_y: i32,
    /// Cumulative animation offset X (from current animation keyframes).
    pub nidorino_anim_dx: i32,
    /// Cumulative animation offset Y (from current animation keyframes).
    pub nidorino_anim_dy: i32,
    /// Which Nidorino sprite sheet to use: 0, 1, or 2.
    pub nidorino_sprite_set: u8,

    /// SFX event for the current frame (consumed by game.rs each frame).
    pub sfx_event: IntroSfxEvent,

    // -- Private animation bookkeeping --
    /// Current animation sub-state.
    anim_state: AnimState,
    /// Pointer to the current animation table (if playing).
    current_anim: &'static [(i8, i8)],
    /// Sub-state for Gengar movement phases.
    gengar_move_state: GengarMoveState,
    /// Step counter for MoveNidorinoRight phase.
    move_step: u32,
    move_delay: u32,
}

impl IntroSceneState {
    pub fn new() -> Self {
        Self {
            phase: IntroPhase::MoveNidorinoRight,
            frame_counter: 0,
            scroll_x: 0,
            gengar_pose: GengarPose::Idle,
            nidorino_base_x: 0,
            nidorino_base_y: 80,
            nidorino_anim_dx: 0,
            nidorino_anim_dy: 0,
            nidorino_sprite_set: 0,
            sfx_event: IntroSfxEvent::None,
            anim_state: AnimState::Idle,
            current_anim: ANIM_1,
            gengar_move_state: GengarMoveState::Moving {
                steps_left: 0,
                delay: 0,
            },
            move_step: 0,
            move_delay: 0,
        }
    }

    /// Reset to initial state (e.g. when transitioning back to intro).
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Get fade progress (0.0 = no fade, 1.0 = fully white).
    pub fn fade_progress(&self) -> f32 {
        if self.phase == IntroPhase::FadeOut {
            (self.frame_counter as f32) / (FADE_OUT_FRAMES as f32)
        } else {
            0.0
        }
    }

    // -- Animation helpers --

    /// Start playing a Nidorino animation table.
    fn start_anim(&mut self, anim: &'static [(i8, i8)]) {
        self.current_anim = anim;
        if anim.is_empty() {
            self.nidorino_anim_dy = 0;
            self.nidorino_anim_dx = 0;
            self.anim_state = AnimState::Idle;
        } else {
            // Apply first keyframe immediately (absolute offset)
            let (dy, dx) = anim[0];
            self.nidorino_anim_dy = dy as i32;
            self.nidorino_anim_dx = dx as i32;
            self.anim_state = AnimState::Playing {
                keyframe: 0,
                frames_left: ANIM_FRAME_DELAY,
            };
        }
    }

    /// Advance one frame of the current Nidorino animation.
    /// Returns true when the animation is complete (all keyframes exhausted).
    fn tick_anim(&mut self) -> bool {
        match self.anim_state {
            AnimState::Idle => true,
            AnimState::Playing {
                ref mut keyframe,
                ref mut frames_left,
            } => {
                *frames_left -= 1;
                if *frames_left == 0 {
                    *keyframe += 1;
                    if *keyframe >= self.current_anim.len() {
                        self.anim_state = AnimState::Idle;
                        return true;
                    }
                    // Set absolute offset from new keyframe (not cumulative)
                    let (dy, dx) = self.current_anim[*keyframe];
                    self.nidorino_anim_dy = dy as i32;
                    self.nidorino_anim_dx = dx as i32;
                    *frames_left = ANIM_FRAME_DELAY;
                }
                false
            }
            AnimState::Waiting {
                ref mut frames_left,
            } => {
                *frames_left -= 1;
                *frames_left == 0
            }
        }
    }

    /// Advance one frame. Returns what action to take.
    ///
    /// `any_button_pressed`: true if the player pressed any button this frame.
    pub fn update_frame(&mut self, any_button_pressed: bool) -> ScreenAction {
        // Clear SFX event from previous frame
        self.sfx_event = IntroSfxEvent::None;

        // Skip to title screen on any button press
        if any_button_pressed {
            return ScreenAction::Transition(GameScreen::TitleScreen);
        }

        self.frame_counter += 1;

        match self.phase {
            IntroPhase::MoveNidorinoRight => self.update_move_right(),
            IntroPhase::HipHop1 => self.update_hiphop(WAIT_AFTER_HIPHOP1, IntroPhase::HipHop2),
            IntroPhase::HipHop2 => self.update_hiphop(WAIT_AFTER_HIPHOP2, IntroPhase::GengarRaise),
            IntroPhase::GengarRaise => self.update_gengar_raise(),
            IntroPhase::GengarSlash => self.update_gengar_slash(),
            IntroPhase::GengarRetreat => self.update_gengar_retreat(),
            IntroPhase::HipHop3 => self.update_hiphop3(),
            IntroPhase::Crouch => self.update_crouch(),
            IntroPhase::Lunge => self.update_lunge(),
            IntroPhase::FadeOut => self.update_fade_out(),
            IntroPhase::Done => ScreenAction::Transition(GameScreen::TitleScreen),
        }
    }

    fn enter_phase(&mut self, phase: IntroPhase) {
        self.phase = phase;
        self.frame_counter = 0;
        self.anim_state = AnimState::Idle;
        self.gengar_move_state = GengarMoveState::Moving {
            steps_left: 0,
            delay: 0,
        };
    }

    // -- Phase 1: MoveNidorinoRight --
    // ASM: MOVE_NIDORINO_RIGHT — each step: wBaseCoordX += 2, hSCX += 2, delay 2 frames.
    // 40 steps total. Nidorino OAM and Gengar background scroll together.
    fn update_move_right(&mut self) -> ScreenAction {
        if self.move_delay > 0 {
            self.move_delay -= 1;
            return ScreenAction::Continue;
        }
        if self.move_step >= MOVE_RIGHT_STEPS {
            self.nidorino_base_x = NIDORINO_FINAL_BASE_X;
            self.enter_hiphop_phase(IntroPhase::HipHop1);
            return ScreenAction::Continue;
        }
        self.nidorino_base_x += MOVE_RIGHT_SPEED;
        self.scroll_x += MOVE_RIGHT_SPEED;
        self.move_step += 1;
        self.move_delay = MOVE_STEP_DELAY;
        ScreenAction::Continue
    }

    // -- Phases 2 & 3: HipHop1 / HipHop2 --
    // ASM: SFX_Intro_Hip → Anim1, SFX_Intro_Hop → Anim2, then wait.
    // These share logic; `wait_frames` and `next_phase` differ.
    fn enter_hiphop_phase(&mut self, phase: IntroPhase) {
        self.enter_phase(phase);
        self.sfx_event = IntroSfxEvent::IntroHip;
        self.start_anim(ANIM_1);
        self.nidorino_sprite_set = 0;
    }

    fn update_hiphop(&mut self, wait_frames: u32, next_phase: IntroPhase) -> ScreenAction {
        match self.anim_state {
            AnimState::Idle => {
                // Anim1 finished → start Anim2 with Hop SFX
                // Or Anim2 finished → enter wait
                // We track which anim by checking current_anim pointer
                if core::ptr::eq(self.current_anim, ANIM_1) {
                    self.sfx_event = IntroSfxEvent::IntroHop;
                    self.start_anim(ANIM_2);
                } else {
                    self.anim_state = AnimState::Waiting {
                        frames_left: wait_frames,
                    };
                }
            }
            AnimState::Playing { .. } => {
                self.tick_anim();
            }
            AnimState::Waiting {
                ref mut frames_left,
            } => {
                *frames_left -= 1;
                if *frames_left == 0 {
                    match next_phase {
                        IntroPhase::HipHop2 => {
                            self.enter_hiphop_phase(IntroPhase::HipHop2);
                        }
                        _ => {
                            self.enter_phase(next_phase);
                            self.gengar_pose = GengarPose::RaiseArm;
                            self.sfx_event = IntroSfxEvent::IntroRaise;
                            self.gengar_move_state = GengarMoveState::Moving {
                                steps_left: GENGAR_RAISE_STEPS,
                                delay: MOVE_STEP_DELAY,
                            };
                        }
                    }
                }
            }
        }
        ScreenAction::Continue
    }

    // -- Phase 4: GengarRaise --
    // ASM: pose=RaiseArm, SFX Raise, move Gengar left 4 steps, wait 30.
    fn update_gengar_raise(&mut self) -> ScreenAction {
        match self.gengar_move_state {
            GengarMoveState::Moving {
                ref mut steps_left,
                ref mut delay,
            } => {
                if *delay > 0 {
                    *delay -= 1;
                } else if *steps_left > 0 {
                    self.scroll_x += GENGAR_SCROLL_SPEED;
                    *steps_left -= 1;
                    *delay = MOVE_STEP_DELAY;
                } else {
                    self.gengar_move_state = GengarMoveState::Waiting {
                        frames_left: WAIT_AFTER_GENGAR_RAISE,
                    };
                }
            }
            GengarMoveState::Animating => {}
            GengarMoveState::Waiting {
                ref mut frames_left,
            } => {
                *frames_left -= 1;
                if *frames_left == 0 {
                    self.enter_phase(IntroPhase::GengarSlash);
                    self.gengar_pose = GengarPose::Slash;
                    self.sfx_event = IntroSfxEvent::IntroCrash;
                    self.gengar_move_state = GengarMoveState::Moving {
                        steps_left: GENGAR_SLASH_STEPS,
                        delay: MOVE_STEP_DELAY,
                    };
                }
            }
        }
        ScreenAction::Continue
    }

    // -- Phase 5: GengarSlash --
    // ASM: pose=Slash, SFX Crash, scroll right 8 steps,
    //      then SFX Hip, sprite_set=1, Anim3, wait 30.
    fn update_gengar_slash(&mut self) -> ScreenAction {
        match self.gengar_move_state {
            GengarMoveState::Moving {
                ref mut steps_left,
                ref mut delay,
            } => {
                if *delay > 0 {
                    *delay -= 1;
                } else if *steps_left > 0 {
                    self.scroll_x -= GENGAR_SCROLL_SPEED;
                    *steps_left -= 1;
                    *delay = MOVE_STEP_DELAY;
                } else {
                    self.sfx_event = IntroSfxEvent::IntroHip;
                    self.nidorino_sprite_set = 1;
                    self.start_anim(ANIM_3);
                    self.gengar_move_state = GengarMoveState::Animating;
                }
            }
            GengarMoveState::Animating => {
                if self.tick_anim() {
                    self.gengar_move_state = GengarMoveState::Waiting {
                        frames_left: WAIT_AFTER_GENGAR_SLASH,
                    };
                }
            }
            GengarMoveState::Waiting {
                ref mut frames_left,
            } => {
                *frames_left -= 1;
                if *frames_left == 0 {
                    self.enter_phase(IntroPhase::GengarRetreat);
                    self.gengar_move_state = GengarMoveState::Moving {
                        steps_left: GENGAR_RETREAT_STEPS,
                        delay: MOVE_STEP_DELAY,
                    };
                }
            }
        }
        ScreenAction::Continue
    }

    // -- Phase 6: GengarRetreat --
    // ASM: scroll left 4 steps, pose=Idle, wait 60.
    fn update_gengar_retreat(&mut self) -> ScreenAction {
        match self.gengar_move_state {
            GengarMoveState::Moving {
                ref mut steps_left,
                ref mut delay,
            } => {
                if *delay > 0 {
                    *delay -= 1;
                } else if *steps_left > 0 {
                    self.scroll_x += GENGAR_SCROLL_SPEED;
                    *steps_left -= 1;
                    *delay = MOVE_STEP_DELAY;
                } else {
                    self.gengar_pose = GengarPose::Idle;
                    self.gengar_move_state = GengarMoveState::Waiting {
                        frames_left: WAIT_AFTER_GENGAR_RETREAT,
                    };
                }
            }
            GengarMoveState::Animating => {}
            GengarMoveState::Waiting {
                ref mut frames_left,
            } => {
                *frames_left -= 1;
                if *frames_left == 0 {
                    self.enter_phase(IntroPhase::HipHop3);
                    self.nidorino_sprite_set = 0;
                    self.nidorino_anim_dx = 0;
                    self.nidorino_anim_dy = 0;
                    self.sfx_event = IntroSfxEvent::IntroHip;
                    self.start_anim(ANIM_4);
                }
            }
        }
        ScreenAction::Continue
    }

    // -- Phase 7: HipHop3 --
    // ASM: sprite_set=0, SFX Hip → Anim4, SFX Hop → Anim5, wait 20.
    fn update_hiphop3(&mut self) -> ScreenAction {
        match self.anim_state {
            AnimState::Idle => {
                if core::ptr::eq(self.current_anim, ANIM_4) {
                    self.sfx_event = IntroSfxEvent::IntroHop;
                    self.start_anim(ANIM_5);
                } else {
                    self.anim_state = AnimState::Waiting {
                        frames_left: WAIT_AFTER_HIPHOP3,
                    };
                }
            }
            AnimState::Playing { .. } => {
                self.tick_anim();
            }
            AnimState::Waiting {
                ref mut frames_left,
            } => {
                *frames_left -= 1;
                if *frames_left == 0 {
                    self.enter_phase(IntroPhase::Crouch);
                    self.nidorino_sprite_set = 1;
                    self.nidorino_anim_dx = 0;
                    self.nidorino_anim_dy = 0;
                    self.start_anim(ANIM_6);
                }
            }
        }
        ScreenAction::Continue
    }

    // -- Phase 8: Crouch --
    // ASM: sprite_set=1, Anim6, wait 30.
    fn update_crouch(&mut self) -> ScreenAction {
        match self.anim_state {
            AnimState::Idle => {
                self.anim_state = AnimState::Waiting {
                    frames_left: WAIT_AFTER_CROUCH,
                };
            }
            AnimState::Playing { .. } => {
                self.tick_anim();
            }
            AnimState::Waiting {
                ref mut frames_left,
            } => {
                *frames_left -= 1;
                if *frames_left == 0 {
                    self.enter_phase(IntroPhase::Lunge);
                    self.sfx_event = IntroSfxEvent::IntroLunge;
                    self.nidorino_sprite_set = 2;
                    self.nidorino_anim_dx = 0;
                    self.nidorino_anim_dy = 0;
                    self.start_anim(ANIM_7);
                }
            }
        }
        ScreenAction::Continue
    }

    // -- Phase 9: Lunge --
    // ASM: SFX Lunge, sprite_set=2, Anim7. When done → FadeOut.
    fn update_lunge(&mut self) -> ScreenAction {
        match self.anim_state {
            AnimState::Idle => {
                self.enter_phase(IntroPhase::FadeOut);
            }
            AnimState::Playing { .. } => {
                self.tick_anim();
            }
            AnimState::Waiting { .. } => {
                self.enter_phase(IntroPhase::FadeOut);
            }
        }
        ScreenAction::Continue
    }

    // -- Phase 10: FadeOut --
    // 24 frames (3 palette steps × 8 frames), then transition.
    fn update_fade_out(&mut self) -> ScreenAction {
        if self.frame_counter >= FADE_OUT_FRAMES {
            self.phase = IntroPhase::Done;
            return ScreenAction::Transition(GameScreen::TitleScreen);
        }
        ScreenAction::Continue
    }
}

impl Default for IntroSceneState {
    fn default() -> Self {
        Self::new()
    }
}
