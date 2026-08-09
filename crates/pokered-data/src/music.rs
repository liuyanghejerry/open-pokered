#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum MusicId {
    PalletTown = 0,
    Pokecenter = 1,
    Gym = 2,
    Cities1 = 3,
    Cities2 = 4,
    Celadon = 5,
    Cinnabar = 6,
    Vermilion = 7,
    Lavender = 8,
    SSAnne = 9,
    MeetProfOak = 10,
    MeetRival = 11,
    MuseumGuy = 12,
    SafariZone = 13,
    PkmnHealed = 14,
    Routes1 = 15,
    Routes2 = 16,
    Routes3 = 17,
    Routes4 = 18,
    IndigoPlateau = 19,
    GymLeaderBattle = 20,
    TrainerBattle = 21,
    WildBattle = 22,
    FinalBattle = 23,
    DefeatedTrainer = 24,
    DefeatedWildMon = 25,
    DefeatedGymLeader = 26,
    TitleScreen = 27,
    Credits = 28,
    HallOfFame = 29,
    OaksLab = 30,
    JigglypuffSong = 31,
    BikeRiding = 32,
    Surfing = 33,
    GameCorner = 34,
    IntroBattle = 35,
    Dungeon1 = 36,
    Dungeon2 = 37,
    Dungeon3 = 38,
    CinnabarMansion = 39,
    PokemonTower = 40,
    SilphCo = 41,
    MeetEvilTrainer = 42,
    MeetFemaleTrainer = 43,
    MeetMaleTrainer = 44,
}

pub const NUM_MUSIC_TRACKS: usize = 45;
pub const SFX_STOP_ALL_MUSIC: u8 = 0xFF;

impl MusicId {
    pub fn from_u8(value: u8) -> Option<MusicId> {
        if (value as usize) < NUM_MUSIC_TRACKS {
            Some(unsafe { core::mem::transmute(value) })
        } else {
            None
        }
    }

    pub fn from_name(name: &str) -> Option<MusicId> {
        // Case/underscore-insensitive: "MUSIC_SS_ANNE", "SsAnne", "SS_ANNE"
        // and "SSAnne" all resolve to the same track.
        let norm: String = name
            .chars()
            .filter(|c| *c != '_')
            .flat_map(|c| c.to_lowercase())
            .collect();
        match norm.as_str() {
            "pallettown" => Some(MusicId::PalletTown),
            "pokecenter" => Some(MusicId::Pokecenter),
            "gym" => Some(MusicId::Gym),
            "cities1" => Some(MusicId::Cities1),
            "cities2" => Some(MusicId::Cities2),
            "celadon" => Some(MusicId::Celadon),
            "cinnabar" => Some(MusicId::Cinnabar),
            "vermilion" => Some(MusicId::Vermilion),
            "lavender" => Some(MusicId::Lavender),
            "ssanne" => Some(MusicId::SSAnne),
            "meetprofoak" => Some(MusicId::MeetProfOak),
            "meetrival" => Some(MusicId::MeetRival),
            "museumguy" => Some(MusicId::MuseumGuy),
            "safarizone" => Some(MusicId::SafariZone),
            "pkmnhealed" => Some(MusicId::PkmnHealed),
            "routes1" => Some(MusicId::Routes1),
            "routes2" => Some(MusicId::Routes2),
            "routes3" => Some(MusicId::Routes3),
            "routes4" => Some(MusicId::Routes4),
            "indigoplateau" => Some(MusicId::IndigoPlateau),
            "gymleaderbattle" => Some(MusicId::GymLeaderBattle),
            "trainerbattle" => Some(MusicId::TrainerBattle),
            "wildbattle" => Some(MusicId::WildBattle),
            "finalbattle" => Some(MusicId::FinalBattle),
            "defeatedtrainer" => Some(MusicId::DefeatedTrainer),
            "defeatedwildmon" => Some(MusicId::DefeatedWildMon),
            "defeatedgymleader" => Some(MusicId::DefeatedGymLeader),
            "titlescreen" => Some(MusicId::TitleScreen),
            "credits" => Some(MusicId::Credits),
            "halloffame" => Some(MusicId::HallOfFame),
            "oakslab" => Some(MusicId::OaksLab),
            "jigglypuffsong" => Some(MusicId::JigglypuffSong),
            "bikeriding" => Some(MusicId::BikeRiding),
            "surfing" => Some(MusicId::Surfing),
            "gamecorner" => Some(MusicId::GameCorner),
            "introbattle" => Some(MusicId::IntroBattle),
            "dungeon1" => Some(MusicId::Dungeon1),
            "dungeon2" => Some(MusicId::Dungeon2),
            "dungeon3" => Some(MusicId::Dungeon3),
            "cinnabarmansion" => Some(MusicId::CinnabarMansion),
            "pokemontower" => Some(MusicId::PokemonTower),
            "silphco" => Some(MusicId::SilphCo),
            "meeteviltrainer" => Some(MusicId::MeetEvilTrainer),
            "meetfemaletrainer" => Some(MusicId::MeetFemaleTrainer),
            "meetmaletrainer" => Some(MusicId::MeetMaleTrainer),
            _ => None,
        }
    }

