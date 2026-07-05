//! The per-connection WebSocket wire tasks (spec §3.1, §6). One `handle_conn` per
//! accepted socket: WS handshake → read + version-gate the first frame → per-IP limit →
//! route to a room → run the read/write I/O loop. All wire input is decode-or-skip;
//! there is **no `unwrap` on any wire path** and a malformed frame never panics.

use std::sync::Arc;
use std::time::Instant;

use async_channel::{Receiver, RecvError, Sender};
use async_net::TcpStream;
use async_tungstenite::WebSocketStream;
use async_tungstenite::tungstenite::Message;
use async_tungstenite::tungstenite::error::Error as WsError;
use async_tungstenite::tungstenite::protocol::WebSocketConfig;
use futures_lite::future;
use futures_lite::stream::StreamExt as _;
use std::net::SocketAddr;

use dooduel_core::game::Config;
use dooduel_core::protocol::{
    ClientIntent, ErrorCode, MAX_FRAME_BYTES, PROTOCOL_VERSION, ServerEvent, WireAvatar,
};
use dooduel_core::transport::ConnId;

use crate::registry::Registry;
use crate::room::RoomMsg;
use crate::util::TokenBucket;

/// The per-connection intent rate cap (spec §3.1). NOTE: set well above the honest
/// client's ceiling. The frozen GUI emits up to **one stroke batch per rendered frame**
/// during continuous drawing (display-refresh-bound — up to ~144/s on a 144 Hz panel),
/// so the spec's illustrative "~30/s" would disconnect a legitimate drawer. These
/// clear that ceiling with margin while still bounding a true flood to ~240 intents/s.
/// (Deviation from the spec's "~30/s" figure — flagged for review; the guard's INTENT,
/// bounding abuse, holds. The stroke path is additionally bounded by MAX_STROKE_POINTS
/// per batch and the op-log's MAX_OP_POINTS auto-split.)
const INTENT_BURST: f64 = 480.0;
const INTENT_REFILL_PER_SEC: f64 = 240.0;

/// The pure classification of a connection's first frame (spec §3.1): the production
/// version gate + the Create/Join split, factored out so it is unit-testable without a
/// socket (W4.4). A non-Create/Join first frame, or a version mismatch, is a `Reject`.
#[derive(Debug, PartialEq)]
enum FirstAction {
    Create {
        name: String,
        avatar: WireAvatar,
    },
    Join {
        room: String,
        name: String,
        avatar: WireAvatar,
        reconnect: Option<String>,
    },
    Reject(ErrorCode),
}

/// Classify the first frame — the version gate runs here, **before** any registry /
/// `Session` touch (W2-review). A `Create`/`Join` with the wrong `protocol_version` is
/// rejected `VersionMismatch`; any other first frame is `Malformed`.
fn classify_first_frame(intent: ClientIntent) -> FirstAction {
    match intent {
        ClientIntent::Create {
            name,
            avatar,
            protocol_version,
        } => {
            if protocol_version != PROTOCOL_VERSION {
                FirstAction::Reject(ErrorCode::VersionMismatch)
            } else {
                FirstAction::Create { name, avatar }
            }
        }
        ClientIntent::Join {
            room,
            name,
            avatar,
            protocol_version,
            reconnect,
        } => {
            if protocol_version != PROTOCOL_VERSION {
                FirstAction::Reject(ErrorCode::VersionMismatch)
            } else {
                FirstAction::Join {
                    room,
                    name,
                    avatar,
                    reconnect,
                }
            }
        }
        _ => FirstAction::Reject(ErrorCode::Malformed),
    }
}

/// Serialize a `ServerEvent` to its wire frame (spec §3.1). Infallible for a plain data
/// enum; the defensive `unwrap_or_default` degrades an impossible failure to an empty
/// frame rather than a server panic.
fn encode(ev: &ServerEvent) -> String {
    serde_json::to_string(ev).unwrap_or_default()
}

fn err_frame(code: ErrorCode, message: &str) -> String {
    encode(&ServerEvent::Error {
        code,
        message: message.to_string(),
    })
}

