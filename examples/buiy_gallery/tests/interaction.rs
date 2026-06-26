//! **Live-interaction gate** — the tier the gallery's prior tests were missing.
//!
//! Every other gallery test either injects widget state directly, drives the
//! AccessKit a11y tree, exercises a one-widget minimal app, or asserts a static
//! layout snapshot / GPU golden. **None composed the REAL shell and drove a REAL
//! pointer click through the REAL picking pipeline** — so the
//! "hidden-top-layer-modal-absorbs-every-click" bug
//! (`docs/reports/2026-06-26-gallery-interactivity-rootcause.md`) was invisible to
//! the whole suite: the app rendered all five screens yet nothing was clickable.
//!
//! This harness closes that gap. It composes the SAME plugin set the binary boots
//! ([`BuiyPlugin`] — which pulls the real `bevy_picking` + Buiy picking backend +
//! the `write_paint_skip` render-prep pass — plus every screen plugin) over a
//! synthetic headless window + camera, lays everything out, lets `write_paint_skip`
//! stamp [`ComputedPaintSkip`] on the closed top-layer modal, and then injects
//! synthetic [`PointerInput`] (move → press → release) through the live backend to
//! `Pointer<Click>` → `OnPress`, exactly the path a real cursor takes. Each test
//! asserts the OBSERVABLE app-state change a real user would see.
//!
//! ## Faithfulness (the closed modal MUST be a paint-skipped occluder)
//!
//! The bug's mechanism is that the S4 dialog is a **detached, full-window,
//! top-layer** `Dialog` (`CssVisibility::Hidden` at rest) that the screen router
//! never `Display::None`s (it lives outside `#ScreenContent`), so it is present and
//! topmost on EVERY screen. For the fix to be exercised, this harness reproduces
//! that exactly: `BuiyPlugin`'s `write_paint_skip` runs in the main world (no
//! RenderApp needed) and marks the hidden dialog `ComputedPaintSkip`, and the
//! backend skips it. The [`nav_clicks_switch_each_screen`] test (and the others)
//! FAIL if the `emit_picks` paint-skip is reverted — that revert/restore check was
//! run while authoring this tier (see the report).

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::component::Component;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::math::Vec2;
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::{Entity, With};
use bevy::scene::ScenePlugin;
use bevy::transform::components::GlobalTransform;
use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};

use buiy::BuiyPlugin;
use buiy_core::ResolvedLayout;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::a11y::{A11yExpanded, A11yToggled, A11yValue, Toggled, set_value};
use buiy_core::focus::FocusedEntity;
use buiy_core::layout::Display;
use buiy_core::render::components::CssVisibility;
use buiy_core::text::Text;
use buiy_core::theme::{Theme, default_dark_theme};

use buiy_gallery::composites::{SegmentedOption, StepperButton, ToastPlugin};
use buiy_gallery::inspector::{AccentSwatch, InspectorPlugin};
use buiy_gallery::shell::{Screen, ScreenNav, ScreenRoot, ScreenRouter, ScreenRouterPlugin};
use buiy_gallery::{
    AddField, ClearCompleted, Filter, FilterButton, FilterMode, ModalDialog, ModalInvoker,
    ModalMode, ModalPlugin, OverlayMenuPlugin, RowCheckbox, ScrollListPlugin, ShowcaseDiscBody,
    ShowcasePlugin, ShowcasePreview, ShowcaseStepper, TodoMvcPlugin, TodoRow,
};
use buiy_widgets::dialog::DialogClose;
use buiy_widgets::{Disclosure, Menu, MenuButton, Slider, Switch};

// ===========================================================================
// The harness — the REAL shell + the REAL picking stack, headless.
// ===========================================================================

/// A composed gallery app: `BuiyPlugin` (the real layout + picking +
/// `write_paint_skip`) + every screen plugin + a synthetic primary window the
/// shell's `Camera2d` (spawned by `ScreenRouterPlugin`'s `setup_shell` startup)
/// resolves against. `window` is the pointer-target window.
struct Gallery {
    app: App,
    window: Entity,
    /// The synthetic mouse pointer entity (`PointerId::Mouse` + its required
    /// `PointerLocation`/`PointerPress`). `bevy_picking`'s `receive` system updates
    /// these from the `PointerInput` we inject; `emit_picks` reads the location.
    _pointer: Entity,
}

