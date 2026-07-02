//! Gate #11 live-catalog scan (coverage.md § "Wiring `forced_colors_analyzer`
//! to the live catalog", Task 4.6). Pure-CPU, headless.
//!
//! The gate-#11 analyzers (`analyze_forced_colors` check (a),
//! `analyze_shadow_only` check (b)) are unchanged; what these tests prove is
//! that the **input source** now comes from the LIVE spawned components of the
//! fixture corpus (`live_catalog_paint`) instead of hand-built `CatalogPaint`
//! descriptors. The teeth test (`broken_fixture_produces_violation`) confirms
//! the producer observes real paint, not a stale descriptor.

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::color::{ColorToken, SystemColorKeyword};
use buiy_core::render::components::{Background, Border, BorderSide, BoxShadow, LineStyle, Shadow};
use buiy_core::render::forced_colors_analyzer::{
    ForcedColorsViolation, analyze_forced_colors, analyze_shadow_only,
};
use buiy_core::theme::{UserPreferences, forced_colors_theme};
use buiy_verify::coverage::{Fixture, live_catalog_paint, paint_for_fixtures};

/// The production scan: every fixture in the real catalog, derived from its LIVE
/// spawned `Background`/`Border`/`Outline`, must pass both gate-#11 checks under
/// the forced-colors theme. This is the live-catalog half of gate #11 — it
/// auto-enrolls every new widget by construction (it reads the same corpus every
/// other tier enrolls).
#[test]
fn live_catalog_has_no_forced_colors_violations() {
    let catalog = live_catalog_paint();
    assert!(
        !catalog.is_empty(),
        "the live catalog must derive paint from at least one fixture"
    );
    let theme = forced_colors_theme();

    let a = analyze_forced_colors(&catalog, &theme);
    assert!(
        a.is_empty(),
        "check (a): every live-catalog paint token must resolve in the \
         system-color set under forced-colors, got: {a:?}"
    );
    let b = analyze_shadow_only(&catalog);
    assert!(
        b.is_empty(),
        "check (b): no live-catalog state may convey its affordance with a \
         shadow alone, got: {b:?}"
    );
}

// ---------------------------------------------------------------------------
// Teeth: a deliberately-broken `#[cfg(test)]`-only fixture painting a BRAND
// token under forced-colors MUST produce a `NonSystemColor` violation through
// `paint_for_fixtures` — proving the producer reads REAL paint off the live
// tree, not a stale hand-built descriptor (the failure mode the re-pointing
// fixes). It is NOT registered with `fixture!`, so it never enters the real
// `catalog()` and never reds the production gate above.
// ---------------------------------------------------------------------------

/// The broken fixture's spawn: a `Name`-tagged root painting a BRAND token
/// (`color.accent`) — absent from the forced-colors system-color map, so it
/// resolves to the magenta sentinel under the forced theme (a violation).
fn spawn_broken_brand_widget(app: &mut App) {
    app.world_mut().spawn(Camera2d);
    app.world_mut().spawn((
        Name::new("brand-badge"),
        Background {
            color: ColorToken::Accent,
        },
        Border {
            top: BorderSide {
                color: ColorToken::SystemColor(SystemColorKeyword::ButtonBorder),
                style: LineStyle::Solid,
            },
            ..Default::default()
        },
    ));
}

static BROKEN_FIXTURE: Fixture = Fixture {
    name: "brand-badge",
    state: "resting",
    spawn: spawn_broken_brand_widget,
    paints_cell: None,
};

#[test]
fn broken_fixture_produces_violation() {
    // Drive the producer over ONLY the broken fixture (excluded from the real
    // catalog). It must observe the live brand-token paint and flag it.
    let fixtures: Vec<&'static Fixture> = vec![&BROKEN_FIXTURE];
    let catalog = paint_for_fixtures(&fixtures);
    assert_eq!(catalog.len(), 1, "one fixture → one CatalogPaint");

    let theme = forced_colors_theme();
    let report = analyze_forced_colors(&catalog, &theme);
    assert_eq!(
        report.len(),
        1,
        "the brand token must produce exactly one NonSystemColor violation, got: {report:?}"
    );
    assert!(
        matches!(
            report[0],
            ForcedColorsViolation::NonSystemColor {
                widget: "brand-badge",
                field: "background",
                ..
            }
        ),
        "the violation must name the live brand-token background, got: {:?}",
        report[0]
    );
}

