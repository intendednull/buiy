//! The pure decoration-emission mirror (decoration-and-paint § 3): walk
//! `LayoutRun.decorations` ourselves and emit f32 logical-px rects,
//! mirroring upstream `render_decoration`'s semantics 1:1
//! (cosmic-text 0.19.0 src/render.rs, source-verified) — with two
//! deliberate substitutions pinned by the § 3.1/§ 3.3 decisions:
//!
//! 1. No `Renderer::rectangle(i32, i32, u32, u32)` quantization — f32
//!    logical px end-to-end (fractional scale factors survive).
//! 2. The thickness floor is `max(1, round(t × scale))` WHOLE PHYSICAL px
//!    (converted back to logical), with y snapped to the same grid —
//!    upstream's `.max(1.0).ceil()` floors at 1 LOGICAL px, which is 1.5
//!    physical px at scale 1.5: the exact AA blur this rule prevents.
//!    x is never snapped (the subpixel discipline).
//!
//! Color precedence mirrors upstream exactly, per kind:
//! `td.<kind>_color_opt → span color → None` (the caller's `currentColor`
//! fallback). As-built tier 1 is structurally `None` (decision 1: the
//! `-color` property is a Buiy `ColorToken` resolved at extract, never an
//! `Attrs` color) and tier 2 is `None` until the rich-text tier sets
//! `Attrs.color_opt` — the mirror still implements all three tiers because
//! it mirrors UPSTREAM, and the drift guard pins upstream.
//!
//! Pure functions — no ECS, no GPU, no FontSystem: unit-testable with
//! hand-built `GlyphDecorationData` (every cosmic type here is plain-pub).

use bevy::math::Vec2;
use cosmic_text::{Color, GlyphDecorationData, LayoutGlyph, UnderlineStyle};
use smallvec::SmallVec;
use std::ops::Range;

/// Which decoration line a rect belongs to — the § 4.2 seat router:
/// `Underline`/`Overline` are quad-tier (`ExtractedTextQuads`),
/// `LineThrough` is a solid-stamp glyph-tier instance emitted after the
/// run's glyphs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecorationKind {
    Underline,
    Overline,
    LineThrough,
}

/// One emitted decoration rect, in WORLD logical px (the `origin` fold is
/// done here, before the § 3.3 y-snap, so the snap lands on the real
/// physical grid — the same reason `physical()` folds the origin before
/// binning, glyph-pipeline § 5.1).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DecorationRect {
    pub kind: DecorationKind,
    /// `[x, y, w, h]`, logical px, window space.
    pub rect: [f32; 4],
    /// Upstream tiers 1–2 (`td.*_color_opt` else the span text color);
    /// `None` = the caller applies `currentColor` (the resolved entity
    /// foreground), after the Buiy `TextDecorations.color` token override.
    pub color_opt: Option<Color>,
}

/// § 3.3: thickness floored to whole physical px, minimum one, expressed
/// in logical px.
pub fn snap_thickness(thickness_logical: f32, scale_factor: f32) -> f32 {
    (thickness_logical * scale_factor).round().max(1.0) / scale_factor
}

/// § 3.3: y snapped to the physical pixel grid, expressed in logical px.
pub fn snap_y(y_logical: f32, scale_factor: f32) -> f32 {
    (y_logical * scale_factor).round() / scale_factor
}

/// The span's horizontal extent `(x_start, width)` in run-local logical px:
/// min/max over the span's glyphs — NOT first/last, because RTL paragraphs
/// store glyphs in right-to-left order (upstream's own comment, mirrored).
/// `None` when the range is empty/out-of-bounds or the width is ≤ 0
/// (upstream's early-outs).
pub fn span_x_extent(glyphs: &[LayoutGlyph], range: &Range<usize>) -> Option<(f32, f32)> {
    let span_glyphs = glyphs.get(range.clone())?;
    if span_glyphs.is_empty() {
        return None;
    }
    let mut x_min = f32::INFINITY;
    let mut x_max = f32::NEG_INFINITY;
    for g in span_glyphs {
        x_min = x_min.min(g.x);
        x_max = x_max.max(g.x + g.w);
    }
    let width = x_max - x_min;
    (width > 0.0).then_some((x_min, width))
}

/// Emit one span's decoration rects (§ 3.2's table, exactly upstream's
/// placement with the § 3.3 substitutions — module doc):
///
/// | line | y (pre-snap, world) | thickness (pre-floor) |
/// |---|---|---|
/// | underline | `origin.y + line_y − underline.offset × fs` | `underline.thickness × fs` |
/// | underline Double | + a second rect at `y + 2 × t` (gap = t) | same |
/// | line-through | `origin.y + line_y − strikeout.offset × fs` | `strikeout.thickness × fs` |
/// | overline | `origin.y + max(line_y − ascent × fs, line_top)` | underline's |
///
/// `line_y`/`line_top` are run-local (`LayoutRun` fields); `x_start`/`width`
/// from [`span_x_extent`]. ≤ 4 rects (Double underline + strike + over).
#[allow(clippy::too_many_arguments)]
pub fn span_decoration_rects(
    origin: Vec2,
    line_y: f32,
    line_top: f32,
    x_start: f32,
    width: f32,
    data: &GlyphDecorationData,
    font_size: f32,
    span_color_opt: Option<Color>,
    scale_factor: f32,
) -> SmallVec<[DecorationRect; 4]> {
    let mut out = SmallVec::new();
    if width <= 0.0 {
        return out;
    }
    let td = &data.text_decoration;
    let x = origin.x + x_start;

    // Underline (Single | Double) — and the thickness overline reuses.
    let underline_t = snap_thickness(data.underline_metrics.thickness * font_size, scale_factor);
    if td.underline != UnderlineStyle::None {
        let color_opt = td.underline_color_opt.or(span_color_opt);
        let y = snap_y(
            origin.y + line_y - data.underline_metrics.offset * font_size,
            scale_factor,
        );
        out.push(DecorationRect {
            kind: DecorationKind::Underline,
            rect: [x, y, width, underline_t],
            color_opt,
        });
        if td.underline == UnderlineStyle::Double {
            // gap = thickness; t is physically integral and y grid-snapped,
            // so the second rect is grid-aligned by construction.
            out.push(DecorationRect {
                kind: DecorationKind::Underline,
                rect: [x, y + 2.0 * underline_t, width, underline_t],
                color_opt,
            });
        }
    }

    // Line-through (its own font table).
    if td.strikethrough {
        let t = snap_thickness(
            data.strikethrough_metrics.thickness * font_size,
            scale_factor,
        );
        let y = snap_y(
            origin.y + line_y - data.strikethrough_metrics.offset * font_size,
            scale_factor,
        );
        out.push(DecorationRect {
            kind: DecorationKind::LineThrough,
            rect: [x, y, width, t],
            color_opt: td.strikethrough_color_opt.or(span_color_opt),
        });
    }

    // Overline — clamped to the line box (run-local, BEFORE the origin
    // fold, mirroring upstream's `(line_y − ascent×fs).max(line_top)`).
    if td.overline {
        let y_local = (line_y - data.ascent * font_size).max(line_top);
        let y = snap_y(origin.y + y_local, scale_factor);
        out.push(DecorationRect {
            kind: DecorationKind::Overline,
            rect: [x, y, width, underline_t],
            color_opt: td.overline_color_opt.or(span_color_opt),
        });
    }
    out
}
