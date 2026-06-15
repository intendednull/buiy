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
use buiy_core::render::color::{ColorToken, SystemColorKeyword};
use buiy_core::render::components::{Background, Border, BorderSide, LineStyle};
use buiy_core::render::forced_colors_analyzer::{
    ForcedColorsViolation, analyze_forced_colors, analyze_shadow_only,
};
use buiy_core::theme::forced_colors_theme;
use buiy_verify::coverage::{Fixture, live_catalog_paint, paint_for_fixtures};
use std::borrow::Cow;

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
            color: ColorToken::Token(Cow::Borrowed("color.accent")),
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
// BLOCKED — forced-colors `BoxShadow` *visual* reftest.
// ---------------------------------------------------------------------------

/// The residual forced-colors *visual* half — the `BoxShadow` draw-skip under
/// `forced-colors: active` — is a Tier-4 reftest **blocked on the unlanded
/// `BoxShadow` extract/draw path** (`extract_buiy_nodes` has no `BoxShadow`
/// branch yet; follow-ups.md:474–478). It is intentionally NOT authored as a
/// runnable test: there is no draw path to assert against, and faking it green
/// would be a stale-positive. The structured `analyze_forced_colors` /
/// `analyze_shadow_only` scan above covers gate #11's static half now and does
/// not depend on it.
///
/// This `#[ignore]`d placeholder documents the dependency so the follow-up is
/// discoverable from the test suite (`cargo test -- --ignored` lists it with
/// its reason); it asserts nothing and must stay ignored until the `BoxShadow`
/// pipeline lands.
#[test]
#[ignore = "BLOCKED on the unlanded BoxShadow extract/draw path (follow-ups.md:474-478); \
            do not author a runnable assertion until extract_buiy_nodes has a BoxShadow branch"]
fn boxshadow_visual_reftest_is_blocked() {
    // Intentionally empty: tracked-but-blocked. See the doc comment.
}
