//! E6 Task 6 — the `TextInput` widget bundle (editing-and-ime § 2.3). It
//! composes the core editor mechanism (`TextEditState` + `SingleLine` +
//! `Placeholder`) with widget policy (sizes, focusable, a11y, focus-on-click).
//! `buiy_widgets` names ZERO cosmic types — `TextEditState::for_font_size`
//! is the seam.

use bevy::prelude::*;
use buiy_core::a11y::{A11yPlaceholder, A11yRole, A11yTextValue};
use buiy_core::components::Node;
use buiy_core::focus::Focusable;
use buiy_core::layout::{BoxModel, Display, Edges, Length, Overflow, OverflowMode, Sizing};
use buiy_core::render::components::{Background, Border};
use buiy_core::text::edit::{Placeholder, SingleLine, TextEditState};
use buiy_core::text::{FontSize, Text};
use buiy_widgets::{TextInput, WidgetsPlugin};

/// The `#[require]` contract: spawning the **bare** `TextInput` marker (the
/// `bsn! { TextInput }` path) materializes the editor mechanism, the display
/// `Text` carrier, the layout-visible Style decomposition, paint, focus, and
/// a11y — everything `base_bundle()` assembles by hand. `SingleLine` is NOT
/// required (it is per-constructor policy added by `single_line()`).
#[test]
fn bare_text_input_marker_materializes_the_full_required_contract() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(TextInput).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Node>(entity).is_some(), "Node");
    assert!(world.get::<Display>(entity).is_some(), "Display");
    let box_model = world.get::<BoxModel>(entity).expect("BoxModel");
    assert_eq!(box_model.width, Sizing::Length(Length::Px(200.0)));
    assert_eq!(box_model.height, Sizing::Length(Length::Px(32.0)));
    assert_eq!(box_model.padding, Edges::all(8.0));
    // The input clips its content (overflow:hidden) so auto-scroll has a
    // scroll container to pan (text_input.rs base_bundle).
    let overflow = world.get::<Overflow>(entity).expect("Overflow");
    assert_eq!(overflow.x, OverflowMode::Hidden);
    assert_eq!(overflow.y, OverflowMode::Hidden);
    assert!(world.get::<Background>(entity).is_some(), "Background");
    assert!(world.get::<Border>(entity).is_some(), "Border");
    // Editor mechanism + display carrier.
    assert!(
        world.get::<TextEditState>(entity).is_some(),
        "TextEditState"
    );
    assert!(world.get::<Text>(entity).is_some(), "display Text carrier");
    assert!(world.get::<FontSize>(entity).is_some(), "FontSize");
    assert!(world.get::<Placeholder>(entity).is_some(), "Placeholder");
    assert!(world.get::<Focusable>(entity).is_some(), "Focusable");
    // `SingleLine` is NOT part of the base contract.
    assert!(
        world.get::<SingleLine>(entity).is_none(),
        "bare TextInput is not single-line"
    );
}

#[test]
fn single_line_text_input_composes_editor_markers_and_focusable() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app
        .world_mut()
        .spawn(TextInput::single_line("Search…"))
        .id();
    app.update();

    let world = app.world();
    assert!(
        world.get::<TextEditState>(entity).is_some(),
        "has the editor"
    );
    assert!(
        world.get::<SingleLine>(entity).is_some(),
        "single-line marker"
    );
    assert!(world.get::<Focusable>(entity).is_some(), "focusable");
    let placeholder = world.get::<Placeholder>(entity).expect("placeholder");
    assert_eq!(placeholder.0, "Search…");
    assert_eq!(
        world.get::<TextEditState>(entity).unwrap().value(),
        "",
        "a fresh input is empty"
    );
}

#[test]
fn multi_line_text_input_has_no_single_line_marker() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(TextInput::multi_line("")).id();
    app.update();

    assert!(
        app.world().get::<SingleLine>(entity).is_none(),
        "multi-line ⇒ no SingleLine"
    );
    assert!(app.world().get::<TextEditState>(entity).is_some());
}

