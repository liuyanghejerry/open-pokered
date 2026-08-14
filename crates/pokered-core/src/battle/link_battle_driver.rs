//! Two-sided link-battle gameplay driver.
//!
//! Drives a real [`BattleScreen`] battle on BOTH sides of a link connection
//! in lockstep, verified against the original disassembly (pret/pokered,
//! engine/battle/core.asm + engine/link/):
//!
//! - **RNG**: both sides resolve every random draw (damage variance, crits,
//!   status procs, multi-hit counts, speed-tie flips, Metronome, traded-mon
//!   disobedience) from ONE shared stream — the host's 10-byte list exchanged
//!   in `PartyExchangeData.random_numbers`, consumed with the original
//!   `BattleRandom` semantics (see [`crate::link::rng::LinkRng`]). The host is
//!   the side that sent `RequestBattle` (the original: the side clocking the
//!   connection, cable_club.asm:157-171).
//! - **Turn loop**: the local player chooses an action via the normal battle
//!   menu; the screen defers it (`BattlePhase::LinkWaiting`), the driver sends
//!   it as [`LinkAction`], and when the remote action arrives
//!   (`TurnReady`) resolves the turn with BOTH actions via
//!   [`BattleScreen::resolve_link_turn`]. The enemy side is the remote
//!   player's action instead of the trainer AI (`TrainerAI` returns early for
//!   `LINK_STATE_BATTLING`, engine/battle/trainer_ai.asm:296-298).
//! - **Run**: allowed in link battles and always succeeds
//!   (`TryRunningFromBattle`, core.asm:1503-1510). It is coordinated: both
//!   ran → DRAW; only one side ran → the runner loses (wBattleResult=1, the
//!   other side sees "Enemy ran!" and wins — `EnemyRan`, core.asm:246-267).
//! - **Switch**: switching is never free in link battles — the opponent's
//!   action still lands on the incoming mon; a switch resolves before any
//!   move (`MainInBattleLoop`, core.asm:342-369).
//! - **Items**: the bag is blocked ("Items can't be used here.",
//!   core.asm:2171-2179).
//! - **End**: each side resolves the outcome locally (win when the remote
//!   party is exhausted, loss when ours is, draw/run results as above), then
//!   announces it with `NetworkMessage::BattleResult` (protocol v2).
//!
//! ## Usage (app layer)
//!
//! ```text
//! let mut driver = LinkBattleDriver::new(transport, party, "PLAYER".into());
//! driver.start_handshake()?;            // then poll() until Connected
//! driver.set_local_party(new_party);    // refresh at the cable-club table
//! driver.request_battle()?;             // (or accept_battle() when poll()
//!                                       //  yields BattleRequested)
//! loop {
//!     for event in driver.poll() { … }
//!     if driver.phase() == &LinkDriverPhase::Battling {
//!         let action = driver.update(battle_input);   // forward input
//!         … render driver.screen() …
//!     }
//! }
//! ```
//!
//! The driver owns the transport (injected, so core stays I/O-free), the
//! local party (healed on construction — the original's `HealParty` at the
//! cable-club table, cable_club.asm:292) and the battle screen. Poll it every
//! frame, forward the app's battle input via [`LinkBattleDriver::update`], and
//! react to the returned events. Once [`LinkBattleDriver::result`] is
//! `Some`, the battle is over — show the result screen ("YOU WIN" / "YOU
//! LOSE" / "DRAW" per the original `EndOfBattle`, end_of_battle.asm:28-56).
//! **Do not honor `ScreenAction::Transition(GameScreen::Overworld)` while a
//! link battle is active or finished** — link battles return to the lobby
//! instead (`update` swallows it once the battle ended).
//!
//! The driver is typically constructed when the connection comes up and the
//! party refreshed at the table with [`Self::set_local_party`] (the save
//! party may change between connect and the gameboy use); the exchange then
//! sends the CURRENT party. The handshake is asymmetric: the connecting
//! ("friend") side calls [`Self::start_handshake`], the hosting side never
//! does — its driver auto-acks the peer's `Hello` from `Idle`.
//!
//! ## Disconnect mid-battle
//!
//! The original has no graceful handling — a yanked cable makes
//! `Serial_ExchangeNybble` spin forever (the game soft-locks). This driver
//! instead surfaces [`LinkDriverPhase::Disconnected`] with a message; the app
//! should show an error screen (there is no original error text to reuse; the
//! game's own "link error" experience is a freeze + reset).

