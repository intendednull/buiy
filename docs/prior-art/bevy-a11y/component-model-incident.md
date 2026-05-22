**Date:** 2026-05-22
**Status:** active
**Subject:** The bevy_a11y BSN-unfriendliness incident — issue #17644, PR #24308, and why Buiy still replaces bevy_a11y after the partial fix

This file is the canonical case study for Buiy's "no megacomponents" rule ([`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.4 "BSN-friendly components"). The incident has three acts: the original megacomponent design (2023-03), the BSN community recognising it as hostile (2025-02), and the partial fix (2026-05). Buiy still replaces bevy_a11y for its windows after the fix because the upstream decomposition trajectory is fundamentally different from Buiy's needed shape.

## Act 1: the original megacomponent (Bevy 0.10, March 2023)

PR [#6874](https://github.com/bevyengine/bevy/pull/6874) by Nolan Darilek (`ndarilek`) integrated AccessKit into Bevy on 2023-03-01. It introduced the `bevy_a11y` crate with this component shape:

```rust
#[derive(Component, Clone, Deref, DerefMut)]
pub struct AccessibilityNode(pub Node);
```

where `Node` is `accesskit::Node`. The PR's framing was "expose AccessKit's API to Bevy developers" — and the most direct way to do that is a thin newtype wrapper. The newtype made sense given AccessKit's own API (a builder over a single `Node` struct with hundreds of setter methods). At the time, BSN didn't exist; Bevy's component-model conventions were still forming; "wrap the underlying library's primary type in a Component" was a reasonable default pattern.

The pattern stuck. Three years later, the wrapper is still the wrapper.

## Act 2: BSN exposes the megacomponent (issue #17644, 2025-02-02)

On 2025-02-02, viridia opened issue [#17644](https://github.com/bevyengine/bevy/issues/17644) titled **"Design of bevy_a11y is BSN-unfriendly"**. The labels: `A-Accessibility`, `A-UI`, `C-Bug`, `S-Needs-Design`. The core arguments:

1. **`accesskit::Node` is a non-decomposable opaque builder.** The struct exposes ~200 fields only through `set_<field>()` and `clear_<field>()` methods. No public field access. To change a property, you call a method.

2. **No way to set individual properties via reflection / BSN syntax.** BSN's design (per cart's [discussion #14437](https://github.com/bevyengine/bevy/discussions/14437)) involves "components are made up of ordinary properties which can be merged and patched." When the component's only "property" is a `pub Node` field that's mutated via method calls, BSN can't author or merge per-property.

3. **Inconsistent setter conventions.** Some setters take owned `Box<str>`, some take primitives, some are `set_x(true)` / `clear_x()` pairs rather than `set_x(Option<bool>)`. From inside BSN, even patching one property requires picking the right method out of an inconsistent family.

4. **Multi-template merge becomes impossible.** This is the load-bearing point. From viridia's issue body:

   > *"I can well imagine wanting to merge together multiple BSN templates, each of which has opinions about various accessibility attributes: the template that determines the label or the role might not be the same template as the one which determines the checked or disable states."*

   > *"This is easy to do if these attributes are separate components, or (at least) allow overwriting of properties using patch."*

5. **Marker-component idiom is blocked.** Disabling a widget should be expressible as inserting a `Disabled` marker component. With `AccessibilityNode`, disabling is `accessible.set_disabled()`; un-disabling is `accessible.clear_disabled()`. There's no spawnable marker; there's only a method-call dance.

The combination is the BSN-hostility complaint. Components with private fields, inconsistent setters, and bundled-everything-into-one are exactly the shape BSN cannot author or patch by composition.

cart's BSN philosophy ([discussion #14437](https://github.com/bevyengine/bevy/discussions/14437)) explicitly calls for "ordinary properties which can be merged and patched" — `AccessibilityNode` is the prototypical violation in the Bevy codebase, which is why it's the named target. See [`/home/user/buiy/docs/prior-art/bevy-ui/component-model.md`](../bevy-ui/component-model.md) for the parallel decomposition story on bevy_ui's own components (`BackgroundColor`, `BorderColor`, `Outline`, `BoxShadow` all separate; `BorderRadius` reverted from separate-component to field-on-`Node` in 0.18, the non-monotonic decomposition pitfall).

## Act 3: the partial fix (PR #24308, merged 2026-05-21, milestone 0.19)

PR [#24308](https://github.com/bevyengine/bevy/pull/24308) by viridia, titled **"Introduce `AccessibleLabel` component"**, was merged on 2026-05-21 into the 0.19 milestone. It closed issue #17644.

The fix shape:

```rust
#[derive(Component, Debug, Default, Clone, Reflect)]
#[reflect(Component, Default, Debug, Clone)]
#[require(AccessibilityNode)]
#[component(immutable, on_insert = on_label_inserted, on_remove = on_label_removed)]
pub struct AccessibleLabel(pub String);
```

with component hooks that mirror the label into `AccessibilityNode`:

```rust
fn on_label_inserted(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    if let Some(label) = world.get::<AccessibleLabel>(entity) {
        let label_text = label.0.clone().into_boxed_str();
        if let Some(mut accessible) = world.get_mut::<AccessibilityNode>(entity) {
            accessible.set_label(label_text);
        }
    }
}

fn on_label_removed(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    if let Some(mut accessible) = world.get_mut::<AccessibilityNode>(entity) {
        accessible.clear_label();
    }
}
```

The PR author's framing in the PR description:

> *"(It may not be a 100% fix, but it's good enough to close the ticket I think.)"*

Testing was VoiceOver on macOS, manual. The PR touches four files: a release-notes markdown, `bevy_ui/src/accessibility.rs` (where the new component lives — **not** `bevy_a11y` itself), `bevy_ui/src/lib.rs`, and the `feathers_gallery` example. **`bevy_a11y/src/lib.rs` is unchanged. `AccessibilityNode(pub Node)` is unchanged.**

What the fix gives:
- `AccessibleLabel` is decomposed, BSN-authorable, immutable, and `#[require(AccessibilityNode)]` auto-inserts the underlying megacomponent when a label is present.
- BSN templates can each emit an `AccessibleLabel` and merging works.
- The label-specific case in viridia's original complaint is addressed.

What the fix does not give:
- Role, value, description, bounds, transform, all state flags (checked, disabled, expanded, selected, busy, hidden, invalid, …), all relations (labelled_by, described_by, controls, owns, flow_to, …), live-region politeness, sort direction, autocomplete, popup-target — every other field still lives inside `AccessibilityNode` as method-call-mutated state.
- A BSN template that wants to "make this widget disabled" still cannot do so by composition; it still has to either reach into `AccessibilityNode` via a system or rely on per-widget systems in `bevy_ui/src/accessibility.rs` to set the field.
- The non-monotonic-decomposition risk (see [`/home/user/buiy/docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) Avoid row) — if more `Accessible<Field>` components get split out over time, each is a breaking change for downstream consumers and another opportunity for reversal.

The trajectory is "decompose lazily, one field per release as someone hits the pain." Three years from now, bevy_a11y may have `AccessibleLabel`, `AccessibleRole`, `AccessibleDescription`, `AccessibleStates`, … — or it may have stopped at `AccessibleLabel` because the per-widget systems in `bevy_ui` handle the rest "well enough." Either trajectory is incompatible with Buiy's day-one decomposition.

## Why Buiy still replaces bevy_a11y after #24308

Three structural reasons. None of them are about #17644 being incompletely fixed (it is, but that's not the dispositive reason).

### 1. The decomposition shape diverges

Even if upstream eventually decomposes fully, the resulting components will be shaped for bevy_ui's needs. `AccessibleLabel`'s `#[require(AccessibilityNode)]` is the trigger: it locks the new component into the megacomponent's lifecycle. Buiy's components don't require `AccessibilityNode`; Buiy's `A11yLabel` requires `A11yRole`, which requires nothing AccessKit-side until the tree-build system reads them all and pushes a `TreeUpdate` directly to `accesskit_winit`. The required-components graph is fundamentally different.

Buiy's foundation spec ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`architecture.md` § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)) commits to:

- `A11yRole` — `accesskit::Role` selector, foundation-tier.
- `A11yLabel` — accessible name input (per ACCNAME 1.2 source priority).
- `A11yDescription` — accessible description input.
- `A11yStates` — tri-state flags for checked / expanded / selected / busy / invalid / disabled / hidden, encoded with the `Toggled` / `Option<bool>` / `Invalid` enums per AccessKit's tri-state model (see [`/home/user/buiy/docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md) Avoid rows).
- `A11yRelations` — `labelled_by`, `described_by`, `controls`, `owns`, `flow_to`, `active_descendant`, `error_message`, `details`, `popup_for`, etc.
- Live-region politeness as a separate component on the announcer entity, not on every node.

None of these are bevy_a11y components. Bevy_a11y's post-#24308 set is `AccessibilityNode` + `AccessibleLabel`. The two component vocabularies share zero names and zero types.

### 2. The integration target is AccessKit, not bevy_a11y

The AccessKit producer-protocol shape is the actual integration target. AccessKit doesn't care which crate produces the `TreeUpdate`; it just needs a `TreeUpdate` per window and an `ActionHandler` for incoming requests. Buiy is the producer; AccessKit is the bridge; bevy_a11y is an alternative producer for non-Buiy windows. Inserting bevy_a11y into the chain on a Buiy window would mean:

- Buiy mutates Buiy components per frame.
- A bridge system reflects Buiy components into bevy_a11y's `AccessibilityNode` (or, per the partial-decomposition trajectory, into `AccessibleLabel` + future siblings + remaining `AccessibilityNode` fields).
- bevy_winit's `update_accessibility_nodes` reads `AccessibilityNode`s and builds the `TreeUpdate`.

Three indirection hops, two component vocabularies, and a per-frame translation tax. None of this earns anything over Buiy talking to `accesskit_winit` directly. See [`/home/user/buiy/docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md) — "AccessKit-first, talk-to-accesskit_winit-directly."

### 3. The adapter slot is structurally single-occupant

`accesskit_winit::Adapter` accepts exactly one tree per window. Two producers cannot push to the same adapter — there's no merge protocol. So even if Buiy *wanted* to layer over bevy_a11y on a shared window, AccessKit's structural shape forbids it. Per-window coexistence (one stack per window, no shared windows) is the only design AccessKit's shape allows. See [`coexistence.md`](coexistence.md) for the long form and [`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) for the committed Buiy rule.

## The Buiy "no megacomponents" rule, restated

From [`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.4: every Buiy component is **small, public-fielded, observable, decomposed**. The #17644 / #24308 incident is the case study cited every time this rule comes up. Concretely it means:

- **Small.** A component owns one concept (a label, a role, a set of related state flags). Not "all of accessibility."
- **Public-fielded.** Fields are `pub`, not method-gated. BSN, reflection, and direct ECS access all see the same thing.
- **Observable.** Per-component change-detection is meaningful — touching one property doesn't dirty unrelated properties.
- **Decomposed.** Independent properties live in independent components. Required-components and on-insert/on-remove hooks compose them when needed.

The constraint applies regardless of BSN's status (see [`/home/user/buiy/docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) — top of file, on BSN-not-yet-landed). #17644 demonstrates the cost of retrofitting; #24308 demonstrates the friction of retrofitting partially under release-cycle pressure. Buiy pays the small-decomposed-component tax in advance.

## Cross-references

- [`architecture.md`](architecture.md) — what bevy_a11y looks like today (megacomponent still present)
- [`api.md`](api.md) — the API surface BSN can and can't reach
- [`coexistence.md`](coexistence.md) — why "layer over" is structurally impossible
- [`focus-model.md`](focus-model.md) — focus mostly lives in `bevy_input_focus`, not `bevy_a11y`
- Sibling [`critiques.md`](critiques.md) — Agent B's longer pushback synthesis
- [`/home/user/buiy/docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md) — the AccessKit-side lessons that say "decomposed is the only shape that maps cleanly onto AccessKit's setter-rich Node API"
- [`/home/user/buiy/docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) — Avoid row "Megacomponents that are BSN-hostile"
- [`/home/user/buiy/docs/prior-art/bevy-ui/component-model.md`](../bevy-ui/component-model.md) — bevy_ui's own decomposition history (BackgroundColor, BorderColor, BorderRadius reversal)

## Sources

- Issue #17644 "Design of bevy_a11y is BSN-unfriendly" by viridia, 2025-02-02: https://github.com/bevyengine/bevy/issues/17644
- PR #24308 "Introduce AccessibleLabel component" by viridia, merged 2026-05-21, milestone 0.19: https://github.com/bevyengine/bevy/pull/24308
- PR #24308 files-changed view: https://github.com/bevyengine/bevy/pull/24308/files
- PR #6874 "Integrate AccessKit" by ndarilek, merged 2023-03-01 for Bevy 0.10: https://github.com/bevyengine/bevy/pull/6874
- Discussion #14437 (cart's BSN philosophy): https://github.com/bevyengine/bevy/discussions/14437
- `bevy_ui/src/accessibility.rs` (main HEAD, post-#24308): https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/accessibility.rs
- `bevy_a11y/src/lib.rs` (main HEAD, unchanged by #24308): https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/src/lib.rs
- Buiy foundation — architecture §2.4, §2.6: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation — accessibility: [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
