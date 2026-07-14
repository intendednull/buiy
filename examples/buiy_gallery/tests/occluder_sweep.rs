//! Standing structural gate (Tier-3 invariant, F6 / app-author-ergonomics
//! 4b-invariant): **no gallery screen leaves a transparent top-layer pick
//! occluder.** Reconciles each of the 5 hand-authored widget-catalog screens (and
//! the unified shell that mounts them) into a headless `World`, then sweeps the
//! reusable `buiy_verify::invariant::no_transparent_top_layer_occluder` predicate
//! over EVERY entity.
//!
//! The gallery is the higher-yield target for this class than the `buiy_view`
//! fixture catalog: its screens are **hand-authored** retained trees (the
//! `spawn_*` imperative idiom), which get NONE of the reconciler's
//! auto-`Pickable::IGNORE` construction guarantee — exactly the surface the
//! invisible-occluder bug class (shipped 3×) lives on. If a future edit
//! (re)introduces a transparent `.top_layer()` container that paints nothing and
//! is not `Pickable::IGNORE`, this gate goes RED the moment the screen is
//! reconciled — independent of anyone remembering to click it.
//!
//! ## Why `BuiyRenderPlugin` is in the driver
//!
//! A closed overlay (the S3 standalone `Popover` is a transparent top-layer node
//! with no fill, held `CssVisibility::Hidden` inside a `Display::None` holder) is
//! the class ONLY while it is a live pick candidate. `write_paint_skip` (owned by
//! `BuiyRenderPlugin`) stamps `ComputedPaintSkip` across every hidden subtree, and
//! the predicate excludes a paint-skipped node (the picking backend already drops
//! it). Without the render-prep pass the sweep would false-positive on that
//! correctly-hidden popover; WITH it, the driver faithfully mirrors what the real
//! app's picking backend sees. (`BuiyRenderPlugin` also schedules the debug-only
//! `Last` coherence panic, so each settled screen is double-checked — a buggy
//! screen fails here whether via this sweep OR that panic.)
//!
//! The fail-on-revert test ([`injecting_a_transparent_top_layer_occluder_makes_the_gate_red`])
//! proves the gate has teeth: it injects the exact bug shape into a swept-green
//! screen and asserts the invariant flips to `Err`, naming the offender.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::{Entity, World};
use bevy::scene::ScenePlugin;
use bevy::window::{PrimaryWindow, Window, WindowResolution};

use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_core::Node;
use buiy_core::focus::FocusPlugin;
use buiy_core::layout::{Stacking, TopLayer};
use buiy_core::render::BuiyRenderPlugin;
use buiy_core::theme::default_dark_theme;
use buiy_gallery::inspector::build_inspector_content;
use buiy_gallery::shell::{ScreenRouter, build_shell, mount_screens_with};
use buiy_gallery::{
    DEMO_SEEDS, append_row, fill_scroll_list, spawn_modal, spawn_overlay_menu, spawn_scroll_screen,
    spawn_showcase, spawn_todomvc_screen,
};
use buiy_verify::invariant::no_transparent_top_layer_occluder;

/// The design preview viewport the shell's `100%` root resolves against.
const VW: u32 = 1280;
const VH: u32 = 800;

/// A headless gallery driver: the same layout/text/widgets/render-prep plugin set
/// the screens boot under, a 1280×800 primary window, the dark theme, and a
/// `ButtonInput` resource (the S4 dialog focus-trap reads it). `BuiyRenderPlugin`
/// is included so `write_paint_skip` settles `ComputedPaintSkip` (see module docs).
fn gallery_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        // The Dialog's C5-d focus-trap rides `FocusPlugin`'s `handle_tab`, which
        // reads `Res<ButtonInput<KeyCode>>` (non-optional).
        .add_plugins(FocusPlugin)
        .add_plugins(WidgetsPlugin)
        // Render-prep (headless-safe): write_clip_rects / write_effect_groups /
        // write_paint_skip + the debug-only Last occluder-coherence panic.
        .add_plugins(BuiyRenderPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(default_dark_theme());
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(VW, VH),
            ..Default::default()
        },
        PrimaryWindow,
    ));
    app
}

/// Settle the driver so layout resolves, transforms propagate, and (crucially)
/// `write_paint_skip` stamps `ComputedPaintSkip` across the hidden overlay
/// subtrees before the sweep reads them.
fn settle(app: &mut App) {
    // Plain updates resolve layout + propagate transforms + run write_paint_skip
    // (the same idiom the shell-chrome layout gate uses); the 1280×800 window is
    // the viewport the `100%` roots resolve against.
    for _ in 0..8 {
        app.update();
    }
}

/// Build a screen with `build`, settle, and return the app. Each screen gets a
/// fresh driver so per-screen state is isolated.
fn screen_app(build: impl FnOnce(&mut World)) -> App {
    let mut app = gallery_app();
    build(app.world_mut());
    settle(&mut app);
    app
}

/// Sweep the invariant over `app`'s world; panic (RED) if any screen entity is a
/// transparent top-layer occluder, naming the offender.
fn assert_no_occluder(app: &App, screen: &str) {
    no_transparent_top_layer_occluder(app.world()).unwrap_or_else(|v| {
        panic!("gallery screen `{screen}` leaves a transparent top-layer pick occluder: {v}")
    });
}

