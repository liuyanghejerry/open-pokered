//! Menu provider for Pokemon Red — bridges the [`MenuSystem`] abstraction
//! with the concrete Pokemon menu screens.
//!
//! [`MenuSystem`]: jrpg_engine::menu::MenuSystem

use jrpg_engine::menu::{MenuLayout, MenuOption, MenuProvider};

// ---------------------------------------------------------------------------
// PokemonMenuId
// ---------------------------------------------------------------------------

/// Identifies a specific Pokemon menu screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokemonMenuId {
    /// Main menu (CONTINUE / NEW GAME / OPTION).
    Main,
    /// Overworld start menu (POKéDEX / POKéMON / ITEM / … / EXIT).
    Start,
    /// Options / settings menu (text speed, battle anim, battle style).
    Options,
    /// Save confirmation menu.
    Save,
}

// ---------------------------------------------------------------------------
// PokemonMenuProvider
// ---------------------------------------------------------------------------

/// Holds the dynamic state needed to provide menu data to a [`MenuSystem`].
///
/// The provider stores the currently-visible options for each menu so that
/// the [`MenuProvider::options`] method can return a `&[MenuOption]` whose
/// lifetime is tied to `&self`.
#[derive(Debug, Clone)]
pub struct PokemonMenuProvider {
    // ---- game state flags ----
    pub has_save: bool,
    pub has_pokedex: bool,
    pub has_pokemon: bool,
    pub is_link_connected: bool,
    pub player_name: String,

    // ---- cached menu data ----
    main_options: Vec<MenuOption>,
    start_options: Vec<MenuOption>,
    options_list: Vec<MenuOption>,
    save_options: Vec<MenuOption>,
}

impl PokemonMenuProvider {
    /// Create a new provider with default state (no save, no pokedex).
    pub fn new() -> Self {
        let mut provider = Self {
            has_save: false,
            has_pokedex: false,
            has_pokemon: true,
            is_link_connected: false,
            player_name: String::new(),
            main_options: Vec::new(),
            start_options: Vec::new(),
            options_list: Vec::new(),
            save_options: Vec::new(),
        };
        provider.refresh_all();
        provider
    }

    /// Update internal state and rebuild all cached option lists.
    pub fn refresh_all(&mut self) {
        self.refresh_main();
        self.refresh_start();
        self.refresh_options();
        self.refresh_save();
    }

    // -- per-menu refresh helpers --

    fn refresh_main(&mut self) {
        self.main_options.clear();
        if self.has_save {
            self.main_options.push(MenuOption::new("CONTINUE"));
            self.main_options.push(MenuOption::new("NEW GAME"));
        } else {
            self.main_options.push(MenuOption::new("NEW GAME"));
        }
        self.main_options.push(MenuOption::new("OPTION"));
    }

    fn refresh_start(&mut self) {
        self.start_options.clear();
        if self.has_pokedex {
            self.start_options.push(MenuOption::new("POKéDEX"));
        }
        if self.has_pokemon {
            self.start_options.push(MenuOption::new("POKéMON"));
        }
        self.start_options.push(MenuOption::new("ITEM"));
        let trainer_label = if self.player_name.is_empty() {
            "".to_string()
        } else {
            self.player_name.clone()
        };
        self.start_options.push(MenuOption::new(trainer_label));
        if self.is_link_connected {
            self.start_options.push(MenuOption::new("RESET"));
        } else {
            self.start_options.push(MenuOption::new("SAVE"));
        }
        self.start_options.push(MenuOption::new("OPTION"));
        self.start_options.push(MenuOption::new("EXIT"));
    }

    fn refresh_options(&mut self) {
        self.options_list.clear();
        self.options_list.push(MenuOption::new("TEXT SPEED"));
        self.options_list.push(MenuOption::new("BATTLE ANIMATION"));
        self.options_list.push(MenuOption::new("BATTLE STYLE"));
        self.options_list.push(MenuOption::new("CANCEL"));
    }

