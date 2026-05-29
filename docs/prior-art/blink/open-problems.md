**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium) — structural open problems and forward-looking gaps

# Open problems

This file is the forward-looking complement to [`critiques.md`](critiques.md). Critiques names costs Blink has already paid and will not easily reverse; this file names *structural* problems that are still open — the ones whose resolution is in flight or genuinely undecided. Each item ties back to a Buiy decision where the structural choice differs. Architecture and pipeline detail are in [`architecture.md`](architecture.md); the CSS-feature surface is in [`layout.md`](layout.md), [`style.md`](style.md), [`stacking-and-paint.md`](stacking-and-paint.md), and [`containment-and-queries.md`](containment-and-queries.md).

## Memory safety in a C++ engine

**Status:** open, mitigated, not solved. Around 70% of Chrome's high-severity security bugs are memory-safety bugs. Mitigations shipped — `MiraclePtr`/`BackupRefPtr` for use-after-free, heap scanning, `base::span` adoption, the migration toward safer container/iterator idioms — and Chromium has begun allowing Rust for *new, leaf, third-party* components. But the rendering engine itself (`third_party/blink`) is not being rewritten in Rust, and there is no credible plan to do so: the codebase is too large and too coupled.

**Blockers:**
1. Blink is multi-million lines of interdependent C++; a Rust rewrite of the renderer has no funding case against incremental hardening.
2. The Chromium Rust policy as of 2026 is "new code / leaf libraries," not "rewrite the core."
3. Web-compat constraints forbid behavior drift, so any rewrite would have to be bug-for-bug.

**Implications for Buiy.** This is the clearest structural reason Buiy's substrate is the *Rust* reference lineage (Servo/Stylo) rather than the C++ one for *implementation* technique, even while citing Blink for *behavior*. Buiy is memory-safe by language; the entire class of UAF/OOB bugs that dominates Chrome's severity reports does not exist in Buiy's `unsafe`-light Rust. See [`comparisons.md`](comparisons.md) and the Buiy foundation [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md).

## The two-layout-engine boundary, generalized

**Status:** LayoutNG has replaced legacy layout for block/inline (Chrome 77, 2019), flex, grid, and table, but the *pattern* — migrating a live engine primitive-by-primitive while keeping a cross-engine boundary consistent — recurs every time a layout subsystem is rebuilt. Block fragmentation (pagination/multicol/region break) was among the last to land on NG and remains the area where fragment-tree edge cases surface.

**Open subtleties:**
1. Fragmentation across nested formatting contexts (a multicol inside a flex item inside a fragmented block) is where the immutable-fragment model is hardest to keep correct.
2. Interactions between new layout features and the fragment tree (e.g. how `contain: size` and `content-visibility` short-circuit fragment generation) are still being refined release-to-release.

**Implications for Buiy.** Buiy deliberately does *not* implement the hard cases: multicol and table are warn-once stubs today (`PostTaffyOverrides` sub-passes `6c` and `6b`), not solvers. This is an honest scoping choice, not a claim of parity — Buiy's spec marks them stubs and defers the fragmentation problem rather than pretending to solve it. Buiy inherits Taffy's single consistent solver for the cases it *does* support, so it never owns a legacy/NG-style boundary. See [`layout.md`](layout.md).

## Style invalidation correctness and cost

**Status:** Blink resolves style via the cascade into `ComputedStyle`, then uses *invalidation sets* to decide, on a DOM/attribute/state change, the minimal set of elements whose style must be recomputed. Invalidation sets are a long-running source of both correctness bugs (under-invalidation → stale style; over-invalidation → wasted recalc) and performance work. This is intrinsically open: any selector feature added (`:has()`, container-query-conditioned style, `@scope`) forces the invalidation model to absorb a new dependency edge.

**Open subtleties:**
1. `:has()` makes invalidation *upward* and sibling-directional, which the original descendant-oriented invalidation-set model did not assume.
2. Container queries make style depend on resolved *layout* size, which couples the style and layout phases that the pipeline tries to keep separable.