// --- Per-screen standing gate ------------------------------------------------

#[test]
fn s1_todomvc_screen_has_no_transparent_top_layer_occluder() {
    let app = screen_app(|world| {
        spawn_todomvc_screen(world);
        for &(label, completed) in DEMO_SEEDS {
            append_row(world, label, completed);
        }
    });
    assert_no_occluder(&app, "todomvc");
}

#[test]
fn s2_scroll_screen_has_no_transparent_top_layer_occluder() {
    let app = screen_app(|world| {
        spawn_scroll_screen(world);
        // The sweep only cares about top-layer nodes; a few rows keep it light.
        fill_scroll_list(world, 3);
    });
    assert_no_occluder(&app, "scroll");
}

#[test]
fn s3_overlay_menu_screen_has_no_transparent_top_layer_occluder() {
    // S3 is the load-bearing case: it holds the MenuButton→Menu (top-layer, painted
    // + starts CssVisibility::Hidden), a TooltipTrigger, and the standalone Popover
    // (top-layer, NO fill, held CssVisibility::Hidden inside a Display::None holder).
    // The popover is safe ONLY because write_paint_skip marks it paint-skipped.
    let app = screen_app(|world| {
        spawn_overlay_menu(world);
    });
    assert_no_occluder(&app, "overlay_menu");
}

#[test]
fn s4_modal_screen_has_no_transparent_top_layer_occluder() {
    // S4 holds the C5-d Dialog (top-layer TopLayer::Modal, painted panel, starts
    // closed via CssVisibility::Hidden).
    let app = screen_app(|world| {
        spawn_modal(world);
    });
    assert_no_occluder(&app, "modal");
}

#[test]
fn s5_showcase_screen_has_no_transparent_top_layer_occluder() {
    let app = screen_app(|world| {
        spawn_showcase(world);
    });
    assert_no_occluder(&app, "showcase");
}

/// The unified shell (the gallery's real headless driver): `build_shell` +
/// `mount_screens_with` mount ALL 5 screens in one world (the default Todo screen
/// active, the other 4 `Display::None`), plus the inspector pane. Sweeping it
/// covers every mounted screen's overlays (the hidden ones paint-skipped with the
/// rest of their `Display::None` subtree) in one shot.
#[test]
fn unified_shell_has_no_transparent_top_layer_occluder() {
    let mut app = gallery_app();
    app.init_resource::<ScreenRouter>();
    let world = app.world_mut();
    build_shell(world);
    // 0 scroll rows keeps the (hidden) scroll screen's dump light — irrelevant to
    // the sweep, which only inspects top-layer nodes.
    mount_screens_with(world, 0);
    build_inspector_content(world);
    settle(&mut app);
    assert_no_occluder(&app, "unified_shell");
}

// --- Fail-on-revert: prove the gate has teeth --------------------------------

/// The acceptance: injecting the exact invisible-occluder bug shape — a
/// transparent (`no fill`) `.top_layer()` `Node` with no `Pickable::IGNORE` — into
/// a screen that swept GREEN makes the invariant flip to `Err`, naming the offender.
///
/// The invariant is called DIRECTLY on the world after injection (no further
/// `app.update()`), so it isolates the SWEEP's signal — the `BuiyRenderPlugin`
/// `Last` coherence panic would otherwise fire first on the same injected node
/// (itself a second, independent proof the bug is caught).
#[test]
fn injecting_a_transparent_top_layer_occluder_makes_the_gate_red() {
    // A green screen first (the S3 overlay screen — the richest top-layer catalog).
    let mut app = screen_app(|world| {
        spawn_overlay_menu(world);
    });
    assert!(
        no_transparent_top_layer_occluder(app.world()).is_ok(),
        "the pristine overlay screen must sweep green before injection",
    );

    // Inject the bug: a transparent top-layer Node with no fill and no IGNORE. It
    // is NOT hidden (no ComputedPaintSkip), so it is a live pick candidate — the
    // class. (Not settled afterward, so write_paint_skip never marks it skipped and
    // the coherence panic never pre-empts the assertion below.)
    let occluder = app
        .world_mut()
        .spawn((
            Node,
            Stacking {
                top_layer: TopLayer::Popover,
                ..Default::default()
            },
        ))
        .id();

    let err = no_transparent_top_layer_occluder(app.world())
        .expect_err("an injected transparent top-layer occluder must make the sweep RED");
    assert_eq!(err.rule, "no_transparent_top_layer_occluder");
    assert!(
        err.detail.contains(&format!("{occluder:?}")),
        "the violation must name the injected offender {occluder:?}, got: {}",
        err.detail,
    );
    // Sanity: the offender is the ONLY one (the screen itself was green).
    let offender_count = app
        .world()
        .iter_entities()
        .filter(|e| {
            e.contains::<Node>()
                && e.get::<Stacking>()
                    .is_some_and(|s| s.top_layer != TopLayer::None)
                && !e.contains::<buiy_core::render::components::ComputedPaintSkip>()
        })
        .map(|e| e.id())
        .collect::<Vec<Entity>>();
    assert!(
        offender_count.contains(&occluder),
        "the injected node is a live top-layer candidate",
    );
}
