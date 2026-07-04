//! The pure Dooduel game core — the match state machine, scoring, guess matching
//! and deterministic bots, with **zero framework coupling**.
//!
//! Everything here is a plain data type + pure `&mut self` methods, so the whole
//! game is unit-testable without an ECS, a GPU, or a clock. The MVU wiring in
//! `lib.rs` is a thin shell: the reducer folds a `Msg` into a call on one of
//! these methods; the tick driver turns wall-clock into a `Tick(now)` message.
//!
//! ## The clock model (the game-seam pattern)
//!
//! We never store `now`. Each phase records a single **anchor** (`phase_started_at`,
//! the `now` at which the phase was entered) and derives its countdown as
//! `total - (now - anchor)`. A steady frame (same whole second) recomputes the
//! *same* derived values, so `set_if_neq` in the MVU drain absorbs it (no model
//! mutation, no view rebuild) — the same poll-style discipline the blink perf
//! fixture proves (`buiy_bench_support::mvu_scenes`).
//!
//! Phase transitions set the anchor to `None`; the *next* tick re-anchors it to
//! `now`. This makes [`Game::tick`] the sole owner of the clock, so transitions
//! triggered by a plain `Msg` (with no timestamp — `StartMatch`, `ChooseWord`)
//! and transitions triggered by a timeout (which do have `now`) share one path.
//!
//! ## Determinism
//!
//! No wall-clock randomness leaks into the state. A `u64` seed is carried in the
//! model and advanced with a pure splitmix64 step; word choices, the auto-pick,
//! hint reveal order and bot fire schedules are all derived from it. Replaying
//! the same `Msg` stream (including the same `Tick(now)` values) reproduces the
//! match byte-for-byte.

use std::time::Duration;

use bevy::prelude::Reflect;

// --- Tunable rules (durations pinned by the Dooduel game spec) --------------

/// Seconds the drawer has to pick a word before one is auto-chosen.
pub const PICK_SECS: u64 = 10;
/// Seconds the turn-end reveal card stays up before the next turn.
pub const REVEAL_SECS: u64 = 6;
/// The three built-in bot guessers' names (the human is seat 0).
pub const PRESET_NAMES: [&str; 3] = ["Priya", "Theo", "Sam"];

/// The built-in word pool (the prototype's 52-word list, verbatim).
pub const WORDS: [&str; 52] = [
    "apple",
    "banana",
    "guitar",
    "robot",
    "castle",
    "dragon",
    "bicycle",
    "umbrella",
    "snowman",
    "rainbow",
    "pizza",
    "rocket",
    "butterfly",
    "lighthouse",
    "octopus",
    "volcano",
    "dinosaur",
    "sandwich",
    "balloon",
    "penguin",
    "cactus",
    "anchor",
    "telescope",
    "mushroom",
    "campfire",
    "kite",
    "jellyfish",
    "pretzel",
    "scarecrow",
    "igloo",
    "windmill",
    "accordion",
    "beehive",
    "cupcake",
    "flamingo",
    "hammer",
    "kangaroo",
    "lantern",
    "mermaid",
    "narwhal",
    "owl",
    "pancake",
    "saxophone",
    "tornado",
    "unicorn",
    "volleyball",
    "waterfall",
    "xylophone",
    "yoyo",
    "zeppelin",
    "skateboard",
    "compass",
];

/// Room rules. Defaults mirror the interactive prototype (rounds **2**, draw
/// **80s**, pick [`PICK_SECS`], reveal [`REVEAL_SECS`], hints **2**); the game
/// spec's "default 3 rounds" is a deferred knob. The phase durations are Config
/// fields (not hard consts) so the W8 multi-agent playtest host can widen them for
/// slow file-protocol agents, and `bots_enabled` lets it turn OFF the built-in bot
/// guessers so all four seats are agent-driven.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct Config {
    pub total_rounds: u32,
    /// Seconds the drawer has to draw before the turn auto-ends.
    pub draw_seconds: u64,
    /// Seconds to pick a word before one is auto-chosen (the safety timeout).
    pub pick_seconds: u64,
    /// Seconds the turn-end reveal card stays up before the next turn.
    pub reveal_seconds: u64,
    pub hint_count: usize,
    /// When `false`, the built-in bot guessers never fire (their plans are still
    /// built so the seeded RNG — and thus word choices — is unchanged; they just
    /// don't guess). The auto-pick safety timeout still runs. The W8 playtest host
    /// sets this so the four seats are all agent-driven.
    pub bots_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            total_rounds: 2,
            draw_seconds: 80,
            pick_seconds: PICK_SECS,
            reveal_seconds: REVEAL_SECS,
            hint_count: 2,
            bots_enabled: true,
        }
    }
}

