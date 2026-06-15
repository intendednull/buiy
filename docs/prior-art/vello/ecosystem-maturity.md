**Date:** 2026-06-14
**Status:** active
**Subject:** What Vello is and isn't — three variants/maturity levels, Parley pairing, adoption, versions, governance

# Ecosystem & maturity

## What Vello is (and isn't)

Vello is "A GPU compute-centric 2D renderer" — a **renderer, not a UI toolkit**. It consumes a scene (paths, fills, gradients, glyph runs, clips, blends) and rasterizes it. It has **no widget tree, no layout, and no accessibility** — a11y lives entirely in the consuming framework (Masonry/Xilem via AccessKit), never in Vello. The lineage is **piet-gpu → piet-gpu-hal → Vello**; Raph Levien retired the bespoke `piet-gpu-hal` HAL in favor of `wgpu` ("Requiem for piet-gpu-hal").

This "renderer with no a11y by design" boundary is itself a data point for Buiy: Vello deliberately keeps a11y *out* of the renderer. Buiy's decomposed a11y components live above the render layer for the same reason.

## Three variants, three maturity levels

Per [DeepWiki architecture](https://deepwiki.com/linebender/vello/1.1-architecture) (wording cross-checked against Linebender blogs):

| Variant | Pipeline | Status | Requires |
|---|---|---|---|
| `vello` (GPU compute) | flagship; prefix-sum `flatten → binning → coarse → fine` | **alpha** (README verbatim) | WebGPU **compute** support |
| `vello_hybrid` | CPU path-processing + GPU fragment-shader rasterization of sparse strips; "targets WebGL2 and resource-constrained GPUs" | **Experimental** | any GPU (incl. web) |
| `vello_cpu` | pure-CPU sparse-strip rasterizer, "for devices without GPU support or debugging purposes" | **Alpha** | none |

The README states Vello "can currently be considered in an alpha state" and "the web is not currently a primary target … WebGPU implementations are incomplete, so you might run into issues" (verbatim). **There is no 1.0, and no stable-release roadmap was found in primary sources — mark the 1.0 timeline as unverified.**

Note the framing nuance that matters for Buiy: DeepWiki positions `vello_cpu` "for debugging purposes," **not** as an authoritative *reference oracle*. So Buiy's plan to promote its own CPU SDF port to a first-class oracle goes *beyond* what Vello formally claims for `vello_cpu`. The basis for that stronger claim is narrow: Buiy's CPU and GPU paths share one analytic function, so their agreement is a durable invariant against implementation drift — but, as [`lessons.md`](lessons.md) records, a bug in the shared SDF itself stays invisible to the cross-check, so the oracle does not replace Buiy's golden/reftest tiers. See [`lessons.md`](lessons.md).

## Ecosystem pairing — Parley

Vello pairs with **Parley** (rich-text layout) as the text companion. Parley v0.10.0 (2026-06-01) switched shaping to **HarfRust** (Google Fonts' Rust HarfBuzz port), replacing Swash, in the Masonry/Xilem 0.4 release (October 2025). Xilem (SwiftUI-inspired) renders via Vello atop Masonry's retained widget tree. (Buiy uses cosmic-text rather than Parley — both shape via harfrust, so the shaper substrate converges even though the layout API diverges; see [`../xilem-masonry/text-and-rendering.md`](../xilem-masonry/text-and-rendering.md).)

## Adoption

Confirmed Vello consumers:

- **Xilem / Masonry** — Linebender's own GUI stack.
- **`bevy_vello`** — third-party Bevy integration; v0.6.0 upgraded to wgpu v26 for Bevy 0.17.
- **`woodpecker_ui`** — StarArawn's Bevy ECS UI crate uses Vello.

**Important correction on the Bevy rumor:** Bevy migrated its *text* stack to **Parley** (replacing cosmic-text), merged 2026-02-11, targeting **Bevy 0.19** ([bevy#21767](https://github.com/bevyengine/bevy/issues/21767), [PR #22879](https://github.com/bevyengine/bevy/pull/22879)). That PR is **Parley-only — it does NOT adopt Vello** as Bevy's renderer (verified: "makes no mention of Vello"). Do not propagate the "Bevy adopted Vello" rumor.

## Versions (crates.io API `created_at` — authoritative)

Note: a GitHub-releases HTML scrape misreported several of these as 2025; the crates.io API timestamps are authoritative.

- **`vello`** (GPU compute): **0.9.0** (2026-05-15); 0.8.0 (2026-03-20); 0.7.0 (2026-01-13); 0.6.0 (2025-10-03). 0.9.0 moved to wgpu v29, peniko 0.6.1, skrifa 0.42. First published 2024-03-04.
- **`vello_cpu` / `vello_hybrid` / `vello_common`**: still **0.0.x** — latest **0.0.9** (2026-05-30); 0.0.8 (2026-05-15); 0.0.7 (2026-03-24). The sub-1.0 `0.0.x` versioning is an explicit "do not depend on stability" signal.

## Numbers (crates.io / GitHub, verified 2026-06-14)

| Metric | Value |
|---|---|
| `vello` latest version | 0.9.0 (2026-05-15) |
| `vello` first published | 2024-03-04 |
| `vello` total downloads | 384,404 (178,450 recent) |
| `vello` GitHub stars | ~4.1k (point-in-time, approximate) |
| `parley` latest version | 0.10.0 |
| `parley` total downloads | 1,285,183 (≫ vello — Parley is useful standalone) |
| `parley` GitHub stars | ~622 (approximate) |

The Parley-≫-Vello download gap reflects the substrate-vs-framework adoption split Linebender exhibits generally: standalone substrate crates outpace the renderer that ties them together.

## Governance & funding

Linebender is an **informal volunteer collective** — "a group of volunteers and enthusiasts who hang out on our Zulip," with all work done in the open and decisions emerging from community discussion rather than a formal hierarchy ([linebender.org/about](https://linebender.org/about/)). **Raph Levien** founded it and "informally leads and drives the work forward" (SIMD, stroke expansion, new rendering approaches).

**Funding (uncertain):** the About page lists none, but blog history records **Google** and **Google Fonts** sponsorship of ecosystem work (Xilem/Masonry/Vello). Exact current dollar figures and sponsor terms could not be pinned to a primary source — **treat the funding specifics as unverified beyond "Google / Google Fonts have sponsored Linebender work."**

## Implications for Buiy

Vello is a moving target: flagship at 0.9.0 alpha, sparse-strips crates at `0.0.x`, no 1.0 in sight. This reinforces the [`lessons.md`](lessons.md) Avoid row — **study Vello's testing pattern, do not take a runtime dependency on `vello` / `vello_cpu`.** The capability set (anti-aliased path fill, gradients in arbitrary color spaces, blur, blend, arbitrary clip-path) is worth modeling as a render-pipeline *target*, but Buiy's renderer is independent.

## Sources

- DeepWiki architecture: https://deepwiki.com/linebender/vello/1.1-architecture
- Vello README: https://github.com/linebender/vello
- crates.io API: https://crates.io/api/v1/crates/vello/versions
- Parley repo: https://github.com/linebender/parley
- bevy#21767 / PR #22879 (Bevy → Parley, NOT Vello): https://github.com/bevyengine/bevy/issues/21767 , https://github.com/bevyengine/bevy/pull/22879
- woodpecker_ui: https://github.com/StarArawn/woodpecker_ui
- Linebender about / governance: https://linebender.org/about/
- "Requiem for piet-gpu-hal": https://github.com/raphlinus/raphlinus.github.io/issues/86
