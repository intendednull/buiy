**Date:** 2026-06-14
**Status:** active
**Subject:** Vello — Linebender GPU-compute 2D renderer with a CPU reference rasterizer (the closest greenfield neighbor to Buiy's CPU-SDF-oracle plan)

# Vello

Vello is "A GPU compute-centric 2D renderer" (the verbatim crate description on [crates.io](https://crates.io/crates/vello)) — the rasterization engine of the Linebender ecosystem, sibling to the Xilem/Masonry/Parley stack. Unlike a traditional rasterizer that hangs work off the GPU's fixed-function rasterizer, the flagship `vello` crate runs nearly the entire pipeline as a chain of WGSL **compute** shaders over `wgpu`. To backstop the resulting GPU-compute-portability problems, Linebender built a second-generation **sparse strips** family — `vello_cpu` (pure software) and `vello_hybrid` (CPU strip generation + GPU fine rasterization) — sharing one scene format. That CPU/GPU split, and the test harness that diffs the two against each other with a perceptual metric, is exactly the pattern Buiy is reaching for: promoting its own CPU SDF port to an oracle that cross-checks the GPU rasterizer without checked-in golden PNGs.

This folder is the consumer-side deep-dive on **Vello's testing strategy and its CPU/GPU split** specifically. The wider Linebender substrate (Vello's capability set as a render target, Parley vs cosmic-text, Linebender Color/Kurbo) is covered from the framework angle in [`../xilem-masonry/`](../xilem-masonry/); this folder does not re-derive that.

## Key facts

| Fact | Value | Source |
|---|---|---|
| Self-description | "A GPU compute-centric 2D renderer" | crates.io crate description |
| Flagship crate version | `vello` **0.9.0** (2026-05-15) | crates.io API `created_at` (authoritative; a GitHub HTML scrape misreported 2025) |
| Sparse-strips crates | `vello_cpu` / `vello_hybrid` / `vello_common` at **0.0.9** (2026-05-30) — still `0.0.x` | crates.io / GitHub releases |
| Maturity | "can currently be considered in an alpha state" (verbatim) — no 1.0; no stable roadmap found | README |
| License | Apache-2.0 OR MIT (shaders additionally Unlicense for research reuse) | README |
| Steward | Linebender — informal volunteer collective; **Raph Levien** "informally leads and drives the work" | linebender.org/about |
| MSRV | Rust 1.88 (`vello`) | README |
| Substrate | `kurbo`, `peniko`, `color`, `skrifa`, `wgpu` | README + release notes |
| Test crate | `vello_tests/` — `nv_flip` perceptual mean-error gate (NOT exact pixel match, NOT yet Kompari) | repo source |
| Adoption | Xilem/Masonry, `bevy_vello`, `woodpecker_ui` | repo / third-party crates |
| Stars (point-in-time) | `vello` ~4.1k; `parley` ~622 | GitHub, approximate |
| Downloads (2026-06-14) | `vello` 384,404 lifetime; `parley` 1,285,183 (Parley useful standalone) | crates.io API |
| Funding | Google / Google Fonts have sponsored Linebender ecosystem work — **exact figures/terms unverified** | blog history |

## Table of contents

- [`architecture.md`](architecture.md) — the classic GPU-compute pipeline: `Scene` → `Encoding` → the WGSL compute-stage sequence (prefix-sum, sort-middle coarse/fine tiling).
- [`sparse-strips.md`](sparse-strips.md) — the second-gen family: `vello_cpu` / `vello_hybrid` / `vello_common`, the SIMD `Level` pattern, and the `u8` vs `f32` pipelines. **The part most relevant to Buiy's oracle.**
- [`cpu-gpu-testing.md`](cpu-gpu-testing.md) — the `vello_tests` harness: three test tiers, GPU-as-source-of-truth, the blessing flow, `vello_cpu`'s `f32` pipeline as the snapshot generator.
- [`metric-and-kompari.md`](metric-and-kompari.md) — `nv_flip` mean-error gate, the **contested-inside-Linebender** xilem tolerance-16 counter-position, and the Kompari convergence plan.
- [`ecosystem-maturity.md`](ecosystem-maturity.md) — what Vello is and isn't (renderer, not toolkit; no a11y), the three variants/maturity levels, Parley pairing, adoption, version numbers, governance.
- [`open-problems.md`](open-problems.md) — what Vello structurally does NOT solve (compute portability, conflation artifacts, GPU memory allocation, glyph caching, oracle status).
- [`lessons.md`](lessons.md) — **the decision file.** Validates / Avoid / Borrow, framed for Buiy. Read this first if you are designing.
- [`glossary.md`](glossary.md) — Vello/Linebender-specific terms.

## Reading order

If you are consulting this folder for a Buiy visual-bug-detection design decision:

1. **Start here:** [`lessons.md`](lessons.md) — Validates / Avoid / Borrow; this is where the CPU-SDF-oracle and `nv-flip`-vs-pixelmatch decisions live.
2. **The mechanism Buiy is copying:** [`cpu-gpu-testing.md`](cpu-gpu-testing.md) then [`metric-and-kompari.md`](metric-and-kompari.md).
3. **The CPU/GPU split that makes it work:** [`sparse-strips.md`](sparse-strips.md).
4. **Why Vello is shaped the way it is (and the wart that justifies the CPU variant):** [`open-problems.md`](open-problems.md), then [`architecture.md`](architecture.md).
5. **Maturity / adoption / governance context:** [`ecosystem-maturity.md`](ecosystem-maturity.md).

## How to use

**Framing disclosure.** These docs are written from Buiy's stance — an AccessKit-first, wgpu + Taffy + cosmic-text, parallel-to-bevy_ui retained-mode engine building a reftests-first layered visual-bug-detection strategy. The "Implications for Buiy" / lessons framing reads Vello through that lens; readers auditing whether that strategy is itself right should weigh the corpus accordingly — it is a learn-from artifact, not a neutral catalog.

## Honesty / verification notes

Several dossier facts are flagged uncertain and carried as-such in the files below:

- The **sparse-strips thesis attribution** (Laurenz Stampl, ETH Zürich master's thesis) is single-sourced to a Linebender blog post and could not be re-verified against the thesis PDF — see [`sparse-strips.md`](sparse-strips.md).
- **Funding figures** beyond "Google / Google Fonts have sponsored Linebender work" are unverified — see [`ecosystem-maturity.md`](ecosystem-maturity.md).
- The `assert_mean_less_than` "**< 0.1 in almost all cases**" string is paraphrased from a WebFetch read of `compare.rs`, not byte-exact — see [`cpu-gpu-testing.md`](cpu-gpu-testing.md).
- Whether **Kompari has replaced `nv_flip`** in `vello_tests` by June 2026 is unconfirmed; as read, the live source still calls `nv_flip` — see [`metric-and-kompari.md`](metric-and-kompari.md).
- A **1.0 timeline** for `vello` could not be found in primary sources — treat as unverified.
- Version dates use crates.io API `created_at` timestamps (authoritative); a GitHub-releases HTML scrape misreported several as 2025.

## Sources

- Vello repo / README: https://github.com/linebender/vello
- crates.io crate page and version API: https://crates.io/crates/vello, https://crates.io/api/v1/crates/vello/versions
- DeepWiki architecture index: https://deepwiki.com/linebender/vello/1.1-architecture
- Linebender about / governance: https://linebender.org/about/
- Sibling Buiy prior-art: [`../xilem-masonry/`](../xilem-masonry/), [`../cosmic-text/`](../cosmic-text/), [`../taffy/`](../taffy/)
