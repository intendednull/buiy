//! The per-view extract mapping, factored into pure functions so the
//! device-independent half (`painters_z` ordering, per-entity record build) is
//! unit-testable on CI runners with no wgpu adapter. Color resolution is
//! delegated to the single canonical resolver
//! [`color::resolve_token`](crate::render::color::resolve_token) — this module
//! holds no second token→`Color` mapping. The paint-skip decision is likewise
//! delegated: the `write_paint_skip` render-prep pass
//! ([`crate::render::visibility`]) resolves the subtree-scoped
//! `CssVisibility::Hidden` / `OffscreenAuto` suppression into the computed
//! [`ComputedPaintSkip`] marker, and extract reads ONLY that marker (single
//! skip source — paint-order-and-top-layer.md § 5.3 / § 5.4). The
//! `extract_buiy_nodes` system (Task 6) is a thin wrapper that calls these.
//!
//! Spec: architecture.md § 1.2/§ 3/§ 4, paint-order-and-top-layer.md § 1/§ 5.

use crate::theme::Theme;
use bevy::prelude::*;

use crate::components::{Node, ResolvedLayout};
use crate::render::components::{
    AncestorClip, Background, ClipRect, ComputedPaintSkip, EffectGroup, EffectReason, Opacity,
    Outline,
};

/// One extracted effect group's CPU record (effect-compositor.md § 1.1). Emitted
/// alongside the flat node list by [`extract_buiy_nodes`]; the prepare pass turns
/// it into a `PreparedEffectGroup` (painted-bounds → bucket_extent → target). The
/// index of a record in the per-view `Vec<EffectGroupExtract>` is the value a
/// node's [`ExtractedNode::group`] holds.
///
/// `bounds` are logical-px, already folded through `GlobalTransform` by the
/// caller (the union of the group entity's own box and every descendant box the
/// group encloses — the v1 painted-bounds input; ink-expansion terms are an
/// upstream tier, not added here). `parent` is the index of the nearest
/// ENCLOSING group, or `None` for a root group (composites into the window).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectGroupExtract {
    /// The main-world group entity (the `EffectGroup` former).
    pub entity: Entity,
    /// Index of the enclosing group's record, or `None` if this group is not
    /// nested inside another effect group (composites into the window target).
    pub parent: Option<usize>,
    /// Group opacity in `[0,1]` (the `Opacity` value; `1.0` if the group formed
    /// for a non-opacity reason only). Applied once at the parent composite.
    pub opacity: f32,
    /// The OR of every reason that formed this group (from the `EffectGroup`
    /// marker). Drives the `plan_allocation` degradation ranking.
    pub reason: EffectReason,
    /// Logical-px painted bounds: the union of the group entity's own box and
    /// every box it transitively encloses, folded through `GlobalTransform`.
    pub bounds: Rect,
}

