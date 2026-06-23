//! The abstract [`Scene`] model + `proptest` generators, and [`realize`] —
//! the bridge that threads a generated `Scene` through the PRODUCTION CPU
//! paint-order assembly into the flat [`ExtractedNodes`] list the predicates
//! assert on (invariants.md § "Scene generators").
//!
//! We generate an abstract `Scene` (not raw Bevy `World`s) so shrinking yields
//! a minimal, printable counterexample and the predicates stay world-agnostic.
//! `realize` does the heavy lifting: it assigns each node a synthetic `Entity`,
//! decides stacking-context formation, builds each forming node's `painters_z`
//! by CALLING the production assembly, then runs the *production*
//! [`context_tree_paint_order`] over a tree whose tails were split with
//! [`partition_top_layer`](buiy_core::render::top_layer::partition_top_layer)
//! and ranked with the promoted [`top_layer_paint_rank`], so the realized order
//! cannot diverge from the engine **over the generated domain**.
//!
//! The per-context paint assembly is NOT mirrored here — `realize` calls the
//! SHARED production fn
//! [`painters_z_for_context`](buiy_core::layout::painters_z_for_context), the
//! same one production's `painters_of` calls (subtree walk in document order,
//! stop-at-nested-context, escape-exclusion, four-tier stable
//! [`paint_key`] sort). There is no parallel 6f implementation to drift, so a
//! regression in that production code reds this invariant directly — the
//! protection comes from sharing the assembly, NOT from a hand-built predicate
//! (testing-audit #6). Because `paint_key` keys on `(Stacking, PositionKind)`, a
//! `SceneNode` carries a generated [`PositionKind`] axis so all four paint tiers
//! — including tier 2 *(positioned, auto-z)* — are reachable (testing-audit
//! #13). `z_index` and `position_kind` vary independently (the old `positioned ⟺
//! z_index.is_some()` domain constraint is gone), so the generator exercises the
//! same `(Stacking, PositionKind)` space production classifies — e.g. a static
//! node with an explicit `z_index` (z ignored, stays in-flow) and a positioned
//! node with auto z (tier 2).

use bevy::prelude::*;
use proptest::prelude::*;

use buiy_core::layout::{
    PositionKind, Stacking, TopLayer, ZIndex, paint_key, top_layer_paint_rank,
};
use buiy_core::render::components::ClipRect;
use buiy_core::render::extract::{ExtractedNode, ExtractedNodes, context_tree_paint_order};

// ---------------------------------------------------------------------------
// The abstract scene model.
// ---------------------------------------------------------------------------

/// A generated node in a bounded hierarchy. `name` is the stable identity used
/// in diagnostics (mirrors Tier 2's `Name`-based dump — never raw `Entity`
/// bits). A shrunk counterexample prints via `Debug` and reproduces from the
/// committed seed alone.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneNode {
    /// Unique within a `Scene` (`n0`, `n1`, …), assigned by a post-generation
    /// pre-order rename so the tree is reproducible and printable.
    pub name: String,
    /// Child subtrees, in document order.
    pub children: Vec<SceneNode>,
    /// CSS `position`. Combined with `z_index` this decides the paint tier
    /// production's [`paint_key`] assigns: a non-`Static` node with an explicit
    /// `z_index` is "positioned" (tiers 0/3) and forms a stacking context; a
    /// non-`Static` node with auto `z_index` is tier 2; a `Static` node ignores
    /// its `z_index` and stays in-flow (tier 1). Varies INDEPENDENTLY of
    /// `z_index` so the generator covers the full `(Stacking, PositionKind)`
    /// space (testing-audit #13).
    pub position_kind: PositionKind,
    /// Explicit `z-index`. On a positioned node it drives stacking-context
    /// formation + the paint tier; on a `Static` node it is IGNORED (CSS quirk).
    /// `None` == auto (in-flow / auto-positioned document order).
    pub z_index: Option<i32>,
    /// `Isolation::Isolate` — forces a stacking context even with no z/transform.
    pub isolation: bool,
    /// Top-layer participation. `None` for the bulk; a non-`None` variant
    /// escapes its parent context to the root top layer (ordered by
    /// [`top_layer_paint_rank`]).
    pub top_layer: TopLayer,
    /// The `compose_transform` inputs (a non-identity transform forms a context).
    pub transform: GenTransform,
    /// Logical-px box (always finite, `≥ 0` by construction).
    pub size: (f32, f32),
    /// Resolved background color (never the magenta missing-token sentinel).
    pub background: Option<[f32; 4]>,
}

/// A generated scene: a forest of root subtrees (typically one root).
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Root subtrees, in document order.
    pub roots: Vec<SceneNode>,
}

