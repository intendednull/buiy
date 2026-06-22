**Date:** 2026-05-22
**Status:** active
**Subject:** woodpecker_ui — critiques, open problems, gaps vs Buiy targets

# Critiques & open problems

This file collects honest critiques and open problems for woodpecker_ui, audited 2026-05-22.

## Adoption & maintenance critiques

### Small adoption

**1,077 lifetime downloads** with **6 in the last 90 days** (verified crates.io 2026-05-22). For comparison: the predecessor `kayak_ui` ended at 18,774, `bevy_feathers` is at 191,700. Even the relatively narrow third-party space woodpecker_ui sits in has alternatives with 5–200× more usage.

Practical consequence: there's effectively no production user base, no community-maintained patch stream, no fork ecosystem to lean on if upstream goes silent. Any project adopting woodpecker_ui is on its own.

### Release silence

**No releases in ~12 months** (`0.1.1` was 2025-05-31; this audit is 2026-05-22). The repo's last commit push was 2025-06-07 — one week after release.

The honest base-rate framing: a solo-author Bevy UI crate with 11+ months of commit silence is in the late stages of the solo-maintainer lifecycle. The predecessor kayak_ui followed an identical curve before going effectively dormant ([`history.md`](history.md) § "Pattern: the second-system trap?"). The crate is not formally archived; the author has not communicated abandonment. But the empirical signal is clear.

### Single maintainer / bus factor 1

Per [`distribution.md`](distribution.md): StarArawn is the sole publisher of both crates and the sole significant committer to both repos. The 4 forks are unverified as active continuations; no co-maintainers have published.

### Bevy version drift

Pinned at **Bevy 0.16** in `Cargo.toml`. Bevy is at 0.18.1 stable + 0.19-rc.2 as of 2026-05-22. **Two unmigrated minor versions of breaking-change accumulation.** Any user attempting to bring woodpecker_ui forward to Bevy 0.18 takes on the migration cost themselves, including:

- `bevy_picking` API churn between 0.16 and 0.18.
- `Bundle` deprecation in favor of `Required Components` (Bevy 0.17+).
- AccessKit cadence (`bevy_a11y` pins different AccessKit majors across this window — see [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) "AccessKit version pin drift").
- `bevy_vello` and Parley have themselves released versions during this window (`bevy_vello` 0.10+; Parley 0.5+ → 0.9 by Bevy 0.19-dev).

This is the most concrete cost of the maintenance silence.

## Architectural critiques

### Megacomponent style