/// Handle one accepted connection (spec §6): WS handshake, first-frame version gate,
/// per-IP limit, room routing, then the I/O loop. Any early exit drops the socket.
pub async fn handle_conn(
    stream: TcpStream,
    peer: SocketAddr,
    registry: Arc<Registry>,
    conn: ConnId,
) {
    // Cap the SERVER-INBOUND frame + message size at 64 KiB (spec §3.1 — the DoS guard;
    // this is inbound only, so a MB-scale outbound CanvasLog is unaffected — tungstenite
    // checks max_message_size on reassembly, never on send, and fragments outbound at
    // max_frame_size which the client reassembles unbounded, W2-review R2).
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));
    let mut ws = match async_tungstenite::accept_async_with_config(stream, Some(config)).await {
        Ok(ws) => ws,
        Err(_) => return, // handshake failed — nothing to say
    };

    // The first frame must be Create/Join; the version gate runs BEFORE the registry.
    let Some(first) = read_intent(&mut ws).await else {
        return; // closed / undecodable before a first frame
    };
    let (name, avatar, reconnect, room_code) = match classify_first_frame(first) {
        FirstAction::Reject(code) => {
            reject(&mut ws, code).await;
            return;
        }
        FirstAction::Create { name, avatar } => (name, avatar, None, None),
        FirstAction::Join {
            room,
            name,
            avatar,
            reconnect,
        } => (name, avatar, reconnect, Some(room)),
    };

    // Per-IP limit (spec §6.2 — the brute-force guard), after the version gate.
    if !registry.check_ip(peer.ip()) {
        reject(&mut ws, ErrorCode::RateLimited).await;
        return;
    }

    // Route: Create mints a room; Join looks one up (unknown ⇒ RoomNotFound).
    let room_tx = match room_code {
        None => registry.create_room(Config::default()).1,
        Some(code) => match registry.lookup(&code) {
            Some(tx) => tx,
            None => {
                reject(&mut ws, ErrorCode::RoomNotFound).await;
                return;
            }
        },
    };

    // Hand the room this connection + its outbox, then run the I/O loop.
    let (obx_tx, obx_rx) = async_channel::unbounded::<ServerEvent>();
    let joined = room_tx
        .send(RoomMsg::Join {
            conn,
            name,
            avatar,
            reconnect,
            outbox: obx_tx,
        })
        .await;
    if joined.is_err() {
        // The room GC'd between lookup and send — treat as gone.
        reject(&mut ws, ErrorCode::RoomNotFound).await;
        return;
    }
    run_io(ws, obx_rx, room_tx, conn).await;
}

/// Read frames until the next decodable [`ClientIntent`] (skipping control frames);
/// `None` on close / an undecodable text frame (the caller rejects/closes).
async fn read_intent(ws: &mut WebSocketStream<TcpStream>) -> Option<ClientIntent> {
    loop {
        match ws.next().await {
            Some(Ok(Message::Text(text))) => {
                return serde_json::from_str::<ClientIntent>(text.as_str()).ok();
            }
            Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => continue,
            _ => return None,
        }
    }
}

/// Send one `Error` frame, then close the socket (spec §3.2 — a rejected connection is
/// told why, then FIN'd).
async fn reject(ws: &mut WebSocketStream<TcpStream>, code: ErrorCode) {
    let text = reject_text(&code);
    let _ = ws.send(Message::text(err_frame(code, text))).await;
    let _ = ws.close(None).await;
}

fn reject_text(code: &ErrorCode) -> &'static str {
    match code {
        ErrorCode::VersionMismatch => "unsupported protocol version",
        ErrorCode::RoomNotFound => "no such room",
        ErrorCode::RateLimited => "too many connection attempts",
        ErrorCode::Malformed => "first frame must be Create or Join",
        _ => "connection rejected",
    }
}

/// What the I/O loop woke on: an inbound WS frame, or an outbound event to write.
// A transient per-iteration stack local (never heap-collected), so the momentary size
// gap between the variants doesn't matter — boxing would just add an alloc per loop turn.
#[allow(clippy::large_enum_variant)]
enum IoEv {
    In(Option<Result<Message, WsError>>),
    Out(Result<ServerEvent, RecvError>),
}