impl Gallery {
    /// Build the composed shell and settle it (Startup builds the tree + mounts
    /// the 5 screens; the extra frames let layout, the transform bridge,
    /// `write_paint_skip`, and the a11y tree reach steady state).
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            // `spawn_scene` (the widget scene-fns the screens use) needs the asset +
            // scene infrastructure; `BuiyPlugin` is added AFTER them (the documented
            // order). No `WindowPlugin` ⇒ `BuiyPlugin` does NOT add the winit cursor
            // reader, so our injected `PointerInput` is the only pointer source.
            .add_plugins(AssetPlugin::default())
            .add_plugins(ScenePlugin)
            // `BuiyPlugin` requires `InputPlugin` (focus/keymap read
            // `ButtonInput<KeyCode>`); it also registers the `KeyboardInput` message
            // the editor-submit / slider-keyboard paths read.
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(BuiyPlugin);

        // The design's dark theme (the binary opts in; the framework default is
        // light). The accent-swap test reads back `Theme`'s resolved accent.
        app.insert_resource(default_dark_theme());

        // The real screen wiring the binary boots. `ScreenRouterPlugin`'s Startup
        // `setup_shell` spawns the `Camera2d`, builds the shell, and mounts all 5
        // screens under `#ScreenContent`.
        app.add_plugins(ScreenRouterPlugin)
            .add_plugins(InspectorPlugin)
            .add_plugins(TodoMvcPlugin)
            .add_plugins(ScrollListPlugin)
            .add_plugins(OverlayMenuPlugin)
            .add_plugins(ModalPlugin)
            .add_plugins(ShowcasePlugin)
            .add_plugins(ToastPlugin);

        // A synthetic primary window — `emit_picks` resolves the camera's default
        // `RenderTarget::Window(Primary)` against it (button.rs's headless pattern).
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: WindowResolution::new(1280, 800),
                    ..Default::default()
                },
                PrimaryWindow,
            ))
            .id();

        // The mouse pointer (no `PointerInputPlugin` to spawn one for us). Its
        // `#[require]` adds `PointerLocation`/`PointerPress`.
        let pointer = app.world_mut().spawn(PointerId::Mouse).id();

        let mut g = Self {
            app,
            window,
            _pointer: pointer,
        };
        g.settle(10);
        g
    }

    fn world_app(&mut self) -> &mut App {
        &mut self.app
    }

    /// Pump `n` frames.
    fn settle(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
        }
    }

    /// A pointer `Location` over our window at `pos` (window-space pixels).
    fn loc(&self, pos: Vec2) -> Location {
        let target = WindowRef::Entity(self.window)
            .normalize(Some(self.window))
            .expect("normalize window ref");
        Location {
            target: NormalizedRenderTarget::Window(target),
            position: pos,
        }
    }

    /// The absolute center of `entity`'s laid-out box (the basis `emit_picks`
    /// hit-tests): `GlobalTransform.translation` is the box top-left, `+ size/2`.
    fn center(&mut self, entity: Entity) -> Vec2 {
        let size = self
            .app
            .world()
            .get::<ResolvedLayout>(entity)
            .unwrap_or_else(|| panic!("entity {entity:?} has no ResolvedLayout (not laid out)"))
            .size;
        let gt = *self
            .app
            .world()
            .get::<GlobalTransform>(entity)
            .unwrap_or_else(|| panic!("entity {entity:?} has no GlobalTransform"));
        gt.translation().truncate() + size / 2.0
    }

    /// Inject one `PointerInput`.
    fn pointer_input(&mut self, pos: Vec2, action: PointerAction) {
        let location = self.loc(pos);
        self.app.world_mut().write_message(PointerInput {
            pointer_id: PointerId::Mouse,
            location,
            action,
        });
    }

    /// Drive a full primary click at `pos` through the live backend: move (so
    /// `emit_picks` builds the hovermap + the hover stage registers `Over`), then
    /// press + release (→ `Pointer<Click>` → the C3b `OnPress` producer). Each
    /// action gets a frame so the picking → interaction → app-logic chain runs.
    fn click_at(&mut self, pos: Vec2) {
        self.pointer_input(pos, PointerAction::Move { delta: Vec2::ZERO });
        self.app.update();
        self.pointer_input(pos, PointerAction::Press(PointerButton::Primary));
        self.app.update();
        self.pointer_input(pos, PointerAction::Release(PointerButton::Primary));
        self.app.update();
        // One settle frame: a few app appliers chain after the same-frame `OnPress`
        // read (e.g. the router + inspector rebuild), and the `.before(A11yUpdate)`
        // window lands the a11y rebuild — settle so observers see the committed tree.
        self.app.update();
    }

    /// Click an entity at its laid-out center.
    fn click(&mut self, entity: Entity) {
        let c = self.center(entity);
        self.click_at(c);
    }

    /// Move focus to `entity` (the keyboard precondition the slider / editor read).
    /// The same `FocusedEntity` a Tab/click focus produces; set directly so the
    /// keyboard-input path is the thing under test.
    fn focus(&mut self, entity: Entity) {
        self.app.world_mut().resource_mut::<FocusedEntity>().0 = Some(entity);
        self.app.update();
    }

    /// Send one `key_code` keypress (Pressed) at the harness window — read by the
    /// focused editor (`apply_keyboard_edits`) and the slider keyboard router.
    fn press_key(&mut self, key_code: KeyCode, logical: Key) {
        self.app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: logical,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: self.window,
        });
        self.app.update();
    }

    /// Switch the active screen via the router message (the same `SwitchScreen` a
    /// nav press writes). Settles so the target screen lays out + is pickable.
    fn goto(&mut self, screen: Screen) {
        self.app
            .world_mut()
            .write_message(buiy_gallery::shell::SwitchScreen(screen));
        self.settle(4);
    }
}

