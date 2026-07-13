//! `#[ignore]` GPU-lane smoke for the qa_seat driver (spec §6). Self-spawns dooduel_server +
//! ≥3 qa_seat example processes; drives create→join→draw→guess through the file protocol.
//! Run: RUST_MIN_STACK=33554432 cargo test -p dooduel --test qa_seat_smoke -- --ignored --test-threads=1
//!
//! It is `#[ignore]` because it needs a real wgpu adapter (each seat renders its own world)
//! and spawns child processes; it compiles under `--all-targets` but never runs in the
//! headless CI gate. Pre-build the two binaries it launches (the panic messages say how if
//! they are missing):
//!   RUST_MIN_STACK=33554432 cargo build -p dooduel_server --locked
//!   RUST_MIN_STACK=33554432 cargo build -p dooduel --example qa_seat --locked
//!
//! ## Parser calibration (HARD GATE — the exact `snapshot_report` line formats)
//!
//! `room_code` / `first_word_choice` key off `snapshot_report`'s line formats. These are the
//! REAL lines captured from a 2-seat manual gate run (W1 Task 1.4,
//! `docs/reports/2026-07-09-qa-seat-driver-assets/ui-samples.txt`) — the parsers below are
//! shaped to match them verbatim (indentation, quoting):
//!
//!   # room-code line (Lobby ui.md "--- text & layout ---" section; a bare role-less Text):
//!   size=75x29 text="39R5OM"
//!
//!   # word-choice Button lines (Picking-overlay ui.md role tree; name = the UPPERCASED word):
//!   Button "UMBRELLA" @411,315 460x80
//!   Button "ZEPPELIN" @411,411 460x80
//!   Button "JELLYFISH" @411,507 460x80

use std::cell::Cell;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// `.../target/<profile>` — parent of both `dooduel_server` and `examples/qa_seat`. The test
/// binary lives in `.../target/<profile>/deps/`, so pop twice. (apps/dooduel's tests have no
/// CARGO_BIN_EXE_dooduel_server — that's only set in dooduel_server's own package.)
fn target_dir() -> PathBuf {
    let mut p = std::env::current_exe().expect("current_exe");
    p.pop(); // test bin name
    if p.file_name().is_some_and(|n| n == "deps") {
        p.pop(); // 'deps'
    }
    p
}
fn server_bin() -> PathBuf {
    target_dir().join("dooduel_server")
}
fn qa_seat_bin() -> PathBuf {
    target_dir().join("examples").join("qa_seat")
}

struct Server {
    child: Child,
    port: u16,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn spawn_server(config: &Path) -> Server {
    assert!(
        server_bin().exists(),
        "dooduel_server binary missing at {} — build it: \
         RUST_MIN_STACK=33554432 cargo build -p dooduel_server --locked",
        server_bin().display()
    );
    let mut child = Command::new(server_bin())
        .args(["--port", "0", "--config"])
        .arg(config)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn dooduel_server");
    let stdout = child.stdout.take().expect("server stdout");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(p) = line.strip_prefix("LISTENING port=") {
                let _ = tx.send(p.trim().parse::<u16>().expect("numeric port"));
                break;
            }
        }
    });
    let port = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("server printed LISTENING within 30s");
    Server { child, port }
}

struct Seat {
    child: Child,
    dir: PathBuf,
    /// The next `consumed: K` index this seat's driver will log. The driver counts EVERY
    /// `\n`-terminated line and the smoke writes exactly one line per [`Seat::send`], so this
    /// counter stays equal to the driver's `consumed_k` — [`Seat::wait_consumed`] keys off it.
    next_k: Cell<u64>,
}
impl Drop for Seat {
    fn drop(&mut self) {
        // Ask it to quit, then kill.
        let _ = append_cmd(&self.dir, r#"{"cmd":"quit"}"#);
        std::thread::sleep(Duration::from_millis(300));
        let _ = self.child.kill();
    }
}
impl Seat {
    /// Append one command line; return the 0-based `consumed: K` index the driver will log it
    /// under. Pair with [`Seat::wait_consumed`] to gate the NEXT command on this one's ack.
    fn send(&self, line: &str) -> u64 {
        let k = self.next_k.get();
        append_cmd(&self.dir, line).expect("append command");
        self.next_k.set(k + 1);
        k
    }

