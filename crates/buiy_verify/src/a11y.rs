//! AccessKit tree snapshot — serializes Buiy's `A11yTreeBuilder` view to
//! stable JSON suitable for golden-file comparison.
//! See: docs/specs/2026-05-07-buiy-foundation/verification.md (CI gate #3).

use buiy_core::a11y::{A11yNodeView, A11yRole};
use buiy_core::a11y::translate::node_id_for;
use serde::Serialize;

// LINT: Field order here is the snapshot wire format. Do not reorder
// without coordinating golden-file regeneration in every consumer.
#[derive(Serialize)]
struct WireNode<'a> {
    // Canonical AccessKit NodeId (= entity.to_bits()+1), set in snapshot_tree;
    // NOT raw entity bits. (A future phase renames this field to `ref`.)
    entity: u64,
    role: &'a str,
    name: &'a str,
    description: &'a str,
    focusable: bool,
}

// LINT: Keep this match in sync with `buiy_core::a11y::A11yRole`.
// `A11yRole` is `#[non_exhaustive]`, so Rust requires a wildcard arm and
// the compiler will *not* surface unhandled variants for us. When new
// roles land in `buiy_core::a11y` (e.g. the v0.x full ARIA taxonomy per
// accessibility.md § 3.11) the unknown stringification below shows up
// in snapshot goldens and PRs touching that file must add the named
// arms in the same PR. The fallback exists so external snapshots stay
// well-formed across version skew, not as a substitute for keeping this
// table current.
fn role_to_str(r: A11yRole) -> &'static str {
    match r {
        A11yRole::Generic => "Generic",
        A11yRole::Button => "Button",
        A11yRole::Link => "Link",
        A11yRole::Image => "Image",
        A11yRole::Text => "Text",
        A11yRole::Heading => "Heading",
        A11yRole::Dialog => "Dialog",
        A11yRole::AlertDialog => "AlertDialog",
        A11yRole::Tooltip => "Tooltip",
        A11yRole::Checkbox => "Checkbox",
        A11yRole::Switch => "Switch",
        A11yRole::Slider => "Slider",
        A11yRole::TextInput => "TextInput",
        A11yRole::MultilineTextInput => "MultilineTextInput",
        A11yRole::Region => "Region",
        A11yRole::Group => "Group",
        _ => "Unknown",
    }
}

pub fn snapshot_tree(nodes: &[A11yNodeView]) -> String {
    let wire: Vec<WireNode> = nodes
        .iter()
        .map(|n| WireNode {
            // Canonical AccessKit ref: node_id_for(entity) = to_bits() + 1.
            // This is the id an inbound ActionRequest's `target` carries, so the
            // snapshot's `entity` field round-trips with `entity_for_node_id`.
            entity: node_id_for(n.entity).0,
            role: role_to_str(n.role),
            name: &n.name,
            description: &n.description,
            focusable: n.focusable,
        })
        .collect();
    // `serde_json::to_string` on a `Vec<WireNode>` cannot fail: WireNode is
    // a fixed-shape struct of `u64 + &str + &str + &str + bool`, and
    // serde_json only errors on map keys that aren't strings, custom
    // Serialize impls that fail, or recursion-limit overruns. None apply
    // here. If WireNode ever grows a non-trivial Serialize, return a
    // `Result` from this fn instead of `unreachable!`.
    match serde_json::to_string(&wire) {
        Ok(s) => s,
        Err(e) => unreachable!("WireNode serialization is infallible by construction; got: {e}"),
    }
}

/// Returns `None` if identical, `Some(diff_text)` otherwise. Phase 0
/// emits a coarse `LEFT:\n…RIGHT:\n…` dump; v0.x will swap to a unified
/// diff via the `similar` crate when the AT-consumer harness (gate #15)
/// lands.
pub fn diff_snapshots(left: &str, right: &str) -> Option<String> {
    if left == right {
        None
    } else {
        Some(format!("LEFT:\n{}\n\nRIGHT:\n{}\n", left, right))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hardcoded list of every Phase 0 `A11yRole` variant. The test
    /// below checks each maps to a non-`"Unknown"` string. New variants
    /// added to `buiy_core::a11y::A11yRole` MUST also be added here AND
    /// to `role_to_str` — renaming a variant will break this test at
    /// compile time, which is the forcing function. (The reverse
    /// direction — adding to A11yRole without updating role_to_str — is
    /// caught downstream by snapshot golden-file diffs surfacing
    /// `"Unknown"`.)
    const KNOWN_ROLES: &[A11yRole] = &[
        A11yRole::Generic,
        A11yRole::Button,
        A11yRole::Link,
        A11yRole::Image,
        A11yRole::Text,
        A11yRole::Heading,
        A11yRole::Dialog,
        A11yRole::AlertDialog,
        A11yRole::Tooltip,
        A11yRole::Checkbox,
        A11yRole::Switch,
        A11yRole::Slider,
        A11yRole::TextInput,
        A11yRole::MultilineTextInput,
        A11yRole::Region,
        A11yRole::Group,
    ];

    #[test]
    fn role_to_str_handles_every_known_variant() {
        for role in KNOWN_ROLES {
            let s = role_to_str(*role);
            assert_ne!(
                s, "Unknown",
                "role {role:?} stringifies to Unknown — add it to role_to_str",
            );
        }
    }
}