/// Companion to the teeth test: prove the producer's pass result is not vacuous
/// — a fixture painting ONLY system-color tokens passes, so the broken-fixture
/// failure above is signal, not a producer that always reports violations.
fn spawn_safe_system_widget(app: &mut App) {
    app.world_mut().spawn(Camera2d);
    app.world_mut().spawn((
        Name::new("safe-badge"),
        Background {
            color: ColorToken::SystemColor(SystemColorKeyword::ButtonText),
        },
    ));
}

static SAFE_FIXTURE: Fixture = Fixture {
    name: "safe-badge",
    state: "resting",
    spawn: spawn_safe_system_widget,
    paints_cell: None,
};

#[test]
fn safe_fixture_produces_no_violation() {
    let fixtures: Vec<&'static Fixture> = vec![&SAFE_FIXTURE];
    let catalog = paint_for_fixtures(&fixtures);
    assert!(
        analyze_forced_colors(&catalog, &forced_colors_theme()).is_empty(),
        "a system-color-only fixture must pass — proves the producer is not \
         a constant-violation function"
    );
}

// ---------------------------------------------------------------------------
// Forced-colors `BoxShadow` *visual* reftest (Tier-4) — the residual visual half.
// ---------------------------------------------------------------------------
//
// The `BoxShadow` extract/draw path landed (`resolve_shadows` is wired in
// `extract.rs`; `shadow.wgsl` rasterizes), and forced-colors SUPPRESSES it —
// `resolve_shadows` returns an empty vec when `prefs.forced_colors` (the
// draw-skip). This reftest renders two scenes in one app: a bordered box WITH a
// `BoxShadow` and the SAME box WITHOUT one, both under `forced-colors: active`.
// Because the shadow is suppressed, the shadowed box must rasterize BYTE-FOR-BYTE
// like the unshadowed box — a `match`. (The draw-skip is also observed headless
// at Tier 1 (unit) and Tier 2 (display-list); this Tier-4 pixel confirmation
// completes the forced-colors MODE cell and replaces the old assertion-free
// `#[ignore]` placeholder — a real test that CAN fail, not a vacuous green:
// disabling the suppression reds it with 566 differing shadow pixels.)

/// The box both scenes share — a 30×30 forced-colors-SAFE bordered box at
/// (24,24), under `forced-colors: active`. System-color tokens
/// (`ButtonText`/`ButtonBorder`) resolve against the installed `forced_colors_theme`,
/// so the box paints a real color rather than the magenta missing-token sentinel.
fn forced_box(app: &mut App) -> Entity {
    app.insert_resource(forced_colors_theme());
    let mut prefs = UserPreferences::default();
    prefs.forced_colors = true;
    app.insert_resource(prefs);
    let side = || BorderSide {
        color: ColorToken::SystemColor(SystemColorKeyword::ButtonBorder),
        style: LineStyle::Solid,
    };
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(24.0)),
                    left: Sizing::Length(Length::px(24.0)),
                    ..default()
                })
                .width_px(30.0)
                .height_px(30.0),
            Background {
                color: ColorToken::SystemColor(SystemColorKeyword::ButtonText),
            },
            Border {
                left: side(),
                right: side(),
                top: side(),
                bottom: side(),
                ..default()
            },
        ))
        .id()
}

/// TEST scene: the box carrying a `BoxShadow` (offset +16,+16, blur 6), under
/// forced-colors. The draw-skip means the shadow is NOT painted, so this must
/// rasterize identically to `plain_box_forced`.
fn shadowed_box_forced(app: &mut App) {
    let w = forced_box(app);
    app.world_mut().entity_mut(w).insert(BoxShadow(vec![Shadow {
        color: ColorToken::SystemColor(SystemColorKeyword::ButtonText),
        offset_x: Length::px(16.0),
        offset_y: Length::px(16.0),
        blur: Length::px(6.0),
        spread: Length::px(0.0),
        inset: false,
    }]));
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[w]);
}

/// REFERENCE scene: the same box with NO `BoxShadow`.
fn plain_box_forced(app: &mut App) {
    let w = forced_box(app);
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[w]);
}

buiy_verify::reftest!(
    match,
    forced_colors_boxshadow_suppressed,
    shadowed_box_forced,
    plain_box_forced
);
