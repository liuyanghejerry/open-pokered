use pokered_app::audio::{play_species_cry, AudioOutput};
use pokered_audio::{music_data::MusicId, sfx_data::SfxId};
use pokered_core::evolution_screen::{
    EvolutionInput, EvolutionScreenState, EvolutionSfx, PendingEvolution,
};
use pokered_data::species::Species;

#[test]
fn muted_sequencer_finishes_species_dependent_evolution_sounds() {
    for (from, to) in [
        (Species::Bulbasaur, Species::Ivysaur),
        (Species::Magikarp, Species::Gyarados),
        (Species::Dragonair, Species::Dragonite),
    ] {
        let audio = AudioOutput::new_pcm();
        let mut state = EvolutionScreenState::new(
            vec![PendingEvolution {
                party_index: 0,
                from,
                to,
                name: format!("{from:?}"),
                force: true,
            }],
            None,
            false,
        );
        let mut previous = state.phase();
        for frame in 0..3000 {
            audio.update_frame();
            state.tick_with_sound(EvolutionInput::none(), audio.is_sfx_playing());
            if previous != state.phase() {
                eprintln!("{from:?} frame={frame} {:?}", state.phase());
                previous = state.phase();
            }
            for sfx in state.pending_sfx.drain(..) {
                eprintln!("frame={frame} {sfx:?}");
                match sfx {
                    EvolutionSfx::StopMusic => audio.stop_all(),
                    EvolutionSfx::Tink => audio.play_sfx(SfxId::Tink),
                    EvolutionSfx::Cry(species) => play_species_cry(&audio, species),
                    EvolutionSfx::MorphMusic => audio.play_music(MusicId::SAFARI_ZONE),
                    EvolutionSfx::GetItem2 => audio.play_sfx(SfxId::GetItem2),
                }
            }
            if state.is_done() {
                break;
            }
        }
        assert!(
            state.is_done(),
            "{from:?} stuck in {:?}, busy={}",
            state.phase(),
            audio.is_sfx_playing()
        );
    }
}

#[test]
fn every_species_cry_finishes_without_a_device_or_pcm_callback() {
    for id in 1..=151 {
        let species = Species::from_index_id(id);
        let audio = AudioOutput::new_pcm();
        play_species_cry(&audio, species);
        for _ in 0..1000 {
            audio.update_frame();
            if !audio.is_sfx_playing() {
                break;
            }
        }
        assert!(!audio.is_sfx_playing(), "cry never finished: {species:?}");
    }
}

#[test]
#[ignore = "writes a matched frame to GAP_SCREENSHOTS"]
fn capture_evolution_frame_190() {
    let output =
        std::path::PathBuf::from(std::env::var("GAP_SCREENSHOTS").expect("output directory"));
    std::fs::create_dir_all(&output).unwrap();
    let audio = AudioOutput::new_pcm();
    let mut state = EvolutionScreenState::new(
        vec![PendingEvolution {
            party_index: 0,
            from: Species::Bulbasaur,
            to: Species::Ivysaur,
            name: "BULBASAUR".into(),
            force: true,
        }],
        None,
        false,
    );
    for _ in 0..190 {
        audio.update_frame();
        state.tick_with_sound(EvolutionInput::none(), audio.is_sfx_playing());
        for sfx in state.pending_sfx.drain(..) {
            match sfx {
                EvolutionSfx::StopMusic => audio.stop_all(),
                EvolutionSfx::Tink => audio.play_sfx(SfxId::Tink),
                EvolutionSfx::Cry(species) => play_species_cry(&audio, species),
                EvolutionSfx::MorphMusic => audio.play_music(MusicId::SAFARI_ZONE),
                EvolutionSfx::GetItem2 => audio.play_sfx(SfxId::GetItem2),
            }
        }
    }
    let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
    let mut fb = pokered_renderer::FrameBuffer::new(config, pokered_renderer::Rgba::WHITE);
    let mut resources = Some(pokered_renderer::resource::ResourceManager::new(
        pokered_renderer::resource::AssetRoot::auto_detect().unwrap(),
    ));
    pokered_app::render::draw_evolution(&state, &mut resources, &mut fb);
    image::RgbaImage::from_fn(160, 144, |x, y| {
        image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
    })
    .save(output.join("evolution-frame-190.png"))
    .unwrap();
}