    pub fn variant_name(self) -> &'static str {
        match self {
            MusicId::PalletTown => "PalletTown",
            MusicId::Pokecenter => "Pokecenter",
            MusicId::Gym => "Gym",
            MusicId::Cities1 => "Cities1",
            MusicId::Cities2 => "Cities2",
            MusicId::Celadon => "Celadon",
            MusicId::Cinnabar => "Cinnabar",
            MusicId::Vermilion => "Vermilion",
            MusicId::Lavender => "Lavender",
            MusicId::SSAnne => "SSAnne",
            MusicId::MeetProfOak => "MeetProfOak",
            MusicId::MeetRival => "MeetRival",
            MusicId::MuseumGuy => "MuseumGuy",
            MusicId::SafariZone => "SafariZone",
            MusicId::PkmnHealed => "PkmnHealed",
            MusicId::Routes1 => "Routes1",
            MusicId::Routes2 => "Routes2",
            MusicId::Routes3 => "Routes3",
            MusicId::Routes4 => "Routes4",
            MusicId::IndigoPlateau => "IndigoPlateau",
            MusicId::GymLeaderBattle => "GymLeaderBattle",
            MusicId::TrainerBattle => "TrainerBattle",
            MusicId::WildBattle => "WildBattle",
            MusicId::FinalBattle => "FinalBattle",
            MusicId::DefeatedTrainer => "DefeatedTrainer",
            MusicId::DefeatedWildMon => "DefeatedWildMon",
            MusicId::DefeatedGymLeader => "DefeatedGymLeader",
            MusicId::TitleScreen => "TitleScreen",
            MusicId::Credits => "Credits",
            MusicId::HallOfFame => "HallOfFame",
            MusicId::OaksLab => "OaksLab",
            MusicId::JigglypuffSong => "JigglypuffSong",
            MusicId::BikeRiding => "BikeRiding",
            MusicId::Surfing => "Surfing",
            MusicId::GameCorner => "GameCorner",
            MusicId::IntroBattle => "IntroBattle",
            MusicId::Dungeon1 => "Dungeon1",
            MusicId::Dungeon2 => "Dungeon2",
            MusicId::Dungeon3 => "Dungeon3",
            MusicId::CinnabarMansion => "CinnabarMansion",
            MusicId::PokemonTower => "PokemonTower",
            MusicId::SilphCo => "SilphCo",
            MusicId::MeetEvilTrainer => "MeetEvilTrainer",
            MusicId::MeetFemaleTrainer => "MeetFemaleTrainer",
            MusicId::MeetMaleTrainer => "MeetMaleTrainer",
        }
    }
}
