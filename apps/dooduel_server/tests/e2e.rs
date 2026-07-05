//! Localhost e2e smoke (spec §9.5, plan W4.7) — the RUN-THE-ARTIFACT evidence produced
//! headlessly: spawn the real `dooduel_server` binary, connect real `WsClientTransport`
//! clients over actual WebSockets, and drive a full Create → Join → StartMatch → Pick →
//! draw → guess loop, asserting the events at each step. This is the wave's proof that
//! the whole wire path works end-to-end through real sockets.
//!
//! No-flake discipline (spec §9.5): the server binds **port 0** and prints
//! `LISTENING port=<n>`; the test parses that line (never a fixed port), and every wait
//! is **condition-based** with a deadline — a small poll interval, never a sleep used AS
//! synchronization. W5 grows this into the four-client + reconnect + replay e2e.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use dooduel_core::protocol::{ClientIntent, PROTOCOL_VERSION, ServerEvent, WireAvatar};
use dooduel_core::transport::{ClientTransport, ConnStatus, WsClientTransport};

/// A generous per-condition deadline — well above the server's 100 ms tick + localhost
/// latency, so a slow CI host never flakes, yet a genuine hang fails in bounded time.
const DEADLINE: Duration = Duration::from_secs(20);

/// The running server process + its bound port. Killed on drop so a panicking test never
/// leaks a listener.
struct ServerProc {
    child: Child,
    port: u16,
}