// --- entity-lookup helpers --------------------------------------------------

/// The single entity carrying marker `C` (panics if zero / many — used where the
/// shell has exactly one, e.g. `AddField`, `ClearCompleted`, `ModalDialog`).
fn single<C: Component>(app: &mut App) -> Entity {
    let mut q = app.world_mut().query_filtered::<Entity, With<C>>();
    let v: Vec<Entity> = q.iter(app.world()).collect();
    assert_eq!(v.len(), 1, "expected exactly one entity with the marker");
    v[0]
}

/// Every entity carrying marker `C`.
fn all<C: Component>(app: &mut App) -> Vec<Entity> {
    let mut q = app.world_mut().query_filtered::<Entity, With<C>>();
    q.iter(app.world()).collect()
}

/// The first entity whose marker `C` satisfies `pred`.
fn find_where<C: Component>(app: &mut App, pred: impl Fn(&C) -> bool) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &C)>();
    q.iter(app.world())
        .find(|(_, c)| pred(c))
        .map(|(e, _)| e)
        .expect("an entity matching the predicate")
}

/// The `ScreenRoot(screen)` root entity.
fn screen_root(app: &mut App, screen: Screen) -> Entity {
    find_where::<ScreenRoot>(app, |r| r.0 == screen)
}

/// Every descendant of `root` (inclusive) carrying marker `C`. Used to SCOPE a
/// shared-marker lookup (e.g. `Switch` / `SegmentedOption` appear on both the
/// showcase AND inside the always-present, full-window hidden modal dialog) to one
/// screen's subtree — an unscoped `all::<Switch>()` would grab the hidden dialog's
/// register switch, which is laid out (the dialog keeps its box) but paint-skipped.
fn in_screen<C: Component>(app: &mut App, screen: Screen) -> Vec<Entity> {
    let root = screen_root(app, screen);
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if app.world().get::<C>(e).is_some() {
            out.push(e);
        }
        if let Some(children) = app.world().get::<bevy::prelude::Children>(e) {
            stack.extend(children.iter());
        }
    }
    out
}

