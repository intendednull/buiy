//! E3 GPU golden (#[ignore], additive — CLAUDE.md GPU lane): a FOCUSED editor's
//! caret, driven END-TO-END through E3's geometry writer (editor cursor state →
//! `write_caret_and_selection` → `CaretVisual` → the T7 caret-stamp paint seat →
//! pixels). Distinct from the T7 `caret_blink_fixed_clock_pair` golden, which
//! hand-authored `CaretVisual`: this proves the E3 writer PRODUCES the seat from
//! editor state. Builds on the `text_selection_caret_gpu` harness + classifiers.
//!
//! ## Scope: caret-narrowed (the plan's sanctioned robust path)
//!
//! The E3 plan (Task 6, Step 6.1 implementer caveat) anticipates that the
//! mixed-BiDi `[10, 18)` selection it sketches cannot be driven deterministically
//! from a headless agent: `Motion::Right` steps in VISUAL order across the BiDi
//! run (cosmic cursor.rs:96 "Move cursor right"), so a fixed right-arrow count
//! does not land on a known LOGICAL byte, and the count cannot be verified without
//! running the GPU lane. The hand-authored `SelectionVisual::new(...)` alternative
//! is genuinely circular here: `write_caret_and_selection` RECOMPUTES the seat
//! from the focused editor's (empty) selection every frame, so it would clobber
//! any literal seat. The plan's explicit non-negotiable is therefore honored:
//! "one #[ignore] golden that exercises the E3 writer producing at least the
//! caret stamp end-to-end … narrow to the caret." The disjoint-selection-rect
//! contract stays proven by the T7 golden (`mixed_bidi_selection_paints_disjoint_
//! rects_and_retints`), whose hand-authored seat is the right tool for that
//! property; this golden proves the orthogonal E3 claim — the WRITER drives the
//! caret seat from editor state.
//!
//! The caret path is BiDi-independent and fully deterministic: type ASCII over a
//! mixed-BiDi corpus, move the caret to the logical line end (`Motion::End`, the
//! rightmost column on this LTR-base line), and the writer stamps a red caret bar
//! right of the white glyph ink. An explicit red `CaretColor` (§ 6.2 tier 1)
//! makes the stamp chroma-distinct from the white ink — the T7 fixture's idiom.
//!
//! Run: cargo test -p buiy_core --test text_caret_selection_e3_gpu -- --ignored --test-threads=1

// perceptual_diff is deprecated (use buiy_verify::metric::compare); this
// unmigrated #[ignore] GPU re-capture test joins the migration backlog like the
// other golden suites (follow-ups.md, text verification.md § 4).
#![allow(deprecated)]

use std::borrow::Cow;
use std::ops::Range;

use bevy::prelude::*;
use buiy_core::layout::Style;
use buiy_core::render::color::{CARET_COLOR_TOKEN, ColorToken};
use buiy_core::render::components::{CaretColor, TextColor};
use buiy_core::render::golden::{GoldenConfig, perceptual_diff};
use buiy_core::text::edit::{EditCommand, TextEditState};
use buiy_core::text::{FamilyEntry, FontFamily, FontSize, FontStack, SharedFontSystem, Text};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::{Metrics, Motion};

const W: u32 = 256;
const H: u32 = 64;
/// Glyph tint: white — chroma-orthogonal to the red caret stamp.
const TEXT_TOKEN: &str = "test.text";

fn caret_red() -> Color {
    Color::srgb(1.0, 0.0, 0.0)
}

// --- pixel classifiers (the T7 golden's composite math) ---------------------

/// Full-strength red — the caret stamp's hard-edged interior (alpha 1, no SDF).
fn is_strong_red(p: [u8; 4]) -> bool {
    p[0] >= 200 && p[1] <= 20 && p[2] <= 20
}

/// Unselected white glyph ink over black: an achromatic pixel at ≥ ~0.61
/// coverage. `g ≥ 180` alone already rejects the red caret stamp.
fn is_white_ink(p: [u8; 4]) -> bool {
    p[0] >= 180 && p[1] >= 180 && p[2] >= 180
}

/// Columns (left→right) where ANY pixel satisfies `pred`.
fn cols_where(pixels: &[u8], w: u32, h: u32, pred: impl Fn([u8; 4]) -> bool) -> Vec<u32> {
    (0..w)
        .filter(|&x| (0..h).any(|y| pred(crate::support::px(pixels, w, x, y))))
        .collect()
}

/// Coalesce sorted indices into contiguous bands.
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

