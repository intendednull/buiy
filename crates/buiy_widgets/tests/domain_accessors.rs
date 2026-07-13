//! Track C / C2 — the widget **domain accessors** (F1): read live widget state as
//! plain `bool`/`f64`/`&str`, never through the foreign `accesskit::Toggled` enum.
//! These assert the marker-namespaced accessors over their state components.

use buiy_core::a11y::{A11yExpanded, A11yTextValue, A11yToggled, A11yValue, Toggled};
use buiy_core::render::components::CssVisibility;
use buiy_widgets::tooltip::TooltipNode;
use buiy_widgets::{Checkbox, Dialog, Disclosure, Menu, Popover, Slider, Switch, TextInput};

#[test]
fn checkbox_checked_and_indeterminate_map_the_tri_state() {
    // Checked ⇔ Toggled::True; a mixed (indeterminate) checkbox is NOT checked.
    assert!(Checkbox::checked(&A11yToggled(Toggled::True)));
    assert!(!Checkbox::checked(&A11yToggled(Toggled::False)));
    assert!(!Checkbox::checked(&A11yToggled(Toggled::Mixed)));

    // Indeterminate ⇔ Toggled::Mixed, distinct from `!checked()`.
    assert!(Checkbox::indeterminate(&A11yToggled(Toggled::Mixed)));
    assert!(!Checkbox::indeterminate(&A11yToggled(Toggled::True)));
    assert!(!Checkbox::indeterminate(&A11yToggled(Toggled::False)));
}

#[test]
fn switch_on_reads_the_binary_toggle() {
    assert!(Switch::on(&A11yToggled(Toggled::True)));
    assert!(!Switch::on(&A11yToggled(Toggled::False)));
    // An out-of-contract `Mixed` on a switch reads as not-on.
    assert!(!Switch::on(&A11yToggled(Toggled::Mixed)));
}

#[test]
fn slider_value_and_fraction() {
    let mid = A11yValue {
        now: 25.0,
        min: 0.0,
        max: 100.0,
        ..Default::default()
    };
    assert_eq!(Slider::value(&mid), 25.0);
    assert_eq!(Slider::fraction(&mid), 0.25);

    // Clamps out-of-range and guards a degenerate (max <= min) range.
    let over = A11yValue {
        now: 150.0,
        min: 0.0,
        max: 100.0,
        ..Default::default()
    };
    assert_eq!(Slider::fraction(&over), 1.0);
    let degenerate = A11yValue {
        now: 5.0,
        min: 10.0,
        max: 10.0,
        ..Default::default()
    };
    assert_eq!(Slider::fraction(&degenerate), 0.0);
}

#[test]
fn disclosure_expanded_reads_the_flag() {
    assert!(Disclosure::expanded(&A11yExpanded(true)));
    assert!(!Disclosure::expanded(&A11yExpanded(false)));
}

#[test]
fn text_input_value_reads_the_projected_string() {
    assert_eq!(
        TextInput::value(&A11yTextValue("hello".to_string())),
        "hello"
    );
    assert_eq!(TextInput::value(&A11yTextValue(String::new())), "");
}

#[test]
fn overlay_is_open_reads_the_visibility_channel() {
    // Every overlay's open-state rides the CssVisibility show/hide channel:
    // Visible (or absent — the default) = open; Hidden/Collapse = closed. Each
    // marker exposes the SAME reader, delegating to the shared `popover::is_open`
    // (Track F — namespace the holdout free fn onto the widget markers).
    assert!(Popover::is_open(None)); // absent ⇒ open (the default)
    assert!(Popover::is_open(Some(&CssVisibility::Visible)));
    assert!(!Popover::is_open(Some(&CssVisibility::Hidden)));
    assert!(!Popover::is_open(Some(&CssVisibility::Collapse)));

    assert!(Menu::is_open(None));
    assert!(Menu::is_open(Some(&CssVisibility::Visible)));
    assert!(!Menu::is_open(Some(&CssVisibility::Hidden)));

    // A dialog starts closed (CssVisibility::Hidden); the invoker opens it.
    assert!(Dialog::is_open(Some(&CssVisibility::Visible)));
    assert!(!Dialog::is_open(Some(&CssVisibility::Hidden)));

    // A tooltip starts hidden; the open-state reader lives on the TooltipNode.
    assert!(TooltipNode::is_open(Some(&CssVisibility::Visible)));
    assert!(!TooltipNode::is_open(Some(&CssVisibility::Hidden)));
}
