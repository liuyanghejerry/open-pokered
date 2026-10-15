//! Pure PrintLetterDelay arithmetic. Callers retain their own counters/input.

/// Finish the current glyph's wait. An A/B observed at the last glyph's
/// normal expiry still schedules the source's one DelayFrame before DONE.
pub(crate) fn poll_wait(
    remaining: &mut u16,
    short_exit_scheduled: &mut bool,
    final_glyph: bool,
    held_ab: bool,
) -> bool {
    if *remaining == 0 {
        return true;
    }
    if final_glyph && held_ab && !*short_exit_scheduled {
        *remaining = 1;
        *short_exit_scheduled = true;
        return false;
    }
    *remaining -= 1;
    if *remaining != 0 {
        if held_ab {
            *remaining = 1;
            *short_exit_scheduled = true;
        }
        return false;
    }
    *short_exit_scheduled = false;
    true
}

/// Joypad is sampled by the caller, rather than automatically every host frame.
pub(crate) fn sample_pressed(last: &mut u8, held: u8) -> u8 {
    let pressed = held & !*last;
    *last = held;
    pressed
}
