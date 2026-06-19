//! Glyphs in effect groups (T8): the partition wiring + the text-in-group
//! composite golden. glyph-pipeline § 11.2; decoration-and-paint § 4.5;
//! verification § 1.3. All #[ignore]: need a wgpu adapter (CLAUDE.md GPU
//! lane).
//!
//! Run: cargo test -p buiy_core --test text_effect_group_gpu -- --ignored --test-threads=1

use bevy::math::Rect;
use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{CaretColor, Opacity, TextColor};
use buiy_core::render::compositor::composite_src_over;
use buiy_core::render::golden::GoldenConfig;
use buiy_core::render::prepare::BuiyInstanceBuffers;
use buiy_core::text::{CaretVisual, FontSize, Text};
use std::borrow::Cow;
use std::ops::Range;

const W: u32 = 128;
const H: u32 = 64;
/// Glyph tint: white (the text_decoration_gpu idiom).
const TEXT_TOKEN: &str = "test.text";
/// Explicit `caret-color` (tier 1 of the § 6.2 chain): pure red.
const CARET_TOKEN: &str = "test.caret";

fn caret_red() -> Color {
    Color::srgb(1.0, 0.0, 0.0)
}

/// White "Hi" at 24 px with the shared text token — the fixture's one leaf
/// shape, spawned twice (grouped + flat).
fn spawn_white_text(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontSize(24.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
        ))
        .id()
}

/// Spawn the shared two-text fixture: white "Hi" inside a BACKGROUNDLESS
/// `Opacity(0.5)` card (top half), white "Hi" flat sibling (bottom half).
/// Returns `(grouped_text, flat_text)`. Backgroundless on purpose: the
/// group's QUAD range is empty while its GLYPH range is not — the D5
/// step-1 skip fix's pin.
fn spawn_group_and_flat_text(app: &mut App) -> (Entity, Entity) {
    let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
    theme.colors.insert(TEXT_TOKEN.into(), Color::WHITE);

    let half = |top: f32| {
        Style::default()
            .absolute()
            .inset(Inset {
                top: Sizing::Length(Length::px(top)),
                left: Sizing::Length(Length::px(0.0)),
                ..default()
            })
            .flex_column()
            .width_px(W as f32)
            .height_px((H / 2) as f32)
    };

    // The Opacity(0.5) card — an EffectGroup former (write_effect_groups
    // marks it) with NO Background, wrapping the grouped text in the top half.
    let grouped = spawn_white_text(app);
    let card = app
        .world_mut()
        .spawn((Node, half(0.0), Opacity(0.5)))
        .add_child(grouped)
        .id();

    // The flat sibling: same text, no effect former, bottom half.
    let flat = spawn_white_text(app);
    let flat_wrap = app
        .world_mut()
        .spawn((Node, half((H / 2) as f32)))
        .add_child(flat)
        .id();

    // One sized root holding both (the text_decoration_gpu idiom).
    app.world_mut()
        .spawn((
            Node,
            Style::default().width_px(W as f32).height_px(H as f32),
        ))
        .add_children(&[card, flat_wrap]);
    (grouped, flat)
}

/// All pixels of the row band `rows` (left→right, top→bottom).
fn pixels_in_rows(frame: &[u8], rows: Range<u32>) -> impl Iterator<Item = [u8; 4]> + '_ {
    rows.flat_map(move |y| (0..W).map(move |x| crate::support::px(frame, W, x, y)))
}

/// The card's half of the frame (the grouped text).
fn top_half_pixels(frame: &[u8]) -> impl Iterator<Item = [u8; 4]> + '_ {
    pixels_in_rows(frame, 0..H / 2)
}

/// The flat sibling's half of the frame.
fn bottom_half_pixels(frame: &[u8]) -> impl Iterator<Item = [u8; 4]> + '_ {
    pixels_in_rows(frame, H / 2..H)
}

/// Glyph-ink white at full (undimmed) strength — the all-channels-≥200
/// predicate from text_decoration_gpu.
fn is_white(p: [u8; 4]) -> bool {
    (0..3).all(|ch| p[ch] >= 200)
}

