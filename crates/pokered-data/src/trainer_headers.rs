use crate::event_flags::EventFlag;
use crate::maps::MapId;

#[derive(Debug, Clone, Copy)]
pub struct TrainerHeaderData {
    pub event_flag: EventFlag,
    pub sight_range: u8,
}

pub static TRAINERS_AGATHAS_ROOM: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_AGATHAS_ROOM_TRAINER_0,
    sight_range: 0,
}];

pub static TRAINERS_BRUNOS_ROOM: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_BRUNOS_ROOM_TRAINER_0,
    sight_range: 0,
}];

pub static TRAINERS_CELADON_GYM: [TrainerHeaderData; 7] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_3,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_4,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_5,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_6,
        sight_range: 3,
    },
];

pub static TRAINERS_CERULEAN_CAVE_B1_F: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_MEWTWO,
    sight_range: 0,
}];

pub static TRAINERS_CERULEAN_GYM: [TrainerHeaderData; 2] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_CERULEAN_GYM_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_CERULEAN_GYM_TRAINER_1,
        sight_range: 3,
    },
];

pub static TRAINERS_FIGHTING_DOJO: [TrainerHeaderData; 4] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_3,
        sight_range: 3,
    },
];

pub static TRAINERS_FUCHSIA_GYM: [TrainerHeaderData; 6] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_3,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_4,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_5,
        sight_range: 2,
    },
];

pub static TRAINERS_LANCES_ROOM: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_LANCES_ROOM_TRAINER_0,
    sight_range: 0,
}];

pub static TRAINERS_LORELEIS_ROOM: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_LORELEIS_ROOM_TRAINER_0,
    sight_range: 0,
}];

pub static TRAINERS_MT_MOON1_F: [TrainerHeaderData; 7] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_3,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_4,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_5,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_6,
        sight_range: 3,
    },
];

pub static TRAINERS_MT_MOON_B2_F: [TrainerHeaderData; 4] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_3,
        sight_range: 4,
    },
];

pub static TRAINERS_PEWTER_GYM: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0,
    sight_range: 5,
}];

pub static TRAINERS_POKEMON_MANSION1_F: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_MANSION_1_TRAINER_0,
    sight_range: 3,
}];

pub static TRAINERS_POKEMON_MANSION2_F: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_MANSION_2_TRAINER_0,
    sight_range: 0,
}];

pub static TRAINERS_POKEMON_MANSION3_F: [TrainerHeaderData; 2] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MANSION_3_TRAINER_0,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MANSION_3_TRAINER_1,
        sight_range: 2,
    },
];

pub static TRAINERS_POKEMON_MANSION_B1_F: [TrainerHeaderData; 2] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MANSION_4_TRAINER_0,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MANSION_4_TRAINER_1,
        sight_range: 3,
    },
];

pub static TRAINERS_POKEMON_TOWER3_F: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_2,
        sight_range: 2,
    },
];

pub static TRAINERS_POKEMON_TOWER4_F: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_2,
        sight_range: 2,
    },
];

pub static TRAINERS_POKEMON_TOWER5_F: [TrainerHeaderData; 4] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_2,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_3,
        sight_range: 2,
    },
];

pub static TRAINERS_POKEMON_TOWER6_F: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_2,
        sight_range: 2,
    },
];

pub static TRAINERS_POKEMON_TOWER7_F: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_2,
        sight_range: 3,
    },
];

pub static TRAINERS_POWER_PLANT: [TrainerHeaderData; 9] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_0,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_1,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_2,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_3,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_4,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_5,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_6,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_7,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ZAPDOS,
        sight_range: 0,
    },
];

pub static TRAINERS_ROCK_TUNNEL1_F: [TrainerHeaderData; 7] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_3,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_4,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_5,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_6,
        sight_range: 4,
    },
];

pub static TRAINERS_ROCK_TUNNEL_B1_F: [TrainerHeaderData; 8] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_3,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_4,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_5,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_6,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_7,
        sight_range: 3,
    },
];

pub static TRAINERS_ROCKET_HIDEOUT_B1_F: [TrainerHeaderData; 5] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_2,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_3,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_4,
        sight_range: 3,
    },
];

pub static TRAINERS_ROCKET_HIDEOUT_B2_F: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_2_TRAINER_0,
    sight_range: 4,
}];

pub static TRAINERS_ROCKET_HIDEOUT_B3_F: [TrainerHeaderData; 2] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_1,
        sight_range: 4,
    },
];

