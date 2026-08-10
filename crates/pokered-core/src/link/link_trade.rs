//! Two-sided link trade driver — port of the Cable Club trade flow
//! (`engine/link/cable_club.asm`) and the post-trade evolution check
//! (`TryEvolvingMon`, `engine/pokemon/evos_moves.asm`).
//!
//! [`LinkTradeManager`] is the wire-protocol state machine: request →
//! accept/decline → both sides select → both confirm → mon data exchanged
//! ([`LinkTradePollResult::TradeExecute`]). It owns no game data.
//!
//! [`LinkTradeDriver`] composes the manager with the local party and player
//! ID and implements the game rules, following the original:
//!
//! - **No selection restrictions.** Gen 1 lets a player trade ANY party
//!   member — fainted, HM-holding, or the last one. The trade menu lists
//!   every slot (`TradeCenter_SelectMon`: `wMaxMenuItem = wPartyCount`,
//!   cable_club.asm:393-400) and `.doTrade` calls `RemovePokemon` with no
//!   party-count guard. (HM-blocking and last-mon protection are later
//!   generations.)
//! - **The received mon keeps its data** (level / EXP / DVs / stats / moves /
//!   PP / PP-Ups / nickname / OT name / OT ID): the enemy party struct is
//!   copied wholesale (`wEnemyMons` → `wLoadedMon` →
//!   `AddEnemyMonToPlayerParty` copy, add_mon.asm:294-302) — our `Pokemon`
//!   serde round-trips the same fields through `TradeComplete`.
//! - **Remove-then-add**: `RemovePokemon` runs before
//!   `AddEnemyMonToPlayerParty` (cable_club.asm:800-817), so a full 6-mon
//!   party still has room for the incoming mon.
//! - **Post-trade updates**: the received species is flagged Pokédex
//!   seen + owned (`AddEnemyMonToPlayerParty`, add_mon.asm:325-337), and the
//!   OT stays the remote trainer's, which makes the mon "traded" for
//!   obedience / 1.5× EXP under the player's own ID (`is_traded_for`).
//! - **Trade evolution**: after the exchange, `TryEvolvingMon` runs on the
//!   received mon (cable_club.asm:851) with `wForceEvolution = TRUE`
//!   (cable_club.asm:822 — B-cancel is disabled, evolution.asm:156-158).
//!   While trading, only `EVOLVE_TRADE` entries can trigger (level entries
//!   are skipped, evos_moves.asm:70-94). The driver DETECTS the evolution
//!   and returns it as a [`PendingEvolution`]; the frontend plays the
//!   cutscene (`crate::evolution_screen`) and applies it with
//!   `crate::pokemon::evolution::finalize_evolution`, exactly like the
//!   battle settlement path (settle.rs detects → app cutscene → finalize).
//! - **Cancel** at any point before both confirmations returns both sides
//!   to selection ("Too bad! The trade was canceled!", cable_club.asm:871-876);
//!   the party is untouched until [`LinkTradeDriver::apply_exchange`].
//! - **Disconnect** anywhere aborts the trade with nothing applied.

