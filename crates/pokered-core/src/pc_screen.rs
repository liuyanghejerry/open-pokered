//! PC storage screens — Bill's PC (#MON storage), the player's item PC, and
//! PROF.OAK's #DEX rating.
//!
//! Pure-logic, I/O-free state machine mirroring the original:
//! - `engine/menus/pc.asm` — `ActivatePC`: Pokémon Center PC main menu
//!   (BILL's/SOMEONE's PC, <NAME>'s PC, PROF.OAK's PC, #MON LEAGUE, LOG OFF;
//!   Oak's PC only after EVENT_GOT_POKEDEX, League PC only after the Hall of
//!   Fame).
//! - `engine/pokemon/bills_pc.asm` — `BillsPC_` / `DisplayPCMainMenu`:
//!   WITHDRAW / DEPOSIT / RELEASE / CHANGE BOX / SEE YA!, the mon list +
//!   WITHDRAW/STATS/CANCEL popup, the release confirmation, and the "can't
//!   deposit the last #MON" / "box is full" / "can't take any more" guards.
//! - `engine/menus/save.asm` — `ChangeBox`: "When you change a #MON BOX, data
//!   will be saved. Is that okay?" + the 12-box chooser + game save.
//! - `engine/menus/players_pc.asm` — `PlayerPC`: WITHDRAW ITEM / DEPOSIT ITEM /
//!   TOSS ITEM / LOG OFF with "How many?" quantity selection; key items skip
//!   the quantity prompt and HMs/key items refuse to toss.
//! - `engine/menus/oaks_pc.asm` + `engine/events/pokedex_rating.asm` —
//!   "Want to get your #DEX rated?" + the owned-count rating table.
//!
//! Rendering lives in the app layer (`pokered-app/src/render/pc.rs`); the
//! screen exposes its phase and cursors for it.

use crate::alloc_prelude::*;
use crate::items::inventory::{
    is_tossable, BAG_ITEM_CAPACITY, MAX_ITEM_QUANTITY, PC_ITEM_CAPACITY, Inventory,
};
use crate::main_menu::MenuInput;
use crate::pokemon::party::Party;
use crate::pokemon::pc_box::{PcStorage, NUM_BOXES};
use crate::pokemon::pc_menu::{BillsPcAction, BillsPcMenuState, PcMainMenuState, PcMainMenuTarget, PlayersPcAction, PlayersPcMenuState};
use crate::pokemon::pokedex::Pokedex;
use pokered_data::items::ItemId;

/// How the PC was opened (which original entry point).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcEntry {
    /// Pokémon Center PC — `ActivatePC` (engine/menus/pc.asm:1): the full
    /// main menu. Triggered by the PC hidden event at (13,3) in every
    /// Pokémon Center (data/events/hidden_events.asm:156 etc.).
    PokemonCenter,
    /// Bedroom PC — `PlayerPC` accessed directly (players_pc.asm:13-17):
    /// item storage only, no main menu. Triggered by the hidden event at
    /// (0,1) in REDS_HOUSE_2F (hidden_events.asm:137).
    PlayersPc,
    /// Bill's house PC — `TextScript_BillsPC` (home/map_objects.asm:35):
    /// straight into the #MON storage system, no main menu.
    BillsPc,
}

/// One recorded Hall of Fame mon, as shown by the #MON LEAGUE PC viewer
/// (LeaguePCShowMon, engine/menus/league_pc.asm:78-113).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HofMonView {
    pub species: pokered_data::species::Species,
    pub level: u8,
    /// Decoded nickname (display-ready).
    pub nickname: String,
}

/// One recorded Hall of Fame team. `team_no` is the original's `wHoFTeamNo`
/// (league_pc.asm:29-35): the all-time team number, so a team recorded after
/// the 50-team SRAM window wrapped keeps its true number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HofTeamRecord {
    pub team_no: u8,
    pub mons: Vec<HofMonView>,
}

/// Flags/names captured when the PC is opened (the app reads them out of the
/// save + script flags; the screen itself is save-agnostic).
#[derive(Debug, Clone)]
pub struct PcOpenContext {
    /// EVENT_GOT_POKEDEX — gates PROF.OAK's PC in the main menu
    /// (bills_pc.asm DisplayPCMainMenu:8).
    pub has_pokedex: bool,
    /// EVENT_MET_BILL — picks "BILL's PC" vs "SOMEONE's PC" labels
    /// (bills_pc.asm:31-39, pc.asm:77-82).
    pub met_bill: bool,
    /// wNumHoFTeams > 0 — adds the #MON LEAGUE entry (bills_pc.asm:5-6,53-63).
    pub beaten_league: bool,
    /// Decoded player name, for the "<NAME>'s PC" label and "turned on" text.
    pub player_name: String,
    /// Recorded Hall of Fame teams (oldest first) for the #MON LEAGUE viewer.
    pub hof_teams: Vec<HofTeamRecord>,
}

/// Sound effects the screen asks the app to play (SFX ids in pokered-audio).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcSfx {
    /// SFX_TURN_ON_PC — PC booted.
    TurnOn,
    /// SFX_TURN_OFF_PC — logged off.
    TurnOff,
    /// SFX_ENTER_PC — entered a sub-PC (pc.asm:54,62,68,74).
    Enter,
    /// SFX_WITHDRAW_DEPOSIT — mon/item moved (bills_pc.asm:133, players_pc.asm:133,188).
    WithdrawDeposit,
    /// The selected Pokémon's cry (BillsPCDeposit/Withdraw/Release).
    Cry(pokered_data::species::Species),
    /// SFX_SAVE — box changed, game saved (save.asm:399).
    Save,
}

/// Result of a single frame update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcScreenAction {
    /// Stay on the PC screen.
    Continue,
    /// Log off / back out of the top level — return to the overworld.
    Exit,
    /// STATS was chosen for a mon; the app should show the stats screen and
    /// then return to the PC (DisplayDepositWithdrawMenu's STATS path,
    /// bills_pc.asm:432-447).
    ShowStats { from_box: bool, index: usize },
}

/// Mutable game state the screen operates on (owned by the app's save data).
pub struct PcContext<'a> {
    pub party: &'a mut Party,
    pub pc_storage: &'a mut PcStorage,
    pub bag: &'a mut Inventory<BAG_ITEM_CAPACITY>,
    pub pc_items: &'a mut Inventory<PC_ITEM_CAPACITY>,
    pub pokedex: &'a Pokedex,
}

/// Which list a mon list is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonListMode {
    Withdraw,
    Deposit,
    Release,
}

/// Which item list is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemListMode {
    Withdraw,
    Deposit,
    Toss,
}

/// Current phase — public so the renderer can switch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcPhase {
    /// A text box page (or pages) is showing; A/B advances.
    Message,
    /// Top-level menu (Pokémon Center entry only).
    MainMenu,
    /// Protected automatic question before the player item menu/list.
    ItemQuestion,
    /// Bill's PC menu (WITHDRAW/DEPOSIT/RELEASE/CHANGE BOX/SEE YA!).
    BillsMenu,
    /// Party/box mon list (WITHDRAW, DEPOSIT or RELEASE mode).
    MonList,
    /// WITHDRAW/DEPOSIT + STATS + CANCEL popup on a listed mon.
    MonAction,
    /// "Once released, X is gone forever. OK?" YES/NO.
    ReleaseConfirm,
    /// "When you change a #MON BOX, data will be saved. Is that okay?" YES/NO.
    ChangeBoxConfirm,
    /// 12-box chooser.
    BoxList,
    /// Player's PC menu (WITHDRAW ITEM/DEPOSIT ITEM/TOSS ITEM/LOG OFF).
    ItemMenu,
    /// Bag/PC item list (WITHDRAW, DEPOSIT or TOSS mode).
    ItemList,
    /// Print the timed DONE quantity question before accepting menu input.
    ItemQuantityPrompt,
    /// "How many?" quantity chooser.
    ItemQuantity,
    /// "Is it OK to toss X?" YES/NO (item_effects.asm TossItem_).
    TossConfirm,
    /// "Want to get your #DEX rated?" YES/NO.
    OaksConfirm,
    /// #MON LEAGUE Hall of Fame viewer — one recorded mon per page with its
    /// "HALL OF FAME No. X" team number (league_pc.asm LeaguePCShowTeam).
    LeagueHoF,
}

/// Where to go when the last message page is dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AfterMessage {
    MainMenu,
    BillsMenu,
    ItemMenu,
    /// Re-enter the item list for the current mode (players_pc.asm loops back
    /// to the list after every successful deposit/withdraw/toss).
    ItemList,
    /// After "Accessed PROF.OAK's PC...": show the rating YES/NO prompt.
    OaksConfirmPage,
    /// After the "#DEX completion is:" page: show the rating text page.
    OaksRating,
    /// After the rating text page: show "Closed link to PROF.OAK's PC."
    OaksClosed,
    /// After "Accessed the HALL OF FAME List.": the #MON LEAGUE HoF viewer.
    LeagueHoF,
    Exit,
}

const MSG_LINES_PER_PAGE: usize = 4;
/// Visible rows in scrolling lists (mon list / item list).
pub const PC_LIST_VISIBLE_ROWS: usize = 4;

/// Rating table thresholds (engine/events/pokedex_rating.asm:58-74): the
/// first entry whose threshold exceeds the owned count is shown.
const DEX_RATINGS: &[(u32, &str)] = &[
    (10, "You still have\nlots to do.\nLook for #MON\nin grassy areas!"),
    (20, "You're on the\nright track!\nGet a FLASH HM\nfrom my AIDE!"),
    (30, "You still need\nmore #MON!\nTry to catch\nother species!"),
    (40, "Good, you're\ntrying hard!\nGet an ITEMFINDER\nfrom my AIDE!"),
    (50, "Looking good!\nGo find my AIDE\nwhen you get 50!"),
    (60, "You finally got at\nleast 50 species!\nBe sure to get\nEXP.ALL from my\nAIDE!"),
    (70, "Ho! This is geting\neven better!"),
    (80, "Very good!\nGo fish for some\nmarine #MON!"),
    (90, "Wonderful!\nDo you like to\ncollect things?"),
    (100, "I'm impressed!\nIt must have been\ndifficult to do!"),
    (110, "You finally got at\nleast 100 species!\nI can't believe\nhow good you are!"),
    (120, "You even have the\nevolved forms of\n#MON! Super!"),
    (130, "Excellent! Trade\nwith friends to\nget some more!"),
    (140, "Outstanding!\nYou've become a\nreal pro at this!"),
    (150, "I have nothing\nleft to say!\nYou're the\nauthority now!"),
    (152, "Your #DEX is\nentirely complete!\nCongratulations!"),
];

/// The #DEX rating text for an owned count (engine/events/pokedex_rating.asm
/// table) — shared by PROF.OAK's PC rating and the Hall of Fame player-stats
/// page (`DisplayDexRating`, engine/movie/hall_of_fame.asm:204-205).
pub fn dex_rating_text(owned: u32) -> &'static str {
    DEX_RATINGS
        .iter()
        .find(|(threshold, _)| owned < *threshold)
        .map(|(_, text)| *text)
        .unwrap_or(DEX_RATINGS[DEX_RATINGS.len() - 1].1)
}

/// Shared by localized PC messages and the Hall of Fame's complete rating.
pub fn chinese_message_lines(lines: &[String], protected: &[&str]) -> Vec<String> {
    pokered_data::ui_text::zh_pc_message(lines).iter()
        .flat_map(|text| crate::text::zh_dialogue::wrap_lines(text, 144, protected))
        .collect()
}

/// The whole PC flow, from "turned on the PC" to LOG OFF.
#[derive(Debug, Clone)]
pub struct PcScreen {
    language: crate::game_state::Lang,
    entry: PcEntry,
    met_bill: bool,
    has_pokedex: bool,
    beaten_league: bool,
    player_name: String,

    phase: PcPhase,

    // Message phase state.
    msg_lines: Vec<String>,
    msg_page: usize,
    msg_next: AfterMessage,

    // Menu states (reused from pokemon::pc_menu, which mirrors the original
    // menu code 1:1).
    main_menu: PcMainMenuState,
    bills_menu: BillsPcMenuState,
    players_menu: PlayersPcMenuState,

    // Mon list state.
    mon_mode: MonListMode,
    mon_cursor: usize,
    mon_action_cursor: usize,

    // YES/NO confirmation phases.
    yes_selected: bool,
    confirm_wait_frames: u8,
    pending_confirmation: Option<bool>,

    // Box chooser.
    box_cursor: usize,

    // Item PC state.
    item_mode: ItemListMode,
    item_list_cursor: usize,
    item_list_scroll: usize,
    item_qty: u8,
    item_text_no_delay: bool,
    item_question_no_delay: bool,
    item_question_is_list: bool,
    item_question_draw_list: bool,
    quantity_text_delay_frames: u16,
    quantity_prompt_intro: u8,
    quantity_prompt_chars: u8,
    quantity_prompt_letter_wait: u16,
    quantity_prompt_one_frame_wait: bool,

    // Side effects for the app.
    sfx: Vec<PcSfx>,
    waiting_for_cry: bool,
    finishing_cry: bool,
    save_requested: bool,
    dex_seen: u32,
    dex_owned: u32,

    // #MON LEAGUE HoF viewer state.
    hof_teams: Vec<HofTeamRecord>,
    league_team: usize,
    league_mon: usize,
}

impl PcScreen {
    pub fn new(entry: PcEntry, open: &PcOpenContext) -> Self {
        Self::new_with_language(entry, open, crate::game_state::Lang::En)
    }

