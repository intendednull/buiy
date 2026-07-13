//! The two-process e2e (spec §9.5, plan W5.3) — the RUN-THE-ARTIFACT proof produced
//! headlessly: spawn the real `dooduel_server` binary and drive it through real
//! `dooduel_mcp::HeadlessClient`s over actual WebSockets. It covers the whole networked
//! spine end-to-end: a full four-client match to the podium, a mid-match live-token
//! reconnect (RoomState + CanvasLog resync + the displaced socket's FIN), the wire-level
//! guards W4 tested only at the sync/pure tier (rate-limit disconnect, per-IP throttle,
//! host migration, MatchInProgress, single-use token rotation), and the networked replay
//! assertion (spec §9.6 — a recorded event stream re-folds byte-identically).
//!
//! No-flake discipline (spec §9.5): the server binds **port 0** and prints
//! `LISTENING port=<n>`; the test parses that line (bounded by a deadline so a bin that
//! never prints it fails fast, not hangs), and every wait is **condition-based** with a
//! deadline — a small poll interval, never a sleep used AS synchronization, never a retry.
//! Each wire-level guard runs against its OWN server so the per-IP limiter bucket (keyed
//! on 127.0.0.1) is isolated per test.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use dooduel_core::game::Phase;
use dooduel_core::protocol::{ClientIntent, ErrorCode, PROTOCOL_VERSION, ServerEvent, WireAvatar};
use dooduel_core::transport::{ConnStatus, WsClientTransport};
use dooduel_mcp::{HeadlessClient, ReplicaFold};

/// A generous per-condition deadline — far above the server's 100 ms tick + localhost
/// latency (so a slow CI host never flakes) yet well under nextest's 600 s hang ceiling
/// (so a genuine deadlock fails in bounded time).
const DEADLINE: Duration = Duration::from_secs(20);

/// The poll interval between condition checks (never a sleep-as-synchronization).
const POLL: Duration = Duration::from_millis(5);

// ---------------------------------------------------------------------------
// The server process.
// ---------------------------------------------------------------------------

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
/// The `read_line` is bounded by [`DEADLINE`] on a reader thread (W5.3): a bin that never
/// prints the discovery line fails fast rather than blocking the test forever.
fn spawn_server() -> ServerProc {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dooduel_server"))
        .args(["--port", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn dooduel_server");
    let stdout = child.stdout.take().expect("server stdout piped");

    // Read the one discovery line on a thread, bounded by a deadline via the channel.
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let _ = reader.read_line(&mut line);
        let _ = tx.send(line);
    });
    let line = match rx.recv_timeout(DEADLINE) {
        Ok(line) => line,
        Err(_) => {
            let _ = child.kill();
            panic!("the server never printed a LISTENING line within {DEADLINE:?}");
        }
    };
    let port = line
        .trim()
        .strip_prefix("LISTENING port=")
        .unwrap_or_else(|| panic!("expected a LISTENING line, got {line:?}"))
        .parse::<u16>()
        .expect("a numeric port");
    ServerProc { child, port }
}

// ---------------------------------------------------------------------------
// A real networked agent (a HeadlessClient + an event buffer / replay recorder).
// ---------------------------------------------------------------------------

/// One networked seat: a real `HeadlessClient` over a WebSocket, plus an inbox (for
/// event-shaped condition waits) and a full ordered recording (for the §9.6 replay).
struct Agent {
    hc: HeadlessClient<WsClientTransport>,
    /// Events pumped-but-not-yet-consumed by a `wait` (searched + drained by condition).
    inbox: Vec<ServerEvent>,
    /// Every event this seat received, in order — the replay recording (spec §9.6).
    record: Vec<ServerEvent>,
}

impl Agent {
    fn connect(port: u16) -> Self {
        let url = format!("ws://127.0.0.1:{port}");
        let hc = HeadlessClient::connect(url).expect("client connects");
        Agent {
            hc,
            inbox: Vec::new(),
            record: Vec::new(),
        }
    }

