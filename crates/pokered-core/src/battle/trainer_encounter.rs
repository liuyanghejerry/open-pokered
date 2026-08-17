//! Trainer-sight engage intro (home/trainers.asm CheckFightingMapTrainers:
//! 129-159 + PlayTrainerMusic 390-443, data/trainers/encounter_types.asm).

use pokered_data::music::MusicId;
use pokered_data::trainer_data::TrainerClass;

/// The MEET_* track a spotted trainer plays (PlayTrainerMusic):
///   * RIVAL1/2/3 keep the current music (early `ret z` per class);
///   * gym leaders keep theirs (wGymLeaderNo non-zero → `ret nz`; the class
///     identifies the leader here);
///   * otherwise by data/trainers/encounter_types.asm — EvilTrainerList
///     (Gambler/Rocker/Juggler/Chief/Scientist/Giovanni/Rocket/unused
///     Juggler) → MUSIC_MEET_EVIL_TRAINER; FemaleTrainerList
///     (Lass/Jr.Trainer♀/Beauty/CooltrainerF) → MUSIC_MEET_FEMALE_TRAINER;
///     default MUSIC_MEET_MALE_TRAINER.
pub fn encounter_music(class: TrainerClass) -> Option<MusicId> {
    use TrainerClass::*;
    match class {
        Rival1 | Rival2 | Rival3 => None, // keep current music
        Brock | Misty | LtSurge | Erika | Koga | Sabrina | Blaine | Giovanni => {
            // Gym leaders (and Giovanni, whose battles set wGymLeaderNo) keep
            // the map music.
            None
        }
        Gambler | Rocker | Juggler | Chief | Scientist | Rocket => {
            Some(MusicId::MeetEvilTrainer)
        }
        Lass | JrTrainerF | Beauty | CooltrainerF => Some(MusicId::MeetFemaleTrainer),
        _ => Some(MusicId::MeetMaleTrainer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rivals_keep_current_music() {
        assert_eq!(encounter_music(TrainerClass::Rival1), None);
        assert_eq!(encounter_music(TrainerClass::Rival3), None);
    }

    #[test]
    fn gym_leaders_keep_map_music() {
        assert_eq!(encounter_music(TrainerClass::Brock), None);
        assert_eq!(encounter_music(TrainerClass::Sabrina), None);
    }

    #[test]
    fn evil_list_gets_meet_evil() {
        // data/trainers/encounter_types.asm EvilTrainerList.
        for c in [
            TrainerClass::Gambler,
            TrainerClass::Rocker,
            TrainerClass::Juggler,
            TrainerClass::Scientist,
            TrainerClass::Rocket,
        ] {
            assert_eq!(encounter_music(c), Some(MusicId::MeetEvilTrainer), "{c:?}");
        }
    }

    #[test]
    fn female_list_gets_meet_female() {
        for c in [
            TrainerClass::Lass,
            TrainerClass::JrTrainerF,
            TrainerClass::Beauty,
            TrainerClass::CooltrainerF,
        ] {
            assert_eq!(encounter_music(c), Some(MusicId::MeetFemaleTrainer), "{c:?}");
        }
    }

    #[test]
    fn everyone_else_gets_meet_male() {
        assert_eq!(encounter_music(TrainerClass::Youngster), Some(MusicId::MeetMaleTrainer));
        assert_eq!(encounter_music(TrainerClass::Hiker), Some(MusicId::MeetMaleTrainer));
        assert_eq!(encounter_music(TrainerClass::Fisher), Some(MusicId::MeetMaleTrainer));
    }
}