    pub fn new_with_language(entry: PcEntry, open: &PcOpenContext, language: crate::game_state::Lang) -> Self {
        let mut screen = Self {
            language,
            entry,
            met_bill: open.met_bill,
            has_pokedex: open.has_pokedex,
            beaten_league: open.beaten_league,
            player_name: open.player_name.clone(),
            phase: PcPhase::Message,
            msg_lines: Vec::new(),
            msg_page: 0,
            msg_next: AfterMessage::Exit,
            main_menu: PcMainMenuState::new(open.has_pokedex, open.beaten_league, open.met_bill),
            bills_menu: BillsPcMenuState::new(0),
            players_menu: PlayersPcMenuState::new(),
            mon_mode: MonListMode::Withdraw,
            mon_cursor: 0,
            mon_action_cursor: 0,
            yes_selected: true,
            confirm_wait_frames: 0,
            pending_confirmation: None,
            box_cursor: 0,
            item_mode: ItemListMode::Withdraw,
            item_list_cursor: 0,
            item_list_scroll: 0,
            item_qty: 1,
            item_text_no_delay: true,
            item_question_no_delay: true,
            item_question_is_list: false,
            item_question_draw_list: false,
            quantity_text_delay_frames: 3,
            quantity_prompt_intro: 0,
            quantity_prompt_chars: 0,
            quantity_prompt_letter_wait: 0,
            quantity_prompt_one_frame_wait: false,
            sfx: Vec::new(),
            waiting_for_cry: false,
            finishing_cry: false,
            save_requested: false,
            dex_seen: 0,
            dex_owned: 0,
            hof_teams: open.hof_teams.clone(),
            league_team: 0,
            league_mon: 0,
        };
        match entry {
            // "<PLAYER> turned on the PC." (_TurnedOnPC1Text)
            PcEntry::PokemonCenter => {
                screen.sfx.push(PcSfx::TurnOn);
                screen.set_message(
                    vec![format!("{} turned on", open.player_name), "the PC.".into()],
                    AfterMessage::MainMenu,
                );
            }
            // Direct access prints _TurnedOnPC2Text (same wording).
            PcEntry::PlayersPc => {
                screen.sfx.push(PcSfx::TurnOn);
                screen.set_message(
                    vec![format!("{} turned on", open.player_name), "the PC.".into()],
                    AfterMessage::ItemMenu,
                );
            }
            // TextScript_BillsPC goes straight into BillsPC_, which prints
            // "Switch on!" when not accessed through the generic PC.
            PcEntry::BillsPc => {
                screen.sfx.push(PcSfx::TurnOn);
                screen.set_message(vec!["Switch on!".into()], AfterMessage::BillsMenu);
            }
        }
        screen
    }

    // ── Accessors for the renderer ────────────────────────────────────────

    pub fn phase(&self) -> PcPhase {
        self.phase
    }
    pub fn entry(&self) -> PcEntry {
        self.entry
    }
    pub fn player_name(&self) -> &str {
        &self.player_name
    }
    pub fn message_lines(&self) -> &[String] {
        &self.msg_lines
    }
    pub fn message_page(&self) -> usize {
        self.msg_page
    }
    pub fn message_page_count(&self) -> usize {
        self.msg_lines.len().div_ceil(MSG_LINES_PER_PAGE).max(1)
    }
    pub fn main_menu(&self) -> &PcMainMenuState {
        &self.main_menu
    }
    pub fn bills_menu(&self) -> &BillsPcMenuState {
        &self.bills_menu
    }
    pub fn players_menu(&self) -> &PlayersPcMenuState {
        &self.players_menu
    }
    pub fn mon_mode(&self) -> MonListMode {
        self.mon_mode
    }
    pub fn mon_cursor(&self) -> usize {
        self.mon_cursor
    }
    pub fn mon_action_cursor(&self) -> usize {
        self.mon_action_cursor
    }
    pub fn yes_selected(&self) -> bool {
        self.yes_selected
    }
    pub fn box_cursor(&self) -> usize {
        self.box_cursor
    }
    pub fn item_mode(&self) -> ItemListMode {
        self.item_mode
    }
    pub fn item_list_cursor(&self) -> usize {
        self.item_list_cursor
    }
    pub fn item_list_scroll(&self) -> usize {
        self.item_list_scroll
    }
    pub fn item_qty(&self) -> u8 {
        self.item_qty
    }
    pub fn dex_seen(&self) -> u32 {
        self.dex_seen
    }
    pub fn dex_owned(&self) -> u32 {
        self.dex_owned
    }
    /// Display labels for the main menu, in order
    /// ("BILL's PC"/"SOMEONE's PC", "<NAME>'s PC", ...).
    pub fn main_menu_labels(&self) -> Vec<String> {
        self.main_menu
            .items()
            .iter()
            .map(|t| match t {
                PcMainMenuTarget::BillsPc => {
                    if self.met_bill {
                        "BILL's PC".to_string()
                    } else {
                        "SOMEONE's PC".to_string()
                    }
                }
                PcMainMenuTarget::PlayersPc => format!("{}'s PC", self.player_name),
                PcMainMenuTarget::OaksPc => "PROF.OAK's PC".to_string(),
                PcMainMenuTarget::PkmnLeague => "#MON LEAGUE".to_string(),
                PcMainMenuTarget::LogOff => "LOG OFF".to_string(),
            })
            .collect()
    }

    // ── Side-effect draining (app) ────────────────────────────────────────

    /// SFX queued since the last drain.
    pub fn take_sfx(&mut self) -> Vec<PcSfx> {
        core::mem::take(&mut self.sfx)
    }

    /// True once after CHANGE BOX switched boxes — the original saves the
    /// game (save.asm:396 `call SaveGameData`), so the app should persist.
    pub fn take_save_request(&mut self) -> bool {
        core::mem::replace(&mut self.save_requested, false)
    }

    /// Current HoF viewer page: the all-time team number ("HALL OF FAME
    /// No. X") and the mon on display. `LeagueHoF` phase only.
    pub fn league_hof_mon(&self) -> Option<(u8, &HofMonView)> {
        if self.phase != PcPhase::LeagueHoF {
            return None;
        }
        let team = self.hof_teams.get(self.league_team)?;
        let mon = team.mons.get(self.league_mon)?;
        Some((team.team_no, mon))
    }

    /// Viewer progress: (team index, team count) — for the renderer's
    /// optional "team X of Y" display.
    pub fn league_hof_progress(&self) -> (usize, usize) {
        (self.league_team, self.hof_teams.len())
    }

    /// Source text speed for the quantity PrintText call; list selection has
    /// cleared BIT_NO_TEXT_DELAY before this nine-character DONE question.
    pub fn set_quantity_text_delay_frames(&mut self, frames: u16) {
        self.quantity_text_delay_frames = frames.max(1);
    }

    pub fn quantity_prompt_chars(&self) -> usize {
        usize::from(self.quantity_prompt_chars)
    }

    // ── Internals ─────────────────────────────────────────────────────────

    fn set_message(&mut self, lines: Vec<String>, next: AfterMessage) {
        self.msg_lines = if self.language == crate::game_state::Lang::Zh {
            chinese_message_lines(&lines, &[&self.player_name])
        } else {
            lines
        };
        self.msg_page = 0;
        self.msg_next = next;
        self.phase = PcPhase::Message;
    }

    fn advance_message(&mut self) {
        if (self.msg_page + 1) * MSG_LINES_PER_PAGE < self.msg_lines.len() {
            self.msg_page += 1;
            return;
        }
        match self.msg_next {
            AfterMessage::MainMenu => self.enter_main_menu(),
            AfterMessage::BillsMenu => self.enter_bills_menu(),
            AfterMessage::ItemMenu => self.enter_item_menu(),
            AfterMessage::ItemList => {
                // players_pc.asm `jp .loop` — re-show the list where the
                // cursor was; update_item_list clamps it if the list shrank.
                self.begin_item_question(true, true);
            }
            // "Accessed PROF.OAK's PC..." → the YES/NO rating prompt.
            AfterMessage::OaksConfirmPage => self.enter_oaks_confirm(),
            // "#DEX completion is: ..." → the rating itself.
            AfterMessage::OaksRating => {
                let owned = self.dex_owned;
                let rating = DEX_RATINGS
                    .iter()
                    .find(|(threshold, _)| owned < *threshold)
                    .map(|(_, text)| *text)
                    .unwrap_or(DEX_RATINGS[DEX_RATINGS.len() - 1].1);
                let lines = rating.split('\n').map(|s| s.to_string()).collect();
                self.set_message(lines, AfterMessage::OaksClosed);
            }
            // Rating text → "Closed link to PROF.OAK's PC." (_ClosedOaksPCText)
            AfterMessage::OaksClosed => {
                self.set_message(
                    vec!["Closed link to".into(), "PROF.OAK's PC.".into()],
                    AfterMessage::MainMenu,
                );
            }
            // "Accessed the HALL OF FAME List." → the HoF team viewer
            // (PKMNLeaguePC's display loop, league_pc.asm:29-46). With no
            // recorded teams (unreachable through the menu, which gates on
            // wNumHoFTeams > 0) fall back to the main menu.
            AfterMessage::LeagueHoF => {
                if self.hof_teams.is_empty() {
                    self.enter_main_menu();
                } else {
                    self.league_team = 0;
                    self.league_mon = 0;
                    self.phase = PcPhase::LeagueHoF;
                    self.queue_league_mon_cry();
                }
            }
            AfterMessage::Exit => {
                self.phase = PcPhase::MainMenu; // placeholder; action is Exit
                self.msg_next = AfterMessage::Exit;
            }
        }
    }

    fn enter_main_menu(&mut self) {
        // DisplayPCMainMenu resets the cursor each visit (bills_pc.asm:82-83).
        self.main_menu = PcMainMenuState::new(self.has_pokedex, self.beaten_league, self.met_bill);
        self.phase = PcPhase::MainMenu;
    }

    fn enter_bills_menu(&mut self) {
        self.bills_menu.restore_saved_cursor();
        self.phase = PcPhase::BillsMenu;
    }

    fn enter_item_menu(&mut self) {
        self.players_menu.restore_saved_cursor();
        self.begin_item_question(false, false);
    }

    fn enter_item_list(&mut self, mode: ItemListMode) {
        self.item_mode = mode;
        self.item_list_cursor = 0;
        self.item_list_scroll = 0;
        self.begin_item_question(true, false);
    }

    fn begin_item_question(&mut self, list: bool, draw_list: bool) {
        self.item_question_is_list = list;
        self.item_question_draw_list = draw_list;
        self.item_question_no_delay = self.item_text_no_delay;
        self.quantity_prompt_intro = 3;
        self.quantity_prompt_chars = 0;
        self.quantity_prompt_letter_wait = 0;
        self.quantity_prompt_one_frame_wait = false;
        self.phase = PcPhase::ItemQuestion;
    }

    fn item_question_full_lines(&self) -> [&'static str; 2] {
        let list = self.phase == PcPhase::ItemList || (self.phase == PcPhase::ItemQuestion && self.item_question_is_list);
        let second = if !list { "to do?" } else { match self.item_mode {
            ItemListMode::Withdraw => "to withdraw?",
            ItemListMode::Deposit => "to deposit?",
            ItemListMode::Toss => "to toss away?",
        }};
        ["What do you want", second]
    }

    pub fn item_question_draw_list(&self) -> bool { self.item_question_draw_list }
    pub fn item_question_is_list(&self) -> bool { self.item_question_is_list }
    pub fn item_question_chars(&self) -> usize { usize::from(self.quantity_prompt_chars) }

    pub fn item_question_lines(&self) -> Vec<String> {
        let lines = self.item_question_full_lines();
        // Localized wording follows the existing PC translation path; the
        // protected controller clock counts authored source glyphs.
        if self.language == crate::game_state::Lang::Zh || self.phase != PcPhase::ItemQuestion {
            return lines.iter().map(|s| (*s).to_string()).collect();
        }
        let mut remaining = usize::from(self.quantity_prompt_chars);
        lines.iter().map(|line| {
            let count = remaining.min(line.chars().count());
            remaining = remaining.saturating_sub(count);
            line.chars().take(count).collect()
        }).collect()
    }

    /// B backs out of the top level of whichever PC we're in.
    fn exit_target(&self) -> AfterMessage {
        match self.entry {
            PcEntry::PokemonCenter => AfterMessage::MainMenu,
            // Direct access (bedroom / Bill's house) has no main menu above
            // it — leaving the sub-PC logs off entirely.
            PcEntry::PlayersPc | PcEntry::BillsPc => AfterMessage::Exit,
        }
    }

    // ── Frame update ──────────────────────────────────────────────────────

    /// No-audio hosts complete the cry boundary immediately.
    pub fn update_frame(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        self.update_frame_without_audio_with_text_input(input, ctx, input.a || input.b)
    }

    /// Preserve immediate cry completion for hosts without an audio backend.
    pub fn update_frame_without_audio_with_text_input(&mut self, input: MenuInput, ctx: &mut PcContext, held_ab: bool) -> PcScreenAction {
        let action = self.update_frame_with_text_input(input, ctx, false, held_ab);
        if self.waiting_for_cry {
            self.update_frame_with_text_input(MenuInput { up: false, down: false, a: false, b: false }, ctx, false, held_ab)
        } else { action }
    }

    pub fn waiting_for_sound(&self) -> bool { self.waiting_for_cry }

    fn start_mon_cry(&mut self, species: pokered_data::species::Species) -> bool {
        if self.finishing_cry { return false; }
        self.sfx.push(PcSfx::Cry(species));
        self.waiting_for_cry = true;
        true
    }

    pub fn update_frame_with_sound(&mut self, input: MenuInput, ctx: &mut PcContext, sound_playing: bool) -> PcScreenAction {
        self.update_frame_with_text_input(input, ctx, sound_playing, input.a || input.b)
    }

