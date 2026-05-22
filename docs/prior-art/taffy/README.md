**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — Rust layout engine (Flexbox + CSS Grid + Block + Float); load-bearing dependency of Buiy's layout subsystem

# Taffy

Taffy is a small (~15k LoC), pre-1.0 Rust crate that computes CSS-shaped layout: it walks a tree of styled nodes and produces a tree of rectangles (`Layout { location, size, content_size, border, padding, scrollbar_size, order }`). It is *only* a layout engine — it owns no rendering, no text shaping, no input, no styling-system. The embedder feeds it a `Style` per node and a measure function for leaves; Taffy hands back rectangles. The crate is the third generation of a single Rust flexbox engine (Stretch → stretch2 → Taffy) and is now the layout engine for Bevy UI, Servo, Blitz (Dioxus Native), Zed (via GPUI), Lapce (via Floem), Slint, and a long tail of niche renderers.

Buiy treats Taffy as a load-bearing dependency. Buiy's [layout architecture](../../specs/2026-05-08-buiy-layout-design/architecture.md) wraps `TaffyTree` in a `NonSendResource` and bridges via a hybrid `Style` builder that expands into ~15 decomposed per-property components (`Display`, `Position`, `LogicalBoxModel`, `FlexParams`, `GridParams`, `Overflow`, `WritingMode`, `Container`, `Anchor`, `Stacking`, `Transform`, `Containment`, `MultiColumn`, `Scroll`, `Children/ChildOf`). An 8-step per-frame pipeline (`RemovedNodesGc → SyncStyles → CqActivate → TaffyCompute → CqFlipCheck → PostTaffyOverrides → WriteResolvedLayout`) keeps Taffy's frame-to-frame cache warm and layers the features Taffy doesn't ship (anchor positioning, container queries, sticky, tables, multi-column) as Buiy-owned passes around it. This folder is the version-pinned reference future Buiy spec authors should consult when designing against Taffy.

## Honest assessment

**Strong points.**

- **Production-proven breadth.** Taffy is the only Rust-native layout engine that ships Flexbox + CSS Grid + Block + Float in one crate. Yoga is Flexbox-only. The breadth is the load-bearing reason Buiy chose Taffy over Yoga-via-bindings.
- **Multi-embedder discipline.** The high-level `TaffyTree` API and the low-level `LayoutPartialTree` / `TraversePartialTree` / `CacheTree` trait surface are both first-class. Servo, Blitz, GPUI, and Slint implement the traits against their own node arenas; Bevy UI and Buiy wrap `TaffyTree`. One algorithm, two entry points.
- **Const-construction discipline.** `Style::DEFAULT` is const; `length()`, `percent()`, `auto()`, `zero()` are `const fn`. Static styles cost nothing at runtime. Validates Buiy's `Style` builder design.
- **WPT-derived test corpus.** The crate ships 1500+ generated tests plus thousands more imported from CSS WG, Chromium, Firefox, and WebKit suites. The pass-rate is not advertised numerically (see [critiques.md § 5](critiques.md)), but the coverage is meaningfully wider than Yoga.
- **Spec-tracking discipline.** When Chromium changes layout behavior, Taffy ports the fix. The lag is months, but the treadmill is real.

**Bus-factor risk.** The day-to-day maintainer is **Nico Burns** — independent, not employed by DioxusLabs, supported by sporadic personal GitHub Sponsors. He authored CSS Grid, the trait restructure, `CacheTree`, `CompactLength`, named grid lines, float, and direction. His absence would visibly stall the project. The repo is GitHub-admin-owned by DioxusLabs (VC-backed via Series A from FutureWei + Khosla Ventures), but Taffy has no separate budget line; the funding goes to Dioxus. Crate ownership is shared across three names (Burns, Alice Cecile, Jonathan Kelley), so the crate cannot be lost to account-death. The mitigating factor: Bevy + Blitz + Servo are large enough downstreams that *somebody* would fork in a crisis. Buiy's contingency: fork Taffy (it's MIT-licensed); ramp cost is months.

**Stuck features.** Three features have been "coming" for years with no PR:

