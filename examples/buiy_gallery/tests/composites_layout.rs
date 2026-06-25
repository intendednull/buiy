//! Headless layout-snapshot + content gate for the **composite widgets** (parity
//! Wave C2 — Tier 1 of the `buiy_verify` pyramid: no GPU, no on-screen window).
//! Drives the SAME [`composites_showcase`] grid the `capture_composites` artifact
//! bin renders (one of each composite: stepper / segmented / search / meter /
//! badge / chip / kbd / status-dots / stat-row / table header+rows), then:
//!
//! 1. pins the resolved layout of every `#Name`-tagged composite entity (a
//!    structural regression — a dropped cell, a wrong box, a lost column — shows
//!    as a `.snap` diff), and
//! 2. **sample-asserts the parity invariants the layout dump cannot show**: the
//!    segmented SELECTED option is the accent fill (+ on-accent label), the meter
//!    fill has a NON-ZERO X scale (the fraction shown), and the SELECTED table row
//!    is the `accent.soft` fill with an accent left-bar child.
//!
//! The showcase page sizes to `100%`, so the test stands up a headless
//! `(Window, PrimaryWindow)` at 1280×900 — the layout viewport the `100%` page
//! resolves against (the `shell_layout` pattern).

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::name::Name;
use bevy::scene::ScenePlugin;
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use buiy::prelude::ColorToken;
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_core::layout::Scale;
use buiy_core::render::components::{Background, TextColor};
use buiy_core::theme::default_dark_theme;
use buiy_gallery::composites::{MeterFill, SegmentedOption, TableRow, composites_showcase};
use buiy_verify::snapshot::assert_layout_snapshot;

/// Build the live composites-showcase tree: a 1280×900 headless window, the dark
/// theme, then [`composites_showcase`]. Returns `(app, meter_fill)`.
fn showcase_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);

    app.insert_resource(default_dark_theme());

    // A headless primary window — the layout viewport the page's `100%` resolves
    // against.
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(1280, 900),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    // `composites_showcase` takes `&mut World` directly (the imperative composite-
    // building idiom).
    let meter_fill = {
        let world = app.world_mut();
        let (_page, fill) = composites_showcase(world);
        fill
    };
    (app, meter_fill)
}

#[test]
fn composites_lay_out_as_expected() {
    let (mut app, _fill) = showcase_app();
    assert_layout_snapshot(&mut app, "composites_showcase");
}

/// Parity invariant: the SELECTED segmented option (index 0 in the showcase) paints
/// the `accent` fill + an on-accent label; the unselected options are transparent +
/// muted. (The layout dump shows the boxes, not the paint — assert it here.)
#[test]
fn segmented_selected_option_is_accent_filled() {
    let (mut app, _fill) = showcase_app();
    app.update();

    let world = app.world_mut();
    let mut q = world.query::<(&SegmentedOption, &Background, &Children)>();
    let mut saw_selected = false;
    let mut saw_unselected = false;
    // Collect first (the label re-query needs a second borrow).
    let rows: Vec<(usize, Background, Vec<Entity>)> = q
        .iter(world)
        .map(|(o, bg, ch)| (o.0, bg.clone(), ch.iter().copied().collect()))
        .collect();
    for (idx, bg, children) in rows {
        // The option's label leaf is its first `TextColor` child.
        let label_color = children
            .iter()
            .find_map(|&c| world.get::<TextColor>(c).cloned());
        if idx == 0 {
            // Selected: accent bg + on-accent label.
            assert_eq!(
                bg.color,
                tok("color.accent"),
                "selected segmented option must be accent-filled, got {:?}",
                bg.color
            );
            assert_eq!(
                label_color.map(|c| c.0),
                Some(tok("color.text.on-accent")),
                "selected segmented label must be on-accent"
            );
            saw_selected = true;
        } else {
            // Unselected: transparent bg + muted label.
            assert_eq!(
                bg.color,
                tok("color.surface.transparent"),
                "unselected segmented option must be transparent, got {:?}",
                bg.color
            );
            assert_eq!(
                label_color.map(|c| c.0),
                Some(tok("color.text.muted")),
                "unselected segmented label must be muted"
            );
            saw_unselected = true;
        }
    }
    assert!(
        saw_selected && saw_unselected,
        "expected ≥1 selected + ≥1 unselected option"
    );
}

/// Parity invariant: the meter fill has a NON-ZERO, ≤1 X scale (the fraction
/// shown). The showcase seeds 0.64 — assert the fill's resting `Scale.0` matches.
#[test]
fn meter_fill_has_nonzero_fraction_scale() {
    let (mut app, fill) = showcase_app();
    app.update();

    let world = app.world();
    assert!(
        world.get::<MeterFill>(fill).is_some(),
        "the meter fill entity must carry the MeterFill marker"
    );
    let scale = world
        .get::<Scale>(fill)
        .expect("the meter fill must carry a resting Scale (its fraction)");
    assert!(
        scale.0 > 0.0 && scale.0 <= 1.0,
        "meter fill X scale must be a fraction in (0,1], got {}",
        scale.0
    );
    assert!(
        (scale.0 - 0.64).abs() < 1e-4,
        "the showcase meter fraction is 0.64, got {}",
        scale.0
    );
}

