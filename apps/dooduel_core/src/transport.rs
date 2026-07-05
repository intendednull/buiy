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
