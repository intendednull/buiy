//! Dooduel — a fully-featured skribbl.io-style draw-and-guess game, shipped on the
//! Buiy framework.
//!
//! One MVU model owns the whole UI (one `ui()` per app); screens are a [`Screen`]
//! enum matched in [`view::view`] (root kind-swap). Since M1 the match state is a
//! [`RoomReplica`] (the client-side mirror of the authoritative session), mutated
//! only by `Msg::Net(ServerEvent)`; the pure rules/scoring/clock core lives in the
//! Bevy-free `dooduel_core::game` crate. The reducer is one pure fold; the F7
//! [`ClockPlugin`] turns wall-clock into a `Msg::Tick(now)` every frame — driving the
//! monotonic countdown. The windowed `dooduel` bin, the wasm `dooduel_web` crate, and
//! the headless `capture` bin share [`install`] / [`install_runtime`].
//!
//! ## Module map (the per-screen split)
//!
//! - [`game`] — the PURE game core, re-exported from `dooduel_core` (phase machine,
//!   scoring, hints, seeded bots, the per-seat `word_display_for` redaction). Zero
//!   framework coupling; unit-testable. The client no longer runs it — the authority
//!   ([`net::LocalAuthorityPlugin`] solo, `dooduel_server` networked) does.
//! - [`net`] — the transport pump ([`net::NetPlugin`]) + the in-process solo
//!   authority ([`net::LocalAuthorityPlugin`]): intents out, `Msg::Net` events in.
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
use buiy_core::text::edit::Clipboard;

pub mod avatar;
pub mod confetti;
/// The pure game core now lives in `dooduel_core`; re-exported here so existing
/// `crate::game::…` / `dooduel::game::…` paths (views, bins, tests) stay stable
/// (M1 W0.2). The pure game unit tests moved with it, into `dooduel_core::game`.
pub use dooduel_core::game;
pub mod net;
pub mod paint;
pub mod storage;
pub mod theme;
pub mod view;

use game::Phase;
use theme::ThemePref;

/// The wire protocol the client speaks (M1 W3): the [`RoomReplica`] the model now
/// holds instead of a local `game::Game`, the [`ClientIntent`]s the reducer sends,
/// and the [`ServerEvent`]s `Msg::Net` folds in. Re-exported so `view/*` + `net`
/// reach them via `crate::…`.
pub use dooduel_core::protocol::{
    CanvasOp, ClientIntent, ErrorCode, ReplicaPlayer, RoomReplica, ServerEvent, WireAvatar,
};

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
    /// The client-side mirror of the authoritative room (M1 W3.1 — replaces the
    /// old local `game::Game`). **Mutated only by `Msg::Net`** (the reducer folds a
    /// [`ServerEvent`] into it); the view reads it per screen. The secret word,
    /// another drawer's pre-pick choices, the RNG seed, and other seats' private
    /// chat have no field here — the negative invariant (spec §4.1).
    pub replica: RoomReplica,
    /// Which session backs this model (spec §4.1). `Solo` = the in-process
    /// [`net::LocalAuthorityPlugin`] authority; `Offline` = Home / the cosmetic
    /// Create-Join lobby (networked Create/Join land in W4). The `Joining` /
    /// `Connected` / `Dropped` arms are the W4 networked path (structurally present).
    pub net: NetState,
    /// The guess/chat field text — local UI state that moved OUT of `game::Game`
    /// (spec §2.3.6); the reducer sends its content as a `Guess` intent on submit.
    pub chat_input: String,
    /// Outbound intents the reducer stages for [`net::NetPlugin`] to send (spec §4.2
    /// — the reducer is pure, so it cannot touch the transport). An append-log a
    /// draining system tracks with a cursor; canvas intents (stroke/fill/undo/clear)
    /// go straight from the paint subsystem, so this carries only the low-frequency
    /// gameplay intents (pick / guess / continue).
    pub net_outbox: Vec<ClientIntent>,
    /// Text the reducer stages for the OS clipboard (the reducer is pure, so it cannot
    /// touch the `Clipboard` resource — an append-log the `drain_clipboard_outbox`
    /// system writes out with a cursor, mirroring [`net_outbox`](Self::net_outbox)).
    /// The lobby's "Copy" button routes the room code through here.
    pub clipboard_outbox: Vec<String>,
    /// Bumped to request a fresh in-process solo `Session` (▶ Play / Lobby Start /
    /// Play again). [`net::LocalAuthorityPlugin`] watches it and rebuilds the
    /// authority (a new match seed each time — spec §8).
    pub solo_epoch: u64,
    /// The phase countdown, anchored to the client's monotonic clock (spec §4.3):
    /// `Msg::Net` records the server's `remaining`, the per-frame `Msg::Tick` folds
    /// it down to derived whole seconds, clamped so the display never jumps upward.
    pub countdown: Countdown,
    /// A monotonic count of canvas RESEED events — a wholesale replacement of the
    /// authoritative log (`RoomState` / `CanvasLog`, i.e. a join or mid-turn
    /// reconnect). Folded into the raster re-render signature (`paint::…`) so a
    /// reseed always re-renders, EVEN when it coincidentally matches the current
    /// log's `(len, last_op_id)`: op ids reset per turn (dense `0,1,…`), so two
    /// equal-length no-undo logs from DIFFERENT turns share that pair — a client
    /// that missed the Picking boundary (a W4 reconnect) would otherwise keep stale
    /// turn-N ink over a turn-N+1 replica. Bumped by the `Msg::Net` fold (funnel-clean,
    /// so it replays deterministically).
    pub canvas_reseeds: u64,
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
    /// A networked connect the reducer staged for [`net::WsClientPlugin`] to open (spec
    /// §4.2 — the pure reducer can't build the `WsClientTransport`). `#[reflect(ignore)]`:
    /// a transient effect trigger, NOT replay state (replay re-feeds the recorded
    /// `Msg::Net` stream, it never re-opens a socket — like `Countdown`'s `pending`).
    #[reflect(ignore)]
    pub pending_connect: Option<Connect>,
    /// The current reconnect token (from the latest `Welcome`; rotated every
    /// (re)connection, spec §6.3). The auto-rejoin on a drop re-attaches with it.
    pub reconnect_token: String,
    /// A transient status/error banner (a rejected join, a drop + reconnect) surfaced
    /// over the lobby / join screens. Cleared on the next successful `Welcome`.
    pub toast: Option<String>,
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

    /// Whether this client's seat is the drawer this turn (replaces the old
    /// `Game::viewer_is_drawer` — the hot-seat viewer is gone, the seat is fixed).
    pub fn is_drawer(&self) -> bool {
        self.replica.drawer == Some(self.replica.my_seat)
    }

    /// Whether this client's seat is the room host (the authoritative, live source —
    /// the server's `host`, which can migrate). The Lobby host-gates Start on this. Falls
    /// back to the optimistic [`Self::is_host`] flag before the first `Roster`/`RoomState`
    /// lands (an empty roster during `Joining`).
    pub fn is_host_seat(&self) -> bool {
        if self.replica.players.is_empty() {
            self.is_host
        } else {
            self.replica.my_seat == self.replica.host
        }
    }

    /// The roster in score order (highest first, ties keep seat order) as
    /// `(seat, player)` — the replica-side replacement for `Game::standings`.
    pub fn standings(&self) -> Vec<(usize, &ReplicaPlayer)> {
        let mut ranked: Vec<(usize, &ReplicaPlayer)> =
            self.replica.players.iter().enumerate().collect();
        ranked.sort_by_key(|(_, p)| std::cmp::Reverse(p.score));
        ranked
    }
}