/// Clicking a text input focuses it — C3c migrated the `focus_on_click` widget
/// policy off the legacy `Hovered` resource onto the bevy_picking `Pointer<E>`
/// layer. The driving mechanism changed (a synthetic `PointerInput` press over
/// the input's absolute box, through the real picking pipeline →
/// `Pointer<Press>` → the `focus_on_click` observer), but the asserted intent is
/// identical: a primary press on the input sets `FocusedEntity` to it.
///
/// The full Buiy backend is added explicitly so a `Pointer<Press>` fires; the
/// input is given a `ResolvedLayout` + `GlobalTransform` (the absolute basis
/// `emit_picks` reads) and the app stands up a window + `Camera2d`.
#[test]
fn clicking_a_text_input_focuses_it() {
    use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
    use bevy::picking::pointer::{
        Location, PointerAction, PointerButton, PointerId, PointerLocation,
    };
    use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};
    use buiy_core::FocusedEntity;
    use buiy_core::ResolvedLayout;
    use buiy_core::focus::FocusPlugin;
    use buiy_core::picking::{BuiyPickingBackendPlugin, PickingPlugin};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(PickingPlugin);
    app.add_plugins(BuiyPickingBackendPlugin);
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    // `FocusPlugin::handle_tab` reads `Res<ButtonInput<KeyCode>>` (the resource a
    // real app gets from `InputPlugin`, absent under MinimalPlugins) — seed it so
    // the focus systems validate.
    app.init_resource::<ButtonInput<KeyCode>>();

    // A synthetic primary window + a Camera2d targeting it (the §3.1 camera
    // resolution the backend needs).
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(800, 600),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    app.world_mut()
        .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(window))));

    let entity = app.world_mut().spawn(TextInput::single_line("")).id();
    // The input's absolute box (0,0)..(200,32) — give it the absolute basis
    // `emit_picks` reads without the full layout→bridge chain.
    app.world_mut().entity_mut(entity).insert((
        ResolvedLayout {
            position: Vec2::ZERO,
            size: Vec2::new(200.0, 32.0),
        },
        GlobalTransform::IDENTITY,
    ));

    // Aim the synthetic pointer at the input's center and press.
    let target = WindowRef::Entity(window).normalize(Some(window)).unwrap();
    let location = Location {
        target: NormalizedRenderTarget::Window(target),
        position: Vec2::new(100.0, 16.0),
    };
    app.world_mut()
        .spawn((PointerId::Mouse, PointerLocation::new(location.clone())));
    app.update(); // backend emits a hit; the hover stage registers Over.
    app.world_mut()
        .write_message(bevy::picking::pointer::PointerInput {
            pointer_id: PointerId::Mouse,
            location,
            action: PointerAction::Press(PointerButton::Primary),
        });
    app.update();

    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(entity),
        "click focuses the input (widget policy)"
    );
}

/// C2 § 5 step 5 — removing the `Text`→editor content seam (the Bug-3 fix)
/// introduces NO seed regression for the empty case: a bare `TextInput`
/// (`Text("")` + `TextEditState::for_font_size`, no explicit seed verb) is `""`
/// at construction AND stays `""` after a `FontsGeneration` bump (the style-only
/// TextSync path never re-`set_text`s the empty editor buffer). `BuiyTextPlugin`
/// is added explicitly so the bump's TextSync sweep actually runs over the
/// widget's editor buffer (WidgetsPlugin alone does not register TextSync — the
/// bump would otherwise be inert and the test vacuous).
#[test]
fn bare_text_input_value_stays_empty_across_a_fonts_generation_bump() {
    use buiy_core::text::FontsGeneration;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_plugins(WidgetsPlugin);

    let entity = app
        .world_mut()
        .spawn(TextInput::single_line("Search…"))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<TextEditState>(entity).unwrap().value(),
        "",
        "precondition: a bare TextInput seeds \"\" (no explicit seed verb needed for the empty case)"
    );

    // Bump FontsGeneration (the runtime add_font / system-font-scan trigger) and
    // run a frame. With the § 2.1 style-only path, the empty editor buffer is
    // never set_text'd to anything else — it stays "".
    app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
    app.update();

    assert_eq!(
        app.world().get::<TextEditState>(entity).unwrap().value(),
        "",
        "a bare TextInput's value stays \"\" after a bump — the Text->editor seam \
         removal (Bug-3 fix) introduces NO seed regression for the empty case (§ 5 step 5)"
    );
}

