//! W2.3 + W2-review I1 — the secrecy scan (spec §9.2, the load-bearing anti-cheat guard).
//!
//! Over a seeded, scripted 2-round match (4 seats, only exact-word correct guesses so no
//! client text is echoed), for **every guesser seat and every turn** we serialize every
//! event that seat actually received and assert the secret is absent from its stream
//! **before `min(that seat's own correct GuessResult, TurnEnded)`** (spec §5.1's
//! `knows(seat)`). Hardened at the W2 review (I1):
//!
//! - the per-turn window extends **back to the previous `TurnEnded`**, so the Picking
//!   window (where a leaked `WordChoices` would land) is covered;
//! - a script guard asserts no secret is a substring of another (guesses == secrets);
//! - a global assertion: a seat receives `WordChoices` **only** on turns it draws;
//! - a mid-turn reconnect leg (drop a guesser mid-Drawing, rejoin by token) whose
//!   `RoomState`/`CanvasLog` reseed is redacted and folded into the scan.
//!
//! Red→green: with per-recipient redaction the scan is green; leaking `WordChoices`
//! (or the word row) to all seats makes it fail on the first pre-reveal event.

mod common;

use std::time::Duration;

use common::{Harness, earned_reveal, json_lower, last_word_choices, welcome_token};
use dooduel_core::game::Config;
use dooduel_core::protocol::{ClientIntent, ServerEvent};
use dooduel_core::transport::ConnId;

fn d(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

/// Split a stream into per-turn segments at each `TurnEnded` (each segment ends with its
/// `TurnEnded`). For a viewer that started on turn `t0`, segment `k` is turn `t0 + k`.
fn segments(log: &[ServerEvent]) -> Vec<Vec<ServerEvent>> {
    let mut segs = Vec::new();
    let mut cur = Vec::new();
    for e in log {
        cur.push(e.clone());
        if matches!(e, ServerEvent::TurnEnded { .. }) {
            segs.push(std::mem::take(&mut cur));
        }
    }
    segs
}

#[test]
fn the_secret_never_reaches_a_guesser_before_they_earn_it() {
    let config = Config {
        total_rounds: 2,
        draw_seconds: 6,
        pick_seconds: 30,
        reveal_seconds: 1,
        hint_count: 2,
        bots_enabled: false,
    };
    let mut h = Harness::new(config, 0);
    let names = ["Ada", "Bo", "Cy", "Dee"];
    for (i, n) in names.iter().enumerate() {
        let c = h.connect(n, None);
        assert_eq!(c as usize, i, "conn i maps to seat i");
    }
    h.send(0, ClientIntent::StartMatch);
    // Seat 1's reconnect token (deterministic counter — captured, not hardcoded).
    let bo_token = welcome_token(h.log_for(1)).expect("Bo has a token");

    let n_seats = names.len();
    let n_turns = 2 * n_seats;
    let mut clock = 0u64;
    let mut secrets: Vec<String> = Vec::new();
    // seat → current connection (updated on reconnect).
    let mut seat_conn: Vec<ConnId> = (0..n_seats as ConnId).collect();
    // (conn, seat, start_turn) viewers to scan; the reconnect adds one.
    let mut viewers: Vec<(ConnId, usize, usize)> =
        (0..n_seats).map(|s| (s as ConnId, s, 0usize)).collect();

    let turns_ended = |h: &Harness| {
        h.log_for(0)
            .iter()
            .filter(|e| matches!(e, ServerEvent::TurnEnded { .. }))
            .count()
    };

    for turn in 0..n_turns {
        let drawer = turn % n_seats;
        let drawer_conn = seat_conn[drawer];
        let secret = last_word_choices(h.log_for(drawer_conn))
            .expect("the drawer was offered choices")[0]
            .clone();
        secrets.push(secret.clone());

        h.send(drawer_conn, ClientIntent::Pick { index: 0 });
        clock += 1;
        h.tick(d(clock));

        // Two guessers earn the word; the third stays silent (forces a timeout).
        let silent = (drawer + 3) % n_seats;
        for off in [1usize, 2] {
            let g = (drawer + off) % n_seats;
            h.send(
                seat_conn[g],
                ClientIntent::Guess {
                    text: secret.clone(),
                },
            );
        }

        // Reconnect leg (turn 2): the silent guesser drops mid-Drawing and rejoins by
        // token; its reseed must carry no secret before it earns the word.
        if turn == 2 {
            assert_ne!(silent, 0, "keep the host (seat 0) stable in this scan");
            let old = seat_conn[silent];
            h.drop_client(old);
            let rc = h.connect("Bo", Some(bo_token.clone()));
            seat_conn[silent] = rc;
            viewers.push((rc, silent, turn));
            for e in h.log_for(rc) {
                assert!(
                    !json_lower(e).contains(&secret.to_lowercase()),
                    "the reconnect reseed leaked the secret to seat {silent}: {e:?}"
                );
            }
        }

        // Run the clock until this turn ends (observed on the stable host stream).
        let mut guard = 0;
        while turns_ended(&h) < turn + 1 {
            clock += 1;
            h.tick(d(clock));
            guard += 1;
            assert!(guard < 500, "turn {turn} should terminate");
        }

        h.send(seat_conn[0], ClientIntent::Continue);
        clock += 1;
        h.tick(d(clock));
    }

    // Script guard: no secret is a substring of another (so an exact-word guess can never
    // carry another turn's secret, spec §9.2).
    for (i, a) in secrets.iter().enumerate() {
        for (j, b) in secrets.iter().enumerate() {
            if i != j {
                assert!(
                    !a.to_lowercase().contains(&b.to_lowercase()),
                    "secret '{a}' contains secret '{b}' — pick a collision-free word set"
                );
            }
        }
    }

    // The scan proper, over every viewer's per-turn segments.
    for (conn, seat, t0) in &viewers {
        for (k, seg) in segments(h.log_for(*conn)).iter().enumerate() {
            let turn = t0 + k;
            if turn >= n_turns {
                break;
            }
            let drawer = turn % n_seats;
            let secret_lc = secrets[turn].to_lowercase();

            // (b) WordChoices only ever reach the drawer of that turn.
            let has_choices = seg
                .iter()
                .any(|e| matches!(e, ServerEvent::WordChoices { .. }));
            if has_choices {
                assert_eq!(
                    *seat, drawer,
                    "seat {seat} received WordChoices on turn {turn} it does not draw"
                );
            }

            if *seat == drawer {
                assert!(
                    seg.iter().any(|e| json_lower(e).contains(&secret_lc)),
                    "the drawer (seat {seat}) must see the word on turn {turn} (scan sanity)"
                );
            } else {
                let cutoff = seg
                    .iter()
                    .position(|e| earned_reveal(e, *seat))
                    .unwrap_or(seg.len());
                for (i, e) in seg[..cutoff].iter().enumerate() {
                    assert!(
                        !json_lower(e).contains(&secret_lc),
                        "LEAK: seat {seat} (conn {conn}) saw the secret on turn {turn} \
                         event #{i} before earning it: {e:?}"
                    );
                }
            }
        }
    }

    assert!(
        h.log_for(0)
            .iter()
            .any(|e| matches!(e, ServerEvent::MatchEnded { .. })),
        "the scripted 2-round match reached the podium"
    );
}