use super::protocol::NetworkMessage;
use super::transport::{NetworkTransport, TransportError};
use crate::battle::obedience::is_traded_for;
use crate::battle::settlement::evolution::check_trade_evolution;
use crate::battle::state::Pokemon;
use crate::evolution_screen::PendingEvolution;
use crate::pokemon::party::Party;
use crate::pokemon::pokedex::Pokedex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTradeState {
    Idle,
    WaitingForTradeResponse,
    PeerRequestedTrade,
    SelectingMon,
    WaitingForPeerSelection,
    BothSelected { local_index: u8, remote_index: u8 },
    WaitingForPeerConfirm { local_index: u8, remote_index: u8 },
    PeerConfirmedWaitingLocal { local_index: u8, remote_index: u8 },
    Trading { local_index: u8, remote_index: u8 },
    Completed,
    Cancelled,
    Error(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum LinkTradePollResult {
    Pending,
    TradeRequested,
    TradeAccepted,
    TradeDeclined,
    PeerSelectedMon(u8),
    BothSelected {
        local_index: u8,
        remote_index: u8,
    },
    PeerConfirmed,
    TradeExecute {
        local_index: u8,
        remote_index: u8,
        received_pokemon: Pokemon,
    },
    PeerCancelled,
    Disconnected,
    Error(String),
}

#[derive(Debug)]
pub struct LinkTradeManager {
    state: LinkTradeState,
    local_selection: Option<u8>,
    remote_selection: Option<u8>,
    local_confirmed: bool,
    remote_confirmed: bool,
    /// Cable Club clock role, set by the session once the connection is up.
    /// Used to break the both-pressed-the-gameboy tie (see the
    /// `WaitingForTradeResponse` × `RequestTrade` arm).
    role: Option<crate::link::LinkRole>,
}

impl LinkTradeManager {
    pub fn new() -> Self {
        Self {
            state: LinkTradeState::Idle,
            local_selection: None,
            remote_selection: None,
            local_confirmed: false,
            remote_confirmed: false,
            role: None,
        }
    }

    /// Set the Cable Club clock role. When both players use the gameboy at
    /// (nearly) the same time, the original resolves the tie by letting the
    /// clocking (hosting) Game Boy win (`LinkMenu`:
    /// "The gameboy that is clocking the connection wins",
    /// engine/menus/main_menu.asm:216-220); the guest silently accepts the
    /// host's request instead of both sides waiting on an accept that never
    /// comes. `None` (the default) keeps single-initiator semantics.
    pub fn set_role(&mut self, role: crate::link::LinkRole) {
        self.role = Some(role);
    }

    pub fn state(&self) -> &LinkTradeState {
        &self.state
    }

    pub fn is_completed(&self) -> bool {
        matches!(
            self.state,
            LinkTradeState::Completed | LinkTradeState::Cancelled | LinkTradeState::Error(_)
        )
    }

    pub fn request_trade(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), TransportError> {
        if self.state != LinkTradeState::Idle {
            return Err(TransportError::IoError(
                "cannot request trade: not idle".into(),
            ));
        }
        transport.send(NetworkMessage::RequestTrade)?;
        self.state = LinkTradeState::WaitingForTradeResponse;
        Ok(())
    }

    pub fn accept_trade(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), TransportError> {
        if self.state != LinkTradeState::PeerRequestedTrade {
            return Err(TransportError::IoError("no pending trade request".into()));
        }
        transport.send(NetworkMessage::AcceptTrade)?;
        self.state = LinkTradeState::SelectingMon;
        Ok(())
    }

    pub fn decline_trade(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), TransportError> {
        if self.state != LinkTradeState::PeerRequestedTrade {
            return Err(TransportError::IoError("no pending trade request".into()));
        }
        transport.send(NetworkMessage::DeclineTrade)?;
        self.state = LinkTradeState::Idle;
        Ok(())
    }

    pub fn select_mon(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
        party_index: u8,
    ) -> Result<(), TransportError> {
        match &self.state {
            LinkTradeState::SelectingMon
            | LinkTradeState::WaitingForPeerSelection
            | LinkTradeState::BothSelected { .. } => {}
            _ => {
                return Err(TransportError::IoError("not in selection state".into()));
            }
        }
        transport.send(NetworkMessage::SelectMon(party_index))?;
        self.local_selection = Some(party_index);
        self.local_confirmed = false;
        self.remote_confirmed = false;
        self.try_transition_to_both_selected();
        Ok(())
    }

    pub fn confirm_trade(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
        pokemon: Pokemon,
    ) -> Result<(), TransportError> {
        let (local_idx, remote_idx) = match &self.state {
            LinkTradeState::BothSelected {
                local_index,
                remote_index,
            } => (*local_index, *remote_index),
            LinkTradeState::PeerConfirmedWaitingLocal {
                local_index,
                remote_index,
            } => (*local_index, *remote_index),
            _ => {
                return Err(TransportError::IoError("not in confirm state".into()));
            }
        };
        transport.send(NetworkMessage::ConfirmTrade)?;
        transport.send(NetworkMessage::TradeComplete(pokemon))?;
        self.local_confirmed = true;

        if self.remote_confirmed {
            self.state = LinkTradeState::Trading {
                local_index: local_idx,
                remote_index: remote_idx,
            };
        } else {
            self.state = LinkTradeState::WaitingForPeerConfirm {
                local_index: local_idx,
                remote_index: remote_idx,
            };
        }
        Ok(())
    }

    pub fn cancel_trade(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), TransportError> {
        transport.send(NetworkMessage::CancelTrade)?;
        self.reset_selection();
        self.state = LinkTradeState::SelectingMon;
        Ok(())
    }

    pub fn poll(&mut self, transport: &mut dyn NetworkTransport<NetworkMessage>) -> LinkTradePollResult {
        let msg = match transport.try_recv() {
            Ok(Some(msg)) => msg,
            Ok(None) => return LinkTradePollResult::Pending,
            Err(TransportError::Disconnected) => {
                self.state = LinkTradeState::Cancelled;
                return LinkTradePollResult::Disconnected;
            }
            Err(e) => {
                let msg = format!("{}", e);
                self.state = LinkTradeState::Error(msg.clone());
                return LinkTradePollResult::Error(msg);
            }
        };

        self.handle_message(msg, transport)
    }

    pub fn poll_blocking(&mut self, transport: &mut dyn NetworkTransport<NetworkMessage>) -> LinkTradePollResult {
        let msg = match transport.recv() {
            Ok(msg) => msg,
            Err(TransportError::Disconnected) => {
                self.state = LinkTradeState::Cancelled;
                return LinkTradePollResult::Disconnected;
            }
            Err(e) => {
                let msg = format!("{}", e);
                self.state = LinkTradeState::Error(msg.clone());
                return LinkTradePollResult::Error(msg);
            }
        };

        self.handle_message(msg, transport)
    }

    fn handle_message(
        &mut self,
        msg: NetworkMessage,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> LinkTradePollResult {
        match (&self.state, msg) {
            (LinkTradeState::Idle, NetworkMessage::RequestTrade) => {
                self.state = LinkTradeState::PeerRequestedTrade;
                LinkTradePollResult::TradeRequested
            }
            // Both players used the gameboy at the same time. The clocking
            // (hosting) side's request wins (engine/menus/main_menu.asm:
            // "The gameboy that is clocking the connection wins"): the host
            // ignores the guest's duplicate request (the guest's AcceptTrade
            // is already in flight); the guest yields by accepting the host's.
            (LinkTradeState::WaitingForTradeResponse, NetworkMessage::RequestTrade) => {
                match self.role {
                    Some(crate::link::LinkRole::Host) => LinkTradePollResult::Pending,
                    Some(crate::link::LinkRole::Guest) => {
                        let _ = transport.send(NetworkMessage::AcceptTrade);
                        self.state = LinkTradeState::SelectingMon;
                        LinkTradePollResult::TradeAccepted
                    }
                    None => {
                        let err = "simultaneous trade requests without a clock role".to_string();
                        self.state = LinkTradeState::Error(err.clone());
                        LinkTradePollResult::Error(err)
                    }
                }
            }
            (LinkTradeState::WaitingForTradeResponse, NetworkMessage::AcceptTrade) => {
                self.state = LinkTradeState::SelectingMon;
                LinkTradePollResult::TradeAccepted
            }
            (LinkTradeState::WaitingForTradeResponse, NetworkMessage::DeclineTrade) => {
                self.state = LinkTradeState::Idle;
                LinkTradePollResult::TradeDeclined
            }

            (
                LinkTradeState::SelectingMon
                | LinkTradeState::WaitingForPeerSelection
                | LinkTradeState::BothSelected { .. },
                NetworkMessage::SelectMon(idx),
            ) => {
                self.remote_selection = Some(idx);
                self.remote_confirmed = false;
                self.try_transition_to_both_selected();
                if self.local_selection.is_some() {
                    LinkTradePollResult::BothSelected {
                        local_index: self.local_selection.unwrap(),
                        remote_index: idx,
                    }
                } else {
                    LinkTradePollResult::PeerSelectedMon(idx)
                }
            }

            (
                LinkTradeState::BothSelected {
                    local_index,
                    remote_index,
                },
                NetworkMessage::ConfirmTrade,
            ) => {
                self.remote_confirmed = true;
                let (li, ri) = (*local_index, *remote_index);
                if self.local_confirmed {
                    self.state = LinkTradeState::Trading {
                        local_index: li,
                        remote_index: ri,
                    };
                } else {
                    self.state = LinkTradeState::PeerConfirmedWaitingLocal {
                        local_index: li,
                        remote_index: ri,
                    };
                }
                LinkTradePollResult::PeerConfirmed
            }

            (
                LinkTradeState::WaitingForPeerConfirm {
                    local_index,
                    remote_index,
                },
                NetworkMessage::ConfirmTrade,
            ) => {
                self.remote_confirmed = true;
                let (li, ri) = (*local_index, *remote_index);
                self.state = LinkTradeState::Trading {
                    local_index: li,
                    remote_index: ri,
                };
                LinkTradePollResult::PeerConfirmed
            }

            (
                LinkTradeState::Trading {
                    local_index,
                    remote_index,
                },
                NetworkMessage::TradeComplete(pokemon),
            ) => {
                let (li, ri) = (*local_index, *remote_index);
                self.state = LinkTradeState::Completed;
                LinkTradePollResult::TradeExecute {
                    local_index: li,
                    remote_index: ri,
                    received_pokemon: pokemon,
                }
            }

            (_, NetworkMessage::CancelTrade) => {
                self.reset_selection();
                self.state = LinkTradeState::SelectingMon;
                LinkTradePollResult::PeerCancelled
            }

            (_, NetworkMessage::Disconnect) => {
                self.state = LinkTradeState::Cancelled;
                LinkTradePollResult::Disconnected
            }

            (state, msg) => {
                let err = format!("unexpected message {:?} in state {:?}", msg, state);
                self.state = LinkTradeState::Error(err.clone());
                LinkTradePollResult::Error(err)
            }
        }
    }

    fn try_transition_to_both_selected(&mut self) {
        if let (Some(local), Some(remote)) = (self.local_selection, self.remote_selection) {
            self.state = LinkTradeState::BothSelected {
                local_index: local,
                remote_index: remote,
            };
        } else if self.local_selection.is_some() {
            self.state = LinkTradeState::WaitingForPeerSelection;
        }
    }

    fn reset_selection(&mut self) {
        self.local_selection = None;
        self.remote_selection = None;
        self.local_confirmed = false;
        self.remote_confirmed = false;
    }

    pub fn reset_for_new_trade(&mut self) {
        self.reset_selection();
        if matches!(
            self.state,
            LinkTradeState::Completed | LinkTradeState::Cancelled
        ) {
            self.state = LinkTradeState::Idle;
        }
    }
}

impl Default for LinkTradeManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Game-level driver
// ---------------------------------------------------------------------------

/// Driver-level errors, kept distinct from wire errors so frontends can tell
/// "bad selection" apart from "the cable broke".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTradeError {
    /// The transport itself failed (disconnect / timeout / serialization).
    Transport(TransportError),
    /// [`LinkTradeDriver::select_mon`] index outside the party.
    InvalidIndex(u8),
    /// The operation is not valid in the current trade state (message from
    /// the underlying state machine).
    WrongState(String),
    /// [`LinkTradeDriver::apply_exchange`] called without a completed
    /// `TradeExecute`.
    NoExchange,
    /// The received mon could not join the party. Cannot happen in practice:
    /// the given mon is removed first, so there is always room.
    PartyFull,
    /// The party could not remove the selected mon. Cannot happen while the
    /// driver owns the party.
    PartyRemove,
}

impl std::fmt::Display for LinkTradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkTradeError::Transport(e) => write!(f, "link transport error: {}", e),
            LinkTradeError::InvalidIndex(i) => write!(f, "invalid party index {}", i),
            LinkTradeError::WrongState(msg) => write!(f, "wrong trade state: {}", msg),
            LinkTradeError::NoExchange => write!(f, "no completed exchange to apply"),
            LinkTradeError::PartyFull => write!(f, "party is full"),
            LinkTradeError::PartyRemove => write!(f, "could not remove the traded mon"),
        }
    }
}