/// The `Screen` of each `ScreenRoot` plus whether it is `Display::None` (the
/// router's hide). Sorted by screen debug name for stable comparison.
fn screen_visibility(app: &mut App) -> Vec<(Screen, bool)> {
    let mut q = app.world_mut().query::<(&ScreenRoot, Option<&Display>)>();
    let mut v: Vec<(Screen, bool)> = q
        .iter(app.world())
        .map(|(r, d)| (r.0, matches!(d, Some(Display::None))))
        .collect();
    v.sort_by_key(|(s, _)| format!("{s:?}"));
    v
}

// ===========================================================================
// 1. Nav rail — clicking each rail button switches the viewport screen.
//
// This is the canonical faithfulness test: the closed S4 dialog is a full-window
// top-layer overlay present on EVERY screen. Without the `emit_picks` paint-skip,
// it absorbs this nav click (the original bug) and the router never moves.
// ===========================================================================

#[test]
fn nav_clicks_switch_each_screen() {
    let mut g = Gallery::new();
    assert_eq!(
        g.world_app().world().resource::<ScreenRouter>().0,
        Screen::Todo,
        "boots on the default Todo screen",
    );

    // Click each of the other four nav buttons in turn and assert the router +
    // the screen-root visibility follow. (Todo is the boot screen; visiting the
    // rest proves every rail button is live.)
    for target in [
        Screen::Scroll,
        Screen::Menu,
        Screen::Modal,
        Screen::Showcase,
        Screen::Todo,
    ] {
        let button = find_where::<ScreenNav>(g.world_app(), |n| n.0 == target);
        g.click(button);
        g.settle(2);

        assert_eq!(
            g.world_app().world().resource::<ScreenRouter>().0,
            target,
            "clicking the {target:?} nav button switches the router to it",
        );
        // Exactly the active screen is laid out; the rest are Display::None.
        for (screen, is_none) in screen_visibility(g.world_app()) {
            if screen == target {
                assert!(!is_none, "the active screen {screen:?} must be laid out");
            } else {
                assert!(
                    is_none,
                    "the inactive screen {screen:?} must be Display::None"
                );
            }
        }
    }
}

/// The rail active-state reflect follows the click: after switching, the target
/// nav button's `Background` is the active `surface.card` token (not transparent).
#[test]
fn nav_click_reflects_rail_active_state() {
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;

    let mut g = Gallery::new();
    let button = find_where::<ScreenNav>(g.world_app(), |n| n.0 == Screen::Modal);
    g.click(button);
    g.settle(2);

    let bg = g
        .world_app()
        .world()
        .get::<Background>(button)
        .expect("nav button has a Background");
    assert!(
        matches!(&bg.color, ColorToken::Token(t) if t.as_ref() == "color.surface.card"),
        "the clicked nav button takes the active card bg, got {:?}",
        bg.color,
    );
}

// ===========================================================================
// 2. Accent swatch — clicking re-themes the whole app (Theme accent token).
// ===========================================================================

#[test]
fn accent_swatch_click_retones_the_theme_accent() {
    let mut g = Gallery::new();

    let green = bevy::color::Color::srgb_u8(0x45, 0xc0, 0x7d);
    // The boot accent is blue, so the green swatch is a real change.
    let before = g
        .world_app()
        .world()
        .resource::<Theme>()
        .color("color.accent")
        .expect("theme resolves an accent");
    assert_ne!(srgb_u8(before), srgb_u8(green), "boot accent is not green");

    let swatch = find_where::<AccentSwatch>(g.world_app(), |s| srgb_u8(s.0) == srgb_u8(green));
    g.click(swatch);
    g.settle(2);

    let after = g
        .world_app()
        .world()
        .resource::<Theme>()
        .color("color.accent")
        .expect("theme resolves an accent");
    assert_eq!(
        srgb_u8(after),
        srgb_u8(green),
        "clicking the Green swatch re-themes the app accent to green (SetAccent → apply_set_accent)",
    );
}

/// srgb u8 triple of a color (the comparison the inspector itself uses).
fn srgb_u8(c: bevy::color::Color) -> (u8, u8, u8) {
    let s = bevy::color::Srgba::from(c);
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    (q(s.red), q(s.green), q(s.blue))
}

// ===========================================================================
// 3. TodoMVC (the boot screen).
// ===========================================================================

