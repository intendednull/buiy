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
//!   and `controls` the menu. The shared [`advance_expanded_on_press`](crate::advance_expanded_on_press) consumer
//!   flips `A11yExpanded` on every `OnPress`; [`sync_menu_open`] reacts to
//!   `Changed<A11yExpanded>` and drives the menu visibility + focus +
//!   active-descendant lifecycle.
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
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer, Press};
use bevy::prelude::*;
use buiy_core::interaction::OnPress;
use buiy_core::{
    a11y::{A11yExpanded, A11yHasPopup, A11yLabel, A11yRelations, A11yRole, HasPopup},
    components::Node,
    focus::{FocusVisible, Focusable, FocusedEntity},
    layout::{BoxModel, Stacking, Style},
    render::color::ColorToken,
    render::components::{Background, Border, Corners, CssVisibility, Radius, TextColor},
    text::{FontSize, Text},
};
use std::borrow::Cow;

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
///   closed). The shared [`advance_expanded_on_press`](crate::advance_expanded_on_press) consumer flips it on
///   `OnPress`; [`sync_menu_open`] drives the menu visibility from it.
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
///   ([`menu_keyboard_nav`] populates it; the P1a `set_active_descendant` fold
///   lowers it to `aria-activedescendant`).
///
/// A menu starts **closed** ([`MenuButton::new`] inserts `CssVisibility::Hidden`);
/// the button opens it via [`A11yExpanded`] → [`sync_menu_open`].
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
)]
pub struct Menu;

// ---------------------------------------------------------------------------
// MenuItem — one entry in the Menu.
// ---------------------------------------------------------------------------

/// MenuItem widget marker — one entry in a [`Menu`] (`A11yRole::MenuItem`,
/// scroll-overlay-modal.md §B.3). A menu item is **not** `Focusable` (the menu
/// container owns focus; the roving model tracks the active item via the menu's
/// `active_descendant`, not per-item DOM focus) and **not** in `is_activatable_role`
/// (a pointer click is not routed through the activatable-role producer — the menu
/// keyboard nav writes the shared `OnPress` for the active item directly).
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
        color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
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
        color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
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
        color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
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
/// the menu's own keyboard nav / a future per-item pointer handler), but never
/// bubbles past the menu to the controlling button. Gated on `Added<Menu>` so the
/// observers attach exactly once per menu. Registered in `WidgetsPlugin`.
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

// ---------------------------------------------------------------------------
// Open/close lifecycle — A11yExpanded (button) ↔ menu visibility + focus +
// active-descendant.
// ---------------------------------------------------------------------------

/// Drive the menu open/close lifecycle from each [`MenuButton`]'s [`A11yExpanded`]
/// (C5-c, §B.3 + §C.4). `A11yExpanded` is flipped by the shared
/// [`advance_expanded_on_press`](crate::advance_expanded_on_press) consumer (pointer / keyboard Enter+Space /
/// AT-`Click`, all converging on `OnPress`) and by the router's generic
/// `Expand`/`Collapse` set-verbs — this system reacts to `Changed<A11yExpanded>`
/// and applies the resulting menu state:
///
/// - **Open** (`expanded == true`): show the controlled menu
///   (`CssVisibility::Visible`), set its `active_descendant` to the **first**
///   [`MenuItem`] child, and move focus into the menu container (the roving /
///   `aria-activedescendant` model — the container holds focus, the item is the
///   active descendant).
/// - **Close** (`expanded == false`): hide the menu (`CssVisibility::Hidden`),
///   clear its `active_descendant`, and **restore focus to the button** (§C.4
///   focus restoration, scoped to the menu's single trigger).
///
/// The controlled menu is the button's [`A11yRelations::controls`] first entry
/// (the `MenuButton::new` wiring). A button with no `controls` menu (a malformed
/// trigger) is a graceful no-op. Runs in `BuiySet::Input` so a same-frame
/// activation (which writes `A11yExpanded` in `BuiySet::Input`) is observed and
/// The per-button query data [`sync_menu_open`] reads: the button entity, its
/// `A11yExpanded` (open state), and its `A11yRelations` (the `controls = [menu]`
/// edge). Aliased so the system signature stays under clippy's `type_complexity`
/// bar (the disclosure `TriggerControlsData` precedent).
type ChangedMenuButtonExpanded = (Entity, &'static A11yExpanded, &'static A11yRelations);

