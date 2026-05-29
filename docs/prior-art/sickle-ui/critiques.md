**Date:** 2026-05-22
**Status:** archived
**Subject:** sickle_ui — critiques + open problems: what doomed the project, what it never solved

# Critiques & Open Problems

This file enumerates sickle_ui's structural problems and the open problems it left unsolved. The critiques are evidence; the conclusions feed [`lessons.md`](lessons.md).

## Critiques

### 1. The 19-month silence — the project is abandoned

The single most important critique is **the maintainer declared the project obsolete and stopped publishing**. The surviving-fork README states verbatim: `"sickle_ui has been made obsolete by changes introduced in Bevy 0.15.0 and will not be publicly maintained. This is the last release, compatible with Bevy 0.14.2."` The original GitHub repository has been deleted (404). The crate is no longer listed in `bevyengine/bevy-assets/Assets/UI/`. From 2024-10-03 to this corpus' writing on 2026-05-22, there has been no Bevy 0.15+ migration, no patch release, no roadmap, no community-elected successor. **Adopting sickle_ui in 2026 means adopting an unmaintained Bevy-0.14 library.**

Every other critique below is conditional on this primary fact: even valid criticisms of design choices are moot if the project is not receiving fixes.

### 2. Bevy version coupling — the structural failure mode

sickle is pinned to Bevy 0.14. The maintainer named Bevy 0.15's `RequiredComponents` changes (PR #14791) as the reason for declaring obsolescence — the spawn-bundle pattern sickle relied on became idiomatically wrong, and the migration was too invasive for a solo maintainer to absorb.

This is **the canonical Bevy-third-party-UI failure mode.** Bevy's component model has churned across every minor release (`BorderRadius` separated then re-merged into `Node`, `BackgroundColor` / `BorderColor` / `Outline` decomposed across 0.15-0.18, `bevy_text` migrated cosmic-text → ab_glyph paths and back, then to parley in 0.19-dev). Any library that depends on a specific component shape is on a migration treadmill, and any library where one minor's migration exceeds the maintainer's free-time-bandwidth gets left behind.

Mitigation patterns:
- **Engine-in-tree:** `bevy_feathers` ships with Bevy and migrates in lockstep. Cost: feature scope and direction is governed by the Bevy team.
- **Parallel stack:** Buiy owns its own component model + render pipeline; engine churn does not break Buiy's component vocabulary. Cost: more substrate to maintain.
- **Sufficient maintenance bandwidth:** `bevy_egui` (and a handful of others) just absorbs the migration each cycle. Cost: a real team of contributors.

sickle had none of these. The structural lesson is on Buiy [`lessons.md` § Avoid](lessons.md).

### 3. The extension-trait DSL is BSN-hostile

`UiBuilder<E>` + 200+ `Ui*Ext` / `Set<Property>Ext` traits is excellent ergonomics for Rust-authored UI and structurally incompatible with BSN-as-data-format. The widget vocabulary lives in method dispatch, not in components. The style vocabulary lives in trait-chained method calls, not in data.

