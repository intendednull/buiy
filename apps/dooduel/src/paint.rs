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
//! turn re-renders, the avatar across editor re-opens). [[buiy-scribbl-campaign]]
//!
//! The model learns the image handles through the funnel
//! (`dooduel::announce_canvases` → `Msg::CanvasesReady`); the reconciler owns the
//! node lifecycle; this module keeps only the CPU pixel state + the `Image`s.

use bevy::asset::{Assets, Handle, RenderAssetUsages};
use bevy::image::Image;
use bevy::math::Vec2;
use bevy::prelude::{Added, Entity, GlobalTransform, On, Query, Reflect};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use buiy::prelude::*;
use buiy_core::render::{Border, Corners, Radius, RasterImage};
use std::collections::HashMap;

/// The in-game drawing surface size in logical px == the image resolution (kept
/// equal so window→pixel mapping is 1:1).
pub const CANVAS_W: usize = 720;
pub const CANVAS_H: usize = 450;

/// The avatar editor's draw-your-own surface (the design's 220×220 canvas, W5).
pub const AVATAR_W: usize = 220;
pub const AVATAR_H: usize = 220;

/// The white paper the canvas starts on (skribbl.io's blank sheet).
pub const PAPER: [u8; 4] = [255, 255, 255, 255];

/// The design's 16-color toolbar palette (`PALETTE`, in order), as exact sRGB.
/// The model's `ToolState.color_idx` (in-game) / `AvatarState.draft_color_idx`
/// (editor) indexes this; the view renders it as swatches, the sync maps the
/// selected index to [`PaintSurface::color`].
pub const PALETTE: [[u8; 4]; 16] = [
    [0x14, 0x16, 0x1b, 255], // ink
    [0xff, 0xff, 0xff, 255], // white
    [0x9a, 0xa0, 0xaa, 255], // grey
    [0xb3, 0x26, 0x1e, 255], // dark red
    [0xe8, 0x45, 0x3f, 255], // red
    [0xf0, 0x8a, 0x3c, 255], // orange
    [0xf4, 0xc2, 0x0d, 255], // yellow
    [0x8b, 0xc3, 0x4a, 255], // lime
    [0x1c, 0x8a, 0x52, 255], // green
    [0x1f, 0x9e, 0x8d, 255], // teal
    [0x2f, 0x9b, 0xdb, 255], // blue
    [0x3a, 0x63, 0xee, 255], // indigo
    [0x5b, 0x46, 0xe5, 255], // violet
    [0x93, 0x33, 0xea, 255], // purple
    [0xe0, 0x52, 0x9c, 255], // pink
    [0x8a, 0x5a, 0x35, 255], // brown
];

/// The design's four brush **diameters** (`BRUSH_SIZES`, logical px). The model's
/// `ToolState.size_idx` / `AvatarState.draft_size_idx` indexes this; the sync
/// halves it to a stamp radius.
pub const BRUSH_SIZES: [i32; 4] = [3, 6, 11, 18];

/// The active tool. `Bucket` flood-fills on press; `Brush`/`Eraser` stroke. Held
/// on the MVU model (`ToolState`) so tool selection is reducer-owned + replayable,
/// and mirrored onto a [`PaintSurface`] each frame by [`sync_tools_to_canvases`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum Tool {
    #[default]
    Brush,
    Eraser,
    Bucket,
}

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
// Pure paint math — no bevy, unit-testable headless. Operates on a flat RGBA
// `[u8]` buffer of `width * height * 4` bytes, row-major, top-left origin.
// ---------------------------------------------------------------------------

/// Read the RGBA at `(x, y)` (caller guarantees in-bounds).
fn get(px: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
    let i = (y * w + x) * 4;
    [px[i], px[i + 1], px[i + 2], px[i + 3]]
}

/// Write the RGBA at `(x, y)` (caller guarantees in-bounds).
fn set(px: &mut [u8], w: usize, x: usize, y: usize, c: [u8; 4]) {
    let i = (y * w + x) * 4;
    px[i..i + 4].copy_from_slice(&c);
}

/// Stamp a filled circle of `radius` logical px centered at `(cx, cy)`, clipped
/// to the buffer. `radius == 0` writes the single center pixel (a 1px dot).
pub fn stamp_circle(
    px: &mut [u8],
    w: usize,
    h: usize,
    cx: i32,
    cy: i32,
    radius: i32,
    color: [u8; 4],
) {
    let r = radius.max(0);
    let r2 = r * r;
    let x0 = (cx - r).max(0);
    let x1 = (cx + r).min(w as i32 - 1);
    let y0 = (cy - r).max(0);
    let y1 = (cy + r).min(h as i32 - 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (dx, dy) = (x - cx, y - cy);
            if dx * dx + dy * dy <= r2 {
                set(px, w, x as usize, y as usize, color);
            }
        }
    }
}

