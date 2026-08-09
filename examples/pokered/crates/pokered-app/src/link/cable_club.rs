//! Cable Club in-room link UI flow (Colosseum / TradeCenter).
//!
//! This is the app-level state machine that turns the CORE drivers'
//! [`LinkDriverEvent`]s / [`LinkTradePollResult`]s into the original's Cable
//! Club screens and input prompts. The core drivers
//! ([`LinkBattleDriver`] / [`LinkTradeDriver`], fed through the
//! [`LinkSession`] router) own the wire protocol AND the game data (parties,
//! exchange, battle screen); this module owns the *room*:
//!
//! - the gameboy-on-the-table interaction (`game.linkStart()` from the map
//!   scene) starts a LINK BATTLE in the Colosseum or a LINK TRADE in the
//!   Trade Center — the original's `CableClubLeftGameboy` /
//!   `CableClubRightGameboy` set `LINK_STATE_START_BATTLE` /
//!   `LINK_STATE_START_TRADE` by room (engine/pokemon/bills_pc.asm:513-533);
//! - the original texts are reproduced verbatim where they exist:
//!   "Just a moment." (`JustAMomentText`), "Waiting...!"
//!   (`WaitingText`, engine/link/print_waiting_text.asm), "PLEASE WAIT!"
//!   (`PleaseWaitString`, engine/link/cable_club.asm:295-296),
//!   "Trade completed!" / "Too bad! The trade was canceled!"
//!   (engine/link/cable_club.asm:882-887) and "The link was canceled."
//!   (`_LinkCanceledText`, data/text/text_2.asm:1691-1694).
//!
//! The request/accept/decline handshake itself is a protocol addition (the
//! original simply exchanges once both players use the gameboy), so the
//! peer's yes/no prompt uses "Start a link battle?" / "Start a link trade?"
//! — documented deviation. The simultaneous-gameboy tie is broken in the
//! core drivers by clock role (host wins, engine/menus/main_menu.asm:
//! "The gameboy that is clocking the connection wins").
//!
//! The flow is modal: while it owns input, the app skips the overworld
//! update (the game freezes, exactly like the original's link screens) and
//! routes A/B/up/down here. Session actions the flow cannot perform itself
//! (it does not own the drivers or the save party) are returned as
//! [`FlowNeed`]s for the game loop to execute.

use pokered_core::battle::link_battle_driver::LinkDriverEvent;
use pokered_core::battle::state::Pokemon;
use pokered_core::link::link_trade::LinkTradePollResult;
use pokered_core::party_select::PartySelectState;
use pokered_core::party_screen::PartyScreenInput;
use pokered_data::maps::MapId;

/// The room's link activity: the Colosseum starts battles, the Trade Center
/// starts trades (the room decides, as in the original).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Battle,
    Trade,
}

/// A driver action the game loop must perform (it owns the drivers and the
/// save party).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowNeed {
    None,
    /// Send `RequestBattle` / `RequestTrade` (the player used the gameboy).
    RequestLink(LinkKind),
    /// Answer the peer's pending request (yes/no).
    ReplyRequest {
        kind: LinkKind,
        accept: bool,
    },
    /// The trade party-selector picked this 0-based index.
    SelectMon(u8),
    /// The player cancelled the trade selection (or the confirm box).
    CancelTrade,
    /// Confirm the trade — the driver sends the selected mon's data.
    ConfirmTrade,
}