/// The `compose_transform` input space (invariants.md § "Scene generators"):
/// the longhand `Translate` (px), `Rotate` (axis-angle), `Scale` (per-axis),
/// all finite and away from the degenerate `0`. The identity (all-default)
/// case is always reachable for shrinking. This is the generator-side mirror
/// of `buiy_core`'s `Translate`/`Rotate`/`Scale` longhands; `transform_roundtrips`
/// feeds it straight through `compose_transform`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenTransform {
    /// Translation in logical px (`x`, `y`, `z`).
    pub translate: [f32; 3],
    /// Rotation as an axis-angle: unit-ish axis (`x`, `y`, `z`) + angle (rad).
    pub rotate_axis: [f32; 3],
    pub rotate_angle: f32,
    /// Per-axis scale (away from `0`).
    pub scale: [f32; 3],
}

impl GenTransform {
    /// The identity transform (all factors neutral). The shrink target.
    pub const IDENTITY: GenTransform = GenTransform {
        translate: [0.0, 0.0, 0.0],
        rotate_axis: [0.0, 0.0, 1.0],
        rotate_angle: 0.0,
        scale: [1.0, 1.0, 1.0],
    };

    /// `true` when this is (numerically) the identity — the formation trigger
    /// "non-identity transform" (a forming context). Uses an exact compare
    /// against the neutral factors; the generator only ever emits the exact
    /// `IDENTITY` or a deliberately non-trivial transform, so no epsilon is
    /// needed here.
    pub fn is_identity(&self) -> bool {
        self.translate == [0.0, 0.0, 0.0]
            && self.rotate_angle == 0.0
            && self.scale == [1.0, 1.0, 1.0]
    }
}

// ---------------------------------------------------------------------------
// Generator budget + strategies.
// ---------------------------------------------------------------------------

/// Bounded generator budget so the property space is finite-depth and shrinking
/// terminates fast (invariants.md § "Strategy budget").
#[derive(Clone, Copy, Debug)]
pub struct SceneParams {
    /// Hierarchy depth cap.
    pub max_depth: u32,
    /// Children-per-node cap.
    pub max_breadth: u32,
    /// Total-node guard (prevents blow-up; `prop_recursive`'s `desired_size`).
    pub max_nodes: u32,
    /// P(a node forms a context via z/isolation).
    pub p_stacking: f64,
    /// P(a node escapes to the top layer).
    pub p_top_layer: f64,
}

impl Default for SceneParams {
    fn default() -> Self {
        Self {
            max_depth: 4,
            max_breadth: 4,
            max_nodes: 24,
            p_stacking: 0.3,
            p_top_layer: 0.1,
        }
    }
}

/// Strategy for a single [`GenTransform`]. Skewed to the identity (the common +
/// shrink case) but reaches a finite, well-conditioned non-identity transform:
/// translate in `-512..512`, rotate angle in `0..2π` about an axis with a
/// non-zero component, scale in `0.1..8.0` per axis (away from `0`). Public so
/// the `transform_roundtrips` proptest can draw inputs directly.
pub fn arb_transform() -> impl Strategy<Value = GenTransform> {
    prop_oneof![
        // Weighted heavily toward identity so most generated nodes are in-flow.
        3 => Just(GenTransform::IDENTITY),
        1 => (
            // translate
            (-512.0f32..512.0, -512.0f32..512.0, -512.0f32..512.0),
            // rotate axis (kept non-degenerate by forcing z away from 0) + angle
            (-1.0f32..1.0, -1.0f32..1.0, 0.1f32..1.0),
            0.0f32..std::f32::consts::TAU,
            // scale away from 0
            (0.1f32..8.0, 0.1f32..8.0, 0.1f32..8.0),
        )
            .prop_map(|(t, axis, angle, s)| GenTransform {
                translate: [t.0, t.1, t.2],
                rotate_axis: [axis.0, axis.1, axis.2],
                rotate_angle: angle,
                scale: [s.0, s.1, s.2],
            }),
    ]
}

