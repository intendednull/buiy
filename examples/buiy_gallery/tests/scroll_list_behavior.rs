//! Headless behavior gate for the S2 entity-tree Virtual List (the design's
//! 1000-node table). Drives the **same** `spawn_scroll_screen` tree the binary
//! authors + the live `ScrollListPlugin` (search-filter + row selection), then
//! asserts the two interactions reach an app-level effect:
//!
//! - **Search filter** — typing into the heading search field (`Filter nodes…`)
//!   hides the rows whose `type`/`name` does not contain the query (`Display::None`)
//!   and keeps the matches, and the heading total label reflects the visible count.
//! - **Row selection** — selecting a row marks exactly one `SelectedRow` (single-
//!   select; the prior selection is cleared) and the footer reports `selected #NNNN`.
//!
//! Plus a content-paint guard (the campaign's invisible-content lesson): the header
//! cells + the seeded row cells must be laid out at a non-zero box so they paint.
//!
//! The screen is built imperatively (`spawn_scroll_screen`, the icon/composite-heavy
//! idiom) and seeded by `fill_scroll_list` — the "example IS the fixture" discipline.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::{ButtonInput, KeyCode, With};
use bevy::scene::ScenePlugin;
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_core::ResolvedLayout;
use buiy_core::a11y::A11yPlugin;
use buiy_core::a11y::set_value;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::focus::FocusPlugin;
use buiy_core::layout::Display;
use buiy_core::scroll::ScrollInputPlugin;
use buiy_core::text::Text;
use buiy_gallery::{
    ScrollCountField, ScrollIntents, ScrollListPlugin, ScrollNode, ScrollSearch, SelectedRow,
    fill_scroll_list, spawn_scroll_screen,
};

/// A small row count keeps the behavior pass fast — the structure + the filter /
/// selection logic are scale-invariant (the 1000-row scale-game is the C8b driver
/// acceptance's job). Picked so the generated set has a mix of node types/names.
const ROWS: usize = 40;

/// Build the live S2 entity-tree tree + the `ScrollListPlugin` app logic. The app
/// has the a11y tree (so `set_value` drives the search field through the same
/// channel the real driver/keyboard uses) + layout (so the rows lay out + the
/// filter's `Display::None` collapses them).
fn scroll_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(A11yPlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(FocusPlugin)
        .add_plugins(ScrollInputPlugin)
        .add_plugins(WidgetsPlugin)
        .add_plugins(ScrollListPlugin);
    // The scroll-input + focus keyboard systems read these (mirrors the C8b
    // `scroll_a11y_app`); absent, an exclusive keyboard system fails validation.
    app.add_message::<KeyboardInput>();
    app.init_resource::<ButtonInput<KeyCode>>();

    spawn_scroll_screen(app.world_mut());
    fill_scroll_list(app.world_mut(), ROWS);
    // Settle: layout → extent → the screen is well-formed.
    for _ in 0..4 {
        app.update();
    }
    app
}

/// The `(visible, hidden)` row counts under the active filter (a hidden row is
/// `Display::None`).
fn row_visibility(app: &mut App) -> (usize, usize) {
    let mut q = app.world_mut().query::<(&ScrollNode, &Display)>();
    let world = app.world();
    let mut visible = 0usize;
    let mut hidden = 0usize;
    for (_, display) in q.iter(world) {
        if *display == Display::None {
            hidden += 1;
        } else {
            visible += 1;
        }
    }
    (visible, hidden)
}

/// The current text of a `ScrollCountField`-tagged label (the heading total or a
/// footer label).
fn count_label(app: &mut App, want: ScrollCountField) -> String {
    let want = std::mem::discriminant(&want);
    let mut q = app.world_mut().query::<(&ScrollCountField, &Text)>();
    let world = app.world();
    q.iter(world)
        .find(|(f, _)| std::mem::discriminant(*f) == want)
        .map(|(_, t)| t.0.clone())
        .unwrap_or_default()
}

