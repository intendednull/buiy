//! Buiy text T4 hello-world: a themed paragraph through the full pipeline —
//! TextSync → Taffy measure → TextCommit → extract_buiy_glyphs →
//! BuiyAtlas (CoverageR8) → the alpha-as-color glyph draw.
//!
//! The automated twin of this scene is `tests/text_gpu.rs`'s gate-#2
//! fixture; this binary is the human-eyes smoke test
//! (`cargo run -p hello_text`).

use bevy::prelude::*;
use buiy::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // TextColor::default() == CurrentColor — the theme default foreground.
    let title = commands
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hello, Buiy text!")),
            FontSize(32.0),
        ))
        .id();
    let body = commands
        .spawn((
            Node,
            Style::default(),
            Text(String::from(
                "The quick brown fox jumps over the lazy dog. Shaped by \
                 cosmic-text at the committed wrap width, rasterized once per \
                 (font, size, weight, subpixel-bin) into the shared coverage \
                 atlas, tinted per instance — a theme switch never touches \
                 the atlas.",
            )),
            FontSize(16.0),
        ))
        .id();
    commands
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(560.0)
                .padding(24.0)
                .gap_px(12.0),
        ))
        .add_children(&[title, body]);
}
