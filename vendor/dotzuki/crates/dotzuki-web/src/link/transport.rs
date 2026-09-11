//! The wasm half of [`super`]: the actual `BroadcastChannel` transport.

use std::sync::mpsc::{self, Receiver};

use dotzuki_engine::link::{NetworkTransport, TransportError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;

use super::envelope::{Frame, decode_line, encode_line};

/// A link transport over `BroadcastChannel` (see [module docs](super)).
pub struct BroadcastChannelTransport<M> {
    channel: web_sys::BroadcastChannel,
    tag: String,
    rx: Receiver<M>,
    /// The `onmessage` handler. Keeping it alive keeps the listener
    /// registered; dropping it unregisters the handler and drops the
    /// channel sender it holds.
    listener: Option<Closure<dyn FnMut(web_sys::MessageEvent)>>,
}

impl<M> BroadcastChannelTransport<M>
where
    M: Serialize + DeserializeOwned + Send + 'static,
{
    /// Join `channel_name` (the link room). Creating the channel object
    /// starts delivery immediately; any handshake is started by the game's
    /// link state machines on top of this transport.
    pub fn new(channel_name: &str) -> Result<Self, TransportError> {
        let channel = web_sys::BroadcastChannel::new(channel_name).map_err(|e| {
            TransportError::IoError(format!(
                "BroadcastChannel '{}' failed: {:?}",
                channel_name, e
            ))
        })?;
        let tag = random_tag();
        let (tx, rx) = mpsc::channel::<M>();
        let listener_tag = tag.clone();
        let listener_tx = tx.clone();
        let listener = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
            let Some(line) = event.data().as_string() else {
                return; // non-string frame (never posted by us): ignore
            };
            match decode_line::<Frame<M>>(&line) {
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
    pub fn tag(&self) -> &str {
        &self.tag
    }
}

impl<M> NetworkTransport<M> for BroadcastChannelTransport<M>
where
    M: Serialize + DeserializeOwned + Send + 'static,
{
    fn send(&mut self, msg: M) -> Result<(), TransportError> {
        let frame = Frame {
            from: self.tag.clone(),
            msg,
        };
        let line = encode_line(&frame)?;
        self.channel.post_message(&JsValue::from_str(&line)).map_err(|e| {
            TransportError::IoError(format!("BroadcastChannel post failed: {:?}", e))
        })
    }

    fn recv(&mut self) -> Result<M, TransportError> {
        self.rx.recv().map_err(|_| TransportError::Disconnected)
    }

    fn try_recv(&mut self) -> Result<Option<M>, TransportError> {
        match self.rx.try_recv() {
            Ok(msg) => Ok(Some(msg)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(TransportError::Disconnected),
        }
    }
}

impl<M> Drop for BroadcastChannelTransport<M> {
    fn drop(&mut self) {
        // Stop delivery first, then drop the listener (and with it the
        // channel sender it holds): frames already in the mpsc queue still
        // drain before `try_recv` reports `Disconnected` — the same drop
        // semantics as a socket transport's reader thread.
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
