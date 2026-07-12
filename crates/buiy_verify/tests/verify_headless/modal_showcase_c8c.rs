//! C8-c — the **S4 (modal + focus-trap) + S5 (Controls showcase) inspection-driver
//! acceptance** (widget-gallery-exemplar §6 / co-drive grounding loops). The LAST
//! pair of screens. Mirrors the C8-a/b acceptance pattern (`todomvc_c8a.rs`,
//! `scroll_overlay_c8b.rs`): every interaction is driven through the C7
//! `PointerHarness` (real synthetic pointer/keyboard over the production focus +
//! dialog-lifecycle path) or the in-process a11y driver
//! (`buiy_core::a11y::inprocess`: `get_by_role`/`snapshot`/`click`/`increment`/
//! `expand`) and asserted through the live state + the a11y tree — never by reading
//! bespoke internal state. The PAINT is asserted at the display-list / extract tier
//! (the border + shadow channels), headlessly, with no GPU.
//!
//! The screens + their composition are `buiy_gallery::{spawn_modal,
//! spawn_showcase, …}` (pure composition over the landed C5-d Dialog lifecycle +
//! the C6 styling + the P1d widgets — the S5 controls grid restyled to exact
//! design parity in parity Wave C3). These are the live gates; the static layout
//! snapshots live in `examples/buiy_gallery/tests`.
//!
//! ## S4 — modal + focus-trap (the C5-d behaviors, composed in a gallery screen)
//!  - **Open**: activating the invoker (the shared `OnPress` sink) opens the
//!    controlled dialog — `A11yModal` is in the snapshot + focus moves to the
//!    dialog's first focusable.
//!  - **Tab traps + wraps**: with the modal open, Tab cycles ONLY the dialog's
//!    focusables and WRAPS; the background button is NEVER reached.
//!  - **Esc closes + restores**: Escape closes the dialog (WCAG 2.1.2) and restores
//!    focus to the invoker (WCAG 2.4.3).
//!  - **Inert background**: while open, the background root is `A11yHidden` →
//!    pruned from the a11y tree (the snapshot shows only the modal subtree);
//!    restored on close.
//!
//! ## S5 — Controls showcase (function via the driver + the design paint)
//!  - **Switch toggles** (driver `click` → `OnPress` → the toggle consumer): the
//!    `A11yToggled` flips, observed through the a11y tree.
//!  - **Slider increments** (driver `increment`): the slider contract raises the
//!    live `A11yValue.now` by step, observed through the a11y tree.
//!  - **Disclosure expands** (driver `expand` / `click`): the `A11yExpanded` flips,
//!    observed through the a11y tree.
//!  - **Design paint**: the five flat controls cards each emit a per-side `Border`
//!    band, the slider preview square emits its single accent-glow `BoxShadow`, and
//!    a keyboard-focused widget emits the C6-a focus-ring `Outline` band — all at
//!    the display-list tier, headlessly.
//!  - **(Optional, `#[ignore]` GPU lane)** a programmatic readback that the slider
//!    preview paints non-canvas pixels on a real adapter (adapter-tolerant).

use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use buiy_core::a11y::inprocess::snapshot;
use buiy_core::a11y::{
    A11yModal, A11yPlugin, A11yRole, A11yToggled, A11yValue, Toggled, click, expand, get_by_role,
    increment,
};
use buiy_core::focus::{FocusPlugin, FocusVisible, FocusedEntity};
use buiy_core::text::BuiyTextPlugin;
use buiy_gallery::{
    MODAL_BG_BUTTON, MODAL_INVOKER, SHOWCASE_DISCLOSURE, SHOWCASE_SLIDER, SHOWCASE_SLIDER_NOW,
    SHOWCASE_SLIDER_STEP, SHOWCASE_SWITCH, ShowcasePlugin, ShowcasePreview, showcase_card_border,
    showcase_preview_shadow, spawn_modal, spawn_showcase,
};
use buiy_verify::pointer::PointerHarness;
use buiy_widgets::WidgetsPlugin;

// ###########################################################################
// S4 — modal + focus-trap
// ###########################################################################

// ---------------------------------------------------------------------------
// S4 open / trap / Esc / restore — driven on the C7 PointerHarness. The dialog
// open is driven by focusing the invoker + writing the shared `OnPress` sink (the
// keyboard/AT activation route the Button keymap / AT-`Click` converge on, the
// `dialog_modal_c5d` isolation). The whole open/close/focus-trap/Esc/restore +
// inert-background lifecycle is the C5-d `WidgetsPlugin` overlay state machine; S4
// is pure composition over `spawn_modal`.
// ---------------------------------------------------------------------------

fn is_open(world: &World, e: Entity) -> bool {
    buiy_widgets::popover::is_open(world.get::<buiy_core::render::components::CssVisibility>(e))
}

fn focused(world: &World) -> Option<Entity> {
    world.resource::<FocusedEntity>().0
}

