//! JSON-line framing codec shared by link transports.
//!
//! The link wire convention is one serde-JSON document per line (the same
//! convention as the debug server). Transports add the `\n` framing
//! themselves: a TCP transport writes the line to a socket, a Web
//! `BroadcastChannel` transport posts it as a string. Both share
//! [`encode_line`]/[`decode_line`] so framing stays byte-identical across
//! transports.
//!
//! This module is pure serde — no I/O, no platform calls — which is why it
//! lives in the engine (the zero-I/O layer) while the transports that use it
//! live in the game or platform layer.
//!
//! Broadcast-style channels deliver every post to EVERY participant on the
//! channel — including the sender's own — so broadcast frames are wrapped in
//! a [`Frame`] envelope carrying a random per-session tag, and receivers
//! drop frames whose tag is their own ([`Frame::is_self`]). The envelope is
//! pure serde, so it is defined (and tested) here rather than inside any
//! one transport.

use serde::{Deserialize, Serialize};

use super::TransportError;

/// A broadcast-channel frame: the sender's per-session tag plus the protocol
/// message.
///
/// Referenced by broadcast-style transports at runtime; it lives here so
/// the envelope contract is verified once for every transport that shares
/// the channel (native tests included).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame<M> {
    /// Random per-session tag; a frame carrying OUR tag is our own echo.
    pub from: String,
    /// The link protocol message.
    pub msg: M,
}

impl<M> Frame<M> {
    /// True when the frame was posted by us — broadcast channels echo every
    /// message back to the sender, and each side must drop its own echo.
    pub fn is_self(&self, my_tag: &str) -> bool {
        self.from == my_tag
    }
}

/// Serialize a value as one JSON line (no trailing newline — the transports
/// add the `\n` framing themselves).
pub fn encode_line<T: Serialize>(value: &T) -> Result<String, TransportError> {
    serde_json::to_string(value).map_err(|e| TransportError::SerializationError(e.to_string()))
}

/// Deserialize one JSON line.
pub fn decode_line<T: serde::de::DeserializeOwned>(line: &str) -> Result<T, TransportError> {
    serde_json::from_str(line).map_err(|e| TransportError::SerializationError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in wire protocol: two "battle" messages and one "trade"
    /// message, mirroring the shape of a real link protocol enum.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum TestMessage {
        Hello { version: u8 },
        HelloAck { version: u8 },
        RequestBattle,
    }

    fn hello() -> TestMessage {
        TestMessage::Hello { version: 2 }
    }

    fn hello_ack() -> TestMessage {
        TestMessage::HelloAck { version: 2 }
    }

    #[test]
    fn bare_message_roundtrips_through_codec() {
        let json = encode_line(&hello()).unwrap();
        // One JSON document, no trailing newline (the transports add it).
        assert!(!json.contains('\n'));
        assert_eq!(decode_line::<TestMessage>(&json).unwrap(), hello());
    }

    #[test]
    fn frame_roundtrips_through_codec_and_self_filter() {
        let frame = Frame {
            from: "abc123".to_string(),
            msg: hello_ack(),
        };
        let json = encode_line(&frame).unwrap();
        // The envelope wraps the bare message under `msg`; `from` carries
        // the sender's tag.
        assert!(json.contains("\"from\":\"abc123\""));
        assert!(json.contains("\"msg\":"));

        let decoded = decode_line::<Frame<TestMessage>>(&json).unwrap();
        assert_eq!(decoded, frame);
        // The self-echo filter: my tag drops my own frames, keeps the peer's.
        assert!(frame.is_self("abc123"));
        assert!(!frame.is_self("peer-tag"));
    }

    #[test]
    fn peer_frame_with_different_tag_is_kept() {
        let frame = Frame {
            from: "peer-tag".to_string(),
            msg: TestMessage::RequestBattle,
        };
        assert!(!frame.is_self("my-tag"));
        let json = encode_line(&frame).unwrap();
        assert_eq!(
            decode_line::<Frame<TestMessage>>(&json).unwrap().msg,
            TestMessage::RequestBattle
        );
    }

    #[test]
    fn malformed_line_is_a_serialization_error() {
        match decode_line::<Frame<TestMessage>>("not json at all") {
            Err(TransportError::SerializationError(_)) => {}
            other => panic!("expected SerializationError, got {:?}", other),
        }
        // A bare message (no envelope) must not decode as a Frame.
        let bare = encode_line(&hello()).unwrap();
        assert!(matches!(
            decode_line::<Frame<TestMessage>>(&bare),
            Err(TransportError::SerializationError(_))
        ));
    }
}
