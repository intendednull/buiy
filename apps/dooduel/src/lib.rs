//! Dooduel — a fully-featured skribbl.io-style draw-and-guess game, shipped on the
//! Buiy framework.
//!
//! One MVU model owns the whole UI (one `ui()` per app); screens are a [`Screen`]
//! enum matched in [`view::view`] (root kind-swap). The match state lives in a
//! nested [`game::Game`] (that module is the pure state machine + the clock model).
//! The reducer is one pure fold; the F7 [`ClockPlugin`] turns wall-clock into a
//! `Msg::Tick(now)` every frame — the "game on a game engine" seam. The windowed
//! `dooduel` bin, the wasm `dooduel_web` crate, the headless `capture` bin, and the
//! `playtest_host` all share [`install`] / [`install_runtime`].
//!
//! ## Module map (the per-screen split)
//!
//! - [`game`] — the PURE game core (phase machine, scoring, hints, seeded bots, the
//!   honest `word_display()` redaction). Zero framework coupling; unit-testable.
//! - [`paint`] — the keyed `PaintCanvases` resource + the model→canvas projection +
//!   the Press/Drag/Release observers (the drawing surface skribbl.io needs).
//! - [`storage`] — the typed per-target persistence seam (native JSON / wasm
//!   localStorage) + avatar PNG-base64 + the `saved_version` race guard.
//! - [`theme`] — the `Palette` LIGHT/DARK ladders + the model→`Theme`-resource sync.
//! - [`avatar`] — the 22 `DoodleAvatar` doodles + `hash_str` + the badge builder.
//! - [`confetti`] — the decoupled podium `ConfettiPlugin` (a rising-edge side effect).
//! - [`view`] — the `Screen` router + the shared widget helpers + the six per-screen
//!   modules (home / join / lobby / in_game / podium / avatar_editor).

use std::time::Duration;

use bevy::asset::Handle;
use bevy::image::Image;
use buiy::prelude::*;
use buiy::view::BuiyViewAppExt;
use buiy_core::mvu::{Cmd, MvuSet, enqueue};

pub mod avatar;
pub mod confetti;
pub mod game;
pub mod paint;
pub mod storage;
pub mod theme;
pub mod view;

use game::{Config, Game, Phase};
use theme::ThemePref;

/// The drawing canvas size in logical px (matches the paint image resolution so
/// window px → texel is 1:1). Shared by the view (raster element size) + paint.
pub use paint::{CANVAS_H, CANVAS_W};

/// The responsive breakpoint (logical px): below this window width the app lays out
/// for a phone (single-column in-game, narrower cards). The design has no automatic
/// switch — it toggles a manual `layout` prop — so the FINAL derives it from the
/// real viewport width, at ≤430px (a real phone; design spec §4.4 / finding #19).
pub const MOBILE_BREAKPOINT: f32 = 430.0;

/// MODEL — the single source of truth for the whole app.
#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct Dooduel {
    pub screen: Screen,
    pub player_name: String,
    pub game: Game,
    /// The drawing-canvas image the in-game `raster(...)` element samples (the
    /// canvas lives INSIDE the view tree, not as a side ECS root). The `paint`
    /// plugin owns the pixels + creates the `Image`, then announces its handle
    /// through the funnel (`Msg::CanvasesReady`) so `view` can place it.
    pub canvas: Handle<Image>,
    /// The invite code shown in the Lobby (create → generated; join → the entered
    /// code, or a generated one).
    pub room_code: String,
    /// The Join screen's code field.
    pub join_code: String,
    /// Whether this seat hosts the Lobby (create ⇒ host, join ⇒ guest). Drives the
    /// host-gated Start vs the joiner "waiting" state + the roster badges.
    pub is_host: bool,
    /// The drawing toolbar's selection. Reducer-owned (so tool changes replay),
    /// mirrored onto the `PaintCanvases` resource each frame by
    /// `paint::sync_tools_to_canvases`. The pixel buffer itself lives outside the
    /// model (the canvas is a side surface), so clear/undo are monotonic request
    /// counters the sync applies.
    pub tools: ToolState,
    /// The avatar editor's state: open/tab/draft-brush + which avatar the human
    /// currently wears. Reducer-owned (replayable); the editor's scratch pixel
    /// buffer is a side surface (like the game canvas), so clear/undo/reset/save
    /// are monotonic request counters `paint::sync_tools_to_canvases` drains.
    pub avatar: AvatarState,
    /// The avatar editor's draw-your-own scratch canvas image (220×220) — the 2nd
    /// `raster(...)` consumer. The paint plugin creates it + announces the handle
    /// through the funnel (`Msg::CanvasesReady`).
    pub avatar_canvas: Handle<Image>,
    /// The committed custom-avatar image displayed around the app (updated by a save).
    pub saved_avatar: Handle<Image>,
    /// The light/dark palette preference. Reducer-owned (a `SetTheme` folds through
    /// the funnel → replayable), mirrored onto the `Theme` RESOURCE by
    /// `theme::sync_theme_resource`, and persisted (`storage`). Loaded at boot.
    pub theme: ThemePref,
    /// The window's logical size. Fed by [`ViewportPlugin`] via `Msg::SetViewport`
    /// (folded `set_if_neq`-clean — it only changes on a real resize). `0.0` until
    /// the first measurement; [`Dooduel::is_mobile`] derives the breakpoint from it.
    pub viewport_w: f32,
    pub viewport_h: f32,
}

