//! Real network transport for link play (native only).
//!
//! Plain `std::net` TCP with newline-framed JSON — no async runtime. The
//! protocol is one [`NetworkMessage`] serialized with `serde_json` per line,
//! the same JSON-line convention as `crates/pokered-debug-server`. A
//! background reader thread parses incoming lines and forwards the messages
//! into an `mpsc` channel, so [`TcpTransport::try_recv`] never blocks the
//! game loop — mirroring the in-memory [`ChannelTransport`] used by the core
//! tests.

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::thread::JoinHandle;

use pokered_core::link::protocol::NetworkMessage;
use pokered_core::link::transport::{NetworkTransport, TransportError};

use super::codec::{decode_line, encode_line};

fn io_err(e: std::io::Error) -> TransportError {
    TransportError::IoError(e.to_string())
}

/// A failed write on an established socket means the peer is gone — map the
/// realistic "connection closed" error kinds to `Disconnected` so the state
/// machines take the disconnect path instead of the error path (mirrors
/// `ChannelTransport::send`, which reports `Disconnected` when the channel
/// is closed).
fn write_err(e: std::io::Error) -> TransportError {
    match e.kind() {
        std::io::ErrorKind::BrokenPipe
        | std::io::ErrorKind::ConnectionReset
        | std::io::ErrorKind::ConnectionAborted
        | std::io::ErrorKind::NotConnected => TransportError::Disconnected,
        _ => TransportError::IoError(e.to_string()),
    }
}

/// A TCP transport for link play.
///
/// Owns one connected socket. Sends are serialized (JSON + `\n`) under a
/// mutex; the receive side is a background reader thread feeding an `mpsc`
/// channel, so `try_recv` is non-blocking and mirrors
/// [`ChannelTransport`]'s semantics exactly: `Ok(None)` when nothing has
/// arrived, `Err(Disconnected)` once the peer is gone.
///
/// Drop shuts the socket down (which unblocks the reader thread's blocking
/// read), joins the reader thread, and only then returns — after a drop,
/// `try_recv` deterministically reports [`TransportError::Disconnected`],
/// same as a dropped `ChannelTransport`.
pub struct TcpTransport {
    /// Writer side of the socket; also used by `Drop` to shut the reader
    /// thread down.
    stream: TcpStream,
    writer: Mutex<TcpStream>,
    reader: Receiver<NetworkMessage>,
    reader_thread: Option<JoinHandle<()>>,
}

impl TcpTransport {
    /// Connect to a link-play peer (client side). Blocks until the TCP
    /// connection is established; use [`LinkServer`] for the listener side.
    pub fn connect(addr: SocketAddr) -> Result<Self, TransportError> {
        let stream = TcpStream::connect(addr).map_err(io_err)?;
        Self::from_stream(stream)
    }

    /// Wrap an already-connected stream (used by [`LinkServer::accept`] and
    /// tests). Forces the stream back into blocking mode — sockets accepted
    /// from a non-blocking listener inherit `O_NONBLOCK`, which would spin
    /// the reader thread on `WouldBlock` — and disables Nagle's algorithm
    /// (link messages are small and latency-sensitive).
    pub fn from_stream(stream: TcpStream) -> Result<Self, TransportError> {
        stream.set_nodelay(true).map_err(io_err)?;
        stream.set_nonblocking(false).map_err(io_err)?;
        let writer = stream.try_clone().map_err(io_err)?;
        let reader_stream = stream.try_clone().map_err(io_err)?;

        let (tx, rx) = mpsc::channel::<NetworkMessage>();
        let reader_thread = std::thread::Builder::new()
            .name("link-reader".to_string())
            .spawn(move || read_loop(reader_stream, tx))
            .map_err(|e| TransportError::IoError(e.to_string()))?;

        Ok(TcpTransport {
            stream,
            writer: Mutex::new(writer),
            reader: rx,
            reader_thread: Some(reader_thread),
        })
    }
}

