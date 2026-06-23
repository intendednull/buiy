//! Decomposed AccessKit **state** components — one tiny, independently-changing
//! component per ARIA concept (the inversion of the bevy_a11y megacomponent
//! anti-pattern #17644). Each maps to exactly one accesskit 0.24 setter in the
//! `to_accesskit_node` derive fold (`translate.rs`, the single emission point);
//! **absence ⇒ not-applicable** (the setter is simply not called).
//!
//! Spec: docs/specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md §§1–2.
//! Phase 1a (this slice) lands the first batch of simple-setter components; the
//! valued-range / text / live / orientation / has-popup batch and the relation
//! struct land in later P1a tasks.

use accesskit::Toggled;
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
