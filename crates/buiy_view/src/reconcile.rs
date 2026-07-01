//! The reconciler — a `Changed<Model>`-gated exclusive system that calls
//! `view(&model)`, diffs the returned [`Element`] tree against the retained
//! Buiy widget entities, and patches / spawns / despawns to match.
//!
//! PR1 (FW1) ships the **positional** reconciler (match children by index) with
//! the structural refines the prototype deferred:
//!
//! - **#9 patchable styling.** Containers emit **decomposed** components and the
//!   reconciler `set_if_neq`-patches them **in place** on change — `FlexParams`
//!   (direction + `align_items` + the `FlexGap`), `BoxModel` (padding),
//!   `Background`, `Border` (radius). `Node` `#[require]`s the full `Style`
//!   decomposition, so a freshly-spawned container already carries every layout
//!   component at its default; the reconciler writes only what a prop set.
//! - **#11 drift-only writes.** Every write is a `set_if_neq` (or an explicit
//!   `!=` guard), so an unchanged prop never trips `Changed` — the funnel's
//!   `set_if_neq` discipline carries through the reconciler.
//! - **#12 internal `ViewSlot`.** A realized `Button` records its label-child
//!   `Text` entity **once at spawn** ([`ViewSlot`]); the label patch reads the
//!   slot instead of re-walking the widget's children.
//!
//! The reconciler is scheduled **`.before(BuiySet::Layout)`** (#10) by
//! [`crate::app`] so a structurally-new node is laid out the same frame it is
//! created (no unlaid-out flash).

use bevy::prelude::*;
use buiy_core::a11y::A11yLabel;
use buiy_core::components::Node;
use buiy_core::layout::{
    AlignItems, BoxModel, Display, Edges, FlexAxis, FlexGap, FlexParams, Length,
};
use buiy_core::mvu::Model;
use buiy_core::render::components::{Background, Border, Opacity};
use buiy_core::text::{FontSize, Text};
use buiy_widgets::Button;

use crate::app::{UiRoot, ViewFn};
use crate::element::{Element, Kind};
use crate::router::PressAction;

/// Records where a realized widget's patchable content lives, so a label patch
/// reads the slot instead of re-walking the widget's children (spec §3 #12).
/// Stamped by the reconciler at spawn — entirely internal to `buiy_view`; the
/// widget crates are untouched.
#[derive(Component, Default)]
pub(crate) struct ViewSlot {
    /// The widget's visible-label `Text` child (a `Button`'s label), if any.
    pub(crate) label: Option<Entity>,
}

/// The reconciler system: diff `view(&model)` against the retained tree and
/// patch / spawn / despawn to match. Exclusive `&mut World` (it spawns real
/// widget entities immediately so they are queryable this frame).
///
/// An exclusive system cannot use a `Changed<M>` run-condition, so idle-frame
/// cheapness comes from an internal `Changed<M>` emptiness early-out (spec §3
/// #10 "Scheduling caveat").
pub(crate) fn reconcile<M: Model>(world: &mut World) {
    // Which model changed this frame? (Frame 1: the `Startup`-spawned model is
    // `Changed`, so the initial tree is built here.) Empty ⇒ idle frame ⇒ return.
    let (model_entity, model) = {
        let mut q = world.query_filtered::<(Entity, &M), Changed<M>>();
        match q.iter(world).next() {
            Some((e, m)) => (e, m.clone()),
            None => return,
        }
    };

    let view = world.resource::<ViewFn<M>>().view;
    let tree = view(&model);

    let prev = world.resource::<UiRoot<M>>().root;
    let root = match prev {
        Some(r) if world.get_entity(r).is_ok() => {
            reconcile_node::<M>(world, r, &tree, model_entity)
        }
        _ => spawn_node::<M>(world, &tree, model_entity),
    };
    world.resource_mut::<UiRoot<M>>().root = Some(root);
}

/// Patch `entity` toward `el` if their kinds match; otherwise despawn + respawn.
/// Returns the (possibly new) entity so parents can re-thread `Children`.
fn reconcile_node<M: Model>(
    world: &mut World,
    entity: Entity,
    el: &Element<M::Msg>,
    model: Entity,
) -> Entity {
    if world.get::<Kind>(entity).copied() != Some(el.kind) {
        // Kind changed (or not one of ours) — the retained entity can't be
        // reused. Despawn the subtree and build fresh.
        world.entity_mut(entity).despawn();
        return spawn_node::<M>(world, el, model);
    }
    patch_node::<M>(world, entity, el, model);
    entity
}