use crate::battle::{BattleInput, BattleScreen, ScreenAction};
use crate::link::link_battle::{LinkBattleManager, LinkBattlePollResult, LinkBattleState};
use crate::link::protocol::{LinkBattleResult, NetworkMessage, PartyExchangeData};
use crate::link::rng::{LinkRng, LINK_RANDOM_LIST_SIZE};
use crate::link::transport::{NetworkTransport, TransportError};
use crate::pokemon::party::Party;

/// The driver's top-level state (a superset of the manager's handshake
/// states, plus the battle lifecycle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkDriverPhase {
    /// Not started — `start_handshake` not called yet.
    Idle,
    /// Hello sent; handshake in progress.
    Connecting,
    /// Handshake complete; the app may `request_battle`.
    Connected,
    /// We sent `RequestBattle`; waiting for the peer's accept/decline.
    RequestingBattle,
    /// The peer requested a battle; the app must `accept_battle` /
    /// `decline_battle`.
    PeerRequestedBattle,
    /// Both sides agreed; party data is being exchanged.
    ExchangingParties,
    /// The battle is live — poll + forward input.
    Battling,
    /// The battle ended (see [`LinkBattleDriver::result`]).
    Finished,
    /// The link failed (peer disconnect / protocol error). The app should
    /// show an error screen.
    Disconnected(String),
}

/// Events surfaced by [`LinkBattleDriver::poll`] for the app layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkDriverEvent {
    /// Handshake completed; both sides are `Connected`.
    Connected,
    /// The peer requested a battle — call `accept_battle` / `decline_battle`.
    BattleRequested,
    /// The peer accepted our battle request.
    BattleAccepted,
    /// The peer declined our battle request.
    BattleDeclined,
    /// Both parties exchanged; the battle screen is live.
    BattleStarted,
    /// Our battle ended with this outcome (also in
    /// [`LinkBattleDriver::result`]).
    BattleResult(LinkBattleResult),
    /// The peer announced its outcome (also in
    /// [`LinkBattleDriver::remote_result`]).
    RemoteResult(LinkBattleResult),
    /// The link was lost; the battle can no longer be resolved.
    Disconnected(String),
}

pub struct LinkBattleDriver {
    transport: Box<dyn NetworkTransport<NetworkMessage>>,
    manager: LinkBattleManager,
    /// The local party (healed at construction, mirroring the original's
    /// cable-club `HealParty`). Mutated by the battle (HP/exp/level-ups) and
    /// synced back when the battle ends.
    local_party: Party,
    local_trainer_name: String,
    /// `true` when WE sent `RequestBattle` — the default host-ship
    /// convention (the requester's random-number list becomes the shared
    /// battle stream). Overridden by [`Self::with_role`], which uses the
    /// Cable Club clock role instead — the original's actual host rule
    /// ("the list generated by the gameboy clocking the connection is used
    /// by both gameboys", cable_club.asm:157-171).
    is_host: bool,
    /// The Cable Club clock role (`hSerialConnectionStatus`), when the app
    /// knows it (the warp side). `None` keeps the requester convention.
    role: Option<crate::link::LinkRole>,
    phase: LinkDriverPhase,
    /// The battle screen for the CURRENT battle (`None` before party
    /// exchange). Created by `poll` when both parties are in.
    screen: Option<BattleScreen>,
    /// Our random-number list, sent in `PartyExchangeData` (the host's is
    /// the shared stream). Settable for deterministic tests.
    host_random_list: [u8; LINK_RANDOM_LIST_SIZE],
    /// The local action was sent; waiting for the remote's (prevents resends
    /// while the screen holds `link_pending_local_action`).
    turn_action_sent: bool,
    result: Option<LinkBattleResult>,
    remote_result: Option<LinkBattleResult>,
}

impl LinkBattleDriver {
    /// Create a driver for a link-battle session.
    ///
    /// `local_party` is healed immediately (`HealParty` at the cable-club
    /// table, cable_club.asm:292 — pass an already-healed party to skip).
    pub fn new(
        transport: Box<dyn NetworkTransport<NetworkMessage>>,
        mut local_party: Party,
        local_trainer_name: String,
    ) -> Self {
        local_party.heal_all();
        Self {
            transport,
            manager: LinkBattleManager::new(),
            local_party,
            local_trainer_name,
            is_host: false,
            role: None,
            phase: LinkDriverPhase::Idle,
            screen: None,
            host_random_list: [0; LINK_RANDOM_LIST_SIZE],
            turn_action_sent: false,
            result: None,
            remote_result: None,
        }
    }