/// The in-room link flow phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CableClubPhase {
    /// No link session, or not inside a Cable Club room.
    Inactive,
    /// Connected and inside Colosseum/TradeCenter: the remote player's
    /// avatar is present; the gameboy on the table is live.
    InRoom,
    /// The player used the gameboy: "Just a moment." box; the request was
    /// already sent.
    JustAMoment {
        kind: LinkKind,
    },
    /// Our request is in flight — modal "Waiting...!" box.
    WaitingResponse {
        kind: LinkKind,
    },
    /// The peer requested — yes/no prompt ("Start a link battle/trade?").
    PeerPrompt {
        kind: LinkKind,
        selected: u8,
    },
    /// Battle party exchange in progress — modal "PLEASE WAIT!" box.
    Exchanging,
    /// Both parties exchanged; the game loop builds the battle next frame.
    BattleSetup,
    /// Link battle in progress (the game loop drives the BattleScreen; this
    /// phase stays dormant, buffering turn/result events).
    Battle,
    /// Trade: the local party selector is on screen.
    TradeSelect,
    /// We selected; waiting for the peer's selection — modal "Waiting...!".
    TradeWaitingPeer,
    /// Both selected — "{mon} will be traded." + yes/no (the original's
    /// `_WillBeTradedText` + TRADE_CANCEL_MENU, engine/link/cable_club.asm:
    /// 714-740).
    TradeConfirm {
        local_index: u8,
        remote_index: u8,
        selected: u8,
    },
    /// Our confirm is in flight — modal "Waiting...!".
    TradeWaitingConfirm,
    /// The trade cutscene is playing (the game loop's `trade_anim`).
    TradeAnim,
    /// Post-trade "Trade completed!" box — A returns to the selection
    /// screen (the original loops via `CableClub_DoBattleOrTradeAgain`,
    /// engine/link/cable_club.asm:870).
    TradeCompleted,
    /// Error / disconnect box — A returns to `Inactive`.
    Error {
        text: String,
    },
}

impl CableClubPhase {
    /// Phases that freeze the overworld and consume input.
    pub fn is_modal(&self) -> bool {
        matches!(
            self,
            CableClubPhase::JustAMoment { .. }
                | CableClubPhase::WaitingResponse { .. }
                | CableClubPhase::PeerPrompt { .. }
                | CableClubPhase::Exchanging
                | CableClubPhase::TradeSelect
                | CableClubPhase::TradeWaitingPeer
                | CableClubPhase::TradeConfirm { .. }
                | CableClubPhase::TradeWaitingConfirm
                | CableClubPhase::TradeCompleted
                | CableClubPhase::Error { .. }
        )
    }
}

/// In-room Cable Club link flow. Held by the game; driven once per frame.
#[derive(Debug)]
pub struct CableClubFlow {
    phase: CableClubPhase,
    /// Trade party selector (created when the trade menu opens).
    selector: Option<PartySelectState>,
    /// The peer's selection index (trade), for the confirm box.
    remote_selection: Option<u8>,
    /// A transient one-line box shown while `InRoom` (e.g. "The link was
    /// canceled." after a declined request); A dismisses it.
    transient_text: Option<String>,
    /// We cancelled the trade selection; the next `PeerCancelled` event
    /// means BOTH sides cancelled — the original returns to the room
    /// (`ReturnToCableClubRoom`, engine/link/cable_club.asm:582-599) instead
    /// of looping back into the selection screen.
    pending_our_cancel: bool,
}

/// Original texts (verbatim where the disassembly has them).
pub const TEXT_JUST_A_MOMENT: &str = "Just a moment.";
pub const TEXT_WAITING: &str = "Waiting...!";
pub const TEXT_PLEASE_WAIT: &str = "PLEASE WAIT!";
pub const TEXT_TRADE_COMPLETED: &str = "Trade completed!";
pub const TEXT_TRADE_CANCELED: &str = "Too bad! The trade\nwas canceled!";
pub const TEXT_LINK_CANCELED: &str = "The link was\ncanceled.";
pub const TEXT_PROMPT_BATTLE: &str = "Start a link\nbattle?";
pub const TEXT_PROMPT_TRADE: &str = "Start a link\ntrade?";

impl CableClubFlow {
    pub fn new() -> Self {
        CableClubFlow {
            phase: CableClubPhase::Inactive,
            selector: None,
            remote_selection: None,
            transient_text: None,
            pending_our_cancel: false,
        }
    }

    pub fn phase(&self) -> &CableClubPhase {
        &self.phase
    }

    pub fn is_active(&self) -> bool {
        self.phase != CableClubPhase::Inactive
    }

    /// True while a modal link screen owns the game (the overworld update is
    /// skipped and input routes to [`CableClubFlow::update`]).
    pub fn is_modal(&self) -> bool {
        self.phase.is_modal()
    }

