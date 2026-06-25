//! Scroll input — the producer that writes [`ScrollOffset`].
//!
//! This realizes the layout spec's deferred "scroll handler" (named
//! `buiy-input-events-design` in `overflow-and-scrolling.md §2`; landed here per
//! `docs/specs/2026-06-22-buiy-widget-catalog-design/scroll-overlay-modal.md`
//! Slice A). The whole downstream chain — the bridge folding `ScrollOffset` into
//! `GlobalTransform`, clip, render paint-skip — is already built and idle; this
//! module is the one upstream system that *writes* `ScrollOffset`, clamped.
//!
//! # What C5-a builds (this slice)
//!
//! - [`ScrollExtent`] — the cached content/viewport extent of a scroll
//!   container, recomputed from a layout-change reaction (NOT per wheel tick).
//! - [`on_scroll`] — a global observer on `Pointer<Scroll>` (C3's wheel entry,
//!   re-exported as `buiy::events::Scroll`) that routes the wheel delta to the
//!   nearest scroll-container ancestor it bubbles to, and updates that
//!   container's `ScrollOffset` clamped to `[0, content − viewport]` per axis,
//!   honoring [`OverscrollBehavior`] (a
//!   `Contain`/`None` axis at a bound stops the event bubbling — no chaining).
//! - [`keyboard_scroll`] — a focused [`ScrollOffset`]-bearing scroll container
//!   scrolls on Arrow (line) / PageUp/PageDown (page) / Home/End (top/bottom),
//!   through the same clamp.
//! - [`update_a11y_scroll`] — keeps the SC-4 [`A11yScroll`]
//!   source in lock-step with `ScrollOffset` + `ScrollExtent`, so `build_tree`
//!   projects `A11yNodeView.scroll` and the AT/driver/snapshot sees the live
//!   scroll offset + extent + scrollable flag.
//!
//! # Invariant — mutating `ScrollOffset` must NOT invalidate `ResolvedLayout`
//!
//! Every write here touches `ScrollOffset` (and the derived `A11yScroll`) only —
//! never `Style`/`BoxModel`/layout inputs — so the `components.rs:516` invariant
//! (asserted by `tests/layout/layout_scroll_offset_no_invalidate.rs`) holds. The
//! extent cache is updated by a *layout-change* reaction
//! ([`update_scroll_extent`]), keeping it off the scroll write-path entirely.
//!
//! # Deferred to later C5 slices
//!
//! - **Smooth `scroll_to`** (§A.5, `BuiySet::Animate` tween + reduced-motion
//!   gate) — discrete wheel/keyboard scroll lands here; the animated programmatic
//!   path is a follow-up.
//! - **Snap-point math** (`ScrollSnapItem`), **scrollbar widget** (C-tier),
//!   **recycling virtualization** (§A.6 — v1 leans on the built
//!   `ContentVisibility::Auto`).

use crate::BuiySet;
use crate::a11y::A11yScroll;
use crate::components::ResolvedLayout;
use crate::focus::FocusedEntity;
use crate::layout::{Overflow, OverscrollBehavior, ScrollOffset};
use bevy::input::keyboard::KeyCode;
use bevy::picking::events::{Pointer, Scroll};
use bevy::prelude::*;

/// The wheel `MouseScrollUnit::Line` → px factor. A line is ~16px and a wheel
/// notch advances ~3 lines (the CSS `WHEEL_DELTA` convention), so one notch
/// scrolls ~48px. Trackpad `Pixel` deltas pass through unscaled.
pub const LINE_HEIGHT_PX: f32 = 16.0;
/// Lines per discrete wheel notch (the `k≈3` scroll-step factor, §A.1).
pub const WHEEL_LINES_PER_NOTCH: f32 = 3.0;
/// One keyboard arrow step in logical px (one "line").
pub const KEY_LINE_STEP_PX: f32 = LINE_HEIGHT_PX * WHEEL_LINES_PER_NOTCH;
/// The overlap (logical px) kept between PageUp/PageDown jumps so a partial line
/// of context carries over — one viewport *minus* this overlap per page.
pub const PAGE_OVERLAP_PX: f32 = 40.0;