/// Patch an entity of the SAME kind in place (the "reuse, don't rebuild" path).
fn patch_node<M: Model>(world: &mut World, entity: Entity, el: &Element<M::Msg>, model: Entity) {
    match el.kind {
        Kind::Text => {
            if let Some(t) = &el.text {
                set_text(world, entity, t);
            }
            set_font_size(world, entity, el.font_size);
        }
        Kind::Button => {
            if let Some(t) = &el.text {
                set_button_label(world, entity, t);
            }
            update_press::<M>(world, entity, el, model);
            update_disabled(world, entity, el.disabled);
        }
        Kind::Column | Kind::Row => {
            apply_container_props(world, entity, el);
            reconcile_positional_children::<M>(world, entity, &el.children, model);
        }
    }
}

/// Positional diff: match existing children to new elements **by index**. A
/// front-insert or reorder churns everything below it — the reason the keyed
/// `keyed_column` (FW2) exists.
fn reconcile_positional_children<M: Model>(
    world: &mut World,
    parent: Entity,
    children_el: &[Element<M::Msg>],
    model: Entity,
) {
    let existing: Vec<Entity> = world
        .get::<Children>(parent)
        .map(|c| c.to_vec())
        .unwrap_or_default();

    let mut ordered = Vec::with_capacity(children_el.len());
    for (i, child_el) in children_el.iter().enumerate() {
        let e = match existing.get(i) {
            Some(&old) => reconcile_node::<M>(world, old, child_el, model),
            None => spawn_node::<M>(world, child_el, model),
        };
        ordered.push(e);
    }
    // Despawn any retained children the new tree no longer has.
    for &old in existing.iter().skip(children_el.len()) {
        world.entity_mut(old).despawn();
    }
    world.entity_mut(parent).replace_children(&ordered);
}

/// Build a fresh entity subtree from `el`, reusing the real widget/primitive
/// constructors. Uses immediate `world.spawn` so the entities are queryable
/// this frame.
fn spawn_node<M: Model>(world: &mut World, el: &Element<M::Msg>, model: Entity) -> Entity {
    match el.kind {
        Kind::Text => {
            // `Node` makes the text node layout-visible (a bare `Text` without
            // `Node` is silently skipped by layout — the widget-catalog lesson).
            world
                .spawn((
                    Node,
                    Text(el.text.clone().unwrap_or_default()),
                    FontSize(el.font_size),
                    Kind::Text,
                ))
                .id()
        }
        Kind::Button => {
            // Reuse the real `buiy_widgets` Button constructor (full Phase-0
            // contract: focus + a11y + a centered, pick-through label `Text`).
            let label = el.text.clone().unwrap_or_default();
            let e = world.spawn((Button::new(label), Kind::Button)).id();
            // #12: record the label-child slot ONCE (found here, not re-walked
            // on every later patch).
            let slot = ViewSlot {
                label: find_label_child(world, e),
            };
            world.entity_mut(e).insert(slot);
            update_press::<M>(world, e, el, model);
            update_disabled(world, e, el.disabled);
            e
        }
        Kind::Column | Kind::Row => {
            let e = world.spawn((Node, el.kind)).id();
            apply_container_props(world, e, el);
            let kids: Vec<Entity> = el
                .children
                .iter()
                .map(|c| spawn_node::<M>(world, c, model))
                .collect();
            world.entity_mut(e).replace_children(&kids);
            e
        }
    }
}

// ---------------------------------------------------------------------------
// #9 — decomposed-style patching (containers only, `set_if_neq` in place).
// ---------------------------------------------------------------------------

/// Compute + `set_if_neq`-patch the decomposed layout/paint components a
/// container's props map to. `Node`'s `#[require]` already put `Display` /
/// `FlexParams` / `BoxModel` on the entity at their defaults, so this only
/// writes what a prop actually changes (spec §3 #9 / #11).
///
/// Only ever touches container-owned layout/paint components — never a widget's
/// `#[require]`'d contract (the §3 #12 suppression-gotcha guard).
fn apply_container_props<Msg>(world: &mut World, e: Entity, el: &Element<Msg>) {
    let axis = match el.kind {
        Kind::Row => FlexAxis::Row,
        _ => FlexAxis::Column,
    };

    // Display: flex row/column.
    if let Some(mut d) = world.get_mut::<Display>(e) {
        d.set_if_neq(Display::Flex(axis));
    }

    // FlexParams: direction + cross-axis alignment + the flex gap (the `FlexGap`
    // is a field of `FlexParams`, not a standalone component).
    if let Some(mut fp) = world.get_mut::<FlexParams>(e) {
        let gap = el.gap.unwrap_or(0.0);
        let mut want = *fp;
        want.direction = axis;
        want.align_items = if el.align_center {
            AlignItems::Center
        } else {
            AlignItems::Stretch
        };
        want.gap = FlexGap {
            row: Length::Px(gap),
            column: Length::Px(gap),
        };
        fp.set_if_neq(want);
    }

    // BoxModel: inner padding.
    if let Some(mut bm) = world.get_mut::<BoxModel>(e) {
        let pad = el.padding.unwrap_or(0.0);
        let mut want = bm.clone();
        want.padding = Edges::all(pad);
        bm.set_if_neq(want);
    }

    apply_background(world, e, el.background);
    apply_radius(world, e, el.radius);
}

