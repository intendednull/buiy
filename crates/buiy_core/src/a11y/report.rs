//! The **agent-facing snapshot serializer** (Track A — the probe / "eyes"):
//! [`snapshot_report`] renders the live semantic tree as a **stable, diffable,
//! Playwright-style indented text tree**, augmented with each node's layout rect
//! and a trailing text section — the form an agent's build/test loop reads to
//! observe an author → build → run → *inspect* cycle without a GPU.
//!
//! It projects from the **same** [`snapshot`] the
//! in-process driver exposes (no second tree path), then augments it with two
//! things the bare [`SemanticTree`](super::inprocess::SemanticTree) omits but
//! agents proved they need:
//!
//! - **layout rects** — each node's [`ResolvedLayout`] (position + size), so
//!   "where is it / how big is it" is observable, resolved by `entity_for_node_id`;
//! - **a `--- text & layout ---` section** — the signal the tree can't carry:
//!   **plain, non-a11y `Text`** content (which the semantic tree drops — a
//!   role-less label never becomes a node) and **zero-size "invisible content"**
//!   (a bug class the widget-catalog campaign hit). It lists laid-out
//!   text-bearing entities but does not re-echo a node's a11y label when that node
//!   is already in the tree section (unless it's zero-size), to keep the report
//!   dense.
//!
//! The output is **deterministic**: the tree walks in canonical document order
//! (roots then children, following the snapshot's `children` refs), and the text
//! section is sorted by reading order (top-to-bottom, left-to-right, then
//! `Entity` as the total-order tiebreak) — no `HashMap` iteration order leaks into
//! the string, so two runs of the same settled frame diff to nothing.
//!
//! JSON is a trivial follow-on (`serde` on `SemanticTree`); this indented text
//! form is v1 — token-efficient and human+agent readable.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use accesskit::{NodeId, Toggled};
use bevy::prelude::*;

use super::inprocess::{NodeState, SemanticNode, TreeView, snapshot};
use crate::a11y::A11yLabel;
use crate::a11y::translate::entity_for_node_id;
use crate::components::ResolvedLayout;
use crate::text::Text;

