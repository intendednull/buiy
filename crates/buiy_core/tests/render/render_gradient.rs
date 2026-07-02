//! Headless (no GPU): the parity Wave B1 gradient EXTRACT tier. A
//! `linear-gradient(150deg, --ac, --ac2)` `BackgroundLayers` layer resolves its
//! stop tokens to concrete linear colors and packs the CSS angle into a y-down
//! gradient axis + the gradient-line length — the CI-safe regression guard for
//! the gradient pipeline (no adapter, no pixels).
//!
//! The GPU readback complement (the actual gradient PAINTS, corner pixels match)
//! is `render_gradient_gpu.rs` (`#[ignore]`, real adapter / lavapipe).

use bevy::prelude::*;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{BackgroundLayer, BackgroundLayers, ColorStop, LinearGradient};
use buiy_core::render::extract::{linear_axis, resolve_gradients};
use buiy_core::render::instance::{GRADIENT_KIND_LINEAR, GRADIENT_KIND_RADIAL};
use buiy_core::theme::{Theme, default_dark_theme};

/// A theme carrying the design's blue accent ramp (`--ac` / `--ac2`). The dark
/// palette's `Accent` / `AccentLighter` resolve to the exact `#5b86f5` / `#7fa1f7`
/// this suite asserts (theme.rs byte-identical guard), so it is the fixture.
fn accent_theme() -> Theme {
    default_dark_theme()
}

fn linear_layers(angle_deg: f32) -> BackgroundLayers {
    BackgroundLayers(vec![BackgroundLayer::Linear(LinearGradient {
        angle_deg,
        stops: vec![
            ColorStop {
                color: ColorToken::Accent,
                position: 0.0,
            },
            ColorStop {
                color: ColorToken::AccentLighter,
                position: 1.0,
            },
        ],
    })])
}

/// `linear_axis` maps the CSS angle (0deg = up, clockwise) to a y-DOWN unit
/// axis `(sinθ, -cosθ)` and the gradient-line length `|W·sinθ| + |H·cosθ|`.
#[test]
fn linear_axis_maps_css_angle_to_y_down_axis_and_line_length() {
    let size = Vec2::new(100.0, 50.0);

    // 0deg points UP: y-up axis (0,1) → y-down (0,-1). Line length = |H·cos0| = 50.
    let (axis, len) = linear_axis(0.0, size);
    assert!(
        (axis[0] - 0.0).abs() < 1e-5,
        "0deg axis x ~ 0, got {axis:?}"
    );
    assert!(
        (axis[1] - -1.0).abs() < 1e-5,
        "0deg axis y ~ -1, got {axis:?}"
    );
    assert!((len - 50.0).abs() < 1e-4, "0deg line len ~ H, got {len}");

    // 90deg points RIGHT: y-up axis (1,0) → y-down (1,0). Line length = |W·sin90| = 100.
    let (axis, len) = linear_axis(90.0, size);
    assert!(
        (axis[0] - 1.0).abs() < 1e-5,
        "90deg axis x ~ 1, got {axis:?}"
    );
    assert!(
        (axis[1] - 0.0).abs() < 1e-5,
        "90deg axis y ~ 0, got {axis:?}"
    );
    assert!((len - 100.0).abs() < 1e-4, "90deg line len ~ W, got {len}");

    // 180deg points DOWN: y-up axis (0,-1) → y-down (0,1).
    let (axis, _) = linear_axis(180.0, size);
    assert!(
        (axis[0] - 0.0).abs() < 1e-5,
        "180deg axis x ~ 0, got {axis:?}"
    );
    assert!(
        (axis[1] - 1.0).abs() < 1e-5,
        "180deg axis y ~ 1, got {axis:?}"
    );
}

/// The design's 150deg gradient: sin150 = 0.5, cos150 = -√3/2 ≈ -0.866. The
/// y-down axis is `(0.5, 0.866)` (points right-and-DOWN), so the top-left of the
/// box is near the START stop (`--ac`) and the bottom-right near the END
/// (`--ac2`) — the corner expectation the GPU proof asserts.
#[test]
fn linear_axis_150deg_points_right_and_down() {
    let (axis, _) = linear_axis(150.0, Vec2::splat(24.0));
    assert!(
        (axis[0] - 0.5).abs() < 1e-4,
        "150deg axis x ~ 0.5, got {axis:?}"
    );
    assert!(
        (axis[1] - 0.8660254).abs() < 1e-4,
        "150deg axis y ~ +0.866 (right-and-down in y-down space), got {axis:?}"
    );
}

