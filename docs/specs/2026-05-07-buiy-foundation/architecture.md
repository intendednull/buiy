# Architectural foundation

**Parent:** [README.md](README.md)

## 2.1 One-line summary

Buiy is a parallel UI stack to bevy_ui, integrating the same underlying primitives (Taffy, cosmic-text, AccessKit, bevy_picking, Bevy's render graph) directly, with its own component model and its own render pipeline.

## 2.2 Underlying primitives Buiy integrates directly

- **[Taffy](https://github.com/DioxusLabs/taffy)** — Flexbox, CSS Grid, Block layout. We feed it our component data. As Taffy adds subgrid, container queries, etc., we get them.
- **[cosmic-text](https://github.com/pop-os/cosmic-text)** — text shaping, BiDi (UAX #9), font fallback, color emoji, RTL. Used directly for both rendering and editing.
- **[AccessKit](https://accesskit.dev)** — accessibility tree + cross-platform AT bridge. We build trees and push `TreeUpdate`s ourselves with our own decomposed components.
- **[bevy_picking](https://docs.rs/bevy_picking)** — hit-testing primitive. We feed our hierarchies into it.
- **Bevy's render graph + wgpu** — our render passes live in Bevy's render graph. Custom shaders for clipping, gradients, borders, filters, blend modes, top layer.
- **Bevy's ECS, observers, change detection, asset system, input, windowing.** Used throughout. Buiy is a Bevy plugin, not a separate framework.

## 2.3 What Buiy owns

- **Component model** — Buiy components (`buiy::Node`, `buiy::Style`, `buiy::Theme`, focus components, a11y components, animation components). Designed BSN-friendly (small, public-fielded, observable, decomposed). Not derived from `bevy_ui::Node`.
- **Render pipeline** — custom Bevy render passes that walk Buiy hierarchies. Full control over rounded clipping, `clip-path` shapes, mask-image, backdrop-filter, mix-blend-mode, isolation/groups, true top-layer compositing, gradients in any color space, border-image, drop-shadow.
- **Layout integration** — drives Taffy ourselves; extends layout (anchor positioning, container queries) without waiting for upstream.
- **Text pipeline** — cosmic-text → glyph atlas → render pass, owned end-to-end. No per-span fonts, no inheritance leaks, no atlas leaks.
- **Focus model** — focus tree, `:focus-visible` semantics, focus rings, focus traps, focus restoration, inert subtrees, roving tabindex, `aria-activedescendant`, sequential-focus-navigation-starting-point, spatial gamepad navigation.
- **A11y integration** — Buiy → AccessKit directly. Decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` components drive `TreeUpdate`s. ACCNAME 1.2 name computation lives in `buiy_core`.
- **Theme system** — token assets, hot-reloadable, OS-pref-driven variant binding.
- **Animation primitives** — property transitions, keyframe timelines, layout transitions, springs, all reduced-motion-gated.
- **Live regions / global announcer** — Buiy resource that renders polite/assertive announcements through AccessKit.
- **Form & validation system** — form state machine, constraint validation, validation pseudo-classes.
- **Devtools** — inspector, layout overlay, AccessKit tree viewer, contrast linter, focus-order visualizer.
- **3D-anchored / diegetic UI** — first-class achievable; Buiy nodes can live in 3D space against `Transform`. Stays its own subsystem spec.
- **Verification harness** — test infrastructure for visual regression, AccessKit tree snapshots, synthesized input replay, APG conformance, WCAG SC verification.

## 2.4 Authoring: ECS-native and BSN, both first-class

- **ECS spawn:** `commands.spawn((buiy::Button, OnPress(submit), children![buiy::Text::new("Save")]))`. Always works.
- **BSN** (Bevy 0.18+): `bsn! { Button [ Text("Save") ] }` or hot-reloadable `.bsn` files.

The BSN-friendliness constraint on every Buiy component is **not optional**:

- Small, public-fielded, observable, decomposed by concern. No megacomponents, no private setters.
- Every component derives `Reflect + FromReflect + Default + Clone + Component`.
- Every component is type-registered via `app.register_type::<T>()` in the owning crate's plugin so `.bsn` asset loading can resolve it.

These constraints follow from BSN's reflection-driven asset format (PR #20158) and from the lesson of bevy issue #17644 (megacomponents are BSN-hostile).

## 2.5 Theming: token-based design system

- Themes are **assets**, hot-reloadable.
- Components consume **semantic tokens** (`color.surface.primary`, `space.4`, `radius.md`, `motion.fast`), never raw values.
- A theme defines a **palette + scales + variant**. Variants: `light`, `dark`, `high-contrast`, plus user-defined.
- OS preferences (`prefers-color-scheme`, `prefers-contrast`, `forced-colors`, `prefers-reduced-motion`, `prefers-reduced-transparency`, `inverted-colors`) surface as a `UserPreferences` resource bound to theme variants automatically.
- Default theme passes WCAG 2.2 AA contrast (4.5:1 / 3:1 / 3:1) by construction. Contrast linter validates custom themes at load and in CI.
- A subtree can override its theme by carrying a `Theme` component.
- **CSS-flavored stylesheet is not in this spec.** Future sub-spec if needed.

## 2.6 Accessibility: AccessKit-first

- The AccessKit tree is the source of truth for accessibility. Built lazily (gated on `AccessibilityRequested`), pushed as `TreeUpdate` diffs.
- Stable `NodeId`s derived from Bevy `Entity`.
- ACCNAME 1.2 name computation lives in `buiy_core`.
- Each widget's APG keyboard contract is part of the widget's contract.

**Adapter ownership.** AccessKit allows exactly one tree per `accesskit_winit::Adapter` per window. Buiy owns the adapter handle on any window where Buiy is present, keyed by winit `WindowId` (not Bevy `Entity`). Buiy does *not* layer over `bevy_a11y` — it replaces `bevy_a11y` for windows it owns. `ActionRequest` events from the adapter are routed to Buiy entities via Buiy's own action plumbing, not bevy_a11y's. See [cross-cutting.md § 3.18](cross-cutting.md) for coexistence rules with bevy_ui.

## 2.7 Reactivity

Observers + change detection only. No signal/computed/effect layer in this spec.

## 2.8 Module organization

Buiy ships as a workspace of focused crates. The principle is **modular subsystems with clean boundaries, opt-in surface area**. The exact partition below is an indicative starting point; final partition is an open question (see [README.md § 5](README.md#5-open-questions)):

- `buiy` — meta-crate, re-exports common API, ships top-level `BuiyPlugin`.
- `buiy_core` — components, render pipeline, layout integration, focus model, theme tokens, a11y primitives, plugin scaffolding (may split further per the open question).
- `buiy_text` — rich text + IME-correct text editing on cosmic-text.
- `buiy_widgets` — APG widget catalog.
- `buiy_animation` — transitions, keyframes, layout transitions, reduced-motion gating.
- `buiy_forms` — form state machine, validation, constraint pseudo-classes.
- `buiy_devtools` — inspector, contrast linter, focus visualizer, AccessKit tree viewer.
- `buiy_3d` — 3D-anchored / diegetic UI.
- `buiy_bsn` — BSN authoring helpers when on Bevy 0.18+.
- `buiy_verify` — verification harness; consumed as `dev-dependency` by every other crate; usable by downstream Buiy users.

**`BuiyPlugin` sub-plugin order.** The top-level plugin adds sub-plugins in this order so dependents see their dependencies on construction: `core` → `theme` → `a11y` → `focus` → `input` → `text` → `widgets` → `animation` → `forms` → `devtools`. Render registration happens in `Plugin::finish` (after `RenderApp` exists).

**System-set partitioning.** Per-frame Buiy work is partitioned into named `SystemSet`s, ordered:

```
BuiySet::Layout → BuiySet::Style → BuiySet::Input → BuiySet::Animate
              → BuiySet::Picking → BuiySet::A11yUpdate → BuiySet::Render
```

Sub-specs hang their systems off these labels. UI animations advance in the `Update` schedule against `Time<Virtual>` (not `FixedUpdate`, which is for game logic). Render-app data is extracted via Bevy's standard `ExtractSchedule` from the main world after `BuiySet::Render` completes.

## 2.9 Compatibility & policy

- **Rolling latest-stable Bevy.** Bevy minor releases drive migration events for underlying primitives. wgpu is a version-pinned dependency of Bevy (Bevy re-exports many wgpu types but the wgpu crate is owned upstream); we follow Bevy's pin. AccessKit releases on its own cadence and is **the open question** of [README.md § 5](README.md#5-open-questions): the policy proposed here is "AccessKit major release between Bevy minors triggers a Buiy patch release with a documented migration note," but this is not yet committed. No back-compat across Bevy minors.
- **MSRV** tracks Bevy's MSRV.
- **`std` only.** AccessKit requires it.
- **Platform support — staged.** Desktop (Windows / macOS / Linux) is committed for v1 with full CI coverage. Android (TalkBack), iOS (UIAccessibility — currently in-progress upstream in AccessKit), and web (AccessKit web adapter — not yet shipped) are deferred until each platform's AccessKit adapter exposes a headless harness usable in CI; until then they live as manual-release-gate platforms.
- **Render passes & picking** — Buiy registers its own render-graph node and its own `bevy_picking` backend. Render-graph node ordering and picking-backend priority versus bevy_ui's own passes / backend are defined per-window (see [cross-cutting.md § 3.18](cross-cutting.md)); Buiy's own passes do not contractually cooperate with bevy_ui's.
- **Coexistence with bevy_ui** — see [cross-cutting.md § 3.18](cross-cutting.md). Coexistence is **per-window**, not per-app-shared-window.
