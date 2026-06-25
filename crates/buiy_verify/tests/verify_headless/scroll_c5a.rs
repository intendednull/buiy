//! C5-a — scroll input, proven headless on the C7 `PointerHarness`
//! (scroll-overlay-modal.md §A, §6 Slice-A gates).
//!
//! Each test drives a synthetic wheel / keyboard scroll through the PRODUCTION
//! scroll pipeline (layout → `update_scroll_extent` → the `Pointer<Scroll>`
//! observer / `keyboard_scroll` → clamped `ScrollOffset`) and asserts on the
//! resulting `ScrollOffset` + the cached `ScrollExtent`. The harness adds
//! `ScrollInputPlugin`, so the wheel/keyboard handlers and the extent cache are
//! the real production systems.
//!
//! Gates exercised (§6 Slice A):
//!  - **Scroll clamp** — a wheel delta advances `ScrollOffset` and clamps to
//!    `[0, content − viewport]` at both ends.
//!  - **Keyboard scroll** — Arrow/Page/Home/End scroll a focused container,
//!    clamped.
//!  - **Unit normalization** — `Line` vs `Pixel` deltas produce the expected px.
//!  - **No-invalidate** — a scroll leaves `ResolvedLayout` unchanged.

use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseScrollUnit;
use bevy::prelude::*;
use buiy_core::a11y::A11yScroll;
use buiy_core::components::ResolvedLayout;
use buiy_core::focus::{Focusable, FocusedEntity};
use buiy_core::layout::{
    BoxModel, FlexAxis, FlexParams, Length, OverflowMode, ScrollOffset, Sizing, Style,
};
use buiy_core::scroll::{KEY_LINE_STEP_PX, LINE_HEIGHT_PX, ScrollExtent, WHEEL_LINES_PER_NOTCH};
use buiy_core::{Node, a11y::A11yRole};
use buiy_verify::pointer::PointerHarness;
use buiy_widgets::ScrollArea;

/// One wheel notch in logical px (16px line × 3 lines/notch = 48px).
const NOTCH_PX: f32 = LINE_HEIGHT_PX * WHEEL_LINES_PER_NOTCH;

/// Spawn a sized vertical scroll container (`200px` viewport) holding a single
/// `600px`-tall child, inside the harness's 800×600 window. Returns the
/// scroll-container entity. Settles layout so `update_scroll_extent` populates
/// `ScrollExtent { content: ~600, viewport: 200, valid: true }` before any
/// scroll. The container carries `A11yRole::Group` + `Focusable` (the
/// `ScrollArea` contract, spelled from `buiy_core` so the harness needs no
/// `WidgetsPlugin`).
fn spawn_scroll_container(h: &mut PointerHarness, viewport_h: f32, content_h: f32) -> Entity {
    let child = h
        .world_mut()
        .spawn((
            Node,
            // `min_height` == the content height so the flex column cannot shrink
            // the overflowing child back to the viewport (CSS flex-shrink would
            // otherwise compress it; pinning min-height keeps it tall, which is
            // what makes the container overflow + scroll).
            Style::default()
                .width_px(180.0)
                .height_px(content_h)
                .min_height(Sizing::Length(Length::Px(content_h))),
        ))
        .id();
    let container = h
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(200.0)
                .height_px(viewport_h)
                .overflow_y(OverflowMode::Scroll),
            ScrollOffset::default(),
            ScrollExtent::default(),
            Focusable::default(),
            A11yRole::Group,
            Name::new("scroll-container"),
        ))
        .add_child(child)
        .id();
    // Settle: layout → update_scroll_extent (after Layout, before Input).
    for _ in 0..4 {
        h.update();
    }
    container
}

fn scroll_offset(h: &PointerHarness, e: Entity) -> Vec2 {
    let off = h
        .world()
        .get::<ScrollOffset>(e)
        .expect("scroll container has ScrollOffset");
    Vec2::new(off.x, off.y)
}

fn scroll_extent(h: &PointerHarness, e: Entity) -> ScrollExtent {
    *h.world()
        .get::<ScrollExtent>(e)
        .expect("scroll container has ScrollExtent")
}