impl Dooduel {
    /// The active color ladder for the current theme. The view threads this so every
    /// surface swaps on a `SetTheme`.
    pub fn palette(&self) -> theme::Palette {
        self.theme.palette()
    }

    /// Whether to lay out for a phone: the window is narrower than
    /// [`MOBILE_BREAKPOINT`]. Defaults to desktop until the viewport is measured
    /// (`viewport_w == 0.0`), so headless/probe views stay on the desktop layout.
    pub fn is_mobile(&self) -> bool {
        self.viewport_w > 0.0 && self.viewport_w < MOBILE_BREAKPOINT
    }
}

/// The avatar editor's reducer-owned state. Mirrors the design's `showAvatarEditor`
/// / `avatarEditorTab` / `avatarDraft*` + the committed avatar.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct AvatarState {
    /// Whether the editor modal is open (reached from Home's pencil affordance).
    pub editor_open: bool,
    /// The active tab (pick-a-doodle gallery vs draw-your-own).
    pub tab: AvatarTab,
    /// What the human seat currently wears (default hash / gallery preset / drawn).
    pub kind: HumanAvatar,
    /// Index into `paint::PALETTE` for the draw brush (default 0 = ink).
    pub draft_color_idx: usize,
    /// Index into `paint::BRUSH_SIZES` (default 1 = 6px).
    pub draft_size_idx: usize,
    /// Whether the eraser is active (a toggle, per the design's `avatarDraftEraser`).
    pub draft_eraser: bool,
    /// Bumped by `AvatarClear` — the sync clears the scratch once per new value.
    pub clear_seq: u64,
    /// Bumped by `AvatarUndo` — the sync pops one undo snapshot per new value.
    pub undo_seq: u64,
    /// Bumped when the scratch should reset (tab→draw) — the sync blanks it once.
    pub reset_seq: u64,
    /// Bumped by `SaveAvatar` — the sync copies the scratch into the saved image.
    pub save_seq: u64,
}

impl Default for AvatarState {
    fn default() -> Self {
        AvatarState {
            editor_open: false,
            tab: AvatarTab::Gallery,
            kind: HumanAvatar::Default,
            draft_color_idx: 0,
            draft_size_idx: 1,
            draft_eraser: false,
            clear_seq: 0,
            undo_seq: 0,
            reset_seq: 0,
            save_seq: 0,
        }
    }
}

/// The avatar editor's two tabs (design `avatarTabOptions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum AvatarTab {
    /// Pick one of the 22 stock doodles.
    #[default]
    Gallery,
    /// Draw your own on the 220×220 canvas.
    Draw,
}

/// The human seat's avatar source: the name-hashed doodle (the default), a gallery
/// preset (an explicit icon + tint), or the drawn custom image.
#[derive(Debug, Clone, Copy, PartialEq, Reflect, Default)]
pub enum HumanAvatar {
    #[default]
    Default,
    Preset {
        icon: usize,
        tint: usize,
    },
    Custom,
}

/// The in-game drawing toolbar's state — held on the model so the reducer owns tool
/// selection (replayable) and the view renders directly from it. The `*_seq` fields
/// are monotonic request counters: the reducer can't touch the out-of-model canvas
/// pixel buffer, so `ClearCanvas`/`UndoStroke` bump a counter that
/// `paint::sync_tools_to_canvases` drains into a real `clear()`/`undo()`.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ToolState {
    /// Brush / Eraser / Bucket (the design's brush/eraser/fill segmented control).
    pub tool: paint::Tool,
    /// Index into `paint::PALETTE` (the 16-color swatch row). Default 0 = ink.
    pub color_idx: usize,
    /// Index into `paint::BRUSH_SIZES` (the four brush dots). Default 1 = 6px.
    pub size_idx: usize,
    /// Bumped by `ClearCanvas` — the sync clears the sheet once per new value.
    pub clear_seq: u64,
    /// Bumped by `UndoStroke` — the sync pops one undo snapshot per new value.
    pub undo_seq: u64,
}

impl Default for ToolState {
    fn default() -> Self {
        // Design defaults: brush, ink (index 0), 6px brush (index 1).
        ToolState {
            tool: paint::Tool::Brush,
            color_idx: 0,
            size_idx: 1,
            clear_seq: 0,
            undo_seq: 0,
        }
    }
}

/// Which screen the app is showing. `InGame`/`Podium` read [`Dooduel::game`].
#[derive(Default, Debug, Clone, PartialEq, Reflect)]
pub enum Screen {
    #[default]
    Home,
    /// Enter-a-room-code screen (reached from Home's "Join a room").
    Join,
    Lobby,
    InGame,
    Podium,
}

impl Model for Dooduel {
    type Msg = Msg;
}

