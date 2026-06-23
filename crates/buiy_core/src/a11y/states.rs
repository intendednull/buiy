//! Decomposed AccessKit **state** components — one tiny, independently-changing
//! component per ARIA concept (the inversion of the bevy_a11y megacomponent
//! anti-pattern #17644). Each maps to exactly one accesskit 0.24 setter in the
//! `to_accesskit_node` derive fold (`translate.rs`, the single emission point);
//! **absence ⇒ not-applicable** (the setter is simply not called).
//!
//! Spec: docs/specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md §§1–2.
//! Phase 1a lands the decomposed state surface in two batches: the first
//! simple-setter batch (toggled / expanded / selected / disabled / modal /
//! hidden) and this second batch — the valued-range, text/placeholder, the two
//! enum-property markers (orientation / has-popup) and the live-region
//! component (with role-implied derivation in `translate::resolve_live`). The
//! relation struct + ACCNAME land in later P1a tasks.

use accesskit::{HasPopup, Live, Orientation, Toggled};
use bevy::prelude::*;

/// Tri-state toggle (`{False, True, Mixed}`) → `set_toggled`. Unifies
/// aria-checked and aria-pressed through one setter; `Mixed` is **never
/// collapsed** to a boolean.
///
/// `accesskit::Toggled` is a foreign type that derives neither `Reflect` nor
/// `Default`, so this newtype is registered **opaquely** (`#[reflect(opaque)]`)
/// and hand-writes `Default` as `Toggled::False`. Opaque registration keeps the
/// component type-registered + BSN-patchable as a whole without recursing into
/// the foreign enum's fields.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(opaque)]
#[reflect(Component, Default, Debug, PartialEq)]
pub struct A11yToggled(pub Toggled);

impl Default for A11yToggled {
    fn default() -> Self {
        Self(Toggled::False)
    }
}

/// Expanded/collapsed disclosure state → `set_expanded(bool)`; absence ⇒
/// `clear_expanded` (the fold omits the arm).
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub struct A11yExpanded(pub bool);

/// Selected state (e.g. a list option, a tab) → `set_selected(bool)`; absence ⇒
/// `clear_selected`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub struct A11ySelected(pub bool);

/// Disabled marker → `set_disabled()` (a no-argument flag in accesskit 0.24's
/// `flag_methods!`). Presence sets the flag; absence leaves it clear.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub struct A11yDisabled;

/// Read-only marker — the **decomposed a11y read-only state** the inbound action
/// router's live filter consults (action-router.md §3): a *mutating* verb
/// (`SetValue`/`Increment`/`Decrement`/`ReplaceSelectedText`/`SetTextSelection`)
/// against an entity carrying this marker is rejected as
/// [`NotActionableReason::ReadOnly`](super::NotActionableReason::ReadOnly), while
/// non-mutating verbs (`Focus`/`Blur`, selecting/copying) stay allowed
/// (Compose's read-only-`TextField`-rejects-`SetText`).
///
/// A presence marker, parallel to [`A11yDisabled`]. It is the a11y-tier
/// counterpart of the editor-local `text::edit::ReadOnly`; the router checks
/// *this* one so the gate is role-agnostic and lives in the a11y surface. No
/// pre-P1d widget advertises a mutating verb, so the filter is forward-looking
/// until P1d lands the value/text widgets that carry it. It does **not** drive an
/// outbound `set_read_only` fold arm in P1c (no widget emits it yet); the
/// inbound live filter is its first consumer.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub struct A11yReadOnly;

/// Modal marker → `set_modal()` (a no-argument flag). A dialog/overlay carries
/// it so an AT announces the rest of the page as inert.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub struct A11yModal;

/// Hidden marker — **carried only** in Phase 1a.
///
/// In the final design (semantic-tree.md §7.4) `A11yHidden` is **not** a node
/// flag: it **prunes** the entity + its subtree from `build_tree`. That prune
/// needs the ECS-tree nesting that lands in **P1b**. P1a has no nesting yet, so
/// this component is **carried for forward-compat with no fold arm and no setter**
/// — it is unobservable at the consumer tier until P1b implements the prune. The
/// `A11yNodeView.hidden` flag is populated from it now so P1b only has to add the
/// prune, not also thread the component through `build_tree`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub struct A11yHidden;

