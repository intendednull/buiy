//! Per-node direction — the § 5.4 strong-mark prepend (measure-and-layout
//! § 5.4): LRM/RLM after collapse forces the UAX #9 P2 paragraph level, so
//! base direction drives reordering, the unaligned `start` default, and
//! the `LayoutRun.rtl` flag. Headless: the rtl flag needs no font coverage
//! (bidi levels come from unicode-bidi, not the font).

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::{
    BuiyTextPlugin, ComputedTextLayout, Text, TextDirection, prepend_strong_marks,
};
use std::borrow::Cow;

// --- the pure pre-pass --------------------------------------------------

#[test]
fn ltr_prepends_lrm_per_non_empty_line() {
    assert_eq!(
        prepend_strong_marks("ab\ncd", TextDirection::Ltr),
        "\u{200E}ab\n\u{200E}cd"
    );
}

#[test]
fn rtl_prepends_rlm_per_non_empty_line() {
    assert_eq!(
        prepend_strong_marks("ab\ncd", TextDirection::Rtl),
        "\u{200F}ab\n\u{200F}cd"
    );
}

#[test]
fn auto_is_borrowed_passthrough() {
    // Auto = cosmic's first-strong default IS CSS dir=auto (§ 5.4); the
    // steady path allocates nothing.
    assert!(matches!(
        prepend_strong_marks("hello", TextDirection::Auto),
        Cow::Borrowed(_)
    ));
}

#[test]
fn empty_lines_stay_unmarked() {
    // Decision 10: a mark on an empty line could shape into a phantom
    // glyph and flip T3's glyphs-keyed ResolvedBaseline for Text("").
    assert_eq!(prepend_strong_marks("", TextDirection::Rtl), "");
    assert_eq!(
        prepend_strong_marks("a\n\nb", TextDirection::Rtl),
        "\u{200F}a\n\n\u{200F}b"
    );
}

// --- end-to-end through TextSync → measure → TextCommit ------------------

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

fn settle(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

fn spawn_line(app: &mut App, text: &str, dir: Option<TextDirection>) -> Entity {
    let mut e = app.world_mut().spawn((
        buiy_core::Node,
        Style::default().width_px(300.0).height_px(60.0),
        Text(String::from(text)),
    ));
    if let Some(dir) = dir {
        e.insert(dir);
    }
    e.id()
}

#[test]
fn rtl_component_forces_rtl_on_latin_text() {
    // The crisp § 5.4 effect: pure-LTR content under dir=rtl must resolve
    // an RTL paragraph level — only the prepended RLM can do that (an
    // isolate wrap cannot set P2, the rejected runner-up).
    let mut app = text_app();
    let e = spawn_line(&mut app, "hello", Some(TextDirection::Rtl));
    settle(&mut app);
    let layout = app.world().get::<ComputedTextLayout>(e).unwrap();
    assert!(layout.lines[0].rtl, "RLM forced the paragraph level");
}

#[test]
fn ltr_component_forces_ltr_on_hebrew_text() {
    let mut app = text_app();
    let e = spawn_line(&mut app, "עולם", Some(TextDirection::Ltr));
    settle(&mut app);
    let layout = app.world().get::<ComputedTextLayout>(e).unwrap();
    assert!(!layout.lines[0].rtl, "LRM forced LTR over RTL content");
}

#[test]
fn auto_follows_first_strong() {
    let mut app = text_app();
    let heb = spawn_line(&mut app, "עולם", None);
    let lat = spawn_line(&mut app, "hello", None);
    settle(&mut app);
    assert!(app.world().get::<ComputedTextLayout>(heb).unwrap().lines[0].rtl);
    assert!(!app.world().get::<ComputedTextLayout>(lat).unwrap().lines[0].rtl);
}

#[test]
fn direction_change_retriggers_sync() {
    // TextDirection joins the § 5.1 trigger union: flipping it must reshape
    // (rtl flips) without touching Text.
    let mut app = text_app();
    let e = spawn_line(&mut app, "hello", Some(TextDirection::Rtl));
    settle(&mut app);
    assert!(app.world().get::<ComputedTextLayout>(e).unwrap().lines[0].rtl);
    app.world_mut().entity_mut(e).insert(TextDirection::Ltr);
    settle(&mut app);
    assert!(!app.world().get::<ComputedTextLayout>(e).unwrap().lines[0].rtl);
}
