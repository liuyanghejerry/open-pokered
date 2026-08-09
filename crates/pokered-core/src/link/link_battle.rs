use super::protocol::{LinkAction, LinkBattleResult, NetworkMessage, PartyExchangeData};
use super::transport::{NetworkTransport, TransportError};

#[derive(Debug)]
pub struct LinkBattleManager {
    state: LinkBattleState,
    local_party_data: Option<PartyExchangeData>,
    remote_party_data: Option<PartyExchangeData>,
    pending_local_action: Option<LinkAction>,
    pending_remote_action: Option<LinkAction>,
    /// The peer's announced end-of-battle outcome (protocol v2+).
    remote_result: Option<LinkBattleResult>,
    /// Cable Club clock role, set by the session once the connection is up.
    /// Used to break the both-pressed-the-gameboy tie (see the
    /// `WaitingForBattleResponse` × `RequestBattle` arm).
    role: Option<crate::link::LinkRole>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkBattleState {
    /// Initial state — not connected.
    Idle,
    /// Hello sent, waiting for HelloAck from peer.
    WaitingForHelloAck,
    /// Handshake complete, waiting for battle request or acceptance.
    Connected,
    /// We sent RequestBattle, waiting for peer to Accept/Decline.
    WaitingForBattleResponse,
    /// Peer requested battle, we haven't responded yet.
    PeerRequestedBattle,
    /// Both sides agreed to battle. Exchanging party data.
    ExchangingParties,
    /// Party data exchanged. Battle in progress — exchanging turn actions.
    Battling,
    /// Battle ended normally.
    Finished,
    /// An error occurred.
    Error(String),
}

/// Result of a single `poll` step.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkBattlePollResult {
    /// Nothing happened (no message available).
    Pending,
    /// Handshake complete — connected to peer.
    HandshakeComplete,
    /// Peer requested a battle. Call `accept_battle` or `decline_battle`.
    BattleRequested,
    /// Both sides agreed — ready to exchange party data.
    BattleAccepted,
    /// Peer declined the battle request.
    BattleDeclined,
    /// Party data received from peer.
    PartyDataReceived,
    /// Both turn actions received — ready to resolve turn.
    TurnReady {
        local_action: LinkAction,
        remote_action: LinkAction,
    },
    /// The peer announced its battle outcome (protocol v2+).
    BattleResultReceived(LinkBattleResult),
    /// Peer disconnected.
    Disconnected,
    /// Protocol error.
    Error(String),
}

impl LinkBattleManager {
    pub fn new() -> Self {
        Self {
            state: LinkBattleState::Idle,
            local_party_data: None,
            remote_party_data: None,
            pending_local_action: None,
            pending_remote_action: None,
            remote_result: None,
            role: None,
        }
    }

    pub fn state(&self) -> &LinkBattleState {
        &self.state
    }

    pub fn remote_party_data(&self) -> Option<&PartyExchangeData> {
        self.remote_party_data.as_ref()
    }