    /// The room's link kind, if one is in flight.
    pub fn kind(&self) -> Option<LinkKind> {
        match &self.phase {
            CableClubPhase::JustAMoment { kind }
            | CableClubPhase::WaitingResponse { kind }
            | CableClubPhase::PeerPrompt { kind, .. } => Some(*kind),
            CableClubPhase::Exchanging
            | CableClubPhase::BattleSetup
            | CableClubPhase::Battle => Some(LinkKind::Battle),
            CableClubPhase::TradeSelect
            | CableClubPhase::TradeWaitingPeer
            | CableClubPhase::TradeConfirm { .. }
            | CableClubPhase::TradeWaitingConfirm
            | CableClubPhase::TradeAnim
            | CableClubPhase::TradeCompleted => Some(LinkKind::Trade),
            CableClubPhase::Inactive | CableClubPhase::InRoom | CableClubPhase::Error { .. } => None,
        }
    }

    /// The box text to draw over the map, if any.
    pub fn text_box(&self) -> Option<String> {
        match &self.phase {
            CableClubPhase::JustAMoment { .. } => Some(TEXT_JUST_A_MOMENT.to_string()),
            CableClubPhase::WaitingResponse { .. } => Some(TEXT_WAITING.to_string()),
            CableClubPhase::Exchanging => Some(TEXT_PLEASE_WAIT.to_string()),
            CableClubPhase::TradeWaitingPeer | CableClubPhase::TradeWaitingConfirm => {
                Some(TEXT_WAITING.to_string())
            }
            CableClubPhase::TradeCompleted => Some(TEXT_TRADE_COMPLETED.to_string()),
            CableClubPhase::Error { text } => Some(text.clone()),
            // The transient box ("The link was canceled." / "Too bad! The
            // trade was canceled!") also shows while idling in the room or
            // back in the trade selection.
            CableClubPhase::InRoom | CableClubPhase::TradeSelect => self.transient_text.clone(),
            _ => None,
        }
    }

    /// The yes/no prompt to draw, if any: `(title, selected index)`.
    pub fn prompt(&self) -> Option<(String, u8)> {
        match &self.phase {
            CableClubPhase::PeerPrompt { kind, selected } => {
                let title = match kind {
                    LinkKind::Battle => TEXT_PROMPT_BATTLE,
                    LinkKind::Trade => TEXT_PROMPT_TRADE,
                };
                Some((title.to_string(), *selected))
            }
            CableClubPhase::TradeConfirm { selected, .. } => {
                // The original's `_WillBeTradedText` names BOTH mons
                // ("{GIVE} and {RECEIVE} will be traded."); the peer's mon
                // species only reaches us with `TradeComplete` (after both
                // confirmations), so v1 names only the local mon — the wire
                // protocol carries no selection metadata. Documented
                // deviation; the peer's mon name appears in the cutscene.
                let local_name = self
                    .selector
                    .as_ref()
                    .and_then(|s| s.party().get(self.trade_confirm_local_index()))
                    .map(|m| m.display_name())
                    .unwrap_or_default();
                Some((format!("{} will\nbe traded.", local_name), *selected))
            }
            _ => None,
        }
    }

    fn trade_confirm_local_index(&self) -> usize {
        match &self.phase {
            CableClubPhase::TradeConfirm { local_index, .. } => *local_index as usize,
            _ => 0,
        }
    }

    /// The active trade party selector (renderer input), if any.
    pub fn party_select(&self) -> Option<&PartySelectState> {
        self.selector.as_ref()
    }

    /// Keep the flow in sync with the connection + room. Called every frame
    /// by the game loop while the overworld is not frozen by the flow.
    pub fn note_presence(&mut self, connected: bool, in_cable_room: bool) {
        if connected && in_cable_room {
            if matches!(self.phase, CableClubPhase::Inactive) {
                self.phase = CableClubPhase::InRoom;
            }
        } else if matches!(
            self.phase,
            CableClubPhase::Inactive | CableClubPhase::InRoom
        ) {
            self.phase = CableClubPhase::Inactive;
        }
    }

    /// The player used the gameboy on the table (the map scene called
    /// `game.linkStart()`). Starts the request for the room's activity.
    pub fn on_gameboy_used(&mut self, map: MapId) -> FlowNeed {
        let kind = link_kind_for_room(map);
        match self.phase {
            CableClubPhase::InRoom | CableClubPhase::Inactive => {
                self.phase = CableClubPhase::JustAMoment { kind };
                self.transient_text = None;
                FlowNeed::RequestLink(kind)
            }
            // Already mid-flow (e.g. the peer requested while the player was
            // at the table): pressing A again answers the pending request.
            CableClubPhase::PeerPrompt { kind, .. } => FlowNeed::ReplyRequest {
                kind,
                accept: true,
            },
            _ => FlowNeed::None,
        }
    }

