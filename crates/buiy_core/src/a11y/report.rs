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
//! - **a `--- text & layout ---` section** — every laid-out entity carrying a
//!   [`Text`] (or an [`A11yLabel`]), so **plain, non-a11y `Text`** content (which
//!   the semantic tree drops — a role-less label never becomes a node) **and
//!   zero-size "invisible content"** (a bug class the widget-catalog campaign hit)
//!   both surface.
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
/// 2. a `--- text & layout ---` section listing every laid-out text-bearing
///    entity as `size=wxh text="…"` (or `label="…"`), flagging any `ZERO-SIZE`
///    box — the invisible-content signal.
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
            Some((n.r#ref, (layout.position, layout.size)))
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

    write_text_section(&mut out, world);
    out
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
    if state.disabled {
        tokens.push("disabled".to_string());
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

/// Append the `--- text & layout ---` section: every laid-out entity carrying a
/// [`Text`] or an [`A11yLabel`], one line each, in reading order. Surfaces plain
/// (role-less) `Text` the semantic tree drops, and flags zero-size boxes.
fn write_text_section(out: &mut String, world: &mut World) {
    let mut rows: Vec<(Vec2, Vec2, Entity, String)> = Vec::new();
    let mut query = world.query::<(Entity, &ResolvedLayout, Option<&A11yLabel>, Option<&Text>)>();
    for (entity, layout, label, text) in query.iter(world) {
        // Prefer the rendered `Text` glyph content (the thing the semantic tree
        // cannot show); fall back to the a11y label for a labeled-but-textless
        // node. Skip pure structural boxes (no text signal at all).
        let content = match (text, label) {
            (Some(text), _) => format!("text={:?}", text.0),
            (None, Some(label)) => format!("label={:?}", label.0),
            (None, None) => continue,
        };
        rows.push((layout.position, layout.size, entity, content));
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
