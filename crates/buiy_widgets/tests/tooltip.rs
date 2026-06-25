//! Wave-3 slice-5 — Tooltip-trigger widget: the P1d a11y SHAPE (the trigger is a
//! focusable `A11yRole::Generic` carrying `A11yTooltipHost` + `A11yLabel`, with
//! `A11yRelations.described_by = [tooltip]`, and the tooltip a real
//! `A11yRole::Tooltip` node starting `CssVisibility::Hidden`) plus the minimal
//! show/hide honor (ShowTooltip → Visible, HideTooltip → Hidden) and pick-through
//! (`Pickable::IGNORE` on the decorative tooltip node).
//!
//! The advertised verb set + the generic Show/HideTooltip honor over the router
//! seam are asserted at the `buiy_core` layer (`a11y_action` / `a11y_inprocess` /
//! `a11y_translate`); here the bundle SHAPE + the `described_by` wiring + the
//! end-to-end show/hide (driven through the headless `dispatch_action_request`
//! seam over the WIRED widget) are exercised. There is NO placement / auto-show
//! timing (C5, Wave 4) — only the static a11y shape + the `CssVisibility` toggle.

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    a11y::translate::node_id_for,
    a11y::{A11yLabel, A11yRelations, A11yRole, A11yTooltipHost, hide_tooltip, show_tooltip},
    components::Node,
    focus::Focusable,
    layout::BoxModel,
    render::components::{Background, Border, CssVisibility},
    text::Text,
};
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::tooltip::{TooltipNode, TooltipTrigger};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);
    app
}

fn child_with<C: Component>(app: &App, root: Entity) -> Entity {
    let world = app.world();
    world
        .get::<Children>(root)
        .unwrap()
        .iter()
        .find(|&c| world.get::<C>(c).is_some())
        .expect("expected child carrying the marker")
}

// ---------------------------------------------------------------------------
// The P1d bundle contract.
// ---------------------------------------------------------------------------

#[test]
fn bare_tooltip_trigger_marker_materializes_the_full_required_contract() {
    let mut app = app();
    let t = app.world_mut().spawn(TooltipTrigger).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Node>(t).is_some(), "Node");
    assert!(world.get::<BoxModel>(t).is_some(), "BoxModel");
    assert!(world.get::<Background>(t).is_some(), "Background");
    assert!(world.get::<Border>(t).is_some(), "Border");
    assert!(world.get::<Focusable>(t).is_some(), "Focusable");
    assert_eq!(
        world.get::<A11yRole>(t).copied(),
        Some(A11yRole::Generic),
        "the trigger role is a NEUTRAL Generic (no role contract; the tooltip verbs \
         ride A11yTooltipHost, not the role — so NO Click)"
    );
    assert!(
        world.get::<A11yTooltipHost>(t).is_some(),
        "A11yTooltipHost present (the state-keyed ShowTooltip/HideTooltip capability)"
    );
    assert!(world.get::<A11yLabel>(t).is_some(), "A11yLabel");
}

#[test]
fn tooltip_trigger_new_spawns_tooltip_and_wires_described_by() {
    let mut app = app();
    let t = app
        .world_mut()
        .spawn(TooltipTrigger::new("Help", "More info here"))
        .id();
    app.update();

    // The AT name stays on the trigger root.
    assert_eq!(
        app.world().get::<A11yLabel>(t).map(|l| l.0.clone()),
        Some("Help".to_string()),
        "the accessible name stays on the trigger root"
    );

    let children = app
        .world()
        .get::<Children>(t)
        .expect("trigger has children")
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(
        children.len(),
        2,
        "the visible trigger glyph child + the controlled tooltip child"
    );
    // One child is the visible trigger glyph `Text` (the rendered icon); the AT
    // name stays on the root. It is NOT the TooltipNode.
    let glyph = children
        .iter()
        .copied()
        .find(|&c| app.world().get::<TooltipNode>(c).is_none())
        .expect("a visible glyph child");
    assert_eq!(
        app.world().get::<Text>(glyph).map(|t| t.0.clone()),
        Some("Help".to_string()),
        "the trigger renders its visible glyph label"
    );

    let tooltip = child_with::<TooltipNode>(&app, t);
    let world = app.world();

    // The tooltip is a real Tooltip node carrying the tip pixels, starting hidden.
    assert_eq!(
        world.get::<A11yRole>(tooltip).copied(),
        Some(A11yRole::Tooltip),
        "the tooltip node is an A11yRole::Tooltip"
    );
    assert_eq!(
        world.get::<Text>(tooltip).map(|t| t.0.clone()),
        Some("More info here".to_string()),
        "the tooltip child carries the visible tip pixels"
    );
    assert_eq!(
        world.get::<CssVisibility>(tooltip).copied(),
        Some(CssVisibility::Hidden),
        "the tooltip starts hidden"
    );
    // The decorative tooltip is Pickable::IGNORE (pick-through).
    assert_eq!(
        world.get::<Pickable>(tooltip).copied(),
        Some(Pickable::IGNORE),
        "the decorative tooltip carries Pickable::IGNORE"
    );

    // The described_by relation was wired to the tooltip.
    let relations = world
        .get::<A11yRelations>(t)
        .expect("the trigger carries A11yRelations after wiring");
    assert_eq!(
        relations.described_by,
        vec![tooltip],
        "the trigger's A11yRelations.described_by references the tooltip"
    );
}

// ---------------------------------------------------------------------------
// End-to-end show/hide: the router honor flips the wired tooltip's CssVisibility.
// ---------------------------------------------------------------------------

#[test]
fn show_tooltip_then_hide_tooltip_flips_the_wired_tooltip_visibility() {
    let mut app = app();
    let t = app
        .world_mut()
        .spawn(TooltipTrigger::new("Help", "More info here"))
        .id();
    app.update();
    let tooltip = child_with::<TooltipNode>(&app, t);

    // Hidden by default.
    assert_eq!(
        app.world().get::<CssVisibility>(tooltip).copied(),
        Some(CssVisibility::Hidden),
    );

    // ShowTooltip on the WIRED trigger reveals its described tooltip — driven
    // through the headless `show_tooltip` driver sugar (the SAME seam the AT/agent
    // path uses). The honor mutates `CssVisibility` synchronously on the live
    // component (asserted directly; the auto-snapshot returns an empty tree under
    // this `CorePlugin`-only harness, which has no `A11yTreeBuilder`).
    show_tooltip(app.world_mut(), node_id_for(t))
        .expect("ShowTooltip honored on the wired trigger");
    assert_eq!(
        app.world().get::<CssVisibility>(tooltip).copied(),
        Some(CssVisibility::Visible),
        "ShowTooltip on the wired trigger flips the tooltip Hidden → Visible"
    );

    // HideTooltip hides it again.
    hide_tooltip(app.world_mut(), node_id_for(t))
        .expect("HideTooltip honored on the wired trigger");
    assert_eq!(
        app.world().get::<CssVisibility>(tooltip).copied(),
        Some(CssVisibility::Hidden),
        "HideTooltip on the wired trigger flips the tooltip Visible → Hidden"
    );
}