    /// Separate held typing input from newly pressed menu/transaction input.
    pub fn update_frame_with_text_input(&mut self, input: MenuInput, ctx: &mut PcContext, sound_playing: bool, held_ab: bool) -> PcScreenAction {
        // DisplayTwoOptionMenu holds its drawn cursor and ignores all input
        // for 15 frames before restoring the screen and returning the choice.
        if self.confirm_wait_frames > 0 {
            self.confirm_wait_frames -= 1;
            if self.confirm_wait_frames == 0 {
                let yes = self.pending_confirmation.take().expect("pending PC choice");
                self.yes_selected = yes;
                let accepted = MenuInput { up: false, down: false, a: yes, b: !yes };
                return match self.phase {
                    PcPhase::ReleaseConfirm => self.update_release_confirm(accepted, ctx),
                    PcPhase::ChangeBoxConfirm => self.update_change_box_confirm(accepted),
                    PcPhase::TossConfirm => self.update_toss_confirm(accepted, ctx),
                    PcPhase::OaksConfirm => self.update_oaks_confirm(accepted, ctx),
                    _ => unreachable!("protected PC confirmation phase"),
                };
            }
            return PcScreenAction::Continue;
        }
        if self.waiting_for_cry {
            if sound_playing { return PcScreenAction::Continue; }
            self.waiting_for_cry = false;
            self.finishing_cry = true;
            let confirm = MenuInput { up: false, down: false, a: true, b: false };
            let action = match self.phase {
                PcPhase::MonAction => self.update_mon_action(confirm, ctx),
                PcPhase::ReleaseConfirm => self.update_release_confirm(confirm, ctx),
                _ => PcScreenAction::Continue,
            };
            self.finishing_cry = false;
            return action;
        }
        match self.phase {
            PcPhase::Message => {
                if input.a || input.b {
                    let was_exit = self.msg_next == AfterMessage::Exit
                        && (self.msg_page + 1) * MSG_LINES_PER_PAGE >= self.msg_lines.len();
                    self.advance_message();
                    if was_exit {
                        self.sfx.push(PcSfx::TurnOff);
                        return PcScreenAction::Exit;
                    }
                }
                PcScreenAction::Continue
            }
            PcPhase::MainMenu => self.update_main_menu(input),
            PcPhase::BillsMenu => self.update_bills_menu(input, ctx),
            PcPhase::MonList => self.update_mon_list(input, ctx),
            PcPhase::MonAction => self.update_mon_action(input, ctx),
            PcPhase::ReleaseConfirm | PcPhase::ChangeBoxConfirm
            | PcPhase::TossConfirm | PcPhase::OaksConfirm => self.update_confirmation(input),
            PcPhase::BoxList => self.update_box_list(input, ctx),
            PcPhase::ItemMenu => self.update_item_menu(input, ctx),
            PcPhase::ItemList => self.update_item_list(input, ctx),
            PcPhase::ItemQuestion => self.update_item_question(held_ab),
            PcPhase::ItemQuantityPrompt => self.update_quantity_prompt(held_ab),
            PcPhase::ItemQuantity => self.update_item_quantity(input, ctx),
            PcPhase::LeagueHoF => self.update_league_hof(input),
        }
    }

    fn update_confirmation(&mut self, input: MenuInput) -> PcScreenAction {
        let drawn_yes = self.yes_selected;
        if input.up {
            self.yes_selected = true;
        } else if input.down {
            self.yes_selected = false;
        }
        if input.a || input.b {
            // HandleMenuInput changes the selected row before returning A/B,
            // but does not redraw it. Hold the old cursor even for UP+A,
            // DOWN+A or B; commit the separately frozen choice after 15 ticks.
            self.pending_confirmation = Some(!input.b && self.yes_selected);
            self.yes_selected = drawn_yes;
            self.confirm_wait_frames = 15;
        }
        PcScreenAction::Continue
    }

    /// HoF viewer (LeaguePCShowTeam, league_pc.asm:52-76): A advances to the
    /// next recorded mon (then the next team, then back to the main menu
    /// after the last one); B bails out of the whole viewer immediately.
    fn update_league_hof(&mut self, input: MenuInput) -> PcScreenAction {
        if input.b {
            self.enter_main_menu();
            return PcScreenAction::Continue;
        }
        if input.a {
            self.league_mon += 1;
            if self.league_mon >= self.hof_teams[self.league_team].mons.len() {
                self.league_mon = 0;
                self.league_team += 1;
                if self.league_team >= self.hof_teams.len() {
                    self.enter_main_menu();
                }
            }
            self.queue_league_mon_cry();
        }
        PcScreenAction::Continue
    }

    fn queue_league_mon_cry(&mut self) {
        if let Some((_, mon)) = self.league_hof_mon() {
            self.sfx.push(PcSfx::Cry(mon.species));
        }
    }

    fn update_main_menu(&mut self, input: MenuInput) -> PcScreenAction {
        match self.main_menu.update_frame(input) {
            None => PcScreenAction::Continue,
            Some(target) => match target {
                PcMainMenuTarget::LogOff => {
                    self.sfx.push(PcSfx::TurnOff);
                    PcScreenAction::Exit
                }
                PcMainMenuTarget::BillsPc => {
                    self.sfx.push(PcSfx::Enter);
                    // BillsPC_ resets wParentMenuItem, preserving the current box.
                    self.bills_menu = BillsPcMenuState::new(self.bills_menu.current_box());
                    // "Accessed BILL's PC. / Accessed #MON Storage System."
                    // (_AccessedBillsPCText / _AccessedSomeonesPCText)
                    let first = if self.met_bill {
                        "Accessed BILL's"
                    } else {
                        "Accessed someone's"
                    };
                    self.set_message(
                        vec![
                            first.into(),
                            "PC.".into(),
                            String::new(),
                            "Accessed #MON".into(),
                            "Storage System.".into(),
                        ],
                        AfterMessage::BillsMenu,
                    );
                    PcScreenAction::Continue
                }
                PcMainMenuTarget::PlayersPc => {
                    // PlayerPC starts a fresh local text context each entry.
                    self.item_text_no_delay = true;
                    self.sfx.push(PcSfx::Enter);
                    // PlayerPC resets wParentMenuItem on each fresh entry.
                    self.players_menu = PlayersPcMenuState::new();
                    // "Accessed my PC. / Accessed Item Storage System."
                    // (_AccessedMyPCText)
                    self.set_message(
                        vec![
                            "Accessed my PC.".into(),
                            String::new(),
                            "Accessed Item".into(),
                            "Storage System.".into(),
                        ],
                        AfterMessage::ItemMenu,
                    );
                    PcScreenAction::Continue
                }
                PcMainMenuTarget::OaksPc => {
                    self.sfx.push(PcSfx::Enter);
                    // "Accessed PROF.OAK's PC. / Accessed #DEX Rating System."
                    // (_AccessedOaksPCText)
                    self.set_message(
                        vec![
                            "Accessed PROF.".into(),
                            "OAK's PC.".into(),
                            String::new(),
                            "Accessed #DEX".into(),
                            "Rating System.".into(),
                        ],
                        AfterMessage::OaksConfirmPage,
                    );
                    PcScreenAction::Continue
                }
                PcMainMenuTarget::PkmnLeague => {
                    self.sfx.push(PcSfx::Enter);
                    // "Accessed #MON LEAGUE's site. / Accessed the HALL OF
                    // FAME List." (_AccessedHoFPCText) — then the HoF team
                    // viewer (PKMNLeaguePC, league_pc.asm:1-50).
                    self.set_message(
                        vec![
                            "Accessed #MON".into(),
                            "LEAGUE's site.".into(),
                            String::new(),
                            "Accessed the HALL".into(),
                            "OF FAME List.".into(),
                        ],
                        AfterMessage::LeagueHoF,
                    );
                    PcScreenAction::Continue
                }
            },
        }
    }

    fn update_bills_menu(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        self.bills_menu.set_current_box(ctx.pc_storage.current_box_index());
        match self.bills_menu.update_frame(input) {
            None => PcScreenAction::Continue,
            Some(action) => match action {
                BillsPcAction::Exit => match self.exit_target() {
                    AfterMessage::Exit => {
                        self.sfx.push(PcSfx::TurnOff);
                        PcScreenAction::Exit
                    }
                    _ => {
                        self.enter_main_menu();
                        PcScreenAction::Continue
                    }
                },
                BillsPcAction::Withdraw => {
                    // bills_pc.asm BillsPCWithdraw: box empty first, then
                    // party full.
                    if ctx.pc_storage.current_box().is_empty() {
                        // "What? There are no #MON here!" (_NoMonText)
                        self.set_message(
                            vec!["What? There are".into(), "no #MON here!".into()],
                            AfterMessage::BillsMenu,
                        );
                    } else if ctx.party.is_full() {
                        // "You can't take any more #MON. Deposit #MON first."
                        // (_CantTakeMonText)
                        self.set_message(
                            vec![
                                "You can't take".into(),
                                "any more #MON.".into(),
                                String::new(),
                                "Deposit #MON".into(),
                                "first.".into(),
                            ],
                            AfterMessage::BillsMenu,
                        );
                    } else {
                        self.mon_mode = MonListMode::Withdraw;
                        self.mon_cursor = 0;
                        self.phase = PcPhase::MonList;
                    }
                    PcScreenAction::Continue
                }
                BillsPcAction::Deposit => {
                    // bills_pc.asm BillsPCDeposit: party-of-one first, then
                    // box full. The original checks the raw party count — a
                    // fainted second mon still allows depositing.
                    if ctx.party.count() <= 1 {
                        // "You can't deposit the last #MON!"
                        // (_CantDepositLastMonText)
                        self.set_message(
                            vec!["You can't deposit".into(), "the last #MON!".into()],
                            AfterMessage::BillsMenu,
                        );
                    } else if ctx.pc_storage.current_box().is_full() {
                        // "Oops! This Box is full of #MON." (_BoxFullText)
                        self.set_message(
                            vec!["Oops! This Box is".into(), "full of #MON.".into()],
                            AfterMessage::BillsMenu,
                        );
                    } else {
                        self.mon_mode = MonListMode::Deposit;
                        self.mon_cursor = 0;
                        self.phase = PcPhase::MonList;
                    }
                    PcScreenAction::Continue
                }
                BillsPcAction::Release => {
                    if ctx.pc_storage.current_box().is_empty() {
                        self.set_message(
                            vec!["What? There are".into(), "no #MON here!".into()],
                            AfterMessage::BillsMenu,
                        );
                    } else {
                        self.mon_mode = MonListMode::Release;
                        self.mon_cursor = 0;
                        self.phase = PcPhase::MonList;
                    }
                    PcScreenAction::Continue
                }
                BillsPcAction::ChangeBox => {
                    // "When you change a #MON BOX, data will be saved. Is
                    // that okay?" (_WhenYouChangeBoxText) YES/NO.
                    self.yes_selected = true;
                    self.phase = PcPhase::ChangeBoxConfirm;
                    PcScreenAction::Continue
                }
            },
        }
    }

    /// Number of selectable rows in the current mon list (mons + CANCEL).
    fn mon_row_count(&self, ctx: &PcContext) -> usize {
        let mons = match self.mon_mode {
            MonListMode::Deposit => ctx.party.count(),
            MonListMode::Withdraw | MonListMode::Release => ctx.pc_storage.current_box().count(),
        };
        mons + 1
    }

    fn update_mon_list(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        let rows = self.mon_row_count(ctx);
        if input.up {
            self.mon_cursor = (self.mon_cursor + rows - 1) % rows;
        }
        if input.down {
            self.mon_cursor = (self.mon_cursor + 1) % rows;
        }
        if input.b {
            self.enter_bills_menu();
            return PcScreenAction::Continue;
        }
        if input.a {
            if self.mon_cursor == rows - 1 {
                // CANCEL row.
                self.enter_bills_menu();
                return PcScreenAction::Continue;
            }
            match self.mon_mode {
                MonListMode::Release => {
                    // bills_pc.asm BillsPCRelease: straight to the "gone
                    // forever" confirmation, no STATS popup.
                    self.yes_selected = true;
                    self.phase = PcPhase::ReleaseConfirm;
                }
                MonListMode::Withdraw | MonListMode::Deposit => {
                    self.mon_action_cursor = 0;
                    self.phase = PcPhase::MonAction;
                }
            }
        }
        PcScreenAction::Continue
    }

    fn update_mon_action(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        // Rows: WITHDRAW/DEPOSIT, STATS, CANCEL (DisplayDepositWithdrawMenu).
        if input.up {
            self.mon_action_cursor = (self.mon_action_cursor + 2) % 3;
        }
        if input.down {
            self.mon_action_cursor = (self.mon_action_cursor + 1) % 3;
        }
        if input.b {
            self.phase = PcPhase::MonList;
            return PcScreenAction::Continue;
        }
        if input.a {
            match self.mon_action_cursor {
                0 => {
                    let idx = self.mon_cursor;
                    match self.mon_mode {
                        MonListMode::Deposit => {
                            if let Some(mon) = ctx.party.get(idx) {
                                if self.start_mon_cry(mon.species) { return PcScreenAction::Continue; }
                            }
                            let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
                            let name = ctx
                                .party
                                .get(idx)
                                .map(|m| m.display_name(&mut name_buf))
                                .unwrap_or("");
                            if let Ok(mon) = ctx.party.remove(idx) {
                                let _ = ctx.pc_storage.current_box_mut().deposit(mon);
                                // "{NAME} was stored in Box {N}." (_MonWasStoredText)
                                let box_no = ctx.pc_storage.current_box_index() + 1;
                                self.set_message(
                                    vec![
                                        format!("{} was", name),
                                        format!("stored in Box {}.", box_no),
                                    ],
                                    AfterMessage::BillsMenu,
                                );
                            } else {
                                self.enter_bills_menu();
                            }
                        }
                        MonListMode::Withdraw => {
                            if !ctx.party.is_full() {
                                if let Some(mon) = ctx.pc_storage.current_box().get(idx) {
                                    if self.start_mon_cry(mon.species) { return PcScreenAction::Continue; }
                                }
                            }
                            let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
                            let name = ctx
                                .pc_storage
                                .current_box()
                                .get(idx)
                                .map(|m| m.display_name(&mut name_buf))
                                .unwrap_or("");
                            if ctx.party.is_full() {
                                // Party filled up between the menu check and
                                // here — leave the mon in the box.
                                self.enter_bills_menu();
                            } else if let Ok(mon) = ctx.pc_storage.current_box_mut().withdraw(idx)
                            {
                                let _ = ctx.party.add(mon);
                                // "{NAME} is taken out. Got {NAME}."
                                // (_MonIsTakenOutText)
                                self.set_message(
                                    vec![
                                        format!("{} is", name),
                                        "taken out.".into(),
                                        format!("Got {}", name),
                                        String::new(),
                                    ],
                                    AfterMessage::BillsMenu,
                                );
                            } else {
                                self.enter_bills_menu();
                            }
                        }
                        MonListMode::Release => unreachable!(),
                    }
                }
                1 => {
                    // STATS — app shows the stats screen and returns here.
                    let from_box = self.mon_mode == MonListMode::Withdraw;
                    return PcScreenAction::ShowStats {
                        from_box,
                        index: self.mon_cursor,
                    };
                }
                _ => {
                    self.phase = PcPhase::MonList;
                }
            }
        }
        PcScreenAction::Continue
    }