/// `resolve_gradients` resolves the 2 stop tokens to concrete LINEAR colors,
/// carries their positions, marks the kind LINEAR, and packs the 150deg axis +
/// line length. This is the extract-tier contract the packer + shader consume.
#[test]
fn resolve_gradients_resolves_stops_and_packs_angle() {
    let theme = accent_theme();
    let size = Vec2::splat(24.0);
    let out = resolve_gradients(
        &linear_layers(150.0),
        Vec2::ZERO,
        size,
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    );
    assert_eq!(out.len(), 1, "one Linear layer → one gradient instance");
    let g = out[0];

    // Stop colors are resolved + CPU-linearized (the same linearization the fill
    // path uses). #5b86f5 → linear; compare against the resolver's own output.
    let lin = |hex: Color| {
        let l = LinearRgba::from(hex);
        [l.red, l.green, l.blue, l.alpha]
    };
    assert_eq!(
        g.color0,
        lin(Color::srgb_u8(0x5b, 0x86, 0xf5)),
        "start = --ac"
    );
    assert_eq!(
        g.color1,
        lin(Color::srgb_u8(0x7f, 0xa1, 0xf7)),
        "end = --ac2"
    );
    assert_eq!(g.stops, [0.0, 1.0], "stop positions");
    assert_eq!(g.kind, GRADIENT_KIND_LINEAR);

    // The packed axis is the 150deg y-down direction (right-and-down).
    assert!((g.axis[0] - 0.5).abs() < 1e-4, "axis x, got {:?}", g.axis);
    assert!(
        (g.axis[1] - 0.8660254).abs() < 1e-4,
        "axis y, got {:?}",
        g.axis
    );
    // Line length for a 24x24 box at 150deg: |24·0.5| + |24·-0.866| = 12 + 20.78.
    assert!(
        (g.line_len - (24.0 * 0.5 + 24.0 * 0.8660254)).abs() < 1e-3,
        "line_len, got {}",
        g.line_len
    );
    assert_eq!(g.rect_size, size);
}

/// A `Solid` layer lowers to a degenerate 2-stop gradient (both stops the same
/// color), so the ONE gradient pipeline paints layered solids with no special
/// case.
#[test]
fn resolve_gradients_lowers_solid_layer_to_flat_two_stop() {
    let theme = accent_theme();
    let layers = BackgroundLayers(vec![BackgroundLayer::Solid(ColorToken::Accent)]);
    let out = resolve_gradients(
        &layers,
        Vec2::ZERO,
        Vec2::splat(10.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].color0, out[0].color1, "solid → both stops equal");
    assert_eq!(out[0].kind, GRADIENT_KIND_LINEAR);
}