/// Strategy for one node's leaf attributes (everything but `children`/`name`).
/// `z_index` is drawn from the interesting `{-1, 0, 1, 2}` partition
/// (negative/zero/positive), gated by `p_stacking`; `position_kind` from all
/// five variants skewed to `Static` (the common in-flow case); `top_layer` from
/// all five variants skewed to `None`, gated by `p_top_layer`. `z_index` and
/// `position_kind` vary INDEPENDENTLY so every `(Stacking, PositionKind)` cell
/// production's [`paint_key`] classifies is reachable — including tier 2
/// *(positioned, auto-z)* and the CSS quirk *(static, explicit-z)* (testing-audit
/// #13).
fn arb_leaf(p: SceneParams) -> impl Strategy<Value = SceneNode> {
    let z_strategy = prop::option::weighted(
        p.p_stacking,
        prop_oneof![Just(-1i32), Just(0), Just(1), Just(2)],
    );
    let position_kind = arb_position_kind(p.p_stacking);
    let isolation = prop::bool::weighted(p.p_stacking);
    let top_layer = arb_top_layer(p.p_top_layer);
    let size = (0.0f32..512.0, 0.0f32..512.0);
    let background = prop::option::of((0.0f32..1.0, 0.0f32..1.0, 0.0f32..1.0, 0.0f32..1.0));

    (
        z_strategy,
        position_kind,
        isolation,
        top_layer,
        arb_transform(),
        size,
        background,
    )
        .prop_map(|(z, pos_kind, iso, tl, transform, size, bg)| SceneNode {
            // Placeholder name; `realize`/`arb_scene` rename pre-order.
            name: String::new(),
            children: Vec::new(),
            position_kind: pos_kind,
            z_index: z,
            isolation: iso,
            top_layer: tl,
            transform,
            size: (size.0, size.1),
            background: bg.map(|(r, g, b, a)| [r, g, b, a]),
        })
}

/// Strategy for [`PositionKind`], all five variants reachable but heavily skewed
/// to `Static` (the common in-flow case) so most generated nodes stay in tier 1
/// and the tree shrinks toward an all-static scene. `p_positioned` (= the
/// stacking probability) gates how often a node is positioned; when it is, the
/// four non-static kinds are equiprobable. A positioned node combined with auto
/// `z_index` is the tier-2 paint class the old generator could not reach
/// (testing-audit #13).
fn arb_position_kind(p_positioned: f64) -> impl Strategy<Value = PositionKind> {
    let positioned = prop_oneof![
        Just(PositionKind::Relative),
        Just(PositionKind::Absolute),
        Just(PositionKind::Fixed),
        Just(PositionKind::Sticky),
    ];
    prop::option::weighted(p_positioned, positioned)
        .prop_map(|opt| opt.unwrap_or(PositionKind::Static))
}

/// Strategy for `TopLayer`, all five variants reachable but heavily skewed to
/// `None` (the common in-flow case). Every escaping variant MUST be reachable
/// so `top_layer_dominates` exercises the full tier rank, not just `Modal`.
fn arb_top_layer(p_top: f64) -> impl Strategy<Value = TopLayer> {
    let escape = prop_oneof![
        Just(TopLayer::Fullscreen),
        Just(TopLayer::Tooltip),
        Just(TopLayer::Popover),
        Just(TopLayer::Modal),
    ];
    prop::option::weighted(p_top, escape).prop_map(|opt| opt.unwrap_or(TopLayer::None))
}

/// Generate a bounded, shrinkable single-root [`Scene`]. `prop_recursive` bounds
/// depth + node count so the tree is finite and shrinks toward the shallow
/// scene (invariants.md § "Strategy budget"). Names are assigned by a final
/// pre-order rename (`n0..nK`) so a shrunk counterexample is reproducible and
/// printable.
pub fn arb_scene(p: SceneParams) -> impl Strategy<Value = Scene> {
    let leaf = arb_leaf(p);
    let tree = leaf.prop_recursive(p.max_depth, p.max_nodes, p.max_breadth, move |inner| {
        (
            arb_leaf(p),
            prop::collection::vec(inner, 0..=p.max_breadth as usize),
        )
            .prop_map(|(mut node, children)| {
                node.children = children;
                node
            })
    });
    // A scene is a SINGLE root tree — the Buiy model is one root context per
    // window (cross-window scoping is a deferred follow-up, per the layout
    // code). One root fully exercises every invariant (nesting, z-order,
    // top-layer escape, context isolation); a multi-root forest would only add
    // a cross-tree paint order that `painters_z` leaves unspecified, forcing
    // every predicate to special-case it without testing anything new.
    tree.prop_map(|mut root| {
        // The ROOT is never a top-layer member: the top layer is an ESCAPE
        // mechanism (a node leaves its parent context to paint at the root), so a
        // node with no parent has nothing to escape. Forcing the root to `None`
        // keeps the model faithful — every top-layer node has a parent to escape
        // from — and `top_layer_dominates` well-defined.
        root.top_layer = TopLayer::None;
        let mut counter = 0u32;
        rename_preorder(&mut root, &mut counter);
        Scene { roots: vec![root] }
    })
}