/// Render-prep cache of a scroll container's content + viewport extent (logical
/// px). Decouples the clamp from an O(children) per-wheel-tick union and keeps
/// the no-invalidate-on-scroll invariant: it is written by the layout-change
/// reaction [`update_scroll_extent`], never by the scroll handlers.
///
/// Spec: scroll-overlay-modal.md §A.3.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct ScrollExtent {
    /// Total content size (the union of child boxes, relative to the container
    /// origin), logical px.
    pub content: Vec2,
    /// Visible viewport size (the container's `ResolvedLayout.size`), logical px.
    pub viewport: Vec2,
    /// `false` on `Default` (spawn frame, before the first layout pass); set
    /// `true` the first time [`update_scroll_extent`] runs with a resolved
    /// layout. While `false`, [`clamp_to_extent`] treats the upper bound as
    /// **unknown** and clamps only to `0` — so a wheel tick in the spawn frame
    /// accumulates rather than being pinned to a not-yet-known zero extent
    /// (§A.3, the unknown-extent sentinel rule).
    pub valid: bool,
}

impl ScrollExtent {
    /// The per-axis maximum scroll offset: `max(0, content − viewport)`. For an
    /// axis whose content fits the viewport this is `0` (no scroll room).
    pub fn max_offset(&self) -> Vec2 {
        (self.content - self.viewport).max(Vec2::ZERO)
    }

    /// `true` iff the content exceeds the viewport on either axis (there is room
    /// to scroll). Mirrors the SC-4 `scrollable` flag.
    pub fn scrollable(&self) -> bool {
        let max = self.max_offset();
        max.x > 0.0 || max.y > 0.0
    }
}

/// Normalize a wheel delta to logical px. `Line` deltas multiply by the
/// per-notch line factor; `Pixel` (trackpad) deltas pass through. Returned as a
/// `(x, y)` px delta to **add** to the current offset.
///
/// `Pointer<Scroll>` carries the scroll *amounts* with the sign convention
/// "positive y = content moves up under the pointer" (a wheel-down). Buiy's
/// `ScrollOffset` increases as content scrolls up (the viewport moves down the
/// content), so a positive wheel-down `y` adds to `ScrollOffset.y` directly —
/// the deltas are added as-is.
pub fn normalize_delta(unit: bevy::input::mouse::MouseScrollUnit, x: f32, y: f32) -> Vec2 {
    use bevy::input::mouse::MouseScrollUnit;
    match unit {
        // One line-delta is one notch worth of px (`LINE_HEIGHT_PX *
        // WHEEL_LINES_PER_NOTCH`), matching the CSS `WHEEL_DELTA` convention.
        MouseScrollUnit::Line => Vec2::new(x, y) * (LINE_HEIGHT_PX * WHEEL_LINES_PER_NOTCH),
        // Pixel deltas pass through unscaled (trackpad).
        MouseScrollUnit::Pixel => Vec2::new(x, y),
    }
}

/// Clamp a candidate offset to the valid scroll range for `extent`.
///
/// Each axis clamps to `[0, max(0, content − viewport)]` once the extent is
/// valid. While `extent.valid == false` (the spawn frame before the first layout
/// pass) the upper bound is **unknown**, so only the lower bound is applied
/// (`offset.max(0)` per axis) — the wheel tick accumulates and is re-clamped
/// correctly next frame once the real extent lands (§A.3, unknown-extent rule).
pub fn clamp_to_extent(offset: Vec2, extent: &ScrollExtent) -> Vec2 {
    if extent.valid {
        offset.clamp(Vec2::ZERO, extent.max_offset())
    } else {
        offset.max(Vec2::ZERO)
    }
}

/// Whether a finished scroll on `axis_overscroll` should **consume** the event
/// (stop it bubbling to an outer scroll container — no scroll chaining).
///
/// A `Contain`/`None` axis that was already at a bound and is being pushed
/// further past it (the new offset equals the old, i.e. the delta was fully
/// absorbed by the clamp at that bound) consumes the event. An `Auto` axis lets
/// the residual bubble. The check is per-axis; the event is consumed iff *every*
/// axis with a non-zero delta is contained at its bound.
fn axis_blocked_at_bound(
    overscroll: OverscrollBehavior,
    old: f32,
    new: f32,
    delta: f32,
) -> Option<bool> {
    if delta == 0.0 {
        // No request on this axis — it neither consumes nor lets through.
        return None;
    }
    let absorbed = (new - old).abs() < f32::EPSILON; // the clamp ate the whole delta
    match overscroll {
        OverscrollBehavior::Auto => Some(false), // residual bubbles to an outer container
        OverscrollBehavior::Contain | OverscrollBehavior::None => Some(absorbed),
    }
}