    fn refresh_save(&mut self) {
        self.save_options.clear();
        self.save_options.push(MenuOption::new("SAVE"));
        self.save_options.push(MenuOption::new("CANCEL"));
    }
}

impl Default for PokemonMenuProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MenuProvider impl
// ---------------------------------------------------------------------------

impl MenuProvider for PokemonMenuProvider {
    type MenuId = PokemonMenuId;

    fn title(&self, menu: Self::MenuId) -> &str {
        match menu {
            PokemonMenuId::Main => "",
            PokemonMenuId::Start => "",
            PokemonMenuId::Options => "OPTION",
            PokemonMenuId::Save => "SAVE",
        }
    }

    fn options(&self, menu: Self::MenuId) -> &[MenuOption] {
        match menu {
            PokemonMenuId::Main => &self.main_options,
            PokemonMenuId::Start => &self.start_options,
            PokemonMenuId::Options => &self.options_list,
            PokemonMenuId::Save => &self.save_options,
        }
    }

    fn option_count(&self, menu: Self::MenuId) -> u8 {
        self.options(menu).len() as u8
    }

    fn scrollable(&self, _menu: Self::MenuId) -> bool {
        false
    }

    fn layout(&self, menu: Self::MenuId) -> MenuLayout {
        match menu {
            PokemonMenuId::Main => MenuLayout::new(5, 6, 10, 7),
            PokemonMenuId::Start => MenuLayout::new(1, 3, 14, 12).with_cursor(true),
            PokemonMenuId::Options => MenuLayout::new(4, 3, 12, 16),
            PokemonMenuId::Save => MenuLayout::new(5, 8, 10, 6),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use jrpg_engine::menu::MenuSystem;

    fn provider() -> PokemonMenuProvider {
        PokemonMenuProvider::new()
    }

    fn provider_with_save() -> PokemonMenuProvider {
        let mut p = PokemonMenuProvider::new();
        p.has_save = true;
        p.refresh_all();
        p
    }

    fn provider_with_pokedex() -> PokemonMenuProvider {
        let mut p = PokemonMenuProvider::new();
        p.has_save = true;
        p.has_pokedex = true;
        p.refresh_all();
        p
    }

    fn provider_link() -> PokemonMenuProvider {
        let mut p = PokemonMenuProvider::new();
        p.is_link_connected = true;
        p.refresh_all();
        p
    }

    // -- PokemonMenuId ------------------------------------------------

    #[test]
    fn menu_id_debug_and_eq() {
        assert_eq!(PokemonMenuId::Main, PokemonMenuId::Main);
        assert_ne!(PokemonMenuId::Main, PokemonMenuId::Start);
        assert_eq!(
            format!("{:?}", PokemonMenuId::Save),
            "Save"
        );
    }

    #[test]
    fn menu_id_copy() {
        let a = PokemonMenuId::Options;
        let b = a;
        assert_eq!(a, b);
    }

    // -- Main menu ----------------------------------------------------

    #[test]
    fn main_menu_no_save_has_two_options() {
        let p = provider();
        let opts = p.options(PokemonMenuId::Main);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].label, "NEW GAME");
        assert_eq!(opts[1].label, "OPTION");
    }

    #[test]
    fn main_menu_with_save_has_three_options() {
        let p = provider_with_save();
        let opts = p.options(PokemonMenuId::Main);
        assert_eq!(opts.len(), 3);
        assert_eq!(opts[0].label, "CONTINUE");
        assert_eq!(opts[1].label, "NEW GAME");
        assert_eq!(opts[2].label, "OPTION");
    }

    #[test]
    fn main_menu_title_empty() {
        let p = provider();
        assert_eq!(p.title(PokemonMenuId::Main), "");
    }

