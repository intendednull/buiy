//! The Taffy measure seam (measure-and-layout §§ 2–3): content sizing,
//! wrap-on-shrink, intrinsic keywords, the intrinsics cache, and the
//! shape_until_scroll total-height pin. Headless — shaping uses the
//! embedded Fira Sans latin subset; no adapter anywhere.
//!
//! Assertion style: layout tests assert RELATIONS and invariants
//! (min < max, height multiples of line_height, equality against the
//! entity's own cached intrinsics) — never font-metric constants, which
//! belong to the GPU-lane goldens (T4+).

use bevy::prelude::*;
use buiy_core::layout::{
    AlignItems, ContainerQuery, ContainerQueryActive, LayoutPlugin, LayoutTaffyComputeCount,
    Length, QueryCondition, Sizing, Style,
};
use buiy_core::text::{
    BuiyTextPlugin, FontSize, LetterSpacing, SharedFontSystem, Text, TextBuffer,
    TextMeasureCallCount, WhiteSpace,
};
use buiy_core::{CorePlugin, Node, ResolvedLayout};
use cosmic_text::{Attrs, Buffer, Metrics, Shaping};

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

/// Spawn frame + the Added<TextBuffer> echo frame (T2's documented
/// creation echo — the echo re-syncs, re-dirty-marks, and re-measures
/// once; steady state begins on frame 3).
fn settle(app: &mut App) {
    app.update();
    app.update();
}

fn measure_calls(app: &App) -> usize {
    app.world().resource::<TextMeasureCallCount>().0
}

/// A text leaf on the main axis of a flex row sizes to its content:
/// width == ceil(max-content) when it fits, height == one line.
/// (align_items: FlexStart so cross-axis stretch doesn't mask the
/// measured height.)
#[test]
fn text_leaf_sizes_to_content_in_a_flex_row() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("hello world"))))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .width_px(600.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);

    let intrinsics = app
        .world()
        .get::<TextBuffer>(text)
        .unwrap()
        .intrinsics()
        .expect("the first measure call computes and caches intrinsics (§ 3.2)");
    assert!(
        0.0 < intrinsics.min_content && intrinsics.min_content < intrinsics.max_content,
        "two words: longest-word min < unwrapped max; got {intrinsics:?}"
    );

    let layout = app.world().get::<ResolvedLayout>(text).unwrap();
    assert_eq!(
        layout.size.x,
        intrinsics.max_content.ceil(),
        "flex-basis auto = the measured max-content width (it fits in 600)"
    );
    // 16 px × 1.2 line-height = 19.2 → measure ceils to 20.
    assert_eq!(layout.size.y, 20.0, "one line at the default metrics");
}

/// The campaign's wrap-on-shrink row: in a flex column the cross axis
/// stretches the text to the parent width; shrinking the parent re-wraps
/// and ResolvedLayout height GROWS accordingly.
#[test]
fn text_wraps_when_parent_width_shrinks_and_height_grows() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from(
                "a reasonably long sentence that will need several lines",
            )),
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(400.0)
                .height_px(300.0),
        ))
        .add_child(text)
        .id();
    settle(&mut app);
    let wide_height = app.world().get::<ResolvedLayout>(text).unwrap().size.y;

    app.world_mut().entity_mut(parent).insert(
        Style::default()
            .flex_column()
            .width_px(120.0)
            .height_px(300.0),
    );
    settle(&mut app);

    let narrow = app.world().get::<ResolvedLayout>(text).unwrap();
    assert_eq!(narrow.size.x, 120.0, "cross-axis stretch to the new width");
    assert!(
        narrow.size.y > wide_height,
        "narrower box ⇒ more lines ⇒ taller: {} !> {wide_height}",
        narrow.size.y
    );
    // Decision 5 ceils the folded TOTAL (Σ line_height), not each line:
    // n lines of 19.2 px measure as ceil(n × 19.2) — e.g. 4 lines = 77,
    // never n × 20 (per-line ceiling would inflate height past content).
    let lines = (narrow.size.y / 19.2).round();
    assert!(lines >= 2.0, "the narrow box holds multiple lines");
    assert_eq!(
        narrow.size.y,
        (lines * 19.2).ceil(),
        "height is the ceil'd Σ of {lines} 19.2-px lines"
    );
}