/// Parity invariant: the SELECTED table row is the `accent.soft` fill and carries a
/// 2.5px accent left-bar child (`#RowSelBar`); the unselected row is transparent
/// with no bar.
#[test]
fn selected_table_row_is_accent_soft_with_left_bar() {
    let (mut app, _fill) = showcase_app();
    app.update();

    let world = app.world_mut();
    let mut q = world.query::<(&TableRow, &Background, &Children)>();
    let rows: Vec<(Background, Vec<Entity>)> = q
        .iter(world)
        .map(|(_, bg, ch)| (bg.clone(), ch.iter().copied().collect()))
        .collect();
    assert_eq!(rows.len(), 2, "the showcase lays out exactly 2 table rows");

    let mut selected = 0;
    let mut unselected = 0;
    for (bg, children) in &rows {
        let has_bar = children.iter().any(|&c| {
            world
                .get::<Name>(c)
                .is_some_and(|n| n.as_str() == "#RowSelBar")
        });
        if bg.color == tok("color.accent.soft") {
            assert!(
                has_bar,
                "the selected (accent.soft) row must carry the left bar"
            );
            selected += 1;
        } else {
            assert_eq!(
                bg.color,
                tok("color.surface.transparent"),
                "the unselected row must be transparent"
            );
            assert!(!has_bar, "the unselected row must NOT carry a left bar");
            unselected += 1;
        }
    }
    assert_eq!(
        (selected, unselected),
        (1, 1),
        "exactly one selected + one unselected row"
    );
}

/// The meter's animation: [`set_meter`] attaches a left-anchored `ScaleTween` that
/// advances the fill's X scale from the current value toward the new fraction with
/// the design easing, and lands exactly on the target. Driven headlessly via
/// manual `Time` (no render loop) — the same pattern the `animation` module tests
/// use.
#[test]
fn set_meter_animates_x_scale_to_target() {
    use std::time::Duration;

    use bevy::time::Time;
    use buiy_core::animation::{AnimationPlugin, ScaleTween};
    use buiy_gallery::composites::{meter, set_meter};

    let mut app = App::new();
    app.init_resource::<Time>();
    app.add_plugins(AnimationPlugin);
    // `AnimationPlugin` schedules into `BuiySet::Animate`; configure the bare set
    // so the systems run in this minimal app (the animation module's test idiom).
    app.configure_sets(bevy::app::Update, buiy_core::BuiySet::Animate);

    // A meter at 0.20, animated to 0.80.
    let fill = {
        let world = app.world_mut();
        let (_track, fill) = meter(world, 240.0, 0.20);
        set_meter(world, fill, 0.80);
        fill
    };

    // A `ScaleTween` was attached (the animation, not a per-frame Taffy width).
    assert!(
        app.world().get::<ScaleTween>(fill).is_some(),
        "set_meter must attach a ScaleTween (transform-only animation)"
    );

    // Advance partway: the X scale is between the start (0.20) and target (0.80).
    {
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(Duration::from_millis(150)); // half of the 0.3s tween
    }
    app.update();
    let mid = app
        .world()
        .get::<Scale>(fill)
        .expect("fill Scale mid-flight")
        .0;
    assert!(
        mid > 0.20 && mid < 0.80,
        "mid-flight X scale must be between start and target, got {mid}"
    );

    // Advance to completion: lands exactly on 0.80 and the tween is removed.
    {
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(Duration::from_millis(300));
    }
    app.update();
    let end = app.world().get::<Scale>(fill).expect("fill Scale at end").0;
    assert!(
        (end - 0.80).abs() < 1e-4,
        "the meter must land exactly on the target fraction, got {end}"
    );
    assert!(
        app.world().get::<ScaleTween>(fill).is_none(),
        "the completed ScaleTween must be removed (end-state kept)"
    );
}

/// The toast lifecycle: [`show_toast`] spawns a top-layer toast card + arms the
/// auto-dismiss timer; [`ToastPlugin`]'s tick despawns it once the ~2.2s lifetime
/// elapses. Driven headlessly via manual `Time`.
#[test]
fn toast_shows_then_auto_dismisses() {
    use std::time::Duration;

    use bevy::time::Time;
    use buiy_gallery::composites::{Toast, ToastPlugin, show_toast};

    let mut app = App::new();
    app.init_resource::<Time>();
    app.add_plugins(ToastPlugin);
    app.configure_sets(bevy::app::Update, buiy_core::BuiySet::Animate);

    // Show a toast: the resource holds the entity + an armed timer.
    {
        let world = app.world_mut();
        show_toast(world, "Saved");
    }
    let toast_entity = app
        .world()
        .resource::<Toast>()
        .entity
        .expect("show_toast must set the live toast entity");
    assert!(
        app.world().get_entity(toast_entity).is_ok(),
        "the toast card must be spawned"
    );

    // Before the lifetime elapses, the toast is still live.
    {
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(Duration::from_millis(500));
    }
    app.update();
    assert!(
        app.world().resource::<Toast>().entity.is_some(),
        "the toast must persist before its lifetime elapses"
    );

    // After > 2.2s total, the tick despawns it + clears the resource.
    {
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(Duration::from_millis(2000));
    }
    app.update();
    assert!(
        app.world().resource::<Toast>().entity.is_none(),
        "the toast must auto-dismiss after its lifetime"
    );
    assert!(
        app.world().get_entity(toast_entity).is_err(),
        "the auto-dismissed toast card must be despawned"
    );
}

