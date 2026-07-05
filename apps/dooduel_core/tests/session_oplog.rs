//! W2.4 — op-log determinism + late-join equivalence (spec §2.2, §3.5).
//!
//! The op log is the canvas sync primitive: identical [`CanvasOp`] sequences rasterize
//! to byte-identical pixels on every replica (the integer-op determinism), undo is a
//! remove-then-re-rasterize, and a late joiner seeded by `CanvasLog` ends pixel-identical
//! to a from-start replica after subsequent ops + an undo that reaches a pre-join op.

mod common;

use std::time::Duration;

use common::{Harness, canvas_op_id, fold_canvas, welcome_token};
use dooduel_core::canvas::{PAPER, PaintBuffer, Tool, flood_fill, stamp_circle, stroke_segment};
use dooduel_core::game::Config;
use dooduel_core::protocol::{CanvasOp, ClientIntent};

const W: usize = 48;
const H: usize = 36;

fn blank() -> Vec<u8> {
    PAPER.iter().copied().cycle().take(W * H * 4).collect()
}

/// Rasterize an op log with the pure integer free-fns (a replica's derivation path).
fn via_freefns(ops: &[CanvasOp]) -> Vec<u8> {
    let mut px = blank();
    for op in ops {
        match op {
            CanvasOp::Stroke {
                points,
                color,
                radius,
                ..
            } => {
                if let Some(&(x0, y0)) = points.first() {
                    stamp_circle(&mut px, W, H, x0, y0, *radius, *color);
                    for pair in points.windows(2) {
                        stroke_segment(&mut px, W, H, pair[0], pair[1], *radius, *color);
                    }
                }
            }
            CanvasOp::Fill { seed, color, .. } => flood_fill(&mut px, W, H, seed.0, seed.1, *color),
        }
    }
    px
}

/// Rasterize an op log through the interactive [`PaintBuffer`] (the drawer's optimistic
/// path) — the same begin/extend/fill the GUI drives.
fn via_paint_buffer(ops: &[CanvasOp]) -> Vec<u8> {
    let mut pb = PaintBuffer::new(W, H, PAPER);
    for op in ops {
        match op {
            CanvasOp::Stroke {
                points,
                color,
                radius,
                ..
            } => {
                pb.color = *color;
                pb.radius = *radius;
                pb.tool = Tool::Brush;
                if let Some(&(x0, y0)) = points.first() {
                    pb.begin(x0, y0);
                    for &(x, y) in &points[1..] {
                        pb.extend(x, y);
                    }
                    pb.end();
                }
            }
            CanvasOp::Fill { seed, color, .. } => {
                pb.color = *color;
                pb.fill(seed.0, seed.1);
            }
        }
    }
    pb.pixels
}

fn sample_log() -> Vec<CanvasOp> {
    vec![
        CanvasOp::Stroke {
            id: 0,
            points: vec![(2, 2), (10, 6), (20, 20), (30, 10)],
            color: [20, 20, 24, 255],
            radius: 3,
        },
        CanvasOp::Stroke {
            id: 1,
            points: vec![(5, 30), (40, 30)],
            color: [0xe8, 0x45, 0x3f, 255],
            radius: 2,
        },
        CanvasOp::Fill {
            id: 2,
            seed: (45, 2),
            color: [0x2f, 0x9b, 0xdb, 255],
        },
    ]
}

