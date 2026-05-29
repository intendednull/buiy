**Date:** 2026-05-29
**Status:** active
**Subject:** Servo / Stylo — folder index: the Rust browser-engine prior art (Servo engine + Stylo style system + WebRender), why it is load-bearing for Buiy, and which file answers which design question.

# Servo / Stylo

Servo is an experimental, embeddable web engine written in Rust, begun at Mozilla Research in 2012 and co-evolved with the Rust language; its two most durable outputs — **Stylo**, the parallel CSS style system, and **WebRender**, the GPU display-list renderer — were upstreamed into Firefox (Quantum CSS in Firefox 57, 2017-11-14; WebRender from Firefox 67, 2019-05-21) while the integrated browser stayed research-grade. After Mozilla laid off the Servo team in August 2020, custody passed to the Linux Foundation and, after a near-dormant 2021–2022, development was revived in 2023 by **Igalia** under Linux Foundation Europe (joined 2023-09-07). Servo, Stylo, and WebRender are **MPL-2.0**.

## For Buiy

Buiy is a retained-mode UI library for Bevy, built parallel to `bevy_ui`, implementing a *typed-Rust subset* of named W3C CSS modules (Display 3, Positioned Layout, Containment 3, Writing Modes 4, Anchor Positioning 1) on a fixed substrate: Taffy for box layout, `cosmic-text` for text, AccessKit for accessibility, wgpu via Bevy's render graph. This folder exists because **browser engines are the reference *implementations* of those W3C modules**, and Servo is the Rust one — the existence proof that a memory-safe, parallel CSS resolution and layout stack ships in production Rust. Critically, Servo's substrate (Stylo for style; Taffy for flex/grid layout via the `stylo_taffy` adapter; and the downstream Blitz project pairing Stylo + Taffy + Parley + Vello) is *nearly Buiy's substrate minus Bevy/ECS*, which makes Servo a closer relative than its "never shipped a browser" reputation suggests. The other load-bearing implementation reference is Blink (`../blink/`), the canonical engine: Blink answers *what* the CSS semantics are; Servo answers *how* to express them in safe Rust. Servo's `display_list/stacking_context.rs` (a sorted `StackingContextTree` handed to render) is the single closest prior art to Buiy's **NEXT** Phase 9 sub-pass 6f.

## Honest assessment

