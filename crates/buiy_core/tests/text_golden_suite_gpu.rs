//! Gate-#2 golden-suite expansion (campaign T9): widget × state × theme ×
//! viewport text fixtures, inline-golden discipline (the stored-PNG deferral
//! stands — verification § 4 as-landed): capture in a fresh app, assert
//! inline expected pixels, capture again in a second fresh app, assert
//! `perceptual_diff < 1e-4` — the re-capture IS the golden. All #[ignore]:
//! need a wgpu adapter (CLAUDE.md GPU lane).
//!
//! Run: cargo test -p buiy_core --test text_golden_suite_gpu -- --ignored --test-threads=1
#![allow(deprecated)] // perceptual_diff is deprecated; these GPU sites migrate to buiy_verify::metric in Phase 3 (tier-5 goldens).

mod support;

use bevy::math::Rect;
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::window::PrimaryWindow;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::atlas::BuiyAtlas;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, Border, Corners, Radius, TextColor};
use buiy_core::render::golden::{GoldenConfig, perceptual_diff};
use buiy_core::text::{
    CaretVisual, DecorationLines, FontSize, ResidentTextKeys, SelectionVisual, Text,
    TextDecorations,
};
use cosmic_text::Cursor;
use std::borrow::Cow;
use std::ops::Range;

/// Channel tolerance for full-coverage texel probes (the text_gpu TOL idiom).
const TOL: i32 = 4;

/// Resolve a `default_light_theme()` token to its shipped color — the
/// expected-value side of the token-tinted asserts reads the SAME source
/// the app's `ThemePlugin` inserts, never a duplicated literal.
fn token_color(token: &str) -> Color {
    buiy_core::theme::default_light_theme()
        .color(token)
        .unwrap_or_else(|| panic!("default_light_theme ships {token}"))
}

/// The sRGB8 a FULL-coverage texel of `color` encodes to (alpha 1 — the
/// backdrop never shows through, so this holds over any dst).
fn full_coverage(color: Color) -> [u8; 4] {
    let lin = LinearRgba::from(color);
    support::expected_full_coverage_srgb([lin.red, lin.green, lin.blue, lin.alpha])
}

fn assert_px_approx(got: [u8; 4], expected: [u8; 4], what: &str) {
    for ch in 0..3 {
        assert!(
            (got[ch] as i32 - expected[ch] as i32).abs() <= TOL,
            "{what}: channel {ch} got {} expected {} (±{TOL}) — full pixel \
             got={got:?} expected={expected:?}",
            got[ch],
            expected[ch],
        );
    }
}

/// `true` when every color channel of `p` is within ±TOL of `expected` —
/// the predicate form of [`assert_px_approx`], for band scans.
fn px_matches(p: [u8; 4], expected: [u8; 4]) -> bool {
    (0..3).all(|ch| (p[ch] as i32 - expected[ch] as i32).abs() <= TOL)
}

/// Brightest painted texel (max RGB sum) — over the opaque-black clear this
/// is the maximum-coverage ink texel (the `text_gpu` idiom).
fn brightest(pixels: &[u8]) -> [u8; 4] {
    pixels
        .chunks_exact(4)
        .max_by_key(|p| p[0] as u32 + p[1] as u32 + p[2] as u32)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .unwrap()
}

/// Columns (left→right) inside the sub-rect where ANY pixel satisfies
/// `pred` — the `text_selection_caret_gpu` band idiom, confined to a region
/// (the opaque-black backdrop and the card's corner-radius AA both read
/// "dark", so whole-frame scans cannot isolate glyph ink).
fn cols_where_in(
    pixels: &[u8],
    w: u32,
    xs: Range<u32>,
    ys: Range<u32>,
    pred: impl Fn([u8; 4]) -> bool,
) -> Vec<u32> {
    xs.filter(|&x| ys.clone().any(|y| pred(support::px(pixels, w, x, y))))
        .collect()
}

/// Rows (top→bottom) inside the sub-rect where ANY pixel satisfies `pred`.
fn rows_where_in(
    pixels: &[u8],
    w: u32,
    xs: Range<u32>,
    ys: Range<u32>,
    pred: impl Fn([u8; 4]) -> bool,
) -> Vec<u32> {
    ys.filter(|&y| xs.clone().any(|x| pred(support::px(pixels, w, x, y))))
        .collect()
}

