//! JSON-line framing codec shared by the link transports.
//!
//! The link wire protocol is one serde-JSON document per line (the same
//! convention as the debug server). [`super::transport::TcpTransport`]
//! writes the line to a socket; the wasm
//! [`super::broadcast_channel::BroadcastChannelTransport`] posts it as a
//! string. Both share [`encode_line`]/[`decode_line`] so framing stays
//! byte-identical across transports.
//!
//! BroadcastChannel delivers every post to EVERY tab on the channel —
//! including the sender's own — so broadcast frames are wrapped in a
//! [`Frame`] envelope carrying a random per-session tag, and receivers drop
//! frames whose tag is their own ([`Frame::is_self`]). The envelope is pure
//! serde, so it is defined (and tested) here on the host rather than inside
//! the wasm-only transport module.

use pokered_core::link::protocol::NetworkMessage;
use pokered_core::link::transport::TransportError;
use serde::{Deserialize, Serialize};

/// A BroadcastChannel frame: the sender's per-session tag plus the protocol
/// message.
///
/// Referenced only by the wasm transport at runtime; it stays compiled (and
/// tested) on the host so the envelope contract is verified natively.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// Random per-session tag; a frame carrying OUR tag is our own echo.
    pub from: String,
    /// The link protocol message.
    pub msg: NetworkMessage,
}

impl Frame {
    /// True when the frame was posted by us — BroadcastChannel echoes every
    /// message back to its sender, and each side must drop its own echo.
    #[allow(dead_code)]
    pub fn is_self(&self, my_tag: &str) -> bool {
        self.from == my_tag
    }
}

/// Serialize a value as one JSON line (no trailing newline — the transports
/// add the `\n` framing themselves).
pub(crate) fn encode_line<T: Serialize>(value: &T) -> Result<String, TransportError> {
    serde_json::to_string(value).map_err(|e| TransportError::SerializationError(e.to_string()))
}

/// Deserialize one JSON line.
pub(crate) fn decode_line<T: serde::de::DeserializeOwned>(
    line: &str,
) -> Result<T, TransportError> {
    serde_json::from_str(line).map_err(|e| TransportError::SerializationError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_message_roundtrips_through_codec() {
        let json = encode_line(&NetworkMessage::hello()).unwrap();
        // One JSON document, no trailing newline (the transports add it).
        assert!(!json.contains('\n'));
        assert_eq!(decode_line::<NetworkMessage>(&json).unwrap(), NetworkMessage::hello());
    }

    #[test]
    fn frame_roundtrips_through_codec_and_self_filter() {
        let frame = Frame {
            from: "abc123".to_string(),
            msg: NetworkMessage::hello_ack(),
        };
        let json = encode_line(&frame).unwrap();
        // The envelope wraps the bare message under `msg`; `from` carries
        // the sender's tag.
        assert!(json.contains("\"from\":\"abc123\""));
        assert!(json.contains("\"msg\":"));

        let decoded = decode_line::<Frame>(&json).unwrap();
        assert_eq!(decoded, frame);
        // The self-echo filter: my tag drops my own frames, keeps the peer's.
        assert!(frame.is_self("abc123"));
        assert!(!frame.is_self("peer-tag"));
    }

    #[test]
    fn peer_frame_with_different_tag_is_kept() {
        let frame = Frame {
            from: "peer-tag".to_string(),
            msg: NetworkMessage::RequestBattle,
        };
        assert!(!frame.is_self("my-tag"));
        let json = encode_line(&frame).unwrap();
        assert_eq!(
            decode_line::<Frame>(&json).unwrap().msg,
            NetworkMessage::RequestBattle
        );
    }

    #[test]
    fn malformed_line_is_a_serialization_error() {
        match decode_line::<Frame>("not json at all") {
            Err(TransportError::SerializationError(_)) => {}
            other => panic!("expected SerializationError, got {:?}", other),
        }
        // A bare message (no envelope) must not decode as a Frame.
        let bare = encode_line(&NetworkMessage::hello()).unwrap();
        assert!(matches!(
            decode_line::<Frame>(&bare),
            Err(TransportError::SerializationError(_))
        ));
    }
}
