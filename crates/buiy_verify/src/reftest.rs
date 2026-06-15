//! Tier 4 — reftests + the CPU-vs-GPU SDF cross-check (reftests.md).
//!
//! A reftest renders a `test` and a `reference` scene with the SAME engine in
//! ONE process and asserts their bitmaps match (`==`) or differ (`!=`), never
//! against a stored baseline — so every platform-variance term (driver SDF
//! rounding, glyph-atlas AA, sRGB encode, clock) cancels in the diff. The
//! harness stores ZERO bytes. GPU-coupled cases are `#[ignore]`; the pairing /
//! aggregation logic and the independence lint are pure-CPU and gate headless.

/// Whether a [`RefCase`] passes on equality or on difference.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefKind {
    /// Pass iff `test` and `reference` render to the same bitmap within `fuzz`.
    Match,
    /// Pass iff they render DIFFERENTLY (a `!=` anti-test guards silent no-ops).
    Mismatch,
}

impl RefKind {
    /// Parse the `reftest!` macro's kind token (`stringify!($kind)`).
    /// Panics on any other token — the macro only ever passes these two.
    pub fn reftest_kind(token: &str) -> Self {
        match token {
            "match" => RefKind::Match,
            "mismatch" => RefKind::Mismatch,
            other => panic!("reftest! kind must be `match` or `mismatch`, got `{other}`"),
        }
    }
}

use crate::metric::{Diff, FuzzBudget};
use bevy::app::App;

/// One reftest pairing. `test` and `reference` each build a scene into a
/// fresh, deterministic `App` (spawn entities; do NOT drive frames —
/// `run_reftest` owns the capture loop). Co-locate the expectation with the
/// `#[test]` the `reftest!` macro generates.
pub struct RefCase {
    pub name: &'static str,
    pub kind: RefKind,
    /// Builds the scene exercising the feature under test.
    pub test: fn(&mut App),
    /// Builds the independent-oracle scene (see "Reference independence").
    pub reference: fn(&mut App),
    /// Per-pairing fuzz, à la Mozilla `fuzzy-if`. Default `(0,0)` once the
    /// determinism stack is in (determinism.md); widen with a documented reason.
    pub fuzz: FuzzBudget,
}

/// The result of running one [`RefCase`].
#[derive(Debug)]
pub struct RefOutcome {
    pub passed: bool,
    pub diff: Diff,
    /// On failure, a self-contained local HTML triage report (test | ref |
    /// diff). Path printed to stderr; never committed.
    pub report_path: Option<std::path::PathBuf>,
}

/// The pure pass-decision: `Match` passes iff the diff fits the budget;
/// `Mismatch` passes iff it does NOT (the feature must *do* something). Split
/// out of `run_reftest` so it gates headless via the aggregation truth table —
/// no GPU. The `(0,0)`-floor enforcement for `Mismatch` lives at macro
/// expansion time, so `evaluate_outcome` takes the budget as given.
pub fn evaluate_outcome(kind: RefKind, diff: &Diff, fuzz: &FuzzBudget) -> bool {
    match kind {
        RefKind::Match => diff.passes(fuzz),
        RefKind::Mismatch => !diff.passes(fuzz),
    }
}

use crate::metric::{CompareOpts, compare};
use buiy_core::render::golden::{GoldenConfig, capture_to_image};

/// The capture viewport for reftest pairings, in logical px. Both halves are
/// captured at this size in one app run; large enough that a single 40px box
/// and a 120px-shifted twin do not overlap (so a moved box is a real diff).
const REFTEST_LOGICAL: (u32, u32) = (200, 120);