/// A fully-transparent layer (`Solid(Transparent)`) paints nothing — no instance
/// (mirrors the fill's `Color::NONE` skip).
#[test]
fn resolve_gradients_skips_fully_transparent_layer() {
    let theme = accent_theme();
    let layers = BackgroundLayers(vec![BackgroundLayer::Solid(ColorToken::Transparent)]);
    let out = resolve_gradients(
        &layers,
        Vec2::ZERO,
        Vec2::splat(10.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    );
    assert!(out.is_empty(), "transparent layer emits no instance");
}

/// Multiple layers emit in BACK-to-front draw order (the producer reverses the
/// CSS index-0-frontmost order so the pipeline draws back-to-front).
#[test]
fn resolve_gradients_emits_layers_back_to_front() {
    let theme = accent_theme();
    // CSS paint order: index 0 (accent) is frontmost, index 1 (accent.lighter)
    // is behind. The draw list must be [behind, front] = [lighter, accent].
    let layers = BackgroundLayers(vec![
        BackgroundLayer::Solid(ColorToken::Accent),
        BackgroundLayer::Solid(ColorToken::AccentLighter),
    ]);
    let out = resolve_gradients(
        &layers,
        Vec2::ZERO,
        Vec2::splat(10.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    );
    assert_eq!(out.len(), 2);
    let lin = |hex: Color| {
        let l = LinearRgba::from(hex);
        [l.red, l.green, l.blue, l.alpha]
    };
    // Drawn first (bottom): the lighter (CSS index 1, behind).
    assert_eq!(out[0].color0, lin(Color::srgb_u8(0x7f, 0xa1, 0xf7)));
    // Drawn last (top): the accent (CSS index 0, frontmost).
    assert_eq!(out[1].color0, lin(Color::srgb_u8(0x5b, 0x86, 0xf5)));
}

/// A `Radial` layer carries the radial kind flag (the B2 seam) — B1 packs it but
/// the shader's radial branch is B2's. Pinned so the enum + flag are complete.
#[test]
fn resolve_gradients_marks_radial_kind() {
    use buiy_core::render::components::RadialGradient;
    let theme = accent_theme();
    let layers = BackgroundLayers(vec![BackgroundLayer::Radial(RadialGradient {
        stops: vec![
            ColorStop {
                color: ColorToken::Accent,
                position: 0.0,
            },
            ColorStop {
                color: ColorToken::AccentLighter,
                position: 1.0,
            },
        ],
        // Single centered radial (the non-tiled B1 path): no explicit radius /
        // tile — box-derived extent, one gradient over the box.
        ..default()
    })]);
    let out = resolve_gradients(
        &layers,
        Vec2::ZERO,
        Vec2::splat(20.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, GRADIENT_KIND_RADIAL);
    // A non-tiled radial: the `axis` slot is the zero tile (single gradient over
    // the box) and the extent is the box farthest-corner default `0.5·|size|`.
    assert_eq!(out[0].axis, [0.0, 0.0], "non-tiled → zero tile");
    let expected_extent = 0.5 * Vec2::splat(20.0).length();
    assert!(
        (out[0].line_len - expected_extent).abs() < 1e-3,
        "single-radial extent = 0.5·|size|, got {}",
        out[0].line_len
    );
}

/// B2: the viewport **dotted radial-grid** (`radial-gradient(#16181c 1px,
/// transparent 1px)`; `background-size: 22px 22px` — values.md § 7.3) resolves at
/// extract to: the dot color (`color.misc.dot-bg` == `#16181c`) in `color0`, the
/// gap (`transparent`) in `color1`, the dot radius (1px) in `line_len`, and the
/// tile size (22×22) in the `axis` slot — the params the shader's radial branch
/// reads to stamp a hard-edged dot in every cell. This is the extract-tier
/// contract the GPU dot-grid proof (`render_gradient_gpu.rs`) consumes.
#[test]
fn resolve_gradients_packs_dotted_grid_params() {
    use buiy_core::render::components::RadialGradient;
    // The default dark theme carries `color.misc.dot-bg` (#16181c) — A2's token.
    let theme = buiy_core::theme::default_dark_theme();
    let layers = BackgroundLayers(vec![BackgroundLayer::Radial(RadialGradient::dot_grid(
        ColorToken::DotBg,
        1.0,
        22.0,
    ))]);
    // A 66×66 box (3×3 tiles) — the GPU proof's geometry.
    let out = resolve_gradients(
        &layers,
        Vec2::ZERO,
        Vec2::splat(66.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    );
    assert_eq!(out.len(), 1, "one Radial dot-grid layer → one instance");
    let g = out[0];
    assert_eq!(g.kind, GRADIENT_KIND_RADIAL);

    // Dot color = #16181c, CPU-linearized (the same path the fill uses).
    let lin = |hex: Color| {
        let l = LinearRgba::from(hex);
        [l.red, l.green, l.blue, l.alpha]
    };
    assert_eq!(
        g.color0,
        lin(Color::srgb_u8(0x16, 0x18, 0x1c)),
        "dot color = #16181c (color.misc.dot-bg)"
    );
    // The gap stop is transparent (the app bg shows through between dots).
    assert_eq!(g.color1[3], 0.0, "between-dots stop is transparent");

    // Dot radius (1px) in line_len; tile size (22×22) in the axis slot.
    assert!(
        (g.line_len - 1.0).abs() < 1e-6,
        "dot radius = 1px in line_len, got {}",
        g.line_len
    );
    assert_eq!(g.axis, [22.0, 22.0], "tile size (22×22) in the axis slot");
}

/// B2: a SINGLE radial with an explicit `radius` uses it as the extent (not the
/// box default), and carries the zero tile (one gradient over the box). Proves
/// `radius` and `tile` are independent — the data model spans both the dotted
/// grid (tile=Some) and a plain radial gradient (tile=None).
#[test]
fn resolve_gradients_single_radial_honors_explicit_radius() {
    use buiy_core::render::components::RadialGradient;
    let theme = accent_theme();
    let layers = BackgroundLayers(vec![BackgroundLayer::Radial(RadialGradient {
        stops: vec![
            ColorStop {
                color: ColorToken::Accent,
                position: 0.0,
            },
            ColorStop {
                color: ColorToken::AccentLighter,
                position: 1.0,
            },
        ],
        radius: Some(7.5),
        tile: None,
    })]);
    let out = resolve_gradients(
        &layers,
        Vec2::ZERO,
        Vec2::splat(40.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    );
    assert_eq!(out.len(), 1);
    assert!(
        (out[0].line_len - 7.5).abs() < 1e-6,
        "explicit radius used as extent, got {}",
        out[0].line_len
    );
    assert_eq!(out[0].axis, [0.0, 0.0], "tile=None → zero tile");
}
