//! The Dooduel wire protocol (spec §3, rev-2.1) — the exact bytes that travel
//! between a client and the authoritative `Session`.
//!
//! Every type here carries `serde::{Serialize, Deserialize}` (one protocol
//! message per WebSocket TEXT frame, `serde_json`) plus `bevy_reflect::Reflect`
//! (and `Clone + Debug + PartialEq`), so a networked `Msg::Net(ServerEvent)` folds
//! through the MVU record/replay funnel like any other message (spec §3.4).
//!
//! Two design facts shape the surface:
//!
//! - **Canvas = op log, not pixels** (spec §2.2/§3.5). A [`CanvasOp`] carries the
//!   *effective* color + radius only — the eraser's [`crate::canvas::PAPER`] color
//!   and its `×1.6` radius ([`crate::canvas::eraser_radius`]) are pre-applied before
//!   the wire, so a replica needs no tool-specific knowledge: color + radius fully
//!   determine the stamp. No tool enum, no erase flag travels.
//! - **Redaction is the server's job** (spec §5). The word only ever reaches a seat
//!   that `crate::game::Game::knows` it; the secret has no field to land in on a
//!   guesser's replica (the [`RoomReplica`] negative invariant, spec §4.1).

use std::time::Duration;

use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::game::{ChatMsg, Phase, TurnResult};

/// The protocol version carried in the first client frame ([`ClientIntent::Create`]
/// / [`ClientIntent::Join`]); a mismatch is rejected with
/// [`ErrorCode::VersionMismatch`] before any `Welcome` (spec §3.1).
pub const PROTOCOL_VERSION: u32 = 1;

// --- Limits (spec §3.1 rev-2.1) — also the DoS guard -----------------------

/// The maximum size of a single WebSocket frame the server accepts.
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
/// The maximum points batched into one [`ClientIntent::Stroke`] frame.
pub const MAX_STROKE_POINTS: usize = 256;
/// The maximum length (chars) of a guess.
pub const MAX_GUESS_LEN: usize = 128;
/// The maximum length (chars) of a player name.
pub const MAX_NAME_LEN: usize = 32;
/// The room-invite code length (`[A-Z0-9]`, server-generated).
pub const ROOM_CODE_LEN: usize = 6;
/// The maximum size of a custom-avatar PNG in **raw** bytes. It travels
/// base64-encoded on the wire (~43 KiB encoded), so even a worst-case
/// max-avatar [`ClientIntent::Join`] frame fits [`MAX_FRAME_BYTES`] — the
/// cap-consistency the boundary test proves (rev-2.1: the rev-2 "64 KiB base64"
/// limit was internally inconsistent with the frame cap).
pub const MAX_AVATAR_PNG: usize = 32 * 1024;

/// A player's avatar as it crosses the wire. The custom image is a one-shot,
/// capped, base64-encoded field (spec §3.1) — the only image the protocol carries
/// (the canvas is an op log, never a raster). `Default` = the name-hashed doodle;
/// `Preset` = an explicit gallery icon + tint; `Custom` = the drawn PNG.
#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum WireAvatar {
    Default,
    Preset {
        icon: usize,
        tint: usize,
    },
    /// The drawn avatar as base64 (**not** `Vec<u8>` — a `serde_json` byte array
    /// is a `~3.6×`-larger number list). Raw PNG ≤ [`MAX_AVATAR_PNG`].
    Custom {
        png_base64: String,
    },
}

/// A single authoritative canvas operation (spec §2.2). The op carries the
/// **effective** color + radius — the eraser is already resolved to
/// [`crate::canvas::PAPER`] + [`crate::canvas::eraser_radius`], so replaying an op
/// log through a [`crate::canvas::PaintBuffer`] reproduces byte-identical pixels on
/// every replica (the integer-op determinism the sync stands on).
#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum CanvasOp {
    /// A stroke: the exact post-`to_pixel` sample sequence, stamped at `radius`.
    Stroke {
        id: u64,
        points: Vec<(i32, i32)>,
        color: [u8; 4],
        radius: i32,
    },
    /// A flood fill seeded at `seed`, filling with `color`.
    Fill {
        id: u64,
        seed: (i32, i32),
        color: [u8; 4],
    },
}

