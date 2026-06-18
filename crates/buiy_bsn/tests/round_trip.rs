//! BSN round-trip authorability — the headless "it works without a GPU" proof
//! (spec § 5). Authors real Buiy components/widgets with the **real** `bsn!`
//! macro (via `buiy_bsn::prelude`), spawns the scenes against a `World`, and
//! asserts the resulting entities carry the authored components with the
//! patched field values. This exercises the actual upstream `Template`/`Scene`
//! path — no mock.
//!
//! Required cases (spec § 5):
//!   (a) bare plain-data components with field patches;
//!   (b) a styled widget — the `#[require]` contract materialized AND the
//!       patches applied (the load-bearing § 4.1a case);
//!   (c) a `Children [ … ]`-nested subtree;
//!   (d) a `#Name` entity ref.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::name::Name;
use bevy::scene::ScenePlugin;
// The crate under test: the BSN authoring surface. `bsn!` and `WorldSceneExt`
// (the `spawn_scene` extension) both resolve through this prelude — proving the
// `buiy_bsn` re-exports are sufficient to author Buiy components in BSN.
use buiy_bsn::prelude::*;
use buiy_core::a11y::A11yRole;
use buiy_core::components::Node;
use buiy_core::focus::Focusable;
use buiy_core::layout::{BoxModel, Display, Edges, FlexParams, Length, Overflow, Position, Sizing};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Background;
use buiy_widgets::{Button, TextInput, WidgetsPlugin};
use std::borrow::Cow;

/// A headless app with the BSN spawn machinery (`AssetPlugin` + `ScenePlugin`
/// back `spawn_scene`) plus the Buiy plugins that register the widget
/// required-components before first spawn (`CorePlugin` + `WidgetsPlugin` +
/// `BuiyTextPlugin`). No render plugin, no GPU.
fn bsn_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(buiy_core::CorePlugin)
        // `TextInput`'s required `TextEditState`/`Text`/`FontSize` ride the
        // text plugin's registrations; `WidgetsPlugin` registers the widget
        // markers (and thus their required-components) before any spawn.
        .add_plugins(buiy_core::text::BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);
    app
}

/// (a) Bare plain-data components author and patch through the blanket
/// `Component + Clone + Default` template path — no `#[derive(FromTemplate)]`,
/// no reflection.
#[test]
fn case_a_plain_data_components_patch() {
    let mut app = bsn_test_app();

    let id = app
        .world_mut()
        .spawn_scene(bsn! {
            Background { color: { ColorToken::Token(Cow::Borrowed("color.brand")) } }
            BoxModel {
                width: { Sizing::Length(Length::Px(64.0)) },
                padding: { Edges::all(4.0) },
            }
        })
        .expect("spawn_scene")
        .id();

    let world = app.world();
    let bg = world.get::<Background>(id).expect("Background present");
    assert_eq!(bg.color, ColorToken::Token(Cow::Borrowed("color.brand")));

    let bm = world.get::<BoxModel>(id).expect("BoxModel present");
    // Patched fields applied…
    assert_eq!(bm.width, Sizing::Length(Length::Px(64.0)));
    assert_eq!(bm.padding, Edges::all(4.0));
    // …unspecified fields fall back to the component `Default`.
    assert_eq!(bm.height, Sizing::default());
    assert_eq!(bm.margin, Edges::default());
}

