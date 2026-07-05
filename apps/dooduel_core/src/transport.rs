//! The transport seam (spec §2.4) — the boundary that keeps the [`crate::session::Session`]
//! authority transport-agnostic.
//!
//! The organizing idea (spec §2): the authority does **no I/O**. A transport moves
//! bytes; the `Session` consumes [`ClientIntent`]s tagged with a [`ConnId`] and emits
//! [`ServerEvent`]s addressed per recipient. A dedicated WebSocket server, an
//! in-process solo run, and (eventually, M6) a peer-hosted P2P session all drive the
//! *same* `Session` behind *different* implementations of these two traits.
//!
//! Both `try_recv` methods are **non-blocking** (spec §2.4): `ewebsock` (the wave-4
//! client transport) is a poll-style receiver, and the client integration is a Bevy
//! system draining `try_recv` each frame — not a blocking read.
//!
//! [`InProcessTransport`] is the channel-backed pair the solo path and the headless
//! tests use: [`InProcessTransport::new_pair`] wires one [`InProcServer`] to `N`
//! [`InProcClient`]s. It is single-threaded by construction (`Rc`/`RefCell`) — the
//! solo GUI and every M1 test run it on one thread; the wire transports (wave 4) are
//! the multi-threaded implementations.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::protocol::{ClientIntent, ServerEvent};

/// A connection identity — the transport assigns one per client connection. It is
/// **not** a seat: a connection acquires a seat only once its `Create`/`Join` is
/// admitted by the `Session`, and a reconnecting client arrives as a *fresh*
/// `ConnId` re-attaching to its held seat (spec §6.3). The connection↔seat mapping
/// lives in the transport-pump glue (the room actor / the solo plugin / the test
/// harness), never in the `Session`, which speaks only in seats.
pub type ConnId = u64;

/// The client-visible connection state, for the reconnect UX (spec §2.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnStatus {
    /// The socket is still being established.
    Connecting,
    /// The socket is open and carrying frames.
    Open,
    /// The socket closed (dropped, or replaced by a live-token rejoin, spec §6.3).
    Closed,
}

/// The `Session`-side face of a transport (spec §2.4). Non-blocking intake, addressed
/// send, and a per-poll list of connections that dropped since the last call.
pub trait ServerTransport {
    /// The next `(connection, intent)` pair, or `None` if none are queued
    /// (non-blocking).
    fn try_recv(&mut self) -> Option<(ConnId, ClientIntent)>;
    /// Send an event to one connection (the pump resolves `Recipient` → `ConnId`).
    fn send(&mut self, to: ConnId, ev: &ServerEvent);
    /// The connections that dropped since the last call (drained).
    fn disconnects(&mut self) -> Vec<ConnId>;
}

/// The client-side face of a transport (spec §2.4).
pub trait ClientTransport {
    /// Queue an intent for the server (non-blocking; the wire impl serializes it).
    fn send(&mut self, intent: &ClientIntent);
    /// The next server event, or `None` if none are queued (non-blocking).
    fn try_recv(&mut self) -> Option<ServerEvent>;
    /// The connection status, for the reconnect UX.
    fn status(&self) -> ConnStatus;
}

// ---------------------------------------------------------------------------
// InProcessTransport — the channel-backed pair (solo + tests).
// ---------------------------------------------------------------------------

/// The shared mailbox behind an [`InProcServer`] and its [`InProcClient`]s. Every
/// end holds an `Rc` to this one cell; sends push, `try_recv`s pop. Single-threaded
/// by construction (the solo path + tests never cross a thread boundary).
#[derive(Default)]
struct Mailbox {
    /// Client → server, tagged by origin, in global intake order (the server's single
    /// deterministic intake queue — intake order = mutation order, spec §6.1).
    to_server: VecDeque<(ConnId, ClientIntent)>,
    /// Server → client `i`, in send order.
    to_client: Vec<VecDeque<ServerEvent>>,
    /// Whether connection `i` is still open (a closed conn drops further traffic).
    open: Vec<bool>,
    /// Connections closed since the last [`ServerTransport::disconnects`] drain.
    dropped: VecDeque<ConnId>,
}

impl Mailbox {
    /// Grow the per-connection vectors so index `conn` is addressable.
    fn ensure(&mut self, conn: ConnId) {
        let need = conn as usize + 1;
        if self.to_client.len() < need {
            self.to_client.resize_with(need, VecDeque::new);
            self.open.resize(need, false);
        }
    }
}

