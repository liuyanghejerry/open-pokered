use crate::items::inventory::{Inventory, BAG_ITEM_CAPACITY, PC_ITEM_CAPACITY};
use crate::options_menu::GameOptions;
use crate::pokemon::pokedex::Pokedex;
use serde::{Deserialize, Serialize};

pub const NAME_LENGTH: usize = 11;
pub const NUM_BADGES: usize = 8;
pub const MAX_WARP_EVENTS: usize = 32;
pub const MAX_BG_EVENTS: usize = 16;
pub const MAX_OBJECT_EVENTS: usize = 16;
pub const SPRITE_SET_LENGTH: usize = 11;
pub const NUM_EVENTS_BYTES: usize = (0xA00 + 7) / 8;
pub const GAME_PROGRESS_FLAGS_SIZE: usize = 0x78;
pub const HIDDEN_ITEMS_BYTES: usize = (112 + 7) / 8;
pub const HIDDEN_COINS_BYTES: usize = (16 + 7) / 8;
pub const CITY_VISITED_BYTES: usize = (11 + 7) / 8;
pub const TOGGLEABLE_OBJECT_BYTES: usize = (256 + 7) / 8;
pub const WILDDATA_LENGTH: usize = 1 + 10 * 2;

// The SRAM event-flags region is the original game's `wEventFlags` array
// (`flag_array NUM_EVENTS`, NUM_EVENTS = $A00 bits → 320 bytes).
// `pokered_data::event_flags::EVENT_FLAGS_SIZE` is the runtime bitset size
// and must stay in sync with this save-layout constant.
const _: () = assert!(NUM_EVENTS_BYTES == pokered_data::event_flags::EVENT_FLAGS_SIZE);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapPosition {
    pub map_id: u8,
    pub y: u8,
    pub x: u8,
    pub y_block: u8,
    pub x_block: u8,
}

impl MapPosition {
    pub fn new() -> Self {
        Self {
            map_id: 0,
            y: 0,
            x: 0,
            y_block: 0,
            x_block: 0,
        }
    }
}

impl Default for MapPosition {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapHeader {
    pub tileset: u8,
    pub height: u8,
    pub width: u8,
    pub data_ptr: u16,
    pub text_ptr: u16,
    pub script_ptr: u16,
    pub connections: u8,
}

impl MapHeader {
    pub fn new() -> Self {
        Self {
            tileset: 0,
            height: 0,
            width: 0,
            data_ptr: 0,
            text_ptr: 0,
            script_ptr: 0,
            connections: 0,
        }
    }
}

impl Default for MapHeader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapConnection {
    pub connected_map: u8,
    pub strip_src: u16,
    pub strip_dest: u16,
    pub strip_length: u8,
    pub connected_width: u8,
    pub y_alignment: u8,
    pub x_alignment: u8,
    pub view_pointer: u16,
}

impl MapConnection {
    pub fn new() -> Self {
        Self {
            connected_map: 0,
            strip_src: 0,
            strip_dest: 0,
            strip_length: 0,
            connected_width: 0,
            y_alignment: 0,
            x_alignment: 0,
            view_pointer: 0,
        }
    }
}

impl Default for MapConnection {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarpEntry {
    pub y: u8,
    pub x: u8,
    pub warp_id: u8,
    pub map_id: u8,
}

impl WarpEntry {
    pub fn new() -> Self {
        Self {
            y: 0,
            x: 0,
            warp_id: 0,
            map_id: 0,
        }
    }
}

impl Default for WarpEntry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayTime {
    pub hours: u8,
    pub maxed: bool,
    pub minutes: u8,
    pub seconds: u8,
    pub frames: u8,
}

impl PlayTime {
    pub fn new() -> Self {
        Self {
            hours: 0,
            maxed: false,
            minutes: 0,
            seconds: 0,
            frames: 0,
        }
    }

    pub fn total_seconds(&self) -> u32 {
        self.hours as u32 * 3600 + self.minutes as u32 * 60 + self.seconds as u32
    }

