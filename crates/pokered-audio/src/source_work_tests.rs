use super::*;
use crate::alloc_prelude::*;
use crate::{music_data::MusicId, sfx_data::SfxId};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Port(AudioAccess),
    StackRead(u16, u8),
    Return(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Bus {
    time: u64,
    sp: u16,
    pc: u16,
    irq_active: bool,
    deny: bool,
    events: Vec<(u64, Event)>,
}

impl Default for Bus {
    fn default() -> Self {
        Self {
            time: 0,
            sp: 0xdffe,
            pc: 0,
            irq_active: true,
            deny: false,
            events: vec![],
        }
    }
}

impl AudioSourceBus for Bus {
    type Error = ();
    fn begin_instruction(&mut self, _: u8) -> Result<(), ()> {
        if self.deny {
            Err(())
        } else {
            Ok(())
        }
    }
    fn elapse(&mut self, tcycles: u8) {
        self.time += u64::from(tcycles);
    }
    fn access(&mut self, access: AudioAccess) {
        self.events.push((self.time, Event::Port(access)));
    }
    fn return_from_call(&mut self, kind: AudioReturn) {
        let (first, total) = match kind {
            AudioReturn::Unconditional => (4, 16),
            AudioReturn::ConditionalTaken => (8, 20),
        };
        self.elapse(first);
        self.events
            .push((self.time, Event::StackRead(self.sp, 0x34)));
        self.sp = self.sp.wrapping_add(1);
        self.elapse(4);
        self.events
            .push((self.time, Event::StackRead(self.sp, 0x12)));
        self.sp = self.sp.wrapping_add(1);
        self.elapse(total - first - 4);
        self.pc = 0x1234;
        self.events.push((self.time, Event::Return(self.pc)));
    }
}

fn owner_image(manager: &AudioManager) -> String {
    format!(
        "{:?}|{:?}|{:?}|{}|{}|{}|{}|{:?}|{:?}",
        manager.engine.sequencer,
        manager.engine.apu,
        manager.engine.fade_state(),
        manager.engine.fade_counter_reload,
        manager.engine.no_audio_fade_out(),
        manager.engine.master_volume_left(),
        manager.engine.master_volume_right(),
        manager.engine.fade_queued_music,
        manager.fade_audio_work()
    )
}

fn manager(path: u8) -> AudioManager {
    let mut manager = AudioManager::new();
    manager.apu.write_register(0xff26, 0x80);
    match path {
        0 => manager.set_master_volume(3, 2),
        1 => {
            manager.set_no_audio_fade_out(true);
            manager.set_master_volume(3, 2);
        }
        2 => {
            manager.play_music(MusicId::PALLET_TOWN);
            manager.fade_out(2);
        }
        3 => {
            manager.play_music(MusicId::PALLET_TOWN);
            manager.fade_out(3);
            manager.sequencer.fade_counter = 0;
            // The live APU differs from the manager's 7/7 shadow. The
            // original LDH must read this 5/3 value, rather than the shadow.
            manager.apu.write_register(0xff24, 0x53);
        }
        _ => unreachable!(),
    }
    if path >= 2 {
        assert_eq!(manager.fade_state(), FadeState::FadingOut);
        assert_eq!(
            manager.sequencer.fade_counter,
            if path == 2 { 2 } else { 0 }
        );
    }
    manager
}

fn finish(manager: &mut AudioManager, bus: &mut Bus) -> Vec<(u8, u8)> {
    let mut result = vec![];
    for _ in 0..40 {
        let Some(instruction) = manager.step_fade_audio_work(bus).unwrap() else {
            return result;
        };
        result.push((instruction.source_line, instruction.tcycles));
    }
    panic!("FadeOutAudio did not return")
}

#[test]
fn source_work_four_primary_paths_have_literal_opcode_and_access_vectors() {
    // Independent literal vectors transcribed from home/fade_audio.asm,
    // including taken/not-taken JR and conditional RET durations. Accesses
    // use opcode-fetch M1 start (absolute data M4=+12, LDH M3=+8), not the
    // Python memory-method hook origin in the historical 871 capture.
    let vectors: [&[(u8, u8)]; 4] = [
        &[
            (2, 16),
            (3, 4),
            (4, 8),
            (5, 16),
            (6, 8),
            (7, 8),
            (8, 8),
            (9, 12),
            (10, 16),
        ],
        &[(2, 16), (3, 4), (4, 8), (5, 16), (6, 8), (7, 20)],
        &[
            (2, 16),
            (3, 4),
            (4, 12),
            (12, 16),
            (13, 4),
            (14, 8),
            (15, 4),
            (16, 16),
            (17, 16),
        ],
        &[
            (2, 16),
            (3, 4),
            (4, 12),
            (12, 16),
            (13, 4),
            (14, 12),
            (19, 16),
            (20, 16),
            (21, 12),
            (22, 4),
            (23, 8),
            (24, 4),
            (25, 8),
            (26, 4),
            (27, 4),
            (28, 4),
            (29, 8),
            (30, 8),
            (31, 4),
            (32, 8),
            (33, 4),
            (34, 12),
            (35, 16),
        ],
    ];
    let accesses = [
        vec![
            (12, AudioAccess::FadeActive(false)),
            (40, AudioAccess::NoFadeOut(false)),
            (76, AudioAccess::WriteNr50(0x77)),
        ],
        vec![
            (12, AudioAccess::FadeActive(false)),
            (40, AudioAccess::NoFadeOut(true)),
        ],
        vec![
            (12, AudioAccess::FadeActive(true)),
            (44, AudioAccess::ReadCounter(2)),
            (76, AudioAccess::WriteCounter(1)),
        ],
        vec![
            (12, AudioAccess::FadeActive(true)),
            (44, AudioAccess::ReadCounter(0)),
            (76, AudioAccess::ReadReload(3)),
            (92, AudioAccess::WriteCounter(3)),
            (104, AudioAccess::ReadNr50(0x53)),
            (184, AudioAccess::WriteNr50(0x42)),
        ],
    ];
    for path in 0..4 {
        let mut manager = manager(path);
        manager.begin_fade_audio_work().unwrap();
        let mut bus = Bus::default();
        assert_eq!(finish(&mut manager, &mut bus), vectors[path as usize]);
        let actual: Vec<_> = bus
            .events
            .iter()
            .filter_map(|(t, e)| match e {
                Event::Port(port) => Some((*t, *port)),
                _ => None,
            })
            .collect();
        assert_eq!(actual, accesses[path as usize]);
        assert_eq!(bus.time, [96, 72, 96, 204][path as usize]);
        let stack_times: Vec<_> = bus
            .events
            .iter()
            .filter_map(|(t, e)| match e {
                Event::StackRead(_, _) => Some(*t),
                _ => None,
            })
            .collect();
        assert_eq!(
            stack_times,
            match path {
                1 => vec![60, 64],
                3 => vec![192, 196],
                _ => vec![84, 88],
            }
        );
        assert_eq!((bus.pc, bus.sp, bus.irq_active), (0x1234, 0xe000, true));
        assert!(manager.fade_audio_work().is_none());
    }
}

#[test]
fn source_work_each_opcode_reservation_rejection_has_no_effect() {
    for path in 0..4 {
        let mut manager = manager(path);
        manager.begin_fade_audio_work().unwrap();
        let mut bus = Bus::default();
        while manager.fade_audio_work().is_some() {
            bus.deny = true;
            let before = owner_image(&manager);
            let bus_before = bus.clone();
            assert_eq!(
                manager.step_fade_audio_work(&mut bus),
                Err(AudioWorkError::Reservation(()))
            );
            assert_eq!(owner_image(&manager), before);
            assert_eq!(bus, bus_before);
            bus.deny = false;
            manager.step_fade_audio_work(&mut bus).unwrap();
        }
    }
}

#[test]
fn source_work_live_music_sfx_and_cry_do_not_gain_a_second_tick() {
    for music in [MusicId::PALLET_TOWN, MusicId::GYM, MusicId::SILPH_CO] {
        let mut manager = manager(0);
        manager.play_music(music);
        manager.play_cry(SfxId::from_u8(20).unwrap(), 7, 9);
        manager.update_frame();
        manager.fade_out(2);
        let channels = format!("{:?}", manager.sequencer.channels);
        let apu = format!("{:?}", manager.apu);
        manager.begin_fade_audio_work().unwrap();
        let mut bus = Bus::default();
        finish(&mut manager, &mut bus);
        assert_eq!(format!("{:?}", manager.sequencer.channels), channels);
        assert_eq!(format!("{:?}", manager.apu), apu);
        assert_eq!(manager.sequencer.fade_counter, 1);
        assert_eq!(manager.last_music_id(), Some(music));
    }
}

#[test]
fn source_work_unsupported_entry_and_atomic_tick_guard_do_not_advance() {
    for raw in [0, 0x07, 0x70, 0x88, 0xff] {
        let mut manager = manager(3);
        manager.apu.write_register(0xff24, raw);
        let before = owner_image(&manager);
        let expected = if raw == 0 {
            AudioWorkUnsupported::FadeCompletePlaySound
        } else {
            AudioWorkUnsupported::UnrepresentableVolume(raw)
        };
        assert_eq!(manager.begin_fade_audio_work(), Err(expected));
        assert_eq!(owner_image(&manager), before);
    }
    let mut manager = manager(2);
    manager.begin_fade_audio_work().unwrap();
    let before = owner_image(&manager);
    assert_eq!(
        manager.begin_fade_audio_work(),
        Err(AudioWorkUnsupported::AlreadyPending)
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| manager.update_frame()));
    assert!(result.is_err());
    assert_eq!(owner_image(&manager), before);
}