/// Coalesce sorted indices into contiguous bands (the
/// `text_selection_caret_gpu` idiom).
fn bands(sorted: &[u32]) -> Vec<Range<u32>> {
    let mut out: Vec<Range<u32>> = Vec::new();
    for &i in sorted {
        match out.last_mut() {
            Some(b) if b.end == i => b.end = i + 1,
            _ => out.push(i..i + 1),
        }
    }
    out
}

// --- Task 1: the widget axis — themed text on a themed card surface. --------

const CARD_W: u32 = 160;
const CARD_H: u32 = 96;
/// The card node's authored size + padding (the fixture's one geometry).
const CARD_SIZE: Vec2 = Vec2::new(120.0, 32.0);
const CARD_PAD: f32 = 8.0;
const CARD_RADIUS: f32 = 6.0;

/// Build app → the card fixture → capture the first text-ready frame.
/// A button-shaped composite assembled IN the fixture (D3): the real
/// Phase-0 `Button` has no `Text` child and growing it one is
/// widget-catalog scope — quad-under-glyph inside one card, both colors
/// token-resolved from the stock `default_light_theme()` (no insertion).
/// Returns the pixels plus the card's laid-out rect (read back from the
/// world, not assumed), so probes derive from real geometry.
fn capture_card() -> (Vec<u8>, Rect) {
    let _cfg = GoldenConfig::deterministic(); // the triad gates this fixture
    let mut app = support::gpu_render_app(CARD_W, CARD_H);

    let label = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Save")),
            FontSize(16.0),
            TextColor(ColorToken::Token(Cow::Borrowed("color.text.primary"))),
        ))
        .id();
    let card = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(CARD_SIZE.x)
                .height_px(CARD_SIZE.y)
                .padding(CARD_PAD),
            Background {
                color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
            },
            Border {
                radius: Corners::all(Radius::circular(CARD_RADIUS)),
                ..default()
            },
        ))
        .id();
    app.world_mut().entity_mut(card).add_child(label);
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(CARD_W as f32)
                .height_px(CARD_H as f32),
        ))
        .add_child(card);

    let target = support::render_to_image(&mut app, CARD_W, CARD_H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    let pixels = support::readback_rgba(&mut app, target);

    let origin = app
        .world()
        .get::<GlobalTransform>(card)
        .expect("the card entity's GlobalTransform")
        .translation()
        .truncate();
    (pixels, Rect::from_corners(origin, origin + CARD_SIZE))
}

