//! Unit tests for `compute_accessible_name` (ACCNAME 1.2, semantic-tree.md §6).
//!
//! P1a realizes the **purely-local** precedence arms — `label > value >
//! placeholder` — that need no tree walk. The `labelledby` (highest precedence)
//! and `contents` arms resolve other nodes / this node's subtree, so they need
//! the nesting that lands in P1b; their fixtures are written here but `#[ignore]`'d
//! until P1b supplies the tree walk (P1b un-ignores them).

use buiy_core::a11y::accname::compute_accessible_name;
use buiy_core::a11y::{A11yLabel, A11yPlaceholder, A11yTextValue, AccNameInputs};

fn label(s: &str) -> A11yLabel {
    A11yLabel(s.to_string())
}
fn value(s: &str) -> A11yTextValue {
    A11yTextValue(s.to_string())
}
fn placeholder(s: &str) -> A11yPlaceholder {
    A11yPlaceholder(s.to_string())
}

#[test]
fn label_beats_value_and_placeholder() {
    // The explicit host label (aria-label) is the top *active* local arm: it
    // wins over both the control value and the placeholder.
    let l = label("Save");
    let v = value("draft text");
    let p = placeholder("Type here…");
    let name = compute_accessible_name(AccNameInputs {
        label: Some(&l),
        value: Some(&v),
        placeholder: Some(&p),
        ..Default::default()
    });
    assert_eq!(name, "Save", "label must win over value and placeholder");
}

#[test]
fn value_beats_placeholder() {
    // With no label, the control's current value wins over the placeholder.
    let v = value("hello");
    let p = placeholder("Type here…");
    let name = compute_accessible_name(AccNameInputs {
        value: Some(&v),
        placeholder: Some(&p),
        ..Default::default()
    });
    assert_eq!(name, "hello", "value must win over placeholder");
}

#[test]
fn label_only_preserves_current_behavior() {
    // A node carrying ONLY an A11yLabel resolves to its text exactly as before
    // this function existed — the no-name-from-label-regression guarantee.
    let l = label("Toggle bold");
    let name = compute_accessible_name(AccNameInputs {
        label: Some(&l),
        ..Default::default()
    });
    assert_eq!(name, "Toggle bold");
}

#[test]
fn value_only_falls_back_to_value() {
    // No label ⇒ the value is the name.
    let v = value("typed");
    let name = compute_accessible_name(AccNameInputs {
        value: Some(&v),
        ..Default::default()
    });
    assert_eq!(name, "typed");
}

#[test]
fn placeholder_only_falls_back_to_placeholder() {
    // No label, no value ⇒ the placeholder is the name.
    let p = placeholder("Search…");
    let name = compute_accessible_name(AccNameInputs {
        placeholder: Some(&p),
        ..Default::default()
    });
    assert_eq!(name, "Search…");
}

#[test]
fn no_local_source_is_empty() {
    // Nothing local contributes ⇒ empty name. (P1b's labelledby/contents arms
    // would contribute here once wired.)
    let name = compute_accessible_name(AccNameInputs::default());
    assert_eq!(name, "", "no local source ⇒ empty accessible name");
}

#[test]
fn empty_label_falls_through_to_value() {
    // An empty source string is "not contributed" (ACCNAME's empty-string skip):
    // an `A11yLabel("")` does not block the value fallback. This preserves the
    // prior behavior that an empty label produced an empty name (it still does
    // when nothing else contributes), while letting a real value win.
    let l = label("");
    let v = value("real value");
    let name = compute_accessible_name(AccNameInputs {
        label: Some(&l),
        value: Some(&v),
        ..Default::default()
    });
    assert_eq!(name, "real value", "empty label must fall through to value");
}

// ---------------------------------------------------------------------------
// DEFERRED arms — P1b un-ignores these once the nesting tree walk exists. They
// pin the precedence ABOVE the local arms (labelledby) and BELOW them
// (contents); in P1a `build_tree` always passes `None` for both inputs, so the
// fixtures would have nothing to resolve. P1b populates `labelledby_name` /
// `contents_name` from the resolved relation targets + the subtree and removes
// the `#[ignore]`.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "P1b un-ignores — needs the nesting tree walk"]
fn labelledby_beats_label() {
    // aria-labelledby (highest precedence) wins over an explicit label. In P1a
    // `build_tree` never resolves a labelledby name (no tree walk), so this is
    // only reachable once P1b populates `labelledby_name`.
    let l = label("Local label");
    let name = compute_accessible_name(AccNameInputs {
        labelledby_name: Some("Referenced name"),
        label: Some(&l),
        ..Default::default()
    });
    assert_eq!(
        name, "Referenced name",
        "labelledby must win over a local label",
    );
}

#[test]
#[ignore = "P1b un-ignores — needs the nesting tree walk"]
fn contents_used_when_no_local_label_value_or_placeholder() {
    // The node's own subtree text is the fallback below placeholder. Needs the
    // P1b child walk to populate `contents_name`.
    let name = compute_accessible_name(AccNameInputs {
        contents_name: Some("Click me"),
        ..Default::default()
    });
    assert_eq!(name, "Click me", "contents is the fallback name");
}