    /// Drain the transport into the fold, recording every event (in order) + buffering it
    /// for condition waits.
    fn pump(&mut self) {
        for ev in self.hc.pump() {
            self.record.push(ev.clone());
            self.inbox.push(ev);
        }
    }

    /// Wait (condition-based) for the first buffered event matching `pred`, removing +
    /// returning it. Panics on the deadline with the buffer for diagnosis.
    fn wait_event<F: Fn(&ServerEvent) -> bool>(&mut self, what: &str, pred: F) -> ServerEvent {
        let start = Instant::now();
        loop {
            self.pump();
            if let Some(i) = self.inbox.iter().position(&pred) {
                return self.inbox.remove(i);
            }
            assert!(
                start.elapsed() < DEADLINE,
                "timed out waiting for {what}; inbox = {:?}",
                self.inbox
            );
            thread::sleep(POLL);
        }
    }

    /// Wait until this agent's transport reports Closed (the displaced-socket FIN / reject).
    fn wait_closed(&mut self, what: &str) {
        let start = Instant::now();
        loop {
            self.pump(); // process the WS Closed event
            if self.hc.status() == ConnStatus::Closed {
                return;
            }
            assert!(
                start.elapsed() < DEADLINE,
                "timed out waiting for {what} to close"
            );
            thread::sleep(POLL);
        }
    }

    fn replica(&self) -> &dooduel_core::protocol::RoomReplica {
        self.hc.replica()
    }
}

/// Pump every agent once (so no socket backs up while we drive the match).
fn pump_all(agents: &mut [Agent]) {
    for a in agents.iter_mut() {
        a.pump();
    }
}

/// Wait until `pred` holds of `agents[observer]`'s replica, pumping ALL agents each poll
/// (so every socket keeps draining). Panics on the deadline.
fn wait_replica<F: Fn(&dooduel_core::protocol::RoomReplica) -> bool>(
    agents: &mut [Agent],
    observer: usize,
    what: &str,
    pred: F,
) {
    let start = Instant::now();
    loop {
        pump_all(agents);
        if pred(agents[observer].replica()) {
            return;
        }
        assert!(
            start.elapsed() < DEADLINE,
            "timed out waiting for {what}; observer replica phase={:?} drawer={:?}",
            agents[observer].replica().phase,
            agents[observer].replica().drawer,
        );
        thread::sleep(POLL);
    }
}

/// Read the drawer's word once its row is fully revealed (the test is omniscient — it
/// reads the secret from the drawer's own stream, exactly as a drawer sees it).
fn read_word(agents: &mut [Agent], drawer: usize) -> String {
    wait_replica(agents, drawer, "the drawer's revealed word", |r| {
        r.phase == Phase::Drawing
            && !r.word_display.is_empty()
            && r.word_slots().iter().all(|(_, revealed)| *revealed)
    });
    agents[drawer]
        .replica()
        .word_slots()
        .iter()
        .map(|(c, _)| *c)
        .collect::<String>()
        .to_lowercase()
}

// ---------------------------------------------------------------------------
// The flagship: a full four-client match to the podium.
// ---------------------------------------------------------------------------

