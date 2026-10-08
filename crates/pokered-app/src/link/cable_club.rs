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

use crate::alloc_prelude::*;
use pokered_core::battle::link_battle_driver::LinkDriverEvent;
use pokered_core::battle::state::Pokemon;
use pokered_core::link::link_trade::LinkTradePollResult;
use pokered_core::party_screen::PartyScreenInput;
use pokered_core::party_select::PartySelectState;
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
    SaveReception,
    CancelReception,
    CancelRoomSelection,
    EnterRoom(LinkKind),
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
    /// Send the original party-list CANCEL choice.
    CancelTrade,
    /// Both parties selected CANCEL: send our choice before leaving.
    CancelTradeAndLeave,
    /// Send NO from the trade confirmation menu.
    RejectTrade,
    /// Send our mon choice against a previously received CANCEL.
    SelectMonAgainstCancel(u8),
    /// Discard exchanged indices and resume the current selection menu.
    ResumeSelection,
    /// Confirm the trade; payload waits until both players choose YES.
    ConfirmTrade,
    ContinueTrade,
    LeaveTrade,
}

/// The in-room link flow phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CableClubPhase {
    /// No link session, or not inside a Cable Club room.
    Inactive,
    ReceptionText,
    ReceptionSave {
        selected: u8,
    },
    ReceptionMenu {
        selected: u8,
    },
    /// LinkMenu keeps the selected cursor visible for 40 frames.
    ReceptionChosen {
        selected: u8,
        frames_left: u8,
    },
    ReceptionWait {
        kind: LinkKind,
        frames_left: u8,
    },
    ReceptionWarpDelay {
        kind: LinkKind,
        frames_left: u8,
    },
    ReceptionCancelDelay {
        frames_left: u8,
    },
    /// Connected and inside Colosseum/TradeCenter: the remote player's
    /// avatar is present; the gameboy on the table is live.
    InRoom,
    /// The player used their gameboy: "Just a moment." before serial work.
    JustAMoment {
        kind: LinkKind,
    },
    /// CableClub_DoBattleOrTrade waits 80 frames after the text closes.
    GameboyDelay { kind: LinkKind, frames_left: u8 },
    /// Our serial exchange is in flight — modal "PLEASE WAIT!" box.
    WaitingResponse {
        kind: LinkKind,
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
            CableClubPhase::ReceptionSave { .. }
                | CableClubPhase::ReceptionMenu { .. }
                | CableClubPhase::ReceptionChosen { .. }
                | CableClubPhase::ReceptionWait { .. }
                | CableClubPhase::ReceptionWarpDelay { .. }
                | CableClubPhase::ReceptionCancelDelay { .. }
                | CableClubPhase::JustAMoment { .. }
                | CableClubPhase::GameboyDelay { .. }
                | CableClubPhase::WaitingResponse { .. }
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
    pending_peer_request: Option<LinkKind>,
    /// Trade party selector (created when the trade menu opens).
    selector: Option<PartySelectState>,
    remote_party: Vec<Pokemon>,
    local_name: String,
    remote_name: String,
    peer_cursor: usize,
    browsing_peer: bool,
    stats: Option<pokered_core::stats_screen::StatsScreenState>,
    /// Original chosePlayerMon opens STATS / TRADE before sending a selection.
    local_action: Option<(usize, bool)>, // mon index, TRADE selected
    peer_confirmation: Option<bool>,
    local_rejected: bool,
    rejection_frames: Option<u16>,
    completed_frames: u16,
    cancel_selected: bool,
    peer_cancel_pending: bool,
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
pub const TEXT_RECEPTION_WAIT: &str = "OK, please wait\njust a moment.";
pub const TEXT_TRADE_COMPLETED: &str = "Trade completed!";
pub const TEXT_TRADE_CANCELED: &str = "Too bad! The trade\nwas canceled!";
pub const TEXT_LINK_CANCELED: &str = "The link was\ncanceled.";

impl CableClubFlow {
    pub fn new() -> Self {
        CableClubFlow {
            phase: CableClubPhase::Inactive,
            pending_peer_request: None,
            selector: None,
            remote_party: Vec::new(),
            local_name: String::new(),
            remote_name: String::new(),
            peer_cursor: 0,
            browsing_peer: false,
            stats: None,
            local_action: None,
            peer_confirmation: None,
            local_rejected: false,
            rejection_frames: None,
            completed_frames: 0,
            cancel_selected: false,
            peer_cancel_pending: false,
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
            | CableClubPhase::ReceptionWait { kind, .. }
            | CableClubPhase::ReceptionWarpDelay { kind, .. }
            | CableClubPhase::GameboyDelay { kind, .. }
            | CableClubPhase::WaitingResponse { kind }
            => Some(*kind),
            CableClubPhase::Exchanging | CableClubPhase::BattleSetup | CableClubPhase::Battle => {
                Some(LinkKind::Battle)
            }
            CableClubPhase::TradeSelect
            | CableClubPhase::TradeWaitingPeer
            | CableClubPhase::TradeConfirm { .. }
            | CableClubPhase::TradeWaitingConfirm
            | CableClubPhase::TradeAnim
            | CableClubPhase::TradeCompleted => Some(LinkKind::Trade),
            CableClubPhase::ReceptionText
            | CableClubPhase::Inactive
            | CableClubPhase::InRoom
            | CableClubPhase::Error { .. }
            | CableClubPhase::ReceptionSave { .. }
            | CableClubPhase::ReceptionMenu { .. }
            | CableClubPhase::ReceptionChosen { .. }
            | CableClubPhase::ReceptionCancelDelay { .. } => None,
        }
    }

    /// The box text to draw over the map, if any.
    pub fn text_box(&self) -> Option<String> {
        match &self.phase {
            CableClubPhase::ReceptionWait { .. } | CableClubPhase::ReceptionWarpDelay { .. } => {
                Some(TEXT_RECEPTION_WAIT.to_string())
            }
            CableClubPhase::JustAMoment { .. } => Some(TEXT_JUST_A_MOMENT.to_string()),
            CableClubPhase::WaitingResponse { .. } => Some(TEXT_PLEASE_WAIT.to_string()),
            CableClubPhase::Exchanging => Some(TEXT_PLEASE_WAIT.to_string()),
            CableClubPhase::TradeWaitingPeer | CableClubPhase::TradeWaitingConfirm => {
                self.transient_text.clone().or_else(|| Some(TEXT_WAITING.to_string()))
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
            CableClubPhase::ReceptionSave { selected } => Some((
                "the link, we have\nto save the game.".to_string(),
                *selected,
            )),
            CableClubPhase::TradeConfirm { selected, .. } => {
                let local_name = self
                    .selector
                    .as_ref()
                    .and_then(|s| {
                        let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
                        s.party()
                            .get(self.trade_confirm_local_index())
                            .map(|m| m.display_name(&mut name_buf).to_string())
                    })
                    .unwrap_or_default();
                let mut buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
                let remote_name = self
                    .remote_selection
                    .and_then(|i| self.remote_party.get(i as usize))
                    .map(|m| m.display_name(&mut buf))
                    .unwrap_or("...");
                Some((
                    format!("{} and\n{} will be traded.", local_name, remote_name),
                    *selected,
                ))
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

    pub fn set_trainer_names(&mut self, local: &str, remote: &str) {
        if self.local_name != local {
            self.local_name = local.to_string();
        }
        if self.remote_name != remote {
            self.remote_name = remote.to_string();
        }
    }

    pub fn trainer_names(&self) -> (&str, &str) {
        (&self.local_name, &self.remote_name)
    }

    pub fn set_remote_party(&mut self, party: &[Pokemon]) {
        if self.remote_party != party {
            self.remote_party = party.to_vec();
            self.peer_cursor = self.peer_cursor.min(party.len().saturating_sub(1));
        }
    }

    pub fn remote_party(&self) -> &[Pokemon] {
        &self.remote_party
    }
    pub fn peer_cursor(&self) -> Option<usize> {
        (self.browsing_peer && !self.cancel_selected).then_some(self.peer_cursor)
    }
    pub fn cancel_selected(&self) -> bool { self.cancel_selected }

    pub fn local_action(&self) -> Option<(usize, bool)> {
        self.local_action
    }

    pub fn stats(&self) -> Option<&pokered_core::stats_screen::StatsScreenState> {
        self.stats.as_ref()
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
            self.pending_peer_request = None;
        }
    }

    /// The player used the gameboy on the table (the map scene called
    /// `game.linkStart()`). Starts the request for the room's activity.
    pub fn on_receptionist_used(&mut self) {
        self.phase = CableClubPhase::ReceptionText;
        self.transient_text = None;
    }

    pub fn on_reception_text_done(&mut self) {
        self.phase = CableClubPhase::ReceptionSave { selected: 0 };
        self.transient_text = None;
    }

    pub fn reception_menu(&self) -> Option<u8> {
        match self.phase {
            CableClubPhase::ReceptionMenu { selected }
            | CableClubPhase::ReceptionChosen { selected, .. } => Some(selected),
            _ => None,
        }
    }

    pub fn on_gameboy_used(&mut self, map: MapId) -> FlowNeed {
        if matches!(self.phase, CableClubPhase::InRoom | CableClubPhase::Inactive) {
            self.phase = CableClubPhase::JustAMoment { kind: link_kind_for_room(map) };
            self.transient_text = None;
        }
        FlowNeed::None
    }

    /// HandleMenuInput plays PRESS_AB only when a watched key exits the
    /// menu. The shared CANCEL row polls Joypad directly and stays silent.
    pub fn menu_button_sound(&self, input: PartyScreenInput, left: bool, right: bool) -> bool {
        if !(input.a || input.b) || self.stats.is_some() || self.rejection_frames.is_some() {
            return false;
        }
        match self.phase {
            CableClubPhase::TradeSelect if !self.cancel_selected => {
                if self.local_action.is_some() { return true; }
                input.a || input.down || (self.browsing_peer && left) || (!self.browsing_peer && right)
            }
            CableClubPhase::TradeConfirm { .. } => true,
            _ => false,
        }
    }

    /// Drive one frame of modal input (only while `is_modal()`). `party` is
    /// the save's current party (used to (re)build the trade selector).
    pub fn update(&mut self, input: PartyScreenInput, party: &[Pokemon]) -> FlowNeed {
        self.update_with_navigation(input, party, false)
    }

    pub fn update_with_navigation(
        &mut self,
        input: PartyScreenInput,
        party: &[Pokemon],
        switch_side: bool,
    ) -> FlowNeed {
        self.update_with_horizontal_navigation(
            input, party,
            switch_side && self.browsing_peer,
            switch_side && !self.browsing_peer,
        )
    }

    /// Separate horizontal keys preserve the original STATS / TRADE menu:
    /// LEFT selects STATS, RIGHT selects TRADE, rather than toggling either key.
    pub fn update_with_horizontal_navigation(
        &mut self,
        input: PartyScreenInput,
        party: &[Pokemon],
        left: bool,
        right: bool,
    ) -> FlowNeed {
        if let Some(remaining) = self.rejection_frames.as_mut() {
            *remaining = remaining.saturating_sub(1);
            if *remaining == 0 {
                self.restart_selection();
                return FlowNeed::ContinueTrade;
            }
            return FlowNeed::None;
        }
        if let Some(stats) = &mut self.stats {
            use pokered_core::stats_screen::{StatsScreenAction, StatsScreenInput};
            if stats.update(StatsScreenInput {
                a: input.a,
                b: input.b,
            }) == StatsScreenAction::BackToParty
            {
                self.stats = None;
            }
            return FlowNeed::None;
        }
        if let Some((index, trade_selected)) = self.local_action {
            // Only the watched direction exits HandleMenuInput, and that
            // branch precedes A/B in the original STATS / TRADE loops.
            if (!trade_selected && right) || (trade_selected && left) {
                self.local_action = Some((index, !trade_selected));
                return FlowNeed::None;
            }
            if input.b {
                self.local_action = None;
            } else if input.a {
                self.local_action = None;
                if trade_selected {
                    if self.peer_cancel_pending {
                        self.restart_selection();
                        return FlowNeed::SelectMonAgainstCancel(index as u8);
                    }
                    self.phase = CableClubPhase::TradeWaitingPeer;
                    return FlowNeed::SelectMon(index as u8);
                }
                if let Some(mon) = self.selector.as_ref().and_then(|sel| sel.party().get(index)) {
                    self.stats = Some(pokered_core::stats_screen::StatsScreenState::new(mon.clone()));
                }
            }
            return FlowNeed::None;
        }
        match self.phase.clone() {
            CableClubPhase::ReceptionSave { mut selected } => {
                if input.up {
                    selected = 0;
                }
                if input.down {
                    selected = 1;
                }
                self.phase = CableClubPhase::ReceptionSave { selected };
                if input.b || (input.a && selected == 1) {
                    self.phase = CableClubPhase::Inactive;
                    return FlowNeed::CancelReception;
                } else if input.a {
                    self.phase = CableClubPhase::ReceptionMenu { selected: 0 };
                    return FlowNeed::SaveReception;
                }
                FlowNeed::None
            }
            CableClubPhase::ReceptionMenu { mut selected } => {
                if input.up {
                    selected = (selected + 2) % 3;
                }
                if input.down {
                    selected = (selected + 1) % 3;
                }
                self.phase = CableClubPhase::ReceptionMenu { selected };
                if input.b || input.a {
                    self.phase = CableClubPhase::ReceptionChosen {
                        selected: if input.b { 2 } else { selected },
                        frames_left: 40,
                    };
                }
                FlowNeed::None
            }
            CableClubPhase::ReceptionChosen {
                selected,
                frames_left,
            } => {
                if frames_left > 1 {
                    self.phase = CableClubPhase::ReceptionChosen {
                        selected,
                        frames_left: frames_left - 1,
                    };
                } else if selected == 2 {
                    self.phase = CableClubPhase::ReceptionCancelDelay { frames_left: 3 };
                } else {
                    self.phase = CableClubPhase::ReceptionWait {
                        kind: if selected == 0 {
                            LinkKind::Trade
                        } else {
                            LinkKind::Battle
                        },
                        frames_left: 50,
                    };
                }
                FlowNeed::None
            }
            CableClubPhase::ReceptionWait { kind, frames_left } => {
                self.phase = if frames_left > 1 {
                    CableClubPhase::ReceptionWait {
                        kind,
                        frames_left: frames_left - 1,
                    }
                } else {
                    CableClubPhase::ReceptionWarpDelay {
                        kind,
                        frames_left: 20,
                    }
                };
                FlowNeed::None
            }
            CableClubPhase::ReceptionWarpDelay { kind, frames_left } => {
                if frames_left > 1 {
                    self.phase = CableClubPhase::ReceptionWarpDelay {
                        kind,
                        frames_left: frames_left - 1,
                    };
                    FlowNeed::None
                } else {
                    self.phase = CableClubPhase::Inactive;
                    FlowNeed::EnterRoom(kind)
                }
            }
            CableClubPhase::ReceptionCancelDelay { frames_left } => {
                if frames_left > 1 {
                    self.phase = CableClubPhase::ReceptionCancelDelay {
                        frames_left: frames_left - 1,
                    };
                    FlowNeed::None
                } else {
                    self.phase = CableClubPhase::Inactive;
                    FlowNeed::CancelRoomSelection
                }
            }
            CableClubPhase::JustAMoment { kind } => {
                if input.a || input.b {
                    self.phase = CableClubPhase::GameboyDelay { kind, frames_left: 80 };
                }
                FlowNeed::None
            }
            CableClubPhase::GameboyDelay { kind, frames_left } => {
                if frames_left > 1 {
                    self.phase = CableClubPhase::GameboyDelay { kind, frames_left: frames_left - 1 };
                    FlowNeed::None
                } else {
                    self.phase = CableClubPhase::WaitingResponse { kind };
                    if self.pending_peer_request.take() == Some(kind) {
                        FlowNeed::ReplyRequest { kind, accept: true }
                    } else {
                        FlowNeed::RequestLink(kind)
                    }
                }
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
                // The shared bottom CANCEL item watches only A and UP.
                // UP always returns to the last member of the player's list.
                if self.cancel_selected {
                    if input.a {
                        if self.peer_cancel_pending {
                            self.phase = CableClubPhase::InRoom;
                            self.clear_selection();
                            return FlowNeed::CancelTradeAndLeave;
                        }
                        self.pending_our_cancel = true;
                        self.phase = CableClubPhase::TradeWaitingPeer;
                        return FlowNeed::CancelTrade;
                    } else if input.up {
                        self.cancel_selected = false;
                        self.browsing_peer = false;
                        if let Some(sel) = self.selector.as_mut() {
                            sel.set_cursor(sel.party().len().saturating_sub(1));
                        }
                    }
                    return FlowNeed::None;
                }
                // HandleMenuInput updates the vertical cursor before
                // returning watched A/LEFT/RIGHT. A then precedes a side swap.
                let (cursor, count) = if self.browsing_peer {
                    (self.peer_cursor, self.remote_party.len())
                } else {
                    let sel = self.selector.as_ref().unwrap();
                    (sel.cursor(), sel.party().len())
                };
                if count == 0 { return FlowNeed::None; }
                let next = if input.up { cursor.saturating_sub(1) }
                    else if input.down { (cursor + 1).min(count) }
                    else { cursor };
                self.cancel_selected = next == count;
                if self.browsing_peer {
                    self.peer_cursor = next.min(count - 1);
                } else if let Some(sel) = self.selector.as_mut() {
                    sel.set_cursor(next.min(count - 1));
                }
                if input.a {
                    self.cancel_selected = false;
                    if self.browsing_peer {
                        self.stats = Some(pokered_core::stats_screen::StatsScreenState::new(
                            self.remote_party[self.peer_cursor].clone(),
                        ));
                    } else if !self.remote_party.is_empty() {
                        self.local_action = Some((next.min(count - 1), false));
                    }
                    return FlowNeed::None;
                }
                if left && self.browsing_peer {
                    if let Some(sel) = self.selector.as_mut() { sel.set_cursor(next); }
                    self.cancel_selected = false;
                    self.browsing_peer = false;
                } else if right && !self.browsing_peer {
                    self.peer_cursor = next.min(self.remote_party.len().saturating_sub(1));
                    self.cancel_selected = false;
                    self.browsing_peer = true;
                }
                // B is not watched by either party list in TradeCenter_SelectMon.
                FlowNeed::None
            }
            CableClubPhase::TradeConfirm {
                local_index,
                remote_index,
                mut selected,
            } => {
                // DisplayTwoOptionMenu uses HandleMenuInput: clamp the
                // vertical cursor first, then return watched A/B. B always
                // chooses the second item, including simultaneous A+B.
                if input.up {
                    selected = 0;
                } else if input.down {
                    selected = 1;
                }
                self.phase = CableClubPhase::TradeConfirm {
                    local_index,
                    remote_index,
                    selected,
                };
                if input.a || input.b {
                    let confirm = !input.b && input.a && selected == 0;
                    self.phase = CableClubPhase::TradeWaitingConfirm;
                    if confirm {
                        if self.peer_confirmation == Some(false) { self.start_rejection_delay(false); }
                        FlowNeed::ConfirmTrade
                    } else {
                        self.local_rejected = true;
                        self.transient_text = Some(TEXT_TRADE_CANCELED.to_string());
                        if self.peer_confirmation.is_some() { self.start_rejection_delay(false); }
                        FlowNeed::RejectTrade
                    }
                } else {
                    FlowNeed::None
                }
            }
            CableClubPhase::TradeCompleted => {
                self.completed_frames = self.completed_frames.saturating_sub(1);
                if self.completed_frames == 0 {
                    self.restart_selection();
                    self.selector = Some(PartySelectState::new(party.to_vec()));
                    return FlowNeed::ContinueTrade;
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
            FlowNeed::ReplyRequest { kind, accept } => {
                if *accept {
                    self.transient_text = None;
                    self.phase = match kind {
                        LinkKind::Trade => CableClubPhase::TradeSelect,
                        LinkKind::Battle => CableClubPhase::Exchanging,
                    };
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
    /// shows "Trade completed!" for 50 frames before returning automatically
    /// (`CableClub_DoBattleOrTradeAgain`).
    pub fn on_trade_anim_done(&mut self) {
        self.completed_frames = 50;
        self.phase = CableClubPhase::TradeCompleted;
    }

    /// Route a battle driver event (from
    /// [`LinkBattleDriver::poll`](pokered_core::battle::link_battle_driver::LinkBattleDriver::poll)).
    pub fn on_battle_event(&mut self, ev: &LinkDriverEvent) -> FlowNeed {
        use LinkDriverEvent::*;
        match ev {
            Connected => FlowNeed::None,
            BattleRequested => {
                // Incoming serial readiness cannot interrupt walking or our
                // own gameboy text/delay. Each player starts at their table.
                if matches!(self.phase, CableClubPhase::InRoom | CableClubPhase::Inactive
                    | CableClubPhase::JustAMoment { .. } | CableClubPhase::GameboyDelay { .. }) {
                    self.pending_peer_request = Some(LinkKind::Battle);
                }
                FlowNeed::None
            }
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
            TradeRequested => {
                // Incoming serial readiness cannot interrupt walking or our
                // own gameboy text/delay. Each player starts at their table.
                if matches!(self.phase, CableClubPhase::InRoom | CableClubPhase::Inactive
                    | CableClubPhase::JustAMoment { .. } | CableClubPhase::GameboyDelay { .. }) {
                    self.pending_peer_request = Some(LinkKind::Trade);
                }
                FlowNeed::None
            }
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
                if self.pending_our_cancel {
                    // Our CANCEL exchanged against the peer's mon: the
                    // original resumes the CANCEL item instead of leaving.
                    self.pending_our_cancel = false;
                    self.remote_selection = None;
                    self.phase = CableClubPhase::TradeSelect;
                    self.cancel_selected = true;
                    return FlowNeed::ResumeSelection;
                }
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
            PeerConfirmed => {
                self.peer_confirmation = Some(true);
                if self.local_rejected { self.start_rejection_delay(true); }
                FlowNeed::None
            },
            TradeExecute { .. } => {
                // The exchange is in the driver (`received_mon`); the game
                // loop starts the cutscene from there.
                self.phase = CableClubPhase::TradeAnim;
                FlowNeed::None
            }
            PeerRejectedTrade => {
                self.peer_confirmation = Some(false);
                if self.phase == CableClubPhase::TradeWaitingConfirm { self.start_rejection_delay(true); }
                FlowNeed::None
            }
            PeerCancelled => {
                self.remote_selection = None;
                if self.pending_our_cancel {
                    self.phase = CableClubPhase::InRoom;
                    self.clear_selection();
                    return FlowNeed::LeaveTrade;
                }
                if matches!(self.phase, CableClubPhase::TradeConfirm { .. } | CableClubPhase::TradeWaitingConfirm) {
                    // Legacy clients used the same message for confirmation NO.
                    self.restart_selection();
                    self.transient_text = Some(TEXT_TRADE_CANCELED.to_string());
                } else if self.phase == CableClubPhase::TradeWaitingPeer {
                    // We sent a mon against $f: redraw the original fresh list.
                    self.restart_selection();
                } else if self.phase == CableClubPhase::TradeSelect {
                    // Serial $f waits until our own choice. Do not interrupt
                    // browsing, an action menu, or either status page.
                    self.peer_cancel_pending = true;
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
        self.pending_peer_request = None;
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

    fn start_rejection_delay(&mut self, before_frame_update: bool) {
        // Network events are delivered before this frame's modal update;
        // local choices are delivered inside it, after the timer tick.
        self.rejection_frames = Some(100 + u16::from(before_frame_update));
        self.transient_text = Some(TEXT_TRADE_CANCELED.to_string());
    }

    fn restart_selection(&mut self) {
        self.selector = None;
        self.stats = None;
        self.local_action = None;
        self.cancel_selected = false;
        self.peer_cancel_pending = false;
        self.peer_confirmation = None;
        self.local_rejected = false;
        self.rejection_frames = None;
        self.browsing_peer = false;
        self.remote_selection = None;
        self.transient_text = None;
        self.pending_our_cancel = false;
        self.phase = CableClubPhase::TradeSelect;
    }

    fn clear_selection(&mut self) {
        self.selector = None;
        self.remote_party.clear();
        self.stats = None;
        self.local_action = None;
        self.cancel_selected = false;
        self.peer_cancel_pending = false;
        self.peer_confirmation = None;
        self.local_rejected = false;
        self.rejection_frames = None;
        self.browsing_peer = false;
        self.remote_selection = None;
        self.transient_text = None;
        self.pending_our_cancel = false;
    }

    fn reset(&mut self) {
        self.pending_peer_request = None;
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

#[cfg(test)]
mod receptionist_fidelity_tests {
    use super::*;
    fn finish_room_delay(flow: &mut CableClubFlow, kind: LinkKind) {
        for _ in 0..40 {
            assert!(flow.reception_menu().is_some());
            assert_eq!(flow.update(PartyScreenInput::none(), &[]), FlowNeed::None);
        }
        assert_eq!(flow.text_box().as_deref(), Some(TEXT_RECEPTION_WAIT));
        for _ in 0..50 {
            assert_eq!(flow.update(PartyScreenInput::none(), &[]), FlowNeed::None);
        }
        assert!(matches!(
            flow.phase(),
            CableClubPhase::ReceptionWarpDelay {
                frames_left: 20,
                ..
            }
        ));
        for _ in 0..19 {
            assert_eq!(flow.update(PartyScreenInput::none(), &[]), FlowNeed::None);
        }
        assert_eq!(
            flow.update(PartyScreenInput::none(), &[]),
            FlowNeed::EnterRoom(kind)
        );
    }
    #[test]
    fn receptionist_requires_save_consent_then_room_selection() {
        let mut flow = CableClubFlow::new();
        flow.on_receptionist_used();
        assert!(!flow.is_modal());
        flow.on_reception_text_done();
        assert_eq!(
            flow.update(
                PartyScreenInput {
                    a: true,
                    ..PartyScreenInput::none()
                },
                &[]
            ),
            FlowNeed::SaveReception
        );
        assert_eq!(flow.reception_menu(), Some(0));
        flow.note_presence(true, false);
        assert_eq!(flow.reception_menu(), Some(0));
        assert_eq!(
            flow.update(
                PartyScreenInput {
                    a: true,
                    ..PartyScreenInput::none()
                },
                &[]
            ),
            FlowNeed::None
        );
        finish_room_delay(&mut flow, LinkKind::Trade);
        assert_eq!(flow.phase(), &CableClubPhase::Inactive);
    }
    #[test]
    fn receptionist_can_cancel_or_choose_colosseum() {
        let mut flow = CableClubFlow::new();
        flow.on_receptionist_used();
        assert!(!flow.is_modal());
        flow.on_reception_text_done();
        assert_eq!(
            flow.update(
                PartyScreenInput {
                    b: true,
                    ..PartyScreenInput::none()
                },
                &[]
            ),
            FlowNeed::CancelReception
        );
        assert_eq!(flow.phase(), &CableClubPhase::Inactive);
        flow.on_receptionist_used();
        assert!(!flow.is_modal());
        flow.on_reception_text_done();
        flow.update(
            PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            },
            &[],
        );
        flow.update(
            PartyScreenInput {
                down: true,
                ..PartyScreenInput::none()
            },
            &[],
        );
        assert_eq!(
            flow.update(
                PartyScreenInput {
                    a: true,
                    ..PartyScreenInput::none()
                },
                &[]
            ),
            FlowNeed::None
        );
        finish_room_delay(&mut flow, LinkKind::Battle);
    }
}

#[cfg(test)]
mod reception_room_cancel_tests {
    use super::*;
    #[test]
    fn b_and_cancel_entry_share_original_link_canceled_path_after_cursor_delay() {
        for use_b in [false, true] {
            let mut flow = CableClubFlow::new();
            flow.on_receptionist_used();
            flow.on_reception_text_done();
            flow.update(
                PartyScreenInput {
                    a: true,
                    ..PartyScreenInput::none()
                },
                &[],
            );
            if !use_b {
                for _ in 0..2 {
                    flow.update(
                        PartyScreenInput {
                            down: true,
                            ..PartyScreenInput::none()
                        },
                        &[],
                    );
                }
            }
            assert_eq!(
                flow.update(
                    PartyScreenInput {
                        a: !use_b,
                        b: use_b,
                        ..PartyScreenInput::none()
                    },
                    &[]
                ),
                FlowNeed::None
            );
            for _ in 0..42 {
                assert_eq!(flow.update(PartyScreenInput::none(), &[]), FlowNeed::None);
            }
            assert_eq!(
                flow.update(PartyScreenInput::none(), &[]),
                FlowNeed::CancelRoomSelection
            );
            assert_eq!(flow.phase(), &CableClubPhase::Inactive);
        }
    }
}
