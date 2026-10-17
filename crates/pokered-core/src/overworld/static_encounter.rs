//! TalkToTrainer's inner text/cry, enemy header and music before outer ACK.
use super::screen::{OverworldAudioRequest, PendingWildEncounter};
use super::{field_cry::FieldCryState, OverworldScreen};
use crate::alloc_prelude::*;
use dotzuki_engine::GameData;
use pokered_data::{maps::MapId, species::Species, tilesets::TilesetId};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StaticEncounterState {
    text: FieldCryState,
    species: Species,
    level: u8,
    map: Option<MapId>,
    prepared: bool,
}
impl StaticEncounterState {
    pub fn new(text: String, species: Species, level: u8) -> Self {
        Self {
            text: FieldCryState::inner_print_text(text, species.to_string()),
            species,
            level,
            map: None,
            prepared: false,
        }
    }
    pub(super) fn encounter(&self) -> PendingWildEncounter {
        PendingWildEncounter {
            species: self.species,
            level: self.level,
            old_man: false,
            hooked: false,
        }
    }
}
impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    pub(super) fn tick_static_encounter(&mut self, state: &mut StaticEncounterState) -> bool {
        if state.map.is_none() {
            state.map = Some(self.state.current_map);
        }
        if !state.prepared {
            self.tick_field_cry_before_ack(&mut state.text);
            if !state.text.at_outer_ack() {
                return false;
            }
            // EngageMapTrainer/StartTrainerBattle prepare species/level and
            // play encounter music now. No enemy DVs or battle screen yet.
            self.audio_requests.push(OverworldAudioRequest::PlaySound {
                sound_id: "SFX_STOP_ALL_MUSIC".into(),
            });
            self.audio_requests.push(OverworldAudioRequest::PlayMusic {
                music_id: "MUSIC_MEET_MALE_TRAINER".into(),
            });
            state.prepared = true;
        }
        // This same update may perform the actual outer Joypad poll. Do not
        // invent a frame between the returned cry/header and its caller ACK.
        self.tick_field_cry(&mut state.text)
    }
}

#[cfg(all(test, not(target_os = "none"), not(feature = "script-boa")))]
mod tests {
    use super::*;
    use crate::foreground_input::{ForegroundJoypad, A, B};
    use crate::overworld::{script_bridge::ScriptEffect, Direction};
    use crate::snapshot::OverworldSnapshot;
    use pokered_data::impl_traits::PokemonRedData;
    type Field = OverworldScreen<PokemonRedData>;
    fn step(f: &mut Field, b: u8) {
        f.update_frame(ForegroundJoypad::as_field_input(b));
        assert_eq!(f.source_foreground_error(), None);
    }
    fn state(f: &Field) -> &StaticEncounterState {
        let Some(ScriptEffect::StaticEncounter { state }) = &f.active_script_effect else {
            panic!(
                "expected static encounter, got {:?}",
                f.active_script_effect
            )
        };
        state
    }
    fn restore(f: &mut Field) {
        let value = serde_json::to_value(OverworldSnapshot::capture(f)).unwrap();
        let snapshot: OverworldSnapshot = serde_json::from_value(value.clone()).unwrap();
        snapshot.restore_into(f);
        assert_eq!(
            serde_json::to_value(OverworldSnapshot::capture(f)).unwrap(),
            value
        );
    }
    #[test]
    fn four_embedded_static_talks_prepare_music_before_ack_and_publish_only_after_close() {
        for (map, x, y, facing, species, level) in [
            (
                MapId::CeruleanCaveB1F,
                27,
                14,
                Direction::Up,
                Species::Mewtwo,
                70,
            ),
            (
                MapId::PowerPlant,
                4,
                8,
                Direction::Down,
                Species::Zapdos,
                50,
            ),
            (
                MapId::SeafoamIslandsB4F,
                6,
                2,
                Direction::Up,
                Species::Articuno,
                50,
            ),
            (
                MapId::VictoryRoad2F,
                11,
                4,
                Direction::Down,
                Species::Moltres,
                50,
            ),
        ] {
            // Preboot component fixture; SFX completion samples below are
            // explicit unit inputs, not an actual PCM/native witness.
            let mut f = Field::new(map, None, PokemonRedData);
            f.state.player.x = x;
            f.state.player.y = y;
            f.state.player.facing = facing;
            for _ in 0..2 {
                step(&mut f, 0);
            }
            for _ in 0..8 {
                step(&mut f, A);
                if matches!(
                    f.active_script_effect,
                    Some(ScriptEffect::StaticEncounter { .. })
                ) {
                    break;
                }
            }
            assert_eq!(state(&f).species, species);
            assert_eq!(state(&f).level, level);
            let mut cries = 0;
            for _ in 0..2000 {
                step(&mut f, 0);
                cries += f
                    .audio_requests
                    .iter()
                    .filter(|r| matches!(r, OverworldAudioRequest::PlayCry { .. }))
                    .count();
                if cries > 0 {
                    break;
                }
            }
            assert_eq!(cries, 1);
            assert!(!state(&f).prepared);
            assert!(f.pending_wild_encounter.is_none());
            assert!(!f.script_awaiting_battle);
            restore(&mut f);
            f.script_sfx_playing = true;
            let last = f.source_foreground().unwrap().last();
            for _ in 0..20 {
                step(&mut f, A);
                assert!(!state(&f).prepared);
                assert!(f.pending_wild_encounter.is_none());
                assert_eq!(f.source_foreground().unwrap().last(), last);
                assert!(f.audio_requests.is_empty());
            }
            f.script_sfx_playing = false;
            step(&mut f, A);
            assert!(state(&f).prepared);
            assert_eq!(f.audio_requests.len(), 2);
            assert!(
                matches!(&f.audio_requests[0],OverworldAudioRequest::PlaySound{sound_id}if sound_id=="SFX_STOP_ALL_MUSIC")
            );
            assert!(
                matches!(&f.audio_requests[1],OverworldAudioRequest::PlayMusic{music_id}if music_id=="MUSIC_MEET_MALE_TRAINER")
            );
            assert!(f.pending_dialogue.as_ref().unwrap().holding_open());
            assert!(f.pending_wild_encounter.is_none());
            assert!(!f.script_awaiting_battle);
            restore(&mut f);
            for _ in 0..12 {
                step(&mut f, A | B);
                assert!(f.pending_dialogue.is_some());
                assert!(f.audio_requests.is_empty());
                assert!(f.pending_wild_encounter.is_none());
            }
            step(&mut f, 0);
            assert!(f.pending_dialogue.is_none());
            assert!(f.pending_wild_encounter.is_none());
            step(&mut f, 0);
            assert!(f.script_awaiting_battle);
            let encounter = f.pending_wild_encounter.as_ref().unwrap();
            assert_eq!(encounter.species, species);
            assert_eq!(encounter.level, level);
            assert!(f.audio_requests.is_empty());
        }
    }
}
