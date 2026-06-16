//! E6 — auto-scroll-into-view (editing-and-ime § 9). The editor's viewport
//! pans via the layout `ScrollOffset` (x single-line / y multi-line); the
//! Buffer is laid out at full content size and never scrolls internally —
//! `ScrollOffset` deliberately does NOT invalidate Taffy
//! (`layout/components.rs:509-526` + its invariant test). After each caret
//! move / edit, `auto_scroll_caret` clamps the caret rect into the node's
//! content-box viewport with a small margin.
//!
//! `clamp_into_view` is a PURE function (the headless unit) — it takes the
//! current offset, the viewport extent on one axis, the caret's leading and
//! trailing coordinates on that axis, and the margin, and returns the new
//! offset. The system reads geometry (`CaretVisual` + `ResolvedLayout`) and
//! applies it on the right axis per the `SingleLine` marker.
//!
//! It names NO cosmic type (the caret rect comes from the E3 `CaretVisual`
//! seat, pure Bevy `Rect`), so it is free of the facade boundary, but it lives
//! in `text::edit` for cohesion.

use bevy::prelude::*;

use super::state::{Disabled, SingleLine, TextEditState};
use crate::components::ResolvedLayout;
use crate::layout::ScrollOffset;
use crate::text::CaretVisual;

/// The keep-off-the-edge margin in logical px. Generous enough to also absorb
/// the small border/padding inset between the border box (`ResolvedLayout.size`)
/// and the content box for v1 (a precise content-box extent via a `Length`→px
/// resolution of `BoxModel.border`/`.padding` is a trivial follow-up).
const SCROLL_MARGIN: f32 = 6.0;

/// Clamp the caret into the viewport window `[offset, offset + extent]`,
/// returning the new offset. The caret is the insertion bar at `caret_lead`
/// (its leading edge), `caret_size` wide; `margin` is the symmetric keep-off
/// distance around the insertion point.
///
/// - If the leading insertion point (+margin) exceeds the window's far edge,
///   pan forward so it is just margin-inside.
/// - Else if the leading point (−margin) is before the window's near edge, pan
///   back so it is just margin-inside.
/// - Then a wide-caret guard: never hide the caret's own trailing edge
///   (`caret_lead + caret_size`) — for the thin insertion bar this is a no-op,
///   but a degenerate wide caret stays fully visible.
///
/// The offset is finally clamped to `>= 0` (content never pans past its start).
/// Auto-scroll reveals the *insertion point* with margin (the standard editor
/// behavior), so the symmetric `caret_lead ± margin` window is the contract; the
/// trailing edge participates only through the wide-caret guard.
pub fn clamp_into_view(
    offset: f32,
    extent: f32,
    caret_lead: f32,
    caret_size: f32,
    margin: f32,
) -> f32 {
    let far = offset + extent;
    let mut new = offset;

    // Forward: keep the leading insertion point margin-inside the far edge.
    if caret_lead + margin > far {
        new = caret_lead + margin - extent;
    }
    // Backward: keep the leading insertion point margin-inside the near edge.
    if caret_lead - margin < new {
        new = caret_lead - margin;
    }
    // Wide-caret guard: never push the caret's trailing edge off the far side
    // (a no-op for the thin insertion bar, where caret_size ≈ 1px).
    let caret_trail = caret_lead + caret_size;
    if caret_trail > new + extent {
        new = caret_trail - extent;
    }
    new.max(0.0)
}

/// Render-prep: pan the focused editor's `ScrollOffset` so the caret stays in
/// view (§ 9). Runs `.after(write_caret_and_selection)` so it reads the caret
/// rect that writer just published; the `ScrollOffset` it writes is consumed
/// by the transform bridge later this frame (`seed_scroll_dirty`,
/// `.after(BuiySet::Animate)`).
///
/// Single-line ⇒ pan x; multi-line ⇒ pan y. The viewport extent is the node's
/// border-box size (`ResolvedLayout.size`); `SCROLL_MARGIN` absorbs the small
/// content inset for v1. Option params / `Without<Disabled>` follow the
/// editor-system discipline.
#[allow(clippy::type_complexity)]
pub fn auto_scroll_caret(
    focused: Option<Res<crate::FocusedEntity>>,
    mut editors: Query<
        (
            &CaretVisual,
            Has<SingleLine>,
            &ResolvedLayout,
            &mut ScrollOffset,
        ),
        (With<TextEditState>, Without<Disabled>),
    >,
) {
    let Some(focused) = focused else { return };
    let Some(entity) = focused.0 else { return };
    let Ok((caret, single_line, layout, mut offset)) = editors.get_mut(entity) else {
        return;
    };

    if single_line {
        let extent = layout.size.x;
        if extent <= 0.0 {
            return; // not laid out yet
        }
        let new_x = clamp_into_view(
            offset.x,
            extent,
            caret.rect.min.x,
            caret.rect.width(),
            SCROLL_MARGIN,
        );
        if offset.x != new_x {
            offset.x = new_x;
        }
        // Single-line never pans y.
        if offset.y != 0.0 {
            offset.y = 0.0;
        }
    } else {
        let extent = layout.size.y;
        if extent <= 0.0 {
            return;
        }
        let new_y = clamp_into_view(
            offset.y,
            extent,
            caret.rect.min.y,
            caret.rect.height(),
            SCROLL_MARGIN,
        );
        if offset.y != new_y {
            offset.y = new_y;
        }
    }
}