/// Per-view list of extracted effect groups (effect-compositor.md § 1.1),
/// carried as a render-world resource shim alongside [`ExtractedNodesView`]
/// (the v1 carrier shape — the per-view-component flip is the shared view-routing
/// follow-up). `prepare_effect_groups` reads this to size + acquire targets. The
/// vec index is the value `ExtractedNode::group` references.
#[derive(Resource, Default, Clone, Debug)]
pub struct ExtractedEffectGroups(pub Vec<EffectGroupExtract>);

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
    /// The 2D linear part of `GlobalTransform`'s affine — the box-local →
    /// window-logical basis, as column vectors `[col0, col1]` where
    /// `col0 = [m00, m10]` and `col1 = [m01, m11]`. Applied per-vertex in the
    /// quad/shadow vertex stage about the box-local origin (the corner the
    /// composed matrix maps `0` to), so a rotated/scaled element paints with the
    /// right orientation and size. Identity `[[1,0],[0,1]]` == no rotation/scale
    /// (the byte-identical fast path). Pillar 5: this reads the propagated
    /// `GlobalTransform`, NOT `ResolvedTransform` — the bridge already folded
    /// `ResolvedTransform.matrix` into `Transform` so render == picking by
    /// construction. FIDELITY: faithful for rotation + (non-)uniform scale;
    /// skew / general `TransformMatrix::Matrix` are bounded by the bridge's
    /// TRS-only `Transform::from_matrix` decompose (a lossy shear) — a separate
    /// residual (clip-and-transform.md § B.5).
    pub affine: [[f32; 2]; 2],
    /// Resolved background fill (already theme-resolved; `Color::NONE` ==
    /// transparent, extract emits no quad for it downstream).
    pub color: Color,
    /// The per-primitive clip AABB (own border box ∩ ancestor clips, from
    /// `clip_for_primitive`). `None` == the full-view sentinel: no ancestor
    /// clips this entity, OR it is a top-layer member (always `None`, per
    /// paint-order-and-top-layer.md § 3.2). Downstream (`pack_extracted`)
    /// packs `None` to `[±INFINITY]` so the fragment discard never fires.
    pub clip: Option<ClipRect>,
    /// The enclosing effect group's index into the per-view
    /// [`EffectGroupExtract`] list (effect-compositor.md § 1.1), or `None` for
    /// an in-flow / top-layer node that no `EffectGroup` ancestor encloses. The
    /// index is the NEAREST `EffectGroup` ancestor: a node nested two groups
    /// deep tags the inner group, and that group's record carries its own
    /// `parent` link up the chain. `pack_view` partitions the flat instance blob
    /// into contiguous per-group ranges off this tag (off-screen targets), so a
    /// group member is drawn once into its target, never flat.
    pub group: Option<usize>,
    /// Resolved `Outline` (the focus ring / selection outline), painted OUTSIDE
    /// the border box through the distinct band-pipeline record
    /// [`BorderBandInstance`](crate::render::instance::BorderBandInstance).
    /// `None` == no outline (the byte-stable fast path). The outline is clipped
    /// by the entity's `AncestorClip`, NOT its own box, so a focus ring survives
    /// an `overflow:hidden` ancestor (styling-f-tier.md § 2.4 — C6-a).
    pub outline: Option<ExtractedOutline>,
}

/// One resolved `Outline` (styling-f-tier.md § 2.4), built at extract time from
/// the author/framework `Outline` component + the entity's border box +
/// `AncestorClip`. A flat `Copy` record (no token, no `Length`) — every term is
/// pre-resolved to logical px / linear color so the packer
/// ([`pack_outline`](crate::render::instance::pack_outline)) and the band shader
/// are pure consumers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtractedOutline {
    /// Outer-box top-left, logical px = border-box origin shifted out by
    /// `width + offset` on the top/left.
    pub outer_pos: Vec2,
    /// Outer-box size, logical px = border-box size grown by `2*(width+offset)`.
    pub outer_size: Vec2,
    /// Pre-linearized ring color (uniform — the outline is a single stroke).
    pub color: [f32; 4],
    /// Ring thickness, logical px (`>= 2px` for the focus ring — WCAG 2.4.11).
    pub width: f32,
    /// Per-corner OUTER elliptical radius `(rx, ry) × 4` (TL, TR, BR, BL): the
    /// border radius grown by `(width + offset)` (CSS-faithful outline
    /// rounding). `[0; 8]` == square.
    pub outer_radius: [f32; 8],
    /// Per-corner INNER radius — the outer edge shrunk inward by `width` so the
    /// ring is `width` thick.
    pub inner_radius: [f32; 8],
    /// The OUTLINE clip = the entity's `AncestorClip` (never its own
    /// `ClipRect`); `None` = the full-view sentinel (no ancestor clips it, or a
    /// top-layer member). styling-f-tier.md § 2.4.
    pub clip: Option<ClipRect>,
    /// The 2D affine basis (same source as [`ExtractedNode::affine`]).
    pub affine: [[f32; 2]; 2],
}

