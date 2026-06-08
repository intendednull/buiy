//! The per-view extract mapping, factored into pure functions so the
//! device-independent half (skip predicates, `painters_z` ordering, per-entity
//! record build) is unit-testable on CI runners with no wgpu adapter. Color
//! resolution is delegated to the single canonical resolver
//! [`color::resolve_token`](crate::render::color::resolve_token) — this module
//! holds no second token→`Color` mapping. The `extract_buiy_nodes` system
//! (Task 6) is a thin wrapper that calls these.
//!
//! Spec: architecture.md § 1.2/§ 3/§ 4, paint-order-and-top-layer.md § 1/§ 5.

use crate::theme::Theme;
use bevy::prelude::*;

use crate::render::components::CssVisibility;

/// Why the forward paint walk skips an entity (paint-order-and-top-layer.md
/// § 5). `Display::None` is NOT a variant: such entities never reach extract
/// (no `ResolvedLayout`, absent from `painters_z`), so there is nothing to
/// skip — the absence IS the skip. `content-visibility: hidden` is likewise NOT
/// a variant: § 5.2 keeps the Hidden entity's own box painting and prunes its
/// descendants layout-side (they never enter `painters_z`), so render inherits
/// the prune for free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// `CssVisibility::Hidden` — render-owned paint-skip, keep the box (§ 5.4).
    CssHidden,
    /// Off-screen `content-visibility: auto` (the `OffscreenAuto` marker, § 5.3).
    OffscreenAuto,
}

/// Decide whether a `Node` entity should be skipped at paint, and why.
/// `None` => paint normally. Inputs are bound as `Option<&T>` / `bool` exactly
/// as the extract fan binds them. Precedence (first match wins): render-owned
/// `CssVisibility::Hidden`, then `OffscreenAuto`. `content-visibility: hidden`
/// is deliberately NOT consulted here — the Hidden entity's own box paints
/// (§ 5.2) and its descendants are pruned layout-side.
pub fn node_skip_reason(
    css_visibility: Option<&CssVisibility>,
    offscreen_auto: bool,
) -> Option<SkipReason> {
    if matches!(css_visibility, Some(CssVisibility::Hidden)) {
        return Some(SkipReason::CssHidden);
    }
    if offscreen_auto {
        return Some(SkipReason::OffscreenAuto);
    }
    None
}

use crate::components::{Node, ResolvedLayout};
use crate::render::components::{AncestorClip, Background, ClipRect, OffscreenAuto};

/// One painted entity's CPU record — the per-frame instance the per-view
/// `ExtractedNodes` (Task 5) holds, keyed by `Entity` so a partial re-extract
/// patches only changed entities (architecture.md § 3.1). v1 carries the
/// solid-fill quad inputs; shadow/border/glyph fields are added by their tier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtractedNode {
    /// The source main-world entity (the partial-re-extract key).
    pub entity: Entity,
    /// Painted top-left, in logical px — `GlobalTransform.translation.xy`
    /// (pillar 5: render reads the propagated transform, not
    /// `ResolvedLayout.position`).
    pub position: Vec2,
    /// Box size in logical px, from `ResolvedLayout.size`.
    pub size: Vec2,
    /// Resolved background fill (already theme-resolved; `Color::NONE` ==
    /// transparent, extract emits no quad for it downstream).
    pub color: Color,
    /// The per-primitive clip AABB (own border box ∩ ancestor clips, from
    /// `clip_for_primitive`). `None` == the full-view sentinel: no ancestor
    /// clips this entity, OR it is a top-layer member (always `None`, per
    /// paint-order-and-top-layer.md § 3.2). Downstream (`pack_extracted`)
    /// packs `None` to `[±INFINITY]` so the fragment discard never fires.
    pub clip: Option<ClipRect>,
}

/// Build one [`ExtractedNode`] from the layout box + composed transform + the
/// (optional) background token + the (optional) per-primitive clip AABB. Pure:
/// no GPU, no ECS access beyond the borrowed components. `position` is the
/// `GlobalTransform` translation; `size` is `ResolvedLayout.size`; `color`
/// resolves the `Background` token (absent background == transparent); `clip`
/// is carried verbatim (`None` == full-view sentinel, see [`ExtractedNode::clip`]).
pub fn extracted_node_for(
    entity: Entity,
    global_transform: &GlobalTransform,
    layout: &ResolvedLayout,
    background: Option<&Background>,
    clip: Option<&ClipRect>,
    theme: &Theme,
) -> ExtractedNode {
    let translation = global_transform.translation();
    let color = match background {
        Some(bg) => crate::render::color::resolve_token(&bg.color, theme),
        None => Color::NONE,
    };
    ExtractedNode {
        entity,
        position: translation.truncate(),
        size: layout.size,
        color,
        clip: clip.copied(),
    }
}