/// The dialog's focusable descendants (the Switch + the Close button), in document
/// order — the set the trap cycles. A focusable is `With<Focusable>`.
fn dialog_focusables(world: &mut World, dialog: Entity) -> Vec<Entity> {
    fn dfs(world: &World, e: Entity, out: &mut Vec<Entity>) {
        if world.get::<buiy_core::focus::Focusable>(e).is_some() {
            out.push(e);
        }
        if let Some(children) = world.get::<Children>(e) {
            let kids: Vec<Entity> = children.iter().collect();
            for c in kids {
                dfs(world, c, out);
            }
        }
    }
    let mut out = Vec::new();
    if let Some(children) = world.get::<Children>(dialog) {
        let kids: Vec<Entity> = children.iter().collect();
        for c in kids {
            dfs(world, c, &mut out);
        }
    }
    out
}

/// Open the dialog via the keyboard/AT activation route: focus the invoker, then
/// write the shared `OnPress(invoker)` sink. Settles the C5-d lifecycle (open →
/// inert + FocusReturn → deferred focus-into).
fn open_via_invoker(h: &mut PointerHarness, invoker: Entity) {
    h.world_mut().resource_mut::<FocusedEntity>().0 = Some(invoker);
    h.world_mut()
        .write_message(buiy_core::interaction::OnPress(invoker));
    for _ in 0..6 {
        h.update();
    }
}

#[test]
fn s4_invoker_activation_opens_the_modal_and_focuses_inside() {
    let mut h = PointerHarness::new();
    let (invoker, dialog, _bg) = spawn_modal(h.world_mut());
    for _ in 0..4 {
        h.update();
    }
    assert!(!is_open(h.world(), dialog), "the dialog starts closed");

    let focusables = dialog_focusables(h.world_mut(), dialog);
    assert!(
        !focusables.is_empty(),
        "the dialog has focusable controls (Switch + Close)"
    );

    open_via_invoker(&mut h, invoker);

    assert!(
        is_open(h.world(), dialog),
        "the invoker activation opened the dialog"
    );
    assert!(
        h.world()
            .resource::<buiy_core::layout::TopLayerActivation>()
            .order
            .contains(&dialog),
        "the open modal joined the TopLayerActivation deque"
    );
    // Focus moved INSIDE the dialog (to its first focusable descendant).
    let f = focused(h.world());
    assert!(
        f.is_some_and(|f| focusables.contains(&f)),
        "focus moved to a dialog focusable on open (got {f:?}, dialog focusables {focusables:?})"
    );
}

#[test]
fn s4_tab_traps_inside_the_modal_and_wraps() {
    let mut h = PointerHarness::new();
    let (invoker, dialog, bg) = spawn_modal(h.world_mut());
    for _ in 0..4 {
        h.update();
    }
    let focusables = dialog_focusables(h.world_mut(), dialog);
    open_via_invoker(&mut h, invoker);
    assert!(is_open(h.world(), dialog));
    // The parity create-modal (Wave C3) has EIGHT focusable controls: the header
    // close ×, the Name input, the three Kind segmented buttons, the Register
    // switch, and the footer Cancel + Create confirm. (The delete body carries no
    // focusable — it is a warning tile + text — and is `Display::None` at rest.)
    assert_eq!(
        focusables.len(),
        8,
        "eight focusable controls in the create modal (close, name, 3×kind, switch, \
         cancel, confirm); got {focusables:?}"
    );

    // Tab N+ times: focus is ALWAYS one of the dialog's focusables — never the
    // invoker, never the background button (the C5-d trap confines Tab to the modal
    // subtree, however many focusables it has).
    for i in 0..(focusables.len() * 2) {
        h.press_key(KeyCode::Tab);
        let f = focused(h.world());
        assert!(
            f.is_some_and(|f| focusables.contains(&f)),
            "Tab #{i}: focus stayed inside the modal (was {f:?}); never the invoker \
             ({invoker:?}) or the background ({bg:?})"
        );
    }

    // The trap WRAPS: from any stop, N Tabs (N = focusable count) return to it, and
    // each single Tab advances (no stuck cursor). Establish a stop, then walk a full
    // cycle and confirm we land back on it.
    h.press_key(KeyCode::Tab);
    let start = focused(h.world());
    let mut prev = start;
    for step in 1..focusables.len() {
        h.press_key(KeyCode::Tab);
        let now = focused(h.world());
        assert_ne!(
            prev, now,
            "Tab step {step} advanced to a different focusable (no stuck cursor)"
        );
        prev = now;
    }
    // The Nth Tab wraps back to the starting stop.
    h.press_key(KeyCode::Tab);
    let wrapped = focused(h.world());
    assert_eq!(
        start, wrapped,
        "Tab wraps within the modal after a full cycle (back to the same stop)"
    );
}

#[test]
fn s4_escape_closes_the_modal_and_restores_focus_to_the_invoker() {
    let mut h = PointerHarness::new();
    let (invoker, dialog, _bg) = spawn_modal(h.world_mut());
    for _ in 0..4 {
        h.update();
    }
    open_via_invoker(&mut h, invoker);
    assert!(
        is_open(h.world(), dialog),
        "the dialog is open before Escape"
    );

    h.press_key(KeyCode::Escape);
    for _ in 0..3 {
        h.update();
    }

    assert!(
        !is_open(h.world(), dialog),
        "Escape closed the dialog (WCAG 2.1.2 — Escape always escapes)"
    );
    assert_eq!(
        focused(h.world()),
        Some(invoker),
        "closing restored focus to the invoker (WCAG 2.4.3)"
    );
}