    /// Tick one frame (60Hz). Carries: frames→seconds→minutes→hours.
    /// Caps at 255:59:59:59 and sets maxed flag.
    pub fn tick(&mut self) {
        if self.maxed {
            return;
        }
        self.frames = self.frames.wrapping_add(1);
        if self.frames >= 60 {
            self.frames = 0;
            self.seconds = self.seconds.wrapping_add(1);
            if self.seconds >= 60 {
                self.seconds = 0;
                self.minutes = self.minutes.wrapping_add(1);
                if self.minutes >= 60 {
                    self.minutes = 0;
                    self.hours = self.hours.wrapping_add(1);
                    if self.hours >= 255 {
                        self.hours = 255;
                        self.minutes = 59;
                        self.seconds = 59;
                        self.frames = 59;
                        self.maxed = true;
                    }
                }
            }
        }
    }
}

impl Default for PlayTime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayCareMon {
    pub in_use: bool,
    pub species: u8,
    pub hp: u16,
    pub box_level: u8,
    pub status: u8,
    pub type1: u8,
    pub type2: u8,
    pub catch_rate: u8,
    pub moves: [u8; 4],
    pub ot_id: u16,
    pub exp: u32,
    pub hp_exp: u16,
    pub attack_exp: u16,
    pub defense_exp: u16,
    pub speed_exp: u16,
    pub special_exp: u16,
    pub dvs: u16,
    pub pp: [u8; 4],
}

impl DayCareMon {
    pub fn new() -> Self {
        Self {
            in_use: false,
            species: 0,
            hp: 0,
            box_level: 0,
            status: 0,
            type1: 0,
            type2: 0,
            catch_rate: 0,
            moves: [0; 4],
            ot_id: 0,
            exp: 0,
            hp_exp: 0,
            attack_exp: 0,
            defense_exp: 0,
            speed_exp: 0,
            special_exp: 0,
            dvs: 0,
            pp: [0; 4],
        }
    }
}

impl Default for DayCareMon {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameData {
    pub pokedex: Pokedex,
    pub bag: Inventory<BAG_ITEM_CAPACITY>,
    pub player_money: u32,
    pub rival_name: Vec<u8>,
    pub options: GameOptions,
    pub obtained_badges: u8,
    pub letter_printing_delay_flags: u8,
    pub player_id: u16,
    pub map_music_sound_id: u8,
    pub map_music_rom_bank: u8,
    pub map_pal_offset: u8,
    pub position: MapPosition,
    pub last_map: u8,
    pub map_header: MapHeader,
    pub connections: [MapConnection; 4],
    pub sprite_set: [u8; SPRITE_SET_LENGTH],
    pub sprite_set_id: u8,
    pub map_background_tile: u8,
    pub num_warps: u8,
    pub warp_entries: [WarpEntry; MAX_WARP_EVENTS],
    pub destination_warp_id: u8,
    pub num_signs: u8,
    pub sign_coords: [(u8, u8); MAX_BG_EVENTS],
    pub sign_text_ids: [u8; MAX_BG_EVENTS],
    pub num_sprites: u8,
    pub map_sprite_data: [(u8, u8); MAX_OBJECT_EVENTS],
    pub map_sprite_extra_data: [(u8, u8); MAX_OBJECT_EVENTS],
    pub current_map_height2: u8,
    pub current_map_width2: u8,
    pub player_moving_direction: u8,
    pub player_last_stop_direction: u8,
    pub player_direction: u8,
    pub tileset_bank: u8,
    pub tileset_blocks_ptr: u16,
    pub tileset_gfx_ptr: u16,
    pub tileset_collision_ptr: u16,
    pub tileset_talking_over_tiles: [u8; 3],
    pub grass_tile: u8,
    pub pc_items: Inventory<PC_ITEM_CAPACITY>,
    pub current_box_num: u8,
    pub num_hof_teams: u8,
    pub player_coins: u16,
    pub toggleable_object_flags: [u8; TOGGLEABLE_OBJECT_BYTES],
    pub game_progress_flags: Vec<u8>,
    pub obtained_hidden_items: [u8; HIDDEN_ITEMS_BYTES],
    pub obtained_hidden_coins: [u8; HIDDEN_COINS_BYTES],
    pub walk_bike_surf_state: u8,
    pub town_visited_flags: [u8; CITY_VISITED_BYTES],
    pub safari_steps: u16,
    pub fossil_item: u8,
    pub fossil_mon: u8,
    pub enemy_mon_or_trainer_class: u8,
    pub rival_starter: u8,
    pub player_starter: u8,
    pub last_blackout_map: u8,
    pub destination_map: u8,
    pub tile_in_front_of_boulder: u8,
    pub dungeon_warp_dest_map: u8,
    pub which_dungeon_warp: u8,
    pub status_flags: [u8; 12],
    pub completed_in_game_trade_flags: u16,
    pub warped_from_warp: u8,
    pub warped_from_map: u8,
    pub card_key_door_y: u8,
    pub card_key_door_x: u8,
    pub first_lock_trash_can: u8,
    pub second_lock_trash_can: u8,
    pub event_flags: Vec<u8>,
    pub grass_rate: u8,
    pub grass_mons: [u8; WILDDATA_LENGTH - 1],
    pub water_rate: u8,
    pub water_mons: [u8; WILDDATA_LENGTH - 1],
    pub trainer_header_ptr: u16,
    pub opponent_after_wrong_answer: u8,
    pub cur_map_script: u8,
    pub play_time: PlayTime,
    pub safari_zone_game_over: u8,
    pub num_safari_balls: u8,
    pub daycare: DayCareMon,
    pub daycare_mon_name: Vec<u8>,
    pub daycare_mon_ot: Vec<u8>,
}

impl GameData {
    /// Credit the Game Corner coin balance, capped at the 9999 maximum (the
    /// 4-digit BCD `wPlayerCoins` field in the original).
    pub fn give_coins(&mut self, amount: u16) {
        self.player_coins = self.player_coins.saturating_add(amount).min(9999);
    }

