//! HiDPI / device-scale-factor layout proxy (Dooduel F9).
//!
//! The prototype's top mobile residual (PROTO1 journal, W7): on wasm at
//! `devicePixelRatio > 1` the whole UI rendered ~dpr× too large and overflowed
//! the viewport. The FINAL spec (§2.10, verify-review finding M4) prescribes a
//! **headless CI proxy FIRST**: construct the layout world at `scale_factor = 2`
//! and assert the resolved geometry fits the LOGICAL viewport (no overflow),
//! de-risking a fix before touching a browser.
//!
//! Layout is defined entirely in LOGICAL pixels — the window feeds its logical
//! size (`resolution.size()` = physical / scale_factor) as the available space,
//! and `ResolvedLayout` stores logical px. Device scale factor therefore must
//! NEVER change a single resolved box; it enters only at the render boundary
//! (the view uniform / physical framebuffer). These tests pin that invariant two
//! ways:
//!
//!  1. [`hidpi_layout_fits_logical_viewport_at_dsf2`] — the spec's literal ask:
//!     the shell fixture fits the logical viewport at `scale_factor = 2`.
//!  2. [`hidpi_layout_is_scale_factor_invariant`] — the sharp guard: the FULL
//!     `ResolvedLayout` tree is byte-identical across `scale_factor` 1 / 2 / 3.
//!     A future `* scale_factor` slip in the window-size read or the
//!     layout->`ResolvedLayout` path fails here, headless, with no browser or GPU.
//!
//! FINDING (F9 investigation): this proxy PASSES — the core layout+scale path is
//! already scale-invariant (as `render::golden::capture_app_scaled` goldens at
//! multiple DPRs also demonstrate). The reported browser "2× too large" turned
//! out to be a headless-Chromium dpr-EMULATION ARTIFACT (inconsistent
//! `devicePixelRatio` vs device-pixel-content-box signals), NOT a real-device
//! bug: winit derives `logical = physical / scale_factor`, and on a real device
//! those signals always agree, so `logical == CSS size`. This test remains the
//! per-wave regression guard for the invariant that must never regress.
//! See `docs/reports/2026-07-04-wasm-hidpi-investigation.md`.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use buiy_core::CorePlugin;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{LayoutPlugin, Length, Sizing, Style};

/// A phone-sized LOGICAL viewport — the prototype's mobile target (journal W7,
/// `is_mobile`/`card_w` read the logical 390/412 width).
const LOGICAL_W: f32 = 390.0;
const LOGICAL_H: f32 = 844.0;

/// Sub-pixel tolerance for the fits-viewport bound (Taffy rounds at integer
/// device px; at dsf != 1 a logical edge can land a fraction off an integer).
const EPS: f32 = 0.5;

/// The entities of the shell fixture, in nesting order.
struct Shell {
    root: Entity,
    card: Entity,
    content: Entity,
}

/// Build the layout world at `scale_factor`, with a `PrimaryWindow` whose
/// LOGICAL size is `LOGICAL_W × LOGICAL_H`. `WindowResolution::new` takes
/// PHYSICAL units, so pass `logical × scale` + the override — the same builder
/// shape `render::golden::capture_app_scaled` uses, so `resolution.size()` reads
/// back the logical size the layout available-space + view uniform are built
/// from.
fn hidpi_app(scale_factor: f32) -> App {
    let resolution = WindowResolution::new(
        (LOGICAL_W * scale_factor).round() as u32,
        (LOGICAL_H * scale_factor).round() as u32,
    )
    .with_scale_factor_override(scale_factor);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin);
    app.world_mut().spawn((
        Window {
            resolution,
            ..default()
        },
        PrimaryWindow,
    ));
    app
}