/// Client → server (spec §3.2). The server enforces the phase/seat gate table on
/// each intent; a rejected intent produces an [`ServerEvent::Error`], never a
/// partial mutation.
#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum ClientIntent {
    /// Open a fresh room; the server generates the code (creator = host).
    Create {
        name: String,
        avatar: WireAvatar,
        protocol_version: u32,
    },
    /// Join an existing room by code (unknown ⇒ [`ErrorCode::RoomNotFound`]); a
    /// valid `reconnect` token re-attaches to a held seat (spec §6.3).
    Join {
        room: String,
        name: String,
        avatar: WireAvatar,
        protocol_version: u32,
        reconnect: Option<String>,
    },
    /// Host-only, in the lobby.
    StartMatch,
    /// Drawer-only, in `Picking`: commit one of the offered word choices.
    Pick { index: usize },
    /// Non-drawer, not-yet-guessed, in `Drawing`.
    Guess { text: String },
    /// Drawer-only, in `Drawing`: a stroke batch under one client-chosen id;
    /// `done` marks the final batch (spec §3.5).
    Stroke {
        stroke_id: u64,
        points: Vec<(i32, i32)>,
        color: [u8; 4],
        radius: i32,
        done: bool,
    },
    /// Drawer-only, in `Drawing`: a flood fill.
    Fill { seed: (i32, i32), color: [u8; 4] },
    /// Drawer-only, in `Drawing`.
    Undo,
    /// Drawer-only, in `Drawing`.
    Clear,
    /// Any seat, in `Reveal`: advance to the next turn.
    Continue,
    /// Any seat, any phase: graceful seat release (skips grace).
    Leave,
}

/// Server → client (spec §3.3), addressed per recipient. The word only reaches a
/// seat that already knows it (`WordUpdate` upgrades on a correct guess;
/// `TurnEnded` legitimately broadcasts the reveal).
// `RoomState(RoomReplica)` is the largest variant by design (the full join/reconnect
// seed); the wire contract (plan W1.1 §3) pins it un-boxed, and this enum is only
// ever heap-owned (a `Msg::Net` payload / a `Vec` outbox entry), so the on-stack
// size difference the lint guards against does not apply.
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum ServerEvent {
    /// The join/reconnect ack: this seat, the room code, the (rotated) reconnect
    /// token, and the server's protocol version.
    Welcome {
        seat: usize,
        room_code: String,
        reconnect_token: String,
        protocol_version: u32,
    },
    /// The full per-recipient replica seed (sent on join/reconnect).
    RoomState(RoomReplica),
    /// The roster changed (carries `guessed` too — a deliberate superset of the
    /// spec §3.3 sketch, so a replica needn't retain a separate guessed set).
    Roster { players: Vec<ReplicaPlayer> },
    /// A phase transition + its clock (`remaining` re-anchored on receipt, §4.3).
    PhaseChanged {
        phase: Phase,
        drawer: Option<usize>,
        round: u32,
        total_rounds: u32,
        remaining: Duration,
    },
    /// A periodic countdown re-sync (re-anchor only).
    CountdownSync { remaining: Duration },
    /// The per-recipient word row (re-sent on a hint flip; upgraded to the full
    /// word for a seat that guesses correctly).
    WordUpdate {
        display: String,
        len: usize,
        hints_revealed: usize,
    },
    /// The drawer's three word choices (drawer only, in `Picking`).
    WordChoices { words: Vec<String> },
    /// One canvas op was applied (broadcast to all but the originating drawer).
    CanvasOpApplied { op: CanvasOp },
    /// The last op was removed (broadcast to all, including the drawer).
    CanvasUndo { removed_id: u64 },
    /// The canvas was cleared.
    CanvasCleared,
    /// The full current-turn op log (late join / reconnect resync).
    CanvasLog { ops: Vec<CanvasOp> },
    /// A chat line (shared broadcast; a private near-miss nudge is addressed only
    /// to its seat by the server).
    ChatLine { line: ChatMsg },
    /// A guess was resolved.
    GuessResult {
        seat: usize,
        correct: bool,
        points: i64,
    },
    /// The turn ended — the reveal rows + the word (legitimately broadcast here).
    TurnEnded {
        results: Vec<TurnResult>,
        word: String,
    },
    /// The match ended — the final podium (`(seat, name, score)`).
    MatchEnded { podium: Vec<(usize, String, i64)> },
    /// A rejected intent or a protocol error.
    Error { code: ErrorCode, message: String },
}

