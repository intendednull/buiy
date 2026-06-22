**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — limitations: performance, styling, a11y, custom widgets, mobile, homogeneity, pin lag, animation, layout, WCAG

# Critiques

bevy_egui is the most-adopted third-party Bevy UI plugin, but adoption is heavily skewed toward debug overlays and dev tooling rather than shipped production game UI. This file walks the structural reasons. These are critiques of the *paradigm* and the *current implementation* together; many of them apply to immediate-mode UI generally, some specifically to bevy_egui's bridge layer.

This is the "be honest" file. Buiy's foundation spec ([`../../specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/)) makes a deliberate parallel-stack, retained-mode, AccessKit-first bet partly to avoid the pitfalls in this file. See [`comparisons.md`](comparisons.md) § "vs Buiy."

## Immediate-mode performance cost

egui rebuilds its entire widget tree every frame from imperative calls. There's no retained-tree caching of "what the UI looked like" between frames — the layout, the hit-test regions, the visual data are all re-derived from your `ui.button(...)` / `ui.label(...)` calls each frame. Implications:

- **CPU cost is roughly proportional to widget count**, even for static UIs. A scene with 1,000 unchanging widgets pays the same per-frame cost as one with 1,000 changing widgets.
- **GPU cost is bounded by egui's per-frame tessellation**: every frame produces a fresh mesh that's uploaded to the GPU. For complex UIs with thousands of widgets this is measurable. egui's `Context::request_discard` (multi-pass) helps with consistency but not with per-pass cost.
- **No published bench at 1000+-widget scale** for bevy_egui specifically; egui upstream has been progressively optimized for Rerun's data volumes but Bevy-specific overhead (the bridge layer, the render-graph integration) is not benchmarked publicly.

Verdict: fine for hundreds of widgets, questionable past a few thousand. The Buiy harness explicitly targets **1000+ nodes** ([`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) Avoid row "Per-frame full layout rebuild") as a verification gate. Retained-mode UI with change-detection has a structural advantage here.

## Styling limitations: `Visuals` is less expressive than CSS

egui's theming is the `Visuals` struct — a flat struct of color / shape / spacing fields applied uniformly to a context. It is far less expressive than CSS or a real design-token system:

- **No cascade, no specificity.** Either you apply a `Visuals` change globally (via `Context::set_visuals`) or scoped to a single block (via `Ui::style_mut`). No selector model, no inheritance chain.
- **No tokens.** `Visuals` is end-state colors / sizes, not a token layer. Light vs dark requires two complete `Visuals` instances; there's no token-level "primary color" that resolves differently per theme.
- **No state-driven styles.** You write `if response.hovered() { ... }` in the application code, not `:hover { ... }` in a style sheet.
- **No animation primitives in `Visuals`.** Color transitions on hover require manual interpolation in app code.
- **No CSS-Custom-Properties-equivalent** for runtime theme variants.

Third-party crates (`catppuccin/egui`, etc.) work *around* `Visuals` by precomputing complete theme instances; they don't extend the model. See [`comparisons.md`](comparisons.md) § "vs Slint / Iced / Dioxus."

## Accessibility: AccessKit exists, but is limited compared to retained-mode

bevy_egui's AccessKit support was finally re-enabled as an opt-in feature in **0.38 (2025-10-13)**, after being disabled in 0.37 while egui upstream caught up. The feature is real but the integration has structural limitations relative to retained-mode UI:

- **Implicit a11y tree only.** In immediate-mode UI you describe widgets by call sequence per frame, so the a11y tree is *inferred* from those calls. There's no place to attach a stable `aria-labelledby` to a widget that exists across frames — the labelling has to be expressed inline at call time.
- **Live regions are awkward.** `aria-live=polite` semantics (announce when content changes) map poorly onto the immediate-mode rebuild — the screen-reader sees a fresh tree every frame.
- **`aria-activedescendant` semantics are difficult** because the "descendant" is identified by per-frame ID, not by stable entity.
- **`role` mapping is limited** to whatever egui-upstream supports natively; novel roles need upstream API work.
- **AccessKit is opt-in by default** (the `accesskit` Cargo feature is *not* in `default-features`). Apps using `bevy_egui` with default-features get no a11y unless they explicitly enable it.
- **No published WCAG 2.2 conformance** — see § below.

This is **not** a critique of AccessKit itself; AccessKit's egui integration is real and works. The critique is that immediate-mode is a structurally harder substrate for full APG-compliant a11y than retained-mode + entity-based tree. The Buiy stance ([`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/)) is retained-mode + decomposed components + AccessKit-direct, which avoids these structural issues.