/// Which session backs the model (spec §4.1). M1 W3 wires `Offline` (Home / the
/// cosmetic lobby) and `Solo` (the in-process authority); the `Joining` /
/// `Connected` / `Dropped` arms are the W4 networked path, present so the reducer
/// and view branches are structurally ready.
#[derive(Debug, Clone, PartialEq, Reflect, Default)]
pub enum NetState {
    /// No session: Home, and the W3 cosmetic Create/Join lobby (no server yet).
    #[default]
    Offline,
    /// The in-process solo authority ([`net::LocalAuthorityPlugin`]) + bots.
    Solo,
    /// (W4) awaiting a WebSocket connection to `dooduel_server`.
    Joining,
    /// Connected to a room on the server.
    Connected { room: String },
    /// Dropped mid-session, holding a reconnect token (a rejoin is auto-attempted).
    Dropped { token: String },
}

impl NetState {
    /// Whether this session is backed by the networked `WsClientTransport` (any of the
    /// connect/connected/dropped states) — as opposed to `Offline`/`Solo`.
    pub fn is_networked(&self) -> bool {
        matches!(
            self,
            NetState::Joining | NetState::Connected { .. } | NetState::Dropped { .. }
        )
    }
}

/// A networked connect request the reducer stages for [`net::WsClientPlugin`] (spec
/// §4.2 — the reducer is pure, so it cannot build the transport itself). The net layer
/// opens a [`dooduel_core::transport::WsClientTransport`] and sends the matching first
/// frame ([`ClientIntent::Create`] / [`ClientIntent::Join`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Connect {
    /// Open a fresh room — the server generates the code, returned in `Welcome`.
    Create,
    /// Join an existing room by code, optionally re-attaching a held seat with a token.
    Join {
        code: String,
        reconnect: Option<String>,
    },
}

/// The phase countdown as the client displays it (spec §4.3). The server sends a
/// `remaining: Duration` on every `PhaseChanged` / `CountdownSync`; the client
/// anchors it to its **monotonic** clock (the `Msg::Tick(now)` value, never
/// wall-clock) and counts down locally, so a dropped/late sync degrades to
/// one-way-latency error in the safe direction. Re-syncs re-anchor, **clamped so
/// the displayed number never jumps upward**.
///
/// Only derived whole-second state is stored (`secs`/`total`), so a steady
/// sub-second `Msg::Tick` folds `set_if_neq`-clean (the F7 poll-clock discipline).
#[derive(Debug, Clone, PartialEq, Reflect, Default)]
pub struct Countdown {
    /// The monotonic instant the current phase's countdown hits zero.
    deadline: Duration,
    /// The full phase length in seconds (the timer-ring denominator), set at each
    /// phase start; `0` before the first phase.
    total: u64,
    /// The displayed whole seconds remaining — the view reads this.
    secs: u64,
    /// A `remaining` received but not yet anchored: the next `Msg::Tick` (which
    /// carries `now`) consumes it. `true` = a fresh phase (reset the deadline + set
    /// `total`); `false` = a mid-phase re-sync (clamp — never move the deadline
    /// later, so the display never jumps up).
    #[reflect(ignore)]
    pending: Option<(Duration, bool)>,
}

impl Countdown {
    /// The displayed whole seconds remaining in the current phase.
    pub fn secs(&self) -> u64 {
        self.secs
    }

    /// The fraction of the phase still remaining (`0..=1`), for the timer ring.
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.secs as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }

    /// Record a server `remaining` to anchor on the next tick. `reset` (a
    /// `PhaseChanged`) starts a fresh phase; `!reset` (a `CountdownSync`) clamps.
    ///
    /// Multiple events can fold in ONE batch before the next `Msg::Tick` consumes the
    /// pending value (I-3): a fresh reset is authoritative and wins outright — a
    /// same-batch `CountdownSync` must NOT override it, or the display would clobber to
    /// 0:00 (a non-reset clamp against the default-zero deadline). Dropping the sync
    /// here is a deliberate CONSERVATIVE choice, not a staleness defense: under FIFO a
    /// sync following a reset was emitted *after* the phase change server-side, so it is
    /// a legitimate newer value — but we can't safely fold it without re-opening the
    /// 0:00 clobber, and its accuracy loss is at most one sync, self-healed by the next
    /// periodic sync within ~a second. A non-reset sync only clamps a pending non-reset
    /// downward, or establishes one when nothing is pending.
    fn anchor(&mut self, remaining: Duration, reset: bool) {
        match self.pending {
            // A fresh phase change always wins (supersedes any earlier pending).
            _ if reset => self.pending = Some((remaining, true)),
            // A same-batch sync cannot override a pending reset — the reset is the
            // guaranteed-correct fresh-phase anchor (dropping the sync is conservative).
            Some((_, true)) => {}
            // Fold a non-reset sync into a pending non-reset (clamp down), or take it.
            Some((prev, false)) => self.pending = Some((prev.min(remaining), false)),
            None => self.pending = Some((remaining, false)),
        }
    }

    /// Fold one monotonic `now`: consume a pending anchor, then derive the whole
    /// seconds left. Idempotent within a second (`set_if_neq`-clean).
    fn on_tick(&mut self, now: Duration) {
        if let Some((remaining, reset)) = self.pending.take() {
            let new_deadline = now + remaining;
            if reset {
                self.deadline = new_deadline;
                self.total = remaining.as_secs().max(1);
            } else {
                // Clamp: only ever pull the deadline earlier, so the display never
                // jumps upward on a late/optimistic re-sync (spec §4.3).
                self.deadline = self.deadline.min(new_deadline);
            }
        }
        let secs = self.deadline.saturating_sub(now).as_secs();
        if self.secs != secs {
            self.secs = secs;
        }
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

/// Which screen the app is showing. `InGame`/`Podium` read [`Dooduel::replica`].
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

/// The app's messages. `Tick` is the per-frame countdown poll; `Net` is the sole
/// replica mutator — every authoritative change arrives as a [`ServerEvent`] the
/// reducer folds into [`Dooduel::replica`] (spec §4.2). Gameplay actions no longer
/// mutate locally; they stage a [`ClientIntent`] for [`net::NetPlugin`] to send.
// `Net(ServerEvent)` is the largest variant (a `RoomState` seed); `Msg` is only ever
// heap-owned (an `Envelope` payload / a `Cmd::Emit`), so the on-stack size the lint
// guards against does not apply — the same rationale `ServerEvent` itself carries.
#[allow(clippy::large_enum_variant)]
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
    /// The lobby "Copy" button — stage the room code for the OS clipboard (written out by
    /// [`drain_clipboard_outbox`]) and flash a confirmation toast. A no-op with no code.
    CopyCode,
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
    /// An authoritative event from the session (the sole [`Dooduel::replica`]
    /// mutator, spec §4.2). Enqueued by [`net::NetPlugin`]'s pump each frame; folds
    /// through the record/replay funnel like any other message, so a networked
    /// session records/replays as a `Msg::Net` stream (spec §3.4). The
    /// `MatchEnded` variant lifts the shell to [`Screen::Podium`].
    Net(ServerEvent),
    /// The net layer failed to OPEN the WebSocket (spec §4.2) — surfaced as a toast and
    /// the session reverts to a usable screen. Enqueued by [`net::WsClientPlugin`].
    ConnectFailed(String),
    /// The net layer observed the live connection drop (spec §6.3) — go to
    /// [`NetState::Dropped`] and stage a token rejoin. Enqueued by [`net::WsClientPlugin`].
    NetDropped,
    /// The bounded auto-rejoin (spec §6.3) exhausted its tries — give up: "Connection
    /// lost" + back to Offline/Home. Enqueued by [`net::WsClientPlugin`].
    NetGaveUp,
    // The clock (folded every frame; a steady frame is a `set_if_neq` no-op).
    Tick(Duration),
    /// The window's logical size changed (the `ViewportPlugin` seam). Folded
    /// `set_if_neq`-clean: only a real resize changes the model, so it never forces a
    /// rebuild on a steady frame (the same discipline as `Tick`).
    SetViewport(f32, f32),
    // In-turn — each stages a `ClientIntent` (the reducer no longer mutates state).
    /// The drawer picked word `i` → `ClientIntent::Pick`.
    ChooseWord(usize),
    /// The guess field text (local UI state, [`Dooduel::chat_input`]).
    SetChatInput(String),
    /// Submit the guess field → `ClientIntent::Guess` (empty guesses are dropped).
    SubmitGuess,
    /// Advance out of the turn-end reveal (the "Continue →" button) →
    /// `ClientIntent::Continue`.
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
}