/// The server end of an [`InProcessTransport`] pair (implements [`ServerTransport`]).
pub struct InProcServer {
    mailbox: Rc<RefCell<Mailbox>>,
    next_conn: ConnId,
}

/// A client end of an [`InProcessTransport`] pair (implements [`ClientTransport`]).
pub struct InProcClient {
    mailbox: Rc<RefCell<Mailbox>>,
    conn: ConnId,
}

/// Constructors for the in-process transport pair.
pub struct InProcessTransport;

impl InProcessTransport {
    /// Wire one server end to `n_clients` client ends, with connection ids `0..n`.
    /// Additional clients (e.g. a reconnecting one arriving as a fresh connection)
    /// are minted later with [`InProcServer::accept`].
    pub fn new_pair(n_clients: usize) -> (InProcServer, Vec<InProcClient>) {
        let mailbox = Rc::new(RefCell::new(Mailbox::default()));
        let mut clients = Vec::with_capacity(n_clients);
        {
            let mut mb = mailbox.borrow_mut();
            for conn in 0..n_clients as ConnId {
                mb.ensure(conn);
                mb.open[conn as usize] = true;
            }
        }
        for conn in 0..n_clients as ConnId {
            clients.push(InProcClient {
                mailbox: Rc::clone(&mailbox),
                conn,
            });
        }
        let server = InProcServer {
            mailbox,
            next_conn: n_clients as ConnId,
        };
        (server, clients)
    }
}

impl InProcServer {
    /// Mint a fresh connection (a new client "socket"): models a reconnecting client
    /// or a late joiner arriving after the initial pair (spec §6.3).
    pub fn accept(&mut self) -> InProcClient {
        let conn = self.next_conn;
        self.next_conn += 1;
        {
            let mut mb = self.mailbox.borrow_mut();
            mb.ensure(conn);
            mb.open[conn as usize] = true;
        }
        InProcClient {
            mailbox: Rc::clone(&self.mailbox),
            conn,
        }
    }

    /// Close a connection from the server side (the live-token-replacement path,
    /// spec §6.3): further traffic to/from it is dropped and the client observes
    /// [`ConnStatus::Closed`].
    pub fn close(&mut self, conn: ConnId) {
        let mut mb = self.mailbox.borrow_mut();
        if (conn as usize) < mb.open.len() && mb.open[conn as usize] {
            mb.open[conn as usize] = false;
            mb.dropped.push_back(conn);
        }
    }
}

impl ServerTransport for InProcServer {
    fn try_recv(&mut self) -> Option<(ConnId, ClientIntent)> {
        self.mailbox.borrow_mut().to_server.pop_front()
    }

    fn send(&mut self, to: ConnId, ev: &ServerEvent) {
        let mut mb = self.mailbox.borrow_mut();
        mb.ensure(to);
        if mb.open[to as usize] {
            mb.to_client[to as usize].push_back(ev.clone());
        }
    }

    fn disconnects(&mut self) -> Vec<ConnId> {
        self.mailbox.borrow_mut().dropped.drain(..).collect()
    }
}

impl InProcClient {
    /// This client's connection id (the seat it maps to is the pump's business).
    pub fn conn(&self) -> ConnId {
        self.conn
    }

    /// Close this client's connection from the client side (a graceful drop / a
    /// simulated tab close): the server observes it via [`ServerTransport::disconnects`].
    pub fn drop_conn(&mut self) {
        let mut mb = self.mailbox.borrow_mut();
        if mb.open[self.conn as usize] {
            mb.open[self.conn as usize] = false;
            mb.dropped.push_back(self.conn);
        }
    }
}

impl ClientTransport for InProcClient {
    fn send(&mut self, intent: &ClientIntent) {
        let mut mb = self.mailbox.borrow_mut();
        if mb.open[self.conn as usize] {
            mb.to_server.push_back((self.conn, intent.clone()));
        }
    }

    fn try_recv(&mut self) -> Option<ServerEvent> {
        self.mailbox.borrow_mut().to_client[self.conn as usize].pop_front()
    }

    fn status(&self) -> ConnStatus {
        if self.mailbox.borrow().open[self.conn as usize] {
            ConnStatus::Open
        } else {
            ConnStatus::Closed
        }
    }
}

// ---------------------------------------------------------------------------
// WsClientTransport — the ewebsock-backed WebSocket client (spec §2.1, W4.2).
// ---------------------------------------------------------------------------