// ---------------------------------------------------------------------------
// S4 inert background — driven on a full A11yPlugin app so build_tree populates
// the canonical tree the snapshot reads.
// ---------------------------------------------------------------------------

/// A headless app with the a11y + focus + layout + widget surface (build_tree runs,
/// so `snapshot` reflects the live tree). Mirrors `dialog_modal_c5d::a11y_app`.
fn modal_a11y_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(FocusPlugin);
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    app.add_plugins(WidgetsPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

#[test]
fn s4_open_modal_prunes_the_background_from_the_a11y_tree_and_restores_on_close() {
    let mut app = modal_a11y_app();
    let (invoker, dialog, _bg) = spawn_modal(app.world_mut());
    for _ in 0..4 {
        app.update();
    }

    // Before open: the background button IS in the a11y tree.
    let before = snapshot(app.world_mut(), Default::default());
    assert!(
        before
            .by_role(A11yRole::Button)
            .any(|n| n.name == MODAL_BG_BUTTON),
        "the background button is in the a11y tree before the modal opens"
    );

    // Open via the invoker route (focus invoker + OnPress).
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(invoker);
    app.world_mut()
        .write_message(buiy_core::interaction::OnPress(invoker));
    for _ in 0..6 {
        app.update();
    }
    assert!(is_open(app.world(), dialog), "the dialog opened");

    // The dialog reports modal through the consumer (the A11yModal in the snapshot).
    let during = snapshot(app.world_mut(), Default::default());
    let dialog_node = during
        .by_role(A11yRole::Dialog)
        .next()
        .expect("the dialog is in the a11y tree while open");
    assert!(
        dialog_node.state.modal,
        "the open dialog reports A11yModal through the consumer (modal in the snapshot)"
    );
    // The component is on the live entity too (the require contract carries it).
    assert!(
        app.world().get::<A11yModal>(dialog).is_some(),
        "the dialog carries A11yModal"
    );

    // While open: the background button + the invoker are pruned; the modal's own
    // controls (the Close button) survive.
    assert!(
        !during
            .by_role(A11yRole::Button)
            .any(|n| n.name == MODAL_BG_BUTTON),
        "the background button is pruned from the a11y tree while the modal is open"
    );
    assert!(
        !during
            .by_role(A11yRole::Button)
            .any(|n| n.name == MODAL_INVOKER),
        "the invoker (a background root) is pruned while the modal is open"
    );
    assert!(
        during.by_role(A11yRole::Button).any(|n| n.name == "Close"),
        "a focusable inside the dialog survives the prune"
    );

    // Close: the prune is reversed — the background button + invoker return.
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::Escape);
    }
    app.update();
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
    }
    for _ in 0..3 {
        app.update();
    }
    assert!(!is_open(app.world(), dialog), "Escape closed the dialog");
    let after = snapshot(app.world_mut(), Default::default());
    assert!(
        after
            .by_role(A11yRole::Button)
            .any(|n| n.name == MODAL_BG_BUTTON),
        "the background button returns to the a11y tree after the modal closes"
    );
    assert!(
        after
            .by_role(A11yRole::Button)
            .any(|n| n.name == MODAL_INVOKER),
        "the invoker returns to the a11y tree after the modal closes"
    );
}

// ---------------------------------------------------------------------------
// S4 create-form (parity Wave C3) — the Kind segmented selection + the Register
// switch, the two interactive controls inside the create body. Driven over the
// shared `OnPress` sink + the in-process `click` (the same routes a real
// pointer/keyboard/AT use); the `ModalPlugin` create-form app logic restyles the
// Kind group, and the buiy_widgets `Switch` toggles itself.
// ---------------------------------------------------------------------------

