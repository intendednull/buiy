//! The per-room actor (spec §6.1). Each room is one async task ([`room_task`]) that
//! **solely owns** its [`Session`] — no mutex on game state, intake order = mutation
//! order = deterministic. The task is a thin channel shuttle around [`Room`], the
//! SYNC, socket-free core that maps connections to seats and fans the Session's
//! `Recipient`-addressed events back to connections. Keeping [`Room`] sync makes the
//! whole actor unit-testable without a runtime (W4.4).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_channel::{Receiver, Sender};
use futures_lite::StreamExt as _;
use futures_lite::future;

use dooduel_core::game::Config;
use dooduel_core::protocol::{ClientIntent, ErrorCode, ServerEvent, WireAvatar};
use dooduel_core::session::{GRACE, Recipient, Session, SessionOpts};
use dooduel_core::transport::ConnId;

use crate::registry::Registry;
use crate::util;

/// A message the wire tasks send the room actor (spec §6.1). `Join` carries the
/// already-parsed, version-checked first frame plus the connection's outbox sender;
/// `Intent` is every later gameplay frame; `Disconnect` is a dropped socket.
pub enum RoomMsg {
    /// A connection joins (fresh or via a reconnect token). The registry has already
    /// resolved WHICH room; the actor does the seat allocation via [`Session::connect`].
    Join {
        conn: ConnId,
        name: String,
        avatar: WireAvatar,
        reconnect: Option<String>,
        /// The room→connection event sink (unbounded; the conn's write task drains it).
        outbox: Sender<ServerEvent>,
    },
    /// A gameplay intent from a seated connection.
    Intent { conn: ConnId, intent: ClientIntent },
    /// The connection dropped (socket closed/errored).
    Disconnect { conn: ConnId },
}

/// The side effects one [`Room`] step produced. The async wrapper executes them
/// against the real channels; the tests read them directly.
#[derive(Default)]
pub struct Outbound {
    /// Events to deliver, already resolved to concrete connections.
    pub deliver: Vec<(ConnId, ServerEvent)>,
    /// Connections to close — a displaced old connection (live-token replacement,
    /// spec §6.3) or a rejected join. The wrapper drops their outbox, which FINs the
    /// connection's write loop.
    pub close: Vec<ConnId>,
}

/// The SYNC room core (spec §6.1): the authoritative [`Session`] plus the live
/// connection↔seat bindings. Socket-free and runtime-free, so it is unit-testable.
pub struct Room {
    session: Session,
    /// Live `(connection, seat)` bindings. One seat has at most one live connection;
    /// a reconnect displaces the old one (spec §6.3).
    seat_of_conn: Vec<(ConnId, usize)>,
    out: Outbound,
    room_code: String,
}

impl Room {
    /// A fresh room owning a lobby [`Session`] with the injected server policy: a
    /// `getrandom` token generator + match seed (both secrecy-critical, spec §4.1/§6.3)
    /// and `fill_bots_to: 0` (networked M1 rooms seat only real players, spec §8).
    pub fn new(
        room_code: String,
        config: Config,
        match_seed: u64,
        token_gen: Box<dyn FnMut() -> String + Send>,
    ) -> Self {
        let opts = SessionOpts {
            token_gen,
            fill_bots_to: 0,
            room_code: room_code.clone(),
            match_seed,
        };
        Room {
            session: Session::new(config, opts),
            seat_of_conn: Vec::new(),
            out: Outbound::default(),
            room_code,
        }
    }

    /// Admit a connection (spec §6.3). A fresh join allocates a seat; a valid
    /// `reconnect` token re-attaches its held seat and — the old-conn replacement — if
    /// another live connection still holds that seat, closes it and rebinds. A rejected
    /// connect delivers the `Error` to the connection and closes it.
    pub fn on_join(
        &mut self,
        conn: ConnId,
        name: String,
        avatar: WireAvatar,
        reconnect: Option<String>,
        now: Duration,
    ) {
        match self
            .session
            .connect(&name, avatar, reconnect.as_deref(), now)
        {
            Ok(seat) => {
                // Live-token replacement (spec §6.3): a still-open socket on this seat
                // is closed and the seat rebinds to the new connection.
                if let Some(pos) = self
                    .seat_of_conn
                    .iter()
                    .position(|&(c, s)| s == seat && c != conn)
                {
                    let (old_conn, _) = self.seat_of_conn.remove(pos);
                    self.out.close.push(old_conn);
                }
                self.seat_of_conn.retain(|&(c, _)| c != conn);
                self.seat_of_conn.push((conn, seat));
                self.pump();
            }
            Err(code) => {
                let message = reject_message(&code).to_string();
                self.out
                    .deliver
                    .push((conn, ServerEvent::Error { code, message }));
                self.out.close.push(conn);
            }
        }
    }

