**Date:** 2026-05-22
**Status:** active
**Subject:** woodpecker_ui — Validates / Avoid / Borrow decisions for Buiy

# Lessons for Buiy

This is the consult-this-when-designing decision file. Other files (`architecture.md`, `api.md`, `critiques.md`, `history.md`) are evidence; this file is the synthesis. Three sections — **Validates**, **Avoid**, **Borrow** — plus a fourth section on the **kayak → woodpecker transition lessons** that frames the small-author-third-party trade-offs.

## Top of file: two findings reframe how to read this corpus

### 1. The kayak_ui lineage is verified — and the abandonment-curve is the load-bearing signal.

The woodpecker_ui README's Q3 (verbatim) confirms StarArawn rewrote kayak_ui as woodpecker_ui because *"Kayak UI suffered from overly complicated internals."* Both crates have the same solo author. kayak_ui (2022–2024, 18,774 lifetime DLs) is now release-silent; woodpecker_ui (2024–2025, 1,077 DLs) has been release-silent for ~12 months. The empirical pattern is: **solo-author Bevy UI crates have a ~15-month-active-development half-life, then go dormant.** Buiy's foundation work needs to internalize this base rate when reasoning about ecosystem dependencies and when staffing its own maintenance plan.

See [`history.md`](history.md) § "Pattern: the second-system trap?" and [`distribution.md`](distribution.md) § "Bus factor."

### 2. woodpecker_ui's substrate choices triangulate close to Buiy's, on a different sub-stack.

woodpecker_ui uses: vello (via `bevy_vello`), Parley, Taffy, `bevy_picking`. Buiy's foundation commits to: own render pipeline (vello is a feasibility witness, not a direct dependency), **cosmic-text** (not Parley), Taffy, `bevy_picking`. The overlap on Taffy + `bevy_picking` is total; the renderer is parallel (vello-capable in both); the text shaper is the only meaningful divergence — and Bevy `main` is itself moving to Parley in 0.19, so the Buiy cosmic-text commitment is increasingly distinctive (see [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Top-of-file finding #2).

The implication: when Buiy needs to learn from a Bevy UI that runs a vello-flavored render path, woodpecker_ui is the only published reference. Use it as such — not as a maintenance commitment.

---

## Validates

Buiy decisions confirmed by woodpecker_ui's experience:

- **Parallel-to-`bevy_ui` is feasible.** woodpecker_ui consumes Bevy with `default-features = false` + `bevy_picking` + `bevy_log` (no `bevy_ui` feature). The crate functions, ships examples, runs on native + WASM. This is a third-party data point in addition to `bevy_lunex` confirming the parallel-stack feasibility (foundation README goal 4). See [`architecture.md`](architecture.md) § "The layer cake."

