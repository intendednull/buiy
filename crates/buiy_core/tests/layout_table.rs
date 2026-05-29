//! Phase 12 — full table layout algorithm (sub-pass 6b). Spawns
//! Display::Table* hierarchies and asserts the corrected
//! ResolvedLayout positions (cells in a column grid, rows + groups
//! stacked vertically).
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.

use bevy::prelude::*;
use buiy_core::layout::{Display, LayoutPlugin, Style};
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
