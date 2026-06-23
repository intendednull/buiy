//! C5-b — Popover positioning + Tooltip placement (scroll-overlay-modal.md §B,
//! §6 Slice-B gates), proven at the widget layer.
//!
//! `position_popover` lowers a `Popover` onto the layout `Anchor` component; the
//! existing `anchor_resolution` (layout sub-pass 6d) then positions the popover
//! box adjacent to its anchor and FLIPS to the next candidate when the first
//! would overflow the viewport. These tests drive the full
//! `WidgetsPlugin` + `LayoutPlugin` stack and assert on the resolved
//! `ResolvedLayout.position`.
//!
//! Gates exercised (§6 Slice B):
//!  - **Popover positions relative to anchor** — the popover box lands adjacent
//!    to (below) the anchor box, NOT at the origin.
//!  - **Popover flips on overflow** — an anchor near the bottom edge whose
//!    below-candidate would overflow flips to the above-candidate.
//!  - **Tooltip placement** — a shown tooltip node is positioned near its
//!    trigger (not at the origin), in the tooltip top layer.

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use buiy_core::CorePlugin;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{
    Anchor, AnchorRef, LayoutPlugin, Stacking, Style, TopLayer, TopLayerActivation,
};
use buiy_widgets::popover::{DEFAULT_POPOVER_GAP, Popover, PopoverPlacement, PopoverSide};
use buiy_widgets::tooltip::{TooltipNode, TooltipTrigger};
use buiy_widgets::{LightDismiss, WidgetsPlugin};

/// A headless app with the full layout + widget stack. `LayoutPlugin` runs
/// `anchor_resolution` (sub-pass 6d) inside `BuiySet::Layout`; `WidgetsPlugin`
/// adds `position_popover` (`.before(BuiySet::Layout)`) + `position_tooltip`.
fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(WidgetsPlugin);
    // `escape_dismiss` reads `Res<ButtonInput<KeyCode>>` (an `InputPlugin`
    // resource absent under `MinimalPlugins`); seed it so the keyboard dismiss
    // system validates. The placement tests never press a key.
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

/// Spawn an `800×600` primary window so `FitsInViewport` has a real upper bound.
fn spawn_window(app: &mut App, w: u32, h: u32) {
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(w, h),
            ..Default::default()
        },
        PrimaryWindow,
    ));
}

fn position_of(app: &App, e: Entity) -> Vec2 {
    app.world()
        .get::<ResolvedLayout>(e)
        .expect("entity has ResolvedLayout")
        .position
}

// ---------------------------------------------------------------------------
// Popover positioning.
// ---------------------------------------------------------------------------

#[test]
fn popover_positions_below_its_anchor() {
    let mut app = app();
    spawn_window(&mut app, 800, 600);

    // Anchor (trigger): 100×40, placed inside the root at a known location.
    let root = app
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .id();
    let anchor = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(40.0)))
        .id();
    app.world_mut().entity_mut(root).add_children(&[anchor]);

    // Popover: 120×60, anchored to the trigger with the default below→above
    // flip chain. A top-layer sibling of the root (NOT a child of the anchor),
    // so its Taffy box is independent and the override positions it absolutely.
    let popover = app
        .world_mut()
        .spawn((
            Style::default().width_px(120.0).height_px(60.0),
            Popover::anchored_to(anchor),
        ))
        .id();

    // Settle: frame 1 sizes boxes; position_popover (before Layout) lowers the
    // Anchor; anchor_resolution (in Layout) applies it.
    for _ in 0..4 {
        app.update();
    }

    // The anchor sits at the root origin (0,0) size 100×40. The popover's
    // below-candidate lands at anchor.y(0) + anchor.h(40) + gap(4) = 44.
    let pos = position_of(&app, popover);
    assert_eq!(
        pos.y,
        40.0 + DEFAULT_POPOVER_GAP,
        "popover sits one gap below the anchor bottom edge"
    );
    // Adjacent to the anchor, NOT abandoned at the origin.
    assert!(pos.y > 0.0, "popover is positioned, not at the origin");

    // The lowering wrote the layout Anchor pointing at the trigger.
    let lowered = app.world().get::<Anchor>(popover).unwrap();
    assert_eq!(lowered.position_anchor, Some(AnchorRef::Entity(anchor)));
    assert_eq!(
        lowered.position_try.len(),
        2,
        "two candidates lowered (below, above)"
    );
}

#[test]
fn popover_flips_above_when_below_would_overflow() {
    let mut app = app();
    // A SHORT window (200px tall) so a popover below a near-bottom anchor
    // overflows and must flip above.
    spawn_window(&mut app, 800, 200);

    let root = app
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(200.0)))
        .id();
    // Anchor: 100×40 pushed to y≈150 via a tall spacer above it, so a 60px
    // popover BELOW it (y = 150 + 40 + 4 = 194, bottom = 254) overflows the
    // 200px window, while ABOVE it (bottom = 150 - 4 = 146, top = 86) fits.
    let spacer = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(150.0)))
        .id();
    let anchor = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(40.0)))
        .id();
    app.world_mut()
        .entity_mut(root)
        .add_children(&[spacer, anchor]);

    let popover = app
        .world_mut()
        .spawn((
            Style::default().width_px(120.0).height_px(60.0),
            Popover::anchored_to(anchor),
        ))
        .id();

    for _ in 0..4 {
        app.update();
    }

    // Anchor top edge is at y = 150 (after the 150px spacer). The flip places
    // the popover's bottom edge one gap above the anchor top: popover.y =
    // anchor.top(150) - gap(4) - popover.h(60) = 86.
    let pos = position_of(&app, popover);
    assert_eq!(
        pos.y,
        150.0 - DEFAULT_POPOVER_GAP - 60.0,
        "popover flips ABOVE the anchor when below would overflow the viewport"
    );
}

