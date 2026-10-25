use super::*;
use pokered_audio::{
    audio_manager::AudioManager,
    music_data::MusicId,
    source_clock::{
        apu_bus::SourceAudioFrameBus,
        frame_instructions::FrameStepError,
        AudioAdmissionError,
    },
};
use pokered_core::overworld::{
    display_delay_work::DelayFrameBus,
    display_lcd_work::{DisplayInstruction, DisplayInstructionBus},
    display_work_time::DisplayWorkTime,
};

fn manager(music: MusicId, confirmation: bool) -> AudioManager {
    let mut audio = AudioManager::new();
    audio.apu.write_register(0xff26, 0x80);
    audio.play_music(music);
    for _ in 0..21 { audio.update_frame(); }
    if confirmation { audio.play_sfx(SfxId::PressAB); }
    audio
}

fn clock(lcdc: u8, dma: bool, budget: u64) -> DisplayWorkTime {
    let mut time = DisplayWorkTime::default();
    time.begin_native_update();
    time.device_mut().request_interrupt(1);
    time.enter_pending_vblank().unwrap();
    time.device_mut().write_port(0xff40, lcdc, 0xc3).unwrap();
    time.device_mut().write_port(0xff43, 7, 0xc3).unwrap();
    time.device_mut().write_port(0xff4b, 7, 0xc3).unwrap();
    time.execute(DisplayInstruction { tcycles: 79, access: None }).unwrap();
    time.set_shadow_oam(&(0..160).map(|n| n as u8).collect::<Vec<_>>()).unwrap();
    if dma { time.device_mut().write_port(0xff46, 0xc3, 0xc3).unwrap(); }
    let mut state = serde_json::to_value(&time).unwrap();
    state["deadline"] = (time.elapsed() + budget).into();
    serde_json::from_value(state).unwrap()
}

#[test]
fn field_audio_prefixes_match_single_instructions_across_device_edges_and_budget_resume() {
    for (music, bank) in [(MusicId::POKECENTER, 2), (MusicId::DUNGEON3, 31), (MusicId::SILPH_CO, 31)] {
        for confirmation in [false, true] {
            for lcdc in [0x11, 0x93, 0xb1] {
                for dma in [false, true] {
                    for budget in [0, 1, 4, 32, 511, 65_535] {
                        let mut actual = manager(music, confirmation);
                        let mut expected = manager(music, confirmation);
                        let mut work = actual.begin_source_audio_frame_with_bus(bank, 0).unwrap();
                        let mut reference = expected.begin_source_audio_frame_with_bus(bank, 0).unwrap();
                        let mut time = clock(lcdc, dma, budget);
                        let mut expected_time = time.clone();
                        loop {
                            let result = actual.resume_source_audio_frame_with_runner(
                                &mut work, FieldAudioBus { time: time.display_bus() },
                            );
                            let reference_result = expected.resume_source_audio_frame_with_bus(
                                &mut reference, |apu, op| {
                                    FieldAudioBus { time: expected_time.display_bus() }.execute(apu, op)
                                },
                            );
                            assert_eq!(result, reference_result);
                            assert_eq!(work, reference);
                            assert_eq!(serde_json::to_value(&time).unwrap(), serde_json::to_value(&expected_time).unwrap());
                            assert_eq!(actual.apu.playback_state(), expected.apu.playback_state());
                            assert_eq!(format!("{:?}", actual.sequencer), format!("{:?}", expected.sequencer));
                            if result.is_ok() { break; }
                            assert_eq!(result, Err(AudioAdmissionError::Admission(FrameStepError::Clock(DisplayWorkError::Budget))));
                            time = serde_json::from_value(serde_json::to_value(&time).unwrap()).unwrap();
                            expected_time = serde_json::from_value(serde_json::to_value(&expected_time).unwrap()).unwrap();
                            time.begin_native_update();
                            expected_time.begin_native_update();
                        }
                        assert!(time.active_vblank());
                        assert_eq!(time.interrupt_returns(), 0);
                    }
                }
            }
        }
    }
}
