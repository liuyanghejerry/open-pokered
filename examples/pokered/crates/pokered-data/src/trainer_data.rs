use crate::species::Species;
use serde::{Deserialize, Serialize};

/// A trainer class identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TrainerClass {
    Nobody = 0,
    Youngster = 1,
    BugCatcher = 2,
    Lass = 3,
    Sailor = 4,
    JrTrainerM = 5,
    JrTrainerF = 6,
    Pokemaniac = 7,
    SuperNerd = 8,
    Hiker = 9,
    Biker = 10,
    Burglar = 11,
    Engineer = 12,
    UnusedJuggler = 13,
    Fisher = 14,
    Swimmer = 15,
    CueBall = 16,
    Gambler = 17,
    Beauty = 18,
    PsychicTr = 19,
    Rocker = 20,
    Juggler = 21,
    Tamer = 22,
    BirdKeeper = 23,
    Blackbelt = 24,
    Rival1 = 25,
    ProfOak = 26,
    Chief = 27,
    Scientist = 28,
    Giovanni = 29,
    Rocket = 30,
    CooltrainerM = 31,
    CooltrainerF = 32,
    Bruno = 33,
    Brock = 34,
    Misty = 35,
    LtSurge = 36,
    Erika = 37,
    Koga = 38,
    Blaine = 39,
    Sabrina = 40,
    Gentleman = 41,
    Rival2 = 42,
    Rival3 = 43,
    Lorelei = 44,
    Channeler = 45,
    Agatha = 46,
    Lance = 47,
}

impl TrainerClass {
    pub fn from_u8(value: u8) -> Self {
        if value <= TrainerClass::Lance as u8 {
            unsafe { std::mem::transmute(value) }
        } else {
            TrainerClass::Nobody
        }
    }