/// Per-view CPU instance set — the `Changed`-gated per-frame product of
/// extract, stored as a COMPONENT on the per-view render entity (architecture
/// § 4, R8), NOT a global resource, so each window's set is isolated. v1 writes
/// every `Node` into the PRIMARY view's `ExtractedNodes` (architecture § 4,
/// D2); a second window's view runs `BuiyNode` but receives an empty set until
/// the per-window partition is wired.
///
/// **R5 owns this type; R6 consumes it.** Single carrier — there is no parallel
/// `ExtractedNodes`/`ExtractedNodesResource` rebuilt from `ExtractedDraws`.
/// `Default` is MANUAL so `scale_factor` is `1.0` (a derived `Default` would be
/// `0.0` and divide-by-zero the logical→physical map).
#[derive(Component, Clone, Debug)]
pub struct ExtractedNodes {
    /// In `painters_z` forward order (index 0 bottom-most). Never re-sorted by
    /// render (pillar 1); hit-test order is this reversed (paint-order § 2).
    pub nodes: Vec<ExtractedNode>,
    /// The view's logical (CSS-px) size — used by R6 to build the view uniform.
    pub logical_size: Vec2,
    /// Device pixel ratio (logical→physical). `1.0` until the window scale is
    /// wired; the manual `Default` keeps it `1.0`, never `0.0`.
    pub scale_factor: f32,
}

impl Default for ExtractedNodes {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            logical_size: Vec2::ZERO,
            scale_factor: 1.0,
        }
    }
}

/// Walk `painters_z` front-to-back (paint-order § 1) and emit each painter's
/// record, skipping entities for which `build` returns `None` (the skip rules,
/// § 5). Pure: the caller supplies `build` closing over the extract query so
/// this assembler stays device- and ECS-free and unit-testable. Emission order
/// is exactly `painters_z` index order — never a re-sort (pillar 1). The
/// view-level `logical_size` / `scale_factor` are filled by the system that
/// owns the window (Task 6); this assembler only orders the node list.
pub fn assemble_in_paint_order(
    painters_z: &[Entity],
    mut build: impl FnMut(Entity) -> Option<ExtractedNode>,
) -> ExtractedNodes {
    let mut nodes = Vec::with_capacity(painters_z.len());
    for &painter in painters_z {
        if let Some(node) = build(painter) {
            nodes.push(node);
        }
    }
    ExtractedNodes {
        nodes,
        ..Default::default()
    }
}

/// Assemble a whole stacking-context TREE into one flat, paint-ordered node
/// list (paint-order-and-top-layer.md § 1.1). Starting at `root`, this emits the
/// root context's own box, then walks its `painters_z` forward; when a painter
/// is itself a stacking-context root (`painters_z_of` returns `Some`), render
/// **descends into that nested context AT THAT POSITION as a unit** — painting
/// the entire nested context before returning to the parent list — rather than
/// flattening or re-sorting across the boundary.
///
/// This mirrors sub-pass 6f's `painters_of`, which stops descending the moment
/// it hits a child that itself forms a context (`if !forming.contains(&node)`),
/// so each `painters_z` is exactly one context's slice and a nested root appears
/// in its parent's list as a single atomic entry (its descendants live only in
/// its own `painters_z`). A naive flat-concatenation of every context's
/// `painters_z` would instead paint nested descendants at the end of their own
/// context's iteration — the wrong global order. Recursion terminates because
/// the context tree is a finite DAG over distinct entities and a context's
/// `painters_z` never lists the context root itself (§ 1.1).
///
/// Pure: `painters_z_of` resolves an entity to its `StackingContext.painters_z`
/// (or `None` for a non-context painter) and `build` resolves an entity to its
/// record (or `None` to skip, § 5); neither touches the GPU or the ECS beyond
/// the borrowed lookups, so the whole tree walk is unit-testable headless.
pub fn assemble_context_tree<'a>(
    root: Entity,
    painters_z_of: &impl Fn(Entity) -> Option<&'a [Entity]>,
    build: &mut impl FnMut(Entity) -> Option<ExtractedNode>,
    out: &mut Vec<ExtractedNode>,
) {
    // The context root paints its OWN box first (CSS painter's algorithm: the
    // SC root's background/borders sit at the bottom of its context). 6f builds
    // a context's `painters_z` from its DESCENDANTS, excluding the root itself,
    // so the root is emitted here, never via the list below.
    if let Some(node) = build(root) {
        out.push(node);
    }
    let Some(painters) = painters_z_of(root) else {
        return;
    };
    for &painter in painters {
        if painters_z_of(painter).is_some() {
            // Nested SC root: descend as a unit at this position (§ 1.1).
            assemble_context_tree(painter, painters_z_of, build, out);
        } else if let Some(node) = build(painter) {
            out.push(node);
        }
    }
}