/// Spawn a FOCUSED editor over the T7 mixed-BiDi corpus line, type the text
/// through the editor, and drive the caret to the logical line end. The E3
/// `write_caret_and_selection` writer then mirrors the editor's cursor into a
/// `CaretVisual` (red stamp), and the producer paints it over the white ink.
fn capture() -> Vec<u8> {
    let _cfg = GoldenConfig::deterministic();
    let mut app = crate::support::gpu_render_app(W, H);
    // `FocusedEntity` is owned by `FocusPlugin`, NOT `CorePlugin` (M2) — and the
    // shared `gpu_render_app` stack adds neither it nor any input infra (it is a
    // render harness). E3's `write_caret_and_selection` only drives the FOCUSED
    // editor's caret, so add `FocusPlugin` here (the `caret_app` / latency-
    // fixture precedent) before setting `FocusedEntity` below; without it that
    // set panics on a missing resource. `FocusPlugin` registers `handle_tab` in
    // `BuiySet::Input`, which reads `Res<ButtonInput<KeyCode>>` — insert it (no
    // InputPlugin in this stack); no Tab is sent, so `handle_tab` is inert.
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.world_mut()
        .insert_resource(ButtonInput::<KeyCode>::default());
    // Realize GoldenConfig::deterministic()'s fixed_clock (the T7 spawn_blink_
    // fixture idiom, text_selection_caret_gpu.rs:237): PAUSE Time<Virtual> from
    // t=0 so the many real-time app.update()s the readback polls drive accrue
    // ZERO virtual elapsed. write_caret_blink reads `now − CaretBlink.origin`
    // (visual.rs); with a paused clock that phase stays ~0 (the visible half-
    // period) across both captures, so the freshly-reset blink origin keeps the
    // caret solid-on through readback. Without this, accumulated wall-clock
    // crosses the 500 ms half-period mid-poll and the caret blinks HIDDEN →
    // zero strong-red pixels → the single-band assertion fails (and run-to-run
    // jitter straddles a blink edge, breaking the re-capture determinism check).
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    // Finish BEFORE registering the fixture font (the T7 idiom): a pre-finish
    // update would run the render schedule without the device/PipelineCache.
    crate::support::finish_and_run(&mut app, 0);
    crate::support::register_fixture_font(
        &mut app,
        "Noto Sans Hebrew",
        "NotoSansHebrew-hebrew.ttf",
    );
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(TEXT_TOKEN.into(), Color::WHITE);
        theme.colors.insert(CARET_COLOR_TOKEN.into(), caret_red());
    }
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("hello עולם world")),
            FontFamily(FontStack(vec![
                FamilyEntry::Named(String::from("Fira Sans")),
                FamilyEntry::Named(String::from("Noto Sans Hebrew")),
            ])),
            FontSize(20.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
            CaretColor(ColorToken::Token(Cow::Borrowed(CARET_COLOR_TOKEN))),
            TextEditState::new(Metrics::new(20.0, 24.0)),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(W as f32)
                .height_px(H as f32),
        ))
        .add_child(editor);
    // Settle TWICE: the sync path lowers the authored `Text` into the editor-
    // OWNED buffer on the frame AFTER spawn (the first update measures/commits an
    // empty editor buffer; the second seeds + reshapes "hello עולם world"). A
    // single update would leave the editor buffer empty, so `Motion::End` below
    // would land at index 0 (the caret stuck at the left edge — the assertion's
    // failure mode). Mirrors the headless caret fixtures' settle discipline.
    app.update();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    // Drive the editor to the logical line END (deterministic, BiDi-independent:
    // the rightmost column on this LTR-base line). The E3 writer mirrors the
    // resulting cursor into the CaretVisual — purely editor-driven, end-to-end.
    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        state.apply(
            &mut fs,
            EditCommand::Motion(Motion::End, false),
            false,
            false,
        );
    }
    app.update(); // E3 writer mirrors the caret into CaretVisual

    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::wait_for_text_ready(&mut app, 60);
    crate::support::readback_rgba(&mut app, target)
}

#[test]
#[ignore = "needs a wgpu adapter; E3 caret end-to-end golden (editing-and-ime §§ 4, 5; verification § 12)"]
fn e3_editor_driven_caret_paints_a_bar_right_of_the_ink() {
    let frame = capture();

    // The E3 writer produced a CaretVisual from the editor cursor: exactly one
    // hard-edged red caret column band, § 3.3-snapped to ≥ 1 physical px.
    let caret_cols = {
        let caret_bands = bands(&cols_where(&frame, W, H, is_strong_red));
        assert_eq!(
            caret_bands.len(),
            1,
            "exactly one E3-driven caret column band: {caret_bands:?}"
        );
        caret_bands.into_iter().next().unwrap()
    };
    assert!(
        caret_cols.end - caret_cols.start >= 1,
        "the caret bar is a ≥ 1 physical-px column: {caret_cols:?}"
    );

    // The caret sits at the logical line END — right of all the white glyph ink.
    let white_cols = cols_where(&frame, W, H, is_white_ink);
    let last_ink = *white_cols.last().expect("the glyph ink painted");
    assert!(
        caret_cols.start > last_ink,
        "the end-of-line caret sits right of the glyph ink (caret {caret_cols:?}, ink to {last_ink})"
    );

    // Re-capture determinism (the hello_text idiom): an independent fresh
    // capture matches — the re-capture IS the golden.
    let frame_b = capture();
    let diff = perceptual_diff(&frame, &frame_b);
    assert!(diff < 1e-4, "two fresh captures diverged: {diff}");
}

