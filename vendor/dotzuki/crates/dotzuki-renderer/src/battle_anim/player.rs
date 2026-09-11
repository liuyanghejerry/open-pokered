use crate::sprite::SpriteOamEntry;

use super::*;

// ─── Animation player ────────────────────────────────────────────────

/// State of the subanimation playback within a single AnimCommand::SubAnim.
#[derive(Debug, Clone)]
struct SubAnimState {
    /// The SubAnimation being played.
    subanim_id: u8,
    /// Resolved transform type (after Enemy resolution).
    transform: SubAnimTransform,
    /// Current frame index within the subanimation.
    frame_index: usize,
    /// Total number of frames.
    num_frames: usize,
    /// Per-frame delay in ticks (from wSubAnimFrameDelay).
    delay: u8,
    /// Whether we're waiting for the caller to apply the delay.
    waiting_for_delay: bool,
    /// Tileset index (0 or 1) for this subanimation.
    tileset: u8,
    /// Move id whose sound this subanimation plays (0 = NO_MOVE).
    sound_id: u8,
    /// Whether the sound still needs to be reported (first frame only —
    /// PlaySubanimation plays the sound once when the subanimation starts).
    sound_pending: bool,
}

/// Plays a move animation, stepping through its command sequence.
///
/// The player is a state machine:
///   1. `start(move_id)` loads the move's command list.
///   2. `tick()` advances one step and returns what happened.
///   3. After each tick, `oam_entries()` has the current sprite data.
///
/// The caller is responsible for:
///   - Rendering OAM entries to the screen each frame.
///   - Applying `AnimEffect` actions (palette changes, screen shake, etc.).
///   - Counting down `WaitDelay` frames before calling tick again.
#[derive(Debug, Clone)]
pub struct AnimationPlayer {
    /// The move animation ID (1-based, matching wAnimationID).
    move_id: usize,
    /// Whether the player's mon is the attacker (affects Enemy transform).
    player_is_attacker: bool,
    /// Index into the current move animation's command list.
    command_index: usize,
    /// Total number of commands in this move animation.
    num_commands: usize,
    /// Current subanimation playback state (Some while playing a SubAnim command).
    subanim_state: Option<SubAnimState>,
    /// Accumulated OAM entries for the current frame.
    oam_buffer: Vec<SpriteOamEntry>,
    /// Whether the animation is finished.
    finished: bool,
}

impl AnimationPlayer {
    /// Create a new idle animation player.
    pub fn new() -> Self {
        Self {
            move_id: 0,
            player_is_attacker: true,
            command_index: 0,
            num_commands: 0,
            subanim_state: None,
            oam_buffer: Vec::with_capacity(40),
            finished: true,
        }
    }

    /// Start playing the animation for the given move.
    /// `move_id` is 0-based index into MOVE_ANIM_DATA.
    /// `player_is_attacker` determines Enemy transform resolution.
    pub fn start(&mut self, move_id: usize, player_is_attacker: bool) {
        // On the enemy's turn, AMNESIA plays CONF_ANIM and REST plays
        // SLP_ANIM instead.
        // Animation ids are 1-based (MOVE_ANIM_DATA index + 1).
        let move_id = if !player_is_attacker {
            match move_id + 1 {
                id if id == AMNESIA as usize => CONF_ANIM as usize - 1,
                id if id == REST as usize => SLP_ANIM as usize - 1,
                _ => move_id,
            }
        } else {
            move_id
        };

        self.move_id = move_id;
        self.player_is_attacker = player_is_attacker;
        self.command_index = 0;
        self.subanim_state = None;
        self.oam_buffer.clear();
        self.finished = false;

        if move_id < NUM_MOVE_ANIMS {
            self.num_commands = MOVE_ANIM_DATA[move_id].len();
        } else {
            self.num_commands = 0;
            self.finished = true;
        }
    }

    /// Whether the animation has finished playing.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Get the current OAM buffer for rendering.
    pub fn oam_entries(&self) -> &[SpriteOamEntry] {
        &self.oam_buffer
    }

    /// Current move-animation tileset index (0/1/2) while a subanimation is active.
    pub fn current_tileset(&self) -> Option<u8> {
        self.subanim_state.as_ref().map(|s| s.tileset)
    }