// ---------------------------------------------------------------------------
// P1d slice-4 — the a11y role split + the synced state + the driver SetValue.
// ---------------------------------------------------------------------------

/// GATE #3 (bundle half): the **role split** is the multiline distinction
/// (widget-contracts.md §5). `single_line()` carries `A11yRole::TextInput`;
/// `multi_line()` / the bare marker carry `A11yRole::MultilineTextInput`. Both
/// carry the synced state carriers `A11yTextValue` + `A11yPlaceholder` (the old
/// `A11yRole::Text` stopgap is retired).
#[test]
fn text_input_role_split_single_vs_multi_and_synced_state_present() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let single = app
        .world_mut()
        .spawn(TextInput::single_line("Search…"))
        .id();
    let multi = app.world_mut().spawn(TextInput::multi_line("Body")).id();
    let bare = app.world_mut().spawn(TextInput).id();
    app.update();

    let world = app.world();
    // Single-line ⇒ Role::TextInput (the single-line role override).
    assert_eq!(
        world.get::<A11yRole>(single).copied(),
        Some(A11yRole::TextInput),
        "single_line ⇒ A11yRole::TextInput"
    );
    // Multi-line + the bare marker ⇒ Role::MultilineTextInput (the #[require] default).
    assert_eq!(
        world.get::<A11yRole>(multi).copied(),
        Some(A11yRole::MultilineTextInput),
        "multi_line ⇒ A11yRole::MultilineTextInput"
    );
    assert_eq!(
        world.get::<A11yRole>(bare).copied(),
        Some(A11yRole::MultilineTextInput),
        "the bare TextInput marker defaults to MultilineTextInput (no SingleLine)"
    );
    // Both carry the synced a11y state carriers (the contract + driver read them).
    for e in [single, multi, bare] {
        assert!(
            world.get::<A11yTextValue>(e).is_some(),
            "A11yTextValue carrier present"
        );
        assert!(
            world.get::<A11yPlaceholder>(e).is_some(),
            "A11yPlaceholder carrier present"
        );
    }
    // The stopgap is gone: no TextInput widget carries A11yRole::Text.
    for e in [single, multi, bare] {
        assert_ne!(
            world.get::<A11yRole>(e).copied(),
            Some(A11yRole::Text),
            "the A11yRole::Text stopgap is retired"
        );
    }
}

/// The `sync_text_input_a11y` system mirrors the editor's live value into
/// `A11yTextValue` and the `Placeholder` into `A11yPlaceholder` after a settle
/// frame. `BuiyTextPlugin` is added so the editor's TextSync runs and the sync
/// system has a real editor to read.
#[test]
fn a11y_text_value_and_placeholder_sync_from_the_editor() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(buiy_core::a11y::A11yPlugin);
    app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_plugins(WidgetsPlugin);

    let e = app
        .world_mut()
        .spawn(TextInput::single_line("Search…"))
        .id();
    app.update();

    // Placeholder is mirrored into A11yPlaceholder; the empty editor ⇒ "".
    assert_eq!(
        app.world().get::<A11yPlaceholder>(e).map(|p| p.0.clone()),
        Some("Search…".to_string()),
        "A11yPlaceholder mirrors the Placeholder string"
    );
    assert_eq!(
        app.world().get::<A11yTextValue>(e).map(|v| v.0.clone()),
        Some(String::new()),
        "a fresh input's A11yTextValue is empty"
    );
}