/// Pre-order rename so every node gets a unique, stable `nK` name.
fn rename_preorder(node: &mut SceneNode, counter: &mut u32) {
    node.name = format!("n{counter}");
    *counter += 1;
    for child in &mut node.children {
        rename_preorder(child, counter);
    }
}

// ---------------------------------------------------------------------------
// `realize` — Scene → ExtractedNodes through the production paint path.
// ---------------------------------------------------------------------------

/// A realized scene: the flat paint-ordered [`ExtractedNodes`] PLUS the
/// per-node stacking-context membership the generator recorded (consumed by
/// `contexts_do_not_interleave`). Kept together so the predicate sees the same
/// context assignment `realize` used.
#[derive(Debug, Clone)]
pub struct Realized {
    /// The flat paint-ordered node list (the production order).
    pub nodes: ExtractedNodes,
    /// `entity → owning stacking-context root entity`, for every painted node.
    pub context_of: std::collections::HashMap<Entity, Entity>,
    /// `context-root entity → every entity painted WITHIN that context's
    /// subtree` (the root + all transitive descendants, including nested
    /// contexts). A stacking context paints as a UNIT, so each such set must be
    /// a contiguous run in the paint order — the property
    /// `contexts_do_not_interleave` checks.
    pub context_members: std::collections::HashMap<Entity, Vec<Entity>>,
    /// `entity → EFFECTIVE top-layer membership`: the nearest top-layer ancestor's
    /// [`TopLayer`] (inclusive of self), or `None` for a purely in-flow node. A
    /// descendant of an escaped node paints INSIDE that escaped context, so it
    /// is part of the top layer and inherits its rank. `ExtractedNode` carries no
    /// top-layer field (a render-only signal), so the dominance predicate
    /// recovers membership from here.
    pub top_layer_of: std::collections::HashMap<Entity, TopLayer>,
    /// `context-root entity → its DIRECT painters` (the `painters_z` list the
    /// SHARED production assembly returned, EXCLUDING the escaped top-layer tail).
    /// A nested context root appears here as a single atomic entry. The
    /// `paint_order_respects_paint_key` predicate reads this to assert each
    /// context's painters are non-decreasing in [`Self::paint_key_of`]; that
    /// predicate *observes* the shared assembly's output — the regression
    /// protection itself comes from `realize` calling the production fn, not from
    /// the predicate (testing-audit #6).
    pub painters_of_ctx: std::collections::HashMap<Entity, Vec<Entity>>,
    /// `entity → its production paint key `(tier, z)``, computed by the real
    /// [`paint_key`]. The oracle `paint_order_respects_paint_key` checks the
    /// shared assembly's output against.
    pub paint_key_of: std::collections::HashMap<Entity, (u8, i32)>,
    /// `entity → node name`, for diagnostics.
    pub name_of: std::collections::HashMap<Entity, String>,
}

/// Realize a [`Scene`] into the flat paint-ordered [`ExtractedNodes`] the
/// predicates assert on, through the PRODUCTION CPU paint assembly. No GPU, no
/// `World`: every node maps to a synthetic `Entity` (pre-order index), each
/// forming context's `painters_z` is built exactly as layout sub-pass 6f does,
/// and the global order comes from the production [`context_tree_paint_order`]
/// over tails split with
/// [`partition_top_layer`](buiy_core::render::top_layer::partition_top_layer),
/// with the escaped top-layer members ordered by [`top_layer_paint_rank`].
pub fn realize(scene: &Scene) -> ExtractedNodes {
    realize_full(scene).nodes
}

