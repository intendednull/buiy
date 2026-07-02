//! The reconciler — a `Changed<Model>`-gated exclusive system that calls
//! `view(&model)`, diffs the returned [`Element`] tree against the retained
//! Buiy widget entities, and patches / spawns / despawns to match.
//!
//! FW1 shipped the **positional** reconciler (match children by index); FW2 adds
//! the **keyed** reconcile ([`reconcile_keyed_children`] — match / reorder rows
//! by [`RowKey`] without rebuild) and the two stateful-leaf widgets (the
//! controlled `Checkbox` leaf + the command-sourced `TextInput` editor, driven
//! drift-only from the model). Both rest on the structural refines the prototype
//! deferred:
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
use buiy_core::a11y::{A11yLabel, A11yToggled, Toggled};
use buiy_core::components::Node;
use buiy_core::layout::{
    AlignItems, BoxModel, Display, Edges, FlexAxis, FlexGap, FlexParams, Length,
};
use buiy_core::mvu::{ControlledLeaf, Envelope, Model, ToggleMsg};
use buiy_core::render::components::{Background, Border, Opacity};
use buiy_core::text::edit::{EditCommand, TextEditState};
use buiy_core::text::{FontSize, SharedFontSystem, Text};
use buiy_widgets::{Button, Checkbox, TextInput};

use crate::app::{UiRoot, ViewFn};
use crate::element::{Element, Kind};
use crate::router::{InputAction, PressAction, SubmitAction};

/// Per-frame reconciler work counts — the host-independent measurement gate for
/// the view surface (spec §5 #14, modeled on `buiy_core::mvu::MvuWorkCounters` /
/// `render::RenderWorkCounters`). A settled app asserts these EXACTLY, identical
/// on any CPU, so a re-introduced rebuild-storm reddens on a slow runner just as
/// on the dev box.
///
/// **Overwrite convention (mirrors `MvuWorkCounters`).** All fields are RESET to
/// 0 at the TOP of the reconcile each frame (before the `Changed<M>` early-out),
/// then accumulated by that same reconcile pass — so the values read after
/// `app.update()` describe THIS frame only. An idle frame reads all-0 (the
/// reconcile early-outs before touching a field).
///
/// **`nodes_patched` counts NODES that received a real value-changing write** — a
/// `set_if_neq` (or `!=`-guarded write) that leaves every component of a node
/// unchanged does **not** increment it. This is the load-bearing rule: a
/// walk-and-write-everything reconciler cannot pass the W4 gate by touching all
/// nodes, because an untripped `set_if_neq` is not counted. Router-handler
/// (re)attach (`PressAction` / `InputAction` / `SubmitAction`) is layout-inert
/// bookkeeping the routers re-read each frame — it is **not** a node patch and is
/// not counted here (it does not feed layout or paint, so it cannot defeat the
/// downstream-bound check).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ViewWorkCounters {
    /// Reconcile passes that actually ran this frame (past the `Changed<M>`
    /// early-out). `0` on an idle frame — the load-bearing proof the funnel's
    /// `set_if_neq` discipline carries through: an idempotent fold leaves
    /// `Changed<M>` untripped, so the reconciler never even runs.
    pub reconciles: u64,
    /// `Element` nodes spawned into fresh entities this frame (`0` on a
    /// patch-in-place value change; `> 0` when a `when`/kind-swap or a keyed
    /// insert realizes new structure).
    pub nodes_spawned: u64,
    /// Retained node subtrees despawned this frame (kind-swap replace, positional
    /// excess, keyed removal). `0` on a patch-in-place value change.
    pub nodes_despawned: u64,
    /// Retained nodes patched in place with a REAL value change this frame (see
    /// the type doc: an untripped `set_if_neq` is NOT counted). Bounded to the
    /// changed subtree — a localized `Inc` patches exactly the one label
    /// (`nodes_patched == 1`), never the whole tree.
    pub nodes_patched: u64,
}