/// **The toast is laid out BOTTOM-CENTER** (the C4 centering fix — C2 left it
/// top-left at `left:50%` with no `translateX(-50%)`). Lays out a shown toast in a
/// full 1280×800 window and asserts (a) the `#ToastWrapper` fills the viewport
/// (the top-layer fixed_root) and (b) the `#Toast` card — parent-relative to that
/// viewport-filling wrapper — is horizontally centered (its box midpoint ≈ the
/// viewport midpoint) and bottom-anchored (its bottom edge is `44px` above the
/// viewport floor — the design's `bottom:44px`).
///
/// Uses `ResolvedLayout` (parent-relative). The card's parent is the wrapper, and
/// the wrapper IS the viewport box at the origin, so the card's parent-relative
/// coordinates ARE its viewport coordinates — no `GlobalTransform` accumulation
/// needed (and the headless layout app has no render bridge to populate it).
#[test]
fn toast_lays_out_bottom_center() {
    use bevy::time::Time;
    use buiy_core::ResolvedLayout;
    use buiy_gallery::composites::{ToastPlugin, show_toast};

    const W: f32 = 1280.0;
    const H: f32 = 800.0;
    const TOAST_BOTTOM: f32 = 44.0;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin)
        .add_plugins(ToastPlugin);
    app.init_resource::<Time>();
    app.insert_resource(default_dark_theme());
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(W as u32, H as u32),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    {
        let world = app.world_mut();
        show_toast(world, "Created primary_button.bsn");
    }
    // Settle layout (the top-layer fixed_root attach + the wrapper's flex centering).
    for _ in 0..6 {
        app.update();
    }

    // The wrapper + card resolved boxes (both `#Name`-tagged).
    let boxes = |app: &mut App, name: &str| -> Option<(bevy::math::Vec2, bevy::math::Vec2)> {
        let mut q = app.world_mut().query::<(&Name, &ResolvedLayout)>();
        let world = app.world();
        q.iter(world)
            .find(|(n, _)| n.as_str() == name)
            .map(|(_, l)| (l.position, l.size))
    };

    // (a) The wrapper fills the layout viewport (the top-layer fixed_root at the
    // origin). Its width is the window width; its height is whatever the engine's
    // layout viewport is (the centering is asserted relative to THIS box, since the
    // card's parent-relative coords are coords within this viewport-filling box).
    let (wrap_pos, wrap_size) = boxes(&mut app, "#ToastWrapper").expect("the toast wrapper exists");
    assert!(
        (wrap_size.x - W).abs() <= 1.0 && wrap_size.y > 0.0 && wrap_pos == bevy::math::Vec2::ZERO,
        "the toast wrapper should fill the viewport from the origin, got pos {wrap_pos:?} size {wrap_size:?}"
    );

    // (b) The card, parent-relative to the viewport-filling wrapper.
    let (pos, size) = boxes(&mut app, "#Toast").expect("the #Toast card must be laid out");
    assert!(
        size.x > 0.0 && size.y > 0.0,
        "the toast card must have a non-zero box, got {size:?}"
    );

    // Horizontally centered within the wrapper (= the viewport), within 1px.
    let card_mid_x = pos.x + size.x / 2.0;
    assert!(
        (card_mid_x - wrap_size.x / 2.0).abs() <= 1.0,
        "the toast should be horizontally centered: card mid-x {card_mid_x} vs viewport mid {} (pos {pos:?} size {size:?})",
        wrap_size.x / 2.0
    );

    // Bottom-anchored: the card's bottom edge sits `TOAST_BOTTOM` px above the
    // wrapper's bottom edge (the design's `bottom:44px`).
    let card_bottom = pos.y + size.y;
    assert!(
        (card_bottom - (wrap_size.y - TOAST_BOTTOM)).abs() <= 1.0,
        "the toast bottom edge {card_bottom} should be {TOAST_BOTTOM}px above the wrapper floor ({})",
        wrap_size.y - TOAST_BOTTOM
    );
    let _ = H;
}

/// A `ColorToken::Token` test helper (mirrors the module's private `tok`).
fn tok(key: &str) -> ColorToken {
    ColorToken::Token(key.to_string().into())
}