#[test]
fn todo_checkbox_click_toggles_completion() {
    let mut g = Gallery::new();

    // The three demo rows seed [done, active, active]; clicking the first row's
    // checkbox (done) flips it to not-done.
    let checkboxes = all::<RowCheckbox>(g.world_app());
    assert_eq!(checkboxes.len(), 3, "three seeded todo rows");

    // Pick the checkbox seeded `Toggled::True` (the completed demo row).
    let done = checkboxes
        .into_iter()
        .find(|&cb| {
            g.world_app().world().get::<A11yToggled>(cb).map(|t| t.0) == Some(Toggled::True)
        })
        .expect("one demo row starts completed");

    g.click(done);
    g.settle(2);

    assert_eq!(
        g.world_app().world().get::<A11yToggled>(done).map(|t| t.0),
        Some(Toggled::False),
        "clicking a completed checkbox toggles it off (the Checkbox widget's A11yToggled)",
    );
}

#[test]
fn todo_filter_pill_click_sets_the_active_filter() {
    let mut g = Gallery::new();
    assert_eq!(
        g.world_app().world().resource::<Filter>().0,
        FilterMode::All,
        "boots with the All filter",
    );

    let active_pill = find_where::<FilterButton>(g.world_app(), |f| f.0 == FilterMode::Active);
    g.click(active_pill);
    g.settle(2);

    assert_eq!(
        g.world_app().world().resource::<Filter>().0,
        FilterMode::Active,
        "clicking the Active pill sets the filter (collect_button_press → Filter)",
    );
    // apply_filter hides the completed demo row (Display::None) under Active.
    let hidden = all::<TodoRow>(g.world_app())
        .into_iter()
        .filter(|&r| matches!(g.world_app().world().get::<Display>(r), Some(Display::None)))
        .count();
    assert_eq!(
        hidden, 1,
        "the one completed row is filtered out under Active"
    );
}

#[test]
fn todo_clear_done_click_removes_completed_rows() {
    let mut g = Gallery::new();
    let before = all::<TodoRow>(g.world_app()).len();
    assert_eq!(before, 3, "three demo rows");

    let clear = single::<ClearCompleted>(g.world_app());
    g.click(clear);
    g.settle(2);

    let after = all::<TodoRow>(g.world_app()).len();
    assert_eq!(
        after, 2,
        "clicking Clear done despawns the one completed demo row (3 → 2)",
    );
}

#[test]
fn todo_add_via_field_focus_type_and_enter_appends_a_row() {
    let mut g = Gallery::new();
    let before = all::<TodoRow>(g.world_app()).len();

    // Click the draft field to focus it (the real picking → focus path), then put
    // text in via the a11y driver channel (the same `set_value` a screen reader
    // drives — it mutates `TextEditState`), then press Enter on the focused field
    // to fire `EditSubmitted` → `collect_add_submit` → `append_row`.
    let field = single::<AddField>(g.world_app());
    g.click(field);
    assert_eq!(
        g.world_app().world().resource::<FocusedEntity>().0,
        Some(field),
        "clicking the draft field focuses it",
    );
    set_value(
        g.world_app().world_mut(),
        node_id_for(field),
        "Wire the harness",
    )
    .expect("a11y set_value honored");
    g.settle(2);
    g.press_key(KeyCode::Enter, Key::Enter);
    g.settle(2);

    let after = all::<TodoRow>(g.world_app());
    assert_eq!(
        after.len(),
        before + 1,
        "submitting the draft field appends a new todo row",
    );
    // The new row carries the typed label.
    let has_label = after
        .iter()
        .any(|&row| row_has_label(&mut g, row, "Wire the harness"));
    assert!(has_label, "the appended row shows the typed text");
}

/// Whether any `Text` descendant of `row` equals `want` (the row label leaf).
fn row_has_label(g: &mut Gallery, row: Entity, want: &str) -> bool {
    // Walk the row's full subtree looking for the visible label text.
    let mut stack = vec![row];
    while let Some(e) = stack.pop() {
        if g.world_app().world().get::<Text>(e).map(|t| t.0.as_str()) == Some(want) {
            return true;
        }
        if let Some(children) = g.world_app().world().get::<bevy::prelude::Children>(e) {
            stack.extend(children.iter());
        }
    }
    false
}