/// Render the live a11y tree + layout as an agent-facing text report (Track A).
///
/// Two sections:
///
/// 1. the **semantic tree**, one line per node, indented by depth —
///    `Role "name" [state] @x,y wxh` (the `[state]` bracket is present only when a
///    node carries observable state: `checked`/`unchecked`/`mixed`,
///    `expanded`/`collapsed`, `disabled`, `focused`, `value=…`);
/// 2. a `--- text & layout ---` section listing laid-out text-bearing entities as
///    `size=wxh text="…"` (or `label="…"`), flagging any `ZERO-SIZE` box — the
///    invisible-content signal. A node's a11y label is not re-echoed here when the
///    node already appears in the tree section (unless it is zero-size).
///
/// Reads the tree from [`snapshot`] (so the caller must have driven at least one
/// `app.update()`; this fn does not tick the schedule — same contract as
/// [`snapshot`]). Pure read-back over the world; deterministic and diffable.
pub fn snapshot_report(world: &mut World) -> String {
    let tree = snapshot(world, TreeView::Unmerged);

    // Resolve each node's rect once, up front, so the recursive tree walk is a
    // pure function of already-collected data (no `&World` threaded through it).
    // `HashMap` is used for O(1) lookup only — its iteration order never reaches
    // the output, so determinism is preserved.
    let rects: HashMap<NodeId, (Vec2, Vec2)> = tree
        .nodes
        .iter()
        .filter_map(|n| {
            let entity = entity_for_node_id(n.r#ref)?;
            let layout = world.get::<ResolvedLayout>(entity)?;
            Some((n.r#ref, (absolute_pos(world, entity, layout), layout.size)))
        })
        .collect();
    let by_id: HashMap<NodeId, &SemanticNode> = tree.nodes.iter().map(|n| (n.r#ref, n)).collect();

    // Roots = nodes that are not any node's child, kept in canonical document
    // order (the order `snapshot` emits them).
    let child_ids: HashSet<NodeId> = tree
        .nodes
        .iter()
        .flat_map(|n| n.children.iter().copied())
        .collect();

    let mut out = String::new();
    let mut visited: HashSet<NodeId> = HashSet::new();
    for node in &tree.nodes {
        if !child_ids.contains(&node.r#ref) {
            write_node(&mut out, node.r#ref, 0, &by_id, &rects, &mut visited);
        }
    }

    // Entities already shown in the tree section (with their role, name, and rect)
    // so the text section can skip re-listing their label — it exists to surface
    // what the tree CAN'T show (plain role-less text, zero-size boxes), not to echo
    // every a11y node.
    let a11y_entities: HashSet<Entity> = tree
        .nodes
        .iter()
        .filter_map(|n| entity_for_node_id(n.r#ref))
        .collect();

    write_text_section(&mut out, world, &a11y_entities);
    out
}

/// The **absolute** top-left of a laid-out entity, in the same logical-px,
/// y-down space as [`ResolvedLayout`]. [`ResolvedLayout::position`] is
/// *parent-relative* and must NOT be used as an absolute coordinate
/// (`components.rs` § `ResolvedLayout`); the transform bridge accumulates
/// `position − ancestor_scroll` into `GlobalTransform`, so a **nested** node
/// reports its real screen position (not its offset within its parent) and the
/// reading-order sort stays correct for real UIs. The bridge runs GPU-free in
/// `Update` (`CorePlugin`), so this is populated under `BuiyProbePlugin`. Falls
/// back to the parent-relative position only if the bridge has not run yet —
/// harmless for roots, which are already absolute.
fn absolute_pos(world: &World, entity: Entity, layout: &ResolvedLayout) -> Vec2 {
    world
        .get::<GlobalTransform>(entity)
        .map(|gt| gt.translation().truncate())
        .unwrap_or(layout.position)
}

/// Emit one node line, indented by `depth`, then recurse into its children in
/// document order. `visited` guards against re-emitting a node (defensive against
/// a malformed non-tree graph — stay total, never loop forever).
fn write_node(
    out: &mut String,
    id: NodeId,
    depth: usize,
    by_id: &HashMap<NodeId, &SemanticNode>,
    rects: &HashMap<NodeId, (Vec2, Vec2)>,
    visited: &mut HashSet<NodeId>,
) {
    if !visited.insert(id) {
        return;
    }
    let Some(node) = by_id.get(&id) else {
        return;
    };

    let indent = "  ".repeat(depth);
    let tokens = state_tokens(&node.state);
    let state = if tokens.is_empty() {
        String::new()
    } else {
        format!(" [{}]", tokens.join(" "))
    };
    let rect = match rects.get(&id) {
        Some((pos, size)) => format!("@{:.0},{:.0} {:.0}x{:.0}", pos.x, pos.y, size.x, size.y),
        // No `ResolvedLayout` on the backing entity (not laid out) — mark it,
        // don't drop the node.
        None => "@?".to_string(),
    };
    // `{:?}` on the role gives the enum variant (`Button`); on the name gives a
    // quoted, escaped string (`"Save"`) — the `Role "name"` form.
    let _ = writeln!(out, "{indent}{:?} {:?}{state} {rect}", node.role, node.name);

    for &child in &node.children {
        write_node(out, child, depth + 1, by_id, rects, visited);
    }
}

/// The compact, present-only state tokens for a node's `[state]` bracket. Empty
/// when the node carries no observable state (the bracket is then omitted).
fn state_tokens(state: &NodeState) -> Vec<String> {
    let mut tokens = Vec::new();
    match state.toggled {
        Some(Toggled::True) => tokens.push("checked".to_string()),
        Some(Toggled::False) => tokens.push("unchecked".to_string()),
        Some(Toggled::Mixed) => tokens.push("mixed".to_string()),
        None => {}
    }
    match state.expanded {
        Some(true) => tokens.push("expanded".to_string()),
        Some(false) => tokens.push("collapsed".to_string()),
        None => {}
    }
    match state.selected {
        Some(true) => tokens.push("selected".to_string()),
        Some(false) => tokens.push("unselected".to_string()),
        None => {}
    }
    if state.disabled {
        tokens.push("disabled".to_string());
    }
    if state.modal {
        tokens.push("modal".to_string());
    }
    if state.focused {
        tokens.push("focused".to_string());
    }
    // A text value (single-line field) takes priority over a numeric one; a
    // valued range (slider/spinner) reports `numeric_value`.
    if let Some(value) = &state.value {
        tokens.push(format!("value={value:?}"));
    } else if let Some(numeric) = state.numeric_value {
        tokens.push(format!("value={numeric}"));
    }
    tokens
}

/// Append the `--- text & layout ---` section — the signal the semantic tree
/// CAN'T show: **plain, role-less `Text`** the tree drops, and **zero-size**
/// ("invisible") boxes. One line per laid-out text-bearing entity, in reading
/// order. Entities already listed in the tree section (`a11y_entities`) are not
/// re-echoed by their label unless they are zero-size (the invisibility signal a
/// tree line carries no room for).
fn write_text_section(out: &mut String, world: &mut World, a11y_entities: &HashSet<Entity>) {
    let mut rows: Vec<(Vec2, Vec2, Entity, String)> = Vec::new();
    let mut query = world.query::<(
        Entity,
        &ResolvedLayout,
        Option<&GlobalTransform>,
        Option<&A11yLabel>,
        Option<&Text>,
    )>();
    for (entity, layout, transform, label, text) in query.iter(world) {
        let zero_size = layout.size.x == 0.0 || layout.size.y == 0.0;
        // Prefer the rendered `Text` glyph content (the thing the semantic tree
        // cannot show — glyph text is independent of a node's accessible name);
        // fall back to the a11y label for a labeled-but-textless entity, but skip
        // that label when the entity is already a tree node (redundant) and not
        // zero-size. Skip pure structural boxes (no text signal at all).
        let content = match (text, label) {
            (Some(text), _) => format!("text={:?}", text.0),
            (None, Some(label)) => {
                if a11y_entities.contains(&entity) && !zero_size {
                    continue;
                }
                format!("label={:?}", label.0)
            }
            (None, None) => continue,
        };
        // Sort by the ABSOLUTE position (parent-relative `ResolvedLayout.position`
        // would put a deeply-nested small-`y` node ahead of an earlier top-level
        // one), same source as the tree rects.
        let pos = transform
            .map(|t| t.translation().truncate())
            .unwrap_or(layout.position);
        rows.push((pos, layout.size, entity, content));
    }

    // Deterministic reading order: top-to-bottom, left-to-right, `Entity` as the
    // total-order tiebreak (query iteration order is not relied upon).
    rows.sort_by(|a, b| {
        a.0.y
            .total_cmp(&b.0.y)
            .then(a.0.x.total_cmp(&b.0.x))
            .then(a.2.cmp(&b.2))
    });

    out.push_str("--- text & layout ---\n");
    for (_pos, size, _entity, content) in &rows {
        let flag = if size.x == 0.0 || size.y == 0.0 {
            "  [ZERO-SIZE]"
        } else {
            ""
        };
        let _ = writeln!(out, "size={:.0}x{:.0} {content}{flag}", size.x, size.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The absolute-position helper (F1): prefer the accumulated `GlobalTransform`
    /// over the *parent-relative* `ResolvedLayout.position`, falling back to the
    /// layout position only when no `GlobalTransform` has propagated yet.
    #[test]
    fn absolute_pos_prefers_global_transform_over_parent_relative_layout() {
        let mut world = World::new();
        let layout = ResolvedLayout {
            position: Vec2::new(8.0, 8.0), // parent-relative offset
            size: Vec2::new(50.0, 20.0),
        };

        // With a `GlobalTransform` (parent 300,200 + local 8,8 = 308,208 absolute),
        // the helper reports the absolute top-left, NOT the (8,8) parent offset.
        let nested = world
            .spawn((
                layout.clone(),
                GlobalTransform::from(Transform::from_xyz(308.0, 208.0, 0.0)),
            ))
            .id();
        assert_eq!(
            absolute_pos(&world, nested, &layout),
            Vec2::new(308.0, 208.0),
        );

        // Without a `GlobalTransform`, fall back to the layout position (harmless
        // for roots, which are already absolute).
        let bare = world.spawn(layout.clone()).id();
        assert_eq!(absolute_pos(&world, bare, &layout), Vec2::new(8.0, 8.0));
    }

    /// The `[state]` token set — including `selected`/`unselected` and `modal`
    /// (F5), which the header advertises but the first cut dropped. An
    /// all-inert state yields no bracket at all.
    #[test]
    fn state_tokens_cover_every_observable_flag() {
        assert!(state_tokens(&NodeState::default()).is_empty());

        let full = NodeState {
            toggled: Some(Toggled::True),
            expanded: Some(false),
            selected: Some(true),
            disabled: true,
            modal: true,
            focused: true,
            ..NodeState::default()
        };
        let tokens = state_tokens(&full);
        for expected in [
            "checked",
            "collapsed",
            "selected",
            "disabled",
            "modal",
            "focused",
        ] {
            assert!(
                tokens.iter().any(|t| t == expected),
                "state tokens {tokens:?} must include {expected:?}",
            );
        }

        assert_eq!(
            state_tokens(&NodeState {
                selected: Some(false),
                ..NodeState::default()
            }),
            vec!["unselected".to_string()],
        );
    }
}
