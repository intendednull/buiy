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