## Custom widget complexity

Building a new widget in egui is harder than in retained-mode equivalents, particularly when the widget needs:

- **Persistent state across frames.** egui has `Memory` and `Id` for this, but the state-keeping idiom (`ui.memory_mut(|m| m.data.get_temp_mut_or_default::<MyState>(id))`) is verbose and error-prone.
- **Complex layout.** egui's layout is single-pass and limited; widgets that need to measure themselves before laying out have to call `ctx.fonts(...)` and lay out manually. There's no `MeasureFunc`-style hook.
- **Composition.** Wrapping an existing widget to add behavior requires understanding egui's `Response` shape and re-implementing parts of it.
- **Custom rendering.** Goes through the paint callback (added 0.29, 2024-08-18) — flexible but the API is low-level.

By contrast, building a new widget in a retained-mode framework is "spawn an entity with these components." The custom-widget complexity gap is one reason the bevy_egui ecosystem has fewer third-party widget libraries than the size of its adoption would suggest — see [`ecosystem.md`](ecosystem.md) § "Adjacent crates."

## Touch / mobile ergonomics

bevy_egui added touch-event support in **0.21 (2023-07-10)** and mobile virtual keyboard (web) in **0.30 (2024-10-04)**. The functionality is real but the README explicitly states the virtual keyboard is "still rough around the edges and only works without `prevent_default_event_handling` set to `false`." Specific gaps:

- **Hit-target sizing.** egui's default widget sizes are tuned for desktop mouse precision — small enough that they're hard to tap precisely on mobile.
- **Scroll vs drag conflict.** Distinguishing scroll-to-pan from drag-on-widget on touch input requires manual handling.
- **Multi-touch / gesture support** is minimal.
- **iOS / Android keyboard handling** is best-effort; the virtual-keyboard fix in 0.30 is web-only.

bevy_egui's touch ergonomics are workable for tools/utilities but not competitive with native mobile UI for shipped consumer apps.

## "Looks like egui" homogeneity

The most visible critique. A Bevy game using default bevy_egui defaults has a distinctive visual signature — the rounded-rect dark-grey panels, the specific font, the specific spacing — that's recognizable as "an egui app." This is not unique to egui (Dear ImGui has the same problem, see [`comparisons.md`](comparisons.md) § "vs imgui") but it's particularly noticeable in Bevy where many showcase reels visibly use egui defaults.

Re-styling egui beyond the default look requires:

- A complete `Visuals` rewrite, not just a token tweak.
- Custom widgets for anything that needs a non-rectangular visual shape.
- Paint-callback rendering for anything that needs non-egui shader work.

In practice this is **substantial work** — orders of magnitude more than re-styling a CSS-driven retained-mode UI. The result is that few games invest in the re-styling and many ship the default look, reinforcing the homogeneity. See [`ecosystem.md`](ecosystem.md) § "Visual homogeneity."

## bevy_egui version pinning frustration

The two-layer pin model — bevy_egui pins both a Bevy version and an egui version — produces a recurring upgrade pain:

- **Bevy minor bumps** (every ~3 months) require a bevy_egui release. Apps that want to track Bevy `main` have to wait days–weeks for vladbat00 to release, or vendor / patch bevy_egui themselves.
- **egui minor bumps** (every ~3 months) require a separate bevy_egui release. The pin can lag egui upstream by 2–8 weeks.
- **No back-port window.** bevy_egui 0.39 supports only Bevy 0.18; older app versions don't receive fixes.

Concrete pain point: a downstream consumer (e.g., bevy-inspector-egui) on Bevy 0.17 cannot pick up an egui 0.33 fix that's only available in bevy_egui 0.38 because 0.38 also targets Bevy 0.17. So the two pins line up *sometimes* but not always. The current state (2026-05-22): bevy_egui 0.39.1 pins egui 0.33, while egui upstream is at 0.34.2 — **one minor behind**. See [`distribution.md`](distribution.md) § "egui version pins."

## WCAG 2.2 conformance gaps

No published WCAG 2.2 conformance audit exists for bevy_egui (or for egui upstream) as of 2026-05-22. Specific gaps observable from the codebase:

- **WCAG 1.3.1 Info and Relationships** — partial. Labels are conveyed but `aria-labelledby`-style relationships are hard to express.
- **WCAG 1.4.3 Contrast Minimum** — depends on `Visuals` choices; the default theme is not audited for AA contrast in all states.
- **WCAG 1.4.10 Reflow** — limited. egui layout doesn't natively reflow at narrow widths the way CSS does.
- **WCAG 2.4.7 Focus Visible** — partial. egui draws focus indicators but they don't match OS conventions (the platform-native focus ring is not used).
- **WCAG 2.5.5 Target Size** — default egui sizes are below the 24×24 CSS-pixel minimum on touch.
- **WCAG 4.1.2 Name, Role, Value** — partial via AccessKit; depends on the role mapping completeness.
- **WCAG 4.1.3 Status Messages** — live-region semantics are awkward in immediate-mode (see § "Accessibility").