    /// Pin OUR random-number list (used when we are the host; the peer's list
    /// is used when we are the guest). Deterministic tests set the same list
    /// on both sides. Default: `rand`-generated, like the original's
    /// `.generateRandomNumberListLoop` (cable_club.asm:98-104).
    pub fn with_host_random_list(mut self, list: [u8; LINK_RANDOM_LIST_SIZE]) -> Self {
        self.host_random_list = list;
        self
    }

    /// Replace the local party with a fresh copy of the save's party.
    ///
    /// The driver is typically constructed when the connection comes up, but
    /// the exchanged party must be the party the player carries to the
    /// cable-club TABLE (the original's `HealParty` runs there,
    /// cable_club.asm:292). The app calls this when the gameboy is used (or
    /// when the peer's request arrives), before `request_battle` /
    /// `accept_battle` send the party. Healed like at construction.
    pub fn set_local_party(&mut self, mut party: Party) {
        party.heal_all();
        self.local_party = party;
    }

    /// Set the Cable Club clock role (`hSerialConnectionStatus`): the
    /// internal-clock (hosting) side's random-number list is the shared
    /// battle stream, matching the original. Also forwards the role to the
    /// manager so a simultaneous request is resolved like the original's
    /// "The gameboy that is clocking the connection wins". Without a role,
    /// the battle REQUESTER is treated as the host (the default for simple
    /// request/accept flows).
    pub fn with_role(mut self, role: crate::link::LinkRole) -> Self {
        self.role = Some(role);
        self.manager.set_role(role);
        self
    }

    /// Whether WE are the host for the shared RNG stream: the clocking
    /// (`Host`) side when a role is set, else the battle requester.
    fn is_host_role(&self) -> bool {
        match self.role {
            Some(crate::link::LinkRole::Host) => true,
            Some(crate::link::LinkRole::Guest) => false,
            None => self.is_host,
        }
    }

    pub fn phase(&self) -> &LinkDriverPhase {
        &self.phase
    }

    /// The live battle screen (during [`LinkDriverPhase::Battling`] and
    /// [`LinkDriverPhase::Finished`]).
    pub fn screen(&self) -> Option<&BattleScreen> {
        self.screen.as_ref()
    }

    pub fn screen_mut(&mut self) -> Option<&mut BattleScreen> {
        self.screen.as_mut()
    }

    /// Our local battle outcome, once the battle has ended.
    pub fn result(&self) -> Option<LinkBattleResult> {
        self.result
    }

    /// The peer's announced outcome (protocol v2+; `None` for v1 peers).
    pub fn remote_result(&self) -> Option<LinkBattleResult> {
        self.remote_result
    }

    /// The remote player's trainer name (from the party exchange).
    pub fn remote_trainer_name(&self) -> Option<String> {
        self.manager
            .remote_party_data()
            .map(|d| String::from_utf8_lossy(&d.trainer_name).into_owned())
    }

    /// The local party (healed copy; synced from the battle when it ends).
    pub fn local_party(&self) -> &Party {
        &self.local_party
    }

    /// Start the handshake (send `Hello`).
    pub fn start_handshake(&mut self) -> Result<(), TransportError> {
        if self.phase == LinkDriverPhase::Idle {
            self.manager.start_handshake(&mut *self.transport)?;
            self.phase = LinkDriverPhase::Connecting;
        }
        Ok(())
    }

    /// Request a battle (after `Connected`). Without a clock role (see
    /// [`Self::with_role`]) we become the HOST: our random-number list is
    /// the shared battle stream.
    pub fn request_battle(&mut self) -> Result<(), TransportError> {
        if self.phase != LinkDriverPhase::Connected {
            return Err(TransportError::IoError(
                "cannot request battle: not connected".into(),
            ));
        }
        self.is_host = true;
        self.manager.request_battle(&mut *self.transport)?;
        self.phase = LinkDriverPhase::RequestingBattle;
        Ok(())
    }