/// A headless app with the a11y/focus/layout/widget surface PLUS the gallery's
/// `ModalPlugin` (the create/delete body swap + Kind-selection wiring). The Kind
/// segmented restyle + the body swap need it; the C5-d open/trap/Esc lifecycle is
/// `WidgetsPlugin`. No ScenePlugin — `spawn_modal` is scene-free.
fn modal_form_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(FocusPlugin);
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.add_plugins(WidgetsPlugin);
    app.add_plugins(buiy_gallery::ModalPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

/// The modal's Kind segmented option buttons, in index order (the C2
/// `SegmentedOption(idx)` children of the modal `ModalKindTrack`).
fn kind_options(world: &mut World) -> Vec<(Entity, usize)> {
    let track = {
        let mut q = world.query_filtered::<Entity, With<buiy_gallery::ModalKindTrack>>();
        q.iter(world).next().expect("the modal has a Kind track")
    };
    let kids: Vec<Entity> = world
        .get::<Children>(track)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    let mut opts: Vec<(Entity, usize)> = kids
        .into_iter()
        .filter_map(|c| {
            world
                .get::<buiy_gallery::composites::SegmentedOption>(c)
                .map(|o| (c, o.0))
        })
        .collect();
    opts.sort_by_key(|&(_, idx)| idx);
    opts
}

/// The `Background` color token of an entity (the segmented pill fill), for the
/// selected/unselected assertion.
fn bg_token(world: &World, e: Entity) -> Option<buiy_core::render::ColorToken> {
    Some(
        world
            .get::<buiy_core::render::components::Background>(e)?
            .color,
    )
}

#[test]
fn s4_create_form_kind_selection_restyles_the_segmented_group() {
    let mut app = modal_form_app();
    let (invoker, dialog, _delete) = spawn_modal(app.world_mut());
    for _ in 0..4 {
        app.update();
    }

    // Open in create mode (the default) so the Kind body is visible.
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(invoker);
    app.world_mut()
        .write_message(buiy_core::interaction::OnPress(invoker));
    for _ in 0..4 {
        app.update();
    }
    assert!(is_open(app.world(), dialog), "the create modal opened");

    let opts = kind_options(app.world_mut());
    assert_eq!(
        opts.len(),
        3,
        "three Kind options (Button / Layout / Input)"
    );
    // At rest the first option (Button, idx 0) is the selected accent pill; the
    // others are transparent.
    assert_eq!(
        bg_token(app.world(), opts[0].0),
        Some(buiy_core::render::ColorToken::Accent),
        "Button is the selected pill at rest"
    );
    assert_eq!(
        bg_token(app.world(), opts[1].0),
        Some(buiy_core::render::ColorToken::Transparent),
        "Layout is unselected at rest"
    );

    // Press the "Layout" option (idx 1) over the shared OnPress sink; ModalPlugin's
    // `select_modal_kind` restyles the group so Layout becomes the accent pill.
    let layout = opts[1].0;
    app.world_mut()
        .write_message(buiy_core::interaction::OnPress(layout));
    for _ in 0..2 {
        app.update();
    }
    assert_eq!(
        bg_token(app.world(), opts[1].0),
        Some(buiy_core::render::ColorToken::Accent),
        "selecting Layout made it the accent pill"
    );
    assert_eq!(
        bg_token(app.world(), opts[0].0),
        Some(buiy_core::render::ColorToken::Transparent),
        "Button deselected when Layout was chosen (exclusive segmented group)"
    );
}

#[test]
fn s4_create_form_register_switch_toggles() {
    let mut app = modal_form_app();
    let (invoker, dialog, _delete) = spawn_modal(app.world_mut());
    for _ in 0..4 {
        app.update();
    }
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(invoker);
    app.world_mut()
        .write_message(buiy_core::interaction::OnPress(invoker));
    for _ in 0..4 {
        app.update();
    }
    assert!(is_open(app.world(), dialog), "the create modal opened");

    // The Register switch — addressable by role+name. It defaults ON (the design's
    // `mPublic:true`).
    let sw = get_by_role(
        app.world_mut(),
        A11yRole::Switch,
        Some("Register globally"),
        None,
    )
    .expect("the register switch is addressable by role+name");
    let sw_entity = buiy_core::a11y::translate::entity_for_node_id(sw).unwrap();
    assert_eq!(
        app.world().get::<A11yToggled>(sw_entity).map(|t| t.0),
        Some(Toggled::True),
        "the register switch defaults ON (mPublic:true)"
    );

    // Driver click → OnPress → the toggle consumer flips it OFF (the control still
    // functions inside the open modal's focus trap).
    click(app.world_mut(), sw).expect("AT click on the register switch honored");
    app.update();
    assert_eq!(
        app.world().get::<A11yToggled>(sw_entity).map(|t| t.0),
        Some(Toggled::False),
        "clicking the register switch toggled it ON → OFF inside the trap"
    );
}

#[test]
fn s4_delete_trigger_opens_the_delete_body() {
    // The "Delete" trigger opens the dialog in delete mode: the delete body shows
    // (the warning text is in the a11y tree) + the title becomes "Delete widget".
    let mut app = modal_form_app();
    let (_create, dialog, delete) = spawn_modal(app.world_mut());
    for _ in 0..4 {
        app.update();
    }

    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(delete);
    app.world_mut()
        .write_message(buiy_core::interaction::OnPress(delete));
    for _ in 0..4 {
        app.update();
    }
    assert!(
        is_open(app.world(), dialog),
        "the delete trigger opened the dialog"
    );

    // The visible title text reflects the delete mode (the design's `modalTitle`).
    let title = {
        let mut q = app
            .world_mut()
            .query_filtered::<&buiy_core::text::Text, With<buiy_gallery::ModalTitleField>>();
        q.iter(app.world()).next().map(|t| t.0.clone())
    };
    assert_eq!(
        title.as_deref(),
        Some(buiy_gallery::MODAL_TITLE_DELETE),
        "the delete trigger swapped the modal title to the delete-mode title"
    );

    // The delete body is now a11y-visible (its warning text is in the tree) and the
    // create body is pruned (`A11yHidden` on switch).
    let snap = snapshot(app.world_mut(), Default::default());
    assert!(
        snap.by_role(A11yRole::Text)
            .any(|n| n.name == buiy_gallery::MODAL_BODY),
        "the delete-mode warning body is in the a11y tree while the delete body shows"
    );
}

// ###########################################################################
// S5 — F-tier showcase
// ###########################################################################

// ---------------------------------------------------------------------------
// S5 function via the in-process driver — a full A11yPlugin app with ScenePlugin
// (so the C2 composites' `spawn_scene` text fields resolve) + the widget systems (the
// OnPress→toggle/expand consumers + the slider contract honor).
// ---------------------------------------------------------------------------

/// A headless app with the a11y tree, layout, focus, the widget systems, and
/// ScenePlugin (so the screen-fn's `spawn_scene` resolves). The driver reads the
/// same canonical `A11yTreeBuilder` a real AT consumes. Mirrors
/// `todomvc_c8a::todomvc_app`'s plugin set (minus the gallery app plugin — S5 is
/// pure composition).
fn showcase_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    // `ShowcasePlugin` runs the S5 controls behavior (the switch/slider/disclosure
    // visual drivers + the segmented/stepper/meter app logic). The driver function
    // tests exercise the widget contracts; the visual drivers settle the pixels.
    app.add_plugins(ShowcasePlugin);
    // The slider's APG keyboard reads `Messages<KeyboardInput>` + the toggle
    // keyboard path reads `Res<ButtonInput<KeyCode>>`. MinimalPlugins seeds neither.
    app.add_message::<KeyboardInput>();
    app.init_resource::<ButtonInput<KeyCode>>();

    spawn_showcase(app.world_mut());
    // Settle so build_tree populates the a11y tree the driver reads + the widget
    // visuals settle from their initial state.
    for _ in 0..4 {
        app.update();
    }
    app
}

