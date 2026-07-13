//! W2-review I4 + Q3b — lobby host migration is visible on the wire, and a departing
//! drawer leaves no phantom stroke.

mod common;

use std::time::Duration;

use common::{Harness, canvas_op_id, fold_canvas, welcome_token};
use dooduel_core::game::Config;
use dooduel_core::protocol::{ClientIntent, ServerEvent};

fn d(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

/// I4 — when the lobby host drops past grace, the remaining clients observe the new
/// host through a `Roster` event (no full `RoomState` needed).
#[test]
fn host_migration_is_visible_via_roster() {
    let mut h = Harness::new(Config::default(), 0);
    let _ada = h.connect("Ada", None); // seat 0 (host)
    let bo = h.connect("Bo", None); // seat 1

    let ada_conn = 0; // conn 0 == seat 0 here
    h.drop_client(ada_conn); // host drops → grace
    h.tick(d(46)); // grace (45s) elapsed ⇒ seat 0 vacated ⇒ host migrates

    let host_seen = h.log_for(bo).iter().rev().find_map(|e| match e {
        ServerEvent::Roster { host, .. } => Some(*host),
        _ => None,
    });
    assert_eq!(
        host_seen,
        Some(1),
        "the remaining client sees the host migrate to seat 1 via Roster"
    );
}

/// Q3b — a drawer with an in-progress (`done:false`) stroke disconnects and reconnects
/// via its token; the discarded stroke never becomes a phantom op, so the guesser's
/// authoritative replay holds exactly the finalized ops.
#[test]
fn a_departing_drawer_leaves_no_phantom_stroke() {
    let config = Config {
        total_rounds: 1,
        draw_seconds: 200,
        pick_seconds: 30,
        reveal_seconds: 2,
        hint_count: 2,
        bots_enabled: false,
    };
    let mut h = Harness::new(config, 0);
    let d0 = h.connect("Ada", None); // seat 0 (drawer turn 1)
    let d1 = h.connect("Bo", None); // seat 1 (guesser)
    let ada_token = welcome_token(h.log_for(d0)).expect("Ada has a reconnect token");
    h.send(0, ClientIntent::StartMatch);
    h.send(0, ClientIntent::Pick { index: 0 });
    h.tick(d(1));

    // A complete op, then an in-progress stroke the drawer never finishes.
    h.send(
        d0,
        ClientIntent::Stroke {
            stroke_id: 1,
            points: vec![(10, 10), (20, 20)],
            color: [0, 0, 0, 255],
            radius: 3,
            done: true,
        },
    );
    h.send(
        d0,
        ClientIntent::Stroke {
            stroke_id: 2,
            points: vec![(30, 30), (40, 40)],
            color: [0, 0, 0, 255],
            radius: 3,
            done: false, // never finalized
        },
    );

    // The drawer drops (open stroke discarded) and reconnects within grace.
    h.drop_client(d0);
    let d0b = h.connect("Ada", Some(ada_token));

    // The drawer draws again — a fresh complete op.
    h.send(
        d0b,
        ClientIntent::Stroke {
            stroke_id: 3,
            points: vec![(50, 50), (60, 60)],
            color: [0, 0, 0, 255],
            radius: 3,
            done: true,
        },
    );

    // The guesser's authoritative replay = the two finalized ops (ids 0 and 1); the
    // discarded in-progress stroke_id 2 minted no op and no id.
    let ops = fold_canvas(h.log_for(d1));
    assert_eq!(
        ops.iter().map(canvas_op_id).collect::<Vec<_>>(),
        vec![0, 1],
        "the discarded open stroke left no phantom op; ids stay dense"
    );
}
