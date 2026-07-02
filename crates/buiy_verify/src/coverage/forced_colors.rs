//! Gate #11 live-catalog producer (coverage.md § "Wiring
//! `forced_colors_analyzer` to the live catalog").
//!
//! The gate-#11 analyzers ([`analyze_forced_colors`](buiy_core::render::forced_colors_analyzer::analyze_forced_colors) check (a),
//! [`analyze_shadow_only`](buiy_core::render::forced_colors_analyzer::analyze_shadow_only) check (b)) are unchanged — they still consume the
//! existing [`CatalogPaint`] descriptor. What moves is the **input source**:
//! instead of hand-built descriptors
//! (`buiy_core/tests/render_forced_colors_analyzer.rs`), [`live_catalog_paint`]
//! derives `CatalogPaint` from the **live spawned components**
//! (`Background` / `Border` / `Outline`) of the same fixture corpus every other
//! tier enrolls. Because it reads the same fixtures, gate #11 auto-enrolls every
//! new widget by construction (follow-ups.md:462–481, now closed for the
//! token-flow half).
//!
//! ## Boundary (honest, documented)
//!
//! The live default [`Button::new`](buiy_widgets::Button::new) paints a *brand*
//! token (`color.surface.secondary`) that is **not** forced-colors-safe — under
//! `forced_colors: active` it would resolve to the magenta sentinel, a genuine
//! `NonSystemColor` violation (color-and-forced-colors.md § 3.1). Making the
//! *default widget* forced-colors-safe is owned by `buiy-widget-catalog-design`,
//! not this campaign. The catalog fixtures therefore author the
//! forced-colors-safe paint the catalog must converge to (system-color tokens),
//! and this producer reads those LIVE components — proving it observes real
//! paint, not a stale descriptor (the `broken_fixture_produces_violation`
//! self-test gives that teeth).
//!
//! ## Residual visual half — LANDED
//!
//! The forced-colors *visual* residual — the `BoxShadow` draw-skip under
//! `forced-colors: active` — is a Tier-4 reftest, `forced_colors_boxshadow_suppressed`
//! in `tests/coverage_forced_colors.rs` (the `BoxShadow` extract/draw path landed:
//! `resolve_shadows` is wired in `extract.rs` and returns empty under
//! `forced_colors`, and `shadow.wgsl` rasterizes). It renders a shadowed vs an
//! unshadowed box under forced-colors and asserts they match byte-for-byte
//! (proven non-vacuous — disabling the suppression reds it). It is NOT this
//! producer's concern; the structured token-flow + shadow-only analyzers here
//! cover gate #11's static half independently.

use bevy::prelude::*;

use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, Border, BoxShadow, LineStyle, Outline};
use buiy_core::render::forced_colors_analyzer::CatalogPaint;

use super::fixture::{Fixture, sorted_catalog};
use super::matrix::{Cell, ThemeAxis, Viewport};

/// Walk the live catalog: for each fixture build a minimal app, query the
/// spawned `Background` / `Border` / `Outline` off the `Name`-tagged root, and
/// project them into the existing [`CatalogPaint`]. The analyzers run unchanged
/// over the result.
///
/// One `CatalogPaint` per fixture (one widget × state). The
/// `has_shadow_only_state_delta` flag is computed across a widget's states: a
/// state whose ONLY paint difference from the widget's resting state is its
/// `BoxShadow` is a shadow-only affordance (check (b)). With a single resting
/// state in the corpus there is no such delta, so the flag is `false`; it
/// activates by construction when hover / focus fixtures land.
pub fn live_catalog_paint() -> Vec<CatalogPaint> {
    paint_for_fixtures(&sorted_catalog())
}

