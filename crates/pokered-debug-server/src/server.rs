use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::protocol::{DebugCommand, DebugResponse};
use log::{error, info, warn};

/// Maximum time to wait for a response from the game loop before timing out.
/// Long synchronous commands (e.g. wait_until with a 1800-frame budget) can
/// legitimately run for several seconds in debug builds, so this must be
/// generous — see the stale-response drain below for why timeouts are still
/// dangerous even when handled.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

/// The debug server listens for TCP connections and forwards commands to the game loop.
pub struct DebugServer {
    listener: TcpListener,
    command_sender: Sender<DebugCommand>,
    response_receiver: Receiver<DebugResponse>,
}

/// Handle to the debug server from the game loop side.
/// Used to poll commands and send responses without blocking the game.
pub struct DebugServerHandle {
    command_receiver: Receiver<DebugCommand>,
    response_sender: Sender<DebugResponse>,
}

impl DebugServerHandle {
    /// Non-blocking poll of all pending commands from the channel.
    /// Returns all commands that have been queued since the last poll.
    pub fn poll_commands(&self) -> Vec<DebugCommand> {
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

impl DebugServer {
    /// Create a new debug server listening on the given port.
    /// Returns the server (to run on a background thread) and a handle
    /// (to use from the game loop for polling commands and sending responses).
    pub fn new(port: u16) -> Result<(Self, DebugServerHandle), std::io::Error> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        info!("Debug server listening on port {}", port);

        let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCommand>();
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

                    match serde_json::from_str::<DebugCommand>(&line) {
                        Ok(cmd) => {
                            info!("Debug server: received command: {:?}", cmd);
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
