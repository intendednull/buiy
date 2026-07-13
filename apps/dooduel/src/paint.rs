//! Dooduel paint subsystem — CPU-authoritative drawing canvases.
//!
//! The drawing surface skribbl.io needs. The app owns an RGBA pixel
//! buffer per canvas, paints into it on the CPU (brush / eraser / flood-fill
//! bucket), mirrors it into a bevy [`Image`] asset, and the framework's
//! [`RasterImage`] primitive samples that texture onto a Buiy layout node.
//!
//! **Why CPU-authoritative** (the W1 strategy decision — see the journal): a
//! plain `Vec<u8>` makes flood-fill a stack-based scanline, the eraser a
//! background-color stamp, and the FINAL's undo/serialization a buffer snapshot —
//! all trivial. The cost is a full buffer re-upload per dirty frame; acceptable
//! for a prototype, noted as the one perf watch-item.
//!
//! **Two canvases (W5 — the two-surfaces question W1 predicted).** The in-game
//! drawing canvas (720×450) and the avatar editor's draw-your-own canvas
//! (220×220) are BOTH consumers of the same [`PaintSurface`] type. Rather than a
//! singleton `Resource`, the surfaces live in ONE [`PaintCanvases`] resource
//! keyed by [`CanvasKind`] — a keyed map that generalizes to N canvases (the
//! scalable answer). Each canvas NODE carries a `CanvasKind` marker so the shared
//! pointer observers know which surface to edit. **Why keep the pixel buffers in
//! a resource (not components on the nodes):** the `buiy_view` reconciler despawns
//! a `raster(...)` node when its screen/overlay closes and respawns it on
//! re-entry; a `PaintSurface` component would be lost on that despawn, but the
//! resource-held buffer survives, so the drawing persists (the game canvas across
//! turn re-renders, the avatar across editor re-opens).
//!
//! The model learns the image handles through the funnel
//! (`dooduel::announce_canvases` → `Msg::CanvasesReady`); the reconciler owns the
//! node lifecycle; this module keeps only the CPU pixel state + the `Image`s.

use bevy::asset::{Assets, Handle, RenderAssetUsages};
use bevy::image::Image;
use bevy::math::Vec2;
use bevy::prelude::{
    Added, Entity, GlobalTransform, NonSendMut, On, Query, Reflect, Res, Resource,
};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::time::Time;
use buiy::prelude::*;
use buiy_core::render::{Border, Corners, Radius, RasterImage};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::time::Duration;

/// The pure pixel math + palette + [`PaintBuffer`] moved to `dooduel_core::canvas`
/// (M1 W0.3); these re-exports keep `paint::PALETTE` / `paint::Tool` / `paint::PAPER`
/// / `paint::BRUSH_SIZES` paths (view modules, bins) stable after the extraction.
pub use dooduel_core::canvas::{BRUSH_SIZES, CANVAS_H, CANVAS_W, PALETTE, PAPER, Tool};
use dooduel_core::canvas::{PaintBuffer, eraser_radius, flood_fill, stamp_circle, stroke_segment};
use dooduel_core::protocol::{CanvasOp, ClientIntent, MAX_OP_POINTS, MAX_STROKE_POINTS};
use dooduel_core::transport::ClientTransport;

use crate::game::Phase;
use crate::net::{CanvasProgress, ClientNet};

// The in-game drawing surface size (logical px == image resolution, 1:1 window→pixel
// mapping) moved to `dooduel_core::canvas` (W2-review I3) so the authority bound-checks
// against the same size; re-exported above to keep `paint::CANVAS_W` / `paint::CANVAS_H`.

/// The avatar editor's draw-your-own surface (the design's 220×220 canvas, W5).
pub const AVATAR_W: usize = 220;
pub const AVATAR_H: usize = 220;

/// Which drawing surface a `raster(...)` node paints into. A marker component the
/// pointer observers read to route an edit to the right [`PaintSurface`] in the
/// keyed [`PaintCanvases`] map (W5 two-canvas generalization). The app's OTHER
/// rasters (the displayed custom-avatar image) carry no `CanvasKind`, so they get
/// no paint observers.
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect)]
pub enum CanvasKind {
    /// The 720×450 in-game drawing sheet.
    Game,
    /// The 220×220 avatar editor's draw-your-own canvas.
    Avatar,
}

// ---------------------------------------------------------------------------
// The drawer's outbound canvas intents (spec §3.5) — the drawer paints its Game
// canvas optimistically AND relays the ops to the authority.
// ---------------------------------------------------------------------------

/// How often a held stroke coalesces its accumulated points into a `done: false`
/// batch (spec §3.5 — transport batching, never decimation). A short press-release
/// finalizes immediately; a long drag flushes every ~this.
const COALESCE: Duration = Duration::from_millis(40);