/// Render BOTH scenes via the buiy_core capture seam in ONE app run and diff
/// with `metric::compare`. Platform variance cancels because both halves share
/// one `wgpu::Device`, driver, atlas, and virtual clock. GPU-coupled.
///
/// Until the determinism stack lands this builds the app via `reftest_app`
/// (the canonical `capture_app` seam); Phase 3 swaps that one line for
/// `DeterministicApp::build` with an identical `&mut App`→capture contract.
pub fn run_reftest(case: &RefCase) -> RefOutcome {
    assert!(
        mismatch_floor_ok(case.kind, &case.fuzz),
        "reftest `{}`: a Mismatch with a non-(0,0) fuzz floor is vacuous",
        case.name
    );
    let (w, h) = REFTEST_LOGICAL;
    let mut app = crate::support::reftest_app(w, h);
    let cfg = GoldenConfig::deterministic();

    let test_img = capture_to_image_with(&mut app, case.test, &cfg);
    let ref_img = capture_to_image_with(&mut app, case.reference, &cfg);

    let diff = compare(&test_img, &ref_img, &CompareOpts::reftest_default());
    let passed = evaluate_outcome(case.kind, &diff, &case.fuzz);
    let report_path = if passed {
        None
    } else {
        Some(emit_report(case.name, &test_img, &ref_img, &diff))
    };
    RefOutcome {
        passed,
        diff,
        report_path,
    }
}

/// Clear the previous scene, spawn `scene`, capture via the buiy_core seam.
fn capture_to_image_with(
    app: &mut bevy::app::App,
    scene: fn(&mut bevy::app::App),
    cfg: &GoldenConfig,
) -> image::RgbaImage {
    crate::support::clear_reftest_scene(app);
    scene(app);
    capture_to_image(app, cfg)
}

/// Write a self-contained HTML triage report (test | ref | diff) to a temp
/// path and return it. Phase 3 swaps this for the golden-tier emitter; until
/// then, a minimal three-PNG dump. Never committed.
fn emit_report(
    name: &str,
    test: &image::RgbaImage,
    reference: &image::RgbaImage,
    diff: &Diff,
) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("buiy-reftest");
    let _ = std::fs::create_dir_all(&dir);
    let base = dir.join(name);
    let _ = test.save(base.with_extension("test.png"));
    let _ = reference.save(base.with_extension("ref.png"));
    if let Some(img) = &diff.diff_image {
        let _ = img.save(base.with_extension("diff.png"));
    }
    let report = base.with_extension("html");
    let _ = std::fs::write(
        &report,
        format!(
            "<h1>reftest {name} FAILED</h1><p>differing_pixels={} max_channel_delta={}</p>\
             <img src='{name}.test.png'><img src='{name}.ref.png'><img src='{name}.diff.png'>",
            diff.differing_pixels, diff.max_channel_delta
        ),
    );
    eprintln!("reftest {name} report: {}", report.display());
    report
}

/// Render the same single primitive on the GPU (one-instance capture) and on
/// the CPU oracle, diff with the AA-aware metric. Tolerates sub-pixel AA noise
/// via `fuzz`; zero stored bytes. Catches SDF AA / implementation drift no
/// markup reftest can, and is kept PERMANENTLY (one shared analytic
/// `sdf_rounded_rect`). A *spec* error in `sdf_rounded_rect` is invisible here
/// (both paths share it) — that is Tier 5's job.
pub fn run_sdf_cross_check(draw: &buiy_core::render::DrawData, fuzz: &FuzzBudget) -> RefOutcome {
    let (w, h) = REFTEST_LOGICAL;
    let cfg = GoldenConfig::deterministic();

    let mut app = crate::support::reftest_app(w, h);
    crate::support::clear_reftest_scene(&mut app);
    spawn_single_primitive(&mut app, draw);
    let gpu = capture_to_image(&mut app, &cfg);

    let cpu = sdf_oracle::rasterize_sdf_rect(draw, w, h);

    let diff = compare(&gpu, &cpu, &CompareOpts::reftest_default());
    let passed = diff.passes(fuzz);
    let report_path = if passed {
        None
    } else {
        Some(emit_report("sdf_cross_check", &gpu, &cpu, &diff))
    };
    RefOutcome {
        passed,
        diff,
        report_path,
    }
}

