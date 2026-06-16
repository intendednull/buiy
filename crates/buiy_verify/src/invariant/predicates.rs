//! The Tier-3 predicate functions (invariants.md § "Predicate functions").
//!
//! Each is a free `pub fn` taking borrowed data and returning
//! `Result<(), Violation>` — NOT a bare `bool` — so a failing property prints
//! *which* relation broke and the offending names/indices. The `proptest!`
//! harness in `tests/invariant_predicates.rs` feeds them generated scenes; the
//! mutation meta-tests in `tests/invariant_mutations.rs` feed them hand-built
//! VIOLATING fixtures to prove each predicate has teeth (a predicate that never
//! fails is worthless).

use std::fmt;

use bevy::prelude::*;

use buiy_core::layout::{
    Length, Rotate, Scale, TopLayer, Translate, UiTransform, compose_transform,
    top_layer_paint_rank,
};
use buiy_core::render::extract::ExtractedNodes;
use buiy_core::render::instance::PackedInstance;

use super::scene::{GenTransform, Realized};

/// A broken invariant relation. Plain struct (no `thiserror`) to keep the dep
/// surface at zero: the `rule` names the predicate, the `detail` carries the
/// offending entity names / indices so the seed + this message reproduce the
/// failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The invariant that broke (a stable `&'static str` id).
    pub rule: &'static str,
    /// Human-readable specifics (which entity, which index, the bad value).
    pub detail: String,
}

impl Violation {
    /// Construct a violation. `pub(crate)` so sibling invariant modules (e.g.
    /// `bidi`) can report their own relations through the shared type.
    pub(crate) fn new(rule: &'static str, detail: impl Into<String>) -> Self {
        Self {
            rule,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.rule, self.detail)
    }
}

/// Tolerance for the metamorphic transform relations. A composed `Mat4` of
/// rotations + scales in `0.1..8.0` accumulates a few ULPs of f32 error; `1e-3`
/// is comfortably above that round-off yet far below any real composition bug
/// (a transposed factor, a dropped term) which shifts entries by `O(1)`.
pub const EPS: f32 = 1e-3;

// ---------------------------------------------------------------------------
// #1 — paint order is a TOTAL order over painted entities.
// ---------------------------------------------------------------------------

