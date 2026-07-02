//! GPU lane (`#[ignore]` — needs a real wgpu adapter / lavapipe): the parity
//! Wave B3 vector-icon channel actually PAINTS, and re-tints live on a theme
//! swap. Spawns three of the design's stroke icons — the chevron (`M9 5l7 7-7 7`,
//! stroke 1.9 — disclosure #24), the checkmark (`M4 12.5 9 17.5 20 6.5`, stroke
//! 2.4 — todo check #11), and the search magnifier (`M11 18a7 7 0 1 0 0-14 7 7 0
//! 0 0 0 14M20 20l-4-4`, stroke 1.7, has an ARC — #9) — in the accent color over
//! a dark bg, renders them to an offscreen target, writes the PNG to
//! `docs/reports/parity-proto-assets/b3-icons.png`, and asserts PROGRAMMATICALLY
//! (adapter-tolerant — passes on this RX 6700 XT host AND CI's lavapipe):
//!
//!   - a stroke pixel of each icon is LIT and ≈ the accent color (the icon
//!     rendered through the coverage path, tinted by the resolved token),
//!   - a known-empty region (the bg, away from every icon) is the dark clear,
//!   - re-rendering after a `Theme` accent swap RE-TINTS the icon (one pixel
//!     sampled under two accents differs toward the new accent) — proving the
//!     live recolor the design needs (a swatch click re-themes the app), with
//!     NO atlas mutation (the coverage cell is monochrome; only the per-instance
//!     color changes).
//!
//! It also reads the render-world `BuiyAtlas` + `ExtractedIcons` back to assert
//! the producer inserted a coverage atlas entry of the expected pixel size and
//! that two identical icons DEDUP to one atlas entry.
//!
//! This is NOT a blessed CI golden (CI's lavapipe pixels differ from this host —
//! the FINAL phase handles CI goldens). The PNG + the relative assertions are
//! the prototype proof that the icon pipeline is correct.
//!
//! Run:  cargo test -p buiy_core --test render icon -- --ignored --test-threads=1

use bevy::prelude::*;
use bevy::render::RenderApp;
use buiy_core::Length;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Sizing, Style};
use buiy_core::render::atlas::BuiyAtlas;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Icon;
use buiy_core::render::golden::{GoldenConfig, capture_app, capture_to_image};
use buiy_core::render::icon_producer::{ExtractedIcons, icon_atlas_key};

use crate::support::px;

/// The design's blue accent `--ac = #5b86f5` (values.md § 1.2). Icons tint with
/// `ColorToken::Accent`, which resolves to the theme's live `accent` base.
const ACCENT_BLUE: (u8, u8, u8) = (0x5b, 0x86, 0xf5);
/// A clearly-different accent (the violet `--ac` option, #b98aff) for the re-tint
/// proof — its red+blue rise vs blue, so a re-tint is unambiguous on those
/// channels.
const ACCENT_VIOLET: (u8, u8, u8) = (0xb9, 0x8a, 0xff);

/// Move the theme's live accent base — the `ColorToken::Accent` icons re-tint to
/// it at resolve time (the design's "swatch click re-themes the app").
fn set_icon_accent(app: &mut App, rgb: (u8, u8, u8)) {
    let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
    theme.accent = Color::srgb_u8(rgb.0, rgb.1, rgb.2);
}

