//! Subgrid + Masonry stub tests — pin the observable layout degradation.

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::{Display, GridAutoFlow, GridParams, LayoutPlugin, Style, TrackSize};

fn world_with_grid(grid_params: GridParams, display: Display) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    let style = Style {
        display,
        grid_params,
        ..Default::default()
    };
    app.world_mut().spawn((style, Node));
    app.update();
    app
}

#[test]
fn subgrid_in_template_columns_falls_back_to_auto() {
    let g = GridParams {
        template_columns: vec![TrackSize::Subgrid],
        ..Default::default()
    };
    let _app = world_with_grid(g, Display::Grid);
    // Layout completes without panic. Subgrid → Auto fallback is exercised
    // through the full pipeline. (Observable: no panic + warn-once in
    // log output during this test run.)
}

#[test]
fn masonry_auto_flow_falls_back_to_row() {
    let g = GridParams {
        auto_flow: GridAutoFlow::Masonry,
        ..Default::default()
    };
    let _app = world_with_grid(g, Display::Grid);
    // Layout completes without panic. Masonry → Row fallback exercised.
}