/// Bump [`ViewWorkCounters::nodes_patched`] (a real value-changing write landed
/// on a node). Inert when the counter is unregistered (the `Option<ResMut>`
/// idiom — a harness that does not install it simply does not count).
fn bump_patched(world: &mut World) {
    if let Some(mut c) = world.get_resource_mut::<ViewWorkCounters>() {
        c.nodes_patched += 1;
    }
}

/// Bump [`ViewWorkCounters::nodes_spawned`].
fn bump_spawned(world: &mut World) {
    if let Some(mut c) = world.get_resource_mut::<ViewWorkCounters>() {
        c.nodes_spawned += 1;
    }
}

/// Bump [`ViewWorkCounters::nodes_despawned`].
fn bump_despawned(world: &mut World) {
    if let Some(mut c) = world.get_resource_mut::<ViewWorkCounters>() {
        c.nodes_despawned += 1;
    }
}

/// Records where a realized widget's patchable content lives, so a label patch
/// reads the slot instead of re-walking the widget's children (spec §3 #12).
/// Stamped by the reconciler at spawn — entirely internal to `buiy_view`; the
/// widget crates are untouched.
#[derive(Component, Default)]
pub(crate) struct ViewSlot {
    /// The widget's visible-label `Text` child (a `Button`'s label), if any.
    pub(crate) label: Option<Entity>,
}

/// The keyed-reconcile identity stamp (spec §2 #4): a [`keyed_column`](crate::keyed_column)
/// child's stable key, stamped on its row-root entity so the reconciler can
/// match / reorder rows by key — preserving each row's widget-entity identity +
/// internal state (its `A11yToggled`, its editor buffer) across add / remove /
/// reorder. Entirely internal to `buiy_view`.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RowKey(pub(crate) u64);

/// The reconciler system: diff `view(&model)` against the retained tree and
/// patch / spawn / despawn to match. Exclusive `&mut World` (it spawns real
/// widget entities immediately so they are queryable this frame).
///
/// An exclusive system cannot use a `Changed<M>` run-condition, so idle-frame
/// cheapness comes from an internal `Changed<M>` emptiness early-out (spec §3
/// #10 "Scheduling caveat").
pub(crate) fn reconcile<M: Model>(world: &mut World) {
    // Reset the per-frame work counters BEFORE the early-out (overwrite
    // convention, spec §5 #14): an idle frame that early-outs reads all-0.
    if let Some(mut c) = world.get_resource_mut::<ViewWorkCounters>() {
        *c = ViewWorkCounters::default();
    }

    // Which model changed this frame? (Frame 1: the `Startup`-spawned model is
    // `Changed`, so the initial tree is built here.) Empty ⇒ idle frame ⇒ return.
    let (model_entity, model) = {
        let mut q = world.query_filtered::<(Entity, &M), Changed<M>>();
        match q.iter(world).next() {
            Some((e, m)) => (e, m.clone()),
            None => return,
        }
    };

    // We are reconciling this frame (a real `Changed<M>` reached us).
    if let Some(mut c) = world.get_resource_mut::<ViewWorkCounters>() {
        c.reconciles += 1;
    }

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
        // reused. Despawn the subtree and build fresh. This is the `when`
        // content↔`Empty` swap (and the `if a else b` two-kind swap): a stable
        // slot whose OCCUPANT changes kind, so siblings keep their identity.
        bump_despawned(world);
        world.entity_mut(entity).despawn();
        return spawn_node::<M>(world, el, model);
    }
    patch_node::<M>(world, entity, el, model);
    entity
}

