**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — accessibility status (absent), and structural open problems Makepad doesn't solve

# Open problems

The set of things Makepad does not solve, surfaced honestly. Accessibility is the load-bearing absence and gets the first section.

## Accessibility (the central gap)

**Status: entirely absent.**

There is no AccessKit integration. There is no AT-SPI / UIA / NSAccessibility / UIAccessibility / TalkBack producer-side wiring. There is no plan-of-record to add any. The only open accessibility issue, **[#196 "On Accessibility"](https://github.com/makepad/makepad/issues/196)**, has been open since **2023-08-08** — over 21 months at folder-write — with zero recorded team responses, zero assignees, zero labels, and no linked PRs.

Quoting the issue verbatim (from `aschrijver`'s opening message, paraphrased per the WebFetch summary):

> In the presentation Rik mentioned that for Accessibility most likely AI would soon do the heavy lifting for us. Please, don't let a11y fall down the road-side like that.

The community member then cites concerns from visually impaired Fediverse users about unmet accessibility needs, and asks that accessibility be "a first-class citizen of any GUI framework."

The maintainers' position, as reported in the issue, is that **AI will handle accessibility later** — either by retrofitting it onto existing UIs at runtime, or by being smart enough to make AT bridging unnecessary, or by some other unspecified future mechanism. This is **not the position the Rust UI ecosystem at large takes** (Slint, egui, Bevy via `bevy_a11y`, Xilem / Masonry, Freya all integrate AccessKit), and it is **not a position Buiy can accept** — WCAG 2.2 AA is the floor per [foundation README goal 2](../../specs/2026-05-07-buiy-foundation/README.md).

Concrete consequences:

- **Screen-reader users cannot use Makepad apps.** VoiceOver on macOS / iOS / iPadOS, NVDA / JAWS / Narrator on Windows, Orca on Linux, TalkBack on Android — all of these find a Makepad window opaque. There is no accessibility tree to read, no focusable elements they recognize, no role / name / value semantics.
- **Switch Control / Voice Control / external accessibility hardware fails.** All of these depend on the same accessibility tree producers.
- **Robrix is inaccessible.** A Matrix client is exactly the kind of application that screen-reader users need; Robrix as shipped is inaccessible for them.
- **WCAG / EN 301 549 / ADA / EAA compliance is unreachable.** Public-sector deployments, EU-resident products under EAA (in force 2025-06-28), US Section 508 — all require accessibility.

The Buiy-side corrective is the entire [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md) sub-spec and the AccessKit-first architecture commitment. See [`lessons.md`](lessons.md) Avoid #1.

## APG widget coverage gaps

Beyond the absence of AccessKit semantics on existing widgets, the **widget catalog itself doesn't cover the WAI-ARIA APG patterns** Buiy commits to. Per `makepad-widgets` 1.0.0 docs scan, Makepad ships:

Present: Button, CheckBox, RadioButton, Slider, TextInput, DropDown, View, ScrollView, ScrollXYView, PortalList, Window, Modal, PopupNotification, Tooltip, Splitter, DockPanel, Tabs, StackNavigation, Label, Icon, Image, VideoPlayer, LoadingSpinner, LinkLabel, Markdown / HTML, SlidesView, PageFlipper, FileTree.

Missing or partial against the APG:

- **Combobox** (autocomplete patterns) — TextInput + DropDown exist but no APG-shaped Combobox with listbox-popup.
- **TreeView** with APG keyboard contract (FileTree is the closest; doesn't claim APG conformance).
- **MenuBar / Menubar** with APG keyboard contract (no dedicated widget).
- **MenuButton** with the APG actions enumerated.
- **DatePicker / Calendar / TimePicker / DateInput** — entirely absent.
- **DialogModal** with focus-trap + initial focus + restore-on-close per APG.
- **Toolbar** with APG roving-tabindex / arrow-key contract.
- **TablistTabsPanel** with APG keyboard contract (Tabs widget exists but doesn't claim APG conformance).
- **Accordion** with APG patterns.
- **Disclosure (details/summary)** widgets.
- **Listbox + Grid** with APG patterns.
- **TreeGrid** — composite APG pattern.
- **Marquee / Carousel** with APG patterns.

Buiy's `buiy-widget-catalog-design` ([foundation README](../../specs/2026-05-07-buiy-foundation/README.md)) is a substantially larger surface than Makepad's catalog.

## Documentation maturity

`docs.rs` reports:

- `makepad-widgets` 1.0.0: **5.92%** of public items documented.
- `makepad-live-compiler` 1.0.0: **0%**.
- `makepad-shader-compiler`: 0%.
- `makepad-platform`: 0% (or near-zero).

The project site `makepad.dev` is a JS SPA shell that returns "Makepad is loading" with no static fallback. The `makepad.nl` vanity domain failed to load during research (certificate validation error at the time of fetch). There is no Slint-grade reference manual, no API tutorials, no language specification for Live, no migration guide for the 0.6 → 1.0 transition.

Learning Makepad is **example-driven**: read `examples/hello_world`, `examples/uizoo`, `examples/hotload_ui`. This is a barrier-to-entry compared with Slint's `docs.slint.dev` or Bevy's `bevyengine.org/learn/` content.

## Tooling outside Makepad Studio

There is no editor-agnostic LSP for the Live language. Makepad Studio is the supported authoring environment; VS Code / Helix / Neovim / Sublime / Emacs users do not have a `slint-lsp`-grade equivalent. Third-party syntax-highlighting extensions exist but lack semantic analysis (go-to-definition, autocomplete with type info, rename, refactor across `.live` ↔ `.rs`).

This *forces* Makepad authors into Makepad Studio for serious work — which then dogfoods Studio, but locks out users who don't want to switch editors. Slint's editor-agnostic LSP is the contrast.

## Adoption gap despite 1.0

`makepad-widgets` lifetime downloads: **16,974** (1,768 recent 90-day). For a 1.0-marked Rust UI library, this is small. For comparison at folder-write:

- Slint 1.16.x: 1.1M lifetime, 236k recent (90-day).
- egui: multi-million lifetime.
- Iced: multi-million lifetime.
- Dioxus: multi-million lifetime.

The gap suggests Makepad is **curiosity-discovered** (6.4k GitHub stars indicates broad awareness) but **not production-adopted** outside the Robius community. Possible explanations: the DSL learning curve, the lack of editor tooling outside Makepad Studio, the documentation gap, the accessibility-absence-blocking-procurement, the "still-feels-experimental" perception.

Buiy planning that assumes 1.0 ⇒ wide adoption should re-read this gap as evidence that **1.0 is necessary but not sufficient**. Documentation, tooling, and accessibility all gate the adoption curve.

## Live language as yet-another-DSL

`.live` is its own syntax. Learning it is a new language acquisition cost. Editor support is concentrated in Makepad Studio. Cross-language refactor (Rust ↔ Live) is manual. Two-language stack traces (errors in expanded Live code surface as compile errors against macro-generated Rust) are confusing.

Slint has the same shape (`.slint` is its own syntax) but with editor-agnostic LSP. Buiy's choice to keep BSN within Rust syntax (macro form) is the corrective design. See [`live-language.md`](live-language.md) and [`../slint/dsl-language.md`](../slint/dsl-language.md).

## Production deployment beyond Robrix and Makepad Studio

The Robius community ports (`makepad_wechat`, `makepad_wonderous`) are demo / learning artifacts, not production deployments. Robrix is alpha (`v1.0.0-alpha.1`). Makepad Studio is the IDE itself.

Net: **two real production users (Studio + Robrix-alpha)**. Buiy's "ecosystem maturity" comparison should be honest: Makepad is at the "small ecosystem with two visible users" stage, not the "Slint at 1.16.1 with named industrial customers" stage. See [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md).

## Custom text-shaping limitations

Makepad's text stack lacks: BiDi (UAX #9), complex-script shaping (Arabic / Indic / Thai joining + reordering), comprehensive font fallback, full IME composition for CJK / Korean. cosmic-text would solve all four; Makepad does not use it. Robrix Matrix messages with mixed RTL / LTR content surface this. See [`gpu-rendering.md`](gpu-rendering.md).

## No theming token system

Live's `THEME_*` global constants + bundled `.live` theme files give per-build theme selection (`theme_desktop_dark` / `theme_desktop_light` / `theme_mobile_*`) but not runtime token cascade. No `prefers-color-scheme` / `forced-colors` / `prefers-contrast` / `prefers-reduced-motion` automatic OS-preference wiring. Buiy's semantic-tokens commitment ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md)) is the corrective.

## No wgpu (the backend-maintenance debt)

Maintaining four GPU backends (Metal / DX11 / OpenGL / WebGL) in-house is an ongoing cost. New GPU features (compute, ray tracing, mesh shaders) require four implementations. WebGPU is not yet a backend. See [`gpu-rendering.md`](gpu-rendering.md).

## No winit (the AccessKit-lockout consequence)

Skipping winit means the canonical `accesskit_winit` adapter path is unavailable. Adding accessibility later would require either replatforming on winit (large refactor) or hand-rolling per-platform AT bridges. See [`architecture.md`](architecture.md).

## Implications for Buiy

Every problem above is a Buiy choice in the opposite direction — Buiy's foundation spec is structurally responsive to Makepad's gaps:

| Makepad gap | Buiy choice that addresses it |
|---|---|
| No AccessKit | AccessKit-first ([architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)) |
| Tiny APG widget set | Full APG widget catalog ([media-and-widgets.md](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) |
| 5.92% / 0% docs | rustdoc-as-spec convention; verification harness enforces examples |
| LSP locked to Makepad Studio | `rust-analyzer` covers BSN macro form; editor-agnostic |
| Custom text without BiDi / complex shaping | cosmic-text ([architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) |
| No theming tokens or OS-preference wiring | Semantic tokens + OS-preference-binding ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md)) |
| No wgpu (4 backends to maintain) | wgpu via Bevy render graph ([architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) |
| No winit (AT bridges unreachable) | winit via Bevy ([architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)) |
| Yet-another-DSL learning curve | BSN keeps Rust syntax ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)) |

This makes Makepad an unusually clean **counter-template**: every Buiy spec decision can be re-grounded against "and why we're not Makepad on this dimension." That's high-value prior art even though Buiy isn't borrowing the architecture.

## Sources

- Issue #196 "On Accessibility": https://github.com/makepad/makepad/issues/196
- docs.rs coverage: https://docs.rs/makepad-widgets/1.0.0/makepad_widgets/
- Makepad website status: direct fetches at folder-write (`makepad.dev` JS shell, `makepad.nl` cert issue)
- Crates.io download counts: https://crates.io/api/v1/crates/makepad-widgets
- Sibling files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`live-language.md`](live-language.md), [`gpu-rendering.md`](gpu-rendering.md), [`mobile-targets.md`](mobile-targets.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`critiques.md`](critiques.md), [`lessons.md`](lessons.md)
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/)
- AccessKit comparison: [`../accesskit/lessons.md`](../accesskit/lessons.md)
- Slint a11y comparison: [`../slint/accessibility.md`](../slint/accessibility.md)