/// One raw ink action the Game-canvas pointer observers record for the wire (the
/// paint stays optimistic; these mirror it to the authority). Tool-change / undo /
/// clear from the toolbar enter as [`InkEvent::Clear`] / [`InkEvent::Undo`].
enum InkEvent {
    /// Pen down: a fresh stroke's first sample + its effective stamp (eraser is
    /// already resolved to `PAPER` + the ×1.6 radius, spec §2.2 — no tool on the wire).
    Begin {
        x: i32,
        y: i32,
        color: [u8; 4],
        radius: i32,
    },
    /// A drag sample (the exact post-`to_pixel` integer coordinate).
    Point { x: i32, y: i32 },
    /// Pen up: finalize the open stroke (`done: true`).
    End,
    /// A bucket fill (a discrete op).
    Fill { x: i32, y: i32, color: [u8; 4] },
    /// The toolbar Clear (drop the open stroke, truncate the log).
    Clear,
    /// The toolbar Undo (finalize the open stroke FIRST — undo-of-open is
    /// unreachable from an honest client, R1 — then remove the last op).
    Undo,
}

/// The stroke the drawer currently has open on the wire (spans batches under one
/// client `stroke_id`, spec §3.5). `sent` is the count already relayed for THIS op —
/// the drawer finalizes locally at [`MAX_OP_POINTS`] (R3) so the server never
/// auto-splits an honest client's stroke.
struct OpenWireStroke {
    id: u64,
    color: [u8; 4],
    radius: i32,
    sent: usize,
}

/// The drawer→authority stroke relay (spec §3.5). The Game-canvas observers push
/// [`InkEvent`]s (Send data); [`flush_strokes`] replays them through [`ClientNet`],
/// coalescing points into batches, finalizing at [`MAX_OP_POINTS`], and finalizing
/// before an Undo. Only the Game canvas feeds this — the avatar editor is local.
#[derive(Resource, Default)]
struct StrokeSender {
    /// This frame's ink, in pointer/toolbar order.
    events: Vec<InkEvent>,
    /// The stroke open on the wire (persists across frames — a drag spans flushes).
    open: Option<OpenWireStroke>,
    /// Accumulated, not-yet-batched points for the open op (the coalescing buffer).
    unsent: Vec<(i32, i32)>,
    /// The next client `stroke_id` (a batching handle; never travels in a logged op).
    next_id: u64,
    /// Elapsed at the last coalesce flush.
    last_flush: Duration,
}

// ---------------------------------------------------------------------------
// One drawing surface — the Bevy-free pixel buffer + its `Image` mirror.
// ---------------------------------------------------------------------------

/// The bevy-facing state of one drawing surface: the pure [`PaintBuffer`] (the
/// pixels, brush, and undo ring, in `dooduel_core::canvas`) plus the bevy `Image`
/// handle it mirrors into, the canvas layout node, and whether painting is accepted.
///
/// [`Deref`]s to the inner [`PaintBuffer`], so `.pixels` / `.tool` / `.begin()` /
/// `.clear()` / … stay reachable unchanged (the buffer's own `dirty` flag drives
/// the CPU→`Image` mirror). Shared verbatim by the in-game canvas and the avatar
/// editor (W5).
pub struct PaintSurface {
    /// The pure pixel state + brush + undo ring (Bevy-free).
    buffer: PaintBuffer,
    pub handle: Handle<Image>,
    /// The canvas layout node (the pointer→pixel mapping reads its transform+rect).
    pub canvas_entity: Entity,
    /// Whether painting is accepted (the drawer in-game / the open editor). The
    /// pointer observers early-out when `false`. Synced from the model each frame.
    pub enabled: bool,
}

impl Deref for PaintSurface {
    type Target = PaintBuffer;
    fn deref(&self) -> &PaintBuffer {
        &self.buffer
    }
}

impl DerefMut for PaintSurface {
    fn deref_mut(&mut self) -> &mut PaintBuffer {
        &mut self.buffer
    }
}

impl PaintSurface {
    /// A blank surface filled with `bg`, mirroring `handle`.
    pub fn new(width: usize, height: usize, bg: [u8; 4], handle: Handle<Image>) -> Self {
        Self {
            buffer: PaintBuffer::new(width, height, bg),
            handle,
            canvas_entity: Entity::PLACEHOLDER,
            enabled: false,
        }
    }

    /// Map a window-space pointer position to canvas pixel coords via the canvas
    /// node's transform + resolved rect. `None` when the point is outside the
    /// canvas (or the node has no size yet).
    pub fn to_pixel(
        &self,
        win_pos: Vec2,
        gt: &GlobalTransform,
        layout: &ResolvedLayout,
    ) -> Option<(i32, i32)> {
        let size = layout.size;
        if size.x <= 0.0 || size.y <= 0.0 {
            return None;
        }
        let local = win_pos - gt.translation().truncate();
        let px = (local.x / size.x * self.width as f32).floor() as i32;
        let py = (local.y / size.y * self.height as f32).floor() as i32;
        if px < 0 || py < 0 || px >= self.width as i32 || py >= self.height as i32 {
            return None;
        }
        Some((px, py))
    }
}

// ---------------------------------------------------------------------------
// The bevy-facing multi-canvas resource + systems.
// ---------------------------------------------------------------------------