/// (b) The load-bearing case (spec § 4.1a): a styled **widget**. Authoring the
/// bare `Button` marker materializes the full `#[require]` contract, and
/// explicit component patches layer over the required defaults. `Background`
/// is a named-field struct (not a tuple struct), so it patches as
/// `Background { color: … }`.
#[test]
fn case_b_styled_widget_require_plus_patches() {
    let mut app = bsn_test_app();

    let brand = ColorToken::Token(Cow::Borrowed("color.brand"));
    let id = app
        .world_mut()
        .spawn_scene(bsn! {
            Button
            Background { color: { brand.clone() } }
            BoxModel { width: { Sizing::Length(Length::Px(240.0)) } }
        })
        .expect("spawn_scene")
        .id();

    let world = app.world();

    // The `#[require]` contract materialized from the bare `Button` marker:
    // the layout-visible Style decomposition + focus + a11y.
    assert!(world.get::<Node>(id).is_some(), "Node (required)");
    assert!(world.get::<Display>(id).is_some(), "Display (required)");
    assert!(world.get::<Position>(id).is_some(), "Position (required)");
    assert!(
        world.get::<FlexParams>(id).is_some(),
        "FlexParams (required)"
    );
    assert!(world.get::<Overflow>(id).is_some(), "Overflow (required)");
    assert!(world.get::<Focusable>(id).is_some(), "Focusable (required)");
    assert_eq!(
        world.get::<A11yRole>(id).copied(),
        Some(A11yRole::Button),
        "A11yRole defaults to Button via #[require]"
    );

    // The explicit patches override the required defaults (required-components
    // are a no-op when the component is explicitly inserted).
    let bg = world.get::<Background>(id).expect("Background present");
    assert_eq!(
        bg.color, brand,
        "Background patch overrides the button default"
    );
    let bm = world.get::<BoxModel>(id).expect("BoxModel present");
    assert_eq!(
        bm.width,
        Sizing::Length(Length::Px(240.0)),
        "BoxModel.width patch applied"
    );
    // Authoring an explicit `BoxModel` patch *replaces* the widget's
    // `#[require(BoxModel = button_box_model())]` initializer entirely:
    // required-components only fill a *missing* component, and the patched
    // `BoxModel` is present, so the require initializer is suppressed. A BSN
    // patch therefore layers onto the **component `Default`** (padding 0), NOT
    // onto the require initializer (which would have been 8px). This is
    // upstream required-component semantics — author the full box when you
    // patch it, or omit the patch to keep the widget's canonical box.
    assert_eq!(
        bm.padding,
        Edges::default(),
        "patching BoxModel suppresses the require initializer; unpatched fields are the component Default"
    );
    assert_eq!(bm.padding, Edges::all(0.0));
}

/// (c) A `Children [ … ]`-nested subtree. The parens group multiple components
/// onto one child; nested children inherit the same authoring contract.
#[test]
fn case_c_children_nested_subtree() {
    let mut app = bsn_test_app();

    let id = app
        .world_mut()
        .spawn_scene(bsn! {
            Node
            Children [
                (Button BoxModel { width: { Sizing::Length(Length::Px(80.0)) } }),
                (TextInput),
            ]
        })
        .expect("spawn_scene")
        .id();

    let world = app.world();
    let children = world.get::<Children>(id).expect("root has Children");
    assert_eq!(children.len(), 2, "two authored children");

    // Child 0: a Button with the require contract + a width patch.
    let c0 = children[0];
    assert!(world.get::<Button>(c0).is_some(), "child 0 is a Button");
    assert!(
        world.get::<Focusable>(c0).is_some(),
        "child Button required Focusable"
    );
    assert_eq!(
        world.get::<BoxModel>(c0).expect("child BoxModel").width,
        Sizing::Length(Length::Px(80.0)),
        "child Button width patch applied"
    );

    // Child 1: a TextInput, full require contract.
    let c1 = children[1];
    assert!(
        world.get::<TextInput>(c1).is_some(),
        "child 1 is a TextInput"
    );
    assert!(
        world
            .get::<buiy_core::text::edit::TextEditState>(c1)
            .is_some(),
        "child TextInput required the editor mechanism"
    );
}

/// (d) A `#Name` entity ref: `#Root` inserts `Name("Root")` and registers the
/// entity in macro scope.
#[test]
fn case_d_named_entity_ref() {
    let mut app = bsn_test_app();

    let id = app
        .world_mut()
        .spawn_scene(bsn! {
            #Root
            Background { color: { ColorToken::CurrentColor } }
        })
        .expect("spawn_scene")
        .id();

    let world = app.world();
    let name = world.get::<Name>(id).expect("#Root inserted a Name");
    assert_eq!(name.as_str(), "Root");
    assert_eq!(
        world.get::<Background>(id).expect("Background").color,
        ColorToken::CurrentColor
    );
}
