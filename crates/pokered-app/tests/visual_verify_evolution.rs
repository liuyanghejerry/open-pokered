//! Visual verification test for the evolution cutscene renderer.
//!
//! Drives `EvolutionScreenState` to its key phases and renders each through
//! the actual pipeline (`draw_evolution`), saving PNGs for inspection.
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_evolution -- --nocapture

use dotzuki_engine::render_config::RenderConfig;
use pokered_app::render::draw_evolution;
use pokered_core::evolution_screen::{
    EvolutionInput, EvolutionPhase, EvolutionScreenState, PendingEvolution,
};
use pokered_data::species::Species;
use pokered_renderer::{resource::ResourceManager, FrameBuffer, Rgba};

/// Create a `ResourceManager` by auto-detecting the gfx/ asset root.
fn create_resource_manager() -> ResourceManager {
    let root = pokered_renderer::resource::AssetRoot::auto_detect()
        .expect("gfx/ asset root not found; run scripts/fetch-gfx.sh");
    ResourceManager::new(root)
}

fn save_frame(fb: &FrameBuffer, filename: &str) {
    let img = image::RgbaImage::from_fn(fb.width(), fb.height(), |x, y| {
        let c = fb.get_pixel(x, y).unwrap_or(pokered_renderer::Rgba::WHITE);
        image::Rgba(c.to_array())
    });
    img.save(filename).expect("Failed to save PNG");
    eprintln!("Saved: {filename}");
}

fn bulbasaur_cutscene() -> EvolutionScreenState {
    EvolutionScreenState::new(
        vec![PendingEvolution {
            party_index: 0,
            from: Species::Bulbasaur,
            to: Species::Ivysaur,
            name: "BULBASAUR".to_string(),
            force: false,
        }],
        None,
        false,
    )
}

fn render(anim: &EvolutionScreenState, rm: &mut Option<ResourceManager>, name: &str) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_evolution(anim, rm, &mut fb);
    save_frame(&fb, name);
}

#[test]
fn render_evolution_phases() {
    let mut rm = Some(create_resource_manager());
    let none = EvolutionInput::none();

    // "What? BULBASAUR is evolving!" — old pic on white.
    let mut anim = bulbasaur_cutscene();
    assert_eq!(anim.phase(), EvolutionPhase::IsEvolving);
    render(&anim, &mut rm, "evolution_1_is_evolving.png");

    // Morph: black screen, flicker showing the new species.
    while anim.phase() != EvolutionPhase::Morph {
        anim.tick(none);
    }
    // Advance past the first 16-frame cancel window into the first flicker
    // (which shows the NEW species for 3 frames).
    for _ in 0..16 {
        anim.tick(none);
    }
    assert!(anim.black_palette());
    assert_eq!(anim.visible_species(), Some(Species::Ivysaur));
    render(&anim, &mut rm, "evolution_2_morph_new.png");
    // 3 frames later the flicker shows the old species again.
    for _ in 0..3 {
        anim.tick(none);
    }
    assert_eq!(anim.visible_species(), Some(Species::Bulbasaur));
    render(&anim, &mut rm, "evolution_3_morph_old.png");

    // Success: "BULBASAUR evolved into IVYSAUR!" with the new pic.
    let mut guard = 0;
    while anim.phase() != EvolutionPhase::EvolvedText {
        anim.tick(none);
        guard += 1;
        assert!(guard < 5000);
    }
    render(&anim, &mut rm, "evolution_4_evolved.png");

    // Cancelled run: "Huh? BULBASAUR stopped evolving!" with the old pic.
    let mut anim = bulbasaur_cutscene();
    while anim.phase() != EvolutionPhase::Morph {
        anim.tick(none);
    }
    anim.tick(EvolutionInput { a: false, b: true });
    assert_eq!(anim.phase(), EvolutionPhase::StoppedText);
    render(&anim, &mut rm, "evolution_5_stopped.png");
}