/// All of the app's drawing surfaces, keyed by [`CanvasKind`] (W5). A keyed map
/// so the design's TWO canvases — and any future one — share one resource and one
/// set of observers/sync systems. Also holds the **committed** custom-avatar
/// image (`saved_avatar`): the avatar editor draws into the `Avatar` scratch
/// surface, and a save COPIES its pixels into `saved_avatar` (the image every
/// avatar around the app samples), so further editing doesn't mutate the
/// already-saved pic.
#[derive(Resource)]
pub struct PaintCanvases {
    surfaces: HashMap<CanvasKind, PaintSurface>,
    /// The committed custom-avatar image — what displays around the app sample.
    pub saved_avatar: Handle<Image>,
    saved_pixels: Vec<u8>,
    saved_dirty: bool,
    /// Bumped whenever `saved_pixels` changes (a save-commit or a boot restore).
    /// The W6 persistence sink keys on this so it re-writes AFTER the scratch→saved
    /// copy lands (which lags the `save_seq` bump by a frame), not before.
    saved_version: u64,
}

impl PaintCanvases {
    /// A shared reference to one surface (panics if the kind was never inserted —
    /// both are inserted at startup).
    pub fn surface(&self, kind: CanvasKind) -> &PaintSurface {
        self.surfaces.get(&kind).expect("canvas surface exists")
    }

    /// A mutable reference to one surface.
    pub fn surface_mut(&mut self, kind: CanvasKind) -> &mut PaintSurface {
        self.surfaces.get_mut(&kind).expect("canvas surface exists")
    }

    /// The image handle a given canvas mirrors into.
    pub fn handle(&self, kind: CanvasKind) -> Handle<Image> {
        self.surface(kind).handle.clone()
    }

    /// The committed custom-avatar pixels (W6 persistence: PNG-encoded on save).
    pub fn saved_pixels(&self) -> &[u8] {
        &self.saved_pixels
    }

    /// A monotonic version of the committed pixels — the W6 persistence sink keys
    /// on this so it writes AFTER a save-commit's pixel copy lands.
    pub fn saved_version(&self) -> u64 {
        self.saved_version
    }

    /// Restore committed custom-avatar pixels (W6 persistence: decoded from the
    /// stored PNG at boot). Marks the saved image dirty so the mirror re-uploads.
    /// The bytes must be `AVATAR_W * AVATAR_H * 4` RGBA; a mismatched length is
    /// ignored (a corrupt/old save is a no-op, not a panic).
    pub fn restore_saved_pixels(&mut self, pixels: Vec<u8>) {
        if pixels.len() == AVATAR_W * AVATAR_H * 4 {
            self.saved_pixels = pixels;
            self.saved_dirty = true;
            self.saved_version = self.saved_version.wrapping_add(1);
        }
    }
}

