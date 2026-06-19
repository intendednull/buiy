//! E5 GPU golden (#[ignore], additive — CLAUDE.md GPU lane): a FOCUSED editor
//! with a live preedit composition, driven END-TO-END (Ime::Preedit →
//! apply_ime splice → measure/commit reshape → write_caret_and_selection
//! projects PreeditVisual → extract emits the underline quad → pixels), on a
//! mixed-direction fixture (caret + selection + preedit underline together,
//! spec § 12). The real-IME-per-platform matrix (CJK + dead keys) is named
//! CI-impossible (§ 12) — this golden proves the PAINT path, not winit IME.
//!
//! The assertion is that a distinct preedit-underline color band appears under
//! the composing glyphs: a chroma-distinct `color.text.preedit-underline` theme
//! token (blue) over white glyph ink makes the strip detectable — the E3 golden's
//! white-ink/red-caret classifier idiom (text_caret_selection_e3_gpu.rs).
//!
//! Run: cargo test -p buiy_core --test text_ime_preedit_gpu -- --ignored --test-threads=1

// perceptual_diff is deprecated (use buiy_verify::metric::compare); this
// unmigrated #[ignore] GPU re-capture test joins the migration backlog like the
// other golden suites (follow-ups.md, text verification.md § 4).
#![allow(deprecated)]

use bevy::prelude::*;
use bevy::window::Ime;
use buiy_core::layout::Style;
use buiy_core::render::color::PREEDIT_UNDERLINE_TOKEN;
use buiy_core::render::golden::perceptual_diff;
use buiy_core::text::edit::TextEditState;
use buiy_core::text::{FontSize, Text};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

const W: u32 = 256;
const H: u32 = 64;
/// Glyph tint: white — chroma-orthogonal to the blue preedit underline.
const TEXT_TOKEN: &str = "test.text";

fn underline_blue() -> Color {
    Color::srgb(0.0, 0.2, 1.0)
}

// --- pixel classifiers (the E3 golden's composite math) ---------------------

/// Strong blue — the preedit underline strip (alpha 1, hard-edged quad). The
/// blue channel dominates while red is suppressed, so this rejects both the
/// white glyph ink and the opaque-black clear.
fn is_strong_blue(p: [u8; 4]) -> bool {
    p[2] >= 180 && p[0] <= 80 && p[1] <= 140
}

/// Unselected white glyph ink over black: an achromatic pixel at high coverage.
/// `b ≥ 180 && r ≥ 180` rejects the blue underline strip (its red is ≤ 80).
fn is_white_ink(p: [u8; 4]) -> bool {
    p[0] >= 180 && p[1] >= 180 && p[2] >= 180
}