    fn update_release_confirm(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        // YesNoChoice starts on YES and clamps without wrapping. UP wins
        // when both directions are pressed (HandleMenuInput).
        if input.up {
            self.yes_selected = true;
        } else if input.down {
            self.yes_selected = false;
        }
        if input.b {
            // B == NO (YesNoChoice cursor on NO cancels).
            self.phase = PcPhase::MonList;
            return PcScreenAction::Continue;
        }
        if input.a {
            if self.yes_selected {
                let idx = self.mon_cursor;
                let species = ctx.pc_storage.current_box().get(idx).map(|mon| mon.species);
                let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
                let name = ctx
                    .pc_storage
                    .current_box()
                    .get(idx)
                    .map(|m| m.display_name(&mut name_buf))
                    .unwrap_or("");
                if ctx.pc_storage.current_box_mut().release(idx).is_ok() {
                    // BillsPCRelease prints its receipt while PlayCry is playing.
                    if let Some(species) = species { self.sfx.push(PcSfx::Cry(species)); }
                    // "{NAME} was released outside. Bye {NAME}!"
                    // (_MonWasReleasedText)
                    self.set_message(
                        vec![
                            format!("{} was", name),
                            "released outside.".into(),
                            format!("Bye {}!", name),
                        ],
                        AfterMessage::BillsMenu,
                    );
                } else {
                    self.enter_bills_menu();
                }
            } else {
                // NO → back to the list (bills_pc.asm:309 `jr nz, .loop`).
                self.phase = PcPhase::MonList;
            }
        }
        PcScreenAction::Continue
    }

    fn update_change_box_confirm(&mut self, input: MenuInput) -> PcScreenAction {
        // YesNoChoice starts on YES and clamps without wrapping. UP wins
        // when both directions are pressed (HandleMenuInput).
        if input.up {
            self.yes_selected = true;
        } else if input.down {
            self.yes_selected = false;
        }
        if input.b {
            self.enter_bills_menu();
            return PcScreenAction::Continue;
        }
        if input.a {
            if self.yes_selected {
                self.box_cursor = self.bills_menu.current_box();
                self.phase = PcPhase::BoxList;
            } else {
                self.enter_bills_menu();
            }
        }
        PcScreenAction::Continue
    }

    fn update_box_list(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        if input.up {
            self.box_cursor = (self.box_cursor + NUM_BOXES - 1) % NUM_BOXES;
        }
        if input.down {
            self.box_cursor = (self.box_cursor + 1) % NUM_BOXES;
        }
        if input.b {
            // save.asm ChangeBox:375 — B leaves without switching or saving.
            self.enter_bills_menu();
            return PcScreenAction::Continue;
        }
        if input.a {
            let _ = ctx.pc_storage.change_box(self.box_cursor);
            self.bills_menu.set_current_box(self.box_cursor);
            // The original copies the boxes around in SRAM and calls
            // SaveGameData (save.asm:377-401) with SFX_SAVE.
            self.save_requested = true;
            self.sfx.push(PcSfx::Save);
            self.enter_bills_menu();
        }
        PcScreenAction::Continue
    }

    fn update_item_menu(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        match self.players_menu.update_frame(input) {
            None => PcScreenAction::Continue,
            Some(action) => match action {
                PlayersPcAction::LogOff => match self.exit_target() {
                    AfterMessage::Exit => {
                        self.sfx.push(PcSfx::TurnOff);
                        PcScreenAction::Exit
                    }
                    _ => {
                        self.enter_main_menu();
                        PcScreenAction::Continue
                    }
                },
                PlayersPcAction::WithdrawItem => {
                    if ctx.pc_items.is_empty() {
                        // "There is nothing stored." (_NothingStoredText)
                        self.set_message(
                            vec!["There is nothing".into(), "stored.".into()],
                            AfterMessage::ItemMenu,
                        );
                    } else {
                        self.enter_item_list(ItemListMode::Withdraw);
                    }
                    PcScreenAction::Continue
                }
                PlayersPcAction::DepositItem => {
                    if ctx.bag.is_empty() {
                        // "You have nothing to deposit." (_NothingToDepositText)
                        self.set_message(
                            vec!["You have nothing".into(), "to deposit.".into()],
                            AfterMessage::ItemMenu,
                        );
                    } else {
                        self.enter_item_list(ItemListMode::Deposit);
                    }
                    PcScreenAction::Continue
                }
                PlayersPcAction::TossItem => {
                    if ctx.pc_items.is_empty() {
                        self.set_message(
                            vec!["There is nothing".into(), "stored.".into()],
                            AfterMessage::ItemMenu,
                        );
                    } else {
                        self.enter_item_list(ItemListMode::Toss);
                    }
                    PcScreenAction::Continue
                }
            },
        }
    }

    /// The inventory the current item list shows (bag for DEPOSIT, PC
    /// storage otherwise), as occupied slots in list order.
    fn item_source<'c>(&self, ctx: &'c PcContext) -> Vec<(ItemId, u8)> {
        let cap = MAX_ITEM_QUANTITY as u32;
        match self.item_mode {
            ItemListMode::Deposit => ctx
                .bag
                .items()
                .into_iter()
                .map(|(id, q)| (id, q.min(cap) as u8))
                .collect(),
            ItemListMode::Withdraw | ItemListMode::Toss => ctx
                .pc_items
                .items()
                .into_iter()
                .map(|(id, q)| (id, q.min(cap) as u8))
                .collect(),
        }
    }

    fn item_row_count(&self, ctx: &PcContext) -> usize {
        self.item_source(ctx).len() + 1 // + CANCEL
    }

    fn clamp_item_scroll(&mut self, ctx: &PcContext) {
        let rows = self.item_row_count(ctx);
        if self.item_list_cursor >= rows {
            self.item_list_cursor = rows - 1;
        }
        if self.item_list_cursor < self.item_list_scroll {
            self.item_list_scroll = self.item_list_cursor;
        } else if self.item_list_cursor >= self.item_list_scroll + PC_LIST_VISIBLE_ROWS {
            self.item_list_scroll = self.item_list_cursor + 1 - PC_LIST_VISIBLE_ROWS;
        }
        let max_scroll = rows.saturating_sub(PC_LIST_VISIBLE_ROWS);
        if self.item_list_scroll > max_scroll {
            self.item_list_scroll = max_scroll;
        }
    }

    fn update_item_list(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        let rows = self.item_row_count(ctx);
        if input.up && self.item_list_cursor > 0 {
            self.item_list_cursor -= 1;
        } else if input.down && self.item_list_cursor < rows - 1 {
            self.item_list_cursor += 1;
        }
        self.clamp_item_scroll(ctx);

        // DisplayListMenuID checks A before B, including simultaneous input.
        if input.b && !input.a {
            self.item_text_no_delay = false; // ExitListMenu
            self.enter_item_menu();
            return PcScreenAction::Continue;
        }
        if input.a {
            // Both storeChosenEntry and ExitListMenu clear NO_TEXT_DELAY.
            self.item_text_no_delay = false;
            if self.item_list_cursor == rows - 1 {
                // CANCEL row.
                self.enter_item_menu();
                return PcScreenAction::Continue;
            }
            let source = self.item_source(ctx);
            let Some((item, have)) = source.get(self.item_list_cursor) else {
                self.enter_item_menu();
                return PcScreenAction::Continue;
            };
            match self.item_mode {
                // Key items are unique: no "How many?" prompt, qty is 1
                // (players_pc.asm:110-121,164-175).
                ItemListMode::Deposit | ItemListMode::Withdraw => {
                    if item.is_key_item() {
                        self.exec_item_move(ctx, self.item_list_cursor, *item, 1);
                    } else {
                        let _ = have;
                        self.enter_quantity_prompt();
                    }
                }
                ItemListMode::Toss => {
                    if !is_tossable(*item) {
                        // HMs/key items: TossItem refuses outright
                        // (item_effects.asm:2550-2559).
                        self.set_message(
                            vec!["That's too impor-".into(), "tant to toss!".into()],
                            AfterMessage::ItemList,
                        );
                    } else {
                        self.enter_quantity_prompt();
                    }
                }
            }
        }
        PcScreenAction::Continue
    }

    fn enter_quantity_prompt(&mut self) {
        self.item_qty = 1;
        self.quantity_prompt_intro = 3;
        self.quantity_prompt_chars = 0;
        self.quantity_prompt_letter_wait = 0;
        self.quantity_prompt_one_frame_wait = false;
        self.phase = PcPhase::ItemQuantityPrompt;
    }

    fn update_quantity_prompt(&mut self, held_ab: bool) -> PcScreenAction {
        self.update_print_question(held_ab, 9, false, PcPhase::ItemQuantity)
    }

    fn update_item_question(&mut self, held_ab: bool) -> PcScreenAction {
        let count = self.item_question_full_lines().iter().map(|s| s.chars().count()).sum::<usize>() as u8;
        let target = if self.item_question_is_list { PcPhase::ItemList } else { PcPhase::ItemMenu };
        self.update_print_question(held_ab, count, self.item_question_no_delay, target)
    }

    fn update_print_question(&mut self, held_ab: bool, count: u8, no_delay: bool, target: PcPhase) -> PcScreenAction {
        if self.quantity_prompt_intro > 0 {
            self.quantity_prompt_intro -= 1;
            if self.quantity_prompt_intro > 0 { return PcScreenAction::Continue; }
        }
        if no_delay {
            self.quantity_prompt_chars = count;
            if target == PcPhase::ItemList { self.item_text_no_delay = true; }
            self.phase = target;
            return PcScreenAction::Continue;
        }
        if self.quantity_prompt_letter_wait > 0 {
            // After the final glyph, PrintLetterDelay samples A/B before
            // returning DONE. Even input on the normal expiry frame calls
            // DelayFrame once. An already scheduled short wait completes
            // without sampling the same held key again.
            if self.quantity_prompt_chars == count && held_ab && !self.quantity_prompt_one_frame_wait {
                self.quantity_prompt_letter_wait = 1;
                self.quantity_prompt_one_frame_wait = true;
                return PcScreenAction::Continue;
            }
            self.quantity_prompt_letter_wait -= 1;
            if self.quantity_prompt_letter_wait > 0 {
                // PrintLetterDelay's A/B exit still calls DelayFrame. The
                // next letter must appear on the following frame, not now.
                if held_ab {
                    self.quantity_prompt_letter_wait = 1;
                    self.quantity_prompt_one_frame_wait = true;
                }
                return PcScreenAction::Continue;
            }
            self.quantity_prompt_one_frame_wait = false;
        }
        if self.quantity_prompt_chars < count {
            self.quantity_prompt_chars += 1;
            self.quantity_prompt_letter_wait = if held_ab { 1 } else { self.quantity_text_delay_frames };
            self.quantity_prompt_one_frame_wait = held_ab;
            return PcScreenAction::Continue;
        }
        // DONE returns without acknowledgement. This typing input cannot also
        // choose a quantity or mutate inventory in the new menu.
        if target == PcPhase::ItemList { self.item_text_no_delay = true; }
        self.phase = target;
        PcScreenAction::Continue
    }

    fn update_item_quantity(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        let source = self.item_source(ctx);
        let Some((item, have)) = source.get(self.item_list_cursor) else {
            self.begin_item_question(true, true);
            return PcScreenAction::Continue;
        };
        // DisplayChooseQuantityMenu: A, B, UP, DOWN in that order;
        // quantities wrap between 1 and the selected stack's maximum.
        if !input.a && !input.b {
            if input.up {
                self.item_qty = if self.item_qty >= *have { 1 } else { self.item_qty + 1 };
            } else if input.down {
                self.item_qty = if self.item_qty <= 1 { *have } else { self.item_qty - 1 };
            }
        }
        if input.b && !input.a {
            self.begin_item_question(true, true);
            return PcScreenAction::Continue;
        }
        if input.a {
            let qty = self.item_qty;
            match self.item_mode {
                ItemListMode::Deposit | ItemListMode::Withdraw => {
                    self.exec_item_move(ctx, self.item_list_cursor, *item, qty);
                }
                ItemListMode::Toss => {
                    // "Is it OK to toss {ITEM}?" (_IsItOKToTossItemText) YES/NO.
                    self.yes_selected = true;
                    self.phase = PcPhase::TossConfirm;
                }
            }
        }
        PcScreenAction::Continue
    }

    /// Move `qty` of the item at `index` between bag and PC storage (either
    /// direction, per `item_mode`), printing the original's result text.
    fn exec_item_move(&mut self, ctx: &mut PcContext, index: usize, item: ItemId, qty: u8) {
        let name = item_name(item);
        match self.item_mode {
            ItemListMode::Deposit => {
                // players_pc.asm PlayerPCDeposit: try the PC first; only on
                // success is the bag slot decremented. The original's
                // AddItemToInventory is all-or-nothing, so trial-add a clone.
                let mut trial = ctx.pc_items.clone();
                if trial.add_item(item, qty).is_err() {
                    // "No room left to store items." (_NoRoomToStoreText)
                    self.set_message(
                        vec!["No room left to".into(), "store items.".into()],
                        AfterMessage::ItemList,
                    );
                    return;
                }
                core::mem::swap(ctx.pc_items, &mut trial);
                let _ = ctx.bag.remove_item_at(index, qty);
                self.sfx.push(PcSfx::WithdrawDeposit);
                // "{ITEM} was stored via PC." (_ItemWasStoredText)
                self.set_message(
                    vec![format!("{} was", name), "stored via PC.".into()],
                    AfterMessage::ItemList,
                );
            }
            ItemListMode::Withdraw => {
                let mut trial = ctx.bag.clone();
                if trial.add_item(item, qty).is_err() {
                    // "You can't carry any more items." (_CantCarryMoreText)
                    self.set_message(
                        vec!["You can't carry".into(), "any more items.".into()],
                        AfterMessage::ItemList,
                    );
                    return;
                }
                core::mem::swap(ctx.bag, &mut trial);
                let _ = ctx.pc_items.remove_item_at(index, qty);
                self.sfx.push(PcSfx::WithdrawDeposit);
                // "Withdrew {ITEM}." (_WithdrewItemText)
                self.set_message(
                    vec![format!("Withdrew"), format!("{}.", name)],
                    AfterMessage::ItemList,
                );
            }
            ItemListMode::Toss => unreachable!(),
        }
    }

    fn update_toss_confirm(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        // YesNoChoice starts on YES and clamps without wrapping. UP wins
        // when both directions are pressed (HandleMenuInput).
        if input.up {
            self.yes_selected = true;
        } else if input.down {
            self.yes_selected = false;
        }
        if input.b {
            self.begin_item_question(true, true);
            return PcScreenAction::Continue;
        }
        if input.a {
            if self.yes_selected {
                let idx = self.item_list_cursor;
                let qty = self.item_qty;
                let name = self
                    .item_source(ctx)
                    .get(idx)
                    .map(|(item, _)| item_name(*item))
                    .unwrap_or_default();
                let _ = ctx.pc_items.remove_item_at(idx, qty);
                // "Threw away {ITEM}." (_ThrewAwayItemText)
                self.set_message(
                    vec!["Threw away".into(), format!("{}.", name)],
                    AfterMessage::ItemList,
                );
            } else {
                self.begin_item_question(true, true);
            }
        }
        PcScreenAction::Continue
    }

    fn update_oaks_confirm(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        // YesNoChoice starts on YES and clamps without wrapping. UP wins
        // when both directions are pressed (HandleMenuInput).
        if input.up {
            self.yes_selected = true;
        } else if input.down {
            self.yes_selected = false;
        }
        if input.b {
            // B on the YES/NO counts as NO (YesNoChoice) → close the link.
            self.set_message(
                vec!["Closed link to".into(), "PROF.OAK's PC.".into()],
                AfterMessage::MainMenu,
            );
            return PcScreenAction::Continue;
        }
        if input.a {
            if self.yes_selected {
                // DisplayDexRating: "#DEX completion is: N #MON seen, M #MON
                // owned. PROF.OAK's Rating:" then the rating text.
                self.dex_seen = ctx.pokedex.seen_count();
                self.dex_owned = ctx.pokedex.owned_count();
                self.set_message(
                    vec![
                        "#DEX comp-".into(),
                        "letion is:".into(),
                        String::new(),
                        format!("{} #MON seen", self.dex_seen),
                        format!("{} #MON owned", self.dex_owned),
                        String::new(),
                        "PROF.OAK's".into(),
                        "Rating:".into(),
                    ],
                    AfterMessage::OaksRating,
                );
            } else {
                self.set_message(
                    vec!["Closed link to".into(), "PROF.OAK's PC.".into()],
                    AfterMessage::MainMenu,
                );
            }
        }
        PcScreenAction::Continue
    }
}