pub static TRAINERS_ROCKET_HIDEOUT_B4_F: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_1,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2,
        sight_range: 1,
    },
];

pub static TRAINERS_ROUTE10: [TrainerHeaderData; 6] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_3,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_4,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_5,
        sight_range: 2,
    },
];

pub static TRAINERS_ROUTE11: [TrainerHeaderData; 10] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_3,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_4,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_5,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_6,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_7,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_8,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_9,
        sight_range: 4,
    },
];

pub static TRAINERS_ROUTE12: [TrainerHeaderData; 7] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_3,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_4,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_5,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_6,
        sight_range: 1,
    },
];

pub static TRAINERS_ROUTE13: [TrainerHeaderData; 10] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_2,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_3,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_4,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_5,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_6,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_7,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_8,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_9,
        sight_range: 4,
    },
];

pub static TRAINERS_ROUTE14: [TrainerHeaderData; 10] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_3,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_4,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_5,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_6,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_7,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_8,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_9,
        sight_range: 4,
    },
];

pub static TRAINERS_ROUTE15: [TrainerHeaderData; 10] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_3,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_4,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_5,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_6,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_7,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_8,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_9,
        sight_range: 3,
    },
];

pub static TRAINERS_ROUTE16: [TrainerHeaderData; 6] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_2,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_3,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_4,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_5,
        sight_range: 4,
    },
];

pub static TRAINERS_ROUTE17: [TrainerHeaderData; 10] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_3,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_4,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_5,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_6,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_7,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_8,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_9,
        sight_range: 4,
    },
];

pub static TRAINERS_ROUTE18: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_2,
        sight_range: 4,
    },
];

pub static TRAINERS_ROUTE19: [TrainerHeaderData; 10] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_3,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_4,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_5,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_6,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_7,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_8,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_9,
        sight_range: 4,
    },
];

pub static TRAINERS_ROUTE20: [TrainerHeaderData; 10] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_2,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_3,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_4,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_5,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_6,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_7,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_8,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_9,
        sight_range: 4,
    },
];

pub static TRAINERS_ROUTE21: [TrainerHeaderData; 9] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_0,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_1,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_3,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_4,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_5,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_6,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_7,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_8,
        sight_range: 0,
    },
];

pub static TRAINERS_ROUTE24: [TrainerHeaderData; 6] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_1,
        sight_range: 1,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_2,
        sight_range: 1,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_3,
        sight_range: 1,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_4,
        sight_range: 1,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_5,
        sight_range: 1,
    },
];

pub static TRAINERS_ROUTE25: [TrainerHeaderData; 9] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_3,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_4,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_5,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_6,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_7,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_8,
        sight_range: 2,
    },
];

pub static TRAINERS_ROUTE3: [TrainerHeaderData; 8] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_2,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_3,
        sight_range: 1,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_4,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_5,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_6,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_7,
        sight_range: 2,
    },
];

pub static TRAINERS_ROUTE4: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_ROUTE_4_TRAINER_0,
    sight_range: 3,
}];

pub static TRAINERS_ROUTE6: [TrainerHeaderData; 6] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_0,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_1,
        sight_range: 0,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_3,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_4,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_5,
        sight_range: 3,
    },
];

pub static TRAINERS_ROUTE8: [TrainerHeaderData; 9] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_3,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_4,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_5,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_6,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_7,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_8,
        sight_range: 4,
    },
];

pub static TRAINERS_ROUTE9: [TrainerHeaderData; 9] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_3,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_4,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_5,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_6,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_7,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_8,
        sight_range: 2,
    },
];

pub static TRAINERS_S_S_ANNE1_F_ROOMS: [TrainerHeaderData; 4] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_2,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_3,
        sight_range: 2,
    },
];

pub static TRAINERS_S_S_ANNE2_F_ROOMS: [TrainerHeaderData; 4] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_3,
        sight_range: 2,
    },
];

pub static TRAINERS_S_S_ANNE_B1_F_ROOMS: [TrainerHeaderData; 6] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_2,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_3,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_4,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_5,
        sight_range: 3,
    },
];

pub static TRAINERS_S_S_ANNE_BOW: [TrainerHeaderData; 2] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_5_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SS_ANNE_5_TRAINER_1,
        sight_range: 3,
    },
];

pub static TRAINERS_SAFFRON_GYM: [TrainerHeaderData; 7] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_3,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_4,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_5,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_6,
        sight_range: 3,
    },
];