/// Paint order is a total order: no entity appears twice in
/// [`ExtractedNodes::nodes`]. Mirrors the non-re-sorting contract of the
/// stored paint order (`extract.rs` "Never re-sorted by render") — a duplicate
/// would mean the same box painted twice, a partial-re-extract or
/// context-walk bug.
///
/// (Stable equal-key order is a property of the *generator's* document order +
/// the production stable sort, exercised by `realize`; the observable invariant
/// here is no-duplicates over the realized list.)
pub fn paint_order_is_total(nodes: &ExtractedNodes) -> Result<(), Violation> {
    let mut seen = std::collections::HashSet::new();
    for (i, node) in nodes.nodes.iter().enumerate() {
        if !seen.insert(node.entity) {
            return Err(Violation::new(
                "paint_order_is_total",
                format!(
                    "entity {:?} appears more than once in painters_z (at index {i})",
                    node.entity
                ),
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// #2 — transform round-trips on the production `compose_transform`.
// ---------------------------------------------------------------------------

/// Three metamorphic relations on the COMPOSED `Mat4` from the production
/// [`compose_transform`] (`systems.rs`, compose `T·R·S·M`), within [`EPS`]:
///
/// - `translate(d) · translate(-d) ≈ I`  (translation is invertible),
/// - `rotate(2π) ≈ I`                     (a full turn is the identity),
/// - `scale(k)` scales every basis vector by its axis factor and leaves the
///   off-diagonals zero (a pure diagonal scale touches nothing else).
///
/// Operates on `compose_transform` OUTPUTS, never a re-implementation, so a
/// mis-applied *single* factor (a dropped term, a wrong sign, a transposed
/// matrix) reds this. Note the SCOPE: each relation feeds exactly one
/// non-identity factor, so an INTER-factor order swap (`T·R·S` vs `T·S·R`) is
/// invisible here by construction — that ordering is pinned independently by
/// buiy_core's own `compose_longhands_with_matrix_order` /
/// `compose_matrix_compose_product_order` unit tests, not by this predicate.
pub fn transform_roundtrips(t: &GenTransform) -> Result<(), Violation> {
    // (a) translate(d) · translate(-d) ≈ I.
    let d = Vec3::from_array(t.translate);
    let fwd = compose_transform(&UiTransform::default(), Some(&translate_of(d)), None, None);
    let back = compose_transform(&UiTransform::default(), Some(&translate_of(-d)), None, None);
    mat4_is_identity("transform_roundtrips/translate", fwd * back)?;

    // (b) rotate(2π) ≈ I. A full turn about the generated axis.
    let axis = Vec3::from_array(t.rotate_axis);
    let axis = if axis.length_squared() > 1e-6 {
        axis.normalize()
    } else {
        Vec3::Z
    };
    let full_turn = Quat::from_axis_angle(axis, std::f32::consts::TAU);
    let rot = compose_transform(
        &UiTransform::default(),
        None,
        Some(&Rotate(full_turn)),
        None,
    );
    mat4_is_identity("transform_roundtrips/rotate2pi", rot)?;

    // (c) scale(k) is a pure diagonal scale: diagonal == k, off-diagonals == 0.
    let k = t.scale;
    let s = compose_transform(
        &UiTransform::default(),
        None,
        None,
        Some(&Scale(k[0], k[1], k[2])),
    );
    mat4_is_pure_scale("transform_roundtrips/scale", s, k)?;
    Ok(())
}

fn translate_of(d: Vec3) -> Translate {
    Translate(Length::Px(d.x), Length::Px(d.y), Length::Px(d.z))
}

/// Assert a `Mat4` is the identity within [`EPS`] (every entry matches `I`). The
/// relation-check half of [`transform_roundtrips`], exposed so the mutation
/// meta-tests can feed it a deliberately mis-composed matrix and confirm it
/// REJECTS (the predicate's teeth, invariants.md § Verification).
pub fn mat4_is_identity(rule: &'static str, m: Mat4) -> Result<(), Violation> {
    check_diagonal(
        rule,
        m,
        [1.0, 1.0, 1.0, 1.0],
        "composition is not the identity",
    )
}

/// Assert a `Mat4` is a pure diagonal scale by `k`: diagonal == `[k.x,k.y,k.z,1]`
/// and every off-diagonal == 0 (within [`EPS`]). A mis-composed matrix
/// (`S·R·T` instead of the pure `S`) leaks an off-diagonal and is rejected — the
/// teeth the mutation meta-test exploits.
pub fn mat4_is_pure_scale(rule: &'static str, m: Mat4, k: [f32; 3]) -> Result<(), Violation> {
    check_diagonal(
        rule,
        m,
        [k[0], k[1], k[2], 1.0],
        "off-diagonal leaked or wrong factor",
    )
}

/// Assert `m` is a diagonal matrix with the given `diag` (within [`EPS`]):
/// every diagonal entry matches `diag[i]` and every off-diagonal is `0`. The
/// shared kernel of [`mat4_is_identity`] and [`mat4_is_pure_scale`].
fn check_diagonal(rule: &'static str, m: Mat4, diag: [f32; 4], why: &str) -> Result<(), Violation> {
    for (c, col) in m.to_cols_array_2d().iter().enumerate() {
        for (r, &value) in col.iter().enumerate() {
            let expected = if c == r { diag[c] } else { 0.0 };
            if (value - expected).abs() > EPS {
                return Err(Violation::new(
                    rule,
                    format!("M[{r}][{c}] = {value} ≠ {expected} ({why})"),
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// #3 — top-layer dominance.
// ---------------------------------------------------------------------------

/// Every `top_layer != None` node paints AFTER every normal-stacking node, and
/// the escaped tail is ordered by paint rank Fullscreen < Tooltip < Popover <
/// Modal — compared via the promoted [`top_layer_paint_rank`], NEVER the enum
/// discriminant (invariants.md deviation #3: the declared enum order is NOT the
/// paint order, so `#[derive(Ord)]` would dominate wrongly).
///
/// Takes the [`Realized`] (not bare `ExtractedNodes`) because `ExtractedNode`
/// carries no top-layer field — membership lives in
/// [`Realized::top_layer_of`].
pub fn top_layer_dominates(r: &Realized) -> Result<(), Violation> {
    let order = &r.nodes.nodes;
    let top_of = |e: Entity| r.top_layer_of.get(&e).copied().unwrap_or(TopLayer::None);
    let name = |e: Entity| {
        r.name_of
            .get(&e)
            .cloned()
            .unwrap_or_else(|| format!("{e:?}"))
    };

    // (a) once a top-layer node has painted, no NORMAL node may paint after it.
    let mut first_top: Option<usize> = None;
    for (i, node) in order.iter().enumerate() {
        let is_top = top_of(node.entity) != TopLayer::None;
        if is_top && first_top.is_none() {
            first_top = Some(i);
        }
        if !is_top && let Some(t) = first_top {
            return Err(Violation::new(
                "top_layer_dominates/normal_after_top",
                format!(
                    "normal node {} (index {i}) paints AFTER top-layer node at index {t}",
                    name(node.entity)
                ),
            ));
        }
    }

    // (b) the escaped tail is non-decreasing in paint rank.
    let mut prev_rank: Option<u8> = None;
    let mut prev_name = String::new();
    for node in order.iter() {
        let tl = top_of(node.entity);
        if tl == TopLayer::None {
            continue;
        }
        let rank = top_layer_paint_rank(tl);
        if let Some(p) = prev_rank
            && rank < p
        {
            return Err(Violation::new(
                "top_layer_dominates/tail_misordered",
                format!(
                    "top-layer {} (rank {rank}) paints after {prev_name} (rank {p}) — \
                     tail not Fullscreen<Tooltip<Popover<Modal",
                    name(node.entity)
                ),
            ));
        }
        prev_rank = Some(rank);
        prev_name = name(node.entity);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// #4 — finiteness / non-negativity.
// ---------------------------------------------------------------------------

/// Every [`ExtractedNode`](buiy_core::render::extract::ExtractedNode) size is
/// finite and non-negative (the un-flipped logical box, `extract.rs`). A `NaN`
/// or negative size is a real extract/layout bug — it would corrupt the packed
/// quad or invert it.
pub fn all_finite(nodes: &ExtractedNodes) -> Result<(), Violation> {
    for (i, node) in nodes.nodes.iter().enumerate() {
        for (axis, v) in [("x", node.size.x), ("y", node.size.y)] {
            if !v.is_finite() || v < 0.0 {
                return Err(Violation::new(
                    "all_finite",
                    format!("node index {i} size.{axis} = {v} (must be finite and ≥ 0)"),
                ));
            }
        }
        for (axis, v) in [("x", node.position.x), ("y", node.position.y)] {
            if !v.is_finite() {
                return Err(Violation::new(
                    "all_finite",
                    format!("node index {i} position.{axis} = {v} (must be finite)"),
                ));
            }
        }
    }
    Ok(())
}

/// Every [`PackedInstance`] field is finite and `rect_size[1] ≥ 0` DIRECTLY
/// (the y-flip lives in the view uniform now, so packed height stays positive —
/// `instance.rs`, invariants.md deviation #2: no un-flip needed). The clip
/// sentinels (`±INFINITY`) are the one allowed non-finite — they encode "no
/// clip" and are checked separately.
pub fn all_finite_packed(packed: &[PackedInstance]) -> Result<(), Violation> {
    for (i, p) in packed.iter().enumerate() {
        let finite_fields: [(&str, f32); 9] = [
            ("rect_pos.x", p.rect_pos[0]),
            ("rect_pos.y", p.rect_pos[1]),
            ("rect_size.x", p.rect_size[0]),
            ("rect_size.y", p.rect_size[1]),
            ("color.r", p.color[0]),
            ("color.g", p.color[1]),
            ("color.b", p.color[2]),
            ("color.a", p.color[3]),
            ("radius", p.radius),
        ];
        for (field, v) in finite_fields {
            if !v.is_finite() {
                return Err(Violation::new(
                    "all_finite_packed",
                    format!("instance {i} {field} = {v} (must be finite)"),
                ));
            }
        }
        // Packed height is POSITIVE (deviation #2) — the y-flip is in the view
        // uniform, so a negative packed height is a real packing bug.
        if p.rect_size[1] < 0.0 {
            return Err(Violation::new(
                "all_finite_packed",
                format!(
                    "instance {i} rect_size[1] = {} < 0 (height must stay positive; \
                     the y-flip lives in the view uniform)",
                    p.rect_size[1]
                ),
            ));
        }
        // The clip AABB must be finite OR the full-view sentinel (both
        // components ±INFINITY). A mixed finite/infinite clip is a packing bug.
        for (field, lo, hi) in [
            ("clip_min", p.clip_min[0], p.clip_min[1]),
            ("clip_max", p.clip_max[0], p.clip_max[1]),
        ] {
            let both_finite = lo.is_finite() && hi.is_finite();
            let both_inf = lo.is_infinite() && hi.is_infinite();
            if !(both_finite || both_inf) || lo.is_nan() || hi.is_nan() {
                return Err(Violation::new(
                    "all_finite_packed",
                    format!("instance {i} {field} = [{lo}, {hi}] (NaN or mixed finite/sentinel)"),
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// #5 — z-isolated containment (no context interleaving).
// ---------------------------------------------------------------------------

/// No stacking context interleaves another: a stacking context paints as a
/// UNIT, so every entity in a context's painted region (the context root + all
/// nested contexts' regions, [`Realized::context_members`]) forms a CONTIGUOUS
/// run in the flattened order — no foreign entity sits between two of them.
/// Guards against subtree leakage across an `isolation` / z boundary (a
/// context-walk that flattened instead of descending as a unit would
/// interleave). A nested context legitimately sits AMONG its parent's direct
/// painters — that is the "descend as a unit at this position" rule, and it is
/// NOT interleaving: the nested region is itself one contiguous block.
pub fn contexts_do_not_interleave(r: &Realized) -> Result<(), Violation> {
    // Index of each entity in the flattened paint order.
    let index_of: std::collections::HashMap<Entity, usize> = r
        .nodes
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.entity, i))
        .collect();

    for (&ctx, members) in &r.context_members {
        let mut indices: Vec<usize> = members
            .iter()
            .filter_map(|e| index_of.get(e).copied())
            .collect();
        if indices.is_empty() {
            continue;
        }
        indices.sort_unstable();
        let span = indices[indices.len() - 1] - indices[0] + 1;
        if span != indices.len() {
            let name = r
                .name_of
                .get(&ctx)
                .cloned()
                .unwrap_or_else(|| format!("{ctx:?}"));
            return Err(Violation::new(
                "contexts_do_not_interleave",
                format!(
                    "context {name}'s painted region spans indices {}..={} ({span} slots) but \
                     has {} members — a foreign entity interleaves it",
                    indices[0],
                    indices[indices.len() - 1],
                    indices.len(),
                ),
            ));
        }
    }
    Ok(())
}
