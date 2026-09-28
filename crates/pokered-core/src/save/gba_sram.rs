//! Bare-metal (GBA) cartridge SRAM access: the 32 KiB save medium is
//! memory-mapped at `0x0E00_0000` and byte-addressable, so the existing
//! desktop SRAM image format ([`super::sram_export`] / [`super::sram_import`])
//! maps directly onto it with no shim layer.
//!
//! Bulk reads and writes use byte-wide volatile access plus a compiler fence:
//! wider memory operations are not supported by the cartridge SRAM bus.

pub const SRAM_BASE: usize = 0x0E00_0000;
/// 32 KiB — the Game Boy save medium the flashcart provides (SuperFW
/// `save_type = 0` in the generated patch).
pub const SRAM_SIZE: usize = 0x8000;

/// Read-only view over the cartridge SRAM (a raw 32 KiB `.sav` image).
pub fn sram() -> &'static [u8] {
    unsafe { core::slice::from_raw_parts(SRAM_BASE as *const u8, SRAM_SIZE) }
}

/// Mutable view over the cartridge SRAM. Callers must issue a
/// `compiler_fence(SeqCst)` after the final store.
pub fn sram_mut() -> &'static mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(SRAM_BASE as *mut u8, SRAM_SIZE) }
}

/// Copy the cartridge SRAM into `buf` with explicit byte-wide volatile
/// loads. Slice reads may compile to wider loads, and the 8-bit SRAM bus
/// does not serve those reliably (observed on mGBA as intermittently
/// corrupted validation input), so the boot import path reads this way.
pub fn read_into(buf: &mut [u8]) {
    let len = buf.len().min(SRAM_SIZE);
    read_bytes(0, &mut buf[..len]);
}

/// Read one region through the byte-wide SRAM bus.
pub fn read_bytes(offset: usize, buf: &mut [u8]) {
    assert!(offset <= SRAM_SIZE && buf.len() <= SRAM_SIZE - offset);
    for (i, b) in buf.iter_mut().enumerate() {
        *b = unsafe { core::ptr::read_volatile((SRAM_BASE + offset + i) as *const u8) };
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

/// Write `data` at `offset` with explicit byte-wide volatile stores.
///
/// SRAM is an 8-bit device: bulk copies through a slice may compile to
/// wider stores, and (empirically, on mGBA) some of those byte groups do
/// not land — a 32 KiB image written via `copy_from_slice` came back with
/// ~100 bytes of stale content. Byte-wise `write_volatile` is both the
/// hardware-correct access size for the SRAM bus and immune to store
/// coalescing.
pub fn write_bytes(offset: usize, data: &[u8]) {
    debug_assert!(offset + data.len() <= SRAM_SIZE);
    for (i, &b) in data.iter().enumerate() {
        unsafe {
            core::ptr::write_volatile((SRAM_BASE + offset + i) as *mut u8, b);
        }
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}