/// Startup: create the game + avatar scratch + saved-avatar [`Image`]s and the
/// [`PaintCanvases`] resource. The canvas NODES are the reconciler-owned
/// `raster(...)` elements; [`wire_canvas_node`] binds observers when they appear.
fn setup_canvases(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let make_image = |w: usize, h: usize| -> Image {
        let pixels: Vec<u8> = PAPER.iter().copied().cycle().take(w * h * 4).collect();
        Image::new(
            Extent3d {
                width: w as u32,
                height: h as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            // MAIN_WORLD | RENDER_WORLD: the render world gets a CLONE each modify
            // and the main-world `data` survives, so we keep painting into it.
            RenderAssetUsages::all(),
        )
    };
    let game = images.add(make_image(CANVAS_W, CANVAS_H));
    let avatar = images.add(make_image(AVATAR_W, AVATAR_H));
    let saved = images.add(make_image(AVATAR_W, AVATAR_H));

    let mut surfaces = HashMap::new();
    surfaces.insert(
        CanvasKind::Game,
        PaintSurface::new(CANVAS_W, CANVAS_H, PAPER, game),
    );
    surfaces.insert(
        CanvasKind::Avatar,
        PaintSurface::new(AVATAR_W, AVATAR_H, PAPER, avatar),
    );
    commands.insert_resource(PaintCanvases {
        surfaces,
        saved_avatar: saved,
        saved_pixels: PAPER
            .iter()
            .copied()
            .cycle()
            .take(AVATAR_W * AVATAR_H * 4)
            .collect(),
        saved_dirty: true,
        saved_version: 0,
    });
}

/// Bind the Press/Drag/Release observers to a view-owned canvas node the frame
/// the `buiy_view` reconciler inserts its [`RasterImage`] (W3a/W5). Matches the
/// node's image handle to a [`CanvasKind`] and tags the node with it so the shared
/// observers route to the right surface. The app's DISPLAY rasters (the saved
/// custom-avatar image around the app) match no canvas handle and are left
/// un-observed. Re-fires whenever a canvas is re-entered; the pixel buffers
/// persist on the resource across respawns, so the drawing survives.
fn wire_canvas_node(
    mut commands: Commands,
    added: Query<(Entity, &RasterImage), Added<RasterImage>>,
    canvases: Option<ResMut<PaintCanvases>>,
) {
    let Some(mut canvases) = canvases else {
        return;
    };
    for (e, raster) in &added {
        let kind = [CanvasKind::Game, CanvasKind::Avatar]
            .into_iter()
            .find(|k| canvases.surface(*k).handle == raster.0);
        let Some(kind) = kind else {
            // A display raster (not paintable). The committed custom-avatar image
            // must render round like the stock doodle badges. The F4b raster
            // rounded clip reads its radius from a `Border` on the node, and the
            // view `raster()` element does NOT lower `.radius()`, so stamp a
            // circular-clip `Border` here. A large radius clamps to half the node
            // size → a circle at any avatar px.
            if raster.0 == canvases.saved_avatar {
                commands.entity(e).insert(Border {
                    radius: Corners::all(Radius::circular(9999.0)),
                    ..Default::default()
                });
            }
            continue;
        };
        canvases.surface_mut(kind).canvas_entity = e;
        commands
            .entity(e)
            .insert(kind)
            .observe(on_canvas_press)
            .observe(on_canvas_drag)
            .observe(on_canvas_release);
    }
}

/// Look up the surface a canvas node paints into (its `CanvasKind` tag).
fn kind_of(q: &Query<&CanvasKind>, e: Entity) -> Option<CanvasKind> {
    q.get(e).ok().copied()
}

/// The effective stamp a stroke op carries on the wire (spec §2.2): the eraser is
/// resolved to `PAPER` + its already-`eraser_radius`-adjusted radius, so color +
/// radius alone determine the stamp (no tool travels).
fn wire_stamp(surface: &PaintSurface) -> ([u8; 4], i32) {
    let color = if surface.tool == Tool::Eraser {
        PAPER
    } else {
        surface.color
    };
    (color, surface.radius)
}

/// Primary press → stroke start (or bucket fill); secondary press → bucket fill. The
/// Game canvas also records the wire op (the drawer relays its ops to the authority).
fn on_canvas_press(
    press: On<Pointer<Press>>,
    mut canvases: ResMut<PaintCanvases>,
    mut sender: ResMut<StrokeSender>,
    kinds: Query<&CanvasKind>,
    xf: Query<(&GlobalTransform, &ResolvedLayout)>,
) {
    let Some(kind) = kind_of(&kinds, press.entity) else {
        return;
    };
    if !canvases.surface(kind).enabled {
        return; // only the drawer / open editor paints
    }
    let Ok((gt, layout)) = xf.get(press.entity) else {
        return;
    };
    let surface = canvases.surface_mut(kind);
    let Some((x, y)) = surface.to_pixel(press.pointer_location.position, gt, layout) else {
        return;
    };
    let button = press.event.button;
    match button {
        PointerButton::Primary => surface.press(x, y),
        PointerButton::Secondary => surface.fill(x, y),
        _ => return,
    }
    if kind == CanvasKind::Game {
        // Mirror the optimistic op onto the wire. A bucket press (or any secondary
        // press) is a Fill; a brush/eraser primary press opens a stroke.
        if surface.tool == Tool::Bucket || button == PointerButton::Secondary {
            sender.events.push(InkEvent::Fill {
                x,
                y,
                color: surface.color,
            });
        } else {
            let (color, radius) = wire_stamp(surface);
            sender.events.push(InkEvent::Begin {
                x,
                y,
                color,
                radius,
            });
        }
    }
}

/// Primary drag → extend the stroke (line-interpolated from the last sample).
fn on_canvas_drag(
    drag: On<Pointer<Drag>>,
    mut canvases: ResMut<PaintCanvases>,
    mut sender: ResMut<StrokeSender>,
    kinds: Query<&CanvasKind>,
    xf: Query<(&GlobalTransform, &ResolvedLayout)>,
) {
    let Some(kind) = kind_of(&kinds, drag.entity) else {
        return;
    };
    if drag.event.button != PointerButton::Primary || !canvases.surface(kind).enabled {
        return;
    }
    let Ok((gt, layout)) = xf.get(drag.entity) else {
        return;
    };
    let surface = canvases.surface_mut(kind);
    // `to_pixel` returns `None` off-canvas, so an edge-drag sample is dropped, never
    // clamped — the exact in-bounds samples the authority accepts whole (R4).
    if let Some((x, y)) = surface.to_pixel(drag.pointer_location.position, gt, layout) {
        surface.extend(x, y);
        if kind == CanvasKind::Game {
            sender.events.push(InkEvent::Point { x, y });
        }
    }
}

/// Release → end the stroke (finalize the wire op).
fn on_canvas_release(
    release: On<Pointer<Release>>,
    mut canvases: ResMut<PaintCanvases>,
    mut sender: ResMut<StrokeSender>,
    kinds: Query<&CanvasKind>,
) {
    if let Some(kind) = kind_of(&kinds, release.entity) {
        canvases.surface_mut(kind).end();
        if kind == CanvasKind::Game {
            sender.events.push(InkEvent::End);
        }
    }
}

/// Mirror every dirty CPU buffer into its [`Image`] asset. `get_mut` fires
/// `AssetEvent::Modified`, so bevy re-extracts + re-uploads the `GpuImage`.
fn sync_canvases_to_images(mut canvases: ResMut<PaintCanvases>, mut images: ResMut<Assets<Image>>) {
    let PaintCanvases {
        surfaces,
        saved_avatar,
        saved_pixels,
        saved_dirty,
        saved_version: _,
    } = &mut *canvases;
    for surface in surfaces.values_mut() {
        if surface.dirty
            && let Some(mut image) = images.get_mut(&surface.handle)
            && let Some(data) = image.data.as_mut()
        {
            data.copy_from_slice(&surface.pixels);
            surface.dirty = false;
        }
    }
    if *saved_dirty
        && let Some(mut image) = images.get_mut(&*saved_avatar)
        && let Some(data) = image.data.as_mut()
    {
        data.copy_from_slice(saved_pixels);
        *saved_dirty = false;
    }
}

/// Mirror the MVU model's tool state onto the [`PaintCanvases`] each frame — the
/// reducer OWNS tool selection (so it is replayable); this is the one-way
/// model→canvas projection the paint observers read. Handles BOTH surfaces + the
/// avatar-save commit.
#[allow(clippy::too_many_arguments)]
fn sync_tools_to_canvases(
    model: Option<Single<&crate::Dooduel>>,
    canvases: Option<ResMut<PaintCanvases>>,
    mut sender: ResMut<StrokeSender>,
    mut last_clear: Local<u64>,
    mut last_undo: Local<u64>,
    mut was_drawing: Local<bool>,
    mut editor_was_open: Local<bool>,
    mut last_a_clear: Local<u64>,
    mut last_a_undo: Local<u64>,
    mut last_a_reset: Local<u64>,
    mut last_save: Local<u64>,
) {
    let (Some(model), Some(mut canvases)) = (model, canvases) else {
        return;
    };

    // --- In-game drawing canvas -------------------------------------------
    {
        let t = &model.tools;
        let g = canvases.surface_mut(CanvasKind::Game);
        g.tool = t.tool;
        g.color = PALETTE[t.color_idx.min(PALETTE.len() - 1)];
        let base = (BRUSH_SIZES[t.size_idx.min(BRUSH_SIZES.len() - 1)] / 2).max(0);
        g.radius = if t.tool == Tool::Eraser {
            eraser_radius(base)
        } else {
            base
        };
        let drawing = model.replica.phase == Phase::Drawing;
        g.enabled = drawing && model.is_drawer();
        let enabled = g.enabled;
        if drawing && !*was_drawing {
            g.clear();
            g.clear_undo();
        }
        *was_drawing = drawing;
        if t.clear_seq != *last_clear {
            g.clear();
            // Clear is NON-undoable locally, matching the server (Clear mints no op,
            // so an undo cannot resurrect the cleared drawing — I-1). Without this a
            // clear-then-undo would restore the pre-clear pixels while the server
            // stays cleared, desyncing the two.
            g.clear_undo();
            *last_clear = t.clear_seq;
            // Relay the drawer's clear to the authority (the guesser's toolbar is
            // disabled, so gate on the drawing-drawer `enabled` flag).
            if enabled {
                sender.events.push(InkEvent::Clear);
            }
        }
        if t.undo_seq != *last_undo {
            // Only relay an Undo intent if the local pop actually succeeded (I-2): a
            // depth-exhausted undo (nothing left in the ring) changes no local pixels,
            // so no Undo may reach the wire — else the server would over-remove ops
            // and desync from the drawer's local buffer.
            let popped = g.undo();
            *last_undo = t.undo_seq;
            if enabled && popped {
                sender.events.push(InkEvent::Undo);
            }
        }
    }

    // --- Avatar editor canvas ---------------------------------------------
    {
        let a = &model.avatar;
        let surface = canvases.surface_mut(CanvasKind::Avatar);
        surface.tool = if a.draft_eraser {
            Tool::Eraser
        } else {
            Tool::Brush
        };
        surface.color = PALETTE[a.draft_color_idx.min(PALETTE.len() - 1)];
        let base = (BRUSH_SIZES[a.draft_size_idx.min(BRUSH_SIZES.len() - 1)] / 2).max(0);
        surface.radius = if a.draft_eraser {
            eraser_radius(base)
        } else {
            base
        };
        surface.enabled = a.editor_open && a.tab == crate::AvatarTab::Draw;
        // A fresh sheet on the editor-open edge (the design's resetAvatarCanvas).
        if a.editor_open && !*editor_was_open {
            surface.clear();
            surface.clear_undo();
        }
        *editor_was_open = a.editor_open;
        if a.reset_seq != *last_a_reset {
            surface.clear();
            surface.clear_undo();
            *last_a_reset = a.reset_seq;
        }
        if a.clear_seq != *last_a_clear {
            surface.clear();
            *last_a_clear = a.clear_seq;
        }
        if a.undo_seq != *last_a_undo {
            surface.undo();
            *last_a_undo = a.undo_seq;
        }
    }

    // --- Commit an avatar save: copy the scratch pixels into the saved image.
    if model.avatar.save_seq != *last_save {
        canvases.saved_pixels = canvases.surface(CanvasKind::Avatar).pixels.clone();
        canvases.saved_dirty = true;
        canvases.saved_version = canvases.saved_version.wrapping_add(1);
        *last_save = model.avatar.save_seq;
    }
}

impl StrokeSender {
    /// Drop all pending ink (no session to relay to — a canvas-only test, or between
    /// matches). Keeps the buffer from growing unbounded.
    fn reset(&mut self) {
        self.events.clear();
        self.open = None;
        self.unsent.clear();
    }
}

/// The stable id of a canvas op (spec §3.5).
fn op_id(op: &CanvasOp) -> u64 {
    match op {
        CanvasOp::Stroke { id, .. } | CanvasOp::Fill { id, .. } => *id,
    }
}

/// Blank a buffer to [`PAPER`] without pushing an undo snapshot (the guesser raster
/// owns the whole surface — it re-rasters, it never undoes).
fn blank(buf: &mut PaintBuffer) {
    for chunk in buf.pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&PAPER);
    }
}

