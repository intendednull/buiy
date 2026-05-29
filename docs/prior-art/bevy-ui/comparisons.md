**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — head-to-head comparisons with parallel and complementary UI stacks

# Comparisons

How bevy_ui compares with the third-party Bevy UI stacks and with Buiy itself. Each section is short by design — the goal is to surface the **one or two design bets** each system diverges on. Deeper coverage of each system belongs in its own prior-art folder; see [ecosystem.md](ecosystem.md) for the queue.

For naming convention: the **design bet** is the single architectural choice that distinguishes the system from bevy_ui. The **cost** is what the bet pays for it.

## vs bevy_lunex

**Design bet:** transform-based positioning. UI nodes use Bevy's standard `Transform` hierarchy, so the same `Transform` propagation that places 3D objects also places UI elements. Works for both 2D and 3D UI.

**Cost:** doesn't compose naturally with Taffy's flexbox/grid layout — lunex either reimplements layout or sits awkwardly alongside it. Less aligned with web-platform UI semantics (where layout is computed from content + box-model rules, not from a transform tree).

**Verdict:** lunex is the closest neighbor to *Buiy's `buiy_3d` subsystem*. Buiy's choice is to keep Taffy as the primary layout primitive and add 3D-anchored UI as a separate, opt-in subsystem (foundation architecture.md § 2.3, 2.8). Best for: games that want UI sprinkled through 3D scenes. Worst for: app-like UI with complex flex/grid layout.

## vs sickle_ui

**Design bet:** **extends** bevy_ui rather than replacing it. Provides a fluent builder API for constructing widgets and a data-driven "skin" system on top of bevy_ui's primitives.

**Cost:** inherits every bevy_ui limitation (rect-only clipping, no backdrop-filter, no isolation, megacomponent a11y) because it sits *on* bevy_ui. You get sickle_ui's ergonomics but bevy_ui's ceiling.

**Verdict:** sickle_ui is the right choice if bevy_ui's renderer + a11y meet your needs and you only want better author ergonomics. Buiy's foundation README § 1.4 explicitly rules this out for Buiy's target (web-platform feature parity), because the underlying limits don't move.

## vs woodpecker_ui