/// Item display name for messages (EN, matching the original's texts).
fn item_name(item: ItemId) -> String {
    pokered_data::item_data::get_item_data(item)
        .map(|d| d.name.to_string())
        .unwrap_or_else(|| "???".to_string())
}

/// Label for a mon-list mode's action popup row 0 ("WITHDRAW"/"DEPOSIT").
pub fn mon_action_label(mode: MonListMode) -> &'static str {
    match mode {
        MonListMode::Withdraw => "WITHDRAW",
        MonListMode::Deposit => "DEPOSIT",
        MonListMode::Release => "RELEASE",
    }
}

// AfterMessage extension: the "Accessed PROF.OAK's PC..." page leads into the
// YES/NO "Want to get your #DEX rated?" prompt (oaks_pc.asm:5-7).
impl PcScreen {
    fn enter_oaks_confirm(&mut self) {
        self.yes_selected = true;
        self.phase = PcPhase::OaksConfirm;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fresh_bills_pc_entry_resets_cursor_and_preserves_current_box() {
        let mut w = World::new();
        w.pc_storage.change_box(2).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s,&mut w);
        s.bills_menu.set_current_box(2);
        for _ in 0..4 { s.update_frame(DOWN,&mut w.ctx()); }
        assert_eq!(s.bills_menu().cursor(),4);
        s.update_frame(A,&mut w.ctx()); assert_eq!(s.phase(),PcPhase::MainMenu);
        s.update_frame(A,&mut w.ctx()); skip_message(&mut s,&mut w);
        assert_eq!(s.phase(),PcPhase::BillsMenu);
        assert_eq!(s.bills_menu().cursor(),0,"fresh BillsPC_ starts at WITHDRAW PKMN");
        assert_eq!(s.bills_menu().current_box(),2,"fresh menu does not change the selected box");
        assert!(!s.take_save_request());
    }

    #[test]
    fn fresh_player_pc_entry_resets_cursor_without_resetting_list_return() {
        fn settle(s: &mut PcScreen, w: &mut World) {
            skip_message(s, w);
            for _ in 0..200 {
                if format!("{:?}", s.phase()) != "ItemQuestion" { break; }
                s.update_frame(MenuInput { a:false, b:false, up:false, down:false }, &mut w.ctx());
            }
            assert_eq!(s.phase(), PcPhase::ItemMenu);
        }
        for cursor in [1, 2] {
            let mut w = World::new();
            w.bag.add_item(ItemId::Potion,4).unwrap();
            w.pc_items.add_item(ItemId::Potion,4).unwrap();
            let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
            skip_message(&mut s,&mut w);
            s.update_frame(DOWN,&mut w.ctx()); s.update_frame(A,&mut w.ctx()); settle(&mut s,&mut w);
            for _ in 0..cursor { s.update_frame(DOWN,&mut w.ctx()); }
            s.update_frame(A,&mut w.ctx());
            for _ in 0..200 {
                if s.phase()==PcPhase::ItemList { break; }
                s.update_frame(MenuInput { a:false,b:false,up:false,down:false },&mut w.ctx());
            }
            assert_eq!(s.phase(),PcPhase::ItemList);
            s.update_frame(B,&mut w.ctx()); settle(&mut s,&mut w);
            assert_eq!(s.players_menu().cursor(),cursor,"return within player PC preserves chosen operation");
            s.update_frame(B,&mut w.ctx()); assert_eq!(s.phase(),PcPhase::MainMenu);
            s.update_frame(DOWN,&mut w.ctx()); s.update_frame(A,&mut w.ctx()); settle(&mut s,&mut w);
            assert_eq!(s.players_menu().cursor(),0,"fresh PlayerPC starts at WITHDRAW ITEM");
            assert_eq!(w.bag.item_quantity(ItemId::Potion),4);
            assert_eq!(w.pc_items.item_quantity(ItemId::Potion),4);
        }
    }

    use super::*;
    use crate::pokemon::stats::create_pokemon;
    use pokered_data::species::Species;

    fn mon(species: Species, level: u8) -> crate::battle::state::Pokemon {
        create_pokemon(species, level, [0x9A, 0x78]).unwrap()
    }

    struct World {
        party: Party,
        pc_storage: PcStorage,
        bag: Inventory<BAG_ITEM_CAPACITY>,
        pc_items: Inventory<PC_ITEM_CAPACITY>,
        pokedex: Pokedex,
    }

    impl World {
        fn new() -> Self {
            Self {
                party: Party::new(),
                pc_storage: PcStorage::new(),
                bag: Inventory::new_bag(),
                pc_items: Inventory::new_pc(),
                pokedex: Pokedex::new(),
            }
        }