use crate::components::StackingContext;
use crate::layout::{Stacking, TopLayer};
use crate::render::clip::clip_for_primitive;
use bevy::render::Extract;
use bevy::window::PrimaryWindow;

/// Resolve one entity's per-primitive clip AABB from its stacking + clip inputs
/// (the decision `extract_buiy_nodes` runs before [`extracted_node_for`]). Pure:
/// `Option<&Stacking>`/`Option<&ClipRect>`/`Option<&AncestorClip>` →
/// `Option<ClipRect>`, no ECS access. A top-layer member (any non-`None`
/// [`TopLayer`]) escapes every ancestor clip and paints over the full view
/// (paint-order-and-top-layer.md § 3.2), so it ALWAYS resolves to the `None`
/// full-view sentinel — even with a (stale) `ClipRect`/`AncestorClip` present.
/// An in-flow member takes its fill clip straight from
/// `clip_for_primitive(false, …)` (own-box ∩ ancestor clips; the
/// `Outline`/`is_outline = true` path is a later tier), which is `None` when
/// nothing clips it.
pub fn effective_clip(
    stacking: Option<&Stacking>,
    clip_rect: Option<&ClipRect>,
    ancestor_clip: Option<&AncestorClip>,
) -> Option<ClipRect> {
    let is_top_layer = stacking.is_some_and(|s| s.top_layer != TopLayer::None);
    if is_top_layer {
        None
    } else {
        clip_for_primitive(false, clip_rect, ancestor_clip)
    }
}