/// Stamp a stroke's exact sample sequence (interpolating between samples), the pure
/// integer op the whole op-log sync stands on (spec §2.2).
fn stamp_points(buf: &mut PaintBuffer, points: &[(i32, i32)], color: [u8; 4], radius: i32) {
    let mut last: Option<(i32, i32)> = None;
    for &(x, y) in points {
        match last {
            Some(l) => stroke_segment(
                &mut buf.pixels,
                buf.width,
                buf.height,
                l,
                (x, y),
                radius,
                color,
            ),
            None => stamp_circle(&mut buf.pixels, buf.width, buf.height, x, y, radius, color),
        }
        last = Some((x, y));
    }
}

/// Replay one [`CanvasOp`] onto a buffer — the guesser rasterizer + (W5) the MCP
/// `get_canvas` share this shape (identical ops ⇒ identical pixels).
fn apply_op(buf: &mut PaintBuffer, op: &CanvasOp) {
    match op {
        CanvasOp::Stroke {
            points,
            color,
            radius,
            ..
        } => stamp_points(buf, points, *color, *radius),
        CanvasOp::Fill { seed, color, .. } => flood_fill(
            &mut buf.pixels,
            buf.width,
            buf.height,
            seed.0,
            seed.1,
            *color,
        ),
    }
}