/// The app's messages. `Tick` is the per-frame clock; `Guess` is the shared funnel
/// entry for *both* human submits and bot fires (see [`update`]).
#[derive(Clone, Debug, PartialEq, Reflect)]
pub enum Msg {
    // Navigation.
    SetName(String),
    /// Home's "▶ Play" — start the match immediately (the design's primary CTA).
    Play,
    /// Home's "Create a room" — open the Lobby as host with a fresh room code.
    CreateRoom,
    /// Home's "Join a room" — open the Join-code screen.
    GoJoin,
    /// The Join screen's code field.
    SetJoinCode(String),
    /// Join screen submit — drop into the Lobby as a guest.
    SubmitJoin,
    Back,
    StartMatch,
    PlayAgain,
    /// Swap the light/dark palette (the floating theme toggle). Folds through the
    /// funnel (replayable) and is persisted by the storage sink.
    SetTheme(ThemePref),
    /// Boot-restore persisted state: the saved theme + name + avatar choice, folded
    /// once at startup by `storage::load_at_boot`. (The custom-avatar PIXELS are
    /// restored directly onto the paint surface, a side channel; this carries only
    /// the model fields.)
    Restore {
        theme: ThemePref,
        name: String,
        avatar: HumanAvatar,
    },
    /// The paint plugin announcing the drawing-canvas image handles (values, so they
    /// fold through the funnel like any other message — the view then places the
    /// `raster(...)` elements sampling them): the in-game sheet, the avatar editor's
    /// scratch, and the committed custom avatar.
    CanvasesReady {
        game: Handle<Image>,
        avatar: Handle<Image>,
        saved: Handle<Image>,
    },
    // The clock (folded every frame; a steady frame is a `set_if_neq` no-op).
    Tick(Duration),
    /// The window's logical size changed (the `ViewportPlugin` seam). Folded
    /// `set_if_neq`-clean: only a real resize changes the model, so it never forces a
    /// rebuild on a steady frame (the same discipline as `Tick`).
    SetViewport(f32, f32),
    // In-turn.
    ChooseWord(usize),
    SwitchSeat(usize),
    SetChatInput(String),
    SubmitGuess,
    /// Force-advance out of the turn-end reveal (the "Continue →" button).
    Continue,
    // Toolbar (reducer-owned so tool selection replays; mirrored to the
    // `PaintCanvases` by `paint::sync_tools_to_canvases`).
    SelectTool(paint::Tool),
    SelectColor(usize),
    SelectSize(usize),
    ClearCanvas,
    UndoStroke,
    // Avatar editor (reducer-owned so the editor state replays; the scratch pixel
    // buffer is a side surface `paint::sync_tools_to_canvases` mirrors).
    /// Open the avatar editor (Home's pencil affordance).
    OpenAvatarEditor,
    /// Close the avatar editor without committing.
    CloseAvatarEditor,
    /// Switch the editor tab (gallery ↔ draw).
    SetAvatarTab(AvatarTab),
    /// Pick stock doodle `i` from the gallery (sets a preset avatar, closes).
    PickGalleryIcon(usize),
    /// Select the draw brush color `i` (index into `paint::PALETTE`).
    SelectAvatarColor(usize),
    /// Select the draw brush size `i` (index into `paint::BRUSH_SIZES`).
    SelectAvatarSize(usize),
    /// Toggle the avatar-canvas eraser.
    ToggleAvatarEraser,
    /// Undo the last avatar stroke.
    AvatarUndo,
    /// Clear the avatar canvas.
    AvatarClear,
    /// Commit the drawn avatar (copies the scratch into the saved image, closes).
    SaveAvatar,
    /// Drop back to the name-hashed default avatar (closes).
    ResetAvatar,
    /// A guess attributed to a specific seat — the shared pipeline entry. Human
    /// submits arrive as `SubmitGuess` (reads `chat_input`); bots emit this from
    /// `Tick`, so both fold through `game::Game::apply_guess`.
    Guess {
        player: usize,
        text: String,
    },
}