/// Per-frame, `Changed`-gated extract (architecture.md § 1.2/§ 3/§ 4). Reads
/// the main world's layout + render-owned components through `Extract`, walks
/// the primary view's stacking order, and writes the per-view `ExtractedNodes`.
///
/// v1 resolves every `Node` to the PRIMARY window's view (architecture § 4,
/// D2). The query reads ALL windows (reserved per-window structure) so the
/// partition can be turned on without a query change; v1 still targets primary.
///
/// Extraction is one-directional and read-only (pillar 1): it never mutates the
/// main world, never re-sorts `painters_z`, never re-derives stacking/geometry.
// A Bevy extract system reads many independently-tracked inputs (the paint-input
// fan, the damage probe, the despawn stream, the context tree, the theme, the
// window set); splitting them into a bundle param would obscure, not clarify.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn extract_buiy_nodes(
    mut commands: Commands,
    // The author-set + handoff fan: Option<&T> for every independently-inserted
    // component (architecture § 1.2 — a non-Option term would silently drop a
    // Node missing that component). Required terms: &ResolvedLayout (a
    // Display::None entity has no ResolvedLayout and is dropped here) and
    // &GlobalTransform (pillar 5).
    // The FULL node set, UN-gated: when the damage gate below decides this frame
    // is dirty, the whole set is re-extracted so a single changed entity does not
    // drop its unchanged siblings (the changed-only replace was the R5 bug —
    // 2026-06-07-render-extract-retain-damage-design.md). The `Changed`-gating is
    // a *whether-to-rebuild* probe (`changed`/`removed`/`theme` below), not a
    // *what-to-include* filter — building the full set is CPU-cheap; the GPU
    // re-upload is what the gate protects (architecture.md § 3.1).
    nodes: Extract<
        Query<
            (
                Entity,
                &GlobalTransform,
                &ResolvedLayout,
                Option<&Background>,
                Option<&CssVisibility>,
                Option<&OffscreenAuto>,
                // Clip inputs (R8b): the computed per-entity clip AABB + its
                // ancestor-only companion (consumed by `clip_for_primitive`),
                // and `Stacking` so a top-layer member is forced to the
                // full-view sentinel (`clip = None`, paint-order § 3.2).
                Option<&ClipRect>,
                Option<&AncestorClip>,
                Option<&Stacking>,
                // FAN: add Option<&BoxShadow>/&Outline/&Opacity/&EffectGroup/
                // &Border and the reserved effect components here as their tier
                // lands (architecture § 1.2 illustrative subset).
                // NOTE: Containment is NOT in the fan — content-visibility:hidden
                // paints the entity's own box and prunes descendants layout-side
                // (paint-order § 5.2), so it is not a render skip input.
            ),
            With<Node>,
        >,
    >,
    // Damage probe: did any paint input change this frame? Entity-only, gated by
    // the architecture.md § 3.1 trigger union. Non-empty ⇒ a paint value or
    // paint-skip flipped (a `Display::None` transition lands here too, via the
    // zero-size `Changed<ResolvedLayout>` re-write — layout never *removes*
    // `ResolvedLayout`).
    changed: Extract<
        Query<
            Entity,
            (
                With<Node>,
                Or<(
                    Changed<GlobalTransform>,
                    Changed<ResolvedLayout>,
                    Changed<Background>,
                    Changed<CssVisibility>,
                    Changed<OffscreenAuto>,
                    Changed<StackingContext>,
                    Changed<Stacking>,
                    Changed<ClipRect>,
                    Changed<AncestorClip>,
                    // FAN: extend the Or-set in lockstep with the `nodes` tuple
                    // (architecture § 3.1 trigger union).
                )>,
            ),
        >,
    >,
    // Entity despawn — the one damage source the `Changed` probe cannot see (a
    // despawn drops every component, emitting no `Changed`). A `Display::None`
    // hide is NOT here (it keeps `ResolvedLayout`); it rides `changed` above.
    mut removed: Extract<RemovedComponents<ResolvedLayout>>,
    // EVERY forming entity's StackingContext, keyed by its entity. The root
    // context(s) are the ones not listed as a painter in any other context's
    // painters_z; nested-context roots appear as a single atomic entry in their
    // parent's list (paint-order § 1.1). The walk descends the tree from the
    // root(s) — it does NOT flat-concatenate this query in archetype order.
    contexts: Extract<Query<(Entity, &StackingContext)>>,
    theme: Extract<Res<Theme>>,
    // Reserved per-window structure: read ALL windows, not just primary
    // (architecture § 4) — v1 still resolves every Node to the primary view.
    _windows: Extract<Query<(Entity, &Window)>>,
    // The primary window's `Window`: its presence gates extraction (a vanished
    // window clears the carrier, below) AND its resolution fills the per-view
    // `logical_size` / `scale_factor` that build the logical→clip view uniform
    // in prepare (`pack_extracted_nodes`). Reading the resolution here is what
    // the assembler doc ("filled by the system that owns the window") defers to
    // this system: without it `logical_size` stays `Vec2::ZERO` and the view
    // transform divides by zero (`sx = 2/0 = ∞`), collapsing every quad off the
    // GPU — invisible to the CPU-only buffer assertions but fatal on a real
    // adapter (caught by the gate-#2 readback harness).
    primary: Extract<Query<&Window, With<PrimaryWindow>>>,
) {
    // Resolve the primary window's view target entity. v1: all Nodes paint into
    // the primary view (D2). If there is no primary window this frame, overwrite
    // the carrier with an EMPTY set rather than early-returning: Phase-0's
    // `extract_buiy_draws` always `insert_resource`s an `ExtractedDraws` (it
    // never early-returns), so a vanished window clears to empty. Returning here
    // would instead leave the prior frame's nodes resident once the carrier is
    // `init_resource`'d (Task 7), and render would keep painting stale nodes.
    // Drain the despawn stream FIRST, before any early-return, so the
    // `RemovedComponents` cursor advances every frame and events never accumulate
    // (incl. the vanished-window path below). A despawn drops `ResolvedLayout` —
    // the one damage source the `Changed` probe cannot see.
    let despawned = removed.read().count() > 0;

    let Ok(primary_window) = primary.single() else {
        commands.insert_resource(ExtractedNodesView(ExtractedNodes::default()));
        return;
    };

    // Damage gate (architecture.md § 3.1 / 2026-06-07-render-extract-retain-damage
    // -design.md). Rebuild only when something this view paints actually changed:
    // a `Changed` paint input (incl. a paint-skip flip), an entity despawn, or a
    // theme/forced-colors swap (a global token re-resolve that bypasses per-entity
    // change detection — color-and-forced-colors.md § 3). On a steady-state frame,
    // return WITHOUT touching the resource — the prior `ExtractedNodesView` stays
    // resident and `is_changed()` is false in prepare, which retains the persistent
    // buffers (the node re-binds + re-draws them). This is the O(0) steady-state
    // the spec's gate-#14 budget requires.
    if changed.is_empty() && !despawned && !theme.is_changed() {
        return;
    }

    // Build a per-entity index so the painters_z walk can look each painter up.
    // (A HashMap keyed by Entity; the partial-re-extract cache keyed by Entity
    // inside ExtractedNodes is R6/R8's optimization — v1 rebuilds the changed set.)
    // `&Theme` via the `Extract<Res<Theme>>` deref chain (matches the
    // `&main_world_theme` idiom in `extract_buiy_draws`); `Res::into_inner`
    // can't be called here because it would move out of the `Extract` deref.
    let theme: &Theme = &theme;
    // `std::collections::HashMap` matches the convention in layout/systems.rs.
    let mut by_entity: std::collections::HashMap<Entity, ExtractedNode> =
        std::collections::HashMap::new();
    for (entity, gt, layout, bg, css_vis, offscreen, clip_rect, ancestor_clip, stacking) in
        nodes.iter()
    {
        let skip = node_skip_reason(css_vis, offscreen.is_some());
        if skip.is_some() {
            continue;
        }
        // A top-layer member escapes every ancestor clip and paints over the
        // full view (paint-order § 3.2 — the `None` sentinel); an in-flow member
        // clips to its own box ∩ ancestor clips. See [`effective_clip`].
        let clip = effective_clip(stacking, clip_rect, ancestor_clip);
        by_entity.insert(
            entity,
            extracted_node_for(entity, gt, layout, bg, clip.as_ref(), theme),
        );
    }

    // Index every forming context by its root entity, then drive the recursive
    // tree walk (paint-order § 1.1): the root context paints its own box, then
    // its painters_z forward, descending into each nested-SC painter AS A UNIT
    // at that position. A nested root is a single atomic entry in its parent's
    // list, so descending there is what places its contents at the right global
    // index — flat-concatenating every context's painters_z would instead emit
    // nested descendants at the end of their own list (the wrong order).
    let sc_by_entity: std::collections::HashMap<Entity, &[Entity]> = contexts
        .iter()
        .map(|(e, sc)| (e, sc.painters_z.as_slice()))
        .collect();
    let painters_z_of = |e: Entity| -> Option<&[Entity]> { sc_by_entity.get(&e).copied() };

    // Root contexts are the forming entities that no other context lists as a
    // painter (a nested root appears in exactly its parent's list, § 1.1). v1
    // expects a single root (architecture § 4, D2); sort the roots by entity so
    // a (degenerate) multi-root tree assembles deterministically rather than in
    // archetype order — cross-root order is unspecified by painters_z, so this
    // tiebreak is render-local and never overrides an in-context order.
    let nested: std::collections::HashSet<Entity> = sc_by_entity
        .values()
        .flat_map(|painters| painters.iter().copied())
        .filter(|e| sc_by_entity.contains_key(e))
        .collect();
    let mut roots: Vec<Entity> = sc_by_entity
        .keys()
        .copied()
        .filter(|e| !nested.contains(e))
        .collect();
    roots.sort_unstable();

    // The view-level logical→clip terms (architecture § 4, D2: every Node
    // resolves to the primary window's view). `BuiyViewUniform::for_view`
    // consumes these in prepare; leaving them at the `Vec2::ZERO` default makes
    // the transform degenerate. Logical (CSS-px) size + the device-pixel ratio
    // come straight off the primary window's resolution. `nodes` is filled by
    // the `assemble_context_tree` walk below (starts empty, Default).
    let mut all = ExtractedNodes {
        logical_size: primary_window.resolution.size(),
        scale_factor: primary_window.resolution.scale_factor(),
        ..Default::default()
    };
    for root in roots {
        // R6/R8: merge cached records for unchanged painters here.
        assemble_context_tree(
            root,
            &painters_z_of,
            &mut |e| by_entity.get(&e).copied(),
            &mut all.nodes,
        );
    }

    // Write the per-view ExtractedNodes onto the primary render view entity.
    // R6/R8 wire the exact main<->render view mapping and consume this component;
    // v1 inserts the single ExtractedNodes carrier (R5 owns the type — there is
    // no ExtractedNodesPrimary/ExtractedNodesResource wrapper). The precise
    // target-entity resolution is the one piece that needs the render world and
    // is exercised only under the GPU e2e path (Task 8 / R6/R8).
    commands.insert_resource(ExtractedNodesView(all));
}

/// v1 carrier-by-resource: the primary view's `ExtractedNodes`, inserted by
/// `extract_buiy_nodes` until R6/R8 wire it onto the resolved render-view entity
/// as a per-view component (architecture § 4). This is a thin newtype over the
/// R5-owned `ExtractedNodes` — NOT a parallel definition. R6 reads the inner
/// `ExtractedNodes`; the type itself stays R5's single carrier.
/// SUPERSEDED-BY: R6/R8 (node.rs/buckets read the per-view `ExtractedNodes`).
#[derive(Resource, Default, Clone, Debug)]
pub struct ExtractedNodesView(pub ExtractedNodes);