impl Drop for ServerProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawn the server on an OS-chosen port and parse its `LISTENING port=` line (spec §9.5).
fn spawn_server() -> ServerProc {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dooduel_server"))
        .args(["--port", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn dooduel_server");
    let stdout = child.stdout.take().expect("server stdout piped");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("read the LISTENING line");
    let port = line
        .trim()
        .strip_prefix("LISTENING port=")
        .unwrap_or_else(|| panic!("expected a LISTENING line, got {line:?}"))
        .parse::<u16>()
        .expect("a numeric port");
    // Drop the reader: the server writes nothing more to stdout (all further logs go to
    // stderr), so the closed pipe is never written to.
    ServerProc { child, port }
}

/// A buffered client: drains the poll-style transport into a buffer and searches it, so
/// an event that arrives before we look for it is never missed.
struct Client {
    t: WsClientTransport,
    buf: Vec<ServerEvent>,
}

impl Client {
    fn connect(port: u16) -> Self {
        let url = format!("ws://127.0.0.1:{port}");
        let t = WsClientTransport::connect(url).expect("client connects");
        Client { t, buf: Vec::new() }
    }

    fn send(&mut self, intent: ClientIntent) {
        self.t.send(&intent);
    }

    fn pump(&mut self) {
        while let Some(ev) = self.t.try_recv() {
            self.buf.push(ev);
        }
    }

    /// Wait (condition-based) for the first buffered event matching `pred`, removing +
    /// returning it. Panics on the deadline with the buffer contents for diagnosis.
    fn wait<F>(&mut self, what: &str, pred: F) -> ServerEvent
    where
        F: Fn(&ServerEvent) -> bool,
    {
        let start = Instant::now();
        loop {
            self.pump();
            if let Some(i) = self.buf.iter().position(&pred) {
                return self.buf.remove(i);
            }
            assert!(
                start.elapsed() < DEADLINE,
                "timed out waiting for {what}; buffer = {:?}",
                self.buf
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Wait until the connection reports Closed (for the reject path).
    fn wait_closed(&mut self, what: &str) {
        let start = Instant::now();
        loop {
            self.pump(); // drain events so the status transitions are processed
            if self.t.status() == ConnStatus::Closed {
                return;
            }
            assert!(
                start.elapsed() < DEADLINE,
                "timed out waiting for {what} to close"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn create(name: &str) -> ClientIntent {
    ClientIntent::Create {
        name: name.to_string(),
        avatar: WireAvatar::Default,
        protocol_version: PROTOCOL_VERSION,
    }
}

#[test]
fn full_match_loop_over_real_sockets() {
    let server = spawn_server();

    // A creates a room; the server issues the code + seats A at 0 (host).
    let mut a = Client::connect(server.port);
    a.send(create("Ada"));
    let room_code = match a.wait("A's Welcome", |e| matches!(e, ServerEvent::Welcome { .. })) {
        ServerEvent::Welcome {
            seat, room_code, ..
        } => {
            assert_eq!(seat, 0, "the creator is seated at 0 (host)");
            room_code
        }
        other => panic!("expected Welcome, got {other:?}"),
    };
    assert_eq!(room_code.len(), 6, "the server issued a 6-char room code");

    // B joins by that code; the server seats B at 1.
    let mut b = Client::connect(server.port);
    b.send(ClientIntent::Join {
        room: room_code.clone(),
        name: "Bo".to_string(),
        avatar: WireAvatar::Default,
        protocol_version: PROTOCOL_VERSION,
        reconnect: None,
    });
    match b.wait("B's Welcome", |e| matches!(e, ServerEvent::Welcome { .. })) {
        ServerEvent::Welcome { seat, .. } => assert_eq!(seat, 1, "the joiner is seated at 1"),
        other => panic!("expected Welcome, got {other:?}"),
    }

    // A (host) starts the match and is offered word choices (drawer-only).
    a.send(ClientIntent::StartMatch);
    a.wait("A's word choices", |e| {
        matches!(e, ServerEvent::WordChoices { .. })
    });

    // A picks a word; as the drawer, A's WordUpdate reveals the full word.
    a.send(ClientIntent::Pick { index: 0 });
    let word = match a.wait("A's revealed word", |e| {
        matches!(e, ServerEvent::WordUpdate { display, .. } if !display.contains('_') && display.chars().any(|c| c.is_ascii_alphabetic()))
    }) {
        ServerEvent::WordUpdate { display, .. } => display
            .chars()
            .filter(|c| c.is_ascii_alphabetic())
            .collect::<String>()
            .to_lowercase(),
        other => panic!("expected WordUpdate, got {other:?}"),
    };
    assert!(!word.is_empty(), "the drawer sees a non-empty word");

    // A draws a stroke; B (the guesser) receives it as an applied canvas op.
    a.send(ClientIntent::Stroke {
        stroke_id: 1,
        points: vec![(10, 10), (20, 20), (30, 25)],
        color: [20, 20, 24, 255],
        radius: 4,
        done: true,
    });
    b.wait("B sees the drawer's stroke", |e| {
        matches!(e, ServerEvent::CanvasOpApplied { .. })
    });

    // B guesses the word (the test is omniscient — it read it from A's stream); the
    // server scores it and broadcasts a correct GuessResult for seat 1.
    b.send(ClientIntent::Guess { text: word });
    match b.wait("B's correct guess", |e| {
        matches!(e, ServerEvent::GuessResult { correct: true, .. })
    }) {
        ServerEvent::GuessResult { seat, correct, .. } => {
            assert_eq!(seat, 1);
            assert!(correct);
        }
        other => panic!("expected a correct GuessResult, got {other:?}"),
    }
}

#[test]
fn version_mismatch_is_rejected_over_the_wire() {
    // The production version gate (W2-review): a first frame with the wrong
    // protocol_version gets Error{VersionMismatch} and the socket is closed — before any
    // Welcome, over a real socket.
    let server = spawn_server();
    let mut c = Client::connect(server.port);
    c.send(ClientIntent::Create {
        name: "Ada".to_string(),
        avatar: WireAvatar::Default,
        protocol_version: PROTOCOL_VERSION + 1,
    });
    let err = c.wait("a version error", |e| {
        matches!(e, ServerEvent::Error { .. })
    });
    match err {
        ServerEvent::Error { code, .. } => assert_eq!(
            code,
            dooduel_core::protocol::ErrorCode::VersionMismatch,
            "a bad protocol version is VersionMismatch"
        ),
        other => panic!("expected an Error, got {other:?}"),
    }
    // And no Welcome ever arrives; the server closes the socket.
    c.wait_closed("the rejected connection");
}