impl std::error::Error for LinkTradeError {}

impl From<TransportError> for LinkTradeError {
    fn from(e: TransportError) -> Self {
        // The manager reports state-guard violations as IoError; surface them
        // as WrongState so the transport case stays unambiguous.
        match e {
            TransportError::IoError(msg) => LinkTradeError::WrongState(msg),
            other => LinkTradeError::Transport(other),
        }
    }
}

/// Game-level link trade driver: owns the local party + player ID on top of
/// the [`LinkTradeManager`] wire protocol and implements the Gen 1 trade
/// rules (see the module docs for the asm evidence).
///
/// Flow for the frontend:
/// 1. Construct with a copy of the save's party and the player's trainer ID.
/// 2. `request_trade` / `accept_trade` / `select_mon` / `confirm_trade` /
///    `cancel_trade`, polling after each send.
/// 3. On [`LinkTradePollResult::TradeExecute`] (both sides confirmed and the
///    peer's mon data arrived), play the trade cutscene, then call
///    [`Self::apply_exchange`] — it removes the given mon, appends the
///    received one (traded flag + Pokédex updated), and returns the pending
///    trade evolution (if any). The original mutates the party BEFORE the
///    cutscene (cable_club.asm:800-817); the driver leaves the timing to the
///    frontend — `apply_exchange` may run before or after the animation.
/// 4. If a [`PendingEvolution`] came back, play the evolution cutscene
///    (`crate::evolution_screen`) and apply a confirmed outcome with
///    `crate::pokemon::evolution::finalize_evolution` on
///    [`Self::received_mon_mut`] (it is forced — `wForceEvolution` — so the
///    cutscene cannot be B-cancelled).
/// 5. Write the party back into the save with [`Self::into_party`].
///
/// Cancel mid-selection returns both sides to selection with the party
/// untouched; a disconnect anywhere aborts the trade the same way.
#[derive(Debug)]
pub struct LinkTradeDriver {
    manager: LinkTradeManager,
    party: Party,
    player_id: u16,
    /// Locally selected party index (cleared when the selection is void).
    local_index: Option<u8>,
    /// Index the peer selected, for reporting.
    remote_index: Option<u8>,
    /// The local mon sent at confirm — removed from `party` by
    /// `apply_exchange`.
    given_mon: Option<Pokemon>,
    /// The peer's mon, waiting for `apply_exchange`.
    received_mon: Option<Pokemon>,
    /// Party index of the received mon once `apply_exchange` ran.
    received_index: Option<usize>,
}