// ===========================================================================
// 4. Overlay menu (S3).
// ===========================================================================

#[test]
fn menu_button_click_opens_then_keyboard_activates_an_item() {
    use buiy_gallery::MenuActivations;

    let mut g = Gallery::new();
    g.goto(Screen::Menu);

    // Closed at rest.
    let button = single::<MenuButton>(g.world_app());
    let menu = single::<Menu>(g.world_app());
    assert_eq!(
        g.world_app()
            .world()
            .get::<A11yExpanded>(button)
            .map(|e| e.0),
        Some(false),
        "the menu button starts collapsed",
    );

    // Click the ⋮ button → A11yExpanded(true) → sync_menu_open shows the dropdown.
    g.click(button);
    g.settle(2);
    assert_eq!(
        g.world_app()
            .world()
            .get::<A11yExpanded>(button)
            .map(|e| e.0),
        Some(true),
        "clicking the menu button opens it (A11yExpanded true)",
    );
    assert_eq!(
        g.world_app().world().get::<CssVisibility>(menu).copied(),
        Some(CssVisibility::Visible),
        "the dropdown becomes visible when opened",
    );

    // Activate the active item via the keyboard (the design's roving-focus model:
    // open → the menu container holds focus with the first item as
    // `active_descendant` → Enter activates it). A *pointer* click on a `MenuItem`
    // does NOT activate it — `MenuItem` is role `MenuItem`, which is not in
    // `is_activatable_role`, and per-item pointer activation is an explicitly
    // deferred handler (`buiy_widgets/src/menu.rs` `guard_menu_clicks` — "a future
    // per-item pointer handler"). Enter on the open+focused menu writes the shared
    // `OnPress` for the active (first) item → `record_menu_activation`.
    g.press_key(KeyCode::Enter, Key::Enter);
    g.settle(2);
    assert_eq!(
        g.world_app()
            .world()
            .resource::<MenuActivations>()
            .0
            .last()
            .map(String::as_str),
        Some("Open"),
        "activating the first item records its label (record_menu_activation)",
    );
}

#[test]
fn menu_outside_click_light_dismisses_the_open_menu() {
    let mut g = Gallery::new();
    g.goto(Screen::Menu);

    let button = single::<MenuButton>(g.world_app());
    g.click(button);
    g.settle(2);
    assert_eq!(
        g.world_app()
            .world()
            .get::<A11yExpanded>(button)
            .map(|e| e.0),
        Some(true),
        "menu open after the button click",
    );

    // Click far away (the canvas backdrop) — the light-dismiss scrim closes it.
    g.click_at(Vec2::new(640.0, 700.0));
    g.settle(2);
    assert_eq!(
        g.world_app()
            .world()
            .get::<A11yExpanded>(button)
            .map(|e| e.0),
        Some(false),
        "an outside click light-dismisses the menu (A11yExpanded back to false)",
    );
}

// ===========================================================================
// 5. Modal (S4) — open traps focus; Esc / DialogClose closes + restores.
// ===========================================================================

#[test]
fn modal_invoker_click_opens_and_traps_focus_then_esc_closes_and_restores() {
    let mut g = Gallery::new();
    g.goto(Screen::Modal);

    let dialog = single::<ModalDialog>(g.world_app());
    assert_eq!(
        g.world_app().world().get::<CssVisibility>(dialog).copied(),
        Some(CssVisibility::Hidden),
        "the dialog is closed at rest",
    );

    // Click the "New widget" create invoker → open_dialog_on_invoker_press shows
    // the dialog + moves focus inside it.
    let invoker = find_where::<ModalInvoker>(g.world_app(), |m| m.0 == ModalMode::Create);
    g.click(invoker);
    g.settle(2);

    assert_eq!(
        g.world_app().world().get::<CssVisibility>(dialog).copied(),
        Some(CssVisibility::Visible),
        "clicking the invoker opens the dialog",
    );
    let focused = g.world_app().world().resource::<FocusedEntity>().0;
    assert!(
        focused.is_some_and(|f| is_descendant(g.world_app(), f, dialog)),
        "focus moves INSIDE the dialog (the focus trap), got {focused:?}",
    );

    // Escape closes it and restores focus to the invoker.
    g.press_key(KeyCode::Escape, Key::Escape);
    g.settle(2);
    assert_eq!(
        g.world_app().world().get::<CssVisibility>(dialog).copied(),
        Some(CssVisibility::Hidden),
        "Escape closes the dialog",
    );
    assert_eq!(
        g.world_app().world().resource::<FocusedEntity>().0,
        Some(invoker),
        "Escape restores focus to the invoker (FocusReturn)",
    );
}

