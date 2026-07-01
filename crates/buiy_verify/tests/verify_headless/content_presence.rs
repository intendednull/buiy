//! The content-presence predicate's RED proof (C7 §2.4, §6). A text-bearing
//! fixture MUST emit > 0 glyph instances on the production extract path; a
//! fixture whose text silently fails to shape is the silent-no-paint failure
//! (Bug 2 release mode) and MUST be caught here. The predicate runs the
//! production `extract_buiy_glyphs` adapterless, so the test app carries the
//! full text+render MAIN-world stack (mirror of `TextExtractHarness`).

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::text::{FontSize, Text};
use buiy_verify::invariant::content_is_present;

/// A real text-bearing scene: a sized column root + a "Hi!" label. The
/// production producer shapes "Hi!" to 3 glyph instances (text_extract.rs:86).
fn spawn_label(app: &mut App) {
    let root = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .id();
    let label = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi!")),
            FontSize(16.0),
        ))
        .id();
    app.world_mut().entity_mut(root).add_child(label);
}

#[test]
fn content_present_passes_for_a_shaping_label() {
    let mut app = content_test_app();
    spawn_label(&mut app);
    app.update(); // TextSync -> measure -> commit, so the buffer is shaped
    assert!(
        content_is_present(&mut app).is_ok(),
        "a label that shapes to 3 glyphs must satisfy content_is_present"
    );
}

#[test]
fn content_present_fails_for_a_zero_glyph_text_fixture() {
    // A text-bearing entity whose content is whitespace-only: the producer
    // emits ZERO glyph instances (text_extract.rs:967, verified). This is the
    // structural stand-in for the silent-no-paint bug — the predicate catches it.
    let mut app = content_test_app();
    let root = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .id();
    let label = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("   ")), // whitespace: text-bearing, zero visible glyphs
            FontSize(16.0),
        ))
        .id();
    app.world_mut().entity_mut(root).add_child(label);
    app.update();

    let result = content_is_present(&mut app);
    assert!(
        result.is_err(),
        "a text-bearing fixture that emits 0 glyphs must violate content_is_present"
    );
    assert_eq!(result.unwrap_err().rule, "content_is_present");
}

/// The full text+render MAIN-world stack the predicate runs the producer
/// against — the exact plugin set `TextExtractHarness` builds
/// (extract_harness.rs:71-85): ThemePlugin + CorePlugin + LayoutPlugin +
/// BuiyTextPlugin + BuiyRenderPlugin (its render half is a no-op without a
/// RenderApp), plus a component-only synthetic PrimaryWindow. No wgpu adapter.
fn content_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::theme::ThemePlugin)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(buiy_core::layout::LayoutPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default())
        .add_plugins(buiy_core::render::BuiyRenderPlugin);
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(640, 480),
            ..Default::default()
        },
        PrimaryWindow,
    ));
    app
}

/// Catalog-wide content-presence auto-check (V13): EVERY text-bearing cell of
/// EVERY fixture must emit > 0 glyph instances on the production extract path.
/// This is the enrollment-tier analogue of the two RED-proof unit tests above —
/// it catches a fixture whose text silently stops shaping (the silent-no-paint
/// bug) across the whole matrix, by construction.
///
/// Both former blockers are gone: `build_app` is now text-capable (it installs
/// `BuiyTextPlugin { system_fonts: false }` + stages Ahem, so `SharedFontSystem`
/// is present and `glyph_census` no longer panics), and the button fixture is a
/// text-bearing cell (its "Save" label measures + shapes under Ahem, V14). The
/// `text_bearing > 0` guard makes the check fail loudly if it ever goes vacuous
/// (a catalog with no text-bearing cell would silently pass without it).
#[test]
fn every_text_bearing_catalog_cell_emits_glyphs() {
    use buiy_verify::coverage::{Matrix, enroll_all};
    use buiy_verify::invariant::glyph_census;

    // `enroll_all` takes `impl Fn`, so the counters use interior mutability.
    let checked = std::cell::Cell::new(0usize);
    let text_bearing = std::cell::Cell::new(0usize);

    enroll_all(&Matrix::cpu_snapshots(), |mut app, key| {
        app.update(); // TextSync -> measure -> commit, so the buffers are shaped
        let (bearing, glyphs) = glyph_census(&mut app);
        checked.set(checked.get() + 1);
        if bearing {
            text_bearing.set(text_bearing.get() + 1);
            assert!(
                glyphs > 0,
                "text-bearing cell {} emitted 0 glyph instances (silent-no-paint)",
                key.stem()
            );
        }
    });

    assert!(checked.get() > 0, "the catalog enrolled no cells");
    assert!(
        text_bearing.get() > 0,
        "no text-bearing cell was exercised — the catalog-wide content-presence \
         auto-check is VACUOUS; a text-bearing fixture must enroll for it to have teeth"
    );
}