/// Drive the FOCUSED editor's caret to the mixed-corpus DIRECTION BOUNDARY (the
/// LTR↔RTL join) so the E3 writer stamps BOTH the primary caret and the
/// secondary split-caret indicator (§§ 4.1, 5). The boundary byte index is
/// DATA-DERIVED from the shaped buffer (not hand-counted), then reached with
/// LOGICAL-order `Motion::Next` steps (NOT the §Scope-flagged visual-order
/// `Right`): `Motion::Next` advances in logical order, so looping until
/// `state.caret().index` equals the derived boundary is fully deterministic.
fn capture_boundary() -> Vec<u8> {
    let _cfg = GoldenConfig::deterministic();
    let mut app = crate::support::gpu_render_app(W, H);
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.world_mut()
        .insert_resource(ButtonInput::<KeyCode>::default());
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    crate::support::finish_and_run(&mut app, 0);
    crate::support::register_fixture_font(
        &mut app,
        "Noto Sans Hebrew",
        "NotoSansHebrew-hebrew.ttf",
    );
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(TEXT_TOKEN.into(), Color::WHITE);
        theme.colors.insert(CARET_COLOR_TOKEN.into(), caret_red());
    }
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("hello עולם world")),
            FontFamily(FontStack(vec![
                FamilyEntry::Named(String::from("Fira Sans")),
                FamilyEntry::Named(String::from("Noto Sans Hebrew")),
            ])),
            FontSize(20.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
            CaretColor(ColorToken::Token(Cow::Borrowed(CARET_COLOR_TOKEN))),
            TextEditState::new(Metrics::new(20.0, 24.0)),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(W as f32)
                .height_px(H as f32),
        ))
        .add_child(editor);
    app.update();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        // DATA-DERIVE the first LTR↔RTL boundary byte index from the editor's
        // own shaped buffer (the same convention the headless geometry unit
        // pins).
        let boundary = state
            .with_buffer(|b| {
                b.layout_runs().next().and_then(|run| {
                    run.glyphs.iter().find_map(|before| {
                        run.glyphs.iter().find_map(|after| {
                            (after.start == before.end
                                && after.level.is_rtl() != before.level.is_rtl())
                            .then_some(before.end)
                        })
                    })
                })
            })
            .expect("the mixed corpus has a direction boundary");

        let mut fs = fonts.lock();
        state.apply(
            &mut fs,
            EditCommand::Motion(Motion::Home, false),
            false,
            false,
        );
        // Walk LOGICAL-order Next until the caret lands on the boundary index.
        // Bounded to avoid a runaway if the buffer changes shape.
        for _ in 0..64 {
            if state.caret().index == boundary {
                break;
            }
            state.apply(
                &mut fs,
                EditCommand::Motion(Motion::Next, false),
                false,
                false,
            );
        }
        drop(fs);
        assert_eq!(
            state.caret().index,
            boundary,
            "logical Next walk reached the derived boundary index"
        );
    }
    app.update(); // E3 writer mirrors the caret into CaretVisual (+ secondary)

    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::wait_for_text_ready(&mut app, 60);
    crate::support::readback_rgba(&mut app, target)
}

#[test]
#[ignore = "needs a wgpu adapter; E3 split-caret secondary indicator golden (editing-and-ime §§ 4.1, 5)"]
fn e3_split_caret_paints_primary_plus_secondary_at_a_bidi_boundary() {
    let frame = capture_boundary();

    // At the direction boundary the writer stamps TWO red caret bands: the
    // primary full-height bar plus the shorter secondary indicator. They may
    // share a column or sit apart depending on the reordered run geometry, so
    // assert the COUNT of bands, not their separation.
    let caret_bands = bands(&cols_where(&frame, W, H, is_strong_red));
    assert_eq!(
        caret_bands.len(),
        2,
        "two E3 split-caret column bands at the bidi boundary: {caret_bands:?}"
    );

    // The secondary indicator is SHORTER in row-extent than the full-height
    // primary: measure each band's vertical red span and assert one is strictly
    // taller (the primary) than the other (the secondary).
    let row_span = |band: &Range<u32>| -> u32 {
        let rows: Vec<u32> = (0..H)
            .filter(|&y| {
                (band.start..band.end).any(|x| is_strong_red(crate::support::px(&frame, W, x, y)))
            })
            .collect();
        match (rows.first(), rows.last()) {
            (Some(&lo), Some(&hi)) => hi - lo + 1,
            _ => 0,
        }
    };
    let mut spans: Vec<u32> = caret_bands.iter().map(row_span).collect();
    spans.sort_unstable();
    assert!(
        spans[0] < spans[1],
        "the secondary indicator is shorter than the full-height primary caret: {spans:?}"
    );

    // Re-capture determinism: an independent fresh capture matches.
    let frame_b = capture_boundary();
    let diff = perceptual_diff(&frame, &frame_b);
    assert!(diff < 1e-4, "two fresh boundary captures diverged: {diff}");
}