fn d(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

/// (a) Identical op logs rasterize to identical pixels — and the op-log replay path
/// matches the interactive PaintBuffer path (the optimistic-paint ≡ authoritative-replay
/// identity, spec §3.5).
#[test]
fn identical_op_logs_rasterize_identically() {
    let log = sample_log();
    assert_eq!(via_freefns(&log), via_freefns(&log), "deterministic replay");
    assert_eq!(
        via_freefns(&log),
        via_paint_buffer(&log),
        "op-log replay matches the interactive PaintBuffer path"
    );
    // A non-empty drawing (sanity: the ops actually painted something).
    assert_ne!(via_freefns(&log), blank(), "the log paints ink");
}

/// (b) Undo = remove the last op + re-rasterize; equivalently, a replica that folded
/// CanvasUndo ends identical to one that never saw the removed op.
#[test]
fn undo_is_remove_then_rerasterize() {
    let log = sample_log();
    let last_id = canvas_op_id(log.last().unwrap());

    // Replay of the truncated log (what every replica re-rasterizes after an undo).
    let truncated: Vec<CanvasOp> = log[..log.len() - 1].to_vec();
    let replay_after_undo = via_freefns(&truncated);

    // A replica that folded the full log + CanvasUndo(last_id).
    let mut folded = log.clone();
    folded.retain(|o| canvas_op_id(o) != last_id);
    let incremental_with_undo = via_freefns(&folded);

    assert_eq!(replay_after_undo, incremental_with_undo);
    // And the undo actually changed the picture (the fill was removed).
    assert_ne!(replay_after_undo, via_freefns(&log));
}

/// (c) A late joiner seeded by `CanvasLog` (a reconnect, spec §6.3) ends pixel-identical
/// to a from-start replica after subsequent ops and an undo that reaches a pre-join op.
#[test]
fn late_join_via_canvas_log_matches_from_start() {
    let config = Config {
        total_rounds: 1,
        draw_seconds: 80,
        pick_seconds: 30,
        reveal_seconds: 2,
        hint_count: 2,
        bots_enabled: false,
    };
    let mut h = Harness::new(config, 0);
    let d0 = h.connect("Ada", None); // seat 0 (drawer turn 1)
    let d1 = h.connect("Bo", None); // seat 1 (guesser)
    let bo_token = welcome_token(h.log_for(d1)).expect("Bo got a reconnect token");
    h.send(0, ClientIntent::StartMatch);
    h.send(0, ClientIntent::Pick { index: 0 });
    h.tick(d(1));

    // The drawer lays down two ops; the guesser sees them incrementally.
    let stroke = |id: u64, a: (i32, i32), b: (i32, i32)| ClientIntent::Stroke {
        stroke_id: id,
        points: vec![a, b],
        color: [20, 20, 24, 255],
        radius: 3,
        done: true,
    };
    h.send(d0, stroke(10, (2, 2), (20, 20)));
    h.send(d0, stroke(11, (30, 5), (5, 30)));

    // The from-start replica (never-dropped guesser) so far:
    let from_start_seed = fold_canvas(h.log_for(d1));
    assert_eq!(from_start_seed.len(), 2, "the guesser saw both ops");

    // Bo drops and reconnects mid-turn ⇒ a fresh connection seeded by CanvasLog.
    h.drop_client(d1);
    let d1b = h.connect("Bo", Some(bo_token));
    let late_seed = fold_canvas(h.log_for(d1b));
    assert_eq!(
        via_freefns(&late_seed),
        via_freefns(&from_start_seed),
        "CanvasLog late-join seeds the same picture as incremental application"
    );

    // A further op, then two undos — the second reaches a pre-reconnect op.
    h.send(d0, stroke(12, (10, 30), (40, 10)));
    h.send(d0, ClientIntent::Undo); // removes op 12
    h.send(d0, ClientIntent::Undo); // removes op 11 (drawn before Bo reconnected)

    // The stroke_id↔op-id reconciliation rule: the server assigns its OWN monotonic op
    // ids (0, 1, 2) independent of the client's stroke_ids (10, 11, 12) — the log +
    // CanvasUndo reference the server ids. Client stroke_id 10→op 0, 11→op 1, 12→op 2;
    // the two undos removed ops 2 then 1, leaving only op 0 — a pre-reconnect op that
    // lived in Bo's CanvasLog seed, proving undo reaches into the late-join seed.
    let reconnected = fold_canvas(h.log_for(d1b));
    assert_eq!(
        reconnected.iter().map(canvas_op_id).collect::<Vec<_>>(),
        vec![0],
        "server op ids are assigned independently of client stroke_ids"
    );
    let survivor = vec![from_start_seed[0].clone()];
    assert_eq!(
        reconnected, survivor,
        "the reconnected replica holds the surviving op"
    );
    assert_eq!(
        via_freefns(&reconnected),
        via_freefns(&survivor),
        "undo reaching a pre-join op leaves the reconnected replica pixel-identical"
    );
}