    /// Apply a seated connection's gameplay intent (spec §3.2). An intent from an
    /// unbound connection (e.g. a displaced old socket still draining) is ignored —
    /// never a panic, never a spurious seat mutation.
    pub fn on_intent(&mut self, conn: ConnId, intent: ClientIntent) {
        if let Some(&(_, seat)) = self.seat_of_conn.iter().find(|&&(c, _)| c == conn) {
            self.session.handle(seat, intent);
            self.pump();
        }
    }

    /// A connection dropped (spec §6.3): unbind it and start its seat's grace window.
    pub fn on_disconnect(&mut self, conn: ConnId, now: Duration) {
        if let Some(pos) = self.seat_of_conn.iter().position(|&(c, _)| c == conn) {
            let (_, seat) = self.seat_of_conn.remove(pos);
            self.session.disconnect(seat, now);
            self.pump();
        }
    }

    /// One authoritative tick (spec §6.1): countdown/hints/auto-pick/turn-end + the bot
    /// drain + grace expiry, then fan the resulting events.
    pub fn on_tick(&mut self, now: Duration) {
        self.session.tick(now);
        self.pump();
    }

    /// Drain the Session's staged events and resolve each `Recipient` to concrete
    /// connections (spec §3.3). `All` fans to every live connection; `Seat(n)` reaches
    /// seat `n`'s connection, or is dropped if that seat has none (a bot, or a seat in
    /// grace). Notable events are logged as the per-turn transcript (spec §1.4).
    fn pump(&mut self) {
        let events = self.session.drain_events();
        for (recipient, ev) in events {
            self.log_transcript(recipient, &ev);
            match recipient {
                Recipient::All => {
                    for &(conn, _) in &self.seat_of_conn {
                        self.out.deliver.push((conn, ev.clone()));
                    }
                }
                Recipient::Seat(seat) => {
                    if let Some(&(conn, _)) = self.seat_of_conn.iter().find(|&&(_, s)| s == seat) {
                        self.out.deliver.push((conn, ev));
                    }
                }
            }
        }
    }

    /// The per-turn transcript (spec §1.4 — the acceptance evidence stream). Logged to
    /// **stderr** so stdout stays clean for the `LISTENING port=` discovery line.
    fn log_transcript(&self, recipient: Recipient, ev: &ServerEvent) {
        if recipient != Recipient::All {
            return; // one line per broadcast event, not per recipient copy
        }
        match ev {
            ServerEvent::PhaseChanged {
                phase,
                drawer,
                round,
                total_rounds,
                ..
            } => eprintln!(
                "[room {}] phase={phase:?} drawer={drawer:?} round={round}/{total_rounds}",
                self.room_code
            ),
            ServerEvent::GuessResult {
                seat,
                correct: true,
                points,
            } => eprintln!(
                "[room {}] seat {seat} guessed correctly (+{points})",
                self.room_code
            ),
            ServerEvent::TurnEnded { word, .. } => {
                eprintln!("[room {}] turn ended — word was '{word}'", self.room_code)
            }
            ServerEvent::MatchEnded { podium } => {
                eprintln!("[room {}] match ended — podium {podium:?}", self.room_code)
            }
            _ => {}
        }
    }

    /// Take the accumulated side effects (the wrapper executes them).
    pub fn take_outbound(&mut self) -> Outbound {
        std::mem::take(&mut self.out)
    }

    /// The count of live connections (the actor GCs the room when this hits 0 and
    /// stays there past grace).
    #[cfg(test)]
    pub fn live_conns(&self) -> usize {
        self.seat_of_conn.len()
    }
}

/// The human-readable companion to a rejection code (spec §3.2). The machine-readable
/// [`ErrorCode`] is authoritative; this is the `message` field.
fn reject_message(code: &ErrorCode) -> &'static str {
    match code {
        ErrorCode::VersionMismatch => "unsupported protocol version",
        ErrorCode::RoomNotFound => "no such room",
        ErrorCode::RoomFull => "the room is full",
        ErrorCode::MatchInProgress => "the match already started — reconnect only",
        ErrorCode::NotHost => "only the host can do that",
        ErrorCode::NotDrawer => "only the drawer can do that",
        ErrorCode::WrongPhase => "not allowed in this phase",
        ErrorCode::BadToken => "invalid or expired reconnect token",
        ErrorCode::RateLimited => "too many requests",
        ErrorCode::TooLarge => "payload too large",
        ErrorCode::Malformed => "malformed request",
    }
}

/// What woke the room loop: a channel message or the 100 ms tick.
enum Wake {
    Msg(Option<RoomMsg>),
    Tick,
}