/// [`realize`] plus the context-membership map (`contexts_do_not_interleave`
/// needs it). Pure-CPU.
pub fn realize_full(scene: &Scene) -> Realized {
    let mut flat: Vec<FlatNode> = Vec::new();
    // Index every node in pre-order; record parent + the synthetic entity.
    for (root_i, root) in scene.roots.iter().enumerate() {
        // EVERY forest root forms its own root stacking context (not just the
        // first) — each is a context tree the production walk runs from.
        let _ = root_i;
        flatten(root, None, true, &mut flat);
    }

    // entity-keyed views.
    let entity_of: std::collections::HashMap<usize, Entity> = flat
        .iter()
        .map(|n| {
            (
                n.idx,
                Entity::from_raw_u32(n.idx as u32 + 1).expect("nonzero index"),
            )
        })
        .collect();
    let name_of: std::collections::HashMap<Entity, String> = flat
        .iter()
        .map(|n| (entity_of[&n.idx], n.name.clone()))
        .collect();

    // Which nodes FORM a stacking context (root | isolation | z | transform).
    let forms: std::collections::HashSet<usize> = flat
        .iter()
        .filter(|n| n.forms_context())
        .map(|n| n.idx)
        .collect();

    // children-by-parent, in document order.
    let mut children_of: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for n in &flat {
        if let Some(p) = n.parent {
            children_of.entry(p).or_default().push(n.idx);
        }
    }

    // The root context each node belongs to: the nearest forming ancestor
    // (inclusive of self iff self forms). Used for context membership + escape.
    let by_idx: std::collections::HashMap<usize, &FlatNode> =
        flat.iter().map(|n| (n.idx, n)).collect();
    let root_context = |mut idx: usize| -> usize {
        loop {
            if forms.contains(&idx) {
                return idx;
            }
            match by_idx[&idx].parent {
                Some(p) => idx = p,
                None => return idx, // a root always forms; defensive
            }
        }
    };
    // The OUTERMOST (tree-root) ancestor of a node — the context an escaped
    // top-layer member attaches to (mirrors sub-pass 6f's `root_ancestor`,
    // systems.rs § 4). Distinct from `root_context`: escape always goes to the
    // top of the tree so a top-layer node paints after EVERY normal node, not
    // just after the normal nodes of a nested context.
    let tree_root = |mut idx: usize| -> usize {
        while let Some(p) = by_idx[&idx].parent {
            idx = p;
        }
        idx
    };

    // Build each forming context's in-flow `painters_z` by calling the SHARED
    // production assembly `buiy_core::layout::painters_z_for_context` — the SAME
    // function production's `painters_of` calls, over flat-node indices instead
    // of `Entity`. There is NO parallel copy in the harness: the subtree walk
    // (document order, STOP at a nested context, EXCLUDE escaping top-layer
    // members) and the four-tier `paint_key` stable sort are production's, so a
    // regression to that code (a reversed sort, descending past a nested context)
    // reds THIS invariant — which a hand-rolled mirror would miss (testing-audit
    // #6). Closures: direct children in document order; `forms` membership; the
    // top-layer exclusion (the scene domain has no `Display::None`); the
    // production paint key over `(Stacking, PositionKind)`.
    let mut painters_z: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for &ctx in &forms {
        let painters = buiy_core::layout::painters_z_for_context(
            ctx,
            |i| children_of.get(&i).cloned().unwrap_or_default(),
            |i| forms.contains(&i),
            |i| by_idx[&i].top_layer != TopLayer::None,
            |i| node_paint_key(by_idx[&i]),
        );
        painters_z.insert(ctx, painters);
    }

    // Escaped top-layer members attach to their root-ancestor context's tail,
    // ordered by `top_layer_paint_rank` (Fullscreen bottom < … < Modal top),
    // stable within a tier (activation = document order here).
    let mut escaped_by_ctx: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for n in &flat {
        if n.top_layer != TopLayer::None {
            // A node that is itself a ROOT does NOT escape — it has no parent
            // context to escape from, so it forms its own root context normally
            // (mirrors sub-pass 6f's `if r != e` guard, systems.rs § 4). Only a
            // top-layer node WITH a parent escapes, attaching to the OUTERMOST
            // (tree-root) context so it paints after EVERY normal node.
            if n.parent.is_some() {
                let host = tree_root(n.idx);
                escaped_by_ctx.entry(host).or_default().push(n.idx);
            }
        }
    }
    for tail in escaped_by_ctx.values_mut() {
        tail.sort_by_key(|&i| top_layer_paint_rank(by_idx[&i].top_layer));
    }

    // Resolve a node index → its `painters_z` slice (or `None` for a
    // non-context painter), the exact contract `context_tree_paint_order` wants.
    // We thread by ENTITY so we can reuse the production fn verbatim.
    let idx_of_entity: std::collections::HashMap<Entity, usize> =
        entity_of.iter().map(|(i, e)| (*e, *i)).collect();
    // Build entity-keyed painters_z (in-flow only; the escaped tail is appended
    // per-root below, mirroring sub-pass 6f's `painters_z.extend(escaped)`).
    let painters_z_entities: std::collections::HashMap<Entity, Vec<Entity>> = painters_z
        .iter()
        .map(|(&ctx, painters)| {
            let mut list: Vec<Entity> = painters.iter().map(|i| entity_of[i]).collect();
            if let Some(escaped) = escaped_by_ctx.get(&ctx) {
                list.extend(escaped.iter().map(|i| entity_of[i]));
            }
            (entity_of[&ctx], list)
        })
        .collect();

    let painters_z_of =
        |e: Entity| -> Option<&[Entity]> { painters_z_entities.get(&e).map(|v| v.as_slice()) };

    // Structural invariant (debug-gated): the context tree we hand the
    // production walk must be well-formed — no entity appears in two
    // `painters_z` lists and no context lists itself — otherwise
    // `context_tree_paint_order` would recurse forever. This guards `realize`
    // against future regressions in the escape / collection logic; it is a
    // property of the BRIDGE, not of the code under test, so it is a
    // `debug_assert` (off in release proptest runs).
    #[cfg(debug_assertions)]
    {
        let mut seen: std::collections::HashSet<Entity> = std::collections::HashSet::new();
        for (&ctx, list) in &painters_z_entities {
            for &p in list {
                debug_assert_ne!(p, ctx, "realize produced a self-referential context");
                debug_assert!(
                    seen.insert(p),
                    "realize listed entity {p:?} in two painters_z lists"
                );
            }
        }
    }

    // Walk the production context-tree paint order from each forest root.
    let mut order: Vec<Entity> = Vec::new();
    for (root_i, _root) in scene.roots.iter().enumerate() {
        let root_idx = root_preorder_index(scene, root_i);
        context_tree_paint_order(entity_of[&root_idx], &painters_z_of, &mut order);
    }

    // The escaped top-layer members were merged into each ROOT context's
    // `painters_z` tail via the production split — layout sub-pass 6f computes
    // that tail with `partition_top_layer` and appends it
    // (`painters_z.extend(escaped)`), exactly what `realize` mirrors above — so
    // the production walk placed the tail after the in-flow painters and `order`
    // IS the paint order. (`partition_top_layer` operates on ONE root context's
    // list, not the flattened multi-context order: a top-layer ROOT legitimately
    // paints first as its own tree's root, so feeding the global `order` through
    // it would wrongly reorder. Global top-layer DOMINANCE is the job of the
    // `top_layer_dominates` predicate, not of this bridge.)

    // Build the ExtractedNode for each entity in paint order.
    let nodes: Vec<ExtractedNode> = order
        .iter()
        .map(|&e| {
            let n = by_idx[&idx_of_entity[&e]];
            extracted_node(e, n)
        })
        .collect();

    // context membership map (entity → owning context root entity) + the
    // top-layer membership map, both over the painted entities.
    let context_of: std::collections::HashMap<Entity, Entity> = order
        .iter()
        .map(|&e| {
            let idx = idx_of_entity[&e];
            (e, entity_of[&root_context(idx)])
        })
        .collect();
    // Effective top-layer membership: a node is "in the top layer" iff it OR a
    // document ancestor escaped (a descendant of an escaped node paints INSIDE
    // that escaped context, so it is part of the top layer). The value is the
    // NEAREST top-layer ancestor's variant (inclusive of self) — the rank source
    // for the dominance tail — or `None` for a purely in-flow node. The
    // dominance predicate reads this, not the per-node own membership, so a
    // normal child of a top-layer node is not mistaken for an in-flow node that
    // "paints after the top layer".
    let effective_top_layer = |mut idx: usize| -> TopLayer {
        loop {
            let tl = by_idx[&idx].top_layer;
            if tl != TopLayer::None {
                return tl;
            }
            match by_idx[&idx].parent {
                Some(p) => idx = p,
                None => return TopLayer::None,
            }
        }
    };
    let top_layer_of: std::collections::HashMap<Entity, TopLayer> = order
        .iter()
        .map(|&e| (e, effective_top_layer(idx_of_entity[&e])))
        .collect();

    // Each forming context's full PAINTED region — exactly what the production
    // `context_tree_paint_order` emits for that context root (root + every
    // nested context's region as a unit; for the tree root, including the
    // escaped top-layer tail). Because the global `order` is the concatenation
    // of these walks descending as units, each region is a contiguous run — the
    // property `contexts_do_not_interleave` checks.
    let context_members: std::collections::HashMap<Entity, Vec<Entity>> = forms
        .iter()
        .map(|&ctx| {
            let mut region = Vec::new();
            context_tree_paint_order(entity_of[&ctx], &painters_z_of, &mut region);
            (entity_of[&ctx], region)
        })
        .collect();

    // Each forming context's DIRECT painters in the order the SHARED production
    // assembly returned (excluding the escaped top-layer tail — that tail is
    // ordered by `top_layer_paint_rank`, not `paint_key`, and is checked by
    // `top_layer_dominates`). `paint_key_of` gives every painted entity its
    // production paint key. Together they let `paint_order_respects_paint_key`
    // observe the shared assembly's output. NB: the regression protection is the
    // shared call above, not this predicate; a reversed production 6f sort reds
    // the invariant because `realize` ran the reversed code, not because the
    // predicate re-derives anything.
    let painters_of_ctx: std::collections::HashMap<Entity, Vec<Entity>> = painters_z
        .iter()
        .map(|(&ctx, painters)| {
            (
                entity_of[&ctx],
                painters.iter().map(|i| entity_of[i]).collect(),
            )
        })
        .collect();
    let paint_key_of: std::collections::HashMap<Entity, (u8, i32)> = order
        .iter()
        .map(|&e| (e, node_paint_key(by_idx[&idx_of_entity[&e]])))
        .collect();

    Realized {
        nodes: ExtractedNodes {
            nodes,
            ..Default::default()
        },
        context_of,
        context_members,
        top_layer_of,
        painters_of_ctx,
        paint_key_of,
        name_of,
    }
}

