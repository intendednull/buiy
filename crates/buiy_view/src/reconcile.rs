//! The reconciler — a `Changed<Model>`-gated exclusive system that calls
//! `view(&model)`, diffs the returned [`Element`] tree against the retained
//! Buiy widget entities, and patches / spawns / despawns to match.
//!
//! FW1 shipped the **positional** reconciler (match children by index); FW2 adds
//! the **keyed** reconcile ([`reconcile_keyed_children`] — match / reorder rows
//! by [`RowKey`] without rebuild) and the two stateful-leaf widgets. **F2** adds
//! the one coherent **layout surface** (spec §2.2) — the whole
//! sizing/flex/spacing/positioning/scroll set lowered here from [`LayoutProps`],
//! the `raster()` element, and the controlled stick-to-bottom
//! ([`stick_scroll_to_bottom`]). Every lowering is a `set_if_neq` / `!=`-guarded
//! drift, so an unchanged prop never trips `Changed` and a node with no layout
//! modifier is byte-identical to a freshly-`#[require]`'d `Node`.
//!
//! - **#9 patchable styling / layout.** Containers + the raster node emit
//!   **decomposed** components and the reconciler `set_if_neq`-patches them **in
//!   place** on change. `Node` `#[require]`s the full `Style` decomposition
//!   (`Display`/`FlexParams`/`BoxModel`/`Position`/`Overflow`/`Stacking`/…), so a
//!   freshly-spawned node already carries every layout component at its default;
//!   the reconciler writes only what a prop set. Components `Node` does not
//!   `#[require]` (`FlexItem`, the `ScrollOffset`/`ScrollExtent` bundle, the
//!   internal `StickBottom` marker, `Background`, `Border`, and `Pickable::IGNORE`
//!   for a click-through node — F6) are inserted on demand and removed when their
//!   prop clears — no `RemovedComponents` dependence.
//! - **#11 drift-only writes.** Every write is a `set_if_neq` (or an explicit
//!   `!=` guard), so an unchanged prop never trips `Changed`.
//! - **#12 internal `ViewSlot`.** A realized `Button` records its label-child
//!   `Text` entity **once at spawn**; the label patch reads the slot.
//!
//! The reconciler is scheduled **`.before(BuiySet::Layout)`** (#10) by
//! [`crate::app`] so a structurally-new node is laid out the same frame it is
//! created (no unlaid-out flash).

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::a11y::{A11yLabel, A11yRole, A11yToggled, Toggled};
use buiy_core::components::Node;
use buiy_core::layout::{
    AlignItems, BoxModel, Display, Edges, FlexAxis, FlexGap, FlexItem, FlexParams, FlexWrap, Inset,
    JustifyContent, Length, Overflow, OverflowMode, Position, PositionKind, Rotate, ScrollOffset,
    Sizing, Stacking, TopLayer, Translate,
};
use buiy_core::mvu::{ControlledLeaf, Envelope, Model, ToggleMsg};
use buiy_core::render::RasterImage;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{
    Background, Border, BorderSide, BoxShadow, Corners, Icon, Opacity, Shadow, TextColor,
};
use buiy_core::scroll::ScrollExtent;
use buiy_core::text::edit::{EditCommand, PendingProgrammaticEdit, Placeholder, TextEditState};
use buiy_core::text::{
    FontFamily, FontSize, FontStack, FontWeight, SharedFontSystem, Text, TextAlign as CoreTextAlign,
};
use buiy_widgets::{Button, Checkbox, TextInput};

use crate::app::{UiRoot, ViewFn};
use crate::element::{Element, Kind};
use crate::interaction::{InteractionState, PressEffect};
use crate::layout::{Align, Justify, LayoutProps, Positioning, Sides, TextAlign};
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

