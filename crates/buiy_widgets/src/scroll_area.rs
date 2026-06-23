//! ScrollArea — the scroll-container widget (C5-a, scroll-overlay-modal.md §A.4).
//!
//! Unlike the P1d a11y widgets (Checkbox/Switch/Slider/…), `ScrollArea` is a
//! **container**, not an APG control: it carries no toggle/value state and no
//! activation contract. Its `#[require(...)]` assembles the scroll substrate —
//! a scrollable [`Overflow`], the runtime [`ScrollOffset`], the cached
//! [`ScrollExtent`], the SC-4 [`A11yScroll`] source the a11y fold projects, and
//! `Focusable` so the **container** owns keyboard scroll (the bevy-ui-widgets
//! lesson: the container owns focus + keyboard; a scrollbar, when one exists, is
//! a pointer-only affordance — C-tier, deferred).
//!
//! The a11y role is [`A11yRole::Group`] (a scroll region; the a11y presence is on
//! the container). The wheel/keyboard handlers + the `A11yScroll` sync all live
//! in `buiy_core::scroll`; this widget only assembles the bundle so a bare
//! `ScrollArea` (or the [`scroll_area`](crate::scene::scroll_area) scene-fn) is a
//! ready scroll container.

use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yRole, A11yScroll},
    components::Node,
    focus::Focusable,
    layout::{Overflow, OverflowMode, ScrollOffset, Style},
    scroll::ScrollExtent,
};

/// ScrollArea widget marker — a scroll container. The `#[require(...)]` is the
/// single source of the scroll-container shape:
///
/// - `Node` — the layout marker (transitively `#[require]`s the `Style`
///   decomposition), so the area is layout-visible without re-spelling it.
/// - `Overflow = scroll_area_overflow()` — `{ y: Auto }` by default (vertical
///   scroll-when-overflowing); a direct `#[require]` initializer wins over
///   `Node`'s transitive `Overflow` default. The axes are configurable by
///   patching `Overflow` (e.g. via the scene-fn).
/// - `ScrollOffset` — the runtime scroll position the wheel/keyboard handlers
///   write (clamped); the bridge folds it into `GlobalTransform`.
/// - `ScrollExtent` — the cached content/viewport extent the clamp reads;
///   `update_scroll_extent` keeps it current after each layout pass.
/// - `Focusable` — the container owns keyboard scroll (Arrow/Page/Home/End).
/// - `A11yRole = A11yRole::Group` — the scroll-region role; a11y on the container.
/// - `A11yScroll` — the SC-4 a11y scroll source; `update_a11y_scroll` populates
///   it from `ScrollOffset` + `ScrollExtent`, and `build_tree` projects it into
///   `A11yNodeView.scroll`.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    Overflow = scroll_area_overflow(),
    ScrollOffset,
    ScrollExtent,
    Focusable,
    A11yRole = A11yRole::Group,
    A11yScroll,
)]
pub struct ScrollArea;

/// The canonical scroll-area overflow: vertical `Auto` (a scrollbar/scroll
/// viewport only when content overflows), horizontal `Visible`. `Auto` makes the
/// element a scroll container (`Overflow::is_scroll_container()`), which is what
/// the wheel/keyboard handlers gate on. `pub(crate)` so the `scene` module's
/// `scroll_area()` scene-fn spells the SAME default.
pub(crate) fn scroll_area_overflow() -> Overflow {
    Style::default().overflow_y(OverflowMode::Auto).overflow
}