pub static TRAINERS_SEAFOAM_ISLANDS_B4_F: [TrainerHeaderData; 1] = [TrainerHeaderData {
    event_flag: EventFlag::EVENT_BEAT_ARTICUNO,
    sight_range: 0,
}];

pub static TRAINERS_SILPH_CO10_F: [TrainerHeaderData; 2] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_10F_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_10F_TRAINER_1,
        sight_range: 4,
    },
];

pub static TRAINERS_SILPH_CO11_F: [TrainerHeaderData; 2] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_11F_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_11F_TRAINER_1,
        sight_range: 3,
    },
];

pub static TRAINERS_SILPH_CO2_F: [TrainerHeaderData; 4] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_3,
        sight_range: 3,
    },
];

pub static TRAINERS_SILPH_CO3_F: [TrainerHeaderData; 2] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_3F_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_3F_TRAINER_1,
        sight_range: 3,
    },
];

pub static TRAINERS_SILPH_CO4_F: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_2,
        sight_range: 4,
    },
];

pub static TRAINERS_SILPH_CO5_F: [TrainerHeaderData; 4] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_0,
        sight_range: 1,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_3,
        sight_range: 3,
    },
];

pub static TRAINERS_SILPH_CO6_F: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_2,
        sight_range: 2,
    },
];

pub static TRAINERS_SILPH_CO7_F: [TrainerHeaderData; 4] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_3,
        sight_range: 4,
    },
];

pub static TRAINERS_SILPH_CO8_F: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_2,
        sight_range: 4,
    },
];

pub static TRAINERS_SILPH_CO9_F: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_2,
        sight_range: 4,
    },
];

pub static TRAINERS_VERMILION_GYM: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_0,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_1,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_2,
        sight_range: 3,
    },
];

pub static TRAINERS_VICTORY_ROAD1_F: [TrainerHeaderData; 2] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_1_TRAINER_0,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_1_TRAINER_1,
        sight_range: 2,
    },
];

pub static TRAINERS_VICTORY_ROAD2_F: [TrainerHeaderData; 6] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_1,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_2,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_3,
        sight_range: 1,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_4,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_MOLTRES,
        sight_range: 0,
    },
];

pub static TRAINERS_VICTORY_ROAD3_F: [TrainerHeaderData; 4] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_0,
        sight_range: 1,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_3,
        sight_range: 4,
    },
];

pub static TRAINERS_VIRIDIAN_FOREST: [TrainerHeaderData; 3] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2,
        sight_range: 1,
    },
];

pub static TRAINERS_VIRIDIAN_GYM: [TrainerHeaderData; 8] = [
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_0,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_1,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_2,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_3,
        sight_range: 2,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_4,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_5,
        sight_range: 4,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_6,
        sight_range: 3,
    },
    TrainerHeaderData {
        event_flag: EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_7,
        sight_range: 4,
    },
];