/// applied the same frame, then settles on the `Changed` gate.
pub fn sync_menu_open(
    changed: Query<ChangedMenuButtonExpanded, (With<MenuButton>, Changed<A11yExpanded>)>,
    items: Query<&Children, With<Menu>>,
    item_markers: Query<(), With<MenuItem>>,
    mut menu_vis: Query<&mut CssVisibility, With<Menu>>,
    mut menu_relations: Query<&mut A11yRelations, (With<Menu>, Without<MenuButton>)>,
    // `FocusedEntity`/`FocusVisible` are owned by `FocusPlugin`; under a partial
    // harness that adds `WidgetsPlugin` without it (the `button`/`disclosure` test
    // harnesses), the resources are absent and the focus moves are skipped — the
    // visibility + active-descendant lifecycle still runs (the same graceful
    // degradation `keyboard_activation` uses for its `Option<Res<FocusedEntity>>`).
    mut focused: Option<ResMut<FocusedEntity>>,
    mut focus_visible: Option<ResMut<FocusVisible>>,
) {
    for (button, expanded, relations) in &changed {
        let Some(&menu) = relations.controls.first() else {
            continue; // malformed MenuButton with no controlled menu — no-op.
        };
        // The focus target this transition wants (None ⇒ leave focus untouched).
        let focus_target;
        if expanded.0 {
            // OPEN: show the menu, set active_descendant to the first item, focus
            // the menu container.
            if let Ok(mut vis) = menu_vis.get_mut(menu)
                && *vis != CssVisibility::Visible
            {
                *vis = CssVisibility::Visible;
            }
            let first_item = items
                .get(menu)
                .ok()
                .and_then(|children| first_menu_item(children, &item_markers));
            if let Ok(mut rel) = menu_relations.get_mut(menu) {
                rel.active_descendant = first_item;
            }
            // Move focus into the menu container (keyboard origin ⇒ focus-visible).
            focus_target = Some(menu);
        } else {
            // CLOSE: hide the menu, clear active_descendant, restore focus to the
            // button.
            if let Ok(mut vis) = menu_vis.get_mut(menu)
                && *vis != CssVisibility::Hidden
            {
                *vis = CssVisibility::Hidden;
            }
            if let Ok(mut rel) = menu_relations.get_mut(menu) {
                rel.active_descendant = None;
            }
            // Restore focus to the button only if the menu (or nothing) had it —
            // never steal focus from an unrelated entity (§C.4). With no focus
            // resource (partial harness) the restore is a no-op.
            let had_menu_or_none = focused
                .as_ref()
                .is_none_or(|f| f.0 == Some(menu) || f.0.is_none());
            focus_target = had_menu_or_none.then_some(button);
        }
        // Apply the focus move (keyboard origin ⇒ focus-visible). Absent resources
        // (partial harness without `FocusPlugin`) ⇒ the focus move is skipped.
        if let Some(target) = focus_target {
            if let Some(f) = focused.as_mut() {
                f.0 = Some(target);
            }
            if let Some(v) = focus_visible.as_mut() {
                v.0 = true;
            }
        }
    }
}

/// The first [`MenuItem`] among `children` in document order, or `None` if the
/// menu has no item children. Shared by [`sync_menu_open`] (open ⇒ first item) and
/// the keyboard nav (Home).
fn first_menu_item(
    children: &Children,
    item_markers: &Query<(), With<MenuItem>>,
) -> Option<Entity> {
    children.iter().find(|&c| item_markers.get(c).is_ok())
}