/// The production client transport (spec §2.1, amended W4.2): a WebSocket to
/// `dooduel_server`, native + wasm behind one poll-style API (`ewebsock` —
/// `tungstenite` on a background thread on native, `web-sys` `WebSocket` on wasm).
/// It serializes each [`ClientIntent`] to one `serde_json` TEXT frame and decodes
/// inbound TEXT frames to [`ServerEvent`]s (spec §3.1).
///
/// Two wire-spec rules shape it:
/// - **No inbound frame cap** (W2-review R2): the 64 KiB
///   [`crate::protocol::MAX_FRAME_BYTES`] cap is SERVER-INBOUND only — a long turn's
///   `CanvasLog`/`RoomState` can be MB-scale, so the client accepts frames of any
///   size (`max_incoming_frame_size: usize::MAX`), never mirroring the cap.
/// - **Never panic on wire input** (spec §6.1): a frame that fails to decode is
///   logged and skipped ([`decode_event`]); `try_recv` yields the next decodable
///   event rather than stalling or panicking.
///
/// It is `Send` (the ewebsock ends are), but the client integration keeps it in the
/// [`ClientTransport`]-boxed `NonSend` slot alongside the in-process transport, so one
/// type spans solo + networked play.
#[cfg(feature = "ws-client")]
pub struct WsClientTransport {
    sender: ewebsock::WsSender,
    receiver: ewebsock::WsReceiver,
    status: ConnStatus,
}

#[cfg(feature = "ws-client")]
impl WsClientTransport {
    /// Open a WebSocket to `url` (e.g. `ws://127.0.0.1:7878`). The socket is
    /// established asynchronously, so [`status`](ClientTransport::status) is
    /// [`ConnStatus::Connecting`] until the first `Opened` event is drained by
    /// `try_recv`. `Err` only on an immediate local failure (native: thread spawn;
    /// web: the `WebSocket` API) — a *connection* failure arrives later as a drained
    /// error event that flips the status to [`ConnStatus::Closed`].
    pub fn connect(url: impl Into<String>) -> Result<Self, String> {
        let options = ewebsock::Options {
            // R2: the client never mirrors the server-inbound frame cap.
            max_incoming_frame_size: usize::MAX,
            ..Default::default()
        };
        let (sender, receiver) = ewebsock::connect(url, options)?;
        Ok(Self {
            sender,
            receiver,
            status: ConnStatus::Connecting,
        })
    }
}

/// Serialize one [`ClientIntent`] to its wire TEXT frame (spec §3.1). Serialization
/// of a plain data enum is infallible in practice; the defensive `unwrap_or_default`
/// degrades an impossible failure to an empty frame the server rejects rather than a
/// client panic (no `unwrap` on the wire path).
#[cfg(feature = "ws-client")]
fn encode_intent(intent: &ClientIntent) -> String {
    serde_json::to_string(intent).unwrap_or_default()
}

/// Decode one inbound TEXT frame to a [`ServerEvent`], logging + dropping a malformed
/// frame (spec §6.1 — never panic on wire input). `None` = undecodable.
#[cfg(feature = "ws-client")]
fn decode_event(text: &str) -> Option<ServerEvent> {
    match serde_json::from_str::<ServerEvent>(text) {
        Ok(ev) => Some(ev),
        Err(e) => {
            // The pure core carries no logging facade (it stays dep-light); stderr is
            // the honest sink on native and a no-op on wasm. The frame is dropped.
            eprintln!("dooduel_core: dropping undecodable server frame: {e}");
            None
        }
    }
}

#[cfg(feature = "ws-client")]
impl ClientTransport for WsClientTransport {
    fn send(&mut self, intent: &ClientIntent) {
        self.sender
            .send(ewebsock::WsMessage::Text(encode_intent(intent)));
    }

    fn try_recv(&mut self) -> Option<ServerEvent> {
        // Drain the non-message events (status transitions) inline so a single call
        // yields the next decodable ServerEvent — the pump never stalls on an
        // Opened/Closed/error, and a malformed TEXT frame is skipped, not returned.
        while let Some(event) = self.receiver.try_recv() {
            match event {
                ewebsock::WsEvent::Opened => self.status = ConnStatus::Open,
                ewebsock::WsEvent::Message(ewebsock::WsMessage::Text(text)) => {
                    if let Some(ev) = decode_event(&text) {
                        return Some(ev);
                    }
                    // Malformed — logged + skipped; keep draining.
                }
                // Binary/Unknown/Ping/Pong carry no protocol payload (the wire is TEXT
                // JSON) — ignore and keep draining.
                ewebsock::WsEvent::Message(_) => {}
                ewebsock::WsEvent::Error(_) | ewebsock::WsEvent::Closed => {
                    self.status = ConnStatus::Closed;
                }
            }
        }
        None
    }