#[test]
fn s5_driver_click_toggles_the_switch() {
    let mut app = showcase_app();

    // Address the switch by role+name; assert it starts off.
    let sw = get_by_role(
        app.world_mut(),
        A11yRole::Switch,
        Some(SHOWCASE_SWITCH),
        None,
    )
    .expect("the showcase switch is addressable by role+name");
    let sw_entity = buiy_core::a11y::translate::entity_for_node_id(sw).unwrap();
    assert_eq!(
        snapshot(app.world_mut(), Default::default())
            .node(sw)
            .unwrap()
            .state
            .toggled,
        Some(Toggled::False),
        "the switch starts off (read through the consumer)"
    );

    // Driver click → OnPress → the toggle consumer (one driven frame later).
    click(app.world_mut(), sw).expect("AT click on the switch honored");
    app.update();
    assert_eq!(
        app.world().get::<A11yToggled>(sw_entity).map(|t| t.0),
        Some(Toggled::True),
        "AT click → OnPress → the switch toggled off → on"
    );
    assert_eq!(
        snapshot(app.world_mut(), Default::default())
            .node(sw)
            .unwrap()
            .state
            .toggled,
        Some(Toggled::True),
        "the toggle is observed through the a11y tree"
    );
}

#[test]
fn s5_driver_increment_raises_the_slider_value_by_step() {
    let mut app = showcase_app();

    let sl = get_by_role(
        app.world_mut(),
        A11yRole::Slider,
        Some(SHOWCASE_SLIDER),
        None,
    )
    .expect("the showcase slider is addressable by role+name");
    let sl_entity = buiy_core::a11y::translate::entity_for_node_id(sl).unwrap();
    assert_eq!(
        app.world().get::<A11yValue>(sl_entity).map(|v| v.now),
        Some(SHOWCASE_SLIDER_NOW),
        "the slider starts at its authored value"
    );

    // Driver increment: the slider contract mutates A11yValue.now synchronously.
    increment(app.world_mut(), sl).expect("AT increment on the slider honored");
    assert_eq!(
        app.world().get::<A11yValue>(sl_entity).map(|v| v.now),
        Some(SHOWCASE_SLIDER_NOW + SHOWCASE_SLIDER_STEP),
        "the slider contract raised now by step (synchronous)"
    );
    app.update();
    // Observed through the a11y tree (the SemanticTree's numeric value).
    let after = snapshot(app.world_mut(), Default::default());
    let node = after.node(sl).expect("the slider is in the tree");
    assert_eq!(
        node.state.numeric_value,
        Some(SHOWCASE_SLIDER_NOW + SHOWCASE_SLIDER_STEP),
        "the raised value is observed through the a11y tree"
    );
}

#[test]
fn s5_driver_expand_opens_the_disclosure_panel() {
    let mut app = showcase_app();

    // The disclosure is role=Button (the state-keyed Expand capability layered on
    // top); address it by name among the Buttons.
    let dis = get_by_role(
        app.world_mut(),
        A11yRole::Button,
        Some(SHOWCASE_DISCLOSURE),
        None,
    )
    .expect("the showcase disclosure trigger is addressable by role+name");
    let dis_entity = buiy_core::a11y::translate::entity_for_node_id(dis).unwrap();
    assert_eq!(
        snapshot(app.world_mut(), Default::default())
            .node(dis)
            .unwrap()
            .state
            .expanded,
        Some(false),
        "the disclosure starts collapsed (read through the consumer)"
    );

    // Driver expand (the generic absolute set-verb): flips A11yExpanded true.
    expand(app.world_mut(), dis).expect("AT expand on the disclosure honored");
    app.update();
    assert_eq!(
        app.world()
            .get::<buiy_core::a11y::A11yExpanded>(dis_entity)
            .map(|e| e.0),
        Some(true),
        "the disclosure expanded (live state)"
    );
    assert_eq!(
        snapshot(app.world_mut(), Default::default())
            .node(dis)
            .unwrap()
            .state
            .expanded,
        Some(true),
        "the expanded state is observed through the a11y tree"
    );
}