/// The core producer over an explicit fixture slice — the seam the
/// `broken_fixture_produces_violation` self-test drives with a `#[cfg(test)]`
/// fixture excluded from the real [`catalog`](super::fixture::catalog).
pub fn paint_for_fixtures(fixtures: &[&'static Fixture]) -> Vec<CatalogPaint> {
    // Per widget, the resting-state paint signature, so non-resting states can
    // be compared against it for the shadow-only-delta check.
    let mut by_widget: std::collections::HashMap<&'static str, PaintProbe> =
        std::collections::HashMap::new();

    // First pass: probe every fixture's live paint.
    let probes: Vec<(&'static Fixture, PaintProbe)> =
        fixtures.iter().map(|&fx| (fx, probe_fixture(fx))).collect();

    // Record each widget's resting signature (the baseline for the delta check).
    for (fx, probe) in &probes {
        if fx.state == "resting" {
            by_widget.insert(fx.name, probe.clone());
        }
    }

    // Second pass: build a CatalogPaint per fixture, computing the
    // shadow-only-delta against the widget's resting baseline.
    probes
        .into_iter()
        .map(|(fx, probe)| {
            let shadow_only = match by_widget.get(fx.name) {
                Some(resting) if fx.state != "resting" => probe.differs_only_in_shadow(resting),
                _ => false,
            };
            CatalogPaint {
                widget: fx.name,
                state: fx.state,
                background: probe.background,
                border: probe.border,
                outline: probe.outline,
                has_shadow_only_state_delta: shadow_only,
            }
        })
        .collect()
}

/// The live paint signature probed off one fixture's `Name`-tagged root.
#[derive(Clone, Debug)]
struct PaintProbe {
    background: ColorToken,
    border: ColorToken,
    outline: ColorToken,
    has_shadow: bool,
}

impl PaintProbe {
    /// True iff `self` differs from `resting` ONLY in its `BoxShadow` presence —
    /// i.e. the three painted colors match but the shadow flag flipped. Such a
    /// state is invisible once shadows are suppressed under forced colors.
    fn differs_only_in_shadow(&self, resting: &PaintProbe) -> bool {
        self.background == resting.background
            && self.border == resting.border
            && self.outline == resting.outline
            && self.has_shadow != resting.has_shadow
    }
}

/// Build the fixture's app, run one update so the bundle settles, then read the
/// live paint off the `Name`-tagged root.
fn probe_fixture(fx: &Fixture) -> PaintProbe {
    // A cheap app: just enough to spawn the fixture and read its components.
    // The fixture spawns a `Camera2d` + the `Name`-tagged root; we never run
    // layout/render — only inspect the authored paint components.
    let cell = probe_cell();
    let mut app = super::enroll::build_app(fx, &cell);
    app.update();

    let world = app.world_mut();
    let mut q = world.query::<(
        &Name,
        Option<&Background>,
        Option<&Border>,
        Option<&Outline>,
        Option<&BoxShadow>,
    )>();
    // The root carries the fixture's `name`; pick that entity (ignore the camera
    // and any unnamed children).
    for (name, bg, border, outline, shadow) in q.iter(world) {
        if name.as_str() == fx.name {
            return PaintProbe {
                background: bg
                    .map(|b| b.color.clone())
                    .unwrap_or(ColorToken::Transparent),
                border: border.map(border_token).unwrap_or(ColorToken::Transparent),
                outline: outline
                    .map(|o| o.color.clone())
                    .unwrap_or(ColorToken::Transparent),
                has_shadow: shadow.map(|s| !s.0.is_empty()).unwrap_or(false),
            };
        }
    }
    // A fixture must `Name`-tag its root with `fx.name`; missing it is an
    // authoring bug, surfaced loudly rather than silently passing.
    panic!(
        "fixture `{}`/`{}` did not spawn a root tagged Name(\"{}\") — every fixture must Name-tag its root",
        fx.name, fx.state, fx.name
    );
}

/// Collapse a `Border`'s four sides to one representative paint token for the
/// analyzer: the first side that actually paints (a non-`None` line style),
/// else `Transparent`. A uniform border (the common case) makes every side
/// equal, so the choice is unambiguous.
fn border_token(border: &Border) -> ColorToken {
    for side in [&border.top, &border.right, &border.bottom, &border.left] {
        if !matches!(side.style, LineStyle::None) {
            return side.color.clone();
        }
    }
    ColorToken::Transparent
}

/// A fixed cell for the paint probe: the forced-colors mode is irrelevant to
/// reading the *authored* token (the analyzer applies the forced theme itself),
/// so use a small light-theme phone cell. Pure-CPU.
fn probe_cell() -> Cell {
    Cell {
        theme: ThemeAxis::Light,
        viewport: Viewport {
            w: 360,
            h: 640,
            key: "phone",
        },
        forced_colors: false,
        dpr: buiy_core::render::golden::Dpr::X1,
    }
}