/// The per-menu query data [`sync_menu_dismissed`] reads: the menu entity + its
/// `CssVisibility` (open state). Aliased so the system signature stays under
/// clippy's `type_complexity` bar.
type DismissedMenuData = (Entity, &'static CssVisibility);

/// Mirror each [`MenuButton`]'s [`A11yExpanded`] from the **actual** visibility of
/// its controlled [`Menu`] (C5-c — the light-dismiss ↔ button-state reconciliation,
/// §B.5 + §C.4). The C5-b light-dismiss (`dismiss.rs`) closes the open menu on an
/// outside press / Escape by flipping the menu's `CssVisibility` to `Hidden`
/// **directly** — it does not know about the button. Without this, the button's
/// `A11yExpanded` would stay `true` after a light-dismiss, desyncing the
/// `aria-expanded` state and breaking re-open (the button would believe the menu is
/// still open). This system reacts to `Changed<CssVisibility>` on a Menu and sets
/// the controlling button's `A11yExpanded` to the menu's open state, so a dismiss
/// from *any* source (outside press, Escape, or the button itself) leaves the two
/// in lock-step.
///
/// Idempotent (writes through `DerefMut` only on a real change), so it does not
/// ping-pong with [`sync_menu_open`]: when the button opens the menu, this sees
/// `expanded == is_open` already and is a no-op; when light-dismiss closes the menu,
/// this flips `expanded` to `false`, which [`sync_menu_open`] then sees as already
/// applied (the menu is already hidden) — both settle in one frame. Runs in
/// `BuiySet::Input` after the dismiss handlers.
pub fn sync_menu_dismissed(
    menus: Query<DismissedMenuData, (With<Menu>, Changed<CssVisibility>)>,
    buttons: Query<(Entity, &A11yRelations), With<MenuButton>>,
    mut expanded: Query<&mut A11yExpanded, With<MenuButton>>,
) {
    for (menu, vis) in &menus {
        let open = is_open(Some(vis));
        // The controlling button is the one whose `controls` references this menu.
        let Some((button, _)) = buttons
            .iter()
            .find(|(_, rel)| rel.controls.first() == Some(&menu))
        else {
            continue;
        };
        if let Ok(mut e) = expanded.get_mut(button)
            && e.0 != open
        {
            e.0 = open;
        }
    }
}

// ---------------------------------------------------------------------------
// Roving keyboard navigation — Arrow/Home/End move active_descendant;
// Enter/Space activate the active item; Escape closes.
// ---------------------------------------------------------------------------

/// The index of `active` within `items`, or `None` if absent. Used to compute the
/// next/previous item with wrap.
fn index_of(items: &[Entity], active: Option<Entity>) -> Option<usize> {
    active.and_then(|a| items.iter().position(|&e| e == a))
}

/// Roving / `aria-activedescendant` keyboard navigation for the focused open menu
/// (C5-c, §B.3). Runs in `BuiySet::Input`, gated on the [`FocusedEntity`] being an
/// **open** [`Menu`]. The menu **container** holds focus (the item is never DOM-
/// focused); the keys move the menu's [`A11yRelations::active_descendant`] across
/// its [`MenuItem`] children:
///
/// - **ArrowDown** — next item, wrapping past the last back to the first.
/// - **ArrowUp** — previous item, wrapping past the first to the last.
/// - **Home** — the first item; **End** — the last item.
/// - **Enter / Space** — activate the active item: write the shared [`OnPress`]
///   sink for it (the SAME sink the pointer / Button-keyboard / AT-`Click` paths
///   write, so a menu-item callback consumer converges on one route) and **close**
///   the menu (flip the button's `A11yExpanded` to `false` via the disclosure
///   set-verb path — [`sync_menu_open`] then hides + restores focus).
/// - **Escape** — close the menu (the C5-b `escape_dismiss` also closes the
///   top-most open light-dismiss overlay; this path additionally clears the
///   button's `A11yExpanded` so the two stay in lock-step).
///
/// An **exclusive** system (`&mut World`) because closing the menu writes the
/// button's `A11yExpanded` (resolved from the menu's controlling button, found via
/// the button whose `controls` references this menu) and activation writes the
/// `OnPress` message — the same `&mut World` shape as the slider keyboard. Under a
/// partial/headless harness with no keyboard infra (`Messages<KeyboardInput>`
/// absent) or no `FocusedEntity`, the system is inert.
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

    // The ordered item set + the current active descendant.
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
    let active = world
        .get::<A11yRelations>(menu)
        .and_then(|r| r.active_descendant);

    let n = items.len();
    let cur = index_of(&items, active);

    for key in keys_down {
        match key {
            KeyCode::ArrowDown => {
                let next = match cur {
                    Some(i) => (i + 1) % n,
                    None => 0,
                };
                set_active(world, menu, items[next]);
            }
            KeyCode::ArrowUp => {
                let prev = match cur {
                    Some(i) => (i + n - 1) % n,
                    None => n - 1,
                };
                set_active(world, menu, items[prev]);
            }
            KeyCode::Home => set_active(world, menu, items[0]),
            KeyCode::End => set_active(world, menu, items[n - 1]),
            KeyCode::Enter | KeyCode::Space => {
                // Activate the active item (write the shared OnPress sink for it),
                // then close the menu.
                let target = active.unwrap_or(items[0]);
                if let Some(mut messages) =
                    world.get_resource_mut::<bevy::ecs::message::Messages<OnPress>>()
                {
                    messages.write(OnPress(target));
                }
                close_menu(world, menu);
            }
            KeyCode::Escape => close_menu(world, menu),
            _ => {}
        }
    }
}