/// Patch (or remove) the container's `Background` fill in place.
fn apply_background(world: &mut World, e: Entity, bg: Option<crate::tokens::Color>) {
    match bg {
        Some(c) => {
            let want = Background {
                color: c.to_token(),
            };
            if let Some(mut cur) = world.get_mut::<Background>(e) {
                cur.set_if_neq(want);
            } else {
                world.entity_mut(e).insert(want);
            }
        }
        None => {
            world.entity_mut(e).remove::<Background>();
        }
    }
}

/// Patch (or remove) the container's rounded-corner `Border` in place.
fn apply_radius(world: &mut World, e: Entity, r: Option<crate::tokens::Radius>) {
    match r {
        Some(radius) => {
            let want = Border {
                radius: radius.to_corners(),
                ..Default::default()
            };
            if let Some(mut cur) = world.get_mut::<Border>(e) {
                cur.set_if_neq(want);
            } else {
                world.entity_mut(e).insert(want);
            }
        }
        None => {
            world.entity_mut(e).remove::<Border>();
        }
    }
}

// ---------------------------------------------------------------------------
// Leaf patch helpers (drift-only writes, #11).
// ---------------------------------------------------------------------------

/// Set a `Text` node's content iff it differs (never trips `Changed` on a no-op).
fn set_text(world: &mut World, entity: Entity, s: &str) {
    if let Some(mut t) = world.get_mut::<Text>(entity)
        && t.0 != s
    {
        t.0 = s.to_string();
    }
}

/// Set a `Text` node's font size iff it differs.
fn set_font_size(world: &mut World, entity: Entity, px: f32) {
    if let Some(mut f) = world.get_mut::<FontSize>(entity)
        && f.0 != px
    {
        f.0 = px;
    }
}

/// The label-child `Text` of a `Button::new(..)` (its centered, pick-through
/// visible label). Found ONCE at spawn and recorded in [`ViewSlot`].
fn find_label_child(world: &mut World, button: Entity) -> Option<Entity> {
    let kids: Vec<Entity> = world
        .get::<Children>(button)
        .map(|c| c.to_vec())
        .unwrap_or_default();
    kids.into_iter().find(|&c| world.get::<Text>(c).is_some())
}

/// A `Button`'s visible label is its `ViewSlot`-recorded child `Text`; its
/// accessible name is the root `A11yLabel`. Patch both in place — no child
/// re-walk (spec §3 #12).
fn set_button_label(world: &mut World, button: Entity, label: &str) {
    if let Some(mut a) = world.get_mut::<A11yLabel>(button)
        && a.0 != label
    {
        a.0 = label.to_string();
    }
    let slot = world.get::<ViewSlot>(button).and_then(|s| s.label);
    if let Some(child) = slot {
        set_text(world, child, label);
    }
}

/// Attach/refresh (or remove) the press handler that the router reads (#11:
/// insert only carries a real value; a disabled/handler-less element removes it).
fn update_press<M: Model>(world: &mut World, entity: Entity, el: &Element<M::Msg>, model: Entity) {
    match &el.on_press {
        Some(msg) if !el.disabled => {
            world.entity_mut(entity).insert(PressAction::<M> {
                msg: msg.clone(),
                model,
            });
        }
        _ => {
            world.entity_mut(entity).remove::<PressAction<M>>();
        }
    }
}

/// Dim a disabled interactive element (evidence in the PNG; the routing is
/// already suppressed by [`update_press`]). Drift-only.
fn update_disabled(world: &mut World, entity: Entity, disabled: bool) {
    let want = if disabled { 0.4 } else { 1.0 };
    if let Some(mut o) = world.get_mut::<Opacity>(entity) {
        if o.0 != want {
            o.0 = want;
        }
    } else {
        world.entity_mut(entity).insert(Opacity(want));
    }
}
