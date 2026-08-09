//! Standalone audio test — plays Pallet Town BGM through speakers.
//! Run: cargo run --example play_audio_test -p pokered-app

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use pokered_audio::audio_manager::AudioManager;
use pokered_audio::music_data::MusicId;
use pokered_audio::sfx_data::SfxId;
use pokered_audio::CPU_CLOCK_HZ;

fn main() {
    println!("=== Pokemon Red/Blue Audio Test ===\n");

    let mut manager = AudioManager::new();

    // APU starts with power=false — must enable (original game writes 0x80 to NR52)
    manager.apu.write_register(0xFF26, 0x80);
    println!("[init] APU power: {}", manager.apu.power);
    println!("[init] NR50 (master vol): 0x{:02X}", manager.apu.nr50);
    println!("[init] NR51 (panning):    0x{:02X}", manager.apu.nr51);

    println!("\n[play] Starting Pallet Town BGM...");
    manager.play_music(MusicId::PALLET_TOWN);
    println!("[play] Music playing: {}", manager.is_music_playing());
    println!("[play] APU power after play_music: {}", manager.apu.power);

    for i in 0..5 {
        manager.update_frame();
        let (l, r) = manager.apu.mix_sample();
        println!(
            "[frame {}] mix_sample=({:5}, {:5})  ch1_en={} ch2_en={} ch3_en={} ch4_en={}",
            i,
            l,
            r,
            manager.apu.ch1.enabled,
            manager.apu.ch2.enabled,
            manager.apu.ch3.enabled,
            manager.apu.ch4.enabled,
        );
    }

    println!("\n[diag] Ticking APU 10000 cycles and sampling...");
    let mut nonzero_count = 0;
    let mut max_sample: i16 = 0;
    for _ in 0..100 {
        manager.apu.tick_n(95);
        let (l, r) = manager.apu.mix_sample();
        if l != 0 || r != 0 {
            nonzero_count += 1;
        }
        max_sample = max_sample.max(l.abs()).max(r.abs());
    }
    println!(
        "[diag] Non-zero samples: {}/100, max amplitude: {}",
        nonzero_count, max_sample
    );

    if max_sample == 0 {
        println!("\n[WARN] All samples are zero! Debugging...");
        println!("  APU power: {}", manager.apu.power);
        println!("  NR50: 0x{:02X}", manager.apu.nr50);
        println!("  NR51: 0x{:02X}", manager.apu.nr51);
        println!(
            "  ch1: enabled={}, freq_reg={}, duty={}",
            manager.apu.ch1.enabled, manager.apu.ch1.freq_reg, manager.apu.ch1.duty as u8,
        );
        println!(
            "  ch2: enabled={}, freq_reg={}, duty={}",
            manager.apu.ch2.enabled, manager.apu.ch2.freq_reg, manager.apu.ch2.duty as u8,
        );
        println!(
            "  ch3: enabled={}, dac_enabled={}, freq_reg={}",
            manager.apu.ch3.enabled, manager.apu.ch3.dac_enabled, manager.apu.ch3.freq_reg,
        );
        println!("  ch4: enabled={}", manager.apu.ch4.enabled);
        println!(
            "  Sequencer music_playing: {}",
            manager.sequencer.music_playing
        );
        for i in 0..4 {
            let ch = &manager.sequencer.channels[i];
            println!(
                "  Seq ch{}: active={}, ptr={}/{}, freq={}, oct={}, vol=0x{:02X}, delay={}",
                i,
                ch.active,
                ch.ptr,
                ch.data.len(),
                ch.frequency,
                ch.octave,
                ch.volume_envelope,
                ch.delay_counter
            );
        }
    }

    println!("\n[audio] Setting up cpal output...");
    let host = cpal::default_host();
    let device = match host.default_output_device() {
        Some(d) => {
            println!("[audio] Output device: {}", d.name().unwrap_or_default());
            d
        }
        None => {
            eprintln!("[ERROR] No audio output device found!");
            return;
        }
    };

    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(44_100),
        buffer_size: cpal::BufferSize::Default,
    };

    let mgr = Arc::new(Mutex::new(manager));
    let mgr_stream = Arc::clone(&mgr);
    let cycles_per_sample = CPU_CLOCK_HZ / 44_100;

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut mgr = mgr_stream.lock().unwrap();
                let max_amplitude = 480.0_f32;
                for frame in data.chunks_mut(2) {
                    mgr.apu.tick_n(cycles_per_sample);
                    let (left, right) = mgr.apu.mix_sample();
                    frame[0] = left as f32 / max_amplitude;
                    frame[1] = right as f32 / max_amplitude;
                }
            },
            |err| eprintln!("[audio ERROR] {}", err),
            None,
        )
        .expect("Failed to build output stream");

    stream.play().expect("Failed to start audio stream");
    println!("[audio] Stream started at 44100 Hz stereo");

    let total_duration = Duration::from_secs(8);
    let frame_interval = Duration::from_micros(16_667);
    let start = Instant::now();
    let mut frame_count = 0u32;

    println!(
        "\n[play] Playing Pallet Town for {} seconds...",
        total_duration.as_secs()
    );
    println!("[play] (You should hear music through your speakers)\n");

    while start.elapsed() < total_duration {
        let frame_start = Instant::now();

        {
            let mut mgr = mgr.lock().unwrap();
            mgr.update_frame();
        }

        frame_count += 1;

        if frame_count % 60 == 0 {
            let mgr = mgr.lock().unwrap();
            let (l, r) = mgr.apu.mix_sample();
            let elapsed = start.elapsed().as_secs();
            println!(
                "  [{:2}s] frame={:4}  sample=({:5},{:5})  music_playing={} channels_active={}/4",
                elapsed,
                frame_count,
                l,
                r,
                mgr.sequencer.music_playing,
                (0..4).filter(|&i| mgr.sequencer.channels[i].active).count(),
            );
        }

        let elapsed = frame_start.elapsed();
        if elapsed < frame_interval {
            std::thread::sleep(frame_interval - elapsed);
        }
    }

    println!("\n[sfx] Playing SFX: PressAB...");
    {
        let mut mgr = mgr.lock().unwrap();
        mgr.play_sfx(SfxId::PressAB);
    }

    let sfx_start = Instant::now();
    let sfx_duration = Duration::from_secs(2);
    while sfx_start.elapsed() < sfx_duration {
        let frame_start = Instant::now();
        {
            let mut mgr = mgr.lock().unwrap();
            mgr.update_frame();
        }
        let elapsed = frame_start.elapsed();
        if elapsed < frame_interval {
            std::thread::sleep(frame_interval - elapsed);
        }
    }

    println!("\n[play] Switching to Title Screen BGM for 5 seconds...");
    {
        let mut mgr = mgr.lock().unwrap();
        mgr.play_music(MusicId::TITLE_SCREEN);
    }

    let title_start = Instant::now();
    let title_duration = Duration::from_secs(5);
    let mut title_frames = 0u32;
    while title_start.elapsed() < title_duration {
        let frame_start = Instant::now();
        {
            let mut mgr = mgr.lock().unwrap();
            mgr.update_frame();
        }
        title_frames += 1;

        if title_frames % 60 == 0 {
            let mgr = mgr.lock().unwrap();
            let (l, r) = mgr.apu.mix_sample();
            println!(
                "  [{:2}s] sample=({:5},{:5})  music_playing={}",
                title_start.elapsed().as_secs(),
                l,
                r,
                mgr.sequencer.music_playing,
            );
        }

        let elapsed = frame_start.elapsed();
        if elapsed < frame_interval {
            std::thread::sleep(frame_interval - elapsed);
        }
    }

    println!("\n=== Audio test complete ===");
    println!("Total frames rendered: {}", frame_count + title_frames);
    println!("If you heard music, the audio system is working!");
    println!("If silent, check the diagnostic output above for clues.");
}