/// Whether the scroll event should stop propagating (be consumed) given the
/// per-axis overscroll behavior and the clamp result. Consumes iff at least one
/// requested axis is contained-at-its-bound and no requested axis still had room
/// (so the residual would otherwise chain to an outer container).
fn should_consume(overflow: &Overflow, old: Vec2, new: Vec2, delta: Vec2) -> bool {
    let x = axis_blocked_at_bound(overflow.overscroll_x, old.x, new.x, delta.x);
    let y = axis_blocked_at_bound(overflow.overscroll_y, old.y, new.y, delta.y);
    // Consume only when every requested axis is blocked-at-bound under a
    // Contain/None policy; if any requested axis still moved (or is Auto), the
    // residual is allowed to bubble.
    let requested = [x, y].into_iter().flatten().collect::<Vec<_>>();
    !requested.is_empty() && requested.iter().all(|&blocked| blocked)
}

/// Global observer on `Pointer<Scroll>` (C3's wheel entry). Because `Pointer<E>`
/// auto-propagates capture→target→bubble along `ChildOf`, this fires for each
/// entity in the bubble chain; it acts only when the *current* bubbling entity
/// (`ev.entity`, the `EntityEvent::event_target`) is a scroll container — so the
/// nearest scroll-container ancestor is reached for free, no manual walk (§A.1).
///
/// Updates that container's `ScrollOffset` by the normalized wheel delta,
/// clamped to the cached [`ScrollExtent`]. A `Contain`/`None` axis already at a
/// bound consumes the event (`propagate(false)`), killing scroll-chaining; an
/// `Auto` axis lets the residual bubble to an outer container.
pub fn on_scroll(
    mut ev: On<Pointer<Scroll>>,
    mut q: Query<(&Overflow, &mut ScrollOffset, &ScrollExtent)>,
) {
    let target = ev.entity;
    let Ok((overflow, mut offset, extent)) = q.get_mut(target) else {
        // Not a scroll container — let the event keep bubbling to an ancestor.
        return;
    };
    if !overflow.is_scroll_container() {
        return;
    }
    let delta = normalize_delta(ev.event.unit, ev.event.x, ev.event.y);
    let old = Vec2::new(offset.x, offset.y);
    let new = clamp_to_extent(old + delta, extent);
    if should_consume(overflow, old, new, delta) {
        ev.propagate(false);
    }
    offset.x = new.x;
    offset.y = new.y;
}

/// Keyboard scroll for a focused scroll container (§A.2). Arrow keys scroll one
/// line, PageUp/PageDown one viewport (minus a small overlap), Home/End to the
/// top/bottom — all through the same [`clamp_to_extent`]. Gated on the focused
/// entity being a scroll container, so it does not steal keys from a focused
/// child editor or button.
///
/// Runs in `BuiySet::Input`. Only the focused scroll container reacts; the
/// global focus model decides which entity that is.
pub fn keyboard_scroll(
    keys: Res<ButtonInput<KeyCode>>,
    focused: Res<FocusedEntity>,
    mut q: Query<(&Overflow, &mut ScrollOffset, &ScrollExtent)>,
) {
    let Some(target) = focused.0 else {
        return;
    };
    let Ok((overflow, mut offset, extent)) = q.get_mut(target) else {
        return;
    };
    if !overflow.is_scroll_container() {
        return;
    }

    let old = Vec2::new(offset.x, offset.y);
    let max = extent.max_offset();
    let page_y = (extent.viewport.y - PAGE_OVERLAP_PX).max(KEY_LINE_STEP_PX);
    let page_x = (extent.viewport.x - PAGE_OVERLAP_PX).max(KEY_LINE_STEP_PX);

    let mut new = old;
    if keys.just_pressed(KeyCode::ArrowDown) {
        new.y += KEY_LINE_STEP_PX;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        new.y -= KEY_LINE_STEP_PX;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        new.x += KEY_LINE_STEP_PX;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        new.x -= KEY_LINE_STEP_PX;
    }
    if keys.just_pressed(KeyCode::PageDown) {
        new.y += page_y;
    }
    if keys.just_pressed(KeyCode::PageUp) {
        new.y -= page_y;
    }
    if keys.just_pressed(KeyCode::Home) {
        // Home → top of the primary (vertical) axis.
        new.y = 0.0;
    }
    if keys.just_pressed(KeyCode::End) {
        // End → bottom of the primary (vertical) axis.
        new.y = max.y;
    }

    if new == old {
        return; // no scroll key this frame — leave the offset (and its change
        // detection) untouched.
    }
    let clamped = clamp_to_extent(new, extent);
    // Mutate through `DerefMut` only when the value actually changes, so a
    // clamped no-op (already at a bound) does not spuriously trip change
    // detection / `seed_scroll_dirty`.
    if clamped.x != offset.x {
        offset.x = clamped.x;
    }
    if clamped.y != offset.y {
        offset.y = clamped.y;
    }
    let _ = page_x; // horizontal paging has no key today (PageUp/Down are vertical).
}