/// Send the accumulated stroke points as batches (spec §3.5): ≤ [`MAX_STROKE_POINTS`]
/// per batch, finalizing (`done: true`) at [`MAX_OP_POINTS`] and continuing a fresh
/// op seeded with the split point (so the server never auto-splits an honest client,
/// R3). `close` finalizes the open stroke (pen up / a fill / an undo).
fn send_batches(sender: &mut StrokeSender, transport: &mut dyn ClientTransport, close: bool) {
    let mut queue = std::mem::take(&mut sender.unsent);
    // Bind COPIES of the open stroke's fields in the condition, so the borrow of
    // `sender.open` ends before the body (which reassigns it on a split/finalize).
    while let Some((id, color, radius, sent)) = sender
        .open
        .as_ref()
        .map(|o| (o.id, o.color, o.radius, o.sent))
    {
        if queue.is_empty() {
            if close {
                sender.open = None;
            }
            break;
        }
        // `op_full` (below) finalizes AT exactly the cap, so `sent < MAX_OP_POINTS`
        // holds at the top of every iteration ⇒ `op_room >= 1` ⇒ the batch is
        // non-empty.
        let op_room = MAX_OP_POINTS.saturating_sub(sent);
        let take = MAX_STROKE_POINTS.min(op_room).min(queue.len());
        let batch: Vec<(i32, i32)> = queue.drain(..take).collect();
        let last = *batch
            .last()
            .expect("op_room >= 1 keeps the batch non-empty");
        let new_sent = sent + batch.len();
        let more = !queue.is_empty();
        // Finalize at EXACTLY the per-op cap (minor-b): the old `op_room.max(1)` +
        // `&& more` let a full op absorb one more point → MAX_OP_POINTS + 1, which
        // trips the server's `> MAX_OP_POINTS` auto-split. Now the op closes at the
        // cap so the server never auto-splits an honest client's stroke (R3).
        let op_full = new_sent >= MAX_OP_POINTS;
        let done = op_full || (close && !more);
        transport.send(&ClientIntent::Stroke {
            stroke_id: id,
            points: batch,
            color,
            radius,
            done,
        });
        if op_full {
            if more || !close {
                // The pen is still down (or more points remain): continue under a
                // fresh op, seeded with the split point for pixel continuity.
                let new_id = sender.next_id;
                sender.next_id += 1;
                sender.open = Some(OpenWireStroke {
                    id: new_id,
                    color,
                    radius,
                    sent: 0,
                });
                queue.insert(0, last);
            } else {
                // Closing exactly at the cap — no continuation.
                sender.open = None;
            }
        } else if done {
            sender.open = None;
        } else if let Some(o) = sender.open.as_mut() {
            o.sent = new_sent;
        }
    }
    sender.unsent = queue;
}