#[test]
fn full_networked_match_to_podium() {
    let server = spawn_server();

    // Four agents take seats 0..4 (create + three joins).
    let mut a = Agent::connect(server.port);
    a.hc.create("Ada");
    let room = match a.wait_event("A's Welcome", |e| matches!(e, ServerEvent::Welcome { .. })) {
        ServerEvent::Welcome {
            seat, room_code, ..
        } => {
            assert_eq!(seat, 0, "the creator is seat 0 (host)");
            assert_eq!(room_code.len(), 6, "a 6-char room code");
            room_code
        }
        other => panic!("expected Welcome, got {other:?}"),
    };

    let mut b = Agent::connect(server.port);
    b.hc.join(&room, "Bo", None);
    b.wait_event("B's Welcome", |e| {
        matches!(e, ServerEvent::Welcome { seat: 1, .. })
    });

    let mut c = Agent::connect(server.port);
    c.hc.join(&room, "Cy", None);
    c.wait_event("C's Welcome", |e| {
        matches!(e, ServerEvent::Welcome { seat: 2, .. })
    });

    let mut d = Agent::connect(server.port);
    d.hc.join(&room, "Di", None);
    d.wait_event("D's Welcome", |e| {
        matches!(e, ServerEvent::Welcome { seat: 3, .. })
    });
    let d_token = d.hc.reconnect_token().to_string();
    assert!(!d_token.is_empty(), "D holds a reconnect token");

    let mut agents = vec![a, b, c, d];

    // Everyone should see the full four-seat roster before the host starts.
    wait_replica(&mut agents, 0, "the four-seat roster", |r| {
        r.players.len() == 4 && r.players.iter().all(|p| p.connected)
    });

    // The host starts the match.
    agents[0].hc.start_match();
    wait_replica(&mut agents, 0, "the first Picking", |r| {
        r.phase == Phase::Picking
    });

    // Drive every turn to the podium. Turn 0 additionally exercises the multi-batch
    // stroke + undo and the mid-match live-token reconnect.
    let mut turn = 0usize;
    let mut displaced: Option<Agent> = None;
    loop {
        // --- Picking: the drawer commits choice 0 ---
        let drawer = agents[0]
            .replica()
            .drawer
            .expect("a drawer is set in Picking");
        // W4-review carry-in (re-added, minor 7): the drawer receives its WordChoices over
        // the wire (drawer-only, in Picking) before it can pick.
        if turn == 0 {
            agents[drawer].wait_event("the drawer's WordChoices wire receipt", |e| {
                matches!(e, ServerEvent::WordChoices { .. })
            });
        }
        agents[drawer].hc.pick(0);
        wait_replica(&mut agents, 0, "Drawing", |r| r.phase == Phase::Drawing);

        if turn == 0 {
            // The multi-batch stroke + a second stroke + an undo (spec §3.5): the guessers
            // observe the finalized ops, and the undo removes the last (id 1), leaving id 0.
            let ink = [20u8, 20, 24, 255];
            agents[drawer]
                .hc
                .stroke(1, vec![(40, 40), (120, 90)], ink, 5, false); // batch 1
            agents[drawer]
                .hc
                .stroke(1, vec![(200, 160), (320, 260)], ink, 5, true); // finalize → op 0
            agents[drawer]
                .hc
                .stroke(2, vec![(400, 300), (500, 360)], ink, 5, true); // op 1
            agents[drawer].hc.undo(); // remove op 1

            // A guesser (seat 1, never the drawer this turn) resolves the multi-batch +
            // undo to exactly one op with the dense id 0.
            wait_replica(
                &mut agents,
                1,
                "the guesser's canvas after stroke+undo",
                |r| {
                    r.canvas_ops.len() == 1
                        && matches!(
                            r.canvas_ops[0],
                            dooduel_core::protocol::CanvasOp::Stroke { id: 0, .. }
                        )
                },
            );
            assert!(
                !agents[1].hc.canvas_png().is_empty(),
                "the guesser rasterizes a non-empty canvas"
            );

            // --- The mid-match live-token reconnect (RoomState + CanvasLog + FIN) ---
            // A fresh socket presents D's token while D's original socket is still open:
            // the server closes the old one (D observes the FIN) and reseeds the new one.
            let mut d_new = Agent::connect(server.port);
            d_new.hc.join(&room, "Di", Some(d_token.clone()));
            d_new.wait_event("D_new's re-Welcome", |e| {
                matches!(e, ServerEvent::Welcome { seat: 3, .. })
            });
            d_new.wait_event("D_new's RoomState resync", |e| {
                matches!(e, ServerEvent::RoomState(_))
            });
            let log = d_new.wait_event("D_new's CanvasLog resync", |e| {
                matches!(e, ServerEvent::CanvasLog { .. })
            });
            match log {
                ServerEvent::CanvasLog { ops } => assert_eq!(
                    ops.len(),
                    1,
                    "the reconnect CanvasLog carries the current turn's one surviving op"
                ),
                _ => unreachable!(),
            }
            // The displaced original socket observes the close (the FIN assertion).
            agents[3].wait_closed("the displaced original D socket");
            // Swap in the reconnected agent for the rest of the match; set the old aside.
            displaced = Some(std::mem::replace(&mut agents[3], d_new));
        }

        // --- Drawing: the guessers guess the (omnisciently-read) word ---
        let word = read_word(&mut agents, drawer);

        // W5-review Important 3 — LIVE secrecy over the wire: before ANY guess this turn,
        // each guesser's honest report must NOT contain the secret; the drawer's DOES (the
        // positive control that proves the check can see it). This asserts the load-bearing
        // anti-cheat property at the live networked tier, every turn.
        for (i, agent) in agents.iter().enumerate() {
            let report = agent.hc.state_report().to_lowercase();
            if i == drawer {
                assert!(
                    report.contains(word.as_str()),
                    "the drawer (seat {i}) report carries the secret (positive control)"
                );
            } else {
                assert!(
                    !report.contains(word.as_str()),
                    "seat {i} (guesser) must NOT see the secret {word:?} pre-guess:\n{report}"
                );
            }
        }

        for (i, agent) in agents.iter_mut().enumerate() {
            if i != drawer {
                agent.hc.guess(word.clone());
            }
        }
        wait_replica(&mut agents, 0, "Reveal (all guessed)", |r| {
            r.phase == Phase::Reveal
        });
        if turn == 0 {
            // W4-review carry-in (re-added, minor 7): a guesser receives a direct correct
            // GuessResult over the wire.
            agents[1].wait_event("a correct GuessResult wire receipt", |e| {
                matches!(e, ServerEvent::GuessResult { correct: true, .. })
            });
        }

        // --- Reveal: advance ---
        agents[0].hc.continue_turn();
        wait_replica(&mut agents, 0, "the next turn or the podium", |r| {
            r.phase == Phase::Picking || r.phase == Phase::Final
        });
        turn += 1;
        if agents[0].replica().phase == Phase::Final {
            break;
        }
        assert!(
            turn < 32,
            "a 4-player 2-round match ends well within 8 turns"
        );
    }

    // The podium (spec §3.3 — the lift rides MatchEnded).
    wait_replica(&mut agents, 0, "the podium", |r| r.podium.is_some());
    let podium = agents[0].replica().podium.clone().expect("a podium");
    assert_eq!(podium.len(), 4, "all four seats appear on the podium");
    assert!(
        podium.windows(2).all(|w| w[0].2 >= w[1].2),
        "the podium is score-ordered (highest first): {podium:?}"
    );

    // --- The networked replay assertion (spec §9.6 / §3.4) ---
    // Re-fold A's recorded event stream into a fresh replica: the fold is a pure function
    // of the stream, so it reproduces A's live replica byte-identically.
    let mut replay = ReplicaFold::default();
    for ev in &agents[0].record {
        replay.apply(ev.clone());
    }
    let live = serde_json::to_string(agents[0].hc.replica()).expect("serialize live");
    let replayed = serde_json::to_string(&replay.replica).expect("serialize replay");
    assert_eq!(
        live, replayed,
        "a recorded networked stream re-folds byte-identically (spec §9.6)"
    );

    drop(displaced);
}

