//! Headless **F5 interaction** verification (spec §2.6).
//!
//! Two things F5 adds, both proven against the REAL pipeline:
//! 1. **The general press route** — a clickable *container* (whose child
//!    intercepts the pointer hit) and a pressable *raster* become activatable and
//!    fold their `Msg`, via a real pointer click (bubbling child→parent) AND via
//!    the role-keyed AT/probe `Action::Click`.
//! 2. **The interaction-state visual layer** — a pressable node dips (a transient
//!    `Translate`) while held and reverts on release.
//!
//! These are the *live-interaction* + *a11y-probe* tiers the spec assigns to F5
//! (a headless component snapshot can't see pick occlusion / bubbling). buiy_view
//! **cannot** depend on `buiy_verify` (that would be a dependency cycle — buiy_verify
//! depends on buiy_view), so this stands up its own minimal synthetic-pointer
//! harness — the same production recipe as `buiy_verify::pointer::PointerHarness`
//! (a `PrimaryWindow` + the `ui()` camera + a `PointerId::Mouse`, injecting
//! `PointerInput` directly). F6 later generalizes this into a reusable tier.

use bevy::camera::NormalizedRenderTarget;
use bevy::picking::pointer::{
    Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};

use buiy_core::ResolvedLayout;
use buiy_core::a11y::{A11yRole, inprocess};
use buiy_core::layout::{Length, Translate};
use buiy_core::mvu::{Cmd, Model};
use buiy_view::{
    BuiyViewAppExt, DEFAULT_PRESS_DEPTH, Element, InteractionState, Kind, button, column,
    find_kind, find_press_target, raster, text,
};

// --- The model under test -------------------------------------------------

#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct M {
    tile_clicks: u32,
    chip_clicks: u32,
    go_clicks: u32,
}
impl Model for M {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Tile,
    Chip,
    Go,
}

fn update(s: &mut M, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Tile => s.tile_clicks += 1,
        Msg::Chip => s.chip_clicks += 1,
        Msg::Go => s.go_clicks += 1,
    }
    Cmd::none()
}

// --- The minimal synthetic-pointer harness (production picking path) -------

struct Live {
    app: App,
    window: Entity,
    pointer: Entity,
}

impl Live {
    fn new(view: fn(&M) -> Element<Msg>) -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::transform::TransformPlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(bevy::picking::PickingPlugin)
            .add_plugins((
                buiy_core::CorePlugin,
                buiy_core::theme::ThemePlugin,
                buiy_core::a11y::A11yPlugin,
                buiy_core::focus::FocusPlugin,
                buiy_core::layout::LayoutPlugin,
                buiy_core::text::BuiyTextPlugin::default(),
                buiy_core::picking::PickingPlugin,
                buiy_core::picking::BuiyPickingBackendPlugin,
                buiy_widgets::WidgetsPlugin,
            ));
        app.ui(M::default(), update, view);

        // A synthetic primary window. The `ui()`-spawned `Camera2d` targets the
        // primary window by default, which `emit_picks` resolves the pointer's
        // target window to — so no second camera is needed.
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: WindowResolution::new(800, 600),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        let target = WindowRef::Entity(window)
            .normalize(Some(window))
            .expect("normalize window target");
        let pointer = app
            .world_mut()
            .spawn((
                PointerId::Mouse,
                PointerLocation::new(Location {
                    target: NormalizedRenderTarget::Window(target),
                    position: Vec2::ZERO,
                }),
            ))
            .id();

        let mut live = Self {
            app,
            window,
            pointer,
        };
        // Startup spawns the model + camera; a few frames run the seed reconcile +
        // layout + transform bridge so every node has ResolvedLayout + GlobalTransform.
        live.settle();
        live
    }

    fn settle(&mut self) {
        for _ in 0..6 {
            self.app.update();
        }
    }

    fn model(&mut self) -> M {
        self.app
            .world_mut()
            .query::<&M>()
            .iter(self.app.world())
            .next()
            .cloned()
            .expect("model exists")
    }

    /// The absolute (window-logical) center of `entity`, from the production
    /// `GlobalTransform` + its `ResolvedLayout` size.
    fn global_center(&self, entity: Entity) -> Vec2 {
        let world = self.app.world();
        let gt = world
            .get::<GlobalTransform>(entity)
            .expect("entity has GlobalTransform (went through the bridge)");
        let size = world
            .get::<ResolvedLayout>(entity)
            .expect("entity has ResolvedLayout")
            .size;
        gt.translation().truncate() + size * 0.5
    }

    fn move_to(&mut self, pos: Vec2) {
        let target = WindowRef::Entity(self.window)
            .normalize(Some(self.window))
            .expect("normalize window target");
        *self
            .app
            .world_mut()
            .get_mut::<PointerLocation>(self.pointer)
            .expect("pointer has PointerLocation") = PointerLocation::new(Location {
            target: NormalizedRenderTarget::Window(target),
            position: pos,
        });
        self.app.update();
    }

    fn button_action(&mut self, action: PointerAction) {
        let location = self
            .app
            .world()
            .get::<PointerLocation>(self.pointer)
            .expect("pointer has PointerLocation")
            .location()
            .expect("pointer has a location")
            .clone();
        self.app.world_mut().write_message(PointerInput {
            pointer_id: PointerId::Mouse,
            location,
            action,
        });
        self.app.update();
    }

    fn press(&mut self) {
        self.button_action(PointerAction::Press(PointerButton::Primary));
    }

    fn release(&mut self) {
        self.button_action(PointerAction::Release(PointerButton::Primary));
    }

    /// A full click at `pos`: move (seat the previous-frame hover map a `Click`
    /// needs), press, release, then settle so the resulting `OnPress` routes →
    /// drains → the reconciler patches the derived tree.
    fn click_at(&mut self, pos: Vec2) {
        self.move_to(pos);
        self.press();
        self.release();
        self.settle();
    }

    /// The node's press-down offset (`Translate.y` in px, `0.0` if none).
    fn translate_y(&self, entity: Entity) -> f32 {
        match self.app.world().get::<Translate>(entity) {
            Some(Translate(_, Length::Px(y), _)) => *y,
            _ => 0.0,
        }
    }
}