**Design bet:** **replace** bevy_ui with a reactive UI framework using **Vello** for rendering (not Bevy's render-graph node). ECS-first; declarative reactive API. Successor to @StarArawn's earlier `kayak_ui`.

**Cost:** Vello is a separate dependency with its own GPU upload path, not the Bevy render-graph. Composing Vello-rendered UI with Bevy 3D scenes is harder (you cross a rendering boundary). Reactive frameworks introduce a separate mental model from Bevy's ECS-and-observers.

**Verdict:** Woodpecker_ui is the most-aspirational third-party Bevy UI — it bet on Vello before Bevy itself moved to GPU-vector rendering. Buiy keeps Bevy's render-graph (foundation architecture.md § 2.2) because the goal is *to extend Bevy's renderer with custom passes*, not to replace it. Different tradeoffs.

## vs kayak_ui (archived in practice)

**Design bet:** declarative UI with a custom proc macro (`rsx!`-flavored) and a CSS-like style system, on top of bevy_ui.

**Cost:** documented by @StarArawn's own pivot to woodpecker_ui — "overly complicated internals, making it difficult to contribute to and causing fundamental bugs." The proc macro grew faster than the underlying support.

**Verdict:** the cautionary tale for declarative-API-first design. Buiy's BSN-friendly stance assumes BSN provides the declarative authoring layer; Buiy components do not ship their own proc-macro DSL (foundation architecture.md § 2.4). kayak_ui's mistake was to invent a parallel authoring layer before the underlying component model was stable.

## vs bevy_egui

**Design bet:** **immediate-mode**. Wraps `egui` (the popular immediate-mode Rust GUI) as a Bevy plugin. UI is redrawn from scratch every frame; no retained tree.

**Cost:** different paradigm — immediate-mode is best for tools, debug overlays, and editors; retained-mode is best for game UIs, complex layouts, and accessibility (which expects a tree to walk). egui's a11y story is "in progress" but not at AccessKit-tree quality. egui's text shaping does not match cosmic-text.

**Verdict:** bevy_egui is the right tool for dev tooling and debug overlays. Buiy is retained-mode by foundation (foundation architecture.md § 2.4 "ECS spawn ... always works"; BSN-friendly retained components). The two **coexist peacefully** — many Bevy projects use both today (bevy_ui for game UI, bevy_egui for inspector). Buiy is expected to coexist with bevy_egui the same way bevy_ui does.

## vs Buiy (this project)

**Design bets that diverge from bevy_ui:**

1. **Parallel stack, not a layer.** Buiy integrates Taffy / cosmic-text / AccessKit / bevy_picking / wgpu directly, with its own component model + render pipeline (foundation architecture.md § 2.2-2.3). bevy_ui consumes these via `bevy_text` / `bevy_a11y` / `bevy_picking` wrappers; Buiy goes one layer down.
2. **Web-platform parity is the floor.** Buiy targets the full HTML / CSS / ARIA / WCAG 2.2 AA feature catalog (foundation README § 1.1). bevy_ui targets "what a game UI needs"; web parity is not a stated bevy_ui goal.
3. **AccessKit-first.** Buiy owns the AccessKit adapter per-window and pushes `TreeUpdate`s directly from its decomposed a11y components (architecture.md § 2.6). bevy_ui talks to AccessKit through `bevy_a11y` and the megacomponent `AccessibilityNode` (issue #17644 highlighted this design).
4. **BSN-native by construction.** Every Buiy component is small, public-fielded, decomposed (foundation architecture.md § 2.4, README goal 3). bevy_ui's `AccessibilityNode` was the cautionary tale that defines this rule.
5. **Token-based theme system with WCAG floor.** Buiy ships hot-reloadable theme assets + semantic tokens + OS-preference binding + a contrast linter (architecture.md § 2.5). bevy_ui has no built-in token system; bevy_feathers ships its own opinionated theme but it's coupled to the editor look-and-feel.
6. **Full renderer ownership.** Buiy implements its own render passes for non-rect clipping, backdrop-filter, mix-blend-mode, isolation, true top layer, gradients in any color space, border-image, drop-shadow. bevy_ui's renderer has none of these (see [critiques.md](critiques.md)).
7. **Verifiable.** Buiy ships a verification harness with CI gates for AccessKit-tree snapshots, visual regression, APG conformance, WCAG SC tests (foundation README goal 7). bevy_ui does not have an equivalent harness.

**Cost of Buiy's choices:**

- **Scope.** A parallel UI stack with web-platform parity is huge; foundation README acknowledges this is a multi-year project and breaks the work into ~19 sub-specs (foundation README § 4).
- **No same-window coexistence with bevy_ui.** Buiy can run in the same `App` as bevy_ui only on different windows (foundation cross-cutting.md § 3.18).
- **Tracks Bevy minor releases.** Each Bevy minor is a migration event; Buiy commits to rolling latest-stable, no multi-version compat (foundation README § 1.5).

**Verdict:** Buiy is the *web-platform-parity + WCAG-AA + AccessKit-first* design bet. The other Bevy UI stacks make different bets (transform-first, immediate-mode, declarative-DSL, layered-on-top). Each is defensible for its target audience; Buiy's defensibility rests on "if you want web-quality apps and games on Bevy, this is what it takes."

## Cross-reference matrix

| Stack | Layout | Text | a11y | Authoring | Coexists w/ bevy_ui? |
|---|---|---|---|---|---|
| bevy_ui | Taffy | cosmic-text via bevy_text | bevy_a11y + AccessKit | ECS direct + bevy_ui_widgets/Feathers; BSN planned | self |
| bevy_lunex | own (transform-based) | bevy_text | none built-in | ECS direct | yes (different paradigm) |
| sickle_ui | bevy_ui's Taffy | bevy_ui's text | bevy_a11y | builder DSL on bevy_ui | uses bevy_ui |
| woodpecker_ui | own | own + Vello | early | reactive proc-macro DSL | parallel |
| kayak_ui | own | own | none | declarative DSL | parallel (archived) |
| bevy_egui | egui | egui (no cosmic-text) | egui's (limited) | immediate-mode | yes |
| bevy_flair | bevy_ui's Taffy | bevy_ui's text | bevy_a11y | CSS stylesheet on bevy_ui | uses bevy_ui |
| **Buiy** | **Taffy direct** | **cosmic-text direct** | **AccessKit direct** | **ECS + BSN; decomposed components** | **per-window only** |

## Sources

- bevy_lunex — `https://github.com/bytestring-net/bevy_lunex`.
- sickle_ui — `https://github.com/UmbraLuminosa/sickle_ui`.
- woodpecker_ui — `https://github.com/StarArawn/woodpecker_ui`.
- kayak_ui — `https://github.com/StarArawn/kayak_ui`.
- bevy_egui — `https://github.com/vladbat00/bevy_egui`.
- bevy_flair — `https://github.com/eckz/bevy_flair`.
- bevy_cosmic_edit — `https://docs.rs/bevy_cosmic_edit/`.
- "A Vision for Bevy UI" — `https://hackmd.io/@bevy/HkjcMkJFC`.
- Buiy foundation spec — `../../specs/2026-05-07-buiy-foundation/README.md`.
- Buiy architecture — `../../specs/2026-05-07-buiy-foundation/architecture.md`.
- Buiy cross-cutting — `../../specs/2026-05-07-buiy-foundation/cross-cutting.md`.
