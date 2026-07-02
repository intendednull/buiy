//! Determinism self-tests (Phase 3.5, verification-design `determinism.md`
//! § Verification #1/#2). All `#[ignore]` — they need a wgpu adapter (real GPU
//! locally / pinned lavapipe in CI). The headless gate stays green WITHOUT
//! these.
//!
//! Run: cargo test -p buiy_verify --test determinism_capture -- --ignored \
//!        --test-threads=1
//!
//! #1 IDEMPOTENT CAPTURE (the headline proof): the SAME scene captured TWICE
//! through two fresh `DeterministicApp`s is byte-identical — `compare(a, b,
//! default).passes(EXACT)` at budget `(0, 0)`. This is the direct proof the
//! knobs actually pin the output; if any nondeterminism leaked, the two
//! captures would diverge.
//!
//! #2 KNOB SENSITIVITY (negatives): flipping each knob CHANGES the bytes, so
//! the knobs are load-bearing, not no-ops.

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::ColorToken;
use buiy_core::render::components::{Background, TextColor};
use buiy_core::text::{FontSize, Text};
use buiy_verify::determinism::{DeterministicApp, Dpr, FontMode};
use buiy_verify::metric::{CompareOpts, FuzzBudget, compare};

/// A known opaque rounded fill on a black ground — an edge-bearing fixture so
/// the SDF analytic AA rim exercises the float path the determinism stack pins.
fn rect_fixture(app: &mut App) {
    let fill = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(8.0)),
                    left: Sizing::Length(Length::px(8.0)),
                    ..default()
                })
                .width_px(32.0)
                .height_px(24.0),
            Background {
                color: ColorToken::Custom(Color::srgb(0.20, 0.65, 0.90)),
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[fill]);
}

/// A text fixture under `font-family: Ahem` so the box-font substitution is
/// exercised. The big size guarantees full-coverage interior texels.
fn text_fixture(app: &mut App) {
    use buiy_core::text::{FamilyEntry, FontFamily, FontStack};
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontFamily(FontStack(vec![FamilyEntry::Named(String::from("Ahem"))])),
            FontSize(28.0),
            TextColor(ColorToken::Custom(Color::srgb(0.95, 0.40, 0.20))),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(48.0)
                .height_px(48.0),
        ))
        .add_child(text);
}

// ---------------------------------------------------------------------------
// #1 — idempotent capture: the same scene twice is bit-identical at (0, 0).
// ---------------------------------------------------------------------------

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn idempotent_capture() {
    // Two fresh DeterministicApps, identical fixture. Every nondeterminism knob
    // is pinned, so the two captures must be byte-identical.
    let a = DeterministicApp::new(48, 40).capture(rect_fixture);
    let b = DeterministicApp::new(48, 40).capture(rect_fixture);

    assert_eq!(
        a.dimensions(),
        b.dimensions(),
        "same logical size, same dpr"
    );
    let diff = compare(&a, &b, &CompareOpts::default());
    assert!(
        diff.passes(&FuzzBudget::EXACT),
        "two fresh DeterministicApp captures of the SAME scene diverged — \
         determinism leaked. differing_pixels={}, max_channel_delta={}",
        diff.differing_pixels,
        diff.max_channel_delta,
    );
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn idempotent_capture_text_under_ahem() {
    // The same proof for a TEXT scene: the Ahem box-font substitution makes the
    // two captures byte-identical (the box-font collapse holds frame-to-frame).
    let a = DeterministicApp::new(48, 48).capture(text_fixture);
    let b = DeterministicApp::new(48, 48).capture(text_fixture);

    let diff = compare(&a, &b, &CompareOpts::default());
    assert!(
        diff.passes(&FuzzBudget::EXACT),
        "two fresh Ahem-text captures diverged — differing_pixels={}",
        diff.differing_pixels,
    );
    // Non-vacuous: the text actually painted (not a blank frame passing
    // trivially).
    assert!(
        a.pixels().any(|p| p.0 != [0, 0, 0, 255]),
        "the Ahem text painted at least one non-clear pixel"
    );
}

// ---------------------------------------------------------------------------
// The brief's second verification: a text scene under FontMode::Ahem renders
// identically regardless of font availability. We prove host-independence by
// capturing the SAME Ahem text scene through two apps that differ in whether
// extra host-style families were registered: the result is identical because
// Ahem is the sole resolvable family the stack names.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn ahem_text_is_font_availability_invariant() {
    use buiy_core::text::{FontFaceDescriptors, FontRegistry};
    use std::sync::Arc;

    // Baseline: the plain Ahem-text capture.
    let baseline = DeterministicApp::new(48, 48).capture(text_fixture);

    // A second capture where an EXTRA family (the embedded Fira bytes under a
    // different name) is also registered — simulating a host that has more
    // fonts. Because the fixture names only "Ahem", the extra family can never
    // win, so the pixels must be identical.
    let with_extra = DeterministicApp::new(48, 48).capture(|app| {
        // Register an extra resolvable family BEFORE the fixture text.
        let extra: Arc<Vec<u8>> = Arc::new(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../buiy_core/tests/fixtures/fonts/NotoSansHebrew-hebrew.ttf"
            ))
            .expect("the Hebrew fixture subset is committed"),
        );
        app.world_mut()
            .resource_mut::<FontRegistry>()
            .register_bytes("Some Host Font", extra, FontFaceDescriptors::default());
        text_fixture(app);
    });

    let diff = compare(&baseline, &with_extra, &CompareOpts::default());
    assert!(
        diff.passes(&FuzzBudget::EXACT),
        "Ahem text changed when an extra host font was available — the box-font \
         substitution is NOT host-independent. differing_pixels={}",
        diff.differing_pixels,
    );
}

