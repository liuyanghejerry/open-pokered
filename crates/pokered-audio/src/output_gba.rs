//! GBA PSG output: the shared sequencer drives physical sound registers.
use crate::audio_manager::AudioManager;
use crate::music_data::MusicId;
use crate::sfx_data::SfxId;
use core::cell::{Cell, RefCell};

static VBLANK_CLOCK: critical_section::Mutex<Cell<u32>> =
    critical_section::Mutex::new(Cell::new(0));
static CLOCK_INSTALLED: critical_section::Mutex<Cell<bool>> =
    critical_section::Mutex::new(Cell::new(false));

fn clock_now() -> u32 {
    critical_section::with(|cs| VBLANK_CLOCK.borrow(cs).get())
}

pub struct AudioOutput {
    last_frame: Cell<u32>,
    manager: RefCell<AudioManager>,
}
impl AudioOutput {
    pub fn new() -> Option<Self> {
        let mut manager = AudioManager::new();
        crate::gba_psg::initialize();
        manager
            .apu
            .set_register_sink(Some(crate::gba_psg::write_register));
        manager.apu.write_register(0xFF26, 0x80);
        let install = critical_section::with(|cs| !CLOCK_INSTALLED.borrow(cs).replace(true));
        if install {
            // Safety: only a protected counter increment runs in the IRQ.
            // The one handler lives for the cartridge's lifetime; sequencing
            // and all allocations remain on the main thread.
            let handler = unsafe {
                agb::interrupt::add_interrupt_handler(agb::interrupt::Interrupt::VBlank, |cs| {
                    let counter = VBLANK_CLOCK.borrow(cs);
                    counter.set(counter.get().wrapping_add(1));
                })
            };
            core::mem::forget(handler);
        }
        Some(Self {
            last_frame: Cell::new(clock_now()),
            manager: RefCell::new(manager),
        })
    }
    /// Set the same NR50 level as the hosted PCM backend.
    pub fn set_master_volume(&self, left: u8, right: u8) {
        self.manager.borrow_mut().set_master_volume(left, right);
    }

    pub fn play_music(&self, id: MusicId) {
        self.manager.borrow_mut().play_music(id)
    }
    pub fn play_music_with_fade(&self, id: MusicId, fade_speed: u8) {
        self.manager
            .borrow_mut()
            .play_music_with_fade(id, fade_speed)
    }
    pub fn clear_saved_music_states(&self) {
        self.manager.borrow_mut().clear_saved_music_states()
    }
    pub fn fade_out(&self, fade_speed: u8) {
        self.manager.borrow_mut().fade_out(fade_speed)
    }
    pub fn play_sfx(&self, id: SfxId) {
        self.manager.borrow_mut().play_sfx(id)
    }
    pub fn play_badge_bank_quirk(&self) {
        self.manager.borrow_mut().play_badge_bank_quirk()
    }
    pub fn play_cry(&self, id: SfxId, pitch_mod: u8, tempo_mod: u8) {
        self.manager.borrow_mut().play_cry(id, pitch_mod, tempo_mod)
    }
    pub fn play_flute_in_battle(&self) {
        self.manager.borrow_mut().play_flute_in_battle()
    }
    pub fn play_flute_overworld(&self, resume_music: MusicId) {
        self.manager.borrow_mut().play_flute_overworld(resume_music)
    }
    pub fn play_script_music(&self, name: &str) -> bool {
        self.manager.borrow_mut().play_script_music(name)
    }
    pub fn is_sfx_playing(&self) -> bool {
        self.manager.borrow_mut().is_sfx_playing()
    }
    pub fn set_low_health_alarm(&self, enable: bool) {
        self.manager.borrow_mut().set_low_health_alarm(enable)
    }
    pub fn low_health_alarm_active(&self) -> bool {
        self.manager.borrow_mut().low_health_alarm_active()
    }
    pub fn stop_music(&self) {
        self.manager.borrow_mut().stop_music()
    }
    pub fn stop_all(&self) {
        self.manager.borrow_mut().stop_all()
    }
    pub fn is_music_playing(&self) -> bool {
        self.manager.borrow_mut().is_music_playing()
    }
    pub fn is_music_channel_playing(&self, channel: usize) -> bool {
        self.manager.borrow().sequencer.is_music_channel_active(channel)
    }
    pub fn last_music_id(&self) -> Option<MusicId> {
        self.manager.borrow_mut().last_music_id()
    }
    pub fn update_frame(&self) {
        let now = clock_now();
        let elapsed = now.wrapping_sub(self.last_frame.replace(now));
        let mut manager = self.manager.borrow_mut();
        for _ in 0..elapsed {
            manager.update_frame();
        }
    }
    pub fn try_resume(&self) {}
}
impl Default for AudioOutput {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        // Muting also prevents an old sustained tone from surviving a load.
        crate::gba_psg::write_register(0xFF26, 0);
    }
}