/// Patch an entity of the SAME kind in place (the "reuse, don't rebuild" path).
///
/// Each write helper returns whether it made a REAL value change; a node is
/// counted in [`ViewWorkCounters::nodes_patched`] once iff any of its content /
/// style writes tripped. Router-handler (re)attach is layout-inert and NOT
/// counted (see the counter type doc). Children bump themselves (they are
/// reconciled by their own `patch_node` calls).
fn patch_node<M: Model>(world: &mut World, entity: Entity, el: &Element<M::Msg>, model: Entity) {
    let mut changed = false;
    match el.kind {
        Kind::Text => {
            if let Some(t) = &el.text {
                changed |= set_text(world, entity, t);
            }
            changed |= set_font_size(world, entity, el.font_size);
        }
        Kind::Button => {
            if let Some(t) = &el.text {
                changed |= set_button_label(world, entity, t);
            }
            update_press::<M>(world, entity, el, model);
            changed |= update_disabled(world, entity, el.disabled);
        }
        Kind::Checkbox => {
            // Controlled: re-assert the leaf `A11yToggled` from the model
            // (drift-only), and refresh the toggle route (`f(!checked)` stamped
            // as a `PressAction`).
            changed |= set_checkbox_checked(world, entity, el.checked);
            update_press::<M>(world, entity, el, model);
        }
        Kind::TextInput => {
            changed |= set_editor_value(world, entity, el.value.as_deref().unwrap_or(""));
            update_text_actions::<M>(world, entity, el, model);
        }
        Kind::Column | Kind::Row => {
            changed |= apply_container_props(world, entity, el);
            reconcile_children::<M>(world, entity, &el.children, model, el.keyed);
        }
        // A placeholder holds a slot but has no state to patch (FW3 `when`).
        Kind::Empty => {}
    }
    if changed {
        bump_patched(world);
    }
}

/// Reconcile a container's children — **by key** if the container is a
/// [`keyed_column`](crate::keyed_column) (`keyed == true`), else **by position**
/// (`column!`/`row!`).
fn reconcile_children<M: Model>(
    world: &mut World,
    parent: Entity,
    children_el: &[Element<M::Msg>],
    model: Entity,
    keyed: bool,
) {
    if keyed {
        reconcile_keyed_children::<M>(world, parent, children_el, model);
    } else {
        reconcile_positional_children::<M>(world, parent, children_el, model);
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
        bump_despawned(world);
        world.entity_mut(old).despawn();
    }
    world.entity_mut(parent).replace_children(&ordered);
}

/// Keyed reconcile (spec §2 #4): match existing rows to new elements **by
/// [`RowKey`]**.
/// - a key present in both is **reconciled in place** (entity + all its widget
///   descendants + their internal state are preserved — a reorder or a sibling
///   add/remove never rebuilds it);
/// - a key only in the new tree is **spawned** (and stamped with its `RowKey`);
/// - a key only in the old tree is **despawned**;
/// - the parent's `Children` are then pinned to the new document order, so rows
///   **move** without losing identity.
fn reconcile_keyed_children<M: Model>(
    world: &mut World,
    parent: Entity,
    children_el: &[Element<M::Msg>],
    model: Entity,
) {
    // Existing rows: key → entity.
    let existing: Vec<Entity> = world
        .get::<Children>(parent)
        .map(|c| c.to_vec())
        .unwrap_or_default();
    let mut by_key: std::collections::HashMap<u64, Entity> = std::collections::HashMap::new();
    for &child in &existing {
        if let Some(&RowKey(k)) = world.get::<RowKey>(child) {
            by_key.insert(k, child);
        }
    }

    let mut new_keys: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut ordered = Vec::with_capacity(children_el.len());
    for child_el in children_el {
        let key = child_el
            .key
            .expect("keyed_column child must carry a key (keyed_column sets it)");
        new_keys.insert(key);
        let e = match by_key.get(&key) {
            // Reuse the existing row entity — reconcile it IN PLACE (no rebuild).
            // Re-stamp the key afterward: on a top-level Kind change (e.g. a keyed
            // row entering per-row edit mode) `reconcile_node` despawns+respawns a
            // fresh, KEYLESS entity — without re-inserting `RowKey` the next
            // reconcile can't match it by key, so it rebuilds the row again
            // (discarding just-built widget state) and orphans the intermediate.
            // The insert is idempotent for the survive case and repairs the respawn.
            Some(&old) => {
                let e = reconcile_node::<M>(world, old, child_el, model);
                world.entity_mut(e).insert(RowKey(key));
                e
            }
            // A new key — build it fresh and stamp its identity.
            None => {
                let e = spawn_node::<M>(world, child_el, model);
                world.entity_mut(e).insert(RowKey(key));
                e
            }
        };
        ordered.push(e);
    }

    // Despawn rows whose key vanished.
    for (&k, &old) in by_key.iter() {
        if !new_keys.contains(&k) {
            bump_despawned(world);
            world.entity_mut(old).despawn();
        }
    }
    // Pin the surviving + new rows to document order (the MOVE, without rebuild).
    world.entity_mut(parent).replace_children(&ordered);
}