    /// Decode a raw command tuple from MOVE_ANIM_DATA into an AnimCommand.
    pub fn decode_command(raw: &(u8, u8, u8, u8)) -> AnimCommand {
        let (kind, sound_val, id_val, packed) = *raw;
        if kind == 0 {
            // SubAnim command
            AnimCommand::SubAnim {
                sound_id: sound_val,
                subanim_id: id_val,
                tileset: packed >> 6,
                delay: packed & 0x3F,
            }
        } else {
            // Effect command
            AnimCommand::Effect {
                sound_id: sound_val,
                effect: SpecialEffect::from_u8(id_val).unwrap_or(SpecialEffect::WavyScreen),
            }
        }
    }

    /// Resolve the effective transform for a subanimation, accounting for
    /// Enemy type and whose turn it is.
    pub fn resolve_transform(&self, raw_transform: SubAnimTransform) -> SubAnimTransform {
        match raw_transform {
            SubAnimTransform::Enemy => {
                if self.player_is_attacker {
                    // Player's turn + Enemy type → HFlip
                    SubAnimTransform::HFlip
                } else {
                    // Enemy's turn + Enemy type → Normal
                    SubAnimTransform::Normal
                }
            }
            other => {
                // For non-Enemy types:
                // If it's the player's turn, use Normal (override).
                // If it's the enemy's turn, use the specified type.
                // (from GetSubanimationTransform1)
                if self.player_is_attacker {
                    SubAnimTransform::Normal
                } else {
                    other
                }
            }
        }
    }
}

// ─── Frame block rendering ──────────────────────────────────────────

impl AnimationPlayer {
    /// Render a frame block into OAM entries, applying the given transform.
    ///
    /// `frame_block_id`: index into FRAME_BLOCK_DATA.
    /// `base_coord_id`: index into BASE_COORDS.
    /// `transform`: the resolved SubAnimTransform to apply.
    /// `dest`: output buffer to append OAM entries to.
    pub fn render_frame_block(
        frame_block_id: usize,
        base_coord_id: usize,
        transform: SubAnimTransform,
        dest: &mut Vec<SpriteOamEntry>,
    ) {
        if frame_block_id >= NUM_FRAMEBLOCKS || base_coord_id >= NUM_BASECOORDS {
            return;
        }

        let fb_data = FRAME_BLOCK_DATA[frame_block_id];
        let (base_y, base_x) = BASE_COORDS[base_coord_id];

        // Frame block offsets are pixel bytes (Y = y_tile*8 + y_px,
        // X = x_tile*8 + x_px), added to the base coordinate.
        for &(x_off, y_off, raw_tile, flags) in fb_data {
            let (screen_y, screen_x, tile_id, oam_flags) = match transform {
                SubAnimTransform::Normal | SubAnimTransform::Reverse => {
                    // No transformation — direct mapping.
                    // y = base_y + y_offset
                    // x = base_x + x_offset
                    let y = base_y as i32 + y_off as i32;
                    let x = base_x as i32 + x_off as i32;
                    let tile = raw_tile.wrapping_add(ANIM_BASE_TILE_ID);
                    (y, x, tile, flags)
                }

                SubAnimTransform::HvFlip => {
                    // Flip both H and V: mirror around (136, 168).
                    // y = 136 - (base_y + y_offset)
                    // x = 168 - (base_x + x_offset)
                    let y = 136i32 - (base_y as i32 + y_off as i32);
                    let x = 168i32 - (base_x as i32 + x_off as i32);
                    let tile = raw_tile.wrapping_add(ANIM_BASE_TILE_ID);
                    // Toggle flip flags: 0x00→0x60, 0x20→0x40, 0x40→0x20, 0x60→0x00
                    let new_flags = match flags & 0x60 {
                        0x00 => (flags & !0x60) | 0x60,
                        0x20 => (flags & !0x60) | 0x40,
                        0x40 => (flags & !0x60) | 0x20,
                        0x60 => flags & !0x60,
                        _ => flags, // unreachable, 0x60 mask covers all
                    };
                    (y, x, tile, new_flags)
                }

                SubAnimTransform::HFlip => {
                    // Flip horizontally + translate 40px down.
                    // y = base_y + y_offset + 40
                    // x = 168 - (base_x + x_offset)
                    let y = base_y as i32 + y_off as i32 + 40;
                    let x = 168i32 - (base_x as i32 + x_off as i32);
                    let tile = raw_tile.wrapping_add(ANIM_BASE_TILE_ID);
                    // Toggle X flip bit only
                    let new_flags = flags ^ OAM_XFLIP;
                    (y, x, tile, new_flags)
                }

                SubAnimTransform::CoordFlip => {
                    // Flip base coordinates, keep offsets normal.
                    // y = (136 - base_y) + y_offset
                    // x = (168 - base_x) + x_offset
                    let y = (136i32 - base_y as i32) + y_off as i32;
                    let x = (168i32 - base_x as i32) + x_off as i32;
                    let tile = raw_tile.wrapping_add(ANIM_BASE_TILE_ID);
                    (y, x, tile, flags)
                }

                SubAnimTransform::Enemy => {
                    // Should have been resolved before calling. Fall back to Normal.
                    let y = base_y as i32 + y_off as i32;
                    let x = base_x as i32 + x_off as i32;
                    let tile = raw_tile.wrapping_add(ANIM_BASE_TILE_ID);
                    (y, x, tile, flags)
                }
            };

            dest.push(SpriteOamEntry::new(screen_y, screen_x, tile_id, oam_flags));
        }
    }
}

