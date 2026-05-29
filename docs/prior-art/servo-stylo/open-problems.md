**Date:** 2026-05-29
**Status:** active
**Subject:** Servo / Stylo — what the engine structurally has not solved or remains incomplete on

# Servo / Stylo — Open Problems

Where Servo is still incomplete or has chosen not to solve a problem. This is the
"do not assume parity" list. It complements the honest history in
[critiques.md](critiques.md) and the engine-vs-engine framing in
[comparisons.md](comparisons.md). For what Servo *does* do well, see
[stylo.md](stylo.md), [layout.md](layout.md), [rendering.md](rendering.md).

## 1. Whole-engine web-platform completeness

Servo does not pass the full web-platform test (WPT) suite and is not a complete
user agent. JavaScript runs via SpiderMonkey bindings, but DOM/web-API coverage,
forms, media, and many CSS features are partial. Igalia's revival has steadily
expanded coverage (tables, floats, more of flexbox/grid, etc.), but the gap to
Blink/Gecko/WebKit is structural: those engines have person-decades of accreted
edge-case handling that a ~5-engineer team cannot replicate quickly. This is the
backdrop for every "Servo supports X" claim — verify the *degree* of support, not
just presence.

Implication for Buiy: Buiy is *not* trying to be a user agent. It implements a
typed-Rust subset of a handful of CSS modules (Display 3, Positioned Layout,
Containment 3, Writing Modes 4, Anchor Positioning 1). Servo's incompleteness as
a browser is a feature for Buiy as prior art — it shows which subset of the
platform is tractable for a small team, and which corners (full inline layout,
fragmentation, pagination) are genuinely hard.

## 2. Inline layout, fragmentation, and pagination

Even within layout 2020, the hard parts of CSS layout are the ones Servo has
spent years iterating on: inline formatting (bidi, complex line breaking,
floats interacting with inlines), fragmentation across columns/pages, and the
table formatting context. Servo issue #22397 ("RFC: Fragmentation / pagination /
multicol / inline layout models") and ongoing PRs (e.g. splitting inline boxes
that contain block-levels) show these are *active, not closed* problems. The
fragment-tree design exists precisely because fragmentation is intrinsically
hard.

Implication for Buiy: Buiy delegates box-level layout to Taffy and treats tables
and multicol as **explicit stubs** in `PostTaffyOverrides` (sub-passes 6b
table-stub, 6c multicol-stub). That mirrors Servo's reality that these are the
costliest formatting contexts — Buiy is honest that they are unsolved rather than
half-implemented. See [layout.md](layout.md).

## 3. Parallel layout is not fully realized

A founding Servo thesis was *parallel layout*: lay out the tree across cores
because Rust makes data races impossible. In practice, layout 2013's eager
parallelism was part of why it was replaced; layout 2020 is more spec-faithful
and does parallelize parts of the work, but "lay out the whole page in parallel"
turned out to be much harder than "match CSS semantics correctly." Stylo's
parallelism (the parallel *cascade* via `rayon` over the DOM) is the part of the
parallel-Rust thesis that decisively succeeded — style resolution parallelizes
cleanly because it is largely per-element; layout does not, because it is
inherently sequential in dependency (parent sizes constrain children, floats and
margins couple siblings).

Implication for Buiy: Buiy's layout pipeline is a deterministic, ordered system
sequence (`RemovedNodesGc` → `WritingModeInherit` → `SyncStyles` → `CqActivate`
→ `TaffyCompute` → `CqFlipCheck` → `CqFlipReRun` → `PostTaffyOverrides` →
`WriteResolvedLayout`). Buiy does *not* chase parallel layout; it leans on
Bevy's ECS scheduler for system-level parallelism elsewhere. Servo's experience
is direct evidence that parallel layout is a deep, possibly-not-worth-it problem,
while parallel *style* is the genuine win — and even that lives upstream in
Firefox more than in a Buiy-shaped use case.

## 4. Embeddability and a stable public API

Servo is marketed as embeddable, but a stable, ergonomic embedding API has been
a long-running open problem. The `servoshell` reference exists, and there is a
webview-style API, but consumers report churn and rough edges; the project's own
roadmaps repeatedly list embedding ergonomics as a focus. The standalone Stylo
crate has *no* stability guarantee at all — `stylo` 0.17.0 / `selectors` 0.38.0
are pre-1.0 and version-churn heavily, tracking Gecko's needs first.

Implication for Buiy: depending directly on `stylo` would tie Buiy to a fast-
moving, Gecko-driven, pre-1.0, MPL-2.0 crate. Buiy instead reuses `cosmic-text`
for text and Taffy for layout, and authors its own typed style components. The
Stylo dependency that Buiy *could* take (via Blitz's `blitz-dom`) is exactly the
one Buiy declines — see [comparisons.md](comparisons.md).

## 5. Accessibility

Servo's accessibility story is thin. As an experimental engine focused on
rendering and layout, the full accessibility tree / platform a11y API integration
that a shipping browser needs has not been a priority; coverage exists but lags
the rendering work. (Notably, AccessKit — the crate Buiy builds on — emerged from
the broader Rust GUI ecosystem, not from Servo.)

Implication for Buiy: this is a place where Buiy deliberately *exceeds* its
browser prior art. Buiy is **AccessKit-first with a WCAG 2.2 AA floor** (see
[../../specs/2026-05-07-buiy-foundation/accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md)).
Servo demonstrates the failure mode (a11y deferred until "later") rather than a
model to copy.

## 6. Sustainability as the meta-problem

The deepest open problem is not technical. Servo's history — Mozilla origin,
2020 layoffs, dormancy, Linux Foundation Europe + Igalia revival, donation-and-
grant funding — shows that the binding constraint on a Rust web engine is
*sustained funding and headcount*, not Rust's capabilities. Every technical gap
above is downstream of this. See [governance.md](governance.md) and
[history.md](history.md).

Implication for Buiy: Buiy's scope discipline (bounded CSS subset, reuse Taffy /
`cosmic-text` / AccessKit / wgpu rather than rebuild) is the engineering answer
to the sustainability problem Servo embodies. Reuse is the survival strategy.

## Sources

- WPT / completeness, JS via SpiderMonkey: https://servo.org/about/ , https://book.servo.org/ , https://en.wikipedia.org/wiki/Servo_(software)
- Inline / fragmentation / multicol RFC + PRs: https://github.com/servo/servo/issues/22397 , https://github.com/servo/servo/pull/41492 , https://github.com/servo/servo/wiki/Layout-2020
- Layout 2013 eager parallelism vs 2020: https://servo.org/blog/2023/04/13/layout-2013-vs-2020/ , https://book.servo.org/design-documentation/layout.html
- Stylo parallel cascade (rayon): https://github.com/servo/stylo , https://hacks.mozilla.org/2017/08/inside-a-super-fast-css-engine-quantum-css-aka-stylo/
- Stylo crate pre-1.0 versions: https://crates.io/crates/stylo , https://crates.io/crates/selectors
- Revival / coverage progress: https://blogs.igalia.com/mrego/servo-revival-2023-2024/ , https://www.phoronix.com/news/Servo-2023-Progress
- Buiy specs: [../../specs/2026-05-08-buiy-layout-design/README.md](../../specs/2026-05-08-buiy-layout-design/README.md) , [../../specs/2026-05-07-buiy-foundation/accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Sibling prior-art: [../taffy/](../taffy/) , [../coherent-gameface/](../coherent-gameface/) , [../rmlui/](../rmlui/)