**Implications for Buiy.** Buiy's container-query story is a same-frame re-layout, explicitly bounded: `CqActivate` → `TaffyCompute` → `CqFlipCheck` → `CqFlipReRun`, capped at 2× Taffy passes per frame (Phase 5, landed; spec [`../../specs/2026-05-08-buiy-layout-design/`](../../specs/2026-05-08-buiy-layout-design/)). Buiy sidesteps general selector-driven invalidation entirely because authoring is ECS components + a `Style` builder, not a cascaded selector engine — there is no `:has()`-style arbitrary selector to invalidate against. This is a smaller, more tractable problem *because* Buiy chose a subset.

## Container-query / layout phase coupling

**Status:** open by construction. Container queries (shipped Chrome 105, 2022-08-30) let an element's *styles* depend on an ancestor container's *resolved size*, and CSS anchor positioning (shipped Chrome 125, 2024-05) plus the newer **anchored container queries** (Chrome 143, article published 2025-10-29 — verified against the developer.chrome.com blog post; this is the highest Chrome version cited in the folder, so treat it as a single-source claim) deepen the dependency between layout results and subsequent style/layout. The pipeline's clean "style → layout → paint" ordering has to admit a controlled feedback edge, and Blink's containment machinery (`contain`, `content-visibility`) exists partly to bound how far that feedback can propagate.

**Implications for Buiy.** Buiy makes the feedback edge explicit and finite rather than open-ended: the `CqFlipCheck`/`CqFlipReRun` pair is the only re-entry into Taffy, and it is hard-capped. Anchor positioning is a *post*-Taffy override (sub-pass `6d`) using a Kahn topological sort with deterministic cycle-edge dropping, so anchor chains cannot loop the layout phase. Containment in Buiy is Phase 8 (landed): `Containment` flags with SIZE / INLINE_SIZE containment zeroing auto sizes, `content-visibility` as a deferred stub, `will-change` stored-only. Buiy gets the *bounding* benefit of containment without yet implementing `content-visibility`'s render-skipping — and says so. See [`containment-and-queries.md`](containment-and-queries.md).

## Stacking, compositing, and the property-tree machinery

**Status:** Blink computes property trees (transform / clip / effect / scroll) during pre-paint, then the compositor (`cc`) and Viz consume them to composite layers. Stacking-context formation is a *union* of triggers (positioned + `z-index`, `opacity < 1`, `transform`, `filter`, `isolation`, `will-change`, `contain`) and the top layer (`dialog.showModal()`, the Popover API in Chrome 114 / 2023, Fullscreen) escapes normal stacking and ancestor clipping. Keeping paint order, hit-testing, and compositing all consistent with this union is a permanent source of subtle bugs — "why is this element painting on top?" is one of the most common web-author confusions, and it reflects genuine machinery complexity.

**Implications for Buiy.** This is exactly Phase 9 (NEXT, not yet built; design at [`../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md`](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)). Buiy mirrors Blink's *model* deliberately: a `Stacking { z_index, isolation, top_layer }` component; stacking-context formation as a union of triggers (positioned + explicit `z-index`, `isolation: isolate`, non-identity transform, `contain: paint`/`strict`, `opacity < 1` / `filter` / `mix-blend-mode`, root); a private `StackingContext { painters_z: Vec<Entity> }` computed by sub-pass `6f` that hands render a pre-sorted paint order (neg `z-index`, in-flow non-positioned, floats, in-flow positioned `z: auto`, positive `z-index`); and a `TopLayer { None | Modal | Popover | Tooltip | Fullscreen }` escape hatch ordered Fullscreen < Tooltip < Popover < Modal via a `TopLayerActivation` `VecDeque` resource, escaping ancestor overflow clip, per window. The structural difference from Blink: Buiy's paint order is *computed once by the layout phase and never recomputed by render* (the layout-writes/render-reads contract), whereas Blink's property-tree → compositor handoff spans the pipeline. See [`stacking-and-paint.md`](stacking-and-paint.md).