- **Subgrid** ([#468](https://github.com/DioxusLabs/taffy/issues/468)) — open since **2023-04-24**. Conflicts with Taffy's strict parent-to-child traversal in `LayoutPartialTree`. Browsers shipped subgrid in 2022-2023; Taffy lags 3+ years. No design doc.
- **Masonry** ([#910](https://github.com/DioxusLabs/taffy/issues/910)) — open since **2026-01-05**. Burns himself notes it depends on subgrid landing first.
- **Anchor positioning** ([#703](https://github.com/DioxusLabs/taffy/issues/703)) — open since **2024-08-03**. Not on the active roadmap. Anchor positioning is a cross-tree post-layout dependency, fundamentally a different primitive from Taffy's intra-formatting-context algorithms.

These three are why Buiy ships container queries and anchor positioning **above Taffy** as Buiy-owned passes (rather than waiting on upstream) and why Buiy reserves stub API + `warn!` for subgrid and masonry.

**README inaccuracy.** Taffy's own README claims Iced uses Taffy. Verified false — Iced has its own per-widget `Widget::layout` protocol with no `taffy` dependency in `iced/Cargo.toml`. Future readers verifying downstream-user claims should consult each project's actual `Cargo.toml`, not Taffy's marketing list. The actual major Dioxus-org consumer is **Blitz**, the browser engine the Dioxus Native target runs on.

## Key facts

| Fact | Value | Source |
|---|---|---|
| Crate name | `taffy` | crates.io |
| Latest stable | `0.10.1` (release 2026-04-14, metadata last updated 2026-05-15) | [crates.io](https://crates.io/crates/taffy) |
| Experimental | `0.11.0-experimental-cache-fix.3` (do not depend on; for Blitz cache-correctness work) | [history.md § 4](history.md) |
| Repo | https://github.com/DioxusLabs/taffy | — |
| License | **MIT** (single-license, *not* dual MIT/Apache-2.0 — unusual for the Rust ecosystem) | [`Cargo.toml`](https://github.com/DioxusLabs/taffy/blob/main/Cargo.toml) |
| MSRV | Rust 1.71 (since 0.10) | [`Cargo.toml`](https://github.com/DioxusLabs/taffy/blob/main/Cargo.toml) |
| Total downloads | 7,250,881 (recent 90d: 2,017,439) | crates.io |
| Crate owners | Jonathan Kelley (DioxusLabs), Alice Cecile (Bevy UI), Nico Burns (independent — day-to-day maintainer) | crates.io owners endpoint |
| Cargo.toml typo | author listed as "Johnathan Kelley"; correct spelling is Jonathan Kelley | [`Cargo.toml`](https://github.com/DioxusLabs/taffy/blob/main/Cargo.toml) |
| Steward | DioxusLabs (GitHub admin); Series A VC-backed from FutureWei + Khosla — funding is for Dioxus, not Taffy | dioxuslabs.com |
| Algorithms | Flexbox · CSS Grid · Block · Float (all default-on; each feature-gated) | [layout-algorithms.md](layout-algorithms.md) |
| `Display` variants | `Block | Flex | Grid | None` only — no inline-*, no table*, no list-item, no contents, no flow-root, no ruby | [api.md § 5](api.md) |
| `Position` variants | `Relative | Absolute` only — no Static, Fixed, Sticky | [api.md § 5](api.md) |
| `Overflow` variants | `Visible | Clip | Hidden | Scroll` — no `Auto` | [api.md § 5](api.md) |
| `Direction` | `Ltr | Rtl` (since 0.10); vertical writing modes open in [#752](https://github.com/DioxusLabs/taffy/issues/752) | [layout-algorithms.md § 6](layout-algorithms.md) |
| Block layout shipped | **0.4.0** (2024-02-13) | [history.md § 4](history.md) |
| Float + Clear shipped | **0.10.0** (2026-03-31) behind `float_layout` feature | [layout-algorithms.md § 4](layout-algorithms.md) |
| `CompactLength` (tagged pointer) | **0.8.0** (2025-04-01) — `Style` became `!Send + !Sync` | [architecture.md § 8](architecture.md) |
| `CacheTree` trait split | **0.7.0** (2024-12-12) | [architecture.md § 2](architecture.md) |
| `Style::DEFAULT` + `length()` etc. | `const`-constructible | [api.md § 6](api.md) |
| Bevy UI adopted Taffy | Bevy **0.9** (December 2022) — *not* Bevy 0.10 | [integration.md § 2](integration.md) |
| Top production users | Bevy (UI), Servo (layout engine), Blitz (Dioxus Native), Zed (GPUI), Lapce (Floem), Slint | [ecosystem.md § 1](ecosystem.md) |
| Iced uses Taffy? | **No** — Taffy README claim is stale | [ecosystem.md § 1](ecosystem.md), [integration.md § 4](integration.md) |
| WPT corpus | 1500+ generated tests + thousands imported from CSS WG / Chromium / Firefox / WebKit; pass-rate not published | [ecosystem.md § 3](ecosystem.md) |

## Contents

- [README.md](README.md) — this file: overview, honest assessment, key facts, contents index, framing disclosure.
- [architecture.md](architecture.md) — the two-API split (`TaffyTree` vs traits), the five-trait stack, storage model, cache strategy, available-space resolution, the four algorithm modules, and what Taffy explicitly does NOT model.
- [layout-algorithms.md](layout-algorithms.md) — Flexbox, CSS Grid (including subgrid + masonry status), Block, Float coverage. The `Display` enum is `{Block, Flex, Grid, None}`; everything else is the embedder's job. Writing modes are LTR/RTL only. Container queries + anchor positioning are not in Taffy.
- [api.md](api.md) — Public surface: `TaffyTree` methods, the low-level trait approach, the `Style` struct, value types (`Length` / `LengthPercentage` / `LengthPercentageAuto` / `Dimension` / `CompactLength`), const construction, `TaffyError`, `parse` feature.
- [capabilities.md](capabilities.md) — Feature-by-feature gap matrix: what CSS Taffy implements, what it doesn't, and how Buiy fills each gap. The three buckets: "algorithmically required but unimplemented" (Buiy layers above), "painting concerns" (Buiy render pipeline owns), "runtime concerns" (Buiy input/render layers own).
- [integration.md](integration.md) — How embedders integrate: wrap-`TaffyTree` vs implement-the-traits. Bevy UI's `UiSurface`; Dioxus/Blitz's trait-against-DOM approach; Iced does NOT use Taffy; the Buiy 8-step pipeline; hazards visible only after integration (stale-cache, `Length(0.0)` vs `Auto`, children-ordering, `NodeId` vs `Entity`).
- [history.md](history.md) — Lineage from Stretch (Emil Sjölander @ Visly Inc., 2018–2020) through stretch2 (Jonathan Kelley fork, March 2022) to Taffy 0.1.0 (rename, 2022-06-10). Per-release table from 0.1 to 0.10.1.
- [governance.md](governance.md) — DioxusLabs as steward; Nico Burns as the load-bearing maintainer; three crate owners; no `CODEOWNERS` file; no funding line for Taffy; bus factor of one.
- [ecosystem.md](ecosystem.md) — Top reverse-dependencies; Yoga comparison; WPT corpus discipline; tooling (`stylo_taffy`, `egui_taffy`, `compose-taffy`); production deployments.
- [critiques.md](critiques.md) — Subgrid + masonry years-open status; performance critiques (wide-tree case underperforms Yoga ~1.8×); API ergonomics (the `Length`/`LengthPercentage`/`LengthPercentageAuto`/`Dimension` split, post-0.8 tagged-pointer inspection, no inheritance modeled, measure-function API churn); long-promised missing features; documentation gaps; the spec-tracking treadmill.
- [open-problems.md](open-problems.md) — Per-feature catalog of gaps: subgrid, masonry, anchor positioning, container queries, writing modes, calc, float, inline, styling system, sticky, scroll snap, aspect-ratio, block gap, `display: contents`/`flow-root`/`list-item`/`ruby`/`table*`, intrinsic-size keywords, multi-column. Summary table at the end.
- [lessons.md](lessons.md) — **The consult-this-when-designing decision file.** Validates / Avoid / Borrow.
- [glossary.md](glossary.md) — System-specific terms (`TaffyTree`, `LayoutPartialTree`, `CompactLength`, `AvailableSpace`, `MeasureFunc`, `WPT`, `Stretch`, `Blitz`, `Yoga`, etc.).

## How to use this prior-art doc

When designing a Buiy feature that touches layout:

1. Start in [lessons.md](lessons.md). It enumerates which Buiy design decisions Taffy already validates, which Taffy pitfalls Buiy has explicit mitigations for, and which Taffy primitives are worth borrowing.
2. If lessons references a specific subsystem, read that subsystem's file (`architecture.md`, `api.md`, `layout-algorithms.md`, `integration.md`, `open-problems.md`).
3. If you're investigating whether Taffy can or should ship a CSS feature, [capabilities.md](capabilities.md) and [open-problems.md](open-problems.md) have the per-feature status with linked Taffy issues.
4. If you're worried about a maintenance/governance question (license, bus factor, funding, fork strategy), read [governance.md](governance.md) and [history.md](history.md).
5. Promote any decision that affects Buiy into a Buiy spec under `docs/specs/`. This folder captures what we learn from Taffy; it does not encode Buiy's own decisions.

**Framing disclosure.** These docs are written from a **"Buiy commits to Taffy as the layout substrate"** stance — most "Buiy mitigation" / "Buiy posture" lines treat Taffy gaps as Buiy's responsibility-above-Taffy, not as reasons to reconsider Taffy itself. Future readers auditing whether *Taffy* is the right primitive — vs forking it, vs writing a Buiy-native layout engine, vs binding to Yoga, vs the next Rust-native contender — should weigh the corpus accordingly. The load-bearing-dependency framing under-emphasizes Taffy alternatives and over-emphasizes gap-filling-above-Taffy. The corpus is a learn-from-Taffy-into-Buiy artifact, not a neutral catalog of layout-engine choices.

## Sources

- Taffy repo: https://github.com/DioxusLabs/taffy
- Taffy on crates.io: https://crates.io/crates/taffy
- Taffy CHANGELOG: https://github.com/DioxusLabs/taffy/blob/main/CHANGELOG.md
- Buiy layout design spec: [`docs/specs/2026-05-08-buiy-layout-design/`](../../specs/2026-05-08-buiy-layout-design/)
- Bevy UI source (the comparator embedder): https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui
- Blitz (the actual major Dioxus-org Taffy consumer): https://github.com/DioxusLabs/blitz
- Stretch repo (Visly, 2018-2020, defunct): https://github.com/vislyhq/stretch
- Yoga (Facebook, the Flexbox-only C++ peer): https://github.com/facebook/yoga
- Per-file `## Sources` sections cite the specific URLs each file relies on.