- **`bevy_picking` as the picking-backend registration point.** woodpecker_ui registers its own backend with `bevy_picking` (the same pattern bevy_ui uses; see [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Borrow #5). This validates the Buiy `buiy-input-events-design` commitment to `bevy_picking` backend registration as the cross-stack pattern.

- **Taffy as the layout substrate.** woodpecker_ui pins `taffy = "0.7"` with `flexbox` + `grid` features and integrates Taffy directly (not via `bevy_ui`). The integration pattern (custom measure functions for leaf widgets, mirror tree, separate `WidgetLayout` / `WidgetPreviousLayout` for input/output) is structurally identical to bevy_ui's. Foundation architecture § 2.2's Taffy commitment is reaffirmed.

- **`Required Components` for widget companions.** woodpecker_ui uses `#[require(WoodpeckerStyle, WidgetChildren)]` on widget components — same Bevy 0.15+ mechanism as bevy_ui's `Node` requires `ComputedNode`, etc. This validates [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Borrow #1 ("`RequiredComponents` mechanism") as a stable third-party pattern.

- **ECS-first authoring is the third-party direction of travel.** The README Q2 frames non-ECS UI crates (egui, iced, dioxus-as-renderer) as having a data-ownership problem (*"They tend to want ownership of the data which means it must live outside of bevy's ECS world. I have problems with this."*) — this matches Buiy's foundation goal 3 (BSN-native) which also keeps state in ECS. Both data points (woodpecker_ui from third-party, BSN from in-tree) suggest the Bevy ecosystem is converging on ECS-as-source-of-truth.

- **WASM as a first-class target.** woodpecker_ui's `Cargo.toml` has a dedicated WASM dependency block, and the README documents the `wasm-server-runner` workflow. This validates that Buiy's foundation README § 5 open question on WASM ("Bevy WASM target policy") can land in the in-scope column without architectural blocks at the UI layer — the question is mostly about a11y (AccessKit web adapter) and platform-specific input.

## Avoid

Pitfalls drawn from woodpecker_ui, with Buiy mitigation.

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **Megacomponent styles** — `WoodpeckerStyle` bundles ~50 fields (layout + box-decoration + text + visibility) into one struct. BSN templates cannot patch individual fields without overwriting the whole struct. Same anti-pattern as `bevy_a11y::AccessibilityNode` ([`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Avoid-row "Megacomponents that are BSN-hostile"). | [`api.md`](api.md) § "Style component"; [`critiques.md`](critiques.md) § "Megacomponent style." | Buiy commits to decomposed components: `BackgroundColor`, `BorderColor`, `BorderRadius`, `Outline`, gradients, shadows, and layout fields each on their own component. Foundation architecture § 2.3, 2.4. |
| **No AccessKit integration, no accessibility story** — woodpecker_ui has zero a11y plumbing. Screen readers cannot navigate it. WCAG 2.2 is not addressable. | [`critiques.md`](critiques.md) § "No accessibility integration." | Buiy is AccessKit-first by foundation commitment (goal 2). AccessKit adapter ownership per window, decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` from day one. Foundation architecture § 2.6. |
| **Minimal focus model** — single `CurrentFocus` resource + `WidgetFocus` / `WidgetBlur` events. No `:focus-visible`, no traps, no inert subtrees, no roving tabindex, no spatial nav. | [`architecture.md`](architecture.md) § "Picking and focus"; [`critiques.md`](critiques.md) § "No focus model worth speaking of." | Buiy ships a unified focus tree per `buiy-focus-model-design`: `:focus-visible`, traps, restoration, inert, roving tabindex, `aria-activedescendant`, sequential-focus-navigation-starting-point, gamepad spatial nav. Foundation architecture § 2.3. |
| **No theme / token system** — per-widget `*Styles` structs without semantic-token indirection, no light/dark variants, no OS-preference binding, no forced-colors fallback. | [`critiques.md`](critiques.md) § "No theme / token system." | Buiy's `buiy-theme-tokens-design`: semantic tokens, theme assets, variants, OS-pref binding, contrast linter. Foundation README § 4. |
| **Locked-Bevy-version drift** — pinned at Bevy 0.16; Bevy is on 0.18.1 stable as of 2026-05-22. Two minor versions of unmigrated breaking changes. | [`integration.md`](integration.md) § "Bevy version compatibility"; [`critiques.md`](critiques.md) § "Bevy version drift." | Buiy commits to rolling latest-stable, no multi-version compat promise (foundation goal 5). Each Bevy minor release is a Buiy migration event, planned via `docs/plans/`. |
| **Solo-maintainer abandonment lifecycle** — kayak_ui (2022-11 → 2024-02 active, then silent) and woodpecker_ui (2024-07 → 2025-06 active, then silent) demonstrate ~15-month solo-author half-life for Bevy UI crates. | [`history.md`](history.md) § "Pattern: the second-system trap?"; [`distribution.md`](distribution.md) § "Governance." | Buiy needs an explicit maintenance plan, co-maintainer pipeline, and CI-pinned versions before declaring v1. The verification harness (foundation verification.md) is the substitute for "trust the maintainer" — even if Buiy goes solo at some point, the test suite anchors the contract. |
| **Game-UI starter set ≠ APG coverage** — ~12 widgets vs Buiy's ~60-pattern target. No Listbox/Combobox/Menu/Tooltip/Disclosure/Progressbar/live regions/Tree/Date picker/etc. | [`api.md`](api.md) § "Widget vocabulary"; [`critiques.md`](critiques.md) § "Coverage gaps." | Buiy's widget catalog (`buiy-widget-catalog-design`) enumerates every APG pattern with tier F/C/E/O. Foundation media-and-widgets § 3.10. |
| **No published APG keyboard contracts** — even shipped widgets (`Toggle`, `Slider`, `Checkbox`, `Dropdown`) don't document or implement APG key bindings (arrow keys, Home/End, Escape, etc.). | [`critiques.md`](critiques.md) § "Coverage gaps vs Buiy widget catalog." | Buiy's foundation media-and-widgets § 3.10 makes APG keyboard contracts a default-shipped property of every widget, covered by verification gate 7. |
| **`bevy-trait-query` runtime dispatch at scale unmeasured** — every widget invocation goes through a virtual call per frame. No benchmark at 1000+ nodes. | [`critiques.md`](critiques.md) § "`bevy-trait-query` dispatch cost." | Buiy's verification harness commits to 1000+ node fixtures. Widget dispatch should use Bevy's standard system scheduling, not a polymorphic-trait-object layer. Foundation verification.md gate #14 (perf budgets). |
| **No coexistence test with `bevy_ui`** — both stacks register `bevy_picking` backends; no example exercises side-by-side operation. | [`integration.md`](integration.md) § "Coexistence with bevy_ui." | Buiy's `buiy-coexistence-design` (foundation README § 4) commits to defining the AccessKit-adapter coordinator, render-pass ordering, and picking-backend priority *before* claiming same-window coexistence. |

## Borrow

Concrete primitives worth studying and adapting from woodpecker_ui:

1. **vello as a Bevy UI render substrate.** `bevy_vello` 0.9 provides the integration point. Vello's capability set covers what `bevy_ui` lacks: rounded clip, `clip-path` shapes, gradients (linear/radial/conic), drop-shadow, blur — all of which are first-class vello scene primitives. Buiy's render pipeline doesn't depend on `bevy_vello`, but reading woodpecker_ui's vello scene emission code (`src/vello_renderer.rs`) is the closest published reference for "how do I emit non-trivial path-rendering on top of Bevy's render graph?" See [`architecture.md`](architecture.md) § "Render pipeline."

2. **dioxus-devtools-based hot reload.** woodpecker_ui's `hotreload` Cargo feature wires `dioxus-devtools` and a `#[hot]` proc-macro into the widget render systems. Once the user runs `dx serve --hotpatch`, render-function bodies hot-patch on save. This is the *only* published Bevy UI demonstration of dioxus-style hot-patch. Worth studying for `buiy-bsn-integration-design`'s hot-reload-of-components question (foundation README § 5). See [`integration.md`](integration.md) § "Hot reload."

3. **`WidgetChildren` fluent-builder + `apply(parent)` reconciliation.** A typed-vector-of-bundles authoring style with explicit reconciliation against the actual entity hierarchy. Buiy's BSN-friendly authoring won't use this exact API — BSN templates are scene-based, not in-code-builder — but `WidgetChildren::apply` is a clean reference for how to *reconcile* a desired-children-spec against the actual entity tree. The Mount lifecycle marker (`Mounted` component, fires once on first insertion) is reusable shape. See [`api.md`](api.md) § "Composition."

4. **`HookHelper::use_state` + `PreviousWidget` mapping.** React-style hooks keyed off the current widget entity, with `PreviousWidget` tracking the mapping across re-renders so state survives despawn / respawn cycles. For Buiy's open question on "reactivity layer" (foundation README § 5), this is one of the cleanest published implementations of a hooks-style reactive state primitive *on top of Bevy ECS*. Study before designing the signal/computed/effect follow-up sub-spec. See [`api.md`](api.md) § "Hooks."

5. **Per-widget `update() -> bool` change-bit pattern.** Each widget exposes a `fn update() -> bool` that returns true if anything tracked has changed; the runner only calls `render()` if `update()` returned true. This is a *simpler* alternative to Bevy's observer + change-detection mix that Buiy commits to (foundation architecture § 2.5). For Buiy, the relevant takeaway is that the dirty-bit pattern can be modeled as syntactic sugar over change detection — useful framing for the user-facing API even when the underlying machinery is observers + `Changed<T>`.

6. **`#[derive(Widget)]` proc-macro with `#[auto_update]` / `#[props]` / `#[state]` / `#[context]` / `#[resource]` attribute set.** A compact, learnable attribute vocabulary for declarative-component reactivity. Most useful as **the negative example** for a Buiy-future macro: every attribute names exactly one role of state, and the macro keeps the user's hand out of the diff bookkeeping. If Buiy ever adds a reactive-widget macro, this is the API surface to study and either improve or explicitly reject. See `crates/woodpecker_ui_macros/src/lib.rs`.

7. **`WidgetRender` leaf enum.** A small enum-of-leaf-content kinds (`Text`, `Image`, `Svg`, `Quad`, `Custom`) attached to leaf widgets — separate from the `WoodpeckerStyle` decoration component. This isn't decomposed enough by Buiy's standards (text-rendering-config and image-source live in the same enum), but the *idea* of a `WidgetRender` content slot separate from the styling component is sound. Buiy's `Element`-like generic container + content-slot pattern can borrow the shape.

8. **`Element` as the generic-container widget.** A single user-facing widget representing the equivalent of an HTML `<div>`. Useful because it tells you: most Buiy authoring will not involve a strongly-typed `Button` / `Image` / etc. — it'll be `Element` with `WidgetRender` content and `WoodpeckerStyle` decoration. Buiy can ship a single canonical `Element` + per-pattern semantic widgets that extend it. Foundation media-and-widgets § 3.10 already implies this via "Group / Section / Article / Region (semantic containers)."

## kayak_ui → woodpecker_ui transition lessons

Specific lessons from the predecessor rewrite, useful for Buiy's foundation thinking:

1. **The 200-line runner claim.** README Q3: *"the primary system that runs the UI was over 1k lines in Kayak and in Woodpecker its less than 200."* The compactness is real; `src/runner.rs` is short. **Takeaway:** complexity in the reactive runtime is a self-inflicted wound, not a feature requirement. Buiy should keep its reactive scheduler small and lean on Bevy's `Schedule` + observers + change detection rather than building a parallel scheduler.

2. **Custom MSDF text → Parley.** kayak_ui shipped its own MSDF font renderer; woodpecker_ui delegated to Parley. **Takeaway:** owning a text shaper is a maintenance commitment with deep complexity (BiDi, IME, complex script, kerning); use the published substrate (parley or cosmic-text). Buiy's foundation architecture § 2.2 commits to cosmic-text directly, which is the same architectural call.

3. **morphorm → Taffy.** kayak_ui's layout engine (morphorm) is now a niche maintenance burden; Taffy is the de-facto Bevy-ecosystem layout. **Takeaway:** Buiy's Taffy commitment is reinforced as the right call.

4. **Custom `bevy_render` UI pass → `bevy_vello` scene compositor.** kayak_ui used a custom UI render pass; woodpecker_ui delegated to `bevy_vello`. **Takeaway:** delegating to a path-rendering substrate cuts a substantial maintenance surface. Buiy's own render pipeline owns the integration but can model individual passes on vello's capability set.

5. **The lifecycle pattern doesn't change with the rewrite.** Even with all the architectural improvements (~10x smaller runner, modern substrate, vello renderer, Parley text), woodpecker_ui's adoption is 17× *smaller* than kayak_ui's. **Takeaway:** the limiting factor for a Bevy UI third-party crate is *not* technical quality of the rewrite — it's the maintenance and ecosystem-integration commitment. Buiy needs to plan for this explicitly; "v2 will be better" is not a meaningful strategy.

## Small-adoption third-party trade-offs

Generalized framing of how to read woodpecker_ui and similar small Bevy UI crates:

- **They are architectural references, not adoption candidates.** Read them for substrate choices, API patterns, render-pipeline ideas. Don't ship a Buiy dependency on them.
- **Their abandonment is the prior, not the exception.** kayak_ui, sickle_ui, woodpecker_ui are three data points; the base rate is "solo Bevy UI crates go dormant in 12–18 months." Plan accordingly.
- **The README's "Q & A" sections are the highest-signal artifact** for understanding why the author made the choices they did. Read them verbatim; they often state opposition to upstream direction (e.g., woodpecker's anti-BSN Q4) more clearly than any external review.
- **The substrate convergence is real.** Taffy, `bevy_picking`, vello-or-Parley-or-cosmic-text, AccessKit (if present) — the substrate set is converging across third-party stacks. Buiy's substrate choices align with this convergence and should benefit from substrate-team maintenance regardless of which third-party UI crate is current.

## How to use this file

When designing a Buiy feature:

1. **Find the row in `Avoid`** that names a pitfall close to your design. Read the linked file for evidence.
2. **Find the entry in `Borrow`** that names a primitive close to what you're designing. Read the woodpecker_ui source via the links in [`architecture.md`](architecture.md) / [`api.md`](api.md), then adapt for Buiy's component model.
3. **Read the README's Q1–Q5 directly** if you want the author's framing — that is the single most informative artifact about why this crate exists.
4. **Promote any decision into a Buiy spec** — this file captures what we learn from woodpecker_ui; the consequent Buiy decisions live in `docs/specs/`.

## Sources

- This corpus's evidence files — [`README.md`](README.md), [`architecture.md`](architecture.md), [`api.md`](api.md), [`integration.md`](integration.md), [`history.md`](history.md), [`distribution.md`](distribution.md), [`critiques.md`](critiques.md), [`ecosystem.md`](ecosystem.md)
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- bevy_ui lessons for cross-reference — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- woodpecker_ui README (Q1–Q5 are the highest-signal sections) — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/README.md
