//! BroadcastChannel link transport (wasm-only): two browser tabs on the same
//! origin can link over the Web `BroadcastChannel` API — no server, no
//! sockets.
//!
//! Framing is the same JSON-line convention as the native TCP transport
//! ([`super::codec`]): each message is one serde-JSON document. Because
//! BroadcastChannel delivers every post to EVERY tab on the channel —
//! including the sender's own — each frame is wrapped in a
//! [`super::codec::Frame`] envelope carrying a random per-session tag, and
//! receivers drop frames whose tag is their own ([`Frame::is_self`]).
//!
//! The channel name acts as the "link room": exactly two participants
//! (tabs) should use the same name. A third tab on the channel would receive
//! (and be received by) both sides' messages — the protocol has no
//! addressing — so use a fresh random name per session.
//!
//! `recv`/`try_recv` drain an `mpsc` channel fed by the `onmessage`
//! listener, mirroring `TcpTransport`'s reader thread. `Drop` closes the
//! channel and drops the listener (unregistering the handler and dropping
//! the channel sender it holds), so pending frames drain before `try_recv`
//! deterministically reports [`TransportError::Disconnected`].

use std::sync::mpsc::{self, Receiver};

use pokered_core::link::protocol::NetworkMessage;
use pokered_core::link::transport::{NetworkTransport, TransportError};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::codec::{Frame, decode_line, encode_line};

/// A link transport over `BroadcastChannel` (see module docs).
pub struct BroadcastChannelTransport {
    channel: web_sys::BroadcastChannel,
    tag: String,
    rx: Receiver<NetworkMessage>,
    /// The `onmessage` handler. Keeping it alive keeps the listener
    /// registered; dropping it unregisters the handler and drops the
    /// channel sender it holds.
    listener: Option<Closure<dyn FnMut(web_sys::MessageEvent)>>,
}

impl BroadcastChannelTransport {
    /// Join `channel_name` (the link room). Creating the channel object
    /// starts delivery immediately; the Hello/HelloAck handshake is started
    /// by the game on its battle driver (via
    /// [`PokemonGame::attach_link_transport`](crate::game::PokemonGame::attach_link_transport)
    /// with `LinkRole::Guest`).
    pub fn new(channel_name: &str) -> Result<Self, TransportError> {
        let channel = web_sys::BroadcastChannel::new(channel_name).map_err(|e| {
            TransportError::IoError(format!(
                "BroadcastChannel '{}' failed: {:?}",
                channel_name, e
            ))
        })?;
        let tag = random_tag();
        let (tx, rx) = mpsc::channel::<NetworkMessage>();
        let listener_tag = tag.clone();
        let listener_tx = tx.clone();
        let listener = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
            let Some(line) = event.data().as_string() else {
                return; // non-string frame (never posted by us): ignore
            };
            match decode_line::<Frame>(&line) {
                Ok(frame) if frame.is_self(&listener_tag) => {
                    // Our own echo — BroadcastChannel delivers to the sender
                    // too; the link protocol sees only the peer's messages.
                }
                Ok(frame) => {
                    if listener_tx.send(frame.msg).is_err() {
                        // Transport dropped; nothing left to feed.
                    }
                }
                Err(e) => {
                    // Malformed frame (foreign tab on the same channel, or a
                    // peer speaking a different protocol version): drop it
                    // and keep the listener alive.
                    log::warn!("[link] dropping malformed broadcast frame: {}", e);
                }
            }
        }) as Box<dyn FnMut(_)>);
        channel.set_onmessage(Some(listener.as_ref().unchecked_ref()));
        Ok(BroadcastChannelTransport {
            channel,
            tag,
            rx,
            listener: Some(listener),
        })
    }

    /// Our per-session tag (handy for debugging which tab is which).
    #[allow(dead_code)]
    pub fn tag(&self) -> &str {
        &self.tag
    }
}

impl NetworkTransport<NetworkMessage> for BroadcastChannelTransport {
    fn send(&mut self, msg: NetworkMessage) -> Result<(), TransportError> {
        let frame = Frame {
            from: self.tag.clone(),
            msg,
        };
        let line = encode_line(&frame)?;
        self.channel.post_message(&JsValue::from_str(&line)).map_err(|e| {
            TransportError::IoError(format!("BroadcastChannel post failed: {:?}", e))
        })
    }

    fn recv(&mut self) -> Result<NetworkMessage, TransportError> {
        self.rx.recv().map_err(|_| TransportError::Disconnected)
    }

    fn try_recv(&mut self) -> Result<Option<NetworkMessage>, TransportError> {
        match self.rx.try_recv() {
            Ok(msg) => Ok(Some(msg)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(TransportError::Disconnected),
        }
    }
}

impl Drop for BroadcastChannelTransport {
    fn drop(&mut self) {
        // Stop delivery first, then drop the listener (and with it the
        // channel sender it holds): frames already in the mpsc queue still
        // drain before `try_recv` reports `Disconnected` — the same drop
        // semantics as `TcpTransport`.
        self.channel.close();
        self.listener.take();
    }
}

/// A random per-session tag: ~53 bits from `Math.random()`. Collision odds
/// between the two tabs of a session are negligible; on a collision each
/// side would filter the other's frames as its own and the handshake would
/// stall (visible in the UI, no data corruption).
fn random_tag() -> String {
    format!("{:x}", (js_sys::Math::random() * 9_007_199_254_740_992.0) as u64)
}
