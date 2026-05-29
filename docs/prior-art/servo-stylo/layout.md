**Date:** 2026-05-29
**Status:** active
**Subject:** Servo Layout 2020 — box tree, fragment tree, formatting contexts, inline layout, stacking, and what it does (and doesn't) borrow from Taffy

# Servo / Stylo — layout

Servo's layout is the engine that turns Stylo's computed styles into geometry and a paint order. The current implementation is "Layout 2020", a from-scratch rewrite that replaced the original "Layout 2013" engine. Layout 2020 is now the **default** layout engine; the legacy engine was moved behind a feature flag (default-off in builds) once Layout 2020 surpassed it on the Web Platform Tests in 2024, and the legacy code is being deleted incrementally. The component now lives at `components/layout/` (the historical `components/layout_2020/` path was retired with the rename).

This file documents the box tree / fragment tree split, formatting contexts, inline layout, and stacking, and corrects the brief's central claim. See [stylo.md](stylo.md) for the style system that feeds layout, [rendering.md](rendering.md) for WebRender, and [architecture.md](architecture.md) for how the pieces wire together.

## 1. The brief was wrong: Servo uses Taffy for flex and grid

The orchestrator brief said "Servo does NOT use Taffy — it has its own layout." **That is half right and was the headline correction of this file.** Servo's Layout 2020 has its own box/fragment tree and its own **block, inline, table, and float** algorithms — but its **flexbox and CSS grid** formatting contexts are implemented *on top of Taffy*. The integration lives in `components/layout/taffy/`: `TaffyContainer` (in `mod.rs`) builds a `ModernContainerBuilder` of `TaffyItemBox` children, and `taffy/stylo_taffy/` adapts Stylo `ComputedValues` into Taffy's style traits (`TaffyStyloStyle`). This adapter was extracted as the standalone `stylo_taffy` crate (now vendored into Blitz) precisely so others can pair Stylo with Taffy.

This makes Servo a far closer relative of Buiy than the brief assumed. Both projects sit Taffy under their flex/grid layout. The difference is *layering*:

- **Servo** calls Taffy *inside* a formatting context — Taffy is one leaf algorithm among several (block/inline/table are Servo's own). Servo owns the surrounding box tree.
- **Buiy** runs Taffy as the *whole* `TaffyCompute` pass and layers its own passes (sticky, anchor, transforms, stacking) *above* Taffy in `PostTaffyOverrides`, never forking it ([Buiy architecture.md § 1](../../specs/2026-05-08-buiy-layout-design/architecture.md#1-bridge-model-buiy--taffy)).
- **Taffy alone** (see [../taffy/](../taffy/)) ships block/flex/grid/float but no inline or table layout — embedders supply those. Servo *is* one such embedder.

So the three-way contrast is real, but it is not "Servo vs Taffy"; it is "Servo embeds Taffy for two of its formatting contexts" vs "Buiy stacks passes above a whole-tree Taffy".

## 2. Box tree and fragment tree

Layout 2020's defining choice is splitting the box tree from the fragment tree, mirroring the CSS spec's own two-level model:

- **Box tree** — the persistent representation of nested formatting contexts. Nodes are atomically reference-counted (`servo_arc::Arc`) and mostly immutable, built once from the DOM and reused. The box tree records *what kind* of box each element generates (block container, inline, table, flex/grid item) and the nested-context structure, not final positions.
- **Fragment tree** — the output of laying the box tree out against a containing block. One box can produce many fragments (an inline split across lines, an element split across columns/pages). `components/layout/fragment_tree/` defines `BoxFragment`, `PositioningFragment`, `ContainingBlock`, and `FragmentTree`. The fragment tree is consumed to build the display list.

Layout runs in three phases: **box-tree construction → fragment-tree construction → display-list construction**. Fragment-tree layout is done in parallel via rayon until a subtree containing floats is reached; that block formatting context is laid out sequentially, then parallel layout resumes past the BFC boundary. The split is what makes incremental layout and fragmentation (multicol, paged) tractable — the exact problems the brief notes Layout 2013 could not solve because its flow nodes conflated boxes with their fragments.

## 3. Formatting contexts as nested enums

Servo encodes the formatting-context taxonomy directly in the type system. `formatting_contexts.rs` defines `IndependentFormattingContext`; the per-context content types live under `flow/` (block + inline), `table/`, `flexbox/`, and the Taffy-backed `taffy/`. Because each context is a distinct enum/struct, the children a context can hold are constrained at compile time to the content the CSS spec permits inside it — a block formatting context cannot accidentally hold raw inline-level content without an anonymous wrapper. The stated design goal is to "not gratuitously deviate from CSS specs in structure or terminology," using "mostly classic imperative code with recursive tree traversals."

Coverage of formatting contexts (mid-2026):

- **Block (BFC)** — Servo's own, `flow/mod.rs` + `flow/construct.rs`. Margin collapsing, in-flow block stacking.
- **Inline (IFC)** — Servo's own, `flow/inline/` (§4).
- **Float** — Servo's own, `flow/float.rs` + `flow/root.rs`. Floats force the sequential layout fallback.
- **Flex** — Taffy-backed via `taffy/`. Parallel flexbox shipped in 2024.
- **Grid** — Taffy-backed via `taffy/`. Shipped experimentally behind `--pref layout.grid.enabled`; the Taffy adapter raised Servo's CSS-grid WPT pass rate from ~18.6% to ~38.3% in the landing PR (#32619).
- **Table** — Servo's own, `table/`. A real table FC, not the sizing hint Taffy exposes.

## 4. Inline layout

Inline layout is one of the places Servo invests where Taffy is silent (Taffy has no inline layout at all). `flow/inline/` contains `text_run.rs`, `inline_box.rs`, `line_breaker.rs`, and `line.rs`: text runs are shaped, the line breaker segments them, and line boxes are assembled with bidi and atomic-inline (inline-block, replaced) handling. This is the IFC the brief asks about. It is also why Buiy's substrate uses `cosmic-text` for shaping above Taffy — neither Taffy nor Buiy reimplements an inline formatting context inside the layout engine; Buiy treats shaped text as measured leaves, where Servo runs a full IFC. (Blitz, the other Stylo+Taffy consumer, uses Parley rather than building its own IFC — see [../dioxus/integration-with-taffy.md](../dioxus/integration-with-taffy.md).)

## 5. Stacking contexts and paint order

`display_list/stacking_context.rs` is where Servo computes paint order — and it is the closest prior art to Buiy's Phase 9 ([stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md), the NEXT sub-pass 6f). Servo builds a `StackingContextTree` of `StackingContext` nodes. Each `StackingContext` carries:

- a `StackingContextType` (real stacking context vs positioned/atomic stacking *container*),
- `StackingContextSection` buckets (`OwnBackgroundsAndBorders`, `DescendantBackgroundsAndBorders`, `Foreground`, `Outline`) — CSS 2.1 Appendix E paint phases,
- four child lists: `real_stacking_contexts_and_positioned_stacking_containers`, `float_stacking_containers`, `atomic_inline_stacking_containers`, and `contents` (the fragment display items).

### How 4 section buckets + 4 child lists reconstruct CSS 2.1 Appendix E's ~7 steps

A 6f implementer mapping Servo's types onto `painters_z` should not be misled by the *four*-variant `StackingContextSection` enum — the full ~7-phase Appendix E order is reconstructed by walking the **child lists and the section buckets together**, not by the enum alone. `build_display_list` emits, in order: (1) the context's own `OwnBackgroundsAndBorders`; (2) **negative-z** entries from `real_stacking_contexts_and_positioned_stacking_containers` (the list is z-sorted, so the negative slice comes first); (3) `DescendantBackgroundsAndBorders` (in-flow block backgrounds); (4) `float_stacking_containers`; (5) `contents` at `Foreground` + `atomic_inline_stacking_containers` (in-flow inline/atomic content); (6) the **z:0 / z:auto positioned** slice of the contexts/containers list; (7) the **positive-z** slice of that same list; (8) `Outline`. So Appendix E's phases are the *cross-product* of (which child list) × (which z-slice) × (which section) — the enum names only the within-box phases, while the document-order/z-order interleave lives in the list splits. (The enumeration above lists eight emit-steps because it splits the context's own backgrounds from the descendant outlines; Appendix E's canonical "~7" collapses some of these.) Buiy's `painters_z` collapses that cross-product into one flat `Vec<Entity>` in exactly this sequence (its spec's five sub-orderings are the same steps; see [stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md) §"paint order").

### Sort stability / z-index ties

`StackingContext::sort()` uses **`sort_by_key`**, which is a *stable* sort: `contents` is keyed on `section()`, and `real_stacking_contexts_and_positioned_stacking_containers` is keyed on `z_index()`. Stability is the answer to a question a `painters_z: Vec<Entity>` author must decide — **equal-z-index siblings keep their original (document/tree-build) order**, because a stable sort never reorders equal keys. Servo gets the CSS "ties break by tree order" rule for free by (a) building the child list in document order and (b) using a stable sort on the z key. Buiy's 6f should do the same: push entities in document order, then `sort_by_key(z_index)` (stable), *not* `sort_unstable_by_key`, or the equal-z tiebreak becomes nondeterministic.

### The Phase 8 → Phase 9 seam: transforms/opacity form new stacking contexts mid-tree

Formation is decided by `style.establishes_stacking_context(flags)`, which (per Servo source) returns true for non-empty `filter`, `opacity != 1.0`, `mix_blend_mode != Normal`, an active `transform` **or** `perspective` (`has_effective_transform_or_perspective()`), a non-`None` `clip_path`, or `transform_style != Flat` — plus positioned + non-auto `z-index`, `isolation: isolate`, `contain: paint`, and the root. This is the seam between Buiy's already-landed Phase 8 `ResolvedTransform` (sub-pass 6e) and Phase 9 (6f): in Servo a transformed element is **both** a transform reference frame **and** a stacking-context root — the transform does not just move pixels, it *forms* a new context that re-roots its descendants' paint order. So 6f cannot read transform state independently of 6e: an entity with a non-identity `ResolvedTransform` must be treated as a stacking-context former (it appears as a single unit in its parent's `painters_z`, and its own descendants sort within it), mirroring `has_effective_transform_or_perspective()` feeding `establishes_stacking_context`. Walking Servo confirms the union-of-triggers in Buiy's spec is complete and that transform/opacity are first-class formers, not afterthoughts.

### Fixed-position containing block under transforms (a notorious edge)

Servo tracks fixed/absolute descendants via `for_absolute_and_fixed_descendants`, which normally carries the spatial id of the **root reference frame** (so fixed elements don't scroll). But when a fragment "establishes a containing block for all descendants" — which a non-identity `transform` does — that spatial id is **updated to the transformed ancestor**. This is the CSS rule that *a transform on an ancestor makes that ancestor the containing block for a `position: fixed` descendant* (the element then scrolls/transforms with the ancestor instead of the viewport). It is relevant to both Phase 8 (the transform that captures the fixed element) and Phase 9 (where that element then stacks). Buiy's 6f/anchor passes must apply the same rule: a `Position::Fixed` entity whose ancestor has a non-identity `ResolvedTransform` resolves its containing block to that ancestor, not the window root — otherwise its position and its stacking membership diverge from CSS.

### Top-layer: zero positive content here (use Blink)

Servo ships **no** CSS top-layer / `popover` / dialog `::backdrop` story comparable to Buiy's `TopLayer`; top-layer ordering is a Buiy addition with no Servo analogue. Phase 9 author expectation, stated plainly: **this folder covers only the z-index/stacking half of sub-pass 6f.** For the other half — `TopLayer { None | Modal | Popover | Tooltip | Fullscreen }`, the `TopLayerActivation` `VecDeque` (activation-order paint within a tier), and the "top-layer element escapes every ancestor `overflow` clip" rule — Servo's source has nothing to settle, so you must leave the folder for `../blink/stacking-and-paint.md`. The redirect is honest, but do not expect Servo to confirm or contradict the top-layer design; it is silent on it.

### Cost of building + sorting the stacking-context tree

The folder can quote WPT pass-rate deltas (CSS grid 18.6% → 38.3%) but has **no published cost/scaling figure** for `StackingContextTree` construction + `sort()` — Servo's blog posts do not benchmark it. The honest read for Buiy: stacking-tree build is an O(n) walk plus a `sort_by_key` per context (so O(k log k) per context with k positioned children, near-linear overall because most boxes are not positioned), but whether running 6f *every frame* versus *incrementally* (only when `Stacking`/`z_index`/`Position`/`ResolvedTransform` components change, via Bevy change-detection) is justified is **not** answerable from Servo's published material. Servo rebuilds per layout, but Servo also rebuilds layout far less often than a 60fps ECS schedule ticks. Treat the every-frame-vs-incremental decision as a Buiy measurement task, not a settled question Servo answers.

## 6. Implemented vs missing

- **Implemented:** block, inline (with bidi, line breaking), float, table, flex (parallel), grid (experimental, Taffy-backed), absolute/relative/fixed positioning (`positioned.rs`), sticky positioning, stacking contexts, transforms and `mix-blend-mode` (consumed in display-list build), iframes, min/max sizing, `text-indent`.
- **Partial / in progress:** CSS counters, vertical writing modes (called out as features needed before fully committing to Layout 2020), fragmentation (the tree split enables it but multicol/paged is incomplete).
- **Architectural note:** because flex/grid are delegated to Taffy, Servo inherits Taffy's gaps there (e.g. subgrid, masonry — see [../taffy/](../taffy/) capabilities), not a separate Servo implementation of them.

## 7. Implications for Buiy

- **Validation of the substrate.** Servo independently arrived at "Stylo for style, Taffy for flex/grid box layout" — the same two crates Buiy commits to. That two production CSS engines (Servo, Blitz) pair Stylo+Taffy is the strongest available evidence the Taffy choice is load-bearing, not a toy.
- **Inline/table are not Taffy's job.** Servo writing its own IFC and table FC confirms Buiy's decision to handle text via `cosmic-text` as measured leaves and tables as a post-Taffy sub-pass (6b) rather than expecting Taffy to grow them.
- **Stacking is a tree-sort handed to render.** Servo's `StackingContextTree` + `sort()` + section buckets is a direct reference implementation for sub-pass 6f. The CSS 2.1 Appendix E ordering Servo encodes is the order Buiy must bake into `painters_z`.
- **License divergence.** Servo and the `stylo_taffy` adapter are MPL-2.0; Buiy is MIT OR Apache-2.0. Buiy can study and cite Servo's layout but cannot copy MPL code into a permissively-licensed crate without relicensing implications — see [governance.md](governance.md). (Taffy itself is MIT, so the shared dependency is clean.)

## Sources

- Servo layout component source (box tree, fragment tree, formatting contexts, Taffy module): https://github.com/servo/servo/tree/master/components/layout
- `components/layout/taffy/mod.rs` (`TaffyContainer`, `TaffyItemBox`, `TaffyStyloStyle`) — verified Servo embeds Taffy for flex/grid: https://github.com/servo/servo/blob/master/components/layout/taffy/mod.rs
- `components/layout/display_list/stacking_context.rs` (`StackingContextTree`, `StackingContext`, `StackingContextSection`, `sort`, `z_index`): https://github.com/servo/servo/blob/master/components/layout/display_list/stacking_context.rs
- `components/layout/flow/inline/` (IFC: `text_run`, `inline_box`, `line_breaker`, `line`): https://github.com/servo/servo/tree/master/components/layout/flow/inline
- Layout 2020 wiki (box/fragment tree split, three phases, parallel layout + float fallback): https://github.com/servo/servo/wiki/Layout-2020
- "Layout 2013 and Layout 2020" (engine history; status as of 2023): https://servo.org/blog/2023/04/13/layout-2013-vs-2020/
- Servo reboot / Layout 2020 became default + legacy behind feature flag (2024): https://www.atbrakhi.dev/blog/oss-north-america and PR #32759 (move legacy layout behind a feature flag): https://github.com/servo/servo/pull/32759
- PR #32619 (CSS Grid via Taffy; 18.6% → 38.3% WPT grid pass rate): https://github.com/servo/servo/pull/32619
- "This month in Servo" (parallel flexbox, grid behind `--pref layout.grid.enabled`): https://servo.org/blog/2024/12/09/this-month-in-servo/
- `stylo_taffy` crate (adapter, MPL-2.0; repo points at Blitz): https://crates.io/crates/stylo_taffy
- Buiy stacking + top-layer spec (sub-pass 6f): [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)
- Buiy layout architecture (Taffy bridge model): [../../specs/2026-05-08-buiy-layout-design/architecture.md](../../specs/2026-05-08-buiy-layout-design/architecture.md)
- Buiy foundation overview: [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling prior-art: [../taffy/](../taffy/), [../dioxus/](../dioxus/), [stylo.md](stylo.md), [rendering.md](rendering.md), [architecture.md](architecture.md), [governance.md](governance.md)