// ---------------------------------------------------------------------------
// The wire-level guards (each on its own server ⇒ an isolated per-IP bucket).
// ---------------------------------------------------------------------------

#[test]
fn version_mismatch_is_rejected_over_the_wire() {
    // A first frame with the wrong protocol_version gets Error{VersionMismatch} + a socket
    // close — before any Welcome, over a real socket (the production version gate, W4).
    let server = spawn_server();
    let mut c = Agent::connect(server.port);
    c.hc.send_raw(ClientIntent::Create {
        name: "Ada".to_string(),
        avatar: WireAvatar::Default,
        protocol_version: PROTOCOL_VERSION + 1,
    });
    match c.wait_event("a version error", |e| {
        matches!(e, ServerEvent::Error { .. })
    }) {
        ServerEvent::Error { code, .. } => {
            assert_eq!(
                code,
                ErrorCode::VersionMismatch,
                "a bad version is VersionMismatch"
            )
        }
        other => panic!("expected an Error, got {other:?}"),
    }
    c.wait_closed("the rejected connection");
}

#[test]
fn a_fresh_join_mid_match_is_rejected_matchinprogress() {
    // Once a match is running, a FRESH join (no reconnect token) is MatchInProgress — M1
    // seats new players only in the lobby (spec §3.2).
    let server = spawn_server();
    let mut a = Agent::connect(server.port);
    a.hc.create("Ada");
    let room = match a.wait_event("A's Welcome", |e| matches!(e, ServerEvent::Welcome { .. })) {
        ServerEvent::Welcome { room_code, .. } => room_code,
        other => panic!("expected Welcome, got {other:?}"),
    };
    let mut b = Agent::connect(server.port);
    b.hc.join(&room, "Bo", None);
    b.wait_event("B's Welcome", |e| matches!(e, ServerEvent::Welcome { .. }));

    a.hc.start_match();
    // Let both drain the match-start events so the server has certainly started.
    a.wait_event("Picking", |e| {
        matches!(
            e,
            ServerEvent::PhaseChanged {
                phase: Phase::Picking,
                ..
            }
        )
    });

    // A fresh fifth join lands mid-match → MatchInProgress + close.
    let mut e = Agent::connect(server.port);
    e.hc.join(&room, "Ev", None);
    match e.wait_event("the mid-match rejection", |ev| {
        matches!(ev, ServerEvent::Error { .. })
    }) {
        ServerEvent::Error { code, .. } => {
            assert_eq!(
                code,
                ErrorCode::MatchInProgress,
                "a fresh mid-match join is MatchInProgress"
            )
        }
        other => panic!("expected an Error, got {other:?}"),
    }
    e.wait_closed("the rejected fifth join");
}