// ─── Animation tick (state machine) ─────────────────────────────────

impl AnimationPlayer {
    /// Advance the animation by one step.
    ///
    /// Returns what happened this tick. The caller should:
    ///   - `Playing` → render `oam_entries()` to screen.
    ///   - `WaitDelay` → wait `frames` frames, then call tick again.
    ///   - `Effect` → apply the special effect, then call tick again.
    ///   - `Done` → animation is finished.
    /// `Playing`/`WaitDelay` also carry a `hook` when the current animation
    /// id has an entry in ANIMATION_ID_HOOKS (the original's
    /// DoSpecialEffectByAnimationId, run after every DrawFrameBlock).
    pub fn tick(&mut self) -> AnimTickResult {
        if self.finished {
            return AnimTickResult::Done;
        }

        // If we're in the middle of playing a subanimation, advance it.
        if let Some(ref mut state) = self.subanim_state {
            // Check if we're waiting for a delay from the previous frame.
            if state.waiting_for_delay {
                state.waiting_for_delay = false;
                // Proceed to render next frame (delay already waited by caller).
            }

            // Render next frame of the subanimation.
            let subanim = get_subanimation(state.subanim_id as usize);

            if state.frame_index < state.num_frames {
                let frame_idx = if state.transform == SubAnimTransform::Reverse {
                    // Reverse: play frames from last to first.
                    state.num_frames - 1 - state.frame_index
                } else {
                    state.frame_index
                };

                let frame = &subanim.frames[frame_idx];
                let mode = frame.mode;

                // Determine whether to clear OAM before drawing.
                if mode.cleans_oam() {
                    self.oam_buffer.clear();
                }

                // Render the frame block tiles.
                Self::render_frame_block(
                    frame.frame_block_id as usize,
                    frame.base_coord_id as usize,
                    state.transform,
                    &mut self.oam_buffer,
                );

                state.frame_index += 1;

                // DoSpecialEffectByAnimationId: after every drawn frame block,
                // run the hook for this animation id (if any). wSubAnimCounter
                // counts down from num_frames to 1 as frame blocks are drawn.
                let frame_hook = get_frame_hook(self.move_id as u8 + 1);
                let counter = (state.num_frames - state.frame_index) as u8 + 1;
                let hook = frame_hook.and_then(|h| h.effect_for_counter(counter));

                // DoGrowlSpecialEffects: copy OAM entries 0-3 to slots 4-7
                // (a second music note flying towards the defending mon).
                if frame_hook == Some(FrameHook::Growl) {
                    let copies: Vec<SpriteOamEntry> =
                        self.oam_buffer.iter().take(4).cloned().collect();
                    for (i, entry) in copies.into_iter().enumerate() {
                        if self.oam_buffer.len() > 4 + i {
                            self.oam_buffer[4 + i] = entry;
                        } else {
                            self.oam_buffer.push(entry);
                        }
                    }
                }

                // The command's sound is reported once, with its first result
                // (PlaySubanimation plays the sound before the frame loop).
                let sound = if state.sound_pending {
                    state.sound_pending = false;
                    (state.sound_id != 0).then_some(state.sound_id)
                } else {
                    None
                };

                // Apply delay based on mode.
                // In the original ASM, DelayFrames waits for wSubAnimFrameDelay VBlank frames.
                // We return WaitDelay to tell the caller to wait, then proceed on next tick.
                if mode.has_delay() && state.delay > 0 {
                    state.waiting_for_delay = true;
                    return AnimTickResult::WaitDelay {
                        frames: state.delay,
                        sound,
                        hook,
                    };
                }

                // Mode02: no delay, keep OAM, advance — immediately process next frame.
                return AnimTickResult::Playing { sound, hook };
            }

            // Subanimation finished — clear state and advance to next command.
            self.subanim_state = None;
            self.oam_buffer.clear();
        }

        // Process next command in the move animation.
        if self.command_index >= self.num_commands {
            self.finished = true;
            return AnimTickResult::Done;
        }

        let raw = &MOVE_ANIM_DATA[self.move_id][self.command_index];
        self.command_index += 1;
        let cmd = Self::decode_command(raw);

        match cmd {
            AnimCommand::SubAnim {
                sound_id,
                subanim_id,
                tileset,
                delay,
            } => {
                let subanim = get_subanimation(subanim_id as usize);
                let raw_transform = subanim.transform;
                let resolved = self.resolve_transform(raw_transform);
                let num_frames = subanim.frames.len();

                self.subanim_state = Some(SubAnimState {
                    subanim_id,
                    transform: resolved,
                    frame_index: 0,
                    num_frames,
                    delay,
                    waiting_for_delay: false,
                    tileset,
                    sound_id,
                    sound_pending: true,
                });

                // Immediately start rendering the first frame.
                self.tick()
            }

            AnimCommand::Effect { sound_id, effect } => AnimTickResult::Effect {
                sound: (sound_id != 0).then_some(sound_id),
                effect,
            },
        }
    }
}

