//! `playtest_host` — the long-running headless host for the W8 multi-agent
//! playtest. It runs the FULL Dooduel app + game (GPU-free) with a real-time
//! clock, and exposes the match over a **file protocol** under a `--dir <root>`
//! so N separate LLM agents (one per seat) can each play a live match by reading
//! their own honest view and appending commands. No GPU: the drawing canvas' CPU
//! pixel buffer is the image source, encoded directly with the `image` crate.
//!
//! # File protocol (all paths relative to `--dir`)
//!
//! **Host → agents (written by the host):**
//! - `seat_<i>_view.md` (i = 0..3) — an honest, per-seat report computed FROM THE
//!   MODEL. It NEVER leaks the secret word to a guesser: the word is shown in full
//!   only to the drawer, to a seat that already guessed it, and during the reveal;
//!   everyone else sees blanks + hint-revealed letters. Its chat is per-seat
//!   filtered, so a private near-miss nudge never leaks to another seat. Refreshed
//!   after every command and on a heartbeat (so countdowns advance).
//! - `state.json` — machine-readable: `phase`, `drawer` (null at the podium),
//!   `round` (clamped display), `total_rounds`, `turn`, `countdown` (wall-clock),
//!   `word_length`, `hint_total`/`hints_revealed`, `guessed_count`/`all_guessed`,
//!   `tick`, `started`, and a `seats` array carrying each seat's score + "can I act
//!   now" flags (`can_pick`/`can_guess`/`can_draw`/`can_continue`) + `guessed`.
//! - `chat.md` — the SHARED broadcast chat as a standalone low-diff feed (private
//!   near-miss nudges are per-seat and appear only in the addressed seat's view).
//! - `canvas.png` — the 720×450 drawing surface, written per-stroke as the drawer
//!   draws (and on a heartbeat), so guessers watch the art appear incrementally.
//! - `host.log` — every applied command + every rejection reason (append-only).
//!
//! **Agents → host (append lines to `commands.jsonl`, one JSON object per line):**
//! - `{"cmd":"start"}` — begin the match (host-side; uses the configured seats +
//!   durations + bots-off).
//! - `{"seat":N,"cmd":"pick","index":0}` — the DRAWER picks word `index` (Picking
//!   phase only). Routed through the real funnel (`Msg::ChooseWord`).
//! - `{"seat":N,"cmd":"guess","text":"..."}` — seat N guesses. Routed through the
//!   real funnel (`Msg::Guess` — the SAME pipeline the app's bots used). The funnel
//!   authoritatively rejects the drawer / a repeat / a wrong-phase guess.
//! - `{"seat":N,"cmd":"stroke","points":[[x,y],...],"color":[r,g,b],"size":k}` —
//!   the DRAWER draws a polyline in canvas-pixel coords (Drawing phase only).
//!   Applied directly on the paint surface's real `stroke_segment` path (input
//!   fidelity is already proven by the D1 drag test + W7 browser drives).
//! - `{"seat":N,"cmd":"clear"}` — the DRAWER clears the canvas (Drawing only).
//! - `{"seat":N,"cmd":"continue"}` — advance past the turn-end reveal (the design's
//!   "Continue →"; Reveal phase only). Routed through the funnel (`Msg::Continue`).
//! - `{"cmd":"status"}` — force an immediate report refresh + a log line.
//! - `{"cmd":"quit"}` — stop the host.
//!
//! Malformed lines are logged and skipped; the host never panics on bad input.
//!
//! # Knobs (env vars; slow file-protocol agents need generous windows)
//! `DOODUEL_DIR` or `--dir` (root, required) · `DOODUEL_DRAW_SECS` (120) ·
//! `DOODUEL_PICK_SECS` (45) · `DOODUEL_REVEAL_SECS` (12) · `DOODUEL_ROUNDS` (2) ·
//! `DOODUEL_SEAT0_NAME` ("Alex") · `DOODUEL_MAX_SECS` (3600 safety cap) ·
//! `DOODUEL_FRAME_MS` (33 loop pacing).

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::ecs::message::Messages;
use bevy::image::Image;
use bevy::prelude::*;
use buiy::prelude::ClockPlugin;
use buiy_core::mvu::Envelope;
use dooduel::game::{ChatMsg, Config, Game, Phase};
use dooduel::paint::{CANVAS_H, CANVAS_W, CanvasKind, PaintCanvases, Tool};
use dooduel::{Dooduel, Msg, Screen};
use serde_json::{Value, json};

/// The number of seats (the design's 4-player room). Views are always written for
/// all four, even before the match starts.
const SEATS: usize = 4;

/// Default ink color for a stroke that omits `color` (the design's near-black).
const DEFAULT_INK: [u8; 4] = [0x14, 0x16, 0x1b, 255];

fn main() {
    let settings = match Settings::parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("playtest_host: {e}");
            std::process::exit(2);
        }
    };
    let mut host = Host::new(settings);
    host.run();
}