/// One flattened node with its pre-order index + parent link.
struct FlatNode {
    idx: usize,
    parent: Option<usize>,
    is_root: bool,
    name: String,
    position_kind: PositionKind,
    z_index: Option<i32>,
    isolation: bool,
    top_layer: TopLayer,
    transform: GenTransform,
    size: (f32, f32),
    background: Option<[f32; 4]>,
}

impl FlatNode {
    /// `true` iff the node is positioned (non-`Static`). The paint tier + the
    /// z-index stacking-context trigger both apply only to positioned nodes; a
    /// `Static` node's `z_index` is ignored (CSS quirk, mirroring production's
    /// `paint_key` / `forms_stacking_context`).
    fn positioned(&self) -> bool {
        !matches!(self.position_kind, PositionKind::Static)
    }

    /// The stacking-context formation triggers we model — the generator-side
    /// mirror of production's `forms_stacking_context` over the triggers this
    /// scene domain can express: root, `Isolation::Isolate`, POSITIONED with an
    /// explicit `z-index` (trigger 1 — a static node's z is ignored), non-identity
    /// transform, and — so it hosts its own escaped subtree — any top-layer
    /// member (a top-layer node always escapes as a context root, paint-order
    /// § 4.1). Render-side formers (opacity/filter/blend) + containment are not
    /// modeled by the scene generator.
    fn forms_context(&self) -> bool {
        self.is_root
            || self.isolation
            || (self.positioned() && self.z_index.is_some())
            || !self.transform.is_identity()
            || self.top_layer != TopLayer::None
    }