// ─── Per-frame hook evaluation ────────────────────────────────────────

impl FrameHook {
    /// Effect fired after the frame block drawn with the given
    /// sub-animation frame counter value (counts down from the
    /// subanimation's frame count to 1). Returns None for hooks the player
    /// handles internally (Growl) or cannot reproduce (ball / trade flow —
    /// see the FrameHook variant docs).
    pub fn effect_for_counter(self, counter: u8) -> Option<AnimEffect> {
        // Flash effect: inverted palette for 2 frames, then white for 2.
        const FLASH: AnimEffect = AnimEffect::FlashScreen { frames: 4 };
        match self {
            FrameHook::FlashScreen => Some(FLASH),
            FrameHook::FlashScreenEveryFour => (counter % 4 == 0).then_some(FLASH),
            FrameHook::FlashScreenEveryEight => (counter % 8 == 0).then_some(FLASH),
            FrameHook::BlizzardFlash => matches!(counter, 13 | 9 | 5 | 1).then_some(FLASH),
            FrameHook::Explode => {
                if counter == 1 {
                    // At the end of the subanimation, hide the attacking
                    // mon's pic.
                    Some(AnimEffect::HidePlayerMon)
                } else {
                    (counter % 4 == 0).then_some(FLASH)
                }
            }
            FrameHook::RockSlide => match counter {
                // Shake the screen horizontally then vertically, both with
                // intensity 1.
                8..=11 => Some(AnimEffect::ShakeScreenHV {
                    pixels: 1,
                    frames: 9,
                }),
                1 => Some(FLASH),
                _ => None,
            },
            FrameHook::Growl
            | FrameHook::TailWhipUnused
            | FrameHook::BallToss
            | FrameHook::BallShake
            | FrameHook::BallPoof
            | FrameHook::TradeHideMonster
            | FrameHook::TradeShakeBall
            | FrameHook::TradeJumpBall => None,
        }
    }
}

// ─── Special effect mapping ─────────────────────────────────────────