/// The host's launch configuration (durations, seats, pacing).
struct Settings {
    dir: PathBuf,
    /// The match rules the host starts with (bots OFF, widened durations).
    config: Config,
    seat0_name: String,
    frame: Duration,
    view_period: Duration,
    canvas_period: Duration,
    max: Duration,
}

impl Settings {
    fn parse() -> Result<Self, String> {
        // `--dir <path>` (or `DOODUEL_DIR`).
        let mut dir: Option<PathBuf> = std::env::var_os("DOODUEL_DIR").map(PathBuf::from);
        let mut args = std::env::args().skip(1);
        while let Some(a) = args.next() {
            match a.as_str() {
                "--dir" => dir = args.next().map(PathBuf::from),
                other => return Err(format!("unknown argument {other:?} (use --dir <path>)")),
            }
        }
        let dir = dir.ok_or("a --dir <path> (or DOODUEL_DIR) is required")?;

        let config = Config {
            total_rounds: env_u64("DOODUEL_ROUNDS", 2) as u32,
            draw_seconds: env_u64("DOODUEL_DRAW_SECS", 120),
            pick_seconds: env_u64("DOODUEL_PICK_SECS", 45),
            reveal_seconds: env_u64("DOODUEL_REVEAL_SECS", 12),
            hint_count: 2,
            // All four seats are agent-driven: no built-in bot guessers.
            bots_enabled: false,
        };
        Ok(Settings {
            dir,
            config,
            seat0_name: env_string("DOODUEL_SEAT0_NAME", "Alex"),
            frame: Duration::from_millis(env_u64("DOODUEL_FRAME_MS", 33)),
            view_period: Duration::from_millis(500),
            canvas_period: Duration::from_millis(500),
            max: Duration::from_secs(env_u64("DOODUEL_MAX_SECS", 3600)),
        })
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_string(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// The running host: the app + the file-protocol bookkeeping.
struct Host {
    settings: Settings,
    app: App,
    started: bool,
    quit: bool,
    /// Bytes of `commands.jsonl` already consumed (append-only file, byte cursor).
    cmd_offset: u64,
    tick: u64,
    start_instant: Instant,
    last_view: Instant,
    last_canvas: Instant,
    last_canvas_hash: u64,
    log: File,
}

impl Host {
    fn new(settings: Settings) -> Self {
        fs::create_dir_all(&settings.dir).expect("create --dir root");

        // GPU-free app: the probe preset (MVU funnel + reconciler + layout/a11y/
        // text/widgets) + the REAL-TIME clock driver + the drawing canvases. No
        // render, no picking — the host reads the model and applies strokes to the
        // CPU paint surface directly. `init_asset::<Image>()` is the one recipe
        // piece the probe preset omits (CanvasPlugin creates + mirrors the image).
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(buiy::BuiyProbePlugin);
        app.init_asset::<Image>();
        dooduel::install(&mut app);
        // The F7 poll-clock (replaces the prototype's hand-rolled GameClockPlugin):
        // enqueues `Msg::Tick(Time::elapsed())` each frame, so the host's real-time
        // loop advances the match at wall-clock rate.
        app.add_plugins(ClockPlugin::<Dooduel>::new(Msg::Tick));
        app.add_plugins(dooduel::paint::CanvasPlugin);
        // Boot: spawn the model (`ui()`), run Startup (create the canvases), and let
        // `announce_canvases` fold the handles in.
        for _ in 0..12 {
            app.update();
        }

        // Bug #1 (pre-start): seed the model's game config from the configured rules
        // so the pre-start reports read the CONFIGURED `total_rounds`, not the stale
        // `Config` default (the "total_rounds reads 2 pre-start vs 1 in-match" half of
        // the round-counter bug). `start` re-applies the same config.
        if let Some(e) = app
            .world_mut()
            .query_filtered::<Entity, With<Dooduel>>()
            .iter(app.world())
            .next()
            && let Some(mut d) = app.world_mut().get_mut::<Dooduel>(e)
        {
            d.game.config = settings.config.clone();
        }

        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(settings.dir.join("host.log"))
            .expect("open host.log");

        let now = Instant::now();
        let mut host = Host {
            settings,
            app,
            started: false,
            quit: false,
            cmd_offset: 0,
            tick: 0,
            start_instant: now,
            last_view: now,
            last_canvas: now,
            last_canvas_hash: 0,
            log,
        };
        host.log_line(format!(
            "host booted; dir={} draw={}s pick={}s reveal={}s rounds={} seat0={:?} bots=OFF",
            host.settings.dir.display(),
            host.settings.config.draw_seconds,
            host.settings.config.pick_seconds,
            host.settings.config.reveal_seconds,
            host.settings.config.total_rounds,
            host.settings.seat0_name,
        ));
        host.write_reports();
        host.maybe_write_canvas();
        host
    }

    fn run(&mut self) {
        loop {
            self.app.update();
            self.tick += 1;

            let processed = self.poll_commands();
            // In-phase delivery (§3.4, couples with fix #3): a gameplay command
            // (guess / pick / continue) is *enqueued* onto the funnel here, but is
            // only *applied* on the next `app.update()`. Drain it now so the report
            // we write this iteration reflects the command's effect immediately —
            // otherwise the per-seat view lags by up to a heartbeat (the observed
            // "the drawer sees the guess only after the turn flips" bug).
            if processed {
                self.app.update();
            }

            let now = Instant::now();
            if processed || now.duration_since(self.last_view) >= self.settings.view_period {
                self.write_reports();
                self.last_view = now;
            }
            if now.duration_since(self.last_canvas) >= self.settings.canvas_period {
                self.maybe_write_canvas();
                self.last_canvas = now;
            }
            if self.quit {
                self.log_line("host quitting (quit command)".into());
                break;
            }
            if self.start_instant.elapsed() >= self.settings.max {
                self.log_line("host quitting (max-secs safety cap reached)".into());
                break;
            }
            std::thread::sleep(self.settings.frame);
        }
        // A final flush so the last frame's state is on disk.
        self.write_reports();
        self.maybe_write_canvas();
        self.log_line("host stopped".into());
    }

    // --- command intake ----------------------------------------------------

    /// Read + process every COMPLETE new line of `commands.jsonl`. Returns whether
    /// any command was processed (to trigger an immediate report refresh).
    fn poll_commands(&mut self) -> bool {
        let path = self.settings.dir.join("commands.jsonl");
        let Ok(mut f) = File::open(&path) else {
            return false; // no commands yet
        };
        if f.seek(SeekFrom::Start(self.cmd_offset)).is_err() {
            return false;
        }
        let mut buf = String::new();
        if f.read_to_string(&mut buf).is_err() {
            return false;
        }
        let mut consumed = 0usize;
        let mut any = false;
        for line in buf.split_inclusive('\n') {
            if !line.ends_with('\n') {
                break; // a partial trailing line — leave it for the next poll
            }
            consumed += line.len();
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                self.process_line(trimmed);
                any = true;
            }
        }
        self.cmd_offset += consumed as u64;
        any
    }

    fn process_line(&mut self, line: &str) {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                self.log_line(format!("SKIP malformed json ({e}): {line}"));
                return;
            }
        };
        match v.get("cmd").and_then(Value::as_str) {
            Some("start") => self.cmd_start(),
            Some("status") => {
                let summary = self.state_summary();
                self.log_line(format!("status: {summary}"));
            }
            Some("quit") => self.quit = true,
            Some("guess") => self.cmd_guess(&v),
            Some("pick") => self.cmd_pick(&v),
            Some("stroke") => self.cmd_stroke(&v),
            Some("clear") => self.cmd_clear(&v),
            Some("continue") => self.cmd_continue(),
            other => self.log_line(format!("SKIP unknown cmd {other:?}: {line}")),
        }
    }

