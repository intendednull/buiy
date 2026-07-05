//! The pure Dooduel game core — the match state machine, scoring, guess matching
//! and deterministic bots, with **zero framework coupling**.
//!
//! Everything here is a plain data type + pure `&mut self` methods, so the whole
//! game is unit-testable without an ECS, a GPU, or a clock. The MVU wiring in
//! the `dooduel` app crate's `lib.rs` is a thin shell: the reducer folds a `Msg`
//! into a call on one of these methods; the tick driver turns wall-clock into a
//! `Tick(now)` message.
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

use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};

// --- Tunable rules (durations pinned by the Dooduel game spec) --------------

/// Seconds the drawer has to pick a word before one is auto-chosen.
pub const PICK_SECS: u64 = 10;
/// Seconds the turn-end reveal card stays up before the next turn.
pub const REVEAL_SECS: u64 = 6;
/// The three built-in bot guessers' names (the human is seat 0).
pub const PRESET_NAMES: [&str; 3] = ["Priya", "Theo", "Sam"];

/// The default match PRNG seed used by [`Game::start_match`] (the solo demo / capture /
/// playtest paths). The networked authority injects a per-match seed instead (spec §4.1
/// — a hardcoded seed makes word choices predictable from public info).
pub const DEFAULT_MATCH_SEED: u64 = 0xD00D_0000_0000_0001;

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
    /// Whether this seat is a bot. The built-in guesser machinery
    /// (`seeded_bot_plans` / the due-guess drain) keys off this, replacing the
    /// removed hot-seat `viewing_as` guard (spec §2.3.2).
    pub is_bot: bool,
    /// Whether this seat is currently held. A seat freed past grace is marked
    /// **vacant** (not removed — indices are identity for rotation, `turn_guesses`,
    /// and private chat `to`); rotation skips vacant seats and the match ends when
    /// fewer than two seats remain occupied (spec §2.3.3).
    pub occupied: bool,
}

/// A roster entry for [`Game::start_match`]: a display name + whether the seat is a
/// bot. The server builds this from real connections; [`Game::start_match_solo`]
/// builds the solo `[human, Priya(bot), Theo(bot), Sam(bot)]` roster.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSpec {
    pub name: String,
    pub is_bot: bool,
}

/// The in-turn phase machine. `Idle` = no match running; `Final` = match over
/// (the shell lifts this to `Screen::Podium`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct TurnResult {
    pub player: usize,
    pub name: String,
    pub delta: i64,
    pub total: i64,
}