impl LinkTradeDriver {
    /// `party` is a copy of the save's party; the frontend writes the result
    /// back with [`Self::into_party`]. `player_id` is the local trainer ID
    /// (`wPlayerID`) — it decides the received mon's traded/obedience flag.
    pub fn new(party: Party, player_id: u16) -> Self {
        Self {
            manager: LinkTradeManager::new(),
            party,
            player_id,
            local_index: None,
            remote_index: None,
            given_mon: None,
            received_mon: None,
            received_index: None,
        }
    }

    /// Set the Cable Club clock role (`hSerialConnectionStatus`): breaks the
    /// both-pressed-the-gameboy tie the same way as
    /// [`LinkTradeManager::set_role`] (the clocking side's request wins,
    /// engine/menus/main_menu.asm:216-220). The host (`--link-listen`) is the
    /// internal clock ("player" side).
    pub fn with_role(mut self, role: crate::link::LinkRole) -> Self {
        self.manager.set_role(role);
        self
    }

    /// Replace the working party with a fresh copy of the save's party.
    ///
    /// The driver is typically constructed when the connection comes up, but
    /// the trade operates on the party the player carries to the table. The
    /// app calls this when the gameboy is used (or when the peer's request
    /// arrives), before `select_mon` / `confirm_trade` read the party.
    pub fn set_party(&mut self, party: Party) {
        self.party = party;
    }