        fn ctx(&mut self) -> PcContext<'_> {
            PcContext {
                party: &mut self.party,
                pc_storage: &mut self.pc_storage,
                bag: &mut self.bag,
                pc_items: &mut self.pc_items,
                pokedex: &self.pokedex,
            }
        }
    }

    #[test]
    fn player_questions_match_initial_and_return_source_clocks_at_all_speeds() {
        for delay in [1u16, 3, 5] {
            for mode in [ItemListMode::Withdraw, ItemListMode::Deposit, ItemListMode::Toss] {
                for (list, no_delay) in [(false,true),(true,true),(true,false),(false,false)] {
                    let mut w = World::new();
                    w.bag.add_item(ItemId::Potion,4).unwrap();
                    w.pc_items.add_item(ItemId::Potion,4).unwrap();
                    let mut s = PcScreen::new(PcEntry::PlayersPc, &open_ctx());
                    s.set_quantity_text_delay_frames(delay);
                    s.item_mode = mode;
                    s.item_text_no_delay = no_delay;
                    s.begin_item_question(list, list && !no_delay);
                    let count = if !list {22} else {match mode {ItemListMode::Withdraw=>28,ItemListMode::Deposit=>27,ItemListMode::Toss=>29}};
                    let ready = if no_delay {3} else {3+count*usize::from(delay)};
                    for t in 0..=ready {
                        if t>0 {s.update_frame(MenuInput {a:false,b:false,up:false,down:true},&mut w.ctx());}
                        assert_eq!(s.phase(),if t<ready {PcPhase::ItemQuestion}else if list {PcPhase::ItemList}else{PcPhase::ItemMenu},"{mode:?} {delay} list{list} no_delay{no_delay} frame{t}");
                        let chars = if t<3 {0} else if no_delay {count} else {((t-3)/usize::from(delay)+1).min(count)};
                        assert_eq!(s.item_question_chars(),chars,"source glyph frame{t}");
                        assert_eq!(s.item_list_cursor,0,"typing cannot move cursor");
                        assert_eq!(w.bag.item_quantity(ItemId::Potion),4);
                        assert_eq!(w.pc_items.item_quantity(ItemId::Potion),4);
                    }
                    assert_eq!(s.item_text_no_delay,if list {true}else{no_delay});
                }
            }
        }
    }

    #[test]
    fn straight_list_cancel_clears_no_delay_and_reprints_operation_question() {
        let mut w=World::new();w.pc_items.add_item(ItemId::Potion,4).unwrap();
        let mut s=PcScreen::new(PcEntry::PlayersPc,&open_ctx());
        s.set_quantity_text_delay_frames(5);
        s.advance_message();assert_eq!(s.phase(),PcPhase::ItemQuestion);
        for _ in 0..3 {s.update_frame(NONE,&mut w.ctx());}
        assert_eq!(s.phase(),PcPhase::ItemMenu);
        s.update_frame(A,&mut w.ctx());assert_eq!(s.phase(),PcPhase::ItemQuestion);
        for _ in 0..3 {s.update_frame(NONE,&mut w.ctx());}
        assert_eq!(s.phase(),PcPhase::ItemList);assert!(s.item_text_no_delay);
        s.update_frame(B,&mut w.ctx());assert_eq!(s.phase(),PcPhase::ItemQuestion);assert!(!s.item_text_no_delay);
        for t in 1..=113 {s.update_frame(NONE,&mut w.ctx());assert_eq!(s.phase(),if t<113 {PcPhase::ItemQuestion}else{PcPhase::ItemMenu});}
        assert_eq!(w.pc_items.item_quantity(ItemId::Potion),4);
    }

    #[test]
    fn confirmations_hold_fifteen_frames_ignore_input_and_commit_once() {
        for phase in [PcPhase::ReleaseConfirm, PcPhase::ChangeBoxConfirm, PcPhase::TossConfirm, PcPhase::OaksConfirm] {
            for (selected_yes, cancel) in [(true, false), (false, false), (true, true)] {
                let mut w = World::new();
                w.pc_storage.current_box_mut().deposit(mon(Species::Abra, 10)).unwrap();
                w.pc_items.add_item(ItemId::Potion, 4).unwrap();
                let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
                s.take_sfx();
                s.phase = phase;
                s.yes_selected = selected_yes;
                s.update_frame(MenuInput { a: true, b: cancel, up: false, down: false }, &mut w.ctx());
                let yes = selected_yes && !cancel;
                assert_eq!(s.pending_confirmation, Some(yes));
                for frame in 1..=15 {
                    s.update_frame(MenuInput { a: true, b: true, up: true, down: true }, &mut w.ctx());
                    assert_eq!(s.confirm_wait_frames, 15-frame);
                    if frame<15 {assert_eq!(s.yes_selected, selected_yes, "drawn cursor held even for B");}
                    else {assert_eq!(s.yes_selected,yes);}
                    assert_eq!(w.pc_storage.current_box().count(), if phase==PcPhase::ReleaseConfirm && yes && frame==15 {0}else{1});
                    assert_eq!(w.pc_items.item_quantity(ItemId::Potion), if phase==PcPhase::TossConfirm && yes && frame==15 {3}else{4});
                    if frame<15 {assert_eq!(s.phase(),phase);assert!(s.take_sfx().is_empty());}
                }
                assert!(s.pending_confirmation.is_none());
                assert_ne!(s.phase(),phase);
                let stock=(w.pc_storage.current_box().count(),w.pc_items.item_quantity(ItemId::Potion));
                s.update_frame(MenuInput { a:false,b:false,up:false,down:false }, &mut w.ctx());
                assert_eq!((w.pc_storage.current_box().count(),w.pc_items.item_quantity(ItemId::Potion)),stock);
            }
        }
    }

    #[test]
    fn quantity_question_matches_recorded_source_clocks_and_protects_stock() {
        for mode in [ItemListMode::Withdraw, ItemListMode::Deposit, ItemListMode::Toss] {
            for delay in [1, 3, 5] {
                if mode != ItemListMode::Withdraw && delay != 5 { continue; }
                for early in [None, Some(A), Some(B)] {
                    if delay != 5 && early.is_some() { continue; }
                    let mut w = World::new();
                    w.bag.add_item(ItemId::Potion,4).unwrap();
                    w.pc_items.add_item(ItemId::Potion,4).unwrap();
                    let mut s = PcScreen::new(PcEntry::PlayersPc,&open_ctx());
                    s.set_quantity_text_delay_frames(delay);
                    s.item_mode = mode;
                    s.phase = PcPhase::ItemList;
                    s.update_frame_with_text_input(A,&mut w.ctx(),false,true);
                    assert_eq!(s.phase(),PcPhase::ItemQuantityPrompt);
                    let boundary = if early.is_some() {42} else {3+9*delay};
                    for frame in 1..=boundary {
                        let input = if frame==10 {early.unwrap_or(NONE)}else{NONE};
                        let held_ab = early.is_some() && (frame==10 || frame==11);
                        s.update_frame_with_text_input(input,&mut w.ctx(),false,held_ab);
                        assert_eq!(s.phase(),if frame<boundary {PcPhase::ItemQuantityPrompt}else{PcPhase::ItemQuantity},"mode{mode:?} delay{delay} frame{frame}");
                        assert_eq!(s.item_qty(),1);
                        assert_eq!(w.bag.item_quantity(ItemId::Potion),4);
                        assert_eq!(w.pc_items.item_quantity(ItemId::Potion),4);
                    }
                    // A fresh press in the quantity menu commits exactly once;
                    // the earlier typing input never consumed a stock item.
                    s.update_frame(A,&mut w.ctx());
                    match mode {
                        ItemListMode::Withdraw=>{assert_eq!(w.bag.item_quantity(ItemId::Potion),5);assert_eq!(w.pc_items.item_quantity(ItemId::Potion),3);},
                        ItemListMode::Deposit=>{assert_eq!(w.bag.item_quantity(ItemId::Potion),3);assert_eq!(w.pc_items.item_quantity(ItemId::Potion),5);},
                        ItemListMode::Toss=>{assert_eq!(s.phase(),PcPhase::TossConfirm);assert_eq!(w.bag.item_quantity(ItemId::Potion),4);assert_eq!(w.pc_items.item_quantity(ItemId::Potion),4);},
                    }
                }
            }
        }
    }

    #[test]
    fn quantity_question_matches_source_letter_and_done_boundary_inputs() {
        for (offset,boundary,glyphs) in [
            (3,40,vec![3,4,5,10,15,20,25,30,35]),
            (8,40,vec![3,8,9,10,15,20,25,30,35]),
            (47,48,vec![3,8,13,18,23,28,33,38,43]),
            (48,49,vec![3,8,13,18,23,28,33,38,43]),
        ] {
            for key in [A,B] {
                let mut w=World::new();
                w.bag.add_item(ItemId::Potion,4).unwrap();
                w.pc_items.add_item(ItemId::Potion,4).unwrap();
                let mut s=PcScreen::new(PcEntry::PlayersPc,&open_ctx());
                s.set_quantity_text_delay_frames(5);
                s.phase=PcPhase::ItemList;
                s.update_frame_with_text_input(A,&mut w.ctx(),false,true);
                let mut observed=Vec::new();let mut chars=0;
                for frame in 1..=boundary {
                    s.update_frame_with_text_input(if frame==offset {key}else{NONE},&mut w.ctx(),false,frame==offset || frame==offset+1);
                    if s.quantity_prompt_chars()!=chars {observed.push(frame);chars=s.quantity_prompt_chars();}
                    assert_eq!(s.phase(),if frame<boundary {PcPhase::ItemQuantityPrompt}else{PcPhase::ItemQuantity});
                    assert_eq!(w.bag.item_quantity(ItemId::Potion),4);
                    assert_eq!(w.pc_items.item_quantity(ItemId::Potion),4);
                }
                assert_eq!(observed,glyphs,"source offset{offset} A{}",key.a);
                s.update_frame(A,&mut w.ctx());
                assert_eq!(w.bag.item_quantity(ItemId::Potion),5);
                assert_eq!(w.pc_items.item_quantity(ItemId::Potion),3);
            }
        }
    }

    fn finish_quantity_prompt(s: &mut PcScreen, w: &mut World) {
        assert_eq!(s.phase(),PcPhase::ItemQuantityPrompt);
        for _ in 0..30 {s.update_frame(NONE,&mut w.ctx());}
        assert_eq!(s.phase(),PcPhase::ItemQuantity);
    }

    #[test]
    fn source_quantity_pc_wraps_both_boundaries_without_moving_stock() {
        for mode in [ItemListMode::Withdraw, ItemListMode::Deposit, ItemListMode::Toss] {
            for have in [1, 4, 99] {
                let mut w = World::new();
                w.bag.add_item(ItemId::Potion, have).unwrap();
                w.pc_items.add_item(ItemId::Potion, have).unwrap();
                let mut s = PcScreen::new(PcEntry::PlayersPc, &open_ctx());
                s.item_mode = mode;
                s.phase = PcPhase::ItemQuantity;
                s.item_qty = 1;
                s.update_frame(DOWN, &mut w.ctx());
                assert_eq!(s.item_qty(), have, "DOWN from1 mode{mode:?}");
                s.update_frame(UP, &mut w.ctx());
                assert_eq!(s.item_qty(), 1, "UP frommax mode{mode:?}");
                s.update_frame(B, &mut w.ctx());
                assert_eq!(w.bag.item_quantity(ItemId::Potion), u16::from(have));
                assert_eq!(w.pc_items.item_quantity(ItemId::Potion), u16::from(have));
            }
        }
    }

    #[test]
    fn source_quantity_pc_confirm_precedes_cancel_and_directions() {
        for mode in [ItemListMode::Withdraw, ItemListMode::Deposit, ItemListMode::Toss] {
            for b in [false, true] {
                let mut w = World::new();
                w.bag.add_item(ItemId::Potion, 4).unwrap();
                w.pc_items.add_item(ItemId::Potion, 4).unwrap();
                let mut s = PcScreen::new(PcEntry::PlayersPc, &open_ctx());
                s.item_mode = mode;
                s.phase = PcPhase::ItemQuantity;
                s.item_qty = 1;
                s.update_frame(MenuInput { a: true, b, up: true, down: true }, &mut w.ctx());
                assert_eq!(s.item_qty(), 1, "A keeps chosen quantity mode{mode:?} B{b}");
                assert_eq!(s.phase(), if mode == ItemListMode::Toss {
                    PcPhase::TossConfirm
                } else { PcPhase::Message });
                match mode {
                    ItemListMode::Withdraw => {
                        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 3);
                        assert_eq!(w.bag.item_quantity(ItemId::Potion), 5);
                    }
                    ItemListMode::Deposit => {
                        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 5);
                        assert_eq!(w.bag.item_quantity(ItemId::Potion), 3);
                    }
                    ItemListMode::Toss => {
                        // This quantity test checks that confirmation precedes
                        // stock mutation, independent of the later YES/NO menu.
                        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 4);
                        assert_eq!(w.bag.item_quantity(ItemId::Potion), 4);
                    }
                }
            }
        }
    }

    fn open_ctx() -> PcOpenContext {
        PcOpenContext {
            has_pokedex: false,
            met_bill: false,
            beaten_league: false,
            player_name: "RED".into(),
            hof_teams: Vec::new(),
        }
    }

    fn hof_open_ctx() -> PcOpenContext {
        let team = |no: u8, n_mons: usize| HofTeamRecord {
            team_no: no,
            mons: (0..n_mons)
                .map(|i| HofMonView {
                    species: Species::Pikachu,
                    level: 50 + i as u8,
                    nickname: format!("MON{}", i),
                })
                .collect(),
        };
        PcOpenContext {
            beaten_league: true,
            hof_teams: vec![team(1, 2), team(2, 1)],
            ..open_ctx()
        }
    }

    const NONE: MenuInput = MenuInput {
        up: false,
        down: false,
        a: false,
        b: false,
    };
    const A: MenuInput = MenuInput {
        up: false,
        down: false,
        a: true,
        b: false,
    };
    const B: MenuInput = MenuInput {
        up: false,
        down: false,
        a: false,
        b: true,
    };
    const UP: MenuInput = MenuInput {
        up: true,
        down: false,
        a: false,
        b: false,
    };
    const DOWN: MenuInput = MenuInput {
        up: false,
        down: true,
        a: false,
        b: false,
    };

    fn finish_confirmation_wait(screen: &mut PcScreen, w: &mut World) {
        assert_eq!(screen.confirm_wait_frames, 15);
        assert!(screen.pending_confirmation.is_some());
        for _ in 0..15 {
            screen.update_frame(MenuInput { up: false, down: false, a: false, b: false }, &mut w.ctx());
        }
        assert_eq!(screen.confirm_wait_frames, 0);
        assert!(screen.pending_confirmation.is_none());
    }

    // Ready-menu fixtures wait with empty input; source clock tests above
    // inspect every intermediate frame without this helper.
    fn finish_item_question(screen: &mut PcScreen, w: &mut World) {
        let bag = w.bag.items().to_vec();
        let pc = w.pc_items.items().to_vec();
        for _ in 0..200 {
            if screen.phase() != PcPhase::ItemQuestion { break; }
            screen.update_frame(NONE, &mut w.ctx());
        }
        assert_ne!(screen.phase(), PcPhase::ItemQuestion);
        assert_eq!(w.bag.items(), bag);
        assert_eq!(w.pc_items.items(), pc);
    }

    /// Advance through every page of the current message.
    fn skip_message(screen: &mut PcScreen, w: &mut World) {
        while screen.phase() == PcPhase::Message {
            assert_eq!(
                screen.update_frame(A, &mut w.ctx()),
                PcScreenAction::Continue
            );
        }
        finish_item_question(screen, w);
    }

    fn open_pokemon_center(screen: &mut PcScreen, w: &mut World) {
        assert_eq!(screen.phase(), PcPhase::Message);
        skip_message(screen, w);
        assert_eq!(screen.phase(), PcPhase::MainMenu);
        // Drain boot SFX so tests can assert per-action queues.
        screen.take_sfx();
    }

    fn open_bills_pc(screen: &mut PcScreen, w: &mut World) {
        open_pokemon_center(screen, w);
        // Cursor 0 = BILL's/SOMEONE's PC.
        assert_eq!(
            screen.update_frame(A, &mut w.ctx()),
            PcScreenAction::Continue
        );
        skip_message(screen, w);
        assert_eq!(screen.phase(), PcPhase::BillsMenu);
        screen.take_sfx();
    }

    // ── Entry / main menu ────────────────────────────────────────────────

    #[test]
    fn pokecenter_main_menu_without_pokedex_or_league() {
        let mut w = World::new();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        assert_eq!(s.take_sfx(), vec![PcSfx::TurnOn]);
        open_pokemon_center(&mut s, &mut w);
        // No Pokédex, no league: BILL's (not met Bill), RED's PC, LOG OFF.
        assert_eq!(
            s.main_menu_labels(),
            vec!["SOMEONE's PC", "RED's PC", "LOG OFF"]
        );
    }

    #[test]
    fn pokecenter_main_menu_with_pokedex_met_bill_and_league() {
        let mut w = World::new();
        let open = PcOpenContext {
            has_pokedex: true,
            met_bill: true,
            beaten_league: true,
            ..open_ctx()
        };
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open);
        open_pokemon_center(&mut s, &mut w);
        assert_eq!(
            s.main_menu_labels(),
            vec!["BILL's PC", "RED's PC", "PROF.OAK's PC", "#MON LEAGUE", "LOG OFF"]
        );
    }

    #[test]
    fn main_menu_b_logs_off() {
        let mut w = World::new();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_pokemon_center(&mut s, &mut w);
        assert_eq!(s.update_frame(B, &mut w.ctx()), PcScreenAction::Exit);
        assert_eq!(s.take_sfx(), vec![PcSfx::TurnOff]);
    }

    #[test]
    fn main_menu_log_off_item_exits() {
        let mut w = World::new();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_pokemon_center(&mut s, &mut w);
        // 3 items: down, down → LOG OFF.
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(DOWN, &mut w.ctx());
        assert_eq!(s.update_frame(A, &mut w.ctx()), PcScreenAction::Exit);
    }

    // ── Bill's PC: deposit ───────────────────────────────────────────────

    #[test]
    fn deposit_last_mon_forbidden() {
        // bills_pc.asm BillsPCDeposit: `wPartyCount - 1 == 0` refuses.
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 5)).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        // Cursor 1 = DEPOSIT #MON.
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::Message);
        assert_eq!(
            s.message_lines(),
            &["You can't deposit".to_string(), "the last #MON!".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::BillsMenu);
        assert_eq!(w.party.count(), 1);
        assert_eq!(w.pc_storage.current_box().count(), 0);
    }

    #[test]
    fn deposit_box_full_forbidden() {
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 5)).unwrap();
        w.party.add(mon(Species::Bulbasaur, 5)).unwrap();
        for _ in 0..20 {
            w.pc_storage
                .current_box_mut()
                .deposit(mon(Species::Bulbasaur, 3))
                .unwrap();
        }
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        assert_eq!(
            s.message_lines(),
            &["Oops! This Box is".to_string(), "full of #MON.".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(w.party.count(), 2);
        assert_eq!(w.pc_storage.current_box().count(), 20);
    }

    #[test]
    fn deposit_moves_mon_from_party_to_box() {
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 12)).unwrap();
        w.party.add(mon(Species::Bulbasaur, 7)).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx()); // DEPOSIT
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::MonList);
        // List shows the party: pick PIKACHU (cursor 0) → action popup.
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::MonAction);
        // Row 0 = DEPOSIT.
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::Message);
        assert_eq!(
            s.message_lines(),
            &[
                "PIKACHU was".to_string(),
                "stored in Box 1.".to_string()
            ]
        );
        assert_eq!(s.take_sfx(), vec![PcSfx::Cry(Species::Pikachu)]);
        skip_message(&mut s, &mut w);
        assert_eq!(w.party.count(), 1);
        assert_eq!(w.pc_storage.current_box().count(), 1);
        assert_eq!(
            w.pc_storage.current_box().get(0).unwrap().species,
            Species::Pikachu
        );
    }

    // ── Bill's PC: withdraw ──────────────────────────────────────────────

    #[test]
    fn withdraw_from_empty_box_forbidden() {
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 5)).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        // Cursor 0 = WITHDRAW.
        s.update_frame(A, &mut w.ctx());
        assert_eq!(
            s.message_lines(),
            &["What? There are".to_string(), "no #MON here!".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::BillsMenu);
    }

    #[test]
    fn withdraw_with_full_party_forbidden() {
        let mut w = World::new();
        for _ in 0..6 {
            w.party.add(mon(Species::Pikachu, 5)).unwrap();
        }
        w.pc_storage
            .current_box_mut()
            .deposit(mon(Species::Bulbasaur, 5))
            .unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        s.update_frame(A, &mut w.ctx()); // WITHDRAW
        assert_eq!(
            s.message_lines()[0],
            "You can't take".to_string()
        );
        skip_message(&mut s, &mut w);
        assert_eq!(w.pc_storage.current_box().count(), 1);
    }

    #[test]
    fn withdraw_moves_mon_from_box_to_party() {
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 5)).unwrap();
        w.pc_storage
            .current_box_mut()
            .deposit(mon(Species::Bulbasaur, 9))
            .unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        s.update_frame(A, &mut w.ctx()); // WITHDRAW
        assert_eq!(s.phase(), PcPhase::MonList);
        s.update_frame(A, &mut w.ctx()); // pick BULBASAUR
        assert_eq!(s.phase(), PcPhase::MonAction);
        s.update_frame(A, &mut w.ctx()); // WITHDRAW
        assert_eq!(
            s.message_lines(),
            &[
                "BULBASAUR is".to_string(),
                "taken out.".to_string(),
                "Got BULBASAUR".to_string(),
                String::new(),
            ]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(w.party.count(), 2);
        assert_eq!(w.pc_storage.current_box().count(), 0);
        assert_eq!(w.party.get(1).unwrap().species, Species::Bulbasaur);
    }

    #[test]
    fn stats_action_reports_source_and_index() {
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 5)).unwrap();
        w.pc_storage
            .current_box_mut()
            .deposit(mon(Species::Bulbasaur, 9))
            .unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        s.update_frame(A, &mut w.ctx()); // WITHDRAW
        s.update_frame(A, &mut w.ctx()); // pick mon
        s.update_frame(DOWN, &mut w.ctx()); // → STATS
        let action = s.update_frame(A, &mut w.ctx());
        assert_eq!(
            action,
            PcScreenAction::ShowStats {
                from_box: true,
                index: 0
            }
        );
    }

    // ── Bill's PC: release ───────────────────────────────────────────────

    #[test]
    fn release_requires_confirmation() {
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 5)).unwrap();
        w.pc_storage
            .current_box_mut()
            .deposit(mon(Species::Bulbasaur, 9))
            .unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(DOWN, &mut w.ctx()); // RELEASE
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::MonList);
        s.update_frame(A, &mut w.ctx()); // pick mon → confirm
        assert_eq!(s.phase(), PcPhase::ReleaseConfirm);
        assert!(s.yes_selected());
        // Select NO explicitly: back to the list, mon kept.
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        finish_confirmation_wait(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::MonList);
        assert_eq!(w.pc_storage.current_box().count(), 1);
        // Again, this time YES.
        s.update_frame(A, &mut w.ctx());
        s.update_frame(UP, &mut w.ctx()); // stay on YES
        s.update_frame(A, &mut w.ctx());
        finish_confirmation_wait(&mut s, &mut w);
        assert_eq!(
            s.message_lines(),
            &[
                "BULBASAUR was".to_string(),
                "released outside.".to_string(),
                "Bye BULBASAUR!".to_string(),
            ]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(w.pc_storage.current_box().count(), 0);
        assert_eq!(w.party.count(), 1);
    }

    // ── Bill's PC: change box ────────────────────────────────────────────

    #[test]
    fn change_box_save_confirm_then_switch() {
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 5)).unwrap();
        w.pc_storage
            .current_box_mut()
            .deposit(mon(Species::Bulbasaur, 9))
            .unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        // CHANGE BOX = cursor 3.
        for _ in 0..3 {
            s.update_frame(DOWN, &mut w.ctx());
        }
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::ChangeBoxConfirm);
        // NO → back, no switch, no save.
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        finish_confirmation_wait(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::BillsMenu);
        assert_eq!(w.pc_storage.current_box_index(), 0);
        assert!(!s.take_save_request());
        // YES → box list, cursor starts on the current box. (The bills menu
        // restored its saved cursor, so CHANGE BOX is still selected.)
        s.update_frame(A, &mut w.ctx());
        s.update_frame(UP, &mut w.ctx()); // YES
        s.update_frame(A, &mut w.ctx());
        finish_confirmation_wait(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::BoxList);
        assert_eq!(s.box_cursor(), 0);
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx()); // choose Box 3
        assert_eq!(s.phase(), PcPhase::BillsMenu);
        assert_eq!(w.pc_storage.current_box_index(), 2);
        assert!(s.take_save_request());
        assert_eq!(s.take_sfx(), vec![PcSfx::Save]);
        // The old box's contents stay in box 1.
        assert_eq!(w.pc_storage.get_box(0).unwrap().count(), 1);
        assert_eq!(w.pc_storage.current_box().count(), 0);
    }

    #[test]
    fn box_list_b_backs_out_without_switching() {
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 5)).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        for _ in 0..3 {
            s.update_frame(DOWN, &mut w.ctx());
        }
        s.update_frame(A, &mut w.ctx());
        s.update_frame(UP, &mut w.ctx()); // YES
        s.update_frame(A, &mut w.ctx());
        finish_confirmation_wait(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::BoxList);
        s.update_frame(DOWN, &mut w.ctx());
        assert_eq!(s.update_frame(B, &mut w.ctx()), PcScreenAction::Continue);
        assert_eq!(s.phase(), PcPhase::BillsMenu);
        assert_eq!(w.pc_storage.current_box_index(), 0);
        assert!(!s.take_save_request());
    }

    #[test]
    fn bills_menu_see_ya_returns_to_main_menu() {
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 5)).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        // SEE YA! = cursor 4.
        for _ in 0..4 {
            s.update_frame(DOWN, &mut w.ctx());
        }
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::MainMenu);
    }

    // ── Player's PC: item storage ────────────────────────────────────────

    fn open_item_menu_from_center(screen: &mut PcScreen, w: &mut World) {
        open_pokemon_center(screen, w);
        screen.update_frame(DOWN, &mut w.ctx()); // RED's PC
        screen.update_frame(A, &mut w.ctx());
        skip_message(screen, w);
        assert_eq!(screen.phase(), PcPhase::ItemMenu);
        screen.take_sfx();
    }

    #[test]
    fn item_menu_empty_storages_messages() {
        let mut w = World::new();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_item_menu_from_center(&mut s, &mut w);
        // WITHDRAW ITEM with empty PC storage.
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        assert_eq!(
            s.message_lines(),
            &["There is nothing".to_string(), "stored.".to_string()]
        );
        skip_message(&mut s, &mut w);
        // DEPOSIT ITEM with empty bag.
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        assert_eq!(
            s.message_lines(),
            &["You have nothing".to_string(), "to deposit.".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::ItemMenu);
    }

    #[test]
    fn deposit_item_with_quantity() {
        let mut w = World::new();
        w.bag.add_item(ItemId::Potion, 5).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_item_menu_from_center(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx()); // DEPOSIT ITEM
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(s.phase(), PcPhase::ItemList);
        s.update_frame(A, &mut w.ctx()); // pick POTION
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(s.phase(), PcPhase::ItemQuantity);
        assert_eq!(s.item_qty(), 1);
        s.update_frame(UP, &mut w.ctx());
        s.update_frame(UP, &mut w.ctx()); // qty 3
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(
            s.message_lines(),
            &["POTION was".to_string(), "stored via PC.".to_string()]
        );
        assert_eq!(s.take_sfx(), vec![PcSfx::WithdrawDeposit]);
        skip_message(&mut s, &mut w);
        // After the message the list re-opens (players_pc.asm `jp .loop`).
        assert_eq!(s.phase(), PcPhase::ItemList);
        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 3);
        assert_eq!(w.bag.item_quantity(ItemId::Potion), 2);
    }

    #[test]
    fn deposit_key_item_skips_quantity_prompt() {
        let mut w = World::new();
        w.bag.add_item(ItemId::Bicycle, 1).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_item_menu_from_center(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx()); // DEPOSIT ITEM
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        s.update_frame(A, &mut w.ctx()); // pick BICYCLE → no quantity prompt
        finish_item_question(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::Message);
        assert_eq!(
            s.message_lines(),
            &["BICYCLE was".to_string(), "stored via PC.".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(w.pc_items.item_quantity(ItemId::Bicycle), 1);
        assert_eq!(w.bag.item_quantity(ItemId::Bicycle), 0);
    }

    #[test]
    fn deposit_into_full_pc_storage_fails_without_moving() {
        let mut w = World::new();
        w.bag.add_item(ItemId::Potion, 3).unwrap();
        // Fill all 50 PC slots with distinct items.
        let fillers = [
            ItemId::Antidote,
            ItemId::BurnHeal,
            ItemId::IceHeal,
            ItemId::Awakening,
            ItemId::ParlyzHeal,
            ItemId::FullRestore,
            ItemId::MaxPotion,
            ItemId::HyperPotion,
            ItemId::SuperPotion,
            ItemId::PokeBall,
            ItemId::GreatBall,
            ItemId::UltraBall,
            ItemId::SafariBall,
            ItemId::SuperRepel,
            ItemId::MaxRepel,
            ItemId::Repel,
            ItemId::EscapeRope,
            ItemId::FullHeal,
            ItemId::Revive,
            ItemId::MaxRevive,
            ItemId::RareCandy,
            ItemId::XAttack,
            ItemId::XDefend,
            ItemId::XSpeed,
            ItemId::XSpecial,
            ItemId::Ether,
            ItemId::MaxEther,
            ItemId::Elixer,
            ItemId::MaxElixer,
            ItemId::HpUp,
            ItemId::Protein,
            ItemId::Iron,
            ItemId::Carbos,
            ItemId::Calcium,
            ItemId::PpUp,
            ItemId::FreshWater,
            ItemId::SodaPop,
            ItemId::Lemonade,
            ItemId::FireStone,
            ItemId::ThunderStone,
            ItemId::WaterStone,
            ItemId::LeafStone,
            ItemId::MoonStone,
            ItemId::Nugget,
            ItemId::DireHit,
            ItemId::GuardSpec,
            ItemId::XAccuracy,
            ItemId::PokeDoll,
            ItemId::Tm01,
            ItemId::Tm02,
        ];
        for item in fillers {
            w.pc_items.add_item(item, 1).unwrap();
        }
        assert!(w.pc_items.is_full());
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_item_menu_from_center(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx()); // DEPOSIT ITEM
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        s.update_frame(A, &mut w.ctx()); // POTION, qty 1
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(
            s.message_lines(),
            &["No room left to".to_string(), "store items.".to_string()]
        );
        skip_message(&mut s, &mut w);
        // Nothing moved.
        assert_eq!(w.bag.item_quantity(ItemId::Potion), 3);
        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 0);
    }

    #[test]
    fn withdraw_item_with_quantity() {
        let mut w = World::new();
        w.pc_items.add_item(ItemId::Potion, 4).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_item_menu_from_center(&mut s, &mut w);
        s.update_frame(A, &mut w.ctx()); // WITHDRAW ITEM
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(s.phase(), PcPhase::ItemList);
        s.update_frame(A, &mut w.ctx()); // POTION
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        s.update_frame(UP, &mut w.ctx()); // qty 2
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(
            s.message_lines(),
            &["Withdrew".to_string(), "POTION.".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(w.bag.item_quantity(ItemId::Potion), 2);
        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 2);
    }

    #[test]
    fn withdraw_into_full_bag_fails() {
        let mut w = World::new();
        w.pc_items.add_item(ItemId::Potion, 2).unwrap();
        // Fill all 20 bag slots with distinct full stacks.
        let fillers = [
            ItemId::Antidote,
            ItemId::BurnHeal,
            ItemId::IceHeal,
            ItemId::Awakening,
            ItemId::ParlyzHeal,
            ItemId::PokeBall,
            ItemId::GreatBall,
            ItemId::UltraBall,
            ItemId::MasterBall,
            ItemId::SuperPotion,
            ItemId::HyperPotion,
            ItemId::MaxPotion,
            ItemId::FullRestore,
            ItemId::Revive,
            ItemId::MaxRevive,
            ItemId::SuperRepel,
            ItemId::MaxRepel,
            ItemId::EscapeRope,
            ItemId::Repel,
            ItemId::FullHeal,
        ];
        for item in fillers {
            w.bag.add_item(item, 1).unwrap();
        }
        assert!(w.bag.is_full());
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_item_menu_from_center(&mut s, &mut w);
        s.update_frame(A, &mut w.ctx()); // WITHDRAW ITEM
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        s.update_frame(A, &mut w.ctx()); // POTION, qty 1
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(
            s.message_lines(),
            &["You can't carry".to_string(), "any more items.".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 2);
    }

    #[test]
    fn toss_item_flow_and_key_item_refusal() {
        let mut w = World::new();
        w.pc_items.add_item(ItemId::Potion, 3).unwrap();
        w.pc_items.add_item(ItemId::Bicycle, 1).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_item_menu_from_center(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(DOWN, &mut w.ctx()); // TOSS ITEM
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(s.phase(), PcPhase::ItemList);
        // POTION: quantity → "Is it OK to toss?" → YES.
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(s.phase(), PcPhase::ItemQuantity);
        s.update_frame(UP, &mut w.ctx()); // 2
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(s.phase(), PcPhase::TossConfirm);
        // Select NO explicitly to keep the items.
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        finish_confirmation_wait(&mut s, &mut w);
        finish_item_question(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::ItemList);
        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 3);
        // YES tosses.
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        s.update_frame(A, &mut w.ctx()); // qty 1
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        s.update_frame(UP, &mut w.ctx()); // stay on YES
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        finish_confirmation_wait(&mut s, &mut w);
        finish_item_question(&mut s, &mut w);
        assert_eq!(
            s.message_lines(),
            &["Threw away".to_string(), "POTION.".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 2);
        // BICYCLE (key item): refused outright, no quantity prompt.
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        finish_item_question(&mut s, &mut w);
        if s.phase()==PcPhase::ItemQuantityPrompt {finish_quantity_prompt(&mut s,&mut w);}
        assert_eq!(s.phase(), PcPhase::Message);
        assert_eq!(
            s.message_lines(),
            &["That's too impor-".to_string(), "tant to toss!".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(w.pc_items.item_quantity(ItemId::Bicycle), 1);
    }

    // ── Bedroom PC (direct item PC, no main menu) ────────────────────────

    #[test]
    fn bedroom_pc_goes_straight_to_item_menu() {
        let mut w = World::new();
        let mut s = PcScreen::new(PcEntry::PlayersPc, &open_ctx());
        assert_eq!(s.phase(), PcPhase::Message);
        skip_message(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::ItemMenu);
        s.take_sfx(); // drain the boot TurnOn
        // LOG OFF exits entirely (no main menu above the bedroom PC).
        for _ in 0..3 {
            s.update_frame(DOWN, &mut w.ctx());
        }
        assert_eq!(s.update_frame(A, &mut w.ctx()), PcScreenAction::Exit);
        assert_eq!(s.take_sfx(), vec![PcSfx::TurnOff]);
    }

    // ── Oak's PC rating ──────────────────────────────────────────────────

    fn open_oaks_confirm(screen: &mut PcScreen, w: &mut World) {
        let open = PcOpenContext {
            has_pokedex: true,
            ..open_ctx()
        };
        let _ = open;
        // (screen was built by caller with has_pokedex = true)
        open_pokemon_center(screen, w);
        screen.update_frame(DOWN, &mut w.ctx());
        screen.update_frame(DOWN, &mut w.ctx()); // PROF.OAK's PC
        screen.update_frame(A, &mut w.ctx());
        skip_message(screen, w);
        assert_eq!(screen.phase(), PcPhase::OaksConfirm);
    }

    #[test]
    fn oaks_rating_pages_and_close() {
        let mut w = World::new();
        for i in 1..=25u8 {
            let species = Species::from_index_id(i);
            w.pokedex.set_seen(species);
            w.pokedex.set_owned(species);
        }
        let open = PcOpenContext {
            has_pokedex: true,
            ..open_ctx()
        };
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open);
        open_oaks_confirm(&mut s, &mut w);
        // YES → completion page (2 pages: 8 lines).
        s.update_frame(UP, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        finish_confirmation_wait(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::Message);
        assert_eq!(s.dex_seen(), 25);
        assert_eq!(s.dex_owned(), 25);
        assert_eq!(s.message_page_count(), 2);
        s.update_frame(A, &mut w.ctx()); // page 2
        assert_eq!(s.message_page(), 1);
        s.update_frame(A, &mut w.ctx()); // → rating text
        assert_eq!(
            s.message_lines(),
            &[
                "You still need".to_string(),
                "more #MON!".to_string(),
                "Try to catch".to_string(),
                "other species!".to_string(),
            ]
        );
        s.update_frame(A, &mut w.ctx()); // → closed link
        assert_eq!(
            s.message_lines(),
            &["Closed link to".to_string(), "PROF.OAK's PC.".to_string()]
        );
        s.update_frame(A, &mut w.ctx()); // → main menu
        assert_eq!(s.phase(), PcPhase::MainMenu);
    }

    #[test]
    fn oaks_rating_table_thresholds() {
        for &(threshold, text) in DEX_RATINGS.iter().take(3) {
            let mut w = World::new();
            let owned = threshold - 1;
            for i in 1..=(owned as u8) {
                w.pokedex.set_owned(Species::from_index_id(i));
            }
            let open = PcOpenContext {
                has_pokedex: true,
                ..open_ctx()
            };
            let mut s = PcScreen::new(PcEntry::PokemonCenter, &open);
            open_oaks_confirm(&mut s, &mut w);
            s.update_frame(UP, &mut w.ctx()); // YES
            s.update_frame(A, &mut w.ctx());
            finish_confirmation_wait(&mut s, &mut w);
            // Skip the completion pages.
            while s.phase() == PcPhase::Message
                && !s.message_lines().first().map_or(false, |l| {
                    text.split('\n').next().unwrap() == l
                })
            {
                s.update_frame(A, &mut w.ctx());
            }
            let expected: Vec<String> = text.split('\n').map(|l| l.to_string()).collect();
            assert_eq!(s.message_lines(), expected.as_slice());
        }
    }

    #[test]
    fn oaks_confirm_no_closes_link() {
        let mut w = World::new();
        let open = PcOpenContext {
            has_pokedex: true,
            ..open_ctx()
        };
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open);
        open_oaks_confirm(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx()); // NO
        s.update_frame(A, &mut w.ctx());
        finish_confirmation_wait(&mut s, &mut w);
        assert_eq!(
            s.message_lines(),
            &["Closed link to".to_string(), "PROF.OAK's PC.".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::MainMenu);
    }

    // ── List navigation ──────────────────────────────────────────────────

    #[test]
    fn mon_list_wraps_and_cancel_row_backs_out() {
        let mut w = World::new();
        w.party.add(mon(Species::Pikachu, 5)).unwrap();
        w.party.add(mon(Species::Bulbasaur, 5)).unwrap();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx()); // DEPOSIT
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::MonList);
        // 2 mons + CANCEL = 3 rows; up from 0 wraps to CANCEL.
        s.update_frame(UP, &mut w.ctx());
        assert_eq!(s.mon_cursor(), 2);
        s.update_frame(A, &mut w.ctx()); // CANCEL
        assert_eq!(s.phase(), PcPhase::BillsMenu);
        assert_eq!(w.party.count(), 2);
    }

    #[test]
    fn message_paging_four_lines_per_page() {
        let mut w = World::new();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_bills_pc(&mut s, &mut w);
        // The "Accessed" message for Bill's PC is 5 lines = 2 pages.
        // (already past it; check the withdraw-party-full message instead)
        for _ in 0..6 {
            w.party.add(mon(Species::Pikachu, 5)).unwrap();
        }
        w.pc_storage
            .current_box_mut()
            .deposit(mon(Species::Bulbasaur, 5))
            .unwrap();
        s.update_frame(A, &mut w.ctx()); // WITHDRAW
        assert_eq!(s.message_page_count(), 2);
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.message_page(), 1);
        assert_eq!(s.phase(), PcPhase::Message);
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::BillsMenu);
    }

    /// #MON LEAGUE → the HoF viewer walks every recorded team's mons in
    /// order, showing the all-time team number (league_pc.asm:29-46).
    #[test]
    fn league_pc_walks_hof_teams() {
        let mut w = World::new();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &hof_open_ctx());
        open_pokemon_center(&mut s, &mut w);
        // Main menu (no pokedex): BILL's PC / <NAME>'s PC / #MON LEAGUE / LOG OFF.
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::Message, "Accessed HALL OF FAME List");
        skip_message(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::LeagueHoF);

        let (no, mon) = s.league_hof_mon().unwrap();
        assert_eq!(no, 1);
        assert_eq!(mon.nickname, "MON0");
        assert_eq!(s.league_hof_progress(), (0, 2));

        s.update_frame(A, &mut w.ctx()); // team 1, mon 2
        let (no, mon) = s.league_hof_mon().unwrap();
        assert_eq!(no, 1);
        assert_eq!(mon.nickname, "MON1");

        s.update_frame(A, &mut w.ctx()); // team 2, mon 1
        let (no, mon) = s.league_hof_mon().unwrap();
        assert_eq!(no, 2);
        assert_eq!(mon.nickname, "MON0");
        assert_eq!(s.league_hof_progress(), (1, 2));

        s.update_frame(A, &mut w.ctx()); // past the last team → main menu
        assert_eq!(s.phase(), PcPhase::MainMenu);
        assert_eq!(s.league_hof_mon(), None);
    }

    /// B bails out of the viewer immediately (league_pc.asm:60-63).
    #[test]
    fn league_pc_b_exits_viewer() {
        let mut w = World::new();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &hof_open_ctx());
        open_pokemon_center(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        skip_message(&mut s, &mut w);
        assert_eq!(s.phase(), PcPhase::LeagueHoF);
        s.update_frame(B, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::MainMenu);
    }

    /// The #MON LEAGUE main-menu entry only appears after the Hall of Fame
    /// (bills_pc.asm:5-6) — and without recorded teams the viewer can't be
    /// reached.
    #[test]
    fn league_pc_entry_gated_on_beating_league() {
        let mut w = World::new();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        open_pokemon_center(&mut s, &mut w);
        let labels = s.main_menu_labels();
        assert!(!labels.iter().any(|l| l == "#MON LEAGUE"));
    }
    #[test]
    fn pokemon_storage_cries_wait_before_receipt_and_ignore_input() {
        for mode in [MonListMode::Deposit, MonListMode::Withdraw] {
            let mut w = World::new();
            w.party.add(mon(Species::Pikachu, 12)).unwrap();
            w.party.add(mon(Species::Bulbasaur, 7)).unwrap();
            w.pc_storage.current_box_mut().deposit(mon(Species::Abra, 10)).unwrap();
            let mut s = PcScreen::new(PcEntry::BillsPc, &open_ctx());
            s.take_sfx();
            s.mon_mode = mode;
            s.mon_cursor = 0;
            s.mon_action_cursor = 0;
            s.yes_selected = true;
            s.phase = if mode == MonListMode::Release { PcPhase::ReleaseConfirm } else { PcPhase::MonAction };
            let before = (w.party.count(), w.pc_storage.current_box().count());
            s.update_frame_with_sound(A, &mut w.ctx(), false);
            let species = if mode == MonListMode::Deposit { Species::Pikachu } else { Species::Abra };
            assert_eq!(s.take_sfx(), vec![PcSfx::Cry(species)]);
            assert!(s.waiting_for_sound());
            for input in [A, B, DOWN] {
                s.update_frame_with_sound(input, &mut w.ctx(), true);
                assert_eq!((w.party.count(),w.pc_storage.current_box().count()), before);
                assert_ne!(s.phase(), PcPhase::Message);
                assert!(s.take_sfx().is_empty());
            }
            s.update_frame_with_sound(B, &mut w.ctx(), false);
            assert!(!s.waiting_for_sound());
            assert_eq!(s.phase(), PcPhase::Message);
            assert!(s.take_sfx().is_empty());
            let after = match mode {
                MonListMode::Deposit => (1,2),
                MonListMode::Withdraw => (3,0),
                MonListMode::Release => (2,0),
            };
            assert_eq!((w.party.count(),w.pc_storage.current_box().count()), after);
        }
    }

    #[test]
    fn release_removes_mon_and_shows_receipt_while_cry_plays() {
        let mut w = World::new();
        w.pc_storage.current_box_mut().deposit(mon(Species::Abra, 10)).unwrap();
        let mut s = PcScreen::new(PcEntry::BillsPc, &open_ctx());
        s.take_sfx();
        s.phase = PcPhase::ReleaseConfirm;
        s.yes_selected = true;
        s.update_frame_with_sound(A, &mut w.ctx(), false);
        assert_eq!(w.pc_storage.current_box().count(), 1);
        assert!(s.take_sfx().is_empty());
        finish_confirmation_wait(&mut s, &mut w);
        assert_eq!(w.pc_storage.current_box().count(), 0);
        assert_eq!(s.phase(), PcPhase::Message);
        assert!(!s.waiting_for_sound());
        assert_eq!(s.take_sfx(), vec![PcSfx::Cry(Species::Abra)]);
    }

    #[test]
    fn league_viewer_cries_for_each_displayed_species_and_not_after_exit() {
        let mut w = World::new();
        let mut s = PcScreen::new(PcEntry::PokemonCenter, &hof_open_ctx());
        open_pokemon_center(&mut s, &mut w);
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
        skip_message(&mut s, &mut w);
        let mut viewed = 0;
        while s.phase() == PcPhase::LeagueHoF {
            let species = s.league_hof_mon().unwrap().1.species;
            let cries: Vec<_> = s.take_sfx().into_iter().filter(|sfx| matches!(sfx, PcSfx::Cry(_))).collect();
            assert_eq!(cries, vec![PcSfx::Cry(species)]);
            viewed += 1;
            s.update_frame(A, &mut w.ctx());
        }
        assert_eq!(viewed, 3);
        assert_eq!(s.phase(), PcPhase::MainMenu);
        assert!(s.take_sfx().is_empty());
    }

}
