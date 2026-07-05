//! W2.3 — the secrecy scan (spec §9.2, the load-bearing anti-cheat guard).
//!
//! Over a seeded, scripted 2-round match (4 seats, words carrying no substring
//! collisions with any scripted text — only exact-word correct guesses are sent, so no
//! client text is echoed), for **every guesser seat and every turn** we serialize every
//! event that seat actually received and assert the secret (case-insensitive) is absent
//! from its stream **before `min(that seat's correct guess, TurnEnded)`** (spec §5.1's
//! `knows(seat)`). The drawer's stream MUST carry the word — the sanity check that
//! proves the scan can see a leak.
//!
//! Red→green evidence: with the `Session`'s per-recipient redaction
//! (`word_display_for(seat)` / drawer-only `WordChoices` / `chat_for(seat)`) the scan is
//! green. Break redaction (e.g. make `word_display_for` always return the full word) and
//! this test fails at the first pre-reveal event — see the wave report.

mod common;

use std::time::Duration;

use common::{Harness, earned_reveal, json_lower, last_word_choices};
use dooduel_core::game::Config;
use dooduel_core::protocol::{ClientIntent, ServerEvent};

fn d(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

#[test]
fn the_secret_never_reaches_a_guesser_before_they_earn_it() {
    // Short draw window keeps the scripted match fast; the silent guesser forces a
    // timeout so the `TurnEnded` cutoff is exercised alongside the correct-guess cutoff.
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
        let conn = h.connect(n, None);
        assert_eq!(conn as usize, i, "conn i maps to seat i (no reconnect)");
        assert_eq!(h.seat_of(conn), Some(i));
    }
    h.send(0, ClientIntent::StartMatch);

    let n_seats = names.len();
    let n_turns = 2 * n_seats; // 2 rounds × 4 seats
    let mut clock = 0u64;
    let mut reached_podium = false;

    for turn in 0..n_turns {
        let drawer = turn % n_seats;
        let drawer_conn = drawer as u64;

        let secret = last_word_choices(h.log_for(drawer_conn))
            .expect("the drawer was offered word choices in Picking")[0]
            .clone();
        let secret_lc = secret.to_lowercase();

        // Checkpoint every stream at Drawing entry (just before the pick).
        let checkpoint: Vec<usize> = (0..n_seats).map(|c| h.log_for(c as u64).len()).collect();

        h.send(drawer_conn, ClientIntent::Pick { index: 0 });
        clock += 1;
        h.tick(d(clock)); // anchor the Drawing clock

        // Script: two guessers earn the word; the third stays silent (forces a timeout).
        let g1 = ((drawer + 1) % n_seats) as u64;
        let g2 = ((drawer + 2) % n_seats) as u64;
        h.send(
            g1,
            ClientIntent::Guess {
                text: secret.clone(),
            },
        );
        h.send(
            g2,
            ClientIntent::Guess {
                text: secret.clone(),
            },
        );

        // Run the clock until this turn ends (a TurnEnded lands in the drawer's stream).
        let mut guard = 0;
        while !h.log_for(drawer_conn)[checkpoint[drawer]..]
            .iter()
            .any(|e| matches!(e, ServerEvent::TurnEnded { .. }))
        {
            clock += 1;
            h.tick(d(clock));
            guard += 1;
            assert!(guard < 500, "turn {turn} should terminate");
        }

        // --- the scan: no guesser sees the secret before earning it ---
        for c in 0..n_seats {
            if c == drawer {
                continue;
            }
            let stream = h.log_for(c as u64);
            let seg = &stream[checkpoint[c]..];
            let cutoff = seg
                .iter()
                .position(|e| earned_reveal(e, c))
                .unwrap_or(seg.len());
            for (idx, ev) in seg[..cutoff].iter().enumerate() {
                assert!(
                    !json_lower(ev).contains(&secret_lc),
                    "LEAK: seat {c} saw the secret '{secret}' in event #{idx} of turn {turn} \
                     before earning it: {ev:?}"
                );
            }
        }

        // Sanity: the drawer's own stream DOES carry the word (the scan can see a leak).
        let drawer_seg = &h.log_for(drawer_conn)[checkpoint[drawer]..];
        assert!(
            drawer_seg
                .iter()
                .any(|e| json_lower(e).contains(&secret_lc)),
            "the drawer's stream must contain the word (scan sanity), turn {turn}"
        );

        // Advance past the reveal (any seat may Continue).
        h.send(0, ClientIntent::Continue);
        clock += 1;
        h.tick(d(clock));

        if (0..n_seats).any(|c| {
            h.log_for(c as u64)
                .iter()
                .any(|e| matches!(e, ServerEvent::MatchEnded { .. }))
        }) {
            reached_podium = true;
            break;
        }
    }

    assert!(
        reached_podium,
        "the scripted 2-round match reached the podium"
    );
}
