//! `dooduel_core` — the pure, Bevy-free heart of Dooduel.
//!
//! This crate holds the game's authority-side logic with **no I/O and no
//! framework coupling** (spec §2.1): the deterministic rules/scoring/clock
//! machinery, the wire protocol, the transport-agnostic `Session` authority, and
//! the Bevy-free pixel surface the op-log canvas rasterizes into. A dedicated
//! server, an in-process solo run, and (eventually) a peer-hosted P2P session all
//! drive the *same* authority behind *different* transports.
//!
//! The one Bevy-family dependency is `bevy_reflect` — every protocol type stays
//! `Reflect`-able so `Msg::Net` folds through the MVU record/replay funnel like
//! any other message. A `cargo tree` tripwire (`tests/purity.rs`) enforces that
//! nothing else from the Bevy/Buiy stack leaks in.
//!
//! Wave 0 extracts the two pure pieces the rest of M1 builds on: the game state
//! machine (`game`) and the canvas pixel surface (`canvas`). Wave 1 adds the wire
//! protocol (`protocol`).

pub mod canvas;
pub mod game;
pub mod protocol;