/// UPDATE — the pure reducer. Thin shell over the pure [`game::Game`] methods.
pub fn update(s: &mut Dooduel, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::SetName(name) => s.player_name = name,
        // "▶ Play" starts the match directly (the design's primary CTA); the Lobby
        // is only reached via Create/Join.
        Msg::Play => {
            s.game.start_match(&s.player_name, Config::default());
            s.screen = Screen::InGame;
        }
        Msg::CreateRoom => {
            s.is_host = true;
            s.room_code = gen_room_code(&s.player_name);
            s.screen = Screen::Lobby;
        }
        Msg::GoJoin => {
            s.join_code.clear();
            s.screen = Screen::Join;
        }
        Msg::SetJoinCode(code) => s.join_code = code.to_uppercase(),
        Msg::SubmitJoin => {
            s.is_host = false;
            s.room_code = if s.join_code.trim().is_empty() {
                gen_room_code(&s.player_name)
            } else {
                s.join_code.trim().to_uppercase()
            };
            s.screen = Screen::Lobby;
        }
        Msg::Back => {
            s.screen = Screen::Home;
            s.game = Game::default();
        }
        Msg::StartMatch | Msg::PlayAgain => {
            s.game.start_match(&s.player_name, Config::default());
            s.screen = Screen::InGame;
        }
        Msg::SetTheme(t) => s.theme = t,
        Msg::Restore {
            theme,
            name,
            avatar,
        } => {
            s.theme = theme;
            s.player_name = name;
            s.avatar.kind = avatar;
        }
        Msg::CanvasesReady {
            game,
            avatar,
            saved,
        } => {
            s.canvas = game;
            s.avatar_canvas = avatar;
            s.saved_avatar = saved;
        }
        Msg::SetViewport(w, h) => {
            s.viewport_w = w;
            s.viewport_h = h;
        }
        Msg::Tick(now) => {
            let pending = s.game.tick(now);
            // A finished match lifts the shell to the podium.
            if s.game.phase == Phase::Final && s.screen != Screen::Podium {
                s.screen = Screen::Podium;
            }
            // Fold each due bot guess back through the funnel as a real `Guess`.
            if !pending.is_empty() {
                return Cmd::Batch(
                    pending
                        .into_iter()
                        .map(|p| {
                            Cmd::emit(Msg::Guess {
                                player: p.player,
                                text: p.text,
                            })
                        })
                        .collect(),
                );
            }
        }
        Msg::ChooseWord(idx) => {
            if let Some(word) = s.game.word_choices.get(idx).cloned() {
                s.game.choose_word(word);
            }
        }
        Msg::SwitchSeat(idx) => s.game.switch_seat(idx),
        Msg::SetChatInput(t) => s.game.chat_input = t,
        Msg::SubmitGuess => {
            let raw = std::mem::take(&mut s.game.chat_input);
            let seat = s.game.viewing_as;
            s.game.apply_guess(seat, &raw);
        }
        Msg::Guess { player, text } => {
            s.game.apply_guess(player, &text);
        }
        Msg::Continue => s.game.continue_now(),
        // Toolbar — plain model writes; the sync projects them onto the canvas.
        // Selecting a color/size does NOT change the tool (design: the swatch/size
        // handlers only set color/size). Ungated: harmless when not the drawer (the
        // sync only *paints* when `enabled`, and the toolbar is dimmed).
        Msg::SelectTool(t) => s.tools.tool = t,
        Msg::SelectColor(i) => s.tools.color_idx = i,
        Msg::SelectSize(i) => s.tools.size_idx = i,
        Msg::ClearCanvas => s.tools.clear_seq = s.tools.clear_seq.wrapping_add(1),
        Msg::UndoStroke => s.tools.undo_seq = s.tools.undo_seq.wrapping_add(1),
        // Avatar editor — reducer-owned state; the scratch buffer is mirrored by
        // `paint::sync_tools_to_canvases` (the seq counters drain into it).
        Msg::OpenAvatarEditor => {
            s.avatar.editor_open = true;
            s.avatar.tab = AvatarTab::Gallery;
        }
        Msg::CloseAvatarEditor => s.avatar.editor_open = false,
        Msg::SetAvatarTab(tab) => {
            s.avatar.tab = tab;
            // Entering the draw tab blanks the scratch (the design's resetAvatarCanvas).
            if tab == AvatarTab::Draw {
                s.avatar.reset_seq = s.avatar.reset_seq.wrapping_add(1);
            }
        }
        Msg::PickGalleryIcon(i) => {
            s.avatar.kind = HumanAvatar::Preset {
                icon: i.min(avatar::ICON_COUNT - 1),
                tint: i % avatar::TINT_COUNT,
            };
            s.avatar.editor_open = false;
        }
        Msg::SelectAvatarColor(i) => {
            s.avatar.draft_color_idx = i;
            s.avatar.draft_eraser = false;
        }
        Msg::SelectAvatarSize(i) => s.avatar.draft_size_idx = i,
        Msg::ToggleAvatarEraser => s.avatar.draft_eraser = !s.avatar.draft_eraser,
        Msg::AvatarUndo => s.avatar.undo_seq = s.avatar.undo_seq.wrapping_add(1),
        Msg::AvatarClear => s.avatar.clear_seq = s.avatar.clear_seq.wrapping_add(1),
        Msg::SaveAvatar => {
            // The sync copies the scratch pixels into the saved image on this bump.
            s.avatar.save_seq = s.avatar.save_seq.wrapping_add(1);
            s.avatar.kind = HumanAvatar::Custom;
            s.avatar.editor_open = false;
        }
        Msg::ResetAvatar => {
            s.avatar.kind = HumanAvatar::Default;
            s.avatar.editor_open = false;
        }
    }
    Cmd::none()
}

/// A deterministic 6-char room invite code from the host's name (design
/// `genRoomCode`-style). Pure — the same name yields the same code.
pub fn gen_room_code(name: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let seed = name.bytes().fold(0x9e37_79b9u32, |h, b| {
        h.wrapping_mul(31).wrapping_add(b as u32)
    });
    let mut x = seed | 1;
    let mut out = String::with_capacity(6);
    for _ in 0..6 {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        out.push(ALPHABET[(x >> 24) as usize % ALPHABET.len()] as char);
    }
    out
}

/// Install Dooduel onto an app already carrying the Buiy plugins. Does **not** add
/// the wall-clock driver — that is the F7 [`ClockPlugin`] (added by
/// [`install_runtime`]), kept separate so the pure logic can be driven by injected
/// `Tick`s in tests (virtual time / `advance_clock`).
pub fn install(app: &mut App) -> &mut App {
    app.register_type::<Screen>();
    app.register_type::<Game>();
    app.register_type::<ToolState>();
    app.register_type::<paint::Tool>();
    app.register_type::<AvatarState>();
    app.register_type::<AvatarTab>();
    app.register_type::<HumanAvatar>();
    app.register_type::<ThemePref>();
    app.ui(Dooduel::default(), update, view::view);
    // Funnel the paint plugin's canvas image handles into the model (once) so the
    // `raster(...)` elements can sample them. Inert when the paint plugin is absent
    // (the GPU-free probe tests install `ui()` without `CanvasPlugin`).
    app.add_systems(Update, announce_canvases.in_set(MvuSet::Enqueue));
    app
}