If BSN (PR #20158) lands as the canonical Bevy authoring format, sickle's DSL would need to be replaced entirely to participate. There is no incremental migration path — every sickle widget is `ui.widget(arguments)`, where `arguments` is a constructor-side `Config` struct + closure children. Translating that into BSN requires the widget to be **expressible as a component (or set of components) on a spawned entity**, which is a different shape than the trait-method one.

The implication for Buiy: avoid trait-dispatched authoring. Widgets must be components. Spawning helpers are sugar, not the primary surface. See [`api.md` § "BSN-compat assessment"](api.md) and [`lessons.md` § Avoid](lessons.md).

### 4. The 200+ extension-trait prelude — discoverability cost

The `sickle_ui::prelude` re-exports **200+ traits** (per the docs.rs prelude index). The user must import the prelude to write `ui.button(...).style().background_color(...)`. IDE autocomplete on `ui.` returns the full union of in-scope widget traits, mixed with the full union of in-scope style traits, with no semantic grouping or filtering.

This is a real ergonomic cost for new users of the library: the first encounter with `ui.` produces an overwhelming dropdown. The 200+ figure is not exaggeration — `StyleCommands` macro generates a separate trait per CSS property, and the library uses ~150 of them.

The mitigation a library starting today would use: **typed builder structs** (one builder type per widget, with builder methods returning the typed struct), or **component-first authoring** (the BSN-friendly path), or **macros** that expand to BSN-style declarations (an unwritten Bevy idiom). sickle used none of these because the extension-trait DSL was the trendy Rust pattern when it was designed.

### 5. Accessibility absence — total

**Zero AccessKit integration.** No `AccessibilityNode` on any widget. No role mapping (a checkbox does not announce as "checkbox" to a screen reader). No accessible name source. No state propagation (checked/expanded/selected do not reach the a11y tree). No focus management — `bevy_input_focus`'s tab-navigation plugin is not registered by sickle, and there is no `:focus-visible` analog beyond `FluxInteraction::PointerEnter`.

For a widget library claiming editor-and-utilities scope, this is the largest single capability gap. Editors are productivity tools; productivity tools have a non-trivial accessibility user base. The maintainer's likely justification — Bevy 0.14's `bevy_a11y` was itself megacomponent-shaped and BSN-hostile (see [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) "Megacomponent that is BSN-hostile") — is real but does not resolve the absence; it explains why the absence persisted but not why no a11y at all would be acceptable.

Buiy's foundation goal 2 (`WCAG 2.2 AA is the floor`) is in part a direct response to libraries like sickle treating a11y as optional.

### 6. APG / WCAG keyboard contracts — universally absent

Cross-cutting with the a11y gap, **no widget in sickle ships an APG keyboard contract.** Specifically:

- Checkbox: no Space-to-toggle.
- Slider: no Arrow-key increment, no Home/End to min/max, no Page-Up/Down.
- Radio group: no Arrow-key cycling.
- Dropdown / combobox: no Alt-Down to expand, no type-ahead, no Esc to close.
- Tabs: no Arrow-key navigation between tabs, no Home/End.
- Menu bar: no Tab-into-menubar, no Arrow-key navigation, no type-ahead, no Esc-to-close.
- Disclosure / foldable: no Enter/Space to toggle.
- Floating panel: no keyboard window-management.

This is the "every interactive widget has a defined keyboard contract" claim Buiy explicitly makes (foundation [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)). sickle treats keyboard interaction as out-of-scope; Buiy treats it as a load-bearing verification claim.

### 7. The bevy_feathers overlap — does sickle still differentiate?

`bevy_feathers` (Bevy 0.17+, in-tree) occupies the same "styled widgets for editors and utilities" niche. sickle's remaining differentiators:

- **More widgets than feathers (especially editor-docking primitives** like `docking_zone`, `floating_panel`, `sized_zone`, `tab_container`-with-close-and-popout context menus). feathers does not currently ship these.
- **The DynamicStyle engine with animated attributes** — feathers' theme system is observer-based color-rewriting; it does not have a built-in interpolation engine. sickle's animation primitives are richer.
- **The `UiContext` named-sub-entity pattern.** feathers does not currently formalize this; styling sub-parts of a composite widget requires direct entity-walking.

In every other dimension feathers wins: maintained, in-tree, Bevy-version-tracking, has-some-a11y, integrates `bevy_input_focus`. The differentiation is real but not enough to justify adopting sickle for new development.

### 8. The "we are bevy_ui" coupling

Every sickle widget is a `bevy_ui::Node`. Every visual property reduces to `bevy_ui` components (`BackgroundColor`, `BorderColor`, `Outline`, `BorderRadius`, ...). This means sickle inherits **every `bevy_ui` renderer limitation**:

- No non-rectangular clipping (`clip-path` shapes). See [`../bevy-ui/critiques.md` § "Non-rectangular clipping"](../bevy-ui/critiques.md).
- No `backdrop-filter`. See [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md) § "Renderer caps."
- No `mix-blend-mode` / isolation.
- No true top-layer compositing (popovers must use z-index hacks).
- Rounded-corner clipping bug (Bevy issue #13093).

For an editor-and-utilities use case these limits are mostly tolerable. For Buiy's broader scope (web-platform-parity, including modern visual effects like blurred backdrop on popovers), they are blocking. This is one of the structural reasons Buiy's foundation chose parallel-stack.

### 9. Documentation completeness — 2.9%

docs.rs reports `"2.9% of the crate is documented"` on every sickle 0.4.0 module page. The README and the `simple_editor` example are the de-facto documentation. There is no published mdBook, no API reference walkthrough, no migration guide.

For a library with a 200+ trait prelude, doc-coverage discipline is the only way users discover what's available without spelunking through `src/`. 2.9% is below the threshold of useful self-documentation.

## Open problems (would-need-to-be-solved if anyone forked sickle)

### Bevy 0.15-0.18 migration not done

The largest single open problem. A working port to Bevy 0.17 (paired with `bevy_ui_widgets` for the headless primitives) is conceivable but no one has done it publicly. The work required:

- Migrate every widget from "spawn this bundle of components" to `#[require(Companion)]` on the marker.
- Reconcile `UiStyleExt::background_color` etc. against the evolved `bevy_ui` component shape (`BorderRadius` was moved back into `Node` in 0.18 — this alone breaks ~20 style-setter traits).
- Resolve the `bevy_input_focus` integration story — sickle's `FluxInteraction` overlaps but does not coordinate with the Tab-navigation focus tree.
- Decide whether to keep sickle's widget catalog distinct or refactor on top of `bevy_ui_widgets`'s headless primitives (the cleaner choice).

Estimated effort: weeks of full-time work, more for a single part-time contributor. No public PR has attempted it.

### BSN-compatibility post-PR #20158

If BSN lands as the canonical authoring format, sickle's extension-trait DSL becomes legacy authoring. A BSN-compatible widget kit needs widgets-as-components-and-companions, not widgets-as-trait-method-calls. The retrofit is large enough that "rewrite as a BSN-friendly kit" is a more honest framing than "port to BSN."

### Accessibility tree integration

No published roadmap exists for integrating with AccessKit (via `bevy_a11y` or directly). The work is well-understood (the WAI-ARIA APG documents every widget's role/state/relationship needs), but no one has done it.

### Performance at scale

No published benchmarks for sickle at 1000+ widget nodes. The `DynamicStyle` engine reconciliation cost (re-evaluating all attributes on `FluxInteraction` / `PseudoStates` change) is unbounded by the library's own implementation; in practice the maintainer wrote it to be small-N performant and never published large-N numbers. An editor workspace with hundreds of nested panels is the realistic stress case; no data exists on whether sickle 0.4.0 sustains 60 Hz there.

### Theme system maturity

sickle ships one theme (loosely Material-Design-3-inspired, dark and light variants). No public theme gallery, no community-contributed themes, no theme-customization tutorials beyond the README's outline. `ThemeData` is well-shaped but underexploited.

### Animation primitives

The `DynamicStyle` engine's animated attributes (interpolate between two endpoints with an easing curve) are the entire animation surface. There is no transitions API (CSS `transition` analog), no keyframes (CSS `@keyframes` analog), no spring physics, no layout-transition primitives, no scroll-driven-animations analog. For an editor-and-utilities library this is acceptable; for production game UI it is sparse.

### Documentation completeness

The 2.9% docs.rs coverage is a project-quality open problem. Any fork that wanted to revive sickle would need to substantially document the public API surface (especially the 200+ extension traits) to be usable by new developers.

## Implications for Buiy

These critiques feed directly into Buiy's `Avoid` list in [`lessons.md`](lessons.md). The high-level summary:

1. **Solo-maintainer + Bevy-version-coupled = the canonical death pattern.** Buiy's policies on tracks-latest-Bevy + verifiable claims are partly designed to make sickle's failure mode not recur.
2. **Extension-trait DSLs are powerful but BSN-hostile.** Buiy commits to components-first authoring.
3. **A11y is foundation, not polish.** Buiy ships AccessKit integration on every widget from day 1; sickle's absence is a counter-example, not an option.
4. **APG keyboard contracts are verifiable claims.** Buiy's CI gates assert them; sickle's absence is precedent for treating them as optional and we choose not to.

## Sources

- Surviving fork README (obsolescence notice) — https://github.com/UkoeHB/sickle_ui
- docs.rs (documentation coverage, prelude trait count) — https://docs.rs/sickle_ui/0.4.0/sickle_ui/
- docs.rs prelude — https://docs.rs/sickle_ui/0.4.0/sickle_ui/prelude/index.html
- Bevy 0.15 RequiredComponents PR — https://github.com/bevyengine/bevy/pull/14791
- BSN PR (still draft) — https://github.com/bevyengine/bevy/pull/20158
- bevy_ui renderer caps (the inheritance) — [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md)
- WAI-ARIA APG (the keyboard contracts sickle does not implement) — https://www.w3.org/WAI/ARIA/apg/
- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation media-and-widgets — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Buiy foundation accessibility — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