/// The machine-readable reason an intent was rejected (spec §3.2/§6). The `message`
/// on [`ServerEvent::Error`] is the human-readable companion.
#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum ErrorCode {
    VersionMismatch,
    RoomNotFound,
    RoomFull,
    NotHost,
    NotDrawer,
    WrongPhase,
    BadToken,
    RateLimited,
    TooLarge,
    Malformed,
}

/// The client-side mirror of the room (spec §4.1). It holds **only** what a
/// recipient is allowed to see: the secret word, the pre-pick choices of another
/// drawer, the RNG seed, the hint schedule, bot plans, and other seats' private
/// chat have **no field to land in** here (the negative invariant). The raster is
/// derived from `canvas_ops`; the countdown is re-anchored from `remaining`.
#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq, Default)]
pub struct RoomReplica {
    pub room_code: String,
    pub my_seat: usize,
    pub host: usize,
    pub players: Vec<ReplicaPlayer>,
    pub phase: Phase,
    pub drawer: Option<usize>,
    pub round: u32,
    pub total_rounds: u32,
    /// Seconds remaining in the current phase at send time; re-anchored to the
    /// client's monotonic receipt instant (§4.3).
    pub remaining: Duration,
    pub word_display: String,
    pub word_len: usize,
    pub hints_revealed: usize,
    pub word_choices: Vec<String>,
    pub chat: Vec<ChatMsg>,
    pub canvas_ops: Vec<CanvasOp>,
    pub turn_results: Vec<TurnResult>,
    pub podium: Option<Vec<(usize, String, i64)>>,
}

impl RoomReplica {
    /// The per-letter word row derived from [`Self::word_display`] — the view's
    /// data source (replaces the old `Game::word_slots`, which keyed off the
    /// hot-seat `viewing_as`). `word_display` is the server-computed, space-joined
    /// row (`crate::game::Game::word_display_for`): each token is either a single
    /// revealed uppercase letter or a `_` blank. Each slot is `(char, revealed)` —
    /// the letter (or `'_'`) and whether it is shown. Empty outside Drawing/Reveal
    /// (there is no word row then).
    pub fn word_slots(&self) -> Vec<(char, bool)> {
        self.word_display
            .split_whitespace()
            .filter_map(|tok| tok.chars().next())
            .map(|c| (c, c != '_'))
            .collect()
    }
}