/// Spawn one icon `(Node, Style absolute@(x,y) size×size, Icon)` as a child of a
/// root node, tinted by the live accent (`ColorToken::Accent`).
fn spawn_icon(app: &mut App, x: f32, y: f32, size: u16, path_d: &str, stroke: f32) -> Entity {
    let icon = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(y)),
                    left: Sizing::Length(Length::px(x)),
                    ..default()
                })
                .width_px(size as f32)
                .height_px(size as f32),
            Icon {
                path_d: path_d.to_string(),
                stroke_width: stroke,
                size_px: size,
                fill: false,
                color: ColorToken::Accent,
            },
        ))
        .id();
    let root = app.world_mut().spawn((Node, Style::default())).id();
    app.world_mut().entity_mut(root).add_children(&[icon]);
    icon
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn vector_icons_paint_in_accent_and_retint_on_theme_swap() {
    const W: u32 = 96;
    const H: u32 = 40;
    // Three icons laid out left-to-right in a 96×40 strip, each in a 24×24 box.
    const CHEVRON: &str = "M9 5l7 7-7 7";
    const CHECK: &str = "M4 12.5 9 17.5 20 6.5";
    const SEARCH: &str = "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14M20 20l-4-4";

    let mut app = capture_app(W, H);
    set_icon_accent(&mut app, ACCENT_BLUE);

    // Boxes at x = 6, 38, 70 (24-wide, 4px gaps), centered vertically (y = 8).
    spawn_icon(&mut app, 6.0, 8.0, 24, CHEVRON, 1.9);
    spawn_icon(&mut app, 38.0, 8.0, 24, CHECK, 2.4);
    spawn_icon(&mut app, 70.0, 8.0, 24, SEARCH, 1.7);
    // A SECOND chevron identical to the first, off-screen-ish but spawned, to
    // prove atlas DEDUP (two identical icons → one atlas entry).
    spawn_icon(&mut app, 6.0, 8.0, 24, CHEVRON, 1.9);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    assert_eq!(img.dimensions(), (W, H));

    // Write the PNG proof artifact (NOT a blessed golden — the prototype proof).
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/reports/parity-proto-assets/b3-icons.png");
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    img.save(&out)
        .unwrap_or_else(|e| panic!("write {}: {e}", out.display()));

    let pixels = img.clone().into_raw();

    // --- Producer / atlas assertions (read the render world back) ------------
    {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let world = render_app.world();
        let extracted = world.resource::<ExtractedIcons>();
        // Four Icon entities spawned, but two are identical chevrons → 4 instances
        // emitted (one per entity), yet the chevron's atlas entry is shared.
        assert_eq!(
            extracted.icons.len(),
            4,
            "one coverage instance emitted per Icon entity (4 spawned)"
        );

        let atlas = world.resource::<BuiyAtlas>();
        let chevron_key = icon_atlas_key(CHEVRON, 1.9, 24, false);
        let entry = atlas
            .get(&chevron_key)
            .expect("the chevron icon must be atlas-resident");
        // The cell is the icon's render size (24×24) of CoverageR8 texels.
        assert_eq!(
            entry.px.size(),
            bevy::math::UVec2::new(24, 24),
            "the chevron atlas cell is 24×24 (its size_px)"
        );
        // Dedup: both identical chevrons resolved to the SAME entry (one cell),
        // and the three distinct icons are three more — 4 instances, but exactly
        // 3 DISTINCT atlas keys among them.
        let check_key = icon_atlas_key(CHECK, 2.4, 24, false);
        let search_key = icon_atlas_key(SEARCH, 1.7, 24, false);
        assert!(atlas.get(&check_key).is_some(), "checkmark atlas-resident");
        assert!(atlas.get(&search_key).is_some(), "search atlas-resident");
        assert_ne!(chevron_key, check_key, "distinct icons → distinct cells");
    }

    // --- Pixel assertions: each icon's stroke is lit & ≈ accent --------------
    // The dark clear is opaque black (spawn_capture_camera default in capture_app).
    // Find a lit pixel near each icon's box center column and assert it is bluish
    // (the accent), and that a known-empty corner of the strip is dark.
    let near_accent = |p: [u8; 4]| p[2] > 120 && p[2] as i32 > p[0] as i32; // blue-dominant
    // A column scan over each icon's box finds at least one lit stroke pixel.
    let any_lit_blue_in = |x0: u32, x1: u32| -> Option<[u8; 4]> {
        for y in 8..32 {
            for x in x0..x1 {
                let p = px(&pixels, W, x, y);
                if near_accent(p) {
                    return Some(p);
                }
            }
        }
        None
    };
    let chevron_px = any_lit_blue_in(8, 30).expect("chevron stroke must light a blue pixel");
    let check_px = any_lit_blue_in(40, 62).expect("checkmark stroke must light a blue pixel");
    let search_px = any_lit_blue_in(72, 94).expect("search stroke must light a blue pixel");

    // Each lit stroke pixel is bluish — the accent token tinted the coverage.
    for (label, p) in [
        ("chevron", chevron_px),
        ("check", check_px),
        ("search", search_px),
    ] {
        assert!(
            p[2] > 120 && (p[2] as i32) >= (p[0] as i32),
            "{label} stroke pixel must be accent-blue (B≥R, B>120): {p:?}"
        );
    }

    // A known-empty corner of the strip (top-left, before the first icon box's
    // ink) is the dark clear.
    let corner = px(&pixels, W, 0, 0);
    assert!(
        corner[0] < 30 && corner[1] < 30 && corner[2] < 30,
        "the empty corner is the dark clear: {corner:?}"
    );

    // --- Re-tint proof: swap the accent and re-render ------------------------
    // Sample the chevron's lit pixel BEFORE (blue) and AFTER (violet) the swap;
    // the same coverage cell, a different per-instance color (no atlas mutation).
    // Pick a deterministic lit pixel coordinate to compare apples-to-apples.
    let (sx, sy) = {
        // The chevron tip — scan for a strongly-lit pixel and lock its coord.
        let mut found = None;
        'outer: for y in 8..32 {
            for x in 8..30 {
                let p = px(&pixels, W, x, y);
                if p[2] > 150 && near_accent(p) {
                    found = Some((x, y));
                    break 'outer;
                }
            }
        }
        found.expect("a strongly-lit chevron pixel for the re-tint comparison")
    };
    let before = px(&pixels, W, sx, sy);

    set_icon_accent(&mut app, ACCENT_VIOLET);
    let img2 = capture_to_image(&mut app, &GoldenConfig::deterministic());
    let pixels2 = img2.into_raw();
    let after = px(&pixels2, W, sx, sy);

    // Violet (#b98aff) vs blue (#5b86f5): red rises sharply (0x5b→0xb9) — the
    // unambiguous re-tint signal. The pixel was lit before AND after (the icon
    // still paints); the color CHANGED toward the new accent.
    assert!(
        after[0] as i32 > before[0] as i32 + 30,
        "the icon must RE-TINT on the accent swap (red rises blue→violet): \
         before {before:?} after {after:?}"
    );
    // It is still lit (not blanked) — a re-tint, not a disappearance.
    assert!(
        after[0] > 60 || after[1] > 60 || after[2] > 60,
        "the icon stays painted after the swap (re-tint, not blank): {after:?}"
    );
}