#[test]
#[ignore = "needs a wgpu adapter; gate-#2 widget-axis golden (text campaign T9)"]
fn widget_card_text_is_deterministic_and_token_tinted() {
    let (frame_a, card) = capture_card();

    // (a) Backdrop outside the card reads the opaque-black clear.
    let backdrop = (CARD_W - 2, CARD_H - 2);
    assert!(
        !card.contains(Vec2::new(backdrop.0 as f32, backdrop.1 as f32)),
        "the backdrop probe sits outside the card rect {card:?}"
    );
    assert_eq!(
        support::px(&frame_a, CARD_W, backdrop.0, backdrop.1),
        [0, 0, 0, 255],
        "backdrop is the opaque-black clear"
    );

    // (b) A card-interior pixel away from the label and the rounded corners
    // reads the resolved `color.surface.secondary` (quad tier, full SDF
    // coverage): 10 px in from the card's right edge at mid height — the
    // 16 px "Save" label ends well left of it.
    let surface = full_coverage(token_color("color.surface.secondary"));
    let interior = (
        (card.max.x - 10.0) as u32,
        (card.min.y + CARD_SIZE.y / 2.0) as u32,
    );
    assert_px_approx(
        support::px(&frame_a, CARD_W, interior.0, interior.1),
        surface,
        "card interior == resolved color.surface.secondary",
    );

    // (c) Dark glyph ink exists within the card rect. Scan the card inset
    // by the corner radius (corner AA blends toward the black backdrop and
    // would alias as "ink"); inside that region the only dark pixels are
    // the label's glyphs.
    let ink = full_coverage(token_color("color.text.primary"));
    let xs = (card.min.x as u32 + CARD_RADIUS as u32)..(card.max.x as u32 - CARD_RADIUS as u32);
    let ys = (card.min.y as u32 + CARD_RADIUS as u32)..(card.max.y as u32 - CARD_RADIUS as u32);
    // "≈ color.text.primary": well below the near-white surface. The band
    // predicate only LOCATES ink (AA edge pixels blend toward the surface,
    // so it stays a threshold); the encode itself is pinned by the
    // two-sided darkest-pixel probe below.
    let is_ink = |p: [u8; 4]| p[0] <= 128 && p[1] <= 128 && p[2] <= 128;
    let ink_cols = cols_where_in(&frame_a, CARD_W, xs.clone(), ys.clone(), is_ink);
    let ink_rows = rows_where_in(&frame_a, CARD_W, xs.clone(), ys.clone(), is_ink);
    assert!(
        !ink_cols.is_empty() && !ink_rows.is_empty(),
        "the label painted dark ink inside the card (cols {ink_cols:?}, rows {ink_rows:?})"
    );
    // The darkest card-interior pixel IS the token's full-coverage encode,
    // two-sided ±TOL (measured [26, 26, 26] on the GPU host — 16 px "Save"
    // glyphs do reach coverage 1.0). Darker fails (black ink / a darker
    // token); lighter fails too (`color.text.secondary` encodes 102, the
    // placeholder 140, and a coverage-capping blend regression lands
    // between 26 and the surface).
    let darkest = xs
        .flat_map(|x| ys.clone().map(move |y| (x, y)))
        .map(|(x, y)| support::px(&frame_a, CARD_W, x, y))
        .min_by_key(|p| p[0] as u32 + p[1] as u32 + p[2] as u32)
        .unwrap();
    assert_px_approx(
        darkest,
        ink,
        "darkest card-interior pixel == the color.text.primary full-coverage encode",
    );

    // (d) gate-#2 determinism: an independent fresh capture matches (the
    // stored-PNG machinery stays deferred; the re-capture IS the golden).
    let (frame_b, _) = capture_card();
    let diff = perceptual_diff(&frame_a, &frame_b);
    assert!(
        diff < 1e-4,
        "two fresh captures diverged: perceptual_diff = {diff}"
    );
}

// --- Task 2: the state axis — placeholder vs filled + selection + caret. ----

const STATE_W: u32 = 192;
const STATE_H: u32 = 64;

/// Build app → the input-state fixture → capture the first text-ready
/// frame. The gate-#2 **state** axis pair (D3): `filled = false` is
/// placeholder text — ordinary text whose foreground resolves to the
/// `color.text.placeholder` token, carrying NO `CaretVisual`/
/// `SelectionVisual` (a placeholder is never selectable —
/// decoration-and-paint § 7); `filled = true` is primary-tinted text plus
/// an authored selection over the first 3 clusters and an authored caret —
/// authoring the visuals is the fixture's job until `buiy-text-editing`
/// lands real drivers (the T7 idiom). All tokens ship in
/// `default_light_theme()` — no insertion needed.
fn capture_input_state(filled: bool) -> Vec<u8> {
    let _cfg = GoldenConfig::deterministic(); // the triad gates this fixture
    let mut app = support::gpu_render_app(STATE_W, STATE_H);

    // fixed_clock, realized (the T7 spawn_blink_fixture idiom): PAUSE the
    // virtual clock so the real-time `app.update()`s of finish_and_run +
    // wait_for_text_ready + readback polling accrue ZERO virtual elapsed —
    // write_caret_blink then evaluates every capture at t = 0 (visible
    // phase) instead of wall-clock-drifting across a 500 ms blink edge,
    // which would flip the authored caret between the two fresh captures
    // assertion (a) compares.
    app.world_mut().resource_mut::<Time<Virtual>>().pause();

    let mut text = app.world_mut().spawn((
        Node,
        Style::default(),
        Text(String::from("Search")),
        FontSize(20.0),
    ));
    if filled {
        text.insert((
            TextColor(ColorToken::Token(Cow::Borrowed("color.text.primary"))),
            // The first ~3 clusters: bytes [0, 3) = "Sea" (1-byte ASCII
            // clusters), the `text_selection_caret_gpu` Cursor authoring.
            SelectionVisual::new(Cursor::new(0, 0), Cursor::new(0, 3)),
            // Entity-local rect: a 1-logical-px column safely right of the
            // 20 px "Search" ink, spanning the 24-row line box (1.2 × 20) —
            // the blink fixture's authoring idiom. No `CaretColor`:
            // caret-color auto resolves to the entity's foreground.
            CaretVisual {
                visible: true,
                rect: Rect::new(100.0, 0.0, 101.0, 24.0),
            },
        ));
    } else {
        text.insert(TextColor::placeholder());
    }
    let text = text.id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(STATE_W as f32)
                .height_px(STATE_H as f32),
        ))
        .add_child(text);

    let target = support::render_to_image(&mut app, STATE_W, STATE_H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    support::readback_rgba(&mut app, target)
}

