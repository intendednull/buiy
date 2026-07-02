//! E6 Task 8 — GPU golden (editing-and-ime § 10, decoration-and-paint § 7):
//! an empty editor with a `Placeholder` paints the placeholder string in the
//! `color.text.placeholder` token; after typing, it paints the typed ink in
//! the normal text color and the placeholder is gone. `#[ignore]` — needs a
//! wgpu adapter; the orchestrator runs the GPU lane. Build-only here.
//!
//! The editor is FOCUSED, so every captured state also paints a solid-stamp
//! caret — independent of the placeholder. A `> 0` ink threshold is therefore
//! satisfied by the caret alone and proves nothing about the placeholder /
//! typed glyphs. To make this golden actually guard the glyph paint, we capture
//! a third FOCUSED-but-placeholder-less `CaretOnly` baseline and assert the
//! placeholder / typed states add ink BEYOND that bare caret, and that the
//! placeholder and typed captures differ materially.
//!
//! Hard-won E5 lesson: a `gpu_render_app` is NOT finished — call
//! `crate::support::finish_and_run(.., 0)` BEFORE `register_fixture_font` (a pre-finish
//! update runs the render schedule with no `RenderDevice`/`PipelineCache` and
//! panics), then spawn the capture camera AFTER the content settles.
//!
//! Run: cargo test -p buiy_core --test text_placeholder_gpu -- --ignored --test-threads=1

// perceptual_diff is deprecated (use buiy_verify::metric::compare); this
// unmigrated #[ignore] GPU re-capture test joins the migration backlog like the
// other golden suites (follow-ups.md, text verification.md § 4).
#![allow(deprecated)]

use bevy::prelude::*;
use buiy_core::layout::Style;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::TextColor;
use buiy_core::render::golden::{GoldenConfig, perceptual_diff};
use buiy_core::text::edit::{EditCommand, Placeholder, TextEditState};
use buiy_core::text::{FamilyEntry, FontFamily, FontSize, FontStack, SharedFontSystem, Text};
use buiy_core::{FocusedEntity, Node};

const W: u32 = 256;
const H: u32 = 48;
// A registerable fixture (NOT the embedded default Fira Sans). Use Hebrew so the
// placeholder + typed glyphs resolve in THIS font, the E3/E5 golden precedent.
const FIXTURE_FAMILY: &str = "Noto Sans Hebrew";
const FIXTURE_FILE: &str = "NotoSansHebrew-hebrew.ttf";
const PLACEHOLDER_TEXT: &str = "שלום"; // "hello" — resolves in the Hebrew fixture
const TYPED_TEXT: &str = "עולם"; // "world"

/// What to capture. The states are deliberately layered so the assertions can
/// SUBTRACT the focused caret stamp from the painted text: every state below is
/// focused (so all share the same solid-stamp caret ink), and only the
/// placeholder / typed legs add glyph ink on top of it.
#[derive(Clone, Copy)]
enum State {
    /// Focused editor, empty buffer, NO `Placeholder` component — the
    /// caret-stamp-only baseline. This is the ink floor every other state must
    /// exceed; without subtracting it, a `> 0` threshold is satisfied by the
    /// caret alone and proves nothing about the placeholder / typed glyphs.
    CaretOnly,
    /// Focused editor, empty buffer, WITH a `Placeholder` — caret + placeholder
    /// glyphs.
    Placeholder,
    /// Focused editor, `Placeholder` present but TYPED_TEXT inserted — caret +
    /// typed glyphs, placeholder suppressed (value no longer empty).
    Typed,
}

/// Capture the field in one of the three [`State`]s on the readback harness.
fn capture(state: State) -> Vec<u8> {
    let _cfg = GoldenConfig::deterministic();
    let mut app = crate::support::gpu_render_app(W, H);
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.world_mut()
        .insert_resource(ButtonInput::<KeyCode>::default());
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    crate::support::finish_and_run(&mut app, 0);
    crate::support::register_fixture_font(&mut app, FIXTURE_FAMILY, FIXTURE_FILE);
    // Track B: the text tint is a `Custom` white (the former `color.text.primary`
    // injection). The placeholder tint is no longer injectable — it resolves
    // through the default theme's `ColorToken::TextPlaceholder` (light-theme
    // default `srgb(0.55, 0.55, 0.55)`, byte-identical to the value this test
    // used to inject), so the placeholder pixel is preserved.
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(W as f32)
                .height_px(H as f32)
                .padding(4.0),
            Text(String::new()),
            FontFamily(FontStack(vec![FamilyEntry::Named(FIXTURE_FAMILY.into())])),
            FontSize(20.0),
            TextColor(ColorToken::Custom(Color::WHITE)),
            TextEditState::for_font_size(20.0),
        ))
        .id();
    // The caret-only baseline carries NO Placeholder; the other two states do.
    if !matches!(state, State::CaretOnly) {
        app.world_mut()
            .entity_mut(editor)
            .insert(Placeholder(String::from(PLACEHOLDER_TEXT)));
    }
    app.update();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    if matches!(state, State::Typed) {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        state.apply(
            &mut fs,
            EditCommand::Insert(TYPED_TEXT.to_string()),
            true,
            false,
        );
    }
    app.update();
    app.update();

    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::wait_for_text_ready(&mut app, 60);
    crate::support::readback_rgba(&mut app, target)
}

/// Count non-background pixels (any pixel with ink) — a crude ink presence
/// classifier. Background is the canvas clear (transparent / black).
fn ink_pixels(px: &[u8]) -> usize {
    px.chunks_exact(4)
        .filter(|p| p[0] as u16 + p[1] as u16 + p[2] as u16 > 24)
        .count()
}

#[test]
#[ignore = "needs a wgpu adapter; run on the GPU lane (CLAUDE.md § GPU lane)"]
fn placeholder_paints_when_empty_and_ink_when_typed() {
    // All three states are FOCUSED, so all three paint the same solid-stamp
    // caret. The caret-only baseline isolates that caret ink; the placeholder /
    // typed states must add glyph ink ON TOP of it. A bare `> 0` threshold would
    // pass on the caret alone (empirically ~24 px) and prove nothing about the
    // placeholder / typed glyphs — so we subtract the baseline instead.
    let caret_only = capture(State::CaretOnly);
    let empty = capture(State::Placeholder);
    let typed = capture(State::Typed);

    let caret_ink = ink_pixels(&caret_only);
    assert!(
        ink_pixels(&empty) > caret_ink,
        "the placeholder adds glyph ink beyond the bare caret \
         (empty={}, caret_only={})",
        ink_pixels(&empty),
        caret_ink,
    );
    assert!(
        ink_pixels(&typed) > caret_ink,
        "the typed value adds glyph ink beyond the bare caret \
         (typed={}, caret_only={})",
        ink_pixels(&typed),
        caret_ink,
    );
    // The placeholder and the typed value paint MATERIALLY different pixels
    // (placeholder gone once the value is non-empty; typed glyphs in the text
    // color). A confounded test where only the shared caret painted would leave
    // these two nearly identical.
    assert!(
        perceptual_diff(&empty, &typed) > 1e-3,
        "placeholder vs typed differ materially (the placeholder is replaced \
         by the typed ink), not just the shared caret"
    );

    // Determinism: re-capture a state in a fresh app and diff.
    let empty_b = capture(State::Placeholder);
    assert!(
        perceptual_diff(&empty, &empty_b) < 1e-4,
        "empty capture is deterministic"
    );
}