// --- Part 2: the general container / raster press route -------------------

/// A clickable CONTAINER: the text child intercepts the pointer hit and carries no
/// role, so a click on it must bubble to the container's own route.
fn tiles_view(_: &M) -> Element<Msg> {
    column![text("cat").size(20.0)]
        .on_press(Msg::Tile)
        .label("cat tile")
        .width(160.0)
        .height(80.0)
}

#[test]
fn a_click_on_a_container_child_bubbles_to_the_container_route() {
    let mut live = Live::new(tiles_view);
    // apply_pressable stamped the container with a real PressAction.
    let tile =
        find_press_target::<M>(live.app.world_mut(), &Msg::Tile).expect("the tile is a route");
    assert_eq!(live.model().tile_clicks, 0);

    // Aim at the CHILD (the text), not the container background — the topmost hit
    // is the child; the click must propagate child→parent to the container.
    let text_child = find_kind(live.app.world_mut(), Kind::Text).expect("the tile's text child");
    assert_ne!(
        text_child, tile,
        "the child is a distinct entity from the container"
    );
    let center = live.global_center(text_child);
    live.click_at(center);

    assert_eq!(
        live.model().tile_clicks,
        1,
        "a click on the child bubbled up to the container's press route"
    );
}

/// A pressable RASTER (the custom-avatar seat chip) — a leaf with its own hit.
fn chip_view(_: &M) -> Element<Msg> {
    raster(Handle::default(), 80.0, 80.0)
        .on_press(Msg::Chip)
        .label("seat chip")
}

#[test]
fn a_click_on_a_pressable_raster_routes() {
    let mut live = Live::new(chip_view);
    let chip =
        find_press_target::<M>(live.app.world_mut(), &Msg::Chip).expect("the chip is a route");
    assert_eq!(live.model().chip_clicks, 0);

    let center = live.global_center(chip);
    live.click_at(center);

    assert_eq!(
        live.model().chip_clicks,
        1,
        "a click on the raster folded its Msg"
    );
}

#[test]
fn the_container_route_is_reachable_by_role_and_name_via_the_probe() {
    let mut live = Live::new(tiles_view);
    assert_eq!(live.model().tile_clicks, 0);

    // The role-keyed AT / probe path (spec §2.6): the container is a `Button` named
    // by its `.label(..)` in the semantic tree, and `Action::Click` on it lowers to
    // the SAME `OnPress` sink — no pointer geometry involved.
    let node = inprocess::get_by_role(
        live.app.world_mut(),
        A11yRole::Button,
        Some("cat tile"),
        None,
    )
    .expect("the clickable container is a Button named 'cat tile' in the a11y tree");
    inprocess::click(live.app.world_mut(), node).expect("Action::Click is honored");
    live.settle();

    assert_eq!(
        live.model().tile_clicks,
        1,
        "the role-keyed Action::Click folded the container's Msg"
    );
}

// --- Part 3: the interaction-state visual layer (press-down) ---------------

fn go_button_view(_: &M) -> Element<Msg> {
    button("Go").on_press(Msg::Go).width(140.0).height(52.0)
}

#[test]
fn a_press_dips_the_button_and_release_reverts_it() {
    let mut live = Live::new(go_button_view);
    let go = find_press_target::<M>(live.app.world_mut(), &Msg::Go).expect("the button is a route");

    // Resting: no press-down.
    assert_eq!(live.translate_y(go), 0.0, "a resting button is not dipped");

    // Seat the hover map, then hold the primary button: the widget runtime dips it
    // by the default depth — a real synthetic press applies the pressed style.
    let center = live.global_center(go);
    live.move_to(center);
    live.press();
    assert_eq!(
        live.translate_y(go),
        DEFAULT_PRESS_DEPTH,
        "a held button dips by the default press depth"
    );

    // Release (still over): reverts to resting.
    live.release();
    assert_eq!(
        live.translate_y(go),
        0.0,
        "a released button reverts to resting"
    );
}

// --- Byte-stability guard: the route + visual layer are OPT-IN -------------

fn plain_view(_: &M) -> Element<Msg> {
    // A plain, non-clickable container (no `on_press`).
    column![text("plain")].width(100.0).height(40.0)
}

#[test]
fn a_plain_container_gets_no_role_and_no_interaction_state() {
    let mut live = Live::new(plain_view);
    let col = find_kind(live.app.world_mut(), Kind::Column).expect("the plain column");
    assert!(
        live.app.world().get::<A11yRole>(col).is_none(),
        "a non-clickable container gets no activatable button role"
    );
    assert!(
        live.app.world().get::<InteractionState>(col).is_none(),
        "and no interaction-state visual layer (opt-in — existing containers untouched)"
    );
}