/// Install Dooduel + ALL its runtime plugins onto an app already carrying the Buiy
/// plugins. The windowed native bin (`main.rs`) and the wasm web bin (`dooduel_web`)
/// share this, so the plugin set never drifts between targets. The capture / probe
/// harnesses use the lower-level [`install`] instead, adding only the plugins they
/// need (the virtual-clock tests deliberately omit the wall-clock [`ClockPlugin`]).
pub fn install_runtime(app: &mut App) -> &mut App {
    install(app);
    // Theme (Caveat/Shantell fonts + the model-driven light/dark `Theme`-resource
    // sync). The F7 poll-clock → `Msg::Tick` driver (replaces the prototype's
    // hand-rolled `GameClockPlugin`; suppressed during replay by construction). The
    // viewport → mobile shell. The drawing canvases (paint pixels + `raster`
    // observers). The podium confetti. Persistence (native file / wasm localStorage).
    app.add_plugins(theme::DooduelThemePlugin);
    app.add_plugins(ClockPlugin::<Dooduel>::new(Msg::Tick));
    app.add_plugins(ViewportPlugin);
    app.add_plugins(paint::CanvasPlugin);
    app.add_plugins(confetti::ConfettiPlugin);
    app.add_plugins(storage::StoragePlugin);
    app
}

/// Announce the drawing-canvas image handles to the model through the funnel,
/// exactly once. `Option<Res<PaintCanvases>>` so it no-ops without the paint plugin;
/// a `Local` latch stops re-enqueueing after the first fold.
fn announce_canvases(
    canvases: Option<Res<paint::PaintCanvases>>,
    model: Option<Single<(Entity, &Dooduel)>>,
    mut commands: Commands,
    mut announced: Local<bool>,
) {
    if *announced {
        return;
    }
    let (Some(canvases), Some(model)) = (canvases, model) else {
        return;
    };
    let (entity, m) = *model;
    let game = canvases.handle(paint::CanvasKind::Game);
    if m.canvas != game {
        enqueue::<Dooduel>(
            &mut commands,
            entity,
            Msg::CanvasesReady {
                game,
                avatar: canvases.handle(paint::CanvasKind::Avatar),
                saved: canvases.saved_avatar.clone(),
            },
        );
        *announced = true;
    }
}

/// Feeds the primary window's LOGICAL size into the model (the responsive shell).
/// Separate from [`install`] so the GPU-free probe tests (which have no window) stay
/// on the default desktop layout. Enqueues a `SetViewport` only when the size
/// actually changes (a `Local` last-seen guard), so it costs nothing on a steady
/// frame; the funnel's `set_if_neq` would absorb a duplicate anyway.
pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drive_viewport.in_set(MvuSet::Enqueue));
    }
}