    /// Return the sprite filename (without extension) matching gfx/trainers/*.png.
    pub fn sprite_name(&self) -> &'static str {
        match self {
            TrainerClass::Nobody => "youngster",
            TrainerClass::Youngster => "youngster",
            TrainerClass::BugCatcher => "bugcatcher",
            TrainerClass::Lass => "lass",
            TrainerClass::Sailor => "sailor",
            TrainerClass::JrTrainerM => "jr.trainerm",
            TrainerClass::JrTrainerF => "jr.trainerf",
            TrainerClass::Pokemaniac => "pokemaniac",
            TrainerClass::SuperNerd => "supernerd",
            TrainerClass::Hiker => "hiker",
            TrainerClass::Biker => "biker",
            TrainerClass::Burglar => "burglar",
            TrainerClass::Engineer => "engineer",
            TrainerClass::UnusedJuggler => "juggler",
            TrainerClass::Fisher => "fisher",
            TrainerClass::Swimmer => "swimmer",
            TrainerClass::CueBall => "cueball",
            TrainerClass::Gambler => "gambler",
            TrainerClass::Beauty => "beauty",
            TrainerClass::PsychicTr => "psychic",
            TrainerClass::Rocker => "rocker",
            TrainerClass::Juggler => "juggler",
            TrainerClass::Tamer => "tamer",
            TrainerClass::BirdKeeper => "birdkeeper",
            TrainerClass::Blackbelt => "blackbelt",
            TrainerClass::Rival1 => "rival1",
            TrainerClass::ProfOak => "prof.oak",
            TrainerClass::Chief => "scientist",
            TrainerClass::Scientist => "scientist",
            TrainerClass::Giovanni => "giovanni",
            TrainerClass::Rocket => "rocket",
            TrainerClass::CooltrainerM => "cooltrainerm",
            TrainerClass::CooltrainerF => "cooltrainerf",
            TrainerClass::Bruno => "bruno",
            TrainerClass::Brock => "brock",
            TrainerClass::Misty => "misty",
            TrainerClass::LtSurge => "lt.surge",
            TrainerClass::Erika => "erika",
            TrainerClass::Koga => "koga",
            TrainerClass::Blaine => "blaine",
            TrainerClass::Sabrina => "sabrina",
            TrainerClass::Gentleman => "gentleman",
            TrainerClass::Rival2 => "rival2",
            TrainerClass::Rival3 => "rival3",
            TrainerClass::Lorelei => "lorelei",
            TrainerClass::Channeler => "channeler",
            TrainerClass::Agatha => "agatha",
            TrainerClass::Lance => "lance",
        }
    }

    /// Return the display name for battle text (e.g. "BUG CATCHER", "LT.SURGE").
    pub fn display_name(&self) -> &'static str {
        match self {
            TrainerClass::Nobody => "TRAINER",
            TrainerClass::Youngster => "YOUNGSTER",
            TrainerClass::BugCatcher => "BUG CATCHER",
            TrainerClass::Lass => "LASS",
            TrainerClass::Sailor => "SAILOR",
            TrainerClass::JrTrainerM => "JR.TRAINER",
            TrainerClass::JrTrainerF => "JR.TRAINER",
            TrainerClass::Pokemaniac => "POKeMANIAC",
            TrainerClass::SuperNerd => "SUPER NERD",
            TrainerClass::Hiker => "HIKER",
            TrainerClass::Biker => "BIKER",
            TrainerClass::Burglar => "BURGLAR",
            TrainerClass::Engineer => "ENGINEER",
            TrainerClass::UnusedJuggler => "JUGGLER",
            TrainerClass::Fisher => "FISHER",
            TrainerClass::Swimmer => "SWIMMER",
            TrainerClass::CueBall => "CUE BALL",
            TrainerClass::Gambler => "GAMBLER",
            TrainerClass::Beauty => "BEAUTY",
            TrainerClass::PsychicTr => "PSYCHIC",
            TrainerClass::Rocker => "ROCKER",
            TrainerClass::Juggler => "JUGGLER",
            TrainerClass::Tamer => "TAMER",
            TrainerClass::BirdKeeper => "BIRD KEEPER",
            TrainerClass::Blackbelt => "BLACKBELT",
            TrainerClass::Rival1 => "RIVAL",
            TrainerClass::ProfOak => "PROF.OAK",
            TrainerClass::Chief => "SCIENTIST",
            TrainerClass::Scientist => "SCIENTIST",
            TrainerClass::Giovanni => "GIOVANNI",
            TrainerClass::Rocket => "ROCKET",
            TrainerClass::CooltrainerM => "COOLTRAINER",
            TrainerClass::CooltrainerF => "COOLTRAINER",
            TrainerClass::Bruno => "BRUNO",
            TrainerClass::Brock => "BROCK",
            TrainerClass::Misty => "MISTY",
            TrainerClass::LtSurge => "LT.SURGE",
            TrainerClass::Erika => "ERIKA",
            TrainerClass::Koga => "KOGA",
            TrainerClass::Blaine => "BLAINE",
            TrainerClass::Sabrina => "SABRINA",
            TrainerClass::Gentleman => "GENTLEMAN",
            TrainerClass::Rival2 => "RIVAL",
            TrainerClass::Rival3 => "RIVAL",
            TrainerClass::Lorelei => "LORELEI",
            TrainerClass::Channeler => "CHANNELER",
            TrainerClass::Agatha => "AGATHA",
            TrainerClass::Lance => "LANCE",
        }
    }
}

pub const NUM_TRAINER_CLASSES: u8 = 47;

/// A single Pokémon in a trainer's party
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerMon {
    pub level: u8,
    pub species: Species,
}

/// A trainer party (one encounter instance)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerParty {
    pub pokemon: Vec<TrainerMon>,
}

/// All parties for a trainer class (multiple trainers of the same class)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerClassData {
    pub class: TrainerClass,
    pub parties: Vec<TrainerParty>,
}

/// Get all trainer class data
pub fn trainer_data() -> Vec<TrainerClassData> {
    include!(concat!(env!("OUT_DIR"), "/trainer_data_gen.rs"))
}