#[test]
fn s5_driver_click_toggles_the_disclosure_too() {
    // Click (pointer/keyboard/AT-Click) converges on the same `A11yExpanded` flip
    // the absolute Expand verb sets — the disclosure's two activation routes.
    let mut app = showcase_app();
    let dis = get_by_role(
        app.world_mut(),
        A11yRole::Button,
        Some(SHOWCASE_DISCLOSURE),
        None,
    )
    .unwrap();
    let dis_entity = buiy_core::a11y::translate::entity_for_node_id(dis).unwrap();

    click(app.world_mut(), dis).expect("AT click on the disclosure honored");
    app.update();
    assert_eq!(
        app.world()
            .get::<buiy_core::a11y::A11yExpanded>(dis_entity)
            .map(|e| e.0),
        Some(true),
        "AT click → OnPress → advance_expanded flipped the disclosure open"
    );
}

// ---------------------------------------------------------------------------
// S5 F-tier paint — the display-list / extract tier (C6-a/b channels), headless.
// Drives the REAL `extract_buiy_nodes` via the adapterless MainWorld swap (the
// `render_focus_ring`/`render_border_shadow` idiom) over the showcase tree, then
// asserts the styled card emits a shadow + border band AND a keyboard-focused
// widget emits the focus-ring Outline band. No GPU.
// ---------------------------------------------------------------------------

use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};
use buiy_core::render::buckets::{
    pack_band_instances, pack_rounded_shadow_instances, pack_shadow_instances,
};
use buiy_core::render::components::Outline;
use buiy_core::render::extract::{ExtractedNode, ExtractedNodesView, extract_buiy_nodes};

/// Adapterless extract harness over the FULL gallery widget surface: swaps the
/// live main world into a bare render world's `MainWorld` slot, runs an
/// `ExtractSchedule` carrying the production `extract_buiy_nodes`, swaps back, and
/// reads `ExtractedNodesView`. The app carries `A11yPlugin` + `WidgetsPlugin` +
/// `FocusPlugin` (so `lower_focus_ring` runs) + ScenePlugin (so the screen-fn
/// resolves) + the render MAIN-world half (write_clip_rects/effects/forced-colors)
/// — but NO RenderApp, so no wgpu adapter is requested.
struct ShowcaseExtractHarness {
    app: App,
    render: World,
    schedule: Schedule,
}

impl ShowcaseExtractHarness {
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::scene::ScenePlugin)
            .add_plugins(buiy_core::theme::ThemePlugin)
            .add_plugins(buiy_core::CorePlugin)
            .add_plugins(A11yPlugin)
            .add_plugins(buiy_core::layout::LayoutPlugin)
            .add_plugins(BuiyTextPlugin::default())
            .add_plugins(bevy::transform::TransformPlugin)
            // FocusPlugin's `lower_focus_ring` lowers the keyboard-focus signal into
            // the framework ring `Outline` the extract reads.
            .add_plugins(FocusPlugin)
            .add_plugins(WidgetsPlugin)
            // `ShowcasePlugin` settles the S5 controls visuals from their state.
            .add_plugins(ShowcasePlugin)
            // The MAIN-world render half (clip/effects/forced-colors) registers
            // headless; its render half is guarded on a RenderApp that never exists.
            .add_plugins(buiy_core::render::BuiyRenderPlugin);
        // `handle_tab` reads `Res<ButtonInput<KeyCode>>`; MinimalPlugins has none.
        app.add_message::<KeyboardInput>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            PrimaryWindow,
        ));

        spawn_showcase(app.world_mut());

        let mut render = World::new();
        render.init_resource::<ExtractedNodesView>();
        render.init_resource::<buiy_core::render::extract::ExtractedEffectGroups>();
        render.init_resource::<MainWorld>();

        let mut schedule = Schedule::new(ExtractSchedule);
        schedule.add_systems(extract_buiy_nodes);

        Self {
            app,
            render,
            schedule,
        }
    }

    fn update(&mut self) {
        self.app.update();
    }

    fn extract(&mut self) {
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
        self.schedule.run(&mut self.render);
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
    }

    fn node_for(&self, entity: Entity) -> Option<ExtractedNode> {
        self.render
            .resource::<ExtractedNodesView>()
            .0
            .nodes
            .iter()
            .find(|n| n.entity == entity)
            .cloned()
    }

    fn shadow_count(&self) -> usize {
        // `.0` = the shadow blob; `.1` = the top-layer boundary (an additive draw
        // scalar, unused by the count helper).
        pack_shadow_instances(&self.render.resource::<ExtractedNodesView>().0.nodes)
            .0
            .len()
    }

    fn rounded_shadow_count(&self) -> usize {
        pack_rounded_shadow_instances(&self.render.resource::<ExtractedNodesView>().0.nodes)
            .0
            .len()
    }

    fn band_count(&self) -> usize {
        pack_band_instances(&self.render.resource::<ExtractedNodesView>().0.nodes)
            .0
            .len()
    }
}