/// Build a fresh entity subtree from `el`, reusing the real widget/primitive
/// constructors. Uses immediate `world.spawn` so the entities are queryable
/// this frame.
fn spawn_node<M: Model>(world: &mut World, el: &Element<M::Msg>, model: Entity) -> Entity {
    // One `Element` node is being realized into a fresh entity (its container
    // children each bump themselves via the recursive `spawn_node` calls below).
    bump_spawned(world);
    match el.kind {
        // A zero-paint placeholder (FW3 `when`): a bare node that occupies its
        // slot so a hidden child does not shift its siblings' indices. `Node`
        // `#[require]`s the full `Style` at defaults; nothing is painted (no
        // `Text` / `Background`).
        Kind::Empty => world.spawn((Node, Kind::Empty)).id(),
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
        Kind::Checkbox => {
            // The real stateful-leaf Checkbox. Seed its `A11yToggled` from the
            // model (the explicit component overrides the `#[require]` default
            // `False`), so a seeded-done item renders checked on frame 1 without
            // waiting for a fold.
            let want = toggled_of(el.checked);
            // `ControlledLeaf` (design §3 #16): the view OWNS this checkbox — its `A11yToggled`
            // is driven by the model (press → `PressAction` → model → the reconciler re-asserts
            // via `set_checkbox_checked`), so it opts OUT of `advance_toggle_on_press`'s
            // press-to-toggle leaf. Without this marker the press would ALSO fold the leaf
            // directly (the double-fold): correct (it converges via drift-reassert) but not
            // cleanly view-owned. The single ordered `ToggleLeafSet::Drain` stays the sole
            // writer; this makes the model the sole *source*.
            let e = world
                .spawn((
                    Checkbox::new(""),
                    A11yToggled(want),
                    ControlledLeaf,
                    Kind::Checkbox,
                ))
                .id();
            // The toggle route: press → the eagerly-resolved `f(!checked)` value
            // → model (stamped as a `PressAction`, exactly like a button).
            update_press::<M>(world, e, el, model);
            e
        }
        Kind::TextInput => {
            // The real command-sourced single-line editor.
            let placeholder = el.placeholder.clone().unwrap_or_default();
            let e = world
                .spawn((TextInput::single_line(placeholder), Kind::TextInput))
                .id();
            // Seed the controlled value into the editor buffer (no
            // `TextChanged`/`EditLog` — `apply` is the low-level seam, not the
            // recorded keyboard system).
            if let Some(v) = &el.value {
                set_editor_value(world, e, v);
            }
            update_text_actions::<M>(world, e, el, model);
            e
        }
        Kind::Column | Kind::Row => {
            let e = world.spawn((Node, el.kind)).id();
            apply_container_props(world, e, el);
            let kids: Vec<Entity> = el
                .children
                .iter()
                .map(|c| {
                    let child = spawn_node::<M>(world, c, model);
                    // A fresh keyed container stamps its children's identity so a
                    // later reconcile can match them by key.
                    if let Some(k) = c.key {
                        world.entity_mut(child).insert(RowKey(k));
                    }
                    child
                })
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
///
/// Returns whether any component was really changed (every write is `set_if_neq`
/// or an `insert`/`remove` of a component that actually appeared/vanished), so
/// the caller counts this container in `nodes_patched` only on real drift.
fn apply_container_props<Msg>(world: &mut World, e: Entity, el: &Element<Msg>) -> bool {
    let axis = match el.kind {
        Kind::Row => FlexAxis::Row,
        _ => FlexAxis::Column,
    };
    let mut changed = false;

    // Display: flex row/column.
    if let Some(mut d) = world.get_mut::<Display>(e) {
        changed |= d.set_if_neq(Display::Flex(axis));
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
        changed |= fp.set_if_neq(want);
    }

    // BoxModel: inner padding.
    if let Some(mut bm) = world.get_mut::<BoxModel>(e) {
        let pad = el.padding.unwrap_or(0.0);
        let mut want = bm.clone();
        want.padding = Edges::all(pad);
        changed |= bm.set_if_neq(want);
    }

    changed |= apply_background(world, e, el.background);
    changed |= apply_radius(world, e, el.radius);
    changed
}

/// Patch (or remove) the container's `Background` fill in place. Returns whether
/// the fill really changed (drift-only).
fn apply_background(world: &mut World, e: Entity, bg: Option<crate::tokens::Color>) -> bool {
    match bg {
        Some(c) => {
            let want = Background {
                color: c.to_token(),
            };
            if let Some(mut cur) = world.get_mut::<Background>(e) {
                cur.set_if_neq(want)
            } else {
                world.entity_mut(e).insert(want);
                true
            }
        }
        None => world.entity_mut(e).take::<Background>().is_some(),
    }
}

/// Patch (or remove) the container's rounded-corner `Border` in place. Returns
/// whether the radius really changed (drift-only).
fn apply_radius(world: &mut World, e: Entity, r: Option<crate::tokens::Radius>) -> bool {
    match r {
        Some(radius) => {
            let want = Border {
                radius: radius.to_corners(),
                ..Default::default()
            };
            if let Some(mut cur) = world.get_mut::<Border>(e) {
                cur.set_if_neq(want)
            } else {
                world.entity_mut(e).insert(want);
                true
            }
        }
        None => world.entity_mut(e).take::<Border>().is_some(),
    }
}

// ---------------------------------------------------------------------------
// Leaf patch helpers (drift-only writes, #11).
// ---------------------------------------------------------------------------

/// Set a `Text` node's content iff it differs (never trips `Changed` on a
/// no-op). Returns whether it really changed.
fn set_text(world: &mut World, entity: Entity, s: &str) -> bool {
    if let Some(mut t) = world.get_mut::<Text>(entity)
        && t.0 != s
    {
        t.0 = s.to_string();
        return true;
    }
    false
}

/// Set a `Text` node's font size iff it differs. Returns whether it changed.
fn set_font_size(world: &mut World, entity: Entity, px: f32) -> bool {
    if let Some(mut f) = world.get_mut::<FontSize>(entity)
        && f.0 != px
    {
        f.0 = px;
        return true;
    }
    false
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
/// re-walk (spec §3 #12). Returns whether the label really changed (a11y name
/// OR visible text), so the button counts as one patched node.
fn set_button_label(world: &mut World, button: Entity, label: &str) -> bool {
    let mut changed = false;
    if let Some(mut a) = world.get_mut::<A11yLabel>(button)
        && a.0 != label
    {
        a.0 = label.to_string();
        changed = true;
    }
    let slot = world.get::<ViewSlot>(button).and_then(|s| s.label);
    if let Some(child) = slot {
        changed |= set_text(world, child, label);
    }
    changed
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
/// already suppressed by [`update_press`]). Drift-only. Returns whether the
/// opacity really changed.
fn update_disabled(world: &mut World, entity: Entity, disabled: bool) -> bool {
    let want = if disabled { 0.4 } else { 1.0 };
    if let Some(mut o) = world.get_mut::<Opacity>(entity) {
        if o.0 != want {
            o.0 = want;
            return true;
        }
        false
    } else {
        world.entity_mut(entity).insert(Opacity(want));
        true
    }
}

// ---------------------------------------------------------------------------
// FW2 — the two stateful-leaf widgets (checkbox leaf + command-sourced editor).
// ---------------------------------------------------------------------------

fn toggled_of(checked: bool) -> Toggled {
    if checked {
        Toggled::True
    } else {
        Toggled::False
    }
}

/// Re-assert a controlled checkbox's `A11yToggled` from the model. Only writes
/// on a real **drift** (#11), and writes THROUGH the toggle leaf's own inbox
/// (`ToggleMsg::Set`) so the leaf's early drain stays the SOLE writer
/// (single-writer discipline). In the live app the leaf + the model route
/// already agree after a click, so this is a no-op there; it earns its keep on
/// **replay** — re-deriving the visual toggle from the replayed model even when
/// the off-log leaf fold dead-lettered.
fn set_checkbox_checked(world: &mut World, entity: Entity, checked: bool) -> bool {
    let want = toggled_of(checked);
    let current = world.get::<A11yToggled>(entity).map(|t| t.0);
    if current != Some(want) {
        world
            .resource_mut::<Messages<Envelope<A11yToggled>>>()
            .write(Envelope::user(entity, ToggleMsg::Set(checked)));
        return true;
    }
    false
}

/// Set the controlled value of a real editor to `value`, iff it differs from
/// the live content (#11 drift-only — so an in-progress edit the user just typed
/// (already equal) is not clobbered, and no redundant work runs). Uses the
/// low-level `apply` seam (NOT the recorded keyboard system), so it emits no
/// `TextChanged` and writes no `EditLog` entry — the controlled set is invisible
/// to both bridges + the record stream, avoiding a feedback loop / log flood.
///
/// `clear ≠ Insert("")`: an empty insert deletes nothing, so clearing is
/// `SelectAll` + `Delete` (select the whole buffer, then delete the selection).
fn set_editor_value(world: &mut World, entity: Entity, value: &str) -> bool {
    let current = world.get::<TextEditState>(entity).map(|s| s.value());
    if current.as_deref() == Some(value) {
        return false;
    }
    let fonts = world.resource::<SharedFontSystem>().clone();
    let mut fs = fonts.lock();
    let mut changed = false;
    if let Some(mut state) = world.get_mut::<TextEditState>(entity) {
        // Replace the whole buffer: select all, then delete-to-clear or
        // insert-over (single_line = true, read_only = false).
        state.apply(&mut fs, EditCommand::SelectAll, true, false);
        if value.is_empty() {
            state.apply(&mut fs, EditCommand::Delete, true, false);
        } else {
            state.apply(&mut fs, EditCommand::Insert(value.to_string()), true, false);
        }
        changed = true;
    }
    changed
}

/// Attach / refresh (or remove) a text-input's `on_input` / `on_submit` handlers
/// that the editor bridges read (#11: an insert only carries a real handler; a
/// handler-less element removes the component).
fn update_text_actions<M: Model>(
    world: &mut World,
    entity: Entity,
    el: &Element<M::Msg>,
    model: Entity,
) {
    match &el.on_input {
        Some(handler) => {
            world.entity_mut(entity).insert(InputAction::<M> {
                handler: handler.clone(),
                model,
            });
        }
        None => {
            world.entity_mut(entity).remove::<InputAction<M>>();
        }
    }
    match &el.on_submit {
        Some(msg) => {
            world.entity_mut(entity).insert(SubmitAction::<M> {
                msg: msg.clone(),
                model,
            });
        }
        None => {
            world.entity_mut(entity).remove::<SubmitAction<M>>();
        }
    }
}