#[test]
#[ignore = "needs a wgpu adapter; the campaign T8 text-in-effect-group composite golden (glyph-pipeline § 11.2 closed; the verification § 1.3 row is now claimable)"]
fn text_in_opacity_group_dims_exactly_once() {
    let _cfg = GoldenConfig::deterministic();
    let mut app = crate::support::gpu_render_app(W, H);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(CARET_TOKEN.into(), caret_red());
    }
    let (grouped, _flat) = spawn_group_and_flat_text(&mut app);
    // A caret on the GROUPED text — § 4.5's caret half, first end-to-end
    // coverage (T7 deferred it here): a 1×24 stamp right of the ink.
    app.world_mut().entity_mut(grouped).insert((
        CaretVisual {
            visible: true,
            rect: Rect::new(60.0, 0.0, 61.0, 24.0),
        },
        CaretColor(ColorToken::Token(Cow::Borrowed(CARET_TOKEN))),
    ));
    // Hold the blink phase visible across the readback's real-time polling
    // frames (the caret-blink pair test's paused-clock idiom).
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 4);
    crate::support::wait_for_text_ready(&mut app, 60);
    let frame = crate::support::readback_rgba(&mut app, target);

    // Expectations via the CPU port (the render_compositor_gpu idiom):
    // a full-coverage group texel composited ONCE at 0.5 over the black
    // clear. "Exactly once": undimmed (≈255) means the glyph bypassed
    // the group (the pre-T8 bug) — and a double-paint (flat AND
    // composited) ALSO reads ≈255 (white over white), so the ≈188 pin
    // catches both failure modes.
    let dim = |c: Color| -> [u8; 4] {
        let lin = composite_src_over(
            LinearRgba::from(c),
            LinearRgba::new(0.0, 0.0, 0.0, 1.0),
            0.5,
        );
        let s = Srgba::from(lin);
        [
            (s.red * 255.0).round() as u8,
            (s.green * 255.0).round() as u8,
            (s.blue * 255.0).round() as u8,
            255,
        ]
    };
    let dim_white = dim(Color::WHITE); // ≈ [188, 188, 188]
    let dim_red = dim(caret_red()); // ≈ [188, 0, 0]
    const TOL: i32 = 4;
    let near = |p: [u8; 4], e: [u8; 4]| (0..3).all(|c| (p[c] as i32 - e[c] as i32).abs() <= TOL);

    // (1) TOP half (the card): NO undimmed-white pixel anywhere…
    assert!(
        top_half_pixels(&frame).all(|p| !is_white(p)),
        "no grouped ink at full strength — the glyphs rode the group target"
    );
    // …and the dimmed ink IS present at the composite-exact value.
    assert!(
        top_half_pixels(&frame).any(|p| near(p, dim_white)),
        "the grouped ink is present at exactly composite_src_over(white, black, 0.5)"
    );
    // (2) The caret column dims identically (stamps are glyph instances).
    assert!(
        top_half_pixels(&frame).any(|p| near(p, dim_red)),
        "the grouped caret stamp dims with its group"
    );
    assert!(
        top_half_pixels(&frame).all(|p| !(p[0] >= 230 && p[1] <= 20 && p[2] <= 20)),
        "no full-strength caret pixel — the stamp did not bypass the group"
    );
    // (3) BOTTOM half (the flat sibling): full-strength ink survives —
    // the flat complement still draws non-group glyphs.
    assert!(
        bottom_half_pixels(&frame).any(is_white),
        "the flat sibling's ink stays undimmed (the complement draw)"
    );
}

#[test]
#[ignore = "needs a wgpu adapter; T8 glyph-partition wiring (D1/D2 — ranges mirror the entity group assignment)"]
fn glyph_partition_mirrors_the_entity_group_assignment() {
    let _cfg = GoldenConfig::deterministic();
    let mut app = crate::support::gpu_render_app(W, H);
    let (_grouped, _flat) = spawn_group_and_flat_text(&mut app);
    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 4);
    crate::support::wait_for_text_ready(&mut app, 60);

    let buffers = crate::support::render_world_resource::<BuiyInstanceBuffers>(&app)
        .expect("BuiyInstanceBuffers");
    // One live group; its glyph range is non-empty (the card's ink).
    assert_eq!(buffers.glyph_group_ranges.len(), 1);
    let g = buffers.glyph_group_ranges[0].clone();
    assert!(
        g.start < g.end,
        "the grouped text's glyphs got a range: {g:?}"
    );
    // The flat complement is non-empty (the sibling's ink) and the two
    // partitions disjointly cover [0, glyph_count) — the exact quad
    // group_ranges/flat_ranges contract.
    assert!(!buffers.glyph_flat_ranges.is_empty());
    let mut all: Vec<_> = buffers.glyph_flat_ranges.clone();
    all.push(g);
    all.sort_by_key(|r| r.start);
    let mut next = 0u32;
    for r in &all {
        assert_eq!(r.start, next, "gapless disjoint cover: {all:?}");
        next = r.end;
    }
    assert_eq!(next, buffers.glyph_count, "…ending at glyph_count");
}