/// The post-join I/O loop (spec §6.1): race an inbound frame against an outbound event.
/// Inbound intents pass the per-connection rate cap then forward to the room; outbound
/// events serialize to TEXT frames. Any close/error, an exhausted rate cap, or the room
/// dropping our outbox ends the loop; on exit we tell the room we're gone and FIN.
async fn run_io(
    ws: WebSocketStream<TcpStream>,
    obx_rx: Receiver<ServerEvent>,
    room_tx: Sender<RoomMsg>,
    conn: ConnId,
) {
    let (mut tx, mut rx) = ws.split();
    let start = Instant::now();
    let mut rate = TokenBucket::new(INTENT_BURST, INTENT_REFILL_PER_SEC);

    loop {
        // `or` polls the inbound future first (read priority). Neither future is lost
        // when the other wins — the loser was Pending, dropped cleanly.
        let ev = future::or(async { IoEv::In(rx.next().await) }, async {
            IoEv::Out(obx_rx.recv().await)
        })
        .await;

        match ev {
            IoEv::In(Some(Ok(Message::Text(text)))) => {
                if !rate.try_take(start.elapsed()) {
                    let _ = tx
                        .send(Message::text(err_frame(
                            ErrorCode::RateLimited,
                            "intent rate limit exceeded",
                        )))
                        .await;
                    break;
                }
                // A malformed frame decodes to Err — skipped, never panicked (spec §6.1).
                if let Ok(intent) = serde_json::from_str::<ClientIntent>(text.as_str())
                    && room_tx
                        .send(RoomMsg::Intent { conn, intent })
                        .await
                        .is_err()
                {
                    break; // the room is gone
                }
            }
            IoEv::In(Some(Ok(Message::Close(_)))) => break,
            IoEv::In(Some(Ok(_))) => {} // ping/pong/binary/frame — ignore (TEXT protocol)
            IoEv::In(Some(Err(_))) | IoEv::In(None) => break, // socket errored / closed
            IoEv::Out(Ok(ev)) => {
                if tx.send(Message::text(encode(&ev))).await.is_err() {
                    break;
                }
            }
            IoEv::Out(Err(_)) => break, // outbox closed = the room decided to close us
        }
    }

    // Tell the room we're gone (idempotent — it may already have dropped us), then FIN.
    let _ = room_tx.send(RoomMsg::Disconnect { conn }).await;
    let _ = tx.close(None).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(version: u32) -> ClientIntent {
        ClientIntent::Create {
            name: "Ada".to_string(),
            avatar: WireAvatar::Default,
            protocol_version: version,
        }
    }

    #[test]
    fn version_gate_rejects_a_mismatched_first_frame() {
        // The production gate (W2-review): a bad protocol_version on the FIRST frame is
        // VersionMismatch — resolved here, before any registry/Session touch.
        assert_eq!(
            classify_first_frame(create(PROTOCOL_VERSION + 1)),
            FirstAction::Reject(ErrorCode::VersionMismatch),
        );
        assert_eq!(
            classify_first_frame(ClientIntent::Join {
                room: "ABC123".to_string(),
                name: "Bo".to_string(),
                avatar: WireAvatar::Default,
                protocol_version: 999,
                reconnect: None,
            }),
            FirstAction::Reject(ErrorCode::VersionMismatch),
        );
    }

    #[test]
    fn version_gate_admits_a_matching_first_frame() {
        assert_eq!(
            classify_first_frame(create(PROTOCOL_VERSION)),
            FirstAction::Create {
                name: "Ada".to_string(),
                avatar: WireAvatar::Default,
            },
        );
    }

    #[test]
    fn a_non_create_join_first_frame_is_malformed() {
        // A gameplay intent as the FIRST frame never reaches a room.
        assert_eq!(
            classify_first_frame(ClientIntent::StartMatch),
            FirstAction::Reject(ErrorCode::Malformed),
        );
        assert_eq!(
            classify_first_frame(ClientIntent::Guess {
                text: "robot".into()
            }),
            FirstAction::Reject(ErrorCode::Malformed),
        );
    }
}