#[test]
fn modal_dialog_close_button_click_closes_the_open_dialog() {
    let mut g = Gallery::new();
    g.goto(Screen::Modal);

    let dialog = single::<ModalDialog>(g.world_app());
    let invoker = find_where::<ModalInvoker>(g.world_app(), |m| m.0 == ModalMode::Create);
    g.click(invoker);
    g.settle(2);
    assert_eq!(
        g.world_app().world().get::<CssVisibility>(dialog).copied(),
        Some(CssVisibility::Visible),
        "dialog open",
    );

    // The modal has 3 `DialogClose`s (header ✕, Cancel, the confirm button); pick a
    // pure-close one (NOT the `ModalConfirm`, which also mutates the registry) — both
    // the header ✕ and Cancel just close. Take the first such, inside the open dialog.
    let close = {
        let mut q = g.world_app().world_mut().query_filtered::<Entity, (
            With<DialogClose>,
            bevy::prelude::Without<buiy_gallery::ModalConfirm>,
        )>();
        q.iter(g.world_app().world())
            .next()
            .expect("a pure-close DialogClose (header ✕ / Cancel)")
    };
    g.click(close);
    g.settle(2);
    assert_eq!(
        g.world_app().world().get::<CssVisibility>(dialog).copied(),
        Some(CssVisibility::Hidden),
        "clicking the dialog ✕ closes it",
    );
}

/// Whether `e` is `ancestor` or a descendant of it (the focus-trap containment).
fn is_descendant(app: &mut App, e: Entity, ancestor: Entity) -> bool {
    let mut cur = e;
    for _ in 0..32 {
        if cur == ancestor {
            return true;
        }
        match app.world().get::<bevy::prelude::ChildOf>(cur) {
            Some(c) => cur = c.parent(),
            None => return false,
        }
    }
    false
}

// ===========================================================================
// 6. Controls showcase (S5).
// ===========================================================================

#[test]
fn showcase_switch_click_toggles_it() {
    let mut g = Gallery::new();
    g.goto(Screen::Showcase);

    // The first showcase Switch.
    // Scope to the showcase screen — `Switch` also lives inside the always-present
    // hidden modal dialog (`#ModalRegisterSwitch`), which an unscoped query grabs.
    let switch = in_screen::<Switch>(g.world_app(), Screen::Showcase)
        .into_iter()
        .next()
        .expect("a showcase switch");
    let before = g
        .world_app()
        .world()
        .get::<A11yToggled>(switch)
        .map(|t| t.0);

    g.click(switch);
    g.settle(2);

    let after = g
        .world_app()
        .world()
        .get::<A11yToggled>(switch)
        .map(|t| t.0);
    assert_ne!(
        before, after,
        "clicking the switch flips its A11yToggled state"
    );
}

#[test]
fn showcase_segmented_click_selects_the_option() {
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;

    let mut g = Gallery::new();
    g.goto(Screen::Showcase);

    // Pick a segmented option that is NOT currently the accent-selected one, click
    // it, and assert it becomes the accent-filled selection. Scope to the showcase
    // screen — the hidden modal dialog also has a `SegmentedOption` (Kind) track.
    let options = in_screen::<SegmentedOption>(g.world_app(), Screen::Showcase);
    assert!(!options.is_empty(), "showcase has segmented options");
    let unselected = options
        .into_iter()
        .find(|&o| {
            !matches!(
                g.world_app().world().get::<Background>(o),
                Some(Background { color: ColorToken::Token(t) }) if t.as_ref() == "color.accent"
            )
        })
        .expect("an unselected segmented option");

    g.click(unselected);
    g.settle(2);

    let bg = g.world_app().world().get::<Background>(unselected).cloned();
    assert!(
        matches!(&bg, Some(Background { color: ColorToken::Token(t) }) if t.as_ref() == "color.accent"),
        "clicking a segmented option makes it the accent-filled selection, got {bg:?}",
    );
}