/// The slider **preview square** entity (the only shadowed element on the flat
/// controls screen — the `shadow.slider-preview` glow target).
fn preview_entity(app: &mut App) -> Entity {
    let mut q = app
        .world_mut()
        .query_filtered::<Entity, With<ShowcasePreview>>();
    q.single(app.world())
        .expect("one ShowcasePreview (the slider gradient square) in the screen")
}

/// A showcase switch entity (a focusable — the focus-ring target). The design's
/// controls screen has three switches; the FIRST in spawn order is the ring target
/// (any switch would do — they share the `#[require]` Focusable contract).
fn switch_entity(app: &mut App) -> Entity {
    let mut q = app
        .world_mut()
        .query_filtered::<Entity, With<buiy_widgets::Switch>>();
    q.iter(app.world())
        .next()
        .expect("at least one Switch in the showcase")
}

fn settle(h: &mut ShowcaseExtractHarness) {
    for _ in 0..5 {
        h.update();
    }
}

#[test]
fn s5_cards_and_slider_preview_extract_border_and_shadow_bands() {
    // The design's controls screen has FLAT cards: each of the five cards emits a
    // 1px border band (radius 12, `border.default`), and the screen's box-shadows
    // are the slider preview square's `shadow.slider-preview` accent glow (HTML
    // 309/331) plus the small switch/slider THUMB drop shadows — but NOT any card
    // (the earlier placeholder F-tier card's invented 2-term elevation is gone —
    // the real cards are bordered, not shadowed). The display-list acceptance: the
    // preview's glow + the per-card border bands ARE in the extract output.
    let mut h = ShowcaseExtractHarness::new();
    settle(&mut h);
    let preview = preview_entity(&mut h.app);

    // Sanity: the shared paint sources spell the real design channels.
    assert_eq!(
        showcase_preview_shadow().0.len(),
        1,
        "the preview authors a single box-shadow term (the slider-preview glow)"
    );
    let _ = showcase_card_border(); // a flat 1px card border (asserted via extract).

    h.extract();

    // The slider preview square extracts its single accent-glow shadow term.
    let preview_node = h
        .node_for(preview)
        .expect("the slider preview square reaches the display list");
    assert_eq!(
        preview_node.shadows.len(),
        1,
        "the slider preview extracts one box-shadow term (the accent glow)"
    );
    assert!(
        preview_node.shadows.iter().all(|s| s.sigma > 0.0),
        "the preview shadow term carries a positive blur sigma"
    );

    // The cards emit per-side border bands: at least five (one per card) reach the
    // packer, each resolving its `border.default` color (not the magenta miss).
    assert!(
        h.band_count() >= 5,
        "at least five border band instances (one per controls card), got {}",
        h.band_count()
    );
    // The screen is lightly shadowed: the preview glow + the switch/slider thumb
    // drop shadows (the three switch thumbs + the slider thumb), but NO card. The
    // exact total is the design's five (1 preview + 3 switch thumbs + 1 slider
    // thumb). All five casters are ROUNDED (the preview's radius-14 square, the
    // circular thumbs), so since F4b-6 they route to the distinct ROUNDED-shadow
    // pipeline — the SQUARE pack is empty and the design's five all land rounded.
    // Assert the TOTAL across both pipelines is the design's five (robust to the
    // per-caster split) and that the rounded pipeline carries them.
    assert_eq!(
        h.shadow_count() + h.rounded_shadow_count(),
        5,
        "five shadow instances total (preview glow + switch/slider thumb shadows), \
         got square={} + rounded={}",
        h.shadow_count(),
        h.rounded_shadow_count()
    );
    assert_eq!(
        h.rounded_shadow_count(),
        5,
        "all five shadows are on ROUNDED casters → the rounded-shadow pipeline (F4b-6)"
    );
}

#[test]
fn s5_keyboard_focused_widget_extracts_the_focus_ring_outline() {
    // A keyboard-focused widget emits the C6-a focus-ring `Outline` band (the
    // audit's WCAG 2.4.7 "focus structurally invisible" fix, proven WITHOUT a GPU).
    let mut h = ShowcaseExtractHarness::new();
    settle(&mut h);
    let switch = switch_entity(&mut h.app);

    // Keyboard-focus the switch: set the FocusedEntity + FocusVisible(true) pair
    // (the net effect of a Tab) so `lower_focus_ring` inserts the framework ring.
    h.app.world_mut().resource_mut::<FocusedEntity>().0 = Some(switch);
    h.app.world_mut().resource_mut::<FocusVisible>().0 = true;
    h.update();
    h.update();

    // The framework ring `Outline` is on the focused widget (the component the
    // lowering owns).
    assert!(
        h.app.world().get::<Outline>(switch).is_some(),
        "lower_focus_ring inserted the framework Outline on the keyboard-focused widget"
    );

    h.extract();
    let node = h
        .node_for(switch)
        .expect("the focused switch reaches the display list");
    let outline = node
        .outline
        .expect("a keyboard-focused widget extracts an Outline band (the C6-a focus ring)");
    assert!(
        outline.width >= 2.0,
        "WCAG 2.4.11: the focus ring is >= 2px"
    );
}

