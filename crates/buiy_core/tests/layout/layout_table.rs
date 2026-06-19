//! Phase 12 — full table layout algorithm (sub-pass 6b). Spawns
//! Display::Table* hierarchies and asserts the corrected
//! ResolvedLayout positions (cells in a column grid, rows + groups
//! stacked vertically).
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.

use bevy::prelude::*;
use buiy_core::layout::{Display, LayoutPlugin, LayoutWarnOnceKey, LayoutWarnedOnceSession, Style};
use buiy_core::{Node, ResolvedLayout};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
}

fn pos(app: &App, e: Entity) -> Vec2 {
    app.world()
        .get::<ResolvedLayout>(e)
        .expect("ResolvedLayout present")
        .position
}

#[test]
fn single_row_two_cells_sit_in_a_column_grid() {
    // Table > Row > [Cell(w=40), Cell(w=60)]. (Bare row → implicit
    // group, D6.) Cell 0 at x=0; cell 1 at x=40 (after column 0).
    let mut app = app();
    let c0 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(40.0)
                .height_px(20.0),
        ))
        .id();
    let c1 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(60.0)
                .height_px(20.0),
        ))
        .id();
    let row = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_children(&[c0, c1])
        .id();
    let _table = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::Table)))
        .add_child(row)
        .id();

    app.update();

    assert_eq!(pos(&app, c0).x, 0.0, "cell 0 at column 0 origin");
    assert!(
        (pos(&app, c1).x - 40.0).abs() < 0.5,
        "cell 1 starts after column 0 (40px)"
    );
    assert_eq!(
        pos(&app, c0).y,
        pos(&app, c1).y,
        "both cells share the row's y"
    );
}

#[test]
fn cell_size_comes_from_taffy_not_overridden() {
    // 6b corrects position only; size stays from Taffy (D1).
    let mut app = app();
    let c0 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(40.0)
                .height_px(25.0),
        ))
        .id();
    let row = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_child(c0)
        .id();
    let _table = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::Table)))
        .add_child(row)
        .id();

    app.update();

    let rl = app.world().get::<ResolvedLayout>(c0).unwrap();
    assert!((rl.size.x - 40.0).abs() < 0.5, "cell width from Taffy");
    assert!((rl.size.y - 25.0).abs() < 0.5, "cell height from Taffy");
}

#[test]
fn columns_size_to_widest_cell_across_rows() {
    // Row 0: cells 30 / 50.  Row 1: cells 70 / 20.
    // Column 0 = max(30,70) = 70; column 1 starts at x=70 for BOTH rows.
    let mut app = app();
    let r0c0 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(30.0)
                .height_px(20.0),
        ))
        .id();
    let r0c1 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(50.0)
                .height_px(20.0),
        ))
        .id();
    let r1c0 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(70.0)
                .height_px(20.0),
        ))
        .id();
    let r1c1 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(20.0)
                .height_px(20.0),
        ))
        .id();
    let row0 = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_children(&[r0c0, r0c1])
        .id();
    let row1 = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_children(&[r1c0, r1c1])
        .id();
    let _table = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::Table)))
        .add_children(&[row0, row1])
        .id();

    app.update();

    // Column 1 starts after the widest column-0 cell (70px) in BOTH rows.
    assert!(
        (pos(&app, r0c1).x - 70.0).abs() < 0.5,
        "row 0 col 1 at x=70 (widest col 0)"
    );
    assert!(
        (pos(&app, r1c1).x - 70.0).abs() < 0.5,
        "row 1 col 1 also at x=70"
    );
    assert_eq!(pos(&app, r0c0).x, 0.0);
    assert_eq!(pos(&app, r1c0).x, 0.0);
}

#[test]
fn rows_stack_by_their_own_height() {
    // Row 0 cell height 25; row 1 cell height 40. Row 1 starts at y=25.
    let mut app = app();
    let r0 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(40.0)
                .height_px(25.0),
        ))
        .id();
    let r1 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(40.0)
                .height_px(40.0),
        ))
        .id();
    let row0 = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_child(r0)
        .id();
    let row1 = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_child(r1)
        .id();
    let _table = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::Table)))
        .add_children(&[row0, row1])
        .id();

    app.update();

    assert_eq!(pos(&app, r0).y, 0.0, "row 0 at top");
    assert!(
        (pos(&app, r1).y - 25.0).abs() < 0.5,
        "row 1 below row 0 (25px tall)"
    );
}