#[test]
fn showcase_stepper_plus_and_minus_clicks_change_the_count() {
    let mut g = Gallery::new();
    g.goto(Screen::Showcase);

    let stepper = single::<ShowcaseStepper>(g.world_app());
    let start = g
        .world_app()
        .world()
        .get::<ShowcaseStepper>(stepper)
        .unwrap()
        .count;

    let plus = find_where::<StepperButton>(g.world_app(), |b| *b == StepperButton::Increment);
    g.click(plus);
    g.settle(2);
    assert_eq!(
        g.world_app()
            .world()
            .get::<ShowcaseStepper>(stepper)
            .unwrap()
            .count,
        start + 1,
        "clicking + increments the stepper count",
    );

    let minus = find_where::<StepperButton>(g.world_app(), |b| *b == StepperButton::Decrement);
    g.click(minus);
    g.settle(2);
    assert_eq!(
        g.world_app()
            .world()
            .get::<ShowcaseStepper>(stepper)
            .unwrap()
            .count,
        start,
        "clicking − decrements it back",
    );
}

#[test]
fn showcase_slider_keyboard_adjust_raises_the_value_and_preview() {
    use buiy_core::layout::Length;
    use buiy_core::render::components::Border;

    let mut g = Gallery::new();
    g.goto(Screen::Showcase);

    let slider = single::<Slider>(g.world_app());
    let before = g.world_app().world().get::<A11yValue>(slider).unwrap().now;

    // Focus the slider (the Tab/click precondition) and press ArrowRight — the
    // a11y keyboard router lowers it to Increment, raising A11yValue by one step.
    g.focus(slider);
    g.press_key(KeyCode::ArrowRight, Key::ArrowRight);
    g.settle(2);

    let after = g.world_app().world().get::<A11yValue>(slider).unwrap().now;
    assert!(
        after > before,
        "ArrowRight on the focused slider raises its value ({before} → {after})",
    );

    // The showcase preview square's corner radius follows the slider value live.
    let preview = single::<ShowcasePreview>(g.world_app());
    let radius = match g
        .world_app()
        .world()
        .get::<Border>(preview)
        .unwrap()
        .radius
        .top_left
        .x
    {
        Length::Px(px) => px,
        other => panic!("preview radius is not px: {other:?}"),
    };
    assert_eq!(
        radius as f64, after,
        "the preview square's radius tracks the slider value",
    );
}

#[test]
fn showcase_disclosure_click_expands_and_collapses() {
    let mut g = Gallery::new();
    g.goto(Screen::Showcase);

    let disclosure = in_screen::<Disclosure>(g.world_app(), Screen::Showcase)
        .into_iter()
        .next()
        .expect("a showcase disclosure");
    let body = in_screen::<ShowcaseDiscBody>(g.world_app(), Screen::Showcase)
        .into_iter()
        .next()
        .expect("the disclosure body");
    // Collapsed at rest: the body is Display::None.
    assert_eq!(
        g.world_app().world().get::<Display>(body).copied(),
        Some(Display::None),
        "the disclosure body starts collapsed",
    );

    g.click(disclosure);
    g.settle(2);
    assert_eq!(
        g.world_app()
            .world()
            .get::<A11yExpanded>(disclosure)
            .map(|e| e.0),
        Some(true),
        "clicking the disclosure header expands it",
    );
    assert_ne!(
        g.world_app().world().get::<Display>(body).copied(),
        Some(Display::None),
        "the body is revealed when expanded",
    );

    g.click(disclosure);
    g.settle(2);
    assert_eq!(
        g.world_app()
            .world()
            .get::<A11yExpanded>(disclosure)
            .map(|e| e.0),
        Some(false),
        "clicking again collapses it",
    );
}