/// Relay the drawer's recorded ink to the authority (spec §3.5): replay this frame's
/// [`InkEvent`]s through [`ClientNet`], coalescing points, finalizing before an undo
/// (R1) and at [`MAX_OP_POINTS`] (R3). `Option<NonSendMut<ClientNet>>` so a
/// canvas-only harness (no `NetPlugin`) simply drops the ink.
fn flush_strokes(
    mut sender: ResMut<StrokeSender>,
    net: Option<NonSendMut<ClientNet>>,
    time: Res<Time>,
) {
    let now = time.elapsed();
    let Some(mut net) = net else {
        sender.reset();
        return;
    };
    let Some(transport) = net.0.as_mut() else {
        sender.reset();
        return;
    };
    let transport: &mut dyn ClientTransport = &mut **transport;
    let sender = &mut *sender;
    for ev in std::mem::take(&mut sender.events) {
        match ev {
            InkEvent::Begin {
                x,
                y,
                color,
                radius,
            } => {
                send_batches(sender, transport, true); // finalize any lingering open
                let id = sender.next_id;
                sender.next_id += 1;
                sender.open = Some(OpenWireStroke {
                    id,
                    color,
                    radius,
                    sent: 0,
                });
                sender.unsent.push((x, y));
            }
            InkEvent::Point { x, y } => sender.unsent.push((x, y)),
            InkEvent::End => send_batches(sender, transport, true),
            InkEvent::Fill { x, y, color } => {
                send_batches(sender, transport, true);
                transport.send(&ClientIntent::Fill {
                    seed: (x, y),
                    color,
                });
            }
            InkEvent::Clear => {
                sender.open = None;
                sender.unsent.clear();
                transport.send(&ClientIntent::Clear);
            }
            InkEvent::Undo => {
                send_batches(sender, transport, true); // finalize before undo (R1)
                transport.send(&ClientIntent::Undo);
            }
        }
    }
    // Coalesce a held stroke's accumulated points into a `done: false` batch.
    if sender.open.is_some()
        && !sender.unsent.is_empty()
        && now.saturating_sub(sender.last_flush) >= COALESCE
    {
        send_batches(sender, transport, false);
        sender.last_flush = now;
    }
}

/// Re-render the Game canvas from the authoritative log (spec §3.5) — **uniformly,
/// every client**: the surface is a render of `replica.canvas_ops` plus the transient
/// [`CanvasProgress`] overlay stamped on top. The drawer's "specialness" is purely
/// OUTBOUND (optimistic paint + the finalize/clamp rules) — there is no per-role
/// render filter, and the reducer applies every canvas event uniformly.
///
/// Why this is correct for the drawer WITHOUT a filter (the load-bearing detail): the
/// server never echoes the drawer its own ops (no-echo, spec §3.5), so during its own
/// turn `replica.canvas_ops` stays **empty** and this re-raster is never triggered —
/// the drawer's optimistic buffer is left untouched, so an incoming `CanvasUndo` /
/// `CanvasCleared` (idempotent against the local optimistic pop/clear) does not blank
/// it. On a mid-turn reconnect the drawer's `CanvasLog` reseed **populates** the log,
/// which triggers the re-raster and restores the canvas from the log.
///
/// The re-raster fires only when the buffer must actually change: `canvas_ops` changed
/// (an op added/removed/cleared, or a reseed) OR the progress overlay was just cleared
/// (a finalize, or a drawer-disconnect wipe — stale progress pixels must go). A growing
/// progress stroke is merely stamped on top (idempotent — earlier points re-stamp), so
/// it never blanks the buffer, and thus never fights the drawer's optimistic paint.
fn rerender_canvas_from_log(
    model: Option<Single<&crate::Dooduel>>,
    canvases: Option<ResMut<PaintCanvases>>,
    progress: Option<Res<CanvasProgress>>,
    mut last_sig: Local<(usize, u64, u64)>,
    mut last_prog: Local<u64>,
    mut was_active: Local<bool>,
    mut showed_progress: Local<bool>,
) {
    let (Some(model), Some(mut canvases)) = (model, canvases) else {
        return;
    };
    let r = &model.replica;
    if !matches!(r.phase, Phase::Drawing | Phase::Reveal) {
        // On LEAVING a turn (Reveal/Drawing → Picking/Idle/Final) blank the local canvas
        // ONCE, so the next drawer's Picking phase shows a clean sheet instead of the
        // previous turn's ink lingering (mostly under the waiting scrim) until the next
        // Drawing edge (QA cycle-1 F1b). This is a purely LOCAL display reset: the
        // authoritative op log (`replica.canvas_ops`) is already emptied by the server's
        // per-turn `CanvasCleared` (session.rs) and is untouched here, nothing is relayed
        // on the wire, and the Drawing edge re-renders from the (now-empty) log anyway —
        // so replay / late-join and the drawer's optimistic paint are unaffected. Blanking
        // on the FALLING edge (not while IN Reveal) keeps the finished drawing visible
        // through the reveal, then clears it for the next pick.
        if *was_active {
            let buf: &mut PaintBuffer = canvases.surface_mut(CanvasKind::Game);
            blank(buf);
            buf.dirty = true;
        }
        *was_active = false;
        return;
    }
    // The reseed counter is part of the signature: op ids reset per turn, so
    // `(len, last_op_id)` alone is degenerate across turns — a `RoomState`/`CanvasLog`
    // reseed to a same-length log with the same dense ids (a W4 mid-turn reconnect that
    // missed the Picking boundary) would otherwise not re-render (keeping stale ink).
    let sig = (
        r.canvas_ops.len(),
        r.canvas_ops.last().map(op_id).unwrap_or(0),
        model.canvas_reseeds,
    );
    let prog_gen = progress.as_ref().map(|p| p.generation).unwrap_or(0);
    let prog_now = progress.as_ref().is_some_and(|p| !p.points.is_empty());
    let entered = !*was_active;
    *was_active = true;

    let ops_changed = entered || sig != *last_sig;
    // A progress overlay that WAS shown and is now gone must be wiped by a full
    // re-raster (a finalize's idempotent re-stamp, or a drawer-disconnect discard).
    let progress_cleared = *showed_progress && !prog_now;

    if ops_changed || progress_cleared {
        *last_sig = sig;
        *last_prog = prog_gen;
        *showed_progress = prog_now;
        let buf: &mut PaintBuffer = canvases.surface_mut(CanvasKind::Game);
        blank(buf);
        for op in &r.canvas_ops {
            apply_op(buf, op);
        }
        if let Some(p) = &progress
            && !p.points.is_empty()
        {
            stamp_points(buf, &p.points, p.color, p.radius);
        }
        buf.dirty = true;
    } else if prog_now && prog_gen != *last_prog {
        // The in-progress stroke grew — stamp it on top (no blank; a growing stroke's
        // earlier points re-stamp idempotently), so this never touches an empty-log
        // drawer's optimistic buffer.
        *last_prog = prog_gen;
        *showed_progress = true;
        if let Some(p) = &progress {
            let buf: &mut PaintBuffer = canvases.surface_mut(CanvasKind::Game);
            stamp_points(buf, &p.points, p.color, p.radius);
            buf.dirty = true;
        }
    }
}