fn drive_viewport(
    windows: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    model: Single<Entity, With<Dooduel>>,
    mut commands: Commands,
    mut last: Local<Option<(f32, f32)>>,
) {
    let Ok(window) = windows.single() else {
        return; // no primary window yet (or headless) — try next frame
    };
    // `Window::width/height` are LOGICAL px (physical / scale factor) — the right
    // unit for a CSS-like breakpoint on a HiDPI phone.
    let size = (window.width(), window.height());
    if *last == Some(size) {
        return;
    }
    *last = Some(size);
    enqueue::<Dooduel>(&mut commands, *model, Msg::SetViewport(size.0, size.1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use buiy::probe::{click, get_by_role, snapshot_report};

    // --- Pure game-logic unit tests (no ECS) -------------------------------

    fn started() -> Game {
        let mut g = Game::default();
        g.start_match("Mara", Config::default());
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
        assert_eq!(game::normalize("  Ro-BOT! "), "robot");
        assert_eq!(game::normalize("Ice Cream"), "icecream");
    }

    #[test]
    fn close_matches_within_edit_distance_two() {
        assert!(game::is_close(&game::normalize("robott"), "robot")); // one insert
        assert!(game::is_close("robto", "robot")); // adjacent swap = distance 2
        assert!(!game::is_close("robot", "robot")); // identical is not "close"
        assert!(!game::is_close("banana", "robot")); // far apart
    }

    #[test]
    fn guesser_points_match_the_spec_formula() {
        // Full time left, first guesser: round(50 + 450) = 500.
        assert_eq!(game::guesser_points(80, 80, 0), 500);
        // Half time, first guesser: round(50 + 225) = 275.
        assert_eq!(game::guesser_points(40, 80, 0), 275);
        // Full time, second guesser: round(500 * 0.82) = 410.
        assert_eq!(game::guesser_points(80, 80, 1), 410);
        // Floor at 20 even with no time / high order.
        assert_eq!(game::guesser_points(0, 80, 5), 20);
    }

    #[test]
    fn drawer_points_scale_with_correct_fraction() {
        assert_eq!(game::drawer_points(3, 3), 100);
        assert_eq!(game::drawer_points(2, 3), 67); // round(200/3)
        assert_eq!(game::drawer_points(0, 3), 0);
    }

    #[test]
    fn match_starts_in_pick_phase_with_four_players() {
        let g = started();
        assert_eq!(g.players.len(), 4);
        assert_eq!(g.players[0].name, "Mara");
        assert_eq!(g.phase, Phase::Picking);
        assert_eq!(g.round, 1);
        // Seat auto-jumps to the drawer (seat 0 for turn 1).
        assert_eq!(g.viewing_as, 0);
        assert_eq!(g.word_choices.len(), 3);
    }

    #[test]
    fn pick_timeout_auto_advances_to_drawing() {
        let mut g = started();
        tick_to(&mut g, 0, game::PICK_SECS);
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
        assert_eq!(outcome, game::GuessOutcome::Correct);
        assert_eq!(g.turn_guesses.len(), 1);
        assert_eq!(g.players[1].score, 500);
        // Guessing again is ignored (already locked).
        assert_eq!(g.apply_guess(1, "robot"), game::GuessOutcome::Ignored);
    }

    #[test]
    fn the_drawer_cannot_guess() {
        let mut g = started();
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        // Seat 0 is the drawer this turn.
        assert_eq!(g.apply_guess(0, "robot"), game::GuessOutcome::Ignored);
        assert!(g.turn_guesses.is_empty());
    }

    #[test]
    fn near_miss_reports_close_without_scoring() {
        let mut g = started();
        g.choose_word("robot".to_string());
        g.tick(Duration::from_secs(0));
        assert_eq!(g.apply_guess(1, "robott"), game::GuessOutcome::Close);
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
                tick_to(&mut a, 0, game::REVEAL_SECS);
                tick_to(&mut b, 0, game::REVEAL_SECS);
            }
        }
    }

    // --- Probe integration test (GPU-free, drives the real funnel) ----------

    /// Boot the app GPU-free, start a match, switch the human to a guesser seat,
    /// drive a human guess through the real funnel, and assert the fold landed.
    #[test]
    fn probe_match_flow_scores_a_human_guess() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(buiy::BuiyProbePlugin);
        install(&mut app);
        for _ in 0..8 {
            app.update();
        }

        // Start the match. (Home's "▶ Play" starts it directly; driven here by
        // the message so the test doesn't depend on the button label glyphs.)
        enqueue_msg(&mut app, Msg::StartMatch);
        settle(&mut app);

        // The drawer (seat 0) picks the first word, entering the draw phase.
        enqueue_msg(&mut app, Msg::ChooseWord(0));
        settle(&mut app);
        let word = current_word(&mut app);
        assert!(!word.is_empty(), "a secret word is set after ChooseWord");

        // Anchor the draw clock at t=0 (virtual time; no GameClockPlugin here).
        enqueue_msg(&mut app, Msg::Tick(Duration::from_secs(0)));
        settle(&mut app);

        // Human hops to seat 1 (a guesser) and submits the exact word.
        enqueue_msg(&mut app, Msg::SwitchSeat(1));
        enqueue_msg(&mut app, Msg::SetChatInput(word));
        enqueue_msg(&mut app, Msg::SubmitGuess);
        settle(&mut app);

        let g = current_game(&mut app);
        assert!(
            g.turn_guesses.iter().any(|gu| gu.player == 1),
            "the human guess folded into a correct guess for seat 1"
        );
        assert!(g.players[1].score > 0, "the guess scored");

        // The in-game screen renders the header (phase machine is live).
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("Round 1 of 2"),
            "in-game header is on screen:\n{report}"
        );
    }

    fn settle(app: &mut App) {
        for _ in 0..4 {
            app.update();
        }
    }

    fn enqueue_msg(app: &mut App, msg: Msg) {
        let e = app
            .world_mut()
            .query_filtered::<Entity, With<Dooduel>>()
            .iter(app.world())
            .next()
            .expect("model entity");
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<buiy_core::mvu::Envelope<Dooduel>>>()
            .write(buiy_core::mvu::Envelope::user(e, msg));
    }

    fn current_game(app: &mut App) -> Game {
        app.world_mut()
            .query::<&Dooduel>()
            .iter(app.world())
            .next()
            .expect("model")
            .game
            .clone()
    }

    fn current_word(app: &mut App) -> String {
        current_game(app).secret_word
    }

    /// W4: the in-game screen's controls are reachable + wired, GPU-free. Drives
    /// to the Drawing phase, then (1) locates every toolbar/chrome button by
    /// role+name, (2) **clicks a seat-switcher avatar chip** — a *pressable icon*
    /// (the W4 view extension: an `icon` carrying `on_press` becomes an
    /// activatable a11y button) — and asserts the viewed seat hopped, and (3)
    /// submits a guess by clicking Send and asserts it scored.
    #[test]
    fn probe_in_game_controls_are_reachable() {
        use buiy_core::a11y::A11yRole;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(buiy::BuiyProbePlugin);
        install(&mut app);
        for _ in 0..8 {
            app.update();
        }

        // Into the Drawing phase (seat 0 = the human drawer this turn).
        enqueue_msg(&mut app, Msg::StartMatch);
        settle(&mut app);
        enqueue_msg(&mut app, Msg::ChooseWord(0));
        settle(&mut app);
        enqueue_msg(&mut app, Msg::Tick(Duration::from_secs(0))); // anchor the clock
        settle(&mut app);
        let word = current_word(&mut app);
        assert!(!word.is_empty(), "a secret word is set");

        // (1) Every toolbar + chrome button is locatable by role + name.
        for label in ["Brush", "Fill", "Eraser", "Undo", "Clear", "Send", "Leave"] {
            assert!(
                get_by_role(app.world_mut(), A11yRole::Button, Some(label), None).is_ok(),
                "the {label:?} button is locatable by role+name"
            );
        }

        // (2) Click the seat-1 avatar chip (a pressable ICON) → the seat hops.
        let chip = get_by_role(app.world_mut(), A11yRole::Button, Some("Priya"), None)
            .expect("the seat-1 avatar chip is a locatable button");
        click(app.world_mut(), chip).expect("the avatar chip is clickable");
        settle(&mut app);
        assert_eq!(
            current_game(&mut app).viewing_as,
            1,
            "clicking the avatar chip hopped the viewed seat to 1 (the pressable-icon route)"
        );

        // (3) Type a guess + click Send → it folds through the funnel and scores.
        enqueue_msg(&mut app, Msg::SetChatInput(word));
        settle(&mut app);
        let send = get_by_role(app.world_mut(), A11yRole::Button, Some("Send"), None)
            .expect("the Send button");
        click(app.world_mut(), send).expect("Send is clickable");
        settle(&mut app);
        let g = current_game(&mut app);
        assert!(
            g.turn_guesses.iter().any(|gu| gu.player == 1),
            "the Send-driven guess scored for seat 1"
        );
        assert!(g.players[1].score > 0, "the guess scored points");

        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("Scoreboard"),
            "the in-game screen renders its panes:\n{report}"
        );
    }

    /// W3 smoke: the app boots GPU-free, Home is locatable by role/name (title,
    /// name input, the Play + Create + Join actions, an avatar), and "Create a
    /// room" navigates to the Lobby (roster + Start).
    #[test]
    fn home_boots_and_create_room_navigates_to_lobby() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(buiy::BuiyProbePlugin);
        install(&mut app);
        for _ in 0..8 {
            app.update();
        }

        // Home elements locatable by role + name.
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("Dooduel"),
            "home shows the wordmark:\n{report}"
        );
        for label in ["▶ Play", "Create a room", "Join a room"] {
            assert!(
                get_by_role(
                    app.world_mut(),
                    buiy_core::a11y::A11yRole::Button,
                    Some(label),
                    None,
                )
                .is_ok(),
                "home has the {label:?} button by role+name:\n{report}"
            );
        }
        // The name input is present as a text field.
        assert!(
            buiy_view::find_kind(app.world_mut(), buiy_view::Kind::TextInput).is_some(),
            "home has a name input"
        );
        // The doodle avatars render as icon nodes (the editable preview + the
        // three "you'll play with" opponents).
        assert!(
            buiy_view::entities_of_kind(app.world_mut(), buiy_view::Kind::Icon).len() >= 4,
            "home shows doodle avatars (icon nodes)"
        );

        // "Create a room" → Lobby.
        let create = get_by_role(
            app.world_mut(),
            buiy_core::a11y::A11yRole::Button,
            Some("Create a room"),
            None,
        )
        .expect("Create-a-room button");
        click(app.world_mut(), create).expect("Create is clickable");
        settle(&mut app);

        assert_eq!(
            current_screen(&mut app),
            Screen::Lobby,
            "Create a room navigates to the Lobby"
        );
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("Private room"),
            "the Lobby shows its eyebrow:\n{report}"
        );
        assert!(
            get_by_role(
                app.world_mut(),
                buiy_core::a11y::A11yRole::Button,
                Some("▶ Start game"),
                None,
            )
            .is_ok(),
            "the Lobby has a Start button:\n{report}"
        );
    }

    /// W3: the Join screen accepts a code and drops into the Lobby as a guest,
    /// with the roster showing the guest's name.
    #[test]
    fn join_flow_reaches_lobby_as_guest() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(buiy::BuiyProbePlugin);
        install(&mut app);
        for _ in 0..8 {
            app.update();
        }

        enqueue_msg(&mut app, Msg::SetName("Zed".to_string()));
        enqueue_msg(&mut app, Msg::GoJoin);
        settle(&mut app);
        assert_eq!(current_screen(&mut app), Screen::Join);

        enqueue_msg(&mut app, Msg::SetJoinCode("abc123".to_string()));
        enqueue_msg(&mut app, Msg::SubmitJoin);
        settle(&mut app);

        assert_eq!(current_screen(&mut app), Screen::Lobby);
        let m = current_model(&mut app);
        assert!(!m.is_host, "joining lands as a guest");
        assert_eq!(
            m.room_code, "ABC123",
            "the entered code is uppercased + shown"
        );
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("Zed"),
            "the roster shows the guest:\n{report}"
        );
        assert!(
            report.contains("Priya"),
            "the roster shows the host:\n{report}"
        );
        // The roster shows an avatar badge (icon node) per player.
        assert!(
            buiy_view::entities_of_kind(app.world_mut(), buiy_view::Kind::Icon).len() >= 4,
            "the lobby roster shows avatar badges"
        );
    }

    fn current_screen(app: &mut App) -> Screen {
        current_model(app).screen
    }

    fn current_model(app: &mut App) -> Dooduel {
        app.world_mut()
            .query::<&Dooduel>()
            .iter(app.world())
            .next()
            .expect("model")
            .clone()
    }

    // --- W5: avatar editor + podium (reducer-level) ------------------------

    #[test]
    fn avatar_editor_open_tab_and_save_round_trip() {
        let mut m = Dooduel::default();
        assert!(!m.avatar.editor_open);
        assert_eq!(m.avatar.kind, HumanAvatar::Default);

        update(&mut m, Msg::OpenAvatarEditor);
        assert!(m.avatar.editor_open);
        assert_eq!(m.avatar.tab, AvatarTab::Gallery, "opens on the gallery tab");

        // Switching to Draw bumps the scratch-reset counter (the sync blanks it).
        let reset_before = m.avatar.reset_seq;
        update(&mut m, Msg::SetAvatarTab(AvatarTab::Draw));
        assert_eq!(m.avatar.tab, AvatarTab::Draw);
        assert_eq!(m.avatar.reset_seq, reset_before + 1);

        // Brush edits are reducer-owned + replayable.
        update(&mut m, Msg::SelectAvatarColor(4));
        assert_eq!(m.avatar.draft_color_idx, 4);
        assert!(!m.avatar.draft_eraser, "picking a color clears the eraser");
        update(&mut m, Msg::ToggleAvatarEraser);
        assert!(m.avatar.draft_eraser);
        update(&mut m, Msg::SelectAvatarSize(2));
        assert_eq!(m.avatar.draft_size_idx, 2);

        // Save commits the drawing (bumps the copy counter, closes, marks custom).
        let save_before = m.avatar.save_seq;
        update(&mut m, Msg::SaveAvatar);
        assert_eq!(m.avatar.save_seq, save_before + 1);
        assert_eq!(m.avatar.kind, HumanAvatar::Custom);
        assert!(!m.avatar.editor_open, "save closes the editor");
    }

    #[test]
    fn gallery_pick_sets_a_preset_and_reset_returns_to_default() {
        let mut m = Dooduel::default();
        update(&mut m, Msg::OpenAvatarEditor);
        update(&mut m, Msg::PickGalleryIcon(5));
        assert_eq!(
            m.avatar.kind,
            HumanAvatar::Preset {
                icon: 5,
                tint: 5 % avatar::TINT_COUNT
            },
            "a gallery pick sets an explicit icon + tint preset"
        );
        assert!(!m.avatar.editor_open, "a gallery pick closes the editor");

        update(&mut m, Msg::ResetAvatar);
        assert_eq!(m.avatar.kind, HumanAvatar::Default);
    }

    /// The avatar editor opens from Home's pencil affordance (a pressable icon),
    /// and Save round-trips the custom-avatar flag through the funnel (GPU-free).
    #[test]
    fn probe_avatar_editor_opens_from_home_and_saves() {
        use buiy_core::a11y::A11yRole;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(buiy::BuiyProbePlugin);
        install(&mut app);
        for _ in 0..8 {
            app.update();
        }

        // The pencil "Edit your avatar" affordance is a pressable icon → a button.
        let edit = get_by_role(
            app.world_mut(),
            A11yRole::Button,
            Some("Edit your avatar"),
            None,
        )
        .expect("the pencil edit affordance is a locatable button");
        click(app.world_mut(), edit).expect("the pencil is clickable");
        settle(&mut app);
        assert!(
            current_model(&mut app).avatar.editor_open,
            "clicking the pencil opens the avatar editor"
        );

        // Switch to the draw tab; the save button is reachable there.
        enqueue_msg(&mut app, Msg::SetAvatarTab(AvatarTab::Draw));
        settle(&mut app);
        let save = get_by_role(
            app.world_mut(),
            A11yRole::Button,
            Some("Use this doodle"),
            None,
        )
        .expect("the save button is reachable on the draw tab");
        click(app.world_mut(), save).expect("save is clickable");
        settle(&mut app);

        let m = current_model(&mut app);
        assert!(!m.avatar.editor_open, "save closes the editor");
        assert_eq!(
            m.avatar.kind,
            HumanAvatar::Custom,
            "save round-trips the custom-avatar flag through the funnel"
        );
    }

    /// A full match driven by injected virtual ticks lifts the shell to the
    /// Podium screen (the W5 reachability probe — reuses the W2 tick pattern).
    #[test]
    fn probe_full_match_reaches_the_podium_screen() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(buiy::BuiyProbePlugin);
        install(&mut app);
        for _ in 0..8 {
            app.update();
        }

        enqueue_msg(&mut app, Msg::StartMatch);
        settle(&mut app);
        let mut clock = 0u64;
        for _ in 0..400 {
            if current_screen(&mut app) == Screen::Podium {
                break;
            }
            match current_game(&mut app).phase {
                Phase::Picking => enqueue_msg(&mut app, Msg::ChooseWord(0)),
                Phase::Reveal => enqueue_msg(&mut app, Msg::Continue),
                _ => {
                    clock += 30;
                    enqueue_msg(&mut app, Msg::Tick(Duration::from_secs(clock)));
                }
            }
            settle(&mut app);
        }
        assert_eq!(
            current_screen(&mut app),
            Screen::Podium,
            "the full match reaches the podium"
        );
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("wins!"),
            "the podium announces the winner:\n{report}"
        );
    }
}