    fn cmd_start(&mut self) {
        if self.started {
            self.log_line("start ignored: match already running".into());
            return;
        }
        let name = self.settings.seat0_name.clone();
        let config = self.settings.config.clone();
        let Some(e) = self.model_entity() else {
            self.log_line("start FAILED: no model entity".into());
            return;
        };
        // Lifecycle (start/restart) is the host harness's job — direct on the model.
        // Gameplay actions (guess/pick) still route through the real funnel below.
        let mut d = self.app.world_mut().get_mut::<Dooduel>(e).expect("model");
        d.player_name = name.clone();
        d.game.start_match(&name, config);
        d.screen = Screen::InGame;
        self.started = true;
        self.log_line(format!("start: match begun; seat0={name:?}"));
    }

    fn cmd_guess(&mut self, v: &Value) {
        let Some(seat) = seat_of(v) else {
            self.log_line(format!("SKIP guess: bad/absent seat: {v}"));
            return;
        };
        let text = v
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let g = self.game();
        if g.phase != Phase::Drawing {
            self.log_line(format!(
                "guess REJECTED (phase={:?}, not Drawing): seat {seat}",
                g.phase
            ));
            return;
        }
        if seat == g.seat_index {
            self.log_line(format!("guess REJECTED (seat {seat} is the drawer)"));
            return;
        }
        // Route through the REAL funnel — `Game::apply_guess` scores / near-misses /
        // rejects repeats exactly as for a human or bot.
        self.enqueue(Msg::Guess {
            player: seat,
            text: text.clone(),
        });
        self.log_line(format!("guess: seat {seat} {text:?} (funnel)"));
    }

