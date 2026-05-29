**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium) — LayoutNG: the fragment-tree model, immutable layout results, the layout input node, per-mode layout, line breaking, and the decade-long legacy-layout migration

# Blink — LayoutNG

LayoutNG is Chromium's layout engine, the rewrite that replaced the legacy "LayoutObject mutates itself in place" engine inherited from WebKit. It is the canonical reference implementation of the CSS box / display / fragmentation model, the same model Buiy implements a typed-Rust subset of. The interesting prior art here is not the algorithms (everyone implements Flexbox roughly the same) — it is the *data-flow rearchitecture*: explicit immutable input and output, a fragment tree separate from the node tree, and the fact that ripping this through a shipping browser took roughly a decade and was never fully finished. That cost is the lesson Buiy's clean-start-on-`Taffy` posture is buying its way out of.

For where layout sits in the broader engine pipeline see [architecture.md](architecture.md); for stacking-context formation and paint order (a *separate* tree LayoutNG feeds) see [stacking-and-paint.md](stacking-and-paint.md); for `contain` / `content-visibility` and container queries see [containment-and-queries.md](containment-and-queries.md); for `ComputedStyle` (LayoutNG's input) see [style.md](style.md).

## 1. The legacy engine LayoutNG replaced

WebKit/Blink's original layout was a tree of `LayoutObject` (formerly `RenderObject`) nodes that each held *both* the input style-derived state and the mutable output geometry, and recomputed in place. There was no explicit boundary between "what was asked of this box" and "what this box decided." That coupling had three consequences the Chromium team called out repeatedly:

- **Re-entrancy and re-measure blowups.** A box could be measured, then laid out, then re-measured under a different constraint, each pass mutating shared state. Intrinsic-size queries (`min-content` / `max-content`) could trigger exponential re-layout — the Chrome team's own demo shows a Grid case that was "fixed in Chrome 93 as a result of moving Grid onto the new architecture," because the old code path re-entered layout super-linearly.
- **Float and margin-collapsing bugs that could not be fixed locally.** Because state was global to the object, fixing one float interaction tended to break another. The `chromium.org` LayoutNG page states the rewrite "fixes many issues around floats and margins" and "a large number of web compatibility issues" — i.e. these were *architectural* bugs, not isolated defects.
- **Poor support for non-Latin scripts** in inline layout, cited as a first-class motivation.

## 2. The LayoutNG data model

LayoutNG's core idea: make layout a pure-ish function with explicit, immutable input and output.

**Layout input node (`NGLayoutInputNode`, now `LayoutInputNode`).** A thin, read-only view over the legacy `LayoutObject` tree plus its `ComputedStyle`. It is the *input* identity — "this box, this style, these children" — and is never mutated by a layout pass. Subclasses (`BlockNode`, `InlineNode`) dispatch to the algorithm for that box's formatting context.

**Constraint space (`NGConstraintSpace` / `ConstraintSpace`).** The immutable input *parameters*: the available inline/block size imposed by the parent, percentage-resolution sizes, fragmentation context (column/page break offsets), float exclusions, writing mode and direction, and cache-affecting flags. Per the deep-dive doc, layout conceptually takes "Style plus DOM" plus "parent constraints from the parent layout system (grid, block, or flex)."

**Fragment tree (physical fragments, `NGPhysicalFragment` / `PhysicalBoxFragment`).** The *output*: "a completely new, immutable object called the fragment tree." A fragment is the geometry a box produced *for one constraint space*. Because a single box can be split across columns or pages, one `LayoutInputNode` can produce *several* fragments — this is why the output is a separate tree rather than a field on the node. Fragments are immutable once produced.

**The cache key.** Alongside each fragment "we store the parent constraints object which generated that fragment. We use this as a cache key." On the next layout, if a box is reached with an equal `ConstraintSpace`, the cached fragment is reused without descending. The doc's framing: explicit input + output data structures plus measure/layout caches "brings the complexity back to O(n), resulting in predictably linear performance." Predictability — not raw speed — was the headline goal.

The hard rule that makes this work: **"accessing the previous state isn't allowed."** A layout algorithm sees only its `ConstraintSpace` and its children's fragments. It cannot reach sideways into a sibling's mutable state, which is exactly what made the legacy engine's bugs non-local.

## 3. Per-mode layout algorithms

LayoutNG dispatches by formatting context, each a separate algorithm class:

- **Block** (`NGBlockLayoutAlgorithm`) — block container layout: in-flow block children, float placement and exclusion, margin collapsing, out-of-flow (absolute/fixed) child collection. The first thing ported.
- **Inline / line breaking** (`NGInlineLayoutAlgorithm`, `NGLineBreaker`) — the inline formatting context. This is the part with no analog in many layout libraries: it collects an *items* list (text runs, atomic inlines, open/close tags), shapes text via HarfBuzz, then runs a greedy line breaker that respects `white-space`, hyphenation, bidi reordering, `vertical-align`, and float intrusion into line boxes. Output is a tree of line-box fragments. Non-Latin script handling lived here and was a stated reason for the rewrite.
- **Flexbox (FlexNG)** — flex container layout on the NG architecture. (Public sources do not pin a single "FlexNG shipped" Chrome version the way block/inline name Chrome 77; the preamble's "~Chrome 87" is plausible but **unverified** — see Sources. What is documented is that flex/grid/table all migrated *after* the Chrome 77 block/inline launch, over subsequent releases.)
- **Grid (GridNG)** — a re-architecture led by the Microsoft Edge team; the Grid migration is what fixed the Chrome 92→93 exponential-layout and hysteresis bugs. Raised targeted Grid test pass rate from ~94% to ~97%.
- **Tables (TablesNG)** — "a multi-year effort to re-architect rendering of tables," landing in the Chrome/Edge 93 timeframe, fixing `position: sticky` table headers and resolving 72 tracked Chromium bugs.
- **Block fragmentation** — splitting content across columns/pages/regions. Core block-container fragmentation shipped around Chrome 102; flex/grid fragmentation around Chrome 103. This was deliberately *last* because it is the feature that forces output to be a multi-fragment tree.

## 4. The migration cost

The order of operations is the prior art. The legacy and NG engines ran **side by side for years**: when an NG algorithm did not yet exist for a box, Blink fell back to the legacy engine for that subtree. The rough public timeline:

- **2016–2019** — design and incremental build behind a flag.
- **Chrome 77 (Sep 2019)** — block + inline layout (incl. floats, out-of-flow positioning) ship on NG; flex/grid/table/fragmentation still legacy.
- **~2020–2021** — FlexNG, GridNG land; legacy flex/grid removed behind them.
- **Chrome 93 (2021)** — TablesNG and the Grid-on-NG fixes; exponential-layout and `sticky`-header classes of bug closed.
- **Chrome 102–103 (2022)** — block, then flex/grid, fragmentation on NG.

So: a layout rewrite announced internally mid-2010s, first user-visible in 2019, and not feature-complete (fragmentation) until 2022 — a **multi-year, arguably decade-long** effort, carried out while never breaking a browser shipping to billions of users and while keeping two engines coherent the whole time. The blog calls it "a multi-year effort."

The unflattering read: the *correct* architecture was understood early, but retrofitting it into a live, dependency-saturated engine — not the algorithms themselves — is what consumed the decade. There is no public "LayoutNG is done, legacy deleted" milestone; cleanup of the last legacy paths trailed for years after the headline launches. The dual-engine period also carried its own bug class — content whose subtree straddled the legacy/NG boundary could hit interop seams that neither engine alone exhibited — so the migration was not merely slow, it temporarily *added* a failure mode that only disappeared once a formatting context was fully ported. This is the tax of incremental rewrite-in-place: correctness regressions you would never have if you started clean.

## 5. Implications for Buiy

Buiy is downstream of this lesson in a precise way: it gets the *output* of the LayoutNG rearchitecture (explicit immutable input → output, layout-writes / render-reads) **for free** because it starts clean on `Taffy`, and it never pays the migration cost because it has no legacy engine to coexist with.

- **The immutable input/output split is already Buiy's contract.** `Taffy` is the pure layout function: styled-node tree in, `Layout` rects out, with frame-to-frame caching keyed on inputs (see [../taffy/architecture.md](../taffy/architecture.md) §4–5). Buiy's pipeline is the same data-flow discipline LayoutNG arrived at — `SyncStyles` → `TaffyCompute` → `WriteResolvedLayout`, with `RemovedNodesGc` and `WritingModeInherit` upstream — expressed as ordered ECS systems rather than a recursive in-place tree. Buiy [architecture.md §3](../../specs/2026-05-08-buiy-layout-design/architecture.md#3-system-pipeline).
- **"Layout writes, render reads" is the same hard rule as "accessing the previous state isn't allowed."** Buiy's `PostTaffyOverrides` sub-passes write *private* render-handoff components (Phase 8's `ResolvedTransform`; Phase 9's `StackingContext { painters_z }`) that render consumes read-only. Render never recomputes stacking or paint order — exactly LayoutNG feeding an immutable fragment/property tree forward to paint. See [stacking-and-paint.md](stacking-and-paint.md).
- **The fragment tree is a feature Buiy deliberately does *not* have yet.** A single LayoutNG node producing multiple physical fragments exists *because* of block fragmentation (columns/pages). Buiy's multi-column and table sub-passes (6b/6c) are explicit **stubs** today, and `Taffy` itself has no inline formatting context or fragmentation ([../taffy/architecture.md](../taffy/architecture.md) §7). Buiy gets one rect per entity. The LayoutNG history says this is the *right* place to stop: fragmentation is what forces the multi-fragment output tree and was the single most expensive part of the migration. Adding it later is an above-`Taffy` pass, not a fork.
- **Inline layout / line breaking is out of scope for `Taffy` and lives in text.** LayoutNG's `NGLineBreaker` (shape → greedy break → bidi → line-box fragments) is mirrored in Buiy by `cosmic-text` doing shaping + line breaking, with `Taffy` treating a text run as a single measured leaf. Buiy never reimplements the inline formatting context inside the layout engine — see [../cosmic-text/](../cosmic-text/) and Buiy foundation [README §4](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap) (`text.md`).
- **Per-mode algorithm dispatch is `Taffy`'s `Display` switch.** LayoutNG's one-algorithm-per-formatting-context structure is `Taffy`'s `compute_flexbox_layout` / `compute_grid_layout` / `compute_block_layout` dispatch — already factored, already tested against WPT-style suites, so Buiy inherits Flexbox + Grid + Block + Float without the Blink-scale build-out.
- **The decade is the headline.** LayoutNG demonstrates the dominant cost of a layout engine is not writing it but *migrating onto it inside a live system with backward-compat obligations*. Buiy has neither a legacy engine nor web-compat obligations, only a typed CSS *subset* it can grow additively. That is the clean-start dividend; it does not make Buiy faster than Blink, it makes Buiy's *change cost* lower.

## Sources

- LayoutNG (chromium.org): block/inline shipped Chrome 77; "fixes many issues around floats and margins"; non-Latin scripts; staged migration — https://www.chromium.org/blink/layoutng/
- RenderingNG deep-dive: LayoutNG (immutable fragment tree, constraint-space cache key, O(n)/"predictably linear", "accessing the previous state isn't allowed", Chrome 92→93 Grid/hysteresis fix): https://developer.chrome.com/docs/chromium/layoutng
- RenderingNG deep-dive: LayoutNG block fragmentation (multi-fragment output, fragmentation timeline): https://developer.chrome.com/docs/chromium/renderingng-fragmentation
- LayoutNG blog ("a multi-year effort", launched in stages): https://developer.chrome.com/blog/layoutNg-2
- RenderingNG architecture (pipeline context, main/compositor/Viz split): https://developer.chrome.com/docs/chromium/renderingng-architecture
- Blink fork announcement, 2013-04-03 (verified): https://blog.chromium.org/2013/04/blink-rendering-engine-for-chromium.html
- TablesNG / GridNG migration, Edge team, Chrome/Edge 93, 72 bugs, ~94%→97% Grid tests (verified): https://web.dev/blog/compat2021-midyear
- LayoutNG source README: https://chromium.googlesource.com/chromium/src/third_party/+/refs/heads/main/blink/renderer/core/layout/layout_ng.md
- Chromium `LICENSE` = top-level BSD-3-Clause (Google copyright holder) with WebKit-inherited LGPL/MIT/MPL per-file headers (verified): https://chromium.googlesource.com/chromium/src/+/main/LICENSE
- Sibling prior-art: [../taffy/architecture.md](../taffy/architecture.md), [../bevy-ui/](../bevy-ui/), [../dioxus/](../dioxus/), [../servo-stylo/](../servo-stylo/) (the other canonical Rust reference implementation of these CSS modules).
- Buiy specs: layout [architecture.md](../../specs/2026-05-08-buiy-layout-design/architecture.md), [stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md); foundation [README.md](../../specs/2026-05-07-buiy-foundation/README.md)