    /// Drive one frame of modal input (only while `is_modal()`). `party` is
    /// the save's current party (used to (re)build the trade selector).
    pub fn update(&mut self, input: PartyScreenInput, party: &[Pokemon]) -> FlowNeed {
        match self.phase.clone() {
            CableClubPhase::JustAMoment { kind } => {
                if input.a {
                    self.phase = CableClubPhase::WaitingResponse { kind };
                }
                FlowNeed::None
            }
            CableClubPhase::PeerPrompt { kind, mut selected } => {
                if input.up {
                    selected = 0;
                    self.phase = CableClubPhase::PeerPrompt { kind, selected };
                } else if input.down {
                    selected = 1;
                    self.phase = CableClubPhase::PeerPrompt { kind, selected };
                } else if input.a || input.b {
                    let accept = input.a && selected == 0;
                    let need = FlowNeed::ReplyRequest { kind, accept };
                    self.phase = match (kind, accept) {
                        (LinkKind::Battle, true) => CableClubPhase::Exchanging,
                        (LinkKind::Trade, true) => CableClubPhase::TradeSelect,
                        _ => CableClubPhase::InRoom,
                    };
                    if !accept {
                        self.transient_text = Some(TEXT_LINK_CANCELED.to_string());
                    }
                    return need;
                }
                FlowNeed::None
            }
            CableClubPhase::TradeSelect => {
                if self.transient_text.is_some() && (input.a || input.b) {
                    // Dismiss the "trade was canceled" box first; the next
                    // press interacts with the list (original: TradeCanceled
                    // text then the selection menu resumes).
                    self.transient_text = None;
                    return FlowNeed::None;
                }
                if self.selector.is_none() {
                    // Fresh selector from the CURRENT party: after a completed
                    // trade the party changed, so the old selector (if any)
                    // was dropped on the way back here.
                    self.selector = Some(PartySelectState::new(party.to_vec()));
                }
                if let Some(ref mut sel) = self.selector {
                    match sel.update_frame(input) {
                        pokered_core::party_select::PartySelectResult::Selected(idx) => {
                            self.phase = CableClubPhase::TradeWaitingPeer;
                            FlowNeed::SelectMon(idx as u8)
                        }
                        pokered_core::party_select::PartySelectResult::Cancelled => {
                            self.pending_our_cancel = true;
                            FlowNeed::CancelTrade
                        }
                        pokered_core::party_select::PartySelectResult::Active => FlowNeed::None,
                    }
                } else {
                    FlowNeed::None
                }
            }
            CableClubPhase::TradeConfirm {
                local_index,
                remote_index,
                mut selected,
            } => {
                if input.up || input.down {
                    selected = 1 - selected;
                    self.phase = CableClubPhase::TradeConfirm {
                        local_index,
                        remote_index,
                        selected,
                    };
                    FlowNeed::None
                } else if input.a || input.b {
                    let confirm = input.a && selected == 0;
                    if confirm {
                        self.phase = CableClubPhase::TradeWaitingConfirm;
                        FlowNeed::ConfirmTrade
                    } else {
                        self.phase = CableClubPhase::TradeSelect;
                        self.transient_text = Some(TEXT_TRADE_CANCELED.to_string());
                        self.pending_our_cancel = true;
                        FlowNeed::CancelTrade
                    }
                } else {
                    FlowNeed::None
                }
            }
            CableClubPhase::TradeCompleted => {
                if input.a || input.b {
                    // Back to the room: the trade manager was reset to Idle
                    // after the exchange (the original loops straight into
                    // the selection screen via CableClub_DoBattleOrTradeAgain;
                    // ours requires a fresh gameboy use — documented
                    // deviation, one extra press).
                    self.selector = None;
                    self.phase = CableClubPhase::InRoom;
                }
                FlowNeed::None
            }
            CableClubPhase::Error { .. } => {
                if input.a || input.b {
                    self.reset();
                }
                FlowNeed::None
            }
            CableClubPhase::InRoom => {
                if self.transient_text.is_some() && input.a {
                    self.transient_text = None;
                }
                FlowNeed::None
            }
            // Modal phases with no input (waiting for the peer).
            _ => FlowNeed::None,
        }
    }