    /// The production [`Stacking`] this node maps to — fed straight into the
    /// production [`paint_key`] so the invariant keys identically. `z_index:
    /// None` → `ZIndex::Auto`; the `top_layer`/`isolation` fields are carried for
    /// fidelity though `paint_key` ignores them.
    fn stacking(&self) -> Stacking {
        Stacking {
            z_index: match self.z_index {
                Some(z) => ZIndex::Layer(z),
                None => ZIndex::Auto,
            },
            isolation: if self.isolation {
                buiy_core::layout::Isolation::Isolate
            } else {
                buiy_core::layout::Isolation::Auto
            },
            top_layer: self.top_layer,
        }
    }
}

/// Flatten the tree pre-order, assigning monotonic indices.
fn flatten(node: &SceneNode, parent: Option<usize>, is_root: bool, out: &mut Vec<FlatNode>) {
    let idx = out.len();
    out.push(FlatNode {
        idx,
        parent,
        is_root,
        name: node.name.clone(),
        position_kind: node.position_kind,
        z_index: node.z_index,
        isolation: node.isolation,
        top_layer: node.top_layer,
        transform: node.transform,
        size: node.size,
        background: node.background,
    });
    for child in &node.children {
        flatten(child, Some(idx), false, out);
    }
}

/// The pre-order index of root `root_i` in the flattened forest.
fn root_preorder_index(scene: &Scene, root_i: usize) -> usize {
    let mut count = 0usize;
    for r in &scene.roots[..root_i] {
        count += subtree_size(r);
    }
    count
}

fn subtree_size(node: &SceneNode) -> usize {
    1 + node.children.iter().map(subtree_size).sum::<usize>()
}

/// The (tier, z) paint key for a flat node, computed by the PRODUCTION
/// [`paint_key`] over the node's `(Stacking, PositionKind)` — never a private
/// copy. Covers all four tiers: negative-z (0), in-flow non-positioned (1),
/// positioned-auto-z (2), positive-z ascending (3); a static node's z is ignored
/// (tier 1). Threading production here is what makes a 6f-sort reversal red the
/// metamorphic invariant (testing-audit #6 / #13).
fn node_paint_key(n: &FlatNode) -> (u8, i32) {
    paint_key(Some(&n.stacking()), n.position_kind)
}