/// The pair's placeholder half (decoration-and-paint § 7).
fn capture_placeholder() -> Vec<u8> {
    capture_input_state(false)
}

/// The pair's filled half: the T7 selection + caret primitives
/// re-exercised in one combined frame.
fn capture_filled() -> Vec<u8> {
    capture_input_state(true)
}

#[test]
#[ignore = "needs a wgpu adapter; gate-#2 state-axis golden pair (text campaign T9; decoration-and-paint §§ 5–7)"]
fn input_state_pair_placeholder_vs_selected() {
    let placeholder_a = capture_placeholder();
    let filled_a = capture_filled();

    // (a) gate-#2 determinism, each state against its own independent fresh
    // re-capture (the re-capture IS the golden).
    let placeholder_diff = perceptual_diff(&placeholder_a, &capture_placeholder());
    assert!(
        placeholder_diff < 1e-4,
        "two fresh placeholder captures diverged: perceptual_diff = {placeholder_diff}"
    );
    let filled_diff = perceptual_diff(&filled_a, &capture_filled());
    assert!(
        filled_diff < 1e-4,
        "two fresh filled captures diverged: perceptual_diff = {filled_diff}"
    );

    // (b) the pair differs — the state axis actually moves pixels.
    let pair_diff = perceptual_diff(&placeholder_a, &filled_a);
    assert!(
        pair_diff > 5e-4,
        "placeholder vs filled+selected must differ: perceptual_diff = {pair_diff}"
    );

    // (c) the placeholder tint: the frame is grey-on-black ONLY, so the
    // brightest texel IS the placeholder grey's full-coverage encode
    // (20 px "Search" stems reach coverage 1.0 — the Task 1 measurement at
    // 16 px already did)…
    assert_px_approx(
        brightest(&placeholder_a),
        full_coverage(token_color("color.text.placeholder")),
        "brightest placeholder texel == the color.text.placeholder full-coverage encode",
    );
    // …and NO selection paint exists anywhere (never selectable, § 7).
    let sel_bg = full_coverage(token_color("color.selection.bg"));
    let stray = rows_where_in(&placeholder_a, STATE_W, 0..STATE_W, 0..STATE_H, |p| {
        px_matches(p, sel_bg)
    });
    assert!(
        stray.is_empty(),
        "the placeholder frame painted selection-bg pixels in rows {stray:?}"
    );

    // (d) the filled frame's selection rect: ONE contiguous row band of
    // exact selection-bg pixels (quad tier, full coverage — every interior
    // band row keeps glyph-free rect pixels at the exact encode; edge rows
    // may AA away, hence the ≥ 3-row width filter).
    let sel_rows = rows_where_in(&filled_a, STATE_W, 0..STATE_W, 0..STATE_H, |p| {
        px_matches(p, sel_bg)
    });
    let sel_bands: Vec<Range<u32>> = bands(&sel_rows)
        .into_iter()
        .filter(|b| b.end - b.start >= 3)
        .collect();
    assert_eq!(
        sel_bands.len(),
        1,
        "the filled frame paints one selection band ≥ 3 rows: got {sel_bands:?} \
         from selection-bg rows {sel_rows:?}"
    );
}

// --- Task 3: the theme axis — one fixture, two palettes + the live swap. ----

const THEME_W: u32 = 128;
const THEME_H: u32 = 64;
/// The fixture's `text-color` token (the `text_gpu` TOKEN idiom — test
/// tokens inserted into the theme, not shipped).
const FG_TOKEN: &str = "test.fg";
/// The fixture's `text-decoration-color` token — `TextDecorations.color`,
/// tier 1 of the § 3.2 precedence (T6's token seat).
const DECO_TOKEN: &str = "test.deco";