/// Build one [`ExtractedNode`] from the layout box + composed transform + the
/// (optional) background token + the (optional) per-primitive clip AABB. Pure:
/// no GPU, no ECS access beyond the borrowed components. `position` is the
/// `GlobalTransform` translation; `size` is `ResolvedLayout.size`; `color`
/// resolves the `Background` token (absent background == transparent); `clip`
/// is carried verbatim (`None` == full-view sentinel, see [`ExtractedNode::clip`]).
/// The effect-group tag starts `None`; `extract_buiy_nodes` overwrites it from
/// the per-entity nearest-`EffectGroup`-ancestor map after this builds the record
/// (the group membership needs the main-world `ChildOf` chain, not just the
/// borrowed paint inputs).
pub fn extracted_node_for(
    entity: Entity,
    global_transform: &GlobalTransform,
    layout: &ResolvedLayout,
    background: Option<&Background>,
    clip: Option<&ClipRect>,
    theme: &Theme,
) -> ExtractedNode {
    let translation = global_transform.translation();
    // The 2D linear part of the composed affine (glam `Affine3A.matrix3`): xy of
    // the x-axis is col0, xy of the y-axis is col1 (COLUMNS, not rows — a
    // transpose would rotate the wrong way). For a pure rotation/scale the
    // matrix maps box-local `0 -> 0`, so `translation.xy` stays the painted
    // top-left and an identity transform yields the `[[1,0],[0,1]]` fast path.
    let m = global_transform.affine().matrix3;
    let affine = [[m.x_axis.x, m.x_axis.y], [m.y_axis.x, m.y_axis.y]];
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
        group: None,
        affine,
        // The outline rides a distinct record + clip (`AncestorClip`, not the
        // own box), so it is resolved separately by `extract_buiy_nodes` via
        // `resolve_outline` and assigned post-build — `extracted_node_for`
        // (also called by the Tier-2 snapshot harness) stays outline-free.
        outline: None,
    }
}

/// Resolve one entity's [`ExtractedOutline`] from its `Outline` component, the
/// border box (`position`/`size`, the painted origin + `ResolvedLayout.size`),
/// the entity's `AncestorClip`-derived outline clip, the resolved 2D affine
/// basis, and the active theme. Returns `None` when the outline paints nothing
/// (no style / zero width / a fully-transparent color), so a node with a
/// defaulted `Outline` rides the byte-stable no-band path.
///
/// Geometry (styling-f-tier.md § 2.4): the outer box is the border box grown by
/// `width + offset` on every side; `outer_radius` is the border radius grown by
/// `(width + offset)`; `inner_radius` is the outer radius shrunk by `width`
/// (clamped `>= 0`) so the ring is exactly `width` thick. The clip is the
/// caller-resolved outline clip (the entity's `AncestorClip`, NEVER its own
/// box), so a focus ring survives an `overflow:hidden` ancestor. Pure: no ECS /
/// GPU access beyond the borrowed inputs, so it is unit-testable headless.
pub fn resolve_outline(
    outline: &Outline,
    position: Vec2,
    size: Vec2,
    outline_clip: Option<ClipRect>,
    affine: [[f32; 2]; 2],
    theme: &Theme,
) -> Option<ExtractedOutline> {
    use crate::render::clip::px_or_zero;
    use crate::render::components::LineStyle;

    // No stroke: `style: None` or a non-positive width paints nothing.
    let width = px_or_zero(outline.width);
    if outline.style == LineStyle::None || width <= 0.0 {
        return None;
    }
    let color = crate::render::color::resolve_token(&outline.color, theme);
    if color == Color::NONE {
        return None; // fully transparent ⇒ no band.
    }
    let lin = LinearRgba::from(color);

    let offset = px_or_zero(outline.offset);
    // The ring sits `width + offset` outside the border box on every side.
    let out = width + offset;
    let outer_pos = position - Vec2::splat(out);
    let outer_size = size + Vec2::splat(2.0 * out);

    // No author `Border.radius` is fed in C6-a, so the focus ring is a square
    // ring (the common case for buttons/inputs). Outline rounding off a real
    // `Border.radius` lands when C6-b feeds the border channel; until then the
    // outer/inner radii are square (`0`), which is the correct CSS rendering of
    // an outline on a square box.
    let outer_radius = [0.0f32; 8];
    let inner_radius = [0.0f32; 8];

    Some(ExtractedOutline {
        outer_pos,
        outer_size,
        color: [lin.red, lin.green, lin.blue, lin.alpha],
        width,
        outer_radius,
        inner_radius,
        clip: outline_clip,
        affine,
    })
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
    let mut order = Vec::new();
    context_tree_paint_order(root, painters_z_of, &mut order);
    out.extend(order.into_iter().filter_map(build));
}

