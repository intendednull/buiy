**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — Bevy's official styled widget kit for editors and utilities, layered over bevy_ui_widgets; the color-only/dark-only theming reference for Buiy's token system

# bevy_feathers

`bevy_feathers` is the official styled widget kit in the Bevy engine workspace (`bevyengine/bevy`, `crates/bevy_feathers/`). It is a **styled layer over `bevy_ui_widgets`** (Bevy's headless widget primitives) running on `bevy_ui` (the substrate): its job is to bind theme tokens to `BackgroundColor` / `BorderColor` / `TextColor`, ship a default dark theme, and ship a fixed set of widget scenes (button, checkbox, slider, menu, text input, …) that wire the headless primitives + tokens + `RoundedCorners` + `FocusIndicator` into a coherent visual. The crate's own `Cargo.toml` describes it as `"A collection of UI widgets for building editors and utilities in Bevy"` — the framing is load-bearing: feathers is opinionated tooling-focused styling, explicitly **not** a general game-UI kit, and per the 0.17 release notes, "Feathers _can_ be used in games, but that is not its motivating use case." The corpus exists because feathers occupies the slot Buiy's own `buiy_widgets` crate parallels (a styled kit over a headless behavior layer), and because its theming model is the closest upstream reference point for the design choices in Buiy's foundation theming spec ([`architecture.md` § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md), [`cross-cutting.md` § 3.14](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).

**Honest assessment.** feathers validates the headless/styled split (`bevy_ui_widgets` ↔ `bevy_feathers`) that Buiy adopts, and its token-as-string-key + observer-driven-reaction shape is clean and change-detection-friendly. But it is young (debuted Bevy 0.17, 2025-09-30, ~8 months old) and narrow by design, and three structural facts make it a reference-not-a-template for Buiy: (1) its `UiTheme` token map is **color-only** — no spacing, radius, motion, or typography scales (those are hardcoded `f32` constants in `src/constants.rs`); (2) it ships **dark-theme-only**, with no light, high-contrast, or OS-preference (`prefers-color-scheme` / `prefers-contrast` / `forced-colors` / `prefers-reduced-motion`) binding; and (3) its dimensions are not audited against WCAG 2.2 AA — `CHECKBOX_SIZE = 18.0` px sits **below the SC 2.5.8 target-size minimum of 24×24** with no documented enlarged hit target. Accessibility is partial-by-construction: most widgets do not set their AccessKit role on the styled entity (the toggle switch is the only one verified to wire `Role::Switch` directly), deferring to whatever `bevy_ui_widgets` sets upstream or shipping with no explicit wiring at all. Each of these is a deliberate Buiy point of departure documented in the children.

## Key facts (verified 2026-05-22)

| Fact | Value |
|---|---|
| Crate | `bevy_feathers` (workspace crate inside `bevyengine/bevy`, `crates/bevy_feathers/`) |
| Role | Styled widget kit layered over `bevy_ui_widgets` (headless) on `bevy_ui` (substrate) |
| Scope framing | "Widgets for building editors and utilities" — explicitly not game-UI-first |
| License | MIT OR Apache-2.0 |
| Latest stable | 0.18.1 (published 2026-03-04) |
| Pre-release | 0.19.0-rc.2 (2026-05-22) |
| Lifetime downloads | 191,700 (fetched 2026-05-22) |
| First release | Bevy 0.17, 2025-09-30 (introduction PR #19730, merged 2025-06-28, viridia) |
| Active maintainer | ickshonpe (per recent commits) |
| Top-level export | `FeathersPlugins` (PluginGroup) bundling `TabNavigationPlugin` + `FeathersCorePlugin` |
| Theme resource | `UiTheme(ThemeProps { color: HashMap<ThemeToken, Color> })` — **color tokens only** (~100+) |
| Theme variants shipped | **Dark only** (`src/dark_theme.rs`); no light or high-contrast theme in source |
| Non-color dimensions | Hardcoded `f32` in `src/constants.rs` (`ROW_HEIGHT=24`, `CHECKBOX_SIZE=18`, `MEDIUM_FONT=14`, …) — not customizable through `UiTheme` |
| OS-preference binding | None (`prefers-color-scheme` / `prefers-contrast` / `forced-colors` / `prefers-reduced-motion` not consulted) |
| WCAG SC 2.5.8 (target size) | `CHECKBOX_SIZE=18` is below the 24×24 minimum; not audited per-widget |
| A11y role wiring | Partial — only the toggle switch sets `AccessibilityNode(Role::Switch)` directly; most widgets defer upstream or are unwired |
| Controls shipped | ~14 (button, checkbox, toggle switch, radio, slider, color slider/plane/swatch, disclosure toggle, menu, text input, number input, virtual keyboard) + pane/subpane/group/spacer containers + icon/label display |
| Governance | Bevy Foundation (Washington 501(c)(3)); inherits the workspace RFC / SME / release apparatus |

## Strengths

- **Clean headless/styled split.** `bevy_ui_widgets` owns behavior; `bevy_feathers` owns presentation. This validates Buiy's intent to keep behavior + presentation cleanly split inside `buiy_widgets`.
- **Simple, reactive theming.** Token-as-string-key lookup with observer-driven application (`ThemeBackgroundColor(token)` etc.); `update_theme` rewrites all colors in one pass on `UiTheme` change. The component-per-styled-property decomposition maps directly onto Buiy's decomposed-component philosophy.
- **Coherent, opinionated editor look** with focus indicators, rounded corners, and a consistent button/slider/menu vocabulary suitable for tooling.

## Weaknesses

- **Color-only token scope.** No `space`, `radius`, `motion`, or `typography` scales — those are hardcoded constants, so a denser/sparser variant requires forking the widget scenes, not retheming.
- **Dark-only, no OS-preference binding.** No light or high-contrast theme ships; `prefers-*` queries are not consulted; missing tokens fall through to a silent default.
- **Dimensions below WCAG 2.2 AA.** `CHECKBOX_SIZE=18` < the SC 2.5.8 24×24 target-size minimum; the token set is not designed against the AA criteria.
- **Partial accessibility.** Most controls don't set their AccessKit role on the styled entity; no tri-state checkbox (`aria-checked="mixed"`), no `Role::Heading`, disclosure toggle inherits checkbox semantics rather than `button` + `aria-expanded`.
- **Narrow catalog.** No dialog/modal, tooltip, tabs, tree, listbox/combobox, table, progress, toast, or multi-line editor — consistent with the editor-utilities scope but not a WCAG 2.2 AA APG widget set.
- **Single global theme resource.** No subtree-scoped theme, no theme hot-reload (it's a runtime resource, not an asset).

## Lessons for Buiy

There is no `lessons.md` in this folder; the consult-this-when-designing material is distributed across the per-file **"Implications for Buiy"** sections — principally [`theming.md`](theming.md) (the load-bearing theming comparison + borrow/avoid list) and [`widgets.md`](widgets.md) (catalog gaps and the a11y-wiring gap) — with the cross-folder BSN-friendliness lesson in [`docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md). All Buiy-side statements are **sourced to the foundation spec as target state, not decided here**:

- **Validates** — the headless/styled split and the per-window coexistence policy (a feathers editor pane and a Buiy game UI live on separate windows; foundation [`cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)). Feathers's partial a11y wiring validates Buiy's AccessKit-first model (role + name + states explicit on every widget; foundation [`architecture.md` § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)).
- **Avoid** — color-only token scope, dark-only default with no variants, flat string namespace with no type guard, global-resource-only theming, no hot-reload / contrast linter / OS-preference binding, and dimensions not audited to WCAG 2.2 AA.
- **Borrow** — token-as-string-key lookup with observer-driven reaction and the component-as-binding pattern, but with hierarchical tokens, a typed-token API, and the multi-scale token set (`color.*` + `space.*` + `radius.*` + `motion.*` + `typography.*`) Buiy's foundation commits to ([`architecture.md` § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md), [`cross-cutting.md` § 3.14](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).

## Reading order

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, key facts, strengths/weaknesses, lessons pointer, reading order. |
| [`architecture.md`](architecture.md) | Plugin shape, module layout, the layer cake over `bevy_ui_widgets` + `bevy_ui`. |
| [`widgets.md`](widgets.md) | Widget catalog: shipped controls, spawn/keyboard/role contracts, gaps relative to WAI-ARIA APG. |
| [`theming.md`](theming.md) | **The load-bearing theming file.** `UiTheme` resource, token taxonomy, color-only scope, dark-only default, OS-preference status, the Buiy comparison table. |
| [`accessibility.md`](accessibility.md) | bevy_a11y integration, per-widget wiring, focus model, WCAG 2.2 gap analysis (incl. the SC 2.5.8 `CHECKBOX_SIZE` gap). |
| [`integration.md`](integration.md) | Adding the plugin, the dependency contract, spawning, per-window coexistence with custom UI and with Buiy. |
| [`comparisons.md`](comparisons.md) | Row-by-row vs `bevy_ui_widgets`, bevy_egui, bevy_lunex, sickle_ui, woodpecker_ui, kayak_ui, and Buiy. |
| [`distribution.md`](distribution.md) | Cargo features, dependencies, platform matrix, MSRV, release cadence, assets. |
| [`ecosystem.md`](ecosystem.md) | Consumers, ecosystem position, relationship to third-party widget kits and to Buiy. |
| [`governance.md`](governance.md) | Stewardship, SMEs, inclusion-decision rationale, direction signals. |
| [`history.md`](history.md) | Chronological history (pre-history → Bevy 0.19-rc), per-release additions. |
| [`critiques.md`](critiques.md) | Third-party critiques, structural gaps, honest limitations. |
| [`open-problems.md`](open-problems.md) | What the crate structurally does not solve as of 2026-05-22. |

## Sources

- `bevy_feathers` on crates.io — https://crates.io/crates/bevy_feathers
- `bevy_feathers` source (main HEAD) — https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers
- `theme.rs` / `tokens.rs` / `dark_theme.rs` / `constants.rs` (main HEAD) — https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers/src
- PR #19730 (bevy_feathers introduction, Bevy 0.17, merged 2025-06-28) — https://github.com/bevyengine/bevy/pull/19730
- Issue #17644 (bevy_a11y BSN-unfriendly) — https://github.com/bevyengine/bevy/issues/17644
- PR #24308 (AccessibleLabel, merged 2026-05-21 for 0.19) — https://github.com/bevyengine/bevy/pull/24308
- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- Bevy 0.17 release notes — https://bevy.org/news/bevy-0-17/
- Buiy foundation — theming/architecture §2.5: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation — cross-cutting §3.14 (theming + user preferences), §3.18 (per-window coexistence): [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- Buiy foundation — widget catalog: [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