    /// Accept the peer's battle request (we become the GUEST — the peer's
    /// random-number list is the shared stream).
    pub fn accept_battle(&mut self) -> Result<(), TransportError> {
        if self.phase != LinkDriverPhase::PeerRequestedBattle {
            return Err(TransportError::IoError("no pending battle request".into()));
        }
        self.is_host = false;
        self.manager.accept_battle(&mut *self.transport)?;
        self.phase = LinkDriverPhase::ExchangingParties;
        self.send_party_data()
    }

    /// Decline the peer's battle request.
    pub fn decline_battle(&mut self) -> Result<(), TransportError> {
        self.manager.decline_battle(&mut *self.transport)?;
        self.phase = LinkDriverPhase::Connected;
        Ok(())
    }

    /// Send `Disconnect` and mark the session finished.
    pub fn disconnect(&mut self) {
        let _ = self.manager.disconnect(&mut *self.transport);
        self.phase = if self.result.is_some() {
            LinkDriverPhase::Finished
        } else {
            LinkDriverPhase::Disconnected("disconnected".into())
        };
    }

    /// Reset for a rematch (keeps the connection; the battle screen and
    /// results are cleared; parties keep their battle state — heal again via
    /// the app, or reconstruct the driver).
    pub fn reset_for_rematch(&mut self) {
        self.manager.reset_for_new_battle();
        self.screen = None;
        self.turn_action_sent = false;
        self.result = None;
        self.remote_result = None;
        self.phase = LinkDriverPhase::Connected;
    }

    /// Forward one frame of battle input to the screen, and send the local
    /// action over the wire once the player commits one (the screen moved to
    /// [`BattlePhase::LinkWaiting`]). Returns the screen's action; the
    /// `Transition` to the overworld is swallowed once the battle ended —
    /// link battles return to the lobby instead.
    pub fn update(&mut self, input: BattleInput) -> ScreenAction {
        if self.phase != LinkDriverPhase::Battling {
            return ScreenAction::Continue;
        }
        let action = match self.screen.as_mut() {
            Some(screen) => screen.update_frame(input),
            None => return ScreenAction::Continue,
        };
        self.maybe_send_pending_action();
        self.check_battle_end();
        if self.result.is_some() {
            // The battle is over: never let the screen's end-of-battle
            // transition move the app to the overworld.
            return ScreenAction::Continue;
        }
        action
    }

    /// Process incoming network messages and advance the state machine. Call
    /// every frame (before or after [`Self::update`] — both orders work).
    pub fn poll(&mut self) -> Vec<LinkDriverEvent> {
        let mut events = Vec::new();
        loop {
            let result = self.manager.poll(&mut *self.transport);
            match result {
                LinkBattlePollResult::Pending => break,
                LinkBattlePollResult::HandshakeComplete => {
                    self.phase = LinkDriverPhase::Connected;
                    events.push(LinkDriverEvent::Connected);
                }
                LinkBattlePollResult::BattleRequested => {
                    self.phase = LinkDriverPhase::PeerRequestedBattle;
                    events.push(LinkDriverEvent::BattleRequested);
                }
                LinkBattlePollResult::BattleAccepted => {
                    self.phase = LinkDriverPhase::ExchangingParties;
                    if self.send_party_data().is_err() {
                        events.push(LinkDriverEvent::Disconnected(
                            "failed to send party data".into(),
                        ));
                        break;
                    }
                    events.push(LinkDriverEvent::BattleAccepted);
                }
                LinkBattlePollResult::BattleDeclined => {
                    self.phase = LinkDriverPhase::Connected;
                    events.push(LinkDriverEvent::BattleDeclined);
                }
                LinkBattlePollResult::PartyDataReceived => {
                    if self.manager.state() == &LinkBattleState::Battling {
                        self.build_battle();
                        self.phase = LinkDriverPhase::Battling;
                        events.push(LinkDriverEvent::BattleStarted);
                    }
                }
                LinkBattlePollResult::TurnReady {
                    local_action: _,
                    remote_action,
                } => {
                    self.turn_action_sent = false;
                    if let Some(screen) = self.screen.as_mut() {
                        screen.resolve_link_turn(remote_action);
                    }
                    self.check_battle_end();
                    if self.result.is_some() {
                        break;
                    }
                }
                LinkBattlePollResult::BattleResultReceived(result) => {
                    self.remote_result = Some(result);
                    events.push(LinkDriverEvent::RemoteResult(result));
                }
                LinkBattlePollResult::Disconnected => {
                    self.phase =
                        LinkDriverPhase::Disconnected("peer disconnected".into());
                    events.push(LinkDriverEvent::Disconnected(
                        "peer disconnected".into(),
                    ));
                    break;
                }
                LinkBattlePollResult::Error(e) => {
                    self.phase = LinkDriverPhase::Disconnected(e.clone());
                    events.push(LinkDriverEvent::Disconnected(e));
                    break;
                }
            }
        }
        events
    }

