//! Menu / MenuButton / MenuItem — C5-c (scroll-overlay-modal.md §B.3).
//!
//! A [`MenuButton`] opens a [`Menu`] popover containing [`MenuItem`]s. The menu
//! IS a [`Popover`] (it composes the anchored top-layer
//! positioning + the `auto` light-dismiss the C5-b popover substrate gives), with
//! a **roving / `aria-activedescendant`** keyboard model layered on top:
//!
//! - **The button** is `A11yRole::Button` (so its `Click` rides the EXISTING
//!   Button contract → `OnPress`, pointer + keyboard Enter/Space + AT-`Click`),
//!   carries [`A11yHasPopup`]`(Menu)` (it opens a menu popup) and the disclosure
//!   state-keyed [`A11yExpanded`] (open/closed — reusing the Disclosure pattern),
//!   and `controls` the menu. **W6a (MVU-as-core):** a press on the button (any
//!   modality, via `OnPress`) is routed by [`route_menu_press`] into the menu's
//!   [`MenuModel`] machine as a [`MenuMsg::Toggle`]; the single ordered drain folds
//!   it and [`bind_menu_model`] projects the model onto the button's `A11yExpanded`,
//!   the menu visibility, focus, and active-descendant (it does NOT use the shared
//!   `advance_expanded_on_press` consumer — that one is the Disclosure's).
//! - **The menu** is `A11yRole::Menu` + `Focusable` (the **container holds
//!   focus**; items are NOT individually focused — the roving / activedescendant
//!   pattern). Its [`A11yRelations::active_descendant`] points at the active item.
//! - **Each item** is `A11yRole::MenuItem`. Items are **not** `Focusable` and not
//!   in `is_activatable_role` (the container, not the item, owns focus); the menu
//!   keyboard nav activates the *active* item by writing the shared `OnPress`
//!   sink for it.
//!
//! Keyboard nav (the C5-c roving system [`menu_keyboard_nav`], in `BuiySet::Input`
//! while the menu is open + focused): ArrowDown/Up move the active item (wrap),
//! Home/End jump to first/last, Enter/Space activate the active item (write
//! `OnPress` for it), Escape closes. The C5-b light-dismiss (`dismiss.rs`) closes
//! the open menu on an outside press too. None of these move per-item DOM focus —
//! focus stays on the menu container; only `active_descendant` tracks the current
//! item (the APG `aria-activedescendant` traversal).

use crate::popover::popover_stacking;
use crate::popover::{Popover, is_open};
use bevy::ecs::message::Messages;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer, Press};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use buiy_core::interaction::OnPress;
use buiy_core::mvu::{Cmd, Envelope, Model, enqueue, fold_one_inline};
use buiy_core::{
    a11y::{
        A11yExpanded, A11yHasPopup, A11yLabel, A11yRelations, A11yRole, Action, ActionData,
        ActionError, HasPopup,
    },
    components::Node,
    focus::{FocusVisible, Focusable, FocusedEntity},
    layout::{BoxModel, Length, Stacking, Style},
    render::color::ColorToken,
    render::components::{
        Background, Border, Corners, CssVisibility, LineStyle, Outline, Radius, TextColor,
    },
    text::{FontSize, Text},
};

use crate::dismiss::DismissCause;

/// The catalog font size for the menu-button label + menu-item glyphs (logical px).
pub(crate) const MENU_FONT_SIZE: f32 = 16.0;

// ---------------------------------------------------------------------------
// MenuButton — the trigger that opens (controls) a Menu popover.
// ---------------------------------------------------------------------------

/// MenuButton widget marker — a [`Button`](crate::Button)-shaped trigger that
/// opens a [`Menu`] popup (scroll-overlay-modal.md §B.3). The `#[require(...)]`
/// contract is the single source of the trigger shape (the `Button`/`Disclosure`
/// precedent): the bare marker materializes the full layout-visible + paintable +
/// focusable + accessible **menu trigger**.
///
/// The require list:
/// - `Node` — the layout marker (pulls the full `Style` decomposition).
/// - `BoxModel`/`Background`/`Border` — the canonical trigger box + paint.
/// - `Focusable` — keyboard-focusable (the implicit `{Focus, Blur}`); its `Click`
///   rides the Button contract (Enter/Space via the existing APG keymap → `OnPress`).
/// - `A11yRole = A11yRole::Button` — so `contract_for(Button)` supplies the `Click`
///   verb + the APG Enter+Space keymap. The menu-popup capability is layered *on
///   top* via [`A11yHasPopup`], and the open state via [`A11yExpanded`] — NOT a new
///   role (the disclosure state-keyed precedent).
/// - `A11yHasPopup = A11yHasPopup(HasPopup::Menu)` — advertises `aria-haspopup=menu`
///   (P1a `set_has_popup`): the trigger opens a menu.
/// - `A11yExpanded` — the open/closed disclosure state (defaults to `false` /
///   closed). **W6a:** it is now a bind-derived PROJECTION of the menu's
///   [`MenuModel`] (`A11yExpanded.0 == MenuModel.open`, written by
///   [`bind_menu_model`]); a press routes through [`route_menu_press`] →
///   [`MenuMsg::Toggle`], NOT the shared `advance_expanded_on_press` flip.
/// - `A11yLabel` — the accessible name.
///
/// The `controls = [menu]` edge references the menu **entity**, unknown to the
/// `#[require]`; [`MenuButton::new`] / the `menu_button(...)` scene-fn author it
/// (they spawn the menu first), so a MenuButton always knows its menu.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = menu_button_box_model(),
    Background = menu_button_background(),
    Border = menu_button_border(),
    Focusable,
    A11yRole = A11yRole::Button,
    A11yHasPopup = menu_haspopup(),
    A11yExpanded,
    A11yLabel,
)]
pub struct MenuButton;

// ---------------------------------------------------------------------------
// Menu — the popover container (A11yRole::Menu) holding MenuItems.
// ---------------------------------------------------------------------------