#[test]
fn explicit_row_groups_stack_in_document_order() {
    // Table > [HeaderGroup > Row > Cell(h=20)], [RowGroup > Row > Cell(h=30)].
    // Header group's row at y=0; body group's row at y=20 (D5 — source order,
    // no header-floats-to-top reorder).
    let mut app = app();
    let hc = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(40.0)
                .height_px(20.0),
        ))
        .id();
    let hrow = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_child(hc)
        .id();
    let header = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableHeaderGroup)))
        .add_child(hrow)
        .id();

    let bc = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(40.0)
                .height_px(30.0),
        ))
        .id();
    let brow = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_child(bc)
        .id();
    let body = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRowGroup)))
        .add_child(brow)
        .id();

    let _table = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::Table)))
        .add_children(&[header, body])
        .id();

    app.update();

    assert_eq!(pos(&app, hc).y, 0.0, "header group row at top");
    assert!(
        (pos(&app, bc).y - 20.0).abs() < 0.5,
        "body group row below header (20px)"
    );
    // Group entities sit at their first row's y.
    assert_eq!(pos(&app, header).y, 0.0);
    assert!((pos(&app, body).y - 20.0).abs() < 0.5);
}

#[test]
fn cell_columns_align_across_groups() {
    // Two groups, each one row of two cells. Column 0 = max widths across
    // BOTH groups' rows; column 1 aligns across groups.
    let mut app = app();
    // group A row: 30 / 50
    let a0 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(30.0)
                .height_px(20.0),
        ))
        .id();
    let a1 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(50.0)
                .height_px(20.0),
        ))
        .id();
    let arow = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_children(&[a0, a1])
        .id();
    let ga = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRowGroup)))
        .add_child(arow)
        .id();
    // group B row: 60 / 20  → column 0 = max(30,60) = 60
    let b0 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(60.0)
                .height_px(20.0),
        ))
        .id();
    let b1 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(20.0)
                .height_px(20.0),
        ))
        .id();
    let brow = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_children(&[b0, b1])
        .id();
    let gb = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRowGroup)))
        .add_child(brow)
        .id();

    let _table = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::Table)))
        .add_children(&[ga, gb])
        .id();

    app.update();

    assert!(
        (pos(&app, a1).x - 60.0).abs() < 0.5,
        "group A col 1 at x=60 (widest col 0 across groups)"
    );
    assert!(
        (pos(&app, b1).x - 60.0).abs() < 0.5,
        "group B col 1 also at x=60"
    );
}

fn count_warns(app: &App, mut pred: impl FnMut(&LayoutWarnOnceKey) -> bool) -> usize {
    app.world()
        .resource::<LayoutWarnedOnceSession>()
        .set
        .iter()
        .filter(|k| pred(k))
        .count()
}

#[test]
fn caption_warns_once_and_is_not_placed() {
    // A caption child is classified but deferred (D4): one warn, no
    // override (its position stays Taffy-block).
    let mut app = app();
    let cap = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCaption)
                .width_px(40.0)
                .height_px(10.0),
        ))
        .id();
    let cell = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(40.0)
                .height_px(20.0),
        ))
        .id();
    let row = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_child(cell)
        .id();
    let _table = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::Table)))
        .add_children(&[cap, row])
        .id();

    app.update();
    app.update(); // second frame must NOT add another warn

    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::TableSubfeatureUnsupported(e) if *e == cap
        )),
        1,
        "caption warns exactly once per (entity, session)",
    );
}

#[test]
fn ragged_rows_warn_span_unsupported_once_per_table() {
    // Row 0 has 2 cells, row 1 has 1 → ragged (span-faking). One
    // TableSpanUnsupported warn for the table entity.
    let mut app = app();
    let a = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(30.0)
                .height_px(20.0),
        ))
        .id();
    let b = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(30.0)
                .height_px(20.0),
        ))
        .id();
    let c = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(30.0)
                .height_px(20.0),
        ))
        .id();
    let row0 = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_children(&[a, b])
        .id();
    let row1 = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_child(c)
        .id();
    let table = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::Table)))
        .add_children(&[row0, row1])
        .id();

    app.update();
    app.update();

    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::TableSpanUnsupported(e) if *e == table
        )),
        1,
        "ragged table warns once per (table, session)",
    );
}

#[test]
fn well_formed_table_emits_no_warns() {
    let mut app = app();
    let a = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(30.0)
                .height_px(20.0),
        ))
        .id();
    let b = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(30.0)
                .height_px(20.0),
        ))
        .id();
    let row = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_children(&[a, b])
        .id();
    let _table = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::Table)))
        .add_child(row)
        .id();

    app.update();

    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::TableSpanUnsupported(_)
                | LayoutWarnOnceKey::TableSubfeatureUnsupported(_)
        )),
        0,
        "a uniform, caption-free table produces no deferral warns",
    );
}