/// A roster entry as a recipient sees it (spec §3.3). `guessed` is per-turn; the
/// avatar is the wire form ([`WireAvatar`]).
#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub struct ReplicaPlayer {
    pub name: String,
    pub avatar: WireAvatar,
    pub connected: bool,
    pub is_bot: bool,
    pub score: i64,
    pub guessed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{ChatKind, ChatMsg, TurnResult};
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;

    /// serde_json round-trip: a value must survive serialize → deserialize
    /// byte-for-value (the property every wire type must hold).
    fn roundtrip<T>(v: &T)
    where
        T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(v).expect("serialize");
        let back: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(*v, back, "round-trip mismatch via {json}");
    }

    /// A populated `ChatMsg` (a game type reused on the wire).
    fn sample_chat() -> ChatMsg {
        ChatMsg {
            seq: 7,
            kind: ChatKind::Correct,
            text: "🎉 Theo guessed the word!".to_string(),
            to: Some(2),
        }
    }

    fn sample_turn_results() -> Vec<TurnResult> {
        vec![
            TurnResult {
                player: 0,
                name: "Mara".to_string(),
                delta: 100,
                total: 340,
            },
            TurnResult {
                player: 1,
                name: "Priya".to_string(),
                delta: 410,
                total: 410,
            },
        ]
    }

    fn sample_ops() -> Vec<CanvasOp> {
        vec![
            CanvasOp::Stroke {
                id: 1,
                points: vec![(3, 4), (5, 6), (7, 8)],
                color: [20, 20, 24, 255],
                radius: 4,
            },
            CanvasOp::Fill {
                id: 2,
                seed: (10, 12),
                color: [0xf4, 0xc2, 0x0d, 255],
            },
        ]
    }

    #[test]
    fn wire_avatar_round_trips_every_variant() {
        for v in [
            WireAvatar::Default,
            WireAvatar::Preset { icon: 5, tint: 3 },
            WireAvatar::Custom {
                png_base64: "aGVsbG8=".to_string(),
            },
        ] {
            roundtrip(&v);
        }
    }

    #[test]
    fn canvas_op_round_trips_every_variant() {
        for op in sample_ops() {
            roundtrip(&op);
        }
    }

    #[test]
    fn client_intent_round_trips_every_variant() {
        let intents = vec![
            ClientIntent::Create {
                name: "Mara".to_string(),
                avatar: WireAvatar::Default,
                protocol_version: PROTOCOL_VERSION,
            },
            ClientIntent::Join {
                room: "ABC123".to_string(),
                name: "Zed".to_string(),
                avatar: WireAvatar::Preset { icon: 1, tint: 2 },
                protocol_version: PROTOCOL_VERSION,
                reconnect: Some("deadbeef".to_string()),
            },
            ClientIntent::StartMatch,
            ClientIntent::Pick { index: 2 },
            ClientIntent::Guess {
                text: "robot".to_string(),
            },
            ClientIntent::Stroke {
                stroke_id: 9,
                points: vec![(1, 1), (2, 2)],
                color: [255, 0, 0, 255],
                radius: 6,
                done: true,
            },
            ClientIntent::Fill {
                seed: (4, 5),
                color: [0, 128, 255, 255],
            },
            ClientIntent::Undo,
            ClientIntent::Clear,
            ClientIntent::Continue,
            ClientIntent::Leave,
        ];
        for i in &intents {
            roundtrip(i);
        }
    }

    #[test]
    fn server_event_round_trips_every_variant() {
        let events = vec![
            ServerEvent::Welcome {
                seat: 0,
                room_code: "ABC123".to_string(),
                reconnect_token: "cafef00ddeadbeef".to_string(),
                protocol_version: PROTOCOL_VERSION,
            },
            ServerEvent::RoomState(sample_replica()),
            ServerEvent::Roster {
                players: sample_replica().players,
            },
            ServerEvent::PhaseChanged {
                phase: Phase::Drawing,
                drawer: Some(1),
                round: 1,
                total_rounds: 2,
                remaining: Duration::from_secs(80),
            },
            ServerEvent::CountdownSync {
                remaining: Duration::from_millis(45_500),
            },
            ServerEvent::WordUpdate {
                display: "_ _ B _ _".to_string(),
                len: 5,
                hints_revealed: 1,
            },
            ServerEvent::WordChoices {
                words: vec![
                    "robot".to_string(),
                    "castle".to_string(),
                    "kite".to_string(),
                ],
            },
            ServerEvent::CanvasOpApplied {
                op: sample_ops()[0].clone(),
            },
            ServerEvent::CanvasUndo { removed_id: 9 },
            ServerEvent::CanvasCleared,
            ServerEvent::CanvasLog { ops: sample_ops() },
            ServerEvent::ChatLine {
                line: sample_chat(),
            },
            ServerEvent::GuessResult {
                seat: 2,
                correct: true,
                points: 410,
            },
            ServerEvent::TurnEnded {
                results: sample_turn_results(),
                word: "robot".to_string(),
            },
            ServerEvent::MatchEnded {
                podium: vec![(1, "Priya".to_string(), 1420), (0, "Mara".to_string(), 980)],
            },
            ServerEvent::Error {
                code: ErrorCode::RoomNotFound,
                message: "no such room".to_string(),
            },
        ];
        for e in &events {
            roundtrip(e);
        }
    }

    #[test]
    fn error_code_round_trips_every_variant() {
        for c in [
            ErrorCode::VersionMismatch,
            ErrorCode::RoomNotFound,
            ErrorCode::RoomFull,
            ErrorCode::NotHost,
            ErrorCode::NotDrawer,
            ErrorCode::WrongPhase,
            ErrorCode::BadToken,
            ErrorCode::RateLimited,
            ErrorCode::TooLarge,
            ErrorCode::Malformed,
        ] {
            roundtrip(&c);
        }
    }

    fn sample_replica() -> RoomReplica {
        RoomReplica {
            room_code: "ABC123".to_string(),
            my_seat: 2,
            host: 0,
            players: vec![
                ReplicaPlayer {
                    name: "Mara".to_string(),
                    avatar: WireAvatar::Default,
                    connected: true,
                    is_bot: false,
                    score: 980,
                    guessed: false,
                },
                ReplicaPlayer {
                    name: "Priya".to_string(),
                    avatar: WireAvatar::Preset { icon: 3, tint: 1 },
                    connected: true,
                    is_bot: true,
                    score: 1420,
                    guessed: true,
                },
            ],
            phase: Phase::Drawing,
            drawer: Some(1),
            round: 2,
            total_rounds: 2,
            remaining: Duration::from_secs(53),
            word_display: "_ _ B _ _".to_string(),
            word_len: 5,
            hints_revealed: 1,
            word_choices: vec![],
            chat: vec![sample_chat()],
            canvas_ops: sample_ops(),
            turn_results: sample_turn_results(),
            podium: None,
        }
    }

    #[test]
    fn room_replica_and_replica_player_round_trip() {
        let r = sample_replica();
        roundtrip(&r);
        for p in &r.players {
            roundtrip(p);
        }
        // The default is a valid, round-trippable seed too.
        roundtrip(&RoomReplica::default());
    }

    // --- The cap-consistency boundary test (spec §3.1 rev-2.1) --------------

    /// A worst-case `Join` — a `MAX_AVATAR_PNG`-sized raw custom avatar (base64 on
    /// the wire), a max-length name, a room code, and a reconnect token — must
    /// serialize to `≤ MAX_FRAME_BYTES`. This is the cap-consistency that forced
    /// the 32 KiB raw avatar cap: the rev-2 "64 KiB base64" avatar limit could not
    /// fit the 64 KiB frame cap (its base64 alone overran it). RED evidence: set
    /// `MAX_AVATAR_PNG = 64 * 1024` and this fails; 32 KiB makes it pass.
    #[test]
    fn max_avatar_join_frame_fits_the_frame_cap() {
        let raw = vec![0xABu8; MAX_AVATAR_PNG];
        let png_base64 = STANDARD.encode(&raw);
        // base64 expands raw bytes by ~4/3.
        assert!(
            png_base64.len() >= MAX_AVATAR_PNG,
            "base64 never shrinks the payload"
        );
        let join = ClientIntent::Join {
            room: "A".repeat(ROOM_CODE_LEN),
            name: "N".repeat(MAX_NAME_LEN),
            avatar: WireAvatar::Custom { png_base64 },
            protocol_version: PROTOCOL_VERSION,
            reconnect: Some("f".repeat(64)),
        };
        let bytes = serde_json::to_vec(&join).expect("serialize");
        assert!(
            bytes.len() <= MAX_FRAME_BYTES,
            "a max-avatar Join must fit the frame cap: {} bytes > {} (MAX_FRAME_BYTES)",
            bytes.len(),
            MAX_FRAME_BYTES,
        );
    }

    // --- word_slots() derivation (spec §4.1) --------------------------------

    fn replica_with_display(display: &str) -> RoomReplica {
        RoomReplica {
            word_display: display.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn word_slots_full_word_is_all_revealed() {
        let slots = replica_with_display("R O B O T").word_slots();
        assert_eq!(
            slots,
            vec![
                ('R', true),
                ('O', true),
                ('B', true),
                ('O', true),
                ('T', true),
            ]
        );
    }

    #[test]
    fn word_slots_all_blanks_are_unrevealed() {
        let slots = replica_with_display("_ _ _ _ _").word_slots();
        assert_eq!(slots.len(), 5);
        assert!(slots.iter().all(|&(c, r)| c == '_' && !r));
    }

    #[test]
    fn word_slots_hint_revealed_is_mixed() {
        // Only the middle letter is revealed (a mid-turn hint flip).
        let slots = replica_with_display("_ _ B _ _").word_slots();
        assert_eq!(
            slots,
            vec![
                ('_', false),
                ('_', false),
                ('B', true),
                ('_', false),
                ('_', false),
            ]
        );
    }

    #[test]
    fn word_slots_is_empty_outside_a_turn() {
        assert!(RoomReplica::default().word_slots().is_empty());
    }
}