/// Build the `ExtractedNode` for one realized node. Position is a deterministic
/// per-index offset (the geometry the predicates assert on is `size`, which
/// comes straight from the generated box); `clip` mirrors the production
/// full-view sentinel (`None`) for top-layer members and `Some(box)` otherwise.
fn extracted_node(entity: Entity, n: &FlatNode) -> ExtractedNode {
    let position = Vec2::new((n.idx as f32) * 8.0, (n.idx as f32) * 8.0);
    let size = Vec2::new(n.size.0, n.size.1);
    let color = match n.background {
        Some([r, g, b, a]) => Color::srgba(r, g, b, a),
        None => Color::NONE,
    };
    let clip = if n.top_layer != TopLayer::None {
        // Top-layer members are unclipped (full-view sentinel, § 3.2).
        None
    } else {
        Some(ClipRect {
            min: position,
            max: position + size,
        })
    };
    ExtractedNode {
        entity,
        position,
        size,
        color,
        clip,
        group: None,
        // The synthetic scene carries no UiTransform; paint axis-aligned.
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(name: &str, children: Vec<SceneNode>) -> SceneNode {
        SceneNode {
            name: name.to_string(),
            children,
            position_kind: PositionKind::Static,
            z_index: None,
            isolation: false,
            top_layer: TopLayer::None,
            transform: GenTransform::IDENTITY,
            size: (10.0, 10.0),
            background: None,
        }
    }

    /// A normal CHILD of an escaped top-layer node is itself "in the top layer"
    /// (it paints inside the escaped context), so it inherits the top-layer
    /// membership — it must NOT be treated as an in-flow node that "paints after
    /// the top layer". Scene `n0 > n1 > {n2(Fullscreen) > {n3}}`.
    #[test]
    fn descendant_of_escaped_node_is_in_top_layer() {
        let mut n2 = plain("n2", vec![plain("n3", vec![])]);
        n2.top_layer = TopLayer::Fullscreen;
        let n1 = plain("n1", vec![n2]);
        let scene = Scene {
            roots: vec![plain("n0", vec![n1])],
        };
        let r = realize_full(&scene);
        // n3's effective membership is Fullscreen (via its escaped parent n2).
        let n3 = r
            .nodes
            .nodes
            .iter()
            .find(|n| r.name_of[&n.entity] == "n3")
            .expect("n3 realized")
            .entity;
        assert_eq!(
            r.top_layer_of[&n3],
            TopLayer::Fullscreen,
            "a descendant of an escaped node inherits its top-layer membership"
        );
        assert!(
            crate::invariant::top_layer_dominates(&r).is_ok(),
            "n3 painting inside n2's escaped region is not a dominance violation"
        );
    }

    /// Regression: `realize` handles a multi-root forest (every root forms its
    /// own context — the early cut marked only `roots[0]` as `is_root`, dropping
    /// later roots' subtrees). The GENERATOR only emits single-root scenes, but
    /// `realize` stays multi-root-correct as a robustness property.
    #[test]
    fn multi_root_forest_realizes_all() {
        let scene = Scene {
            roots: vec![plain("n0", vec![]), plain("n1", vec![plain("n2", vec![])])],
        };
        let nodes = realize(&scene);
        assert_eq!(
            nodes.nodes.len(),
            3,
            "all 3 nodes across both roots realized"
        );
    }

    /// A nested isolated context paints AS A UNIT at its document position
    /// among its parent's painters — its region is one contiguous block and the
    /// parent's region (which INCLUDES the nested block) is also contiguous.
    /// `n0 > n1(plain) > {n2(isolation), n3(plain)}`: the order is
    /// `[n0, n1, n2, n3]`, n2 forms its own context spanning just `[2..=2]`, and
    /// n0's region is the whole `[0..=3]` — neither interleaves.
    #[test]
    fn nested_isolated_context_is_a_contiguous_unit() {
        let mut n2 = plain("n2", vec![]);
        n2.isolation = true;
        let n1 = plain("n1", vec![n2, plain("n3", vec![])]);
        let scene = Scene {
            roots: vec![plain("n0", vec![n1])],
        };
        let r = realize_full(&scene);
        assert_eq!(r.nodes.nodes.len(), 4);
        assert!(
            crate::invariant::contexts_do_not_interleave(&r).is_ok(),
            "a nested isolated context is a contiguous unit, not interleaving"
        );
    }

    /// A top-layer node that is itself a forest ROOT does NOT escape (no parent
    /// context to escape to) — it must still realize exactly once, never list
    /// itself in its own `painters_z`.
    #[test]
    fn top_layer_root_does_not_self_reference() {
        let mut root = plain("n0", vec![plain("n1", vec![])]);
        root.top_layer = TopLayer::Modal;
        let scene = Scene { roots: vec![root] };
        let nodes = realize(&scene);
        assert_eq!(nodes.nodes.len(), 2, "the top-layer root + its child");
    }
}