    pub fn state(&self) -> &LinkTradeState {
        self.manager.state()
    }

    pub fn is_completed(&self) -> bool {
        self.manager.is_completed()
    }

    /// The working party (the exchange applied once `apply_exchange` ran).
    pub fn party(&self) -> &Party {
        &self.party
    }

    /// Consume the driver and return the party for save writeback.
    pub fn into_party(self) -> Party {
        self.party
    }

    /// The local mon sent at confirm (still in the party until
    /// `apply_exchange` removes it).
    pub fn given_mon(&self) -> Option<&Pokemon> {
        self.given_mon.as_ref()
    }

    /// The mon received from the peer. Before `apply_exchange` it is the
    /// wire copy; afterwards it is the party member at the received slot
    /// (where a pending evolution would land).
    pub fn received_mon(&self) -> Option<&Pokemon> {
        self.received_mon
            .as_ref()
            .or_else(|| self.received_index.and_then(|i| self.party.get(i)))
    }

    pub fn received_mon_mut(&mut self) -> Option<&mut Pokemon> {
        if let Some(i) = self.received_index {
            return self.party.get_mut(i);
        }
        self.received_mon.as_mut()
    }

    /// The party index selected locally, if any.
    pub fn local_index(&self) -> Option<u8> {
        self.local_index
    }