/// § 3.3 — Sizing::MinContent/MaxContent on a text leaf resolve from the
/// cached intrinsics (realized in the measure closure; sizing_to_dim
/// still translates the keyword to Dimension::auto).
#[test]
fn intrinsic_keywords_resolve_on_text_leaves() {
    let mut app = text_app();
    let min_leaf = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::MinContent),
            Text(String::from("alpha beta gammaword")),
        ))
        .id();
    let max_leaf = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::MaxContent),
            Text(String::from("alpha beta gammaword")),
        ))
        .id();
    let parent = app.world_mut().spawn((
        Node,
        Style::default()
            .flex_row()
            .align_items(AlignItems::FlexStart)
            .width_px(600.0)
            .height_px(200.0),
    ));
    let parent = parent.id();
    app.world_mut()
        .entity_mut(parent)
        .add_children(&[min_leaf, max_leaf]);
    settle(&mut app);

    let min_intr = app
        .world()
        .get::<TextBuffer>(min_leaf)
        .unwrap()
        .intrinsics()
        .unwrap();
    let min_layout = app.world().get::<ResolvedLayout>(min_leaf).unwrap();
    assert_eq!(
        min_layout.size.x,
        min_intr.min_content.ceil(),
        "MinContent = longest-word width under Wrap::Word"
    );
    assert!(
        min_layout.size.y > 20.0,
        "min-content width wraps the three words onto multiple lines"
    );

    let max_intr = app
        .world()
        .get::<TextBuffer>(max_leaf)
        .unwrap()
        .intrinsics()
        .unwrap();
    let max_layout = app.world().get::<ResolvedLayout>(max_leaf).unwrap();
    assert_eq!(max_layout.size.x, max_intr.max_content.ceil());
    assert_eq!(max_layout.size.y, 20.0, "max-content never wraps");
}

/// CJK: break opportunities exist between characters (unicode-linebreak
/// is character-class-driven — no CJK font coverage needed; the embedded
/// subset renders .notdef but widths are real), so min-content is far
/// below max-content.
#[test]
fn cjk_min_content_breaks_between_characters() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::MinContent),
            Text(String::from("漢字文章測試")),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .width_px(600.0)
                .height_px(300.0),
        ))
        .add_child(text);
    settle(&mut app);

    let intr = app
        .world()
        .get::<TextBuffer>(text)
        .unwrap()
        .intrinsics()
        .unwrap();
    assert!(
        intr.min_content < intr.max_content / 2.0,
        "six CJK chars: min (one char) ≪ max (six chars); got {intr:?}"
    );
}

/// No-break fixture: a single unbreakable word ⇒ min == max.
#[test]
fn single_word_min_content_equals_max_content() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Antidisestablishmentarianism")),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .width_px(600.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);

    let intr = app
        .world()
        .get::<TextBuffer>(text)
        .unwrap()
        .intrinsics()
        .unwrap();
    assert_eq!(intr.min_content, intr.max_content);
}

/// Tab fixture (charter risk 4): in preserve mode tabs advance to the
/// 8-column tab stops, so max-content with a tab exceeds the same text
/// without one.
#[test]
fn preserved_tabs_advance_intrinsic_width() {
    let mut app = text_app();
    let tabbed = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("a\tb")),
            WhiteSpace::Pre,
        ))
        .id();
    let plain = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("ab")),
            WhiteSpace::Pre,
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .width_px(600.0)
                .height_px(100.0),
        ))
        .id();
    app.world_mut()
        .entity_mut(parent)
        .add_children(&[tabbed, plain]);
    settle(&mut app);

    let tabbed_intr = app
        .world()
        .get::<TextBuffer>(tabbed)
        .unwrap()
        .intrinsics()
        .unwrap();
    let plain_intr = app
        .world()
        .get::<TextBuffer>(plain)
        .unwrap()
        .intrinsics()
        .unwrap();
    assert!(tabbed_intr.max_content > plain_intr.max_content);
}