    /// Send our `PartyExchangeData` (party + trainer name + our random list).
    fn send_party_data(&mut self) -> Result<(), TransportError> {
        let data = PartyExchangeData {
            trainer_name: self.local_trainer_name.as_bytes().to_vec(),
            party: self.local_party.clone(),
            random_numbers: self.host_random_list,
        };
        self.manager.send_party_data(&mut *self.transport, data)
    }

    /// Build the battle screen once both parties have been exchanged: our
    /// party vs the remote party, enemy trainer = the remote player, shared
    /// RNG stream = the HOST's list (ours if we requested, theirs otherwise).
    fn build_battle(&mut self) {
        let remote = match self.manager.remote_party_data() {
            Some(d) => d.clone(),
            None => return,
        };
        let host_list = if self.is_host_role() {
            self.host_random_list
        } else {
            remote.random_numbers
        };
        let mut screen = BattleScreen::from_parties(
            false,
            &self.local_party.to_vec(),
            &remote.party.to_vec(),
            None,
        );
        screen.trainer_name = Some(String::from_utf8_lossy(&remote.trainer_name).into_owned());
        screen.player_name = Some(self.local_trainer_name.clone());
        screen.link_mode = true;
        if let Some(bs) = screen.battle_state.as_mut() {
            bs.link_battle = true;
        }
        screen.link_rng = Some(LinkRng::new(host_list));
        self.screen = Some(screen);
    }

    /// If the local player committed an action and it hasn't been sent yet,
    /// send it (the manager holds it until the remote action arrives).
    fn maybe_send_pending_action(&mut self) {
        if self.turn_action_sent {
            return;
        }
        let pending = match self.screen.as_ref().and_then(|s| s.link_pending_local_action) {
            Some(a) => a,
            None => return,
        };
        if self.manager.send_turn_action(&mut *self.transport, pending).is_err() {
            self.phase = LinkDriverPhase::Disconnected(
                "failed to send turn action".into(),
            );
            return;
        }
        self.turn_action_sent = true;
    }

    /// If the battle ended (the screen set `link_result`), finalize: announce
    /// `BattleResult`, sync the party, move to `Finished`.
    fn check_battle_end(&mut self) {
        let result = match self.screen.as_ref().and_then(|s| s.link_result) {
            Some(r) => r,
            None => return,
        };
        if let Some(screen) = self.screen.as_mut() {
            screen.link_result = None;
        }
        self.result = Some(result);
        self.phase = LinkDriverPhase::Finished;
        let _ = self
            .manager
            .send_battle_result(&mut *self.transport, result);
        // Sync the battle-mutated party back (HP, exp, level-ups) so rematch
        // sessions can read the outcome party.
        if let Some(screen) = self.screen.as_ref() {
            if let Some(bs) = screen.battle_state.as_ref() {
                self.local_party = Party::from(bs.player.party.clone());
            }
        }
    }
}

/// Convenience for tests: a fully wired pair of drivers over a
/// [`crate::link::transport::ChannelTransport`] pair, with the same pinned
/// host list on both sides (the guest's copy is ignored — only the host's is
/// used, mirroring the asm). The first driver is the battle requester.
#[cfg(test)]
pub(crate) fn test_driver_pair(
    host_list: [u8; LINK_RANDOM_LIST_SIZE],
    party_a: Party,
    party_b: Party,
) -> (LinkBattleDriver, LinkBattleDriver) {
    let (t_a, t_b) = crate::link::transport::ChannelTransport::new_pair();
    (
        LinkBattleDriver::new(Box::new(t_a), party_a, "ALICE".into())
            .with_host_random_list(host_list),
        LinkBattleDriver::new(Box::new(t_b), party_b, "BOB".into())
            .with_host_random_list(host_list),
    )
}