    /// Debit the Game Corner coin balance, saturating at zero.
    pub fn take_coins(&mut self, amount: u16) {
        self.player_coins = self.player_coins.saturating_sub(amount);
    }

    /// Advance the deposited Day Care Pokémon's experience by one overworld
    /// step (original `IncrementDayCareMonExp`). No-op if nothing is deposited.
    /// Capped at the species' level-100 experience so the derived level never
    /// exceeds 100.
    pub fn tick_daycare_exp(&mut self) {
        use crate::battle::experience::growth::max_exp;
        use pokered_data::pokemon_data::get_base_stats;
        use pokered_data::species::Species;
        if self.daycare.in_use {
            let species = Species::from_index_id(self.daycare.species);
            let cap = get_base_stats(species)
                .map(|b| max_exp(b.growth_rate))
                .unwrap_or(u32::MAX);
            self.daycare.exp = self.daycare.exp.saturating_add(1).min(cap);
        }
    }
}

impl GameData {
    pub fn new() -> Self {
        Self {
            pokedex: Pokedex::new(),
            bag: Inventory::new_bag(),
            player_money: 0,
            rival_name: Vec::new(),
            options: GameOptions::default(),
            obtained_badges: 0,
            letter_printing_delay_flags: 0,
            player_id: 0,
            map_music_sound_id: 0,
            map_music_rom_bank: 0,
            map_pal_offset: 0,
            position: MapPosition::new(),
            last_map: 0,
            map_header: MapHeader::new(),
            connections: [MapConnection::new(); 4],
            sprite_set: [0; SPRITE_SET_LENGTH],
            sprite_set_id: 0,
            map_background_tile: 0,
            num_warps: 0,
            warp_entries: [WarpEntry::new(); MAX_WARP_EVENTS],
            destination_warp_id: 0,
            num_signs: 0,
            sign_coords: [(0, 0); MAX_BG_EVENTS],
            sign_text_ids: [0; MAX_BG_EVENTS],
            num_sprites: 0,
            map_sprite_data: [(0, 0); MAX_OBJECT_EVENTS],
            map_sprite_extra_data: [(0, 0); MAX_OBJECT_EVENTS],
            current_map_height2: 0,
            current_map_width2: 0,
            player_moving_direction: 0,
            player_last_stop_direction: 0,
            player_direction: 0,
            tileset_bank: 0,
            tileset_blocks_ptr: 0,
            tileset_gfx_ptr: 0,
            tileset_collision_ptr: 0,
            tileset_talking_over_tiles: [0; 3],
            grass_tile: 0,
            pc_items: Inventory::new_pc(),
            current_box_num: 0,
            num_hof_teams: 0,
            player_coins: 0,
            toggleable_object_flags: [0; TOGGLEABLE_OBJECT_BYTES],
            game_progress_flags: vec![0; GAME_PROGRESS_FLAGS_SIZE],
            obtained_hidden_items: [0; HIDDEN_ITEMS_BYTES],
            obtained_hidden_coins: [0; HIDDEN_COINS_BYTES],
            walk_bike_surf_state: 0,
            town_visited_flags: [0; CITY_VISITED_BYTES],
            safari_steps: 0,
            fossil_item: 0,
            fossil_mon: 0,
            enemy_mon_or_trainer_class: 0,
            rival_starter: 0,
            player_starter: 0,
            last_blackout_map: 0,
            destination_map: 0,
            tile_in_front_of_boulder: 0,
            dungeon_warp_dest_map: 0,
            which_dungeon_warp: 0,
            status_flags: [0; 12],
            completed_in_game_trade_flags: 0,
            warped_from_warp: 0,
            warped_from_map: 0,
            card_key_door_y: 0,
            card_key_door_x: 0,
            first_lock_trash_can: 0,
            second_lock_trash_can: 0,
            event_flags: vec![0; NUM_EVENTS_BYTES],
            grass_rate: 0,
            grass_mons: [0; WILDDATA_LENGTH - 1],
            water_rate: 0,
            water_mons: [0; WILDDATA_LENGTH - 1],
            trainer_header_ptr: 0,
            opponent_after_wrong_answer: 0,
            cur_map_script: 0,
            play_time: PlayTime::new(),
            safari_zone_game_over: 0,
            num_safari_balls: 0,
            daycare: DayCareMon::new(),
            daycare_mon_name: Vec::new(),
            daycare_mon_ot: Vec::new(),
        }
    }

