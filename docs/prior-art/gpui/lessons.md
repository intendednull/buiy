**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — lessons for Buiy: which Buiy design choices GPUI's experience validates, which pitfalls to avoid, which primitives to borrow

# Lessons for Buiy

This is the consult-this-when-designing decision file. Other files in this corpus are evidence; this file is the synthesis.

GPUI is the **strongest existing-art for "GPU-accelerated retained-mode app UI in Rust ships a production product."** Zed 1.0 on macOS, Linux, and Windows is the existence proof. But GPUI's design serves Zed first, the community a distant second, and accessibility/mobile/web not at all. The lessons separate cleanly into "validate Buiy's bets," "avoid GPUI's mistakes," and "borrow GPUI's primitives at the design level."

## Top-of-file finding: Bevy ECS provides equivalent semantics to GPUI's ownership model

GPUI's central architectural achievement is the `App` + `Entity<T>` + effect-queue ownership model — typed handles to globally-owned state with structured update flow and run-to-completion semantics. This is the result of multi-year iteration by Nathan Sobo and Antonio Scandurra.

**Bevy's ECS provides the same semantics**: `World` + `Entity` + observers + change detection + system scheduling. The match is uncanny — see [`architecture.md` § "Comparison to Bevy's ECS"](architecture.md) and [`element-tree.md` § "vs Bevy ECS UI (and Buiy)"](element-tree.md).

**The decisive consequence:** Buiy does not need to invent GPUI's ownership model. Bevy's ECS already provides it. The expensive part of GPUI's design (the ownership story) is free for Buiy. The remaining work — rendering primitives, widget catalog, accessibility integration, theming — is exactly the work Buiy's foundation commits to. **GPUI is the validation that this architecture pattern reaches production; Bevy is the substrate that lets Buiy get there without rebuilding the substrate.**

## Validates

These Buiy design choices are confirmed by GPUI's production experience:

- **Custom retained-mode GPU pipeline ships a serious productivity app.** Zed 1.0 on three platforms is the proof. Foundation §2.2-§2.3 (own render pipeline, Bevy render graph + wgpu, custom shaders for clipping/gradients/borders/filters/top-layer) is the right bet. The doubt was "does anyone ship serious UI this way in Rust?" The answer is yes, Zed does.
- **Taffy as the layout primitive.** GPUI pins `taffy = 0.10.1`; their experience with Taffy at editor-scale (with custom escape hatches for line layout) validates the choice. Foundation §2.2 commits to the same crate.
- **Layout escape hatches for high-cardinality lists.** GPUI's `UniformList` (O(1) for fixed-height items) and `List` (O(log N) via SumTree for variable heights) skip Taffy entirely. Validates that Taffy is the default, not the only path. Buiy's widget catalog (foundation §3.9, `buiy-widget-catalog-design` sub-spec) will need equivalents for virtualized tables and lists.
- **SDF-based shape evaluation in fragment shaders.** Rounded rectangles, drop shadows, anti-aliasing via signed distance functions in the fragment shader. The technique is GPU-friendly, batches well, and produces pixel-perfect anti-aliasing at any zoom. Foundation §2.3 (rounded clipping, drop-shadow primitives) can use this approach directly.
- **Batched instance draws per primitive type.** A full editor frame issues single-digit GPU draw calls despite painting thousands of elements. Validates that a small fixed primitive set with per-type batched pipelines is the right rendering shape for UI.
- **Glyph atlas with alpha-as-coverage and per-instance color.** Store glyphs as single-channel alpha; apply color at draw time. One atlas serves any tint, theme color changes don't require atlas regeneration. Same trick works for monochrome icons.
- **Effect-queue-driven reactivity over hook-driven re-render.** GPUI's run-to-completion semantics structurally prevent reentrancy bugs. Bevy's observers + change detection give Buiy the same semantics for free. Foundation §2.7 (observers + change detection only in v1) is validated as sufficient — no need to chase React-style hook reactivity.
- **Hybrid declarative + imperative escape hatch.** GPUI's `Render` (declarative) + `Element` (imperative) decomposition lets custom widget authors do anything when the declarative path can't express it. Buiy should leave the same door open — foundation §2.3 ("custom Bevy render passes that walk Buiy hierarchies") implies the same; the widget-catalog sub-spec should make the contract explicit.

## Avoid

These GPUI choices are concrete anti-patterns for Buiy:

- **Single-product dogfooding without an explicit ecosystem-friendliness commitment.** GPUI's "build for Zed, hope it generalizes" strategy produces a framework with no widgets, no theming model, no animation system, no forms — all the parts Zed didn't need. Buiy's foundation §3 catalogs every feature explicitly tagged F/C/E so the breadth is intentional, not accidental.
- **Apache-2.0-only license.** Buiy commits to dual MIT/Apache (matching Bevy, matching Rust ecosystem norms). The single-license choice creates friction for downstream MIT-or-permissive projects. Borrow GPUI's design _ideas_, but clean-room reimplement; do not vendor GPUI source.
- **Three native rendering backends instead of one.** GPUI's Metal-on-macOS + wgpu-on-Linux + DX11-on-Windows split achieves per-platform polish at 3× code maintenance cost. Buiy commits to Bevy's wgpu uniformly (foundation §2.2). The trade is "lower per-platform polish ceiling" for "consistent behavior + one code path." Accept the tradeoff; reject the temptation to split backends if wgpu falls short on one platform — fix wgpu instead.
- **Defer accessibility past v1.** GPUI's 2.5-year-and-counting accessibility debt is the canonical cautionary tale. The retrofit cost grows superlinearly with codebase age. Foundation §2.6 (AccessKit-first, decomposed a11y components, per-widget keyboard contracts) is the inverse commitment, and GPUI's experience makes it unambiguously correct.
- **Defer a widget library.** GPUI's "no widgets ship in mainline" forces every adopter to either write widgets from scratch (Zed did) or take on a third-party dependency (`longbridge/gpui-component`). Foundation §3.9 (widget catalog covering APG patterns) commits to shipping widgets as first-class. Don't replicate GPUI's "you build the widgets" gap.
- **Defer a theming model.** GPUI's "themes are user-code structs" model means no hot-reload, no contrast linter, no OS-preference binding. Foundation §2.5 (token assets, OS-pref binding, hot-reload, contrast linting) is the inverse commitment.
- **Inline-only styling.** GPUI's `Styled` trait fluent setters (`.bg(red()).rounded(px(8.0))`) is ergonomic but resists hot-reload and theme swap. Buiy's foundation §2.5 commits to semantic tokens consumed by widgets, with hot-reloadable theme assets. Inline styling can be an ergonomic shorthand; the underlying model must be token-driven.
- **Pre-1.0 indefinite churn.** GPUI's three crates.io publishes in 18 months and "breaking changes between any two states" disclaimer makes it hard for downstream adopters. Buiy commits to Bevy's release cadence as the migration heartbeat (foundation §2.9), with semver discipline on each release.
- **Monorepo crate publishes without release discipline.** GPUI's crates.io `0.2.x` trails Zed's `main` substantially with no maintenance branch and outdated examples. If Buiy publishes to crates.io, do it with release discipline (maintenance branches, version policy, working examples) — or commit to "vendor from workspace" and skip the crates.io publish.
- **Single-corporate-steward governance under VC pressure.** Zed Industries' Series B in August 2025 was followed by the February 2026 community-deprioritization announcement six months later. The pattern is predictable. Buiy avoids it structurally by being downstream of Bevy's foundation-governed ecosystem; do not introduce a single-corporate-steward dependency anywhere in Buiy's critical path.

## Borrow

These GPUI design ideas are worth studying and clean-room reimplementing in Buiy:

- **The four-stage paint pipeline (layout → prepaint → paint → GPU submit).** Borrow the decomposition. Buiy's `buiy-render-pipeline-design` sub-spec can adopt the same stage structure on Bevy's render graph. Layout via Taffy is shared. Prepaint = hit-test registration + focus path registration. Paint = scene primitive emission into per-type batches. GPU submit = Bevy's render-graph node executing.
- **The fixed primitive set: `Quad`, `Shadow`, `Glyph`, `MonochromeSprite`, `PolychromeSprite`, `Underline`, `Path`, `Surface`.** ~8 typed primitives that compose into all UI. Borrow this decomposition. Each primitive gets its own Bevy `Material2d` (or custom render-graph pipeline) with batched instances.
- **SDF-based rounded-rectangle + border + drop-shadow math.** The shader is short, well-understood, and Apache-2.0-published (Scandurra's blog post). Clean-room reimplement in WGSL for Bevy's wgpu path. Anti-aliasing is automatic via the SDF smoothstep.
- **Alpha-channel glyph atlas with per-instance color.** Apply color at draw time, not in the atlas. One atlas serves any tint. Same trick for monochrome icons. Polychrome icons get a separate full-color atlas.
- **Bounds-clipped fragment-shader clipping.** Clip rect passed as instance attribute; fragment shader discards outside the rect. Cheap, supports arbitrary nesting, integrates with the SDF math. For Buiy's foundation §2.3 commitment to richer clip-path shapes, this extends to "clip rect + mask texture" or "clip rect + SDF expression."
- **Scene-level draw-order overrides for the top layer.** Elements declare a draw layer; the scene sorter places them after everything else regardless of element-tree position. This is how Buiy implements foundation §2.3's "true top-layer compositing" — render-graph node sorts primitives by `(layer, type)` before emitting draws.
- **Specialized layout elements for high-cardinality lists.** `UniformList` (O(1) for fixed-height items) and `List` (O(log N) via SumTree). Borrow the pattern: Buiy's widget catalog needs virtualized lists/tables/trees that skip per-item Taffy invocation. The SumTree itself is in Zed's `sum_tree` crate (Apache-2.0); could be vendored or reimplemented.
- **Keymap-as-asset + typed action dispatch over context-tagged focus path.** GPUI's `keymap.json` + `key_context` + typed `Action` + bubbling dispatch is a clean model. Buiy's foundation §3.7 input-events sub-spec should adopt the same shape: hot-reloadable keymap assets bound to typed Buiy actions, dispatched via Bevy's observer system over the focus path.
- **Per-window adapter ownership (for AccessKit when Buiy wires it up).** GPUI doesn't have AccessKit, but if it did, per-`Window` adapter ownership would be the right shape. Buiy's foundation §2.6 already commits to this pattern (per-window keyed by winit `WindowId`). Validated against GPUI's window-scoped resource model.
- **The publish-from-monorepo pattern (with release discipline added).** GPUI ships from `crates/gpui/` inside the Zed monorepo. The pattern is correct (single workspace; canonical source-of-truth; tight integration). What GPUI gets wrong is the publish discipline (no maintenance branches, infrequent publishes, outdated examples). Buiy will likely want the same monorepo pattern; do it with discipline.
- **Tailwind-style fluent styling as ergonomic shorthand.** GPUI's `Styled` trait (`.bg(color).rounded(px).p_4()`) is fast to type and reads well. Buiy can offer a similar surface as **convenience over the token model**: `.bg(theme.colors.surface_primary).rounded(theme.radius.md)`. The underlying model remains tokens; the syntactic sugar makes it ergonomic.

## Cross-link map

- [`/home/user/buiy/docs/prior-art/bevy-egui/lessons.md`](../bevy-egui/lessons.md) — corrects "Zed uses egui" conflation; names GPUI as the actual Zed UI stack.
- [`/home/user/buiy/docs/prior-art/egui/lessons.md`](../egui/lessons.md) — immediate-mode-only counterpoint to GPUI's hybrid.
- [`/home/user/buiy/docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md) — names GPUI as verified-false AccessKit adopter; reinforces a11y-first commitment.
- [`/home/user/buiy/docs/prior-art/iced/lessons.md`](../iced/lessons.md) — wgpu-uniform retained-mode counterpoint to GPUI's three-backend native-API strategy.
- [`/home/user/buiy/docs/prior-art/cosmic-text/`](../cosmic-text/) — text-stack alternative to GPUI's OS-native shaping.
- [`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) §§2.2-2.3 — the foundation sections this corpus most directly informs.

## Sources

- All evidence files in this folder: [`README.md`](README.md), [`architecture.md`](architecture.md), [`element-tree.md`](element-tree.md), [`gpu-rendering.md`](gpu-rendering.md), [`text-and-input.md`](text-and-input.md), [`accessibility.md`](accessibility.md), [`history.md`](history.md), [`distribution-and-governance.md`](distribution-and-governance.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md), [`glossary.md`](glossary.md).
- Primary external: [Scandurra _Videogame_ post](https://zed.dev/blog/videogame), [_Ownership and data flow in GPUI_](https://zed.dev/blog/gpui-ownership), [DeepWiki GPUI](https://deepwiki.com/zed-industries/zed/2.2-ui-framework-(gpui)), [GPUI source](https://github.com/zed-industries/zed/tree/main/crates/gpui).