/// The prototype's overflowing mobile shell, reduced to its load-bearing shape:
/// a viewport-filling root with `Space::Lg`-ish padding holding a full-width
/// card that itself holds a full-width content row. Each level is `100%` wide,
/// so a dpr-scaled available space would blow the whole nest past the viewport —
/// exactly the W7 overflow.
fn spawn_shell(app: &mut App) -> Shell {
    let percent100 = || Sizing::Length(Length::Percent(100.0));

    let root = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .border_box()
                .width(percent100())
                .height(percent100())
                .padding(24.0),
        ))
        .id();
    let card = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .border_box()
                .width(percent100())
                .height_px(220.0)
                .padding(16.0),
        ))
        .id();
    let content = app
        .world_mut()
        .spawn((Node, Style::default().width(percent100()).height_px(48.0)))
        .id();

    app.world_mut().entity_mut(card).add_children(&[content]);
    app.world_mut().entity_mut(root).add_children(&[card]);
    Shell {
        root,
        card,
        content,
    }
}

/// Drive the static tree to a layout fixed point (spawn frame + steady frames).
fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}

fn resolved(app: &App, e: Entity) -> ResolvedLayout {
    app.world()
        .get::<ResolvedLayout>(e)
        .expect("entity has ResolvedLayout after settle")
        .clone()
}

#[test]
fn hidpi_layout_fits_logical_viewport_at_dsf2() {
    let mut app = hidpi_app(2.0);
    let shell = spawn_shell(&mut app);
    settle(&mut app);

    let root = resolved(&app, shell.root);
    let card = resolved(&app, shell.card);
    let content = resolved(&app, shell.content);

    // The window feeds its LOGICAL size as the root's available space: a root
    // sized `100% × 100%` resolves to the logical viewport, NOT the physical
    // (dpr-scaled) pixel grid. This is the assertion that would fire dpr× large
    // if the window-size read leaked physical px into layout.
    assert!(
        (root.size.x - LOGICAL_W).abs() <= EPS && (root.size.y - LOGICAL_H).abs() <= EPS,
        "root should fill the LOGICAL viewport {LOGICAL_W}×{LOGICAL_H} at dsf=2, got {:?} \
         — a dpr-scaled root means physical px leaked into the layout available space",
        root.size,
    );

    // Every box fits inside its parent (position >= 0, position+size <= parent
    // size). With the root == the viewport, this transitively proves the whole
    // nest fits the logical viewport — no top/right clip.
    for (child, parent, name) in [(&card, &root, "card"), (&content, &card, "content")] {
        assert!(
            child.position.x >= -EPS && child.position.y >= -EPS,
            "{name} has a negative parent-relative origin {:?} at dsf=2",
            child.position,
        );
        let far = child.position + child.size;
        assert!(
            far.x <= parent.size.x + EPS && far.y <= parent.size.y + EPS,
            "{name} overflows its parent at dsf=2: box {:?}..{:?} exceeds parent {:?} \
             — HiDPI layout is being scaled by dpr",
            child.position,
            far,
            parent.size,
        );
    }
}

#[test]
fn hidpi_layout_is_scale_factor_invariant() {
    // The reference tree at dsf=1.
    let mut base = hidpi_app(1.0);
    let base_shell = spawn_shell(&mut base);
    settle(&mut base);
    let base_boxes = [
        resolved(&base, base_shell.root),
        resolved(&base, base_shell.card),
        resolved(&base, base_shell.content),
    ];

    // The SAME tree at dsf 2 and 3 must resolve byte-identically: device scale
    // factor is a render-boundary concern, invisible to logical-px layout.
    for dsf in [2.0_f32, 3.0] {
        let mut app = hidpi_app(dsf);
        let shell = spawn_shell(&mut app);
        settle(&mut app);
        let boxes = [
            resolved(&app, shell.root),
            resolved(&app, shell.card),
            resolved(&app, shell.content),
        ];
        for (name, base_b, b) in [
            ("root", &base_boxes[0], &boxes[0]),
            ("card", &base_boxes[1], &boxes[1]),
            ("content", &base_boxes[2], &boxes[2]),
        ] {
            // ResolvedLayout is logical-px position + size; compare both. (The
            // struct is not PartialEq — comparing its two Vec2 fields is the
            // equivalent, and keeps this a test-only concern.)
            assert_eq!(
                (b.position, b.size),
                (base_b.position, base_b.size),
                "{name} ResolvedLayout differs at dsf={dsf} vs dsf=1 — layout is NOT \
                 scale-invariant (a `* scale_factor` slipped into the window-size read \
                 or the layout->ResolvedLayout path)",
            );
        }
    }
}