impl AnimationPlayer {
    /// Map a SpecialEffect to a high-level AnimEffect the caller should apply.
    ///
    /// This translates the original ASM effect handlers into abstract operations.
    /// The caller's rendering layer is responsible for executing these.
    pub fn apply_effect(effect: SpecialEffect) -> AnimEffect {
        match effect {
            SpecialEffect::WavyScreen => AnimEffect::WavyScreen,
            SpecialEffect::SubstituteMon => AnimEffect::SubstituteMon,
            SpecialEffect::ShakeBackAndForth => AnimEffect::ShakeBackAndForth,
            SpecialEffect::SlideEnemyMonOff => AnimEffect::SlideEnemyMonOff,
            SpecialEffect::ShowEnemyMonPic => AnimEffect::ShowEnemyMon,
            SpecialEffect::ShowMonPic => AnimEffect::ShowPlayerMon,
            SpecialEffect::BlinkEnemyMon => AnimEffect::BlinkEnemyMon { times: 6 },
            SpecialEffect::HideEnemyMonPic => AnimEffect::HideEnemyMon,
            SpecialEffect::FlashEnemyMonPic => AnimEffect::FlashEnemyMonPic,
            SpecialEffect::DelayAnimation10 => AnimEffect::Delay10,
            SpecialEffect::SpiralBallsInward => AnimEffect::SpiralBallsInward,
            SpecialEffect::ShakeEnemyHud2 => AnimEffect::ShakeEnemyHud { variant: 2 },
            SpecialEffect::ShakeEnemyHud => AnimEffect::ShakeEnemyHud { variant: 1 },
            SpecialEffect::SlideMonHalfOff => AnimEffect::SlidePlayerMonHalfOff,
            SpecialEffect::PetalsFalling => AnimEffect::PetalsFalling,
            SpecialEffect::LeavesFalling => AnimEffect::LeavesFalling,
            SpecialEffect::TransformMon => AnimEffect::TransformMon,
            SpecialEffect::SlideMonDownAndHide => AnimEffect::SlidePlayerMonDownAndHide,
            SpecialEffect::MinimizeMon => AnimEffect::MinimizeMon,
            SpecialEffect::BounceUpAndDown => AnimEffect::BounceUpAndDown,
            SpecialEffect::ShootManyBallsUpward => AnimEffect::ShootBallsUpward { many: true },
            SpecialEffect::ShootBallsUpward => AnimEffect::ShootBallsUpward { many: false },
            SpecialEffect::SquishMonPic => AnimEffect::SquishMonPic,
            SpecialEffect::HideMonPic => AnimEffect::HidePlayerMon,
            SpecialEffect::LightScreenPalette => AnimEffect::LightScreenPalette,
            SpecialEffect::ResetMonPosition => AnimEffect::ResetPlayerMonPosition,
            SpecialEffect::MoveMonHorizontally => AnimEffect::MovePlayerMonH,
            SpecialEffect::BlinkMon => AnimEffect::BlinkPlayerMon { times: 6 },
            // AnimationSlideMonOff: e = 8 slides the whole 7-tile-wide mon
            // pic off the screen (AnimationSlideMonHalfOff uses e = 4).
            SpecialEffect::SlideMonOff => AnimEffect::SlidePlayerMonOff,
            SpecialEffect::FlashMonPic => AnimEffect::FlashPlayerMonPic,
            SpecialEffect::SlideMonDown => AnimEffect::SlidePlayerMonDown,
            SpecialEffect::SlideMonUp => AnimEffect::SlidePlayerMonUp,
            // AnimationFlashScreenLong: 3 cycles through a 12-palette table
            // ("flashes the screen for an extended period (48 frames)").
            SpecialEffect::FlashScreenLong => AnimEffect::FlashScreen { frames: 48 },
            SpecialEffect::DarkenMonPalette => AnimEffect::DarkenMonPalette,
            SpecialEffect::WaterDropletsEverywhere => AnimEffect::WaterDroplets,
            // AnimationShakeScreen: PredefShakeScreenHorizontally with b = 8 —
            // amplitude decays 8→1 px at ~9 frames per step (≈72 frames).
            SpecialEffect::ShakeScreen => AnimEffect::ShakeScreenH {
                pixels: 8,
                frames: 72,
            },
            SpecialEffect::ResetScreenPalette => AnimEffect::ResetScreenPalette,
            SpecialEffect::DarkScreenPalette => AnimEffect::DarkScreenPalette,
            // AnimationFlashScreen: inverted palette 2 frames + white 2 frames.
            SpecialEffect::DarkScreenFlash => AnimEffect::FlashScreen { frames: 4 },
        }
    }
}

