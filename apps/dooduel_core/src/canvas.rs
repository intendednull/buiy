//! The Bevy-free drawing surface — pure integer pixel math + a CPU pixel buffer.
//!
//! Extracted from `apps/dooduel/src/paint.rs` (M1 W0.3) so the authoritative
//! op-log canvas (spec §2.2) and the MCP `get_canvas` rasterizer can replay
//! `CanvasOp`s without any Bevy coupling. The stamp/stroke/flood-fill ops are
//! deterministic integer operations, so identical op sequences produce
//! byte-identical pixels on every replica (the property the op-log sync stands on).
//!
//! `apps/dooduel/src/paint.rs`'s `PaintSurface` becomes a thin Bevy wrapper (an
//! `Image` mirror + pointer observers) around [`PaintBuffer`] here.

use bevy_reflect::Reflect;

/// The white paper the canvas starts on (skribbl.io's blank sheet).
pub const PAPER: [u8; 4] = [255, 255, 255, 255];

/// The design's 16-color toolbar palette (`PALETTE`, in order), as exact sRGB.
/// The model's `ToolState.color_idx` (in-game) / `AvatarState.draft_color_idx`
/// (editor) indexes this; the view renders it as swatches, the sync maps the
/// selected index to the buffer's `color`.
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

/// The in-game canvas width in pixels (the shared authority + GUI dimension). Moved
/// here from `apps/dooduel/src/paint.rs` (W2-review I3) so the [`crate::session::Session`]
/// can bound-check incoming stroke/fill coordinates against the same size the GUI
/// paints; `dooduel::paint` re-exports it so view/paint code is unchanged.
pub const CANVAS_W: usize = 720;
/// The in-game canvas height in pixels — the companion to [`CANVAS_W`].
pub const CANVAS_H: usize = 450;

/// The active tool. `Bucket` flood-fills on press; `Brush`/`Eraser` stroke. Held
/// on the MVU model (`ToolState`) so tool selection is reducer-owned + replayable,
/// and mirrored onto a [`PaintBuffer`] each frame by the GUI's canvas sync.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum Tool {
    #[default]
    Brush,
    Eraser,
    Bucket,
}

/// The eraser's stamp radius for a given base brush radius: the ×1.6 rule
/// (`paint.rs`, spec §3.5), rounded. Extracted as a helper so the wire encoder can
/// pre-apply it — an eraser `CanvasOp` carries the [`PAPER`] color + this radius,
/// so replicas need no tool-specific knowledge (a `Stroke` op's color + radius
/// fully determine the stamp).
pub fn eraser_radius(base: i32) -> i32 {
    ((base.max(0) as f32) * 1.6).round() as i32
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
// The Bevy-free pixel surface — the CPU pixel state + brush + the undo ring.
// ---------------------------------------------------------------------------

/// How many undo snapshots the ring keeps (each is `w*h*4` bytes).
const UNDO_DEPTH: usize = 12;

/// The Bevy-free pixel surface: an RGBA8 buffer + brush state + the undo ring.
/// `PaintSurface` (`apps/dooduel/src/paint.rs`) becomes a thin Bevy wrapper
/// (`Image` mirror + pointer observers) around this; the headless server + MCP
/// clients drive it directly to rasterize the op log.
pub struct PaintBuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    /// The background (paper) color: the eraser stamp + `clear` paint this.
    pub bg: [u8; 4],
    pub color: [u8; 4],
    pub radius: i32,
    pub tool: Tool,
    /// Set on any edit. The Bevy wrapper (`PaintSurface`) clears it after mirroring
    /// the buffer into its `Image` asset; headless rasterizer users ignore it.
    pub dirty: bool,
    /// The previous drag sample, so `extend` interpolates from it.
    last: Option<(i32, i32)>,
    /// Undo snapshots (full-buffer copies, newest last), pushed BEFORE each edit.
    /// Capped at [`UNDO_DEPTH`] — the design's snapshot undo.
    undo_stack: Vec<Vec<u8>>,
}

impl PaintBuffer {
    /// A blank surface filled with `bg`.
    pub fn new(width: usize, height: usize, bg: [u8; 4]) -> Self {
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
            color: [20, 20, 24, 255], // ink: near-black
            radius: 4,
            tool: Tool::Brush,
            dirty: true, // mirror the blank sheet on frame 1
            last: None,
            undo_stack: Vec::new(),
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

    /// Restore the most recent snapshot (the toolbar Undo). Returns whether a
    /// snapshot was popped (`false` ⇒ nothing to undo — a no-op).
    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.pixels = prev;
            self.last = None;
            self.dirty = true;
            true
        } else {
            false
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
        let mut c = PaintBuffer::new(4, 4, [255, 255, 255, 255]);
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
        let mut c = PaintBuffer::new(4, 4, [255, 255, 255, 255]);
        c.color = [1, 2, 3, 255];
        c.begin(2, 2);
        assert_eq!(get(&c.pixels, 4, 2, 2), [1, 2, 3, 255]);
        c.undo();
        // Back to blank paper.
        assert_eq!(get(&c.pixels, 4, 2, 2), [255, 255, 255, 255]);
    }
}