#[test]
fn s5_pointer_focused_widget_gets_no_focus_ring() {
    // The `:focus-visible` discipline: a pointer-focused (FocusVisible=false)
    // widget gets NO ring — only keyboard focus shows it (the regression guard the
    // C6-a path enforces, mirrored here on the composed showcase).
    let mut h = ShowcaseExtractHarness::new();
    settle(&mut h);
    let switch = switch_entity(&mut h.app);

    h.app.world_mut().resource_mut::<FocusedEntity>().0 = Some(switch);
    h.app.world_mut().resource_mut::<FocusVisible>().0 = false;
    h.update();

    assert!(
        h.app.world().get::<Outline>(switch).is_none(),
        "pointer focus (FocusVisible=false) shows no focus ring"
    );
    h.extract();
    let node = h.node_for(switch).expect("switch in the display list");
    assert!(
        node.outline.is_none(),
        "no Outline band for a pointer-focused (not focus-visible) widget"
    );
}

// ---------------------------------------------------------------------------
// S5 GPU programmatic readback (the additive `#[ignore]` GPU lane) — paints the
// design's slider preview square (the gradient-filled, accent-glow-shadowed box —
// the same `shadow.slider-preview` channel + a rounded border the showcase
// authors, spelled via the shared `showcase_preview_shadow()` source) to an
// offscreen texture on a real adapter through the canonical
// `DeterministicApp::capture` path, and asserts non-canvas pixels are present.
// Adapter-TOLERANT: it asserts the fixture painted SOMETHING beyond the black
// canvas (the gradient fill + the glow blur rasterized), not an RX-XT-exact pixel
// golden (CI is pinned lavapipe). Run on a GPU host:
//   `cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1`
//
// The capture stack (`capture_app_scaled`) is paint-only (no ScenePlugin /
// WidgetsPlugin), so the fixture spawns the preview square with EXPLICIT components
// (the gradient + the `showcase_preview_shadow()` source) rather than the
// `spawn_showcase` builder — the same shape the display-list tier asserts on the
// preview, now rasterized on the adapter.
// ---------------------------------------------------------------------------

/// Spawn the design's slider preview square into the capture app: a 150deg accent
/// gradient fill + the `shadow.slider-preview` accent glow + a rounded border. The
/// gradient + the shadow are render components from the `buiy_gallery` source so the
/// GPU residue matches the screen's preview.
#[cfg(test)]
fn showcase_preview_fixture(app: &mut App) {
    use buiy_core::components::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::ColorToken;
    use buiy_core::render::components::{
        BackgroundLayer, BackgroundLayers, Border, ColorStop, Corners, LinearGradient, Radius,
    };

    let preview = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(16.0)),
                    left: Sizing::Length(Length::px(16.0)),
                    ..default()
                })
                .width_px(88.0)
                .height_px(88.0),
            // The 150deg accent gradient (`gradient.accent-150`).
            BackgroundLayers(vec![BackgroundLayer::Linear(LinearGradient {
                angle_deg: 150.0,
                stops: vec![
                    ColorStop {
                        color: ColorToken::Accent,
                        position: 0.0,
                    },
                    ColorStop {
                        color: ColorToken::AccentLighter,
                        position: 1.0,
                    },
                ],
            })]),
            Border {
                radius: Corners::all(Radius::circular(14.0)),
                ..Default::default()
            },
            // The shared preview-shadow source (the slider-preview accent glow).
            showcase_preview_shadow(),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[preview]);
}

#[test]
#[ignore = "GPU lane: needs a real wgpu adapter (offscreen render-to-texture + readback)"]
fn s5_slider_preview_paints_gradient_and_glow_on_a_real_adapter() {
    use buiy_verify::determinism::DeterministicApp;

    // Capture the preview square to an offscreen RGBA image on the real adapter (the
    // canonical deterministic capture path the GPU goldens use).
    let image = DeterministicApp::new(160, 120).capture(showcase_preview_fixture);

    // Adapter-tolerant: the fixture painted beyond the black canvas — the gradient
    // fill + the accent-glow blur both rasterized. We assert non-clear pixels exist
    // (the `goldens.rs` non-vacuous discipline), NOT an exact golden (lavapipe vs
    // RX-XT differ on the AA rim / blur kernel).
    let painted = image.pixels().filter(|p| p.0 != [0, 0, 0, 255]).count();
    assert!(
        painted > 100,
        "the slider preview painted a meaningful number of non-canvas pixels on the adapter \
         (the gradient fill + the accent-glow blur), got {painted}"
    );
}