/// Installs the drawing canvases: setup + the CPU→Image mirror + the model→canvas
/// tool sync + the drawer's outbound stroke relay + the uniform op-log re-render.
/// Kept as a distinct plugin (NOT folded into `dooduel::install`) so the canvases are
/// visibly decoupled from the MVU app — the coexistence story.
pub struct CanvasPlugin;

impl Plugin for CanvasPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CanvasKind>();
        app.init_resource::<StrokeSender>();
        app.add_systems(Startup, setup_canvases);
        app.add_systems(
            Update,
            (
                wire_canvas_node,
                sync_tools_to_canvases,
                flush_strokes,
                rerender_canvas_from_log,
                sync_canvases_to_images,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dooduel_core::protocol::ServerEvent;
    use dooduel_core::transport::ConnStatus;
    use std::collections::HashMap;

    /// A transport that records the intents sent to it (no inbound events).
    struct Rec(Vec<ClientIntent>);
    impl ClientTransport for Rec {
        fn send(&mut self, intent: &ClientIntent) {
            self.0.push(intent.clone());
        }
        fn try_recv(&mut self) -> Option<ServerEvent> {
            None
        }
        fn status(&self) -> ConnStatus {
            ConnStatus::Open
        }
    }

    fn points(n: usize) -> Vec<(i32, i32)> {
        (0..n as i32).map(|x| (x % 100, x / 100)).collect()
    }

    fn per_op_point_counts(rec: &Rec) -> HashMap<u64, usize> {
        let mut per_op: HashMap<u64, usize> = HashMap::new();
        for i in &rec.0 {
            if let ClientIntent::Stroke {
                stroke_id, points, ..
            } = i
            {
                *per_op.entry(*stroke_id).or_default() += points.len();
            }
        }
        per_op
    }

    /// Minor-b: NO logged op may hold more than [`MAX_OP_POINTS`] points (which would
    /// trip the server's `> MAX_OP_POINTS` auto-split). The bug reproduces across TWO
    /// flushes: flush 1 fills the op to EXACTLY the cap with an empty queue, and flush
    /// 2 adds one more point. Red evidence: the old `op_room.max(1)` + `&& more` left
    /// the op open at the cap in flush 1, then absorbed the extra point in flush 2 →
    /// `MAX_OP_POINTS + 1`. The fix finalizes at exactly the cap in flush 1.
    #[test]
    fn send_batches_never_exceeds_max_op_points_across_flushes() {
        let mut sender = StrokeSender {
            open: Some(OpenWireStroke {
                id: 0,
                color: [0, 0, 0, 255],
                radius: 1,
                sent: 0,
            }),
            next_id: 1, // the open op holds id 0; a continuation gets a distinct id
            unsent: points(MAX_OP_POINTS), // fills the op to EXACTLY the cap
            ..Default::default()
        };
        let mut rec = Rec(Vec::new());
        send_batches(&mut sender, &mut rec, false); // flush 1: coalesce, keep open

        sender.unsent = points(10); // flush 2: more points (pen still down) + close
        send_batches(&mut sender, &mut rec, true);

        for (id, n) in per_op_point_counts(&rec) {
            assert!(
                n <= MAX_OP_POINTS,
                "op {id} holds {n} points, exceeding MAX_OP_POINTS ({MAX_OP_POINTS})"
            );
        }
    }
}
