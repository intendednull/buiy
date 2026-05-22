**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — open problems: a11y maturity, i18n, theme expressiveness, touch/gamepad, performance at scale, multi-window context, render-graph integration, WASM size, cadence misalignment, the production-game-UI gap

# Open problems

The unresolved questions and structural gaps facing bevy_egui (and, where the issue is upstream, egui itself) as of 2026-05-22. Each section names the problem, the current state of the world, and where the work would have to happen to close it.

This file is the "what would have to change for bevy_egui to be a credible production game UI substrate" inventory. Several of these problems are paradigm-structural (cannot be closed without abandoning immediate-mode) and several are integration-engineering (could be closed with focused effort). They are distinguished below.

## Accessibility — full APG conformance, screen-reader interop maturity

**State.** AccessKit support was re-enabled in bevy_egui 0.38 (2025-10-13) as an opt-in feature; egui upstream's a11y model was matured to support this. The basic shape — name, role, value, focus — is wired. But:

- **No published WAI-ARIA APG widget conformance matrix.** No claim that the egui Listbox is APG-conformant, that the egui Combobox handles `aria-activedescendant` per the APG spec, etc.
- **Live regions are paradigm-awkward** (see [`critiques.md`](critiques.md) § "Accessibility").
- **`aria-labelledby` referencing across widgets** requires stable IDs, which immediate-mode either has to fake or expose explicitly.
- **No screen-reader test matrix** — no documented testing against NVDA / JAWS / VoiceOver / TalkBack / Orca.
- **`aria-live=polite` and `aria-live=assertive`** semantics need explicit programmer attention to keep the AccessKit tree consistent across the immediate-mode rebuild.

**Where the work happens.** Most of this is egui-upstream territory (the `Atoms`-based widget reconstruction in egui 0.28+ creates the stable-ID infrastructure that APG conformance needs). bevy_egui inherits whatever egui ships; the bevy_egui-specific work is narrower (ensuring the AccessKit adapter is plumbed correctly per-window and AccessKit version pinning is sound). Structural: paradigm-tied — full APG-quality a11y in immediate-mode is genuinely harder than in retained-mode.

## Internationalization — BiDi text, vertical writing modes, complex script shaping

**State.** egui's text rendering is via its own text-layout code historically; egui 0.34 switched to `skrifa` + `vello_cpu` for sharper rendering with font hinting and variations. BiDi (right-to-left languages like Arabic, Hebrew) support is partial. Vertical writing modes (Japanese, Mongolian) are not supported. Complex script shaping (Arabic ligature, Devanagari conjuncts) depends on whichever font engine the egui version uses.

- **`Atoms`-based widgets** in egui 0.28+ are a step toward better text-mixed-with-iconography layout, but full BiDi-conformant inline layout is not there.
- **No language tag propagation** to drive locale-specific behavior.
- **No locale-specific number / date formatting hooks** — apps wire `chrono` / `icu` themselves and feed strings to egui.

