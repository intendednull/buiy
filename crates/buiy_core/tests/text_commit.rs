//! `BuiyLayoutStep::TextCommit` (measure-and-layout §§ 4.2, 5.3, 6;
//! architecture §§ 3.3, 4.2): reshape at the final content-box, align at
//! commit, idempotent output writes, and the steady-state instruments.

use bevy::prelude::*;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::{
    BuiyTextPlugin, ComputedTextLayout, FontsGeneration, ResolvedBaseline, Text, TextAlign,
    TextBuffer, TextCommitReshapeCount, TextMeasureCallCount,
};
use buiy_core::{BuiySet, CorePlugin, Node, ResolvedLayout};
use cosmic_text::Align;

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

fn settle(app: &mut App) {
    app.update();
    app.update();
}

/// § 4.2 — the buffer's last measured width can differ from the laid-out
/// width (stretch): commit reconciles to the final CONTENT-BOX. In a flex
/// column the leaf stretches to the parent width — wider than its
/// max-content — and measure left height_opt = None; commit must end with
/// buffer.size() == (Some(parent_w), Some(content_h)).
#[test]
fn commit_reshapes_the_buffer_to_the_final_content_box() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("short"))))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);

    let buffer = &app.world().get::<TextBuffer>(text).unwrap().buffer;
    let (w, h) = buffer.size();
    assert_eq!(w, Some(300.0), "stretched to the parent content width");
    assert_eq!(h, Some(20.0), "committed height = the ceil'd measured line");
}

/// § 6 — ComputedTextLayout carries the per-line LayoutRun geometry and
/// ResolvedBaseline carries first/last line_y.
#[test]
fn commit_writes_computed_layout_and_baseline() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("first line second line third line")),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(80.0)
                .height_px(200.0),
        ))
        .add_child(text);
    settle(&mut app);

    let computed = app.world().get::<ComputedTextLayout>(text).unwrap();
    assert!(computed.lines.len() > 1, "80 px wraps the three 'words'");
    assert!(computed.size.x > 0.0 && computed.size.y > 0.0);
    let baseline = app.world().get::<ResolvedBaseline>(text).unwrap();
    assert_eq!(baseline.first, computed.lines.first().unwrap().line_y);
    assert_eq!(baseline.last, computed.lines.last().unwrap().line_y);
    assert!(
        baseline.last > baseline.first,
        "multi-line: baselines descend"
    );
}

/// § 5.3 — align is applied AT COMMIT, per line, against the final width;
/// and the Some→None transition works (set_text(None) leaves reused
/// lines' align untouched — only the commit loop can clear it).
#[test]
fn align_applies_at_commit_and_clears_back_to_start() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("centered")),
            TextAlign::Center,
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);

    {
        let buffer = &app.world().get::<TextBuffer>(text).unwrap().buffer;
        assert!(
            buffer
                .lines
                .iter()
                .all(|l| l.align() == Some(Align::Center)),
            "every line carries the committed align"
        );
        let run = buffer.layout_runs().next().expect("one run");
        assert!(
            run.glyphs.first().expect("glyphs").x > 0.0,
            "centered in a 300-px box: the first glyph is offset from x=0"
        );
    }

    app.world_mut().entity_mut(text).insert(TextAlign::Start);
    settle(&mut app);
    let buffer = &app.world().get::<TextBuffer>(text).unwrap().buffer;
    assert!(
        buffer.lines.iter().all(|l| l.align().is_none()),
        "Start → None: the commit loop owns the Some→None transition"
    );
}

#[derive(Resource, Default)]
struct LayoutTickCount(usize);

fn count_layout_ticks(
    mut count: ResMut<LayoutTickCount>,
    changed: Query<(), Changed<ComputedTextLayout>>,
) {
    count.0 += changed.iter().count();
}

/// The campaign's moved-from-T2 test: ComputedTextLayout is
/// idempotent-insert (architecture § 3.3) — a steady frame leaves the
/// change tick untouched; a real change ticks exactly once.
#[test]
fn computed_text_layout_write_is_idempotent() {
    let mut app = text_app();
    app.init_resource::<LayoutTickCount>();
    app.add_systems(Update, count_layout_ticks.after(BuiySet::Layout));
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("tick discipline")),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);
    let after_settle = app.world().resource::<LayoutTickCount>().0;
    assert_eq!(
        after_settle, 1,
        "exactly the first write (the echo frame reshapes to identical geometry — no tick)"
    );

    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<LayoutTickCount>().0,
        after_settle,
        "steady frames never tick ComputedTextLayout"
    );

    app.world_mut().get_mut::<Text>(text).unwrap().0 =
        String::from("genuinely new content that changes geometry");
    app.update();
    assert_eq!(
        app.world().resource::<LayoutTickCount>().0,
        after_settle + 1,
        "a real change ticks exactly once"
    );
}