Buiy commits to WCAG 2.2 AA per-widget mapping in [`../../specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/); bevy_egui has no comparable artifact.

## Animation / transition primitives are weak

egui has minimal animation primitives:

- **`Context::animate_bool`** and `Context::animate_value` — basic 0→1 / value→value interpolation over a fixed duration. Useful for fade-in.
- **No keyframes, no spring physics, no easing-function library** (apart from linear).
- **No transition system** ("transition this property when it changes") — every animation is explicitly driven by app code.
- **No layout transitions** (animate a widget moving between layout positions).
- **No reduced-motion gate** that hooks OS preference.

For UI that *animates* — production game UI in particular, where menus slide in, panels expand, focus rings ease — this is a substantial gap. Buiy's animation primitives ([`../bevy-feathers/comparisons.md`](../bevy-feathers/comparisons.md) § "vs Buiy") include transitions, keyframes, layout transitions, springs, and a reduced-motion gate; bevy_egui has approximately none of this.

## Layout flexibility: simpler than Flexbox / Grid

egui's layout system is intentionally simpler than CSS Flexbox or CSS Grid:

- **One-dimensional flow per `Ui`.** Each `Ui` is either horizontal or vertical (or a grid, but the grid is a fixed-column simple layout); no full Flexbox `flex-grow` / `flex-shrink` / `flex-basis` / `flex-wrap` interaction.
- **No CSS Grid equivalent.** egui's `Grid` widget is a simple two-axis aligner, not the full CSS Grid spec with named lines, fractional units, auto-placement.
- **No container queries.** Layout cannot respond to its container's resolved size.
- **No subgrid.**
- **No anchor positioning, no `position: absolute` with full inset shorthand.**

Third-party `egui_flex` exists to add Flexbox-like behavior but it's not part of egui upstream. See [`ecosystem.md`](ecosystem.md) § "Adjacent egui-ecosystem crates."

For dev tools and debug overlays this simplicity is a feature — egui-the-paradigm is "fast to write, doesn't ask you to think hard about layout." For production game UI it's a constraint — adapting layouts to varied screen sizes / aspect ratios / accessibility-driven font scales is harder.

## The "dev tool, not production UI" framing

Aggregating the above: bevy_egui's strengths (immediate-mode ergonomics, fast iteration, low API surface) cluster on the **developer-facing UI** axis. Its weaknesses (styling rigidity, a11y rough edges, layout simplicity, animation gaps, performance at scale, no flagship game using it) cluster on the **player-facing production UI** axis. The community has internalized this: bevy_egui dominates as a dev-tool substrate (bevy-inspector-egui, editor experiments, debug overlays) and remains rare as a shipped-game-UI substrate.

For Buiy this informs scope: Buiy's "Game and app, both" goal ([`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) goal 6) explicitly targets the production-UI axis that bevy_egui struggles with. See [`comparisons.md`](comparisons.md) § "vs Buiy" and [`open-problems.md`](open-problems.md) § "The production game UI gap."

## Sources

- bevy_egui README — `https://github.com/vladbat00/bevy_egui/blob/main/README.md`.
- bevy_egui CHANGELOG — `https://github.com/vladbat00/bevy_egui/blob/main/CHANGELOG.md`.
- egui CHANGELOG — `https://github.com/emilk/egui/blob/main/CHANGELOG.md`.
- egui `Visuals` source — `https://github.com/emilk/egui/blob/main/crates/egui/src/style.rs`.
- AccessKit — `https://accesskit.dev`.
- WCAG 2.2 — `https://www.w3.org/TR/WCAG22/`.
- WAI-ARIA APG — `https://www.w3.org/WAI/ARIA/apg/`.
- Tiny Glade (custom UI, not bevy_egui) — Pounce Light, 2024.
- `egui_flex` (third-party Flexbox shim) — `https://crates.io/crates/egui_flex`.
- Sibling files: [`distribution.md`](distribution.md), [`history.md`](history.md), [`ecosystem.md`](ecosystem.md), [`comparisons.md`](comparisons.md), [`open-problems.md`](open-problems.md).
- [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § "Per-frame full layout rebuild" and "No flagship game."
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md).