    fn cmd_pick(&mut self, v: &Value) {
        let Some(seat) = seat_of(v) else {
            self.log_line(format!("SKIP pick: bad/absent seat: {v}"));
            return;
        };
        let index = v.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let g = self.game();
        if g.phase != Phase::Picking {
            self.log_line(format!(
                "pick REJECTED (phase={:?}, not Picking): seat {seat}",
                g.phase
            ));
            return;
        }
        if seat != g.seat_index {
            self.log_line(format!(
                "pick REJECTED (seat {seat} is not the drawer, seat {} is)",
                g.seat_index
            ));
            return;
        }
        if index >= g.word_choices.len() {
            self.log_line(format!(
                "pick REJECTED (index {index} out of range 0..{})",
                g.word_choices.len()
            ));
            return;
        }
        let word = g.word_choices[index].clone();
        self.enqueue(Msg::ChooseWord(index));
        self.log_line(format!(
            "pick: seat {seat} chose word[{index}] {word:?} (funnel)"
        ));
    }

    fn cmd_stroke(&mut self, v: &Value) {
        let Some(seat) = seat_of(v) else {
            self.log_line(format!("SKIP stroke: bad/absent seat: {v}"));
            return;
        };
        let g = self.game();
        if g.phase != Phase::Drawing {
            self.log_line(format!(
                "stroke REJECTED (phase={:?}, not Drawing): seat {seat}",
                g.phase
            ));
            return;
        }
        if seat != g.seat_index {
            self.log_line(format!("stroke REJECTED (seat {seat} is not the drawer)"));
            return;
        }
        let points = parse_points(v);
        if points.is_empty() {
            self.log_line(format!("stroke REJECTED (no valid points): {v}"));
            return;
        }
        let color = parse_color(v).unwrap_or(DEFAULT_INK);
        let size = v.get("size").and_then(Value::as_u64).unwrap_or(6) as i32;
        let Some(mut canvases) = self.app.world_mut().get_resource_mut::<PaintCanvases>() else {
            self.log_line("stroke FAILED: no PaintCanvases".into());
            return;
        };
        let s = canvases.surface_mut(CanvasKind::Game);
        // Set brush directly for this stroke; the per-frame model→canvas sync will
        // restore the model's tool afterwards, but the pixels are already painted.
        s.tool = Tool::Brush;
        s.color = color;
        s.radius = (size / 2).max(0);
        let (x0, y0) = points[0];
        s.begin(x0, y0);
        for &(x, y) in &points[1..] {
            s.extend(x, y); // real line-interpolated stroke_segment path
        }
        s.end();
        self.log_line(format!(
            "stroke: seat {seat}, {} pts, color {color:?}, size {size}",
            points.len()
        ));
        // Per-stroke PNG flush (§3.4): stream each stroke to `canvas.png` as it lands
        // (the content-hash gate skips a no-op), so guessers watch the art appear
        // incrementally instead of all-at-once on the ~2 Hz heartbeat.
        self.maybe_write_canvas();
    }

    fn cmd_clear(&mut self, v: &Value) {
        let Some(seat) = seat_of(v) else {
            self.log_line(format!("SKIP clear: bad/absent seat: {v}"));
            return;
        };
        let g = self.game();
        if g.phase != Phase::Drawing || seat != g.seat_index {
            self.log_line(format!("clear REJECTED (seat {seat}, phase {:?})", g.phase));
            return;
        }
        if let Some(mut canvases) = self.app.world_mut().get_resource_mut::<PaintCanvases>() {
            canvases.surface_mut(CanvasKind::Game).clear();
            self.log_line(format!("clear: seat {seat} cleared the canvas"));
        }
        self.maybe_write_canvas(); // stream the cleared sheet immediately (§3.4)
    }

    /// Advance out of the turn-end reveal (the design's "Continue →", §3.4). Routed
    /// through the real funnel (`Msg::Continue` → `Game::continue_now`). No-op in any
    /// phase but `Reveal`.
    fn cmd_continue(&mut self) {
        let g = self.game();
        if g.phase != Phase::Reveal {
            self.log_line(format!(
                "continue REJECTED (phase={:?}, not Reveal)",
                g.phase
            ));
            return;
        }
        self.enqueue(Msg::Continue);
        self.log_line("continue: advancing past the reveal (funnel)".into());
    }

    // --- reporting ---------------------------------------------------------