/// Base money reward per trainer class (in yen / currency units).
/// Prize money = base_money × level of last enemy Pokémon.
/// Data from data/trainers/pic_pointers_money.asm.
pub fn get_base_money(class: TrainerClass) -> u16 {
    match class {
        TrainerClass::Nobody => 0,
        TrainerClass::Youngster => 15,
        TrainerClass::BugCatcher => 10,
        TrainerClass::Lass => 15,
        TrainerClass::Sailor => 30,
        TrainerClass::JrTrainerM => 20,
        TrainerClass::JrTrainerF => 20,
        TrainerClass::Pokemaniac => 50,
        TrainerClass::SuperNerd => 25,
        TrainerClass::Hiker => 35,
        TrainerClass::Biker => 20,
        TrainerClass::Burglar => 90,
        TrainerClass::Engineer => 50,
        TrainerClass::UnusedJuggler => 35,
        TrainerClass::Fisher => 35,
        TrainerClass::Swimmer => 5,
        TrainerClass::CueBall => 25,
        TrainerClass::Gambler => 70,
        TrainerClass::Beauty => 70,
        TrainerClass::PsychicTr => 10,
        TrainerClass::Rocker => 25,
        TrainerClass::Juggler => 35,
        TrainerClass::Tamer => 40,
        TrainerClass::BirdKeeper => 25,
        TrainerClass::Blackbelt => 25,
        TrainerClass::Rival1 => 35,
        TrainerClass::ProfOak => 99,
        TrainerClass::Chief => 30,
        TrainerClass::Scientist => 50,
        TrainerClass::Giovanni => 99,
        TrainerClass::Rocket => 30,
        TrainerClass::CooltrainerM => 35,
        TrainerClass::CooltrainerF => 35,
        TrainerClass::Bruno => 99,
        TrainerClass::Brock => 99,
        TrainerClass::Misty => 99,
        TrainerClass::LtSurge => 99,
        TrainerClass::Erika => 99,
        TrainerClass::Koga => 99,
        TrainerClass::Blaine => 99,
        TrainerClass::Sabrina => 99,
        TrainerClass::Gentleman => 70,
        TrainerClass::Rival2 => 65,
        TrainerClass::Rival3 => 99,
        TrainerClass::Lorelei => 99,
        TrainerClass::Channeler => 30,
        TrainerClass::Agatha => 99,
        TrainerClass::Lance => 99,
    }
}

/// Get a specific trainer party by class and party index.
/// Returns None if the class or party index is invalid.
pub fn get_trainer_party(class: TrainerClass, party_index: usize) -> Option<&'static TrainerParty> {
    // Editor-injected runtime override shadows the baseline.
    if let Some(ov) = crate::runtime_overrides::trainer_override(class) {
        return ov.parties.get(party_index);
    }
    static TRAINER_DATA: std::sync::OnceLock<Vec<TrainerClassData>> = std::sync::OnceLock::new();
    let data = TRAINER_DATA.get_or_init(trainer_data);

    data.iter()
        .find(|c| c.class == class)
        .and_then(|c| c.parties.get(party_index))
}

/// Parse a trainer ID string like "OPP_RIVAL1" into (TrainerClass, party_index).
/// The format is "OPP_<CLASSNAME><optional_number>".
/// For example: "OPP_RIVAL1" -> (TrainerClass::Rival1, 0)
///              "OPP_YOUNGSTER" -> (TrainerClass::Youngster, 0)
pub fn parse_trainer_id(trainer_id: &str) -> Option<(TrainerClass, usize)> {
    if !trainer_id.starts_with("OPP_") {
        return None;
    }

    let name = &trainer_id[4..];

    let digit_count = name
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .count();

    let (class_name, party_index) = if digit_count > 0 {
        let split_point = name.len() - digit_count;
        let class_part = &name[..split_point];
        let index_part = &name[split_point..];
        let index = index_part.parse::<usize>().ok()?;
        (class_part, index.saturating_sub(1))
    } else {
        (name, 0)
    };

    let class = match class_name {
        "RIVAL" => TrainerClass::Rival1,
        "RIVAL1" => TrainerClass::Rival1,
        "RIVAL2" => TrainerClass::Rival2,
        "RIVAL3" => TrainerClass::Rival3,
        "YOUNGSTER" => TrainerClass::Youngster,
        "BUG_CATCHER" => TrainerClass::BugCatcher,
        "LASS" => TrainerClass::Lass,
        "SAILOR" => TrainerClass::Sailor,
        "JR_TRAINER_M" => TrainerClass::JrTrainerM,
        "JR_TRAINER_F" => TrainerClass::JrTrainerF,
        "POKEMANIAC" => TrainerClass::Pokemaniac,
        "SUPER_NERD" => TrainerClass::SuperNerd,
        "HIKER" => TrainerClass::Hiker,
        "BIKER" => TrainerClass::Biker,
        "BURGLAR" => TrainerClass::Burglar,
        "ENGINEER" => TrainerClass::Engineer,
        "FISHER" => TrainerClass::Fisher,
        "SWIMMER" => TrainerClass::Swimmer,
        "CUE_BALL" => TrainerClass::CueBall,
        "GAMBLER" => TrainerClass::Gambler,
        "BEAUTY" => TrainerClass::Beauty,
        "PSYCHIC" => TrainerClass::PsychicTr,
        "ROCKER" => TrainerClass::Rocker,
        "JUGGLER" => TrainerClass::Juggler,
        "TAMER" => TrainerClass::Tamer,
        "BIRD_KEEPER" => TrainerClass::BirdKeeper,
        "BLACKBELT" => TrainerClass::Blackbelt,
        "OAK" => TrainerClass::ProfOak,
        "SCIENTIST" => TrainerClass::Scientist,
        "GIOVANNI" => TrainerClass::Giovanni,
        "ROCKET" => TrainerClass::Rocket,
        "COOLTRAINER_M" => TrainerClass::CooltrainerM,
        "COOLTRAINER_F" => TrainerClass::CooltrainerF,
        "BRUNO" => TrainerClass::Bruno,
        "BROCK" => TrainerClass::Brock,
        "MISTY" => TrainerClass::Misty,
        "LT_SURGE" => TrainerClass::LtSurge,
        "ERIKA" => TrainerClass::Erika,
        "KOGA" => TrainerClass::Koga,
        "BLAINE" => TrainerClass::Blaine,
        "SABRINA" => TrainerClass::Sabrina,
        "GENTLEMAN" => TrainerClass::Gentleman,
        "LORELEI" => TrainerClass::Lorelei,
        "CHANNELER" => TrainerClass::Channeler,
        "AGATHA" => TrainerClass::Agatha,
        "LANCE" => TrainerClass::Lance,
        _ => return None,
    };

    Some((class, party_index))
}