/// Spawn one rounded-rect under a root, mapping `DrawData`'s position/size/
/// radius to the layout + render components the extract path turns back into one
/// `DrawData`. The corner radius is carried on `Border.radius`
/// (`Corners::all(Radius::circular(..))`) — that is the component
/// `draw_for_node` reads for the quad radius (`render/mod.rs:373`); a bare
/// `Radius` component is NOT consumed by the fill path. The `Border` band is
/// zero-width (width lives in `BoxModel`), so only the rounded fill paints.
fn spawn_single_primitive(app: &mut bevy::app::App, draw: &buiy_core::render::DrawData) {
    use bevy::prelude::*;
    use buiy_core::components::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::ColorToken;
    use buiy_core::render::components::{Background, Border, Corners, Radius};
    use std::borrow::Cow;
    // The capture path resolves a token; install draw.color under a fixed key.
    let key = "sdf.cross.fill";
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(key.into(), draw.color);
    }
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    left: Sizing::Length(Length::px(draw.position.x)),
                    top: Sizing::Length(Length::px(draw.position.y)),
                    ..default()
                })
                .width_px(draw.size.x)
                .height_px(draw.size.y),
            Background {
                color: ColorToken::Token(Cow::Borrowed(key)),
            },
            Border {
                radius: Corners::all(Radius::circular(draw.radius)),
                ..default()
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[e]);
}

/// A `Mismatch` budget that tolerates difference is meaningless — its floor
/// must be `(0,0)`. `Match` may carry any widening. Pure CPU so it gates
/// headless (reftests.md § Verification #2); the `reftest!` macro enforces the
/// same at expansion time, and `run_reftest` asserts it as a belt.
pub fn mismatch_floor_ok(kind: RefKind, fuzz: &FuzzBudget) -> bool {
    match kind {
        RefKind::Mismatch => *fuzz == FuzzBudget::EXACT,
        RefKind::Match => true,
    }
}

/// Pure-CPU per-pixel evaluation of the WGSL SDF + AA coverage step, the
/// golden-free oracle for SDF corner AA (Tier 4.5). The SDF formula is shared
/// 1:1 with `shader.wgsl:60` / `:76-:79` — the port and the shader must stay
/// identical, pinned by the point-probe test that re-derives the values
/// `tests/render_instance.rs:12` already asserts.
pub mod sdf_oracle {
    use bevy::math::Vec2;
    use buiy_core::render::DrawData;

    /// 1:1 CPU port of `shader.wgsl::sdf_rounded_rect`.
    pub fn sdf_rounded_rect(p: Vec2, half_size: Vec2, r: f32) -> f32 {
        let q = p.abs() - half_size + Vec2::splat(r);
        q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - r
    }

