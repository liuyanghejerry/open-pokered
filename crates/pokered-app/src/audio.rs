//! Audio for the app frontend: the shared [`AudioOutput`] (device glue lives
//! in `pokered_audio::output`, on top of the generic `dotzuki_audio::output`
//! backends) plus the species-cry helper.

use pokered_audio::sfx_data::SfxId;
use pokered_data::species::Species;

pub use pokered_audio::output::AudioOutput;

/// Play a species' cry with its pitch/length modifiers (`PlayCry` in
/// home/pokemon.asm: `GetCryData` + `PlaySound`).
pub fn play_species_cry(audio: &AudioOutput, species: Species) {
    let c = pokered_data::cries::cry_data(species);
    if let Some(id) = SfxId::from_u8(c.sfx) {
        audio.play_cry(id, c.pitch, c.length);
    }
}