/// Convert trainer class + set index into an OPP_-style trainer ID string,
/// e.g. `(Youngster, 0)` → `"OPP_YOUNGSTER1"`.
pub fn make_trainer_id(class: TrainerClass, set: u8) -> String {
    let name = trainer_class_name(class);
    format!("OPP_{}{}", name, set + 1)
}

/// Return the uppercase string name for a trainer class (without OPP_ prefix).
pub fn trainer_class_name(class: TrainerClass) -> &'static str {
    match class {
        TrainerClass::Nobody => "NOBODY",
        TrainerClass::Youngster => "YOUNGSTER",
        TrainerClass::BugCatcher => "BUG_CATCHER",
        TrainerClass::Lass => "LASS",
        TrainerClass::Sailor => "SAILOR",
        TrainerClass::JrTrainerM => "JR_TRAINER_M",
        TrainerClass::JrTrainerF => "JR_TRAINER_F",
        TrainerClass::Pokemaniac => "POKEMANIAC",
        TrainerClass::SuperNerd => "SUPER_NERD",
        TrainerClass::Hiker => "HIKER",
        TrainerClass::Biker => "BIKER",
        TrainerClass::Burglar => "BURGLAR",
        TrainerClass::Engineer => "ENGINEER",
        TrainerClass::UnusedJuggler => "JUGGLER",
        TrainerClass::Fisher => "FISHER",
        TrainerClass::Swimmer => "SWIMMER",
        TrainerClass::CueBall => "CUE_BALL",
        TrainerClass::Gambler => "GAMBLER",
        TrainerClass::Beauty => "BEAUTY",
        TrainerClass::PsychicTr => "PSYCHIC",
        TrainerClass::Rocker => "ROCKER",
        TrainerClass::Juggler => "JUGGLER",
        TrainerClass::Tamer => "TAMER",
        TrainerClass::BirdKeeper => "BIRD_KEEPER",
        TrainerClass::Blackbelt => "BLACKBELT",
        TrainerClass::Rival1 => "RIVAL1",
        TrainerClass::ProfOak => "OAK",
        TrainerClass::Chief => "CHIEF",
        TrainerClass::Scientist => "SCIENTIST",
        TrainerClass::Giovanni => "GIOVANNI",
        TrainerClass::Rocket => "ROCKET",
        TrainerClass::CooltrainerM => "COOLTRAINER_M",
        TrainerClass::CooltrainerF => "COOLTRAINER_F",
        TrainerClass::Bruno => "BRUNO",
        TrainerClass::Brock => "BROCK",
        TrainerClass::Misty => "MISTY",
        TrainerClass::LtSurge => "LT_SURGE",
        TrainerClass::Erika => "ERIKA",
        TrainerClass::Koga => "KOGA",
        TrainerClass::Blaine => "BLAINE",
        TrainerClass::Sabrina => "SABRINA",
        TrainerClass::Gentleman => "GENTLEMAN",
        TrainerClass::Rival2 => "RIVAL2",
        TrainerClass::Rival3 => "RIVAL3",
        TrainerClass::Lorelei => "LORELEI",
        TrainerClass::Channeler => "CHANNELER",
        TrainerClass::Agatha => "AGATHA",
        TrainerClass::Lance => "LANCE",
    }
}
