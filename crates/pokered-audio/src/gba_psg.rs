//! GB sequencer registers mapped to the GBA's four compatible PSG channels.
//! Register map and wave banking: https://mgba-emu.github.io/gbatek/#gbasoundcontroller

/// GBA wave RAM exposes the bank opposite the playback bank. While the GB
/// sequencer disables the DAC to upload a wave, select playback bank 1 so
/// writes fill bank 0. Enabling the DAC switches playback back to bank 0.
fn mapped_register(addr: u16, value: u8) -> Option<(usize, u8)> {
    // Zero marks an unused register. A bounded lookup avoids walking the
    // register ranges on every note, volume and routing write on ARM7.
    const OFFSETS: [u8; 48] = [
        0x60, 0x62, 0x63, 0x64, 0x65, 0, 0x68, 0x69, 0x6C, 0x6D, 0x70, 0x72, 0x73, 0x74, 0x75, 0,
        0x78, 0x79, 0x7C, 0x7D, 0x80, 0x81, 0x84, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x90, 0x91, 0x92,
        0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F,
    ];
    let offset = *OFFSETS.get(usize::from(addr.wrapping_sub(0xFF10)))?;
    if offset == 0 {
        return None;
    }
    let value = match addr {
        0xFF1A => {
            if value & 0x80 != 0 {
                0x80
            } else {
                0x40
            }
        }
        0xFF26 => value & 0x80,
        _ => value,
    };
    Some((usize::from(offset), value))
}

#[cfg(target_os = "none")]
pub fn write_register(addr: u16, value: u8) {
    if let Some((offset, value)) = mapped_register(addr, value) {
        // IO supports byte writes, unlike GBA VRAM. All accesses are owned
        // by the audio manager on the main thread.
        unsafe {
            core::ptr::write_volatile((0x0400_0000 + offset) as *mut u8, value);
        }
    }
}

#[cfg(target_os = "none")]
pub fn initialize() {
    write_register(0xFF26, 0x80);
    // PSG at full volume; direct-sound FIFO channels disabled.
    unsafe {
        core::ptr::write_volatile(0x0400_0082 as *mut u16, 2);
    }
    write_register(0xFF24, 0x77);
    write_register(0xFF25, 0xFF);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_channels_without_writing_gba_register_gaps() {
        for (gb, gba) in [
            (0xFF10, 0x60),
            (0xFF11, 0x62),
            (0xFF14, 0x65),
            (0xFF16, 0x68),
            (0xFF17, 0x69),
            (0xFF18, 0x6C),
            (0xFF19, 0x6D),
            (0xFF1B, 0x72),
            (0xFF1E, 0x75),
            (0xFF20, 0x78),
            (0xFF21, 0x79),
            (0xFF22, 0x7C),
            (0xFF23, 0x7D),
            (0xFF24, 0x80),
            (0xFF25, 0x81),
            (0xFF30, 0x90),
            (0xFF3F, 0x9F),
        ] {
            assert_eq!(mapped_register(gb, 0xAB), Some((gba, 0xAB)));
        }
        for gap in [0xFF15, 0xFF1F, 0xFF27, 0xFF2F] {
            assert_eq!(mapped_register(gap, 0), None);
        }
    }
    #[test]
    fn wave_upload_and_playback_use_the_same_bank() {
        assert_eq!(mapped_register(0xFF1A, 0), Some((0x70, 0x40)));
        assert_eq!(mapped_register(0xFF1A, 0x80), Some((0x70, 0x80)));
        assert_eq!(mapped_register(0xFF26, 0xFF), Some((0x84, 0x80)));
    }
}
