//! Cartridge-session restart. No Rust frame may survive the RAM reset.

use crate::Mode4Presenter;
use pokered_app::game::PokemonGame;
use pokered_renderer::FrameBuffer;

const WHITE_VBLANKS: usize = 32;

/// Stop hardware sound, publish a real white frame, then wait 32 separate
/// hardware VBlanks. No old Game update, save, or render runs in this interval.
pub(crate) fn restart_after_white(fb: &mut FrameBuffer, presenter: &mut Mode4Presenter) -> ! {
    // The sequencer's stop_all alone does not silence already-latched PSG
    // registers when no further audio tick runs during the white interval.
    unsafe { core::ptr::write_volatile(0x0400_0084 as *mut u16, 0) };
    PokemonGame::draw_soft_reset_white_frame(fb);
    presenter.present(fb, None);
    // Unlike VBlank::wait_for_vblank's missed-frame fast path, this helper
    // leaves any current blank before waiting for the next physical blank.
    agb::display::busy_wait_for_vblank();
    presenter.commit();
    for _ in 0..WHITE_VBLANKS {
        agb::display::busy_wait_for_vblank();
    }
    // IRQs remain live through the display interval. The ROM trampoline
    // disables them and DMA, then replaces the EWRAM SP before clearing RAM.
    unsafe { __pokered_restart_cartridge() }
}

// agb0.25.0 has no public reset syscall. This local Thumb ROM entry follows
// the BIOS ABI documented by libgba src/Reset.s, not a call back to Rust main.
// The linker puts .text.* in ROM; the hot-code IWRAM rules do not match it.
core::arch::global_asm!(
    r#"
    .syntax unified
    .section .text.pokered_gba_restart_cartridge,"ax",%progbits
    .p2align 2
    .thumb
    .global __pokered_restart_cartridge
    .type __pokered_restart_cartridge,%function
    .thumb_func
__pokered_restart_cartridge:
    ldr r1, =0x04000208
    movs r0, #0
    strh r0, [r1]
    ldr r1, =0x040000BA
    strh r0, [r1, #0]
    strh r0, [r1, #12]
    strh r0, [r1, #24]
    strh r0, [r1, #36]
    ldr r1, =0x03007F00
    mov sp, r1
    movs r0, #255
    swi 1
    ldr r1, =0x03007FFA
    movs r0, #0
    strb r0, [r1]
    swi 0
1:
    b 1b
    .size __pokered_restart_cartridge, .-__pokered_restart_cartridge
    .ltorg
"#
);

unsafe extern "C" {
    fn __pokered_restart_cartridge() -> !;
}
