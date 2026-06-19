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
- **BSN** (Bevy 0.19+): `bsn! { Button [ Text("Save") ] }`. The BSN baseline (`bsn!` / Templates) landed in Bevy 0.19 via PR #23413 (`bevy_scene` crate); Buiy authors inline `bsn!` today. The hot-reloadable `.bsn` **asset-file** form is deferred upstream (its loader was not part of #23413) — see [`2026-06-18-buiy-bsn-integration-design.md`](../2026-06-18-buiy-bsn-integration-design.md) §§ 1, 4.4.

The BSN-friendliness constraint on every Buiy component is **not optional**:

- Small, public-fielded, observable, decomposed by concern. No megacomponents, no private setters.
- Every component derives `Reflect + FromReflect + Default + Clone + Component`. Note: inline `bsn!` itself is **compile-time and reflection-free** — its plain-data template path needs only `Clone + Default` (a `bsn!` patch layers onto a component's `Default` base). The `Reflect + FromReflect` derives are retained for the *future* reflection-driven consumers — the deferred `.bsn` asset-file loader and the editor/inspector — not for inline `bsn!`.
- Every component is type-registered via `app.register_type::<T>()` in the owning crate's plugin. Likewise, the type registry is consumed by the future `.bsn` loader / inspector, **not** by inline `bsn!` (which resolves component types at compile time); registration is kept so those consumers can resolve every component when they land.

These constraints follow from the lesson of bevy issue #17644 (megacomponents are BSN-hostile) and keep every component authorable in BSN: inline `bsn!` (landed in Bevy 0.19, PR #23413 in `bevy_scene`) via the `Clone + Default` template contract today, and the reflection-driven `.bsn` asset format (deferred upstream — original draft PR #20158 framing) when its loader lands. See [`2026-06-18-buiy-bsn-integration-design.md`](../2026-06-18-buiy-bsn-integration-design.md) §§ 4, 6.

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
- `buiy_bsn` — BSN authoring helpers (Bevy 0.19+): re-exports `bsn!` / `bsn_list!` + spawn ext traits into a `buiy_bsn::prelude`; reached via `buiy::bsn` and folded into `buiy::prelude`.
- `buiy_verify` — verification harness; consumed as `dev-dependency` by every other crate; usable by downstream Buiy users.

**`BuiyPlugin` sub-plugin order (long-term target).** The top-level plugin adds sub-plugins in this order so dependents see their dependencies on construction: `core` → `theme` → `a11y` → `focus` → `input` → `text` → `widgets` → `animation` → `forms` → `devtools`. This is the target shape once every subsystem has a plugin.

**As built today (Phase 0).** The realized order is `core` → `theme` → `a11y` (the `A11yPlugin` plus its `AccessKitAdapter`) → `focus` → `layout` → `picking` → `text` → `widgets` → `render`. Notes on the divergence: `input` is not a sub-plugin — it is the `BuiySet::Input` system set (see below), not a separate plugin; `layout` and `picking` are present as their own sub-plugins; and `animation` / `forms` / `devtools` are not yet present (those subsystems are unbuilt). Render is registered in the render sub-plugin's `build()` (not deferred to `Plugin::finish`).

**System-set partitioning.** Per-frame Buiy work is partitioned into named `SystemSet`s, ordered:

```
BuiySet::Layout → BuiySet::Style → BuiySet::Input → BuiySet::Animate
              → BuiySet::Picking → BuiySet::A11yUpdate → BuiySet::Render
```

Sub-specs hang their systems off these labels. UI animations advance in the `Update` schedule against `Time<Virtual>` (not `FixedUpdate`, which is for game logic). Render-app data is extracted via Bevy's standard `ExtractSchedule` from the main world after `BuiySet::Render` completes.

## 2.9 Compatibility & policy

- **Rolling latest-stable Bevy.** Bevy minor releases drive migration events for underlying primitives. wgpu is a version-pinned dependency of Bevy (Bevy re-exports many wgpu types but the wgpu crate is owned upstream); we follow Bevy's pin. AccessKit releases on its own cadence and is **the open question** of [README.md § 5](README.md#5-open-questions): the policy proposed here is "AccessKit major release between Bevy minors triggers a Buiy patch release with a documented migration note," but this is not yet committed. No back-compat across Bevy minors.
  - **Active exception (2026-06-18): pinned to `0.19.0-rc.3`.** Buiy currently pins a Bevy **release candidate**, a deliberate, scoped exception to "rolling latest-stable," taken because BSN authoring (goal 3) is unreachable on any stable Bevy and the user chose to build real `bsn!` now (the BSN baseline ships only in the 0.19 line — PR #23413 — and 0.19 has no stable tag yet). The exception is bounded: when 0.19.0 stable releases, Buiy bumps to it and the exception closes (a likely small rc.3→stable follow-up). Owned by [`2026-06-18-buiy-bsn-integration-design.md § 2`](../2026-06-18-buiy-bsn-integration-design.md#2-decision-pin-bevy-0190-rc3-policy-exception); tracked in [`follow-ups.md`](../../plans/follow-ups.md).
- **MSRV** tracks Bevy's MSRV.
- **`std` only.** AccessKit requires it.
- **Platform support — staged.** Desktop (Windows / macOS / Linux) is committed for v1 with full CI coverage. Android (TalkBack), iOS (UIAccessibility — `accesskit_ios 0.1.0` shipped 2026-05-11), and web (AccessKit web adapter — not yet shipped) are deferred until each platform's AccessKit adapter exposes a headless harness usable in CI; until then they live as manual-release-gate platforms. (The iOS adapter shipping does not change the deferred-platform CI posture — the blocker is a CI-usable headless harness, not adapter availability.)
- **Render passes & picking** — Buiy registers its own render-graph node and its own `bevy_picking` backend. Render-graph node ordering and picking-backend priority versus bevy_ui's own passes / backend are defined per-window (see [cross-cutting.md § 3.18](cross-cutting.md)); Buiy's own passes do not contractually cooperate with bevy_ui's.
- **Coexistence with bevy_ui** — see [cross-cutting.md § 3.18](cross-cutting.md). Coexistence is **per-window**, not per-app-shared-window.