/// One theme-axis palette: the colors the fixture's two tokens resolve to.
/// Deco colors stay single-channel-pure so band detection is a channel-
/// dominance test ([`is_deco_band`]) that neither palette's glyph ink
/// (grey / amber — never single-channel) can satisfy.
struct Palette {
    name: &'static str,
    fg: Color,
    deco: Color,
    /// The deco hue's hot RGB channel index (the other two stay ≈ 0).
    deco_channel: usize,
}

/// Palette A: light-grey ink, accent-blue underline.
fn palette_a() -> Palette {
    Palette {
        name: "palette A",
        fg: Color::srgb(0.80, 0.80, 0.80),
        deco: Color::srgb(0.0, 0.0, 1.0),
        deco_channel: 2,
    }
}

/// Palette B: clearly different hues — amber ink, pure-green underline.
fn palette_b() -> Palette {
    Palette {
        name: "palette B",
        fg: Color::srgb(0.95, 0.60, 0.10),
        deco: Color::srgb(0.0, 1.0, 0.0),
        deco_channel: 1,
    }
}

/// (Re)write the two test tokens — both the fixture setup AND the live-swap
/// mutation: writing through `resource_mut` fires `theme.is_changed()`, the
/// § 6.2 re-emit gate the swap half exercises.
fn insert_palette(app: &mut App, palette: &Palette) {
    let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
    theme.colors.insert(FG_TOKEN.into(), palette.fg);
    theme.colors.insert(DECO_TOKEN.into(), palette.deco);
}

/// Build app → the themed fixture under `palette` → drive to the first
/// text-ready frame. Returns the still-live app + target so the swap half
/// can mutate the tokens and recapture IN PLACE; [`capture_themed`] is the
/// cold-path wrapper that reads back once and drops the app.
fn build_themed_app(palette: &Palette) -> (App, Handle<Image>) {
    let _cfg = GoldenConfig::deterministic(); // the triad gates this fixture
    let mut app = support::gpu_render_app(THEME_W, THEME_H);
    insert_palette(&mut app, palette);
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Theme")),
            FontSize(24.0),
            TextColor(ColorToken::Token(Cow::Borrowed(FG_TOKEN))),
            TextDecorations {
                line: DecorationLines::UNDERLINE,
                color: Some(ColorToken::Token(Cow::Borrowed(DECO_TOKEN))),
                ..default()
            },
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(THEME_W as f32)
                .height_px(THEME_H as f32),
        ))
        .add_child(text);

    let target = support::render_to_image(&mut app, THEME_W, THEME_H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    (app, target)
}

/// A fresh cold capture under `palette` (independent app per call — the
/// re-capture IS the golden).
fn capture_themed(palette: &Palette) -> Vec<u8> {
    let (mut app, target) = build_themed_app(palette);
    support::readback_rgba(&mut app, target)
}

/// Band-strength deco pixel: hot channel ≥ 200, both other color channels
/// ≤ 20. The 24 px underline is § 3.3-floored to 1 physical px, whose
/// single interior row reads alpha 0.84375 ≈ sRGB 237 (the
/// `text_decoration_gpu` shader-AA math); the AA bleed rows (≈ 110) and
/// both palettes' glyph inks fail the test.
fn is_deco_band(p: [u8; 4], hot: usize) -> bool {
    (0..3).all(|ch| if ch == hot { p[ch] >= 200 } else { p[ch] <= 20 })
}

/// Assertion (d): exactly one deco-colored band — a real horizontal run —
/// strictly below the glyph-ink rows ("Theme" has no descenders, so no ink
/// crosses the baseline).
fn assert_underline_below_ink(pixels: &[u8], palette: &Palette) {
    let what = palette.name;
    // Ink rows located by the fg token's exact full-coverage encode (24 px
    // stems reach coverage 1.0 — the Task 1/2 measurements at 16/20 px).
    let ink_expected = full_coverage(palette.fg);
    let ink_rows = rows_where_in(pixels, THEME_W, 0..THEME_W, 0..THEME_H, |p| {
        px_matches(p, ink_expected)
    });
    assert!(
        !ink_rows.is_empty(),
        "{what}: full-coverage glyph ink painted"
    );

    let deco_rows = rows_where_in(pixels, THEME_W, 0..THEME_W, 0..THEME_H, |p| {
        is_deco_band(p, palette.deco_channel)
    });
    let deco_bands = bands(&deco_rows);
    assert_eq!(
        deco_bands.len(),
        1,
        "{what}: exactly one underline band: {deco_bands:?}"
    );
    let band = &deco_bands[0];
    assert!(
        band.start > *ink_rows.last().unwrap(),
        "{what}: the underline ({band:?}) sits BELOW the ink rows ({ink_rows:?})"
    );
    // A horizontal RUN, not a speck: the band spans the label's width.
    let band_cols = cols_where_in(pixels, THEME_W, 0..THEME_W, band.clone(), |p| {
        is_deco_band(p, palette.deco_channel)
    });
    assert!(
        band_cols.len() >= 16,
        "{what}: the underline is a horizontal run (got {} deco columns)",
        band_cols.len()
    );
}