// ---------------------------------------------------------------------------
// #2 — knob sensitivity: each knob flip changes the bytes.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn knob_sensitivity_dpr() {
    // 1× vs 2× is a different rasterization (different physical pixel grid), so
    // the images differ — the metric's dimension-mismatch sentinel saturates.
    let one_x = DeterministicApp::new(48, 40)
        .dpr(Dpr::X1)
        .capture(rect_fixture);
    let two_x = DeterministicApp::new(48, 40)
        .dpr(Dpr::X2)
        .capture(rect_fixture);

    assert_ne!(
        one_x.dimensions(),
        two_x.dimensions(),
        "2× capture is physically larger than 1× (the DPR axis is real)"
    );
    let diff = compare(&one_x, &two_x, &CompareOpts::default());
    assert!(
        !diff.passes(&FuzzBudget::EXACT),
        "dpr(X1) and dpr(X2) captures must differ — the DPR knob is a no-op"
    );
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn knob_sensitivity_font_mode() {
    // Real vs Ahem of the SAME text fixture differ: the box-font rasterizes
    // solid em-squares, the real face rasterizes glyph outlines.
    let ahem = DeterministicApp::new(48, 48)
        .font_mode(FontMode::Ahem)
        .capture(text_fixture);
    // FontMode::Real does NOT stage Ahem; the fixture names "Ahem" which is not
    // registered, so the stack falls through to the embedded default face —
    // genuine glyph outlines, a visibly different image.
    let real = DeterministicApp::new(48, 48)
        .font_mode(FontMode::Real)
        .capture(text_fixture);

    assert_eq!(
        ahem.dimensions(),
        real.dimensions(),
        "same logical size + dpr"
    );
    let diff = compare(&ahem, &real, &CompareOpts::default());
    assert!(
        !diff.passes(&FuzzBudget::EXACT),
        "FontMode::Real and FontMode::Ahem captures of the same text must \
         differ — the font-mode knob is a no-op. differing_pixels={}",
        diff.differing_pixels,
    );
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn msaa_is_inert_for_the_in_shader_aa_pipeline() {
    use buiy_core::render::golden::{CAPTURE_MSAA, capture_app, readback_rgba_into};

    // The MSAA pin's rationale, VERIFIED (determinism.md): Buiy antialiases the
    // SDF analytically in-shader and paints axis-aligned, pixel-covering quads,
    // so a hardware MSAA *resolve* is identity for this pipeline — it changes
    // nothing while costing cross-driver determinism. CAPTURE_MSAA pins it OFF
    // to remove that risk; here we confirm it is genuinely a no-op (a 4× capture
    // is byte-identical to the single-sampled one), which is exactly WHY the pin
    // is free. (MSAA is a module constant, not a DeterministicApp knob, so this
    // drives the capture camera directly.)
    assert_eq!(CAPTURE_MSAA, bevy::render::view::Msaa::Off);

    let pinned = capture_at_msaa(bevy::render::view::Msaa::Off);
    let four_x = capture_at_msaa(bevy::render::view::Msaa::Sample4);

    let diff = compare(&pinned, &four_x, &CompareOpts::default());
    assert!(
        diff.passes(&FuzzBudget::EXACT),
        "4× MSAA changed the in-shader-AA pipeline's output — the MSAA pin is \
         NOT free; revisit the determinism.md claim. differing_pixels={}, \
         max_channel_delta={}",
        diff.differing_pixels,
        diff.max_channel_delta,
    );
    // Non-vacuous: the fixture actually painted (both captures are real frames).
    assert!(
        pinned.pixels().any(|p| p.0 != [0, 0, 0, 255]),
        "the rect fixture painted at least one non-clear pixel"
    );

    // Inline capture at an explicit MSAA, mirroring capture_to_image's offscreen
    // target setup but with a caller-chosen sample count on the capture camera.
    fn capture_at_msaa(msaa: bevy::render::view::Msaa) -> image::RgbaImage {
        use bevy::asset::RenderAssetUsages;
        use bevy::camera::RenderTarget;
        use bevy::image::Image;
        use bevy::render::render_resource::{TextureFormat, TextureUsages};

        const W: u32 = 48;
        const H: u32 = 40;
        let mut app = capture_app(W, H);
        rect_fixture(&mut app);

        let target = {
            let mut image = Image::new_target_texture(W, H, TextureFormat::Rgba8UnormSrgb, None);
            image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
            image.asset_usage = RenderAssetUsages::all();
            app.world_mut().resource_mut::<Assets<Image>>().add(image)
        };
        app.world_mut().spawn((
            Camera2d,
            RenderTarget::from(target.clone()),
            msaa,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
        ));
        app.finish();
        app.cleanup();
        for _ in 0..4 {
            app.update();
        }
        let bytes = readback_rgba_into(&mut app, &target, W, H);
        image::RgbaImage::from_raw(W, H, bytes).expect("W*H*4 bytes")
    }
}