    /// The peer's announced battle outcome (protocol v2+), set once a
    /// `BattleResult` message was received. `None` for v1 peers, which never
    /// send it.
    pub fn remote_result(&self) -> Option<LinkBattleResult> {
        self.remote_result
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

    pub fn is_battling(&self) -> bool {
        self.state == LinkBattleState::Battling
    }

    pub fn is_finished(&self) -> bool {
        matches!(
            self.state,
            LinkBattleState::Finished | LinkBattleState::Error(_)
        )
    }

    /// Start the handshake by sending Hello.
    pub fn start_handshake(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), TransportError> {
        transport.send(NetworkMessage::hello())?;
        self.state = LinkBattleState::WaitingForHelloAck;
        Ok(())
    }

    /// Request a battle (after handshake is complete).
    pub fn request_battle(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), TransportError> {
        if self.state != LinkBattleState::Connected {
            return Err(TransportError::IoError(
                "cannot request battle: not connected".into(),
            ));
        }
        transport.send(NetworkMessage::RequestBattle)?;
        self.state = LinkBattleState::WaitingForBattleResponse;
        Ok(())
    }

    /// Accept a battle request from the peer.
    pub fn accept_battle(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), TransportError> {
        if self.state != LinkBattleState::PeerRequestedBattle {
            return Err(TransportError::IoError("no pending battle request".into()));
        }
        transport.send(NetworkMessage::AcceptBattle)?;
        self.state = LinkBattleState::ExchangingParties;
        Ok(())
    }

    /// Decline a battle request from the peer.
    pub fn decline_battle(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), TransportError> {
        if self.state != LinkBattleState::PeerRequestedBattle {
            return Err(TransportError::IoError("no pending battle request".into()));
        }
        transport.send(NetworkMessage::DeclineBattle)?;
        self.state = LinkBattleState::Connected;
        Ok(())
    }

    /// Send our party data to the peer.
    pub fn send_party_data(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
        data: PartyExchangeData,
    ) -> Result<(), TransportError> {
        if self.state != LinkBattleState::ExchangingParties {
            return Err(TransportError::IoError(
                "not in party exchange state".into(),
            ));
        }
        transport.send(NetworkMessage::PartyData(data.clone()))?;
        self.local_party_data = Some(data);
        self.try_transition_to_battling();
        Ok(())
    }

    /// Send our turn action.
    pub fn send_turn_action(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
        action: LinkAction,
    ) -> Result<(), TransportError> {
        if self.state != LinkBattleState::Battling {
            return Err(TransportError::IoError("not in battling state".into()));
        }
        transport.send(NetworkMessage::TurnAction(action))?;
        self.pending_local_action = Some(action);
        Ok(())
    }

    /// Announce our end-of-battle outcome to the peer (protocol v2+). The
    /// result is computed locally from the shared resolution, so this is
    /// informational — never required for the battle to end.
    pub fn send_battle_result(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
        result: LinkBattleResult,
    ) -> Result<(), TransportError> {
        if !matches!(
            self.state,
            LinkBattleState::Battling | LinkBattleState::Finished
        ) {
            return Err(TransportError::IoError(
                "cannot send battle result: not battling".into(),
            ));
        }
        transport.send(NetworkMessage::BattleResult(result))
    }

    /// Send disconnect and transition to Finished.
    pub fn disconnect(
        &mut self,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> Result<(), TransportError> {
        let _ = transport.send(NetworkMessage::Disconnect);
        self.state = LinkBattleState::Finished;
        Ok(())
    }

    /// Poll for incoming messages and advance the state machine.
    /// Call this in your game loop to process network events.
    pub fn poll(&mut self, transport: &mut dyn NetworkTransport<NetworkMessage>) -> LinkBattlePollResult {
        let msg = match transport.try_recv() {
            Ok(Some(msg)) => msg,
            Ok(None) => return LinkBattlePollResult::Pending,
            Err(TransportError::Disconnected) => {
                self.state = LinkBattleState::Finished;
                return LinkBattlePollResult::Disconnected;
            }
            Err(e) => {
                let msg = format!("{}", e);
                self.state = LinkBattleState::Error(msg.clone());
                return LinkBattlePollResult::Error(msg);
            }
        };

        self.handle_message(msg, transport)
    }

    /// Poll using blocking recv (for synchronous usage / tests).
    pub fn poll_blocking(&mut self, transport: &mut dyn NetworkTransport<NetworkMessage>) -> LinkBattlePollResult {
        let msg = match transport.recv() {
            Ok(msg) => msg,
            Err(TransportError::Disconnected) => {
                self.state = LinkBattleState::Finished;
                return LinkBattlePollResult::Disconnected;
            }
            Err(e) => {
                let msg = format!("{}", e);
                self.state = LinkBattleState::Error(msg.clone());
                return LinkBattlePollResult::Error(msg);
            }
        };

        self.handle_message(msg, transport)
    }

    fn handle_message(
        &mut self,
        msg: NetworkMessage,
        transport: &mut dyn NetworkTransport<NetworkMessage>,
    ) -> LinkBattlePollResult {
        match (&self.state, msg) {
            (LinkBattleState::WaitingForHelloAck, NetworkMessage::HelloAck { version }) => {
                if !NetworkMessage::is_compatible_version(version) {
                    let err = format!(
                        "version mismatch: local={}, remote={}",
                        NetworkMessage::PROTOCOL_VERSION,
                        version
                    );
                    self.state = LinkBattleState::Error(err.clone());
                    return LinkBattlePollResult::Error(err);
                }
                self.state = LinkBattleState::Connected;
                LinkBattlePollResult::HandshakeComplete
            }
            (LinkBattleState::Idle, NetworkMessage::Hello { version }) => {
                if !NetworkMessage::is_compatible_version(version) {
                    let err = format!(
                        "version mismatch: local={}, remote={}",
                        NetworkMessage::PROTOCOL_VERSION,
                        version
                    );
                    self.state = LinkBattleState::Error(err.clone());
                    return LinkBattlePollResult::Error(err);
                }
                let _ = transport.send(NetworkMessage::hello_ack());
                self.state = LinkBattleState::Connected;
                LinkBattlePollResult::HandshakeComplete
            }

            (LinkBattleState::Connected, NetworkMessage::RequestBattle) => {
                self.state = LinkBattleState::PeerRequestedBattle;
                LinkBattlePollResult::BattleRequested
            }
            // Both players used the gameboy at the same time. The clocking
            // (hosting) side's request wins (engine/menus/main_menu.asm:
            // "The gameboy that is clocking the connection wins"): the host
            // ignores the guest's duplicate request (the guest's AcceptBattle
            // is already in flight); the guest yields by accepting the host's.
            (LinkBattleState::WaitingForBattleResponse, NetworkMessage::RequestBattle) => {
                match self.role {
                    Some(crate::link::LinkRole::Host) => LinkBattlePollResult::Pending,
                    Some(crate::link::LinkRole::Guest) => {
                        let _ = transport.send(NetworkMessage::AcceptBattle);
                        self.state = LinkBattleState::ExchangingParties;
                        LinkBattlePollResult::BattleAccepted
                    }
                    None => {
                        let err = "simultaneous battle requests without a clock role".to_string();
                        self.state = LinkBattleState::Error(err.clone());
                        LinkBattlePollResult::Error(err)
                    }
                }
            }
            (LinkBattleState::WaitingForBattleResponse, NetworkMessage::AcceptBattle) => {
                self.state = LinkBattleState::ExchangingParties;
                LinkBattlePollResult::BattleAccepted
            }
            (LinkBattleState::WaitingForBattleResponse, NetworkMessage::DeclineBattle) => {
                self.state = LinkBattleState::Connected;
                LinkBattlePollResult::BattleDeclined
            }

            (LinkBattleState::ExchangingParties, NetworkMessage::PartyData(data)) => {
                self.remote_party_data = Some(data);
                self.try_transition_to_battling();
                LinkBattlePollResult::PartyDataReceived
            }

            (LinkBattleState::Battling, NetworkMessage::TurnAction(action)) => {
                self.pending_remote_action = Some(action);
                self.try_resolve_turn()
            }

            (LinkBattleState::Battling, NetworkMessage::BattleResult(result)) => {
                self.remote_result = Some(result);
                LinkBattlePollResult::BattleResultReceived(result)
            }
            // A late result (peer resolved slightly earlier) after our battle
            // already ended — record it, keep Finished.
            (LinkBattleState::Finished, NetworkMessage::BattleResult(result)) => {
                self.remote_result = Some(result);
                LinkBattlePollResult::BattleResultReceived(result)
            }

            (_, NetworkMessage::Disconnect) => {
                self.state = LinkBattleState::Finished;
                LinkBattlePollResult::Disconnected
            }

            (state, msg) => {
                let err = format!("unexpected message {:?} in state {:?}", msg, state);
                self.state = LinkBattleState::Error(err.clone());
                LinkBattlePollResult::Error(err)
            }
        }
    }

    /// If both sides have exchanged party data, transition to Battling.
    fn try_transition_to_battling(&mut self) {
        if self.local_party_data.is_some() && self.remote_party_data.is_some() {
            self.state = LinkBattleState::Battling;
        }
    }

    /// If both turn actions are present, return TurnReady and clear them.
    fn try_resolve_turn(&mut self) -> LinkBattlePollResult {
        if let (Some(local), Some(remote)) = (
            self.pending_local_action.take(),
            self.pending_remote_action.take(),
        ) {
            LinkBattlePollResult::TurnReady {
                local_action: local,
                remote_action: remote,
            }
        } else {
            LinkBattlePollResult::Pending
        }
    }

    /// Reset for a new battle (keeps connection).
    pub fn reset_for_new_battle(&mut self) {
        self.local_party_data = None;
        self.remote_party_data = None;
        self.pending_local_action = None;
        self.pending_remote_action = None;
        self.remote_result = None;
        if matches!(
            self.state,
            LinkBattleState::Battling | LinkBattleState::Finished
        ) {
            self.state = LinkBattleState::Connected;
        }
    }
}

impl Default for LinkBattleManager {
    fn default() -> Self {
        Self::new()
    }
}
