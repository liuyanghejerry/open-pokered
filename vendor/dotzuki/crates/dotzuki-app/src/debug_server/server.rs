//! The generic debug server: TCP listener ↔ game-loop command forwarding.
//!
//! [`DebugServer`] listens for TCP connections and forwards commands to the
//! game loop; [`DebugServerHandle`] is the game-loop side, polling commands
//! and sending responses without blocking the game. The wire protocol is
//! one serde-JSON document per line in each direction — a command
//! (`{"cmd": ...}`, deserialized as `C`) in, a [`DebugResponse`] out.
//!
//! The command type `C` is the game's debug protocol: use
//! [`CoreDebugCommand`](super::protocol::CoreDebugCommand) directly, or an
//! untagged wrapper extending it with game-specific commands (see the
//! [protocol module docs](super::protocol)).

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use log::{error, info, warn};
use serde::de::DeserializeOwned;

use super::protocol::DebugResponse;

/// Maximum time to wait for a response from the game loop before timing
/// out. Long synchronous commands (e.g. step_frames / wait_until with a
/// big frame budget) legitimately run for a minute or more in debug
/// builds; if the timeout fires, the late response skews the FIFO reply
/// stream (the driver then reads impossible map/coord pairs and appears
/// frozen). A generous timeout plus the stale-response drain below make
/// that practically impossible.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);

/// The debug server listens for TCP connections and forwards commands to the game loop.
pub struct DebugServer<C> {
    listener: TcpListener,
    command_sender: Sender<C>,
    response_receiver: Receiver<DebugResponse>,
}

/// Handle to the debug server from the game loop side.
/// Used to poll commands and send responses without blocking the game.
pub struct DebugServerHandle<C> {
    command_receiver: Receiver<C>,
    response_sender: Sender<DebugResponse>,
}

impl<C> DebugServerHandle<C> {
    /// Non-blocking poll of all pending commands from the channel.
    /// Returns all commands that have been queued since the last poll.
    pub fn poll_commands(&self) -> Vec<C> {
        let mut commands = Vec::new();
        loop {
            match self.command_receiver.try_recv() {
                Ok(cmd) => commands.push(cmd),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("DebugServerHandle: command channel disconnected");
                    break;
                }
            }
        }
        commands
    }

    /// Non-blocking send of a response back to the TCP client.
    /// Uses an unbounded channel so send never blocks the game loop.
    pub fn send_response(&self, response: DebugResponse) {
        match self.response_sender.send(response) {
            Ok(()) => {}
            Err(mpsc::SendError(resp)) => {
                warn!(
                    "DebugServerHandle: response channel disconnected, dropping response: {:?}",
                    resp
                );
            }
        }
    }
}

impl<C: DeserializeOwned> DebugServer<C> {
    /// Create a new debug server listening on the given port.
    /// Returns the server (to run on a background thread) and a handle
    /// (to use from the game loop for polling commands and sending responses).
    pub fn new(port: u16) -> Result<(Self, DebugServerHandle<C>), std::io::Error> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        info!("Debug server listening on port {}", port);

        let (cmd_tx, cmd_rx) = mpsc::channel::<C>();
        let (resp_tx, resp_rx) = mpsc::channel::<DebugResponse>();

        let server = DebugServer {
            listener,
            command_sender: cmd_tx,
            response_receiver: resp_rx,
        };

        let handle = DebugServerHandle {
            command_receiver: cmd_rx,
            response_sender: resp_tx,
        };