/// DRIVER ACCEPTANCE: `get_by_role(TextInput)` → driver `set_value("hello")` →
/// the editor content becomes "hello" AND `A11yTextValue` reflects "hello" after a
/// settle frame (the `perform`-then-`update` contract). Typing via the keyboard
/// still works through the existing editor path.
#[test]
fn driver_set_value_drives_the_text_input_and_a11y_reflects() {
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput};
    use buiy_core::a11y::inprocess::TreeView;
    use buiy_core::a11y::{get_by_role, set_value, snapshot};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(buiy_core::a11y::A11yPlugin);
    app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    // Keyboard infra MinimalPlugins/BuiyTextPlugin do not seed: `apply_keyboard_edits`
    // reads `Messages<KeyboardInput>` + `Res<ButtonInput<KeyCode>>` (both optional —
    // absent ⇒ the system no-ops), so the keyboard-still-works leg needs them present.
    app.add_message::<KeyboardInput>();
    app.init_resource::<ButtonInput<KeyCode>>();

    let e = app
        .world_mut()
        .spawn(TextInput::single_line("Search…"))
        .id();
    // Settle a frame so build_tree populates the a11y tree.
    app.update();

    // Address the single-line input by role (strict single match).
    let node = get_by_role(app.world_mut(), A11yRole::TextInput, None, None)
        .expect("exactly one TextInput node");

    // Drive set_value through the inbound seam (act-then-observe). The editor
    // mutates synchronously inside `perform`.
    set_value(app.world_mut(), node, "hello").expect("set_value honored");
    assert_eq!(
        app.world().get::<TextEditState>(e).unwrap().value(),
        "hello",
        "the driver set_value lowers through SelectAll+Insert → editor is \"hello\""
    );

    // Settle a frame so `sync_text_input_a11y` reflects the new value into
    // `A11yTextValue` and `build_tree` re-emits it; then snapshot through the
    // consumer and assert the node's value field reads "hello".
    app.update();
    assert_eq!(
        app.world().get::<A11yTextValue>(e).map(|v| v.0.clone()),
        Some("hello".to_string()),
        "A11yTextValue reflects the new editor value after a settle frame"
    );
    let tree = snapshot(app.world_mut(), TreeView::default());
    let observed = tree
        .by_role(A11yRole::TextInput)
        .next()
        .expect("the TextInput node is in the snapshot");
    assert_eq!(
        observed.state.value.as_deref(),
        Some("hello"),
        "the consumer-observed node value reflects \"hello\" (round-trip through the a11y tree)"
    );
    // The SetValue/Focus/Blur advertisement is asserted at the core layer
    // (`a11y_action::text_input_contract_advertises_only_set_value` + the inprocess
    // `actions` projection); `buiy_widgets` does not depend on `accesskit`, so it
    // does not re-assert the raw `Action` set here.

    // Typing via the keyboard still works through the existing editor path: set
    // focus, send a Character key, and the editor inserts it (the SetValue lowering
    // is additive — it does not displace the keyboard path).
    app.world_mut().resource_mut::<buiy_core::FocusedEntity>().0 = Some(e);
    // Clear first (a fresh SelectAll+Insert) so we type onto a known base.
    set_value(app.world_mut(), node, "").expect("clear honored");
    app.update();
    let window = app.world_mut().spawn(()).id();
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyZ,
        logical_key: Key::Character("z".into()),
        state: ButtonState::Pressed,
        text: Some("z".into()),
        repeat: false,
        window,
    });
    app.update();
    assert_eq!(
        app.world().get::<TextEditState>(e).unwrap().value(),
        "z",
        "keyboard typing still inserts through the existing editor path (apply_keyboard_edits)"
    );
}
