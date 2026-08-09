pub use pokered_data as data;

pub mod battle;
pub mod credits;
pub mod debug_log;
pub mod events;
pub mod evolution_screen;
pub mod game_state;
pub mod gamefreak_splash;
pub mod hof_ceremony;
pub mod intro_scene;
pub mod items;
pub mod link;
pub mod main_menu;
pub mod naming_screen;
pub mod oak_speech;
pub mod pc_screen;
pub mod options_menu;
pub mod overworld;
pub mod bag_screen;
pub mod party_screen;
pub mod party_select;
pub mod pokedex_screen;
pub mod stats_screen;
pub mod pokemon;
pub mod save;
pub mod save_menu;
pub mod slots;
pub mod slots_screen;
pub mod elevator_screen;
pub mod start_menu;
pub mod text;
pub mod title_screen;
pub mod town_map_screen;
pub mod trade;
pub mod trainer_card_screen;

#[cfg(test)]
mod main_menu_tests;

#[cfg(test)]
mod naming_screen_tests;

#[cfg(test)]
mod options_menu_tests;

#[cfg(test)]
mod save_menu_tests;

#[cfg(test)]
mod start_menu_tests;

#[cfg(test)]
mod title_screen_tests;