        Ok((server, handle))
    }

    /// The bound address (useful with port 0, e.g. in tests).
    pub fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
        self.listener.local_addr()
    }

    /// Run the debug server in a loop (should be called from a background thread).
    /// Accepts one connection at a time, reads JSON-line commands, forwards them
    /// to the game loop via channel, waits for response, and writes it back.
    pub fn run(&self) {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    info!(
                        "Debug server: client connected from {:?}",
                        stream.peer_addr()
                    );
                    self.handle_client(stream);
                    info!("Debug server: client disconnected");
                }
                Err(e) => {
                    error!("Debug server: failed to accept connection: {}", e);
                }
            }
        }
    }

    fn handle_client(&self, stream: TcpStream) {
        let reader = BufReader::new(match stream.try_clone() {
            Ok(s) => s,
            Err(_) => return,
        });
        let mut writer = stream;

        for line in reader.lines() {
            match line {
                Ok(line) => {
                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }

                    // Stale-response drain: requests and responses are
                    // correlated only by FIFO order on this shared channel.
                    // If an earlier command timed out, its late response
                    // would otherwise be delivered as the answer to THIS
                    // command and permanently skew the stream (the driver
                    // then reads a frozen world while the game runs on).
                    // Discard anything already queued before forwarding.
                    while let Ok(_) = self.response_receiver.try_recv() {}

                    match serde_json::from_str::<C>(&line) {
                        Ok(cmd) => {
                            match self.command_sender.send(cmd) {
                                Ok(()) => {
                                    match self
                                        .response_receiver
                                        .recv_timeout(RESPONSE_TIMEOUT)
                                    {
                                        Ok(resp) => {
                                            if let Ok(json) = serde_json::to_string(&resp) {
                                                let _ = writeln!(writer, "{}", json);
                                                let _ = writer.flush();
                                            }
                                        }
                                        Err(mpsc::RecvTimeoutError::Timeout) => {
                                            let resp = DebugResponse::err(
                                                "timeout waiting for game loop response"
                                                    .to_string(),
                                            );
                                            if let Ok(json) = serde_json::to_string(&resp) {
                                                let _ = writeln!(writer, "{}", json);
                                                let _ = writer.flush();
                                            }
                                        }
                                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                                            warn!(
                                                "Debug server: response channel disconnected"
                                            );
                                            return;
                                        }
                                    }
                                }
                                Err(mpsc::SendError(_)) => {
                                    let resp = DebugResponse::err(
                                        "game loop command channel disconnected".to_string(),
                                    );
                                    if let Ok(json) = serde_json::to_string(&resp) {
                                        let _ = writeln!(writer, "{}", json);
                                        let _ = writer.flush();
                                    }
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Debug server: failed to parse command: {}", e);
                            let resp =
                                DebugResponse::err(format!("invalid command: {}", e));
                            if let Ok(json) = serde_json::to_string(&resp) {
                                let _ = writeln!(writer, "{}", json);
                                let _ = writer.flush();
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Debug server: error reading from client: {}", e);
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug_server::protocol::CoreDebugCommand;
    use std::io::{BufRead, BufReader, Write};

    /// End-to-end over a real loopback socket: a client speaks the JSON-line
    /// protocol; the game-loop side (the test itself) polls the command off
    /// the handle and answers it.
    #[test]
    fn server_roundtrip_over_loopback() {
        let (server, handle) = DebugServer::<CoreDebugCommand>::new(0).unwrap();
        let addr = server.local_addr().unwrap();
        std::thread::spawn(move || server.run());

        let mut stream = TcpStream::connect(addr).unwrap();
        writeln!(stream, r#"{{"cmd":"step_frames","count":40}}"#).unwrap();
        stream.flush().unwrap();

        // Game-loop side: the exact command arrives on the handle.
        let mut received = None;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while received.is_none() && std::time::Instant::now() < deadline {
            let cmds = handle.poll_commands();
            if !cmds.is_empty() {
                received = Some(cmds);
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let cmds = received.expect("command forwarded to game loop");
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], CoreDebugCommand::StepFrames { count: 40 }));

        // The response written by the game loop comes back as one JSON line.
        handle.send_response(DebugResponse::ok_with_data(serde_json::json!({
            "stepped": 40
        })));
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["stepped"], 40);
    }

    /// A malformed line gets an error response without disturbing the
    /// connection — the next valid command still goes through.
    #[test]
    fn malformed_line_gets_error_response() {
        let (server, handle) = DebugServer::<CoreDebugCommand>::new(0).unwrap();
        let addr = server.local_addr().unwrap();
        std::thread::spawn(move || server.run());

        let mut stream = TcpStream::connect(addr).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        writeln!(stream, "not json at all").unwrap();
        stream.flush().unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().starts_with("invalid command"));

        // The connection survives: a valid command still reaches the handle.
        writeln!(stream, r#"{{"cmd":"get_state"}}"#).unwrap();
        stream.flush().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let cmds = handle.poll_commands();
            if !cmds.is_empty() {
                assert!(matches!(cmds[0], CoreDebugCommand::GetState));
                break;
            }
            assert!(std::time::Instant::now() < deadline, "command never arrived");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