    /// The party index the peer selected, if known.
    pub fn remote_index(&self) -> Option<u8> {
        self.remote_index
    }

    pub fn request_trade(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), LinkTradeError> {
        self.manager.request_trade(transport).map_err(Into::into)
    }

    pub fn accept_trade(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), LinkTradeError> {
        self.manager.accept_trade(transport).map_err(Into::into)
    }

    pub fn decline_trade(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), LinkTradeError> {
        self.manager.decline_trade(transport).map_err(Into::into)
    }

    /// Select the party mon to trade. Any party member is legal in Gen 1 —
    /// fainted, HM-holding, or the last one (`TradeCenter_SelectMon` lists
    /// every slot, cable_club.asm:393-400); only the index is bounds-checked.
    pub fn select_mon(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
        party_index: u8,
    ) -> Result<(), LinkTradeError> {
        if party_index as usize >= self.party.count() {
            return Err(LinkTradeError::InvalidIndex(party_index));
        }
        self.manager.select_mon(transport, party_index)?;
        self.local_index = Some(party_index);
        Ok(())
    }

    /// Confirm the trade: sends the selected party mon (its full data rides
    /// the wire, so level / EXP / DVs / PP-Ups / nickname / OT all survive).
    pub fn confirm_trade(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), LinkTradeError> {
        let idx = self
            .local_index
            .ok_or_else(|| LinkTradeError::WrongState("no mon selected".into()))?;
        let mon = self
            .party
            .get(idx as usize)
            .ok_or(LinkTradeError::InvalidIndex(idx))?
            .clone();
        self.manager.confirm_trade(transport, mon.clone())?;
        self.given_mon = Some(mon);
        Ok(())
    }

    pub fn cancel_trade(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), LinkTradeError> {
        self.manager.cancel_trade(transport)?;
        self.clear_pending_trade();
        Ok(())
    }

    pub fn poll(&mut self, transport: &mut dyn NetworkTransport<NetworkMessage>) -> LinkTradePollResult {
        let result = self.manager.poll(transport);
        self.observe(&result);
        result
    }

    pub fn poll_blocking(&mut self, transport: &mut dyn NetworkTransport<NetworkMessage>) -> LinkTradePollResult {
        let result = self.manager.poll_blocking(transport);
        self.observe(&result);
        result
    }

