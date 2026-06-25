//! Accessible-name computation (ACCNAME 1.2) — **a function, not a component**.
//!
//! The accessible name is *derived every build, never stored* (semantic-tree.md
//! §6): `compute_accessible_name` runs in `build_tree` to fill
//! [`A11yNodeView::name`](super::A11yNodeView), which the derive fold then emits
//! through `set_label` (the single emission point, `translate.rs`).
//!
//! # Precedence (ACCNAME 1.2 — semantic-tree.md §6, wai-aria-apg/name-computation.md)
//!
//! ```text
//! 1. labelledby   (aria-labelledby — resolves OTHER nodes' names)   ── P1b
//! 2. label        (aria-label — the explicit host label)           ── P1a
//! 3. value        (a control's current text value)                 ── P1a
//! 4. placeholder  (the empty-input prompt)                         ── P1a
//! 5. contents     (the node's own subtree text)                    ── P1b
//! 6. title        (the tooltip / title attribute)                  ── (no local source yet)
//! ```
//!
//! # Arm sourcing
//!
//! The **purely-local** arms — `label` ([`A11yLabel`]), `value`
//! ([`A11yTextValue`]), and `placeholder` ([`A11yPlaceholder`]) — are the sources
//! a single node carries directly; they landed in P1a. A node carrying an
//! `A11yLabel` resolves to its text exactly as before this function existed
//! (**no name-from-label regression**); a node *without* a label falls back to
//! `value` then `placeholder`.
//!
//! The two **tree-walk** arms are wired by `build_tree` (P1b), which now supplies
//! their inputs from the resolved ECS nesting:
//!
//! - **`labelledby`** (highest precedence) — `build_tree` resolves the names of
//!   the *other* nodes referenced by [`A11yRelations::labelled_by`] and passes
//!   them as `inputs.labelledby_name`.
//! - **`contents`** — `build_tree` concatenates the node's collapsed a11y
//!   children's names and passes them as `inputs.contents_name`.
//!
//! Both arrive as the `inputs.labelledby_name` / `inputs.contents_name` `Option`s.
//! `title` still has **no local component source**, so it is never contributed
//! yet.
//!
//! [`A11yRelations::labelled_by`]: super::A11yRelations::labelled_by

use super::{A11yLabel, A11yPlaceholder, A11yTextValue};

/// The inputs to [`compute_accessible_name`] — one borrowed `Option` per ACCNAME
/// source. The local arms (`label`/`value`/`placeholder`) come straight off the
/// node's components; the two tree-walk arms (`labelledby_name`/`contents_name`)
/// are resolved by `build_tree` from the node's ECS nesting (P1b).
#[derive(Clone, Copy, Debug, Default)]
pub struct AccNameInputs<'a> {
    /// Resolved name contributed by `aria-labelledby` (highest precedence).
    /// `build_tree` resolves it from the [`A11yRelations::labelled_by`] targets'
    /// names (P1b).
    ///
    /// [`A11yRelations::labelled_by`]: super::A11yRelations::labelled_by
    pub labelledby_name: Option<&'a str>,
    /// The explicit host label (`aria-label`), from [`A11yLabel`].
    pub label: Option<&'a A11yLabel>,
    /// A control's current text value, from [`A11yTextValue`].
    pub value: Option<&'a A11yTextValue>,
    /// The empty-input prompt, from [`A11yPlaceholder`].
    pub placeholder: Option<&'a A11yPlaceholder>,
    /// The node's own subtree text. `build_tree` concatenates the node's
    /// collapsed a11y children's names (P1b).
    pub contents_name: Option<&'a str>,
}

/// Compute the accessible name (ACCNAME 1.2) from a node's locally-available
/// sources, in the §6 precedence order: `labelledby > label > value >
/// placeholder > contents > title`.
///
/// Returns the first non-empty source in precedence order, or `String::new()`
/// when none contributes a name. An **empty** source string is treated as "not
/// contributed" and falls through to the next arm — matching ACCNAME's
/// empty-string skip and preserving the prior `A11yLabel`-only behavior (an
/// `A11yLabel("")` produced an empty name, and still does, by falling through to
/// the other empty arms back to `""`).
///
/// # Tree-walk arms (P1b)
///
/// `labelledby` (highest precedence) and `contents` (after placeholder) resolve
/// the names of *other* nodes / this node's subtree. `build_tree` walks the ECS
/// nesting and passes the resolved strings as `labelledby_name`/`contents_name`;
/// callers without a tree (the pure-function fixtures) pass `None`, and the
/// function is exact for the local arms regardless. `title` has no local
/// component source and is therefore never contributed here yet.
pub fn compute_accessible_name(inputs: AccNameInputs<'_>) -> String {
    // Each arm contributes its source string; an empty string is "not
    // contributed" and falls through (`non_empty`). The first contributor in
    // precedence order wins.
    //
    // 1. labelledby — resolved by `build_tree` from the labelled_by targets (P1b).
    // 2. label (aria-label) — the top local arm. A node carrying an `A11yLabel`
    //    resolves to its text exactly as before this function existed (no
    //    name-from-label regression).
    // 3. value (a control's current text value).
    // 4. placeholder (the empty-input prompt).
    // 5. contents — resolved by `build_tree` from the node's subtree (P1b).
    // 6. title — no local component source; never contributed yet.
    inputs
        .labelledby_name
        .and_then(non_empty)
        .or_else(|| inputs.label.map(|l| l.0.as_str()).and_then(non_empty))
        .or_else(|| inputs.value.map(|v| v.0.as_str()).and_then(non_empty))
        .or_else(|| inputs.placeholder.map(|p| p.0.as_str()).and_then(non_empty))
        .or_else(|| inputs.contents_name.and_then(non_empty))
        .unwrap_or_default()
        .to_owned()
}

/// Treat an empty string as "not contributed" (ACCNAME's empty-string skip).
fn non_empty(source: &str) -> Option<&str> {
    (!source.is_empty()).then_some(source)
}