    #[test]
    fn main_menu_has_layout() {
        let p = provider();
        let layout = p.layout(PokemonMenuId::Main);
        assert_eq!(layout.position.tx, 5);
        assert_eq!(layout.position.ty, 6);
        assert_eq!(layout.size.tw, 10);
        assert_eq!(layout.size.th, 7);
    }

    // -- Start menu ---------------------------------------------------

    #[test]
    fn start_menu_basic_has_six_items() {
        let p = provider();
        let opts = p.options(PokemonMenuId::Start);
        assert_eq!(opts.len(), 6);
        assert_eq!(opts[0].label, "POKéMON");
        assert_eq!(opts[1].label, "ITEM");
        assert_eq!(opts[4].label, "SAVE");
        assert_eq!(opts[5].label, "EXIT");
    }

    #[test]
    fn start_menu_with_pokedex_has_seven() {
        let p = provider_with_pokedex();
        let opts = p.options(PokemonMenuId::Start);
        assert_eq!(opts.len(), 7);
        assert_eq!(opts[0].label, "POKéDEX");
        assert_eq!(opts[1].label, "POKéMON");
    }

    #[test]
    fn start_menu_link_shows_reset() {
        let p = provider_link();
        let opts = p.options(PokemonMenuId::Start);
        assert!(opts.iter().any(|o| o.label == "RESET"));
        assert!(!opts.iter().any(|o| o.label == "SAVE"));
    }

    #[test]
    fn start_menu_shows_player_name() {
        let mut p = provider();
        p.player_name = "RED".into();
        p.refresh_all();
        let opts = p.options(PokemonMenuId::Start);
        assert_eq!(opts[3].label, "RED");
    }

    #[test]
    fn start_menu_no_pokemon_omits_pokemon() {
        let mut p = provider();
        p.has_pokemon = false;
        p.refresh_all();
        let opts = p.options(PokemonMenuId::Start);
        assert_eq!(opts.len(), 5);
        assert_eq!(opts[0].label, "ITEM");
        assert_eq!(opts[4].label, "EXIT");
    }

    // -- Options menu -------------------------------------------------

    #[test]
    fn options_menu_has_four_rows() {
        let p = provider();
        let opts = p.options(PokemonMenuId::Options);
        assert_eq!(opts.len(), 4);
        assert_eq!(opts[0].label, "TEXT SPEED");
        assert_eq!(opts[1].label, "BATTLE ANIMATION");
        assert_eq!(opts[2].label, "BATTLE STYLE");
        assert_eq!(opts[3].label, "CANCEL");
    }

    #[test]
    fn options_menu_title() {
        let p = provider();
        assert_eq!(p.title(PokemonMenuId::Options), "OPTION");
    }

    // -- Save menu ----------------------------------------------------