pub fn get_trainer_headers(map: MapId) -> &'static [TrainerHeaderData] {
    match map {
        MapId::AgathasRoom => &TRAINERS_AGATHAS_ROOM,
        MapId::BrunosRoom => &TRAINERS_BRUNOS_ROOM,
        MapId::CeladonGym => &TRAINERS_CELADON_GYM,
        MapId::CeruleanCaveB1F => &TRAINERS_CERULEAN_CAVE_B1_F,
        MapId::CeruleanGym => &TRAINERS_CERULEAN_GYM,
        MapId::FightingDojo => &TRAINERS_FIGHTING_DOJO,
        MapId::FuchsiaGym => &TRAINERS_FUCHSIA_GYM,
        MapId::LancesRoom => &TRAINERS_LANCES_ROOM,
        MapId::LoreleisRoom => &TRAINERS_LORELEIS_ROOM,
        MapId::MtMoon1F => &TRAINERS_MT_MOON1_F,
        MapId::MtMoonB2F => &TRAINERS_MT_MOON_B2_F,
        MapId::PewterGym => &TRAINERS_PEWTER_GYM,
        MapId::PokemonMansion1F => &TRAINERS_POKEMON_MANSION1_F,
        MapId::PokemonMansion2F => &TRAINERS_POKEMON_MANSION2_F,
        MapId::PokemonMansion3F => &TRAINERS_POKEMON_MANSION3_F,
        MapId::PokemonMansionB1F => &TRAINERS_POKEMON_MANSION_B1_F,
        MapId::PokemonTower3F => &TRAINERS_POKEMON_TOWER3_F,
        MapId::PokemonTower4F => &TRAINERS_POKEMON_TOWER4_F,
        MapId::PokemonTower5F => &TRAINERS_POKEMON_TOWER5_F,
        MapId::PokemonTower6F => &TRAINERS_POKEMON_TOWER6_F,
        MapId::PokemonTower7F => &TRAINERS_POKEMON_TOWER7_F,
        MapId::PowerPlant => &TRAINERS_POWER_PLANT,
        MapId::RockTunnel1F => &TRAINERS_ROCK_TUNNEL1_F,
        MapId::RockTunnelB1F => &TRAINERS_ROCK_TUNNEL_B1_F,
        MapId::RocketHideoutB1F => &TRAINERS_ROCKET_HIDEOUT_B1_F,
        MapId::RocketHideoutB2F => &TRAINERS_ROCKET_HIDEOUT_B2_F,
        MapId::RocketHideoutB3F => &TRAINERS_ROCKET_HIDEOUT_B3_F,
        MapId::RocketHideoutB4F => &TRAINERS_ROCKET_HIDEOUT_B4_F,
        MapId::Route10 => &TRAINERS_ROUTE10,
        MapId::Route11 => &TRAINERS_ROUTE11,
        MapId::Route12 => &TRAINERS_ROUTE12,
        MapId::Route13 => &TRAINERS_ROUTE13,
        MapId::Route14 => &TRAINERS_ROUTE14,
        MapId::Route15 => &TRAINERS_ROUTE15,
        MapId::Route16 => &TRAINERS_ROUTE16,
        MapId::Route17 => &TRAINERS_ROUTE17,
        MapId::Route18 => &TRAINERS_ROUTE18,
        MapId::Route19 => &TRAINERS_ROUTE19,
        MapId::Route20 => &TRAINERS_ROUTE20,
        MapId::Route21 => &TRAINERS_ROUTE21,
        MapId::Route24 => &TRAINERS_ROUTE24,
        MapId::Route25 => &TRAINERS_ROUTE25,
        MapId::Route3 => &TRAINERS_ROUTE3,
        MapId::Route4 => &TRAINERS_ROUTE4,
        MapId::Route6 => &TRAINERS_ROUTE6,
        MapId::Route8 => &TRAINERS_ROUTE8,
        MapId::Route9 => &TRAINERS_ROUTE9,
        MapId::SSAnne1FRooms => &TRAINERS_S_S_ANNE1_F_ROOMS,
        MapId::SSAnne2FRooms => &TRAINERS_S_S_ANNE2_F_ROOMS,
        MapId::SSAnneB1FRooms => &TRAINERS_S_S_ANNE_B1_F_ROOMS,
        MapId::SSAnneBow => &TRAINERS_S_S_ANNE_BOW,
        MapId::SaffronGym => &TRAINERS_SAFFRON_GYM,
        MapId::SeafoamIslandsB4F => &TRAINERS_SEAFOAM_ISLANDS_B4_F,
        MapId::SilphCo10F => &TRAINERS_SILPH_CO10_F,
        MapId::SilphCo11F => &TRAINERS_SILPH_CO11_F,
        MapId::SilphCo2F => &TRAINERS_SILPH_CO2_F,
        MapId::SilphCo3F => &TRAINERS_SILPH_CO3_F,
        MapId::SilphCo4F => &TRAINERS_SILPH_CO4_F,
        MapId::SilphCo5F => &TRAINERS_SILPH_CO5_F,
        MapId::SilphCo6F => &TRAINERS_SILPH_CO6_F,
        MapId::SilphCo7F => &TRAINERS_SILPH_CO7_F,
        MapId::SilphCo8F => &TRAINERS_SILPH_CO8_F,
        MapId::SilphCo9F => &TRAINERS_SILPH_CO9_F,
        MapId::VermilionGym => &TRAINERS_VERMILION_GYM,
        MapId::VictoryRoad1F => &TRAINERS_VICTORY_ROAD1_F,
        MapId::VictoryRoad2F => &TRAINERS_VICTORY_ROAD2_F,
        MapId::VictoryRoad3F => &TRAINERS_VICTORY_ROAD3_F,
        MapId::ViridianForest => &TRAINERS_VIRIDIAN_FOREST,
        MapId::ViridianGym => &TRAINERS_VIRIDIAN_GYM,
        _ => &[],
    }
}
