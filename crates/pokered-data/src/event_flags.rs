//! Event flag definitions — auto-generated from event_constants.asm
//!
//! DO NOT EDIT MANUALLY. Run scripts/parse_event_flags.py to regenerate.

/// Size of the original game's `wEventFlags` array, in bytes.
///
/// The original `wram.asm` declares `wEventFlags:: flag_array NUM_EVENTS`
/// with `NUM_EVENTS = $A00` (2560) bits, so the array is
/// `($A00 + 7) / 8 = 320` bytes. The highest *defined* flag (0x9DA) only
/// needs 316 bytes, but SRAM always holds the full 320-byte region, and
/// bits beyond the last named flag are still addressable — so the bitset
/// storage must match the original 320-byte layout.
pub const EVENT_FLAGS_SIZE: usize = 320;

/// Total number of defined event flags.
pub const NUM_EVENT_FLAGS: usize = 507;

/// Event flag identifiers with their bit indices.
///
/// Each variant maps to a specific bit position in the wEventFlags array.
/// Use `EventFlag::bit_index()` to get the bit position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum EventFlag {
    /// Bit index: 0x000 (0)
    EVENT_FOLLOWED_OAK_INTO_LAB = 0x000,
    /// Bit index: 0x003 (3)
    EVENT_HALL_OF_FAME_DEX_RATING = 0x003,
    /// Bit index: 0x006 (6)
    EVENT_PALLET_AFTER_GETTING_POKEBALLS = 0x006,
    /// Bit index: 0x018 (24)
    EVENT_GOT_TOWN_MAP = 0x018,
    /// Bit index: 0x019 (25)
    EVENT_ENTERED_BLUES_HOUSE = 0x019,
    /// Bit index: 0x01A (26)
    EVENT_DAISY_WALKING = 0x01A,
    /// Bit index: 0x020 (32)
    EVENT_FOLLOWED_OAK_INTO_LAB_2 = 0x020,
    /// Bit index: 0x021 (33)
    EVENT_OAK_ASKED_TO_CHOOSE_MON = 0x021,
    /// Bit index: 0x022 (34)
    EVENT_GOT_STARTER = 0x022,
    /// Bit index: 0x023 (35)
    EVENT_BATTLED_RIVAL_IN_OAKS_LAB = 0x023,
    /// Bit index: 0x024 (36)
    EVENT_GOT_POKEBALLS_FROM_OAK = 0x024,
    /// Bit index: 0x025 (37)
    EVENT_GOT_POKEDEX = 0x025,
    /// Bit index: 0x026 (38)
    EVENT_PALLET_AFTER_GETTING_POKEBALLS_2 = 0x026,
    /// Bit index: 0x027 (39)
    EVENT_OAK_APPEARED_IN_PALLET = 0x027,
    /// Bit index: 0x028 (40)
    EVENT_VIRIDIAN_GYM_OPEN = 0x028,
    /// Bit index: 0x029 (41)
    EVENT_GOT_TM42 = 0x029,
    /// Bit index: 0x038 (56)
    EVENT_OAK_GOT_PARCEL = 0x038,
    /// Bit index: 0x039 (57)
    EVENT_GOT_OAKS_PARCEL = 0x039,
    /// Bit index: 0x050 (80)
    EVENT_GOT_TM27 = 0x050,
    /// Bit index: 0x051 (81)
    EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI = 0x051,
    /// Bit index: 0x052 (82)
    EVENT_BEAT_VIRIDIAN_GYM_TRAINER_0 = 0x052,
    /// Bit index: 0x053 (83)
    EVENT_BEAT_VIRIDIAN_GYM_TRAINER_1 = 0x053,
    /// Bit index: 0x054 (84)
    EVENT_BEAT_VIRIDIAN_GYM_TRAINER_2 = 0x054,
    /// Bit index: 0x055 (85)
    EVENT_BEAT_VIRIDIAN_GYM_TRAINER_3 = 0x055,
    /// Bit index: 0x056 (86)
    EVENT_BEAT_VIRIDIAN_GYM_TRAINER_4 = 0x056,
    /// Bit index: 0x057 (87)
    EVENT_BEAT_VIRIDIAN_GYM_TRAINER_5 = 0x057,
    /// Bit index: 0x058 (88)
    EVENT_BEAT_VIRIDIAN_GYM_TRAINER_6 = 0x058,
    /// Bit index: 0x059 (89)
    EVENT_BEAT_VIRIDIAN_GYM_TRAINER_7 = 0x059,
    /// Bit index: 0x068 (104)
    EVENT_BOUGHT_MUSEUM_TICKET = 0x068,
    /// Bit index: 0x069 (105)
    EVENT_GOT_OLD_AMBER = 0x069,
    /// Bit index: 0x072 (114)
    EVENT_BEAT_PEWTER_GYM_TRAINER_0 = 0x072,
    /// Bit index: 0x076 (118)
    EVENT_GOT_TM34 = 0x076,
    /// Bit index: 0x077 (119)
    EVENT_BEAT_BROCK = 0x077,
    /// Bit index: 0x098 (152)
    EVENT_BEAT_CERULEAN_RIVAL = 0x098,
    /// Bit index: 0x0A7 (167)
    EVENT_BEAT_CERULEAN_ROCKET_THIEF = 0x0A7,
    /// Bit index: 0x0BA (186)
    EVENT_BEAT_CERULEAN_GYM_TRAINER_0 = 0x0BA,
    /// Bit index: 0x0BB (187)
    EVENT_BEAT_CERULEAN_GYM_TRAINER_1 = 0x0BB,
    /// Bit index: 0x0BE (190)
    EVENT_GOT_TM11 = 0x0BE,
    /// Bit index: 0x0BF (191)
    EVENT_BEAT_MISTY = 0x0BF,
    /// Bit index: 0x0C0 (192)
    EVENT_GOT_BICYCLE = 0x0C0,
    /// Bit index: 0x0EE (238)
    EVENT_POKEMON_TOWER_RIVAL_ON_LEFT = 0x0EE,
    /// Bit index: 0x0EF (239)
    EVENT_BEAT_POKEMON_TOWER_RIVAL = 0x0EF,
    /// Bit index: 0x0F1 (241)
    EVENT_BEAT_POKEMONTOWER_3_TRAINER_0 = 0x0F1,
    /// Bit index: 0x0F2 (242)
    EVENT_BEAT_POKEMONTOWER_3_TRAINER_1 = 0x0F2,
    /// Bit index: 0x0F3 (243)
    EVENT_BEAT_POKEMONTOWER_3_TRAINER_2 = 0x0F3,
    /// Bit index: 0x0F9 (249)
    EVENT_BEAT_POKEMONTOWER_4_TRAINER_0 = 0x0F9,
    /// Bit index: 0x0FA (250)
    EVENT_BEAT_POKEMONTOWER_4_TRAINER_1 = 0x0FA,
    /// Bit index: 0x0FB (251)
    EVENT_BEAT_POKEMONTOWER_4_TRAINER_2 = 0x0FB,
    /// Bit index: 0x102 (258)
    EVENT_BEAT_POKEMONTOWER_5_TRAINER_0 = 0x102,
    /// Bit index: 0x103 (259)
    EVENT_BEAT_POKEMONTOWER_5_TRAINER_1 = 0x103,
    /// Bit index: 0x104 (260)
    EVENT_BEAT_POKEMONTOWER_5_TRAINER_2 = 0x104,
    /// Bit index: 0x105 (261)
    EVENT_BEAT_POKEMONTOWER_5_TRAINER_3 = 0x105,
    /// Bit index: 0x107 (263)
    EVENT_IN_PURIFIED_ZONE = 0x107,
    /// Bit index: 0x109 (265)
    EVENT_BEAT_POKEMONTOWER_6_TRAINER_0 = 0x109,
    /// Bit index: 0x10A (266)
    EVENT_BEAT_POKEMONTOWER_6_TRAINER_1 = 0x10A,
    /// Bit index: 0x10B (267)
    EVENT_BEAT_POKEMONTOWER_6_TRAINER_2 = 0x10B,
    /// Bit index: 0x10F (271)
    EVENT_BEAT_GHOST_MAROWAK = 0x10F,
    /// Bit index: 0x111 (273)
    EVENT_BEAT_POKEMONTOWER_7_TRAINER_0 = 0x111,
    /// Bit index: 0x112 (274)
    EVENT_BEAT_POKEMONTOWER_7_TRAINER_1 = 0x112,
    /// Bit index: 0x113 (275)
    EVENT_BEAT_POKEMONTOWER_7_TRAINER_2 = 0x113,
    /// Bit index: 0x117 (279)
    EVENT_RESCUED_MR_FUJI_2 = 0x117,
    /// Bit index: 0x128 (296)
    EVENT_GOT_POKE_FLUTE = 0x128,
    /// Bit index: 0x151 (337)
    EVENT_GOT_BIKE_VOUCHER = 0x151,
    /// Bit index: 0x156 (342)
    EVENT_SEEL_FAN_BOAST = 0x156,
    /// Bit index: 0x157 (343)
    EVENT_PIKACHU_FAN_BOAST = 0x157,
    /// Bit index: 0x160 (352)
    EVENT_2ND_LOCK_OPENED = 0x160,
    /// Bit index: 0x161 (353)
    EVENT_1ST_LOCK_OPENED = 0x161,
    /// Bit index: 0x162 (354)
    EVENT_BEAT_VERMILION_GYM_TRAINER_0 = 0x162,
    /// Bit index: 0x163 (355)
    EVENT_BEAT_VERMILION_GYM_TRAINER_1 = 0x163,
    /// Bit index: 0x164 (356)
    EVENT_BEAT_VERMILION_GYM_TRAINER_2 = 0x164,
    /// Bit index: 0x166 (358)
    EVENT_GOT_TM24 = 0x166,
    /// Bit index: 0x167 (359)
    EVENT_BEAT_LT_SURGE = 0x167,
    /// Bit index: 0x180 (384)
    EVENT_GOT_TM41 = 0x180,
    /// Bit index: 0x18C (396)
    EVENT_GOT_TM13 = 0x18C,
    /// Bit index: 0x18D (397)
    EVENT_GOT_TM48 = 0x18D,
    /// Bit index: 0x18E (398)
    EVENT_GOT_TM49 = 0x18E,
    /// Bit index: 0x18F (399)
    EVENT_GOT_TM18 = 0x18F,
    /// Bit index: 0x1A8 (424)
    EVENT_GOT_TM21 = 0x1A8,
    /// Bit index: 0x1A9 (425)
    EVENT_BEAT_ERIKA = 0x1A9,
    /// Bit index: 0x1AA (426)
    EVENT_BEAT_CELADON_GYM_TRAINER_0 = 0x1AA,
    /// Bit index: 0x1AB (427)
    EVENT_BEAT_CELADON_GYM_TRAINER_1 = 0x1AB,
    /// Bit index: 0x1AC (428)
    EVENT_BEAT_CELADON_GYM_TRAINER_2 = 0x1AC,
    /// Bit index: 0x1AD (429)
    EVENT_BEAT_CELADON_GYM_TRAINER_3 = 0x1AD,
    /// Bit index: 0x1AE (430)
    EVENT_BEAT_CELADON_GYM_TRAINER_4 = 0x1AE,
    /// Bit index: 0x1AF (431)
    EVENT_BEAT_CELADON_GYM_TRAINER_5 = 0x1AF,
    /// Bit index: 0x1B0 (432)
    EVENT_BEAT_CELADON_GYM_TRAINER_6 = 0x1B0,
    /// Bit index: 0x1B8 (440)
    EVENT_1B8 = 0x1B8,
    /// Bit index: 0x1B9 (441)
    EVENT_FOUND_ROCKET_HIDEOUT = 0x1B9,
    /// Bit index: 0x1BA (442)
    EVENT_GOT_10_COINS = 0x1BA,
    /// Bit index: 0x1BB (443)
    EVENT_GOT_20_COINS = 0x1BB,
    /// Bit index: 0x1BC (444)
    EVENT_GOT_20_COINS_2 = 0x1BC,
    /// Bit index: 0x1BF (447)
    EVENT_1BF = 0x1BF,
    /// Bit index: 0x1E0 (480)
    EVENT_GOT_COIN_CASE = 0x1E0,
    /// Bit index: 0x238 (568)
    EVENT_GOT_HM04 = 0x238,
    /// Bit index: 0x239 (569)
    EVENT_GAVE_GOLD_TEETH = 0x239,
    /// Bit index: 0x24E (590)
    EVENT_SAFARI_GAME_OVER = 0x24E,
    /// Bit index: 0x24F (591)
    EVENT_IN_SAFARI_ZONE = 0x24F,
    /// Bit index: 0x258 (600)
    EVENT_GOT_TM06 = 0x258,
    /// Bit index: 0x259 (601)
    EVENT_BEAT_KOGA = 0x259,
    /// Bit index: 0x25A (602)
    EVENT_BEAT_FUCHSIA_GYM_TRAINER_0 = 0x25A,
    /// Bit index: 0x25B (603)
    EVENT_BEAT_FUCHSIA_GYM_TRAINER_1 = 0x25B,
    /// Bit index: 0x25C (604)
    EVENT_BEAT_FUCHSIA_GYM_TRAINER_2 = 0x25C,
    /// Bit index: 0x25D (605)
    EVENT_BEAT_FUCHSIA_GYM_TRAINER_3 = 0x25D,
    /// Bit index: 0x25E (606)
    EVENT_BEAT_FUCHSIA_GYM_TRAINER_4 = 0x25E,
    /// Bit index: 0x25F (607)
    EVENT_BEAT_FUCHSIA_GYM_TRAINER_5 = 0x25F,
    /// Bit index: 0x278 (632)
    EVENT_MANSION_SWITCH_ON = 0x278,
    /// Bit index: 0x289 (649)
    EVENT_BEAT_MANSION_1_TRAINER_0 = 0x289,
    /// Bit index: 0x298 (664)
    EVENT_GOT_TM38 = 0x298,
    /// Bit index: 0x299 (665)
    EVENT_BEAT_BLAINE = 0x299,
    /// Bit index: 0x29A (666)
    EVENT_BEAT_CINNABAR_GYM_TRAINER_0 = 0x29A,
    /// Bit index: 0x29B (667)
    EVENT_BEAT_CINNABAR_GYM_TRAINER_1 = 0x29B,
    /// Bit index: 0x29C (668)
    EVENT_BEAT_CINNABAR_GYM_TRAINER_2 = 0x29C,
    /// Bit index: 0x29D (669)
    EVENT_BEAT_CINNABAR_GYM_TRAINER_3 = 0x29D,
    /// Bit index: 0x29E (670)
    EVENT_BEAT_CINNABAR_GYM_TRAINER_4 = 0x29E,
    /// Bit index: 0x29F (671)
    EVENT_BEAT_CINNABAR_GYM_TRAINER_5 = 0x29F,
    /// Bit index: 0x2A0 (672)
    EVENT_BEAT_CINNABAR_GYM_TRAINER_6 = 0x2A0,
    /// Bit index: 0x2A7 (679)
    EVENT_2A7 = 0x2A7,
    /// Bit index: 0x2A8 (680)
    EVENT_CINNABAR_GYM_GATE0_UNLOCKED = 0x2A8,
    /// Bit index: 0x2A9 (681)
    EVENT_CINNABAR_GYM_GATE1_UNLOCKED = 0x2A9,
    /// Bit index: 0x2AA (682)
    EVENT_CINNABAR_GYM_GATE2_UNLOCKED = 0x2AA,
    /// Bit index: 0x2AB (683)
    EVENT_CINNABAR_GYM_GATE3_UNLOCKED = 0x2AB,
    /// Bit index: 0x2AC (684)
    EVENT_CINNABAR_GYM_GATE4_UNLOCKED = 0x2AC,
    /// Bit index: 0x2AD (685)
    EVENT_CINNABAR_GYM_GATE5_UNLOCKED = 0x2AD,
    /// Bit index: 0x2AE (686)
    EVENT_CINNABAR_GYM_GATE6_UNLOCKED = 0x2AE,
    /// Bit index: 0x2D7 (727)
    EVENT_GOT_TM35 = 0x2D7,
    /// Bit index: 0x2E0 (736)
    EVENT_GAVE_FOSSIL_TO_LAB = 0x2E0,
    /// Bit index: 0x2E1 (737)
    EVENT_LAB_STILL_REVIVING_FOSSIL = 0x2E1,
    /// Bit index: 0x2E2 (738)
    EVENT_LAB_HANDING_OVER_FOSSIL_MON = 0x2E2,
    /// Bit index: 0x340 (832)
    EVENT_GOT_TM31 = 0x340,
    /// Bit index: 0x350 (848)
    EVENT_DEFEATED_FIGHTING_DOJO = 0x350,
    /// Bit index: 0x351 (849)
    EVENT_BEAT_KARATE_MASTER = 0x351,
    /// Bit index: 0x352 (850)
    EVENT_BEAT_FIGHTING_DOJO_TRAINER_0 = 0x352,
    /// Bit index: 0x353 (851)
    EVENT_BEAT_FIGHTING_DOJO_TRAINER_1 = 0x353,
    /// Bit index: 0x354 (852)
    EVENT_BEAT_FIGHTING_DOJO_TRAINER_2 = 0x354,
    /// Bit index: 0x355 (853)
    EVENT_BEAT_FIGHTING_DOJO_TRAINER_3 = 0x355,
    /// Bit index: 0x356 (854)
    EVENT_GOT_HITMONLEE = 0x356,
    /// Bit index: 0x357 (855)
    EVENT_GOT_HITMONCHAN = 0x357,
    /// Bit index: 0x360 (864)
    EVENT_GOT_TM46 = 0x360,
    /// Bit index: 0x361 (865)
    EVENT_BEAT_SABRINA = 0x361,
    /// Bit index: 0x362 (866)
    EVENT_BEAT_SAFFRON_GYM_TRAINER_0 = 0x362,
    /// Bit index: 0x363 (867)
    EVENT_BEAT_SAFFRON_GYM_TRAINER_1 = 0x363,
    /// Bit index: 0x364 (868)
    EVENT_BEAT_SAFFRON_GYM_TRAINER_2 = 0x364,
    /// Bit index: 0x365 (869)
    EVENT_BEAT_SAFFRON_GYM_TRAINER_3 = 0x365,
    /// Bit index: 0x366 (870)
    EVENT_BEAT_SAFFRON_GYM_TRAINER_4 = 0x366,
    /// Bit index: 0x367 (871)
    EVENT_BEAT_SAFFRON_GYM_TRAINER_5 = 0x367,
    /// Bit index: 0x368 (872)
    EVENT_BEAT_SAFFRON_GYM_TRAINER_6 = 0x368,
    /// Bit index: 0x397 (919)
    EVENT_SILPH_CO_RECEPTIONIST_AT_DESK = 0x397,
    /// Bit index: 0x3B0 (944)
    EVENT_GOT_TM29 = 0x3B0,
    /// Bit index: 0x3C0 (960)
    EVENT_GOT_POTION_SAMPLE = 0x3C0,
    /// Bit index: 0x3D8 (984)
    EVENT_GOT_HM05 = 0x3D8,
    /// Bit index: 0x3E2 (994)
    EVENT_BEAT_ROUTE_3_TRAINER_0 = 0x3E2,
    /// Bit index: 0x3E3 (995)
    EVENT_BEAT_ROUTE_3_TRAINER_1 = 0x3E3,
    /// Bit index: 0x3E4 (996)
    EVENT_BEAT_ROUTE_3_TRAINER_2 = 0x3E4,
    /// Bit index: 0x3E5 (997)
    EVENT_BEAT_ROUTE_3_TRAINER_3 = 0x3E5,
    /// Bit index: 0x3E6 (998)
    EVENT_BEAT_ROUTE_3_TRAINER_4 = 0x3E6,
    /// Bit index: 0x3E7 (999)
    EVENT_BEAT_ROUTE_3_TRAINER_5 = 0x3E7,
    /// Bit index: 0x3E8 (1000)
    EVENT_BEAT_ROUTE_3_TRAINER_6 = 0x3E8,
    /// Bit index: 0x3E9 (1001)
    EVENT_BEAT_ROUTE_3_TRAINER_7 = 0x3E9,
    /// Bit index: 0x3F2 (1010)
    EVENT_BEAT_ROUTE_4_TRAINER_0 = 0x3F2,
    /// Bit index: 0x3FF (1023)
    EVENT_BOUGHT_MAGIKARP = 0x3FF,
    /// Bit index: 0x411 (1041)
    EVENT_BEAT_ROUTE_6_TRAINER_0 = 0x411,
    /// Bit index: 0x412 (1042)
    EVENT_BEAT_ROUTE_6_TRAINER_1 = 0x412,
    /// Bit index: 0x413 (1043)
    EVENT_BEAT_ROUTE_6_TRAINER_2 = 0x413,
    /// Bit index: 0x414 (1044)
    EVENT_BEAT_ROUTE_6_TRAINER_3 = 0x414,
    /// Bit index: 0x415 (1045)
    EVENT_BEAT_ROUTE_6_TRAINER_4 = 0x415,
    /// Bit index: 0x416 (1046)
    EVENT_BEAT_ROUTE_6_TRAINER_5 = 0x416,
    /// Bit index: 0x431 (1073)
    EVENT_BEAT_ROUTE_8_TRAINER_0 = 0x431,
    /// Bit index: 0x432 (1074)
    EVENT_BEAT_ROUTE_8_TRAINER_1 = 0x432,
    /// Bit index: 0x433 (1075)
    EVENT_BEAT_ROUTE_8_TRAINER_2 = 0x433,
    /// Bit index: 0x434 (1076)
    EVENT_BEAT_ROUTE_8_TRAINER_3 = 0x434,
    /// Bit index: 0x435 (1077)
    EVENT_BEAT_ROUTE_8_TRAINER_4 = 0x435,
    /// Bit index: 0x436 (1078)
    EVENT_BEAT_ROUTE_8_TRAINER_5 = 0x436,
    /// Bit index: 0x437 (1079)
    EVENT_BEAT_ROUTE_8_TRAINER_6 = 0x437,
    /// Bit index: 0x438 (1080)
    EVENT_BEAT_ROUTE_8_TRAINER_7 = 0x438,
    /// Bit index: 0x439 (1081)
    EVENT_BEAT_ROUTE_8_TRAINER_8 = 0x439,
    /// Bit index: 0x441 (1089)
    EVENT_BEAT_ROUTE_9_TRAINER_0 = 0x441,
    /// Bit index: 0x442 (1090)
    EVENT_BEAT_ROUTE_9_TRAINER_1 = 0x442,
    /// Bit index: 0x443 (1091)
    EVENT_BEAT_ROUTE_9_TRAINER_2 = 0x443,
    /// Bit index: 0x444 (1092)
    EVENT_BEAT_ROUTE_9_TRAINER_3 = 0x444,
    /// Bit index: 0x445 (1093)
    EVENT_BEAT_ROUTE_9_TRAINER_4 = 0x445,
    /// Bit index: 0x446 (1094)
    EVENT_BEAT_ROUTE_9_TRAINER_5 = 0x446,
    /// Bit index: 0x447 (1095)
    EVENT_BEAT_ROUTE_9_TRAINER_6 = 0x447,
    /// Bit index: 0x448 (1096)
    EVENT_BEAT_ROUTE_9_TRAINER_7 = 0x448,
    /// Bit index: 0x449 (1097)
    EVENT_BEAT_ROUTE_9_TRAINER_8 = 0x449,
    /// Bit index: 0x451 (1105)
    EVENT_BEAT_ROUTE_10_TRAINER_0 = 0x451,
    /// Bit index: 0x452 (1106)
    EVENT_BEAT_ROUTE_10_TRAINER_1 = 0x452,
    /// Bit index: 0x453 (1107)
    EVENT_BEAT_ROUTE_10_TRAINER_2 = 0x453,
    /// Bit index: 0x454 (1108)
    EVENT_BEAT_ROUTE_10_TRAINER_3 = 0x454,
    /// Bit index: 0x455 (1109)
    EVENT_BEAT_ROUTE_10_TRAINER_4 = 0x455,
    /// Bit index: 0x456 (1110)
    EVENT_BEAT_ROUTE_10_TRAINER_5 = 0x456,
    /// Bit index: 0x459 (1113)
    EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_0 = 0x459,
    /// Bit index: 0x45A (1114)
    EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_1 = 0x45A,
    /// Bit index: 0x45B (1115)
    EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_2 = 0x45B,
    /// Bit index: 0x45C (1116)
    EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_3 = 0x45C,
    /// Bit index: 0x45D (1117)
    EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_4 = 0x45D,
    /// Bit index: 0x45E (1118)
    EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_5 = 0x45E,
    /// Bit index: 0x45F (1119)
    EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_6 = 0x45F,
    /// Bit index: 0x461 (1121)
    EVENT_BEAT_POWER_PLANT_VOLTORB_0 = 0x461,
    /// Bit index: 0x462 (1122)
    EVENT_BEAT_POWER_PLANT_VOLTORB_1 = 0x462,
    /// Bit index: 0x463 (1123)
    EVENT_BEAT_POWER_PLANT_VOLTORB_2 = 0x463,
    /// Bit index: 0x464 (1124)
    EVENT_BEAT_POWER_PLANT_VOLTORB_3 = 0x464,
    /// Bit index: 0x465 (1125)
    EVENT_BEAT_POWER_PLANT_VOLTORB_4 = 0x465,
    /// Bit index: 0x466 (1126)
    EVENT_BEAT_POWER_PLANT_VOLTORB_5 = 0x466,
    /// Bit index: 0x467 (1127)
    EVENT_BEAT_POWER_PLANT_VOLTORB_6 = 0x467,
    /// Bit index: 0x468 (1128)
    EVENT_BEAT_POWER_PLANT_VOLTORB_7 = 0x468,
    /// Bit index: 0x469 (1129)
    EVENT_BEAT_ZAPDOS = 0x469,
    /// Bit index: 0x471 (1137)
    EVENT_BEAT_ROUTE_11_TRAINER_0 = 0x471,
    /// Bit index: 0x472 (1138)
    EVENT_BEAT_ROUTE_11_TRAINER_1 = 0x472,
    /// Bit index: 0x473 (1139)
    EVENT_BEAT_ROUTE_11_TRAINER_2 = 0x473,
    /// Bit index: 0x474 (1140)
    EVENT_BEAT_ROUTE_11_TRAINER_3 = 0x474,
    /// Bit index: 0x475 (1141)
    EVENT_BEAT_ROUTE_11_TRAINER_4 = 0x475,
    /// Bit index: 0x476 (1142)
    EVENT_BEAT_ROUTE_11_TRAINER_5 = 0x476,
    /// Bit index: 0x477 (1143)
    EVENT_BEAT_ROUTE_11_TRAINER_6 = 0x477,
    /// Bit index: 0x478 (1144)
    EVENT_BEAT_ROUTE_11_TRAINER_7 = 0x478,
    /// Bit index: 0x479 (1145)
    EVENT_BEAT_ROUTE_11_TRAINER_8 = 0x479,
    /// Bit index: 0x47A (1146)
    EVENT_BEAT_ROUTE_11_TRAINER_9 = 0x47A,
    /// Bit index: 0x47F (1151)
    EVENT_GOT_ITEMFINDER = 0x47F,
    /// Bit index: 0x480 (1152)
    EVENT_GOT_TM39 = 0x480,
    /// Bit index: 0x482 (1154)
    EVENT_BEAT_ROUTE_12_TRAINER_0 = 0x482,
    /// Bit index: 0x483 (1155)
    EVENT_BEAT_ROUTE_12_TRAINER_1 = 0x483,
    /// Bit index: 0x484 (1156)
    EVENT_BEAT_ROUTE_12_TRAINER_2 = 0x484,
    /// Bit index: 0x485 (1157)
    EVENT_BEAT_ROUTE_12_TRAINER_3 = 0x485,
    /// Bit index: 0x486 (1158)
    EVENT_BEAT_ROUTE_12_TRAINER_4 = 0x486,
    /// Bit index: 0x487 (1159)
    EVENT_BEAT_ROUTE_12_TRAINER_5 = 0x487,
    /// Bit index: 0x488 (1160)
    EVENT_BEAT_ROUTE_12_TRAINER_6 = 0x488,
    /// Bit index: 0x48E (1166)
    EVENT_FIGHT_ROUTE12_SNORLAX = 0x48E,
    /// Bit index: 0x48F (1167)
    EVENT_BEAT_ROUTE12_SNORLAX = 0x48F,
    /// Bit index: 0x491 (1169)
    EVENT_BEAT_ROUTE_13_TRAINER_0 = 0x491,
    /// Bit index: 0x492 (1170)
    EVENT_BEAT_ROUTE_13_TRAINER_1 = 0x492,
    /// Bit index: 0x493 (1171)
    EVENT_BEAT_ROUTE_13_TRAINER_2 = 0x493,
    /// Bit index: 0x494 (1172)
    EVENT_BEAT_ROUTE_13_TRAINER_3 = 0x494,
    /// Bit index: 0x495 (1173)
    EVENT_BEAT_ROUTE_13_TRAINER_4 = 0x495,
    /// Bit index: 0x496 (1174)
    EVENT_BEAT_ROUTE_13_TRAINER_5 = 0x496,
    /// Bit index: 0x497 (1175)
    EVENT_BEAT_ROUTE_13_TRAINER_6 = 0x497,
    /// Bit index: 0x498 (1176)
    EVENT_BEAT_ROUTE_13_TRAINER_7 = 0x498,
    /// Bit index: 0x499 (1177)
    EVENT_BEAT_ROUTE_13_TRAINER_8 = 0x499,
    /// Bit index: 0x49A (1178)
    EVENT_BEAT_ROUTE_13_TRAINER_9 = 0x49A,
    /// Bit index: 0x4A1 (1185)
    EVENT_BEAT_ROUTE_14_TRAINER_0 = 0x4A1,
    /// Bit index: 0x4A2 (1186)
    EVENT_BEAT_ROUTE_14_TRAINER_1 = 0x4A2,
    /// Bit index: 0x4A3 (1187)
    EVENT_BEAT_ROUTE_14_TRAINER_2 = 0x4A3,
    /// Bit index: 0x4A4 (1188)
    EVENT_BEAT_ROUTE_14_TRAINER_3 = 0x4A4,
    /// Bit index: 0x4A5 (1189)
    EVENT_BEAT_ROUTE_14_TRAINER_4 = 0x4A5,
    /// Bit index: 0x4A6 (1190)
    EVENT_BEAT_ROUTE_14_TRAINER_5 = 0x4A6,
    /// Bit index: 0x4A7 (1191)
    EVENT_BEAT_ROUTE_14_TRAINER_6 = 0x4A7,
    /// Bit index: 0x4A8 (1192)
    EVENT_BEAT_ROUTE_14_TRAINER_7 = 0x4A8,
    /// Bit index: 0x4A9 (1193)
    EVENT_BEAT_ROUTE_14_TRAINER_8 = 0x4A9,
    /// Bit index: 0x4AA (1194)
    EVENT_BEAT_ROUTE_14_TRAINER_9 = 0x4AA,
    /// Bit index: 0x4B0 (1200)
    EVENT_GOT_EXP_ALL = 0x4B0,
    /// Bit index: 0x4B1 (1201)
    EVENT_BEAT_ROUTE_15_TRAINER_0 = 0x4B1,
    /// Bit index: 0x4B2 (1202)
    EVENT_BEAT_ROUTE_15_TRAINER_1 = 0x4B2,
    /// Bit index: 0x4B3 (1203)
    EVENT_BEAT_ROUTE_15_TRAINER_2 = 0x4B3,
    /// Bit index: 0x4B4 (1204)
    EVENT_BEAT_ROUTE_15_TRAINER_3 = 0x4B4,
    /// Bit index: 0x4B5 (1205)
    EVENT_BEAT_ROUTE_15_TRAINER_4 = 0x4B5,
    /// Bit index: 0x4B6 (1206)
    EVENT_BEAT_ROUTE_15_TRAINER_5 = 0x4B6,
    /// Bit index: 0x4B7 (1207)
    EVENT_BEAT_ROUTE_15_TRAINER_6 = 0x4B7,
    /// Bit index: 0x4B8 (1208)
    EVENT_BEAT_ROUTE_15_TRAINER_7 = 0x4B8,
    /// Bit index: 0x4B9 (1209)
    EVENT_BEAT_ROUTE_15_TRAINER_8 = 0x4B9,
    /// Bit index: 0x4BA (1210)
    EVENT_BEAT_ROUTE_15_TRAINER_9 = 0x4BA,
    /// Bit index: 0x4C1 (1217)
    EVENT_BEAT_ROUTE_16_TRAINER_0 = 0x4C1,
    /// Bit index: 0x4C2 (1218)
    EVENT_BEAT_ROUTE_16_TRAINER_1 = 0x4C2,
    /// Bit index: 0x4C3 (1219)
    EVENT_BEAT_ROUTE_16_TRAINER_2 = 0x4C3,
    /// Bit index: 0x4C4 (1220)
    EVENT_BEAT_ROUTE_16_TRAINER_3 = 0x4C4,
    /// Bit index: 0x4C5 (1221)
    EVENT_BEAT_ROUTE_16_TRAINER_4 = 0x4C5,
    /// Bit index: 0x4C6 (1222)
    EVENT_BEAT_ROUTE_16_TRAINER_5 = 0x4C6,
    /// Bit index: 0x4C8 (1224)
    EVENT_FIGHT_ROUTE16_SNORLAX = 0x4C8,
    /// Bit index: 0x4C9 (1225)
    EVENT_BEAT_ROUTE16_SNORLAX = 0x4C9,
    /// Bit index: 0x4CE (1230)
    EVENT_GOT_HM02 = 0x4CE,
    /// Bit index: 0x4CF (1231)
    EVENT_RESCUED_MR_FUJI = 0x4CF,
    /// Bit index: 0x4D1 (1233)
    EVENT_BEAT_ROUTE_17_TRAINER_0 = 0x4D1,
    /// Bit index: 0x4D2 (1234)
    EVENT_BEAT_ROUTE_17_TRAINER_1 = 0x4D2,
    /// Bit index: 0x4D3 (1235)
    EVENT_BEAT_ROUTE_17_TRAINER_2 = 0x4D3,
    /// Bit index: 0x4D4 (1236)
    EVENT_BEAT_ROUTE_17_TRAINER_3 = 0x4D4,
    /// Bit index: 0x4D5 (1237)
    EVENT_BEAT_ROUTE_17_TRAINER_4 = 0x4D5,
    /// Bit index: 0x4D6 (1238)
    EVENT_BEAT_ROUTE_17_TRAINER_5 = 0x4D6,
    /// Bit index: 0x4D7 (1239)
    EVENT_BEAT_ROUTE_17_TRAINER_6 = 0x4D7,
    /// Bit index: 0x4D8 (1240)
    EVENT_BEAT_ROUTE_17_TRAINER_7 = 0x4D8,
    /// Bit index: 0x4D9 (1241)
    EVENT_BEAT_ROUTE_17_TRAINER_8 = 0x4D9,
    /// Bit index: 0x4DA (1242)
    EVENT_BEAT_ROUTE_17_TRAINER_9 = 0x4DA,
    /// Bit index: 0x4E1 (1249)
    EVENT_BEAT_ROUTE_18_TRAINER_0 = 0x4E1,
    /// Bit index: 0x4E2 (1250)
    EVENT_BEAT_ROUTE_18_TRAINER_1 = 0x4E2,
    /// Bit index: 0x4E3 (1251)
    EVENT_BEAT_ROUTE_18_TRAINER_2 = 0x4E3,
    /// Bit index: 0x4F1 (1265)
    EVENT_BEAT_ROUTE_19_TRAINER_0 = 0x4F1,
    /// Bit index: 0x4F2 (1266)
    EVENT_BEAT_ROUTE_19_TRAINER_1 = 0x4F2,
    /// Bit index: 0x4F3 (1267)
    EVENT_BEAT_ROUTE_19_TRAINER_2 = 0x4F3,
    /// Bit index: 0x4F4 (1268)
    EVENT_BEAT_ROUTE_19_TRAINER_3 = 0x4F4,
    /// Bit index: 0x4F5 (1269)
    EVENT_BEAT_ROUTE_19_TRAINER_4 = 0x4F5,
    /// Bit index: 0x4F6 (1270)
    EVENT_BEAT_ROUTE_19_TRAINER_5 = 0x4F6,
    /// Bit index: 0x4F7 (1271)
    EVENT_BEAT_ROUTE_19_TRAINER_6 = 0x4F7,
    /// Bit index: 0x4F8 (1272)
    EVENT_BEAT_ROUTE_19_TRAINER_7 = 0x4F8,
    /// Bit index: 0x4F9 (1273)
    EVENT_BEAT_ROUTE_19_TRAINER_8 = 0x4F9,
    /// Bit index: 0x4FA (1274)
    EVENT_BEAT_ROUTE_19_TRAINER_9 = 0x4FA,
    /// Bit index: 0x500 (1280)
    EVENT_IN_SEAFOAM_ISLANDS = 0x500,
    /// Bit index: 0x501 (1281)
    EVENT_BEAT_ROUTE_20_TRAINER_0 = 0x501,
    /// Bit index: 0x502 (1282)
    EVENT_BEAT_ROUTE_20_TRAINER_1 = 0x502,
    /// Bit index: 0x503 (1283)
    EVENT_BEAT_ROUTE_20_TRAINER_2 = 0x503,
    /// Bit index: 0x504 (1284)
    EVENT_BEAT_ROUTE_20_TRAINER_3 = 0x504,
    /// Bit index: 0x505 (1285)
    EVENT_BEAT_ROUTE_20_TRAINER_4 = 0x505,
    /// Bit index: 0x506 (1286)
    EVENT_BEAT_ROUTE_20_TRAINER_5 = 0x506,
    /// Bit index: 0x507 (1287)
    EVENT_BEAT_ROUTE_20_TRAINER_6 = 0x507,
    /// Bit index: 0x508 (1288)
    EVENT_BEAT_ROUTE_20_TRAINER_7 = 0x508,
    /// Bit index: 0x509 (1289)
    EVENT_BEAT_ROUTE_20_TRAINER_8 = 0x509,
    /// Bit index: 0x50A (1290)
    EVENT_BEAT_ROUTE_20_TRAINER_9 = 0x50A,
    /// Bit index: 0x50E (1294)
    EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE = 0x50E,
    /// Bit index: 0x50F (1295)
    EVENT_SEAFOAM1_BOULDER2_DOWN_HOLE = 0x50F,
    /// Bit index: 0x511 (1297)
    EVENT_BEAT_ROUTE_21_TRAINER_0 = 0x511,
    /// Bit index: 0x512 (1298)
    EVENT_BEAT_ROUTE_21_TRAINER_1 = 0x512,
    /// Bit index: 0x513 (1299)
    EVENT_BEAT_ROUTE_21_TRAINER_2 = 0x513,
    /// Bit index: 0x514 (1300)
    EVENT_BEAT_ROUTE_21_TRAINER_3 = 0x514,
    /// Bit index: 0x515 (1301)
    EVENT_BEAT_ROUTE_21_TRAINER_4 = 0x515,
    /// Bit index: 0x516 (1302)
    EVENT_BEAT_ROUTE_21_TRAINER_5 = 0x516,
    /// Bit index: 0x517 (1303)
    EVENT_BEAT_ROUTE_21_TRAINER_6 = 0x517,
    /// Bit index: 0x518 (1304)
    EVENT_BEAT_ROUTE_21_TRAINER_7 = 0x518,
    /// Bit index: 0x519 (1305)
    EVENT_BEAT_ROUTE_21_TRAINER_8 = 0x519,
    /// Bit index: 0x520 (1312)
    EVENT_1ST_ROUTE22_RIVAL_BATTLE = 0x520,
    /// Bit index: 0x521 (1313)
    EVENT_2ND_ROUTE22_RIVAL_BATTLE = 0x521,
    /// Bit index: 0x525 (1317)
    EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE = 0x525,
    /// Bit index: 0x526 (1318)
    EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE = 0x526,
    /// Bit index: 0x527 (1319)
    EVENT_ROUTE22_RIVAL_WANTS_BATTLE = 0x527,
    /// Bit index: 0x530 (1328)
    EVENT_PASSED_CASCADEBADGE_CHECK = 0x530,
    /// Bit index: 0x531 (1329)
    EVENT_PASSED_THUNDERBADGE_CHECK = 0x531,
    /// Bit index: 0x532 (1330)
    EVENT_PASSED_RAINBOWBADGE_CHECK = 0x532,
    /// Bit index: 0x533 (1331)
    EVENT_PASSED_SOULBADGE_CHECK = 0x533,
    /// Bit index: 0x534 (1332)
    EVENT_PASSED_MARSHBADGE_CHECK = 0x534,
    /// Bit index: 0x535 (1333)
    EVENT_PASSED_VOLCANOBADGE_CHECK = 0x535,
    /// Bit index: 0x536 (1334)
    EVENT_PASSED_EARTHBADGE_CHECK = 0x536,
    /// Bit index: 0x538 (1336)
    EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1 = 0x538,
    /// Bit index: 0x539 (1337)
    EVENT_BEAT_VICTORY_ROAD_2_TRAINER_0 = 0x539,
    /// Bit index: 0x53A (1338)
    EVENT_BEAT_VICTORY_ROAD_2_TRAINER_1 = 0x53A,
    /// Bit index: 0x53B (1339)
    EVENT_BEAT_VICTORY_ROAD_2_TRAINER_2 = 0x53B,
    /// Bit index: 0x53C (1340)
    EVENT_BEAT_VICTORY_ROAD_2_TRAINER_3 = 0x53C,
    /// Bit index: 0x53D (1341)
    EVENT_BEAT_VICTORY_ROAD_2_TRAINER_4 = 0x53D,
    /// Bit index: 0x53E (1342)
    EVENT_BEAT_MOLTRES = 0x53E,
    /// Bit index: 0x53F (1343)
    EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2 = 0x53F,
    /// Bit index: 0x540 (1344)
    EVENT_GOT_NUGGET = 0x540,
    /// Bit index: 0x541 (1345)
    EVENT_BEAT_ROUTE24_ROCKET = 0x541,
    /// Bit index: 0x542 (1346)
    EVENT_BEAT_ROUTE_24_TRAINER_0 = 0x542,
    /// Bit index: 0x543 (1347)
    EVENT_BEAT_ROUTE_24_TRAINER_1 = 0x543,
    /// Bit index: 0x544 (1348)
    EVENT_BEAT_ROUTE_24_TRAINER_2 = 0x544,
    /// Bit index: 0x545 (1349)
    EVENT_BEAT_ROUTE_24_TRAINER_3 = 0x545,
    /// Bit index: 0x546 (1350)
    EVENT_BEAT_ROUTE_24_TRAINER_4 = 0x546,
    /// Bit index: 0x547 (1351)
    EVENT_BEAT_ROUTE_24_TRAINER_5 = 0x547,
    /// Bit index: 0x549 (1353)
    EVENT_NUGGET_REWARD_AVAILABLE = 0x549,
    /// Bit index: 0x550 (1360)
    EVENT_MET_BILL = 0x550,
    /// Bit index: 0x551 (1361)
    EVENT_BEAT_ROUTE_25_TRAINER_0 = 0x551,
    /// Bit index: 0x552 (1362)
    EVENT_BEAT_ROUTE_25_TRAINER_1 = 0x552,
    /// Bit index: 0x553 (1363)
    EVENT_BEAT_ROUTE_25_TRAINER_2 = 0x553,
    /// Bit index: 0x554 (1364)
    EVENT_BEAT_ROUTE_25_TRAINER_3 = 0x554,
    /// Bit index: 0x555 (1365)
    EVENT_BEAT_ROUTE_25_TRAINER_4 = 0x555,
    /// Bit index: 0x556 (1366)
    EVENT_BEAT_ROUTE_25_TRAINER_5 = 0x556,
    /// Bit index: 0x557 (1367)
    EVENT_BEAT_ROUTE_25_TRAINER_6 = 0x557,
    /// Bit index: 0x558 (1368)
    EVENT_BEAT_ROUTE_25_TRAINER_7 = 0x558,
    /// Bit index: 0x559 (1369)
    EVENT_BEAT_ROUTE_25_TRAINER_8 = 0x559,
    /// Bit index: 0x55B (1371)
    EVENT_USED_CELL_SEPARATOR_ON_BILL = 0x55B,
    /// Bit index: 0x55C (1372)
    EVENT_GOT_SS_TICKET = 0x55C,
    /// Bit index: 0x55D (1373)
    EVENT_MET_BILL_2 = 0x55D,
    /// Bit index: 0x55E (1374)
    EVENT_BILL_SAID_USE_CELL_SEPARATOR = 0x55E,
    /// Bit index: 0x55F (1375)
    EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING = 0x55F,
    /// Bit index: 0x562 (1378)
    EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0 = 0x562,
    /// Bit index: 0x563 (1379)
    EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_1 = 0x563,
    /// Bit index: 0x564 (1380)
    EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2 = 0x564,
    /// Bit index: 0x571 (1393)
    EVENT_BEAT_MT_MOON_1_TRAINER_0 = 0x571,
    /// Bit index: 0x572 (1394)
    EVENT_BEAT_MT_MOON_1_TRAINER_1 = 0x572,
    /// Bit index: 0x573 (1395)
    EVENT_BEAT_MT_MOON_1_TRAINER_2 = 0x573,
    /// Bit index: 0x574 (1396)
    EVENT_BEAT_MT_MOON_1_TRAINER_3 = 0x574,
    /// Bit index: 0x575 (1397)
    EVENT_BEAT_MT_MOON_1_TRAINER_4 = 0x575,
    /// Bit index: 0x576 (1398)
    EVENT_BEAT_MT_MOON_1_TRAINER_5 = 0x576,
    /// Bit index: 0x577 (1399)
    EVENT_BEAT_MT_MOON_1_TRAINER_6 = 0x577,
    /// Bit index: 0x579 (1401)
    EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD = 0x579,
    /// Bit index: 0x57A (1402)
    EVENT_BEAT_MT_MOON_3_TRAINER_0 = 0x57A,
    /// Bit index: 0x57B (1403)
    EVENT_BEAT_MT_MOON_3_TRAINER_1 = 0x57B,
    /// Bit index: 0x57C (1404)
    EVENT_BEAT_MT_MOON_3_TRAINER_2 = 0x57C,
    /// Bit index: 0x57D (1405)
    EVENT_BEAT_MT_MOON_3_TRAINER_3 = 0x57D,
    /// Bit index: 0x57E (1406)
    EVENT_GOT_DOME_FOSSIL = 0x57E,
    /// Bit index: 0x57F (1407)
    EVENT_GOT_HELIX_FOSSIL = 0x57F,
    /// Bit index: 0x5C4 (1476)
    EVENT_BEAT_SS_ANNE_5_TRAINER_0 = 0x5C4,
    /// Bit index: 0x5C5 (1477)
    EVENT_BEAT_SS_ANNE_5_TRAINER_1 = 0x5C5,
    /// Bit index: 0x5E0 (1504)
    EVENT_GOT_HM01 = 0x5E0,
    /// Bit index: 0x5E1 (1505)
    EVENT_RUBBED_CAPTAINS_BACK = 0x5E1,
    /// Bit index: 0x5E2 (1506)
    EVENT_SS_ANNE_LEFT = 0x5E2,
    /// Bit index: 0x5E3 (1507)
    EVENT_WALKED_PAST_GUARD_AFTER_SS_ANNE_LEFT = 0x5E3,
    /// Bit index: 0x5E4 (1508)
    EVENT_STARTED_WALKING_OUT_OF_DOCK = 0x5E4,
    /// Bit index: 0x5E5 (1509)
    EVENT_WALKED_OUT_OF_DOCK = 0x5E5,
    /// Bit index: 0x5F1 (1521)
    EVENT_BEAT_SS_ANNE_8_TRAINER_0 = 0x5F1,
    /// Bit index: 0x5F2 (1522)
    EVENT_BEAT_SS_ANNE_8_TRAINER_1 = 0x5F2,
    /// Bit index: 0x5F3 (1523)
    EVENT_BEAT_SS_ANNE_8_TRAINER_2 = 0x5F3,
    /// Bit index: 0x5F4 (1524)
    EVENT_BEAT_SS_ANNE_8_TRAINER_3 = 0x5F4,
    /// Bit index: 0x601 (1537)
    EVENT_BEAT_SS_ANNE_9_TRAINER_0 = 0x601,
    /// Bit index: 0x602 (1538)
    EVENT_BEAT_SS_ANNE_9_TRAINER_1 = 0x602,
    /// Bit index: 0x603 (1539)
    EVENT_BEAT_SS_ANNE_9_TRAINER_2 = 0x603,
    /// Bit index: 0x604 (1540)
    EVENT_BEAT_SS_ANNE_9_TRAINER_3 = 0x604,
    /// Bit index: 0x611 (1553)
    EVENT_BEAT_SS_ANNE_10_TRAINER_0 = 0x611,
    /// Bit index: 0x612 (1554)
    EVENT_BEAT_SS_ANNE_10_TRAINER_1 = 0x612,
    /// Bit index: 0x613 (1555)
    EVENT_BEAT_SS_ANNE_10_TRAINER_2 = 0x613,
    /// Bit index: 0x614 (1556)
    EVENT_BEAT_SS_ANNE_10_TRAINER_3 = 0x614,
    /// Bit index: 0x615 (1557)
    EVENT_BEAT_SS_ANNE_10_TRAINER_4 = 0x615,
    /// Bit index: 0x616 (1558)
    EVENT_BEAT_SS_ANNE_10_TRAINER_5 = 0x616,
    /// Bit index: 0x660 (1632)
    EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH1 = 0x660,
    /// Bit index: 0x661 (1633)
    EVENT_BEAT_VICTORY_ROAD_3_TRAINER_0 = 0x661,
    /// Bit index: 0x662 (1634)
    EVENT_BEAT_VICTORY_ROAD_3_TRAINER_1 = 0x662,
    /// Bit index: 0x663 (1635)
    EVENT_BEAT_VICTORY_ROAD_3_TRAINER_2 = 0x663,
    /// Bit index: 0x664 (1636)
    EVENT_BEAT_VICTORY_ROAD_3_TRAINER_3 = 0x664,
    /// Bit index: 0x666 (1638)
    EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2 = 0x666,
    /// Bit index: 0x671 (1649)
    EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_0 = 0x671,
    /// Bit index: 0x672 (1650)
    EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_1 = 0x672,
    /// Bit index: 0x673 (1651)
    EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_2 = 0x673,
    /// Bit index: 0x674 (1652)
    EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_3 = 0x674,
    /// Bit index: 0x675 (1653)
    EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_4 = 0x675,
    /// Bit index: 0x677 (1655)
    EVENT_ENTERED_ROCKET_HIDEOUT = 0x677,
    /// Bit index: 0x67F (1663)
    EVENT_67F = 0x67F,
    /// Bit index: 0x681 (1665)
    EVENT_BEAT_ROCKET_HIDEOUT_2_TRAINER_0 = 0x681,
    /// Bit index: 0x691 (1681)
    EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_0 = 0x691,
    /// Bit index: 0x692 (1682)
    EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_1 = 0x692,
    /// Bit index: 0x6A2 (1698)
    EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0 = 0x6A2,
    /// Bit index: 0x6A3 (1699)
    EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_1 = 0x6A3,
    /// Bit index: 0x6A4 (1700)
    EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2 = 0x6A4,
    /// Bit index: 0x6A5 (1701)
    EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED = 0x6A5,
    /// Bit index: 0x6A6 (1702)
    EVENT_ROCKET_DROPPED_LIFT_KEY = 0x6A6,
    /// Bit index: 0x6A7 (1703)
    EVENT_BEAT_ROCKET_HIDEOUT_GIOVANNI = 0x6A7,
    /// Bit index: 0x6F2 (1778)
    EVENT_BEAT_SILPH_CO_2F_TRAINER_0 = 0x6F2,
    /// Bit index: 0x6F3 (1779)
    EVENT_BEAT_SILPH_CO_2F_TRAINER_1 = 0x6F3,
    /// Bit index: 0x6F4 (1780)
    EVENT_BEAT_SILPH_CO_2F_TRAINER_2 = 0x6F4,
    /// Bit index: 0x6F5 (1781)
    EVENT_BEAT_SILPH_CO_2F_TRAINER_3 = 0x6F5,
    /// Bit index: 0x6FD (1789)
    EVENT_SILPH_CO_2_UNLOCKED_DOOR1 = 0x6FD,
    /// Bit index: 0x6FE (1790)
    EVENT_SILPH_CO_2_UNLOCKED_DOOR2 = 0x6FE,
    /// Bit index: 0x6FF (1791)
    EVENT_GOT_TM36 = 0x6FF,
    /// Bit index: 0x702 (1794)
    EVENT_BEAT_SILPH_CO_3F_TRAINER_0 = 0x702,
    /// Bit index: 0x703 (1795)
    EVENT_BEAT_SILPH_CO_3F_TRAINER_1 = 0x703,
    /// Bit index: 0x708 (1800)
    EVENT_SILPH_CO_3_UNLOCKED_DOOR1 = 0x708,
    /// Bit index: 0x709 (1801)
    EVENT_SILPH_CO_3_UNLOCKED_DOOR2 = 0x709,
    /// Bit index: 0x712 (1810)
    EVENT_BEAT_SILPH_CO_4F_TRAINER_0 = 0x712,
    /// Bit index: 0x713 (1811)
    EVENT_BEAT_SILPH_CO_4F_TRAINER_1 = 0x713,
    /// Bit index: 0x714 (1812)
    EVENT_BEAT_SILPH_CO_4F_TRAINER_2 = 0x714,
    /// Bit index: 0x718 (1816)
    EVENT_SILPH_CO_4_UNLOCKED_DOOR1 = 0x718,
    /// Bit index: 0x719 (1817)
    EVENT_SILPH_CO_4_UNLOCKED_DOOR2 = 0x719,
    /// Bit index: 0x722 (1826)
    EVENT_BEAT_SILPH_CO_5F_TRAINER_0 = 0x722,
    /// Bit index: 0x723 (1827)
    EVENT_BEAT_SILPH_CO_5F_TRAINER_1 = 0x723,
    /// Bit index: 0x724 (1828)
    EVENT_BEAT_SILPH_CO_5F_TRAINER_2 = 0x724,
    /// Bit index: 0x725 (1829)
    EVENT_BEAT_SILPH_CO_5F_TRAINER_3 = 0x725,
    /// Bit index: 0x728 (1832)
    EVENT_SILPH_CO_5_UNLOCKED_DOOR1 = 0x728,
    /// Bit index: 0x729 (1833)
    EVENT_SILPH_CO_5_UNLOCKED_DOOR2 = 0x729,
    /// Bit index: 0x72A (1834)
    EVENT_SILPH_CO_5_UNLOCKED_DOOR3 = 0x72A,
    /// Bit index: 0x736 (1846)
    EVENT_BEAT_SILPH_CO_6F_TRAINER_0 = 0x736,
    /// Bit index: 0x737 (1847)
    EVENT_BEAT_SILPH_CO_6F_TRAINER_1 = 0x737,
    /// Bit index: 0x738 (1848)
    EVENT_BEAT_SILPH_CO_6F_TRAINER_2 = 0x738,
    /// Bit index: 0x73F (1855)
    EVENT_SILPH_CO_6_UNLOCKED_DOOR = 0x73F,
    /// Bit index: 0x740 (1856)
    EVENT_BEAT_SILPH_CO_RIVAL = 0x740,
    /// Bit index: 0x745 (1861)
    EVENT_BEAT_SILPH_CO_7F_TRAINER_0 = 0x745,
    /// Bit index: 0x746 (1862)
    EVENT_BEAT_SILPH_CO_7F_TRAINER_1 = 0x746,
    /// Bit index: 0x747 (1863)
    EVENT_BEAT_SILPH_CO_7F_TRAINER_2 = 0x747,
    /// Bit index: 0x748 (1864)
    EVENT_BEAT_SILPH_CO_7F_TRAINER_3 = 0x748,
    /// Bit index: 0x74C (1868)
    EVENT_SILPH_CO_7_UNLOCKED_DOOR1 = 0x74C,
    /// Bit index: 0x74D (1869)
    EVENT_SILPH_CO_7_UNLOCKED_DOOR2 = 0x74D,
    /// Bit index: 0x74E (1870)
    EVENT_SILPH_CO_7_UNLOCKED_DOOR3 = 0x74E,
    /// Bit index: 0x752 (1874)
    EVENT_BEAT_SILPH_CO_8F_TRAINER_0 = 0x752,
    /// Bit index: 0x753 (1875)
    EVENT_BEAT_SILPH_CO_8F_TRAINER_1 = 0x753,
    /// Bit index: 0x754 (1876)
    EVENT_BEAT_SILPH_CO_8F_TRAINER_2 = 0x754,
    /// Bit index: 0x758 (1880)
    EVENT_SILPH_CO_8_UNLOCKED_DOOR = 0x758,
    /// Bit index: 0x762 (1890)
    EVENT_BEAT_SILPH_CO_9F_TRAINER_0 = 0x762,
    /// Bit index: 0x763 (1891)
    EVENT_BEAT_SILPH_CO_9F_TRAINER_1 = 0x763,
    /// Bit index: 0x764 (1892)
    EVENT_BEAT_SILPH_CO_9F_TRAINER_2 = 0x764,
    /// Bit index: 0x768 (1896)
    EVENT_SILPH_CO_9_UNLOCKED_DOOR1 = 0x768,
    /// Bit index: 0x769 (1897)
    EVENT_SILPH_CO_9_UNLOCKED_DOOR2 = 0x769,
    /// Bit index: 0x76A (1898)
    EVENT_SILPH_CO_9_UNLOCKED_DOOR3 = 0x76A,
    /// Bit index: 0x76B (1899)
    EVENT_SILPH_CO_9_UNLOCKED_DOOR4 = 0x76B,
    /// Bit index: 0x771 (1905)
    EVENT_BEAT_SILPH_CO_10F_TRAINER_0 = 0x771,
    /// Bit index: 0x772 (1906)
    EVENT_BEAT_SILPH_CO_10F_TRAINER_1 = 0x772,
    /// Bit index: 0x778 (1912)
    EVENT_SILPH_CO_10_UNLOCKED_DOOR = 0x778,
    /// Bit index: 0x784 (1924)
    EVENT_BEAT_SILPH_CO_11F_TRAINER_0 = 0x784,
    /// Bit index: 0x785 (1925)
    EVENT_BEAT_SILPH_CO_11F_TRAINER_1 = 0x785,
    /// Bit index: 0x788 (1928)
    EVENT_SILPH_CO_11_UNLOCKED_DOOR = 0x788,
    /// Bit index: 0x78D (1933)
    EVENT_GOT_MASTER_BALL = 0x78D,
    /// Bit index: 0x78F (1935)
    EVENT_BEAT_SILPH_CO_GIOVANNI = 0x78F,
    /// Bit index: 0x801 (2049)
    EVENT_BEAT_MANSION_2_TRAINER_0 = 0x801,
    /// Bit index: 0x811 (2065)
    EVENT_BEAT_MANSION_3_TRAINER_0 = 0x811,
    /// Bit index: 0x812 (2066)
    EVENT_BEAT_MANSION_3_TRAINER_1 = 0x812,
    /// Bit index: 0x821 (2081)
    EVENT_BEAT_MANSION_4_TRAINER_0 = 0x821,
    /// Bit index: 0x822 (2082)
    EVENT_BEAT_MANSION_4_TRAINER_1 = 0x822,
    /// Bit index: 0x880 (2176)
    EVENT_GOT_HM03 = 0x880,
    /// Bit index: 0x8C1 (2241)
    EVENT_BEAT_MEWTWO = 0x8C1,
    /// Bit index: 0x8E1 (2273)
    EVENT_BEAT_LORELEIS_ROOM_TRAINER_0 = 0x8E1,
    /// Bit index: 0x8E6 (2278)
    EVENT_AUTOWALKED_INTO_LORELEIS_ROOM = 0x8E6,
    /// Bit index: 0x8E9 (2281)
    EVENT_BEAT_BRUNOS_ROOM_TRAINER_0 = 0x8E9,
    /// Bit index: 0x8EE (2286)
    EVENT_AUTOWALKED_INTO_BRUNOS_ROOM = 0x8EE,
    /// Bit index: 0x8F1 (2289)
    EVENT_BEAT_AGATHAS_ROOM_TRAINER_0 = 0x8F1,
    /// Bit index: 0x8F6 (2294)
    EVENT_AUTOWALKED_INTO_AGATHAS_ROOM = 0x8F6,
    /// Bit index: 0x8F9 (2297)
    EVENT_BEAT_LANCES_ROOM_TRAINER_0 = 0x8F9,
    /// Bit index: 0x8FE (2302)
    EVENT_BEAT_LANCE = 0x8FE,
    /// Bit index: 0x8FF (2303)
    EVENT_LANCES_ROOM_LOCK_DOOR = 0x8FF,
    /// Bit index: 0x901 (2305)
    EVENT_BEAT_CHAMPION_RIVAL = 0x901,
    /// Bit index: 0x911 (2321)
    EVENT_BEAT_VICTORY_ROAD_1_TRAINER_0 = 0x911,
    /// Bit index: 0x912 (2322)
    EVENT_BEAT_VICTORY_ROAD_1_TRAINER_1 = 0x912,
    /// Bit index: 0x917 (2327)
    EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH = 0x917,
    /// Bit index: 0x9B1 (2481)
    EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_0 = 0x9B1,
    /// Bit index: 0x9B2 (2482)
    EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_1 = 0x9B2,
    /// Bit index: 0x9B3 (2483)
    EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_2 = 0x9B3,
    /// Bit index: 0x9B4 (2484)
    EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_3 = 0x9B4,
    /// Bit index: 0x9B5 (2485)
    EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_4 = 0x9B5,
    /// Bit index: 0x9B6 (2486)
    EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_5 = 0x9B6,
    /// Bit index: 0x9B7 (2487)
    EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_6 = 0x9B7,
    /// Bit index: 0x9B8 (2488)
    EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_7 = 0x9B8,
    /// Bit index: 0x9C0 (2496)
    EVENT_SEAFOAM2_BOULDER1_DOWN_HOLE = 0x9C0,
    /// Bit index: 0x9C1 (2497)
    EVENT_SEAFOAM2_BOULDER2_DOWN_HOLE = 0x9C1,
    /// Bit index: 0x9C8 (2504)
    EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE = 0x9C8,
    /// Bit index: 0x9C9 (2505)
    EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE = 0x9C9,
    /// Bit index: 0x9D0 (2512)
    EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE = 0x9D0,
    /// Bit index: 0x9D1 (2513)
    EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE = 0x9D1,
    /// Bit index: 0x9DA (2522)
    EVENT_BEAT_ARTICUNO = 0x9DA,
}