/// Decision 15 — empty text has no baseline.
#[test]
fn empty_text_gets_no_resolved_baseline() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::new())))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);
    assert!(app.world().get::<ResolvedBaseline>(text).is_none());
}

/// measure § 7 + § 8 item 4 — THE steady-state contract: a no-change
/// frame performs zero measure invocations (Taffy's cache holds; the
/// edge-triggered context registration holds) and zero buffer relayouts
/// (the commit guard holds).
#[test]
fn steady_state_zero_measure_calls_and_zero_reshapes() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("steady as she goes")),
            TextAlign::Center,
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);
    app.update(); // flush any cascade remnants

    app.update(); // THE steady frame
    assert_eq!(
        app.world().resource::<TextMeasureCallCount>().0,
        0,
        "no-change frame: zero measure invocations"
    );
    assert_eq!(
        app.world().resource::<TextCommitReshapeCount>().0,
        0,
        "no-change frame: zero buffer relayouts"
    );
}

/// Decision 7's regression edge — the probe-left buffer: an ancestor
/// resize re-probes the leaf (new available space ⇒ Taffy cache miss ⇒
/// measure runs ⇒ buffer left at a probe width with height_opt = None)
/// while the leaf's RESOLVED size is unchanged (max-content text narrower
/// than both widths). Changed<ResolvedLayout> never fires for the leaf —
/// the § 5.1 trigger row would skip it — but the commit guard catches the
/// None height and reconciles. With align set, a probe-width buffer would
/// paint glyphs offset against the wrong width in T4 — this is the test
/// that keeps that bug impossible.
///
/// Fixture note (verified against vendored taffy 0.10.1 cache.rs): the
/// parent must grow in HEIGHT, not just width. Width-only growth never
/// re-probes a non-wrapping leaf — MaxContent measure entries are
/// width-independent and the final-layout entry skips the available-space
/// comparison once known dimensions are set — so a width-only resize
/// re-invokes measure zero times and this test would pass vacuously.
/// Available HEIGHT does participate in entry matching
/// (`known.height.is_some() || is_roughly_equal(available.height)`), so
/// the height grow forces the re-probe; the measure-ran precondition
/// below keeps the fixture honest.
#[test]
fn ancestor_resize_with_unchanged_leaf_size_still_reconciles_the_buffer() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("tiny")),
            TextAlign::Center,
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(buiy_core::layout::AlignItems::FlexStart)
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text)
        .id();
    settle(&mut app);
    let committed = app.world().get::<TextBuffer>(text).unwrap().buffer.size();
    let leaf_size = app.world().get::<ResolvedLayout>(text).unwrap().size;

    // Grow the parent (the height grow is the load-bearing part — see the
    // fixture note): leaf max-content fits both widths ⇒ leaf size
    // unchanged ⇒ no Changed<ResolvedLayout> on the leaf.
    app.world_mut().entity_mut(parent).insert(
        Style::default()
            .flex_row()
            .align_items(buiy_core::layout::AlignItems::FlexStart)
            .width_px(500.0)
            .height_px(150.0),
    );
    app.update();

    assert!(
        app.world().resource::<TextMeasureCallCount>().0 > 0,
        "fixture precondition: the resize frame re-probed the leaf \
         (measure ran, leaving the buffer at a probe width)"
    );
    assert_eq!(
        app.world().get::<ResolvedLayout>(text).unwrap().size,
        leaf_size,
        "fixture precondition: the leaf's resolved size did not change"
    );
    assert_eq!(
        app.world().get::<TextBuffer>(text).unwrap().buffer.size(),
        committed,
        "the commit catch-all reconciled the probe-left buffer back to \
         its content box (height Some, width = content)"
    );
}

/// architecture § 2.2 end-to-end: a FontsGeneration bump sweeps every
/// buffer through sync → measure → commit in one frame.
#[test]
fn fonts_generation_bump_remeasures_and_recommits() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("sweep me"))))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);
    app.update();

    app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
    app.update();
    assert!(
        app.world().resource::<TextMeasureCallCount>().0 > 0,
        "the sweep dirty-marked the node — re-measured against the (new) font set"
    );
    let _ = text; // geometry assertions stay relational; same font ⇒ same layout

    app.update();
    assert_eq!(app.world().resource::<TextMeasureCallCount>().0, 0);
    assert_eq!(app.world().resource::<TextCommitReshapeCount>().0, 0);
}
