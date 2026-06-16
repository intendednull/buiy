//! E6 Task 7 — the editing Message taxonomy audit (editing-and-ime § 11). Every
//! row of the § 11 table must EXIST as a registered Bevy `Message` after
//! `BuiyTextPlugin`. This is the campaign's completeness gate: `TextChanged`
//! (E2), `SelectionChanged`/`CaretMoved` (E3), `EditUndone`/`EditRedone` (E4),
//! `CompositionStart/Update/End` (E5), `EditSubmitted` (E6).

use bevy::prelude::*;
use buiy_core::text::BuiyTextPlugin;
use buiy_core::text::edit::{
    CaretMoved, CompositionEnd, CompositionStart, CompositionUpdate, EditRedone, EditSubmitted,
    EditUndone, SelectionChanged, TextChanged,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(BuiyTextPlugin::default());
    app
}

/// A registered `Message` has a `Messages<T>` resource. Assert each row.
macro_rules! assert_registered {
    ($app:expr, $($t:ty),+ $(,)?) => {
        $(
            assert!(
                $app.world().get_resource::<Messages<$t>>().is_some(),
                "§ 11 taxonomy: {} must be a registered Message",
                std::any::type_name::<$t>()
            );
        )+
    };
}

#[test]
fn full_section_11_taxonomy_is_registered() {
    let app = app();
    assert_registered!(
        app,
        TextChanged,
        SelectionChanged,
        CaretMoved,
        EditUndone,
        EditRedone,
        CompositionStart,
        CompositionUpdate,
        CompositionEnd,
        EditSubmitted,
    );
}