impl NetworkTransport<NetworkMessage> for TcpTransport {
    fn send(&mut self, msg: NetworkMessage) -> Result<(), TransportError> {
        let json = encode_line(&msg)?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| TransportError::IoError("link writer lock poisoned".into()))?;
        writer.write_all(json.as_bytes()).map_err(write_err)?;
        writer.write_all(b"\n").map_err(write_err)?;
        writer.flush().map_err(write_err)?;
        Ok(())
    }

    fn recv(&mut self) -> Result<NetworkMessage, TransportError> {
        self.reader.recv().map_err(|_| TransportError::Disconnected)
    }

    fn try_recv(&mut self) -> Result<Option<NetworkMessage>, TransportError> {
        match self.reader.try_recv() {
            Ok(msg) => Ok(Some(msg)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(TransportError::Disconnected),
        }
    }
}

impl Drop for TcpTransport {
    fn drop(&mut self) {
        // Shutting the socket down unblocks the reader thread's blocking
        // read; joining it guarantees its channel sender is gone before we
        // return, so a subsequent `try_recv` deterministically reports
        // `Disconnected` (identical to a dropped `ChannelTransport`).
        let _ = self.stream.shutdown(Shutdown::Both);
        if let Some(thread) = self.reader_thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reader thread body: reads newline-framed JSON until EOF or an I/O error,
/// forwarding each parsed message into the channel. The sender is dropped on
/// exit, which makes the owning transport's `try_recv` return
/// `TransportError::Disconnected` (peer closed or transport dropped).
fn read_loop(stream: TcpStream, tx: mpsc::Sender<NetworkMessage>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF: peer closed the connection
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match decode_line::<NetworkMessage>(trimmed) {
                    Ok(msg) => {
                        if tx.send(msg).is_err() {
                            break; // transport dropped
                        }
                    }
                    Err(e) => {
                        // Malformed peer data: drop the frame and keep the
                        // connection. Newline framing stays intact, so the
                        // stream cannot desync, but the state machines may
                        // — log loudly.
                        log::warn!("[link] dropping malformed frame: {}", e);
                    }
                }
            }
            Err(_) => break, // I/O error (e.g. socket shut down on drop)
        }
    }
}

/// Listener side of link play (the `--link-listen` host).
///
/// One peer per game — the original Cable Club is single-link — so after
/// [`LinkServer::accept`] returns a transport the server is dropped and the
/// [`TcpTransport`] takes over.
///
/// `accept` is non-blocking: the listener is set non-blocking at bind time
/// and the game loop polls it once per frame. A blocking accept-thread was
/// considered but adds thread lifecycle and shutdown complexity for zero
/// benefit — at 60 fps polling the accept latency is at most one frame,
/// which is irrelevant when waiting for a human to join.
pub struct LinkServer {
    listener: TcpListener,
}

impl LinkServer {
    /// Bind a listener on `addr` in non-blocking mode.
    pub fn new(addr: SocketAddr) -> Result<Self, TransportError> {
        let listener = TcpListener::bind(addr).map_err(io_err)?;
        listener.set_nonblocking(true).map_err(io_err)?;
        Ok(LinkServer { listener })
    }

