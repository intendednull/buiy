**Date:** 2026-05-29
**Status:** active
**Subject:** Servo / Stylo — honest third-party critiques: incompleteness as a shipping browser, perennial under-resourcing, layout-rewrite churn, license divergence

# Servo / Stylo — Critiques

Servo is load-bearing prior art for Buiy because it is the Rust reference
implementation of the same W3C modules Buiy implements a typed subset of (see
[stylo.md](stylo.md), [layout.md](layout.md), [rendering.md](rendering.md)).
Its components — `stylo`, `selectors`, `cssparser`, WebRender, and the layout
lineage that produced Taffy — are widely reused, and that reuse can flatter the
underlying engine. This file collects the unflattering facts so Buiy borrows
architecture with eyes open.

## Critique 1 — Servo is not a shipping browser, and never was

Servo was announced by Mozilla Research in 2012 as a research engine for
parallelism and Rust, co-developed with the language itself. After more than a
decade it remains, in its own framing, an "experimental" / embeddable web engine,
not a daily-driver browser. The reference shell (`servoshell`) renders many real
sites but is explicitly not a complete user agent: it does not pass the full
web-platform test suite, and has long-standing gaps in JS/DOM/web-API coverage
(see [open-problems.md](open-problems.md)).

The honest reading: Servo proved that a parallel, memory-safe Rust engine is
viable and that its *components* are production-grade — Stylo shipped in Firefox,
WebRender shipped in Firefox — but the *whole browser* was never "done." The wins
were extracted and upstreamed into Gecko; the integrated engine kept being
research. Buiy should treat Servo as a catalogue of validated *parts*, not as a
finished product to copy wholesale.

## Critique 2 — Perennial under-resourcing

The defining fact of Servo's history is that it has almost never had enough
people. Mozilla laid off the Servo team in **August 2020** as part of a
company-wide cut of roughly 250 staff (~25% of the workforce). A Hacker News
thread from the period summarised the perception that "the Servo team was mostly
(maybe even entirely)" eliminated. The project went dormant.

Stewardship moved to the Linux Foundation in 2020 and to **Linux Foundation
Europe in September 2023**, with **Igalia** as the funded maintainer running
day-to-day development — on the order of ~5 engineers full-time, per Igalia's
own write-ups. Funding is community plus grant based: the Servo **Open
Collective** raised roughly **$59,000 in 2025 (about +62% over 2024)**,
supplemented by the **Sovereign Tech Fund**. These are real, meaningful sums for
an open-source project, but they are orders of magnitude below the engineering
budgets of Blink (Google) or even Gecko (Mozilla). A handful of engineers cannot
close a decade-plus feature gap against the canonical engines — and they do not
claim to.

Implication for Buiy: a CSS-faithful engine is enormous surface area, and even a
focused, funded team treats most of it as perpetually incomplete. Buiy's decision
to implement a **typed-Rust subset** of CSS — and to build *above* Taffy rather
than fork it — is the correct response. The scope must be deliberately bounded,
with the bound written down (see
[../../specs/2026-05-08-buiy-layout-design/README.md](../../specs/2026-05-08-buiy-layout-design/README.md)).

## Critique 3 — Layout-rewrite churn ("layout 2013" → "layout 2020")

Servo's layout engine was rewritten. The original engine ("layout 2013") used a
single "flow tree" with eager parallelism: boxes and fragments lived in the same
tree, internal nodes were `Flow` nodes for block/inline formatting contexts, and
leaves were fragments. The current engine ("layout 2020", originally in
`components/layout_2020`) separates the **box tree** (nested formatting-context
enums) from the **fragment tree** (the result of line breaking, columns,
pagination), matching how the CSS specs actually define layout. Servo's own blog
frames 2013-vs-2020 as a substantial redesign, and the 2013 engine was removed.

The lesson is not "rewrites are bad" — the second design is clearly better
spec-aligned. The lesson is that **the first plausible layout architecture was
wrong enough to throw away**, and that the eager-parallelism instinct (parallel
*because Rust makes it safe*) did not by itself produce a maintainable layout
tree. Spec fidelity (box tree separate from fragment tree) mattered more than
parallelism. Buiy avoids this class of churn by **not writing a layout solver at
all**: Taffy owns box-level Flexbox/Grid/Block/Float, and Buiy's
`PostTaffyOverrides` sub-passes layer CSS semantics above it (sticky, anchor,
transforms, stacking) without re-implementing the formatting-context machinery
Servo had to rewrite. See [comparisons.md](comparisons.md) and [layout.md](layout.md).