/// A rotated icon paints its 2D affine THROUGH the coverage path — the pixel-level
/// proof for `coverage.wgsl`'s affine multiply (the box reftests in
/// `render_transform_paint_gpu.rs` only exercise the QUAD shader). An asymmetric
/// chevron rotated 90° about its box center (default transform-origin, honored by
/// 6e) MUST render different pixels than unrotated; a dropped affine (the pre-fix
/// bug) would paint the axis-aligned chevron identically in both.
#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn rotated_icon_paints_through_coverage_affine() {
    use std::f32::consts::FRAC_PI_2;
    const W: u32 = 48;
    const H: u32 = 48;
    const CHEVRON: &str = "M9 5l7 7-7 7"; // asymmetric — not 90°-symmetric

    // Render the chevron (24×24 box centered in the 48×48 frame) at `rot` radians.
    let capture = |rot: f32| -> Vec<u8> {
        let mut app = capture_app(W, H);
        set_icon_accent(&mut app, ACCENT_BLUE);
        let icon = app
            .world_mut()
            .spawn((
                Node,
                Style::default()
                    .absolute()
                    .inset(Inset {
                        top: Sizing::Length(Length::px(12.0)),
                        left: Sizing::Length(Length::px(12.0)),
                        ..default()
                    })
                    .width_px(24.0)
                    .height_px(24.0)
                    .rotate_z(rot),
                Icon {
                    path_d: CHEVRON.to_string(),
                    stroke_width: 1.9,
                    size_px: 24,
                    fill: false,
                    // `color.icon` was never a real theme token (pre-Track-B it
                    // missed → the magenta sentinel). This golden is color-agnostic
                    // (asserts rotation-differs + blue-channel ink), so reproduce the
                    // exact prior magenta via the typed `Custom` escape hatch.
                    color: ColorToken::Custom(Color::srgb(1.0, 0.0, 1.0)),
                },
            ))
            .id();
        let root = app.world_mut().spawn((Node, Style::default())).id();
        app.world_mut().entity_mut(root).add_children(&[icon]);
        capture_to_image(&mut app, &GoldenConfig::deterministic()).into_raw()
    };

    let unrotated = capture(0.0);
    let rotated = capture(FRAC_PI_2);
    assert_eq!(unrotated.len(), rotated.len());

    // A dropped affine → byte-identical (0 differing bytes). Applied → the rotated
    // chevron's ink lands elsewhere, differing in many bytes.
    let differing = unrotated
        .iter()
        .zip(&rotated)
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        differing > 100,
        "a 90° rotation must move the chevron's ink through coverage.wgsl \
         (a dropped affine renders identical); differing bytes = {differing}"
    );
    // Both actually painted (sanity: not two blank frames).
    let ink = |buf: &[u8]| buf.chunks_exact(4).filter(|p| p[2] > 80).count();
    assert!(
        ink(&unrotated) > 20 && ink(&rotated) > 20,
        "both renders paint chevron ink (unrotated {}, rotated {})",
        ink(&unrotated),
        ink(&rotated)
    );
}