#[test]
#[ignore = "needs a wgpu adapter; gate-#2 theme-axis golden pair (text campaign T9)"]
fn themed_text_pair_and_swap_equivalence() {
    let fresh_a = capture_themed(&palette_a());
    let fresh_b = capture_themed(&palette_b());

    // (a) gate-#2 determinism: fresh-A vs an independent fresh-A.
    let a_diff = perceptual_diff(&fresh_a, &capture_themed(&palette_a()));
    assert!(
        a_diff < 1e-4,
        "two fresh palette-A captures diverged: perceptual_diff = {a_diff}"
    );

    // (b) the theme axis moves pixels: fresh-B differs from fresh-A.
    let pair_diff = perceptual_diff(&fresh_a, &fresh_b);
    assert!(
        pair_diff > 5e-4,
        "palette A vs palette B must differ: perceptual_diff = {pair_diff}"
    );

    // (d) ink + underline both painted, band below the ink, in BOTH
    // palettes — the text-ink AND decoration token seats really re-resolve.
    assert_underline_below_ink(&fresh_a, &palette_a());
    assert_underline_below_ink(&fresh_b, &palette_b());

    // (c) swap-equals-cold: capture under A, mutate the tokens to B in the
    // SAME app (`theme.is_changed()` → the § 6.2 re-emit gate), settle 3
    // frames (the retint idiom), recapture. Same logical fixture, same
    // fonts, only tokens moved — the live-swap path must land on the cold
    // path's pixels.
    let (mut app, target) = build_themed_app(&palette_a());
    let pre_swap = support::readback_rgba(&mut app, target.clone());
    let pre_diff = perceptual_diff(&pre_swap, &fresh_a);
    assert!(
        pre_diff < 1e-4,
        "the swap app's pre-swap frame IS a fresh-A capture (the swap below \
         really starts from A's pixels): perceptual_diff = {pre_diff}"
    );
    insert_palette(&mut app, &palette_b());
    for _ in 0..3 {
        app.update();
    }
    let swapped = support::readback_rgba(&mut app, target);
    let swap_diff = perceptual_diff(&swapped, &fresh_b);
    assert!(
        swap_diff < 1e-4,
        "the live token swap lands on the cold palette-B pixels: \
         perceptual_diff = {swap_diff}"
    );
}

// --- Task 4: the viewport axis — scale factor 2.0, physical re-raster. ------

/// The fixture's LOGICAL size — identical at every scale; only the physical
/// pixel grid moves ("shape logical, rasterize physical").
const VIEW_W: u32 = 128;
const VIEW_H: u32 = 64;
/// The fixture's text-color token (the `text_gpu` TOKEN idiom — a test token
/// inserted into the theme): bright, so the brightest texel over the black
/// clear IS the full-coverage ink encode assertion (d) probes.
const VIEW_TOKEN: &str = "test.viewport.fg";

fn viewport_tint() -> Color {
    Color::srgba(0.10, 0.85, 0.30, 1.0)
}