/// Scroll clamp (§A.1 / §6): a wheel-down delta advances `ScrollOffset.y` by the
/// normalized px, and a delta past the content extent clamps to
/// `content − viewport`; a wheel-up past the top clamps to `0`.
#[test]
fn wheel_scroll_advances_and_clamps_at_both_ends() {
    let mut h = PointerHarness::new();
    let container = spawn_scroll_container(&mut h, 200.0, 600.0);

    // The extent must be current before scroll (the ordered producer guarantee).
    let extent = scroll_extent(&h, container);
    assert!(
        extent.valid,
        "the extent must be valid after the layout pass"
    );
    assert_eq!(
        extent.viewport,
        Vec2::new(200.0, 200.0),
        "viewport = box size"
    );
    assert_eq!(
        extent.content,
        Vec2::new(200.0, 600.0),
        "content extent = union of child boxes (600px tall child)"
    );
    let max_y = extent.max_offset().y;
    assert_eq!(max_y, 400.0, "max offset = content − viewport = 600 − 200");

    // Position the pointer over the container and scroll DOWN one notch.
    let center = h.global_center(container);
    h.move_to(center);
    assert_eq!(scroll_offset(&h, container), Vec2::ZERO, "starts at top");

    h.scroll(MouseScrollUnit::Line, 0.0, 1.0); // one notch down (+48px)
    assert_eq!(
        scroll_offset(&h, container).y,
        NOTCH_PX,
        "one wheel-down notch advances ScrollOffset.y by 48px"
    );

    // Scroll far past the bottom — clamps to the max (400), never overshoots.
    h.scroll(MouseScrollUnit::Line, 0.0, 100.0);
    assert_eq!(
        scroll_offset(&h, container).y,
        max_y,
        "a large wheel-down clamps to content − viewport, no overshoot"
    );

    // Scroll up past the top — clamps to 0.
    h.scroll(MouseScrollUnit::Line, 0.0, -100.0);
    assert_eq!(
        scroll_offset(&h, container).y,
        0.0,
        "a large wheel-up clamps to 0, no undershoot"
    );
}

/// Unit normalization (§A.1): a `Pixel` (trackpad) delta passes through 1:1,
/// while a `Line` delta scales by the per-notch factor.
#[test]
fn pixel_and_line_deltas_normalize() {
    let mut h = PointerHarness::new();
    let container = spawn_scroll_container(&mut h, 200.0, 600.0);
    let center = h.global_center(container);
    h.move_to(center);

    h.scroll(MouseScrollUnit::Pixel, 0.0, 30.0);
    assert_eq!(
        scroll_offset(&h, container).y,
        30.0,
        "a Pixel delta scrolls 1:1 (trackpad)"
    );

    // A Line delta of 1 adds one notch (48px) on top of the 30px pixel scroll.
    h.scroll(MouseScrollUnit::Line, 0.0, 1.0);
    assert_eq!(
        scroll_offset(&h, container).y,
        30.0 + NOTCH_PX,
        "a Line delta scales by 16px × 3 lines/notch"
    );
}

/// Keyboard scroll (§A.2 / §6): a FOCUSED scroll container scrolls on Arrow
/// (line), PageDown (page), Home/End (top/bottom) — clamped.
#[test]
fn keyboard_scroll_arrows_page_home_end_clamped() {
    let mut h = PointerHarness::new();
    let container = spawn_scroll_container(&mut h, 200.0, 600.0);
    let max_y = scroll_extent(&h, container).max_offset().y;

    // Focus the container so keyboard scroll targets it.
    h.world_mut().resource_mut::<FocusedEntity>().0 = Some(container);

    // ArrowDown → one line (KEY_LINE_STEP_PX = 48px).
    h.press_key(KeyCode::ArrowDown);
    assert_eq!(
        scroll_offset(&h, container).y,
        KEY_LINE_STEP_PX,
        "ArrowDown scrolls one line"
    );

    // PageDown → one viewport minus the overlap (200 − 40 = 160), on top of the
    // current 48 → 208 (still within the 400 max).
    h.press_key(KeyCode::PageDown);
    let after_page = scroll_offset(&h, container).y;
    assert!(
        after_page > KEY_LINE_STEP_PX && after_page <= max_y,
        "PageDown advances ~one viewport, clamped within range (got {after_page})"
    );

    // End → the bottom bound.
    h.press_key(KeyCode::End);
    assert_eq!(
        scroll_offset(&h, container).y,
        max_y,
        "End scrolls to the bottom bound"
    );

    // ArrowDown at the bottom is a clamped no-op (stays at max).
    h.press_key(KeyCode::ArrowDown);
    assert_eq!(
        scroll_offset(&h, container).y,
        max_y,
        "ArrowDown at the bottom clamps (no overshoot)"
    );

    // Home → back to the top.
    h.press_key(KeyCode::Home);
    assert_eq!(
        scroll_offset(&h, container).y,
        0.0,
        "Home scrolls to the top"
    );
}