    fn status(&self) -> ConnStatus {
        self.status
    }
}

#[cfg(all(test, feature = "ws-client"))]
mod ws_client_tests {
    //! Framing round-trips (spec §3.1) — no live socket at this tier (W4.2). The
    //! transport's send path is `encode_intent`; its recv path is `decode_event`. We
    //! prove: an intent survives encode → the server's decode, a server event survives
    //! its encode → the client's `decode_event`, and a malformed frame is dropped (not
    //! a panic).
    use super::*;
    use crate::protocol::{
        CanvasOp, ErrorCode, PROTOCOL_VERSION, ReplicaPlayer, RoomReplica, ServerEvent, WireAvatar,
    };
    use std::time::Duration;

    #[test]
    fn intent_round_trips_through_the_send_framing() {
        // Every intent the client sends must survive encode_intent (client send) →
        // serde_json decode (what the server's wire layer does).
        let intents = vec![
            ClientIntent::Create {
                name: "Ada".to_string(),
                avatar: WireAvatar::Default,
                protocol_version: PROTOCOL_VERSION,
            },
            ClientIntent::Join {
                room: "ABC123".to_string(),
                name: "Bo".to_string(),
                avatar: WireAvatar::Preset { icon: 2, tint: 1 },
                protocol_version: PROTOCOL_VERSION,
                reconnect: Some("tok-deadbeef".to_string()),
            },
            ClientIntent::StartMatch,
            ClientIntent::Pick { index: 1 },
            ClientIntent::Guess {
                text: "robot".to_string(),
            },
            ClientIntent::Stroke {
                stroke_id: 7,
                points: vec![(1, 2), (3, 4)],
                color: [10, 20, 30, 255],
                radius: 4,
                done: false,
            },
            ClientIntent::Fill {
                seed: (5, 6),
                color: [0, 128, 255, 255],
            },
            ClientIntent::Undo,
            ClientIntent::Clear,
            ClientIntent::Continue,
            ClientIntent::Leave,
        ];
        for intent in &intents {
            let frame = encode_intent(intent);
            let back: ClientIntent =
                serde_json::from_str(&frame).expect("the server decodes the client's frame");
            assert_eq!(&back, intent, "intent framing round-trips via {frame}");
        }
    }

    #[test]
    fn server_event_round_trips_through_decode_event() {
        // A representative populated event (incl. the large RoomState seed) must
        // survive the server's encode → the client's decode_event.
        let events = vec![
            ServerEvent::Welcome {
                seat: 1,
                room_code: "ABC123".to_string(),
                reconnect_token: "cafef00d".to_string(),
                protocol_version: PROTOCOL_VERSION,
            },
            ServerEvent::CanvasOpApplied {
                op: CanvasOp::Stroke {
                    id: 3,
                    points: vec![(7, 8)],
                    color: [1, 2, 3, 255],
                    radius: 2,
                },
            },
            ServerEvent::CountdownSync {
                remaining: Duration::from_secs(42),
            },
            ServerEvent::Error {
                code: ErrorCode::VersionMismatch,
                message: "bad version".to_string(),
            },
            ServerEvent::RoomState(RoomReplica {
                room_code: "ABC123".to_string(),
                my_seat: 1,
                players: vec![ReplicaPlayer {
                    name: "Ada".to_string(),
                    avatar: WireAvatar::Default,
                    connected: true,
                    is_bot: false,
                    score: 0,
                    guessed: false,
                }],
                ..Default::default()
            }),
        ];
        for ev in &events {
            let frame = serde_json::to_string(ev).expect("the server serializes the event");
            let back = decode_event(&frame).expect("the client decodes the server's frame");
            assert_eq!(&back, ev, "event framing round-trips via {frame}");
        }
    }