/// The async room actor (spec §6.1): own the [`Room`], loop over (intake channel, 100 ms
/// tick), and after each step deliver the staged events to per-connection outboxes +
/// close any displaced/rejected connections. GC the room — and remove it from the
/// registry — once it has had no live connection for a grace window (spec §6.2).
pub async fn room_task(
    room_code: String,
    rx: Receiver<RoomMsg>,
    registry: Arc<Registry>,
    config: Config,
) {
    let mut room = Room::new(
        room_code.clone(),
        config,
        util::random_seed(),
        Box::new(util::random_token),
    );
    let mut outboxes: HashMap<ConnId, Sender<ServerEvent>> = HashMap::new();
    let start = Instant::now();
    let mut ticker = async_io::Timer::interval(Duration::from_millis(100));
    let mut idle_since: Option<Instant> = None;

    loop {
        // Race the next intake message against the next tick, giving messages priority
        // (`or` polls the first future first) so intents process promptly. Neither
        // future is lost when the other wins: the loser was Pending, dropped cleanly.
        let wake = future::or(async { Wake::Msg(rx.recv().await.ok()) }, async {
            ticker.next().await;
            Wake::Tick
        })
        .await;

        let now = start.elapsed();
        match wake {
            Wake::Msg(Some(RoomMsg::Join {
                conn,
                name,
                avatar,
                reconnect,
                outbox,
            })) => {
                outboxes.insert(conn, outbox);
                room.on_join(conn, name, avatar, reconnect, now);
            }
            Wake::Msg(Some(RoomMsg::Intent { conn, intent })) => room.on_intent(conn, intent),
            Wake::Msg(Some(RoomMsg::Disconnect { conn })) => {
                room.on_disconnect(conn, now);
                outboxes.remove(&conn);
            }
            // Every wire task holds a clone of the intake sender; `None` means they all
            // dropped (no connections will ever arrive) — GC immediately.
            Wake::Msg(None) => break,
            Wake::Tick => room.on_tick(now),
        }

        let out = room.take_outbound();
        for (conn, ev) in out.deliver {
            if let Some(tx) = outboxes.get(&conn) {
                // Unbounded outbox: a send only fails if the conn's write task is gone.
                let _ = tx.try_send(ev);
            }
        }
        for conn in out.close {
            // Dropping the outbox FINs the connection's write loop (spec §6.3).
            outboxes.remove(&conn);
        }

        // GC (spec §6.2): once no live connection remains for a grace window, exit and
        // deregister. A seat in grace has no connection but a reconnect can still land
        // within GRACE, so the window matches the Session's own vacate deadline.
        if outboxes.is_empty() {
            match idle_since {
                None => idle_since = Some(Instant::now()),
                Some(t) if t.elapsed() >= GRACE => break,
                Some(_) => {}
            }
        } else {
            idle_since = None;
        }
    }

    registry.remove_room(&room_code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dooduel_core::game::Phase;

    // A deterministic token generator for the sync tests (the server uses getrandom).
    fn counter_tokens() -> Box<dyn FnMut() -> String + Send> {
        let mut n = 0u64;
        Box::new(move || {
            n += 1;
            format!("tok-{n}")
        })
    }

    fn new_room() -> Room {
        Room::new(
            "ABC123".to_string(),
            Config::default(),
            0xD00D_2026,
            counter_tokens(),
        )
    }

    fn d(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    fn join(room: &mut Room, conn: ConnId, name: &str, now: Duration) {
        room.on_join(conn, name.to_string(), WireAvatar::Default, None, now);
    }

    /// The event addressed to `conn` matching `pred`, if any (drains take_outbound).
    fn delivered_to(out: &Outbound, conn: ConnId) -> Vec<ServerEvent> {
        out.deliver
            .iter()
            .filter(|(c, _)| *c == conn)
            .map(|(_, e)| e.clone())
            .collect()
    }

    #[test]
    fn a_fresh_join_seats_and_reseeds_the_connection() {
        let mut room = new_room();
        join(&mut room, 10, "Ada", d(0));
        let out = room.take_outbound();
        let evs = delivered_to(&out, 10);
        assert!(
            evs.iter()
                .any(|e| matches!(e, ServerEvent::Welcome { seat: 0, .. })),
            "the first joiner is seated at 0 with a Welcome: {evs:?}"
        );
        assert!(
            evs.iter().any(|e| matches!(e, ServerEvent::RoomState(_))),
            "and gets a RoomState seed"
        );
        assert_eq!(room.live_conns(), 1);
    }

    #[test]
    fn the_room_forwards_intents_to_the_owned_session_faithfully() {
        // Room-actor determinism (spec §9.5): a scripted intent script driven THROUGH
        // the room produces the same authoritative OUTPUT as the same script run against
        // a bare Session — the actor adds no drift. We compare the drawer's word
        // choices (a pure function of the injected seed + roster), an observable that
        // needs no private-field access.
        let mut room = new_room();
        join(&mut room, 1, "Ada", d(0));
        join(&mut room, 2, "Bo", d(0));
        room.take_outbound();
        room.on_intent(1, ClientIntent::StartMatch); // seat 0 hosts + draws
        let out = room.take_outbound();
        let room_choices = out
            .deliver
            .iter()
            .find_map(|(c, e)| match e {
                ServerEvent::WordChoices { words } if *c == 1 => Some(words.clone()),
                _ => None,
            })
            .expect("the drawer (conn 1) is offered word choices");

        // The same script on a bare Session (same seed) offers the same choices.
        let mut bare = Session::new(
            Config::default(),
            SessionOpts {
                token_gen: counter_tokens(),
                fill_bots_to: 0,
                room_code: "ABC123".to_string(),
                match_seed: 0xD00D_2026,
            },
        );
        bare.connect("Ada", WireAvatar::Default, None, d(0))
            .unwrap();
        bare.connect("Bo", WireAvatar::Default, None, d(0)).unwrap();
        bare.handle(0, ClientIntent::StartMatch);
        let bare_choices = bare
            .drain_events()
            .into_iter()
            .find_map(|(r, e)| match (r, e) {
                (Recipient::Seat(0), ServerEvent::WordChoices { words }) => Some(words),
                _ => None,
            })
            .expect("the bare Session offers the drawer word choices");

        assert_eq!(
            room_choices, bare_choices,
            "the room's Session behaves identically to a bare one (no drift)"
        );
    }

    #[test]
    fn a_reconnect_replaces_the_old_connection_and_reseeds_the_new() {
        // The old-conn-replacement UX (spec §6.3, W2-review): a live-token Join while
        // the original socket is still open closes the old connection (it is queued to
        // close) and rebinds the seat to the new one (which is reseeded).
        let mut room = new_room();
        join(&mut room, 100, "Ada", d(0)); // seat 0, token tok-1
        let out = room.take_outbound();
        let token = out
            .deliver
            .iter()
            .find_map(|(_, e)| match e {
                ServerEvent::Welcome {
                    reconnect_token, ..
                } => Some(reconnect_token.clone()),
                _ => None,
            })
            .expect("the first join issues a reconnect token");

        // A NEW connection (200) presents the token while conn 100 is still bound.
        room.on_join(
            200,
            "Ada".to_string(),
            WireAvatar::Default,
            Some(token),
            d(2),
        );
        let out = room.take_outbound();
        assert!(
            out.close.contains(&100),
            "the displaced old connection is closed: {:?}",
            out.close
        );
        assert!(
            delivered_to(&out, 200)
                .iter()
                .any(|e| matches!(e, ServerEvent::RoomState(_))),
            "the new connection is reseeded with a RoomState"
        );
        assert_eq!(room.live_conns(), 1, "still one live seat, now on conn 200");
    }

    #[test]
    fn a_bad_reconnect_token_is_rejected_and_closed() {
        let mut room = new_room();
        join(&mut room, 1, "Ada", d(0));
        room.take_outbound();
        room.on_join(
            2,
            "Bo".to_string(),
            WireAvatar::Default,
            Some("not-a-real-token".to_string()),
            d(1),
        );
        let out = room.take_outbound();
        assert!(out.close.contains(&2), "the bad-token conn is closed");
        assert!(
            delivered_to(&out, 2).iter().any(|e| matches!(
                e,
                ServerEvent::Error {
                    code: ErrorCode::BadToken,
                    ..
                }
            )),
            "with a BadToken error"
        );
    }

    #[test]
    fn a_mismatched_version_never_reaches_the_room() {
        // The version gate lives in the WIRE layer (before the room), so the room never
        // sees a Create/Join — but if a wrong-version intent DID arrive as a gameplay
        // frame from an unbound conn, the room ignores it (no panic, no mutation).
        let mut room = new_room();
        room.on_intent(
            999,
            ClientIntent::Guess {
                text: "robot".into(),
            },
        );
        let out = room.take_outbound();
        assert!(out.deliver.is_empty() && out.close.is_empty());
        // A well-formed match still starts afterwards (the stray intent left no state).
        join(&mut room, 1, "Ada", d(0));
        join(&mut room, 2, "Bo", d(0));
        room.take_outbound();
        room.on_intent(1, ClientIntent::StartMatch);
        let out = room.take_outbound();
        assert!(
            out.deliver.iter().any(|(_, e)| matches!(
                e,
                ServerEvent::PhaseChanged {
                    phase: Phase::Picking,
                    ..
                }
            )),
            "a well-formed match still starts (Picking) after the stray intent"
        );
    }
}
