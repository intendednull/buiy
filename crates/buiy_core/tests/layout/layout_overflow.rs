//! Phase 2 integration: every `OverflowMode` variant produces the
//! expected `taffy::Style.overflow` value through the full Buiy
//! pipeline, and `Overflow::is_scroll_container` matches spec § 1.2.

use bevy::prelude::*;
use buiy_core::{
    BoxModel, CorePlugin, LayoutTree, Length, Node, Overflow, OverflowMode, Sizing, Style,
    layout::LayoutPlugin,
};

fn run_one_frame_with_overflow(overflow: Overflow) -> taffy::Style {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: BoxModel {
                    width: Sizing::Length(Length::Px(100.0)),
                    height: Sizing::Length(Length::Px(100.0)),
                    ..Default::default()
                },
                overflow,
                ..Default::default()
            },
        ))
        .id();
    app.update();
    let tree = app.world().non_send_resource::<LayoutTree>();
    let id = *tree.by_entity().get(&entity).expect("Taffy node assigned");
    tree.tree_ref()
        .style(id)
        .expect("style retrievable")
        .clone()
}

#[test]
fn overflow_visible_maps_to_taffy_visible() {
    let s = run_one_frame_with_overflow(Overflow::default());
    assert_eq!(s.overflow.x, taffy::Overflow::Visible);
    assert_eq!(s.overflow.y, taffy::Overflow::Visible);
}

#[test]
fn overflow_hidden_and_clip_both_map_to_taffy_hidden() {
    let hidden = run_one_frame_with_overflow(Overflow {
        x: OverflowMode::Hidden,
        y: OverflowMode::Hidden,
        ..Default::default()
    });
    assert_eq!(hidden.overflow.x, taffy::Overflow::Hidden);
    assert_eq!(hidden.overflow.y, taffy::Overflow::Hidden);

    let clip = run_one_frame_with_overflow(Overflow {
        x: OverflowMode::Clip,
        y: OverflowMode::Clip,
        ..Default::default()
    });
    assert_eq!(clip.overflow.x, taffy::Overflow::Hidden);
    assert_eq!(clip.overflow.y, taffy::Overflow::Hidden);
}

#[test]
fn overflow_scroll_and_auto_both_map_to_taffy_scroll() {
    let scroll = run_one_frame_with_overflow(Overflow {
        x: OverflowMode::Scroll,
        y: OverflowMode::Scroll,
        ..Default::default()
    });
    assert_eq!(scroll.overflow.x, taffy::Overflow::Scroll);
    assert_eq!(scroll.overflow.y, taffy::Overflow::Scroll);

    let auto = run_one_frame_with_overflow(Overflow {
        x: OverflowMode::Auto,
        y: OverflowMode::Auto,
        ..Default::default()
    });
    assert_eq!(auto.overflow.x, taffy::Overflow::Scroll);
    assert_eq!(auto.overflow.y, taffy::Overflow::Scroll);
}

#[test]
fn is_scroll_container_matches_spec() {
    assert!(!Overflow::default().is_scroll_container());
    assert!(
        !Overflow {
            x: OverflowMode::Hidden,
            y: OverflowMode::Hidden,
            ..Default::default()
        }
        .is_scroll_container()
    );
    assert!(
        Overflow {
            x: OverflowMode::Scroll,
            ..Default::default()
        }
        .is_scroll_container()
    );
    assert!(
        Overflow {
            y: OverflowMode::Auto,
            ..Default::default()
        }
        .is_scroll_container()
    );
}
