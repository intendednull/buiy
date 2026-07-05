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
use bevy::prelude::{Added, Entity, GlobalTransform, On, Query, Reflect};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use buiy::prelude::*;
use buiy_core::render::{Border, Corners, Radius, RasterImage};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

/// The pure pixel math + palette + [`PaintBuffer`] moved to `dooduel_core::canvas`
/// (M1 W0.3); these re-exports keep `paint::PALETTE` / `paint::Tool` / `paint::PAPER`
/// / `paint::BRUSH_SIZES` paths (view modules, bins) stable after the extraction.
pub use dooduel_core::canvas::{BRUSH_SIZES, CANVAS_H, CANVAS_W, PALETTE, PAPER, Tool};
use dooduel_core::canvas::{PaintBuffer, eraser_radius};

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
            eraser_radius(base)
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