/// Drive the full Ime::Preedit → splice → reshape → project → extract → paint
/// spine on a FOCUSED editor and read back the rendered texture.
fn capture() -> Vec<u8> {
    // `gpu_render_app` ALREADY adds `BuiyTextPlugin` (support/mod.rs:193) — do
    // NOT re-add it (Bevy panics "plugin was already added"; M2, the E3 GPU
    // test's lesson, text_caret_selection_e3_gpu.rs:96-105). Add only
    // `FocusPlugin` (owns `FocusedEntity`, not in the shared stack) and the
    // `Ime` message (the harness has no winit, so it is not auto-registered).
    let mut app = crate::support::gpu_render_app(W, H);
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.add_message::<Ime>();
    // `FocusPlugin`'s `handle_tab` + the text plugin's `apply_keyboard_edits`
    // read `Res<ButtonInput<KeyCode>>`; the GPU stack has no `InputPlugin`, so
    // insert it (matching the E3 golden, text_caret_selection_e3_gpu.rs:107) —
    // else `handle_tab`'s first run panics on the missing resource.
    app.insert_resource(ButtonInput::<KeyCode>::default());
    // Pause Time<Virtual> from t=0 (the E3 golden idiom): the many real-time
    // `app.update()`s the readback polls drive accrue ZERO virtual elapsed, so a
    // co-painted caret stays solid-on (its blink phase never crosses the 500 ms
    // half-period) and the re-capture determinism check below holds.
    app.world_mut().resource_mut::<Time<Virtual>>().pause();

    // FINISH before any `app.update()` (the `gpu_render_app` contract: the
    // returned App is NOT finished — support/mod.rs:134). `RenderPlugin::finish`
    // inserts the `RenderDevice`, and the RenderApp's first extract runs the
    // once-only `RenderStartup` schedule, whose CorePipeline
    // `init_depth_pyramid_dummy_texture` needs `Res<RenderDevice>`. Updating an
    // un-finished app panics there ("Resource does not exist") BEFORE any Buiy
    // system runs — the E3 golden finishes first for exactly this reason
    // (text_caret_selection_e3_gpu.rs:121).
    crate::support::finish_and_run(&mut app, 0);

    // Chroma-distinct tokens: white ink, blue preedit underline — so the strip is
    // separable from the glyph ink (the E3 white-ink/red-caret idiom).
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(TEXT_TOKEN.into(), Color::WHITE);
        theme
            .colors
            .insert(PREEDIT_UNDERLINE_TOKEN.into(), underline_blue());
    }

    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(240.0).height_px(48.0),
            Text("ab".to_string()),
            FontSize(24.0), // tuple struct (components.rs:97) — no `::px` ctor (m1)
            buiy_core::render::components::TextColor(buiy_core::render::ColorToken::Token(
                std::borrow::Cow::Borrowed(TEXT_TOKEN),
            )),
            TextEditState::new(Metrics::new(24.0, 28.8)),
        ))
        .id();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    // Settle TWICE: TextSync lowers the authored `Text` into the editor-OWNED
    // buffer on the frame AFTER spawn (the campaign N→N+1 measure→commit latency,
    // OQ#1) — the E3 golden's settle discipline (text_caret_selection_e3_gpu.rs).
    app.update();
    app.update();

    // Compose a preedit after "ab".
    app.world_mut().write_message(Ime::Preedit {
        window: Entity::PLACEHOLDER,
        value: "ni".to_string(),
        cursor: Some((0, 2)),
    });
    // Settle the IME spine: apply_ime splices + dirty-marks (Input), the buffer
    // reshapes at N+1, and write_caret_and_selection projects the PreeditVisual
    // seat the extractor's underline emitter reads.
    app.update(); // splice + dirty-mark
    app.update(); // reshape + project PreeditVisual

    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::wait_for_text_ready(&mut app, 60);
    crate::support::readback_rgba(&mut app, target)
}

#[test]
#[ignore = "GPU lane: needs a real wgpu adapter (CLAUDE.md GPU lane)"]
fn preedit_underline_paints_over_the_composing_span() {
    let frame = capture();

    // The composing "ni" renders white glyph ink…
    let white_cols: Vec<u32> = (0..W)
        .filter(|&x| (0..H).any(|y| is_white_ink(crate::support::px(&frame, W, x, y))))
        .collect();
    assert!(
        !white_cols.is_empty(),
        "the composing field renders glyph ink"
    );

    // …with a chroma-distinct blue preedit underline strip beneath it (the seat
    // E5 projects from the live composition). The strip is a thin band, so a
    // presence count is the right assertion (not a column-band shape).
    let blue_px = frame
        .chunks_exact(4)
        .filter(|p| is_strong_blue([p[0], p[1], p[2], p[3]]))
        .count();
    assert!(
        blue_px > 0,
        "the preedit underline paints a chroma-distinct strip under the composing span"
    );

    // Re-capture determinism (the hello_text idiom): an independent fresh capture
    // matches — the re-capture IS the golden.
    let frame_b = capture();
    let diff = perceptual_diff(&frame, &frame_b);
    assert!(diff < 1e-4, "two fresh captures diverged: {diff}");
}
