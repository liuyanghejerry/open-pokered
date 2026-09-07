//! Shared opening-screen fades for graphical and terminal frontends.

use crate::FrameBuffer;
use dotzuki_renderer::transition::{FADE_PAL_6, FADE_PAL_7, FADE_PAL_8};

/// Apply the three GB fade-to-white steps to a completed frame.
///
/// Remap the display palette, preserving sprite transparency and background
/// occlusion. The intro's 24-frame fade holds each step for eight frames;
/// other durations retain their existing timing and divide it into thirds.
/// Call only during FadeOut, after drawing (clear resets the display palette).
pub fn apply_white_fade(fb: &mut FrameBuffer, frame: u32, duration: u32) {
    assert!(duration > 0);
    let step = ((u64::from(frame) * 3 / u64::from(duration)) as usize).min(2);
    fb.apply_bgp([FADE_PAL_6, FADE_PAL_7, FADE_PAL_8][step].bgp);
}
