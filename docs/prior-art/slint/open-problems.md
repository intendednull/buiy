**Date:** 2026-05-22
**Status:** active
**Subject:** Slint — open problems and structural gaps: license gate, mobile/WASM maturity, AccessKit pin drift, DSL friction, renderer + binding parity

# Open problems

This file collects the things Slint structurally does not solve, organized by area. The point is not to pillory the project — Slint is a real, shipping toolkit with three years of post-1.0 stability — but to give Buiy spec authors a concrete reference for where Slint's design choices leave problems on the table. Several of these are explicit non-goals of Slint (commercial open-core is *by design*); others are roadmap items SixtyFPS GmbH has publicly acknowledged ([Making Slint Desktop-Ready](https://slint.dev/blog/making-slint-desktop-ready)); a few are unresolved tensions in the architecture itself.

## License model: GPL + commercial dual gate

Slint ships under `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0` (see [`governance-and-distribution.md`](governance-and-distribution.md)). The royalty-free option (added 2023-06 with 1.1) covers desktop proprietary applications under specific terms; the **commercial license is mandatory for proprietary embedded or mobile deployment**, and *any* proprietary use that needs guarantees beyond the royalty-free terms becomes a procurement conversation rather than a `cargo add`.

For a **game-engine ecosystem** where Bevy itself is dual-MIT/Apache-2.0 and the broader Rust GUI mainstream (Iced, egui, Dioxus, Xilem) is MIT/Apache, Slint's GPL+commercial gate is a structural mismatch. Two specific frictions:

- **GPL ambient infection.** A GPL-3.0 dependency in a binary means the binary must ship as GPL-3.0 unless covered by the royalty-free or commercial license. Open-source consumers under permissive licenses (MIT, Apache-2.0, BSD) cannot mix Slint with their stack without bumping to GPL-3.0.
- **Royalty-free terms are narrower than they read.** The royalty-free option excludes embedded and mobile; revenue-thresholds and attribution requirements apply; the terms can change between versions (the 2.0 version added 2023-06; future revisions are at SixtyFPS GmbH's discretion).

Buiy ships under MIT OR Apache-2.0 dual-permissive and treats this as a hard non-negotiable; this is the single largest structural difference between the two projects.

## DSL learning curve and `.slint`-vs-Rust integration friction

The `.slint` DSL is well-designed (see [`dsl-language.md`](dsl-language.md)) and the founders' QtQml expertise is real, but the **context switch between `.slint` and host language is a continuing tax**:

- Editors have to support two languages in one project; the LSP (`slint-lsp`) is good but not equal to rust-analyzer.
- Refactoring across the DSL boundary is manual — renaming a Rust struct field that backs a Slint property requires touching both files.
- Stack traces from binding errors surface from generated code, not from `.slint` source — the compiler emits source-map metadata in some paths but the diagnostic experience is uneven.
- Programmatic UI construction (game inventory grids built from runtime data, IDE tree views, dynamic forms) fights the AOT-compiled-DSL grain. `slint-interpreter` exists for runtime-DSL evaluation but at the cost of losing compile-time type checks.

Buiy's "ECS + BSN in Rust, no DSL" choice eliminates this category of friction at the cost of giving up the once-authored-deployed-everywhere cross-language story.

## Mobile target completeness (iOS + Android)

Android backend landed in 1.5 (May 2024); iOS landed in 1.10 (February 2025). Both are functional but **immature relative to desktop**:

- **No documented mobile AccessKit story.** AccessKit's Android adapter and iOS adapter exist but Slint's mobile backends don't visibly wire them; the [`accessibility.md`](accessibility.md) integration is winit-desktop-shaped.
- **Touch / gesture surface is thin.** Slint exposes `TouchArea` with pressed/released/clicked semantics; multi-touch, gesture composition (pinch / pan / rotate), and platform-native scroll physics are partial.
- **Soft-keyboard / IME integration is uneven.** iOS soft-keyboard handling has known quirks per the GitHub issue tracker; Android IME composition works for Latin scripts but complex-script behavior is not extensively documented.
- **Real-device CI is not visible.** SixtyFPS GmbH does not publicly publish a mobile-device CI matrix; mobile users report regressions release-over-release.

Slint's mobile posture is "shipping but evolving"; production deployments at scale on mobile are not visibly documented.

## AccessKit version-pin drift

Slint is one of the named AccessKit "verified adopters" but accumulates pin lag with respect to upstream — see [`accessibility.md`](accessibility.md). Issue [#8148](https://github.com/slint-ui/slint/issues/8148) (April 2025) flagged that Slint cannot upgrade to `accesskit_winit` 0.26 because the new `ActiveEventLoop`-based adapter constructor is incompatible with Slint's event-loop initialization sequence. As of folder-writing time, Slint had absorbed AccessKit updates in 1.12 and 1.13 but had not yet caught up to the recent (2026-05) co-release pin set seen in [`../accesskit/README.md`](../accesskit/README.md).

This is **the live integration cost of consuming AccessKit through `accesskit_winit`** — every upstream `accesskit_winit` major release that touches event-loop ownership is a Slint integration project, not a `cargo update`. Buiy's "AccessKit major release between Bevy minors triggers a Buiy patch release" policy ([foundation README § 5](../../specs/2026-05-07-buiy-foundation/README.md)) will face the same shape.

## Full WCAG 2.2 SC coverage is not claimed

Slint's accessibility documentation enumerates which `accessible-*` properties exist and how they map to AccessKit roles. It does **not** claim full WCAG 2.2 SC conformance. Specifically:

- **No live-region surface in `.slint`.** AccessKit's `Live { Off, Polite, Assertive }` plus `is_live_atomic` / `is_busy` are not exposed as item properties; live-region announcements require host-language API calls or are unavailable. WCAG 4.1.3 (Status Messages) is hard to meet without a live-region story.
- **ACCNAME 1.2 not implemented producer-side.** Slint's `accessible-label` is a direct setter; the full ACCNAME 1.2 cascade (labelledby → aria-label → content → title → placeholder) is not visibly computed. App authors construct labels themselves.
- **Reflow (1.4.10), Non-text Contrast (1.4.11), Focus Not Obscured (2.4.11), Target Size (2.5.8) are theme-and-author responsibilities, not platform guarantees.**
- **Reduced-motion (2.3.3), forced-colors (1.4.3), prefers-contrast** — Slint does not wire OS preferences into a global theming primitive; apps respond to OS preferences themselves where they respond at all.

Buiy's foundation goal 2 (WCAG 2.2 AA as the floor — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)) is more ambitious than Slint's stated posture. Slint is a useful technical reference for AccessKit wiring, not a WCAG conformance reference.

## Language-binding parity

The Slint compiler emits Rust, C++, JavaScript/Node.js, and Python. **Feature parity across bindings drifts**:

- **Rust + C++** are the lead bindings; new features generally land here first.
- **JavaScript / Node.js** is beta; the napi-rs 3.0 port in 1.16 was a significant rework.
- **Python** is beta (added 1.13.0, September 2025); newer and less battle-tested than the others.

Async / event-loop semantics differ per binding (sync callbacks in Rust, promises in JS, async/await in Python with caveats), and feature availability (newer DSL constructs, experimental APIs) reaches the secondary bindings on a lag. Maintaining a four-language binding matrix is a continuing cost that Buiy's "Rust-only" stance sidesteps.

## Renderer maturity by backend

Four renderers ship ([`architecture.md`](architecture.md)), and they are not equivalent:

- **Skia** (default on Win/macOS since 1.14): full feature parity; largest binary footprint (~5–8 MB statically linked).
- **FemtoVG** (OpenGL ES 2.0): lighter; some Skia-specific features (advanced text shaping, certain blend modes) are absent.
- **FemtoVG-WGPU** (1.12+): early; targets `wgpu` 0.x; some Vulkan/Metal/DX12-specific bugs surface as `wgpu` upstream changes.
- **Software** (CPU rasterizer; MCU target): smallest footprint; minimal text-rendering capabilities; no GPU-only features (mix-blend-mode, backdrop-filter, complex gradients).
- **Qt**: depends on Qt installation; pulls in Qt licensing concerns.

The **WGPU renderer is the most-likely-relevant one for a Buiy comparison** because Bevy is wgpu-based. Slint's FemtoVG-WGPU is a younger codepath than Skia and lacks the same feature-parity guarantees.

## Multi-window support depth

Slint supports multiple windows (multiple `Window`-rooted components per app) but with **structural limitations**:

- **Popups render inside the host window**, not as real OS windows. This is called out in [Making Slint Desktop-Ready](https://slint.dev/blog/making-slint-desktop-ready) as a planned-fix gap — combobox dropdowns, tooltips, context menus that escape the host window's bounds clip rather than overflow.
- **Modal windows are limited** — Slint's `Dialog` is a styled component, not an OS-modal-attached child. The blog post lists "modal windows" as a desktop-readiness gap.
- **Cross-window drag-and-drop** is listed as a desktop-readiness gap.
- **Per-window AccessKit adapter** ownership is correctly modeled (one adapter per `WinitWindowAdapter`) but the inter-window focus model is not extensively documented.

Buiy's [`buiy-window-and-surface-design`](../../specs/2026-05-07-buiy-foundation/README.md) sub-spec needs to address these explicitly; Slint's gaps are a checklist.

## Web / WASM target gaps

Slint compiles to WASM via `wasm-pack` + winit's `wasm32` target. The result renders visually and accepts input, but:

- **No web AccessKit adapter.** AccessKit's web adapter does not exist as of folder-writing time ([`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md)); Slint's WASM target ships a UI with no a11y tree exposure to browsers' screen-reader machinery.
- **Filesystem / OS-integration APIs are absent** — clipboard, file open/save, system-color resolution either don't work or work through partial JS shims.
- **Bundle size is large** — Slint runtime + Skia compiled to WASM is multi-megabyte; the Live Preview is fine; production deployments where weight matters are not Slint's target.

The Slint Online Editor (https://slint.dev/editor) is the demonstration of WASM-Slint working; it's also the demonstration of the gaps (no AT exposure, no native OS-feature reach).

## Theme system flexibility vs CSS richness

Slint ships four built-in styles (Fluent, Material, Cupertino, Cosmic) selectable per build. The styles are **not user-extensible at runtime** in the CSS-cascade sense:

- Theme switching at runtime is supported but not arbitrary-tokens-at-runtime; styles are compile-time constructs with runtime palette overrides.
- No equivalent to CSS custom properties (`var(--foo)`) at the language level — the DSL has property bindings, but cross-component theme propagation is by convention, not by cascade.
- Author-defined theming (an app that wants "our brand palette") is by component-level overrides, not by a global token system in the SaaS-design-system sense.

Buiy's [`buiy-theme-tokens-design`](../../specs/2026-05-07-buiy-foundation/README.md) commits to semantic tokens with OS-preference binding; Slint's theme system is a less-rich precedent.

## Embedded-focus shapes feature scope

Slint's MCU target ([`embedded-focus.md`](embedded-focus.md)) is genuinely impressive — bare-metal RGB565 framebuffer rendering on ESP32 / STM32 / Cortex-M devices, no-`std`, allocation discipline. But **the MCU posture shapes what gets prioritized**:

- Rich-text / complex-script shaping is limited because MCUs can't afford it.
- Drag-and-drop, system tray, system clipboard — desktop-readiness gaps that lag because the MCU customer doesn't need them.
- Backdrop blur, mix-blend-mode, complex filter graphs — GPU-shader-heavy features that the MCU customer doesn't need.

This is a deliberate scope choice and a reasonable one for Slint's market; Buiy's web-platform-parity goal points in the opposite direction (full filter pipeline, modal-window OS integration, drag-and-drop, rich-text editing). The two projects optimize for different feature surfaces from a shared technical base.

## Complex animation primitives beyond states + transitions

Slint's animation model is `animate <property> { duration, easing }` plus named states with transitions ([`dsl-language.md`](dsl-language.md)). This covers the common UI-animation cases (button press, hover, page transition) cleanly. Less well covered:

- **Keyframe animations with intermediate frames** — Slint supports linear/eased property tweens; multi-keyframe sequences require composing multiple animate blocks.
- **Springs / physics-based motion** — no built-in spring primitive; the duration+easing surface is the only animation API.
- **Layout transitions (FLIP-style "move from A to B" with implicit layout change)** — partial; element reorder animations are a manual undertaking.
- **Scroll-driven animation / view-transitions API equivalents** — not present.

Buiy's `buiy-animation-design` sub-spec faces these as design decisions; Slint's animation surface is a starting point, not a complete answer.

## Stewardship concentration

SixtyFPS GmbH is a small team (~5–10 named members on the about-us page as of folder-writing). The founders contribute heavily to the codebase; the bus factor is real and acknowledged in [`governance-and-distribution.md`](governance-and-distribution.md). **No foundation, no formal RFC process, no charter** — same shape as AccessKit's stewardship concentration ([`../accesskit/governance.md`](../accesskit/governance.md)), but here the funding model is commercial-license revenue rather than NLnet grants.

A future where SixtyFPS GmbH pivots, sells, or shutters has uncertain implications for Slint's open-source code path. The royalty-free and GPL licenses do not lapse, but ongoing maintenance depends on the company's continued operation. This is the structural risk of single-vendor open-core stewardship; Buiy's foundation-style governance plan ([foundation README](../../specs/2026-05-07-buiy-foundation/README.md)) takes the opposite bet (no revenue stream, community maintenance) with its own structural tradeoffs.

## Sources

- Slint repo: https://github.com/slint-ui/slint
- Slint blog "Making Slint Desktop-Ready": https://slint.dev/blog/making-slint-desktop-ready
- Slint CHANGELOG: https://github.com/slint-ui/slint/blob/master/CHANGELOG.md
- Slint issue #8148 (AccessKit pin drift): https://github.com/slint-ui/slint/issues/8148
- Slint blog "Slint 1.10 Released" (iOS backend): https://slint.dev/blog/slint-1.10-released
- Slint Online Editor (WASM demo): https://slint.dev/editor
- AccessKit platform adapters: [`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md)
- AccessKit governance: [`../accesskit/governance.md`](../accesskit/governance.md)
- Sibling files: [`README.md`](README.md), [`accessibility.md`](accessibility.md), [`architecture.md`](architecture.md), [`dsl-language.md`](dsl-language.md), [`embedded-focus.md`](embedded-focus.md), [`governance-and-distribution.md`](governance-and-distribution.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`history.md`](history.md)
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md), [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
