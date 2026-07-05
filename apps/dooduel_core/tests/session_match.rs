//! W2.5 — the in-process full match (spec §9.4). One `Session` + an in-process client
//! (plus `fill_bots_to` bots) plays a whole 2-round match to the podium; the podium
//! must equal the same seed/roster/config run **directly** on a bare `Game` — the
//! authority adds no scoring drift.
//!
//! Both are driven purely by the clock (auto-pick, seeded bot guesses, draw/reveal
//! timeouts), so with the same fixed seed they evolve identically; the test proves the
//! `Session` layer (validation, redaction, event staging) leaves the game arithmetic
//! untouched.

mod common;

use std::time::Duration;

use common::Harness;
use dooduel_core::game::{Config, Game, PRESET_NAMES, Phase, PlayerSpec};
use dooduel_core::protocol::{ClientIntent, ServerEvent};

fn d(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

fn solo_roster() -> Vec<PlayerSpec> {
    let mut roster = vec![PlayerSpec {
        name: "Ada".to_string(),
        is_bot: false,
    }];
    for n in PRESET_NAMES {
        roster.push(PlayerSpec {
            name: n.to_string(),
            is_bot: true,
        });
    }
    roster
}

/// The oracle podium: a bare `Game`, same roster/config, driven purely by the clock.
fn oracle_podium(config: Config, ticks: &[u64]) -> Vec<(usize, String, i64)> {
    let mut g = Game::default();
    g.start_match(&solo_roster(), config);
    for &sec in ticks {
        let pending = g.tick(d(sec));
        for p in pending {
            g.apply_guess(p.player, &p.text);
        }
        if g.phase == Phase::Final {
            break;
        }
    }
    assert_eq!(g.phase, Phase::Final, "the oracle match reached the podium");
    g.standings()
        .into_iter()
        .map(|(i, p)| (i, p.name, p.score))
        .collect()
}

#[test]
fn in_process_full_match_matches_a_direct_game_oracle() {
    let config = Config {
        total_rounds: 2,
        draw_seconds: 20,
        pick_seconds: 5,
        reveal_seconds: 2,
        hint_count: 2,
        bots_enabled: true,
    };

    // The Session: one human client (Ada, seat 0) + fill_bots_to = 4 ⇒ the same
    // [Ada, Priya, Theo, Sam] roster the oracle uses.
    let mut h = Harness::new(config.clone(), 4);
    let ada = h.connect("Ada", None);
    h.send(ada, ClientIntent::StartMatch);

    // Drive by the clock until the Session broadcasts MatchEnded. The human never
    // guesses (it is not a bot); the match self-drives via auto-pick + seeded bots +
    // timeouts — exactly what the oracle replays.
    let mut ticks: Vec<u64> = Vec::new();
    let mut podium: Option<Vec<(usize, String, i64)>> = None;
    for sec in 0..2000u64 {
        ticks.push(sec);
        h.tick(d(sec));
        if let Some(p) = h.log_for(ada).iter().find_map(|e| match e {
            ServerEvent::MatchEnded { podium } => Some(podium.clone()),
            _ => None,
        }) {
            podium = Some(p);
            break;
        }
    }
    let podium = podium.expect("the in-process match reached the podium");

    // The oracle run over the same tick sequence.
    let oracle = oracle_podium(config, &ticks);

    assert_eq!(
        podium, oracle,
        "the Session podium must equal the direct-Game oracle (no scoring drift)"
    );
    assert_eq!(podium.len(), 4, "four seats on the podium");
    assert!(
        podium.iter().any(|(_, _, score)| *score > 0),
        "the match actually scored points"
    );
    // The podium is ranked highest-first.
    assert!(
        podium.windows(2).all(|w| w[0].2 >= w[1].2),
        "podium is sorted by score descending"
    );
}