/// A seat at the table. `name` is a roster label; `score` accrues across the match.
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct Player {
    pub name: String,
    pub score: i64,
}

/// The in-turn phase machine. `Idle` = no match running; `Final` = match over
/// (the shell lifts this to `Screen::Podium`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
pub enum Phase {
    #[default]
    Idle,
    Picking,
    Drawing,
    Reveal,
    Final,
}

/// A correct guess this turn: who, how many points, and their 0-based order.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct Guess {
    pub player: usize,
    pub points: i64,
    pub order: usize,
}

/// One row of the turn-end reveal card: the per-player delta + running total.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct TurnResult {
    pub player: usize,
    pub name: String,
    pub delta: i64,
    pub total: i64,
}

/// Where a chat line came from (drives its styling in later waves).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ChatKind {
    /// Round/word announcements ("Round 1 of 2 — Priya is drawing").
    System,
    /// An ordinary wrong guess (shows the text).
    Guess,
    /// A correct guess ("🎉 Theo guessed the word!").
    Correct,
    /// A private near-miss nudge ("So close! 👀").
    Close,
}

/// One chat/guess-log line. `seq` is a monotonic per-match id used as the keyed
/// list key (deterministic — never a random id).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ChatMsg {
    pub seq: u64,
    pub kind: ChatKind,
    pub text: String,
    /// Who may see this line. `None` = shared (broadcast to everyone, as in the
    /// real game); `Some(seat)` = a PRIVATE line only `seat` sees — the near-miss
    /// "So close! 👀" nudge (bug #4). Per-seat filtering has ONE home,
    /// [`Game::chat_for`], so a private line can never leak to another seat.
    pub to: Option<usize>,
}

/// A scheduled bot guess: seat `player` fires at `fire_at` elapsed-seconds into
/// the draw phase. In this prototype every bot guess is *correct* (bots exist to
/// keep the match self-driving); wrong/near-miss bot chatter is a deferred knob.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BotPlan {
    pub player: usize,
    pub fire_at: u64,
}

/// What [`Game::apply_guess`] did — surfaced so the shell/tests can assert without
/// re-deriving. (The state mutation is already applied.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuessOutcome {
    /// Exact match: the guesser was scored and locked.
    Correct,
    /// Levenshtein-near (private nudge to the guesser).
    Close,
    /// A miss (posted to chat as an ordinary guess).
    Wrong,
    /// Rejected before matching (wrong phase, the drawer, or already-guessed).
    Ignored,
}

/// One letter slot of the in-game word row (the design's underlined blanks): the
/// uppercase letter when the current viewer should see it (`Some`), else a blank
/// (`None`). `revealed` drives the underline color (accent when shown, ink when
/// still blank).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WordSlot {
    pub ch: Option<char>,
    pub revealed: bool,
}

/// A bot guess the tick wants folded back through the funnel as a real `Msg`.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingGuess {
    pub player: usize,
    pub text: String,
}

/// The whole match state — grown as one struct so the reducer stays a single
/// pure fold (the "one giant reducer" shape the wave is stress-testing).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct Game {
    pub config: Config,
    pub players: Vec<Player>,
    /// 1-based round counter.
    pub round: u32,
    /// The seat drawing this turn.
    pub seat_index: usize,
    /// The seat the human currently controls (auto-jumps to the drawer at each
    /// turn start; movable with [`Game::switch_seat`]).
    pub viewing_as: usize,
    pub phase: Phase,
    /// The `now` the current phase was entered at; `None` ⇒ re-anchor next tick.
    pub phase_started_at: Option<Duration>,
    pub word_choices: Vec<String>,
    pub secret_word: String,
    /// Per-letter "revealed?" mask (derived from elapsed each tick).
    pub reveal_mask: Vec<bool>,
    pub pick_seconds_left: u64,
    pub draw_seconds_left: u64,
    pub reveal_seconds_left: u64,
    /// Correct guesses this turn, in guess order.
    pub turn_guesses: Vec<Guess>,
    /// The last turn's reveal rows (shown during `Reveal`).
    pub turn_results: Vec<TurnResult>,
    pub chat: Vec<ChatMsg>,
    pub chat_input: String,
    chat_seq: u64,
    /// Elapsed-seconds at which each successive hint flips open (ascending).
    hint_reveal_at: Vec<u64>,
    /// Letter positions to reveal, in hint order (a seeded permutation).
    hint_positions: Vec<usize>,
    bot_plans: Vec<BotPlan>,
    /// Words already used this match (no-repeat selection).
    used_words: Vec<String>,
    /// The deterministic PRNG state.
    rng: u64,
}