    /// The game loop executed a [`FlowNeed`]; confirm bookkeeping.
    pub fn on_need_done(&mut self, need: &FlowNeed) {
        match need {
            FlowNeed::SelectMon(idx) => {
                // If the peer's pick already arrived (PeerSelectedMon before
                // our own selection), both sides are selected: move straight
                // to the confirm box. (The side whose selection arrives LAST
                // gets the BothSelected event instead — the manager only
                // reports the transition once per received message.)
                if let Some(remote) = self.remote_selection {
                    self.phase = CableClubPhase::TradeConfirm {
                        local_index: *idx,
                        remote_index: remote,
                        selected: 0,
                    };
                }
            }
            FlowNeed::ConfirmTrade => {
                self.remote_selection = None;
            }
            FlowNeed::ReplyRequest { accept, .. } => {
                if *accept {
                    self.transient_text = None;
                }
            }
            _ => {}
        }
    }

    /// The party exchange was rejected by the transport (send failed).
    pub fn on_session_error(&mut self, text: String) {
        self.phase = CableClubPhase::Error { text };
        self.clear_selection();
    }

    /// The game loop started the link battle (`BattleScreen` + link mode).
    pub fn on_battle_started(&mut self) {
        self.phase = CableClubPhase::Battle;
    }

    /// The battle ended (normal end or disconnect teardown) and the game
    /// loop returned to the overworld room.
    pub fn on_battle_ended(&mut self) {
        self.phase = CableClubPhase::InRoom;
    }

    /// The game loop started the trade cutscene (`trade_anim`).
    pub fn on_trade_anim_started(&mut self) {
        self.phase = CableClubPhase::TradeAnim;
    }

    /// The trade cutscene finished; the exchange was applied and the box
    /// shows "Trade completed!" — A returns to the selection screen (the
    /// original loops via `CableClub_DoBattleOrTradeAgain`).
    pub fn on_trade_anim_done(&mut self) {
        self.phase = CableClubPhase::TradeCompleted;
    }

    /// Route a battle driver event (from
    /// [`LinkBattleDriver::poll`](pokered_core::battle::link_battle_driver::LinkBattleDriver::poll)).
    pub fn on_battle_event(&mut self, ev: &LinkDriverEvent) -> FlowNeed {
        use LinkDriverEvent::*;
        match ev {
            Connected => FlowNeed::None,
            BattleRequested => match self.phase {
                CableClubPhase::InRoom | CableClubPhase::Inactive => {
                    self.phase = CableClubPhase::PeerPrompt {
                        kind: LinkKind::Battle,
                        selected: 0,
                    };
                    FlowNeed::None
                }
                _ => FlowNeed::None,
            },
            BattleAccepted => {
                // Reached via: our request accepted, or (guest role) the
                // simultaneous-gameboy auto-accept. The driver already sent
                // our party data with the accept.
                self.phase = CableClubPhase::Exchanging;
                FlowNeed::None
            }
            BattleDeclined => {
                self.phase = CableClubPhase::InRoom;
                self.transient_text = Some(TEXT_LINK_CANCELED.to_string());
                FlowNeed::None
            }
            BattleStarted => {
                // Both parties exchanged — the driver built the battle
                // screen; the game loop mirrors it and transitions.
                self.phase = CableClubPhase::BattleSetup;
                FlowNeed::None
            }
            // Informational (the driver resolves the battle itself; the
            // result is also in `LinkBattleDriver::result`).
            BattleResult(_) | RemoteResult(_) => FlowNeed::None,
            Disconnected(_) => {
                self.on_disconnected();
                FlowNeed::None
            }
        }
    }