/// Layout-change reaction that refreshes [`ScrollExtent`] for every scroll
/// container (§A.3). Content extent = the union of the container's direct
/// children's `ResolvedLayout` boxes (each `position + size`, parent-relative to
/// the container origin); viewport = the container's own `ResolvedLayout.size`.
///
/// Scheduled `.after(BuiySet::Layout).before(BuiySet::Input)` so within any
/// frame where layout resolved, the extent is current **before** [`on_scroll`] /
/// [`keyboard_scroll`] consume a scroll. Writes only when the value differs, so
/// it does not spuriously trip change detection in steady state. This is the
/// derived-cache-before-consumer ordering (mirrors C1's
/// `write_clip_rects.after(...).before(Picking)`).
pub fn update_scroll_extent(
    mut containers: Query<(
        &Overflow,
        &ResolvedLayout,
        Option<&Children>,
        &mut ScrollExtent,
    )>,
    child_layouts: Query<&ResolvedLayout>,
) {
    for (overflow, layout, children, mut extent) in containers.iter_mut() {
        if !overflow.is_scroll_container() {
            continue;
        }
        // Content = union of child boxes relative to the container origin. With
        // no children, the content collapses to the viewport (no scroll room).
        let mut content = layout.size;
        if let Some(children) = children {
            for child in children.iter() {
                if let Ok(cl) = child_layouts.get(child) {
                    content = content.max(cl.position + cl.size);
                }
            }
        }
        let next = ScrollExtent {
            content,
            viewport: layout.size,
            valid: true,
        };
        if *extent != next {
            *extent = next;
        }
    }
}

/// Populate the SC-4 [`A11yScroll`] source on each
/// scroll container from its live `ScrollOffset` + `ScrollExtent`, so the
/// outbound a11y fold (`build_tree`, in `BuiySet::A11yUpdate`) projects
/// `A11yNodeView.scroll` and the AT/driver/snapshot sees the offset + extent +
/// scrollable flag.
///
/// Runs in `BuiySet::Animate` (the `CorePlugin` set-chain orders it strictly
/// before `A11yUpdate`), so an offset mutated this frame (wheel or keyboard, in
/// `BuiySet::Input`) is mirrored into `A11yScroll` and folded into the a11y tree
/// the SAME frame — the precedent the `TextInput` `sync_text_input_a11y` set.
/// Writes only on a change (the `PartialEq` gate), so a steady-state frame is a
/// no-op for change detection.
pub fn update_a11y_scroll(mut q: Query<(&ScrollOffset, &ScrollExtent, &mut A11yScroll)>) {
    for (offset, extent, mut a11y) in q.iter_mut() {
        let next = A11yScroll {
            offset: Vec2::new(offset.x, offset.y),
            content_extent: extent.content,
            viewport_extent: extent.viewport,
            scrollable: extent.scrollable(),
        };
        if *a11y != next {
            *a11y = next;
        }
    }
}

/// Registers the C5-a scroll input pipeline: the `Pointer<Scroll>` observer, the
/// keyboard-scroll handler, the layout-change extent cache, and the SC-4
/// `A11yScroll` sync. Composed into the meta-crate `BuiyPlugin`; the headless
/// `PointerHarness` adds it explicitly to exercise scroll.
///
/// Spec: scroll-overlay-modal.md §A.1 (the `ScrollInputPlugin` canonical home).
pub struct ScrollInputPlugin;

