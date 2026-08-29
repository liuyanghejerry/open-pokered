//! The debug-server transport machinery (TCP listener, JSON-line framing,
//! mpsc forwarding, 5s response timeout) lives in the engine's platform
//! layer as `dotzuki_app::debug_server`, generic over the game's command
//! type. This module binds it to pokered's [`DebugCommand`].

use crate::protocol::DebugCommand;

/// The debug server, bound to pokered's debug command protocol. See
/// [`dotzuki_app::debug_server::DebugServer`].
pub type DebugServer = dotzuki_app::debug_server::DebugServer<DebugCommand>;

/// Handle to the debug server from the game loop side. See
/// [`dotzuki_app::debug_server::DebugServerHandle`].
pub type DebugServerHandle = dotzuki_app::debug_server::DebugServerHandle<DebugCommand>;