#[test]
fn host_migration_over_the_wire() {
    // The host leaves gracefully (skipping the 45 s grace); a guest observes the
    // `Roster{host}` migrate to it over the wire — no clock wait needed (spec §6.2).
    let server = spawn_server();
    let mut a = Agent::connect(server.port);
    a.hc.create("Ada");
    let room = match a.wait_event("A's Welcome", |e| matches!(e, ServerEvent::Welcome { .. })) {
        ServerEvent::Welcome { room_code, .. } => room_code,
        other => panic!("expected Welcome, got {other:?}"),
    };
    let mut b = Agent::connect(server.port);
    b.hc.join(&room, "Bo", None);
    b.wait_event("B's Welcome", |e| {
        matches!(e, ServerEvent::Welcome { seat: 1, .. })
    });

    // B first sees the host as seat 0.
    wait_replica(std::slice::from_mut(&mut b), 0, "B sees host 0", |r| {
        r.players.len() == 2 && r.host == 0
    });

    // The host (seat 0) leaves; host migrates to seat 1 (B).
    a.hc.leave();
    let start = Instant::now();
    loop {
        b.pump();
        if b.replica().host == 1 {
            break;
        }
        assert!(start.elapsed() < DEADLINE, "host never migrated to seat 1");
        thread::sleep(POLL);
    }
    assert_eq!(
        b.replica().host,
        1,
        "host migrated to the remaining seat over the wire"
    );
}