/// One splitmix64 step — a pure, well-distributed PRNG advance. Kept local so the
/// reducer's randomness is fully reproducible from the carried seed.
fn next_rng(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Normalize a guess/word for comparison: lowercase, keep only `[a-z0-9]`
/// (matches the prototype's `trim().toLowerCase().replace(/[^a-z0-9]/gi,'')`).
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Levenshtein edit distance (classic DP over `char` sequences).
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// A near-miss: different, both non-empty, length diff ≤ 2, edit distance ≤ 2
/// (the prototype's `isClose`). Inputs are expected already-normalized.
pub fn is_close(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() || a == b {
        return false;
    }
    if a.len().abs_diff(b.len()) > 2 {
        return false;
    }
    levenshtein(a, b) <= 2
}

/// Guesser points: `max(20, round((50 + 450·frac) · 0.82^order))`, `frac` the
/// fraction of the draw window still left, `order` the 0-based guess order.
pub fn guesser_points(draw_seconds_left: u64, total_draw_seconds: u64, order: usize) -> i64 {
    let frac = if total_draw_seconds == 0 {
        0.0
    } else {
        draw_seconds_left as f64 / total_draw_seconds as f64
    };
    let raw = (50.0 + 450.0 * frac) * 0.82f64.powi(order as i32);
    (raw.round() as i64).max(20)
}

/// Drawer points: `round(100 · correct / guessers)`, or 0 if nobody guessed.
pub fn drawer_points(correct_count: usize, guesser_count: usize) -> i64 {
    if correct_count == 0 || guesser_count == 0 {
        return 0;
    }
    (100.0 * correct_count as f64 / guesser_count as f64).round() as i64
}

impl Game {
    /// Start a fresh match: seat 0 is the human (`human_name`), seats 1..=3 the
    /// preset bots. Resets scores/round/used-words and begins the first turn.
    pub fn start_match(&mut self, human_name: &str, config: Config) {
        let name = if human_name.trim().is_empty() {
            "You".to_string()
        } else {
            human_name.trim().to_string()
        };
        let mut players = vec![Player { name, score: 0 }];
        for n in PRESET_NAMES {
            players.push(Player {
                name: n.to_string(),
                score: 0,
            });
        }
        self.config = config;
        self.players = players;
        self.round = 1;
        self.seat_index = 0;
        self.chat.clear();
        self.chat_seq = 0;
        self.used_words.clear();
        // Seed from a fixed constant so a match is reproducible turn-for-turn.
        self.rng = 0xD00D_0000_0000_0001;
        self.begin_turn();
    }

    /// Enter the `Picking` phase for the current `seat_index`: offer three fresh
    /// word choices, auto-jump the human to the drawer, clear the turn, and post
    /// the round banner. The clock re-anchors on the next tick.
    pub fn begin_turn(&mut self) {
        let drawer = self.seat_index;
        self.word_choices = self.pick_word_choices();
        self.phase = Phase::Picking;
        self.phase_started_at = None;
        self.pick_seconds_left = self.config.pick_seconds;
        self.viewing_as = drawer;
        self.turn_guesses.clear();
        self.turn_results.clear();
        self.secret_word.clear();
        self.reveal_mask.clear();
        let drawer_name = self.players[drawer].name.clone();
        let banner = format!(
            "Round {} of {} — {} is drawing",
            self.round, self.config.total_rounds, drawer_name
        );
        self.push_chat(ChatKind::System, banner);
    }

    /// Commit `word` as the secret and enter `Drawing`: compute the hint schedule,
    /// reveal order and the bots' guess schedule (all from the seed), reset the
    /// blank mask, and re-anchor the clock on the next tick.
    pub fn choose_word(&mut self, word: String) {
        if self.phase != Phase::Picking {
            return;
        }
        self.used_words.push(word.clone());
        let total = self.config.draw_seconds;
        let letter_count = word.chars().filter(|c| c.is_ascii_alphabetic()).count();
        let hint_count = self.config.hint_count.min(letter_count.saturating_sub(1));

        // Hint reveal schedule: thresholds are seconds-LEFT (spec formula); store
        // them as ascending elapsed-seconds so `revealed_count` is a simple count.
        let mut reveal_at: Vec<u64> = Vec::new();
        for i in 1..=hint_count {
            let secs_left = ((total as f64) * (0.6 - i as f64 * 0.18)).floor().max(1.0) as u64;
            reveal_at.push(total.saturating_sub(secs_left));
        }
        reveal_at.sort_unstable();
        self.hint_reveal_at = reveal_at;
        self.hint_positions = self.seeded_hint_positions(&word, hint_count);

        // Bot schedule: every non-drawer seat guesses correctly once, at a seeded
        // moment in the middle of the draw window (staggered so orders differ).
        self.bot_plans = self.seeded_bot_plans(total);

        self.secret_word = word.clone();
        self.reveal_mask = vec![false; word.chars().count()];
        self.draw_seconds_left = total;
        self.turn_guesses.clear();
        self.phase = Phase::Drawing;
        self.phase_started_at = None;
    }

    /// Fold one `Tick(now)`: advance the active phase's countdown, flip hints,
    /// and drive timeouts. Returns the bot guesses that should be folded back as
    /// real `Msg`s this frame (empty on a steady frame — the `set_if_neq` no-op).
    pub fn tick(&mut self, now: Duration) -> Vec<PendingGuess> {
        match self.phase {
            Phase::Idle | Phase::Final => Vec::new(),
            Phase::Picking => {
                let elapsed = self.elapsed_secs(now);
                self.pick_seconds_left = self.config.pick_seconds.saturating_sub(elapsed);
                if elapsed >= self.config.pick_seconds {
                    // Auto-pick a random offered word and start drawing.
                    let idx = (next_rng(&mut self.rng) as usize) % self.word_choices.len().max(1);
                    if let Some(w) = self.word_choices.get(idx).cloned() {
                        self.choose_word(w);
                    }
                }
                Vec::new()
            }
            Phase::Drawing => {
                let total = self.config.draw_seconds;
                let elapsed = self.elapsed_secs(now);
                self.draw_seconds_left = total.saturating_sub(elapsed);
                self.apply_reveal_mask(elapsed);
                if elapsed >= total {
                    self.end_turn();
                    return Vec::new();
                }
                self.due_bot_guesses(elapsed)
            }
            Phase::Reveal => {
                let elapsed = self.elapsed_secs(now);
                self.reveal_seconds_left = self.config.reveal_seconds.saturating_sub(elapsed);
                if elapsed >= self.config.reveal_seconds {
                    self.advance_turn();
                }
                Vec::new()
            }
        }
    }

    /// Apply a guess by `player` (human or bot) with the shared pipeline:
    /// normalize → exact / near / miss → score + chat + early turn end.
    pub fn apply_guess(&mut self, player: usize, raw: &str) -> GuessOutcome {
        if self.phase != Phase::Drawing || player >= self.players.len() {
            return GuessOutcome::Ignored;
        }
        // The drawer cannot guess; nobody guesses twice.
        if player == self.seat_index || self.turn_guesses.iter().any(|g| g.player == player) {
            return GuessOutcome::Ignored;
        }
        let guess = normalize(raw);
        if guess.is_empty() {
            return GuessOutcome::Ignored;
        }
        let secret = normalize(&self.secret_word);
        let name = self.players[player].name.clone();

        if guess == secret {
            let order = self.turn_guesses.len();
            let points = guesser_points(self.draw_seconds_left, self.config.draw_seconds, order);
            self.players[player].score += points;
            self.turn_guesses.push(Guess {
                player,
                points,
                order,
            });
            self.push_chat(ChatKind::Correct, format!("🎉 {name} guessed the word!"));
            // End the turn early once every guesser has it.
            let guesser_count = self.players.len().saturating_sub(1);
            if self.turn_guesses.len() >= guesser_count {
                self.end_turn();
            }
            GuessOutcome::Correct
        } else if is_close(&guess, &secret) {
            // A near-miss is a PRIVATE nudge to the guesser only (bug #4) — it must
            // not leak a one-letter-off guess to the whole room.
            self.push_chat_to(Some(player), ChatKind::Close, "So close! 👀".to_string());
            GuessOutcome::Close
        } else {
            // A plain wrong guess is broadcast to the shared chat (name + literal
            // text) exactly as in the real game — everyone sees it.
            self.push_chat(ChatKind::Guess, format!("{name}: {raw}"));
            GuessOutcome::Wrong
        }
    }

    /// Move the human's controlled seat (the "playing as" switcher).
    pub fn switch_seat(&mut self, idx: usize) {
        if idx < self.players.len() {
            self.viewing_as = idx;
        }
    }

    /// The word as the *current* viewer should see it: the full word for the
    /// drawer / anyone who has guessed / during the reveal, else blanks + hints.
    pub fn word_display(&self) -> String {
        if !matches!(self.phase, Phase::Drawing | Phase::Reveal) {
            return String::new();
        }
        let seat = self.viewing_as;
        let knows = seat == self.seat_index
            || self.phase == Phase::Reveal
            || self.turn_guesses.iter().any(|g| g.player == seat);
        if knows {
            return self
                .secret_word
                .chars()
                .map(|c| c.to_ascii_uppercase())
                .collect::<Vec<_>>()
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ");
        }
        self.secret_word
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if *self.reveal_mask.get(i).unwrap_or(&false) {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The word as **per-letter slots** for the design's underlined word row
    /// (each slot: the uppercase letter if the current viewer should see it, else
    /// a blank). Same "who knows the word" logic as [`word_display`](Game::word_display):
    /// the drawer / anyone who has guessed / the reveal phase see every letter;
    /// others see only the hint-revealed positions. Empty outside Drawing/Reveal.
    pub fn word_slots(&self) -> Vec<WordSlot> {
        if !matches!(self.phase, Phase::Drawing | Phase::Reveal) {
            return Vec::new();
        }
        let seat = self.viewing_as;
        let knows = seat == self.seat_index
            || self.phase == Phase::Reveal
            || self.turn_guesses.iter().any(|g| g.player == seat);
        self.secret_word
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let revealed = knows || *self.reveal_mask.get(i).unwrap_or(&false);
                WordSlot {
                    ch: revealed.then(|| c.to_ascii_uppercase()),
                    revealed,
                }
            })
            .collect()
    }

    /// Force-advance out of the `Reveal` phase (the design's "Continue →" button;
    /// the reveal otherwise auto-advances on the [`REVEAL_SECS`] timeout). No-op
    /// in any other phase.
    pub fn continue_now(&mut self) {
        if self.phase == Phase::Reveal {
            self.advance_turn();
        }
    }

    /// The chat lines seat `seat` should see: every SHARED line plus the PRIVATE
    /// lines addressed to `seat`. The one home for chat visibility (bug #4) — the
    /// near-miss nudge is private, so it cannot leak to another seat.
    pub fn chat_for(&self, seat: usize) -> impl Iterator<Item = &ChatMsg> {
        self.chat
            .iter()
            .filter(move |m| m.to.is_none() || m.to == Some(seat))
    }

    /// How many guessers have guessed correctly THIS turn (live during Drawing —
    /// the drawer's "who has it" signal, bug #3). Zero outside a turn.
    pub fn guessed_count(&self) -> usize {
        self.turn_guesses.len()
    }

    /// Whether every guesser has guessed correctly this turn (the turn-end trigger,
    /// exposed live so the drawer knows when to stop, bug #3).
    pub fn all_guessed(&self) -> bool {
        let guesser_count = self.players.len().saturating_sub(1);
        guesser_count > 0 && self.turn_guesses.len() >= guesser_count
    }

    /// How many hint letters will reveal over this turn (the scheduled count for the
    /// current word — a machine-readable signal for the playtest host).
    pub fn hint_total(&self) -> usize {
        self.hint_reveal_at.len()
    }

    /// How many hint letters have revealed so far this turn.
    pub fn hints_revealed(&self) -> usize {
        self.reveal_mask.iter().filter(|r| **r).count()
    }

    /// The number of letters in the current word (visible to everyone — it is the
    /// count of blanks — `0` outside the draw / reveal phases).
    pub fn word_length(&self) -> usize {
        if matches!(self.phase, Phase::Drawing | Phase::Reveal) {
            self.secret_word.chars().count()
        } else {
            0
        }
    }

    /// The round number to DISPLAY (bug #1): clamped to `[1, total_rounds]` so it
    /// never reads as the post-increment overflow (`round == total + 1` on the
    /// transition to `Final`, which produced the podium's "Round 2/1"). The raw
    /// `round` field still overflows internally — it is the `Final` trigger — so
    /// this accessor is the single display-safe reading.
    pub fn round_display(&self) -> u32 {
        self.round.clamp(1, self.config.total_rounds.max(1))
    }

    /// The seat drawing THIS turn, or `None` when there is no active drawer (the
    /// match is over or not started) — bug #2. At `Final` the raw `seat_index` has
    /// wrapped back to a real seat, so reading it directly tagged a finished-match
    /// seat as "(drawing)". Callers must gate the drawer tag on this accessor.
    pub fn current_drawer(&self) -> Option<usize> {
        matches!(self.phase, Phase::Picking | Phase::Drawing | Phase::Reveal)
            .then_some(self.seat_index)
    }

    /// The name of the seat drawing this turn, or `None` when there is no active
    /// drawer (bug #2 — absent at the podium). Mirrors [`Game::current_drawer`].
    pub fn drawer_name(&self) -> Option<&str> {
        self.current_drawer()
            .and_then(|i| self.players.get(i))
            .map(|p| p.name.as_str())
    }

    /// A human-readable label for the current phase (minimal in-game header).
    pub fn phase_label(&self) -> &'static str {
        match self.phase {
            Phase::Idle => "Waiting",
            Phase::Picking => "Choosing a word",
            Phase::Drawing => "Drawing",
            Phase::Reveal => "Turn over",
            Phase::Final => "Match over",
        }
    }

    /// Whether the current viewer is the drawer this turn.
    pub fn viewer_is_drawer(&self) -> bool {
        self.viewing_as == self.seat_index
    }

    /// Final standings, highest score first (ties keep roster order).
    pub fn standings(&self) -> Vec<(usize, Player)> {
        let mut ranked: Vec<(usize, Player)> = self.players.iter().cloned().enumerate().collect();
        ranked.sort_by_key(|(_, p)| std::cmp::Reverse(p.score));
        ranked
    }

    // --- internals ----------------------------------------------------------

    /// Elapsed whole-seconds in the current phase, anchoring the clock on the
    /// first tick of the phase (so a `Msg`-triggered transition needs no `now`).
    fn elapsed_secs(&mut self, now: Duration) -> u64 {
        let anchor = match self.phase_started_at {
            Some(t) => t,
            None => {
                self.phase_started_at = Some(now);
                now
            }
        };
        now.saturating_sub(anchor).as_secs()
    }

    /// Reveal exactly the hints whose scheduled elapsed-time has passed.
    fn apply_reveal_mask(&mut self, elapsed: u64) {
        let count = self
            .hint_reveal_at
            .iter()
            .filter(|&&t| t <= elapsed)
            .count();
        // Rebuild the mask idempotently so a steady frame reproduces it exactly.
        let mut mask = vec![false; self.secret_word.chars().count()];
        for &pos in self.hint_positions.iter().take(count) {
            if pos < mask.len() {
                mask[pos] = true;
            }
        }
        self.reveal_mask = mask;
    }

    /// Bots whose fire time has passed and who haven't guessed yet (and aren't
    /// the human's seat or the drawer). Idempotent via `turn_guesses` membership:
    /// once a bot's correct guess folds, it is locked and won't re-fire. Disabled
    /// wholesale by `config.bots_enabled == false` (the W8 all-agents playtest).
    fn due_bot_guesses(&self, elapsed: u64) -> Vec<PendingGuess> {
        if !self.config.bots_enabled {
            return Vec::new();
        }
        self.bot_plans
            .iter()
            .filter(|p| {
                p.fire_at <= elapsed
                    && p.player != self.seat_index
                    && p.player != self.viewing_as
                    && !self.turn_guesses.iter().any(|g| g.player == p.player)
            })
            .map(|p| PendingGuess {
                player: p.player,
                text: self.secret_word.clone(),
            })
            .collect()
    }

    /// Score the drawer, build the reveal rows, and enter `Reveal`.
    fn end_turn(&mut self) {
        if self.phase != Phase::Drawing {
            return;
        }
        let drawer = self.seat_index;
        let guesser_count = self.players.len().saturating_sub(1);
        let correct = self.turn_guesses.len();
        let drawer_pts = drawer_points(correct, guesser_count);
        self.players[drawer].score += drawer_pts;

        let results = self
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let delta = if i == drawer {
                    drawer_pts
                } else {
                    self.turn_guesses
                        .iter()
                        .find(|g| g.player == i)
                        .map(|g| g.points)
                        .unwrap_or(0)
                };
                TurnResult {
                    player: i,
                    name: p.name.clone(),
                    delta,
                    total: p.score,
                }
            })
            .collect();
        self.turn_results = results;
        self.phase = Phase::Reveal;
        self.phase_started_at = None;
        self.reveal_seconds_left = self.config.reveal_seconds;
        let word = self.secret_word.to_uppercase();
        self.push_chat(ChatKind::System, format!("The word was \"{word}\""));
    }

    /// Rotate to the next drawer / round, or finish the match.
    fn advance_turn(&mut self) {
        if self.phase != Phase::Reveal {
            return;
        }
        self.seat_index += 1;
        if self.seat_index >= self.players.len() {
            self.seat_index = 0;
            self.round += 1;
        }
        if self.round > self.config.total_rounds {
            self.phase = Phase::Final;
            self.phase_started_at = None;
        } else {
            self.begin_turn();
        }
    }

    /// Three distinct fresh words (no-repeat until the pool is nearly exhausted).
    fn pick_word_choices(&mut self) -> Vec<String> {
        if self.used_words.len() >= WORDS.len().saturating_sub(3) {
            self.used_words.clear();
        }
        let mut pool: Vec<String> = WORDS
            .iter()
            .filter(|w| !self.used_words.iter().any(|u| u == *w))
            .map(|w| w.to_string())
            .collect();
        let mut choices = Vec::new();
        for _ in 0..3 {
            if pool.is_empty() {
                break;
            }
            let idx = (next_rng(&mut self.rng) as usize) % pool.len();
            choices.push(pool.swap_remove(idx));
        }
        choices
    }

    /// A seeded permutation of the word's letter positions, truncated to the hint
    /// count — the order in which blanks flip open.
    fn seeded_hint_positions(&mut self, word: &str, hint_count: usize) -> Vec<usize> {
        let mut positions: Vec<usize> = (0..word.chars().count()).collect();
        // Fisher–Yates with the carried PRNG.
        for i in (1..positions.len()).rev() {
            let j = (next_rng(&mut self.rng) as usize) % (i + 1);
            positions.swap(i, j);
        }
        positions.truncate(hint_count);
        positions
    }

    /// One correct-guess plan per non-drawer seat, fired at a seeded moment in the
    /// `[0.25, 0.75]` band of the draw window (staggered so guess orders differ).
    fn seeded_bot_plans(&mut self, total: u64) -> Vec<BotPlan> {
        let mut plans = Vec::new();
        for player in 0..self.players.len() {
            if player == self.seat_index {
                continue;
            }
            // Fire time in [0.25·total, 0.75·total], seeded per bot.
            let r = (next_rng(&mut self.rng) % 1000) as f64 / 1000.0;
            let fire_at = ((0.25 + 0.5 * r) * total as f64).floor() as u64;
            plans.push(BotPlan { player, fire_at });
        }
        // Fire in time order so the earliest bot gets guess order 0 (best points).
        plans.sort_by_key(|p| p.fire_at);
        plans
    }

    /// Push a SHARED chat line (broadcast to every seat).
    fn push_chat(&mut self, kind: ChatKind, text: String) {
        self.push_chat_to(None, kind, text);
    }

    /// Push a chat line visible only to `to` (`None` ⇒ shared). The near-miss nudge
    /// is the sole private caller — see [`Game::chat_for`] for the read side.
    fn push_chat_to(&mut self, to: Option<usize>, kind: ChatKind, text: String) {
        let seq = self.chat_seq;
        self.chat_seq += 1;
        self.chat.push(ChatMsg {
            seq,
            kind,
            text,
            to,
        });
    }
}