/// Menu widget marker — the roving popover container (`A11yRole::Menu`) holding
/// [`MenuItem`]s (scroll-overlay-modal.md §B.3). **The menu IS a [`Popover`]**: it
/// composes the C5-b anchored top-layer positioning + the `auto` light-dismiss
/// substrate (so an outside press / Escape closes it), and adds the menu role +
/// the container-holds-focus roving model.
///
/// The require list:
/// - `Node` — the layout marker.
/// - `BoxModel`/`Background`/`Border` — the canonical menu panel box + paint.
/// - `Popover` — the positioning primitive (transitively `#[require]`s the
///   top-layer `Stacking`, the `Anchor` the positioning lowers into, and the
///   `auto` `LightDismiss` policy). [`MenuButton::new`] sets `Popover.anchor` to
///   the button so the menu is positioned below it.
/// - `Focusable` — the **container** owns focus (the roving / `aria-activedescendant`
///   pattern); items are not individually focused.
/// - `A11yRole = A11yRole::Menu` — the APG `menu` role.
/// - `A11yRelations` — its `active_descendant` tracks the active [`MenuItem`]
///   ([`bind_menu_model`] writes it from [`MenuModel::active`]; the P1a
///   `set_active_descendant` fold lowers it to `aria-activedescendant`).
/// - `MenuModel` — the machine-tier model (W6a) owning open/active/dismissed; the
///   single ordered drain is its sole writer.
///
/// A menu starts **closed** ([`MenuButton::new`] inserts `CssVisibility::Hidden`,
/// matching the default [`MenuModel`]`{ open: false, .. }`); a press routes through
/// [`route_menu_press`] → [`MenuMsg::Toggle`] → [`bind_menu_model`] to open it.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = menu_box_model(),
    Background = menu_background(),
    Border = menu_border(),
    Popover,
    Stacking = popover_stacking(),
    Focusable,
    A11yRole = A11yRole::Menu,
    A11yRelations,
    // W6a (MVU-as-core) — the machine-tier model. Every `Menu` owns a [`MenuModel`]
    // (default closed); the single ordered drain is the SOLE writer of its
    // multi-field open/active/dismissed state (see the "machine tier" section
    // below). Inserted by `#[require]` so a bare `Menu` marker — the gallery's
    // imperative idiom OR `Menu::new` — gets it for free.
    MenuModel,
)]
pub struct Menu;

// ---------------------------------------------------------------------------
// MenuItem — one entry in the Menu.
// ---------------------------------------------------------------------------

/// MenuItem widget marker — one entry in a [`Menu`] (`A11yRole::MenuItem`,
/// scroll-overlay-modal.md §B.3). A menu item is **not** `Focusable` (the menu
/// container owns focus; the roving model tracks the active item via the menu's
/// `active_descendant`, not per-item DOM focus) and **not** in `is_activatable_role`
/// (so the generic `pointer_click_emits_on_press` producer does not fire for it —
/// activating an item must ALSO close the menu, which that role-keyed producer can
/// not do). A pointer click on an item is instead routed through the dedicated
/// [`menu_item_click_emits_on_press`] observer (writes the shared `OnPress` sink +
/// closes the menu), the pointer mirror of the keyboard nav's Enter/Space branch.
///
/// The require list:
/// - `Node` — the layout marker.
/// - `BoxModel`/`Background` — the canonical item row box + fill.
/// - `A11yRole = A11yRole::MenuItem` — the APG `menuitem` role.
/// - `A11yLabel` — the accessible name (the visible label `Text` carries the
///   pixels; the name stays on the item root).
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = menu_item_box_model(),
    Background = menu_item_background(),
    A11yRole = A11yRole::MenuItem,
    A11yLabel,
)]
pub struct MenuItem;

// ---------------------------------------------------------------------------
// Canonical styling — `pub(crate)` so the `scene` module spells the same values.
// ---------------------------------------------------------------------------

/// The canonical menu-button box: a 120×32 trigger (the Button box).
// TODO(buiy-widget-catalog-design): replace hardcoded sizes with size tokens.
pub(crate) fn menu_button_box_model() -> BoxModel {
    Style::default()
        .width_px(120.0)
        .height_px(32.0)
        .padding(8.0)
        .box_model
}

/// The default menu-button fill (the `color.surface.secondary` token).
pub(crate) fn menu_button_background() -> Background {
    Background {
        color: ColorToken::SurfaceSecondary,
    }
}

/// The default menu-button border: rounded corners (`radius.md`).
pub(crate) fn menu_button_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(6.0)),
        ..Default::default()
    }
}

/// The canonical menu panel box: a 160px-wide column.
pub(crate) fn menu_box_model() -> BoxModel {
    Style::default()
        .flex_column()
        .width_px(160.0)
        .padding(4.0)
        .box_model
}

/// The default menu panel fill (the `color.surface.primary` token — a distinct
/// panel).
pub(crate) fn menu_background() -> Background {
    Background {
        color: ColorToken::SurfacePrimary,
    }
}

/// The default menu panel border: rounded corners (`radius.md`).
pub(crate) fn menu_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(8.0)),
        ..Default::default()
    }
}

/// The canonical menu-item row box: full menu width, a comfortable row height.
pub(crate) fn menu_item_box_model() -> BoxModel {
    Style::default().width_px(152.0).height_px(28.0).box_model
}

/// The default menu-item fill (transparent — the menu panel shows through; an
/// item highlight is a C6 paint concern, not built here).
pub(crate) fn menu_item_background() -> Background {
    Background {
        color: ColorToken::SurfacePrimary,
    }
}

/// The `aria-haspopup=menu` value the MenuButton advertises. `pub(crate)` so the
/// scene-fn spells the SAME value as the `#[require]` initializer.
pub(crate) fn menu_haspopup() -> A11yHasPopup {
    A11yHasPopup(HasPopup::Menu)
}

// ---------------------------------------------------------------------------
// Constructors — spawn-ready bundles wiring the button↔menu↔items relations.
// ---------------------------------------------------------------------------

impl MenuItem {
    /// Spawn-ready bundle for one labelled menu item. Returns `impl Bundle`
    /// carrying the full item contract (role `MenuItem` + the box + a11y) plus a
    /// visible label `Text` child (`Pickable::IGNORE` so a hit resolves to the
    /// item root — pick-through). The accessible name stays on the item root.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>) -> impl Bundle {
        let label = label.into();
        (
            MenuItem,
            A11yLabel(label.clone()),
            children![(
                Text(label),
                FontSize(MENU_FONT_SIZE),
                TextColor::default(),
                Pickable::IGNORE,
            )],
        )
    }
}

impl Menu {
    /// Spawn-ready bundle for a menu containing `items` (each an
    /// [`item`](MenuItem::new) bundle, passed via `children![ … ]`). Returns
    /// `impl Bundle` carrying the full menu contract (role `Menu` + the popover
    /// positioning substrate + container focus) plus the items as children. The
    /// menu starts **closed** (`CssVisibility::Hidden`) and **unanchored** —
    /// [`MenuButton::new`] anchors it to the button + opens it on activation.
    ///
    /// `items` is a `children![ … ]` bundle (e.g.
    /// `children![MenuItem::new("Cut"), MenuItem::new("Copy")]`) — the
    /// `Children::spawn(...)` result, merged onto the menu so its items are authored
    /// inline. Prefer [`MenuButton::new`], which spawns the menu + the button +
    /// wires the `controls`/`anchor` edges together; `Menu::new` is the lower-level
    /// builder for an author who spawns the two halves separately.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(items: impl Bundle) -> impl Bundle {
        (Menu, CssVisibility::Hidden, items)
    }
}