/// Where a chat line came from (drives its styling in later waves).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
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
    /// Start a fresh match from an explicit `roster` (seat order = roster order):
    /// every seat is marked occupied; scores/round/used-words reset; the first turn
    /// begins. The server builds the roster from real connections;
    /// [`Game::start_match_solo`] is the solo/verification shim (spec §2.3.1).
    pub fn start_match(&mut self, roster: &[PlayerSpec], config: Config) {
        self.start_match_with_seed(roster, config, DEFAULT_MATCH_SEED);
    }

    /// Start a match with an explicit PRNG `seed` (spec §4.1: the seed is secret — a
    /// hardcoded constant makes word choices predictable from public info, so the
    /// networked authority injects a per-match seed via `SessionOpts`). The seed is
    /// guarded away from the degenerate zero-state (`seed.max(1)`). `start_match`
    /// delegates here with [`DEFAULT_MATCH_SEED`] so the solo demo / capture /
    /// playtest paths stay reproducible turn-for-turn.
    pub fn start_match_with_seed(&mut self, roster: &[PlayerSpec], config: Config, seed: u64) {
        self.players = roster
            .iter()
            .map(|spec| Player {
                name: spec.name.clone(),
                score: 0,
                is_bot: spec.is_bot,
                occupied: true,
            })
            .collect();
        self.config = config;
        self.round = 1;
        self.seat_index = 0;
        self.chat.clear();
        self.chat_seq = 0;
        self.used_words.clear();
        self.rng = seed.max(1);
        self.begin_turn();
    }

    /// The solo / in-process verification path (spec §8): seat 0 is the human
    /// (`human_name`, defaulting to "You" when blank), seats 1..=3 the
    /// [`PRESET_NAMES`] bots. Delegates to [`Game::start_match`].
    pub fn start_match_solo(&mut self, human_name: &str, config: Config) {
        let name = if human_name.trim().is_empty() {
            "You".to_string()
        } else {
            human_name.trim().to_string()
        };
        let mut roster = vec![PlayerSpec {
            name,
            is_bot: false,
        }];
        for n in PRESET_NAMES {
            roster.push(PlayerSpec {
                name: n.to_string(),
                is_bot: true,
            });
        }
        self.start_match(&roster, config);
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
        // The drawer cannot guess; a vacated seat cannot guess; nobody guesses twice.
        if player == self.seat_index
            || !self.players[player].occupied
            || self.turn_guesses.iter().any(|g| g.player == player)
        {
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
            // End the turn early once every occupied guesser has it (membership,
            // not counts — a guessed-then-vacated seat must not shrink the bar for
            // the players still here).
            if self.all_guessed() {
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

    /// The redaction predicate (spec §5.1, three-way): whether `seat` may see the
    /// secret word this turn — the drawer, anyone during the reveal/final, or a
    /// seat that has already guessed correctly. The single home `Session` calls per
    /// recipient (a correct guesser's [`word_display_for`](Game::word_display_for)
    /// upgrades to the full word mid-turn).
    pub fn knows(&self, seat: usize) -> bool {
        seat == self.seat_index
            || matches!(self.phase, Phase::Reveal | Phase::Final)
            || self.turn_guesses.iter().any(|g| g.player == seat)
    }

    /// The word as `seat` should see it: the full uppercase word when
    /// [`knows`](Game::knows)`(seat)`, else blanks with the hint-revealed letters
    /// shown. Space-joined (the design's underlined row). Empty outside
    /// Drawing/Reveal (there is no word row then). The single redaction home
    /// (spec §2.3.5).
    pub fn word_display_for(&self, seat: usize) -> String {
        if !matches!(self.phase, Phase::Drawing | Phase::Reveal) {
            return String::new();
        }
        if self.knows(seat) {
            return self
                .secret_word
                .chars()
                .map(|c| c.to_ascii_uppercase().to_string())
                .collect::<Vec<_>>()
                .join(" ");
        }
        self.secret_word
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if *self.reveal_mask.get(i).unwrap_or(&false) {
                    c.to_ascii_uppercase().to_string()
                } else {
                    "_".to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
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

    /// Whether every occupied guesser has guessed correctly this turn (the turn-end
    /// trigger, exposed live so the drawer knows when to stop, bug #3). Membership,
    /// not counts: every OCCUPIED non-drawer seat must itself appear in
    /// `turn_guesses` — a guessed-then-vacated seat's entry neither counts for the
    /// players still here nor blocks them (W1-review IMPORTANT-1).
    pub fn all_guessed(&self) -> bool {
        let mut any = false;
        for (i, p) in self.players.iter().enumerate() {
            if i == self.seat_index || !p.occupied {
                continue;
            }
            if !self.turn_guesses.iter().any(|g| g.player == i) {
                return false;
            }
            any = true;
        }
        any
    }

    /// How many seats are currently occupied (spec §2.3.3). The match ends when
    /// this drops below 2.
    pub fn occupied_count(&self) -> usize {
        self.players.iter().filter(|p| p.occupied).count()
    }

    /// The number of occupied non-drawer seats — the guessers who can score this
    /// turn (spec §2.3.3). With every seat occupied this is `players.len() - 1`.
    pub fn guesser_count(&self) -> usize {
        self.players
            .iter()
            .enumerate()
            .filter(|(i, p)| *i != self.seat_index && p.occupied)
            .count()
    }

    /// Free `seat` — a departure past grace: marks it vacant without removing it,
    /// so seat indices stay stable for rotation / `turn_guesses` / private chat
    /// `to` (spec §2.3.3). No-op on an out-of-range seat.
    ///
    /// Re-evaluates the early-turn-end trigger (W1-review IMPORTANT-2): when the
    /// last not-yet-guessed guesser leaves mid-`Drawing`, the players still here
    /// have all guessed — the turn ends now, not at the draw-window timeout.
    /// (The drawer-drop case is the caller's `force_end_turn`, spec §2.3.4.)
    pub fn vacate_seat(&mut self, seat: usize) {
        if let Some(p) = self.players.get_mut(seat) {
            p.occupied = false;
        }
        if self.phase == Phase::Drawing && self.all_guessed() {
            self.end_turn();
        }
    }

    /// Force the current turn to end — the drawer-drop-past-grace path (spec
    /// §2.3.4). A public wrapper over the private turn-end; scores the drawer,
    /// builds the reveal rows, and enters `Reveal`. No-op outside `Drawing`.
    pub fn force_end_turn(&mut self) {
        self.end_turn();
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

    /// Bots (`is_bot` seats) whose fire time has passed and who haven't guessed yet
    /// (and aren't the drawer). Idempotent via `turn_guesses` membership: once a
    /// bot's correct guess folds, it is locked and won't re-fire. Disabled wholesale
    /// by `config.bots_enabled == false` (the all-agents playtest). The removed
    /// `viewing_as` guard was the hot-seat filter (spec §2.3.2) — bots are now keyed
    /// purely off `is_bot`, so a human seat is never auto-guessed.
    fn due_bot_guesses(&self, elapsed: u64) -> Vec<PendingGuess> {
        if !self.config.bots_enabled {
            return Vec::new();
        }
        self.bot_plans
            .iter()
            .filter(|p| {
                p.fire_at <= elapsed
                    && p.player != self.seat_index
                    && self.players.get(p.player).is_some_and(|pl| pl.is_bot)
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
        let guesser_count = self.guesser_count();
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

    /// Rotate to the next OCCUPIED drawer / round, or finish the match. The match
    /// ends when occupancy drops below 2 (nobody left to play against) or the round
    /// counter passes `total_rounds`; vacant seats are skipped in the rotation
    /// (spec §2.3.3).
    fn advance_turn(&mut self) {
        if self.phase != Phase::Reveal {
            return;
        }
        if self.occupied_count() < 2 {
            self.phase = Phase::Final;
            self.phase_started_at = None;
            return;
        }
        // Advance to the next occupied seat, incrementing the round on wrap. The
        // `occupied_count() >= 2` guard above bounds this loop (an occupied seat is
        // always within one full cycle); the iteration cap is a defensive backstop.
        for _ in 0..=self.players.len() {
            self.seat_index += 1;
            if self.seat_index >= self.players.len() {
                self.seat_index = 0;
                self.round += 1;
            }
            if self.round > self.config.total_rounds {
                self.phase = Phase::Final;
                self.phase_started_at = None;
                return;
            }
            if self
                .players
                .get(self.seat_index)
                .is_some_and(|p| p.occupied)
            {
                self.begin_turn();
                return;
            }
        }
        self.phase = Phase::Final;
        self.phase_started_at = None;
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
            // Only bot seats get an auto-guess plan (spec §2.3.2); the drawer never
            // guesses. A human seat is skipped, so a 2-human roster plans nothing.
            if player == self.seat_index || !self.players[player].is_bot {
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- Pure game-logic unit tests (no ECS) -------------------------------
    // Extracted from `apps/dooduel/src/lib.rs`'s test module when the pure core
    // moved to `dooduel_core` (M1 W0.2). Everything here uses only `Game`/`Config`
    // + the pure free fns — no App/probe. The probe/reducer-level tests stay in
    // `apps/dooduel`.

    fn started() -> Game {
        let mut g = Game::default();
        g.start_match_solo("Mara", Config::default());
        g
    }

    /// Fold `Tick`s into `g` at 1-second virtual steps from `from` to `to`
    /// (inclusive), draining bot guesses through `apply_guess` like the funnel.
    fn tick_to(g: &mut Game, from: u64, to: u64) {
        for sec in from..=to {
            let pending = g.tick(Duration::from_secs(sec));
            for p in pending {
                g.apply_guess(p.player, &p.text);
            }
        }
    }

    #[test]
    fn normalize_strips_and_lowercases() {
        assert_eq!(normalize("  Ro-BOT! "), "robot");
        assert_eq!(normalize("Ice Cream"), "icecream");
    }

    #[test]
    fn close_matches_within_edit_distance_two() {
        assert!(is_close(&normalize("robott"), "robot")); // one insert
        assert!(is_close("robto", "robot")); // adjacent swap = distance 2
        assert!(!is_close("robot", "robot")); // identical is not "close"
        assert!(!is_close("banana", "robot")); // far apart
    }

    #[test]
    fn guesser_points_match_the_spec_formula() {
        // Full time left, first guesser: round(50 + 450) = 500.
        assert_eq!(guesser_points(80, 80, 0), 500);
        // Half time, first guesser: round(50 + 225) = 275.
        assert_eq!(guesser_points(40, 80, 0), 275);
        // Full time, second guesser: round(500 * 0.82) = 410.
        assert_eq!(guesser_points(80, 80, 1), 410);
        // Floor at 20 even with no time / high order.
        assert_eq!(guesser_points(0, 80, 5), 20);
    }

    #[test]
    fn drawer_points_scale_with_correct_fraction() {
        assert_eq!(drawer_points(3, 3), 100);
        assert_eq!(drawer_points(2, 3), 67); // round(200/3)
        assert_eq!(drawer_points(0, 3), 0);
    }

    #[test]
    fn match_starts_in_pick_phase_with_four_players() {
        let g = started();
        assert_eq!(g.players.len(), 4);
        assert_eq!(g.players[0].name, "Mara");
        assert_eq!(g.phase, Phase::Picking);
        assert_eq!(g.round, 1);
        assert_eq!(g.seat_index, 0, "seat 0 draws turn 1");
        assert_eq!(g.word_choices.len(), 3);
    }

    #[test]
    fn pick_timeout_auto_advances_to_drawing() {
        let mut g = started();
        tick_to(&mut g, 0, PICK_SECS);
        assert_eq!(g.phase, Phase::Drawing);
        assert!(!g.secret_word.is_empty(), "a word was auto-picked");
        assert_eq!(g.draw_seconds_left, g.config.draw_seconds);
    }

    #[test]
    fn choosing_a_word_starts_the_draw_countdown() {
        let mut g = started();
        // The drawer picks the first offered word before the pick timer expires.
        g.choose_word(g.word_choices[0].clone());
        assert_eq!(g.phase, Phase::Drawing);
        // First tick anchors the clock; countdown ticks down per second.
        g.tick(Duration::from_secs(0));
        assert_eq!(g.draw_seconds_left, g.config.draw_seconds);
        g.tick(Duration::from_secs(5));
        assert_eq!(g.draw_seconds_left, g.config.draw_seconds - 5);
    }

    #[test]
    fn hints_reveal_on_the_spec_schedule() {
        let mut g = started();
        g.choose_word("robot".to_string()); // 5 letters, 2 hints
        g.tick(Duration::from_secs(0)); // anchor
        // Thresholds (total 80): hint1 at 33s-left (elapsed 47), hint2 at 19s-left
        // (elapsed 61). Before 47s: zero hints revealed.
        g.tick(Duration::from_secs(46));
        assert_eq!(g.reveal_mask.iter().filter(|b| **b).count(), 0);
        g.tick(Duration::from_secs(47));
        assert_eq!(g.reveal_mask.iter().filter(|b| **b).count(), 1);
        g.tick(Duration::from_secs(61));
        assert_eq!(g.reveal_mask.iter().filter(|b| **b).count(), 2);
    }

    #[test]
    fn a_correct_human_guess_scores_and_locks() {
        let mut g = started();
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0)); // anchor, 80s left
        // Seat 1 (a guesser) submits the word: full-ish time, order 0.
        let outcome = g.apply_guess(1, "ROBOT!");
        assert_eq!(outcome, GuessOutcome::Correct);
        assert_eq!(g.turn_guesses.len(), 1);
        assert_eq!(g.players[1].score, 500);
        // Guessing again is ignored (already locked).
        assert_eq!(g.apply_guess(1, "robot"), GuessOutcome::Ignored);
    }

    #[test]
    fn the_drawer_cannot_guess() {
        let mut g = started();
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        // Seat 0 is the drawer this turn.
        assert_eq!(g.apply_guess(0, "robot"), GuessOutcome::Ignored);
        assert!(g.turn_guesses.is_empty());
    }

    #[test]
    fn near_miss_reports_close_without_scoring() {
        let mut g = started();
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        assert_eq!(g.apply_guess(1, "robott"), GuessOutcome::Close);
        assert_eq!(g.players[1].score, 0);
        assert!(g.turn_guesses.is_empty());
    }

    #[test]
    fn all_guessers_correct_ends_the_turn_early() {
        let mut g = started();
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        g.apply_guess(1, "robot");
        g.apply_guess(2, "robot");
        assert_eq!(g.phase, Phase::Drawing);
        g.apply_guess(3, "robot"); // the 3rd (last) guesser
        assert_eq!(g.phase, Phase::Reveal, "turn ends once everyone has it");
        // Drawer scored 100 (all 3 guessers correct).
        assert_eq!(g.players[0].score, 100);
    }

    /// Drive one virtual second: fold `Tick(sec)` and apply any due bot guesses.
    fn one_second(g: &mut Game, sec: u64) {
        let pending = g.tick(Duration::from_secs(sec));
        for p in pending {
            g.apply_guess(p.player, &p.text);
        }
    }

    /// Auto-pick each turn and run the clock until the match finishes.
    fn drive_to_final(g: &mut Game) {
        let mut sec = 0u64;
        let mut guard = 0;
        loop {
            if g.phase == Phase::Picking {
                g.choose_word(g.word_choices[0].clone());
                sec = 0;
            }
            if g.phase == Phase::Final {
                break;
            }
            one_second(g, sec);
            sec += 1;
            guard += 1;
            assert!(guard < 10_000, "match should terminate");
        }
    }

    #[test]
    fn bots_drive_the_turn_to_reveal_on_their_own() {
        let mut g = started();
        g.choose_word("robot".to_string());
        // Tick until the turn leaves the draw phase; the seeded bots guess along
        // the way and the third correct guess ends the turn early.
        let mut sec = 0u64;
        while g.phase == Phase::Drawing {
            one_second(&mut g, sec);
            sec += 1;
            assert!(sec < 200, "turn should end");
        }
        assert_eq!(g.phase, Phase::Reveal);
        assert_eq!(g.turn_guesses.len(), 3, "all three bots guessed");
        assert_eq!(g.players[0].score, 100, "drawer scored the full 100");
    }

    #[test]
    fn turn_rotation_and_match_end_reach_the_podium() {
        let mut g = started();
        drive_to_final(&mut g);
        assert_eq!(g.phase, Phase::Final, "the match reaches its end");
        // Every player accrued some score across 8 turns of drawing + guessing.
        assert!(g.players.iter().all(|p| p.score > 0));
    }

    #[test]
    fn determinism_same_seed_same_match() {
        let mut a = started();
        let mut b = started();
        let total = a.config.draw_seconds;
        for _ in 0..3 {
            if a.phase == Phase::Picking {
                let wa = a.word_choices[0].clone();
                let wb = b.word_choices[0].clone();
                assert_eq!(wa, wb, "same seeded word choices");
                a.choose_word(wa);
                b.choose_word(wb);
            }
            tick_to(&mut a, 0, total);
            tick_to(&mut b, 0, total);
            assert_eq!(a, b, "identical Msg streams produce identical state");
            if a.phase == Phase::Reveal {
                tick_to(&mut a, 0, REVEAL_SECS);
                tick_to(&mut b, 0, REVEAL_SECS);
            }
        }
    }

    // --- Playtest-found gameplay-bug regression tests (pure game core) ------

    /// Bug #1 — the round counter must never display the post-increment overflow.
    /// The raw `round` field intentionally overflows to `total + 1` on the
    /// transition to `Final` (it IS the Final trigger), which produced the podium's
    /// "Round 2/1"; `round_display()` clamps the reading to `[1, total]`.
    #[test]
    fn round_display_clamps_the_final_overflow() {
        let mut g = started();
        let total = g.config.total_rounds;
        drive_to_final(&mut g);
        assert_eq!(g.phase, Phase::Final);
        assert!(
            g.round > total,
            "raw round ({}) is the overflowed Final trigger past total ({total})",
            g.round
        );
        assert_eq!(
            g.round_display(),
            total,
            "round_display() must clamp the Final overflow (the podium 'Round 2/1' bug)"
        );
        assert!(g.round_display() <= total);
    }

    /// Bug #1 — `total_rounds` is authoritative from the start config, not a stale
    /// default; the first round displays as 1.
    #[test]
    fn total_rounds_comes_from_the_start_config() {
        let mut g = Game::default();
        let cfg = Config {
            total_rounds: 5,
            ..Config::default()
        };
        g.start_match_solo("Mara", cfg);
        assert_eq!(
            g.config.total_rounds, 5,
            "start_match_solo sets total_rounds from the config"
        );
        assert_eq!(g.round_display(), 1, "the first round displays as 1");
    }

    /// Bug #2 — at the podium no seat is the active drawer (the raw `seat_index`
    /// wraps back to a real seat at `Final`, which tagged a finished-match seat as
    /// "(drawing)" and left a stale drawer name).
    #[test]
    fn no_active_drawer_once_the_match_is_over() {
        let mut g = started();
        assert_eq!(
            g.current_drawer(),
            Some(0),
            "mid-match there is an active drawer"
        );
        assert_eq!(g.drawer_name(), Some("Mara"));
        drive_to_final(&mut g);
        assert_eq!(g.phase, Phase::Final);
        assert_eq!(
            g.current_drawer(),
            None,
            "no seat draws once the match is over (the stale-drawer podium bug)"
        );
        assert_eq!(g.drawer_name(), None, "the podium shows no drawer name");
    }

    /// Bug #3 — the drawer is not blind: `guessed_count()` / `all_guessed()` track
    /// correct guessers LIVE during the draw phase (so the drawer knows when the
    /// word is already fully guessed — the wasted-redraw bug).
    #[test]
    fn guessed_count_tracks_correct_guessers_live_during_drawing() {
        let mut g = started();
        g.config.bots_enabled = false;
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        assert_eq!(g.guessed_count(), 0);
        assert!(!g.all_guessed());
        g.apply_guess(1, "robot");
        assert_eq!(
            g.guessed_count(),
            1,
            "the drawer sees one guesser is done mid-draw"
        );
        assert!(!g.all_guessed());
        g.apply_guess(2, "robot");
        assert_eq!(g.guessed_count(), 2);
        assert_eq!(
            g.phase,
            Phase::Drawing,
            "still drawing — the count is visible in-phase"
        );
        // The "guessed" announcement folds into the shared chat immediately (in-phase).
        let announced = g
            .chat_for(g.seat_index)
            .filter(|m| m.kind == ChatKind::Correct)
            .count();
        assert_eq!(
            announced, 2,
            "the drawer sees both 'guessed the word!' lines in-phase"
        );
    }

    /// Bug #4 — the design-exact three-way: a WRONG guess broadcasts to the shared
    /// chat (name + literal text, everyone), a NEAR-MISS fires a PRIVATE nudge only
    /// the guesser sees, and an EXACT guess announces to all while the word stays
    /// hidden from those who have not guessed.
    #[test]
    fn near_miss_nudge_is_private_and_wrong_guesses_are_shared() {
        let mut g = started();
        g.config.bots_enabled = false;
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        // Seat 1 (Priya) near-misses; seat 2 (Theo) makes a plain wrong guess.
        assert_eq!(g.apply_guess(1, "robott"), GuessOutcome::Close);
        assert_eq!(g.apply_guess(2, "banana"), GuessOutcome::Wrong);

        // The WRONG guess is broadcast — every seat sees "Theo: banana".
        for seat in 0..4 {
            let sees = g
                .chat_for(seat)
                .any(|m| m.text.contains("Theo") && m.text.contains("banana"));
            assert!(
                sees,
                "seat {seat} must see the shared wrong guess (name + literal)"
            );
        }
        // The NEAR-MISS nudge is private to the guesser (seat 1) only.
        assert!(
            g.chat_for(1).any(|m| m.text.contains("So close")),
            "the guesser sees the private 'So close' nudge"
        );
        for other in [0usize, 2, 3] {
            assert!(
                !g.chat_for(other).any(|m| m.text.contains("So close")),
                "seat {other} must NOT see seat 1's private near-miss nudge"
            );
        }
    }

    /// Bug #4 — an exact guess announces to everyone but keeps the word hidden from
    /// non-guessers (word redaction has one home, `word_display_for`).
    #[test]
    fn exact_guess_announces_but_hides_the_word_from_non_guessers() {
        let mut g = started();
        g.config.bots_enabled = false;
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        assert_eq!(g.apply_guess(1, "robot"), GuessOutcome::Correct);
        assert!(
            g.chat_for(0)
                .any(|m| m.kind == ChatKind::Correct && m.text.contains("guessed the word")),
            "the exact guess announces to everyone"
        );
        assert!(
            g.word_display_for(1).contains('R'),
            "the correct guesser sees the letters"
        );
        assert!(
            !g.word_display_for(2).contains('R') && g.word_display_for(2).contains('_'),
            "a non-guesser still sees blanks"
        );
    }

    /// Bug #5 — the hint reveal fires on the THRESHOLD CROSSING (not equality, so a
    /// poll that skips the exact second still latches the reveal), at random,
    /// previously-unrevealed `[a-z]` positions, never exceeding
    /// `min(hint_count, letters − 1)`.
    #[test]
    fn hints_reveal_on_the_crossing_at_random_unrevealed_positions() {
        let mut g = started();
        g.config.bots_enabled = false; // let the full draw window run (bots preempted it live)
        g.choose_word("robot".to_string()); // 5 letters ⇒ cap = min(2, 4) = 2
        g.tick(Duration::from_secs(0)); // anchor
        let cap = g.config.hint_count.min("robot".len() - 1);
        assert_eq!(cap, 2);
        let revealed = |g: &Game| g.reveal_mask.iter().filter(|b| **b).count();

        // Thresholds are 33s-left (elapsed 47) and 19s-left (elapsed 61). Poll SKIPS
        // the exact threshold seconds; crossing semantics must still fire each hint.
        g.tick(Duration::from_secs(46));
        assert_eq!(
            revealed(&g),
            0,
            "nothing revealed before the first threshold"
        );
        g.tick(Duration::from_secs(48)); // skipped 47 — crossing must fire hint 1
        assert_eq!(
            revealed(&g),
            1,
            "crossing latches hint 1 even when the poll skips the second"
        );
        g.tick(Duration::from_secs(60));
        assert_eq!(revealed(&g), 1);
        g.tick(Duration::from_secs(62)); // skipped 61 — crossing must fire hint 2
        assert_eq!(revealed(&g), 2, "crossing latches hint 2");
        g.tick(Duration::from_secs(79)); // ticking on never exceeds the cap
        assert!(
            revealed(&g) <= cap,
            "never exceeds min(hint_count, letters − 1)"
        );

        // The revealed positions are distinct, in range, and real [a-z] letters.
        let positions: Vec<usize> = g
            .reveal_mask
            .iter()
            .enumerate()
            .filter(|(_, r)| **r)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(positions.len(), 2);
        let word: Vec<char> = "robot".chars().collect();
        for p in &positions {
            assert!(
                word[*p].is_ascii_lowercase(),
                "a revealed position is an [a-z] letter"
            );
        }
        assert_ne!(
            positions[0], positions[1],
            "the two revealed positions are distinct"
        );
    }

    /// Bug #5 — determinism holds: the seeded reveal positions replay byte-identical
    /// (the hint randomness comes from the carried splitmix64 stream, not wall-clock).
    #[test]
    fn hint_positions_are_deterministic_across_replays() {
        let mask_after = |secs: u64| {
            let mut g = started();
            g.config.bots_enabled = false;
            g.choose_word("robot".to_string());
            g.tick(Duration::from_secs(0));
            g.tick(Duration::from_secs(secs));
            g.reveal_mask.clone()
        };
        assert_eq!(
            mask_after(62),
            mask_after(62),
            "seeded hint positions replay identically"
        );
    }

    /// Bug #6 — the draw countdown is derived from WALL-CLOCK (`now − anchor`), not
    /// a frame/tick count: successive whole-second `now`s decrement it 1-per-second
    /// and a sub-second re-poll (same whole second) does not move it.
    #[test]
    fn draw_countdown_decrements_one_per_wall_second() {
        let mut g = started();
        g.config.bots_enabled = false;
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        assert_eq!(g.draw_seconds_left, g.config.draw_seconds);
        g.tick(Duration::from_millis(500)); // same whole second — a steady frame
        assert_eq!(
            g.draw_seconds_left, g.config.draw_seconds,
            "a sub-second re-poll does not move the countdown"
        );
        for s in 1..=5 {
            g.tick(Duration::from_secs(s));
            assert_eq!(
                g.draw_seconds_left,
                g.config.draw_seconds - s,
                "one wall-second decrements the countdown by exactly one"
            );
        }
    }

    // --- W1.3 Game API delta (M1 spec §2.3) --------------------------------

    /// Build a match from an explicit roster (the server's entry point) —
    /// `(name, is_bot)` pairs.
    fn started_roster(specs: &[(&str, bool)]) -> Game {
        let roster: Vec<PlayerSpec> = specs
            .iter()
            .map(|(n, bot)| PlayerSpec {
                name: n.to_string(),
                is_bot: *bot,
            })
            .collect();
        let mut g = Game::default();
        g.start_match(&roster, Config::default());
        g
    }

    /// §2.3.1/§2.3.2 — the roster-parameterized start seats real players, and a
    /// roster with NO bots never fires an auto-guess (the removed hot-seat guard
    /// would have auto-guessed a human seat).
    #[test]
    fn a_two_human_roster_never_bot_guesses() {
        let mut g = started_roster(&[("Ada", false), ("Bo", false)]);
        assert_eq!(g.players.len(), 2);
        assert!(g.players.iter().all(|p| !p.is_bot && p.occupied));
        g.choose_word("robot".to_string());
        // Across the whole draw window no bot guess is ever due.
        for sec in 0..=g.config.draw_seconds {
            let pending = g.tick(Duration::from_secs(sec));
            assert!(
                pending.is_empty(),
                "a 2-human roster must never bot-guess (elapsed {sec})"
            );
        }
    }

    /// §2.3.2 — the solo shim keeps its 1-human + 3-bot roster (regression guard on
    /// the `is_bot` switch).
    #[test]
    fn solo_shim_seats_one_human_and_three_bots() {
        let g = started();
        assert_eq!(g.players.len(), 4);
        assert!(!g.players[0].is_bot, "seat 0 is the human");
        assert!(
            g.players[1..].iter().all(|p| p.is_bot),
            "seats 1..=3 are bots"
        );
        assert!(
            g.players.iter().all(|p| p.occupied),
            "every seat is occupied"
        );
    }

    /// §2.3.3 — rotation skips a vacant seat.
    #[test]
    fn rotation_skips_a_vacant_seat() {
        let mut g = started(); // seat 0 draws turn 1
        g.config.bots_enabled = false;
        g.choose_word("robot".to_string());
        g.vacate_seat(1); // the next drawer in rotation
        g.force_end_turn();
        assert_eq!(g.phase, Phase::Reveal);
        g.continue_now(); // advance the rotation
        assert_eq!(g.phase, Phase::Picking);
        assert_eq!(
            g.seat_index, 2,
            "rotation skipped the vacant seat 1 (0 → 2)"
        );
    }

    /// W1-review IMPORTANT-1 (§2.3.3): a guessed-then-vacated seat must not lower
    /// the early-end bar — the turn ends only when every seat STILL HERE has it.
    #[test]
    fn a_guessed_then_vacated_seat_does_not_end_the_turn_early() {
        let mut g = started_roster(&[("A", false), ("B", false), ("C", false), ("D", false)]);
        g.choose_word(g.word_choices[0].clone());
        g.tick(Duration::from_secs(0));
        let secret = g.secret_word.clone();
        assert_eq!(g.apply_guess(1, &secret), GuessOutcome::Correct);
        g.vacate_seat(1);
        assert_eq!(g.apply_guess(2, &secret), GuessOutcome::Correct);
        assert_eq!(
            g.phase,
            Phase::Drawing,
            "occupied seat 3 has not guessed — the vacated seat's entry must not end the turn"
        );
        assert_eq!(g.apply_guess(3, &secret), GuessOutcome::Correct);
        assert_eq!(g.phase, Phase::Reveal, "everyone still here has it now");
    }

    /// W1-review IMPORTANT-2 (§2.3.3): when the last not-yet-guessed guesser
    /// vacates, the turn ends immediately — not at the draw-window timeout.
    #[test]
    fn vacating_the_last_pending_guesser_ends_the_turn() {
        let mut g = started_roster(&[("A", false), ("B", false), ("C", false), ("D", false)]);
        g.choose_word(g.word_choices[0].clone());
        g.tick(Duration::from_secs(0));
        let secret = g.secret_word.clone();
        assert_eq!(g.apply_guess(1, &secret), GuessOutcome::Correct);
        assert_eq!(g.apply_guess(2, &secret), GuessOutcome::Correct);
        assert_eq!(g.phase, Phase::Drawing, "seat 3 still pending");
        g.vacate_seat(3);
        assert_eq!(
            g.phase,
            Phase::Reveal,
            "the remaining guessers all have it — the turn ends on the vacate"
        );
    }

    /// W1-review S1 (§2.3.3): a vacated seat can no longer guess, score, or enter
    /// `turn_guesses`.
    #[test]
    fn a_vacated_seat_cannot_guess() {
        let mut g = started_roster(&[("A", false), ("B", false), ("C", false), ("D", false)]);
        g.choose_word(g.word_choices[0].clone());
        g.tick(Duration::from_secs(0));
        let secret = g.secret_word.clone();
        g.vacate_seat(2);
        let before = g.players[2].score;
        assert_eq!(g.apply_guess(2, &secret), GuessOutcome::Ignored);
        assert_eq!(g.players[2].score, before, "no points for a vacated seat");
        assert!(g.turn_guesses.iter().all(|gu| gu.player != 2));
    }

    /// §2.3.3/§2.3.4 composition: vacating the CURRENT drawer mid-turn does not
    /// end the turn by itself (that is the caller's `force_end_turn`, the
    /// Session's drawer-drop rule), and the follow-up rotation lands on an
    /// occupied seat.
    #[test]
    fn vacating_the_current_drawer_then_force_end_advances_to_an_occupied_seat() {
        let mut g = started_roster(&[("A", false), ("B", false), ("C", false), ("D", false)]);
        g.choose_word(g.word_choices[0].clone());
        g.tick(Duration::from_secs(0));
        g.vacate_seat(0); // the drawer drops; no guesses yet
        assert_eq!(
            g.phase,
            Phase::Drawing,
            "vacating the drawer alone is not a turn end"
        );
        g.force_end_turn();
        assert_eq!(g.phase, Phase::Reveal);
        g.continue_now();
        assert_eq!(g.phase, Phase::Picking);
        assert_eq!(g.seat_index, 1, "rotation lands on the next occupied seat");
        assert!(g.players[g.seat_index].occupied);
    }

    /// §2.3.3: wrap-around rotation skips a vacant seat 0 and still increments
    /// the round exactly once.
    #[test]
    fn wrap_rotation_skips_vacant_seat_zero_and_increments_round() {
        let mut g = started_roster(&[("A", false), ("B", false), ("C", false), ("D", false)]);
        // Turn 1: seat 0 draws, then leaves after its turn.
        g.choose_word(g.word_choices[0].clone());
        g.tick(Duration::from_secs(0));
        g.force_end_turn();
        g.continue_now();
        g.vacate_seat(0);
        // Seats 1..=3 each take their turn.
        for expected in [1usize, 2, 3] {
            assert_eq!(g.seat_index, expected);
            assert_eq!(g.round, 1);
            g.choose_word(g.word_choices[0].clone());
            g.tick(Duration::from_secs(0));
            g.force_end_turn();
            g.continue_now();
        }
        // The wrap skips vacant seat 0 straight to seat 1, round 2.
        assert_eq!(g.phase, Phase::Picking);
        assert_eq!(g.seat_index, 1, "wrap skipped the vacant seat 0");
        assert_eq!(g.round, 2, "the wrap incremented the round exactly once");
    }

    /// §2.3.3 — `guesser_count` / `all_guessed` count only OCCUPIED non-drawer
    /// seats, so vacating a guesser lets the remaining occupied guessers end the
    /// turn early.
    #[test]
    fn guesser_count_and_all_guessed_track_occupancy() {
        let mut g = started(); // seat 0 draws, 4 occupied
        g.config.bots_enabled = false;
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        assert_eq!(g.guesser_count(), 3, "3 occupied non-drawer seats");
        g.vacate_seat(3);
        assert_eq!(g.guesser_count(), 2, "vacating a guesser drops the count");
        g.apply_guess(1, "robot");
        assert!(!g.all_guessed(), "one of two occupied guessers");
        g.apply_guess(2, "robot");
        assert!(g.all_guessed(), "both occupied guessers have it");
        assert_eq!(
            g.phase,
            Phase::Reveal,
            "the turn ends once all OCCUPIED guessers guess (not all 3 original seats)"
        );
    }

    /// §2.3.3 — the match ends (→ `Final`) when occupancy drops below 2.
    #[test]
    fn occupancy_below_two_ends_the_match() {
        let mut g = started();
        g.config.bots_enabled = false;
        g.choose_word("robot".to_string());
        g.vacate_seat(1);
        g.vacate_seat(2);
        g.vacate_seat(3);
        assert_eq!(g.occupied_count(), 1);
        g.force_end_turn();
        g.continue_now();
        assert_eq!(
            g.phase,
            Phase::Final,
            "occupancy < 2 ends the match on the next advance"
        );
    }

    /// §2.3.4 — `force_end_turn` ends a Drawing turn to Reveal (with results) and is
    /// a no-op in any other phase.
    #[test]
    fn force_end_turn_ends_drawing_to_reveal() {
        let mut g = started();
        g.config.bots_enabled = false;
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        assert_eq!(g.phase, Phase::Drawing);
        g.force_end_turn();
        assert_eq!(g.phase, Phase::Reveal, "force_end_turn ends to Reveal");
        assert!(!g.turn_results.is_empty(), "the reveal rows are built");
        // Outside Drawing it is a no-op.
        let before = g.clone();
        g.force_end_turn();
        assert_eq!(g, before, "force_end_turn is a no-op outside Drawing");
    }

    /// §5.1 — `knows(seat)` is three-way: the drawer, a correct guesser (mid-turn),
    /// or everyone at the reveal.
    #[test]
    fn knows_is_three_way_drawer_correct_guesser_or_reveal() {
        let mut g = started();
        g.config.bots_enabled = false;
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        assert!(g.knows(0), "the drawer knows");
        assert!(!g.knows(1), "a guesser who hasn't guessed does not");
        g.apply_guess(1, "robot");
        assert!(g.knows(1), "a correct guesser knows mid-turn");
        assert!(!g.knows(2), "another guesser still does not");
        g.force_end_turn();
        assert_eq!(g.phase, Phase::Reveal);
        for seat in 0..4 {
            assert!(g.knows(seat), "at the reveal everyone knows");
        }
    }

    /// §2.3.5 — `word_display_for` redacts per seat, and `word_display()` delegates
    /// to the viewing seat.
    #[test]
    fn word_display_for_redacts_per_seat() {
        let mut g = started();
        g.config.bots_enabled = false;
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        assert!(
            g.word_display_for(0).contains('R'),
            "the drawer sees the letters"
        );
        let guesser = g.word_display_for(1);
        assert!(
            !guesser.contains('R') && guesser.contains('_'),
            "a guesser sees blanks: {guesser}"
        );
        assert!(
            Game::default().word_display_for(0).is_empty(),
            "no word row outside Drawing/Reveal"
        );
    }
}