## Writing modes and logical properties across the whole engine

**Status:** open in the sense that *every* layout and paint subsystem must independently honor writing-mode (`horizontal-tb` / `vertical-rl` / `vertical-lr`) and direction (`ltr` / `rtl`). LayoutNG's physical fragment tree is explicitly physical so that paint and hit-test never re-derive direction, but the *logical → physical* resolution (block/inline axes, logical insets, logical box-model properties) has to be threaded through layout, scrolling, fragmentation, and overflow. `sideways-lr` / `sideways-rl` and full vertical-text typography remain among the rougher corners, and bidi interaction with new layout features is a recurring bug source.

**Implications for Buiy.** Buiy resolves writing mode *once*, early, in a dedicated pass: `WritingModeInherit` produces `WritingModeResolved`, and a `LogicalBoxModel` / `LogicalInset` builder maps logical authoring to the physical values Taffy consumes (Phase 4, landed). Like Blink, Buiy keeps the post-resolution representation physical so render never re-derives direction. Like Blink, Buiy also defers the hardest typography: `sideways-*` are warn-once stubs, stated as such rather than claimed. The structural win is the same as Blink's fragment-tree discipline — resolve direction once, hand physical values forward — achieved with one pipeline pass instead of threading it through a monolith. See [`layout.md`](layout.md).

## Governance concentration

**Status:** open and unlikely to change. The Blink launch process (Intent to Prototype / Experiment / Ship on `blink-dev`, with three API-owner LGTMs required to ship) is genuinely open and documented (see [`governance.md`](governance.md)), but the engine is overwhelmingly Google-funded, and the monoculture (see [`critiques.md`](critiques.md)) means Blink's go/no-go decisions effectively decide the web platform's direction. There is no external body with veto over what Blink ships.

**Implications for Buiy.** Not directly applicable — Buiy is not a web engine and not governed by `blink-dev`. The relevant takeaway is *which spec to cite*: Buiy cites the W3C module text (Display 3, Positioned Layout, Containment 3, Writing Modes 4, Anchor Positioning 1) as the contract and treats Blink as the reference *implementation* of that text, not as the contract itself. Where Blink and the spec diverge, Buiy follows the spec and notes the divergence.

## Sources

- Chrome memory-safety / ~70% high-severity bugs — https://www.chromium.org/Home/chromium-security/memory-safety/
- LayoutNG (block/inline Chrome 77, 2019; later flex/grid/table) — https://www.chromium.org/blink/layoutng/
- RenderingNG architecture (pipeline stages) — https://developer.chrome.com/docs/chromium/renderingng-architecture
- CSS Container Queries shipped Chrome 105 (2022-08-30) — https://caniuse.com/css-container-queries
- CSS anchor positioning available from Chrome 125 (2024-05) — https://developer.chrome.com/blog/anchor-positioning-api
- Anchored container queries (Chrome 143; article published 2025-10-29; verified) — https://developer.chrome.com/blog/anchored-container-queries
- Popover API shipped Chrome 114 (2023) — https://developer.chrome.com/blog/introducing-popover-api
- content-visibility shipped Chrome 85 (2020) — https://web.dev/articles/content-visibility
- Blink launch process / Intents — https://www.chromium.org/blink/launching-features/
- CSS Writing Modes Level 4 (W3C) — https://www.w3.org/TR/css-writing-modes-4/
- LayoutNG physical fragment tree (paint/hit-test) — https://developer.chrome.com/docs/chromium/layoutng
- Buiy layout: stacking + top layer spec — ../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md
- Buiy layout design (folder) — ../../specs/2026-05-08-buiy-layout-design/
- Buiy foundation README — ../../specs/2026-05-07-buiy-foundation/README.md
