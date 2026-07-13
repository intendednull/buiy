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
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Background;
use buiy_view::{
    BuiyViewAppExt, Color, DEFAULT_PRESS_DEPTH, Element, InteractionState, Kind, button, column,
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

    /// The node's resolved `Background` fill token (`None` if it carries no
    /// `Background` component).
    fn background_token(&self, entity: Entity) -> Option<ColorToken> {
        self.app.world().get::<Background>(entity).map(|b| b.color)
    }

    /// The `ui()`-spawned model entity (the sole `M`).
    fn model_entity(&mut self) -> Entity {
        self.app
            .world_mut()
            .query_filtered::<Entity, With<M>>()
            .iter(self.app.world())
            .next()
            .expect("model entity exists")
    }

    /// Apply `mutate` to the model (tripping `Changed<M>`, exactly as a reducer
    /// fold does) then run **one** frame. This reproduces the §2 shared-`Background`
    /// write race with SAME-FRAME precision: `Changed<M>` is guaranteed present at
    /// the top of this single frame, so the reconciler definitely re-derives (and
    /// clobbers) `Background` this frame, and the hover resolver must re-win it in
    /// the same frame — a multi-frame settle would mask a one-frame ordering lag
    /// (the fill would recover the frame after a mis-ordered resolver, hiding the
    /// bug). A direct mutation (not the `Envelope` inbox) is used deliberately: the
    /// `MvuSet::Drain → ViewSet::Reconcile` order is not pinned, so a folded msg's
    /// clobber may land a frame later — non-deterministic for a teeth test. The
    /// realistic inbox→fold→reconcile→resolve path is exercised by the live test's
    /// `release()`. The pointer stays parked, so `InteractionState` never changes —
    /// only the reconcile `Background` write does.
    fn model_changed_frame(&mut self, mutate: impl FnOnce(&mut M)) {
        let model = self.model_entity();
        mutate(
            &mut self
                .app
                .world_mut()
                .get_mut::<M>(model)
                .expect("model exists"),
        );
        self.app.update();
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

// --- Track D: the declarative :hover / :active fill (spec §3) ---------------

/// A styled, pressable button with a declarative hover fill: resting
/// `Color::Surface` (`SurfacePrimary`), hover `Color::Accent` (`Accent`).
fn hover_button_view(_: &M) -> Element<Msg> {
    button("Go")
        .on_press(Msg::Go)
        .background(Color::Surface)
        .hover_bg(Color::Accent)
        .width(140.0)
        .height(52.0)
}

#[test]
fn hover_paints_the_hover_fill_and_press_composes_with_the_depth_dip() {
    let mut live = Live::new(hover_button_view);
    let go = find_press_target::<M>(live.app.world_mut(), &Msg::Go).expect("the button is a route");

    // The pointer spawns at the window origin, which sits inside a root-placed
    // button's box — park it off-target first for a deterministic resting state.
    live.move_to(Vec2::new(4000.0, 4000.0));

    // Resting: the resting fill (SurfacePrimary), no dip.
    assert_eq!(
        live.background_token(go),
        Some(ColorToken::SurfacePrimary),
        "a resting node paints its resting fill"
    );
    assert_eq!(live.translate_y(go), 0.0, "a resting node is not dipped");

    // Hover (pointer over, no press): the hover fill (Accent), still no dip.
    let center = live.global_center(go);
    live.move_to(center);
    assert_eq!(
        live.background_token(go),
        Some(ColorToken::Accent),
        "hover (None→Hover) paints the hover fill"
    );
    assert_eq!(live.translate_y(go), 0.0, "hover alone does not dip");

    // Press: the hover fill AND the press-down depth together (:active folds into
    // the hover fill; the depth dip is the distinct pressed look).
    live.press();
    assert_eq!(
        live.background_token(go),
        Some(ColorToken::Accent),
        "a held node keeps the hover fill (Press ⇒ hover token)"
    );
    assert_eq!(
        live.translate_y(go),
        DEFAULT_PRESS_DEPTH,
        "a held node dips — the hover fill and the depth compose"
    );

    // Release then leave (a real Pointer<Out>): revert to the resting fill + no dip.
    live.release();
    live.move_to(Vec2::new(4000.0, 4000.0)); // far off the button → Pointer<Out>
    assert_eq!(
        live.background_token(go),
        Some(ColorToken::SurfacePrimary),
        "leaving (Hover→None) reverts to the resting fill"
    );
    assert_eq!(live.translate_y(go), 0.0, "leaving reverts the dip");
}

#[test]
fn a_model_change_while_hovering_does_not_clobber_the_hover_fill() {
    // The §2 crux, as a regression with TEETH. `Background` is shared-ownership:
    // the reconciler re-derives it from `Element::background` on every `Changed<M>`
    // frame. While the node is hovered, a model change triggers that reconcile
    // write in the SAME frame the hover fill should hold — the resolver must re-win.
    //
    // This test FAILS if `apply_hover_visual` drops the `Or<Changed<Background>>`
    // half of its gate (then the reconcile write is never re-visited — the fill
    // stays clobbered to resting), OR if it is no longer ordered
    // `.after(reconcile::<M>)` (then it runs BEFORE the clobber and never sees it).
    // Proven by removing each: the assertion below goes red.
    let mut live = Live::new(hover_button_view);
    let go = find_press_target::<M>(live.app.world_mut(), &Msg::Go).expect("the button is a route");

    // Park the pointer over the node — it stays hovered for the rest of the test.
    let center = live.global_center(go);
    live.move_to(center);
    assert_eq!(
        live.background_token(go),
        Some(ColorToken::Accent),
        "precondition: the node is hovered and painting the hover fill"
    );

    // Change the model with the pointer parked: `InteractionState` stays Hover (no
    // `Changed<InteractionState>`), so ONLY the reconcile `Background` write (its
    // `set_if_neq` back to the resting `SurfacePrimary`) can re-trip the resolver —
    // the exact same-frame race, in a single deterministic frame.
    live.model_changed_frame(|m| m.go_clicks += 1);

    assert_eq!(
        live.background_token(go),
        Some(ColorToken::Accent),
        "the hover fill SURVIVED a same-frame reconcile Background write (the §2 race)"
    );
    // The model change really reached the reconciler this frame (else the assertion
    // above would be vacuous — no clobber to survive).
    assert_eq!(
        live.model().go_clicks,
        1,
        "the model change reconciled this frame (the clobber really happened)"
    );
}

/// An UNSTYLED (no `.background()`), hover-styled, pressable CONTAINER — a `column!`
/// carrying `.on_press` + `.hover_bg` but no explicit fill. This is the path that
/// takes `apply_background`'s `None` arm (the companion fix), **not** a button's
/// `apply_button_style` (which never touches `Background` when unstyled). Resting
/// fill is transparent (an unstyled container has no default fill), hover fill Accent.
fn hover_container_view(_: &M) -> Element<Msg> {
    column![text("tap").size(20.0)]
        .on_press(Msg::Tile)
        .hover_bg(Color::Accent)
        .label("tap tile")
        .width(160.0)
        .height(80.0)
}

#[test]
fn a_model_change_while_hovering_an_unstyled_container_keeps_the_hover_fill() {
    // The **companion-fix branch** (`apply_background`'s `None` arm, reconcile.rs).
    // An unstyled but hover-styled CONTAINER has no `Element::background`, so the
    // reconciler re-derives its `Background` via that `None` arm on every
    // `Changed<M>` frame. The fix makes the arm `HoverStyle`-aware — restoring the
    // fill to the resting token instead of stripping the `Background` the resolver's
    // non-optional `&mut Background` query requires. The existing styled-button race
    // test above exercises the DIFFERENT `apply_button_style` path; this is the only
    // coverage of the unstyled-container `None`-arm branch.
    let mut live = Live::new(hover_container_view);
    let tile =
        find_press_target::<M>(live.app.world_mut(), &Msg::Tile).expect("the container is a route");

    // The pointer spawns at the window origin, which sits inside a root-placed
    // container's box — park it off-target first for a deterministic resting state.
    live.move_to(Vec2::new(4000.0, 4000.0));
    assert_eq!(
        live.background_token(tile),
        Some(ColorToken::Transparent),
        "an unstyled hover-styled container rests transparent — a PRESENT Background \
         at the resting token, not a stripped/absent one"
    );

    // Hover (pointer over, no press): the resolver paints the hover fill (Accent).
    let center = live.global_center(tile);
    live.move_to(center);
    assert_eq!(
        live.background_token(tile),
        Some(ColorToken::Accent),
        "hovering the unstyled container paints the hover fill"
    );

    // A same-frame model change WHILE hovering (the §2 race on the `None`-arm path):
    // the reconciler re-derives `Background` via the `None` arm THIS exact frame; the
    // fix keeps it present at the resting token, and the `.after(reconcile)` resolver
    // re-wins it to Accent — one deterministic `Changed<M>` frame (a multi-frame
    // settle would mask a one-frame ordering lag).
    live.model_changed_frame(|m| m.tile_clicks += 1);
    assert_eq!(
        live.background_token(tile),
        Some(ColorToken::Accent),
        "the hover fill SURVIVED the same-frame `None`-arm re-derive (companion fix)"
    );
    // The model change really reached the reconciler this frame (else the assertion
    // above would be vacuous — no re-derive to survive).
    assert_eq!(
        live.model().tile_clicks,
        1,
        "the model change reconciled this frame (the `None`-arm re-derive really ran)"
    );

    // Un-hover (a real `Pointer<Out>`): revert to the resting token (transparent).
    live.move_to(Vec2::new(4000.0, 4000.0));
    assert_eq!(
        live.background_token(tile),
        Some(ColorToken::Transparent),
        "leaving reverts the unstyled container to its resting (transparent) fill"
    );
}