/// Flatten one stacking-context tree into entity paint order: the root's own
/// box first, then its `painters_z` forward, descending into each nested
/// context AS A UNIT at its position (paint-order § 1.1). The entity-order
/// core of [`assemble_context_tree`], shared with the glyph producer
/// (`text::extract_buiy_glyphs`) so the two walks can never diverge.
pub fn context_tree_paint_order<'a>(
    root: Entity,
    painters_z_of: &impl Fn(Entity) -> Option<&'a [Entity]>,
    out: &mut Vec<Entity>,
) {
    // The context root paints its OWN box first (CSS painter's algorithm: the
    // SC root's background/borders sit at the bottom of its context). 6f builds
    // a context's `painters_z` from its DESCENDANTS, excluding the root itself,
    // so the root is emitted here, never via the list below.
    out.push(root);
    let Some(painters) = painters_z_of(root) else {
        return;
    };
    for &painter in painters {
        if painters_z_of(painter).is_some() {
            // Nested SC root: descend as a unit at this position (§ 1.1).
            context_tree_paint_order(painter, painters_z_of, out);
        } else {
            out.push(painter);
        }
    }
}

/// The root context entities of a forming-context map — those no other
/// context lists as a painter (a nested root appears in exactly its parent's
/// list, paint-order § 1.1) — sorted by entity so a (degenerate) multi-root
/// tree assembles deterministically rather than in archetype order (the
/// `extract_buiy_nodes` tiebreak, hoisted so both producers share it).
/// Cross-root order is unspecified by `painters_z`, so the tiebreak is
/// render-local and never overrides an in-context order.
pub fn context_roots(sc_by_entity: &std::collections::HashMap<Entity, &[Entity]>) -> Vec<Entity> {
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
    roots
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

/// The OUTLINE clip for one entity (the focus-ring / selection-outline band).
/// Like [`effective_clip`] this forces the full-view sentinel for a top-layer
/// member, but otherwise takes the `Outline` path of `clip_for_primitive`
/// (`is_outline = true`): the entity's **`AncestorClip`** WITHOUT the own-box
/// step, so a ring drawn outside the border box is cropped by ancestor clips
/// (e.g. a scroll container) yet NOT erased by the element's own
/// `overflow:hidden` box (styling-f-tier.md § 2.4 / clip-and-transform.md § A.2).
pub fn effective_outline_clip(
    stacking: Option<&Stacking>,
    ancestor_clip: Option<&AncestorClip>,
) -> Option<ClipRect> {
    let is_top_layer = stacking.is_some_and(|s| s.top_layer != TopLayer::None);
    if is_top_layer {
        None
    } else {
        clip_for_primitive(true, None, ancestor_clip)
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
                // The computed subtree paint-skip marker (§ 5.3 / § 5.4) —
                // the SINGLE skip source. `write_paint_skip` resolves the
                // per-entity `CssVisibility` / `OffscreenAuto` inputs into it
                // (root AND descendants), so extract never reads those
                // author-set components directly.
                Option<&ComputedPaintSkip>,
                // Clip inputs (R8b): the computed per-entity clip AABB + its
                // ancestor-only companion (consumed by `clip_for_primitive`),
                // and `Stacking` so a top-layer member is forced to the
                // full-view sentinel (`clip = None`, paint-order § 3.2).
                Option<&ClipRect>,
                Option<&AncestorClip>,
                Option<&Stacking>,
                // Effect-compositor fan (effect-compositor.md § 1.1): the
                // `EffectGroup` marker (which entities form an off-screen group +
                // their `reason`) and `Opacity` (the alpha applied at composite).
                // FAN: add Option<&BoxShadow>/&Border and the reserved effect
                // components here as their tier lands (architecture § 1.2
                // illustrative subset). C6-a adds Option<&Outline> below (the
                // focus-ring / selection-outline band channel).
                // NOTE: Containment is NOT in the fan — content-visibility:hidden
                // paints the entity's own box and prunes descendants layout-side
                // (paint-order § 5.2), so it is not a render skip input.
                Option<&EffectGroup>,
                Option<&Opacity>,
                // C6-a: the outline paint (focus ring / selection outline). It
                // rides a DISTINCT band record + the entity's `AncestorClip`
                // (resolved below), not its own box (styling-f-tier.md § 2.4).
                Option<&Outline>,
            ),
            With<Node>,
        >,
    >,
    // The parent link, used to resolve each painted node's nearest `EffectGroup`
    // ancestor (the off-screen group it belongs to). Read over ALL `Node`s so the
    // walk can climb through non-group ancestors to the enclosing group.
    child_of: Extract<Query<&bevy::prelude::ChildOf, With<Node>>>,
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
                    // The paint-skip flip, ADD direction: an entity (or its
                    // subtree) newly suppressed gets the computed marker
                    // inserted by `write_paint_skip` the same frame
                    // (`Changed` includes `Added`). The REMOVE direction
                    // (hide→show) cannot fire `Changed` — it rides the
                    // `RemovedComponents<ComputedPaintSkip>` stream below.
                    Changed<ComputedPaintSkip>,
                    Changed<StackingContext>,
                    Changed<Stacking>,
                    Changed<ClipRect>,
                    Changed<AncestorClip>,
                    // Effect-compositor damage (effect-compositor.md § 1.1): a
                    // group forming/dropping (`EffectGroup`) or an opacity change
                    // re-extracts so group membership + the composite alpha never
                    // go stale. Kept in lockstep with the `nodes` fan above.
                    Changed<EffectGroup>,
                    Changed<Opacity>,
                    // C6-a: an outline insert/remove/edit (the focus ring is a
                    // framework-owned `Outline` the ring lowering toggles) must
                    // re-extract so the band appears/vanishes. Kept in lockstep
                    // with the `nodes` fan above. The `AncestorClip` the outline
                    // clips against already rides `Changed<AncestorClip>` above.
                    Changed<Outline>,
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
    // The paint-skip flip, REMOVE direction: a hide→show transition removes
    // the computed marker, which emits no `Changed` — without this stream the
    // re-shown subtree would never re-extract and stay vanished forever (the
    // exact mirror of the despawn stream above; retain-damage design § "The
    // fix", trigger-union extension).
    mut removed_skip: Extract<RemovedComponents<ComputedPaintSkip>>,
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
    // Drain the removal streams FIRST, before any early-return, so the
    // `RemovedComponents` cursors advance every frame and events never
    // accumulate (incl. the vanished-window path below). A despawn drops
    // `ResolvedLayout`; a hide→show flip drops `ComputedPaintSkip` — the two
    // damage sources the `Changed` probe cannot see.
    let despawned = removed.read().count() > 0;
    let skip_lifted = removed_skip.read().count() > 0;

    let Ok(primary_window) = primary.single() else {
        commands.insert_resource(ExtractedNodesView(ExtractedNodes::default()));
        commands.insert_resource(ExtractedEffectGroups::default());
        return;
    };

    // Damage gate (architecture.md § 3.1 / 2026-06-07-render-extract-retain-damage
    // -design.md). Rebuild only when something this view paints actually changed:
    // a `Changed` paint input (incl. a paint-skip ADD), a paint-skip LIFT (the
    // marker-removal stream — a re-shown subtree must reappear), an entity
    // despawn, or a theme/forced-colors swap (a global token re-resolve that
    // bypasses per-entity change detection — color-and-forced-colors.md § 3).
    // On a steady-state frame,
    // return WITHOUT touching the resource — the prior `ExtractedNodesView` stays
    // resident and `is_changed()` is false in prepare, which retains the persistent
    // buffers (the node re-binds + re-draws them). This is the O(0) steady-state
    // the spec's gate-#14 budget requires.
    if changed.is_empty() && !despawned && !skip_lifted && !theme.is_changed() {
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
    // Effect-group formers seen this frame, keyed by entity → (reason, opacity).
    // The painted-bounds union + parent links are derived below from the
    // `ChildOf` chain; the per-entity `EffectReason`/`Opacity` are captured here
    // while the fan is borrowed (effect-compositor.md § 1.1).
    let mut group_formers: std::collections::HashMap<Entity, (EffectReason, f32)> =
        std::collections::HashMap::new();
    for (
        entity,
        gt,
        layout,
        bg,
        paint_skip,
        clip_rect,
        ancestor_clip,
        stacking,
        effect_group,
        opacity,
        outline,
    ) in nodes.iter()
    {
        // The subtree-scoped paint skip (§ 5.3 / § 5.4): presence of the
        // computed marker ⇒ emit nothing for this entity. `write_paint_skip`
        // stamps the marker on the hidden/offscreen ROOT and every
        // descendant, so this one per-entity read covers the whole subtree.
        if paint_skip.is_some() {
            continue;
        }
        if let Some(eg) = effect_group {
            // `Opacity` default is 1.0 (no-op); only an opacity-formed group
            // carries a `< 1` value, but capture it unconditionally so a group
            // that ALSO has opacity composites at the right alpha.
            let a = opacity.map(|o| o.0).unwrap_or(1.0);
            group_formers.insert(entity, (eg.reason, a));
        }
        // A top-layer member escapes every ancestor clip and paints over the
        // full view (paint-order § 3.2 — the `None` sentinel); an in-flow member
        // clips to its own box ∩ ancestor clips. See [`effective_clip`].
        let clip = effective_clip(stacking, clip_rect, ancestor_clip);
        let mut node = extracted_node_for(entity, gt, layout, bg, clip.as_ref(), theme);
        // C6-a: resolve the outline (focus ring / selection outline) against the
        // entity's `AncestorClip` (NOT its own box, `effective_outline_clip`) so
        // it survives an `overflow:hidden` ancestor (styling-f-tier.md § 2.4).
        if let Some(outline) = outline {
            let outline_clip = effective_outline_clip(stacking, ancestor_clip);
            node.outline = resolve_outline(
                outline,
                node.position,
                node.size,
                outline_clip,
                node.affine,
                theme,
            );
        }
        by_entity.insert(entity, node);
    }

    // Resolve each painted node's NEAREST `EffectGroup` ancestor (the group it
    // belongs to). Membership is the `ChildOf` subtree of a former, NOT an SC
    // boundary: the nearest-former climb (a node is its OWN group if it is a
    // former) is the v1 nesting source (effect-compositor.md § 1.1, decided
    // fork 5). The trigger-5 SC formers have since landed (layout/systems.rs
    // `forms_stacking_context`), making every SC-forming group's subtree one
    // atomic `painters_z` slice — the climb stays because it is SC-agnostic
    // and also covers the `backdrop-filter` former, which forms no SC.
    let nearest_group_entity = |start: Entity| -> Option<Entity> {
        let mut cur = start;
        loop {
            if group_formers.contains_key(&cur) {
                return Some(cur);
            }
            match child_of.get(cur) {
                Ok(parent) => cur = parent.parent(),
                Err(_) => return None,
            }
        }
    };

    // Assign each former a stable index, then compute its parent link (the
    // nearest enclosing former of its OWN parent) and seed its painted bounds.
    let mut group_entities: Vec<Entity> = group_formers.keys().copied().collect();
    group_entities.sort_unstable(); // deterministic indices
    let group_index: std::collections::HashMap<Entity, usize> = group_entities
        .iter()
        .enumerate()
        .map(|(i, &e)| (e, i))
        .collect();
    let mut groups: Vec<EffectGroupExtract> = group_entities
        .iter()
        .map(|&e| {
            let (reason, opacity) = group_formers[&e];
            // The enclosing group is the nearest former STRICTLY above `e`.
            let parent = child_of
                .get(e)
                .ok()
                .and_then(|p| nearest_group_entity(p.parent()))
                .and_then(|pe| group_index.get(&pe).copied());
            // Seed bounds with the former's own box (a former with no painted
            // record — transparent fill — still bounds-anchors its subtree).
            let own = by_entity.get(&e).map(|n| Rect {
                min: n.position,
                max: n.position + n.size,
            });
            EffectGroupExtract {
                entity: e,
                parent,
                opacity,
                reason,
                bounds: own.unwrap_or(Rect {
                    min: Vec2::ZERO,
                    max: Vec2::ZERO,
                }),
            }
        })
        .collect();

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

    // Root contexts (the shared [`context_roots`] helper): the forming
    // entities no other context lists as a painter, entity-sorted. v1 expects
    // a single root (architecture § 4, D2).
    let roots = context_roots(&sc_by_entity);

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

    // Tag every assembled node with its nearest-`EffectGroup`-ancestor index and
    // grow that group's painted bounds by the member box. The walk emits a
    // group's subtree contiguously (it descends a node's children before its
    // siblings), so `pack_view` partitions the flat blob into contiguous per-group
    // ranges off this tag (effect-compositor.md § 1.1 / decided fork 3). Bounds
    // grow to the UNION of every member box (painted_bounds' descendant term).
    if !groups.is_empty() {
        for node in &mut all.nodes {
            if let Some(ge) = nearest_group_entity(node.entity)
                && let Some(&gi) = group_index.get(&ge)
            {
                node.group = Some(gi);
                let b = &mut groups[gi].bounds;
                b.min = b.min.min(node.position);
                b.max = b.max.max(node.position + node.size);
            }
        }
    }

    // Write the per-view ExtractedNodes onto the primary render view entity.
    // R6/R8 wire the exact main<->render view mapping and consume this component;
    // v1 inserts the single ExtractedNodes carrier (R5 owns the type — there is
    // no ExtractedNodesPrimary/ExtractedNodesResource wrapper). The precise
    // target-entity resolution is the one piece that needs the render world and
    // is exercised only under the GPU e2e path (Task 8 / R6/R8).
    commands.insert_resource(ExtractedNodesView(all));
    // The per-view effect-group list (effect-compositor.md § 1.1). Emitted on
    // EVERY rebuild frame (incl. when empty) so a frame that drops the last group
    // clears the carrier — mirrors the `ExtractedNodesView` overwrite contract.
    commands.insert_resource(ExtractedEffectGroups(std::mem::take(&mut groups)));
}

/// v1 carrier-by-resource: the primary view's `ExtractedNodes`, inserted by
/// `extract_buiy_nodes` until R6/R8 wire it onto the resolved render-view entity
/// as a per-view component (architecture § 4). This is a thin newtype over the
/// R5-owned `ExtractedNodes` — NOT a parallel definition. R6 reads the inner
/// `ExtractedNodes`; the type itself stays R5's single carrier.
/// SUPERSEDED-BY: R6/R8 (node.rs/buckets read the per-view `ExtractedNodes`).
#[derive(Resource, Default, Clone, Debug)]
pub struct ExtractedNodesView(pub ExtractedNodes);

/// One text quad-tier visual (decoration-and-paint § 4.6): selection rects
/// (T7) and underline/overline (T6), keyed by the SOURCE entity. A flat
/// `Copy` record — deliberately NO order and NO group field: paint order is
/// the implicit `Vec` order of `ExtractedNodes.nodes`, and BOTH derive from
/// the fresh node list at pack time (a recorded index would go stale
/// whenever the node walk rebuilds while text quads are retained — the
/// spec's rejected round-1 design). Carries no cosmic-text type (the seam
/// contract).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextQuad {
    /// The source main-world entity — the splice key.
    pub entity: Entity,
    /// Painted top-left, logical px, window space (origin-folded by the
    /// producer; § 3.3 y already snapped).
    pub position: Vec2,
    /// Quad size, logical px (height = the § 3.3 floored thickness).
    pub size: Vec2,
    /// Resolved paint color (§ 3.2 precedence applied at extract);
    /// `Color::NONE` = skip at pack (mirrors `ExtractedNode.color`).
    pub color: Color,
    /// The entity's SELF-INCLUSIVE clip (same resolution as its glyphs,
    /// glyph-pipeline § 8); `None` = the full-view sentinel.
    pub clip: Option<ClipRect>,
}

/// Render-world carrier for text's quad-tier visuals (decoration-and-paint
/// § 4.6). Producer: `text::extract_buiy_glyphs` — rebuilt alongside
/// `ExtractedGlyphs` under the same § 6.2 probe union (one damage decision),
/// retained untouched on steady frames so `is_changed()` is the third quad
/// gate term in `prepare_buiy_instances`. ENTITY-GROUPED: each entity's
/// quads are contiguous, in § 4.4 emission order (the pack debug_asserts
/// the grouping).
#[derive(Resource, Default, Clone, Debug)]
pub struct ExtractedTextQuads {
    pub quads: Vec<TextQuad>,
}