/// Keyboard scroll does NOT steal keys when the focused entity is not a scroll
/// container (§A.2 — gated on the focused entity being a scroll container).
#[test]
fn keyboard_scroll_ignores_non_scroll_focus() {
    let mut h = PointerHarness::new();
    let container = spawn_scroll_container(&mut h, 200.0, 600.0);
    // Focus a DIFFERENT, non-scroll entity.
    let other = h
        .world_mut()
        .spawn((Node, Style::default().width_px(20.0).height_px(20.0)))
        .id();
    h.world_mut().resource_mut::<FocusedEntity>().0 = Some(other);

    h.press_key(KeyCode::ArrowDown);
    assert_eq!(
        scroll_offset(&h, container).y,
        0.0,
        "a non-scroll focus does not scroll the container"
    );
}

/// The real `ScrollArea` widget (§A.4): its `#[require]` materializes a working
/// scroll container — `Overflow` (scrollable), `ScrollOffset`, `ScrollExtent`,
/// `Focusable`, `A11yRole::Group`, and the SC-4 `A11yScroll` source — so a wheel
/// over it scrolls (clamped) and the a11y scroll source is present + updated.
#[test]
fn scroll_area_widget_scrolls_and_carries_a11y_source() {
    let mut h = PointerHarness::new();
    // The tall content child (pinned min-height so the flex column can't shrink
    // it back to the viewport).
    let child = h
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(180.0)
                .height_px(600.0)
                .min_height(Sizing::Length(Length::Px(600.0))),
        ))
        .id();
    // The real widget — `Overflow = scroll_area_overflow()` is `{ y: Auto }`,
    // which is a scroll container. Size it via the DECOMPOSED `BoxModel` +
    // `FlexParams` (NOT the whole `Style` bundle, which carries an `Overflow`
    // that would suppress the widget's `#[require(Overflow = …)]` — the §4.1c
    // suppression gotcha). 200px viewport, column layout.
    let area = h
        .world_mut()
        .spawn((
            ScrollArea,
            BoxModel {
                width: Sizing::Length(Length::Px(200.0)),
                height: Sizing::Length(Length::Px(200.0)),
                ..Default::default()
            },
            FlexParams {
                direction: FlexAxis::Column,
                ..Default::default()
            },
        ))
        .add_child(child)
        .id();
    for _ in 0..4 {
        h.update();
    }

    // The `#[require]` bundle is present.
    assert!(
        h.world().get::<A11yScroll>(area).is_some(),
        "ScrollArea #[require]s the SC-4 A11yScroll source"
    );
    assert_eq!(
        h.world().get::<A11yRole>(area).copied(),
        Some(A11yRole::Group),
        "ScrollArea is a Group (scroll region)"
    );

    // A wheel over the widget scrolls it (clamped to the content extent).
    let center = h.global_center(area);
    h.move_to(center);
    h.scroll(MouseScrollUnit::Line, 0.0, 2.0);
    let off = scroll_offset(&h, area).y;
    assert!(
        off > 0.0,
        "the ScrollArea widget scrolls on a wheel (got {off})"
    );

    // The a11y source mirrors the live offset (update_a11y_scroll, in Animate).
    let a11y = *h.world().get::<A11yScroll>(area).unwrap();
    assert_eq!(
        a11y.offset.y, off,
        "A11yScroll mirrors the live ScrollOffset"
    );
    assert!(
        a11y.scrollable,
        "content exceeds viewport ⇒ scrollable flag set"
    );
}

/// No-invalidate (§6): a scroll leaves the container's + child's `ResolvedLayout`
/// unchanged (the `ScrollOffset` write must not re-run layout). Mirrors the
/// `layout_scroll_offset_no_invalidate` invariant, observed through the harness.
#[test]
fn scroll_does_not_change_resolved_layout() {
    let mut h = PointerHarness::new();
    let container = spawn_scroll_container(&mut h, 200.0, 600.0);
    let before = {
        let rl = h
            .world()
            .get::<ResolvedLayout>(container)
            .expect("container has ResolvedLayout");
        (rl.position, rl.size)
    };

    let center = h.global_center(container);
    h.move_to(center);
    h.scroll(MouseScrollUnit::Line, 0.0, 2.0);
    h.update();

    let after = {
        let rl = h
            .world()
            .get::<ResolvedLayout>(container)
            .expect("container has ResolvedLayout");
        (rl.position, rl.size)
    };
    assert_eq!(
        before, after,
        "scrolling must NOT invalidate / change ResolvedLayout"
    );
    assert!(
        scroll_offset(&h, container).y > 0.0,
        "but the scroll DID move the offset"
    );
}
