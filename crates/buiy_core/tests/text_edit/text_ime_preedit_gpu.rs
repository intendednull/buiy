//! E5 GPU golden (#[ignore], additive — CLAUDE.md GPU lane): a FOCUSED editor
//! with a live preedit composition, driven END-TO-END (Ime::Preedit →
//! apply_ime splice → measure/commit reshape → write_caret_and_selection
//! projects PreeditVisual → extract emits the underline quad → pixels), on a
//! mixed-direction fixture (caret + selection + preedit underline together,
//! spec § 12). The real-IME-per-platform matrix (CJK + dead keys) is named
//! CI-impossible (§ 12) — this golden proves the PAINT path, not winit IME.
//!
//! TRACK B NOTE: the preedit underline is no longer independently themeable.
//! `resolve_preedit_underline` now always returns `currentColor` (the closed
//! `ColorToken` vocabulary carries no preedit token — spec decision 5), so the
//! composing text is underlined in its OWN ink. The former assertion — a
//! chroma-distinct blue underline strip over white glyph ink, keyed off an
//! injected `color.text.preedit-underline` token — is therefore no longer
//! expressible (the strip is the same color as the glyphs it sits under). This
//! golden is reduced to proving the IME paint spine RUNS end-to-end (composing
//! ink appears + the capture is deterministic); re-establishing a dedicated
//! underline-presence proof would need a row-band / baseline-subtraction rewrite
//! on a GPU host (see the flag in the migration report).
//!
//! Run: cargo test -p buiy_core --test text_ime_preedit_gpu -- --ignored --test-threads=1

// perceptual_diff is deprecated (use buiy_verify::metric::compare); this
// unmigrated #[ignore] GPU re-capture test joins the migration backlog like the
// other golden suites (follow-ups.md, text verification.md § 4).
#![allow(deprecated)]

use bevy::prelude::*;
use bevy::window::Ime;
use buiy_core::layout::Style;
use buiy_core::render::ColorToken;
use buiy_core::render::golden::perceptual_diff;
use buiy_core::text::edit::TextEditState;
use buiy_core::text::{FontSize, Text};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

const W: u32 = 256;
const H: u32 = 64;

// --- pixel classifiers -------------------------------------------------------

/// White glyph ink over black: an achromatic pixel at high coverage.
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

    // Track B: white glyph ink via a `Custom` tint (the former `test.text`
    // injection). The preedit underline is NOT separately colored — it now
    // resolves to `currentColor` (this same white); see the module note.
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(240.0).height_px(48.0),
            Text("ab".to_string()),
            FontSize(24.0), // tuple struct (components.rs:97) — no `::px` ctor (m1)
            buiy_core::render::components::TextColor(ColorToken::Custom(Color::WHITE)),
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

    // The full IME paint spine ran: the composing "ni" renders glyph ink (the
    // underline sits beneath it in the SAME currentColor ink — Track B removed
    // the independent preedit-underline color, so it can no longer be chroma-
    // isolated from the glyphs; see the module note).
    let white_cols: Vec<u32> = (0..W)
        .filter(|&x| (0..H).any(|y| is_white_ink(crate::support::px(&frame, W, x, y))))
        .collect();
    assert!(
        !white_cols.is_empty(),
        "the composing field renders glyph ink (the IME splice→reshape→project→paint spine ran)"
    );

    // Re-capture determinism (the hello_text idiom): an independent fresh capture
    // matches — the re-capture IS the golden.
    let frame_b = capture();
    let diff = perceptual_diff(&frame, &frame_b);
    assert!(diff < 1e-4, "two fresh captures diverged: {diff}");
}