## Critique 4 — Component success can mask integration debt

Stylo and WebRender are genuine successes precisely *because* they were carved
out and integrated into a different, fully-staffed engine (Gecko). That carve-out
is also a critique: the value flowed *out* of Servo. Stylo lives at
`github.com/servo/stylo` as standalone crates (`stylo` 0.17.0, `selectors`
0.38.0; `cssparser` 0.37.0 at `github.com/servo/rust-cssparser`) consumed by
Firefox, by Servo, and by downstream projects like Blitz. WebRender similarly
became Gecko's compositor. The integrated Servo engine — the thing meant to be a
browser — is the part that stalled. A healthy "everyone reuses our crates" story
coexists with a "the product never shipped" story. Both are true.

## Critique 5 — License divergence (MPL-2.0)

Servo and Stylo are licensed **MPL-2.0** (Mozilla Public License 2.0), confirmed
in `servo/servo`'s `LICENSE`. This **diverges from Buiy's MIT OR Apache-2.0**.
MPL-2.0 is a weak/file-level copyleft: modifications to MPL-licensed *files* must
be released under MPL, but the license permits combining MPL files with
differently-licensed code in a larger work. The practical consequence for Buiy:
**read Servo/Stylo source for understanding and architecture; do not copy code
verbatim** into Buiy's MIT/Apache tree. Taffy (used by Buiy) is dual MIT/Apache
and is a clean re-implementation in the lineage of Servo's layout — Buiy inherits
the *ideas*, not MPL obligations. See [comparisons.md](comparisons.md) for the
Taffy relationship, and [governance.md](governance.md) for stewardship details.

## Implications for Buiy (summary)

- Treat Servo as a parts catalogue of validated *components*, not a finished
  browser. Borrow architecture; expect incompleteness.
- Under-resourcing is structural for CSS-faithful engines; Buiy's bounded
  typed-subset plus build-above-Taffy stance is the correct hedge.
- Layout-architecture churn is real even for the experts; not owning the solver
  (delegating to Taffy) sidesteps Servo's biggest rewrite.
- Honor the license boundary: MPL-2.0 source is for reading, not copying into
  Buiy's MIT/Apache codebase.

## Sources

- Servo origin / 2012 / experimental status: https://en.wikipedia.org/wiki/Servo_(software) , https://servo.org/about/ , https://blogs.igalia.com/mrego/servo-a-new-web-engine-written-in-rust/
- Mozilla August 2020 layoffs (~250 / ~25%): https://mjtsai.com/blog/2020/08/11/mozilla-layoffs/ , https://news.ycombinator.com/item?id=36095379
- Linux Foundation Europe (Sept 2023) + Igalia stewardship (~5 engineers): https://www.igalia.com/2023/09/07/The-Servo-project-is-joining-Linux-Foundation-Europe.html , https://blogs.igalia.com/mrego/servo-revival-2023-2024/
- Funding (Open Collective ~$59k 2025, +62%; Sovereign Tech Fund): https://opencollective.com/servo , https://www.igalia.com/2025/10/09/Igalia,-Servo,-and-the-Sovereign-Tech-Fund.html
- Layout 2013 vs 2020 rewrite: https://servo.org/blog/2023/04/13/layout-2013-vs-2020/ , https://github.com/servo/servo/wiki/Layout-2020 , https://book.servo.org/design-documentation/layout.html
- Stylo / selectors / cssparser crates: https://github.com/servo/stylo , https://crates.io/crates/stylo , https://crates.io/crates/selectors , https://crates.io/crates/cssparser
- License (MPL-2.0): https://github.com/servo/servo/blob/main/LICENSE
- Buiy specs: [../../specs/2026-05-08-buiy-layout-design/README.md](../../specs/2026-05-08-buiy-layout-design/README.md) , [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling prior-art: [../taffy/](../taffy/) , [../bevy-ui/](../bevy-ui/) , [../dioxus/](../dioxus/)