/// A `LetterSpacing(px)` leaf, sized to a parent wide enough that nothing
/// wraps, whose unwrapped `max_content` width.
fn spacing_max_content(app: &mut App, content: &str, font_px: f32, tracking: Option<f32>) -> f32 {
    let mut entity = app.world_mut().spawn((
        Node,
        Style::default(),
        Text(String::from(content)),
        FontSize(font_px),
        // Keep it one line so max_content is the full shaped width.
        WhiteSpace::Pre,
    ));
    if let Some(px) = tracking {
        entity.insert(LetterSpacing(px));
    }
    let leaf = entity.id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .width_px(4000.0)
                .height_px(200.0),
        ))
        .id();
    app.world_mut().entity_mut(parent).add_child(leaf);
    settle(app);

    app.world()
        .get::<TextBuffer>(leaf)
        .unwrap()
        .intrinsics()
        .expect("first measure caches intrinsics")
        .max_content
}

/// parity-prototype A1 (the C3 letter-spacing bug fix): `LetterSpacing(px)`
/// is **logical px** — its on-screen tracking is independent of font size.
/// cosmic-text 0.19's `Attrs::letter_spacing` is **em** (`shape.rs` adds it to
/// the em-unit advance, then multiplies by font-size at width time), so the
/// lowering divides by font-size (`px / font_size`). This pins the contract:
/// the EXTRA width `LetterSpacing(2.0)` adds is the SAME at 10 px and at 30 px
/// (the buggy raw-px lowering made it 3× larger at 30 px — the "S C R E E N S"
/// sprawl). Asserts a relation (delta-at-10 == delta-at-30), never a font
/// constant — the measure-test style (header).
#[test]
fn letter_spacing_adds_the_same_px_tracking_at_every_font_size() {
    let mut app = text_app();
    // A single word (no internal spaces) so max_content == the whole shaped run
    // and Wrap can never split it; 6 glyphs.
    let word = "tracks";
    let tracking = 2.0_f32;

    let plain_10 = spacing_max_content(&mut app, word, 10.0, None);
    let spaced_10 = spacing_max_content(&mut app, word, 10.0, Some(tracking));
    let plain_30 = spacing_max_content(&mut app, word, 30.0, None);
    let spaced_30 = spacing_max_content(&mut app, word, 30.0, Some(tracking));

    let delta_10 = spaced_10 - plain_10;
    let delta_30 = spaced_30 - plain_30;

    // The added tracking is purely the letter-spacing contribution: the glyph
    // advances are identical between the plain/spaced runs at a fixed size, so
    // the delta is `tracking_px × glyph_count` and depends ONLY on px — NOT on
    // font size. Equal at 10 px and 30 px to within rasterization rounding.
    assert!(
        delta_10 > 0.0,
        "LetterSpacing(2.0) must add tracking, got delta {delta_10} @ 10 px"
    );
    assert!(
        (delta_10 - delta_30).abs() < 0.05,
        "px-not-em: the tracking LetterSpacing(2.0) adds must be the SAME at \
         10 px and 30 px — got {delta_10} @ 10 px vs {delta_30} @ 30 px \
         (a 3× gap here would mean the buggy em lowering is back)"
    );
    // And it is a meaningful amount (6 glyphs × 2 px ≈ 10–12 px), not a no-op.
    assert!(
        delta_10 > 6.0,
        "6 glyphs × 2 px of tracking should add ~10 px, got {delta_10}"
    );
}

/// Charter risk 2 — `shape_until_scroll(fs, false)` with `height_opt =
/// None` shapes and lays out ALL lines (scroll_end = ∞, buffer.rs:609).
/// Direct unit-style pin against the real engine.
#[test]
fn shape_until_scroll_with_no_height_lays_out_every_line() {
    let fonts = SharedFontSystem::new();
    let mut font_system = fonts.lock();
    let mut buffer = Buffer::new_empty(Metrics::new(16.0, 20.0));
    let text = (0..100)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    buffer.set_text(&text, &Attrs::new(), Shaping::Advanced, None);
    buffer.set_size(Some(500.0), None);
    buffer.shape_until_scroll(&mut font_system, false);

    let runs: Vec<_> = buffer.layout_runs().collect();
    assert_eq!(runs.len(), 100, "every buffer line produced a layout run");
    let total: f32 = runs.iter().map(|r| r.line_height).sum();
    assert_eq!(total, 100.0 * 20.0, "total height = Σ line_height");
}

