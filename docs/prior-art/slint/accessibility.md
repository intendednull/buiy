**Date:** 2026-05-22
**Status:** active
**Subject:** Slint — AccessKit integration: producer-side wiring through `accesskit_winit`, pin-drift pattern, widget-level `accessible-*` properties

# Accessibility

Slint was one of the early AccessKit production adopters and is named as a verified adopter in [`../accesskit/ecosystem.md`](../accesskit/ecosystem.md). The integration is producer-side: Slint constructs an AccessKit `Tree` from the live UI tree, owns one `accesskit_winit::Adapter` per window, and pushes `TreeUpdate`s as properties change. The .slint DSL exposes per-item `accessible-role`, `accessible-label`, `accessible-description`, and friends — author-level fields that the runtime maps to AccessKit node properties.

This file walks the integration shape and the lessons it surfaces for Buiy's own AccessKit-first design ([`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.6).

## Integration history

| Date | Slint version | What landed |
|---|---|---|
| 2023-06-15 | (pre-1.1) | PR [#2865](https://github.com/slint-ui/slint/pull/2865) merged — initial AccessKit producer wiring through `accesskit_winit`. Pinned `accesskit` 0.11.0 + `accesskit_winit` 0.14.0. Author: @tronical (Simon Hausmann, co-founder), with collaboration from Matt Campbell on stable `NodeId` semantics. |
| 2023-Q3 | 1.1.x | Linux AccessKit startup panic fixed. |
| 2024-05-13 | 1.6.0 | "Annotated more widgets with accessible properties and actions." |
| 2024-07-18 | 1.7.0 | Updated to **winit 0.30 + AccessKit 0.16**. |
| 2024-12-18 | 1.9.0 | Fixed panic when `PopupWindow` is opened while AccessKit is active. |
| 2025-06-16 | 1.12.0 | "Updated AccessKit." (changelog short note) |
| 2025-09-03 | 1.13.0 | "Updated AccessKit." (changelog short note) |
| 2025-04 | issue [#8148](https://github.com/slint-ui/slint/issues/8148) | Slint cannot upgrade to `accesskit_winit` 0.26 — `ActiveEventLoop` adapter constructor change is incompatible with Slint's event-loop initialization sequence. Open as of folder-writing time. |

The pattern is clear: Slint stays a few AccessKit minor releases behind upstream, with periodic catch-up bumps. Issue #8148 documents the most recent stall — winit / AccessKit's "construct the adapter from an `ActiveEventLoop`" API breaks Slint's adapter-construction timing, which currently happens before the event loop is active.

## The DSL surface for accessibility

`.slint` exposes accessibility as item-level properties:

```slint
Rectangle {
    accessible-role: button;
    accessible-label: "Send";
    accessible-description: "Send the composed message";
    accessible-enabled: !sending;
    accessible-action-default => { send(); }
}
```

Selected fields (the set has grown across releases; this is the 1.16 surface):

- `accessible-role: <enum>` — one of ~30 enum values (`button`, `text`, `text-input`, `slider`, `combobox`, `checkbox`, `tab`, `tab-list`, `tab-panel`, `list`, `list-item`, `progress-indicator`, `dialog`, `group-box`, …). The enum maps to AccessKit `Role`.
- `accessible-label`, `accessible-description`, `accessible-placeholder-text`, `accessible-value`, `accessible-value-minimum`, `accessible-value-maximum`, `accessible-value-step`.
- State: `accessible-checked`, `accessible-expanded`, `accessible-selected`, `accessible-enabled`, `accessible-read-only`.
- Actions: `accessible-action-default => { … }`, `accessible-action-increment`, `accessible-action-decrement`, `accessible-action-set-value`, `accessible-action-expand`, `accessible-action-collapse`.

The author writes declarative accessibility properties; the runtime stitches them into AccessKit's `Node` API. This is morally identical to the Buiy decomposed-components shape (`A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, …) but expressed in DSL property form rather than ECS components.

## Standard widgets pre-wire accessibility

The Slint standard widgets (`Button`, `CheckBox`, `LineEdit`, `Slider`, `TabWidget`, `ListView`, …) already set their `accessible-role` and route their public properties (`text`, `checked`, `value`) to the corresponding `accessible-*` fields. App authors get accessibility "for free" for the standard widget set; custom widgets carry the burden explicitly.

This is the same posture as Buiy's "every Buiy widget ships with its APG keyboard contract, accessible name/role/value, focus management, AccessKit tree wiring" ([`docs/specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) goal 2). Slint validates the choice — pre-wired widgets work; the burden lands when authors build custom controls.

## Known gaps

- **Live regions not in the `.slint` surface.** AccessKit's `Live { Off, Polite, Assertive }` + `is_live_atomic` + `is_busy` are not exposed as item properties as of 1.16. Apps that want live-region announcements have to fall back to runtime API or accept the gap. Buiy commits to a global announcer service ([`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)); Slint does not.
- **ACCNAME 1.2 is not fully implemented producer-side.** AccessKit deliberately doesn't compute accessible names (see [`../accesskit/lessons.md`](../accesskit/lessons.md) — that's the consumer/producer split). Slint's `accessible-label` is a direct setter; the full ACCNAME 1.2 algorithm (which walks `aria-labelledby`, then `aria-label`, then content text, then `title`, etc.) is not visibly implemented. Apps wanting WCAG-2.2-AA-compliant naming have to construct the labels themselves.
- **Rich-text / hypertext through AccessKit is flattened.** Same constraint as everyone else (see [`../accesskit/critiques.md`](../accesskit/critiques.md) §9): AccessKit doesn't model rich text. Slint's `TextEdit` widget exposes plain text to AccessKit; styled runs are visual-only.
- **List virtualization through AccessKit is partial.** AccessKit's recent list-view improvements (referenced in issue #8148) require the upgraded `accesskit_winit` 0.26+ that Slint cannot yet adopt. Large lists report as un-virtualized to the AT, which can cause AT navigation lag on long lists.
- **WCAG 2.2 conformance is not claimed.** Slint's accessibility documentation is technical (which fields exist, which roles they map to) — not WCAG-conformance-claimed. Apps building safety-critical UIs (OTIV, KDAB clients) presumably handle WCAG audit themselves.

## Implications for Buiy

- **Slint is real-world evidence that AccessKit producer integration ships.** Three years of production AccessKit integration in safety-critical industrial UIs (OTIV rail automation). The pattern works; the bugs are scoped (popup panics, list-view virtualization gap, version-pin lag). Buiy's AccessKit-first commitment ([`docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.6) is validated.
- **Property-level a11y attributes work — and translate cleanly to decomposed-components.** Slint's `accessible-*` item properties are the DSL form of Buiy's `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` ECS components. Same separation of concerns, different authoring surface. Both designs avoid the `bevy_a11y::AccessibilityNode` megacomponent pitfall (see [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) row 1).
- **Version-pin lag is the real ongoing cost.** Slint pins AccessKit and accumulates lag — currently 4 minor releases behind `accesskit_winit` 0.33.0 (Slint stuck around 0.16-era; upstream at 0.33.0). Buiy's "AccessKit major release between Bevy minors triggers a Buiy patch release" policy ([`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.9, foundation README § 5 open question) needs to be lived in practice. Slint's experience suggests upgrade-blocked windows of 6–12 months happen; Buiy should plan for that.
- **Standard widgets must pre-wire a11y.** Slint's stdlib widgets pre-wire `accessible-role` and route public properties to a11y fields; custom widgets carry the burden explicitly. Buiy's widget-catalog sub-spec ([`docs/specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) commits to this — every APG-shaped Buiy widget ships with its AccessKit wiring. Slint validates that this is the right scope; the cost of NOT pre-wiring is "third-party widgets are inaccessible by default," which is how the broader Rust UI ecosystem looks today.
- **Slint does NOT compute ACCNAME 1.2.** Buiy's "ACCNAME 1.2 in `buiy_core`" decision ([`../accesskit/lessons.md`](../accesskit/lessons.md) Validates §5) is genuinely a Buiy-specific commitment, not industry-standard. Don't expect AccessKit consumers (Slint, egui, Freya, Xilem) to provide a reference implementation; Buiy is on its own here.

## Sources

- Slint AccessKit PR #2865: https://github.com/slint-ui/slint/pull/2865
- Slint issue #8148 (AccessKit version-pin drift): https://github.com/slint-ui/slint/issues/8148
- Slint blog "Slint 1.7 Released" (AccessKit 0.16 upgrade): https://slint.dev/blog/slint-1.7-released
- Slint CHANGELOG: https://github.com/slint-ui/slint/blob/master/CHANGELOG.md
- Slint accessible properties (`.slint` reference): https://docs.slint.dev/latest/docs/slint/
- AccessKit project: https://accesskit.dev
- Sibling prior-art: [`../accesskit/README.md`](../accesskit/README.md), [`../accesskit/lessons.md`](../accesskit/lessons.md), [`../accesskit/ecosystem.md`](../accesskit/ecosystem.md), [`../accesskit/critiques.md`](../accesskit/critiques.md)
- Buiy foundation accessibility: [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation architecture §2.6 / §2.9: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling files: [`architecture.md`](architecture.md), [`dsl-language.md`](dsl-language.md), [`open-problems.md`](open-problems.md)