    fn write_reports(&mut self) {
        let model = self.snapshot();
        let g = &model.game;
        let canvas_ink = self.canvas_ink();

        // Per-seat rows carry the score PLUS the "can I act now" flags (drawer-vs-
        // guesser gating) + guessed state, so an agent needn't re-derive them (§3.4).
        let seats: Vec<Value> = g
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let is_drawer = g.current_drawer() == Some(i);
                let guessed = g.turn_guesses.iter().any(|gu| gu.player == i);
                json!({
                    "seat": i,
                    "name": p.name,
                    "score": p.score,
                    "is_drawer": is_drawer,
                    "guessed": guessed,
                    "can_pick": g.phase == Phase::Picking && is_drawer,
                    "can_guess": g.phase == Phase::Drawing && !is_drawer && !guessed,
                    "can_draw": g.phase == Phase::Drawing && is_drawer,
                    "can_continue": g.phase == Phase::Reveal,
                })
            })
            .collect();
        let state = json!({
            "phase": phase_str(g.phase),
            "screen": screen_str(&model.screen),
            "started": self.started,
            // Bug #2: no active drawer once the match is over (null at the podium).
            "drawer": g.current_drawer(),
            "drawer_name": g.drawer_name(),
            // Bug #1: the DISPLAY round is clamped to [1, total]; total is authoritative
            // from the config (seeded at boot), so it is stable pre-start and in-match.
            "round": g.round_display(),
            "total_rounds": g.config.total_rounds,
            // The turn's position in the rotation (1-based seat), or null when idle/over.
            "turn": g.current_drawer().map(|s| s + 1),
            // Bug #6: the countdown is the wall-clock `now − anchor` accessor, NOT the
            // frame counter (`tick`) — they are separate fields on purpose.
            "countdown": countdown(g),
            "word_length": g.word_length(),
            "hint_total": g.hint_total(),
            "hints_revealed": g.hints_revealed(),
            // Bug #3: the live "who has the word" signal, visible in-phase.
            "guessed_count": g.guessed_count(),
            "all_guessed": g.all_guessed(),
            "tick": self.tick,
            "canvas_ink": canvas_ink,
            "seats": seats,
        });
        write_atomic(
            &self.settings.dir.join("state.json"),
            serde_json::to_string_pretty(&state)
                .unwrap_or_default()
                .as_bytes(),
        );

        // The shared broadcast chat as a dedicated low-diff feed (§3.4) — only the
        // SHARED lines (private near-miss nudges stay in the addressed seat's view).
        write_atomic(
            &self.settings.dir.join("chat.md"),
            render_shared_chat(g).as_bytes(),
        );

        for i in 0..SEATS {
            let view = render_seat_view(&model, i, self.started);
            write_atomic(
                &self.settings.dir.join(format!("seat_{i}_view.md")),
                view.as_bytes(),
            );
        }
    }

    /// Encode the Game canvas' CPU buffer to `canvas.png` iff it changed since the
    /// last write (a cheap content hash gates re-encoding).
    fn maybe_write_canvas(&mut self) {
        let Some((w, h, pixels, hash)) =
            self.app.world().get_resource::<PaintCanvases>().map(|c| {
                let s = c.surface(CanvasKind::Game);
                (
                    s.width as u32,
                    s.height as u32,
                    s.pixels.clone(),
                    fnv1a(&s.pixels),
                )
            })
        else {
            return;
        };
        if hash == self.last_canvas_hash {
            return;
        }
        self.last_canvas_hash = hash;
        let Some(img) = image::RgbaImage::from_raw(w, h, pixels) else {
            self.log_line("canvas encode FAILED (bad raw length)".into());
            return;
        };
        let tmp = self.settings.dir.join("canvas.png.tmp");
        let dst = self.settings.dir.join("canvas.png");
        if let Err(e) = img.save_with_format(&tmp, image::ImageFormat::Png) {
            self.log_line(format!("canvas write FAILED: {e}"));
            return;
        }
        let _ = fs::rename(&tmp, &dst);
    }

    /// The count of non-paper pixels on the Game canvas (a machine-readable "has
    /// the drawer drawn anything?" signal for agents + the rehearsal ink check).
    fn canvas_ink(&self) -> usize {
        self.app
            .world()
            .get_resource::<PaintCanvases>()
            .map(|c| {
                c.surface(CanvasKind::Game)
                    .pixels
                    .chunks_exact(4)
                    .filter(|p| *p != dooduel::paint::PAPER)
                    .count()
            })
            .unwrap_or(0)
    }

    // --- world helpers -----------------------------------------------------

    fn model_entity(&mut self) -> Option<Entity> {
        self.app
            .world_mut()
            .query_filtered::<Entity, With<Dooduel>>()
            .iter(self.app.world())
            .next()
    }

    fn snapshot(&mut self) -> Dooduel {
        self.app
            .world_mut()
            .query::<&Dooduel>()
            .iter(self.app.world())
            .next()
            .cloned()
            .unwrap_or_default()
    }

    fn game(&mut self) -> Game {
        self.snapshot().game
    }

    fn enqueue(&mut self, msg: Msg) {
        let Some(e) = self.model_entity() else {
            return;
        };
        self.app
            .world_mut()
            .resource_mut::<Messages<Envelope<Dooduel>>>()
            .write(Envelope::user(e, msg));
    }

    fn state_summary(&mut self) -> String {
        let g = self.game();
        format!(
            "phase={:?} round={}/{} drawer={:?} countdown={}s tick={}",
            g.phase,
            g.round_display(),
            g.config.total_rounds,
            g.current_drawer(),
            countdown(&g),
            self.tick
        )
    }

    fn log_line(&mut self, msg: String) {
        let _ = writeln!(self.log, "[t={:>5}] {msg}", self.tick);
        let _ = self.log.flush();
    }
}

// --- pure helpers ----------------------------------------------------------

/// Parse `seat` as a valid seat index (`0..SEATS`).
fn seat_of(v: &Value) -> Option<usize> {
    let s = v.get("seat")?.as_u64()? as usize;
    (s < SEATS).then_some(s)
}

