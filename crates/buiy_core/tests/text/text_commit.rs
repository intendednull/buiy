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
    // The shape_stale guard (C2 § 2.2) WALKS layout_runs().count() here but must
    // not TRIGGER a reshape in steady state: layout_runs().count() ==
    // computed.lines.len() holds, so the short-circuit still fires (§ 3.4 cost).
    assert_eq!(
        app.world().resource::<TextCommitReshapeCount>().0,
        0,
        "no-change frame: zero buffer relayouts"
    );
}

/// Bug-2 ISOLATION (C2 § 2.2; audit § 2 Bug 2, Appendix-A.5). The `shape_stale`
/// guard's NON-VACUOUS proof: a buffer that was committed (has a
/// `ComputedTextLayout`) but is then UNSHAPED — with its content-box size,
/// per-line align, and content offset all UNCHANGED — must be reshaped by
/// `text_commit`, because extract asserts `layout_runs().count() ==
/// computed.lines.len()` (extract.rs:712).
///
/// Why this isolates `shape_stale` where the end-to-end font-reload path
/// CANNOT: the real `FontsGeneration` bump auto-heals via the
/// `text_sync_buffers` sweep, which calls `tree.mark_dirty_for_entity`
/// (sync.rs) → Taffy re-measures → the buffer reshapes regardless of the
/// guard. This test removes that auto-heal entirely: it unshapes the buffer by
/// a DIRECT `reset_shaping()` on the buffer line (NO FontsGeneration bump, NO
/// Text/style-carrier edit), so the TextSync sweep never runs (its triggers are
/// `fonts_generation.is_changed()` or the `Or<(Changed<Text>, …)>` set;
/// `Changed<TextBuffer>` is NOT a trigger) and Taffy never re-measures. The ONLY
/// system that can reshape the buffer on the next frame is `text_commit`, and the
/// ONLY guard term that can fire is `shape_stale` (size/align/offset are equal) —
/// so a PASS proves the `shape_stale` term did the reshape. WITHOUT the term this
/// is RED (the buffer stays unshaped, `layout_runs().count() == 0`).
#[test]
fn shape_stale_reshapes_a_committed_but_unshaped_buffer() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("hello"))))
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
    app.update(); // flush any cascade remnant — reach a true steady state
    // (the steady_state test's discipline): a plain frame here
    // reshapes 0 buffers, so the post-unshape reshape below is
    // attributable solely to shape_stale.

    // Precondition: committed and shaped. One line ("hello"), one layout run.
    let committed_lines = {
        let tb = app.world().get::<TextBuffer>(text).unwrap();
        let computed = app.world().get::<ComputedTextLayout>(text).unwrap();
        assert_eq!(
            tb.buffer.layout_runs().count(),
            computed.lines.len(),
            "precondition: settled buffer is shaped (runs == committed lines)"
        );
        assert_eq!(computed.lines.len(), 1, "single-line 'hello'");
        let size = tb.buffer.size();
        assert!(size.0.is_some() && size.1.is_some(), "both axes committed");
        computed.lines.len()
    };

    // Construct the committed-but-UNSHAPED state directly: reset the line's
    // shape+layout cache. This is the EXACT mismatch extract asserts —
    // layout_runs() now terminates at the first unshaped line — while
    // buffer.size()/align/content_offset are all unchanged (so commit's
    // size/align/offset terms stay false). Mutating via a direct get_mut does
    // NOT bump FontsGeneration and does NOT touch Text, so the TextSync sweep
    // (the auto-heal) never runs.
    {
        let mut tb = app.world_mut().get_mut::<TextBuffer>(text).unwrap();
        tb.buffer.lines[0].reset_shaping();
        assert_eq!(
            tb.buffer.layout_runs().count(),
            0,
            "constructed RED state: buffer unshaped (runs=0) while committed lines=1"
        );
    }

    // One frame: TextSync does NOT sweep (no bump, no Text change), Taffy does
    // NOT re-measure (resolved size unchanged), so text_commit is the only
    // system that can reshape — and only via shape_stale.
    app.update();

    let tb = app.world().get::<TextBuffer>(text).unwrap();
    assert_eq!(
        tb.buffer.layout_runs().count(),
        committed_lines,
        "shape_stale must reshape the unshaped-but-committed buffer back to \
         layout_runs().count() == computed.lines.len() (WITHOUT the term this \
         is 0 — the silent-no-paint / debug_assert state at extract)"
    );
    assert_eq!(
        app.world().resource::<TextCommitReshapeCount>().0,
        1,
        "exactly one buffer reshaped this frame — the shape_stale-triggered reshape"
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

/// Decision 2 (T4): glyph/run coordinates are content-box relative while
/// GlobalTransform lands on the border box — TextCommit writes the
/// border+padding offset so the producer can fold the § 5.1 content origin
/// without Taffy access.
#[test]
fn commit_writes_the_content_box_offset() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default().padding(6.0).border(2.0),
            Text(String::from("offset")),
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

    let layout = app.world().get::<ComputedTextLayout>(text).unwrap();
    assert_eq!(
        layout.content_offset,
        Vec2::new(8.0, 8.0),
        "border 2 + padding 6"
    );
}

/// The steady-state short-circuit must not strand a stale offset: grow the
/// padding while growing the box so the CONTENT size is unchanged — the
/// buffer target compares equal, but the offset moved and must re-commit.
/// `border_box()` is load-bearing: the fixture's "constant content box"
/// arithmetic sizes the BORDER box (the default `BoxSizing` is ContentBox).
#[test]
fn padding_change_with_constant_content_box_updates_the_offset() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .border_box()
                .width_px(100.0)
                .height_px(40.0)
                .padding(5.0),
            Text(String::from("x")),
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
    assert_eq!(
        app.world()
            .get::<ComputedTextLayout>(text)
            .unwrap()
            .content_offset,
        Vec2::splat(5.0)
    );

    // 90x30 content box both times: (100-2*5) → (110-2*10).
    app.world_mut().entity_mut(text).insert(
        Style::default()
            .border_box()
            .width_px(110.0)
            .height_px(50.0)
            .padding(10.0),
    );
    settle(&mut app);
    assert_eq!(
        app.world()
            .get::<ComputedTextLayout>(text)
            .unwrap()
            .content_offset,
        Vec2::splat(10.0),
        "offset re-committed even though the content-box size held"
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