    /// Rasterize one `DrawData` rounded-rect into a `w×h` RGBA tile that matches
    /// the **capture output**, not just the fragment shader. It mirrors the full
    /// GPU chain so the cross-check compares like-for-like (`run_sdf_cross_check`
    /// captures the GPU box over the capture camera's opaque-black clear):
    ///
    /// 1. **SDF + AA** — the shared `sdf_rounded_rect` in logical px, AA via a
    ///    `fwidth` estimate (the per-pixel SDF gradient by central difference)
    ///    fed to `smoothstep(-aa, aa, d)` → straight-alpha `coverage`
    ///    (`shader.wgsl:60`/`:76-:79`).
    /// 2. **Linear-space SrcOver over opaque black** — the pipeline blends
    ///    `ALPHA_BLENDING` (SrcOver) in LINEAR space into the `Rgba8UnormSrgb`
    ///    target, and the capture camera clears to **opaque black**. So the
    ///    composite is `out_linear = src_linear · coverage` (the black backdrop
    ///    contributes nothing) with the result fully opaque (alpha 255) — the
    ///    same alpha the GPU readback carries everywhere, including OUTSIDE the
    ///    box (where coverage 0 → opaque black). Comparing a transparent CPU
    ///    backdrop against the GPU's opaque-black clear is exactly the
    ///    every-pixel alpha-255-vs-0 mismatch this composite removes.
    /// 3. **sRGB encode** — the target is `Rgba8UnormSrgb`, so the linear result
    ///    is sRGB-encoded on write (matched here via `Srgba::from(LinearRgba)`).
    pub fn rasterize_sdf_rect(draw: &DrawData, w: u32, h: u32) -> image::RgbaImage {
        let half = draw.size * 0.5;
        let center = draw.position + half;
        let r = draw.radius;
        // Source color in LINEAR space (the space the GPU blends in), with its
        // own straight alpha folded into the coverage below.
        let src_lin = bevy::color::LinearRgba::from(draw.color);
        let src_a = src_lin.alpha;

        let mut img = image::RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - center;
                let d = sdf_rounded_rect(p, half, r);
                let dx = (sdf_rounded_rect(p + Vec2::X, half, r)
                    - sdf_rounded_rect(p - Vec2::X, half, r))
                .abs()
                    * 0.5;
                let dy = (sdf_rounded_rect(p + Vec2::Y, half, r)
                    - sdf_rounded_rect(p - Vec2::Y, half, r))
                .abs()
                    * 0.5;
                let aa = (dx + dy).max(1e-4);
                let coverage = 1.0 - smoothstep(-aa, aa, d);
                // SrcOver over opaque black in LINEAR space: the black backdrop
                // (0,0,0,1) contributes nothing to RGB, and the result is opaque.
                let a_src = (src_a * coverage).clamp(0.0, 1.0);
                let out_lin = bevy::color::LinearRgba::new(
                    src_lin.red * a_src,
                    src_lin.green * a_src,
                    src_lin.blue * a_src,
                    1.0,
                );
                // sRGB-encode on write (Rgba8UnormSrgb target).
                let out = bevy::color::Srgba::from(out_lin);
                img.put_pixel(
                    x,
                    y,
                    image::Rgba([
                        (out.red * 255.0).round().clamp(0.0, 255.0) as u8,
                        (out.green * 255.0).round().clamp(0.0, 255.0) as u8,
                        (out.blue * 255.0).round().clamp(0.0, 255.0) as u8,
                        255,
                    ]),
                );
            }
        }
        img
    }

    /// `smoothstep` matching WGSL `smoothstep(edge0, edge1, x)`.
    fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

use bevy::prelude::World;

/// A structural marker the independence lint can query for in a built world.
/// Each variant maps to a `buiy_core` component (or a distinguishing field on
/// one) whose *presence* proves a reference re-used the feature under test.
/// Value-encoded features (`justify-content`, `direction`, `gap` — fields on a
/// shared `Style`) have NO marker here and fall to human review (see
/// [`assert_reference_independent`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComponentMarker {
    /// A `Containment` whose `content_visibility` is `Hidden`.
    ContentVisibilityHidden,
    /// Any `ContainerQuery` component.
    ContainerQuery,
    /// A `Stacking` whose `top_layer` is non-`None` (top-layer participation).
    /// `TopLayer` is a field on the `Stacking` component, not a component of its
    /// own, so the lint queries `Stacking` and checks the field — structurally
    /// equivalent to the `ContentVisibilityHidden`/`Containment` routing.
    TopLayer,
    /// Any `Translate` component.
    Translate,
}

impl ComponentMarker {
    /// True iff ANY entity in `world` carries this marker.
    fn present_in(self, world: &mut World) -> bool {
        use buiy_core::layout::{
            Containment, ContainerQuery, ContentVisibility, Stacking, TopLayer, Translate,
        };
        match self {
            ComponentMarker::ContentVisibilityHidden => world
                .query::<&Containment>()
                .iter(world)
                .any(|c| c.content_visibility == ContentVisibility::Hidden),
            ComponentMarker::ContainerQuery => {
                world.query::<&ContainerQuery>().iter(world).next().is_some()
            }
            ComponentMarker::TopLayer => world
                .query::<&Stacking>()
                .iter(world)
                .any(|s| s.top_layer != TopLayer::None),
            ComponentMarker::Translate => {
                world.query::<&Translate>().iter(world).next().is_some()
            }
        }
    }
}

