**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — chronological history (Bevy 0.4 → 0.19-rc.1)

# History

`bevy_ui` is the official ECS-native UI crate in the Bevy workspace. It first appeared in Bevy 0.4 (December 2020) and has shipped on Bevy's ~3-month minor cadence ever since. The crate has been rewritten or substantially re-plumbed at three distinct moments: the **Stretch → Taffy** layout-engine swap (0.8), the **ab_glyph → cosmic-text** text-stack swap (0.15), and the **Required Components / NodeBundle deprecation** refactor (also 0.15). It has *not* yet absorbed BSN — PR #20158 is still draft and explicitly slated to *not* land in 0.18.

## Release timeline (crates.io-verified dates)

| Version | crates.io publish | UI-relevant news |
|---|---|---|
| 0.1.0 / 0.2.x | 2020-08–09 | No `bevy_ui` crate yet. UI was hand-rolled on top of `bevy_render`. |
| 0.3.0 | 2020-11-03 | Still no separate `bevy_ui`. |
| **0.4.0** | **2020-12-19** | **First `bevy_ui`.** Stretch-backed flexbox, `Node` + `Style`, basic Button. ECS-native from day one. |
| 0.5.0 | 2021-04-06 | Z-order, render-pipeline refactor. |
| 0.6.0 | 2022-01-08 | New renderer (`bevy_render` rewrite); `bevy_ui` ported but still on Stretch. |
| 0.7.0 | 2022-04-15 | Glyph atlasing improvements; text on `ab_glyph` + `glyph_brush_layout`. |
| **0.8.0** | **2022-07-30** | **Stretch → Taffy.** PR #4716 swaps the layout engine; Dioxus + Bevy maintain Taffy as the hard fork. Exponential-blowup bug in deep trees is fixed in the process. |
| 0.9.0 | 2022-11-12 | UI rendering refactored to use the new render-graph; relative cursor position component. |
| **0.10.0** | **2023-03-06** | **AccessKit lands.** PR #6874 (Nolan Darilek) adds `bevy_a11y` as an on-by-default crate; Bevy becomes "the first general-purpose game engine with built-in accessibility support" (per AccessKit blog). `AccessibilityNode` is the megacomponent that issue #17644 will later flag. |
| 0.11.0 | 2023-07-09 | UI texture-atlas support. |
| 0.12.0 | 2023-11-04 | Refactored `bevy_a11y` re-exports; partial decoupling work begins. |
| 0.13.0 | 2024-02-17 | UI material support; `Outline` component; gradients RFC discussed. |
| 0.14.0 | 2024-07-04 | `Style` field rename pass; preparation for required-components migration. |
| **0.15.0** | **2024-11-29** | **Three simultaneous refactors.** (a) `ab_glyph` → **cosmic-text** for shaping, BiDi, fallback (the migration tracked in issue #7616). (b) **Required Components** land (PR #14791 by cart); all `*Bundle` types deprecated. `NodeBundle` → spawn `Node` directly. PR #15898 ports UI bundles. (c) `Style` fields are folded into `Node`; `Text` becomes a simpler component; text spans become entities instead of internal arrays. (d) `InputFocus` resource replaces `Focus`; tab + 2D-spatial navigation strategies land. |
| 0.16.0 | 2025-04-24 | `TextShadow` component; transparent sprite picking; GPU-driven rendering improvements that the UI pass benefits from. **No** `bevy_feathers` yet — the orchestrator pre-amble's "ships in Bevy 0.16+" was wrong; Feathers debuts in 0.17. |
| **0.17.0** | **2025-09-30** | **`bevy_ui_widgets` + `bevy_feathers` debut.** `bevy_ui_widgets` is the headless primitive set (Button, Slider, Scrollbar, Checkbox, RadioButton — output of discussion #16900). `bevy_feathers` is the opinionated tooling-targeted widget kit, gated behind the `experimental_bevy_feathers` feature flag. Also lands: `BackgroundGradient`/`BorderGradient`, per-side `BorderColor`, `ViewportNode`, `UiTransform` (specialized 2D transform), `Val` helper functions (`px()`, `percent()`, `vw()`, `vh()`, `vmin()`, `vmax()`), `TextBackgroundColor`, virtual keyboard (Feathers). Renderer API decoupled from `bevy_render` so non-rendering UI uses are possible. |
| **0.18.0** | **2026-01-13** | `Popover` (floating-ui-inspired), `MenuPopup` (keyboard-nav dropdown), improved `RadioButton`/`RadioGroup` event propagation, `ColorPlane` (2D color picker) in Feathers. Font variations: variable weights, strikethroughs, underlines, OpenType features. Pickable text sections. `IgnoreScroll` for sticky headers. `AutoDirectionalNavigation` for arrow-keys / gamepad. `TryStableInterpolate` for animating `Val`. **BSN did NOT merge** (PR #20158 still draft per cart's own commentary; the orchestrator pre-amble's claim that BSN "landed in 0.18" is a fabrication. It is *expected* to land "in some form in 0.18" per cart's July-2025 PR description, but the PR has not been merged). |
| 0.18.1 | 2026-03-04 | Patch release; no major UI changes. |
| **0.19.0-rc.1** | **2026-05-13** | Workspace HEAD is `0.19.0-dev`; MSRV is `1.95.0`. The `Cargo.toml` confirms four features only: `default`, `serialize`, `bevy_picking` (optional), `ghost_nodes` (experimental). |

## Major rewrites in detail

### Stretch → Taffy (0.8, July 2022)

Stretch was the original flexbox layout engine, written by Visly. It went unmaintained in 2020. By early 2022 both Dioxus and Bevy were carrying patches against it. PR [#4716](https://github.com/bevyengine/bevy/pull/4716) (colepoirier) cut over to **Taffy** — a hard fork that DioxusLabs + the Bevy community jointly took over. Taffy fixed an O(2^n) blowup in deep trees during the cut-over. Taffy has since added CSS Grid (Taffy 0.3), block layout (0.4), and continues to be the layout engine Bevy ships with. Bevy currently tracks Taffy 0.6+ (per PR #15844). **Buiy will continue to integrate Taffy directly** (architecture.md § 2.2), so this is also Buiy's layout substrate.

### ab_glyph → cosmic-text (0.15, November 2024)

Text in Bevy 0.4–0.14 was shaped by `ab_glyph` + `glyph_brush_layout`. Issue [#7616](https://github.com/bevyengine/bevy/issues/7616) tracked the migration to cosmic-text from early 2023; it landed in 0.15 (November 2024). cosmic-text brings font shaping for non-Latin scripts (Devanagari, Arabic), bidirectional text (UAX #9), system-font enumeration, and is used in production by Iced, COSMIC Desktop, Zed, and Lapce. Note: system-font-loading wiring was *not* complete at the 0.15 release; it was finished in subsequent patches. cosmic-text is now on 0.16 (PR #22308). **Buiy uses cosmic-text directly** for both rendering and editing.

### Required Components + decomposed components (0.15, November 2024)

The single most consequential refactor for Buiy's BSN-friendly-components stance. PR [#14791](https://github.com/bevyengine/bevy/pull/14791) (cart) introduced *required components* — components that automatically pull in dependencies on insert — and deprecated all `*Bundle` types. PR [#15898](https://github.com/bevyengine/bevy/pull/15898) migrated `NodeBundle` / `ButtonBundle` / `ImageBundle` / `TextBundle` to the new pattern. Components that were bundled together are now spawned individually; `Button` requires `Node`, `Node` is the foundational marker. This was the architectural prerequisite for BSN.

### The bevy_a11y BSN-incompatibility incident (issue #17644)

Filed by **@viridia** on 2025-02-02 (per WebFetch summary; date approximate). Title: "Design of bevy_a11y is BSN-unfriendly." Core argument: the `AccessibilityNode` component bundles all a11y properties together with **private fields exposed only via inconsistent method-style setters**, so BSN — which composes templates by *merging and patching component properties* — cannot patch a11y attributes from layered templates. Viridia: "Because of this, I can well imagine wanting to merge together multiple BSN templates, each of which has opinions about various accessibility attributes." Issue #17644 closed with PR #24308 (a partial fix per viridia's own comment "not a 100% fix"); the megacomponent remains as of Bevy 0.19.0-rc.2. **The general lesson** Buiy embeds (per architecture.md § 2.4 + foundation README goal 3) is: every Buiy component must be small, public-fielded, observable, and decomposed by concern. No megacomponents, no private setters. This is the lesson the orchestrator pre-amble correctly highlights.

### BSN (PR #20158) — *NOT* yet merged

Opened by cart on 2025-07-16 as a **draft**. The PR is explicitly framed as a "public experimentation phase, not intended to be merged in current form." cart wrote that BSN is "unlikely to land in the upcoming Bevy 0.17, but very likely to land in some form in Bevy 0.18." The 0.18 release notes (published 2026-01-13) make **no mention** of BSN landing. As of 2026-05-22 the PR is still draft / closed-but-unmerged. The orchestrator pre-amble's claim that BSN landed in 0.18 is **incorrect**; this doc reports the verified state.

## People

bevy_ui is maintained by Bevy's UI subject-matter experts. Recent UI commits cluster around:

- **@cart** (Carter Anderson) — founder, project lead, Bevy Foundation President. Owns the BSN design and the data-model-for-UI vision.
- **@alice-i-cecile** (Alice Cecile) — Bevy Foundation Secretary, maintainer; reviews most UI PRs and authored the "Vision for Bevy UI" hackmd. Frequent reviewer on widget PRs.
- **@viridia** — author of issue #17644, discussion #16900 (Standard Headless Widgets), and PR #19366 (core button); the de-facto lead on `bevy_ui_widgets` and Feathers.
- **@ickshonpe** — author of `bevy_feathers` PR #19730, UI scrolling improvements (#20093), UI gradients (#18139), cosmic-text 0.16 upgrade (#22308). Probably the highest-volume `bevy_ui` contributor in 2025–2026.
- **@nicoburns** — Taffy maintainer outside Bevy; upgrades Bevy's Taffy pin.
- **@TimJentzsch** — author of the 10 Challenges for Bevy UI Frameworks discussion (#11100, December 2023).

## Sources

- bevy_ui crates.io publishing history — `https://crates.io/api/v1/crates/bevy_ui/versions` (fetched 2026-05-22).
- Bevy 0.8 release notes — `https://bevy.org/news/bevy-0-8/`.
- Bevy 0.10 release notes — `https://bevy.org/news/bevy-0-10/`.
- Bevy 0.15 release notes — `https://bevy.org/news/bevy-0-15/`.
- Bevy 0.16 release notes — `https://bevy.org/news/bevy-0-16/`.
- Bevy 0.17 release notes — `https://bevy.org/news/bevy-0-17/`.
- Bevy 0.18 release notes — `https://bevy.org/news/bevy-0-18/`.
- PR #4716 Stretch → Taffy — `https://github.com/bevyengine/bevy/pull/4716`.
- PR #6874 AccessKit integration — `https://github.com/bevyengine/bevy/pull/6874`.
- PR #14791 Required Components — `https://github.com/bevyengine/bevy/pull/14791`.
- PR #15898 UI bundles → required components — `https://github.com/bevyengine/bevy/pull/15898`.
- Issue #7616 cosmic-text migration — `https://github.com/bevyengine/bevy/issues/7616`.
- Issue #17644 bevy_a11y BSN-unfriendly — `https://github.com/bevyengine/bevy/issues/17644`.
- Discussion #14437 BSN tracking — `https://github.com/bevyengine/bevy/discussions/14437`.
- Discussion #16900 Standard Headless Widgets — `https://github.com/bevyengine/bevy/discussions/16900`.
- Discussion #11100 10 Challenges — `https://github.com/bevyengine/bevy/issues/11100`.
- PR #20158 BSN — `https://github.com/bevyengine/bevy/pull/20158`.
- PR #19730 bevy_feathers — `https://github.com/bevyengine/bevy/pull/19730`.
- AccessKit blog "Bevy first general-purpose game engine with built-in accessibility" — `https://accesskit.dev/accesskit-integration-makes-bevy-the-first-general-purpose-game-engine-with-built-in-accessibility-support/`.
- This Week in Bevy (cosmic-text adoption issue) — `https://thisweekinbevy.com/issue/2024-07-08-bevy-014s-release-cosmic-text-and-water-reflections`.