    #[test]
    fn save_menu_has_two_options() {
        let p = provider();
        let opts = p.options(PokemonMenuId::Save);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].label, "SAVE");
        assert_eq!(opts[1].label, "CANCEL");
    }

    #[test]
    fn save_menu_title() {
        let p = provider();
        assert_eq!(p.title(PokemonMenuId::Save), "SAVE");
    }

    // -- MenuSystem integration ---------------------------------------

    #[test]
    fn menu_system_main_navigates() {
        let p = provider_with_save();
        let mut sys = MenuSystem::new(&p);
        sys.open(PokemonMenuId::Main);

        assert!(sys.is_open());
        assert_eq!(sys.cursor, 0);

        use jrpg_engine::menu::{MenuAction, MenuInput};
        let action = sys.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::Down);
        assert_eq!(sys.cursor, 1);
    }

    #[test]
    fn menu_system_start_opens_and_closes() {
        let p = provider_with_pokedex();
        let mut sys = MenuSystem::new(&p);
        sys.open(PokemonMenuId::Start);
        assert!(sys.is_open());
        sys.close();
        assert!(!sys.is_open());
    }

    #[test]
    fn menu_system_options_selects() {
        let p = provider();
        let mut sys = MenuSystem::new(&p);
        sys.open(PokemonMenuId::Options);

        use jrpg_engine::menu::{MenuAction, MenuInput};
        let action = sys.handle_input(&MenuInput {
            confirm: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::Selected(0));
    }

    #[test]
    fn menu_system_save_cancels() {
        let p = provider();
        let mut sys = MenuSystem::new(&p);
        sys.open(PokemonMenuId::Save);

        use jrpg_engine::menu::{MenuAction, MenuInput};
        let action = sys.handle_input(&MenuInput {
            cancel: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::Cancelled);
        assert!(!sys.is_open());
    }

    #[test]
    fn menu_system_all_options_enabled() {
        let p = provider_with_pokedex();
        let mut sys = MenuSystem::new(&p);
        sys.open(PokemonMenuId::Start);

        use jrpg_engine::menu::MenuInput;

        let total = p.option_count(PokemonMenuId::Start);
        for i in 0..total {
            let opt = sys.selected_option();
            assert!(opt.is_some(), "cursor {} has no option", i);
            assert!(opt.unwrap().enabled, "option {} is disabled", i);

            if i + 1 < total {
                sys.handle_input(&MenuInput {
                    down: true,
                    ..Default::default()
                });
            }
        }
    }

    #[test]
    fn menu_system_selected_option_returns_label() {
        let mut p = provider();
        p.player_name = "ASH".into();
        p.refresh_all();
        let mut sys = MenuSystem::new(&p);
        sys.open(PokemonMenuId::Start);

        use jrpg_engine::menu::MenuInput;

        for _ in 0..3 {
            sys.handle_input(&MenuInput {
                down: true,
                ..Default::default()
            });
        }
        let opt = sys.selected_option().unwrap();
        assert_eq!(opt.label, "ASH");
    }

    #[test]
    fn menu_provider_default_creates_valid_state() {
        let p = PokemonMenuProvider::default();
        assert!(!p.has_save);
        assert!(!p.has_pokedex);
        assert!(p.has_pokemon);
        assert!(!p.is_link_connected);
        assert_eq!(p.options(PokemonMenuId::Main).len(), 2);
        assert_eq!(p.options(PokemonMenuId::Start).len(), 6);
    }

    #[test]
    fn refresh_all_rebuilds_everything() {
        let mut p = PokemonMenuProvider::new();
        assert_eq!(p.options(PokemonMenuId::Main).len(), 2);

        p.has_save = true;
        p.has_pokedex = true;
        p.player_name = "RED".into();
        p.refresh_all();

        assert_eq!(p.options(PokemonMenuId::Main).len(), 3);
        assert_eq!(p.options(PokemonMenuId::Start).len(), 7);
        assert_eq!(p.options(PokemonMenuId::Start)[0].label, "POKéDEX");
        assert_eq!(p.options(PokemonMenuId::Start)[3].label, "RED");
    }

    #[test]
    fn menu_layout_has_correct_sizes() {
        let p = provider();

        let main = p.layout(PokemonMenuId::Main);
        assert_eq!(main.size.tw, 10);
        assert_eq!(main.size.th, 7);

        let start = p.layout(PokemonMenuId::Start);
        assert_eq!(start.size.tw, 14);
        assert_eq!(start.size.th, 12);

        let options = p.layout(PokemonMenuId::Options);
        assert_eq!(options.size.tw, 12);
        assert_eq!(options.size.th, 16);

        let save = p.layout(PokemonMenuId::Save);
        assert_eq!(save.size.tw, 10);
        assert_eq!(save.size.th, 6);
    }

    #[test]
    fn no_menus_are_scrollable() {
        let p = provider();
        assert!(!p.scrollable(PokemonMenuId::Main));
        assert!(!p.scrollable(PokemonMenuId::Start));
        assert!(!p.scrollable(PokemonMenuId::Options));
        assert!(!p.scrollable(PokemonMenuId::Save));
    }
}
