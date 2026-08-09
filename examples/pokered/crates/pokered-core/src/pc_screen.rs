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

use crate::items::inventory::{is_tossable, Inventory};
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
    pub bag: &'a mut Inventory,
    pub pc_items: &'a mut Inventory,
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

/// The whole PC flow, from "turned on the PC" to LOG OFF.
#[derive(Debug, Clone)]
pub struct PcScreen {
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

    // Box chooser.
    box_cursor: usize,

    // Item PC state.
    item_mode: ItemListMode,
    item_list_cursor: usize,
    item_list_scroll: usize,
    item_qty: u8,

    // Side effects for the app.
    sfx: Vec<PcSfx>,
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
        let mut screen = Self {
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
            yes_selected: false,
            box_cursor: 0,
            item_mode: ItemListMode::Withdraw,
            item_list_cursor: 0,
            item_list_scroll: 0,
            item_qty: 1,
            sfx: Vec::new(),
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
        std::mem::take(&mut self.sfx)
    }

    /// True once after CHANGE BOX switched boxes — the original saves the
    /// game (save.asm:396 `call SaveGameData`), so the app should persist.
    pub fn take_save_request(&mut self) -> bool {
        std::mem::replace(&mut self.save_requested, false)
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

    // ── Internals ─────────────────────────────────────────────────────────

    fn set_message(&mut self, lines: Vec<String>, next: AfterMessage) {
        self.msg_lines = lines;
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
                self.phase = PcPhase::ItemList;
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
        self.phase = PcPhase::ItemMenu;
    }

    fn enter_item_list(&mut self, mode: ItemListMode) {
        self.item_mode = mode;
        self.item_list_cursor = 0;
        self.item_list_scroll = 0;
        self.phase = PcPhase::ItemList;
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

    pub fn update_frame(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
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
            PcPhase::ReleaseConfirm => self.update_release_confirm(input, ctx),
            PcPhase::ChangeBoxConfirm => self.update_change_box_confirm(input),
            PcPhase::BoxList => self.update_box_list(input, ctx),
            PcPhase::ItemMenu => self.update_item_menu(input, ctx),
            PcPhase::ItemList => self.update_item_list(input, ctx),
            PcPhase::ItemQuantity => self.update_item_quantity(input, ctx),
            PcPhase::TossConfirm => self.update_toss_confirm(input, ctx),
            PcPhase::OaksConfirm => self.update_oaks_confirm(input, ctx),
            PcPhase::LeagueHoF => self.update_league_hof(input),
        }
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
            self.sfx.push(PcSfx::Enter);
            self.league_mon += 1;
            if self.league_mon >= self.hof_teams[self.league_team].mons.len() {
                self.league_mon = 0;
                self.league_team += 1;
                if self.league_team >= self.hof_teams.len() {
                    self.enter_main_menu();
                }
            }
        }
        PcScreenAction::Continue
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
                    self.sfx.push(PcSfx::Enter);
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
                    self.yes_selected = false;
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
                    self.yes_selected = false;
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
                            let name = ctx
                                .party
                                .get(idx)
                                .map(|m| m.display_name())
                                .unwrap_or_default();
                            if let Ok(mon) = ctx.party.remove(idx) {
                                let _ = ctx.pc_storage.current_box_mut().deposit(mon);
                                self.sfx.push(PcSfx::WithdrawDeposit);
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
                            let name = ctx
                                .pc_storage
                                .current_box()
                                .get(idx)
                                .map(|m| m.display_name())
                                .unwrap_or_default();
                            if ctx.party.is_full() {
                                // Party filled up between the menu check and
                                // here — leave the mon in the box.
                                self.enter_bills_menu();
                            } else if let Ok(mon) = ctx.pc_storage.current_box_mut().withdraw(idx)
                            {
                                let _ = ctx.party.add(mon);
                                self.sfx.push(PcSfx::WithdrawDeposit);
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
        if input.up || input.down {
            self.yes_selected = !self.yes_selected;
        }
        if input.b {
            // B == NO (YesNoChoice cursor on NO cancels).
            self.phase = PcPhase::MonList;
            return PcScreenAction::Continue;
        }
        if input.a {
            if self.yes_selected {
                let idx = self.mon_cursor;
                let name = ctx
                    .pc_storage
                    .current_box()
                    .get(idx)
                    .map(|m| m.display_name())
                    .unwrap_or_default();
                if ctx.pc_storage.current_box_mut().release(idx).is_ok() {
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
        if input.up || input.down {
            self.yes_selected = !self.yes_selected;
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

    /// The inventory the current item list shows.
    fn item_source<'c>(&self, ctx: &'c PcContext) -> &'c Inventory {
        match self.item_mode {
            ItemListMode::Deposit => ctx.bag,
            ItemListMode::Withdraw | ItemListMode::Toss => ctx.pc_items,
        }
    }

    fn item_row_count(&self, ctx: &PcContext) -> usize {
        self.item_source(ctx).count() + 1 // + CANCEL
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

        if input.b {
            self.enter_item_menu();
            return PcScreenAction::Continue;
        }
        if input.a {
            if self.item_list_cursor == rows - 1 {
                // CANCEL row.
                self.enter_item_menu();
                return PcScreenAction::Continue;
            }
            let Some((item, have)) = self.item_source(ctx).get(self.item_list_cursor) else {
                self.enter_item_menu();
                return PcScreenAction::Continue;
            };
            match self.item_mode {
                // Key items are unique: no "How many?" prompt, qty is 1
                // (players_pc.asm:110-121,164-175).
                ItemListMode::Deposit | ItemListMode::Withdraw => {
                    if item.is_key_item() {
                        self.exec_item_move(ctx, self.item_list_cursor, item, 1);
                    } else {
                        self.item_qty = 1;
                        let _ = have;
                        self.phase = PcPhase::ItemQuantity;
                    }
                }
                ItemListMode::Toss => {
                    if !is_tossable(item) {
                        // HMs/key items: TossItem refuses outright
                        // (item_effects.asm:2550-2559).
                        self.set_message(
                            vec!["That's too impor-".into(), "tant to toss!".into()],
                            AfterMessage::ItemList,
                        );
                    } else {
                        self.item_qty = 1;
                        self.phase = PcPhase::ItemQuantity;
                    }
                }
            }
        }
        PcScreenAction::Continue
    }

    fn update_item_quantity(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        let Some((item, have)) = self.item_source(ctx).get(self.item_list_cursor) else {
            self.phase = PcPhase::ItemList;
            return PcScreenAction::Continue;
        };
        if input.up && self.item_qty < have {
            self.item_qty += 1;
        } else if input.down && self.item_qty > 1 {
            self.item_qty -= 1;
        }
        if input.b {
            self.phase = PcPhase::ItemList;
            return PcScreenAction::Continue;
        }
        if input.a {
            let qty = self.item_qty;
            match self.item_mode {
                ItemListMode::Deposit | ItemListMode::Withdraw => {
                    self.exec_item_move(ctx, self.item_list_cursor, item, qty);
                }
                ItemListMode::Toss => {
                    // "Is it OK to toss {ITEM}?" (_IsItOKToTossItemText) YES/NO.
                    self.yes_selected = false;
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
                std::mem::swap(ctx.pc_items, &mut trial);
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
                std::mem::swap(ctx.bag, &mut trial);
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
        if input.up || input.down {
            self.yes_selected = !self.yes_selected;
        }
        if input.b {
            self.phase = PcPhase::ItemList;
            return PcScreenAction::Continue;
        }
        if input.a {
            if self.yes_selected {
                let idx = self.item_list_cursor;
                let qty = self.item_qty;
                let name = self
                    .item_source(ctx)
                    .get(idx)
                    .map(|(item, _)| item_name(item))
                    .unwrap_or_default();
                let _ = ctx.pc_items.remove_item_at(idx, qty);
                // "Threw away {ITEM}." (_ThrewAwayItemText)
                self.set_message(
                    vec!["Threw away".into(), format!("{}.", name)],
                    AfterMessage::ItemList,
                );
            } else {
                self.phase = PcPhase::ItemList;
            }
        }
        PcScreenAction::Continue
    }

    fn update_oaks_confirm(&mut self, input: MenuInput, ctx: &mut PcContext) -> PcScreenAction {
        if input.up || input.down {
            self.yes_selected = !self.yes_selected;
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
        self.yes_selected = false;
        self.phase = PcPhase::OaksConfirm;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pokemon::stats::create_pokemon;
    use pokered_data::species::Species;

    fn mon(species: Species, level: u8) -> crate::battle::state::Pokemon {
        create_pokemon(species, level, [0x9A, 0x78]).unwrap()
    }

    struct World {
        party: Party,
        pc_storage: PcStorage,
        bag: Inventory,
        pc_items: Inventory,
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

    /// Advance through every page of the current message.
    fn skip_message(screen: &mut PcScreen, w: &mut World) {
        while screen.phase() == PcPhase::Message {
            assert_eq!(
                screen.update_frame(A, &mut w.ctx()),
                PcScreenAction::Continue
            );
        }
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
        assert_eq!(s.take_sfx(), vec![PcSfx::WithdrawDeposit]);
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
        assert!(!s.yes_selected());
        // NO (default): back to the list, mon kept.
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::MonList);
        assert_eq!(w.pc_storage.current_box().count(), 1);
        // Again, this time YES.
        s.update_frame(A, &mut w.ctx());
        s.update_frame(UP, &mut w.ctx()); // toggle to YES
        s.update_frame(A, &mut w.ctx());
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
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::BillsMenu);
        assert_eq!(w.pc_storage.current_box_index(), 0);
        assert!(!s.take_save_request());
        // YES → box list, cursor starts on the current box. (The bills menu
        // restored its saved cursor, so CHANGE BOX is still selected.)
        s.update_frame(A, &mut w.ctx());
        s.update_frame(UP, &mut w.ctx()); // YES
        s.update_frame(A, &mut w.ctx());
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
        assert_eq!(
            s.message_lines(),
            &["There is nothing".to_string(), "stored.".to_string()]
        );
        skip_message(&mut s, &mut w);
        // DEPOSIT ITEM with empty bag.
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
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
        assert_eq!(s.phase(), PcPhase::ItemList);
        s.update_frame(A, &mut w.ctx()); // pick POTION
        assert_eq!(s.phase(), PcPhase::ItemQuantity);
        assert_eq!(s.item_qty(), 1);
        s.update_frame(UP, &mut w.ctx());
        s.update_frame(UP, &mut w.ctx()); // qty 3
        s.update_frame(A, &mut w.ctx());
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
        s.update_frame(A, &mut w.ctx()); // pick BICYCLE → no quantity prompt
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
        s.update_frame(A, &mut w.ctx()); // POTION, qty 1
        s.update_frame(A, &mut w.ctx());
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
        assert_eq!(s.phase(), PcPhase::ItemList);
        s.update_frame(A, &mut w.ctx()); // POTION
        s.update_frame(UP, &mut w.ctx()); // qty 2
        s.update_frame(A, &mut w.ctx());
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
        s.update_frame(A, &mut w.ctx()); // POTION, qty 1
        s.update_frame(A, &mut w.ctx());
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
        assert_eq!(s.phase(), PcPhase::ItemList);
        // POTION: quantity → "Is it OK to toss?" → YES.
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::ItemQuantity);
        s.update_frame(UP, &mut w.ctx()); // 2
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::TossConfirm);
        // NO keeps the items.
        s.update_frame(A, &mut w.ctx());
        assert_eq!(s.phase(), PcPhase::ItemList);
        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 3);
        // YES tosses.
        s.update_frame(A, &mut w.ctx());
        s.update_frame(A, &mut w.ctx()); // qty 1
        s.update_frame(UP, &mut w.ctx()); // toggle YES
        s.update_frame(A, &mut w.ctx());
        assert_eq!(
            s.message_lines(),
            &["Threw away".to_string(), "POTION.".to_string()]
        );
        skip_message(&mut s, &mut w);
        assert_eq!(w.pc_items.item_quantity(ItemId::Potion), 2);
        // BICYCLE (key item): refused outright, no quantity prompt.
        s.update_frame(DOWN, &mut w.ctx());
        s.update_frame(A, &mut w.ctx());
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
        s.update_frame(A, &mut w.ctx()); // NO
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
}