/// What a reference scene is FORBIDDEN to contain, per feature under test.
pub struct IndependenceRule {
    pub feature: &'static str,
    pub forbidden_in_reference: &'static [ComponentMarker],
}

/// The registered marker rules for marker-bearing features. Value-encoded
/// features (flex `justify-content`, `direction`, `gap`) are deliberately
/// ABSENT — component-presence cannot distinguish them, so they fall to the
/// PR-time review checklist. A pairing whose feature has no rule here fails the
/// lint until a rule (or documented waiver) is added — independence is
/// opt-out-impossible by construction for marker features.
pub fn default_rules() -> Vec<IndependenceRule> {
    vec![
        IndependenceRule {
            feature: "content-visibility",
            forbidden_in_reference: &[ComponentMarker::ContentVisibilityHidden],
        },
        IndependenceRule {
            feature: "@container",
            forbidden_in_reference: &[ComponentMarker::ContainerQuery],
        },
        IndependenceRule {
            feature: "top-layer",
            forbidden_in_reference: &[ComponentMarker::TopLayer],
        },
        IndependenceRule {
            feature: "translate",
            forbidden_in_reference: &[ComponentMarker::Translate],
        },
    ]
}

/// Assert the case's `reference` scene carries NONE of the marker components a
/// rule forbids. Builds the reference into a headless **no-GPU** `App` (layout
/// types registered, no render plugins) and queries the built world. Panics
/// naming the feature + marker on violation.
///
/// **Limit — value-encoded features fall to human review.** Features that are
/// field *values* on a shared `Style`/`Node` (`justify-content`, `direction`,
/// `gap`) have no distinct marker, so this lint cannot see them; mechanism 1
/// (route the reference through the primitive literal-`Node` layer) keeps THOSE
/// independent, and the PR-time checklist enforces it. This backstops only
/// marker-bearing features.
pub fn assert_reference_independent(case: &RefCase, rules: &[IndependenceRule]) {
    let mut app = bevy::app::App::new();
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    (case.reference)(&mut app);
    let world = app.world_mut();
    for rule in rules {
        for &marker in rule.forbidden_in_reference {
            assert!(
                !marker.present_in(world),
                "reference for `{}` illegally contains {:?} — it re-uses the \
                 feature under test, so the comparison would pass vacuously \
                 (reftests.md § Reference independence)",
                rule.feature,
                marker
            );
        }
    }
}