impl EventFlag {
    /// Get the bit index of this event flag.
    #[inline]
    pub const fn bit_index(self) -> u16 {
        self as u16
    }

    /// Get the byte offset in the event flags array.
    #[inline]
    pub const fn byte_offset(self) -> usize {
        (self as u16 as usize) / 8
    }

    /// Get the bit within the byte.
    #[inline]
    pub const fn bit_mask(self) -> u8 {
        1 << ((self as u16) % 8)
    }

    /// Get the string name of this event flag.
    pub const fn name(self) -> &'static str {
        match self {
            EventFlag::EVENT_FOLLOWED_OAK_INTO_LAB => "EVENT_FOLLOWED_OAK_INTO_LAB",
            EventFlag::EVENT_HALL_OF_FAME_DEX_RATING => "EVENT_HALL_OF_FAME_DEX_RATING",
            EventFlag::EVENT_PALLET_AFTER_GETTING_POKEBALLS => {
                "EVENT_PALLET_AFTER_GETTING_POKEBALLS"
            }
            EventFlag::EVENT_GOT_TOWN_MAP => "EVENT_GOT_TOWN_MAP",
            EventFlag::EVENT_ENTERED_BLUES_HOUSE => "EVENT_ENTERED_BLUES_HOUSE",
            EventFlag::EVENT_DAISY_WALKING => "EVENT_DAISY_WALKING",
            EventFlag::EVENT_FOLLOWED_OAK_INTO_LAB_2 => "EVENT_FOLLOWED_OAK_INTO_LAB_2",
            EventFlag::EVENT_OAK_ASKED_TO_CHOOSE_MON => "EVENT_OAK_ASKED_TO_CHOOSE_MON",
            EventFlag::EVENT_GOT_STARTER => "EVENT_GOT_STARTER",
            EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB => "EVENT_BATTLED_RIVAL_IN_OAKS_LAB",
            EventFlag::EVENT_GOT_POKEBALLS_FROM_OAK => "EVENT_GOT_POKEBALLS_FROM_OAK",
            EventFlag::EVENT_GOT_POKEDEX => "EVENT_GOT_POKEDEX",
            EventFlag::EVENT_PALLET_AFTER_GETTING_POKEBALLS_2 => {
                "EVENT_PALLET_AFTER_GETTING_POKEBALLS_2"
            }
            EventFlag::EVENT_OAK_APPEARED_IN_PALLET => "EVENT_OAK_APPEARED_IN_PALLET",
            EventFlag::EVENT_VIRIDIAN_GYM_OPEN => "EVENT_VIRIDIAN_GYM_OPEN",
            EventFlag::EVENT_GOT_TM42 => "EVENT_GOT_TM42",
            EventFlag::EVENT_OAK_GOT_PARCEL => "EVENT_OAK_GOT_PARCEL",
            EventFlag::EVENT_GOT_OAKS_PARCEL => "EVENT_GOT_OAKS_PARCEL",
            EventFlag::EVENT_GOT_TM27 => "EVENT_GOT_TM27",
            EventFlag::EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI => "EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI",
            EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_0 => "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_0",
            EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_1 => "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_1",
            EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_2 => "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_2",
            EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_3 => "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_3",
            EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_4 => "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_4",
            EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_5 => "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_5",
            EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_6 => "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_6",
            EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_7 => "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_7",
            EventFlag::EVENT_BOUGHT_MUSEUM_TICKET => "EVENT_BOUGHT_MUSEUM_TICKET",
            EventFlag::EVENT_GOT_OLD_AMBER => "EVENT_GOT_OLD_AMBER",
            EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0 => "EVENT_BEAT_PEWTER_GYM_TRAINER_0",
            EventFlag::EVENT_GOT_TM34 => "EVENT_GOT_TM34",
            EventFlag::EVENT_BEAT_BROCK => "EVENT_BEAT_BROCK",
            EventFlag::EVENT_BEAT_CERULEAN_RIVAL => "EVENT_BEAT_CERULEAN_RIVAL",
            EventFlag::EVENT_BEAT_CERULEAN_ROCKET_THIEF => "EVENT_BEAT_CERULEAN_ROCKET_THIEF",
            EventFlag::EVENT_BEAT_CERULEAN_GYM_TRAINER_0 => "EVENT_BEAT_CERULEAN_GYM_TRAINER_0",
            EventFlag::EVENT_BEAT_CERULEAN_GYM_TRAINER_1 => "EVENT_BEAT_CERULEAN_GYM_TRAINER_1",
            EventFlag::EVENT_GOT_TM11 => "EVENT_GOT_TM11",
            EventFlag::EVENT_BEAT_MISTY => "EVENT_BEAT_MISTY",
            EventFlag::EVENT_GOT_BICYCLE => "EVENT_GOT_BICYCLE",
            EventFlag::EVENT_POKEMON_TOWER_RIVAL_ON_LEFT => "EVENT_POKEMON_TOWER_RIVAL_ON_LEFT",
            EventFlag::EVENT_BEAT_POKEMON_TOWER_RIVAL => "EVENT_BEAT_POKEMON_TOWER_RIVAL",
            EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_0 => "EVENT_BEAT_POKEMONTOWER_3_TRAINER_0",
            EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_1 => "EVENT_BEAT_POKEMONTOWER_3_TRAINER_1",
            EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_2 => "EVENT_BEAT_POKEMONTOWER_3_TRAINER_2",
            EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_0 => "EVENT_BEAT_POKEMONTOWER_4_TRAINER_0",
            EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_1 => "EVENT_BEAT_POKEMONTOWER_4_TRAINER_1",
            EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_2 => "EVENT_BEAT_POKEMONTOWER_4_TRAINER_2",
            EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_0 => "EVENT_BEAT_POKEMONTOWER_5_TRAINER_0",
            EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_1 => "EVENT_BEAT_POKEMONTOWER_5_TRAINER_1",
            EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_2 => "EVENT_BEAT_POKEMONTOWER_5_TRAINER_2",
            EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_3 => "EVENT_BEAT_POKEMONTOWER_5_TRAINER_3",
            EventFlag::EVENT_IN_PURIFIED_ZONE => "EVENT_IN_PURIFIED_ZONE",
            EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_0 => "EVENT_BEAT_POKEMONTOWER_6_TRAINER_0",
            EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_1 => "EVENT_BEAT_POKEMONTOWER_6_TRAINER_1",
            EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_2 => "EVENT_BEAT_POKEMONTOWER_6_TRAINER_2",
            EventFlag::EVENT_BEAT_GHOST_MAROWAK => "EVENT_BEAT_GHOST_MAROWAK",
            EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_0 => "EVENT_BEAT_POKEMONTOWER_7_TRAINER_0",
            EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_1 => "EVENT_BEAT_POKEMONTOWER_7_TRAINER_1",
            EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_2 => "EVENT_BEAT_POKEMONTOWER_7_TRAINER_2",
            EventFlag::EVENT_RESCUED_MR_FUJI_2 => "EVENT_RESCUED_MR_FUJI_2",
            EventFlag::EVENT_GOT_POKE_FLUTE => "EVENT_GOT_POKE_FLUTE",
            EventFlag::EVENT_GOT_BIKE_VOUCHER => "EVENT_GOT_BIKE_VOUCHER",
            EventFlag::EVENT_SEEL_FAN_BOAST => "EVENT_SEEL_FAN_BOAST",
            EventFlag::EVENT_PIKACHU_FAN_BOAST => "EVENT_PIKACHU_FAN_BOAST",
            EventFlag::EVENT_2ND_LOCK_OPENED => "EVENT_2ND_LOCK_OPENED",
            EventFlag::EVENT_1ST_LOCK_OPENED => "EVENT_1ST_LOCK_OPENED",
            EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_0 => "EVENT_BEAT_VERMILION_GYM_TRAINER_0",
            EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_1 => "EVENT_BEAT_VERMILION_GYM_TRAINER_1",
            EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_2 => "EVENT_BEAT_VERMILION_GYM_TRAINER_2",
            EventFlag::EVENT_GOT_TM24 => "EVENT_GOT_TM24",
            EventFlag::EVENT_BEAT_LT_SURGE => "EVENT_BEAT_LT_SURGE",
            EventFlag::EVENT_GOT_TM41 => "EVENT_GOT_TM41",
            EventFlag::EVENT_GOT_TM13 => "EVENT_GOT_TM13",
            EventFlag::EVENT_GOT_TM48 => "EVENT_GOT_TM48",
            EventFlag::EVENT_GOT_TM49 => "EVENT_GOT_TM49",
            EventFlag::EVENT_GOT_TM18 => "EVENT_GOT_TM18",
            EventFlag::EVENT_GOT_TM21 => "EVENT_GOT_TM21",
            EventFlag::EVENT_BEAT_ERIKA => "EVENT_BEAT_ERIKA",
            EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_0 => "EVENT_BEAT_CELADON_GYM_TRAINER_0",
            EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_1 => "EVENT_BEAT_CELADON_GYM_TRAINER_1",
            EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_2 => "EVENT_BEAT_CELADON_GYM_TRAINER_2",
            EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_3 => "EVENT_BEAT_CELADON_GYM_TRAINER_3",
            EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_4 => "EVENT_BEAT_CELADON_GYM_TRAINER_4",
            EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_5 => "EVENT_BEAT_CELADON_GYM_TRAINER_5",
            EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_6 => "EVENT_BEAT_CELADON_GYM_TRAINER_6",
            EventFlag::EVENT_1B8 => "EVENT_1B8",
            EventFlag::EVENT_FOUND_ROCKET_HIDEOUT => "EVENT_FOUND_ROCKET_HIDEOUT",
            EventFlag::EVENT_GOT_10_COINS => "EVENT_GOT_10_COINS",
            EventFlag::EVENT_GOT_20_COINS => "EVENT_GOT_20_COINS",
            EventFlag::EVENT_GOT_20_COINS_2 => "EVENT_GOT_20_COINS_2",
            EventFlag::EVENT_1BF => "EVENT_1BF",
            EventFlag::EVENT_GOT_COIN_CASE => "EVENT_GOT_COIN_CASE",
            EventFlag::EVENT_GOT_HM04 => "EVENT_GOT_HM04",
            EventFlag::EVENT_GAVE_GOLD_TEETH => "EVENT_GAVE_GOLD_TEETH",
            EventFlag::EVENT_SAFARI_GAME_OVER => "EVENT_SAFARI_GAME_OVER",
            EventFlag::EVENT_IN_SAFARI_ZONE => "EVENT_IN_SAFARI_ZONE",
            EventFlag::EVENT_GOT_TM06 => "EVENT_GOT_TM06",
            EventFlag::EVENT_BEAT_KOGA => "EVENT_BEAT_KOGA",
            EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_0 => "EVENT_BEAT_FUCHSIA_GYM_TRAINER_0",
            EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_1 => "EVENT_BEAT_FUCHSIA_GYM_TRAINER_1",
            EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_2 => "EVENT_BEAT_FUCHSIA_GYM_TRAINER_2",
            EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_3 => "EVENT_BEAT_FUCHSIA_GYM_TRAINER_3",
            EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_4 => "EVENT_BEAT_FUCHSIA_GYM_TRAINER_4",
            EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_5 => "EVENT_BEAT_FUCHSIA_GYM_TRAINER_5",
            EventFlag::EVENT_MANSION_SWITCH_ON => "EVENT_MANSION_SWITCH_ON",
            EventFlag::EVENT_BEAT_MANSION_1_TRAINER_0 => "EVENT_BEAT_MANSION_1_TRAINER_0",
            EventFlag::EVENT_GOT_TM38 => "EVENT_GOT_TM38",
            EventFlag::EVENT_BEAT_BLAINE => "EVENT_BEAT_BLAINE",
            EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_0 => "EVENT_BEAT_CINNABAR_GYM_TRAINER_0",
            EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_1 => "EVENT_BEAT_CINNABAR_GYM_TRAINER_1",
            EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_2 => "EVENT_BEAT_CINNABAR_GYM_TRAINER_2",
            EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_3 => "EVENT_BEAT_CINNABAR_GYM_TRAINER_3",
            EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_4 => "EVENT_BEAT_CINNABAR_GYM_TRAINER_4",
            EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_5 => "EVENT_BEAT_CINNABAR_GYM_TRAINER_5",
            EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_6 => "EVENT_BEAT_CINNABAR_GYM_TRAINER_6",
            EventFlag::EVENT_2A7 => "EVENT_2A7",
            EventFlag::EVENT_CINNABAR_GYM_GATE0_UNLOCKED => "EVENT_CINNABAR_GYM_GATE0_UNLOCKED",
            EventFlag::EVENT_CINNABAR_GYM_GATE1_UNLOCKED => "EVENT_CINNABAR_GYM_GATE1_UNLOCKED",
            EventFlag::EVENT_CINNABAR_GYM_GATE2_UNLOCKED => "EVENT_CINNABAR_GYM_GATE2_UNLOCKED",
            EventFlag::EVENT_CINNABAR_GYM_GATE3_UNLOCKED => "EVENT_CINNABAR_GYM_GATE3_UNLOCKED",
            EventFlag::EVENT_CINNABAR_GYM_GATE4_UNLOCKED => "EVENT_CINNABAR_GYM_GATE4_UNLOCKED",
            EventFlag::EVENT_CINNABAR_GYM_GATE5_UNLOCKED => "EVENT_CINNABAR_GYM_GATE5_UNLOCKED",
            EventFlag::EVENT_CINNABAR_GYM_GATE6_UNLOCKED => "EVENT_CINNABAR_GYM_GATE6_UNLOCKED",
            EventFlag::EVENT_GOT_TM35 => "EVENT_GOT_TM35",
            EventFlag::EVENT_GAVE_FOSSIL_TO_LAB => "EVENT_GAVE_FOSSIL_TO_LAB",
            EventFlag::EVENT_LAB_STILL_REVIVING_FOSSIL => "EVENT_LAB_STILL_REVIVING_FOSSIL",
            EventFlag::EVENT_LAB_HANDING_OVER_FOSSIL_MON => "EVENT_LAB_HANDING_OVER_FOSSIL_MON",
            EventFlag::EVENT_GOT_TM31 => "EVENT_GOT_TM31",
            EventFlag::EVENT_DEFEATED_FIGHTING_DOJO => "EVENT_DEFEATED_FIGHTING_DOJO",
            EventFlag::EVENT_BEAT_KARATE_MASTER => "EVENT_BEAT_KARATE_MASTER",
            EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_0 => "EVENT_BEAT_FIGHTING_DOJO_TRAINER_0",
            EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_1 => "EVENT_BEAT_FIGHTING_DOJO_TRAINER_1",
            EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_2 => "EVENT_BEAT_FIGHTING_DOJO_TRAINER_2",
            EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_3 => "EVENT_BEAT_FIGHTING_DOJO_TRAINER_3",
            EventFlag::EVENT_GOT_HITMONLEE => "EVENT_GOT_HITMONLEE",
            EventFlag::EVENT_GOT_HITMONCHAN => "EVENT_GOT_HITMONCHAN",
            EventFlag::EVENT_GOT_TM46 => "EVENT_GOT_TM46",
            EventFlag::EVENT_BEAT_SABRINA => "EVENT_BEAT_SABRINA",
            EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_0 => "EVENT_BEAT_SAFFRON_GYM_TRAINER_0",
            EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_1 => "EVENT_BEAT_SAFFRON_GYM_TRAINER_1",
            EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_2 => "EVENT_BEAT_SAFFRON_GYM_TRAINER_2",
            EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_3 => "EVENT_BEAT_SAFFRON_GYM_TRAINER_3",
            EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_4 => "EVENT_BEAT_SAFFRON_GYM_TRAINER_4",
            EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_5 => "EVENT_BEAT_SAFFRON_GYM_TRAINER_5",
            EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_6 => "EVENT_BEAT_SAFFRON_GYM_TRAINER_6",
            EventFlag::EVENT_SILPH_CO_RECEPTIONIST_AT_DESK => "EVENT_SILPH_CO_RECEPTIONIST_AT_DESK",
            EventFlag::EVENT_GOT_TM29 => "EVENT_GOT_TM29",
            EventFlag::EVENT_GOT_POTION_SAMPLE => "EVENT_GOT_POTION_SAMPLE",
            EventFlag::EVENT_GOT_HM05 => "EVENT_GOT_HM05",
            EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_0 => "EVENT_BEAT_ROUTE_3_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_1 => "EVENT_BEAT_ROUTE_3_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_2 => "EVENT_BEAT_ROUTE_3_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_3 => "EVENT_BEAT_ROUTE_3_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_4 => "EVENT_BEAT_ROUTE_3_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_5 => "EVENT_BEAT_ROUTE_3_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_6 => "EVENT_BEAT_ROUTE_3_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_7 => "EVENT_BEAT_ROUTE_3_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_4_TRAINER_0 => "EVENT_BEAT_ROUTE_4_TRAINER_0",
            EventFlag::EVENT_BOUGHT_MAGIKARP => "EVENT_BOUGHT_MAGIKARP",
            EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_0 => "EVENT_BEAT_ROUTE_6_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_1 => "EVENT_BEAT_ROUTE_6_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_2 => "EVENT_BEAT_ROUTE_6_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_3 => "EVENT_BEAT_ROUTE_6_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_4 => "EVENT_BEAT_ROUTE_6_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_5 => "EVENT_BEAT_ROUTE_6_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_0 => "EVENT_BEAT_ROUTE_8_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_1 => "EVENT_BEAT_ROUTE_8_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_2 => "EVENT_BEAT_ROUTE_8_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_3 => "EVENT_BEAT_ROUTE_8_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_4 => "EVENT_BEAT_ROUTE_8_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_5 => "EVENT_BEAT_ROUTE_8_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_6 => "EVENT_BEAT_ROUTE_8_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_7 => "EVENT_BEAT_ROUTE_8_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_8 => "EVENT_BEAT_ROUTE_8_TRAINER_8",
            EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_0 => "EVENT_BEAT_ROUTE_9_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_1 => "EVENT_BEAT_ROUTE_9_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_2 => "EVENT_BEAT_ROUTE_9_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_3 => "EVENT_BEAT_ROUTE_9_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_4 => "EVENT_BEAT_ROUTE_9_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_5 => "EVENT_BEAT_ROUTE_9_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_6 => "EVENT_BEAT_ROUTE_9_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_7 => "EVENT_BEAT_ROUTE_9_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_8 => "EVENT_BEAT_ROUTE_9_TRAINER_8",
            EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_0 => "EVENT_BEAT_ROUTE_10_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_1 => "EVENT_BEAT_ROUTE_10_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_2 => "EVENT_BEAT_ROUTE_10_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_3 => "EVENT_BEAT_ROUTE_10_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_4 => "EVENT_BEAT_ROUTE_10_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_5 => "EVENT_BEAT_ROUTE_10_TRAINER_5",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_0 => "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_0",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_1 => "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_1",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_2 => "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_2",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_3 => "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_3",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_4 => "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_4",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_5 => "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_5",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_6 => "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_6",
            EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_0 => "EVENT_BEAT_POWER_PLANT_VOLTORB_0",
            EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_1 => "EVENT_BEAT_POWER_PLANT_VOLTORB_1",
            EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_2 => "EVENT_BEAT_POWER_PLANT_VOLTORB_2",
            EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_3 => "EVENT_BEAT_POWER_PLANT_VOLTORB_3",
            EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_4 => "EVENT_BEAT_POWER_PLANT_VOLTORB_4",
            EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_5 => "EVENT_BEAT_POWER_PLANT_VOLTORB_5",
            EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_6 => "EVENT_BEAT_POWER_PLANT_VOLTORB_6",
            EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_7 => "EVENT_BEAT_POWER_PLANT_VOLTORB_7",
            EventFlag::EVENT_BEAT_ZAPDOS => "EVENT_BEAT_ZAPDOS",
            EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_0 => "EVENT_BEAT_ROUTE_11_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_1 => "EVENT_BEAT_ROUTE_11_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_2 => "EVENT_BEAT_ROUTE_11_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_3 => "EVENT_BEAT_ROUTE_11_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_4 => "EVENT_BEAT_ROUTE_11_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_5 => "EVENT_BEAT_ROUTE_11_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_6 => "EVENT_BEAT_ROUTE_11_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_7 => "EVENT_BEAT_ROUTE_11_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_8 => "EVENT_BEAT_ROUTE_11_TRAINER_8",
            EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_9 => "EVENT_BEAT_ROUTE_11_TRAINER_9",
            EventFlag::EVENT_GOT_ITEMFINDER => "EVENT_GOT_ITEMFINDER",
            EventFlag::EVENT_GOT_TM39 => "EVENT_GOT_TM39",
            EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_0 => "EVENT_BEAT_ROUTE_12_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_1 => "EVENT_BEAT_ROUTE_12_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_2 => "EVENT_BEAT_ROUTE_12_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_3 => "EVENT_BEAT_ROUTE_12_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_4 => "EVENT_BEAT_ROUTE_12_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_5 => "EVENT_BEAT_ROUTE_12_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_6 => "EVENT_BEAT_ROUTE_12_TRAINER_6",
            EventFlag::EVENT_FIGHT_ROUTE12_SNORLAX => "EVENT_FIGHT_ROUTE12_SNORLAX",
            EventFlag::EVENT_BEAT_ROUTE12_SNORLAX => "EVENT_BEAT_ROUTE12_SNORLAX",
            EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_0 => "EVENT_BEAT_ROUTE_13_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_1 => "EVENT_BEAT_ROUTE_13_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_2 => "EVENT_BEAT_ROUTE_13_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_3 => "EVENT_BEAT_ROUTE_13_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_4 => "EVENT_BEAT_ROUTE_13_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_5 => "EVENT_BEAT_ROUTE_13_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_6 => "EVENT_BEAT_ROUTE_13_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_7 => "EVENT_BEAT_ROUTE_13_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_8 => "EVENT_BEAT_ROUTE_13_TRAINER_8",
            EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_9 => "EVENT_BEAT_ROUTE_13_TRAINER_9",
            EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_0 => "EVENT_BEAT_ROUTE_14_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_1 => "EVENT_BEAT_ROUTE_14_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_2 => "EVENT_BEAT_ROUTE_14_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_3 => "EVENT_BEAT_ROUTE_14_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_4 => "EVENT_BEAT_ROUTE_14_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_5 => "EVENT_BEAT_ROUTE_14_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_6 => "EVENT_BEAT_ROUTE_14_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_7 => "EVENT_BEAT_ROUTE_14_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_8 => "EVENT_BEAT_ROUTE_14_TRAINER_8",
            EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_9 => "EVENT_BEAT_ROUTE_14_TRAINER_9",
            EventFlag::EVENT_GOT_EXP_ALL => "EVENT_GOT_EXP_ALL",
            EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_0 => "EVENT_BEAT_ROUTE_15_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_1 => "EVENT_BEAT_ROUTE_15_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_2 => "EVENT_BEAT_ROUTE_15_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_3 => "EVENT_BEAT_ROUTE_15_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_4 => "EVENT_BEAT_ROUTE_15_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_5 => "EVENT_BEAT_ROUTE_15_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_6 => "EVENT_BEAT_ROUTE_15_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_7 => "EVENT_BEAT_ROUTE_15_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_8 => "EVENT_BEAT_ROUTE_15_TRAINER_8",
            EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_9 => "EVENT_BEAT_ROUTE_15_TRAINER_9",
            EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_0 => "EVENT_BEAT_ROUTE_16_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_1 => "EVENT_BEAT_ROUTE_16_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_2 => "EVENT_BEAT_ROUTE_16_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_3 => "EVENT_BEAT_ROUTE_16_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_4 => "EVENT_BEAT_ROUTE_16_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_5 => "EVENT_BEAT_ROUTE_16_TRAINER_5",
            EventFlag::EVENT_FIGHT_ROUTE16_SNORLAX => "EVENT_FIGHT_ROUTE16_SNORLAX",
            EventFlag::EVENT_BEAT_ROUTE16_SNORLAX => "EVENT_BEAT_ROUTE16_SNORLAX",
            EventFlag::EVENT_GOT_HM02 => "EVENT_GOT_HM02",
            EventFlag::EVENT_RESCUED_MR_FUJI => "EVENT_RESCUED_MR_FUJI",
            EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_0 => "EVENT_BEAT_ROUTE_17_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_1 => "EVENT_BEAT_ROUTE_17_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_2 => "EVENT_BEAT_ROUTE_17_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_3 => "EVENT_BEAT_ROUTE_17_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_4 => "EVENT_BEAT_ROUTE_17_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_5 => "EVENT_BEAT_ROUTE_17_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_6 => "EVENT_BEAT_ROUTE_17_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_7 => "EVENT_BEAT_ROUTE_17_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_8 => "EVENT_BEAT_ROUTE_17_TRAINER_8",
            EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_9 => "EVENT_BEAT_ROUTE_17_TRAINER_9",
            EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_0 => "EVENT_BEAT_ROUTE_18_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_1 => "EVENT_BEAT_ROUTE_18_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_2 => "EVENT_BEAT_ROUTE_18_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_0 => "EVENT_BEAT_ROUTE_19_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_1 => "EVENT_BEAT_ROUTE_19_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_2 => "EVENT_BEAT_ROUTE_19_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_3 => "EVENT_BEAT_ROUTE_19_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_4 => "EVENT_BEAT_ROUTE_19_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_5 => "EVENT_BEAT_ROUTE_19_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_6 => "EVENT_BEAT_ROUTE_19_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_7 => "EVENT_BEAT_ROUTE_19_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_8 => "EVENT_BEAT_ROUTE_19_TRAINER_8",
            EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_9 => "EVENT_BEAT_ROUTE_19_TRAINER_9",
            EventFlag::EVENT_IN_SEAFOAM_ISLANDS => "EVENT_IN_SEAFOAM_ISLANDS",
            EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_0 => "EVENT_BEAT_ROUTE_20_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_1 => "EVENT_BEAT_ROUTE_20_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_2 => "EVENT_BEAT_ROUTE_20_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_3 => "EVENT_BEAT_ROUTE_20_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_4 => "EVENT_BEAT_ROUTE_20_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_5 => "EVENT_BEAT_ROUTE_20_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_6 => "EVENT_BEAT_ROUTE_20_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_7 => "EVENT_BEAT_ROUTE_20_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_8 => "EVENT_BEAT_ROUTE_20_TRAINER_8",
            EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_9 => "EVENT_BEAT_ROUTE_20_TRAINER_9",
            EventFlag::EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE => "EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE",
            EventFlag::EVENT_SEAFOAM1_BOULDER2_DOWN_HOLE => "EVENT_SEAFOAM1_BOULDER2_DOWN_HOLE",
            EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_0 => "EVENT_BEAT_ROUTE_21_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_1 => "EVENT_BEAT_ROUTE_21_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_2 => "EVENT_BEAT_ROUTE_21_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_3 => "EVENT_BEAT_ROUTE_21_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_4 => "EVENT_BEAT_ROUTE_21_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_5 => "EVENT_BEAT_ROUTE_21_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_6 => "EVENT_BEAT_ROUTE_21_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_7 => "EVENT_BEAT_ROUTE_21_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_8 => "EVENT_BEAT_ROUTE_21_TRAINER_8",
            EventFlag::EVENT_1ST_ROUTE22_RIVAL_BATTLE => "EVENT_1ST_ROUTE22_RIVAL_BATTLE",
            EventFlag::EVENT_2ND_ROUTE22_RIVAL_BATTLE => "EVENT_2ND_ROUTE22_RIVAL_BATTLE",
            EventFlag::EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE => "EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE",
            EventFlag::EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE => "EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE",
            EventFlag::EVENT_ROUTE22_RIVAL_WANTS_BATTLE => "EVENT_ROUTE22_RIVAL_WANTS_BATTLE",
            EventFlag::EVENT_PASSED_CASCADEBADGE_CHECK => "EVENT_PASSED_CASCADEBADGE_CHECK",
            EventFlag::EVENT_PASSED_THUNDERBADGE_CHECK => "EVENT_PASSED_THUNDERBADGE_CHECK",
            EventFlag::EVENT_PASSED_RAINBOWBADGE_CHECK => "EVENT_PASSED_RAINBOWBADGE_CHECK",
            EventFlag::EVENT_PASSED_SOULBADGE_CHECK => "EVENT_PASSED_SOULBADGE_CHECK",
            EventFlag::EVENT_PASSED_MARSHBADGE_CHECK => "EVENT_PASSED_MARSHBADGE_CHECK",
            EventFlag::EVENT_PASSED_VOLCANOBADGE_CHECK => "EVENT_PASSED_VOLCANOBADGE_CHECK",
            EventFlag::EVENT_PASSED_EARTHBADGE_CHECK => "EVENT_PASSED_EARTHBADGE_CHECK",
            EventFlag::EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1 => {
                "EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1"
            }
            EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_0 => "EVENT_BEAT_VICTORY_ROAD_2_TRAINER_0",
            EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_1 => "EVENT_BEAT_VICTORY_ROAD_2_TRAINER_1",
            EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_2 => "EVENT_BEAT_VICTORY_ROAD_2_TRAINER_2",
            EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_3 => "EVENT_BEAT_VICTORY_ROAD_2_TRAINER_3",
            EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_4 => "EVENT_BEAT_VICTORY_ROAD_2_TRAINER_4",
            EventFlag::EVENT_BEAT_MOLTRES => "EVENT_BEAT_MOLTRES",
            EventFlag::EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2 => {
                "EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2"
            }
            EventFlag::EVENT_GOT_NUGGET => "EVENT_GOT_NUGGET",
            EventFlag::EVENT_BEAT_ROUTE24_ROCKET => "EVENT_BEAT_ROUTE24_ROCKET",
            EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_0 => "EVENT_BEAT_ROUTE_24_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_1 => "EVENT_BEAT_ROUTE_24_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_2 => "EVENT_BEAT_ROUTE_24_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_3 => "EVENT_BEAT_ROUTE_24_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_4 => "EVENT_BEAT_ROUTE_24_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_5 => "EVENT_BEAT_ROUTE_24_TRAINER_5",
            EventFlag::EVENT_NUGGET_REWARD_AVAILABLE => "EVENT_NUGGET_REWARD_AVAILABLE",
            EventFlag::EVENT_MET_BILL => "EVENT_MET_BILL",
            EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_0 => "EVENT_BEAT_ROUTE_25_TRAINER_0",
            EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_1 => "EVENT_BEAT_ROUTE_25_TRAINER_1",
            EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_2 => "EVENT_BEAT_ROUTE_25_TRAINER_2",
            EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_3 => "EVENT_BEAT_ROUTE_25_TRAINER_3",
            EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_4 => "EVENT_BEAT_ROUTE_25_TRAINER_4",
            EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_5 => "EVENT_BEAT_ROUTE_25_TRAINER_5",
            EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_6 => "EVENT_BEAT_ROUTE_25_TRAINER_6",
            EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_7 => "EVENT_BEAT_ROUTE_25_TRAINER_7",
            EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_8 => "EVENT_BEAT_ROUTE_25_TRAINER_8",
            EventFlag::EVENT_USED_CELL_SEPARATOR_ON_BILL => "EVENT_USED_CELL_SEPARATOR_ON_BILL",
            EventFlag::EVENT_GOT_SS_TICKET => "EVENT_GOT_SS_TICKET",
            EventFlag::EVENT_MET_BILL_2 => "EVENT_MET_BILL_2",
            EventFlag::EVENT_BILL_SAID_USE_CELL_SEPARATOR => "EVENT_BILL_SAID_USE_CELL_SEPARATOR",
            EventFlag::EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING => {
                "EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING"
            }
            EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0 => {
                "EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0"
            }
            EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_1 => {
                "EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_1"
            }
            EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2 => {
                "EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2"
            }
            EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_0 => "EVENT_BEAT_MT_MOON_1_TRAINER_0",
            EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_1 => "EVENT_BEAT_MT_MOON_1_TRAINER_1",
            EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_2 => "EVENT_BEAT_MT_MOON_1_TRAINER_2",
            EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_3 => "EVENT_BEAT_MT_MOON_1_TRAINER_3",
            EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_4 => "EVENT_BEAT_MT_MOON_1_TRAINER_4",
            EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_5 => "EVENT_BEAT_MT_MOON_1_TRAINER_5",
            EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_6 => "EVENT_BEAT_MT_MOON_1_TRAINER_6",
            EventFlag::EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD => "EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD",
            EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_0 => "EVENT_BEAT_MT_MOON_3_TRAINER_0",
            EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_1 => "EVENT_BEAT_MT_MOON_3_TRAINER_1",
            EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_2 => "EVENT_BEAT_MT_MOON_3_TRAINER_2",
            EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_3 => "EVENT_BEAT_MT_MOON_3_TRAINER_3",
            EventFlag::EVENT_GOT_DOME_FOSSIL => "EVENT_GOT_DOME_FOSSIL",
            EventFlag::EVENT_GOT_HELIX_FOSSIL => "EVENT_GOT_HELIX_FOSSIL",
            EventFlag::EVENT_BEAT_SS_ANNE_5_TRAINER_0 => "EVENT_BEAT_SS_ANNE_5_TRAINER_0",
            EventFlag::EVENT_BEAT_SS_ANNE_5_TRAINER_1 => "EVENT_BEAT_SS_ANNE_5_TRAINER_1",
            EventFlag::EVENT_GOT_HM01 => "EVENT_GOT_HM01",
            EventFlag::EVENT_RUBBED_CAPTAINS_BACK => "EVENT_RUBBED_CAPTAINS_BACK",
            EventFlag::EVENT_SS_ANNE_LEFT => "EVENT_SS_ANNE_LEFT",
            EventFlag::EVENT_WALKED_PAST_GUARD_AFTER_SS_ANNE_LEFT => {
                "EVENT_WALKED_PAST_GUARD_AFTER_SS_ANNE_LEFT"
            }
            EventFlag::EVENT_STARTED_WALKING_OUT_OF_DOCK => "EVENT_STARTED_WALKING_OUT_OF_DOCK",
            EventFlag::EVENT_WALKED_OUT_OF_DOCK => "EVENT_WALKED_OUT_OF_DOCK",
            EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_0 => "EVENT_BEAT_SS_ANNE_8_TRAINER_0",
            EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_1 => "EVENT_BEAT_SS_ANNE_8_TRAINER_1",
            EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_2 => "EVENT_BEAT_SS_ANNE_8_TRAINER_2",
            EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_3 => "EVENT_BEAT_SS_ANNE_8_TRAINER_3",
            EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_0 => "EVENT_BEAT_SS_ANNE_9_TRAINER_0",
            EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_1 => "EVENT_BEAT_SS_ANNE_9_TRAINER_1",
            EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_2 => "EVENT_BEAT_SS_ANNE_9_TRAINER_2",
            EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_3 => "EVENT_BEAT_SS_ANNE_9_TRAINER_3",
            EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_0 => "EVENT_BEAT_SS_ANNE_10_TRAINER_0",
            EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_1 => "EVENT_BEAT_SS_ANNE_10_TRAINER_1",
            EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_2 => "EVENT_BEAT_SS_ANNE_10_TRAINER_2",
            EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_3 => "EVENT_BEAT_SS_ANNE_10_TRAINER_3",
            EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_4 => "EVENT_BEAT_SS_ANNE_10_TRAINER_4",
            EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_5 => "EVENT_BEAT_SS_ANNE_10_TRAINER_5",
            EventFlag::EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH1 => {
                "EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH1"
            }
            EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_0 => "EVENT_BEAT_VICTORY_ROAD_3_TRAINER_0",
            EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_1 => "EVENT_BEAT_VICTORY_ROAD_3_TRAINER_1",
            EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_2 => "EVENT_BEAT_VICTORY_ROAD_3_TRAINER_2",
            EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_3 => "EVENT_BEAT_VICTORY_ROAD_3_TRAINER_3",
            EventFlag::EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2 => {
                "EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2"
            }
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_0 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_0"
            }
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_1 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_1"
            }
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_2 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_2"
            }
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_3 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_3"
            }
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_4 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_4"
            }
            EventFlag::EVENT_ENTERED_ROCKET_HIDEOUT => "EVENT_ENTERED_ROCKET_HIDEOUT",
            EventFlag::EVENT_67F => "EVENT_67F",
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_2_TRAINER_0 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_2_TRAINER_0"
            }
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_0 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_0"
            }
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_1 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_1"
            }
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0"
            }
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_1 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_1"
            }
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2 => {
                "EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2"
            }
            EventFlag::EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED => {
                "EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED"
            }
            EventFlag::EVENT_ROCKET_DROPPED_LIFT_KEY => "EVENT_ROCKET_DROPPED_LIFT_KEY",
            EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_GIOVANNI => "EVENT_BEAT_ROCKET_HIDEOUT_GIOVANNI",
            EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_0 => "EVENT_BEAT_SILPH_CO_2F_TRAINER_0",
            EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_1 => "EVENT_BEAT_SILPH_CO_2F_TRAINER_1",
            EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_2 => "EVENT_BEAT_SILPH_CO_2F_TRAINER_2",
            EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_3 => "EVENT_BEAT_SILPH_CO_2F_TRAINER_3",
            EventFlag::EVENT_SILPH_CO_2_UNLOCKED_DOOR1 => "EVENT_SILPH_CO_2_UNLOCKED_DOOR1",
            EventFlag::EVENT_SILPH_CO_2_UNLOCKED_DOOR2 => "EVENT_SILPH_CO_2_UNLOCKED_DOOR2",
            EventFlag::EVENT_GOT_TM36 => "EVENT_GOT_TM36",
            EventFlag::EVENT_BEAT_SILPH_CO_3F_TRAINER_0 => "EVENT_BEAT_SILPH_CO_3F_TRAINER_0",
            EventFlag::EVENT_BEAT_SILPH_CO_3F_TRAINER_1 => "EVENT_BEAT_SILPH_CO_3F_TRAINER_1",
            EventFlag::EVENT_SILPH_CO_3_UNLOCKED_DOOR1 => "EVENT_SILPH_CO_3_UNLOCKED_DOOR1",
            EventFlag::EVENT_SILPH_CO_3_UNLOCKED_DOOR2 => "EVENT_SILPH_CO_3_UNLOCKED_DOOR2",
            EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_0 => "EVENT_BEAT_SILPH_CO_4F_TRAINER_0",
            EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_1 => "EVENT_BEAT_SILPH_CO_4F_TRAINER_1",
            EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_2 => "EVENT_BEAT_SILPH_CO_4F_TRAINER_2",
            EventFlag::EVENT_SILPH_CO_4_UNLOCKED_DOOR1 => "EVENT_SILPH_CO_4_UNLOCKED_DOOR1",
            EventFlag::EVENT_SILPH_CO_4_UNLOCKED_DOOR2 => "EVENT_SILPH_CO_4_UNLOCKED_DOOR2",
            EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_0 => "EVENT_BEAT_SILPH_CO_5F_TRAINER_0",
            EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_1 => "EVENT_BEAT_SILPH_CO_5F_TRAINER_1",
            EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_2 => "EVENT_BEAT_SILPH_CO_5F_TRAINER_2",
            EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_3 => "EVENT_BEAT_SILPH_CO_5F_TRAINER_3",
            EventFlag::EVENT_SILPH_CO_5_UNLOCKED_DOOR1 => "EVENT_SILPH_CO_5_UNLOCKED_DOOR1",
            EventFlag::EVENT_SILPH_CO_5_UNLOCKED_DOOR2 => "EVENT_SILPH_CO_5_UNLOCKED_DOOR2",
            EventFlag::EVENT_SILPH_CO_5_UNLOCKED_DOOR3 => "EVENT_SILPH_CO_5_UNLOCKED_DOOR3",
            EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_0 => "EVENT_BEAT_SILPH_CO_6F_TRAINER_0",
            EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_1 => "EVENT_BEAT_SILPH_CO_6F_TRAINER_1",
            EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_2 => "EVENT_BEAT_SILPH_CO_6F_TRAINER_2",
            EventFlag::EVENT_SILPH_CO_6_UNLOCKED_DOOR => "EVENT_SILPH_CO_6_UNLOCKED_DOOR",
            EventFlag::EVENT_BEAT_SILPH_CO_RIVAL => "EVENT_BEAT_SILPH_CO_RIVAL",
            EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_0 => "EVENT_BEAT_SILPH_CO_7F_TRAINER_0",
            EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_1 => "EVENT_BEAT_SILPH_CO_7F_TRAINER_1",
            EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_2 => "EVENT_BEAT_SILPH_CO_7F_TRAINER_2",
            EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_3 => "EVENT_BEAT_SILPH_CO_7F_TRAINER_3",
            EventFlag::EVENT_SILPH_CO_7_UNLOCKED_DOOR1 => "EVENT_SILPH_CO_7_UNLOCKED_DOOR1",
            EventFlag::EVENT_SILPH_CO_7_UNLOCKED_DOOR2 => "EVENT_SILPH_CO_7_UNLOCKED_DOOR2",
            EventFlag::EVENT_SILPH_CO_7_UNLOCKED_DOOR3 => "EVENT_SILPH_CO_7_UNLOCKED_DOOR3",
            EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_0 => "EVENT_BEAT_SILPH_CO_8F_TRAINER_0",
            EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_1 => "EVENT_BEAT_SILPH_CO_8F_TRAINER_1",
            EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_2 => "EVENT_BEAT_SILPH_CO_8F_TRAINER_2",
            EventFlag::EVENT_SILPH_CO_8_UNLOCKED_DOOR => "EVENT_SILPH_CO_8_UNLOCKED_DOOR",
            EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_0 => "EVENT_BEAT_SILPH_CO_9F_TRAINER_0",
            EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_1 => "EVENT_BEAT_SILPH_CO_9F_TRAINER_1",
            EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_2 => "EVENT_BEAT_SILPH_CO_9F_TRAINER_2",
            EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR1 => "EVENT_SILPH_CO_9_UNLOCKED_DOOR1",
            EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR2 => "EVENT_SILPH_CO_9_UNLOCKED_DOOR2",
            EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR3 => "EVENT_SILPH_CO_9_UNLOCKED_DOOR3",
            EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR4 => "EVENT_SILPH_CO_9_UNLOCKED_DOOR4",
            EventFlag::EVENT_BEAT_SILPH_CO_10F_TRAINER_0 => "EVENT_BEAT_SILPH_CO_10F_TRAINER_0",
            EventFlag::EVENT_BEAT_SILPH_CO_10F_TRAINER_1 => "EVENT_BEAT_SILPH_CO_10F_TRAINER_1",
            EventFlag::EVENT_SILPH_CO_10_UNLOCKED_DOOR => "EVENT_SILPH_CO_10_UNLOCKED_DOOR",
            EventFlag::EVENT_BEAT_SILPH_CO_11F_TRAINER_0 => "EVENT_BEAT_SILPH_CO_11F_TRAINER_0",
            EventFlag::EVENT_BEAT_SILPH_CO_11F_TRAINER_1 => "EVENT_BEAT_SILPH_CO_11F_TRAINER_1",
            EventFlag::EVENT_SILPH_CO_11_UNLOCKED_DOOR => "EVENT_SILPH_CO_11_UNLOCKED_DOOR",
            EventFlag::EVENT_GOT_MASTER_BALL => "EVENT_GOT_MASTER_BALL",
            EventFlag::EVENT_BEAT_SILPH_CO_GIOVANNI => "EVENT_BEAT_SILPH_CO_GIOVANNI",
            EventFlag::EVENT_BEAT_MANSION_2_TRAINER_0 => "EVENT_BEAT_MANSION_2_TRAINER_0",
            EventFlag::EVENT_BEAT_MANSION_3_TRAINER_0 => "EVENT_BEAT_MANSION_3_TRAINER_0",
            EventFlag::EVENT_BEAT_MANSION_3_TRAINER_1 => "EVENT_BEAT_MANSION_3_TRAINER_1",
            EventFlag::EVENT_BEAT_MANSION_4_TRAINER_0 => "EVENT_BEAT_MANSION_4_TRAINER_0",
            EventFlag::EVENT_BEAT_MANSION_4_TRAINER_1 => "EVENT_BEAT_MANSION_4_TRAINER_1",
            EventFlag::EVENT_GOT_HM03 => "EVENT_GOT_HM03",
            EventFlag::EVENT_BEAT_MEWTWO => "EVENT_BEAT_MEWTWO",
            EventFlag::EVENT_BEAT_LORELEIS_ROOM_TRAINER_0 => "EVENT_BEAT_LORELEIS_ROOM_TRAINER_0",
            EventFlag::EVENT_AUTOWALKED_INTO_LORELEIS_ROOM => "EVENT_AUTOWALKED_INTO_LORELEIS_ROOM",
            EventFlag::EVENT_BEAT_BRUNOS_ROOM_TRAINER_0 => "EVENT_BEAT_BRUNOS_ROOM_TRAINER_0",
            EventFlag::EVENT_AUTOWALKED_INTO_BRUNOS_ROOM => "EVENT_AUTOWALKED_INTO_BRUNOS_ROOM",
            EventFlag::EVENT_BEAT_AGATHAS_ROOM_TRAINER_0 => "EVENT_BEAT_AGATHAS_ROOM_TRAINER_0",
            EventFlag::EVENT_AUTOWALKED_INTO_AGATHAS_ROOM => "EVENT_AUTOWALKED_INTO_AGATHAS_ROOM",
            EventFlag::EVENT_BEAT_LANCES_ROOM_TRAINER_0 => "EVENT_BEAT_LANCES_ROOM_TRAINER_0",
            EventFlag::EVENT_BEAT_LANCE => "EVENT_BEAT_LANCE",
            EventFlag::EVENT_LANCES_ROOM_LOCK_DOOR => "EVENT_LANCES_ROOM_LOCK_DOOR",
            EventFlag::EVENT_BEAT_CHAMPION_RIVAL => "EVENT_BEAT_CHAMPION_RIVAL",
            EventFlag::EVENT_BEAT_VICTORY_ROAD_1_TRAINER_0 => "EVENT_BEAT_VICTORY_ROAD_1_TRAINER_0",
            EventFlag::EVENT_BEAT_VICTORY_ROAD_1_TRAINER_1 => "EVENT_BEAT_VICTORY_ROAD_1_TRAINER_1",
            EventFlag::EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH => {
                "EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH"
            }
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_0 => "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_0",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_1 => "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_1",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_2 => "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_2",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_3 => "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_3",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_4 => "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_4",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_5 => "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_5",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_6 => "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_6",
            EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_7 => "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_7",
            EventFlag::EVENT_SEAFOAM2_BOULDER1_DOWN_HOLE => "EVENT_SEAFOAM2_BOULDER1_DOWN_HOLE",
            EventFlag::EVENT_SEAFOAM2_BOULDER2_DOWN_HOLE => "EVENT_SEAFOAM2_BOULDER2_DOWN_HOLE",
            EventFlag::EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE => "EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE",
            EventFlag::EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE => "EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE",
            EventFlag::EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE => "EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE",
            EventFlag::EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE => "EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE",
            EventFlag::EVENT_BEAT_ARTICUNO => "EVENT_BEAT_ARTICUNO",
        }
    }

    /// Look up an event flag by its string name.
    pub fn from_name(name: &str) -> Option<EventFlag> {
        match name {
            "EVENT_FOLLOWED_OAK_INTO_LAB" => Some(EventFlag::EVENT_FOLLOWED_OAK_INTO_LAB),
            "EVENT_HALL_OF_FAME_DEX_RATING" => Some(EventFlag::EVENT_HALL_OF_FAME_DEX_RATING),
            "EVENT_PALLET_AFTER_GETTING_POKEBALLS" => {
                Some(EventFlag::EVENT_PALLET_AFTER_GETTING_POKEBALLS)
            }
            "EVENT_GOT_TOWN_MAP" => Some(EventFlag::EVENT_GOT_TOWN_MAP),
            "EVENT_ENTERED_BLUES_HOUSE" => Some(EventFlag::EVENT_ENTERED_BLUES_HOUSE),
            "EVENT_DAISY_WALKING" => Some(EventFlag::EVENT_DAISY_WALKING),
            "EVENT_FOLLOWED_OAK_INTO_LAB_2" => Some(EventFlag::EVENT_FOLLOWED_OAK_INTO_LAB_2),
            "EVENT_OAK_ASKED_TO_CHOOSE_MON" => Some(EventFlag::EVENT_OAK_ASKED_TO_CHOOSE_MON),
            "EVENT_GOT_STARTER" => Some(EventFlag::EVENT_GOT_STARTER),
            "EVENT_BATTLED_RIVAL_IN_OAKS_LAB" => Some(EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB),
            "EVENT_GOT_POKEBALLS_FROM_OAK" => Some(EventFlag::EVENT_GOT_POKEBALLS_FROM_OAK),
            "EVENT_GOT_POKEDEX" => Some(EventFlag::EVENT_GOT_POKEDEX),
            "EVENT_PALLET_AFTER_GETTING_POKEBALLS_2" => {
                Some(EventFlag::EVENT_PALLET_AFTER_GETTING_POKEBALLS_2)
            }
            "EVENT_OAK_APPEARED_IN_PALLET" => Some(EventFlag::EVENT_OAK_APPEARED_IN_PALLET),
            "EVENT_VIRIDIAN_GYM_OPEN" => Some(EventFlag::EVENT_VIRIDIAN_GYM_OPEN),
            "EVENT_GOT_TM42" => Some(EventFlag::EVENT_GOT_TM42),
            "EVENT_OAK_GOT_PARCEL" => Some(EventFlag::EVENT_OAK_GOT_PARCEL),
            "EVENT_GOT_OAKS_PARCEL" => Some(EventFlag::EVENT_GOT_OAKS_PARCEL),
            "EVENT_GOT_TM27" => Some(EventFlag::EVENT_GOT_TM27),
            "EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI" => Some(EventFlag::EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI),
            "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_0)
            }
            "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_1)
            }
            "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_2)
            }
            "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_3" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_3)
            }
            "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_4" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_4)
            }
            "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_5" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_5)
            }
            "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_6" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_6)
            }
            "EVENT_BEAT_VIRIDIAN_GYM_TRAINER_7" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_7)
            }
            "EVENT_BOUGHT_MUSEUM_TICKET" => Some(EventFlag::EVENT_BOUGHT_MUSEUM_TICKET),
            "EVENT_GOT_OLD_AMBER" => Some(EventFlag::EVENT_GOT_OLD_AMBER),
            "EVENT_BEAT_PEWTER_GYM_TRAINER_0" => Some(EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0),
            "EVENT_GOT_TM34" => Some(EventFlag::EVENT_GOT_TM34),
            "EVENT_BEAT_BROCK" => Some(EventFlag::EVENT_BEAT_BROCK),
            "EVENT_BEAT_CERULEAN_RIVAL" => Some(EventFlag::EVENT_BEAT_CERULEAN_RIVAL),
            "EVENT_BEAT_CERULEAN_ROCKET_THIEF" => Some(EventFlag::EVENT_BEAT_CERULEAN_ROCKET_THIEF),
            "EVENT_BEAT_CERULEAN_GYM_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_CERULEAN_GYM_TRAINER_0)
            }
            "EVENT_BEAT_CERULEAN_GYM_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_CERULEAN_GYM_TRAINER_1)
            }
            "EVENT_GOT_TM11" => Some(EventFlag::EVENT_GOT_TM11),
            "EVENT_BEAT_MISTY" => Some(EventFlag::EVENT_BEAT_MISTY),
            "EVENT_GOT_BICYCLE" => Some(EventFlag::EVENT_GOT_BICYCLE),
            "EVENT_POKEMON_TOWER_RIVAL_ON_LEFT" => {
                Some(EventFlag::EVENT_POKEMON_TOWER_RIVAL_ON_LEFT)
            }
            "EVENT_BEAT_POKEMON_TOWER_RIVAL" => Some(EventFlag::EVENT_BEAT_POKEMON_TOWER_RIVAL),
            "EVENT_BEAT_POKEMONTOWER_3_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_0)
            }
            "EVENT_BEAT_POKEMONTOWER_3_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_1)
            }
            "EVENT_BEAT_POKEMONTOWER_3_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_2)
            }
            "EVENT_BEAT_POKEMONTOWER_4_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_0)
            }
            "EVENT_BEAT_POKEMONTOWER_4_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_1)
            }
            "EVENT_BEAT_POKEMONTOWER_4_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_2)
            }
            "EVENT_BEAT_POKEMONTOWER_5_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_0)
            }
            "EVENT_BEAT_POKEMONTOWER_5_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_1)
            }
            "EVENT_BEAT_POKEMONTOWER_5_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_2)
            }
            "EVENT_BEAT_POKEMONTOWER_5_TRAINER_3" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_3)
            }
            "EVENT_IN_PURIFIED_ZONE" => Some(EventFlag::EVENT_IN_PURIFIED_ZONE),
            "EVENT_BEAT_POKEMONTOWER_6_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_0)
            }
            "EVENT_BEAT_POKEMONTOWER_6_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_1)
            }
            "EVENT_BEAT_POKEMONTOWER_6_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_2)
            }
            "EVENT_BEAT_GHOST_MAROWAK" => Some(EventFlag::EVENT_BEAT_GHOST_MAROWAK),
            "EVENT_BEAT_POKEMONTOWER_7_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_0)
            }
            "EVENT_BEAT_POKEMONTOWER_7_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_1)
            }
            "EVENT_BEAT_POKEMONTOWER_7_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_2)
            }
            "EVENT_RESCUED_MR_FUJI_2" => Some(EventFlag::EVENT_RESCUED_MR_FUJI_2),
            "EVENT_GOT_POKE_FLUTE" => Some(EventFlag::EVENT_GOT_POKE_FLUTE),
            "EVENT_GOT_BIKE_VOUCHER" => Some(EventFlag::EVENT_GOT_BIKE_VOUCHER),
            "EVENT_SEEL_FAN_BOAST" => Some(EventFlag::EVENT_SEEL_FAN_BOAST),
            "EVENT_PIKACHU_FAN_BOAST" => Some(EventFlag::EVENT_PIKACHU_FAN_BOAST),
            "EVENT_2ND_LOCK_OPENED" => Some(EventFlag::EVENT_2ND_LOCK_OPENED),
            "EVENT_1ST_LOCK_OPENED" => Some(EventFlag::EVENT_1ST_LOCK_OPENED),
            "EVENT_BEAT_VERMILION_GYM_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_0)
            }
            "EVENT_BEAT_VERMILION_GYM_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_1)
            }
            "EVENT_BEAT_VERMILION_GYM_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_2)
            }
            "EVENT_GOT_TM24" => Some(EventFlag::EVENT_GOT_TM24),
            "EVENT_BEAT_LT_SURGE" => Some(EventFlag::EVENT_BEAT_LT_SURGE),
            "EVENT_GOT_TM41" => Some(EventFlag::EVENT_GOT_TM41),
            "EVENT_GOT_TM13" => Some(EventFlag::EVENT_GOT_TM13),
            "EVENT_GOT_TM48" => Some(EventFlag::EVENT_GOT_TM48),
            "EVENT_GOT_TM49" => Some(EventFlag::EVENT_GOT_TM49),
            "EVENT_GOT_TM18" => Some(EventFlag::EVENT_GOT_TM18),
            "EVENT_GOT_TM21" => Some(EventFlag::EVENT_GOT_TM21),
            "EVENT_BEAT_ERIKA" => Some(EventFlag::EVENT_BEAT_ERIKA),
            "EVENT_BEAT_CELADON_GYM_TRAINER_0" => Some(EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_0),
            "EVENT_BEAT_CELADON_GYM_TRAINER_1" => Some(EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_1),
            "EVENT_BEAT_CELADON_GYM_TRAINER_2" => Some(EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_2),
            "EVENT_BEAT_CELADON_GYM_TRAINER_3" => Some(EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_3),
            "EVENT_BEAT_CELADON_GYM_TRAINER_4" => Some(EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_4),
            "EVENT_BEAT_CELADON_GYM_TRAINER_5" => Some(EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_5),
            "EVENT_BEAT_CELADON_GYM_TRAINER_6" => Some(EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_6),
            "EVENT_1B8" => Some(EventFlag::EVENT_1B8),
            "EVENT_FOUND_ROCKET_HIDEOUT" => Some(EventFlag::EVENT_FOUND_ROCKET_HIDEOUT),
            "EVENT_GOT_10_COINS" => Some(EventFlag::EVENT_GOT_10_COINS),
            "EVENT_GOT_20_COINS" => Some(EventFlag::EVENT_GOT_20_COINS),
            "EVENT_GOT_20_COINS_2" => Some(EventFlag::EVENT_GOT_20_COINS_2),
            "EVENT_1BF" => Some(EventFlag::EVENT_1BF),
            "EVENT_GOT_COIN_CASE" => Some(EventFlag::EVENT_GOT_COIN_CASE),
            "EVENT_GOT_HM04" => Some(EventFlag::EVENT_GOT_HM04),
            "EVENT_GAVE_GOLD_TEETH" => Some(EventFlag::EVENT_GAVE_GOLD_TEETH),
            "EVENT_SAFARI_GAME_OVER" => Some(EventFlag::EVENT_SAFARI_GAME_OVER),
            "EVENT_IN_SAFARI_ZONE" => Some(EventFlag::EVENT_IN_SAFARI_ZONE),
            "EVENT_GOT_TM06" => Some(EventFlag::EVENT_GOT_TM06),
            "EVENT_BEAT_KOGA" => Some(EventFlag::EVENT_BEAT_KOGA),
            "EVENT_BEAT_FUCHSIA_GYM_TRAINER_0" => Some(EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_0),
            "EVENT_BEAT_FUCHSIA_GYM_TRAINER_1" => Some(EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_1),
            "EVENT_BEAT_FUCHSIA_GYM_TRAINER_2" => Some(EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_2),
            "EVENT_BEAT_FUCHSIA_GYM_TRAINER_3" => Some(EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_3),
            "EVENT_BEAT_FUCHSIA_GYM_TRAINER_4" => Some(EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_4),
            "EVENT_BEAT_FUCHSIA_GYM_TRAINER_5" => Some(EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_5),
            "EVENT_MANSION_SWITCH_ON" => Some(EventFlag::EVENT_MANSION_SWITCH_ON),
            "EVENT_BEAT_MANSION_1_TRAINER_0" => Some(EventFlag::EVENT_BEAT_MANSION_1_TRAINER_0),
            "EVENT_GOT_TM38" => Some(EventFlag::EVENT_GOT_TM38),
            "EVENT_BEAT_BLAINE" => Some(EventFlag::EVENT_BEAT_BLAINE),
            "EVENT_BEAT_CINNABAR_GYM_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_0)
            }
            "EVENT_BEAT_CINNABAR_GYM_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_1)
            }
            "EVENT_BEAT_CINNABAR_GYM_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_2)
            }
            "EVENT_BEAT_CINNABAR_GYM_TRAINER_3" => {
                Some(EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_3)
            }
            "EVENT_BEAT_CINNABAR_GYM_TRAINER_4" => {
                Some(EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_4)
            }
            "EVENT_BEAT_CINNABAR_GYM_TRAINER_5" => {
                Some(EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_5)
            }
            "EVENT_BEAT_CINNABAR_GYM_TRAINER_6" => {
                Some(EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_6)
            }
            "EVENT_2A7" => Some(EventFlag::EVENT_2A7),
            "EVENT_CINNABAR_GYM_GATE0_UNLOCKED" => {
                Some(EventFlag::EVENT_CINNABAR_GYM_GATE0_UNLOCKED)
            }
            "EVENT_CINNABAR_GYM_GATE1_UNLOCKED" => {
                Some(EventFlag::EVENT_CINNABAR_GYM_GATE1_UNLOCKED)
            }
            "EVENT_CINNABAR_GYM_GATE2_UNLOCKED" => {
                Some(EventFlag::EVENT_CINNABAR_GYM_GATE2_UNLOCKED)
            }
            "EVENT_CINNABAR_GYM_GATE3_UNLOCKED" => {
                Some(EventFlag::EVENT_CINNABAR_GYM_GATE3_UNLOCKED)
            }
            "EVENT_CINNABAR_GYM_GATE4_UNLOCKED" => {
                Some(EventFlag::EVENT_CINNABAR_GYM_GATE4_UNLOCKED)
            }
            "EVENT_CINNABAR_GYM_GATE5_UNLOCKED" => {
                Some(EventFlag::EVENT_CINNABAR_GYM_GATE5_UNLOCKED)
            }
            "EVENT_CINNABAR_GYM_GATE6_UNLOCKED" => {
                Some(EventFlag::EVENT_CINNABAR_GYM_GATE6_UNLOCKED)
            }
            "EVENT_GOT_TM35" => Some(EventFlag::EVENT_GOT_TM35),
            "EVENT_GAVE_FOSSIL_TO_LAB" => Some(EventFlag::EVENT_GAVE_FOSSIL_TO_LAB),
            "EVENT_LAB_STILL_REVIVING_FOSSIL" => Some(EventFlag::EVENT_LAB_STILL_REVIVING_FOSSIL),
            "EVENT_LAB_HANDING_OVER_FOSSIL_MON" => {
                Some(EventFlag::EVENT_LAB_HANDING_OVER_FOSSIL_MON)
            }
            "EVENT_GOT_TM31" => Some(EventFlag::EVENT_GOT_TM31),
            "EVENT_DEFEATED_FIGHTING_DOJO" => Some(EventFlag::EVENT_DEFEATED_FIGHTING_DOJO),
            "EVENT_BEAT_KARATE_MASTER" => Some(EventFlag::EVENT_BEAT_KARATE_MASTER),
            "EVENT_BEAT_FIGHTING_DOJO_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_0)
            }
            "EVENT_BEAT_FIGHTING_DOJO_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_1)
            }
            "EVENT_BEAT_FIGHTING_DOJO_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_2)
            }
            "EVENT_BEAT_FIGHTING_DOJO_TRAINER_3" => {
                Some(EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_3)
            }
            "EVENT_GOT_HITMONLEE" => Some(EventFlag::EVENT_GOT_HITMONLEE),
            "EVENT_GOT_HITMONCHAN" => Some(EventFlag::EVENT_GOT_HITMONCHAN),
            "EVENT_GOT_TM46" => Some(EventFlag::EVENT_GOT_TM46),
            "EVENT_BEAT_SABRINA" => Some(EventFlag::EVENT_BEAT_SABRINA),
            "EVENT_BEAT_SAFFRON_GYM_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_0),
            "EVENT_BEAT_SAFFRON_GYM_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_1),
            "EVENT_BEAT_SAFFRON_GYM_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_2),
            "EVENT_BEAT_SAFFRON_GYM_TRAINER_3" => Some(EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_3),
            "EVENT_BEAT_SAFFRON_GYM_TRAINER_4" => Some(EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_4),
            "EVENT_BEAT_SAFFRON_GYM_TRAINER_5" => Some(EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_5),
            "EVENT_BEAT_SAFFRON_GYM_TRAINER_6" => Some(EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_6),
            "EVENT_SILPH_CO_RECEPTIONIST_AT_DESK" => {
                Some(EventFlag::EVENT_SILPH_CO_RECEPTIONIST_AT_DESK)
            }
            "EVENT_GOT_TM29" => Some(EventFlag::EVENT_GOT_TM29),
            "EVENT_GOT_POTION_SAMPLE" => Some(EventFlag::EVENT_GOT_POTION_SAMPLE),
            "EVENT_GOT_HM05" => Some(EventFlag::EVENT_GOT_HM05),
            "EVENT_BEAT_ROUTE_3_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_0),
            "EVENT_BEAT_ROUTE_3_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_1),
            "EVENT_BEAT_ROUTE_3_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_2),
            "EVENT_BEAT_ROUTE_3_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_3),
            "EVENT_BEAT_ROUTE_3_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_4),
            "EVENT_BEAT_ROUTE_3_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_5),
            "EVENT_BEAT_ROUTE_3_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_6),
            "EVENT_BEAT_ROUTE_3_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_7),
            "EVENT_BEAT_ROUTE_4_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_4_TRAINER_0),
            "EVENT_BOUGHT_MAGIKARP" => Some(EventFlag::EVENT_BOUGHT_MAGIKARP),
            "EVENT_BEAT_ROUTE_6_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_0),
            "EVENT_BEAT_ROUTE_6_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_1),
            "EVENT_BEAT_ROUTE_6_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_2),
            "EVENT_BEAT_ROUTE_6_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_3),
            "EVENT_BEAT_ROUTE_6_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_4),
            "EVENT_BEAT_ROUTE_6_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_5),
            "EVENT_BEAT_ROUTE_8_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_0),
            "EVENT_BEAT_ROUTE_8_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_1),
            "EVENT_BEAT_ROUTE_8_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_2),
            "EVENT_BEAT_ROUTE_8_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_3),
            "EVENT_BEAT_ROUTE_8_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_4),
            "EVENT_BEAT_ROUTE_8_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_5),
            "EVENT_BEAT_ROUTE_8_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_6),
            "EVENT_BEAT_ROUTE_8_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_7),
            "EVENT_BEAT_ROUTE_8_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_8),
            "EVENT_BEAT_ROUTE_9_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_0),
            "EVENT_BEAT_ROUTE_9_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_1),
            "EVENT_BEAT_ROUTE_9_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_2),
            "EVENT_BEAT_ROUTE_9_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_3),
            "EVENT_BEAT_ROUTE_9_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_4),
            "EVENT_BEAT_ROUTE_9_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_5),
            "EVENT_BEAT_ROUTE_9_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_6),
            "EVENT_BEAT_ROUTE_9_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_7),
            "EVENT_BEAT_ROUTE_9_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_8),
            "EVENT_BEAT_ROUTE_10_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_0),
            "EVENT_BEAT_ROUTE_10_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_1),
            "EVENT_BEAT_ROUTE_10_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_2),
            "EVENT_BEAT_ROUTE_10_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_3),
            "EVENT_BEAT_ROUTE_10_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_4),
            "EVENT_BEAT_ROUTE_10_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_5),
            "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_0)
            }
            "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_1)
            }
            "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_2)
            }
            "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_3" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_3)
            }
            "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_4" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_4)
            }
            "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_5" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_5)
            }
            "EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_6" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_6)
            }
            "EVENT_BEAT_POWER_PLANT_VOLTORB_0" => Some(EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_0),
            "EVENT_BEAT_POWER_PLANT_VOLTORB_1" => Some(EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_1),
            "EVENT_BEAT_POWER_PLANT_VOLTORB_2" => Some(EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_2),
            "EVENT_BEAT_POWER_PLANT_VOLTORB_3" => Some(EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_3),
            "EVENT_BEAT_POWER_PLANT_VOLTORB_4" => Some(EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_4),
            "EVENT_BEAT_POWER_PLANT_VOLTORB_5" => Some(EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_5),
            "EVENT_BEAT_POWER_PLANT_VOLTORB_6" => Some(EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_6),
            "EVENT_BEAT_POWER_PLANT_VOLTORB_7" => Some(EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_7),
            "EVENT_BEAT_ZAPDOS" => Some(EventFlag::EVENT_BEAT_ZAPDOS),
            "EVENT_BEAT_ROUTE_11_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_0),
            "EVENT_BEAT_ROUTE_11_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_1),
            "EVENT_BEAT_ROUTE_11_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_2),
            "EVENT_BEAT_ROUTE_11_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_3),
            "EVENT_BEAT_ROUTE_11_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_4),
            "EVENT_BEAT_ROUTE_11_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_5),
            "EVENT_BEAT_ROUTE_11_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_6),
            "EVENT_BEAT_ROUTE_11_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_7),
            "EVENT_BEAT_ROUTE_11_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_8),
            "EVENT_BEAT_ROUTE_11_TRAINER_9" => Some(EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_9),
            "EVENT_GOT_ITEMFINDER" => Some(EventFlag::EVENT_GOT_ITEMFINDER),
            "EVENT_GOT_TM39" => Some(EventFlag::EVENT_GOT_TM39),
            "EVENT_BEAT_ROUTE_12_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_0),
            "EVENT_BEAT_ROUTE_12_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_1),
            "EVENT_BEAT_ROUTE_12_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_2),
            "EVENT_BEAT_ROUTE_12_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_3),
            "EVENT_BEAT_ROUTE_12_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_4),
            "EVENT_BEAT_ROUTE_12_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_5),
            "EVENT_BEAT_ROUTE_12_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_6),
            "EVENT_FIGHT_ROUTE12_SNORLAX" => Some(EventFlag::EVENT_FIGHT_ROUTE12_SNORLAX),
            "EVENT_BEAT_ROUTE12_SNORLAX" => Some(EventFlag::EVENT_BEAT_ROUTE12_SNORLAX),
            "EVENT_BEAT_ROUTE_13_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_0),
            "EVENT_BEAT_ROUTE_13_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_1),
            "EVENT_BEAT_ROUTE_13_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_2),
            "EVENT_BEAT_ROUTE_13_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_3),
            "EVENT_BEAT_ROUTE_13_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_4),
            "EVENT_BEAT_ROUTE_13_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_5),
            "EVENT_BEAT_ROUTE_13_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_6),
            "EVENT_BEAT_ROUTE_13_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_7),
            "EVENT_BEAT_ROUTE_13_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_8),
            "EVENT_BEAT_ROUTE_13_TRAINER_9" => Some(EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_9),
            "EVENT_BEAT_ROUTE_14_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_0),
            "EVENT_BEAT_ROUTE_14_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_1),
            "EVENT_BEAT_ROUTE_14_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_2),
            "EVENT_BEAT_ROUTE_14_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_3),
            "EVENT_BEAT_ROUTE_14_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_4),
            "EVENT_BEAT_ROUTE_14_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_5),
            "EVENT_BEAT_ROUTE_14_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_6),
            "EVENT_BEAT_ROUTE_14_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_7),
            "EVENT_BEAT_ROUTE_14_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_8),
            "EVENT_BEAT_ROUTE_14_TRAINER_9" => Some(EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_9),
            "EVENT_GOT_EXP_ALL" => Some(EventFlag::EVENT_GOT_EXP_ALL),
            "EVENT_BEAT_ROUTE_15_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_0),
            "EVENT_BEAT_ROUTE_15_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_1),
            "EVENT_BEAT_ROUTE_15_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_2),
            "EVENT_BEAT_ROUTE_15_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_3),
            "EVENT_BEAT_ROUTE_15_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_4),
            "EVENT_BEAT_ROUTE_15_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_5),
            "EVENT_BEAT_ROUTE_15_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_6),
            "EVENT_BEAT_ROUTE_15_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_7),
            "EVENT_BEAT_ROUTE_15_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_8),
            "EVENT_BEAT_ROUTE_15_TRAINER_9" => Some(EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_9),
            "EVENT_BEAT_ROUTE_16_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_0),
            "EVENT_BEAT_ROUTE_16_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_1),
            "EVENT_BEAT_ROUTE_16_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_2),
            "EVENT_BEAT_ROUTE_16_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_3),
            "EVENT_BEAT_ROUTE_16_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_4),
            "EVENT_BEAT_ROUTE_16_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_5),
            "EVENT_FIGHT_ROUTE16_SNORLAX" => Some(EventFlag::EVENT_FIGHT_ROUTE16_SNORLAX),
            "EVENT_BEAT_ROUTE16_SNORLAX" => Some(EventFlag::EVENT_BEAT_ROUTE16_SNORLAX),
            "EVENT_GOT_HM02" => Some(EventFlag::EVENT_GOT_HM02),
            "EVENT_RESCUED_MR_FUJI" => Some(EventFlag::EVENT_RESCUED_MR_FUJI),
            "EVENT_BEAT_ROUTE_17_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_0),
            "EVENT_BEAT_ROUTE_17_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_1),
            "EVENT_BEAT_ROUTE_17_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_2),
            "EVENT_BEAT_ROUTE_17_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_3),
            "EVENT_BEAT_ROUTE_17_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_4),
            "EVENT_BEAT_ROUTE_17_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_5),
            "EVENT_BEAT_ROUTE_17_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_6),
            "EVENT_BEAT_ROUTE_17_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_7),
            "EVENT_BEAT_ROUTE_17_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_8),
            "EVENT_BEAT_ROUTE_17_TRAINER_9" => Some(EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_9),
            "EVENT_BEAT_ROUTE_18_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_0),
            "EVENT_BEAT_ROUTE_18_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_1),
            "EVENT_BEAT_ROUTE_18_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_2),
            "EVENT_BEAT_ROUTE_19_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_0),
            "EVENT_BEAT_ROUTE_19_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_1),
            "EVENT_BEAT_ROUTE_19_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_2),
            "EVENT_BEAT_ROUTE_19_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_3),
            "EVENT_BEAT_ROUTE_19_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_4),
            "EVENT_BEAT_ROUTE_19_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_5),
            "EVENT_BEAT_ROUTE_19_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_6),
            "EVENT_BEAT_ROUTE_19_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_7),
            "EVENT_BEAT_ROUTE_19_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_8),
            "EVENT_BEAT_ROUTE_19_TRAINER_9" => Some(EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_9),
            "EVENT_IN_SEAFOAM_ISLANDS" => Some(EventFlag::EVENT_IN_SEAFOAM_ISLANDS),
            "EVENT_BEAT_ROUTE_20_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_0),
            "EVENT_BEAT_ROUTE_20_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_1),
            "EVENT_BEAT_ROUTE_20_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_2),
            "EVENT_BEAT_ROUTE_20_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_3),
            "EVENT_BEAT_ROUTE_20_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_4),
            "EVENT_BEAT_ROUTE_20_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_5),
            "EVENT_BEAT_ROUTE_20_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_6),
            "EVENT_BEAT_ROUTE_20_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_7),
            "EVENT_BEAT_ROUTE_20_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_8),
            "EVENT_BEAT_ROUTE_20_TRAINER_9" => Some(EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_9),
            "EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE" => {
                Some(EventFlag::EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE)
            }
            "EVENT_SEAFOAM1_BOULDER2_DOWN_HOLE" => {
                Some(EventFlag::EVENT_SEAFOAM1_BOULDER2_DOWN_HOLE)
            }
            "EVENT_BEAT_ROUTE_21_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_0),
            "EVENT_BEAT_ROUTE_21_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_1),
            "EVENT_BEAT_ROUTE_21_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_2),
            "EVENT_BEAT_ROUTE_21_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_3),
            "EVENT_BEAT_ROUTE_21_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_4),
            "EVENT_BEAT_ROUTE_21_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_5),
            "EVENT_BEAT_ROUTE_21_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_6),
            "EVENT_BEAT_ROUTE_21_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_7),
            "EVENT_BEAT_ROUTE_21_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_8),
            "EVENT_1ST_ROUTE22_RIVAL_BATTLE" => Some(EventFlag::EVENT_1ST_ROUTE22_RIVAL_BATTLE),
            "EVENT_2ND_ROUTE22_RIVAL_BATTLE" => Some(EventFlag::EVENT_2ND_ROUTE22_RIVAL_BATTLE),
            "EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE" => {
                Some(EventFlag::EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE)
            }
            "EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE" => {
                Some(EventFlag::EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE)
            }
            "EVENT_ROUTE22_RIVAL_WANTS_BATTLE" => Some(EventFlag::EVENT_ROUTE22_RIVAL_WANTS_BATTLE),
            "EVENT_PASSED_CASCADEBADGE_CHECK" => Some(EventFlag::EVENT_PASSED_CASCADEBADGE_CHECK),
            "EVENT_PASSED_THUNDERBADGE_CHECK" => Some(EventFlag::EVENT_PASSED_THUNDERBADGE_CHECK),
            "EVENT_PASSED_RAINBOWBADGE_CHECK" => Some(EventFlag::EVENT_PASSED_RAINBOWBADGE_CHECK),
            "EVENT_PASSED_SOULBADGE_CHECK" => Some(EventFlag::EVENT_PASSED_SOULBADGE_CHECK),
            "EVENT_PASSED_MARSHBADGE_CHECK" => Some(EventFlag::EVENT_PASSED_MARSHBADGE_CHECK),
            "EVENT_PASSED_VOLCANOBADGE_CHECK" => Some(EventFlag::EVENT_PASSED_VOLCANOBADGE_CHECK),
            "EVENT_PASSED_EARTHBADGE_CHECK" => Some(EventFlag::EVENT_PASSED_EARTHBADGE_CHECK),
            "EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1" => {
                Some(EventFlag::EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1)
            }
            "EVENT_BEAT_VICTORY_ROAD_2_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_0)
            }
            "EVENT_BEAT_VICTORY_ROAD_2_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_1)
            }
            "EVENT_BEAT_VICTORY_ROAD_2_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_2)
            }
            "EVENT_BEAT_VICTORY_ROAD_2_TRAINER_3" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_3)
            }
            "EVENT_BEAT_VICTORY_ROAD_2_TRAINER_4" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_4)
            }
            "EVENT_BEAT_MOLTRES" => Some(EventFlag::EVENT_BEAT_MOLTRES),
            "EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2" => {
                Some(EventFlag::EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2)
            }
            "EVENT_GOT_NUGGET" => Some(EventFlag::EVENT_GOT_NUGGET),
            "EVENT_BEAT_ROUTE24_ROCKET" => Some(EventFlag::EVENT_BEAT_ROUTE24_ROCKET),
            "EVENT_BEAT_ROUTE_24_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_0),
            "EVENT_BEAT_ROUTE_24_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_1),
            "EVENT_BEAT_ROUTE_24_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_2),
            "EVENT_BEAT_ROUTE_24_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_3),
            "EVENT_BEAT_ROUTE_24_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_4),
            "EVENT_BEAT_ROUTE_24_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_5),
            "EVENT_NUGGET_REWARD_AVAILABLE" => Some(EventFlag::EVENT_NUGGET_REWARD_AVAILABLE),
            "EVENT_MET_BILL" => Some(EventFlag::EVENT_MET_BILL),
            "EVENT_BEAT_ROUTE_25_TRAINER_0" => Some(EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_0),
            "EVENT_BEAT_ROUTE_25_TRAINER_1" => Some(EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_1),
            "EVENT_BEAT_ROUTE_25_TRAINER_2" => Some(EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_2),
            "EVENT_BEAT_ROUTE_25_TRAINER_3" => Some(EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_3),
            "EVENT_BEAT_ROUTE_25_TRAINER_4" => Some(EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_4),
            "EVENT_BEAT_ROUTE_25_TRAINER_5" => Some(EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_5),
            "EVENT_BEAT_ROUTE_25_TRAINER_6" => Some(EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_6),
            "EVENT_BEAT_ROUTE_25_TRAINER_7" => Some(EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_7),
            "EVENT_BEAT_ROUTE_25_TRAINER_8" => Some(EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_8),
            "EVENT_USED_CELL_SEPARATOR_ON_BILL" => {
                Some(EventFlag::EVENT_USED_CELL_SEPARATOR_ON_BILL)
            }
            "EVENT_GOT_SS_TICKET" => Some(EventFlag::EVENT_GOT_SS_TICKET),
            "EVENT_MET_BILL_2" => Some(EventFlag::EVENT_MET_BILL_2),
            "EVENT_BILL_SAID_USE_CELL_SEPARATOR" => {
                Some(EventFlag::EVENT_BILL_SAID_USE_CELL_SEPARATOR)
            }
            "EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING" => {
                Some(EventFlag::EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING)
            }
            "EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0)
            }
            "EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_1)
            }
            "EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2)
            }
            "EVENT_BEAT_MT_MOON_1_TRAINER_0" => Some(EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_0),
            "EVENT_BEAT_MT_MOON_1_TRAINER_1" => Some(EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_1),
            "EVENT_BEAT_MT_MOON_1_TRAINER_2" => Some(EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_2),
            "EVENT_BEAT_MT_MOON_1_TRAINER_3" => Some(EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_3),
            "EVENT_BEAT_MT_MOON_1_TRAINER_4" => Some(EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_4),
            "EVENT_BEAT_MT_MOON_1_TRAINER_5" => Some(EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_5),
            "EVENT_BEAT_MT_MOON_1_TRAINER_6" => Some(EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_6),
            "EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD" => {
                Some(EventFlag::EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD)
            }
            "EVENT_BEAT_MT_MOON_3_TRAINER_0" => Some(EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_0),
            "EVENT_BEAT_MT_MOON_3_TRAINER_1" => Some(EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_1),
            "EVENT_BEAT_MT_MOON_3_TRAINER_2" => Some(EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_2),
            "EVENT_BEAT_MT_MOON_3_TRAINER_3" => Some(EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_3),
            "EVENT_GOT_DOME_FOSSIL" => Some(EventFlag::EVENT_GOT_DOME_FOSSIL),
            "EVENT_GOT_HELIX_FOSSIL" => Some(EventFlag::EVENT_GOT_HELIX_FOSSIL),
            "EVENT_BEAT_SS_ANNE_5_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SS_ANNE_5_TRAINER_0),
            "EVENT_BEAT_SS_ANNE_5_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SS_ANNE_5_TRAINER_1),
            "EVENT_GOT_HM01" => Some(EventFlag::EVENT_GOT_HM01),
            "EVENT_RUBBED_CAPTAINS_BACK" => Some(EventFlag::EVENT_RUBBED_CAPTAINS_BACK),
            "EVENT_SS_ANNE_LEFT" => Some(EventFlag::EVENT_SS_ANNE_LEFT),
            "EVENT_WALKED_PAST_GUARD_AFTER_SS_ANNE_LEFT" => {
                Some(EventFlag::EVENT_WALKED_PAST_GUARD_AFTER_SS_ANNE_LEFT)
            }
            "EVENT_STARTED_WALKING_OUT_OF_DOCK" => {
                Some(EventFlag::EVENT_STARTED_WALKING_OUT_OF_DOCK)
            }
            "EVENT_WALKED_OUT_OF_DOCK" => Some(EventFlag::EVENT_WALKED_OUT_OF_DOCK),
            "EVENT_BEAT_SS_ANNE_8_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_0),
            "EVENT_BEAT_SS_ANNE_8_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_1),
            "EVENT_BEAT_SS_ANNE_8_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_2),
            "EVENT_BEAT_SS_ANNE_8_TRAINER_3" => Some(EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_3),
            "EVENT_BEAT_SS_ANNE_9_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_0),
            "EVENT_BEAT_SS_ANNE_9_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_1),
            "EVENT_BEAT_SS_ANNE_9_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_2),
            "EVENT_BEAT_SS_ANNE_9_TRAINER_3" => Some(EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_3),
            "EVENT_BEAT_SS_ANNE_10_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_0),
            "EVENT_BEAT_SS_ANNE_10_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_1),
            "EVENT_BEAT_SS_ANNE_10_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_2),
            "EVENT_BEAT_SS_ANNE_10_TRAINER_3" => Some(EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_3),
            "EVENT_BEAT_SS_ANNE_10_TRAINER_4" => Some(EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_4),
            "EVENT_BEAT_SS_ANNE_10_TRAINER_5" => Some(EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_5),
            "EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH1" => {
                Some(EventFlag::EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH1)
            }
            "EVENT_BEAT_VICTORY_ROAD_3_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_0)
            }
            "EVENT_BEAT_VICTORY_ROAD_3_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_1)
            }
            "EVENT_BEAT_VICTORY_ROAD_3_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_2)
            }
            "EVENT_BEAT_VICTORY_ROAD_3_TRAINER_3" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_3)
            }
            "EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2" => {
                Some(EventFlag::EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2)
            }
            "EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_0)
            }
            "EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_1)
            }
            "EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_2)
            }
            "EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_3" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_3)
            }
            "EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_4" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_4)
            }
            "EVENT_ENTERED_ROCKET_HIDEOUT" => Some(EventFlag::EVENT_ENTERED_ROCKET_HIDEOUT),
            "EVENT_67F" => Some(EventFlag::EVENT_67F),
            "EVENT_BEAT_ROCKET_HIDEOUT_2_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_2_TRAINER_0)
            }
            "EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_0)
            }
            "EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_1)
            }
            "EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0)
            }
            "EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_1)
            }
            "EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2)
            }
            "EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED" => {
                Some(EventFlag::EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED)
            }
            "EVENT_ROCKET_DROPPED_LIFT_KEY" => Some(EventFlag::EVENT_ROCKET_DROPPED_LIFT_KEY),
            "EVENT_BEAT_ROCKET_HIDEOUT_GIOVANNI" => {
                Some(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_GIOVANNI)
            }
            "EVENT_BEAT_SILPH_CO_2F_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_0),
            "EVENT_BEAT_SILPH_CO_2F_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_1),
            "EVENT_BEAT_SILPH_CO_2F_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_2),
            "EVENT_BEAT_SILPH_CO_2F_TRAINER_3" => Some(EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_3),
            "EVENT_SILPH_CO_2_UNLOCKED_DOOR1" => Some(EventFlag::EVENT_SILPH_CO_2_UNLOCKED_DOOR1),
            "EVENT_SILPH_CO_2_UNLOCKED_DOOR2" => Some(EventFlag::EVENT_SILPH_CO_2_UNLOCKED_DOOR2),
            "EVENT_GOT_TM36" => Some(EventFlag::EVENT_GOT_TM36),
            "EVENT_BEAT_SILPH_CO_3F_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SILPH_CO_3F_TRAINER_0),
            "EVENT_BEAT_SILPH_CO_3F_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SILPH_CO_3F_TRAINER_1),
            "EVENT_SILPH_CO_3_UNLOCKED_DOOR1" => Some(EventFlag::EVENT_SILPH_CO_3_UNLOCKED_DOOR1),
            "EVENT_SILPH_CO_3_UNLOCKED_DOOR2" => Some(EventFlag::EVENT_SILPH_CO_3_UNLOCKED_DOOR2),
            "EVENT_BEAT_SILPH_CO_4F_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_0),
            "EVENT_BEAT_SILPH_CO_4F_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_1),
            "EVENT_BEAT_SILPH_CO_4F_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_2),
            "EVENT_SILPH_CO_4_UNLOCKED_DOOR1" => Some(EventFlag::EVENT_SILPH_CO_4_UNLOCKED_DOOR1),
            "EVENT_SILPH_CO_4_UNLOCKED_DOOR2" => Some(EventFlag::EVENT_SILPH_CO_4_UNLOCKED_DOOR2),
            "EVENT_BEAT_SILPH_CO_5F_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_0),
            "EVENT_BEAT_SILPH_CO_5F_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_1),
            "EVENT_BEAT_SILPH_CO_5F_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_2),
            "EVENT_BEAT_SILPH_CO_5F_TRAINER_3" => Some(EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_3),
            "EVENT_SILPH_CO_5_UNLOCKED_DOOR1" => Some(EventFlag::EVENT_SILPH_CO_5_UNLOCKED_DOOR1),
            "EVENT_SILPH_CO_5_UNLOCKED_DOOR2" => Some(EventFlag::EVENT_SILPH_CO_5_UNLOCKED_DOOR2),
            "EVENT_SILPH_CO_5_UNLOCKED_DOOR3" => Some(EventFlag::EVENT_SILPH_CO_5_UNLOCKED_DOOR3),
            "EVENT_BEAT_SILPH_CO_6F_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_0),
            "EVENT_BEAT_SILPH_CO_6F_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_1),
            "EVENT_BEAT_SILPH_CO_6F_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_2),
            "EVENT_SILPH_CO_6_UNLOCKED_DOOR" => Some(EventFlag::EVENT_SILPH_CO_6_UNLOCKED_DOOR),
            "EVENT_BEAT_SILPH_CO_RIVAL" => Some(EventFlag::EVENT_BEAT_SILPH_CO_RIVAL),
            "EVENT_BEAT_SILPH_CO_7F_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_0),
            "EVENT_BEAT_SILPH_CO_7F_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_1),
            "EVENT_BEAT_SILPH_CO_7F_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_2),
            "EVENT_BEAT_SILPH_CO_7F_TRAINER_3" => Some(EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_3),
            "EVENT_SILPH_CO_7_UNLOCKED_DOOR1" => Some(EventFlag::EVENT_SILPH_CO_7_UNLOCKED_DOOR1),
            "EVENT_SILPH_CO_7_UNLOCKED_DOOR2" => Some(EventFlag::EVENT_SILPH_CO_7_UNLOCKED_DOOR2),
            "EVENT_SILPH_CO_7_UNLOCKED_DOOR3" => Some(EventFlag::EVENT_SILPH_CO_7_UNLOCKED_DOOR3),
            "EVENT_BEAT_SILPH_CO_8F_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_0),
            "EVENT_BEAT_SILPH_CO_8F_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_1),
            "EVENT_BEAT_SILPH_CO_8F_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_2),
            "EVENT_SILPH_CO_8_UNLOCKED_DOOR" => Some(EventFlag::EVENT_SILPH_CO_8_UNLOCKED_DOOR),
            "EVENT_BEAT_SILPH_CO_9F_TRAINER_0" => Some(EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_0),
            "EVENT_BEAT_SILPH_CO_9F_TRAINER_1" => Some(EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_1),
            "EVENT_BEAT_SILPH_CO_9F_TRAINER_2" => Some(EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_2),
            "EVENT_SILPH_CO_9_UNLOCKED_DOOR1" => Some(EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR1),
            "EVENT_SILPH_CO_9_UNLOCKED_DOOR2" => Some(EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR2),
            "EVENT_SILPH_CO_9_UNLOCKED_DOOR3" => Some(EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR3),
            "EVENT_SILPH_CO_9_UNLOCKED_DOOR4" => Some(EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR4),
            "EVENT_BEAT_SILPH_CO_10F_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_SILPH_CO_10F_TRAINER_0)
            }
            "EVENT_BEAT_SILPH_CO_10F_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_SILPH_CO_10F_TRAINER_1)
            }
            "EVENT_SILPH_CO_10_UNLOCKED_DOOR" => Some(EventFlag::EVENT_SILPH_CO_10_UNLOCKED_DOOR),
            "EVENT_BEAT_SILPH_CO_11F_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_SILPH_CO_11F_TRAINER_0)
            }
            "EVENT_BEAT_SILPH_CO_11F_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_SILPH_CO_11F_TRAINER_1)
            }
            "EVENT_SILPH_CO_11_UNLOCKED_DOOR" => Some(EventFlag::EVENT_SILPH_CO_11_UNLOCKED_DOOR),
            "EVENT_GOT_MASTER_BALL" => Some(EventFlag::EVENT_GOT_MASTER_BALL),
            "EVENT_BEAT_SILPH_CO_GIOVANNI" => Some(EventFlag::EVENT_BEAT_SILPH_CO_GIOVANNI),
            "EVENT_BEAT_MANSION_2_TRAINER_0" => Some(EventFlag::EVENT_BEAT_MANSION_2_TRAINER_0),
            "EVENT_BEAT_MANSION_3_TRAINER_0" => Some(EventFlag::EVENT_BEAT_MANSION_3_TRAINER_0),
            "EVENT_BEAT_MANSION_3_TRAINER_1" => Some(EventFlag::EVENT_BEAT_MANSION_3_TRAINER_1),
            "EVENT_BEAT_MANSION_4_TRAINER_0" => Some(EventFlag::EVENT_BEAT_MANSION_4_TRAINER_0),
            "EVENT_BEAT_MANSION_4_TRAINER_1" => Some(EventFlag::EVENT_BEAT_MANSION_4_TRAINER_1),
            "EVENT_GOT_HM03" => Some(EventFlag::EVENT_GOT_HM03),
            "EVENT_BEAT_MEWTWO" => Some(EventFlag::EVENT_BEAT_MEWTWO),
            "EVENT_BEAT_LORELEIS_ROOM_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_LORELEIS_ROOM_TRAINER_0)
            }
            "EVENT_AUTOWALKED_INTO_LORELEIS_ROOM" => {
                Some(EventFlag::EVENT_AUTOWALKED_INTO_LORELEIS_ROOM)
            }
            "EVENT_BEAT_BRUNOS_ROOM_TRAINER_0" => Some(EventFlag::EVENT_BEAT_BRUNOS_ROOM_TRAINER_0),
            "EVENT_AUTOWALKED_INTO_BRUNOS_ROOM" => {
                Some(EventFlag::EVENT_AUTOWALKED_INTO_BRUNOS_ROOM)
            }
            "EVENT_BEAT_AGATHAS_ROOM_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_AGATHAS_ROOM_TRAINER_0)
            }
            "EVENT_AUTOWALKED_INTO_AGATHAS_ROOM" => {
                Some(EventFlag::EVENT_AUTOWALKED_INTO_AGATHAS_ROOM)
            }
            "EVENT_BEAT_LANCES_ROOM_TRAINER_0" => Some(EventFlag::EVENT_BEAT_LANCES_ROOM_TRAINER_0),
            "EVENT_BEAT_LANCE" => Some(EventFlag::EVENT_BEAT_LANCE),
            "EVENT_LANCES_ROOM_LOCK_DOOR" => Some(EventFlag::EVENT_LANCES_ROOM_LOCK_DOOR),
            "EVENT_BEAT_CHAMPION_RIVAL" => Some(EventFlag::EVENT_BEAT_CHAMPION_RIVAL),
            "EVENT_BEAT_VICTORY_ROAD_1_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_1_TRAINER_0)
            }
            "EVENT_BEAT_VICTORY_ROAD_1_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_VICTORY_ROAD_1_TRAINER_1)
            }
            "EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH" => {
                Some(EventFlag::EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH)
            }
            "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_0" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_0)
            }
            "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_1" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_1)
            }
            "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_2" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_2)
            }
            "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_3" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_3)
            }
            "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_4" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_4)
            }
            "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_5" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_5)
            }
            "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_6" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_6)
            }
            "EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_7" => {
                Some(EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_7)
            }
            "EVENT_SEAFOAM2_BOULDER1_DOWN_HOLE" => {
                Some(EventFlag::EVENT_SEAFOAM2_BOULDER1_DOWN_HOLE)
            }
            "EVENT_SEAFOAM2_BOULDER2_DOWN_HOLE" => {
                Some(EventFlag::EVENT_SEAFOAM2_BOULDER2_DOWN_HOLE)
            }
            "EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE" => {
                Some(EventFlag::EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE)
            }
            "EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE" => {
                Some(EventFlag::EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE)
            }
            "EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE" => {
                Some(EventFlag::EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE)
            }
            "EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE" => {
                Some(EventFlag::EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE)
            }
            "EVENT_BEAT_ARTICUNO" => Some(EventFlag::EVENT_BEAT_ARTICUNO),
            _ => None,
        }
    }

    /// Get all event flags as a slice.
    pub const ALL: &[EventFlag] = &[
        EventFlag::EVENT_FOLLOWED_OAK_INTO_LAB,
        EventFlag::EVENT_HALL_OF_FAME_DEX_RATING,
        EventFlag::EVENT_PALLET_AFTER_GETTING_POKEBALLS,
        EventFlag::EVENT_GOT_TOWN_MAP,
        EventFlag::EVENT_ENTERED_BLUES_HOUSE,
        EventFlag::EVENT_DAISY_WALKING,
        EventFlag::EVENT_FOLLOWED_OAK_INTO_LAB_2,
        EventFlag::EVENT_OAK_ASKED_TO_CHOOSE_MON,
        EventFlag::EVENT_GOT_STARTER,
        EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB,
        EventFlag::EVENT_GOT_POKEBALLS_FROM_OAK,
        EventFlag::EVENT_GOT_POKEDEX,
        EventFlag::EVENT_PALLET_AFTER_GETTING_POKEBALLS_2,
        EventFlag::EVENT_OAK_APPEARED_IN_PALLET,
        EventFlag::EVENT_VIRIDIAN_GYM_OPEN,
        EventFlag::EVENT_GOT_TM42,
        EventFlag::EVENT_OAK_GOT_PARCEL,
        EventFlag::EVENT_GOT_OAKS_PARCEL,
        EventFlag::EVENT_GOT_TM27,
        EventFlag::EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI,
        EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_0,
        EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_1,
        EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_2,
        EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_3,
        EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_4,
        EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_5,
        EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_6,
        EventFlag::EVENT_BEAT_VIRIDIAN_GYM_TRAINER_7,
        EventFlag::EVENT_BOUGHT_MUSEUM_TICKET,
        EventFlag::EVENT_GOT_OLD_AMBER,
        EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0,
        EventFlag::EVENT_GOT_TM34,
        EventFlag::EVENT_BEAT_BROCK,
        EventFlag::EVENT_BEAT_CERULEAN_RIVAL,
        EventFlag::EVENT_BEAT_CERULEAN_ROCKET_THIEF,
        EventFlag::EVENT_BEAT_CERULEAN_GYM_TRAINER_0,
        EventFlag::EVENT_BEAT_CERULEAN_GYM_TRAINER_1,
        EventFlag::EVENT_GOT_TM11,
        EventFlag::EVENT_BEAT_MISTY,
        EventFlag::EVENT_GOT_BICYCLE,
        EventFlag::EVENT_POKEMON_TOWER_RIVAL_ON_LEFT,
        EventFlag::EVENT_BEAT_POKEMON_TOWER_RIVAL,
        EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_0,
        EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_1,
        EventFlag::EVENT_BEAT_POKEMONTOWER_3_TRAINER_2,
        EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_0,
        EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_1,
        EventFlag::EVENT_BEAT_POKEMONTOWER_4_TRAINER_2,
        EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_0,
        EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_1,
        EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_2,
        EventFlag::EVENT_BEAT_POKEMONTOWER_5_TRAINER_3,
        EventFlag::EVENT_IN_PURIFIED_ZONE,
        EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_0,
        EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_1,
        EventFlag::EVENT_BEAT_POKEMONTOWER_6_TRAINER_2,
        EventFlag::EVENT_BEAT_GHOST_MAROWAK,
        EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_0,
        EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_1,
        EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_2,
        EventFlag::EVENT_RESCUED_MR_FUJI_2,
        EventFlag::EVENT_GOT_POKE_FLUTE,
        EventFlag::EVENT_GOT_BIKE_VOUCHER,
        EventFlag::EVENT_SEEL_FAN_BOAST,
        EventFlag::EVENT_PIKACHU_FAN_BOAST,
        EventFlag::EVENT_2ND_LOCK_OPENED,
        EventFlag::EVENT_1ST_LOCK_OPENED,
        EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_0,
        EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_1,
        EventFlag::EVENT_BEAT_VERMILION_GYM_TRAINER_2,
        EventFlag::EVENT_GOT_TM24,
        EventFlag::EVENT_BEAT_LT_SURGE,
        EventFlag::EVENT_GOT_TM41,
        EventFlag::EVENT_GOT_TM13,
        EventFlag::EVENT_GOT_TM48,
        EventFlag::EVENT_GOT_TM49,
        EventFlag::EVENT_GOT_TM18,
        EventFlag::EVENT_GOT_TM21,
        EventFlag::EVENT_BEAT_ERIKA,
        EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_0,
        EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_1,
        EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_2,
        EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_3,
        EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_4,
        EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_5,
        EventFlag::EVENT_BEAT_CELADON_GYM_TRAINER_6,
        EventFlag::EVENT_1B8,
        EventFlag::EVENT_FOUND_ROCKET_HIDEOUT,
        EventFlag::EVENT_GOT_10_COINS,
        EventFlag::EVENT_GOT_20_COINS,
        EventFlag::EVENT_GOT_20_COINS_2,
        EventFlag::EVENT_1BF,
        EventFlag::EVENT_GOT_COIN_CASE,
        EventFlag::EVENT_GOT_HM04,
        EventFlag::EVENT_GAVE_GOLD_TEETH,
        EventFlag::EVENT_SAFARI_GAME_OVER,
        EventFlag::EVENT_IN_SAFARI_ZONE,
        EventFlag::EVENT_GOT_TM06,
        EventFlag::EVENT_BEAT_KOGA,
        EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_0,
        EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_1,
        EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_2,
        EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_3,
        EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_4,
        EventFlag::EVENT_BEAT_FUCHSIA_GYM_TRAINER_5,
        EventFlag::EVENT_MANSION_SWITCH_ON,
        EventFlag::EVENT_BEAT_MANSION_1_TRAINER_0,
        EventFlag::EVENT_GOT_TM38,
        EventFlag::EVENT_BEAT_BLAINE,
        EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_0,
        EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_1,
        EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_2,
        EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_3,
        EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_4,
        EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_5,
        EventFlag::EVENT_BEAT_CINNABAR_GYM_TRAINER_6,
        EventFlag::EVENT_2A7,
        EventFlag::EVENT_CINNABAR_GYM_GATE0_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE1_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE2_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE3_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE4_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE5_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE6_UNLOCKED,
        EventFlag::EVENT_GOT_TM35,
        EventFlag::EVENT_GAVE_FOSSIL_TO_LAB,
        EventFlag::EVENT_LAB_STILL_REVIVING_FOSSIL,
        EventFlag::EVENT_LAB_HANDING_OVER_FOSSIL_MON,
        EventFlag::EVENT_GOT_TM31,
        EventFlag::EVENT_DEFEATED_FIGHTING_DOJO,
        EventFlag::EVENT_BEAT_KARATE_MASTER,
        EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_0,
        EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_1,
        EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_2,
        EventFlag::EVENT_BEAT_FIGHTING_DOJO_TRAINER_3,
        EventFlag::EVENT_GOT_HITMONLEE,
        EventFlag::EVENT_GOT_HITMONCHAN,
        EventFlag::EVENT_GOT_TM46,
        EventFlag::EVENT_BEAT_SABRINA,
        EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_0,
        EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_1,
        EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_2,
        EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_3,
        EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_4,
        EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_5,
        EventFlag::EVENT_BEAT_SAFFRON_GYM_TRAINER_6,
        EventFlag::EVENT_SILPH_CO_RECEPTIONIST_AT_DESK,
        EventFlag::EVENT_GOT_TM29,
        EventFlag::EVENT_GOT_POTION_SAMPLE,
        EventFlag::EVENT_GOT_HM05,
        EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_3_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_4_TRAINER_0,
        EventFlag::EVENT_BOUGHT_MAGIKARP,
        EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_6_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_8_TRAINER_8,
        EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_9_TRAINER_8,
        EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_10_TRAINER_5,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_0,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_1,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_2,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_3,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_4,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_5,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_6,
        EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_0,
        EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_1,
        EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_2,
        EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_3,
        EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_4,
        EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_5,
        EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_6,
        EventFlag::EVENT_BEAT_POWER_PLANT_VOLTORB_7,
        EventFlag::EVENT_BEAT_ZAPDOS,
        EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_8,
        EventFlag::EVENT_BEAT_ROUTE_11_TRAINER_9,
        EventFlag::EVENT_GOT_ITEMFINDER,
        EventFlag::EVENT_GOT_TM39,
        EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_12_TRAINER_6,
        EventFlag::EVENT_FIGHT_ROUTE12_SNORLAX,
        EventFlag::EVENT_BEAT_ROUTE12_SNORLAX,
        EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_8,
        EventFlag::EVENT_BEAT_ROUTE_13_TRAINER_9,
        EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_8,
        EventFlag::EVENT_BEAT_ROUTE_14_TRAINER_9,
        EventFlag::EVENT_GOT_EXP_ALL,
        EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_8,
        EventFlag::EVENT_BEAT_ROUTE_15_TRAINER_9,
        EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_16_TRAINER_5,
        EventFlag::EVENT_FIGHT_ROUTE16_SNORLAX,
        EventFlag::EVENT_BEAT_ROUTE16_SNORLAX,
        EventFlag::EVENT_GOT_HM02,
        EventFlag::EVENT_RESCUED_MR_FUJI,
        EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_8,
        EventFlag::EVENT_BEAT_ROUTE_17_TRAINER_9,
        EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_18_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_8,
        EventFlag::EVENT_BEAT_ROUTE_19_TRAINER_9,
        EventFlag::EVENT_IN_SEAFOAM_ISLANDS,
        EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_8,
        EventFlag::EVENT_BEAT_ROUTE_20_TRAINER_9,
        EventFlag::EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE,
        EventFlag::EVENT_SEAFOAM1_BOULDER2_DOWN_HOLE,
        EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_21_TRAINER_8,
        EventFlag::EVENT_1ST_ROUTE22_RIVAL_BATTLE,
        EventFlag::EVENT_2ND_ROUTE22_RIVAL_BATTLE,
        EventFlag::EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE,
        EventFlag::EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE,
        EventFlag::EVENT_ROUTE22_RIVAL_WANTS_BATTLE,
        EventFlag::EVENT_PASSED_CASCADEBADGE_CHECK,
        EventFlag::EVENT_PASSED_THUNDERBADGE_CHECK,
        EventFlag::EVENT_PASSED_RAINBOWBADGE_CHECK,
        EventFlag::EVENT_PASSED_SOULBADGE_CHECK,
        EventFlag::EVENT_PASSED_MARSHBADGE_CHECK,
        EventFlag::EVENT_PASSED_VOLCANOBADGE_CHECK,
        EventFlag::EVENT_PASSED_EARTHBADGE_CHECK,
        EventFlag::EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_0,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_1,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_2,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_3,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_2_TRAINER_4,
        EventFlag::EVENT_BEAT_MOLTRES,
        EventFlag::EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2,
        EventFlag::EVENT_GOT_NUGGET,
        EventFlag::EVENT_BEAT_ROUTE24_ROCKET,
        EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_24_TRAINER_5,
        EventFlag::EVENT_NUGGET_REWARD_AVAILABLE,
        EventFlag::EVENT_MET_BILL,
        EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_0,
        EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_1,
        EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_2,
        EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_3,
        EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_4,
        EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_5,
        EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_6,
        EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_7,
        EventFlag::EVENT_BEAT_ROUTE_25_TRAINER_8,
        EventFlag::EVENT_USED_CELL_SEPARATOR_ON_BILL,
        EventFlag::EVENT_GOT_SS_TICKET,
        EventFlag::EVENT_MET_BILL_2,
        EventFlag::EVENT_BILL_SAID_USE_CELL_SEPARATOR,
        EventFlag::EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING,
        EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0,
        EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_1,
        EventFlag::EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2,
        EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_0,
        EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_1,
        EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_2,
        EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_3,
        EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_4,
        EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_5,
        EventFlag::EVENT_BEAT_MT_MOON_1_TRAINER_6,
        EventFlag::EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD,
        EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_0,
        EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_1,
        EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_2,
        EventFlag::EVENT_BEAT_MT_MOON_3_TRAINER_3,
        EventFlag::EVENT_GOT_DOME_FOSSIL,
        EventFlag::EVENT_GOT_HELIX_FOSSIL,
        EventFlag::EVENT_BEAT_SS_ANNE_5_TRAINER_0,
        EventFlag::EVENT_BEAT_SS_ANNE_5_TRAINER_1,
        EventFlag::EVENT_GOT_HM01,
        EventFlag::EVENT_RUBBED_CAPTAINS_BACK,
        EventFlag::EVENT_SS_ANNE_LEFT,
        EventFlag::EVENT_WALKED_PAST_GUARD_AFTER_SS_ANNE_LEFT,
        EventFlag::EVENT_STARTED_WALKING_OUT_OF_DOCK,
        EventFlag::EVENT_WALKED_OUT_OF_DOCK,
        EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_0,
        EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_1,
        EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_2,
        EventFlag::EVENT_BEAT_SS_ANNE_8_TRAINER_3,
        EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_0,
        EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_1,
        EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_2,
        EventFlag::EVENT_BEAT_SS_ANNE_9_TRAINER_3,
        EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_0,
        EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_1,
        EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_2,
        EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_3,
        EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_4,
        EventFlag::EVENT_BEAT_SS_ANNE_10_TRAINER_5,
        EventFlag::EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH1,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_0,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_1,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_2,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_3_TRAINER_3,
        EventFlag::EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_0,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_1,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_2,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_3,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_4,
        EventFlag::EVENT_ENTERED_ROCKET_HIDEOUT,
        EventFlag::EVENT_67F,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_2_TRAINER_0,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_0,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_3_TRAINER_1,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_1,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2,
        EventFlag::EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED,
        EventFlag::EVENT_ROCKET_DROPPED_LIFT_KEY,
        EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_GIOVANNI,
        EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_0,
        EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_1,
        EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_2,
        EventFlag::EVENT_BEAT_SILPH_CO_2F_TRAINER_3,
        EventFlag::EVENT_SILPH_CO_2_UNLOCKED_DOOR1,
        EventFlag::EVENT_SILPH_CO_2_UNLOCKED_DOOR2,
        EventFlag::EVENT_GOT_TM36,
        EventFlag::EVENT_BEAT_SILPH_CO_3F_TRAINER_0,
        EventFlag::EVENT_BEAT_SILPH_CO_3F_TRAINER_1,
        EventFlag::EVENT_SILPH_CO_3_UNLOCKED_DOOR1,
        EventFlag::EVENT_SILPH_CO_3_UNLOCKED_DOOR2,
        EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_0,
        EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_1,
        EventFlag::EVENT_BEAT_SILPH_CO_4F_TRAINER_2,
        EventFlag::EVENT_SILPH_CO_4_UNLOCKED_DOOR1,
        EventFlag::EVENT_SILPH_CO_4_UNLOCKED_DOOR2,
        EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_0,
        EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_1,
        EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_2,
        EventFlag::EVENT_BEAT_SILPH_CO_5F_TRAINER_3,
        EventFlag::EVENT_SILPH_CO_5_UNLOCKED_DOOR1,
        EventFlag::EVENT_SILPH_CO_5_UNLOCKED_DOOR2,
        EventFlag::EVENT_SILPH_CO_5_UNLOCKED_DOOR3,
        EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_0,
        EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_1,
        EventFlag::EVENT_BEAT_SILPH_CO_6F_TRAINER_2,
        EventFlag::EVENT_SILPH_CO_6_UNLOCKED_DOOR,
        EventFlag::EVENT_BEAT_SILPH_CO_RIVAL,
        EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_0,
        EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_1,
        EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_2,
        EventFlag::EVENT_BEAT_SILPH_CO_7F_TRAINER_3,
        EventFlag::EVENT_SILPH_CO_7_UNLOCKED_DOOR1,
        EventFlag::EVENT_SILPH_CO_7_UNLOCKED_DOOR2,
        EventFlag::EVENT_SILPH_CO_7_UNLOCKED_DOOR3,
        EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_0,
        EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_1,
        EventFlag::EVENT_BEAT_SILPH_CO_8F_TRAINER_2,
        EventFlag::EVENT_SILPH_CO_8_UNLOCKED_DOOR,
        EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_0,
        EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_1,
        EventFlag::EVENT_BEAT_SILPH_CO_9F_TRAINER_2,
        EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR1,
        EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR2,
        EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR3,
        EventFlag::EVENT_SILPH_CO_9_UNLOCKED_DOOR4,
        EventFlag::EVENT_BEAT_SILPH_CO_10F_TRAINER_0,
        EventFlag::EVENT_BEAT_SILPH_CO_10F_TRAINER_1,
        EventFlag::EVENT_SILPH_CO_10_UNLOCKED_DOOR,
        EventFlag::EVENT_BEAT_SILPH_CO_11F_TRAINER_0,
        EventFlag::EVENT_BEAT_SILPH_CO_11F_TRAINER_1,
        EventFlag::EVENT_SILPH_CO_11_UNLOCKED_DOOR,
        EventFlag::EVENT_GOT_MASTER_BALL,
        EventFlag::EVENT_BEAT_SILPH_CO_GIOVANNI,
        EventFlag::EVENT_BEAT_MANSION_2_TRAINER_0,
        EventFlag::EVENT_BEAT_MANSION_3_TRAINER_0,
        EventFlag::EVENT_BEAT_MANSION_3_TRAINER_1,
        EventFlag::EVENT_BEAT_MANSION_4_TRAINER_0,
        EventFlag::EVENT_BEAT_MANSION_4_TRAINER_1,
        EventFlag::EVENT_GOT_HM03,
        EventFlag::EVENT_BEAT_MEWTWO,
        EventFlag::EVENT_BEAT_LORELEIS_ROOM_TRAINER_0,
        EventFlag::EVENT_AUTOWALKED_INTO_LORELEIS_ROOM,
        EventFlag::EVENT_BEAT_BRUNOS_ROOM_TRAINER_0,
        EventFlag::EVENT_AUTOWALKED_INTO_BRUNOS_ROOM,
        EventFlag::EVENT_BEAT_AGATHAS_ROOM_TRAINER_0,
        EventFlag::EVENT_AUTOWALKED_INTO_AGATHAS_ROOM,
        EventFlag::EVENT_BEAT_LANCES_ROOM_TRAINER_0,
        EventFlag::EVENT_BEAT_LANCE,
        EventFlag::EVENT_LANCES_ROOM_LOCK_DOOR,
        EventFlag::EVENT_BEAT_CHAMPION_RIVAL,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_1_TRAINER_0,
        EventFlag::EVENT_BEAT_VICTORY_ROAD_1_TRAINER_1,
        EventFlag::EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_0,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_1,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_2,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_3,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_4,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_5,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_6,
        EventFlag::EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_7,
        EventFlag::EVENT_SEAFOAM2_BOULDER1_DOWN_HOLE,
        EventFlag::EVENT_SEAFOAM2_BOULDER2_DOWN_HOLE,
        EventFlag::EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE,
        EventFlag::EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE,
        EventFlag::EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE,
        EventFlag::EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE,
        EventFlag::EVENT_BEAT_ARTICUNO,
    ];
}