impl MenuButton {
    /// Spawn-ready bundle for a labelled menu button **and** its menu. Returns
    /// `impl Bundle` carrying the full trigger contract (role `Button` + the APG
    /// Enter/Space keymap + `A11yHasPopup(Menu)` + `A11yExpanded` + `A11yLabel`)
    /// plus two children: a visible label `Text` (`Pickable::IGNORE`, pick-through)
    /// and the controlled [`Menu`] (closed, the `items` as `MenuItem` children).
    ///
    /// The menu is authored as a `children!` of the button (the
    /// [`TooltipTrigger`](crate::TooltipTrigger) precedent — a top-layer overlay
    /// authored as a child of its trigger): top-layer membership rides
    /// `Stacking.top_layer`, **not** the ECS parent, so the menu still escapes the
    /// button's stacking context and positions via the popover anchor pipeline. The
    /// two-way edges — the button's `A11yRelations.controls = [menu]` and the menu's
    /// `Popover.anchor = button` — reference each other's entities (unknown at
    /// construction), so [`wire_menu_button`] fills them once the children exist
    /// (the disclosure `wire_disclosure_controls` precedent).
    ///
    /// `items` is a `children![ … ]` bundle of `MenuItem::new(...)` entries.
    ///
    /// ```ignore
    /// use buiy::prelude::*;
    /// world.spawn(MenuButton::new(
    ///     "Edit",
    ///     children![MenuItem::new("Cut"), MenuItem::new("Copy"), MenuItem::new("Paste")],
    /// ));
    /// ```
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>, items: impl Bundle) -> impl Bundle {
        let label = label.into();
        (
            MenuButton,
            A11yLabel(label.clone()),
            children![
                // The visible label pixels (the AT name stays on the button root).
                (
                    Text(label),
                    FontSize(MENU_FONT_SIZE),
                    TextColor::default(),
                    Pickable::IGNORE,
                ),
                // The controlled menu — closed, the items as children. `controls` /
                // `Popover.anchor` are wired by `wire_menu_button` once spawned.
                Menu::new(items),
            ],
        )
    }
}

/// The query data [`wire_menu_button`] reads per newly-childed [`MenuButton`]: the
/// button entity, its `Children` (to find the menu child), and its current
/// `A11yRelations` (to preserve author-set edges + check idempotency). Aliased so
/// the system signature stays under clippy's `type_complexity` bar (the disclosure
/// `TriggerControlsData` precedent).
type MenuButtonWireData = (Entity, &'static Children, Option<&'static A11yRelations>);

/// The change-detection filter for [`wire_menu_button`]: a `MenuButton` that just
/// gained its `Children` this frame (the `children!` macro inserts `Children` after
/// the root spawns), so the button↔menu edge wiring runs once per button.
type NewlyChildedMenuButton = (With<MenuButton>, Added<Children>);

/// Wire each [`MenuButton`]↔[`Menu`] pair's two-way edges once the button's
/// `children!` exist (C5-c — the disclosure `wire_disclosure_controls` precedent).
/// The button's `A11yRelations.controls = [menu]` and the menu's `Popover.anchor =
/// button` reference each other's **entities**, which do not exist until the
/// button's `children!` spawn, so neither can be set in [`MenuButton::new`]'s bundle
/// / the `#[require]` contract; this system fills both once, on the frame the button
/// gains its children.
///
/// Gated on `Added<Children>` so it runs once per newly-childed button. The
/// `controls` edge is filled only if absent (so an author who set it directly is
/// not overwritten — idempotent), and the menu's `Popover.anchor` is pointed at the
/// button (so `position_popover` places the menu below the button). Registered in
/// `WidgetsPlugin`.
pub fn wire_menu_button(
    mut commands: Commands,
    buttons: Query<MenuButtonWireData, NewlyChildedMenuButton>,
    menus: Query<(), With<Menu>>,
    mut popovers: Query<&mut Popover, With<Menu>>,
) {
    for (button, children, relations) in &buttons {
        let Some(menu) = children.iter().find(|&c| menus.get(c).is_ok()) else {
            continue; // No menu child (a malformed button) — nothing to wire.
        };
        // Fill the button's `controls = [menu]` unless an author already set it.
        if relations.is_none_or(|r| r.controls.is_empty()) {
            let mut next = relations.cloned().unwrap_or_default();
            next.controls = vec![menu];
            commands.entity(button).insert(next);
        }
        // Anchor the menu's popover to the button (positioned below it).
        if let Ok(mut popover) = popovers.get_mut(menu)
            && popover.anchor != Some(button)
        {
            popover.anchor = Some(button);
        }
    }
}

/// Attach the **click-containment** observers to each newly-spawned [`Menu`]
/// (C5-c). The menu is authored as a `children!` of the [`MenuButton`] (the
/// [`TooltipTrigger`](crate::TooltipTrigger) precedent — a top-layer overlay
/// authored under its trigger), so a `Pointer<Press>` / `Pointer<Click>` inside the
/// menu would otherwise **bubble up the `ChildOf` chain to the button** — which is
/// an `A11yRole::Button` whose `Click` activates it (the buiy_core
/// `pointer_click_emits_on_press` producer) and whose press re-focuses it (the
/// shared `focus_on_click`). That would toggle the menu **closed** the instant the
/// user clicks an item, and steal focus off the menu container.
///
/// To contain the menu's pointer events without touching the buiy_core producers
/// (which are correctly role-keyed and widget-agnostic), each menu gets an entity
/// observer that **stops propagation** of `Pointer<Press>`/`Pointer<Click>` at the
/// menu root: the event still reaches the menu and its items (item activation rides
/// the menu's own keyboard nav and the [`menu_item_click_emits_on_press`] per-item
/// pointer handler — which fires at the item DURING the bubble, before this stop),
/// but never bubbles past the menu to the controlling button. Gated on `Added<Menu>`
/// so the observers attach exactly once per menu. Registered in `WidgetsPlugin`.
pub fn guard_menu_clicks(mut commands: Commands, menus: Query<Entity, Added<Menu>>) {
    for menu in &menus {
        commands
            .entity(menu)
            .observe(|mut press: On<Pointer<Press>>| {
                // Contain the press: do not let it bubble to the controlling
                // button (which would re-focus the button + arm its activation).
                press.propagate(false);
            })
            .observe(|mut click: On<Pointer<Click>>| {
                // Contain the click: do not let it bubble to the button (whose
                // `pointer_click_emits_on_press` would toggle the menu closed).
                click.propagate(false);
            });
    }
}

