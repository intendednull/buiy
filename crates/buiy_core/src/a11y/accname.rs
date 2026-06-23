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
//! 2. label        (aria-label — the explicit host label)           ── P1a (here)
//! 3. value        (a control's current text value)                 ── P1a (here)
//! 4. placeholder  (the empty-input prompt)                         ── P1a (here)
//! 5. contents     (the node's own subtree text)                    ── P1b
//! 6. title        (the tooltip / title attribute)                  ── (no local source yet)
//! ```
//!
//! # P1a scope — the **purely-local** arms only
//!
//! P1a (no ECS-tree nesting yet) realizes only the arms that need **no tree
//! walk** — the sources a single node carries directly: `label`
//! ([`A11yLabel`]), `value` ([`A11yTextValue`]), and `placeholder`
//! ([`A11yPlaceholder`]). With `labelledby` deferred, the
//! top active arm is `label`, so a node carrying an `A11yLabel` resolves to its
//! text exactly as before this function existed — **no name-from-label
//! regression**; the new behavior is only that a node *without* a label now falls
//! back to `value` then `placeholder`.
//!
//! Two arms are **stubbed/empty until P1b supplies the tree walk**:
//!
//! - **`labelledby`** (highest precedence) — resolves the names of *other* nodes
//!   referenced by `aria-labelledby`; it needs the `labelled_by`-resolution + the
//!   nesting that lands in P1b.
//! - **`contents`** — the node's own subtree text; it needs the child walk that
//!   lands in P1b.
//!
//! Both are represented as `inputs.labelledby_name` / `inputs.contents_name`
//! `Option`s that P1a always leaves `None`; P1b populates them (and un-ignores the
//! `#[ignore]`'d fixtures that pin their precedence). `title` has **no local
//! component source** in P1a, so it is likewise never contributed yet.

use super::{A11yLabel, A11yPlaceholder, A11yTextValue};

/// The locally-available inputs to [`compute_accessible_name`] — one borrowed
/// `Option` per ACCNAME source a single node can carry without a tree walk.
///
/// `labelledby_name` and `contents_name` are the **deferred** arms: P1a always
/// passes `None` for them (no tree walk exists yet); P1b fills them in from the
/// resolved `labelled_by` targets and the node's subtree. Keeping them in the
/// signature now means P1b only flips the call site — the precedence order
/// encoded here is already correct and tested (the `#[ignore]`'d fixtures un-gate
/// in P1b).
#[derive(Clone, Copy, Debug, Default)]
pub struct AccNameInputs<'a> {
    /// Resolved name contributed by `aria-labelledby` (highest precedence).
    /// **P1b** — `None` in P1a (needs the tree walk to resolve referenced nodes).
    pub labelledby_name: Option<&'a str>,
    /// The explicit host label (`aria-label`), from [`A11yLabel`].
    pub label: Option<&'a A11yLabel>,
    /// A control's current text value, from [`A11yTextValue`].
    pub value: Option<&'a A11yTextValue>,
    /// The empty-input prompt, from [`A11yPlaceholder`].
    pub placeholder: Option<&'a A11yPlaceholder>,
    /// The node's own subtree text. **P1b** — `None` in P1a (needs the child
    /// walk).
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
/// # Deferred arms (P1b)
///
/// `labelledby` (highest precedence) and `contents` (after placeholder) resolve
/// the names of *other* nodes / this node's subtree, which only become walkable
/// once P1b lands the nesting/`labelled_by` resolution. In P1a the corresponding
/// inputs are always `None`, so this function is exact for the local arms and
/// already correctly ordered for when P1b supplies them. `title` has no local
/// component source in P1a and is therefore never contributed here yet.
pub fn compute_accessible_name(inputs: AccNameInputs<'_>) -> String {
    // Each arm contributes its source string; an empty string is "not
    // contributed" and falls through (`non_empty`). The first contributor in
    // precedence order wins.
    //
    // 1. labelledby — P1b un-ignores; needs the nesting tree walk. `None` in P1a.
    // 2. label (aria-label) — the top *active* local arm in P1a. A node carrying
    //    an `A11yLabel` resolves to its text exactly as before this function
    //    existed (no name-from-label regression).
    // 3. value (a control's current text value).
    // 4. placeholder (the empty-input prompt).
    // 5. contents — P1b un-ignores; needs the nesting tree walk. `None` in P1a.
    // 6. title — no local component source in P1a; never contributed yet.
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