    /// Block until the driver logged `consumed: {k} → …` (the ack it writes AFTER applying the
    /// command). AMENDMENT 1 (W1 re-gate finding): gate a submit-click on its preceding
    /// `set_value`'s ack so the pair never batches into one drain — a same-drain
    /// `set_value`+`Send` can submit before the value folds into the MVU model.
    fn wait_consumed(&self, k: u64, secs: u64) {
        let log = self.dir.join("driver.log");
        let prefix = format!("consumed: {k} ");
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            if let Ok(s) = std::fs::read_to_string(&log)
                && s.lines().any(|l| l.starts_with(&prefix))
            {
                return;
            }
            if Instant::now() > deadline {
                let got = std::fs::read_to_string(&log).unwrap_or_default();
                panic!(
                    "{:?} driver.log never logged {prefix:?} within {secs}s. Got:\n{got}",
                    self.dir
                );
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

fn spawn_seat(port: u16, name: &str, root: &Path) -> Seat {
    assert!(
        qa_seat_bin().exists(),
        "qa_seat example binary missing at {} — build it: \
         RUST_MIN_STACK=33554432 cargo build -p dooduel --example qa_seat --locked",
        qa_seat_bin().display()
    );
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("seat dir");
    let child = Command::new(qa_seat_bin())
        .args(["--dir"])
        .arg(&dir)
        .args(["--url", &format!("ws://127.0.0.1:{port}"), "--name", name])
        .env("RUST_MIN_STACK", "33554432")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn qa_seat");
    Seat {
        child,
        dir,
        next_k: Cell::new(0),
    }
}

fn append_cmd(dir: &Path, line: &str) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("commands.jsonl"))?;
    writeln!(f, "{line}")
}

/// Poll `<dir>/ui.md` until it contains `needle` (or deadline). Returns the matched ui.md.
fn wait_ui_contains(dir: &Path, needle: &str, secs: u64) -> String {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Ok(s) = std::fs::read_to_string(dir.join("ui.md"))
            && s.contains(needle)
        {
            return s;
        }
        if Instant::now() > deadline {
            let got = std::fs::read_to_string(dir.join("ui.md")).unwrap_or_default();
            panic!("{dir:?} ui.md never contained {needle:?} within {secs}s. Got:\n{got}");
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Poll `<dir>/ui.md` until a field reports the exact state token `value="expected"` — the
/// `snapshot_report` rendering of a folded `TextInput` value (`report.rs:196`,
/// `value={value:?}`). AMENDMENT 1: proves a `set_value` folded into the field BEFORE the
/// submit-click reads it (belt-and-braces with [`Seat::wait_consumed`]: the ack proves the
/// command ran, this proves the resulting value is on screen).
fn wait_value(dir: &Path, expected: &str, secs: u64) {
    let needle = format!("value={expected:?}"); // e.g. value="39R5OM"
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Ok(s) = std::fs::read_to_string(dir.join("ui.md"))
            && s.contains(&needle)
        {
            return;
        }
        if Instant::now() > deadline {
            let got = std::fs::read_to_string(dir.join("ui.md")).unwrap_or_default();
            panic!("{dir:?} ui.md never showed {needle:?} within {secs}s. Got:\n{got}");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn write_config(root: &Path) -> PathBuf {
    let p = root.join("qa-server.toml");
    // Wide phase timers (spec §6 operational dependency): the smoke drives commands
    // programmatically so it is fast, but the auto-pick / draw / reveal timeouts must not fire
    // mid-flow. bots=false so every seat is driver-controlled.
    std::fs::write(
        &p,
        "[room]\nrounds = 1\ndraw_seconds = 150\npick_seconds = 30\nreveal_seconds = 12\nhints = 2\nbots = false\n",
    )
    .expect("write config");
    p
}

/// Parse the 6-char room code out of a Lobby ui.md text section (`… text="ABC123"`). Matches
/// the calibrated sample `size=75x29 text="39R5OM"` — scan every line for a `text="…"` whose
/// value is 6 uppercase/digit chars.
fn room_code(ui: &str) -> String {
    for line in ui.lines() {
        if let Some(rest) = line.split("text=\"").nth(1) {
            let code: String = rest.chars().take_while(|c| *c != '"').collect();
            if code.len() == 6
                && code
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            {
                return code;
            }
        }
    }
    panic!("no 6-char room code in ui.md:\n{ui}");
}

/// The first word-choice button label (UPPERCASE) from a pick-overlay ui.md. Matches the
/// calibrated samples `Button "UMBRELLA" @411,315 460x80` (etc.) — a role-tree `Button "…"`
/// line whose name is ≥2 all-uppercase ASCII letters. (`line.trim()` absorbs the tree
/// indentation; the theme toggle "Light"/"Dark" is mixed-case so it never matches.)
fn first_word_choice(ui: &str) -> String {
    for line in ui.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Button \"") {
            let label: String = rest.chars().take_while(|c| *c != '"').collect();
            if label.len() >= 2 && label.chars().all(|c| c.is_ascii_uppercase()) {
                return label;
            }
        }
    }
    panic!("no UPPERCASE word-choice Button in pick ui.md:\n{ui}");
}

/// The top-of-window band (top-bar + header card) where the ~1 Hz countdown + drawer-progress
/// repaint live (`in_game.rs` top_bar/header_card; the timer number at `in_game.rs:392`). At
/// the FIXED 1280×800 desktop layout the canvas + toolbar sit BELOW this, so cropping it out
/// isolates the stroke from the timer confound.
///
/// **Calibrated 2026-07-10 against a real drawing-phase `screen.png` from this smoke's own run**
/// (AMENDMENT 2, host AMD RX 6700 XT / RADV): the header content — the "GUESS THE WORD" masked
/// slots + progress dots + countdown text — sits entirely in rows 0..~200; the Game canvas rect
/// is `@40,220 … 700x525` (top edge y=220), so the toolbar/canvas region is rows ≥220. A
/// HEADER_SKIP of 220 excludes the whole header (timer included) and keeps the entire canvas.
/// See the W3 journal for the measured screenshot/ui.md.
const HEADER_SKIP_PX: u32 = 220;

/// Poll until the CANVAS REGION of screen.png differs from `before` — decoding both PNGs and
/// comparing only rows `[HEADER_SKIP_PX..H]`, so the ~1 Hz countdown repaint in the header
/// (`in_game.rs:392`, `text!("{}", secs)`) can't masquerade as ink. A raw PNG byte-diff (a
/// naive `now != before`) is CONFOUNDED: the whole file changes every second regardless of the
/// stroke. Requires the `image` crate (a `dooduel` dep).
fn wait_canvas_changed(dir: &Path, before: &[u8], secs: u64) {
    let base = image::load_from_memory(before)
        .expect("decode pre-stroke png")
        .to_rgba8();
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Ok(bytes) = std::fs::read(dir.join("screen.png"))
            && let Ok(img) = image::load_from_memory(&bytes)
        {
            let now = img.to_rgba8();
            if now.dimensions() == base.dimensions() {
                let (w, h) = now.dimensions();
                let mut diff = 0u32;
                for y in HEADER_SKIP_PX..h {
                    for x in 0..w {
                        if now.get_pixel(x, y) != base.get_pixel(x, y) {
                            diff += 1;
                        }
                    }
                }
                if diff > 200 {
                    return; // the stroke inked the canvas region (timer band excluded)
                }
            }
        }
        if Instant::now() > deadline {
            panic!(
                "{dir:?} canvas region (rows {HEADER_SKIP_PX}..) never changed after the stroke within {secs}s"
            );
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[test]
#[ignore = "GPU-lane: needs a real wgpu adapter + spawns processes"]
fn three_seats_create_join_draw_guess() {
    let root = std::env::temp_dir().join(format!("qa-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    // Seat dirs + logs live here; removed on SUCCESS at the end, KEPT on failure (a panic
    // unwinds before the cleanup) for post-mortem. Print the path so a failure is findable.
    eprintln!(
        "qa_seat_smoke: artifacts under {} (removed on success, kept on failure)",
        root.display()
    );
    let cfg = write_config(&root);
    let server = spawn_server(&cfg);

    // ≥3 concurrent seats (spec §C4 multi-render-world regime): each its own process + wgpu
    // render world on the one host adapter.
    let host = spawn_seat(server.port, "Host", &root);
    let p2 = spawn_seat(server.port, "Priya", &root);
    let p3 = spawn_seat(server.port, "Theo", &root);

    // Checkpoint 1 — the composed-stack boot: if a seat boots the composition + renders, its
    // first ui.md appears (proves boot + snapshot; the later clicks prove the pointer path).
    wait_ui_contains(&host.dir, "Create a room", 60);
    wait_ui_contains(&p2.dir, "Create a room", 60);
    wait_ui_contains(&p3.dir, "Create a room", 60);

    // Checkpoint 2 — host creates via UI; read the code from the Lobby.
    let k = host.send(r#"{"cmd":"click","role":"Button","name":"Create a room"}"#);
    host.wait_consumed(k, 30);
    let ui = wait_ui_contains(&host.dir, "Copy", 30); // the Lobby's "Copy" button
    let code = room_code(&ui);

    // Checkpoint 3 — p2 + p3 join by the corrected flow. AMENDMENT 1: set_value and the
    // "Join room" submit are SEPARATE settled steps — send set_value, wait its ack AND the
    // `value="CODE"` on screen, THEN send the submit click (never batched).
    for seat in [&p2, &p3] {
        let k = seat.send(r#"{"cmd":"click","role":"Button","name":"Join a room"}"#);
        seat.wait_consumed(k, 30);
        wait_ui_contains(&seat.dir, "Enter a room code", 30); // the Join screen is up
        let k = seat.send(&format!(
            r#"{{"cmd":"set_value","role":"TextInput","text":"{code}"}}"#
        ));
        seat.wait_consumed(k, 30);
        wait_value(&seat.dir, &code, 30); // the code folded into the field
        let k = seat.send(r#"{"cmd":"click","role":"Button","name":"Join room"}"#);
        seat.wait_consumed(k, 30);
    }
    wait_ui_contains(&host.dir, "Priya", 30);
    wait_ui_contains(&host.dir, "Theo", 30);

    // Checkpoint 4 — start; host is seat 0, the first drawer (game.rs:1030). Read one word
    // choice from the pick overlay and click it → Drawing.
    let k = host.send(r#"{"cmd":"click","role":"Button","name":"▶ Start game"}"#);
    host.wait_consumed(k, 30);
    let pick_ui = wait_ui_contains(&host.dir, "Pick a word", 30);
    let word = first_word_choice(&pick_ui);
    let k = host.send(&format!(
        r#"{{"cmd":"click","role":"Button","name":"{word}"}}"#
    ));
    host.wait_consumed(k, 30);
    wait_ui_contains(&host.dir, "Brush", 30); // the drawer's Drawing toolbar is up

    // Checkpoint 5 — select label-only toolbar buttons (the `.label()`-derived accessible-name
    // path a text-only "Brush" click never exercises), settle, capture the pre-stroke frame,
    // THEN stroke — so the only canvas-region change vs. `after` is the stroke itself.
    let mut k = 0;
    for c in [
        r#"{"cmd":"click","role":"Button","name":"Brush"}"#,
        r#"{"cmd":"click","role":"Button","name":"Color 3"}"#,
        r#"{"cmd":"click","role":"Button","name":"Brush size 6"}"#,
    ] {
        k = host.send(c);
    }
    host.wait_consumed(k, 30);
    std::thread::sleep(Duration::from_secs(2)); // let the selection settle into screen.png
    let before = std::fs::read(host.dir.join("screen.png")).expect("pre-stroke screen.png");
    let k = host.send(r#"{"cmd":"stroke","points":[[120,90],[300,110],[480,300]]}"#);
    host.wait_consumed(k, 30);
    wait_canvas_changed(&host.dir, &before, 30);

    // Checkpoint 6 — a guesser guesses. AMENDMENT 1: set_value + Send as separate settled
    // steps. DEVIATION from the skeleton's `word.to_lowercase()` assertion: a CORRECT guess is
    // spoiler-safe — the game broadcasts `ChatKind::Correct` "{name} guessed the word!" to
    // everyone (game.rs:541) and withholds the word from chat (the guesser's word row upgrades
    // via a per-seat WordUpdate, session.rs:666, but as the UPPERCASE display, not the typed
    // lowercase). So the reliable "the guess reached chat" signal is the "guessed the word"
    // marker, not the redacted word.
    let word_l = word.to_lowercase();
    let k = p2.send(&format!(
        r#"{{"cmd":"set_value","role":"TextInput","text":"{word_l}"}}"#
    ));
    p2.wait_consumed(k, 30);
    wait_value(&p2.dir, &word_l, 30); // the guess folded into the chat field before Send
    let k = p2.send(r#"{"cmd":"click","role":"Button","name":"Send"}"#);
    p2.wait_consumed(k, 30);
    wait_ui_contains(&p2.dir, "guessed the word", 30); // the Correct chat line landed

    drop((host, p2, p3, server)); // Drop order kills seats then server.
    // Success only: remove the artifacts. A panic above unwinds past this, keeping them.
    std::fs::remove_dir_all(&root).ok();
}