#[test]
fn search_filter_hides_non_matching_rows_and_updates_the_total() {
    let mut app = scroll_app();

    // At rest: every row is visible + the total reads the full node count.
    let (visible0, hidden0) = row_visibility(&mut app);
    assert_eq!(visible0, ROWS, "all rows visible at rest");
    assert_eq!(hidden0, 0, "no row collapsed at rest");
    assert!(
        count_label(&mut app, ScrollCountField::Total).contains("nodes · windowed"),
        "the total label reads the windowed node count at rest, got {:?}",
        count_label(&mut app, ScrollCountField::Total)
    );

    // Type a query that matches only the `Scroll`-type nodes (type `Scroll` is in
    // the generated cycle). Drive the search field through the a11y `set_value`
    // channel (the same path the in-process driver + a real keyboard reach), then
    // settle so `TextChanged` → collect → apply runs.
    let field = {
        let mut q = app
            .world_mut()
            .query_filtered::<Entity, With<ScrollSearch>>();
        q.iter(app.world()).next().expect("the search field exists")
    };
    set_value(app.world_mut(), node_id_for(field), "scroll").expect("driver set_value honored");
    for _ in 0..4 {
        app.update();
    }

    let (visible1, hidden1) = row_visibility(&mut app);
    assert!(
        visible1 > 0 && visible1 < ROWS,
        "the 'scroll' query hides the non-matching rows (kept {visible1} of {ROWS})"
    );
    assert_eq!(
        visible1 + hidden1,
        ROWS,
        "every row is either shown or collapsed"
    );
    // Every still-visible row actually matches the query (type/name contains it).
    {
        let mut q = app.world_mut().query::<(&ScrollNode, &Display)>();
        let world = app.world();
        for (node, display) in q.iter(world) {
            if *display != Display::None {
                assert!(
                    node.haystack.contains("scroll"),
                    "a visible row {:?} does not match the 'scroll' query",
                    node.haystack
                );
            }
        }
    }
    // The total label now reads "N of M nodes" (the filtered form).
    assert!(
        count_label(&mut app, ScrollCountField::Total).contains(" of "),
        "the total label switches to the filtered 'N of M nodes' form, got {:?}",
        count_label(&mut app, ScrollCountField::Total)
    );

    // Clearing the query restores every row.
    set_value(app.world_mut(), node_id_for(field), "").expect("clear honored");
    for _ in 0..4 {
        app.update();
    }
    let (visible2, _) = row_visibility(&mut app);
    assert_eq!(visible2, ROWS, "clearing the query restores every row");
}

#[test]
fn clicking_a_row_selects_it_single_select_and_updates_the_footer() {
    let mut app = scroll_app();

    // No selection at rest.
    assert_eq!(
        count_label(&mut app, ScrollCountField::FooterSelection),
        "no selection",
        "the footer reads 'no selection' at rest"
    );

    // The first two rows (by node index), to prove single-select replaces.
    let rows: Vec<(Entity, usize)> = {
        let mut q = app.world_mut().query::<(Entity, &ScrollNode)>();
        let world = app.world();
        let mut v: Vec<(Entity, usize)> = q.iter(world).map(|(e, n)| (e, n.index)).collect();
        v.sort_by_key(|&(_, i)| i);
        v
    };
    let (row_a, idx_a) = rows[0];
    let (row_b, idx_b) = rows[1];

    // Select row A (stage the intent the `Pointer<Click>` observer stages, then run
    // the applier). The row is marked + the footer reports its index.
    app.world_mut().resource_mut::<ScrollIntents>().select = Some(row_a);
    for _ in 0..2 {
        app.update();
    }
    assert!(
        app.world().get::<SelectedRow>(row_a).is_some(),
        "row A is the SelectedRow after selection"
    );
    assert_eq!(
        count_label(&mut app, ScrollCountField::FooterSelection),
        format!("selected #{idx_a:04}"),
        "the footer reports the selected node index"
    );

    // Select row B — single-select: A is cleared, B is now the only selection.
    app.world_mut().resource_mut::<ScrollIntents>().select = Some(row_b);
    for _ in 0..2 {
        app.update();
    }
    assert!(
        app.world().get::<SelectedRow>(row_a).is_none(),
        "row A is deselected when row B is selected (single-select)"
    );
    assert!(
        app.world().get::<SelectedRow>(row_b).is_some(),
        "row B is the SelectedRow after the second selection"
    );
    let selected_count = {
        let mut q = app.world_mut().query::<&SelectedRow>();
        q.iter(app.world()).count()
    };
    assert_eq!(
        selected_count, 1,
        "exactly one row is selected (single-select)"
    );
    assert_eq!(
        count_label(&mut app, ScrollCountField::FooterSelection),
        format!("selected #{idx_b:04}"),
        "the footer reports row B's node index"
    );
}

/// The campaign's invisible-content guard (gallery-grounded): the seeded row cells
/// must be laid out at a non-zero box so they reach the screen — the require(Node)
/// regression class. Walks the live rows' content text (idx / type / name / ms /
/// state cells) and asserts each has a non-zero `ResolvedLayout`.
#[test]
fn scroll_row_content_is_laid_out_so_it_paints() {
    let mut app = scroll_app();
    for _ in 0..4 {
        app.update();
    }

    let mut q = app
        .world_mut()
        .query::<(Entity, &Text, Option<&ResolvedLayout>)>();
    let world = app.world();
    let mut checked = 0usize;
    for (e, text, layout) in q.iter(world) {
        if text.0.trim().is_empty() {
            continue;
        }
        let layout = layout.unwrap_or_else(|| {
            panic!(
                "scroll content label {e:?} {:?} has NO ResolvedLayout — it paints nothing \
                 (the require(Node) regression)",
                text.0
            )
        });
        assert!(
            layout.size.x > 0.0 && layout.size.y > 0.0,
            "scroll content label {:?} has a zero-size box {:?} — it paints nothing",
            text.0,
            layout.size,
        );
        checked += 1;
    }
    // The heading (H1 + total) + the 4 header cells + ROWS × (idx + type + name + ms
    // + state) cells = many visible labels; a vacuous 0 would hide a regression.
    assert!(
        checked >= ROWS,
        "expected the live entity-tree to carry many painted content labels, only saw {checked}"
    );
}