#[test]
fn popover_is_a_top_layer_overlay_in_the_activation_deque() {
    let mut app = app();
    spawn_window(&mut app, 800, 600);

    let anchor = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(40.0)))
        .id();
    let popover = app
        .world_mut()
        .spawn((
            Style::default().width_px(120.0).height_px(60.0),
            Popover::anchored_to(anchor),
        ))
        .id();

    for _ in 0..4 {
        app.update();
    }

    // The `#[require]` put the popover in the popover top layer.
    let stacking = app.world().get::<Stacking>(popover).unwrap();
    assert_eq!(stacking.top_layer, TopLayer::Popover);

    // `stacking_context` (6f) registered it in the activation deque, so the
    // dismiss handlers can find it as a top-most open overlay.
    let activation = app.world().resource::<TopLayerActivation>();
    assert!(
        activation.order.contains(&popover),
        "an open popover joins the TopLayerActivation deque"
    );

    // The auto light-dismiss policy is required, with the trigger kept exempt.
    let ld = app.world().get::<LightDismiss>(popover).unwrap();
    assert_eq!(
        ld.trigger,
        Some(anchor),
        "position_popover syncs LightDismiss.trigger from the popover anchor"
    );
}

// ---------------------------------------------------------------------------
// Tooltip placement (the C5-b half of the P1d Tooltip).
// ---------------------------------------------------------------------------

#[test]
fn tooltip_node_is_placed_near_its_trigger_not_at_the_origin() {
    let mut app = app();
    spawn_window(&mut app, 800, 600);

    // Push the trigger down so a tooltip at the origin (0,0) is clearly distinct
    // from a tooltip placed near the trigger.
    let root = app
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .id();
    let spacer = app
        .world_mut()
        .spawn((Node, Style::default().width_px(24.0).height_px(100.0)))
        .id();
    // A real tooltip trigger (carries its tooltip child + the described_by wiring).
    let trigger = app
        .world_mut()
        .spawn(TooltipTrigger::new("?", "More info"))
        .id();
    app.world_mut()
        .entity_mut(root)
        .add_children(&[spacer, trigger]);

    for _ in 0..4 {
        app.update();
    }

    // Find the tooltip node child.
    let tooltip = app
        .world()
        .get::<Children>(trigger)
        .unwrap()
        .iter()
        .find(|&c| app.world().get::<TooltipNode>(c).is_some())
        .expect("trigger has a TooltipNode child");

    // The trigger sits at y = 100 (after the 100px spacer). The tooltip is
    // anchored BELOW it: trigger.y(100) + trigger.h(24) + gap = > 100, NOT 0.
    let trigger_pos = position_of(&app, trigger);
    let tooltip_pos = position_of(&app, tooltip);
    assert!(
        trigger_pos.y >= 100.0,
        "trigger pushed below the spacer (y = {})",
        trigger_pos.y
    );
    assert_ne!(
        tooltip_pos,
        Vec2::ZERO,
        "the tooltip is positioned near the trigger, not left at the origin"
    );
    assert!(
        tooltip_pos.y > trigger_pos.y,
        "the tooltip sits below its trigger (tooltip.y {} > trigger.y {})",
        tooltip_pos.y,
        trigger_pos.y
    );

    // The placement wiring anchored the tooltip to the trigger and put it in the
    // tooltip top layer.
    let anchor = app.world().get::<Anchor>(tooltip).unwrap();
    assert_eq!(anchor.position_anchor, Some(AnchorRef::Entity(trigger)));
    let stacking = app.world().get::<Stacking>(tooltip).unwrap();
    assert_eq!(stacking.top_layer, TopLayer::Tooltip);
    // Escape-dismissable per WCAG 1.4.13, with the trigger exempt.
    let ld = app.world().get::<LightDismiss>(tooltip).unwrap();
    assert_eq!(ld.trigger, Some(trigger));
}

#[test]
fn author_positioned_popover_is_not_anchored() {
    // A `Popover { anchor: None, .. }` is author-positioned — `position_popover`
    // leaves it without a position_anchor (no flip-chain lowering).
    let mut app = app();
    spawn_window(&mut app, 800, 600);
    let popover = app
        .world_mut()
        .spawn((
            Style::default().width_px(120.0).height_px(60.0),
            Popover::default(), // anchor: None
        ))
        .id();
    for _ in 0..4 {
        app.update();
    }
    let anchor = app.world().get::<Anchor>(popover).unwrap();
    assert_eq!(
        anchor.position_anchor, None,
        "an unanchored popover is not lowered onto the anchor pipeline"
    );
    // Spell PopoverPlacement/PopoverSide so the unused-import lint sees them used.
    let _ = PopoverPlacement {
        side: PopoverSide::Right,
        ..Default::default()
    };
}