**Where the work happens.** Almost entirely egui upstream. bevy_egui passes text strings through unmodified. Buiy commits to cosmic-text directly (which has more mature BiDi + complex-script support than egui's historical engine); this is one of the structural Buiy-vs-bevy_egui differences.

## Theme expressiveness — tokens, variants, OS-pref binding

**State.** Per [`critiques.md`](critiques.md) § "Styling limitations": egui's `Visuals` is a flat struct, not a token system. No cascade, no specificity, no state-driven styles in the data model. Third-party theme crates (`catppuccin/egui`, custom palette packages) work around `Visuals` by shipping complete theme instances.

What's missing for parity with web-platform design systems:

- **A token layer**: `--primary-color` resolving to different end-colors per theme variant.
- **Variant binding to OS preferences** (`prefers-color-scheme`, `prefers-contrast`, `prefers-reduced-motion`).
- **Subtree-scoped overrides** comparable to CSS custom-property inheritance.
- **Hot reload** of theme data without rebuilding the app.
- **Contrast linting** — proactive WCAG 1.4.3 / 1.4.11 checks.

**Where the work happens.** Egui upstream would need to redesign `Visuals` as a token model. This is a substantial breaking change and there's no public proposal for it as of 2026-05-22. bevy_egui inherits whatever upstream ships. Structural cost: high.

## Touch / gamepad UX

**State.** Touch event support since bevy_egui 0.21 (2023-07-10); mobile virtual keyboard (web) since 0.30 (2024-10-04). README is explicit the virtual keyboard is "still rough around the edges and only works without `prevent_default_event_handling` set to `false`." Gamepad support is **not native** to egui — `bevy_egui_kbgp` adds it as a third-party crate.

- **Hit-target sizing**: egui defaults are mouse-precision; not tuned to the WCAG 2.5.5 (24×24 CSS-pixel) or Material (48dp) targets.
- **Scroll-vs-drag** disambiguation on touch needs manual app code.
- **Multi-touch / gesture** support is minimal.
- **Gamepad navigation** is `bevy_egui_kbgp`'s responsibility, not egui's; no shared / standardized focus-and-confirm model.

**Where the work happens.** Some upstream (default widget sizing, gesture API), some bevy_egui (mobile keyboard polish, gamepad integration), some standardized at the ecosystem level (the gamepad-focus standard).

## Performance at 1000+-widget scale

**State.** No published benchmark of bevy_egui at 1000+-widget scale. egui upstream has been optimized for Rerun's data volumes (see [`history.md`](history.md) § "Rerun.io stewardship"), and Rerun's viewer can plausibly render thousands of UI elements at 60fps — but Rerun is non-Bevy and the bevy_egui bridge layer adds its own overhead.

- **Per-frame full rebuild** is the paradigm cost (see [`critiques.md`](critiques.md) § "Immediate-mode performance cost"). Even widgets that don't change pay the full per-frame cost in CPU.
- **No published bench** at productivity-app scale (e.g., a 5,000-row table, a deep tree view with thousands of expanded nodes, a multi-panel IDE-style layout).
- **Multi-pass mode** (`Context::request_discard`, wired through in bevy_egui 0.34) multiplies the per-frame cost when it triggers.

**Where the work happens.** Egui upstream's continuing optimization work + bevy_egui's bridge-layer profiling. Paradigm-tied to immediate-mode; retained-mode + change-detection has a structural advantage that bevy_egui cannot adopt without abandoning the paradigm.

## Multi-window egui context management

**State.** Multi-window support landed in bevy_egui 0.4 (2021-04-10) and has been refined repeatedly. The 0.35 release (2025-06-30) refactored context attachment to cameras (not windows alone), enabling per-camera egui surfaces. Worldspace UI (0.29) and mesh-picking diegetic UI (0.35) further extended the multi-context story.

Open questions:

- **Context lifecycle vs window lifecycle.** Window destruction has to be reflected in egui state without leaks; bevy_egui 0.37.1 (2025-10-08) shipped a fix for minimization/closure panics. The shape is sound but edge cases recur.
- **Cross-context focus.** When focus moves between two egui contexts (two windows, two cameras), the focus model needs consistency. There's no published focus-tree-across-contexts story.
- **Cross-context clipboard / drag-and-drop** is the typical app multi-window UX and is not well-documented for bevy_egui.

**Where the work happens.** bevy_egui-specific integration work, building on egui upstream's `Context`-per-pane model.

## Custom render-pass integration with Bevy's modern render graph

**State.** bevy_egui's render path bypasses `bevy_ui` and goes directly to Bevy's render graph. Paint callbacks (since 0.29, 2024-08-18) let apps inject custom render passes inside egui paint. The `render` Cargo feature gates this; apps can run bevy_egui without it for headless / measurement use cases. Bindless texture support landed in 0.37 (2025-10-01) for large texture sets.

Open questions:

- **Compatibility with Bevy 0.18's render-graph evolution.** Bevy's render graph has been changing across recent releases (the camera/view refactor; the render-graph node API churn); bevy_egui has to track these without exposing too much surface to consumers.
- **Order of bevy_egui vs bevy_ui passes.** Configurable since 0.36 (2025-08-04), but the semantics ("paint over" vs "paint under" vs "interleave") are limited.
- **Custom shader effects.** Paint callbacks are the extension point but they're low-level; there's no `UiMaterial`-style high-level material abstraction.
- **Render-graph integration with stencil / depth** is unclear for diegetic UI on complex meshes.

**Where the work happens.** Almost entirely bevy_egui (the Bevy-side integration); some egui upstream coordination for the paint-callback API surface.

## WASM bundle size

**State.** bevy_egui supports WASM well (see [`distribution.md`](distribution.md) § "Platform support"), but no published bundle-size benchmark exists. WASM bundle sizes for Bevy itself are commonly **multi-megabyte**, dominated by Bevy + wgpu + winit + the rendering substrate; adding bevy_egui adds the egui upstream code plus the bridge layer.

- **No tracked size budget** in either the bevy_egui or egui release notes.
- **`default-features = false`** can shed a meaningful chunk (no clipboard, no URL opening, no AccessKit) but the size is still dominated by Bevy.
- **Tree-shaking** is limited — Rust's monomorphization tends to produce more code, not less, when generics are used aggressively (which egui does).

**Where the work happens.** Cross-ecosystem — Bevy core + egui upstream + bevy_egui bridge each contribute. Probably stuck at "multi-megabyte" for the forseeable future.

## egui upstream cadence vs Bevy cadence — misaligned

**State.** Both projects ship ~3-month minor releases, but their release calendars don't align. Examples:

- egui 0.32 (2025-07-10); Bevy 0.17 (2025-09-30); bevy_egui 0.36 caught egui 0.32 first (2025-08-04), then bevy_egui 0.37 (2025-10-01) caught Bevy 0.17 — two releases to absorb two upstream cadences.
- bevy_egui 0.38 (2025-10-13) caught egui 0.33 *after* bevy_egui 0.37 had caught Bevy 0.17; a 12-day gap between two bevy_egui releases.
- As of 2026-05-22, egui is at 0.34.2 while bevy_egui 0.39.1 pins egui 0.33 — **one minor behind** ([`distribution.md`](distribution.md) § "egui version pins").

**Where the work happens.** Structural. The two-layer maintenance model (see [`governance.md`](governance.md)) makes this inherent — vladbat00 will always be chasing two cadences. The only fix would be tighter coordination (e.g., aligning Bevy and egui release windows), which neither project has incentive to do.

## Multi-pass mode policy

**State.** bevy_egui 0.34 deprecated the option to disable multi-pass mode. Multi-pass is now the default and (in 0.39 onward) only mode. egui's multi-pass model allows another paint pass on demand (via `Context::request_discard`) when a widget's layout requires re-measurement.

Open questions:

- **Per-frame cost ceiling.** Multi-pass can multiply per-frame work when discards trigger. No published bound on how often this happens in typical apps.
- **Frame pacing.** Multi-pass interacts with Bevy's frame scheduling in ways that aren't fully documented.
- **App control.** No knob for app code to force single-pass for performance-critical scenes.

**Where the work happens.** egui upstream (the model definition) + bevy_egui (the Bevy scheduling integration).

## The "production game UI" gap

**The headline open problem.** Despite 2M+ lifetime downloads and dominance in dev tooling, **no flagship commercial game ships its production player-facing UI on bevy_egui**. Tiny Glade (the most-cited Bevy commercial release) wrote its own UI renderer. Other commercial Bevy releases either don't ship significant UI or use `bevy_ui` directly.

The gap is structural, not incidental. The cluster of weaknesses ([`critiques.md`](critiques.md)) — styling rigidity, animation gaps, layout simplicity, immediate-mode a11y limits, touch ergonomics, visual homogeneity — all point at "production-UI-grade polish is genuinely hard in egui." Closing the gap requires either:

1. **Substantial upstream egui work** to add the missing primitives (tokenized theming, animation system, full APG conformance, etc.) — years of work, multiple persons.
2. **Substantial bevy_egui work** to layer those primitives over egui — risks fork-from-upstream divergence; vladbat00 isn't resourced for this.
3. **A retained-mode alternative** — what `bevy_feathers` is aiming at on the editor axis, and what Buiy is aiming at on the comprehensive app-and-game axis.

The path of least resistance is (3): the production-UI niche gets served by retained-mode UIs (bevy_ui, bevy_feathers, bevy_lunex, Buiy), while immediate-mode bevy_egui continues to dominate the dev-tool niche. This is the implicit ecosystem trajectory; no public decision has been published but the editor-roadmap signaling (see [`ecosystem.md`](ecosystem.md) § "Bevy editor experiments") points this way.

For Buiy: the production-game-UI gap is exactly the slot Buiy targets. The Buiy spec's "Game and app, both" goal ([`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) goal 6) and the parallel-stack web-platform-parity stance are the design response to this gap.

## Sources

- bevy_egui CHANGELOG — `https://github.com/vladbat00/bevy_egui/blob/main/CHANGELOG.md`.
- bevy_egui README — `https://github.com/vladbat00/bevy_egui/blob/main/README.md`.
- egui CHANGELOG — `https://github.com/emilk/egui/blob/main/CHANGELOG.md`.
- WAI-ARIA APG — `https://www.w3.org/WAI/ARIA/apg/`.
- WCAG 2.2 — `https://www.w3.org/TR/WCAG22/`.
- AccessKit — `https://accesskit.dev`.
- Bevy 0.17 release notes (editor direction) — `https://bevy.org/news/bevy-0-17/`.
- Bevy 0.18 release notes — `https://bevy.org/news/bevy-0-18/`.
- Tiny Glade — Pounce Light, 2024.
- Rerun.io (egui production user, non-Bevy) — `https://www.rerun.io/`.
- `bevy_egui_kbgp` (gamepad nav for egui) — `https://crates.io/crates/bevy_egui_kbgp`.
- Sibling files: [`distribution.md`](distribution.md), [`history.md`](history.md), [`governance.md`](governance.md), [`ecosystem.md`](ecosystem.md), [`critiques.md`](critiques.md), [`comparisons.md`](comparisons.md).
- [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) "No flagship game = no UX battle-testing."
- Buiy foundation spec — [`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/).