    pub fn badge_count(&self) -> u8 {
        self.obtained_badges.count_ones() as u8
    }

    pub fn has_badge(&self, badge: u8) -> bool {
        badge < NUM_BADGES as u8 && (self.obtained_badges & (1 << badge)) != 0
    }

    pub fn set_badge(&mut self, badge: u8) {
        if badge < NUM_BADGES as u8 {
            self.obtained_badges |= 1 << badge;
        }
    }

    /// Mark a city map as visited (gates the FLY destination list).
    /// From MarkTownVisitedAndLoadToggleableObjects: bit index = map ID
    /// (city maps are $00-$0A; wTownVisitedFlag is 2 bytes).
    pub fn mark_town_visited(&mut self, map: pokered_data::maps::MapId) {
        let id = map as usize;
        if id < pokered_data::map_flags::FIRST_ROUTE_MAP as usize {
            self.town_visited_flags[id / 8] |= 1 << (id % 8);
        }
    }

    /// Has this city map been visited?
    pub fn is_town_visited(&self, map: pokered_data::maps::MapId) -> bool {
        let id = map as usize;
        id < pokered_data::map_flags::FIRST_ROUTE_MAP as usize
            && self.town_visited_flags[id / 8] & (1 << (id % 8)) != 0
    }

    /// The FLY destination list — visited city maps in map-ID order,
    /// mirroring BuildFlyLocationsList (engine/items/town_map.asm).
    pub fn fly_destinations(&self) -> Vec<pokered_data::maps::MapId> {
        (0..pokered_data::map_flags::FIRST_ROUTE_MAP)
            .filter_map(pokered_data::maps::MapId::from_u8)
            .filter(|&m| self.is_town_visited(m))
            .collect()
    }

    pub fn serialize_into(&self, buf: &mut Vec<u8>) {
        crate::save::ser_game_data::serialize_game_data_into(self, buf);
    }
}

impl Default for GameData {
    fn default() -> Self {
        Self::new()
    }
}