    /// The bound address (useful with port 0, e.g. `127.0.0.1:0` in tests).
    #[allow(dead_code)]
    pub fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
        self.listener.local_addr()
    }

    /// Try to accept the single peer. `Ok(None)` when no connection is
    /// pending yet (call again next frame).
    pub fn accept(&self) -> Result<Option<TcpTransport>, TransportError> {
        match self.listener.accept() {
            Ok((stream, _peer)) => Ok(Some(TcpTransport::from_stream(stream)?)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(io_err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    /// Poll `cond` until it returns true or `timeout` elapses. Bounded and
    /// sleep-free on the happy path: 1 ms sleeps only while waiting.
    fn wait_until<F: FnMut() -> bool>(timeout: Duration, mut cond: F) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if cond() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// A connected `(client, server)` pair on loopback with a real socket.
    fn tcp_pair() -> (TcpTransport, TcpTransport) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpTransport::connect(addr).unwrap();
        let (raw_server, _peer) = listener.accept().unwrap();
        (client, TcpTransport::from_stream(raw_server).unwrap())
    }

    #[test]
    fn connect_listen_exchange_hello_ack() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        // Client side: direct connect.
        let mut client = TcpTransport::connect(addr).unwrap();

        // Server side: non-blocking accept on a LinkServer.
        let server = LinkServer::new("127.0.0.1:0".parse().unwrap()).unwrap();
        drop(server); // (unused here; LinkServer itself is exercised below)

        let (raw_server, _peer) = listener.accept().unwrap();
        let mut server_transport = TcpTransport::from_stream(raw_server).unwrap();

        // Exchange Hello/HelloAck through the real sockets.
        client.send(NetworkMessage::hello()).unwrap();
        assert!(wait_until(Duration::from_secs(5), || {
            matches!(
                server_transport.try_recv(),
                Ok(Some(NetworkMessage::Hello {
                    version: NetworkMessage::PROTOCOL_VERSION
                }))
            )
        }));
        server_transport.send(NetworkMessage::hello_ack()).unwrap();
        assert!(wait_until(Duration::from_secs(5), || {
            matches!(
                client.try_recv(),
                Ok(Some(NetworkMessage::HelloAck {
                    version: NetworkMessage::PROTOCOL_VERSION
                }))
            )
        }));
    }

    #[test]
    fn link_server_accept_is_nonblocking_then_returns_peer() {
        let server = LinkServer::new("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = server.local_addr().unwrap();

        // No peer yet: accept returns Ok(None) immediately, non-blocking.
        assert!(matches!(server.accept(), Ok(None)));

        // Once the client connects, a subsequent accept returns the peer.
        let _client = TcpStream::connect(addr).unwrap();
        assert!(wait_until(Duration::from_secs(5), || {
            matches!(server.accept(), Ok(Some(_)))
        }));
    }

    #[test]
    fn fragmented_write_is_reassembled() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let mut raw_client = TcpStream::connect(addr).unwrap();
        let (raw_server, _peer) = listener.accept().unwrap();

        // Wrap the server first so its reader thread is live before the
        // payload arrives — it may see a partial line mid-frame.
        let mut server = TcpTransport::from_stream(raw_server).unwrap();

        // Send one message's JSON split into three fragments with flushes in
        // between, as TCP segmentation may deliver it.
        let json = serde_json::to_string(&NetworkMessage::hello()).unwrap();
        let bytes = json.as_bytes();
        let chunk_size = bytes.len() / 3 + 1;
        for chunk in bytes.chunks(chunk_size) {
            raw_client.write_all(chunk).unwrap();
            raw_client.flush().unwrap();
        }
        raw_client.write_all(b"\n").unwrap();
        raw_client.flush().unwrap();

        // The reader thread must reassemble the line no matter how the
        // fragments interleave with its reads.
        let mut received = None;
        assert!(wait_until(Duration::from_secs(5), || match server.try_recv() {
            Ok(Some(msg)) => {
                received = Some(msg);
                true
            }
            _ => false,
        }));
        assert_eq!(received, Some(NetworkMessage::hello()));
    }

    #[test]
    fn drop_signals_disconnected_to_peer() {
        let (client, mut server) = tcp_pair();

        // Dropping the client shuts the socket down; the server's reader
        // thread hits EOF and drops its channel sender → Disconnected.
        drop(client);
        assert!(wait_until(Duration::from_secs(5), || {
            matches!(server.try_recv(), Err(TransportError::Disconnected))
        }));

        // And in the other direction.
        let (mut client2, server2) = tcp_pair();
        drop(server2);
        assert!(wait_until(Duration::from_secs(5), || {
            matches!(client2.try_recv(), Err(TransportError::Disconnected))
        }));
    }

    #[test]
    fn send_after_peer_drop_is_disconnected() {
        let (mut client, server) = tcp_pair();
        drop(server);
        assert!(wait_until(Duration::from_secs(5), || {
            matches!(
                client.send(NetworkMessage::hello()),
                Err(TransportError::Disconnected)
            )
        }));
    }
}