/// Parse `points: [[x,y], ...]` into clamped canvas-pixel coords, skipping any
/// malformed entry.
fn parse_points(v: &Value) -> Vec<(i32, i32)> {
    let Some(arr) = v.get("points").and_then(Value::as_array) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|p| {
            let a = p.as_array()?;
            let x = a.first()?.as_f64()?;
            let y = a.get(1)?.as_f64()?;
            Some((
                (x.round() as i32).clamp(0, CANVAS_W as i32 - 1),
                (y.round() as i32).clamp(0, CANVAS_H as i32 - 1),
            ))
        })
        .collect()
}

/// Parse `color: [r,g,b]` (0..=255) into an opaque RGBA.
fn parse_color(v: &Value) -> Option<[u8; 4]> {
    let a = v.get("color")?.as_array()?;
    let ch = |i: usize| a.get(i).and_then(Value::as_u64).map(|n| n.min(255) as u8);
    Some([ch(0)?, ch(1)?, ch(2)?, 255])
}

fn phase_str(p: Phase) -> &'static str {
    match p {
        Phase::Idle => "idle",
        Phase::Picking => "picking",
        Phase::Drawing => "drawing",
        Phase::Reveal => "reveal",
        Phase::Final => "final",
    }
}

fn screen_str(s: &Screen) -> &'static str {
    match s {
        Screen::Home => "home",
        Screen::Join => "join",
        Screen::Lobby => "lobby",
        Screen::InGame => "in_game",
        Screen::Podium => "podium",
    }
}

fn countdown(g: &Game) -> u64 {
    match g.phase {
        Phase::Picking => g.pick_seconds_left,
        Phase::Drawing => g.draw_seconds_left,
        Phase::Reveal => g.reveal_seconds_left,
        _ => 0,
    }
}

/// The word AS SEAT `seat` should see it — reuses the exact `word_display` "who
/// knows" logic (drawer / already-guessed / reveal see the letters; others see
/// blanks + hint-revealed positions) by projecting onto a clone, so the secret is
/// never leaked to a guesser.
fn word_as_seen_by(game: &Game, seat: usize) -> String {
    let mut g = game.clone();
    g.viewing_as = seat;
    g.word_display()
}

/// A per-seat honest markdown view.
fn render_seat_view(model: &Dooduel, seat: usize, started: bool) -> String {
    let g = &model.game;
    let mut out = String::new();
    let me = g.players.get(seat).map(|p| p.name.as_str()).unwrap_or("—");
    out.push_str(&format!("# Dooduel — Seat {seat} ({me})\n\n"));

    if !started || g.phase == Phase::Idle {
        out.push_str("**Waiting for the match to start.**\n\n");
        out.push_str("_Available action:_ none yet — the host starts on the `start` command.\n");
        return out;
    }

    // Bug #2: read the active drawer through the accessor — `None` at the podium, so
    // no finished-match seat is mis-labelled as drawing.
    let drawer = g.current_drawer();
    out.push_str(&format!(
        "**Phase:** {} · **Round {}/{}** · **{}s left**\n",
        phase_str(g.phase),
        g.round_display(), // bug #1: clamped display round
        g.config.total_rounds,
        countdown(g), // bug #6: wall-clock `now − anchor`, not the frame count
    ));
    match drawer {
        Some(d) => {
            let drawer_name = g.drawer_name().unwrap_or("—");
            let you_draw = if seat == d { " — that's YOU" } else { "" };
            out.push_str(&format!(
                "**Drawing:** {drawer_name} (seat {d}){you_draw}\n"
            ));
        }
        None => out.push_str("**Drawing:** — (no active drawer)\n"),
    }
    // Bug #3: the drawer sees the LIVE guessed count in-phase (never blind).
    if g.phase == Phase::Drawing {
        let guessers = g.players.len().saturating_sub(1);
        out.push_str(&format!(
            "**Guessed so far:** {} / {} guessers{}\n",
            g.guessed_count(),
            guessers,
            if g.all_guessed() {
                " — everyone has it!"
            } else {
                ""
            },
        ));
    }
    out.push('\n');

    // Word — as THIS seat sees it (never leaks), with the letter count everyone knows.
    let word = word_as_seen_by(g, seat);
    if word.is_empty() {
        out.push_str("**Word:** —\n\n");
    } else {
        out.push_str(&format!(
            "**Word:** {word}  ({} letters, {}/{} hints revealed)\n\n",
            g.word_length(),
            g.hints_revealed(),
            g.hint_total(),
        ));
    }

    // Scoreboard (sorted high→low), marking the drawer + correct guessers.
    out.push_str("## Scoreboard\n");
    for (rank, (i, p)) in g.standings().into_iter().enumerate() {
        let mut tags = Vec::new();
        // Bug #2: only the ACTIVE drawer is tagged "drawing" (gone at the podium).
        if drawer == Some(i) && g.phase == Phase::Drawing {
            tags.push("drawing");
        }
        if g.turn_guesses.iter().any(|gu| gu.player == i) {
            tags.push("guessed");
        }
        if i == seat {
            tags.push("you");
        }
        let tag = if tags.is_empty() {
            String::new()
        } else {
            format!(" ({})", tags.join(", "))
        };
        out.push_str(&format!("{}. {} — {}{}\n", rank + 1, p.name, p.score, tag));
    }
    out.push('\n');

    // Chat tail (last 15) AS THIS SEAT SEES IT (bug #4): shared lines plus only the
    // private nudges addressed to `seat` — a near-miss "So close!" never leaks to
    // another seat. (The secret word only appears in shared chat at the reveal.)
    out.push_str("## Chat (recent)\n");
    let visible: Vec<&ChatMsg> = g.chat_for(seat).collect();
    let tail = visible.len().saturating_sub(15);
    for m in &visible[tail..] {
        out.push_str(&format!("- {}\n", m.text));
    }
    out.push('\n');

    // Seat-specific available actions.
    out.push_str("## Your actions\n");
    out.push_str(&seat_actions(g, seat));
    out
}