    /// Apply the completed exchange: remove the given mon, append the
    /// received one, stamp its traded flag against the local player ID and
    /// the Pokédex, and detect a forced trade evolution on the received mon
    /// (returns it as a pending evolution for the frontend's cutscene).
    ///
    /// Ordering mirrors `.doTrade` (cable_club.asm:800-817): removal first,
    /// so a full party still has room; the received mon lands in the last
    /// party slot — the same slot `TryEvolvingMon` checks after the trade
    /// anim (cable_club.asm:851, `wWhichPokemon = wPartyCount - 1`).
    ///
    /// Idempotent-by-construction: a second call before a new `TradeExecute`
    /// fails with [`LinkTradeError::NoExchange`].
    pub fn apply_exchange(
        &mut self,
        pokedex: &mut Pokedex,
    ) -> Result<Option<PendingEvolution>, LinkTradeError> {
        let received = self.received_mon.take().ok_or(LinkTradeError::NoExchange)?;
        let local_index = self
            .local_index
            .ok_or(LinkTradeError::NoExchange)? as usize;
        let given = self
            .party
            .remove_for_trade(local_index)
            .map_err(|_| LinkTradeError::PartyRemove)?;
        self.given_mon = Some(given);

        let mut received = received;
        // OT name/ID stay the remote trainer's (add_mon.asm:303-313); the
        // traded flag is recomputed against OUR ID — that is what drives
        // obedience and the 1.5x EXP bonus.
        received.is_traded = is_traded_for(received.ot_id, self.player_id);
        let species = received.species;
        let index = self
            .party
            .add(received)
            .map_err(|_| LinkTradeError::PartyFull)?;
        self.received_index = Some(index);

        // AddEnemyMonToPlayerParty: owned + seen (add_mon.asm:325-337).
        pokedex.set_owned(species);
        pokedex.set_seen(species);

        // TryEvolvingMon after the trade anim (cable_club.asm:851) with
        // wForceEvolution set (cable_club.asm:822): only EVOLVE_TRADE entries
        // can trigger while trading (evos_moves.asm:70-94) and the cutscene
        // cannot be B-cancelled (evolution.asm:156-158).
        let pending = match self.party.get(index) {
            Some(mon) => {
                let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
                check_trade_evolution(mon.species, mon.level).map(|to| PendingEvolution {
                    party_index: index,
                    from: mon.species,
                    to,
                    name: mon.display_name(&mut name_buf).to_string(),
                    force: true,
                })
            }
            None => None,
        };
        self.local_index = None;
        self.remote_index = None;
        Ok(pending)
    }

    /// Reset to a fresh trade (after `Completed` / `Cancelled`), dropping any
    /// un-applied exchange state. Use when trading again in the same session.
    pub fn reset_for_new_trade(&mut self) {
        self.manager.reset_for_new_trade();
        self.clear_pending_trade();
    }

    fn observe(&mut self, result: &LinkTradePollResult) {
        match result {
            LinkTradePollResult::BothSelected {
                local_index,
                remote_index,
            } => {
                self.local_index = Some(*local_index);
                self.remote_index = Some(*remote_index);
            }
            LinkTradePollResult::PeerSelectedMon(idx) => {
                self.remote_index = Some(*idx);
            }
            LinkTradePollResult::TradeExecute {
                local_index,
                remote_index,
                received_pokemon,
            } => {
                self.local_index = Some(*local_index);
                self.remote_index = Some(*remote_index);
                self.received_mon = Some(received_pokemon.clone());
            }
            // The selection is void again: both sides are back to picking.
            LinkTradePollResult::PeerCancelled | LinkTradePollResult::Disconnected => {
                self.clear_pending_trade();
            }
            _ => {}
        }
    }

    fn clear_pending_trade(&mut self) {
        self.local_index = None;
        self.remote_index = None;
        self.given_mon = None;
        self.received_mon = None;
        self.received_index = None;
    }
}