/// Generate one `#[test] #[ignore]` per reftest pairing — keeps each case at
/// the unit/integration tier under the existing `cargo test -- --ignored` GPU
/// lane, no new CI infra, no manifest file (the type system IS the manifest).
///
/// ```ignore
/// reftest!(match,    flex_justify_end, flex_test, literal_offsets_ref);
/// reftest!(mismatch, cv_hidden_hides,  cv_visible, cv_hidden);
/// reftest!(match,    transform_xy,     xfm_test,   literal_ref, fuzz = (1, 8));
/// ```
///
/// A non-`(0,0)` fuzz floor on a `mismatch` fails to COMPILE (a `const`
/// assertion), not at runtime — reftests.md § Verification #2.
#[macro_export]
macro_rules! reftest {
    // mismatch with explicit fuzz → compile-time reject of a non-zero floor.
    (mismatch, $fn:ident, $test:path, $reference:path, fuzz = ($d:literal, $p:literal)) => {
        const _: () = assert!(
            $d == 0 && $p == 0,
            concat!(
                "reftest mismatch `",
                stringify!($fn),
                "`: a non-(0,0) fuzz floor is vacuous"
            ),
        );
        $crate::reftest!(@gen mismatch, $fn, $test, $reference, ($d, $p));
    };
    // match with explicit fuzz.
    (match, $fn:ident, $test:path, $reference:path, fuzz = ($d:literal, $p:literal)) => {
        $crate::reftest!(@gen match, $fn, $test, $reference, ($d, $p));
    };
    // no explicit fuzz → (0,0) for either kind.
    ($kind:ident, $fn:ident, $test:path, $reference:path) => {
        $crate::reftest!(@gen $kind, $fn, $test, $reference, (0, 0));
    };
    // internal: emit the #[ignore] test named $fn.
    (@gen $kind:ident, $fn:ident, $test:path, $reference:path, ($d:literal, $p:literal)) => {
        #[test]
        #[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
        fn $fn() {
            let case = $crate::reftest::RefCase {
                name: stringify!($fn),
                kind: $crate::reftest::RefKind::reftest_kind(stringify!($kind)),
                test: $test,
                reference: $reference,
                fuzz: $crate::metric::FuzzBudget {
                    max_channel_delta: $d,
                    max_diff_pixels: $p,
                },
            };
            let outcome = $crate::reftest::run_reftest(&case);
            assert!(
                outcome.passed,
                "reftest {} failed: {:?} (report: {:?})",
                stringify!($fn),
                outcome.diff,
                outcome.report_path
            );
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reftest_kind_parses_both_tokens() {
        assert_eq!(RefKind::reftest_kind("match"), RefKind::Match);
        assert_eq!(RefKind::reftest_kind("mismatch"), RefKind::Mismatch);
    }

    #[test]
    #[should_panic(expected = "must be `match` or `mismatch`")]
    fn reftest_kind_rejects_garbage() {
        let _ = RefKind::reftest_kind("nope");
    }

    #[test]
    fn refcase_is_constructible_with_zero_fuzz_default() {
        use crate::metric::FuzzBudget;
        use bevy::app::App;
        fn noop(_: &mut App) {}
        let case = RefCase {
            name: "constructs",
            kind: RefKind::Match,
            test: noop,
            reference: noop,
            fuzz: FuzzBudget::EXACT,
        };
        assert_eq!(case.name, "constructs");
        assert_eq!(case.fuzz, FuzzBudget::EXACT);
    }

    use crate::metric::Diff;

    /// A stub Diff with `n` differing pixels and `max_channel_delta = d`, no MSSIM.
    fn stub_diff(n: u32, d: u8) -> Diff {
        Diff {
            differing_pixels: n,
            max_channel_delta: d,
            total_pixels: 1024,
            mssim: None,
            diff_image: None,
            saturated: false,
        }
    }

    #[test]
    fn match_passes_within_fuzz_fails_outside() {
        assert!(evaluate_outcome(
            RefKind::Match,
            &stub_diff(0, 0),
            &FuzzBudget::EXACT
        ));
        assert!(!evaluate_outcome(
            RefKind::Match,
            &stub_diff(1, 200),
            &FuzzBudget::EXACT
        ));
        assert!(evaluate_outcome(
            RefKind::Match,
            &stub_diff(1, 8),
            &FuzzBudget {
                max_channel_delta: 8,
                max_diff_pixels: 1
            }
        ));
    }

    #[test]
    fn mismatch_passes_outside_fuzz_fails_within() {
        assert!(evaluate_outcome(
            RefKind::Mismatch,
            &stub_diff(50, 200),
            &FuzzBudget::EXACT
        ));
        // A scene that did NOT change (zero diff) FAILS a mismatch — the no-op guard.
        assert!(!evaluate_outcome(
            RefKind::Mismatch,
            &stub_diff(0, 0),
            &FuzzBudget::EXACT
        ));
    }

    #[test]
    fn mismatch_requires_zero_fuzz_floor() {
        assert!(mismatch_floor_ok(RefKind::Mismatch, &FuzzBudget::EXACT));
        assert!(!mismatch_floor_ok(
            RefKind::Mismatch,
            &FuzzBudget {
                max_channel_delta: 1,
                max_diff_pixels: 0
            }
        ));
        assert!(!mismatch_floor_ok(
            RefKind::Mismatch,
            &FuzzBudget {
                max_channel_delta: 0,
                max_diff_pixels: 1
            }
        ));
        // Match may carry any budget.
        assert!(mismatch_floor_ok(
            RefKind::Match,
            &FuzzBudget {
                max_channel_delta: 8,
                max_diff_pixels: 4
            }
        ));
    }
}