- **Servo is not a shipping browser and never was.** After 14 years it remains "experimental." The wins (Stylo, WebRender) were *extracted into Gecko*; the integrated engine stalled. Treat Servo as a parts catalogue of validated components, not a finished product.
- **The brief's "Servo does NOT use Taffy" is wrong.** Servo's current layout owns its *own* block / inline / table / float algorithms but delegates **flexbox and CSS grid to Taffy** via `components/layout/taffy/` and the `stylo_taffy` adapter (CSS-grid WPT pass rate jumped ~18.6%→38.3% in PR #32619). Servo embeds Taffy for two formatting contexts; Buiy stacks passes above whole-tree Taffy. (See [layout.md](layout.md).)
- **MPL-2.0 diverges from Buiy's MIT OR Apache-2.0.** Stylo/Servo/WebRender are reading-and-citing references, not vendoring sources. This is a direct reason Buiy reimplements a typed-CSS subset rather than depending on `stylo`.
- **Under-resourcing is structural.** ~5 Igalia engineers; donations (~$33.6k in 2024, ~$59k project-reported in 2025) fund CI, not salaries. A handful of engineers cannot close a decade-plus feature gap against Blink/Gecko — and don't claim to.
- **Bus-factor shifted, not eliminated.** Mozilla was a single corporate point of failure; Igalia is now a single-consultancy point of failure for the *paid* engineering. The 2020→2023 dead period shows the failure mode is real. Igalia authored only 26% of 2024 merged PRs (40% other humans, 34% bots) — the largest single contributor, not a numeric majority.
- **Parallel layout — the founding thesis — never fully landed.** Parallel *style* (Stylo's `rayon` cascade) decisively succeeded; "lay out the whole page in parallel" turned out far harder and was part of why layout 2013 was thrown away. Spec fidelity beat parallelism.
- **No top-layer / `popover` / `::backdrop` story comparable to Buiy's `TopLayer`.** Buiy's top-layer escape hatch has *no direct Servo analogue to cite* — Blink is the reference there.
- **Accessibility is thin.** A11y was deferred; AccessKit emerged from the broader Rust GUI ecosystem, not Servo. This is a place Buiy deliberately exceeds its browser prior art.

## Key facts (verified 2026-05)

| Fact | Value | Source |
|---|---|---|
| Origin | Mozilla Research, 2012; first large non-compiler Rust codebase | Wikipedia; servo.org/about |
| Mozilla layoffs (Servo team cut) | 2020-08-11 (~250 staff, ~25% of workforce) | gHacks |
| Custody → Linux Foundation | 2020 (custodial only, *not* re-funding; *not* LF Europe) | servo.org |
| Revival under Igalia | reactivation announced 2023-01-16; joined LF Europe 2023-09-07 | igalia.com; linuxfoundation.eu |
| Igalia share of 2024 merged PRs | 26% of PRs (40% other humans, 34% bots); separately, 679 commits | servo.org 2024 report |
| Quantum CSS (Stylo) ships in Firefox | Firefox 57, 2017-11-14 | MDN |
| WebRender ships in Firefox | Firefox 67, 2019-05-21 (narrow ~4% rollout; full ~Firefox 92, 2021) | mozillagfx; bugzilla |
| `stylo` crate | v0.17.0, repo `github.com/servo/stylo`, ~129K downloads | crates.io |
| `selectors` crate | v0.38.0, repo `github.com/servo/stylo`, ~44M downloads | crates.io |
| `cssparser` crate | v0.37.0, repo `github.com/servo/rust-cssparser`, ~51M downloads | crates.io |
| Standalone `servo/stylo` repo created | 2024-02-07 | github.com/servo/stylo |
| Layout engine status | layout 2013 default-off behind feature flag (2024, PR #32759); legacy fully removed 2025 (PR #35943); crates merged into `layout` (PR #36613) | servo PRs |
| Layout uses Taffy? | **Yes** — for flexbox + CSS grid via `stylo_taffy`; own block/inline/table/float | components/layout/taffy/ |
| Servo first numbered release | v0.0.1, 2025-10 (Apple-silicon support) | servo.org |
| License | MPL-2.0 (Servo, Stylo, WebRender) — diverges from Buiy MIT/Apache | github.com/servo/servo |
| Blitz substrate | Stylo + Taffy + Parley + Vello (via `AnyRender`); MIT/Apache | github.com/DioxusLabs/blitz |

## Contents

| File | Subject |
|---|---|
| [README.md](README.md) | This index: what Servo/Stylo is, honest assessment, key facts, how to use the folder. |
| [lessons.md](lessons.md) | **The decision file.** Validates / Avoid / Borrow for Buiy mechanisms (`StackingContext.painters_z`, `TopLayerActivation`, Containment SIZE-zeroing, `PostTaffyOverrides`, Taffy-above-passes, license). |
| [architecture.md](architecture.md) | Process/thread model (constellation, script thread, layout, compositor, WebRender); style/layout/paint split; layout-2013→current rewrite; embedding surface (`libservo`, servoshell, Verso, Tauri). |
| [stylo.md](stylo.md) | The parallel Rust CSS cascade: `rayon` work-stealing, the rule tree, style sharing cache, Bloom-filter ancestor matching, `ComputedValues`, the crate split, Quantum CSS, Blitz. |
| [layout.md](layout.md) | Layout's box-tree/fragment-tree split, formatting contexts as nested enums, own inline+table layout, **Taffy for flex/grid**, `StackingContextTree` + paint order, implemented-vs-missing. |
| [rendering.md](rendering.md) | WebRender pipeline (display list → retained scene → frame builder → batching → single-pass GPU raster); Firefox upstreaming; costs (driver sensitivity, `swgl`, off-screen passes); wgpu/Vello/Blitz; text stack. |
| [governance.md](governance.md) | Who-owns-what / who-pays: Mozilla origin, 2020 layoffs, two-step LF→LF-Europe move, Igalia stewardship, TSC, funding, MPL-2.0, bus-factor. |
| [history.md](history.md) | Dated timeline 2012→2025: origin, Project Quantum, Quantum CSS, WebRender, layoffs, custody, revival, layout consolidation, funding, v0.0.1, the Blitz parallel track. |
| [critiques.md](critiques.md) | Unflattering facts: not a shipping browser, under-resourcing, layout-rewrite churn, component-success-masks-integration-debt, license divergence. |
| [open-problems.md](open-problems.md) | What Servo has not solved: WPT completeness, inline/fragmentation/pagination, parallel layout not realized, stable embedding API, accessibility, sustainability. |
| [comparisons.md](comparisons.md) | Servo vs Blink (canonical), Gecko (downstream consumer), Taffy (Buiy's substrate), Buiy itself, and Blitz (the near-miss Buiy substrate). |

## How to use this prior-art doc

1. **Designing Phase 9 (stacking + top-layer, sub-pass 6f)?** Start at [lessons.md](lessons.md) "Avoid"/"Borrow", then [layout.md](layout.md) §5 — which now reconciles Servo's 4-bucket `StackingContextSection` with CSS 2.1 Appendix E's ~7 steps, settles **z-index ties** (Servo's stable `sort_by_key` = document-order tiebreak), walks the **Phase 8 → 9 transform/opacity seam** (a transformed element forms a stacking context *and* becomes the containing block for fixed descendants), and notes there is **no published cost figure** for the tree build. Then [rendering.md](rendering.md) "effects force off-screen passes" and "Hit-test order is paint order, reversed." Servo's sorted-tree-handed-to-render is the closest model for `StackingContext { painters_z: Vec<Entity> }`. **Top-layer:** this folder covers only the z-index/stacking *half* of 6f — Servo has no `popover`/`::backdrop`/`TopLayer` analogue, so go to `../blink/stacking-and-paint.md` for `TopLayer`, `TopLayerActivation`, and overflow-clip escaping.
2. **Validating the Stylo+Taffy substrate bet?** Read [layout.md](layout.md) §1 (Servo embeds Taffy for flex/grid) and [stylo.md](stylo.md) (parallel cascade shipped in Firefox) plus [comparisons.md](comparisons.md) §5 (Blitz). Two production engines pairing Stylo+Taffy is the strongest evidence the Taffy choice is load-bearing.
3. **Deciding whether to depend on `stylo` / vendor any Servo code?** Read [governance.md](governance.md) "License" and [stylo.md](stylo.md) "What Quantum CSS proved (and the licensing catch)". MPL-2.0 answer: read and cite, never vendor.
4. **Designing the render handoff (Phase 8 `ResolvedTransform`, Phase 9 `painters_z`)?** Read [rendering.md](rendering.md) (display list → retained scene → batch) — the existence proof that "layout writes a paint description, render reads it" scales.
5. **Designing the layout pipeline shape / box-vs-result separation?** Read [architecture.md](architecture.md) (style→layout→display-list contract; the 2013→2020 immutable-fragment-tree lesson).
6. **Scoping the layout effort / writing the case for a bounded subset?** Read [critiques.md](critiques.md) (under-resourcing, rewrite churn) and [open-problems.md](open-problems.md) (which corners are genuinely hard: inline, fragmentation, tables).
7. **Reasoning about Buiy's dependency-survival strategy?** Read [governance.md](governance.md) and [history.md](history.md) (components outlive engines when they enter a host product; custody ≠ funding).
8. **Comparing against the canonical engine or Buiy's actual substrate?** Read [comparisons.md](comparisons.md) (Blink/Gecko/Taffy/Buiy/Blitz) and cross-link `../blink/`, `../taffy/`, `../dioxus/`.

## Framing disclosure

This corpus is written from Buiy's design stance: a retained-mode UI library built *parallel to `bevy_ui`*, implementing a *CSS-faithful typed-Rust subset above Taffy* (never forking Taffy), with `cosmic-text` for text and an **AccessKit-first, WCAG 2.2 AA** accessibility floor, under MIT OR Apache-2.0. Every "Implications for Buiy" / "Relevance to Buiy" sub-section in these files reflects that bias *by design* — it reads Servo/Stylo through what is and isn't useful to Buiy, not as a neutral engine survey. Where Servo's choices diverge from Buiy's (owning its block/inline/table/float solver while embedding Taffy only for flex/grid, the CSS cascade and selector machinery, MPL-2.0, parallel-layout ambition, deferred accessibility), that divergence is treated as informative, sometimes as inverted prior art (e.g. Stylo's cascade machinery is cited to *justify not having one*). Read these files as a Buiy-builder's annotated reference, and consult the cited primary sources for the unbiased picture.

## Sources

- Servo origin, status, MPL-2.0: https://en.wikipedia.org/wiki/Servo_(software) ; https://github.com/servo/servo ; https://servo.org/about/
- Mozilla August 2020 layoffs: https://www.ghacks.net/2020/08/11/mozilla-lays-off-250-employees-in-massive-company-reorganization/
- Igalia revival + Linux Foundation Europe (2023-09-07): https://www.igalia.com/2023/09/07/The-Servo-project-is-joining-Linux-Foundation-Europe.html ; https://linuxfoundation.eu/newsroom/servo-web-rendering-engine-joins-linux-foundation-europe ; https://servo.org/blog/2025/01/31/servo-in-2024/
- Quantum CSS / Firefox 57: https://developer.mozilla.org/en-US/docs/Mozilla/Firefox/Releases/57 ; https://hacks.mozilla.org/2017/08/inside-a-super-fast-css-engine-quantum-css-aka-stylo/
- WebRender / Firefox 67: https://mozillagfx.wordpress.com/2019/05/21/graphics-team-ships-webrender-mvp/
- Crates: https://crates.io/crates/stylo ; https://crates.io/crates/selectors ; https://crates.io/crates/cssparser
- Layout uses Taffy + legacy removal: https://github.com/servo/servo/blob/master/components/layout/taffy/mod.rs ; https://github.com/servo/servo/pull/32619 ; https://github.com/servo/servo/pull/35943 ; https://github.com/servo/servo/pull/36613
- Blitz: https://github.com/DioxusLabs/blitz
- Buiy specs: [../../specs/2026-05-08-buiy-layout-design/README.md](../../specs/2026-05-08-buiy-layout-design/README.md) ; [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md) ; [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling prior-art: [../blink/](../blink/) ; [../taffy/](../taffy/) ; [../dioxus/](../dioxus/) ; [../bevy-ui/](../bevy-ui/) ; [../cosmic-text/](../cosmic-text/) ; [../xilem-masonry/](../xilem-masonry/) ; [../rmlui/](../rmlui/) ; [../coherent-gameface/](../coherent-gameface/)