/// Build app at `scale` → the SAME logical fixture → capture the first
/// text-ready frame. Returns the physical-pixel frame plus the maximum
/// atlas-cell height among the app's `ResidentTextKeys` entries — the
/// physical-re-raster witness assertion (c) compares across scales (glyph
/// instance RECTS are logical, `text/extract.rs`; only the atlas raster is
/// physical).
fn capture_viewport(scale: f32) -> (Vec<u8>, u32) {
    let _cfg = GoldenConfig::deterministic(); // the triad gates this fixture
    let mut app = if scale == 1.0 {
        support::gpu_render_app(VIEW_W, VIEW_H)
    } else {
        support::gpu_render_app_scaled(VIEW_W, VIEW_H, scale)
    };

    // The scaled builder's contract, verified before sizing the target
    // (plan Task 4): Bevy 0.18's `WindowResolution::new` takes PHYSICAL
    // units, so `gpu_render_app_scaled` must have ended up logical
    // VIEW_W×VIEW_H at `scale` — physical = logical × scale.
    let physical = {
        let mut q = app
            .world_mut()
            .query_filtered::<&Window, With<PrimaryWindow>>();
        let window = q.single(app.world()).expect("primary window");
        assert_eq!(
            window.scale_factor(),
            scale,
            "the scale-factor override applied"
        );
        assert_eq!(
            window.resolution.size(),
            Vec2::new(VIEW_W as f32, VIEW_H as f32),
            "the logical size is scale-invariant"
        );
        window.physical_size()
    };
    assert_eq!(
        physical,
        UVec2::new(
            (VIEW_W as f32 * scale) as u32,
            (VIEW_H as f32 * scale) as u32
        ),
        "physical size == logical × scale"
    );

    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(VIEW_TOKEN.into(), viewport_tint());
    }
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontSize(20.0),
            TextColor(ColorToken::Token(Cow::Borrowed(VIEW_TOKEN))),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(VIEW_W as f32)
                .height_px(VIEW_H as f32),
        ))
        .add_child(text);

    // The capture image is sized to the window's PHYSICAL size (the
    // `gpu_render_app_scaled` contract: the view uniform maps logical
    // (0,0)..(w,h) to the full clip square; the target supplies the grid).
    let target = support::render_to_image(&mut app, physical.x, physical.y);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    let pixels = support::readback_rgba(&mut app, target);

    // The re-raster witness: every resident key's atlas cell height, max —
    // the `text_gpu` resource-reading idiom (`ResidentTextKeys` +
    // `BuiyAtlas::get(key).px` from the RenderApp world).
    let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
    let world = render_app.world();
    let atlas = world.resource::<BuiyAtlas>();
    let max_cell_h = world
        .resource::<ResidentTextKeys>()
        .keys
        .iter()
        .map(|key| {
            atlas
                .get(key)
                .expect("every resident key has an atlas entry")
                .px
                .height()
        })
        .max()
        .expect("the fixture emitted at least one glyph");
    (pixels, max_cell_h)
}

#[test]
#[ignore = "needs a wgpu adapter; gate-#2 viewport-axis golden (text campaign T9; glyph-pipeline §§ 3–5 end-to-end)"]
fn scaled_viewport_rerasterizes_at_physical_scale() {
    let (unscaled, cell_h_1x) = capture_viewport(1.0);
    let (scaled_a, cell_h_2x) = capture_viewport(2.0);

    // (a) gate-#2 determinism: the scaled capture against an independent
    // fresh re-capture (the re-capture IS the golden).
    let (scaled_b, _) = capture_viewport(2.0);
    let diff = perceptual_diff(&scaled_a, &scaled_b);
    assert!(
        diff < 1e-4,
        "two fresh scaled captures diverged: perceptual_diff = {diff}"
    );

    // (b) ink present in both frames.
    for (name, frame) in [("unscaled", &unscaled), ("scaled", &scaled_a)] {
        assert!(
            frame.chunks_exact(4).any(|p| p != [0, 0, 0, 255]),
            "{name}: the glyphs painted at least one pixel"
        );
    }

    // (c) the physical re-raster: the scaled app's tallest atlas cell is 2×
    // the unscaled app's (±1 px rounding) — same logical fixture, doubled
    // raster, the § 3–5 "shape logical, rasterize physical" spine end-to-end.
    assert!(
        (i64::from(cell_h_2x) - 2 * i64::from(cell_h_1x)).abs() <= 1,
        "scaled max atlas cell height {cell_h_2x} ≈ 2 × unscaled {cell_h_1x} (±1 px)"
    );

    // (d) alpha-as-color holds at scale: the scaled frame's brightest texel
    // is still the tint's full-coverage encode (the atlas stores coverage,
    // never color, at every scale factor).
    assert_px_approx(
        brightest(&scaled_a),
        full_coverage(viewport_tint()),
        "brightest scaled texel == the tint's full-coverage encode",
    );
}