impl Plugin for ScrollInputPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ScrollExtent>();
        // The extent cache: refreshed after layout, before the scroll consumers,
        // so the clamp reads a current extent the same frame layout resolved.
        app.add_systems(
            Update,
            update_scroll_extent
                .after(BuiySet::Layout)
                .before(BuiySet::Input),
        );
        // Keyboard scroll runs in the input stage alongside the other keyboard
        // handlers; it gates on a focused scroll container before touching keys.
        app.add_systems(Update, keyboard_scroll.in_set(BuiySet::Input));
        // The SC-4 a11y source sync, in Animate (strictly before A11yUpdate), so
        // a same-frame scroll is folded into the a11y tree the same frame.
        app.add_systems(Update, update_a11y_scroll.in_set(BuiySet::Animate));
        // The wheel observer (global; filtered to scroll containers, reached via
        // `Pointer<Scroll>` bubbling).
        app.add_observer(on_scroll);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::mouse::MouseScrollUnit;

    #[test]
    fn line_delta_scales_by_notch_factor() {
        // One notch (y = -1 line) is 48px (16px line × 3 lines/notch).
        let one = normalize_delta(MouseScrollUnit::Line, 0.0, -1.0);
        assert_eq!(one.y, -(LINE_HEIGHT_PX * WHEEL_LINES_PER_NOTCH));
        // Three line-deltas scale linearly → 144px.
        let three = normalize_delta(MouseScrollUnit::Line, 0.0, -3.0);
        assert_eq!(three.y, -3.0 * LINE_HEIGHT_PX * WHEEL_LINES_PER_NOTCH);
    }

    #[test]
    fn pixel_delta_passes_through() {
        let d = normalize_delta(MouseScrollUnit::Pixel, 5.0, -12.0);
        assert_eq!(d, Vec2::new(5.0, -12.0));
    }

    #[test]
    fn clamp_pins_to_range_when_valid() {
        let extent = ScrollExtent {
            content: Vec2::new(100.0, 500.0),
            viewport: Vec2::new(100.0, 200.0),
            valid: true,
        };
        // Max offset is (0, 300).
        assert_eq!(extent.max_offset(), Vec2::new(0.0, 300.0));
        // Overshoot is clamped to the bound.
        assert_eq!(
            clamp_to_extent(Vec2::new(50.0, 9999.0), &extent),
            Vec2::new(0.0, 300.0)
        );
        // Below zero clamps to zero.
        assert_eq!(clamp_to_extent(Vec2::new(-5.0, -5.0), &extent), Vec2::ZERO);
    }

    #[test]
    fn clamp_unknown_extent_only_lower_bounds() {
        let extent = ScrollExtent::default(); // valid == false
        // Upper bound unknown ⇒ a positive accumulation is preserved (not pinned
        // to a not-yet-known zero content extent).
        assert_eq!(
            clamp_to_extent(Vec2::new(0.0, 144.0), &extent),
            Vec2::new(0.0, 144.0)
        );
        // Lower bound is still applied.
        assert_eq!(clamp_to_extent(Vec2::new(0.0, -5.0), &extent), Vec2::ZERO);
    }

    #[test]
    fn contain_at_bound_consumes_no_chaining() {
        let overflow = Overflow {
            y: crate::layout::OverflowMode::Scroll,
            overscroll_y: OverscrollBehavior::Contain,
            ..Default::default()
        };
        // Already at the bottom bound (300), pushing further down (the clamp eats
        // the whole delta) ⇒ consume.
        assert!(should_consume(
            &overflow,
            Vec2::new(0.0, 300.0),
            Vec2::new(0.0, 300.0),
            Vec2::new(0.0, 48.0),
        ));
        // Still has room ⇒ does NOT consume (residual could matter).
        assert!(!should_consume(
            &overflow,
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 48.0),
            Vec2::new(0.0, 48.0),
        ));
    }

    #[test]
    fn auto_at_bound_lets_residual_bubble() {
        let overflow = Overflow {
            y: crate::layout::OverflowMode::Scroll,
            overscroll_y: OverscrollBehavior::Auto,
            ..Default::default()
        };
        // Auto never consumes — the residual bubbles to an outer container.
        assert!(!should_consume(
            &overflow,
            Vec2::new(0.0, 300.0),
            Vec2::new(0.0, 300.0),
            Vec2::new(0.0, 48.0),
        ));
    }
}
