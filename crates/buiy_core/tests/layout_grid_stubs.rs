//! Subgrid + Masonry fallback tests — pin the observable layout degradation
//! AND the testable warn (audit #24, campaign T2.3).
//!
//! Before T2.3 these bodies were comment-only and vacuous: the subgrid/masonry
//! fallbacks warned via a process-global `AtomicBool` in `translate.rs` that no
//! test could observe (UNLIKE table/multicol, which route through the testable
//! `LayoutWarnedOnceSession` resource). The production warn was refactored to
//! mirror that precedent exactly — the subgrid/masonry fallback warns now
//! record `LayoutWarnOnceKey::GridSubgridUnsupported` /
//! `GridMasonryUnsupported` in `sync_styles` (where the session-scoped warn
//! resource lives), while the FALLBACK BEHAVIOR (Subgrid → Auto, Masonry →
//! Row) is unchanged and still lives in the pure `style_to_taffy`.
//!
//! Each test asserts BOTH halves: (1) the warn key is recorded exactly once
//! per (entity, session), and (2) the observable Auto/Row fallback geometry.

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::components::ResolvedLayout;
use buiy_core::layout::{
    Display, GridAutoFlow, GridParams, LayoutPlugin, LayoutWarnOnceKey, LayoutWarnedOnceSession,
    Length, Style, TrackSize,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
}

/// Count session-recorded warns matching `pred` (mirrors
/// `layout_table.rs::count_warns` / the multicol idiom).
fn count_warns(app: &App, mut pred: impl FnMut(&LayoutWarnOnceKey) -> bool) -> usize {
    app.world()
        .resource::<LayoutWarnedOnceSession>()
        .set
        .iter()
        .filter(|k| pred(k))
        .count()
}

#[test]
fn subgrid_in_template_columns_warns_once_and_falls_back_to_auto() {
    let mut app = app();
    // 200x100 grid whose single column template is `subgrid` → falls back to
    // Auto. The lone child has no explicit width, so an Auto column sizes it to
    // its (empty) content: width 0 at x=0. (A real `Px`/`Fr` track would give
    // it a non-zero width — the Auto fallback is what collapses it.)
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style {
                display: Display::Grid,
                grid_params: GridParams {
                    template_columns: vec![TrackSize::Subgrid],
                    ..Default::default()
                },
                ..Default::default()
            }
            .width_px(200.0)
            .height_px(100.0),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().height_px(40.0)))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    app.update();
    app.update(); // second frame must NOT add another warn (session dedup)

    // (1) The subgrid fallback warn is recorded exactly once for the container.
    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::GridSubgridUnsupported(e) if *e == parent
        )),
        1,
        "subgrid fallback warns exactly once per (entity, session)",
    );

    // (2) Observable Auto fallback geometry: the child sits at x=0 with width 0
    // (an Auto track collapsed to the empty child's content size). Layout
    // completed without panic.
    let rl = app
        .world()
        .get::<ResolvedLayout>(child)
        .expect("ResolvedLayout written (no panic)");
    assert!(
        rl.position.x.abs() < 0.5,
        "Auto-fallback child x = {}",
        rl.position.x
    );
    assert!(
        rl.size.x.abs() < 0.5,
        "Auto-fallback track collapses empty child to width 0; got {}",
        rl.size.x
    );
}

#[test]
fn subgrid_nested_in_repeat_also_warns() {
    // `repeat(2, [subgrid])` — the inner subgrid track reaches
    // `track_to_sizing`'s Subgrid arm too, so the recursive detector in
    // `sync_styles` must catch it (not just a bare top-level Subgrid).
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style {
                display: Display::Grid,
                grid_params: GridParams {
                    template_columns: vec![TrackSize::Repeat(
                        buiy_core::layout::RepeatCount::Count(2),
                        vec![TrackSize::Subgrid],
                    )],
                    ..Default::default()
                },
                ..Default::default()
            }
            .width_px(200.0)
            .height_px(100.0),
        ))
        .id();
    app.update();
    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::GridSubgridUnsupported(e) if *e == parent
        )),
        1,
        "subgrid nested inside repeat() is detected and warns once",
    );
}

#[test]
fn masonry_auto_flow_warns_once_and_falls_back_to_row() {
    let mut app = app();
    // A 2-column 200x200 grid with `auto-flow: masonry` → falls back to Row.
    // Two children placed into the two 100px columns: child 0 at x=0, child 1
    // at x=100, both on the first row (y=0).
    //
    // NOTE on what this geometry pins: side-by-side first-row placement is what
    // a NORMAL grid auto-flow produces — it does NOT distinguish Row from
    // Column for this 2x2 fixture (Column auto-flow would place these same two
    // items identically). So this assertion pins masonry-VS-not (a real,
    // non-panicking grid layout happened instead of the unimplemented masonry
    // packing) together with the warn below. The Row CHOICE specifically is
    // pinned at the translation tier by
    // `translate::tests::map_grid_auto_flow_masonry_falls_back_to_row`, which
    // reddens if the fallback is changed to Column.
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style {
                display: Display::Grid,
                grid_params: GridParams {
                    template_columns: vec![
                        TrackSize::Length(Length::Px(100.0)),
                        TrackSize::Length(Length::Px(100.0)),
                    ],
                    auto_flow: GridAutoFlow::Masonry,
                    ..Default::default()
                },
                ..Default::default()
            }
            .width_px(200.0)
            .height_px(200.0),
        ))
        .id();
    let c0 = app
        .world_mut()
        .spawn((Node, Style::default().height_px(40.0)))
        .id();
    let c1 = app
        .world_mut()
        .spawn((Node, Style::default().height_px(40.0)))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[c0, c1]);

    app.update();
    app.update(); // session dedup: no second warn

    // (1) Masonry fallback warn recorded exactly once for the container.
    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::GridMasonryUnsupported(e) if *e == parent
        )),
        1,
        "masonry fallback warns exactly once per (entity, session)",
    );

    // (2) Observable normal-auto-flow fallback geometry (masonry-vs-not, not
    // Row-vs-Column): placement into the two 100px columns — c0 at x=0, c1 at
    // x=100, both first-row (y=0). A real grid layout ran, no panic.
    let r0 = app
        .world()
        .get::<ResolvedLayout>(c0)
        .expect("c0 layout (no panic)");
    let r1 = app
        .world()
        .get::<ResolvedLayout>(c1)
        .expect("c1 layout (no panic)");
    assert!(
        r0.position.x.abs() < 0.5,
        "auto-flow-fallback c0 x = {}",
        r0.position.x
    );
    assert!(
        (r1.position.x - 100.0).abs() < 0.5,
        "auto-flow-fallback c1 x = {} (expected 100; placed into col 2)",
        r1.position.x
    );
    assert!(r0.position.y.abs() < 0.5, "c0 y = {}", r0.position.y);
    assert!(r1.position.y.abs() < 0.5, "c1 y = {}", r1.position.y);
}

#[test]
fn plain_block_with_inert_grid_params_does_not_warn() {
    // A non-grid box carries default `GridParams` (auto_flow Row, empty
    // templates) — it must NOT trip either warn. Guards the `Display::Grid |
    // InlineGrid` gate on the warn site.
    let mut app = app();
    let _e = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
        .id();
    app.update();
    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::GridSubgridUnsupported(_)
                | LayoutWarnOnceKey::GridMasonryUnsupported(_)
        )),
        0,
        "a plain block box with inert grid defaults warns for neither",
    );
}