/// Stamp circles along the segment `from -> to` at ≤1px spacing, so a fast drag
/// never leaves gaps between pointer samples (the line-interpolation property).
/// Endpoints inclusive.
pub fn stroke_segment(
    px: &mut [u8],
    w: usize,
    h: usize,
    from: (i32, i32),
    to: (i32, i32),
    radius: i32,
    color: [u8; 4],
) {
    let (x0, y0) = from;
    let (x1, y1) = to;
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let x = (x0 as f32 + (x1 - x0) as f32 * t).round() as i32;
        let y = (y0 as f32 + (y1 - y0) as f32 * t).round() as i32;
        stamp_circle(px, w, h, x, y, radius, color);
    }
}

/// Stack-based scanline flood fill from `(x, y)`: replace the contiguous region
/// of the seed's color with `new_color`. Bounded (each pixel is set at most once;
/// the seed-color != new_color guard rules out re-visits), so it always
/// terminates. A seed outside the buffer, or already `new_color`, is a no-op.
pub fn flood_fill(px: &mut [u8], w: usize, h: usize, x: i32, y: i32, new_color: [u8; 4]) {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        return;
    }
    let (sx, sy) = (x as usize, y as usize);
    let target = get(px, w, sx, sy);
    if target == new_color {
        return; // nothing to do — avoids the infinite-loop degenerate.
    }
    let mut stack = vec![(sx, sy)];
    while let Some((seed_x, seed_y)) = stack.pop() {
        if get(px, w, seed_x, seed_y) != target {
            continue; // already filled via another span
        }
        // Expand the span to the row's contiguous target run.
        let mut lx = seed_x;
        while lx > 0 && get(px, w, lx - 1, seed_y) == target {
            lx -= 1;
        }
        let mut rx = seed_x;
        while rx + 1 < w && get(px, w, rx + 1, seed_y) == target {
            rx += 1;
        }
        for fx in lx..=rx {
            set(px, w, fx, seed_y, new_color);
        }
        // Seed each contiguous target run in the rows above and below.
        for ny in [
            seed_y.checked_sub(1),
            (seed_y + 1 < h).then_some(seed_y + 1),
        ]
        .into_iter()
        .flatten()
        {
            let mut fx = lx;
            while fx <= rx {
                if get(px, w, fx, ny) == target {
                    stack.push((fx, ny));
                    while fx <= rx && get(px, w, fx, ny) == target {
                        fx += 1;
                    }
                } else {
                    fx += 1;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// One drawing surface — the CPU pixel state + brush, mirrored into one `Image`.
// ---------------------------------------------------------------------------

/// The CPU-authoritative state of a single drawing surface: the pixel buffer, the
/// current brush, and the handle to the [`Image`] asset the buffer mirrors into.
/// Shared verbatim by the in-game canvas and the avatar editor (W5).
pub struct PaintSurface {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub bg: [u8; 4],
    pub handle: Handle<Image>,
    /// The canvas layout node (the pointer→pixel mapping reads its transform+rect).
    pub canvas_entity: Entity,
    pub color: [u8; 4],
    pub radius: i32,
    pub tool: Tool,
    /// Whether painting is accepted (the drawer in-game / the open editor). The
    /// pointer observers early-out when `false`. Synced from the model each frame.
    pub enabled: bool,
    /// Undo snapshots (full-buffer copies, newest last), pushed BEFORE each edit.
    /// Capped at [`UNDO_DEPTH`] — the design's snapshot undo.
    undo_stack: Vec<Vec<u8>>,
    /// The previous drag sample, so `extend` interpolates from it.
    last: Option<(i32, i32)>,
    /// Set on any paint edit; `sync_canvases_to_images` clears it after upload.
    dirty: bool,
}

/// How many undo snapshots the ring keeps (each is `w*h*4` bytes).
const UNDO_DEPTH: usize = 12;

impl PaintSurface {
    /// A blank surface filled with `bg`, mirroring `handle`.
    pub fn new(width: usize, height: usize, bg: [u8; 4], handle: Handle<Image>) -> Self {
        Self {
            width,
            height,
            pixels: bg
                .iter()
                .copied()
                .cycle()
                .take(width * height * 4)
                .collect(),
            bg,
            handle,
            canvas_entity: Entity::PLACEHOLDER,
            color: [20, 20, 24, 255], // ink: near-black
            radius: 4,
            tool: Tool::Brush,
            enabled: false,
            undo_stack: Vec::new(),
            last: None,
            dirty: true, // upload the blank sheet on frame 1
        }
    }

    /// Push the current buffer onto the undo ring BEFORE an edit (bounded to
    /// [`UNDO_DEPTH`] — the oldest snapshot is dropped when full).
    fn snapshot(&mut self) {
        if self.undo_stack.len() >= UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.pixels.clone());
    }

    /// Restore the most recent snapshot (the toolbar Undo). No-op when empty.
    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.pixels = prev;
            self.last = None;
            self.dirty = true;
        }
    }

    /// The color a stamp paints: the eraser paints the background (paper).
    fn effective_color(&self) -> [u8; 4] {
        match self.tool {
            Tool::Eraser => self.bg,
            _ => self.color,
        }
    }

    /// Begin a stroke: snapshot for undo, stamp at `(x, y)`, anchor interpolation.
    pub fn begin(&mut self, x: i32, y: i32) {
        self.snapshot();
        let color = self.effective_color();
        stamp_circle(
            &mut self.pixels,
            self.width,
            self.height,
            x,
            y,
            self.radius,
            color,
        );
        self.last = Some((x, y));
        self.dirty = true;
    }

    /// Extend the stroke to `(x, y)`, interpolating from the last sample.
    pub fn extend(&mut self, x: i32, y: i32) {
        let color = self.effective_color();
        match self.last {
            Some(last) => stroke_segment(
                &mut self.pixels,
                self.width,
                self.height,
                last,
                (x, y),
                self.radius,
                color,
            ),
            None => stamp_circle(
                &mut self.pixels,
                self.width,
                self.height,
                x,
                y,
                self.radius,
                color,
            ),
        }
        self.last = Some((x, y));
        self.dirty = true;
    }

    /// End the current stroke (drop the interpolation anchor).
    pub fn end(&mut self) {
        self.last = None;
    }

    /// Flood-fill the region under `(x, y)` with the current color.
    pub fn fill(&mut self, x: i32, y: i32) {
        self.snapshot();
        flood_fill(&mut self.pixels, self.width, self.height, x, y, self.color);
        self.dirty = true;
    }

    /// Handle a primary press per the active tool (bucket fills, others stroke).
    pub fn press(&mut self, x: i32, y: i32) {
        match self.tool {
            Tool::Bucket => self.fill(x, y),
            _ => self.begin(x, y),
        }
    }

    /// Reset to the blank background sheet.
    pub fn clear(&mut self) {
        self.snapshot();
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&self.bg);
        }
        self.dirty = true;
    }

    /// Drop the undo history (used on a fresh-turn / editor-open reset).
    pub fn clear_undo(&mut self) {
        self.undo_stack.clear();
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

/// Primary press → stroke start (or bucket fill); secondary press → bucket fill.
fn on_canvas_press(
    press: On<Pointer<Press>>,
    mut canvases: ResMut<PaintCanvases>,
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
    match press.event.button {
        PointerButton::Primary => surface.press(x, y),
        PointerButton::Secondary => surface.fill(x, y),
        _ => {}
    }
}

/// Primary drag → extend the stroke (line-interpolated from the last sample).
fn on_canvas_drag(
    drag: On<Pointer<Drag>>,
    mut canvases: ResMut<PaintCanvases>,
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
    if let Some((x, y)) = surface.to_pixel(drag.pointer_location.position, gt, layout) {
        surface.extend(x, y);
    }
}

/// Release → end the stroke.
fn on_canvas_release(
    release: On<Pointer<Release>>,
    mut canvases: ResMut<PaintCanvases>,
    kinds: Query<&CanvasKind>,
) {
    if let Some(kind) = kind_of(&kinds, release.entity) {
        canvases.surface_mut(kind).end();
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
            ((base as f32) * 1.6).round() as i32
        } else {
            base
        };
        let drawing = model.game.phase == crate::game::Phase::Drawing;
        g.enabled = drawing && model.game.viewer_is_drawer();
        if drawing && !*was_drawing {
            g.clear();
            g.clear_undo();
        }
        *was_drawing = drawing;
        if t.clear_seq != *last_clear {
            g.clear();
            *last_clear = t.clear_seq;
        }
        if t.undo_seq != *last_undo {
            g.undo();
            *last_undo = t.undo_seq;
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
            ((base as f32) * 1.6).round() as i32
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

/// Installs the drawing canvases: setup + the CPU→Image mirror + the model→canvas
/// tool sync. Kept as a distinct plugin (NOT folded into `dooduel::install`) so
/// the canvases are visibly decoupled from the MVU app — the coexistence story.
pub struct CanvasPlugin;

impl Plugin for CanvasPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CanvasKind>();
        app.add_systems(Startup, setup_canvases);
        app.add_systems(
            Update,
            (
                wire_canvas_node,
                sync_tools_to_canvases,
                sync_canvases_to_images,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(w: usize, h: usize) -> Vec<u8> {
        vec![0u8; w * h * 4]
    }

    #[test]
    fn stamp_circle_radius_zero_is_one_pixel() {
        let (w, h) = (5, 5);
        let mut px = blank(w, h);
        stamp_circle(&mut px, w, h, 2, 2, 0, [1, 2, 3, 4]);
        assert_eq!(get(&px, w, 2, 2), [1, 2, 3, 4]);
        // Neighbors untouched.
        assert_eq!(get(&px, w, 1, 2), [0, 0, 0, 0]);
        assert_eq!(get(&px, w, 2, 1), [0, 0, 0, 0]);
    }

    #[test]
    fn stamp_circle_clips_at_edges_without_panicking() {
        let (w, h) = (4, 4);
        let mut px = blank(w, h);
        // Center off the top-left corner — the clip must keep every write in-bounds.
        stamp_circle(&mut px, w, h, 0, 0, 3, [9, 9, 9, 9]);
        assert_eq!(get(&px, w, 0, 0), [9, 9, 9, 9]);
    }

    #[test]
    fn stroke_segment_leaves_no_gaps_on_a_fast_diagonal() {
        // A near-diagonal jump with radius 0: every stepped pixel on the line must
        // be painted (the line-interpolation gap guarantee).
        let (w, h) = (16, 16);
        let mut px = blank(w, h);
        stroke_segment(&mut px, w, h, (1, 1), (12, 9), 0, [7, 7, 7, 7]);
        // Endpoints painted.
        assert_eq!(get(&px, w, 1, 1), [7, 7, 7, 7]);
        assert_eq!(get(&px, w, 12, 9), [7, 7, 7, 7]);
        // No 2px gap: every column between the endpoints has at least one painted
        // pixel (dx=11 >= dy=8, so the walk steps once per column).
        for x in 1..=12usize {
            let any = (0..h).any(|y| get(&px, w, x, y) == [7, 7, 7, 7]);
            assert!(any, "column {x} has a gap");
        }
    }

    #[test]
    fn flood_fill_recolors_a_bounded_region_and_stops_at_a_wall() {
        // A 5x5 field split by a vertical wall at x=2; filling the left half must
        // not cross the wall.
        let (w, h) = (5, 5);
        let mut px = vec![0u8; w * h * 4]; // all target (0,0,0,0)
        for y in 0..h {
            set(&mut px, w, 2, y, [1, 1, 1, 1]); // the wall (non-target)
        }
        flood_fill(&mut px, w, h, 0, 0, [5, 5, 5, 5]);
        // Left of the wall: filled.
        assert_eq!(get(&px, w, 0, 0), [5, 5, 5, 5]);
        assert_eq!(get(&px, w, 1, 4), [5, 5, 5, 5]);
        // The wall: untouched.
        assert_eq!(get(&px, w, 2, 2), [1, 1, 1, 1]);
        // Right of the wall: NOT filled (still target).
        assert_eq!(get(&px, w, 3, 0), [0, 0, 0, 0]);
        assert_eq!(get(&px, w, 4, 4), [0, 0, 0, 0]);
    }

    #[test]
    fn flood_fill_same_color_is_a_noop() {
        let (w, h) = (3, 3);
        let mut px = vec![5u8; w * h * 4];
        // Seed color == new color: must return immediately, buffer unchanged.
        flood_fill(&mut px, w, h, 1, 1, [5, 5, 5, 5]);
        assert!(px.iter().all(|&b| b == 5));
    }

    #[test]
    fn flood_fill_out_of_bounds_seed_is_a_noop() {
        let (w, h) = (3, 3);
        let mut px = blank(w, h);
        flood_fill(&mut px, w, h, -1, 0, [9, 9, 9, 9]);
        flood_fill(&mut px, w, h, 3, 3, [9, 9, 9, 9]);
        assert!(px.iter().all(|&b| b == 0));
    }

    #[test]
    fn eraser_stamps_the_background_color() {
        let mut c = PaintSurface::new(4, 4, [255, 255, 255, 255], Handle::default());
        c.color = [10, 20, 30, 255];
        c.tool = Tool::Brush;
        c.begin(1, 1);
        assert_eq!(get(&c.pixels, 4, 1, 1), [10, 20, 30, 255]);
        c.tool = Tool::Eraser;
        c.begin(1, 1);
        assert_eq!(get(&c.pixels, 4, 1, 1), [255, 255, 255, 255]);
    }

    #[test]
    fn undo_restores_the_prior_buffer() {
        let mut c = PaintSurface::new(4, 4, [255, 255, 255, 255], Handle::default());
        c.color = [1, 2, 3, 255];
        c.begin(2, 2);
        assert_eq!(get(&c.pixels, 4, 2, 2), [1, 2, 3, 255]);
        c.undo();
        // Back to blank paper.
        assert_eq!(get(&c.pixels, 4, 2, 2), [255, 255, 255, 255]);
    }
}