/// Internal marker: this scroll container carries the model's stick-to-bottom
/// intent (spec §2.2). Written by the reconciler from `LayoutProps.stick`
/// (insert/remove, drift-only), consumed by [`stick_scroll_to_bottom`] AFTER
/// layout — pinning `ScrollOffset.y` to the content's max only while present.
#[derive(Component, Default)]
pub(crate) struct StickBottom;

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
/// style / layout writes tripped. Router-handler (re)attach is layout-inert and
/// NOT counted. Children bump themselves (they are reconciled by their own
/// `patch_node` calls).
fn patch_node<M: Model>(world: &mut World, entity: Entity, el: &Element<M::Msg>, model: Entity) {
    let mut changed = false;
    match el.kind {
        Kind::Text => {
            if let Some(t) = &el.text {
                changed |= set_text(world, entity, t);
            }
            changed |= set_font_size(world, entity, el.font_size);
            // F3: explicit ink / family / weight on the text node.
            changed |= set_text_color(world, entity, el.color);
            changed |= set_font_family(world, entity, el.font_family.as_ref());
            changed |= set_font_weight(world, entity, el.font_weight);
            changed |= set_text_align(world, entity, el.layout.text_align);
            // A text node is a plain `Node` (no widget contract), so the whole
            // layout surface applies to it too (`.width`/`.grow`/`.fixed`/…).
            changed |= apply_node_layout(world, entity, &el.layout, false);
        }
        Kind::Button => {
            if let Some(t) = &el.text {
                changed |= set_button_label(world, entity, t);
            }
            update_press::<M>(world, entity, el, model);
            // The interaction-state visual layer (spec §2.6 part 3) — a button
            // dips while held. The route already lives on the widget's `#[require]`
            // A11yRole; only the press VISUAL is added here.
            update_press_visual(world, entity, el.on_press.is_some() && !el.disabled);
            changed |= update_disabled(world, entity, el.disabled);
            // F3: styled button (fill/radius/border/shadow/size + label style),
            // gated so an unstyled button keeps every widget default.
            changed |= apply_button_style(world, entity, el);
        }
        Kind::Icon => {
            // F3: the vector icon + its optional tinted-badge paint (background +
            // border/radius + shadow) + the node layout (size). An icon node is a
            // plain `Node`, so `apply_node_layout` is safe (no widget contract).
            changed |= set_icon(world, entity, el);
            changed |= apply_background(world, entity, el.background);
            changed |= apply_border(world, entity, el);
            changed |= apply_shadow(world, entity, el);
            changed |= apply_node_layout(world, entity, &el.layout, false);
        }
        Kind::Checkbox => {
            // Controlled: re-assert the leaf `A11yToggled` from the model
            // (drift-only), and refresh the toggle route (`f(!checked)` stamped
            // as a `PressAction`).
            changed |= set_checkbox_checked(world, entity, el.checked);
            update_press::<M>(world, entity, el, model);
        }
        Kind::TextInput => {
            // The controlled model→editor push — SKIPPED while an out-of-band AT /
            // probe `SetValue` is pending fold (`PendingProgrammaticEdit`), so this
            // front-of-frame reconcile does not clobber that un-folded edit before
            // `route_text_input` reads it (the rebuilding-screen race). Keyboard
            // edits fold same-frame and never carry the marker, so live typing and
            // the post-submit clear (an already-consumed editor) are unaffected.
            if world.get::<PendingProgrammaticEdit>(entity).is_none() {
                changed |= set_editor_value(world, entity, el.value.as_deref().unwrap_or(""));
            }
            // The placeholder is a controlled prop too — re-patch it (drift-only) so
            // a phase-driven prompt updates on a rebuild, not only at spawn.
            changed |= set_placeholder(world, entity, el.placeholder.as_deref().unwrap_or(""));
            update_text_actions::<M>(world, entity, el, model);
        }
        Kind::Column | Kind::Row => {
            changed |= apply_container_props(world, entity, el);
            // A clickable container (pick-word tiles) routes its `on_press` — the
            // click bubbles from a child that intercepted the hit (spec §2.6).
            apply_pressable::<M>(world, entity, el, model);
            reconcile_children::<M>(world, entity, &el.children, model, el.keyed);
        }
        Kind::Raster => {
            // Patch the sampled image in place BY IDENTITY (entity preserved — the
            // canvas keeps its texture across unrelated re-renders) + its layout.
            changed |= set_raster_image(world, entity, el.raster.as_ref());
            changed |= apply_node_layout(world, entity, &el.layout, false);
            // A pressable raster (the custom-avatar seat chip) becomes activatable.
            apply_pressable::<M>(world, entity, el, model);
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
        // `insert` returns `false` on a collision: the app's `key_fn` returned the SAME key for
        // two rows. That silently corrupts reconciliation — both rows resolve to one `by_key`
        // entity (the second reconcile clobbers the first's content) and that entity is pushed to
        // `ordered` twice, so `replace_children` gets a duplicate. Guard it loudly in dev/test (a
        // `key_fn` bug the author must fix), free in release: the `insert` runs unconditionally and
        // `fresh` stays used via the `cfg!` short-circuit, so only the panic path is debug-only.
        let fresh = new_keys.insert(key);
        if cfg!(debug_assertions) && !fresh {
            panic!(
                "keyed_column: duplicate key {key} — key_fn must return a UNIQUE key per row \
                 (a collision silently corrupts reconciliation)"
            );
        }
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
            let e = world
                .spawn((
                    Node,
                    Text(el.text.clone().unwrap_or_default()),
                    FontSize(el.font_size),
                    Kind::Text,
                ))
                .id();
            // F3: explicit ink / family / weight, if the author set one.
            set_text_color(world, e, el.color);
            set_font_family(world, e, el.font_family.as_ref());
            set_font_weight(world, e, el.font_weight);
            set_text_align(world, e, el.layout.text_align);
            apply_node_layout(world, e, &el.layout, false);
            e
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
            // The interaction-state visual layer (spec §2.6 part 3) — press-down.
            update_press_visual(world, e, el.on_press.is_some() && !el.disabled);
            update_disabled(world, e, el.disabled);
            // F3: a styled button — the fill / radius / border / shadow / size on
            // the button entity, the label color / font / weight on its slot child.
            apply_button_style(world, e, el);
            e
        }
        Kind::Checkbox => {
            // The real stateful-leaf Checkbox. Seed its `A11yToggled` from the
            // model via the builder's `.checked(bool)` setter (Track C / C4), so a
            // seeded-done item renders checked on frame 1 without waiting for a
            // fold. NOTE: the builder now CARRIES `A11yToggled`, so a sibling
            // `A11yToggled(..)` in this tuple would be a duplicate component (a
            // runtime panic) — the setter is the only correct seed path.
            // `ControlledLeaf` (design §3 #16): the view OWNS this checkbox — its `A11yToggled`
            // is driven by the model (press → `PressAction` → model → the reconciler re-asserts
            // via `set_checkbox_checked`), so it opts OUT of `advance_toggle_on_press`'s
            // press-to-toggle leaf. Without this marker the press would ALSO fold the leaf
            // directly (the double-fold): correct (it converges via drift-reassert) but not
            // cleanly view-owned. The single ordered `ToggleLeafSet::Drain` stays the sole
            // writer; this makes the model the sole *source*.
            let e = world
                .spawn((
                    Checkbox::new("").checked(el.checked),
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
        Kind::Raster => {
            // A layout `Node` carrying F1's `RasterImage` (fixed-size textured
            // quad). `Node` gives it `ResolvedLayout` + `GlobalTransform` so
            // extract can size/place the sampled image; the app owns + paints it.
            let handle = el.raster.clone().unwrap_or_default();
            let e = world.spawn((Node, RasterImage(handle), Kind::Raster)).id();
            apply_node_layout(world, e, &el.layout, false);
            // A pressable raster (the custom-avatar seat chip) becomes activatable.
            apply_pressable::<M>(world, e, el, model);
            e
        }
        Kind::Icon => {
            // A layout `Node` carrying the vector `Icon` (F3). The icon paints
            // centered in the node's box at its native `size_px`; a `.background()`
            // + `.radius()` on the same node makes the tinted badge under it (the
            // fill quad below the icon coverage tier).
            let e = world.spawn((Node, icon_component(el), Kind::Icon)).id();
            apply_background(world, e, el.background);
            apply_border(world, e, el);
            apply_shadow(world, e, el);
            apply_node_layout(world, e, &el.layout, false);
            e
        }
        Kind::Column | Kind::Row => {
            let e = world.spawn((Node, el.kind)).id();
            apply_container_props(world, e, el);
            apply_pressable::<M>(world, e, el, model);
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
// The coherent layout lowering (spec §2.2). Containers + the raster node share
// `apply_node_layout` (sizing / flex-item / position / scroll / stacking / the
// stick marker); a container additionally lowers its `FlexParams` (direction +
// justify + align + wrap + gap) and paints (background / radius). Every write is
// `set_if_neq` / `!=`-guarded, so a node with no layout modifier is a no-op.
// ---------------------------------------------------------------------------

/// Compute + patch a **container**'s decomposed layout + paint components. Only
/// ever touches container-owned components — never a widget's `#[require]`'d
/// contract (the §3 #12 suppression-gotcha guard). Returns whether any component
/// really changed, so the caller counts the node in `nodes_patched` only on real
/// drift.
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
    // FlexParams (container-only): direction + main/cross alignment + wrap + gap.
    changed |= apply_flex_params(world, e, &el.layout, axis);
    // The structural auto-ignore (F6, spec §2.7): a **transparent** (no painted
    // fill) `.top_layer()` container is auto-`Pickable::IGNORE`d so it can never
    // sit topmost and swallow every click beneath it (the invisible-occluder bug
    // class, shipped 3×). Two gates make it correct alongside F3 + F5:
    //   * "Transparent" == paints no visible fill: no `background`, OR F3's explicit
    //     fully-transparent `Color::Custom(_,_,_,0)` (the semantic facade tokens are
    //     all opaque, so only an alpha-0 `Custom` reads as transparent-via-color).
    //     See [`is_transparent_fill`].
    //   * `on_press.is_none()` — an INTERACTIVE container (F5's container press
    //     route, `apply_pressable`) IS a hit target, so it must never be
    //     auto-ignored (that would swallow its own clicks). `apply_pressable` keys
    //     on the same `on_press` and relies on the DEFAULT `Pickable`, which our
    //     `Pickable::IGNORE` insert would clobber — hence the gate.
    let auto_ignore =
        el.layout.top_layer && is_transparent_fill(el.background) && el.on_press.is_none();
    // The node-layout common to containers + raster (sizing/flex-item/position/
    // scroll/stacking/stick + picking transparency).
    changed |= apply_node_layout(world, e, &el.layout, auto_ignore);
    // Paint (containers only): fill + border/radius + shadow (F3).
    changed |= apply_background(world, e, el.background);
    changed |= apply_border(world, e, el);
    changed |= apply_shadow(world, e, el);
    changed
}

/// The node-level layout shared by every `Node`-bearing kind (containers + the
/// raster element): the box model (sizing + padding + the center-self margin),
/// the flex item (grow/shrink), positioning (kind + inset), scroll
/// (overflow + the runtime scroll bundle), the top-layer escape, the
/// stick-to-bottom marker, and picking transparency. Returns whether anything
/// really changed.
///
/// `auto_ignore` is the container-level structural rule (a transparent top-layer
/// container, [`apply_container_props`]); it ORs with the explicit
/// `.ignore_picking()` flag so a single writer drives `Pickable::IGNORE`. A
/// non-container caller (a `text`/`raster` node) passes `false` — only its own
/// `.ignore_picking()` applies (a top-layer `text`/`raster` paints content, so it
/// is not the invisible-occluder class the auto-rule guards).
fn apply_node_layout(
    world: &mut World,
    e: Entity,
    layout: &LayoutProps,
    auto_ignore: bool,
) -> bool {
    let mut changed = false;
    changed |= apply_box_model(world, e, layout);
    changed |= apply_flex_item(world, e, layout);
    changed |= apply_position(world, e, layout);
    changed |= apply_scroll(world, e, layout);
    changed |= apply_top_layer(world, e, layout.top_layer);
    changed |= apply_stick_marker(world, e, layout.stick);
    changed |= apply_picking(world, e, layout.ignore_picking || auto_ignore);
    changed |= apply_rotate(world, e, layout.rotate);
    changed
}

/// Drive a node's 2D `.rotate(deg)` transform (F4b, spec §2.2 `.rotate` row). The
/// core transform chain already exists — `Rotate(Quat)` → the layout compose
/// sub-pass → `ResolvedTransform` → the render bridge → `GlobalTransform`'s 2D
/// linear part → `PackedInstance.affine` (confetti proved it cheap: `Rotate`
/// forms only a stacking context, never an `EffectGroup`). `Node` does NOT
/// `#[require]` `Rotate`, so it is **inserted on demand** (only when a non-zero
/// angle is asked) and toggling back writes identity (drift-only — kept present,
/// no `RemovedComponents`), mirroring [`apply_flex_item`].
///
/// The pivot is the element's center (`transform-origin`'s default). **Picking
/// caveat:** the picking AABB is translation-anchored and does NOT model rotation
/// (`layout/systems.rs`), so a large rotation on an interactive node mis-hits —
/// `.rotate()` is a decoration modifier (confetti / the ribbon / a small tile
/// tilt); a heavily-rotated interactive element wants `.ignore_picking()`.
fn apply_rotate(world: &mut World, e: Entity, rotate: Option<f32>) -> bool {
    let radians = rotate.unwrap_or(0.0).to_radians();
    let want = Quat::from_rotation_z(radians);
    if let Some(mut r) = world.get_mut::<Rotate>(e) {
        if r.0 != want {
            r.0 = want;
            return true;
        }
        false
    } else if radians != 0.0 {
        world.entity_mut(e).insert(Rotate(want));
        true
    } else {
        false
    }
}

/// `set_if_neq`-patch a container's `FlexParams` (direction + justify + align +
/// wrap + the `FlexGap`, all fields of `FlexParams`). Containers only.
fn apply_flex_params(world: &mut World, e: Entity, layout: &LayoutProps, axis: FlexAxis) -> bool {
    let Some(mut fp) = world.get_mut::<FlexParams>(e) else {
        return false;
    };
    let gap = layout.gap.unwrap_or(0.0);
    let mut want = *fp;
    want.direction = axis;
    want.justify_content = lower_justify(layout.justify);
    want.align_items = lower_align(layout.align);
    want.wrap = if layout.wrap {
        FlexWrap::Wrap
    } else {
        FlexWrap::NoWrap
    };
    want.gap = FlexGap {
        row: Length::Px(gap),
        column: Length::Px(gap),
    };
    fp.set_if_neq(want)
}

/// `set_if_neq`-patch the box model (sizing + per-side padding + the center-self
/// margin). Clones the current `BoxModel` and overrides only the fields the view
/// owns, so a `border`/`box_sizing`/`aspect_ratio` set elsewhere (F3) survives.
fn apply_box_model(world: &mut World, e: Entity, layout: &LayoutProps) -> bool {
    let Some(mut bm) = world.get_mut::<BoxModel>(e) else {
        return false;
    };
    let mut want = bm.clone();
    want.width = sizing_axis(layout.width, layout.fill_width);
    want.height = sizing_axis(layout.height, layout.fill_height);
    want.min_width = opt_len_sizing(layout.min_width);
    want.min_height = opt_len_sizing(layout.min_height);
    want.max_width = opt_len_sizing(layout.max_width);
    want.max_height = opt_len_sizing(layout.max_height);
    want.padding = padding_edges(&layout.padding);
    want.margin = center_self_margin(layout);
    bm.set_if_neq(want)
}

/// Drive a flex child's main-axis `grow` + `shrink`. `Node` does NOT `#[require]`
/// `FlexItem`, so it is **inserted on demand** (only when a non-default grow /
/// shrink is asked); toggling back to the default writes `grow = 0` /
/// `shrink = 1` (drift-only — kept present, no `RemovedComponents`). Returns
/// whether it changed.
fn apply_flex_item(world: &mut World, e: Entity, layout: &LayoutProps) -> bool {
    let want_grow = layout.grow;
    let want_shrink = layout.shrink;
    let needs = want_grow != 0.0 || want_shrink != 1.0;
    if let Some(mut fi) = world.get_mut::<FlexItem>(e) {
        let mut changed = false;
        if fi.grow != want_grow {
            fi.grow = want_grow;
            changed = true;
        }
        if fi.shrink != want_shrink {
            fi.shrink = want_shrink;
            changed = true;
        }
        changed
    } else if needs {
        world.entity_mut(e).insert(FlexItem {
            grow: want_grow,
            shrink: want_shrink,
            ..Default::default()
        });
        true
    } else {
        false
    }
}

/// `set_if_neq`-patch `Position` (kind + inset). `.fixed()`/`.absolute()` with no
/// explicit inset on an axis default that axis's START edge to `0`, so `.fixed()`
/// pins to the viewport origin `(0,0)` regardless of root padding (Taffy insets a
/// fixed/absolute child by the containing block's BORDER only — root padding is
/// excluded for an explicit inset; guarded by
/// `buiy_core` `fixed_explicit_zero_inset_ignores_root_padding`).
fn apply_position(world: &mut World, e: Entity, layout: &LayoutProps) -> bool {
    let want = Position {
        kind: lower_position_kind(layout.position),
        inset: resolve_inset(layout),
    };
    if let Some(mut pos) = world.get_mut::<Position>(e) {
        pos.set_if_neq(want)
    } else {
        false
    }
}

/// Drive a node's overflow modes (`.scroll_x`/`.scroll_y`) + the opt-in runtime
/// scroll bundle. `Overflow` is `#[require]`'d (patched via `set_if_neq`);
/// `ScrollOffset` + `ScrollExtent` are NOT (the extent cache queries `&mut
/// ScrollExtent`, and the scroll input owns `ScrollOffset`), so the reconciler
/// **inserts them on a scroll container** and **removes them when scrolling is
/// turned off** — tracking the flag exactly. Returns whether anything changed.
fn apply_scroll(world: &mut World, e: Entity, layout: &LayoutProps) -> bool {
    let mut changed = apply_overflow(world, e, layout);
    let scrolling = layout.scroll_x || layout.scroll_y;
    let has_bundle = world.get::<ScrollOffset>(e).is_some();
    if scrolling && !has_bundle {
        world
            .entity_mut(e)
            .insert((ScrollOffset::default(), ScrollExtent::default()));
        changed = true;
    } else if !scrolling && has_bundle {
        world.entity_mut(e).remove::<ScrollOffset>();
        world.entity_mut(e).remove::<ScrollExtent>();
        changed = true;
    }
    changed
}

/// `set_if_neq`-patch the `Overflow` axis modes. Clones the current value so the
/// scrollbar-* fields are preserved; only `x`/`y` are the view's to own.
fn apply_overflow(world: &mut World, e: Entity, layout: &LayoutProps) -> bool {
    let Some(mut ov) = world.get_mut::<Overflow>(e) else {
        return false;
    };
    let mut want = ov.clone();
    want.x = if layout.scroll_x {
        OverflowMode::Scroll
    } else {
        OverflowMode::Visible
    };
    want.y = if layout.scroll_y {
        OverflowMode::Scroll
    } else {
        OverflowMode::Visible
    };
    ov.set_if_neq(want)
}

/// Drive a container's top-layer escape (`.top_layer()` ⇒ `Stacking.top_layer =
/// Popover`). Drift-only (`Stacking` is `#[require]`'d, so `get_mut` + `!=`).
fn apply_top_layer(world: &mut World, e: Entity, top_layer: bool) -> bool {
    let want = if top_layer {
        TopLayer::Popover
    } else {
        TopLayer::None
    };
    if let Some(mut st) = world.get_mut::<Stacking>(e)
        && st.top_layer != want
    {
        st.top_layer = want;
        return true;
    }
    false
}

/// Whether a container's background paints NO visible fill (F6, spec §2.7) — no
/// `background` at all, or F3's explicit fully-transparent `Color::Custom(_,_,_,0)`.
/// The semantic facade tokens (`Surface`, `Accent`, …) are all opaque, so only an
/// alpha-0 `Custom` reads as transparent-via-color. The auto-ignore rule keys on
/// this so neither the "no background" nor the "explicitly transparent" spelling of
/// a `.top_layer()` container can occlude picks.
fn is_transparent_fill(bg: Option<crate::tokens::Color>) -> bool {
    matches!(bg, None | Some(crate::tokens::Color::Custom(_, _, _, 0)))
}

/// Drive a node's pointer transparency (F6, spec §2.7): `Pickable::IGNORE` when
/// `want_ignore` — the node stops being a hit-target AND stops occluding picks
/// beneath it, while its interactive CHILDREN (separate entities carrying their
/// own `Pickable`) stay pickable. `want_ignore` folds the explicit
/// `.ignore_picking()` with the reconciler's transparent-top-layer auto-rule.
///
/// `Node` does NOT `#[require]` `Pickable`, so it is inserted on demand and
/// removed when the flag clears (the reconciler owns these container/text/raster
/// entities, so `Pickable`'s presence tracks this decision exactly). The remove
/// is guarded on the present value being OUR `IGNORE` marker, so a blocking
/// `Pickable` set by some other system (e.g. a future interactive container) is
/// never stripped. Drift-only — returns whether it changed.
fn apply_picking(world: &mut World, e: Entity, want_ignore: bool) -> bool {
    let present = world.get::<Pickable>(e).copied();
    match (want_ignore, present) {
        (true, Some(p)) if p == Pickable::IGNORE => false,
        (true, _) => {
            world.entity_mut(e).insert(Pickable::IGNORE);
            true
        }
        // Only reclaim an IGNORE we own; leave any other `Pickable` untouched.
        (false, Some(p)) if p == Pickable::IGNORE => {
            world.entity_mut(e).remove::<Pickable>();
            true
        }
        (false, _) => false,
    }
}

/// Insert/remove the internal [`StickBottom`] marker from the model's stick
/// intent (drift-only). Consumed by [`stick_scroll_to_bottom`] post-layout.
fn apply_stick_marker(world: &mut World, e: Entity, stick: bool) -> bool {
    let present = world.get::<StickBottom>(e).is_some();
    if stick && !present {
        world.entity_mut(e).insert(StickBottom);
        true
    } else if !stick && present {
        world.entity_mut(e).remove::<StickBottom>();
        true
    } else {
        false
    }
}

/// Patch a raster node's sampled image in place (drift-only). Compares the
/// `Handle<Image>` (`RasterImage` is not `PartialEq`, so `set_if_neq` cannot be
/// used) and rewrites only on a real change — so an unrelated model fold never
/// re-uploads the texture, and the canvas entity is preserved. Returns whether
/// the handle changed.
fn set_raster_image(world: &mut World, entity: Entity, handle: Option<&Handle<Image>>) -> bool {
    let Some(handle) = handle else {
        return false;
    };
    if let Some(mut cur) = world.get_mut::<RasterImage>(entity) {
        if &cur.0 != handle {
            cur.0 = handle.clone();
            return true;
        }
        false
    } else {
        world.entity_mut(entity).insert(RasterImage(handle.clone()));
        true
    }
}

// --- Lowering helpers (view intents → the decomposed layout types) ----------

/// Lower a per-axis sizing intent: an explicit `.width`/`.height` px wins; else
/// `.fill*` maps to `100%` of the containing block; else `Auto` (content-sized).
fn sizing_axis(explicit: Option<f32>, fill: bool) -> Sizing {
    match explicit {
        Some(px) => Sizing::Length(Length::Px(px)),
        None if fill => Sizing::Length(Length::Percent(100.0)),
        None => Sizing::Auto,
    }
}

/// Lower a min/max sizing intent: an explicit px, else `Auto` (the layout
/// default — no constraint).
fn opt_len_sizing(px: Option<f32>) -> Sizing {
    match px {
        Some(v) => Sizing::Length(Length::Px(v)),
        None => Sizing::Auto,
    }
}

/// Lower per-side padding: an unset side resolves to `0`.
fn padding_edges(sides: &Sides) -> Edges {
    Edges {
        top: Length::Px(sides.top.unwrap_or(0.0)),
        right: Length::Px(sides.right.unwrap_or(0.0)),
        bottom: Length::Px(sides.bottom.unwrap_or(0.0)),
        left: Length::Px(sides.left.unwrap_or(0.0)),
    }
}

/// The negative half-size margin that centers an absolutely-positioned box at its
/// containing block's center (paired with the 50%/50% inset from [`resolve_inset`]).
/// An axis with no explicit size degrades to `0` (corner-at-50% placement) — set
/// an explicit `.width()`/`.height()` for exact centering. `Edges::default()`
/// (all-zero) when not centering, so the write is a no-op for a normal node.
fn center_self_margin(layout: &LayoutProps) -> Edges {
    if !layout.center_self {
        return Edges::default();
    }
    let mx = layout.width.map(|w| -w / 2.0).unwrap_or(0.0);
    let my = layout.height.map(|h| -h / 2.0).unwrap_or(0.0);
    Edges {
        top: Length::Px(my),
        left: Length::Px(mx),
        ..Default::default()
    }
}

fn lower_position_kind(p: Positioning) -> PositionKind {
    match p {
        Positioning::Static => PositionKind::Static,
        Positioning::Relative => PositionKind::Relative,
        Positioning::Absolute => PositionKind::Absolute,
        Positioning::Fixed => PositionKind::Fixed,
    }
}

/// Resolve the [`Inset`] for a positioned box. `Static`/`Relative` carry none.
/// `center_self` uses 50%/50% (the half-size margin does the centering).
/// Otherwise an explicit-per-side inset, with a per-axis default to the START
/// edge (`0`) when neither side of an axis is set — so `.fixed()` pins to the
/// viewport origin and a bare `.absolute()` to its containing block's top-left.
fn resolve_inset(layout: &LayoutProps) -> Inset {
    if matches!(layout.position, Positioning::Static | Positioning::Relative) {
        return Inset::default();
    }
    if layout.center_self {
        return Inset {
            top: len_pct(50.0),
            left: len_pct(50.0),
            ..Default::default()
        };
    }
    let s = &layout.inset;
    let mut inset = Inset::default();
    match (s.top, s.bottom) {
        (None, None) => inset.top = len_px(0.0),
        (t, b) => {
            if let Some(t) = t {
                inset.top = len_px(t);
            }
            if let Some(b) = b {
                inset.bottom = len_px(b);
            }
        }
    }
    match (s.left, s.right) {
        (None, None) => inset.left = len_px(0.0),
        (l, r) => {
            if let Some(l) = l {
                inset.left = len_px(l);
            }
            if let Some(r) = r {
                inset.right = len_px(r);
            }
        }
    }
    inset
}

fn len_px(px: f32) -> Sizing {
    Sizing::Length(Length::Px(px))
}

fn len_pct(pct: f32) -> Sizing {
    Sizing::Length(Length::Percent(pct))
}

/// Map the view [`Justify`] facade to the layout `JustifyContent` (all 6 values).
fn lower_justify(j: Justify) -> JustifyContent {
    match j {
        Justify::Start => JustifyContent::FlexStart,
        Justify::Center => JustifyContent::Center,
        Justify::End => JustifyContent::FlexEnd,
        Justify::Between => JustifyContent::SpaceBetween,
        Justify::Around => JustifyContent::SpaceAround,
        Justify::Evenly => JustifyContent::SpaceEvenly,
    }
}

/// Map the view [`Align`] facade to the layout `AlignItems`.
fn lower_align(a: Align) -> AlignItems {
    match a {
        Align::Start => AlignItems::FlexStart,
        Align::Center => AlignItems::Center,
        Align::End => AlignItems::FlexEnd,
        Align::Stretch => AlignItems::Stretch,
    }
}

/// Map the view [`TextAlign`] facade to the layout engine's `TextAlign`.
fn lower_text_align(a: TextAlign) -> CoreTextAlign {
    match a {
        TextAlign::Start => CoreTextAlign::Start,
        TextAlign::Center => CoreTextAlign::Center,
        TextAlign::End => CoreTextAlign::End,
        TextAlign::Justify => CoreTextAlign::Justify,
    }
}

/// Insert / patch (or remove) a `Text` node's inline `TextAlign` (drift-only).
fn set_text_align(world: &mut World, e: Entity, align: Option<TextAlign>) -> bool {
    match align {
        Some(a) => {
            let want = lower_text_align(a);
            if let Some(mut cur) = world.get_mut::<CoreTextAlign>(e) {
                cur.set_if_neq(want)
            } else {
                world.entity_mut(e).insert(want);
                true
            }
        }
        None => world.entity_mut(e).take::<CoreTextAlign>().is_some(),
    }
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

/// The resolved per-corner radius for a node: `.radius_corners(..)` (the design's
/// asymmetric wobble) takes precedence over the uniform `.radius(..)` token, else
/// `None` (square).
fn resolve_corners<Msg>(el: &Element<Msg>) -> Option<Corners> {
    if let Some([tl, tr, br, bl]) = el.radius_corners {
        Some(crate::tokens::corners_from_px(tl, tr, br, bl))
    } else {
        el.radius.map(|r| r.to_corners())
    }
}

/// Patch a node's `Border` (per-side outline + per-corner radius) and its
/// layout-owned border WIDTH (`BoxModel.border`) in place (F3, drift-only).
///
/// The `Border` component carries the per-side color/style + the corner radius;
/// the WIDTH lives on `BoxModel.border` (a Taffy input). `.border(w, c, style)`
/// sets both; `.radius(..)` / `.radius_corners(..)` alone set the corners with no
/// painting side — which draws no band and, via the borderless-rounded-fill path
/// (F3 `ExtractedNode.radius`), rounds the background FILL. The width is written
/// ONLY when `.border(..)` is set, so a radius-only patch never zeroes a styled
/// widget's own border width. Returns whether anything really changed. A
/// `.radius(..)`-only container reproduces the pre-F3 `Border { radius, ..default }`
/// byte-for-byte (default sides).
fn apply_border<Msg>(world: &mut World, e: Entity, el: &Element<Msg>) -> bool {
    let mut changed = false;
    let corners = resolve_corners(el);
    // Layout-owned border width — only touched when `.border(..)` is set.
    if let Some((w, _, _)) = el.border
        && let Some(mut bm) = world.get_mut::<BoxModel>(e)
    {
        let want = Edges::all(w);
        if bm.border != want {
            bm.border = want;
            changed = true;
        }
    }
    // The `Border` component: painting sides (from `.border`) + corners.
    if el.border.is_some() || corners.is_some() {
        let side = match el.border {
            Some((_, c, style)) => BorderSide {
                color: c.to_token(),
                style,
            },
            None => BorderSide::default(),
        };
        let want = Border {
            top: side.clone(),
            right: side.clone(),
            bottom: side.clone(),
            left: side,
            radius: corners.unwrap_or(Corners::ZERO),
        };
        if let Some(mut cur) = world.get_mut::<Border>(e) {
            changed |= cur.set_if_neq(want);
        } else {
            world.entity_mut(e).insert(want);
            changed = true;
        }
    } else {
        changed |= world.entity_mut(e).take::<Border>().is_some();
    }
    changed
}

/// Patch (or remove) a node's `BoxShadow` from its `.shadow(..)` terms (F3,
/// drift-only). Front-to-back CSS paint order (index 0 frontmost); every term is
/// outset (`inset: false`). Empty ⇒ remove. Returns whether the list changed.
fn apply_shadow<Msg>(world: &mut World, e: Entity, el: &Element<Msg>) -> bool {
    if el.shadows.is_empty() {
        return world.entity_mut(e).take::<BoxShadow>().is_some();
    }
    let want = BoxShadow(
        el.shadows
            .iter()
            .map(|s| Shadow {
                color: s.color.to_token(),
                offset_x: Length::px(s.dx),
                offset_y: Length::px(s.dy),
                blur: Length::px(s.blur),
                spread: Length::px(s.spread),
                inset: false,
            })
            .collect(),
    );
    if let Some(mut cur) = world.get_mut::<BoxShadow>(e) {
        cur.set_if_neq(want)
    } else {
        world.entity_mut(e).insert(want);
        true
    }
}

/// Patch (or remove) an explicit `TextColor` on a text / label entity (F3,
/// drift-only). `None` removes the override so the node falls back to the theme
/// ink (`CurrentColor`). Returns whether the color really changed.
fn set_text_color(world: &mut World, entity: Entity, color: Option<crate::tokens::Color>) -> bool {
    match color {
        Some(c) => {
            let want = TextColor(c.to_token());
            if let Some(mut cur) = world.get_mut::<TextColor>(entity) {
                cur.set_if_neq(want)
            } else {
                world.entity_mut(entity).insert(want);
                true
            }
        }
        None => world.entity_mut(entity).take::<TextColor>().is_some(),
    }
}

/// Patch (or remove) an explicit `FontFamily` on a text / label entity (F3,
/// drift-only). `None` removes it so the node falls back to the default sans.
fn set_font_family(world: &mut World, entity: Entity, family: Option<&FontStack>) -> bool {
    match family {
        Some(stack) => {
            if let Some(cur) = world.get::<FontFamily>(entity)
                && &cur.0 == stack
            {
                return false;
            }
            world.entity_mut(entity).insert(FontFamily(stack.clone()));
            true
        }
        None => world.entity_mut(entity).take::<FontFamily>().is_some(),
    }
}

/// Patch (or remove) an explicit `FontWeight` on a text / label entity (F3,
/// drift-only) — the variable-font weight axis the shaper already threads. `None`
/// removes it so the node renders at the family's default instance.
fn set_font_weight(
    world: &mut World,
    entity: Entity,
    weight: Option<crate::tokens::Weight>,
) -> bool {
    match weight {
        Some(w) => {
            let want = FontWeight(w.value());
            if let Some(mut cur) = world.get_mut::<FontWeight>(entity) {
                cur.set_if_neq(want)
            } else {
                world.entity_mut(entity).insert(want);
                true
            }
        }
        None => world.entity_mut(entity).take::<FontWeight>().is_some(),
    }
}

/// Build the `Icon` component from a [`Kind::Icon`] element's props (F3). Always
/// stroked (round cap/join); `.color(..)` sets the stroke tint, else the theme
/// ink (`CurrentColor`). The `viewbox` carries the author coordinate space.
fn icon_component<Msg>(el: &Element<Msg>) -> Icon {
    Icon {
        path_d: el.icon_path.clone().unwrap_or_default(),
        stroke_width: el.icon_stroke_width,
        size_px: el.icon_size_px,
        viewbox: el.icon_viewbox,
        fill: false,
        color: el
            .color
            .map(|c| c.to_token())
            .unwrap_or(ColorToken::CurrentColor),
    }
}

/// Patch an icon node's `Icon` in place (F3, drift-only — `Icon` is `PartialEq`).
/// Returns whether the icon really changed.
fn set_icon<Msg>(world: &mut World, e: Entity, el: &Element<Msg>) -> bool {
    let want = icon_component(el);
    if let Some(mut cur) = world.get_mut::<Icon>(e) {
        cur.set_if_neq(want)
    } else {
        world.entity_mut(e).insert(want);
        true
    }
}

/// Style a `Button` (F3): the fill / radius / border / shadow / per-axis size /
/// grow on the button entity, and the label color / font / weight / size on its
/// recorded `ViewSlot` child. Each style applies **only when the author set it**,
/// so an unstyled `button("x")` is a complete no-op here — it keeps every
/// `buiy_widgets::Button` default (its fill, rounding, padding, label size), the
/// §3 #12 suppression safety that keeps the counter / gallery goldens byte-
/// identical. A per-axis size preserves the button's default on the axis the
/// author did not set (never zeroing its `#[require]` box). Returns whether
/// anything really changed.
fn apply_button_style<Msg>(world: &mut World, e: Entity, el: &Element<Msg>) -> bool {
    let mut changed = false;
    if el.background.is_some() {
        changed |= apply_background(world, e, el.background);
    }
    if el.border.is_some() || el.radius.is_some() || el.radius_corners.is_some() {
        changed |= apply_border(world, e, el);
    }
    if !el.shadows.is_empty() {
        changed |= apply_shadow(world, e, el);
    }
    // Per-axis fixed size — set only the axis the author asked for (never padding:
    // a button owns its own inner padding), preserving the button's `#[require]`
    // default on the untouched axis.
    let wants_w = el.layout.width.is_some() || el.layout.fill_width;
    let wants_h = el.layout.height.is_some() || el.layout.fill_height;
    if (wants_w || wants_h)
        && let Some(mut bm) = world.get_mut::<BoxModel>(e)
    {
        let mut want = bm.clone();
        if wants_w {
            want.width = sizing_axis(el.layout.width, el.layout.fill_width);
        }
        if wants_h {
            want.height = sizing_axis(el.layout.height, el.layout.fill_height);
        }
        changed |= bm.set_if_neq(want);
    }
    changed |= apply_flex_item(world, e, &el.layout);
    // Label styling on the slot child — applied only when the button is EXPLICITLY
    // styled (a fill / color / font / weight set), so an unstyled `button("x")`
    // keeps every widget default incl. its label size (the shared-crate goldens).
    let styled = el.background.is_some()
        || el.color.is_some()
        || el.font_family.is_some()
        || el.font_weight.is_some();
    let slot = world.get::<ViewSlot>(e).and_then(|s| s.label);
    if let (true, Some(child)) = (styled, slot) {
        if el.color.is_some() {
            changed |= set_text_color(world, child, el.color);
        }
        if el.font_family.is_some() {
            changed |= set_font_family(world, child, el.font_family.as_ref());
        }
        if el.font_weight.is_some() {
            changed |= set_font_weight(world, child, el.font_weight);
        }
        changed |= set_font_size(world, child, el.font_size);
    }
    changed
}

// ---------------------------------------------------------------------------
// The controlled stick-to-bottom system (spec §2.2 finding #3).
// ---------------------------------------------------------------------------

/// Pin a stuck scroll container to the bottom (spec §2.2 controlled
/// stick-to-bottom). Runs AFTER `update_scroll_extent` (post-layout), so the
/// just-appended content's extent is fresh: for each [`StickBottom`] container
/// with a valid extent, drift-set `ScrollOffset.y` to the max offset. Clearing
/// the model's stick intent removes the marker (the reconciler), so the pin
/// stops and a scrolled-away offset is left where the user (the scroll input)
/// put it. Writing only `ScrollOffset` keeps `ResolvedLayout` valid (the
/// scroll-O(0) invariant, `ScrollOffset` is excluded from the `sync_styles`
/// trigger). No-op while no container sticks (the `With<StickBottom>` filter).
pub(crate) fn stick_scroll_to_bottom(
    mut q: Query<(&ScrollExtent, &mut ScrollOffset), With<StickBottom>>,
) {
    for (extent, mut offset) in &mut q {
        if !extent.valid {
            // The extent cache has not run yet (spawn frame, or no scroll
            // pipeline) — do not pin to a not-yet-known zero max.
            continue;
        }
        let max_y = extent.max_offset().y;
        if offset.y != max_y {
            offset.y = max_y;
        }
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

/// Give a **non-widget node** — a clickable container ([`Kind::Column`]/[`Kind::Row`])
/// or a pressable [`raster`](crate::raster) — the button activation contract when
/// it carries an [`on_press`](Element::on_press) (spec §2.6 parts 1 & 2). A `button`
/// already owns this contract via its `#[require]`; a bare container/raster does not,
/// so it is stamped by hand:
/// - `A11yRole::Button` — the activatable role BOTH the pointer producer
///   (`pointer_click_emits_on_press`) and the AT/probe path (`Action::Click` on the
///   role-keyed `Button` contract) gate on. A container's children intercept the
///   pointer hit and carry no role, so the role must live on the container itself;
///   the click reaches it by the child→parent `Pointer<Click>` propagation (the
///   producer fires for the activatable ancestor). The same role serves touch.
/// - `A11yLabel` — the accessible name from [`Element::label`] (reuses the `text`
///   slot), so the node is locatable by role+name and announced.
/// - `PressAction` — the value the router enqueues, via [`update_press`] (the
///   identical route a `button` uses).
///
/// Also installs the [interaction-state visual layer](crate::interaction) so the
/// node dips while held. Drift-safe: a node that stops being pressable (disabled or
/// handler removed) has the whole contract stripped, so it never keeps a stale
/// button role.
fn apply_pressable<M: Model>(world: &mut World, e: Entity, el: &Element<M::Msg>, model: Entity) {
    let pressable = el.on_press.is_some() && !el.disabled;
    if pressable {
        if world.get::<A11yRole>(e) != Some(&A11yRole::Button) {
            world.entity_mut(e).insert(A11yRole::Button);
        }
        let name = el.text.clone().unwrap_or_default();
        if let Some(mut cur) = world.get_mut::<A11yLabel>(e) {
            if cur.0 != name {
                cur.0 = name;
            }
        } else {
            world.entity_mut(e).insert(A11yLabel(name));
        }
        update_press::<M>(world, e, el, model);
    } else if world.get::<A11yRole>(e).is_some() {
        world
            .entity_mut(e)
            .remove::<A11yRole>()
            .remove::<A11yLabel>()
            .remove::<PressAction<M>>();
    }
    update_press_visual(world, e, pressable);
}

/// Install (or remove) the [interaction-state visual layer](crate::interaction) on
/// a pressable entity: the [`InteractionState`] the pointer observers write, its
/// [`PressEffect`] depth, and the `Translate` the resolver mutates in place. The
/// three are stamped together (once, on the first reconcile a node is pressable), so
/// the resolver only ever mutates an existing `Translate` — the press-down lands the
/// same frame with no deferred insert. All three are layout-inert at rest (identity
/// `Translate`, `InteractionState::None`), so this is NOT counted as a node patch.
///
/// A node that stops being pressable drops the state + effect and resets any
/// lingering press-down to identity.
fn update_press_visual(world: &mut World, e: Entity, pressable: bool) {
    if pressable {
        if world.get::<InteractionState>(e).is_none() {
            world
                .entity_mut(e)
                .insert((InteractionState::default(), PressEffect::default()));
        }
        if world.get::<Translate>(e).is_none() {
            world.entity_mut(e).insert(Translate::default());
        }
    } else if world.get::<InteractionState>(e).is_some() {
        world
            .entity_mut(e)
            .remove::<InteractionState>()
            .remove::<PressEffect>();
        if let Some(mut t) = world.get_mut::<Translate>(e) {
            t.1 = Length::ZERO;
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
/// **Pending-edit guard.** The reconcile calls this only when the editor carries
/// no [`PendingProgrammaticEdit`] (design
/// `docs/specs/2026-07-10-dooduel-controlled-input-setvalue-fold-design.md`): the
/// reconcile runs at the front of the frame (`.before(BuiySet::Layout)`), one leg
/// ahead of the editor→model fold (`route_text_input` in `MvuSet::Enqueue`, late),
/// so on a screen that rebuilds every frame it would otherwise re-assert the STALE
/// model value over an out-of-band AT/probe `SetValue` before that edit folds —
/// destroying it. The marker (set by `honor_text_set_value`, cleared by
/// `route_text_input` when it folds) suppresses exactly that one clobber.
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

/// Re-assert a controlled text-input's placeholder from the view (drift-only, so
/// an unchanged prompt never trips `Changed`). A real change trips the downstream
/// `sync_placeholder` reshape + the `A11yPlaceholder` mirror. Patched every
/// reconcile (unlike the value, it is never a "pending edit") so a phase-driven
/// prompt (`match phase { … }`) updates on a rebuild, not only at spawn — the
/// realized `TextInput` `#[require]`s `Placeholder`, so it is always present.
fn set_placeholder(world: &mut World, entity: Entity, want: &str) -> bool {
    if let Some(mut ph) = world.get_mut::<Placeholder>(entity) {
        if ph.0 != want {
            ph.0 = want.to_string();
            return true;
        }
        false
    } else {
        // Defensive (a realized TextInput always carries `Placeholder`): seed one if
        // a non-empty prompt is wanted.
        if want.is_empty() {
            false
        } else {
            world
                .entity_mut(entity)
                .insert(Placeholder(want.to_string()));
            true
        }
    }
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
        Some(handler) => {
            world.entity_mut(entity).insert(SubmitAction::<M> {
                handler: handler.clone(),
                model,
            });
        }
        None => {
            world.entity_mut(entity).remove::<SubmitAction<M>>();
        }
    }
}