#[test]
fn the_reconnect_token_is_single_use() {
    // Token rotation (spec §6.3): a reconnect rotates the seat's token; the OLD token then
    // fails BadToken — closing the sniffed-token replay hole.
    let server = spawn_server();
    let mut a = Agent::connect(server.port);
    a.hc.create("Ada");
    let (room, tok1) =
        match a.wait_event("A's Welcome", |e| matches!(e, ServerEvent::Welcome { .. })) {
            ServerEvent::Welcome {
                room_code,
                reconnect_token,
                ..
            } => (room_code, reconnect_token),
            other => panic!("expected Welcome, got {other:?}"),
        };

    // A reconnect with tok1 rotates the seat's token to tok2 (and displaces A's socket).
    let mut a2 = Agent::connect(server.port);
    a2.hc.join(&room, "Ada", Some(tok1.clone()));
    let tok2 = match a2.wait_event("A2's re-Welcome", |e| {
        matches!(e, ServerEvent::Welcome { .. })
    }) {
        ServerEvent::Welcome {
            reconnect_token, ..
        } => reconnect_token,
        other => panic!("expected Welcome, got {other:?}"),
    };
    assert_ne!(tok1, tok2, "the reconnect rotated the token");

    // A THIRD connection presenting the now-stale tok1 is rejected BadToken.
    let mut a3 = Agent::connect(server.port);
    a3.hc.join(&room, "Ada", Some(tok1));
    match a3.wait_event("the stale-token rejection", |e| {
        matches!(e, ServerEvent::Error { .. })
    }) {
        ServerEvent::Error { code, .. } => {
            assert_eq!(
                code,
                ErrorCode::BadToken,
                "the rotated-away token is single-use"
            )
        }
        other => panic!("expected an Error, got {other:?}"),
    }
    a3.wait_closed("the stale-token connection");
}

#[test]
fn a_flooding_client_is_rate_limited_and_disconnected() {
    // The per-connection intent cap (spec §3.1): a client bursting far past the 480-token
    // bucket gets Error{RateLimited} and the socket is closed.
    let server = spawn_server();
    let mut a = Agent::connect(server.port);
    a.hc.create("Ada");
    a.wait_event("A's Welcome", |e| matches!(e, ServerEvent::Welcome { .. }));

    // Flood well past the burst (480) faster than the 240/s refill can keep up.
    for _ in 0..2000 {
        a.hc.guess("x");
    }
    match a.wait_event("the rate-limit error", |e| {
        matches!(
            e,
            ServerEvent::Error {
                code: ErrorCode::RateLimited,
                ..
            }
        )
    }) {
        ServerEvent::Error { code, .. } => assert_eq!(code, ErrorCode::RateLimited),
        other => panic!("expected RateLimited, got {other:?}"),
    }
    a.wait_closed("the flooding connection");
}

#[test]
fn per_ip_join_attempts_are_throttled() {
    // The per-IP limiter (spec §6.2 — the room-code brute-force guard): a CONCURRENT burst
    // of join attempts past the per-IP bucket (20) is rejected RateLimited over the wire.
    // Firing them concurrently (W5-review minor 5) removes the RTT-dependence a serial loop
    // had — the burst genuinely races the bucket refill rather than being paced by each
    // attempt's round-trip. All attempts share the 127.0.0.1 bucket on this fresh registry.
    let server = spawn_server();
    let port = server.port;
    let handles: Vec<_> = (0..40)
        .map(|i| {
            std::thread::spawn(move || {
                let mut c = Agent::connect(port);
                c.hc.join("ZZZZZZ", format!("brute{i}"), None);
                let code = match c
                    .wait_event("a join result", |e| matches!(e, ServerEvent::Error { .. }))
                {
                    ServerEvent::Error { code, .. } => code,
                    _ => unreachable!(),
                };
                c.wait_closed("the throttled/rejected attempt");
                code
            })
        })
        .collect();
    let codes: Vec<ErrorCode> = handles
        .into_iter()
        .map(|h| h.join().expect("attempt thread"))
        .collect();
    for c in &codes {
        assert!(
            matches!(c, ErrorCode::RateLimited | ErrorCode::RoomNotFound),
            "each attempt is throttled or room-not-found, got {c:?}"
        );
    }
    let rate_limited = codes
        .iter()
        .filter(|c| **c == ErrorCode::RateLimited)
        .count();
    let not_found = codes
        .iter()
        .filter(|c| **c == ErrorCode::RoomNotFound)
        .count();
    assert!(
        rate_limited > 0,
        "the concurrent burst is throttled over the wire (rate_limited={rate_limited}, room_not_found={not_found})"
    );
    assert!(
        not_found > 0,
        "the initial burst passed the per-IP bucket (got RoomNotFound)"
    );
}