/// Valued range (a slider / spinner / progress) — the one ARIA concept whose
/// five numeric fields co-vary, so they live in a single component rather than
/// five markers. Maps to six setters in the fold: `now`/`min`/`max` →
/// `set_numeric_value`/`set_min_numeric_value`/`set_max_numeric_value`;
/// `step`/`jump` (when present) → `set_numeric_value_step`/`set_numeric_value_jump`;
/// `text` (when present) → `set_value` (a human-readable rendering of the value,
/// e.g. "50%").
///
/// All five accesskit setters take `f64`; `set_value` takes `impl Into<Box<str>>`
/// (verified against accesskit 0.24.1's `f64_property_methods!` /
/// `string_property_methods!`).
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub struct A11yValue {
    /// Current value → `set_numeric_value`.
    pub now: f64,
    /// Minimum → `set_min_numeric_value`.
    pub min: f64,
    /// Maximum → `set_max_numeric_value`.
    pub max: f64,
    /// Step increment → `set_numeric_value_step` (omitted ⇒ no step).
    pub step: Option<f64>,
    /// Large "page" jump → `set_numeric_value_jump` (omitted ⇒ no jump).
    pub jump: Option<f64>,
    /// Human-readable value text (e.g. "50%") → `set_value` (omitted ⇒ unset).
    pub text: Option<String>,
}

/// Single-line text value (a text input's current contents) → `set_value`. The
/// *role* disambiguates this from [`A11yValue`]'s numeric `text`: a `TextInput`
/// carries `A11yTextValue`, a `Slider` carries `A11yValue`. `set_value` takes
/// `impl Into<Box<str>>`.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub struct A11yTextValue(pub String);

/// Placeholder / prompt text shown in an empty input → `set_placeholder`
/// (`impl Into<Box<str>>`).
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub struct A11yPlaceholder(pub String);

/// Control orientation (a slider, toolbar, separator…) → `set_orientation`.
///
/// `accesskit::Orientation` (`{Horizontal, Vertical}`) is a foreign type that
/// derives neither `Reflect` nor `Default`, so — exactly as [`A11yToggled`] —
/// this newtype is registered **opaquely** (`#[reflect(opaque)]`) and hand-writes
/// `Default` as `Orientation::Vertical` (the accesskit enum-property default).
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(opaque)]
#[reflect(Component, Default, Debug, PartialEq)]
pub struct A11yOrientation(pub Orientation);

impl Default for A11yOrientation {
    fn default() -> Self {
        Self(Orientation::Vertical)
    }
}

/// Popup kind a control opens (a menu button, combobox…) → `set_has_popup`.
///
/// `accesskit::HasPopup` (`{Menu, Listbox, Tree, Grid, Dialog}`) is foreign and
/// derives neither `Reflect` nor `Default`, so it is registered **opaquely** and
/// hand-writes `Default` as `HasPopup::Menu` (the accesskit enum-property
/// default).
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(opaque)]
#[reflect(Component, Default, Debug, PartialEq)]
pub struct A11yHasPopup(pub HasPopup);

impl Default for A11yHasPopup {
    fn default() -> Self {
        Self(HasPopup::Menu)
    }
}

/// Live-region announcement policy → `set_live(politeness)` plus, when
/// `atomic`, the no-argument `set_live_atomic()` marker (**not** `set_atomic`,
/// which does not exist in accesskit 0.24).
///
/// An *explicit* `A11yLive` overrides the role-implied default; absence falls
/// back to [`translate::resolve_live`](super::translate), which derives the
/// policy from the node's role (`Alert`/`Status`/`Log`). `politeness` /
/// `atomic` map one-to-one with the two setters.
///
/// `accesskit::Live` (`{Off, Polite, Assertive}`) is foreign + not `Default`;
/// the component holds it directly (not a tuple newtype) but is registered
/// **opaquely** for the same reason as [`A11yToggled`], with a hand-written
/// `Default` of `Live::Off` + `atomic: false` (the inert default — no
/// announcement unless something opts in).
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(opaque)]
#[reflect(Component, Default, Debug, PartialEq)]
pub struct A11yLive {
    /// Announcement politeness → `set_live`.
    pub politeness: Live,
    /// Whether the whole region is announced atomically → `set_live_atomic()`
    /// (a no-argument marker; `true` calls it, `false` omits it).
    pub atomic: bool,
}

impl Default for A11yLive {
    fn default() -> Self {
        Self {
            politeness: Live::Off,
            atomic: false,
        }
    }
}
