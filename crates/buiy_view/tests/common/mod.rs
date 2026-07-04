//! Shared headless harness for the `buiy_view` logic tests (no GPU).
#![allow(dead_code)]

use bevy::camera::NormalizedRenderTarget;
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};
use buiy_core::ResolvedLayout;
use buiy_core::interaction::OnPress;

/// The logic-plugin subset (everything the surface needs except the GPU render
/// plugin) — mirrors the `hello_button` MVU logic test. MVU scaffolding
/// (`MvuCorePlugin`) rides in with `WidgetsPlugin`.
pub fn logic_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins((
            buiy_core::CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::text::BuiyTextPlugin::default(),
            buiy_widgets::WidgetsPlugin,
        ));
    app
}

/// Enough frames for the seed reconcile (frame 1, before Layout) to build the
/// tree from the `Startup`-spawned model.
pub fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

/// Synthesize a real `OnPress` on `target` and drive it through the funnel.
///
/// Two updates because the reconciler runs **`.before(BuiySet::Layout)`** (#10):
/// frame N routes + drains (the model changes), frame N+1's front-of-frame
/// reconcile reads that `Changed<M>` and patches the derived tree.
pub fn press(app: &mut App, target: Entity) {
    app.world_mut()
        .resource_mut::<Messages<OnPress>>()
        .write(OnPress(target));
    app.update(); // frame N: route(Enqueue) → drain(Drain) — model changes
    app.update(); // frame N+1: reconcile(before Layout) patches the derived tree
}

/// The **native-pointer live-interaction tier for view apps** (F6, spec §2.7) —
/// drives REAL synthetic pointer clicks through the production `bevy_picking` +
/// Buiy backend against a running `ui()` app, so a test observes the exact
/// **pick occlusion** the invisible-occluder bug class lives in.
///
/// This is deliberately NOT [`press`] (which writes `OnPress(target)` directly)
/// and NOT an a11y-probe `Action::Click` — both action a widget by role/label and
/// **bypass** pick occlusion, so they can never see a topmost transparent box
/// swallowing every click (the bug that shipped 3x). Here a click is a real
/// `PointerInput` move -> press -> release at a window position; the hit is
/// whatever the real hit-test + stacking + `Pickable` arbitration resolves — the
/// same path a live cursor takes. It is the default gate for any `.top_layer()` /
/// transparent-container change.
///
/// Composes the picking stack onto a [`logic_app`]-based `ui()` app: bevy's core
/// `PickingPlugin` (registers `PointerInput`/`PointerHits`), Buiy's `PickingPlugin`
/// (the hover -> `Pointer<E>` layer + the `Pointer<Click>` -> `OnPress` producer),
/// and the Buiy backend (`emit_picks`). A synthetic primary window + `PointerId::Mouse`
/// are the only pointer source (no `PointerInputPlugin` winit reader). The `ui()`
/// camera targets `Window(Primary)` and resolves against the synthetic window.
pub struct ViewPointer {
    app: App,
    window: Entity,
    _pointer: Entity,
}

impl ViewPointer {
    /// Wrap an app that has already had `.ui(init, update, view)` installed, add
    /// the real picking stack + a synthetic window/pointer, and settle so the tree
    /// is built + laid out + a `GlobalTransform` exists on every node.
    pub fn new(mut app: App) -> Self {
        app.add_plugins(bevy::picking::PickingPlugin)
            .add_plugins(buiy_core::picking::PickingPlugin)
            .add_plugins(buiy_core::picking::BuiyPickingBackendPlugin);

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
        // The mouse pointer (no `PointerInputPlugin` to spawn one). Its `#[require]`
        // adds `PointerLocation`/`PointerPress`, which our injected `PointerInput`
        // drives via bevy_picking's `receive`.
        let pointer = app.world_mut().spawn(PointerId::Mouse).id();

        let mut h = Self {
            app,
            window,
            _pointer: pointer,
        };
        h.settle(10);
        h
    }

    /// Pump `n` frames.
    pub fn settle(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
        }
    }

    /// Read-only world access for assertions.
    pub fn world(&self) -> &World {
        self.app.world()
    }

    /// Mutable world access (entity-lookup queries).
    pub fn world_mut(&mut self) -> &mut World {
        self.app.world_mut()
    }

    /// The absolute (window-logical) center of `entity`'s laid-out box — the basis
    /// `emit_picks` hit-tests (`GlobalTransform.translation` is the top-left).
    pub fn center(&self, entity: Entity) -> Vec2 {
        let size = self
            .app
            .world()
            .get::<ResolvedLayout>(entity)
            .unwrap_or_else(|| panic!("entity {entity:?} has no ResolvedLayout (not laid out)"))
            .size;
        let gt = self
            .app
            .world()
            .get::<GlobalTransform>(entity)
            .unwrap_or_else(|| panic!("entity {entity:?} has no GlobalTransform"));
        gt.translation().truncate() + size / 2.0
    }

    fn loc(&self, pos: Vec2) -> Location {
        let target = WindowRef::Entity(self.window)
            .normalize(Some(self.window))
            .expect("normalize window ref");
        Location {
            target: NormalizedRenderTarget::Window(target),
            position: pos,
        }
    }

    fn pointer_input(&mut self, pos: Vec2, action: PointerAction) {
        let location = self.loc(pos);
        self.app.world_mut().write_message(PointerInput {
            pointer_id: PointerId::Mouse,
            location,
            action,
        });
    }

    /// Drive a full primary click at a WINDOW-space position through the live
    /// backend: move (build the hovermap) -> press -> release (-> `Pointer<Click>`
    /// -> the `OnPress` producer -> the view's `route_presses` -> the MVU drain). A
    /// trailing settle frame lets the reconcile that runs `.before(Layout)` next
    /// frame patch the derived tree.
    pub fn click_at(&mut self, pos: Vec2) {
        self.pointer_input(pos, PointerAction::Move { delta: Vec2::ZERO });
        self.app.update();
        self.pointer_input(pos, PointerAction::Press(PointerButton::Primary));
        self.app.update();
        self.pointer_input(pos, PointerAction::Release(PointerButton::Primary));
        self.app.update();
        self.app.update();
    }

    /// Click an entity at its laid-out center.
    pub fn click(&mut self, entity: Entity) {
        let c = self.center(entity);
        self.click_at(c);
    }
}