impl Default for AnimationPlayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_animation(move_id: usize) -> Vec<AnimTickResult> {
        run_animation_as(move_id, true)
    }

    fn run_animation_as(move_id: usize, player_is_attacker: bool) -> Vec<AnimTickResult> {
        let mut player = AnimationPlayer::new();
        player.start(move_id, player_is_attacker);
        let mut results = Vec::new();
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 10000;

        loop {
            if iterations >= MAX_ITERATIONS {
                break;
            }
            iterations += 1;

            let result = player.tick();
            results.push(result.clone());

            match result {
                AnimTickResult::Done => break,
                AnimTickResult::WaitDelay { frames: n, .. } => {
                    for _ in 0..n {
                        let delay_result = player.tick();
                        results.push(delay_result.clone());
                        iterations += 1;
                    }
                }
                _ => {}
            }
        }
        results
    }

    #[test]
    fn pound_animation_has_frames() {
        let results = run_animation(0x00);
        let has_frames = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Playing { .. } | AnimTickResult::WaitDelay { .. }
            )
        });
        assert!(has_frames, "Pound should have animation frames");
    }

    #[test]
    fn earthquake_has_screen_shake() {
        let results = run_animation(0x58);
        let shake_count = results
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    AnimTickResult::Effect {
                        effect: SpecialEffect::ShakeScreen,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(shake_count, 2, "Earthquake should have 2 screen shakes");
    }

    #[test]
    fn thunder_punch_has_palette_effects() {
        let results = run_animation(0x08);
        let dark_palette = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::DarkScreenPalette,
                    ..
                }
            )
        });
        let reset_palette = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::ResetScreenPalette,
                    ..
                }
            )
        });
        assert!(dark_palette, "ThunderPunch should have dark screen palette");
        assert!(
            reset_palette,
            "ThunderPunch should have reset screen palette"
        );
    }

    #[test]
    fn selfdestruct_has_explosion() {
        let results = run_animation(0x77);
        let has_frames = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Playing { .. } | AnimTickResult::WaitDelay { .. }
            )
        });
        assert!(has_frames, "Selfdestruct should have explosion frames");
    }

    #[test]
    fn splash_has_bounce() {
        let results = run_animation(0x95);
        let bounce = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::BounceUpAndDown,
                    ..
                }
            )
        });
        assert!(bounce, "Splash should have bounce effect");
    }

    #[test]
    fn teleport_has_squish_and_balls() {
        let results = run_animation(0x63);
        let squish = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::SquishMonPic,
                    ..
                }
            )
        });
        let balls = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::ShootBallsUpward,
                    ..
                }
            )
        });
        assert!(squish, "Teleport should have squish effect");
        assert!(balls, "Teleport should have shoot balls upward");
    }

    #[test]
    fn acid_armor_slides_down() {
        let results = run_animation(0x96);
        let slide = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::SlideMonDownAndHide,
                    ..
                }
            )
        });
        assert!(slide, "Acid Armor should have slide down and hide");
    }

    #[test]
    fn minimize_has_minimize_effect() {
        let results = run_animation(0x6A);
        let minimize = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::MinimizeMon,
                    ..
                }
            )
        });
        assert!(minimize, "Minimize should have minimize effect");
    }

    #[test]
    fn transform_has_transform_effect() {
        let results = run_animation(0x8F);
        let transform = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::TransformMon,
                    ..
                }
            )
        });
        assert!(transform, "Transform should have transform effect");
    }

    #[test]
    fn substitute_has_substitute_effect() {
        let results = run_animation(0xA3);
        let substitute = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::SubstituteMon,
                    ..
                }
            )
        });
        assert!(substitute, "Substitute should have substitute effect");
    }

    #[test]
    fn double_team_has_shake() {
        let results = run_animation(0x67);
        let shake = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::ShakeBackAndForth,
                    ..
                }
            )
        });
        assert!(shake, "Double Team should have shake back and forth");
    }

    #[test]
    fn recover_has_blink() {
        let results = run_animation(0x68);
        let blink = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::BlinkMon,
                    ..
                }
            )
        });
        assert!(blink, "Recover should have blink effect");
    }

    #[test]
    fn whirlwind_slides_enemy_off() {
        let results = run_animation(0x11);
        let slide = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::SlideEnemyMonOff,
                    ..
                }
            )
        });
        assert!(slide, "Whirlwind should slide enemy off");
    }

    #[test]
    fn dig_slides_mon_up() {
        let results = run_animation(0x5A);
        let slide = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::SlideMonUp,
                    ..
                }
            )
        });
        assert!(slide, "Dig should slide mon up");
    }

    #[test]
    fn confusion_has_flash() {
        let results = run_animation(0x5C);
        let flash = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::FlashScreenLong,
                    ..
                }
            )
        });
        assert!(flash, "Confusion should have screen flash");
    }

    #[test]
    fn psychic_has_flash_and_wavy() {
        let results = run_animation(0x5D);
        let flash = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::FlashScreenLong,
                    ..
                }
            )
        });
        let wavy = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::WavyScreen,
                    ..
                }
            )
        });
        assert!(flash, "Psychic should have screen flash");
        assert!(wavy, "Psychic should have wavy screen");
    }

    #[test]
    fn hyper_beam_has_complex_sequence() {
        let results = run_animation(0x3E);
        let dark_palette = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::DarkScreenPalette,
                    ..
                }
            )
        });
        let spiral = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::SpiralBallsInward,
                    ..
                }
            )
        });
        let reset = results.iter().any(|r| {
            matches!(
                r,
                AnimTickResult::Effect {
                    effect: SpecialEffect::ResetScreenPalette,
                    ..
                }
            )
        });
        assert!(dark_palette, "Hyper Beam should have dark palette");
        assert!(spiral, "Hyper Beam should have spiral balls");
        assert!(reset, "Hyper Beam should have reset palette");
    }

    #[test]
    fn slide_mon_off_is_full_slide() {
        // AnimationSlideMonOff: e = 8 slides the whole 7-tile-wide pic off
        // screen, unlike AnimationSlideMonHalfOff (e = 4).
        assert_eq!(
            AnimationPlayer::apply_effect(SpecialEffect::SlideMonOff),
            AnimEffect::SlidePlayerMonOff
        );
        assert_eq!(
            AnimationPlayer::apply_effect(SpecialEffect::SlideMonHalfOff),
            AnimEffect::SlidePlayerMonHalfOff
        );
    }

    #[test]
    fn flash_screen_long_and_shake_screen_params() {
        // AnimationFlashScreenLong: 3 cycles through a 12-palette table
        // ("an extended period (48 frames)").
        assert_eq!(
            AnimationPlayer::apply_effect(SpecialEffect::FlashScreenLong),
            AnimEffect::FlashScreen { frames: 48 }
        );
        // AnimationShakeScreen: PredefShakeScreenHorizontally with b = 8.
        assert_eq!(
            AnimationPlayer::apply_effect(SpecialEffect::ShakeScreen),
            AnimEffect::ShakeScreenH {
                pixels: 8,
                frames: 72
            }
        );
    }

    #[test]
    fn share_move_animations_enemy_turn() {
        // ShareMoveAnimations: on the enemy's turn AMNESIA plays CONF_ANIM
        // and REST plays SLP_ANIM instead.
        assert_eq!(
            run_animation_as(AMNESIA as usize - 1, false),
            run_animation_as(CONF_ANIM as usize - 1, false)
        );
        assert_eq!(
            run_animation_as(REST as usize - 1, false),
            run_animation_as(SLP_ANIM as usize - 1, false)
        );
    }

    #[test]
    fn share_move_animations_player_turn_unchanged() {
        // On the player's turn the animation is NOT replaced: AMNESIA uses
        // SUBANIM_0_STATUS_CONFUSED (base coords 0x71+), while the enemy-turn
        // CONF_ANIM replacement uses SUBANIM_0_STATUS_CONFUSED_ENEMY (0x01+).
        let mut player = AnimationPlayer::new();
        player.start(AMNESIA as usize - 1, true);
        player.tick();
        // SpriteOamEntry has no PartialEq; compare positions.
        let player_turn_oam: Vec<(i32, i32)> =
            player.oam_entries().iter().map(|e| (e.y, e.x)).collect();

        let mut enemy = AnimationPlayer::new();
        enemy.start(AMNESIA as usize - 1, false);
        enemy.tick();
        let enemy_turn_oam: Vec<(i32, i32)> =
            enemy.oam_entries().iter().map(|e| (e.y, e.x)).collect();
        assert_ne!(player_turn_oam, enemy_turn_oam);
    }

    /// Extract the per-frame hook effect from a tick result, if any.
    fn hook_of(r: &AnimTickResult) -> Option<&AnimEffect> {
        match r {
            AnimTickResult::Playing { hook, .. } | AnimTickResult::WaitDelay { hook, .. } => {
                hook.as_ref()
            }
            _ => None,
        }
    }

    #[test]
    fn hyper_beam_hook_flashes_every_four_frame_blocks() {
        // FlashScreenEveryFourFrameBlocks: flash when the subanimation
        // counter is divisible by 4. Subanim_0Beam has 14 frames → counters
        // 12, 8, 4; Subanim_1StarBigMoving has 3 frames → no flashes.
        let results = run_animation(0x3E);
        let flashes = results
            .iter()
            .filter(|r| matches!(hook_of(r), Some(AnimEffect::FlashScreen { .. })))
            .count();
        assert_eq!(flashes, 3, "Hyper Beam should flash 3 times via the hook");
    }

    #[test]
    fn selfdestruct_hook_flashes_and_hides_mon() {
        // DoExplodeSpecialEffects: flash every 4 frame blocks (Subanim_1Selfdestruct
        // has 21 frames → counters 20, 16, 12, 8, 4) and hide the attacking
        // mon at counter 1.
        let results = run_animation(0x77);
        let flashes = results
            .iter()
            .filter(|r| matches!(hook_of(r), Some(AnimEffect::FlashScreen { .. })))
            .count();
        assert_eq!(flashes, 5, "Selfdestruct should flash 5 times via the hook");
        let hides = results
            .iter()
            .filter(|r| matches!(hook_of(r), Some(AnimEffect::HidePlayerMon)))
            .count();
        assert_eq!(hides, 1, "Selfdestruct should hide the attacking mon once");
    }

    #[test]
    fn rock_slide_hook_shakes_and_flashes() {
        // DoRockSlideSpecialEffects: shake H+V at counters 8..=11
        // (Subanim_0RocksLift has 15 frames → 4 shakes) and flash at
        // counter 1 of every subanimation (RocksLift, RocksToss and
        // StarBigMoving → 3 flashes).
        let results = run_animation(0x9C);
        let shakes = results
            .iter()
            .filter(|r| matches!(hook_of(r), Some(AnimEffect::ShakeScreenHV { .. })))
            .count();
        assert_eq!(shakes, 4, "Rock Slide should shake 4 times via the hook");
        let flashes = results
            .iter()
            .filter(|r| matches!(hook_of(r), Some(AnimEffect::FlashScreen { .. })))
            .count();
        assert_eq!(flashes, 3, "Rock Slide should flash 3 times via the hook");
    }

    #[test]
    fn growl_hook_duplicates_oam_entries() {
        // DoGrowlSpecialEffects copies OAM entries 0-3 to slots 4-7 after
        // every frame block (FrameBlock17 = 4 tiles → 8 entries per frame).
        let mut player = AnimationPlayer::new();
        player.start(0x2C, true); // GROWL
        player.tick();
        assert_eq!(player.oam_entries().len(), 8);
    }

    #[test]
    fn sound_id_reported_on_first_frame_only() {
        // Pound: battle_anim POUND, SUBANIM_0_STAR_TWICE, 0, 8 — the sound
        // (POUND = 1) plays once when the subanimation starts.
        let mut player = AnimationPlayer::new();
        player.start(0x00, true);
        match player.tick() {
            AnimTickResult::Playing { sound, .. } | AnimTickResult::WaitDelay { sound, .. } => {
                assert_eq!(sound, Some(1));
            }
            other => panic!("Expected Playing/WaitDelay, got {:?}", other),
        }
        match player.tick() {
            AnimTickResult::Playing { sound, .. } | AnimTickResult::WaitDelay { sound, .. } => {
                assert_eq!(sound, None);
            }
            other => panic!("Expected Playing/WaitDelay, got {:?}", other),
        }
    }

    #[test]
    fn effect_command_reports_sound() {
        // Tackle: battle_anim LEECH_SEED, SE_MOVE_MON_HORIZONTALLY —
        // the effect command carries LEECH_SEED's sound (73 = 0x49).
        let mut player = AnimationPlayer::new();
        player.start(0x20, true);
        match player.tick() {
            AnimTickResult::Effect { sound, .. } => {
                assert_eq!(sound, Some(73));
            }
            other => panic!("Expected Effect, got {:?}", other),
        }
    }

    #[test]
    fn all_moves_produce_animations() {
        for move_id in 0..203 {
            let mut player = AnimationPlayer::new();
            player.start(move_id, true);
            let mut has_frames = false;
            let mut iterations = 0;

            for _ in 0..1000 {
                if iterations >= 1000 {
                    break;
                }
                iterations += 1;

                match player.tick() {
                    AnimTickResult::Done => break,
                    AnimTickResult::Playing { .. } => has_frames = true,
                    AnimTickResult::Effect { .. } => has_frames = true,
                    AnimTickResult::WaitDelay { frames: n, .. } => {
                        has_frames = true;
                        for _ in 0..n {
                            player.tick();
                            iterations += 1;
                        }
                    }
                }
            }
            assert!(
                has_frames,
                "Move 0x{:02X} should produce animation frames",
                move_id
            );
        }
    }
}