/// Set the menu's `active_descendant` to `item` (idempotent — only writes through
/// `DerefMut` on a real change so a no-op key does not tick `Changed<A11yRelations>`).
fn set_active(world: &mut World, menu: Entity, item: Entity) {
    if let Some(mut rel) = world.get_mut::<A11yRelations>(menu)
        && rel.active_descendant != Some(item)
    {
        rel.active_descendant = Some(item);
    }
}

/// Close `menu` by flipping its controlling [`MenuButton`]'s [`A11yExpanded`] to
/// `false` — the SAME disclosure set-verb path the AT `Collapse` and the
/// click-toggle use, so [`sync_menu_open`] does the actual hide + active-descendant
/// clear + focus restoration. The controlling button is the one whose
/// `A11yRelations.controls` references this menu (the `MenuButton::new` edge).
///
/// Finding the button by its `controls` edge (rather than carrying a back-pointer
/// on the menu) keeps the menu free of a redundant relation — the `controls` edge
/// is the single source of the button↔menu link. If no controlling button is found
/// (a standalone `Menu` with no `MenuButton`), the menu is hidden directly so
/// Escape still closes it.
fn close_menu(world: &mut World, menu: Entity) {
    // Find the controlling button (the one whose `controls` references this menu).
    let mut button = None;
    let mut q = world.query_filtered::<(Entity, &A11yRelations), With<MenuButton>>();
    for (e, rel) in q.iter(world) {
        if rel.controls.first() == Some(&menu) {
            button = Some(e);
            break;
        }
    }
    if let Some(button) = button {
        if let Some(mut expanded) = world.get_mut::<A11yExpanded>(button)
            && expanded.0
        {
            expanded.0 = false; // sync_menu_open reacts: hide + clear + restore focus.
        }
    } else {
        // No controlling button — hide the menu directly (standalone Menu).
        if let Some(mut vis) = world.get_mut::<CssVisibility>(menu)
            && *vis != CssVisibility::Hidden
        {
            *vis = CssVisibility::Hidden;
        }
        if let Some(mut rel) = world.get_mut::<A11yRelations>(menu) {
            rel.active_descendant = None;
        }
    }
}