    #[test]
    fn a_malformed_frame_is_dropped_not_a_panic() {
        // Non-JSON, valid-JSON-wrong-shape, and empty (the encode_intent failure
        // degrade) all decode to None — the spec §6.1 never-panic guarantee.
        assert_eq!(decode_event("this is not json"), None);
        assert_eq!(decode_event("{\"NoSuchVariant\":{}}"), None);
        assert_eq!(decode_event(""), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ErrorCode, PROTOCOL_VERSION, WireAvatar};

    fn create(name: &str) -> ClientIntent {
        ClientIntent::Create {
            name: name.to_string(),
            avatar: WireAvatar::Default,
            protocol_version: PROTOCOL_VERSION,
        }
    }

    #[test]
    fn client_intent_reaches_the_server_tagged_by_conn() {
        let (mut server, mut clients) = InProcessTransport::new_pair(2);
        clients[0].send(&create("Ada"));
        clients[1].send(&ClientIntent::StartMatch);
        // Intake is a single ordered queue (spec §6.1: intake order = mutation order).
        assert_eq!(server.try_recv(), Some((0, create("Ada"))));
        assert_eq!(server.try_recv(), Some((1, ClientIntent::StartMatch)));
        assert_eq!(server.try_recv(), None);
    }

    #[test]
    fn server_send_is_addressed_and_isolated_per_recipient() {
        let (mut server, mut clients) = InProcessTransport::new_pair(2);
        let ev = ServerEvent::CanvasCleared;
        server.send(0, &ev);
        // Only conn 0 sees it — the per-recipient isolation the redaction rides on.
        assert_eq!(clients[0].try_recv(), Some(ServerEvent::CanvasCleared));
        assert_eq!(clients[0].try_recv(), None);
        assert_eq!(clients[1].try_recv(), None);
    }

    #[test]
    fn distinct_recipients_get_distinct_payloads() {
        // The property the per-recipient word redaction depends on: two recipients
        // can be handed *different* bytes for the "same" logical update.
        let (mut server, mut clients) = InProcessTransport::new_pair(2);
        server.send(
            0,
            &ServerEvent::WordUpdate {
                display: "R O B O T".to_string(),
                len: 5,
                hints_revealed: 5,
            },
        );
        server.send(
            1,
            &ServerEvent::WordUpdate {
                display: "_ _ _ _ _".to_string(),
                len: 5,
                hints_revealed: 0,
            },
        );
        let seen0 = clients[0].try_recv().unwrap();
        let seen1 = clients[1].try_recv().unwrap();
        assert_ne!(seen0, seen1);
    }

    #[test]
    fn a_dropped_client_is_reported_once_to_the_server() {
        let (mut server, mut clients) = InProcessTransport::new_pair(2);
        clients[1].drop_conn();
        assert_eq!(server.disconnects(), vec![1]);
        // Drained: a second poll reports nothing.
        assert_eq!(server.disconnects(), Vec::<ConnId>::new());
        // A send to the dropped conn is silently dropped, not delivered.
        server.send(1, &ServerEvent::CanvasCleared);
        // (The client is gone; nothing to assert on its inbox beyond no panic.)
        assert_eq!(clients[1].status(), ConnStatus::Closed);
        assert_eq!(clients[0].status(), ConnStatus::Open);
    }

    #[test]
    fn accept_mints_a_fresh_connection_for_reconnect() {
        let (mut server, mut clients) = InProcessTransport::new_pair(1);
        let mut rejoin = server.accept();
        assert_eq!(rejoin.conn(), 1);
        rejoin.send(&ClientIntent::Join {
            room: "ABC123".to_string(),
            name: "Ada".to_string(),
            avatar: WireAvatar::Default,
            protocol_version: PROTOCOL_VERSION,
            reconnect: Some("tok".to_string()),
        });
        // The fresh conn's frame reaches the server tagged with the new id.
        let (conn, _) = server.try_recv().unwrap();
        assert_eq!(conn, 1);
        let _ = &mut clients; // the original client end stays valid alongside the rejoin
    }

    #[test]
    fn server_close_replaces_a_live_connection() {
        // The live-token-replacement path (spec §6.3): the server closes the old
        // connection; the old client observes Closed and the server sees the drop.
        let (mut server, clients) = InProcessTransport::new_pair(1);
        server.close(0);
        assert_eq!(clients[0].status(), ConnStatus::Closed);
        assert_eq!(server.disconnects(), vec![0]);
        // An Error to a closed conn is dropped (no panic).
        server.send(
            0,
            &ServerEvent::Error {
                code: ErrorCode::BadToken,
                message: "gone".to_string(),
            },
        );
    }
}
