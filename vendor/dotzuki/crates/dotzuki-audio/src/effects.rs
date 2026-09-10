//! Audio effect processors: vibrato, pitch slide, duty cycle rotation.
//!
//! These are applied per-frame to active channels, modifying frequency or
//! duty cycle based on the channel's current effect state.

use crate::{ChannelFlags1, ChannelState};

// ── Vibrato ──────────────────────────────────────────────────────────────

/// Apply vibrato to a channel's frequency.
///
/// Replicates the classic GB vibrato application.
/// Returns the modified **low byte only** to write to REG_FREQUENCY_LO.
///
/// The original routine only modifies the frequency low byte register,
/// never the high byte. On overflow the low byte clamps to 0xFF,
/// on underflow it clamps to 0x00. The stored `channel.frequency`
/// is never modified — vibrato is a pure hardware-level modulation.
///
/// The vibrato alternates between adding and subtracting from the base
/// frequency. The extent byte is split: upper nibble = upward amount,
/// lower nibble = downward amount.
pub fn apply_vibrato(channel: &mut ChannelState) -> Option<u8> {
    // If vibrato extent is zero, no vibrato
    if channel.vibrato.extent == 0 {
        return None;
    }

    // Delay phase: count down before vibrato starts
    if channel.vibrato.delay_counter > 0 {
        channel.vibrato.delay_counter -= 1;
        return None;
    }

    // Rate counter: tick down, only apply vibrato when it reaches zero
    let rate_counter = channel.vibrato.rate_counter();
    if rate_counter > 0 {
        channel.vibrato.set_rate_counter(rate_counter - 1);
        return None;
    }

    // Reload rate counter
    channel
        .vibrato
        .set_rate_counter(channel.vibrato.rate_reload());

    // Read the direction bit BEFORE toggling (matches the classic engine's
    // control flow: test the vibrato-direction bit, then branch).
    //   if set → clear → subtract (going down)
    //   if unset → set → add (going up)
    let going_down = channel.flags1.contains(ChannelFlags1::VIBRATO_DOWN);

    // Toggle direction for next tick
    channel.flags1.toggle(ChannelFlags1::VIBRATO_DOWN);

    let base_lo = channel.freq_lo_saved;

    let new_lo = if going_down {
        // Direction bit was SET → subtract lower nibble from base
        // (classic routine: subtract, clamping to 0 on underflow)
        let amount = channel.vibrato.extent_down();
        if base_lo >= amount {
            base_lo - amount
        } else {
            0x00 // clamp to 0 on underflow
        }
    } else {
        // Direction bit was UNSET → add upper nibble to base
        // (classic routine: add, clamping to $FF on overflow)
        let amount = channel.vibrato.extent_up();
        let sum = base_lo as u16 + amount as u16;
        if sum > 0xFF {
            0xFF // clamp to 0xFF on overflow
        } else {
            sum as u8
        }
    };

    Some(new_lo)
}

// ── Pitch Slide ──────────────────────────────────────────────────────────

/// Apply pitch slide to a channel.
///
/// Replicates the classic GB pitch-slide application.
/// Returns `Some(new_freq)` if the frequency changed, or `None` if slide
/// has reached target and should be deactivated.
pub fn apply_pitch_slide(channel: &mut ChannelState) -> Option<u16> {
    if !channel.flags1.contains(ChannelFlags1::PITCH_SLIDE_ON) {
        return None;
    }

    let step = channel.pitch_slide.freq_step;
    let current = channel.frequency;
    let target = channel.pitch_slide.target_freq;

    let new_freq;

    if channel.flags1.contains(ChannelFlags1::PITCH_SLIDE_DEC) {
        // Sliding down (decreasing frequency register = higher pitch in GB terms)
        if current <= target {
            // Reached target
            channel.flags1.remove(ChannelFlags1::PITCH_SLIDE_ON);
            return None;
        }
        new_freq = current.saturating_sub(step);
        if new_freq <= target {
            channel.flags1.remove(ChannelFlags1::PITCH_SLIDE_ON);
            return Some(target);
        }
    } else {
        // Sliding up (increasing frequency register)
        if current >= target {
            channel.flags1.remove(ChannelFlags1::PITCH_SLIDE_ON);
            return None;
        }
        new_freq = current.saturating_add(step);
        if new_freq >= target {
            channel.flags1.remove(ChannelFlags1::PITCH_SLIDE_ON);
            return Some(target);
        }
    }

    channel.frequency = new_freq;
    Some(new_freq)
}

// ── Duty Cycle Rotation ──────────────────────────────────────────────────

/// Rotate the duty cycle pattern by 2 bits (one position in the 4-slot cycle).
///
/// The pattern byte holds 4 duty cycles (2 bits each):
/// bits 7-6 = slot 0, bits 5-4 = slot 1, bits 3-2 = slot 2, bits 1-0 = slot 3.
///
/// Each note, the byte is rotated left by 2 bits and the new current duty
/// is taken from bits 7-6.
///
/// Returns the new duty cycle value (0-3).
pub fn rotate_duty_cycle(channel: &mut ChannelState) -> u8 {
    // Rotate left by 2 bits (the low 2 bits wrap to the top)
    let pattern = channel.duty_cycle_pattern;
    let rotated = (pattern << 2) | (pattern >> 6);
    channel.duty_cycle_pattern = rotated;

    // The current duty is the top 2 bits
    (rotated >> 6) & 0x03
}