/// UPDATE — the pure reducer. Navigation + local UI state; gameplay actions stage a
/// [`ClientIntent`] (sent by [`net::NetPlugin`]) and every authoritative change
/// arrives back as `Msg::Net` (folded by `apply_event`). The reducer never mutates the
/// replica except through `Msg::Net` (spec §4.2).
pub fn update(s: &mut Dooduel, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::SetName(name) => s.player_name = name,
        // "▶ Play" starts a solo in-process match directly (the design's primary
        // CTA); the Lobby is only reached via Create/Join.
        Msg::Play => start_solo(s),
        // Networked create: the server issues the room code (we do NOT gen one locally),
        // so we open a connection and show the Lobby's "Connecting…" state until Welcome.
        Msg::CreateRoom => start_connect(s, Connect::Create, true),
        Msg::GoJoin => {
            s.join_code.clear();
            s.toast = None;
            s.screen = Screen::Join;
        }
        Msg::SetJoinCode(code) => s.join_code = code.to_uppercase(),
        Msg::SubmitJoin => {
            let code = s.join_code.trim().to_uppercase();
            // Client input hygiene: an empty code never opens a connection.
            if code.is_empty() {
                s.toast = Some("Enter a room code.".to_string());
            } else {
                start_connect(
                    s,
                    Connect::Join {
                        code,
                        reconnect: None,
                    },
                    false,
                );
            }
        }
        Msg::Back => {
            // Leaving a networked room releases the seat gracefully — skipping the 45s
            // grace so it doesn't ghost (spec §3.2, I-4). Best-effort: `Back` also flips
            // `net` to Offline, and `WsClientPlugin` tears the transport down in the SAME
            // funnel batch — the outbox drain (MvuSet::Enqueue, before the pump) sends
            // this `Leave` first, but a lost flush is covered by the server's own grace
            // (an un-Left seat just vacates after 45s). The same best-effort applies to
            // `PlayAgain`'s Leave below.
            if s.net.is_networked() {
                s.net_outbox.push(ClientIntent::Leave);
            }
            s.screen = Screen::Home;
            s.net = NetState::Offline;
            s.pending_connect = None;
            s.reconnect_token.clear();
            s.toast = None;
            s.is_host = false;
            s.replica = RoomReplica::default();
            s.countdown = Countdown::default();
        }
        // In a networked lobby the host starts the match on the SERVER (an intent — the
        // server also host-gates it); the solo/offline path starts an in-process match.
        Msg::StartMatch => {
            if s.net.is_networked() {
                s.net_outbox.push(ClientIntent::StartMatch);
            } else {
                start_solo(s);
            }
        }
        // Copy the room code to the OS clipboard (Buiy has no text-selection on static
        // labels yet — a full selectable-text pass is a follow-up), with a toast for
        // feedback. A no-op before a code exists, so it never copies an empty string.
        Msg::CopyCode => {
            if !s.replica.room_code.is_empty() {
                s.clipboard_outbox.push(s.replica.room_code.clone());
                s.toast = Some("Copied!".to_string());
            }
        }
        // Play-again: solo replays in place; the networked in-place rematch is out of M1
        // (spec §11), so it leaves + returns Home for the host to create a fresh room.
        Msg::PlayAgain => {
            if s.net.is_networked() {
                s.net_outbox.push(ClientIntent::Leave);
                s.screen = Screen::Home;
                s.net = NetState::Offline;
                s.pending_connect = None;
                s.reconnect_token.clear();
                s.replica = RoomReplica::default();
                s.countdown = Countdown::default();
            } else {
                start_solo(s);
            }
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
        // The monotonic countdown poll (spec §4.3) — derive whole seconds from the
        // anchor. No game tick here: the authority ticks itself (solo via
        // `LocalAuthorityPlugin`), and phase changes arrive only as `Msg::Net`.
        Msg::Tick(now) => s.countdown.on_tick(now),
        Msg::Net(ev) => apply_event(s, ev),
        // The net layer failed to OPEN the socket — toast + revert Home so the user
        // can retry (spec §4.2).
        Msg::ConnectFailed(err) => {
            s.toast = Some(format!("Could not connect: {err}"));
            s.net = NetState::Offline;
            s.pending_connect = None;
            s.screen = Screen::Home;
        }
        // The live connection dropped (spec §6.3): stage a token rejoin, keeping the
        // replica so the game keeps rendering while `WsClientPlugin` re-attaches.
        Msg::NetDropped => {
            if !s.reconnect_token.is_empty() && !s.room_code.is_empty() {
                s.toast = Some("Reconnecting…".to_string());
                s.net = NetState::Dropped {
                    token: s.reconnect_token.clone(),
                };
                s.pending_connect = Some(Connect::Join {
                    code: s.room_code.clone(),
                    reconnect: Some(s.reconnect_token.clone()),
                });
            } else {
                s.toast = Some("Disconnected.".to_string());
                s.net = NetState::Offline;
                s.screen = Screen::Home;
            }
        }
        // The bounded auto-rejoin gave up (spec §6.3): a terminal "connection lost".
        Msg::NetGaveUp => {
            s.toast = Some("Connection lost.".to_string());
            s.net = NetState::Offline;
            s.pending_connect = None;
            s.reconnect_token.clear();
            s.screen = Screen::Home;
        }
        Msg::ChooseWord(idx) => s.net_outbox.push(ClientIntent::Pick { index: idx }),
        Msg::SetChatInput(t) => s.chat_input = t,
        Msg::SubmitGuess => {
            let raw = std::mem::take(&mut s.chat_input);
            let text = raw.trim();
            // Client input hygiene (R4): don't wire an empty guess.
            if !text.is_empty() {
                s.net_outbox.push(ClientIntent::Guess {
                    text: text.to_string(),
                });
            }
        }
        Msg::Continue => s.net_outbox.push(ClientIntent::Continue),
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

/// Request a fresh in-process solo match: mark the session `Solo`, bump the epoch
/// (so [`net::LocalAuthorityPlugin`] tears down any prior `Session` and builds a new
/// one with a fresh match seed), reset the replica + countdown, and show the game.
/// The plugin does the `connect` + `StartMatch` (solo bypasses the lobby, spec §8).
fn start_solo(s: &mut Dooduel) {
    s.net = NetState::Solo;
    s.solo_epoch = s.solo_epoch.wrapping_add(1);
    s.replica = RoomReplica::default();
    s.countdown = Countdown::default();
    s.screen = Screen::InGame;
}

/// Begin a networked session (spec §4.2): stage the connect for [`net::WsClientPlugin`],
/// mark the session `Joining`, reset the replica/countdown, and show the Lobby — which
/// renders a "Connecting…" state until `Welcome` flips it to `Connected`. The server
/// issues the room code (for `Create`), so none is generated locally.
fn start_connect(s: &mut Dooduel, req: Connect, is_host: bool) {
    s.is_host = is_host;
    s.net = NetState::Joining;
    s.pending_connect = Some(req);
    s.toast = None;
    s.reconnect_token.clear();
    s.room_code.clear();
    s.replica = RoomReplica::default();
    s.countdown = Countdown::default();
    s.screen = Screen::Lobby;
}

/// Lift the shell out of the networked Lobby once the match is underway — the symmetric
/// partner to the `MatchEnded`→`Podium` lift. A networked client sits in the Lobby (spec
/// §4.2) until the server's first in-match phase arrives: the host's own `StartMatch`, a
/// peer host's, or a mid-match reconnect's `RoomState` seed. Without this the host who
/// pressed Start stayed welded to the lobby as the AFK first drawer (the first-playtest
/// bug). Guarded on `Lobby`, so it never yanks a player off Home/Join and is inert once
/// in-game (the solo path lifts to `InGame` directly, never touching the Lobby).
fn lift_lobby_on_match_start(s: &mut Dooduel) {
    if s.screen != Screen::Lobby {
        return;
    }
    match s.replica.phase {
        Phase::Picking | Phase::Drawing | Phase::Reveal => s.screen = Screen::InGame,
        Phase::Final => s.screen = Screen::Podium,
        Phase::Idle => {} // pre-start: stay in the lobby
    }
}

/// The stable id of a canvas op (spec §3.5) — a `CanvasUndo { removed_id }` resolves
/// against the log by this.
fn op_id(op: &CanvasOp) -> u64 {
    match op {
        CanvasOp::Stroke { id, .. } | CanvasOp::Fill { id, .. } => *id,
    }
}

/// Fold one authoritative [`ServerEvent`] into the replica (spec §3.3 event→replica
/// mapping) — the sole replica mutator. Each arm sets exactly the fields the event
/// carries; the countdown is re-anchored from `remaining` (spec §4.3), and the
/// transient `CanvasStrokeProgress` is handled off-model by [`net::NetPlugin`] (it
/// has no replica field — the negative invariant).
fn apply_event(s: &mut Dooduel, ev: ServerEvent) {
    match ev {
        ServerEvent::Welcome {
            seat,
            room_code,
            reconnect_token,
            ..
        } => {
            s.replica.my_seat = seat;
            s.replica.room_code = room_code.clone();
            s.room_code = room_code;
            s.reconnect_token = reconnect_token;
            s.toast = None;
            // A successful (re)connect: mark Connected + consume the pending connect so
            // `WsClientPlugin` doesn't re-open. (No-op on the solo path — never networked.)
            if s.net.is_networked() {
                s.net = NetState::Connected {
                    room: s.room_code.clone(),
                };
                s.pending_connect = None;
            }
        }
        ServerEvent::RoomState(replica) => {
            let remaining = replica.remaining;
            s.replica = replica;
            // A full seed re-anchors the countdown as a fresh phase, and is a canvas
            // reseed (the raster must re-render even if the log coincidentally matches).
            s.countdown.anchor(remaining, true);
            s.canvas_reseeds = s.canvas_reseeds.wrapping_add(1);
            // A mid-match reconnect seed carries a live phase — lift straight to the board.
            lift_lobby_on_match_start(s);
        }
        ServerEvent::Roster { players, host } => {
            s.replica.players = players;
            s.replica.host = host;
        }
        ServerEvent::PhaseChanged {
            phase,
            drawer,
            round,
            total_rounds,
            remaining,
        } => {
            s.replica.phase = phase;
            s.replica.drawer = drawer;
            s.replica.round = round;
            s.replica.total_rounds = total_rounds;
            if phase == Phase::Picking {
                // A fresh turn: last turn's word row / choices / reveal rows clear
                // (the server sends WordChoices to the drawer + WordUpdate on
                // Drawing; nothing re-blanks a guesser's row, so do it here).
                s.replica.word_display.clear();
                s.replica.word_len = 0;
                s.replica.hints_revealed = 0;
                s.replica.word_choices.clear();
                s.replica.turn_results.clear();
            }
            s.countdown.anchor(remaining, true);
            // The match is underway — lift the host/guests out of the Lobby onto the board.
            lift_lobby_on_match_start(s);
        }
        ServerEvent::CountdownSync { remaining } => s.countdown.anchor(remaining, false),
        ServerEvent::WordUpdate {
            display,
            len,
            hints_revealed,
        } => {
            s.replica.word_display = display;
            s.replica.word_len = len;
            s.replica.hints_revealed = hints_revealed;
        }
        ServerEvent::WordChoices { words } => s.replica.word_choices = words,
        ServerEvent::CanvasOpApplied { op } => s.replica.canvas_ops.push(op),
        ServerEvent::CanvasUndo { removed_id } => {
            s.replica.canvas_ops.retain(|op| op_id(op) != removed_id);
        }
        ServerEvent::CanvasCleared => s.replica.canvas_ops.clear(),
        ServerEvent::CanvasLog { ops } => {
            // A wholesale log replace (late join / reconnect) — a canvas reseed, so the
            // raster re-renders even when `(len, last_op_id)` coincides across turns.
            s.replica.canvas_ops = ops;
            s.canvas_reseeds = s.canvas_reseeds.wrapping_add(1);
        }
        // Transient live-stroke relay — painted immediately by the paint subsystem
        // (off-model, spec §3.5); it has no replica field.
        ServerEvent::CanvasStrokeProgress { .. } => {}
        ServerEvent::ChatLine { line } => s.replica.chat.push(line),
        ServerEvent::GuessResult { seat, correct, .. } => {
            // The score + a full Roster follow; reflect the guessed flag at once so
            // the drawer's live count + the guesser's role badge update immediately.
            if correct && let Some(p) = s.replica.players.get_mut(seat) {
                p.guessed = true;
            }
        }
        ServerEvent::TurnEnded { results, word } => {
            s.replica.turn_results = results;
            // The reveal legitimately broadcasts the word (spec §3.3): show it as the
            // full, space-joined row so the header + reveal overlay read it.
            s.replica.word_len = word.chars().count();
            s.replica.word_display = word
                .chars()
                .map(|c| c.to_ascii_uppercase().to_string())
                .collect::<Vec<_>>()
                .join(" ");
            // The Drawing→Reveal phase flip itself arrives via PhaseChanged.
        }
        ServerEvent::MatchEnded { podium } => {
            s.replica.podium = Some(podium);
            s.replica.phase = Phase::Final;
            // The podium lift rides MatchEnded, not Tick (spec §3.3/§4.1).
            s.screen = Screen::Podium;
        }
        // Rejected intents / protocol errors: surface as a toast; a fatal connect/rejoin
        // failure reverts to a usable screen (the solo authority never rejects an honest
        // client, so this path is networked-only in practice).
        ServerEvent::Error { code, message } => {
            s.toast = Some(net_error_text(&code, &message));
            if matches!(s.net, NetState::Joining | NetState::Dropped { .. }) {
                s.net = NetState::Offline;
                s.pending_connect = None;
                s.screen = match code {
                    // A failed JOIN (a guest) drops back to the code entry to fix it;
                    // everything else returns Home.
                    ErrorCode::RoomNotFound | ErrorCode::RoomFull | ErrorCode::MatchInProgress
                        if !s.is_host =>
                    {
                        Screen::Join
                    }
                    _ => Screen::Home,
                };
            }
        }
    }
}

/// The human-readable toast for a networked rejection (spec §4.2).
fn net_error_text(code: &ErrorCode, message: &str) -> String {
    match code {
        ErrorCode::RoomNotFound => "No room with that code.".to_string(),
        ErrorCode::RoomFull => "That room is full.".to_string(),
        ErrorCode::MatchInProgress => "That match already started.".to_string(),
        ErrorCode::VersionMismatch => "Version mismatch — update the app.".to_string(),
        ErrorCode::RateLimited => "Too many attempts — try again shortly.".to_string(),
        ErrorCode::BadToken => "Could not rejoin — the session expired.".to_string(),
        _ => message.to_string(),
    }
}

// NOTE (M1 W4.5): the client-side `gen_room_code` (a deterministic name-hash code) is
// RETIRED for networked play (spec §3.2 — deterministic name-hash codes collide two
// same-named hosts into one room). The server now generates every room code with
// `getrandom` and returns it in `Welcome`; the solo path uses the literal "SOLO".

/// Install Dooduel onto an app already carrying the Buiy plugins. Does **not** add
/// the wall-clock driver — that is the F7 [`ClockPlugin`] (added by
/// [`install_runtime`]), kept separate so the pure logic can be driven by injected
/// `Tick`s in tests (virtual time / `advance_clock`).
pub fn install(app: &mut App) -> &mut App {
    app.register_type::<Screen>();
    // The replica + net state + the wire types `Msg::Net` / `net_outbox` carry (the
    // derive registers their nested field types transitively — CanvasOp, Phase,
    // ChatMsg, ReplicaPlayer, … — so a networked `Msg` stream reflects for
    // record/replay, spec §3.4).
    app.register_type::<RoomReplica>();
    app.register_type::<NetState>();
    app.register_type::<Countdown>();
    app.register_type::<ServerEvent>();
    app.register_type::<ClientIntent>();
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
    // The client transport pump (drains events → `Msg::Net`, sends `net_outbox`
    // intents) + the in-process solo authority (the `Session` behind an
    // `InProcessTransport`, spec §8). The networked path (W4) keeps `NetPlugin` and
    // swaps `LocalAuthorityPlugin` for a `WsClientTransport`.
    app.add_plugins(net::NetPlugin);
    app.add_plugins(net::LocalAuthorityPlugin);
    app.add_plugins(net::WsClientPlugin);
    app.add_plugins(paint::CanvasPlugin);
    app.add_plugins(confetti::ConfettiPlugin);
    app.add_plugins(storage::StoragePlugin);
    // Write the reducer's staged clipboard copies (the "Copy code" button) out to the OS
    // clipboard. Not in `install` (the probe/virtual-clock harnesses drive the reducer
    // arm directly and add this only when they want a clipboard sink).
    app.add_systems(Update, drain_clipboard_outbox);
    app
}

/// Write the reducer's staged [`Dooduel::clipboard_outbox`] entries to the OS clipboard
/// (the reducer is pure, so `Msg::CopyCode` appends here and this drains). A `Local`
/// cursor tracks progress — the mirror of [`net::NetPlugin`]'s outbox drain — and a
/// missing [`Clipboard`] resource degrades to a no-op that still advances the cursor, so
/// a later-inserted clipboard never replays stale copies.
fn drain_clipboard_outbox(
    model: Query<&Dooduel>,
    clipboard: Option<ResMut<Clipboard>>,
    mut cursor: Local<usize>,
) {
    let Ok(model) = model.single() else {
        return;
    };
    let Some(mut clipboard) = clipboard else {
        *cursor = model.clipboard_outbox.len();
        return;
    };
    if *cursor > model.clipboard_outbox.len() {
        *cursor = 0; // defensive: the outbox was reset out from under us
    }
    for text in &model.clipboard_outbox[*cursor..] {
        clipboard.0.set_text(text.clone());
    }
    *cursor = model.clipboard_outbox.len();
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
    use buiy_core::a11y::A11yRole;
    use buiy_core::mvu::clock::advance_clock;
    use dooduel_core::game::{ChatKind, ChatMsg};

    // --- Boot helpers -------------------------------------------------------

    /// A GPU-free probe app with Dooduel installed but NO net plugins — for the pure
    /// scripted-`Msg::Net` view tests (the reducer folds events with no live session).
    fn boot_probe() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(buiy::BuiyProbePlugin);
        install(&mut app);
        for _ in 0..8 {
            app.update();
        }
        app
    }

    /// A GPU-free probe app with the in-process solo authority — for the live
    /// solo-flow + replay tests. The `LocalAuthorityPlugin` runs a real `Session`
    /// behind an `InProcessTransport`, driven by the virtual clock.
    fn boot_solo() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(buiy::BuiyProbePlugin);
        install(&mut app);
        app.add_plugins(net::NetPlugin);
        app.add_plugins(net::LocalAuthorityPlugin);
        for _ in 0..8 {
            app.update();
        }
        app
    }

    fn settle(app: &mut App) {
        for _ in 0..6 {
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

    fn net(app: &mut App, ev: ServerEvent) {
        enqueue_msg(app, Msg::Net(ev));
    }

    fn current_model(app: &mut App) -> Dooduel {
        app.world_mut()
            .query::<&Dooduel>()
            .iter(app.world())
            .next()
            .expect("model")
            .clone()
    }

    fn current_screen(app: &mut App) -> Screen {
        current_model(app).screen
    }

    fn four_players() -> Vec<ReplicaPlayer> {
        ["Mara", "Priya", "Theo", "Sam"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| ReplicaPlayer {
                name: name.to_string(),
                avatar: WireAvatar::Default,
                connected: true,
                is_bot: i != 0,
                score: 0,
                guessed: false,
            })
            .collect()
    }

    /// Drive the shell to the in-game Drawing screen via a scripted `ServerEvent`
    /// stream (`Msg::Play` lifts the screen; the events seed the replica).
    fn enter_drawing(app: &mut App, my_seat: usize, drawer: usize, display: &str, hints: usize) {
        enqueue_msg(app, Msg::Play);
        settle(app);
        net(
            app,
            ServerEvent::Welcome {
                seat: my_seat,
                room_code: "SOLO".to_string(),
                reconnect_token: String::new(),
                protocol_version: dooduel_core::protocol::PROTOCOL_VERSION,
            },
        );
        net(
            app,
            ServerEvent::Roster {
                players: four_players(),
                host: 0,
            },
        );
        net(
            app,
            ServerEvent::PhaseChanged {
                phase: Phase::Drawing,
                drawer: Some(drawer),
                round: 1,
                total_rounds: 2,
                remaining: Duration::from_secs(60),
            },
        );
        net(
            app,
            ServerEvent::WordUpdate {
                display: display.to_string(),
                len: display.split_whitespace().count(),
                hints_revealed: hints,
            },
        );
        enqueue_msg(app, Msg::Tick(Duration::ZERO));
        settle(app);
    }

    // --- Pure reducer: intents + event folding ------------------------------

    #[test]
    fn gameplay_actions_stage_client_intents() {
        let mut m = Dooduel::default();
        update(&mut m, Msg::ChooseWord(2));
        assert_eq!(m.net_outbox.last(), Some(&ClientIntent::Pick { index: 2 }));

        m.chat_input = "robot".to_string();
        update(&mut m, Msg::SubmitGuess);
        assert_eq!(
            m.net_outbox.last(),
            Some(&ClientIntent::Guess {
                text: "robot".to_string()
            })
        );
        assert!(m.chat_input.is_empty(), "submitting clears the field");

        // Client input hygiene (R4): an all-whitespace guess is not wired.
        let before = m.net_outbox.len();
        m.chat_input = "   ".to_string();
        update(&mut m, Msg::SubmitGuess);
        assert_eq!(m.net_outbox.len(), before, "an empty guess is dropped");

        update(&mut m, Msg::Continue);
        assert_eq!(m.net_outbox.last(), Some(&ClientIntent::Continue));
    }

    #[test]
    fn net_events_fold_into_the_replica() {
        let mut m = Dooduel::default();
        apply_event(
            &mut m,
            ServerEvent::Welcome {
                seat: 2,
                room_code: "ABC123".to_string(),
                reconnect_token: "tok".to_string(),
                protocol_version: 1,
            },
        );
        assert_eq!(m.replica.my_seat, 2);
        assert_eq!(m.replica.room_code, "ABC123");

        apply_event(
            &mut m,
            ServerEvent::PhaseChanged {
                phase: Phase::Drawing,
                drawer: Some(0),
                round: 1,
                total_rounds: 2,
                remaining: Duration::from_secs(60),
            },
        );
        assert_eq!(m.replica.phase, Phase::Drawing);
        assert_eq!(m.replica.drawer, Some(0));

        apply_event(
            &mut m,
            ServerEvent::WordUpdate {
                display: "_ _ B".to_string(),
                len: 3,
                hints_revealed: 1,
            },
        );
        assert_eq!(
            m.replica.word_slots(),
            vec![('_', false), ('_', false), ('B', true)]
        );

        // A correct guess flags the seat at once (score + roster follow).
        apply_event(
            &mut m,
            ServerEvent::Roster {
                players: four_players(),
                host: 0,
            },
        );
        apply_event(
            &mut m,
            ServerEvent::GuessResult {
                seat: 1,
                correct: true,
                points: 410,
            },
        );
        assert!(
            m.replica.players[1].guessed,
            "the correct guesser is flagged"
        );

        // TurnEnded reveals the full word; MatchEnded lifts the podium.
        apply_event(
            &mut m,
            ServerEvent::TurnEnded {
                results: vec![],
                word: "robot".to_string(),
            },
        );
        assert_eq!(
            m.replica.word_slots(),
            vec![
                ('R', true),
                ('O', true),
                ('B', true),
                ('O', true),
                ('T', true)
            ],
            "the reveal shows the full word"
        );
        apply_event(
            &mut m,
            ServerEvent::MatchEnded {
                podium: vec![(1, "Priya".to_string(), 1420)],
            },
        );
        assert_eq!(m.screen, Screen::Podium, "MatchEnded lifts the podium");
        assert_eq!(m.replica.phase, Phase::Final);
        assert_eq!(m.replica.podium, Some(vec![(1, "Priya".to_string(), 1420)]));
    }

    #[test]
    fn countdown_anchors_and_never_jumps_upward() {
        let mut m = Dooduel::default();
        apply_event(
            &mut m,
            ServerEvent::PhaseChanged {
                phase: Phase::Drawing,
                drawer: Some(0),
                round: 1,
                total_rounds: 2,
                remaining: Duration::from_secs(80),
            },
        );
        update(&mut m, Msg::Tick(Duration::from_secs(0)));
        assert_eq!(m.countdown.secs(), 80, "anchored at the phase start");
        assert!((m.countdown.fraction() - 1.0).abs() < 1e-6);

        update(&mut m, Msg::Tick(Duration::from_secs(5)));
        assert_eq!(m.countdown.secs(), 75, "counts down locally");

        // A late sync that would read HIGHER (78 → deadline in the future) is clamped.
        apply_event(
            &mut m,
            ServerEvent::CountdownSync {
                remaining: Duration::from_secs(78),
            },
        );
        update(&mut m, Msg::Tick(Duration::from_secs(10)));
        assert_eq!(
            m.countdown.secs(),
            70,
            "no upward jump — the local countdown held"
        );

        // A corrective sync that reads LOWER pulls the countdown down.
        apply_event(
            &mut m,
            ServerEvent::CountdownSync {
                remaining: Duration::from_secs(60),
            },
        );
        update(&mut m, Msg::Tick(Duration::from_secs(15)));
        assert_eq!(m.countdown.secs(), 60, "a corrective sync pulls it down");
    }

    #[test]
    fn countdown_reset_wins_over_a_same_batch_sync() {
        // A fresh PhaseChanged (80s reset) and a CountdownSync (3s) fold in ONE batch
        // before the next Tick. The I-3 bug: the old code let the sync OVERWRITE the
        // reset, then clamped the non-reset against the default-zero deadline → 0:00.
        //
        // The fix drops a same-batch sync when a reset is pending. This is a deliberate
        // CONSERVATIVE choice, NOT a staleness defense: under FIFO a sync that follows a
        // reset in the stream was emitted AFTER the phase change server-side, so it is
        // actually a legitimate NEWER value — dropping it costs at most one sync's worth
        // of accuracy, self-healed by the next periodic sync within ~a second. We favor
        // the guaranteed-correct fresh-phase anchor over a same-batch sync we cannot
        // safely fold without re-introducing the 0:00 clobber. Here that shows 80.
        let mut m = Dooduel::default();
        apply_event(
            &mut m,
            ServerEvent::PhaseChanged {
                phase: Phase::Drawing,
                drawer: Some(0),
                round: 1,
                total_rounds: 2,
                remaining: Duration::from_secs(80),
            },
        );
        apply_event(
            &mut m,
            ServerEvent::CountdownSync {
                remaining: Duration::from_secs(3),
            },
        );
        update(&mut m, Msg::Tick(Duration::from_secs(0)));
        assert_eq!(
            m.countdown.secs(),
            80,
            "the fresh phase's countdown wins over a stale same-batch sync"
        );
    }

    // --- Scripted-view (probe) ---------------------------------------------

    #[test]
    fn guesser_view_redacts_word_and_renders_chat_and_countdown() {
        let mut app = boot_probe();
        // This client is seat 1 (a guesser); seat 0 draws. The guesser gets a redacted
        // word row (blanks + one hint) — the full word never reaches the replica.
        enter_drawing(&mut app, 1, 0, "_ _ B _ _", 1);
        net(
            &mut app,
            ServerEvent::ChatLine {
                line: ChatMsg {
                    seq: 1,
                    kind: ChatKind::Correct,
                    text: "🎉 Sam guessed the word!".to_string(),
                    to: None,
                },
            },
        );
        settle(&mut app);

        let m = current_model(&mut app);
        assert_eq!(
            m.replica.word_display, "_ _ B _ _",
            "the guesser's replica holds only the redacted row — no secret leaks"
        );
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("Round 1 / 2"),
            "the in-game header renders:\n{report}"
        );
        assert!(report.contains("60"), "the countdown shows:\n{report}");
        assert!(
            report.contains("guessed the word"),
            "the chat line renders (emoji stripped):\n{report}"
        );
    }

    #[test]
    fn round_string_forms_render_per_surface() {
        // Desktop uses the slash form "Round r / t".
        let mut app = boot_probe();
        enter_drawing(&mut app, 0, 0, "R O B O T", 0);
        let desktop = snapshot_report(app.world_mut());
        assert!(
            desktop.contains("Round 1 / 2"),
            "desktop header uses the slash form:\n{desktop}"
        );

        // Phone uses the word form "Round r of t".
        let mut app = boot_probe();
        enter_drawing(&mut app, 0, 0, "R O B O T", 0);
        enqueue_msg(&mut app, Msg::SetViewport(390.0, 780.0));
        settle(&mut app);
        let mobile = snapshot_report(app.world_mut());
        assert!(
            mobile.contains("Round 1 of 2"),
            "phone header uses the word form:\n{mobile}"
        );
        assert!(!mobile.contains("Round 1 / 2"));
    }

    #[test]
    fn drawer_header_shows_the_live_guessed_count() {
        let mut app = boot_probe();
        // This client is the drawer (seat 0); the header shows the live guessed count.
        enter_drawing(&mut app, 0, 0, "R O B O T", 0);
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("0 of 3 guessed"),
            "the drawer sees the initial live guessed count:\n{report}"
        );
        // A guesser gets it → the count updates.
        net(
            &mut app,
            ServerEvent::GuessResult {
                seat: 1,
                correct: true,
                points: 410,
            },
        );
        settle(&mut app);
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("1 of 3 guessed"),
            "the drawer's guessed count updates live:\n{report}"
        );
    }

    #[test]
    fn in_game_controls_are_reachable_by_role_and_name() {
        let mut app = boot_probe();
        enter_drawing(&mut app, 0, 0, "R O B O T", 0);
        for label in ["Brush", "Fill", "Eraser", "Undo", "Clear", "Send", "Leave"] {
            assert!(
                get_by_role(app.world_mut(), A11yRole::Button, Some(label), None).is_ok(),
                "the {label:?} button is locatable by role+name"
            );
        }
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("Scoreboard"),
            "the in-game screen renders its panes:\n{report}"
        );
    }

    // --- Live solo authority (integration) ---------------------------------

    /// The solo in-process `Session` self-drives a full match to the podium: ▶ Play
    /// connects the human + fills three bots, and the virtual clock advances through
    /// auto-pick + bot guesses + the reveal, until `MatchEnded` lifts the podium.
    /// (This is the W3.5 "run the artifact" substitute — a headless match end-to-end.)
    #[test]
    fn solo_full_match_reaches_the_podium() {
        let mut app = boot_solo();
        enqueue_msg(&mut app, Msg::Play);
        settle(&mut app);
        // The session seeds Picking within a couple frames.
        assert_ne!(
            current_model(&mut app).replica.players.len(),
            0,
            "the solo session connected the roster"
        );

        let mut reached = false;
        for _ in 0..400 {
            if current_screen(&mut app) == Screen::Podium {
                reached = true;
                break;
            }
            advance_clock(&mut app, Duration::from_secs(5));
        }
        assert!(reached, "the solo match self-drove to the podium screen");
        let m = current_model(&mut app);
        assert!(
            m.replica.podium.as_ref().is_some_and(|p| !p.is_empty()),
            "the podium is populated: {:?}",
            m.replica.podium
        );
    }

    /// A solo session records + replays byte-identically (spec §3.4): the `Msg::Net`
    /// stream the authority produced re-folds into a FRESH app (no live session) to
    /// the same model — the record/replay guarantee for networked play.
    #[test]
    fn solo_session_records_and_replays_byte_identical() {
        use buiy_core::mvu::{MsgLog, RecordSession};
        use buiy_core::replay::replay_into;
        use buiy_core::text::edit::EditLog;

        // Record a solo session through several turns of authoritative events.
        let mut rec = boot_solo();
        rec.world_mut().resource_mut::<RecordSession>().start();
        enqueue_msg(&mut rec, Msg::Play);
        settle(&mut rec);
        for _ in 0..30 {
            advance_clock(&mut rec, Duration::from_secs(5));
        }
        let recorded = current_model(&mut rec);
        assert!(
            !recorded.replica.players.is_empty(),
            "the recorded session has a populated replica"
        );

        // Replay the recorded Msg stream into a fresh app WITHOUT a live authority —
        // the recorded `Msg::Net` events reconstruct the replica on their own.
        let mut replay = boot_probe();
        {
            let world = rec.world();
            replay_into(
                &mut replay,
                world.resource::<MsgLog>(),
                world.resource::<EditLog>(),
            );
        }
        settle(&mut replay);
        let replayed = current_model(&mut replay);
        assert_eq!(
            replayed.replica, recorded.replica,
            "the replica replays state-identically from the recorded Msg::Net stream"
        );
        assert_eq!(
            replayed.screen, recorded.screen,
            "the screen (incl. any podium lift) replays identically"
        );
    }

    // --- Avatar editor + navigation (reducer-level, unchanged by W3) --------

    #[test]
    fn avatar_editor_open_tab_and_save_round_trip() {
        let mut m = Dooduel::default();
        assert!(!m.avatar.editor_open);
        assert_eq!(m.avatar.kind, HumanAvatar::Default);

        update(&mut m, Msg::OpenAvatarEditor);
        assert!(m.avatar.editor_open);
        assert_eq!(m.avatar.tab, AvatarTab::Gallery, "opens on the gallery tab");

        let reset_before = m.avatar.reset_seq;
        update(&mut m, Msg::SetAvatarTab(AvatarTab::Draw));
        assert_eq!(m.avatar.tab, AvatarTab::Draw);
        assert_eq!(m.avatar.reset_seq, reset_before + 1);

        update(&mut m, Msg::SelectAvatarColor(4));
        assert_eq!(m.avatar.draft_color_idx, 4);
        assert!(!m.avatar.draft_eraser, "picking a color clears the eraser");
        update(&mut m, Msg::ToggleAvatarEraser);
        assert!(m.avatar.draft_eraser);
        update(&mut m, Msg::SelectAvatarSize(2));
        assert_eq!(m.avatar.draft_size_idx, 2);

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
        );
        assert!(!m.avatar.editor_open);
        update(&mut m, Msg::ResetAvatar);
        assert_eq!(m.avatar.kind, HumanAvatar::Default);
    }

    #[test]
    fn home_boots_and_create_room_navigates_to_lobby() {
        let mut app = boot_probe();
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("Dooduel"),
            "home shows the wordmark:\n{report}"
        );
        for label in ["▶ Play", "Create a room", "Join a room"] {
            assert!(
                get_by_role(app.world_mut(), A11yRole::Button, Some(label), None).is_ok(),
                "home has the {label:?} button by role+name"
            );
        }
        let create = get_by_role(
            app.world_mut(),
            A11yRole::Button,
            Some("Create a room"),
            None,
        )
        .expect("Create-a-room button");
        click(app.world_mut(), create).expect("Create is clickable");
        settle(&mut app);
        // Create opens a NETWORKED connection (Joining) and shows the connecting card
        // (a probe app has no server, so it stays Joining until a scripted Welcome).
        assert_eq!(current_screen(&mut app), Screen::Lobby);
        assert_eq!(
            current_model(&mut app).net,
            NetState::Joining,
            "Create opens a networked connection (server-issued code)"
        );
        assert!(
            snapshot_report(app.world_mut()).contains("Connecting"),
            "the connecting card renders while Joining"
        );

        // Simulate the server admitting us as seat 0 (host); the live lobby renders.
        seed_lobby(&mut app, 0, 0, "ABCDEF");
        assert!(
            matches!(current_model(&mut app).net, NetState::Connected { .. }),
            "Welcome marks the session Connected"
        );
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("Private room"),
            "the live lobby renders:\n{report}"
        );
        assert!(
            report.contains("ABCDEF"),
            "the server-issued room code is shown:\n{report}"
        );
        assert!(
            get_by_role(
                app.world_mut(),
                A11yRole::Button,
                Some("▶ Start game"),
                None
            )
            .is_ok(),
            "the host has an enabled Start button"
        );
    }

    /// Simulate the server admitting this client to a lobby: a `Welcome` (which marks the
    /// session Connected) + a `RoomState` seeding the roster at phase Idle (the lobby).
    fn seed_lobby(app: &mut App, my_seat: usize, host: usize, code: &str) {
        net(
            app,
            ServerEvent::Welcome {
                seat: my_seat,
                room_code: code.to_string(),
                reconnect_token: "tok".to_string(),
                protocol_version: dooduel_core::protocol::PROTOCOL_VERSION,
            },
        );
        let players = vec![
            ReplicaPlayer {
                name: "Ada".to_string(),
                avatar: WireAvatar::Default,
                connected: true,
                is_bot: false,
                score: 0,
                guessed: false,
            },
            ReplicaPlayer {
                name: "Bo".to_string(),
                avatar: WireAvatar::Default,
                connected: true,
                is_bot: false,
                score: 0,
                guessed: false,
            },
        ];
        net(
            app,
            ServerEvent::RoomState(RoomReplica {
                room_code: code.to_string(),
                my_seat,
                host,
                players,
                phase: Phase::Idle,
                ..Default::default()
            }),
        );
        settle(app);
    }

    #[test]
    fn join_connects_then_the_guest_lobby_waits_for_the_host() {
        let mut app = boot_probe();
        enqueue_msg(&mut app, Msg::SetName("Zed".to_string()));
        enqueue_msg(&mut app, Msg::GoJoin);
        settle(&mut app);
        assert_eq!(current_screen(&mut app), Screen::Join);

        enqueue_msg(&mut app, Msg::SetJoinCode("abcdef".to_string()));
        enqueue_msg(&mut app, Msg::SubmitJoin);
        settle(&mut app);
        assert_eq!(
            current_model(&mut app).net,
            NetState::Joining,
            "Join opens a networked connection"
        );

        // The server seats us at 1 (a guest; host is seat 0).
        seed_lobby(&mut app, 1, 0, "ABCDEF");
        let m = current_model(&mut app);
        assert!(!m.is_host_seat(), "seat 1 is a guest, not the host");
        let report = snapshot_report(app.world_mut());
        assert!(
            report.contains("You're in"),
            "the guest lobby renders:\n{report}"
        );
        assert!(
            report.contains("Waiting for the host"),
            "a guest waits for the host to start:\n{report}"
        );
        assert!(
            get_by_role(
                app.world_mut(),
                A11yRole::Button,
                Some("▶ Start game"),
                None
            )
            .is_err(),
            "a guest has no Start button"
        );
    }

    #[test]
    fn copy_code_stages_the_room_code_for_the_clipboard_with_a_toast() {
        let mut m = Dooduel {
            replica: RoomReplica {
                room_code: "ABCDEF".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        update(&mut m, Msg::CopyCode);
        assert_eq!(
            m.clipboard_outbox,
            vec!["ABCDEF".to_string()],
            "the code is staged for the clipboard drain"
        );
        assert_eq!(
            m.toast.as_deref(),
            Some("Copied!"),
            "a confirmation toast flashes"
        );

        // With no code yet (never reached a lobby) it is a no-op — never an empty copy.
        let mut blank = Dooduel::default();
        update(&mut blank, Msg::CopyCode);
        assert!(
            blank.clipboard_outbox.is_empty(),
            "no room code → nothing staged"
        );
    }

    #[test]
    fn drain_clipboard_outbox_writes_staged_copies_to_the_clipboard() {
        use buiy_core::text::edit::{Clipboard, MemClipboard};
        let mut app = App::new();
        app.insert_resource(Clipboard(Box::new(MemClipboard::default())));
        app.world_mut().spawn(Dooduel {
            clipboard_outbox: vec!["ROOMXY".to_string()],
            ..Default::default()
        });
        app.add_systems(Update, drain_clipboard_outbox);
        app.update();
        let mut clip = app.world_mut().resource_mut::<Clipboard>();
        assert_eq!(
            clip.0.get_text().as_deref(),
            Some("ROOMXY"),
            "the staged copy reached the clipboard resource"
        );
    }

    // --- Networked reducer paths (W4.5) -------------------------------------

    #[test]
    fn networked_start_match_sends_the_intent_not_a_solo_launch() {
        let mut m = Dooduel {
            net: NetState::Connected {
                room: "ABCDEF".to_string(),
            },
            ..Default::default()
        };
        update(&mut m, Msg::StartMatch);
        assert_eq!(
            m.net_outbox.last(),
            Some(&ClientIntent::StartMatch),
            "a networked StartMatch is an intent, not a solo launch"
        );
        assert!(
            matches!(m.net, NetState::Connected { .. }),
            "the session stays networked"
        );
    }

    #[test]
    fn a_networked_match_start_lifts_the_host_out_of_the_lobby() {
        // The bug the first human playtest hit: the host reaches the Lobby, presses
        // Start (an intent), the server broadcasts the first PhaseChanged — but the
        // shell stayed welded to the Lobby, leaving the host the AFK first drawer. The
        // screen must lift to InGame on match start, symmetric with MatchEnded→Podium.
        let mut m = Dooduel::default();
        update(&mut m, Msg::CreateRoom);
        assert_eq!(m.screen, Screen::Lobby, "Create shows the lobby");
        update(
            &mut m,
            Msg::Net(ServerEvent::Welcome {
                seat: 0,
                room_code: "ABCDEF".to_string(),
                reconnect_token: "tok".to_string(),
                protocol_version: dooduel_core::protocol::PROTOCOL_VERSION,
            }),
        );
        assert_eq!(
            m.screen,
            Screen::Lobby,
            "still the lobby until the match actually starts"
        );
        update(
            &mut m,
            Msg::Net(ServerEvent::PhaseChanged {
                phase: Phase::Picking,
                drawer: Some(0),
                round: 1,
                total_rounds: 2,
                remaining: Duration::from_secs(15),
            }),
        );
        assert_eq!(
            m.screen,
            Screen::InGame,
            "the first in-match phase lifts the host out of the lobby onto the board"
        );
    }

    #[test]
    fn a_reconnect_into_a_live_match_lands_on_the_board_not_the_lobby() {
        // A mid-match reconnect replays the full RoomState seed carrying the live phase;
        // the returning player must land on the board, not stall in the lobby.
        let mut m = Dooduel::default();
        start_connect(
            &mut m,
            Connect::Join {
                code: "ABCDEF".to_string(),
                reconnect: Some("tok".to_string()),
            },
            false,
        );
        assert_eq!(m.screen, Screen::Lobby);
        apply_event(
            &mut m,
            ServerEvent::Welcome {
                seat: 1,
                room_code: "ABCDEF".to_string(),
                reconnect_token: "tok2".to_string(),
                protocol_version: dooduel_core::protocol::PROTOCOL_VERSION,
            },
        );
        let replica = RoomReplica {
            room_code: "ABCDEF".to_string(),
            phase: Phase::Drawing,
            ..Default::default()
        };
        apply_event(&mut m, ServerEvent::RoomState(replica));
        assert_eq!(
            m.screen,
            Screen::InGame,
            "a live-match reconnect lands on the board"
        );
    }

    #[test]
    fn a_rejected_join_toasts_and_reverts_to_the_code_screen() {
        let mut m = Dooduel::default();
        start_connect(
            &mut m,
            Connect::Join {
                code: "ZZZZZZ".to_string(),
                reconnect: None,
            },
            false,
        );
        assert_eq!(m.net, NetState::Joining);
        apply_event(
            &mut m,
            ServerEvent::Error {
                code: ErrorCode::RoomNotFound,
                message: "no such room".to_string(),
            },
        );
        assert!(m.toast.is_some(), "the rejection surfaces as a toast");
        assert_eq!(
            m.net,
            NetState::Offline,
            "a failed join drops the connection"
        );
        assert_eq!(
            m.screen,
            Screen::Join,
            "a guest returns to the code screen to retry"
        );
    }

    #[test]
    fn a_drop_stages_a_token_rejoin() {
        let mut m = Dooduel {
            net: NetState::Connected {
                room: "ABCDEF".to_string(),
            },
            room_code: "ABCDEF".to_string(),
            reconnect_token: "tok-xyz".to_string(),
            ..Default::default()
        };
        update(&mut m, Msg::NetDropped);
        assert!(
            matches!(m.net, NetState::Dropped { .. }),
            "a drop enters the Dropped state"
        );
        assert_eq!(
            m.pending_connect,
            Some(Connect::Join {
                code: "ABCDEF".to_string(),
                reconnect: Some("tok-xyz".to_string()),
            }),
            "a drop stages a token rejoin for WsClientPlugin"
        );
    }

    #[test]
    fn networked_back_stages_a_leave_before_going_offline() {
        // I-4: leaving a networked room releases the seat gracefully (skips the 45s
        // grace) so it doesn't ghost.
        let mut m = Dooduel {
            net: NetState::Connected {
                room: "ABCDEF".to_string(),
            },
            ..Default::default()
        };
        update(&mut m, Msg::Back);
        assert_eq!(
            m.net_outbox.last(),
            Some(&ClientIntent::Leave),
            "networked Back stages a Leave intent"
        );
        assert_eq!(m.net, NetState::Offline);
        assert_eq!(m.screen, Screen::Home);
        // An offline Back has no session to leave — no intent staged.
        let mut solo = Dooduel::default();
        update(&mut solo, Msg::Back);
        assert!(
            solo.net_outbox.is_empty(),
            "an offline Back stages no intent"
        );
    }

    #[test]
    fn giving_up_the_rejoin_is_a_terminal_connection_lost() {
        // I-1: when the bounded auto-rejoin exhausts its tries, the UI reaches a terminal
        // "connection lost" state (Offline + Home), not a spin.
        let mut m = Dooduel {
            net: NetState::Dropped {
                token: "t".to_string(),
            },
            room_code: "ABCDEF".to_string(),
            reconnect_token: "t".to_string(),
            ..Default::default()
        };
        update(&mut m, Msg::NetGaveUp);
        assert_eq!(m.net, NetState::Offline);
        assert_eq!(m.screen, Screen::Home);
        assert!(m.pending_connect.is_none(), "no further rejoin is staged");
        assert_eq!(m.toast.as_deref(), Some("Connection lost."));
    }

    #[test]
    fn probe_avatar_editor_opens_from_home_and_saves() {
        let mut app = boot_probe();
        let edit = get_by_role(
            app.world_mut(),
            A11yRole::Button,
            Some("Edit your avatar"),
            None,
        )
        .expect("the pencil edit affordance is a locatable button");
        click(app.world_mut(), edit).expect("the pencil is clickable");
        settle(&mut app);
        assert!(current_model(&mut app).avatar.editor_open);

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
        assert_eq!(m.avatar.kind, HumanAvatar::Custom);
    }
}
