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
//!   everyone else sees blanks + hint-revealed letters. Refreshed after every
//!   command and on a heartbeat (so countdowns advance).
//! - `state.json` — machine-readable: `phase`, `drawer`, `round`, `countdown`,
//!   `tick`, `started`, `scores`.
//! - `canvas.png` — the 720×450 drawing surface, written (throttled ≈2 Hz) whenever
//!   the pixels change.
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
use dooduel::game::{Config, Game, Phase};
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
    }

    // --- reporting ---------------------------------------------------------

    fn write_reports(&mut self) {
        let model = self.snapshot();
        let g = &model.game;
        let canvas_ink = self.canvas_ink();

        let scores: Vec<Value> = g
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| json!({"seat": i, "name": p.name, "score": p.score}))
            .collect();
        let state = json!({
            "phase": phase_str(g.phase),
            "screen": screen_str(&model.screen),
            "started": self.started,
            "drawer": g.seat_index,
            "drawer_name": g.players.get(g.seat_index).map(|p| p.name.as_str()).unwrap_or(""),
            "round": g.round,
            "total_rounds": g.config.total_rounds,
            "countdown": countdown(g),
            "tick": self.tick,
            "canvas_ink": canvas_ink,
            "scores": scores,
        });
        write_atomic(
            &self.settings.dir.join("state.json"),
            serde_json::to_string_pretty(&state)
                .unwrap_or_default()
                .as_bytes(),
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
            "phase={:?} round={}/{} drawer={} countdown={}s tick={}",
            g.phase,
            g.round,
            g.config.total_rounds,
            g.seat_index,
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

    let drawer = g.seat_index;
    let drawer_name = g
        .players
        .get(drawer)
        .map(|p| p.name.as_str())
        .unwrap_or("—");
    out.push_str(&format!(
        "**Phase:** {} · **Round {}/{}** · **{}s left**\n",
        phase_str(g.phase),
        g.round,
        g.config.total_rounds,
        countdown(g),
    ));
    let you_draw = if seat == drawer {
        " — that's YOU"
    } else {
        ""
    };
    out.push_str(&format!(
        "**Drawing:** {drawer_name} (seat {drawer}){you_draw}\n\n"
    ));

    // Word — as THIS seat sees it (never leaks).
    let word = word_as_seen_by(g, seat);
    if word.is_empty() {
        out.push_str("**Word:** —\n\n");
    } else {
        out.push_str(&format!("**Word:** {word}\n\n"));
    }

    // Scoreboard (sorted high→low), marking the drawer + correct guessers.
    out.push_str("## Scoreboard\n");
    for (rank, (i, p)) in g.standings().into_iter().enumerate() {
        let mut tags = Vec::new();
        if i == drawer {
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

    // Chat tail (last 15) — safe for all seats (the secret only appears at reveal).
    out.push_str("## Chat (recent)\n");
    let tail = g.chat.len().saturating_sub(15);
    for m in &g.chat[tail..] {
        out.push_str(&format!("- {}\n", m.text));
    }
    out.push('\n');

    // Seat-specific available actions.
    out.push_str("## Your actions\n");
    out.push_str(&seat_actions(g, seat));
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
        Phase::Reveal => "Turn over — the next turn starts automatically.\n".to_string(),
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