/// Pointer-side activation for a clicked [`MenuItem`] (C5-c — the per-item pointer
/// handler [`guard_menu_clicks`] deferred). `MenuItem` is deliberately NOT in
/// `is_activatable_role`, so the generic buiy_core `pointer_click_emits_on_press`
/// producer never lowers a clicked item to the shared [`OnPress`] sink — this
/// dedicated global observer does, and it is the pointer **mirror of the keyboard
/// nav's Enter/Space branch** ([`menu_keyboard_nav`]): on a primary `Pointer<Click>`
/// whose (bubbled) target is a `MenuItem`, it (1) writes `OnPress(item)` — the SAME
/// sink the keyboard / AT path write, so an item callback consumer converges on one
/// route — and (2) **closes** the containing menu by **enqueueing** a
/// [`MenuMsg::Close`] (W6a: the single ordered drain folds it; the [`bind_menu_model`]
/// projection hides the menu + clears the active descendant + restores focus),
/// exactly the keyboard close-on-activate.
///
/// **Why a global observer keyed on the marker works through the propagation stop:**
/// `Pointer<Click>` is an `EntityEvent` that bubbles up the `ChildOf` chain,
/// re-targeting `click.entity` to each ancestor as it goes. The deepest pick may be
/// an item's label/icon child, but the event then bubbles to the `MenuItem` root —
/// where this observer fires with `click.entity == item` — and only AFTER that
/// reaches the menu root, where [`guard_menu_clicks`] stops propagation. So this
/// fires at the item regardless of which descendant was picked (no per-child
/// `Pickable::IGNORE` needed) and before the containment stop. Registered as a
/// global observer by [`WidgetsPlugin`](crate::WidgetsPlugin).
pub fn menu_item_click_emits_on_press(
    click: On<Pointer<Click>>,
    items: Query<(), With<MenuItem>>,
    parents: Query<&ChildOf>,
    menus: Query<(), With<Menu>>,
    mut writer: MessageWriter<OnPress>,
    mut commands: Commands,
) {
    if click.event.button != PointerButton::Primary {
        return;
    }
    let item = click.entity;
    // Only fire when the (bubbled) event target is a MenuItem — a click on the menu
    // panel chrome / a non-item child resolves to a non-`MenuItem` target and is a
    // no-op here (the bubble visits the item itself separately).
    if !items.contains(item) {
        return;
    }
    // Activate via the shared sink (the pointer mirror of the keyboard Enter branch;
    // `record_menu_activation` / a callback consumer read it).
    writer.write(OnPress(item));
    // W6a single-writer reroute (D3): instead of the old `close_menu` exclusive-World
    // mutation, ENQUEUE a `MenuMsg::Close` to the owning menu's [`MenuModel`]; the
    // single ordered drain is the sole writer. Items are direct children of their
    // menu (the `MenuButton::new` / `Menu::new` authoring + the gallery's imperative
    // idiom both spawn them so), so the owning menu is the item's `ChildOf` parent.
    if let Ok(parent) = parents.get(item) {
        let menu = parent.parent();
        if menus.contains(menu) {
            enqueue::<MenuModel>(
                &mut commands,
                menu,
                MenuMsg::Close(DismissReason::Activated),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The machine tier (W6a, MVU-as-core FINAL) — `MenuModel` + `MenuMsg` + the reducer.
//
// The exemplar machine of the spec's tiered model (§3): a real `Model` + reducer
// owning the menu's MULTI-FIELD state (open / active item / dismiss reason) where ONE
// owning model **deletes the reconciliation code** (D3). Before W6a the "menu is open"
// fact lived in TWO components — `A11yExpanded` on the button and `CssVisibility` on
// the menu — with two sync systems keeping them in lock-step: `sync_menu_open` (button
// → menu) and `sync_menu_dismissed` (menu → button, the named D3 cure-target, which
// existed ONLY because the generic light-dismiss writes the menu's `CssVisibility`
// directly without knowing the button). Now [`MenuModel`] is the single source of
// truth; the single ordered drain is its SOLE writer; [`bind_menu_model`] PROJECTS it
// onto `CssVisibility` + `active_descendant` + the button's `A11yExpanded` + focus.
// Both sync systems are deleted: the projection replaces `sync_menu_open`, and the
// light-dismiss now ENQUEUES (it never writes `CssVisibility`), so the reconciliation
// `sync_menu_dismissed` cured can no longer arise by construction.
//
// THE D4 CORRECTION (spec §4 — the load-bearing fix the prototype did NOT make): the
// machine's WHOLE chain — `Enqueue → ApplyDeferred → Drain → Bind` — is pinned into an
// EARLY window `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)` ([`MenuSet`]).
// The a11y tree reads the button's `A11yExpanded` (`translate.rs`), which `bind_menu_model`
// writes; pinning the bind EARLY (not the prototype's late `MvuSet::Bind`) makes a
// keyboard/pointer-driven open refresh `aria-expanded` in the SAME `app.update()` — the
// base is same-frame-correct, so a late bind would be a one-frame regression.
// ---------------------------------------------------------------------------

/// The MVU sub-sets for the [`Menu`] machine, chained `Enqueue → Drain → Bind` and
/// pinned EARLY — `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)` (spec §4, the
/// D4 correction). Mirrors [`ToggleLeafSet`](buiy_core::mvu::ToggleLeafSet), but adds a
/// `Bind` stage because the machine's a11y state is a *projection* of the model, NOT
/// the model itself (the leaf/machine asymmetry, §4.1).
///
/// The whole chain is early so the keyboard/pointer open→fold→**project** completes
/// BEFORE `BuiySet::A11yUpdate` builds the tree, so the button's projected
/// `A11yExpanded` is fresh SAME-frame. The prototype kept the late `MvuSet::Bind`
/// (`.after(A11yUpdate)`), which lagged `aria-expanded` one frame — a regression vs the
/// base (whose `A11yExpanded` write at `BuiySet::Input` is same-frame-correct). A pinned
/// `ApplyDeferred` between [`MenuSet::Enqueue`] and [`MenuSet::Drain`] flushes the
/// `commands`-deferred enqueues (incl. the §9 light-dismiss observer's) before the early
/// drain reads the inbox. Configured by [`WidgetsPlugin`](crate::WidgetsPlugin).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MenuSet {
    /// The enqueue-only edge: press routing ([`route_menu_press`]), keyboard nav
    /// ([`menu_keyboard_nav`]), and the dismiss→`Close` producers all enqueue here
    /// (or before this window). Runs after `BuiySet::Picking`, where every `OnPress`
    /// producer has written.
    Enqueue,
    /// The early ordered drain: [`menu_reducer`] folds + `set_if_neq` commits, the
    /// SOLE writer of [`MenuModel`] — before `BuiySet::A11yUpdate` builds the tree.
    Drain,
    /// The projection bind ([`bind_menu_model`]): writes `CssVisibility` +
    /// `active_descendant` + the button's `A11yExpanded` + focus from the folded model
    /// — ALSO before `A11yUpdate`, so the projected `aria-expanded` is fresh same-frame
    /// (the D4 correction; the leaf has no separate bind stage because its model IS the
    /// consumed component).
    Bind,
}

/// Why a [`Menu`] last closed — recorded on [`MenuModel::dismissed`] when it folds
/// shut. Diagnostic + the load-bearing fact for the escape-hatch story (which writer
/// closed it). `None` while open or never-closed.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DismissReason {
    /// An item was activated (keyboard Enter/Space or a pointer click on the item).
    Activated,
    /// The Escape key (the menu's roving-nav Escape, or the light-dismiss keyboard
    /// channel).
    Escape,
    /// An outside press (the C5-b light-dismiss pointer channel).
    OutsidePress,
    /// The trigger toggled it closed (a second press on the [`MenuButton`]).
    Toggle,
}

/// The machine-tier model for a [`Menu`] (W6a, spec §3). Owns the menu's multi-field
/// overlay state; the single ordered drain is the SOLE writer (every other would-be
/// writer ENQUEUES a [`MenuMsg`]). `#[require]`d by [`Menu`], so it defaults **closed**.
///
/// Fields:
/// - `open` — shown vs hidden. [`bind_menu_model`] projects it onto the menu's
///   `CssVisibility` and the button's `A11yExpanded`.
/// - `active` — the highlighted item as an **index** into the menu's [`MenuItem`]
///   children (document order), NOT an `Entity`: a stable index keeps the recorded
///   `MenuMsg` log entity-free and replay-portable (spec §7). The bind translates it
///   back to the item entity for `active_descendant`.
/// - `dismissed` — the [`DismissReason`] of the last close (`None` while open).
///
/// The focus-return target is **not** a field: it is structurally the controlling
/// `MenuButton` (the one whose `A11yRelations.controls` references this menu), so the
/// bind resolves it rather than storing a (replay-unstable) `Entity`.
#[derive(Component, Reflect, Clone, PartialEq, Debug, Default)]
#[reflect(Component, Default)]
pub struct MenuModel {
    /// Whether the menu is open (shown).
    pub open: bool,
    /// The highlighted / active item, as an index into the [`MenuItem`] children
    /// (document order). `None` ⇒ no active item.
    pub active: Option<usize>,
    /// Why the menu last closed (`None` while open / never-closed).
    pub dismissed: Option<DismissReason>,
}

/// The message vocabulary the [`menu_reducer`] folds into a [`MenuModel`]. EVERY
/// would-be writer of the menu's open/active state enqueues one of these; the drain is
/// the sole writer (D3 single-writer).
#[derive(Clone, Debug, Reflect, PartialEq)]
pub enum MenuMsg {
    /// Open the menu (highlights the first item).
    Open,
    /// Close the menu, recording why.
    Close(DismissReason),
    /// Toggle open↔closed — the trigger-press path. Folds (via [`Cmd::Emit`]) to
    /// `Open` when closed or `Close(Toggle)` when open, so the toggle decision lives
    /// in one place and the effect runs-to-completion in the same drain pass.
    Toggle,
    /// Highlight the item at this **absolute** index (the roving nav computes the
    /// wrap; the reducer just sets it). Ignored while closed.
    Highlight(usize),
}

/// The [`Menu`] machine reducer (W6a). Pure value-folding over [`MenuModel`] — no env,
/// no side effects beyond the [`Cmd`] it returns. The single ordered drain commits the
/// result via `set_if_neq` (so an idempotent fold cannot cascade), as the SOLE writer
/// of the menu's open/active/dismissed state.
///
/// `Toggle` returns [`Cmd::emit`] (the effect-as-value re-fold) rather than branching
/// imperatively, so the "what does a press do" decision is one fold and the resolved
/// verb (`Open`/`Close`) is what lands in the record log.
pub fn menu_reducer(model: &mut MenuModel, msg: MenuMsg) -> Cmd<MenuMsg> {
    match msg {
        MenuMsg::Open => {
            model.open = true;
            model.active = Some(0); // first item highlighted on open (APG)
            model.dismissed = None;
        }
        MenuMsg::Close(reason) => {
            model.open = false;
            model.active = None;
            model.dismissed = Some(reason);
        }
        MenuMsg::Toggle => {
            return if model.open {
                Cmd::emit(MenuMsg::Close(DismissReason::Toggle))
            } else {
                Cmd::emit(MenuMsg::Open)
            };
        }
        MenuMsg::Highlight(index) => {
            if model.open {
                model.active = Some(index);
            }
        }
    }
    Cmd::none()
}

/// The leaf reuses the existing single-source-of-truth component AS the model; the
/// machine declares a fresh [`MenuModel`] with the multi-field state. This impl only
/// names its [`MenuMsg`] inbox.
impl Model for MenuModel {
    type Msg = MenuMsg;
}

// ---------------------------------------------------------------------------
// Press routing + the bind (the model's projection).
// ---------------------------------------------------------------------------

/// Route a [`MenuButton`] press into the menu funnel (W6a). Replaces the menu half of
/// the shared `advance_expanded_on_press` consumer (the menu button is now EXCLUDED
/// from it, `Without<MenuButton>`, so it no longer double-writes the button's
/// `A11yExpanded` — which is now a bind-derived projection of [`MenuModel`]). On each
/// `OnPress` whose target is a `MenuButton`, it enqueues a [`MenuMsg::Toggle`] to the
/// button's controlled menu. Pointer click, keyboard Enter/Space (the Button APG
/// keymap), and an inbound AT `Action::Click` all converge on `OnPress`, so all three
/// modalities lower through the one funnel. Runs in [`MenuSet::Enqueue`] (the EARLY
/// enqueue edge); the pinned `ApplyDeferred` flushes the enqueue so the early drain
/// folds it the same frame, and the early bind projects it BEFORE `A11yUpdate`.
pub fn route_menu_press(
    mut reader: MessageReader<OnPress>,
    buttons: Query<&A11yRelations, With<MenuButton>>,
    menus: Query<(), With<Menu>>,
    mut commands: Commands,
) {
    for OnPress(entity) in reader.read() {
        let Ok(rel) = buttons.get(*entity) else {
            continue; // not a MenuButton — inert here (handled by another consumer).
        };
        let Some(&menu) = rel.controls.first() else {
            continue; // malformed MenuButton with no controlled menu — no-op.
        };
        if menus.contains(menu) {
            enqueue::<MenuModel>(&mut commands, menu, MenuMsg::Toggle);
        }
    }
}

/// The [`Menu`] machine's **inline AT set-verb hook** (W6b, spec §5.4) — registered once
/// into the core [`InlineActionRegistry`](buiy_core::a11y::InlineActionRegistry) at
/// [`WidgetsPlugin`](crate::WidgetsPlugin) build, consulted by the generic
/// `Expand`/`Collapse` honor (`action.rs`) BEFORE its default direct `A11yExpanded` write.
/// This closes the W5/W6a "advertised but inert" gap: before W6b an AT `Expand` on a
/// `MenuButton` wrote `A11yExpanded` directly, which [`bind_menu_model`] then re-clobbered
/// from the unchanged `MenuModel` (the model never moved) — so the menu never opened.
///
/// **The cross-entity hop (spec §5.4).** AT `Expand`/`Collapse` target the **`MenuButton`**
/// (it carries `A11yExpanded`), but the model lives on the **`Menu`**. The hook resolves
/// `button.A11yRelations.controls[0] → menu`, then folds the **ABSOLUTE** verb through the
/// SAME [`menu_reducer`] the batch drain uses, on a DIFFERENT entity than was dispatched:
/// `Expand ⇒ MenuMsg::Open`, `Collapse ⇒ MenuMsg::Close` — **never `MenuMsg::Toggle`**
/// (folding `Toggle` would wrongly CLOSE an already-open menu, since AT set-verbs are
/// absolute, not toggles).
///
/// **Live-component-synchronous + perform-then-update (spec §5.1).** [`fold_one_inline`]
/// mutates the live `MenuModel.open` the instant `dispatch_action_request` returns; the
/// early [`MenuSet::Bind`] (W6a) then projects `MenuModel.open → button.A11yExpanded` on the
/// next `app.update()`, so the snapshot reflects `aria-expanded` perform-then-update.
///
/// Returns `Some(Ok)` once it has folded (or for a malformed button with no controlled
/// menu — so it does NOT fall through to a direct write the bind would clobber). Returns
/// `None` for any non-`MenuButton` / non-Expand verb, so a plain disclosure falls through
/// to the default direct `A11yExpanded` write.
///
/// **Click vs Expand (spec §5.5).** This rides only the absolute `Expand`/`Collapse`
/// set-verbs. A `MenuButton`'s `Click` (activation/toggle) stays ASYNC via
/// `OnPress → route_menu_press → MenuMsg::Toggle → the batch drain` — the shared modality
/// path pointer/keyboard/AT-`Click` all converge on — and is intentionally NOT routed
/// through this inline path (a screen reader sending both is the documented
/// dual-advertisement caveat).
pub fn menu_inline_action_hook(
    world: &mut World,
    entity: Entity,
    action: Action,
    _data: Option<&ActionData>,
) -> Option<Result<(), ActionError>> {
    // Only Expand/Collapse on a MenuButton; everything else falls through (`None`).
    if !matches!(action, Action::Expand | Action::Collapse) {
        return None;
    }
    world.get::<MenuButton>(entity)?; // not a MenuButton ⇒ fall through (default write).
    // The cross-entity hop: button → controls[0] → the Menu carrying MenuModel.
    let menu = world
        .get::<A11yRelations>(entity)
        .and_then(|r| r.controls.first().copied());
    let Some(menu) = menu else {
        // A malformed MenuButton with no controlled menu — nothing to fold. Treat as
        // HANDLED (Ok) so it does NOT fall through to a direct `A11yExpanded` write the
        // bind would immediately clobber from the unchanged model.
        return Some(Ok(()));
    };
    if world.get::<MenuModel>(menu).is_none() {
        return Some(Ok(()));
    }
    // The ABSOLUTE verb (NOT Toggle): Expand ⇒ Open, Collapse ⇒ Close. A `Collapse` via
    // AT is the screen-reader analog of toggling the trigger shut, so it records
    // `DismissReason::Toggle`.
    let msg = if action == Action::Expand {
        MenuMsg::Open
    } else {
        MenuMsg::Close(DismissReason::Toggle)
    };
    // Fold INLINE through the shared body — live-component-synchronous: `MenuModel.open`
    // mutates before this returns (the early `MenuSet::Bind` projects it next update).
    fold_one_inline::<MenuModel>(world, menu, msg, menu_reducer);
    Some(Ok(()))
}

/// The [`Menu`]'s [`DismissRegistry`](crate::dismiss::DismissRegistry) close-hook (W7 —
/// the un-invert of the W6a `With<MenuModel>` dismiss stopgap, spec §9). Registered into
/// the registry at [`WidgetsPlugin`](crate::WidgetsPlugin) build; consulted by the generic
/// model-agnostic [`close_overlay`](crate::dismiss) light-dismiss / Escape sink BEFORE its
/// default direct `CssVisibility::Hidden` write.
///
/// For an overlay carrying a [`MenuModel`] it **enqueues** [`MenuMsg::Close`] — mapping the
/// model-agnostic [`DismissCause`] to the menu's [`DismissReason`] — so the single ordered
/// drain is the SOLE writer and [`bind_menu_model`] projects `open=false` back onto
/// `CssVisibility` + the button's `A11yExpanded` (deleting the old `sync_menu_dismissed`
/// reconciliation by construction). The enqueue writes the inbox directly (the hook holds
/// `&mut World`, so it cannot use the `Commands`-based [`enqueue`]); the pinned
/// `ApplyDeferred` flushed this hook's deferring `commands.queue` step before the early
/// `MenuSet::Drain` reads the inbox, so it folds SAME-frame (spec §4.3/§4.4).
///
/// Returns `Some(())` once it has enqueued; `None` for any non-`Menu` overlay, so a raw
/// tooltip / popover falls through to the default direct hide. The dismiss substrate stays
/// model-agnostic — this is the one place the menu's `DismissReason` mapping lives.
pub fn menu_dismiss_hook(world: &mut World, overlay: Entity, cause: DismissCause) -> Option<()> {
    world.get::<MenuModel>(overlay)?; // not a Menu ⇒ fall through to the default hide.
    let reason = match cause {
        DismissCause::OutsidePress => DismissReason::OutsidePress,
        DismissCause::Escape => DismissReason::Escape,
    };
    enqueue_menu(world, overlay, MenuMsg::Close(reason));
    Some(())
}

/// The `idx`-th [`MenuItem`] among `menu`'s children in document order, or `None`.
/// Translates [`MenuModel::active`] (an index) back to the item entity for
/// `active_descendant`.
fn nth_menu_item(
    menu: Entity,
    idx: usize,
    children: &Query<&Children, With<Menu>>,
    item_markers: &Query<(), With<MenuItem>>,
) -> Option<Entity> {
    children
        .get(menu)
        .ok()?
        .iter()
        .filter(|&c| item_markers.contains(c))
        .nth(idx)
}

/// Project each changed [`MenuModel`] onto the observable menu state (W6a — the
/// machine-tier bind that REPLACES both `sync_menu_open` and `sync_menu_dismissed`).
/// Reacts to `Changed<MenuModel>` and writes, idempotently (`set_if_neq` discipline):
///
/// - the menu's `CssVisibility` — `Visible` when open, else `Hidden`;
/// - the menu's `A11yRelations.active_descendant` — the item entity at
///   [`MenuModel::active`] (translated from index), or `None`;
/// - the controlling [`MenuButton`]'s `A11yExpanded` — mirrors `open` (so the AT
///   `aria-expanded` projection stays a pure derivative of the model);
/// - focus — open ⇒ the menu container (roving / `aria-activedescendant`); close ⇒
///   restore to the button (only if the menu, or nothing, held focus — never steal
///   from an unrelated entity, §C.4).
///
/// The controlling button is the one whose `A11yRelations.controls` references this
/// menu (the `MenuButton::new` / gallery wiring). `FocusedEntity`/`FocusVisible` are
/// owned by `FocusPlugin`; absent (a partial harness) ⇒ the focus moves are skipped,
/// the visibility + active-descendant projection still runs.
///
/// **THE D4 CORRECTION (spec §4):** registered in [`MenuSet::Bind`], the EARLY bind
/// stage `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)` — NOT the prototype's
/// late `MvuSet::Bind` (`.after(A11yUpdate)`). So the button's projected `A11yExpanded`
/// (which `build_tree` reads to emit `aria-expanded`) is fresh in the SAME
/// `app.update()` a keyboard/pointer press opens the menu. The `CssVisibility` write is
/// also before `Render`, so paint is not lagged either.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn bind_menu_model(
    changed: Query<(Entity, &MenuModel), Changed<MenuModel>>,
    children: Query<&Children, With<Menu>>,
    item_markers: Query<(), With<MenuItem>>,
    buttons: Query<(Entity, &A11yRelations), With<MenuButton>>,
    mut menu_vis: Query<&mut CssVisibility, With<Menu>>,
    mut menu_relations: Query<&mut A11yRelations, (With<Menu>, Without<MenuButton>)>,
    mut button_expanded: Query<&mut A11yExpanded, With<MenuButton>>,
    // Per-item ring state (audit N2): does the item carry ANY outline, and is it OUR
    // ring? `Has` is metadata-only, so this never conflicts with the `Commands` writes.
    item_rings: Query<(Has<Outline>, Has<MenuActiveRing>), With<MenuItem>>,
    mut focused: Option<ResMut<FocusedEntity>>,
    mut focus_visible: Option<ResMut<FocusVisible>>,
    mut commands: Commands,
) {
    for (menu, model) in &changed {
        // Resolve the controlling button (the one whose `controls` references this
        // menu) — the focus-return target + the `A11yExpanded` projection home.
        let button = buttons
            .iter()
            .find(|(_, rel)| rel.controls.first() == Some(&menu))
            .map(|(e, _)| e);

        // Visibility (set_if_neq — idempotent).
        let want_vis = if model.open {
            CssVisibility::Visible
        } else {
            CssVisibility::Hidden
        };
        if let Ok(mut vis) = menu_vis.get_mut(menu)
            && *vis != want_vis
        {
            *vis = want_vis;
        }

        // Active descendant — translate the model's active INDEX to the item entity.
        let active_entity = model
            .active
            .and_then(|i| nth_menu_item(menu, i, &children, &item_markers));
        if let Ok(mut rel) = menu_relations.get_mut(menu)
            && rel.active_descendant != active_entity
        {
            rel.active_descendant = active_entity;
        }

        // Roving-active item ring (audit N2): give the active item a framework
        // focus-ring `Outline` so sighted keyboard users see where roving focus is
        // (the AT `active_descendant` is set above; only the visual was missing). This
        // is folded into the bind — NOT a sibling system — because adding a system to
        // `Update` perturbs the executor's ordering enough to flip a schedule-fragile
        // hidden-node layout under the MT executor. Mirrors [`lower_focus_ring`]: the
        // [`MenuActiveRing`] marker gates the framework ring so it never clobbers an
        // author `Outline`; removal is scoped to THIS menu's items (so a second open
        // menu is untouched); close folds `active = None`, clearing every ring.
        if let Ok(menu_children) = children.get(menu) {
            for &item in menu_children {
                // Skip a non-`MenuItem` direct child (menu chrome, if any).
                let Ok((has_outline, has_ring)) = item_rings.get(item) else {
                    continue;
                };
                if Some(item) == active_entity {
                    if !has_ring && !has_outline {
                        commands
                            .entity(item)
                            .insert((menu_active_ring_outline(), MenuActiveRing));
                    }
                } else if has_ring {
                    commands
                        .entity(item)
                        .remove::<Outline>()
                        .remove::<MenuActiveRing>();
                }
            }
        }

        // The button's `A11yExpanded` mirrors `open` (set_if_neq).
        if let Some(button) = button
            && let Ok(mut e) = button_expanded.get_mut(button)
            && e.0 != model.open
        {
            e.0 = model.open;
        }

        // Focus: open ⇒ focus the menu container; close ⇒ restore to the button
        // (only if the menu / nothing had it). Keyboard origin ⇒ focus-visible.
        let focus_target = if model.open {
            Some(menu)
        } else {
            let had_menu_or_none = focused
                .as_ref()
                .is_none_or(|f| f.0 == Some(menu) || f.0.is_none());
            had_menu_or_none.then_some(button).flatten()
        };
        if let Some(target) = focus_target {
            if let Some(f) = focused.as_mut()
                && f.0 != Some(target)
            {
                f.0 = Some(target);
            }
            if let Some(v) = focus_visible.as_mut()
                && !v.0
            {
                v.0 = true;
            }
        }
    }
}

/// Marks the [`Outline`] that [`bind_menu_model`]'s roving-active ring pass owns, so it
/// only ever inserts/removes the roving-active ring and never disturbs an author's own
/// `Outline` on a menu item. Paint-only, framework-written — mirrors
/// [`buiy_core::focus::FocusRingMarker`], hence the leaner derives.
#[derive(Component, Clone, Copy, Debug)]
pub struct MenuActiveRing;

/// The active-menu-item ring: the framework focus-ring token, drawn ON the item's
/// border-box edge (`offset: 0`) rather than the +2px *outset* the standalone focus
/// ring uses ([`buiy_core::focus`]) — menu items are full-width and tightly stacked, so
/// an outset ring would overlap neighbors / clip at the panel edge, while an edge ring
/// reads cleanly. Width + color match the canonical focus ring (WCAG 2.4.11 ≥ 2px;
/// [`ColorToken::FocusRing`] is ≥ 3:1 and forced-colors-safe).
fn menu_active_ring_outline() -> Outline {
    Outline {
        color: ColorToken::FocusRing,
        style: LineStyle::Solid,
        width: Length::px(2.0),
        offset: Length::px(0.0),
    }
}

// ---------------------------------------------------------------------------
// Roving keyboard navigation — Arrow/Home/End highlight; Enter/Space activate;
// Escape closes. Every state change ENQUEUES a `MenuMsg` (W6a single-writer).
// ---------------------------------------------------------------------------

/// Roving / `aria-activedescendant` keyboard navigation for the focused open menu
/// (C5-c, §B.3). Runs in [`MenuSet::Enqueue`] (the early enqueue edge), gated on the
/// [`FocusedEntity`] being an **open** [`Menu`]. The menu **container** holds focus
/// (the item is never DOM-focused); the keys ENQUEUE [`MenuMsg`]s that the single
/// ordered drain folds into [`MenuModel`] (the drain is the sole writer):
///
/// - **ArrowDown** — highlight the next item (wrap past the last to the first).
/// - **ArrowUp** — highlight the previous item (wrap past the first to the last).
/// - **Home** — the first item; **End** — the last item.
/// - **Enter / Space** — activate the active item (write the shared [`OnPress`]
///   sink for it — the SAME sink the pointer / Button-keyboard / AT-`Click` paths
///   write) and enqueue [`MenuMsg::Close`]`(Activated)`.
/// - **Escape** — enqueue [`MenuMsg::Close`]`(Escape)`. (The C5-b `escape_dismiss`
///   ALSO closes the top-most open light-dismiss overlay through the funnel; both
///   enqueue, so the idempotent fold settles either way.)
///
/// An **exclusive** system (`&mut World`) because it reads the live model + writes
/// the `Envelope<MenuModel>` inbox and the `OnPress` message directly (the same
/// `&mut World` shape as the slider keyboard). The running active **index** is
/// tracked locally across the keys in one frame, so multiple keys in a single frame
/// compound correctly even though the model only updates at the later drain. It runs
/// in [`MenuSet::Enqueue`] (before the early drain) so the enqueued messages fold the
/// SAME frame. Under a partial/headless harness with no keyboard infra
/// (`Messages<KeyboardInput>` absent) or no `FocusedEntity`, the system is inert.
pub fn menu_keyboard_nav(world: &mut World) {
    use bevy::input::ButtonState;
    use bevy::input::keyboard::KeyboardInput;

    // Gate FIRST on a focused, open menu — before any keyboard access — so a
    // non-menu focus leaves the keyboard message buffer untouched for the other
    // keyboard handlers' readers (the `slider_keyboard` discipline).
    let Some(menu) = world.get_resource::<FocusedEntity>().and_then(|f| f.0) else {
        return; // no focus resource / nothing focused — inert.
    };
    if world.get::<Menu>(menu).is_none() {
        return; // the focused entity is not a menu — inert.
    }
    if !is_open(world.get::<CssVisibility>(menu)) {
        return; // the menu is closed — its keys are inert.
    }

    // A focused open menu: read out its KeyDown key codes. Copy them out so the
    // message borrow ends before the `&mut World` mutations below.
    let keys_down: Vec<KeyCode> = {
        let Some(mut messages) =
            world.get_resource_mut::<bevy::ecs::message::Messages<KeyboardInput>>()
        else {
            return; // no keyboard infra — inert.
        };
        messages
            .drain()
            .filter(|ev| ev.state == ButtonState::Pressed)
            .map(|ev| ev.key_code)
            .collect()
    };
    if keys_down.is_empty() {
        return;
    }

    // The ordered item set (for the Enter target + the count).
    let items: Vec<Entity> = {
        let Some(children) = world.get::<Children>(menu) else {
            return; // no items — nothing to navigate.
        };
        children
            .iter()
            .filter(|&c| world.get::<MenuItem>(c).is_some())
            .collect()
    };
    if items.is_empty() {
        return;
    }
    let n = items.len();
    // The running active INDEX, seeded from the model. Tracked locally so several
    // keys in one frame compound (the drain folds the enqueued Highlights in order).
    let mut cur: Option<usize> = world.get::<MenuModel>(menu).and_then(|m| m.active);

    for key in keys_down {
        match key {
            KeyCode::ArrowDown => {
                let next = match cur {
                    Some(i) => (i + 1) % n,
                    None => 0,
                };
                cur = Some(next);
                enqueue_menu(world, menu, MenuMsg::Highlight(next));
            }
            KeyCode::ArrowUp => {
                let prev = match cur {
                    Some(i) => (i + n - 1) % n,
                    None => n - 1,
                };
                cur = Some(prev);
                enqueue_menu(world, menu, MenuMsg::Highlight(prev));
            }
            KeyCode::Home => {
                cur = Some(0);
                enqueue_menu(world, menu, MenuMsg::Highlight(0));
            }
            KeyCode::End => {
                cur = Some(n - 1);
                enqueue_menu(world, menu, MenuMsg::Highlight(n - 1));
            }
            KeyCode::Enter | KeyCode::Space => {
                // Activate the active item (write the shared OnPress sink for it),
                // then enqueue Close — the drain folds it shut.
                let target = items[cur.unwrap_or(0)];
                if let Some(mut messages) = world.get_resource_mut::<Messages<OnPress>>() {
                    messages.write(OnPress(target));
                }
                enqueue_menu(world, menu, MenuMsg::Close(DismissReason::Activated));
            }
            KeyCode::Escape => enqueue_menu(world, menu, MenuMsg::Close(DismissReason::Escape)),
            _ => {}
        }
    }
}

/// Write a [`MenuMsg`] directly into the [`MenuModel`] inbox. The exclusive-`&mut
/// World` keyboard nav can't use the `Commands`-based [`enqueue`]; it writes the
/// `Envelope` straight into `Messages<Envelope<MenuModel>>`, which the single ordered
/// drain reads later the same frame. A no-op if the MVU chain isn't present (a partial
/// harness without the model registered).
fn enqueue_menu(world: &mut World, menu: Entity, msg: MenuMsg) {
    if let Some(mut messages) = world.get_resource_mut::<Messages<Envelope<MenuModel>>>() {
        messages.write(Envelope::user(menu, msg));
    }
}
