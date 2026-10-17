//! DisplayDexRating's shared sound continuation. Text selection stays pure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DexRatingSound {
    Denied,
    PokedexRating,
    GetItem1,
    CaughtMon,
    LevelUp,
    GetKeyItem,
    GetItem2,
}
impl DexRatingSound {
    pub fn for_owned(owned: u32) -> Self {
        match owned {
            0..=9 => Self::Denied,
            10..=39 => Self::PokedexRating,
            40..=59 => Self::GetItem1,
            60..=89 => Self::CaughtMon,
            90..=119 => Self::LevelUp,
            120..=149 => Self::GetKeyItem,
            _ => Self::GetItem2,
        }
    }
    pub fn script_id(self) -> &'static str {
        match self {
            Self::Denied => "SFX_DENIED",
            Self::PokedexRating => "SFX_POKEDEX_RATING",
            Self::GetItem1 => "SFX_GET_ITEM_1",
            Self::CaughtMon => "SFX_CAUGHT_MON",
            Self::LevelUp => "SFX_LEVEL_UP",
            Self::GetKeyItem => "SFX_GET_KEY_ITEM",
            Self::GetItem2 => "SFX_GET_ITEM_2",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    PriorSound,
    RatingSound,
    Restarted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DexRatingAudio {
    sound: DexRatingSound,
    phase: Phase,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatingAudioStep {
    Waiting,
    Play(DexRatingSound),
    RestartMap,
    Ready,
}
impl DexRatingAudio {
    pub fn new(owned: u32) -> Self {
        Self {
            sound: DexRatingSound::for_owned(owned),
            phase: Phase::PriorSound,
        }
    }
    /// No Joypad polls and no guessed durations. Play yields to the host
    /// before the next completion sample; restart does not add an ACK delay.
    pub fn tick(&mut self, sound_playing: bool) -> RatingAudioStep {
        match self.phase {
            Phase::PriorSound if !sound_playing => {
                self.phase = Phase::RatingSound;
                RatingAudioStep::Play(self.sound)
            }
            Phase::RatingSound if !sound_playing => {
                self.phase = Phase::Restarted;
                RatingAudioStep::RestartMap
            }
            Phase::Restarted => RatingAudioStep::Ready,
            _ => RatingAudioStep::Waiting,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_rating_sound_bands_cover_every_obtainable_count() {
        use DexRatingSound::*;
        let bands = [
            (0, 9, Denied),
            (10, 39, PokedexRating),
            (40, 59, GetItem1),
            (60, 89, CaughtMon),
            (90, 119, LevelUp),
            (120, 149, GetKeyItem),
            (150, 151, GetItem2),
        ];
        let mut checked = 0;
        for (first, last, sound) in bands {
            for owned in first..=last {
                assert_eq!(DexRatingSound::for_owned(owned), sound, "owned={owned}");
                checked += 1;
            }
        }
        assert_eq!(checked, 152);
    }

    #[test]
    fn rating_waits_for_prior_and_selected_sound_then_restarts_once() {
        let mut gate = DexRatingAudio::new(150);
        for _ in 0..120 {
            assert_eq!(gate.tick(true), RatingAudioStep::Waiting);
        }
        assert_eq!(
            gate.tick(false),
            RatingAudioStep::Play(DexRatingSound::GetItem2)
        );
        for _ in 0..600 {
            assert_eq!(gate.tick(true), RatingAudioStep::Waiting);
        }
        assert_eq!(gate.tick(false), RatingAudioStep::RestartMap);
        // Map music is unrelated to SFX completion once the gate returned.
        for playing in [false, true, true, false] {
            assert_eq!(gate.tick(playing), RatingAudioStep::Ready);
        }
    }

    #[test]
    fn saved_rating_audio_wait_keeps_completion_without_replaying_fanfare() {
        let mut gate = DexRatingAudio::new(70);
        assert_eq!(
            gate.tick(false),
            RatingAudioStep::Play(DexRatingSound::CaughtMon)
        );
        let serialized = serde_json::to_string(&gate).unwrap();
        let mut restored: DexRatingAudio = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored.tick(true), RatingAudioStep::Waiting);
        assert_eq!(restored.tick(false), RatingAudioStep::RestartMap);
        let serialized = serde_json::to_string(&restored).unwrap();
        let mut restored: DexRatingAudio = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored.tick(false), RatingAudioStep::Ready);
    }
}