    /// Route a trade driver poll result (from
    /// [`LinkTradeDriver::poll`](pokered_core::link::link_trade::LinkTradeDriver::poll)).
    pub fn on_trade_event(&mut self, ev: &LinkTradePollResult) -> FlowNeed {
        use LinkTradePollResult::*;
        match ev {
            Pending => FlowNeed::None,
            TradeRequested => match self.phase {
                CableClubPhase::InRoom | CableClubPhase::Inactive => {
                    self.phase = CableClubPhase::PeerPrompt {
                        kind: LinkKind::Trade,
                        selected: 0,
                    };
                    FlowNeed::None
                }
                _ => FlowNeed::None,
            },
            TradeAccepted => {
                self.phase = CableClubPhase::TradeSelect;
                FlowNeed::None
            }
            TradeDeclined => {
                self.phase = CableClubPhase::InRoom;
                self.transient_text = Some(TEXT_LINK_CANCELED.to_string());
                FlowNeed::None
            }
            PeerSelectedMon(idx) => {
                self.remote_selection = Some(*idx);
                FlowNeed::None
            }
            BothSelected {
                local_index,
                remote_index,
            } => {
                self.remote_selection = Some(*remote_index);
                self.phase = CableClubPhase::TradeConfirm {
                    local_index: *local_index,
                    remote_index: *remote_index,
                    selected: 0,
                };
                FlowNeed::None
            }
            PeerConfirmed => FlowNeed::None,
            TradeExecute { .. } => {
                // The exchange is in the driver (`received_mon`); the game
                // loop starts the cutscene from there.
                self.phase = CableClubPhase::TradeAnim;
                FlowNeed::None
            }
            PeerCancelled => {
                if self.pending_our_cancel {
                    // We cancelled too — BOTH sides backed out: return to the
                    // room (original's ReturnToCableClubRoom after the
                    // both-cancel nybble, engine/link/cable_club.asm:571-580).
                    self.pending_our_cancel = false;
                    self.phase = CableClubPhase::InRoom;
                    self.transient_text = None;
                } else if matches!(
                    self.phase,
                    CableClubPhase::TradeSelect
                        | CableClubPhase::TradeWaitingPeer
                        | CableClubPhase::TradeConfirm { .. }
                        | CableClubPhase::TradeWaitingConfirm
                ) {
                    // The peer cancelled unilaterally: "Too bad! The trade
                    // was canceled!" then back to selection
                    // (engine/link/cable_club.asm:871-876).
                    self.phase = CableClubPhase::TradeSelect;
                    self.transient_text = Some(TEXT_TRADE_CANCELED.to_string());
                }
                FlowNeed::None
            }
            Disconnected => {
                self.on_disconnected();
                FlowNeed::None
            }
            Error(e) => {
                self.phase = CableClubPhase::Error {
                    text: format!("link error: {}", e),
                };
                self.clear_selection();
                FlowNeed::None
            }
        }
    }

    /// The link was lost: error box with the original's per-phase text (the
    /// trade flows show "Too bad! The trade was canceled!", everything else
    /// "The link was canceled."). Shared by both drivers' `Disconnected`
    /// events.
    fn on_disconnected(&mut self) {
        let text = match self.phase {
            CableClubPhase::TradeSelect
            | CableClubPhase::TradeWaitingPeer
            | CableClubPhase::TradeConfirm { .. }
            | CableClubPhase::TradeWaitingConfirm
            | CableClubPhase::TradeAnim => TEXT_TRADE_CANCELED.to_string(),
            _ => TEXT_LINK_CANCELED.to_string(),
        };
        self.phase = CableClubPhase::Error { text };
        self.clear_selection();
    }

    fn clear_selection(&mut self) {
        self.selector = None;
        self.remote_selection = None;
        self.transient_text = None;
        self.pending_our_cancel = false;
    }

    fn reset(&mut self) {
        self.phase = CableClubPhase::Inactive;
        self.clear_selection();
    }
}

impl Default for CableClubFlow {
    fn default() -> Self {
        Self::new()
    }
}

/// The room decides battle vs trade, as in the original
/// (`CableClubLeftGameboy`/`CableClubRightGameboy` set the link state by
/// `wCurMap == TRADE_CENTER`, engine/pokemon/bills_pc.asm:511-532).
pub fn link_kind_for_room(map: MapId) -> LinkKind {
    match map {
        MapId::Colosseum => LinkKind::Battle,
        _ => LinkKind::Trade,
    }
}

/// True when the map is one of the Cable Club rooms.
pub fn is_cable_room(map: MapId) -> bool {
    matches!(map, MapId::Colosseum | MapId::TradeCenter)
}