/// The SHARED broadcast chat as a standalone markdown feed (§3.4) — only the lines
/// everyone sees (`to == None`). Private near-miss nudges are per-seat and stay in
/// the addressed seat's view, so this file never leaks one.
fn render_shared_chat(g: &Game) -> String {
    let mut out = String::from("# Dooduel — shared chat\n\n");
    let shared: Vec<&ChatMsg> = g.chat.iter().filter(|m| m.to.is_none()).collect();
    if shared.is_empty() {
        out.push_str("_(no messages yet)_\n");
        return out;
    }
    for m in &shared {
        out.push_str(&format!("- {}\n", m.text));
    }
    out
}

/// The concrete actions seat `seat` can take right now (with the command shapes).
fn seat_actions(g: &Game, seat: usize) -> String {
    let is_drawer = seat == g.seat_index;
    match g.phase {
        Phase::Picking if is_drawer => {
            let mut s = String::from("You are the drawer. **Pick a word** by index:\n");
            for (i, w) in g.word_choices.iter().enumerate() {
                s.push_str(&format!(
                    "- `{{\"seat\":{seat},\"cmd\":\"pick\",\"index\":{i}}}` → {w}\n"
                ));
            }
            s
        }
        Phase::Picking => format!("Wait — seat {} is choosing a word.\n", g.seat_index),
        Phase::Drawing if is_drawer => format!(
            "You are DRAWING. Use `{{\"seat\":{seat},\"cmd\":\"stroke\",\"points\":[[x,y],...],\"color\":[r,g,b],\"size\":k}}` \
             (canvas is {CANVAS_W}×{CANVAS_H} px) and `{{\"seat\":{seat},\"cmd\":\"clear\"}}`.\n"
        ),
        Phase::Drawing => format!(
            "**Guess the word:** `{{\"seat\":{seat},\"cmd\":\"guess\",\"text\":\"...\"}}`.\n"
        ),
        Phase::Reveal => format!(
            "Turn over — the next turn starts automatically, or advance it now with \
             `{{\"seat\":{seat},\"cmd\":\"continue\"}}`.\n"
        ),
        Phase::Final | Phase::Idle => "The match is over.\n".to_string(),
    }
}

/// Write `bytes` to `path` atomically (temp file + rename), so a polling agent
/// never reads a half-written file.
fn write_atomic(path: &std::path::Path, bytes: &[u8]) {
    let tmp = path.with_extension("tmp");
    if fs::write(&tmp, bytes).is_ok() {
        let _ = fs::rename(&tmp, path);
    }
}