`WoodpeckerStyle` is a single ~50-field component covering layout + box-decoration + text + visibility. This is exactly the pattern [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Avoid-row "Megacomponents that are BSN-hostile" warns against:

> *"Megacomponents that are BSN-hostile — `bevy_a11y::AccessibilityNode` bundled all a11y properties as private fields behind inconsistent method-style setters. BSN couldn't patch them. PR #24308 broke it up after the fact."*

woodpecker_ui's `WoodpeckerStyle` would have the same problem if BSN landed and someone tried to patch the `background_color` field in a layered template. Per the README Q4, the author's stance on BSN is explicitly skeptical (*"I'm personally not a huge fan of using scenes and also the new BSN macro"*), which is internally consistent — but it means woodpecker_ui's component shape is structurally incompatible with the Bevy ecosystem's current direction.

For Buiy (foundation goal 3): the decomposed-components rule (`BackgroundColor`, `BorderColor`, `BorderRadius`, etc. as separate components) is the opposite call.

### No accessibility integration

woodpecker_ui does not integrate with **AccessKit**. There is no `accesskit` dependency in `Cargo.toml`. There is no `bevy_a11y` dependency. There is no role/label/value plumbing in any widget. Screen readers cannot navigate a woodpecker_ui interface.

For game-UI use cases this might be acceptable. For a Buiy target that commits to WCAG 2.2 AA (foundation goal 2), it is a non-starter. The gap is not "a11y is incomplete" — it is "a11y is not present."

### No focus model worth speaking of

A single `CurrentFocus` resource holds the currently focused entity. `WidgetFocus` / `WidgetBlur` events fire on transitions. That is the entire focus subsystem.

Missing:
- `:focus-visible` distinction (keyboard vs pointer focus styling).
- Focus traps for modal dialogs.
- Focus restoration when a modal closes.
- Inert subtrees (`inert` attribute analogue).
- Roving tabindex.
- `aria-activedescendant`.
- Sequential-focus-navigation-starting-point.
- Spatial / gamepad navigation.

The Buiy foundation focus model (`docs/specs/2026-05-07-buiy-foundation/architecture.md` § 2.3, `buiy-focus-model-design` sub-spec) commits to all of these.

### No theme / token system

Style is per-widget `WoodpeckerStyle` components plus per-widget-type `*Styles` structs (e.g., `ButtonStyles`, `CheckboxStyles`, `ToggleStyles`). There is no semantic-token layer, no light/dark variant binding, no forced-colors fallback, no `prefers-contrast` / `prefers-color-scheme` integration, no OS-preference resolution.

For game UI this is acceptable. For Buiy's `buiy-theme-tokens-design` commitment (foundation README § 4) it is not.

### No animation primitive

The `transition` module ships a basic interpolation helper (using the `interpolation` crate), but there is no:
- CSS-style `transition` shorthand.
- Keyframe animation.
- Layout transitions (FLIP).
- Spring physics.
- Reduced-motion gating (no `prefers-reduced-motion` query).
- Per-property timing functions.

The Buiy `buiy-animation-design` sub-spec commits to all of the above.

### No live regions / no rich text-input

- No `aria-live` analog.
- No `Alert` / `Status` / `Log` / `Timer` widgets.
- No toast / snackbar implementation.
- No carousel auto-rotation with pause/stop (WCAG 2.2.2).
- The `TextBox` widget is single-line; no multi-line, no IME (Compose events not handled), no BiDi caret, no rich-text editing.

### `bevy-trait-query` dispatch cost

Widget polymorphism is via `bevy-trait-query` 0.16, which is runtime trait-object dispatch (one virtual call per widget per frame). For a small UI this is unmeasurable; at the 1000+ node fixture sizes Buiy commits to (foundation verification.md), the per-frame cost would need to be characterized. No published benchmark.

### No same-window coexistence story with `bevy_ui`

Both stacks register `bevy_picking` backends and emit their own render passes; nothing arbitrates them. No example or test verifies side-by-side operation. See [`integration.md`](integration.md) § "Coexistence with bevy_ui."

## Coverage gaps vs Buiy widget catalog

Per [`api.md`](api.md) § "Missing vs Buiy widget catalog", woodpecker_ui ships **about 12 user-facing widgets** vs Buiy's **~60-pattern target** ([`media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)). Critical gaps:

| Pattern | woodpecker_ui status | Buiy tier |
|---|---|---|
| Link | missing | F |
| Heading (with level) | missing | F |
| Label | missing | F |
| Landmarks (banner / navigation / main / ...) | missing | F |
| Radio Group | missing | F |
| Listbox (full) | partial via Dropdown | F |
| Combobox | partial via Dropdown | F |
| Spinbutton | missing | F |
| Searchbox | missing | F |
| Menu / Menubar / Menu Button | missing | F |
| Tooltip | missing | F |
| Disclosure / Accordion | missing | F |
| Progressbar / Meter | missing | F (Progress), C (Meter) |
| Alert / Status / Log / Timer (live regions) | missing | F (Alert/Status), C (Log/Timer) |
| Toast / Snackbar | missing | F |
| Table / Grid / Treegrid | missing | C |
| Date / Time / File picker | missing | C |
| Color picker | shipped | C |
| Tabs (auto-activate + manual-activate) | partial via `TabButton` | F |

The shipped widgets do not encode APG keyboard contracts — `Toggle`, `Slider`, `Checkbox`, `Dropdown` do not declare their key bindings beyond click. No `Home`/`End`/arrow-key contract on `Slider`. No `Space`/`Enter` distinction on `WButton`.

## Open problems

These are problems woodpecker_ui has not solved and that would block adoption for a Buiy-sized commitment. The list is the structural-gap inventory:

1. **Accessibility tree integration.** AccessKit wiring, role / label / value plumbing per widget, focus tree feeding the AccessKit tree, live-region announcement plumbing.
2. **WCAG 2.2 conformance.** Touch-target size (2.5.8), focus-not-obscured (2.4.11/2.4.12), reflow (1.4.10), reduced motion (`prefers-reduced-motion`), forced-colors fallback.
3. **APG keyboard contracts.** Every interactive widget needs its APG keys (arrow keys, Home/End, Escape, Tab, Enter/Space) with documented contracts and tests.
4. **IME / BiDi / complex text.** Parley supports complex script shaping; woodpecker_ui hasn't wired IME composition events or BiDi caret handling.
5. **Theme / token system.** Light/dark variants, OS-preference binding, semantic tokens, contrast linting.
6. **Animation primitives.** CSS-transition analog, keyframes, FLIP layout transitions, springs.
7. **Bevy version migration.** Bring forward from 0.16 to 0.18.1, then track 0.19.
8. **Parley migration.** From 0.4 to 0.9 (current); breaking API changes between those versions.
9. **Performance verification.** No published benchmark at 1000+ nodes; `bevy-trait-query` dispatch cost unknown at scale.
10. **BSN compatibility.** Either restructure `WoodpeckerStyle` into decomposed components, or ship an adapter, or formally opt out of BSN-authoring support.
11. **Documentation depth.** README + examples only. No reference docs beyond rustdoc, no widget-by-widget user guide, no a11y guidance, no migration guide for Bevy upgrades.
12. **Multi-window / multi-camera test coverage.** The render-target conversion plugin exists but is not exercised in published examples.

## Sources

- crates.io download counts — https://crates.io/api/v1/crates/woodpecker_ui
- woodpecker_ui README — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/README.md
- woodpecker_ui Cargo.toml — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/Cargo.toml
- `src/styles/mod.rs` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/src/styles/mod.rs
- `src/widgets/mod.rs` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/src/widgets/mod.rs
- Buiy foundation media-and-widgets — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- bevy_ui megacomponent / BSN lessons — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- Sibling: [`api.md`](api.md), [`distribution.md`](distribution.md), [`history.md`](history.md), [`lessons.md`](lessons.md)