/// Site 3 (cq_descendant_rerun) re-entrancy: a container resize seeds the
/// descendant cascade the SAME frame (layout step 8→9); the re-run's
/// compute must measure the text leaf at its NEW width — with plain
/// compute_layout it would zero-collapse (measure § 4.3's named bug).
/// The compute ceiling holds: count == 2 × roots.
#[test]
fn cq_descendant_rerun_remeasures_text_same_frame_within_2x() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from(
                "long enough content to wrap at the narrow width for sure",
            )),
        ))
        .id();
    let mid = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::Cqw(50.0))),
        ))
        .add_child(text)
        .id();
    let container = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(800.0)
                .height_px(600.0)
                .container_size(),
        ))
        .add_child(mid)
        .id();
    settle(&mut app);
    settle(&mut app); // Cqw resolves against the settled snapshot
    let wide_height = app.world().get::<ResolvedLayout>(text).unwrap().size.y;

    // Shrink the container: step 7 surfaces the new size, step 8 seeds
    // mid+text dirty, step 9 re-translates them against the NEW snapshot
    // and recomputes — measuring text at ~200 px THIS frame.
    app.world_mut().entity_mut(container).insert(
        Style::default()
            .flex_column()
            .width_px(400.0)
            .height_px(600.0)
            .container_size(),
    );
    app.update();

    assert_eq!(
        app.world().resource::<LayoutTaffyComputeCount>().0,
        2,
        "descendant-cascade frame: exactly 2 Taffy passes (1 root × 2)"
    );
    assert!(
        measure_calls(&app) > 0,
        "the re-run measured text (site 3 carries the closure)"
    );
    let narrow = app.world().get::<ResolvedLayout>(text).unwrap();
    assert!(
        narrow.size.y >= wide_height && narrow.size.y > 0.0,
        "same-frame re-wrap at the narrower width — never zero-collapsed \
         (got {} after {wide_height})",
        narrow.size.y
    );

    // Let the multi-frame cascade settle, then: steady state.
    settle(&mut app);
    app.update();
    assert_eq!(measure_calls(&app), 0, "steady frame after the cascade");
}

/// Site 2 (cq_flip_rerun) re-entrancy: the activation-flip re-run
/// completes with text in the tree — no deadlock (the lock is scoped per
/// helper call), the 2× cap holds, and the text leaf keeps its measured
/// size through the flip frame.
#[test]
fn cq_flip_rerun_with_text_holds_the_2x_cap() {
    let mut app = text_app();
    // The cq_same_frame_relayout_caps_at_2x_taffy fixture + a text leaf.
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("flip me"))))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
            },
        ))
        .add_child(text)
        .id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(500.0)
                .height_px(400.0)
                .container_size(),
        ))
        .add_child(child)
        .id();
    settle(&mut app);

    app.world_mut().entity_mut(parent).insert(
        Style::default()
            .flex_column()
            .width_px(700.0)
            .height_px(400.0)
            .container_size(),
    );
    app.update(); // the flip frame: taffy_compute + cq_flip_rerun

    assert!(app.world().get::<ContainerQueryActive>(child).is_some());
    assert_eq!(
        app.world().resource::<LayoutTaffyComputeCount>().0,
        2,
        "flip frame runs Taffy exactly twice — measure rode both passes"
    );
    assert!(
        app.world().get::<ResolvedLayout>(text).unwrap().size.y > 0.0,
        "text stayed measured through the flip re-run"
    );
}

/// The instrument: text changes re-measure; the count is per-frame.
#[test]
fn text_change_invokes_measure_and_count_resets_per_frame() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("count me"))))
        .id();
    settle(&mut app);

    app.world_mut().get_mut::<Text>(text).unwrap().0 = String::from("count me again");
    app.update();
    assert!(
        measure_calls(&app) > 0,
        "a content change dirty-marks the node and re-measures"
    );
}