/// FNV-1a 64-bit over a byte slice — a cheap change-detector for the canvas buffer.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use dooduel::game::GuessOutcome;

    /// Wrap a `Game` in a model for the pure render helpers.
    fn model_with(game: Game) -> Dooduel {
        Dooduel {
            game,
            ..Default::default()
        }
    }

    /// A bots-off match sitting in the draw phase with a known word.
    fn drawing_game() -> Game {
        let mut g = Game::default();
        g.start_match(
            "Alex",
            Config {
                bots_enabled: false,
                ..Config::default()
            },
        );
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0)); // anchor the clock
        g
    }

    /// Run a bots-off match to the podium (`Final`).
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
            let _ = g.tick(Duration::from_secs(sec));
            sec += 1;
            guard += 1;
            assert!(guard < 10_000, "match should terminate");
        }
    }

    /// The honest per-seat view: the drawer sees the word, a guesser sees blanks —
    /// the word-leak risk is closed by construction (the single `word_display` home).
    #[test]
    fn seat_view_shows_the_word_to_the_drawer_and_blanks_to_a_guesser() {
        let m = model_with(drawing_game());
        let drawer = m.game.current_drawer().unwrap();
        let drawer_view = render_seat_view(&m, drawer, true);
        assert!(
            drawer_view.contains("R O B O T"),
            "the drawer sees the word:\n{drawer_view}"
        );
        let guesser = (drawer + 1) % SEATS;
        let guesser_view = render_seat_view(&m, guesser, true);
        assert!(
            !guesser_view.to_uppercase().contains("ROBOT"),
            "a guesser never sees the word:\n{guesser_view}"
        );
    }

    /// Bug #4 (host): a near-miss nudge is private — it shows in the guesser's view
    /// but not in any other seat's view nor in the shared chat feed.
    #[test]
    fn seat_view_hides_a_private_near_miss_from_other_seats() {
        let mut g = drawing_game();
        assert_eq!(g.apply_guess(1, "robott"), GuessOutcome::Close);
        let m = model_with(g);
        assert!(
            render_seat_view(&m, 1, true).contains("So close"),
            "the guesser sees their own private nudge"
        );
        for other in [0usize, 2, 3] {
            assert!(
                !render_seat_view(&m, other, true).contains("So close"),
                "seat {other} must not see seat 1's private near-miss nudge"
            );
        }
        assert!(
            !render_shared_chat(&m.game).contains("So close"),
            "the shared chat feed never carries a private nudge"
        );
    }

    /// Bug #4 (host): a WRONG guess is broadcast — it appears in the shared chat feed
    /// and in every seat's view (name + literal text).
    #[test]
    fn wrong_guess_is_broadcast_to_the_shared_chat() {
        let mut g = drawing_game();
        assert_eq!(g.apply_guess(2, "banana"), GuessOutcome::Wrong);
        let m = model_with(g);
        assert!(
            render_shared_chat(&m.game).contains("banana"),
            "the shared chat feed carries the broadcast wrong guess"
        );
        for seat in 0..SEATS {
            assert!(
                render_seat_view(&m, seat, true).contains("banana"),
                "seat {seat} sees the broadcast wrong guess"
            );
        }
    }

    /// Bug #2 (host): at the podium no seat is tagged "drawing" and there is no
    /// active drawer / drawer name.
    #[test]
    fn no_drawer_is_reported_at_the_podium() {
        let mut g = Game::default();
        g.start_match(
            "Alex",
            Config {
                bots_enabled: false,
                ..Config::default()
            },
        );
        drive_to_final(&mut g);
        assert_eq!(g.phase, Phase::Final);
        assert_eq!(g.current_drawer(), None);
        assert_eq!(g.drawer_name(), None);
        let m = model_with(g);
        for seat in 0..SEATS {
            let v = render_seat_view(&m, seat, true);
            assert!(
                !v.contains("(drawing"),
                "no seat is tagged drawing at the podium:\n{v}"
            );
            assert!(
                v.contains("no active drawer"),
                "the podium view names no drawer:\n{v}"
            );
        }
    }

    /// Bug #1 (host): the reported round is clamped by `round_display` — the podium
    /// never reads the "Round 2/1" overflow.
    #[test]
    fn round_display_is_clamped_at_the_podium() {
        let mut g = Game::default();
        g.start_match(
            "Alex",
            Config {
                bots_enabled: false,
                ..Config::default()
            },
        );
        drive_to_final(&mut g);
        let total = g.config.total_rounds;
        assert_eq!(g.round_display(), total);
        let m = model_with(g);
        let v = render_seat_view(&m, 0, true);
        assert!(
            v.contains(&format!("Round {total}/{total}")),
            "the podium round reads within total:\n{v}"
        );
    }

    /// Bug #3 (host): the drawer's view shows the live guessed count in-phase.
    #[test]
    fn seat_view_shows_the_live_guessed_count_during_drawing() {
        let mut g = drawing_game();
        g.apply_guess(1, "robot");
        let m = model_with(g);
        let drawer = m.game.current_drawer().unwrap();
        let v = render_seat_view(&m, drawer, true);
        assert!(
            v.contains("Guessed so far:** 1 / 3"),
            "the drawer sees the live guessed count:\n{v}"
        );
    }

    /// Bug #6 (host): the reported countdown is the wall-clock `now − anchor`
    /// accessor, NOT a frame/tick count — a burst of sub-second re-polls (many
    /// frames, same wall-second) does not move it; each whole wall-second is one
    /// step down. A frame-count countdown would race ahead of wall time.
    #[test]
    fn countdown_is_wall_clock_not_a_frame_count() {
        let mut g = drawing_game();
        assert_eq!(countdown(&g), g.config.draw_seconds);
        for s in 1..=10 {
            g.tick(Duration::from_secs(s));
        }
        assert_eq!(countdown(&g), g.config.draw_seconds - 10);
        // 30 extra frames all inside wall-second 10 must not advance the countdown.
        for _ in 0..30 {
            g.tick(Duration::from_millis(10_400));
        }
        assert_eq!(
            countdown(&g),
            g.config.draw_seconds - 10,
            "frames don't advance the countdown; only wall-seconds do"
        );
    }
}
