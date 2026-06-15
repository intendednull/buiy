**Date:** 2026-06-14
**Status:** active
**Subject:** What Vello structurally does NOT solve — the wart list the README ships, plus the gaps relevant to a greenfield neighbor

# Open problems

The Vello README itself names four open problem areas and an alpha-state caveat. This file collects those plus the deeper structural gaps relevant to Buiy. Honest tone: these are real adoption blockers, quoted where possible.

## The README's own four open areas (verbatim)

The README states Vello "can currently be considered in an alpha state," with four open problem areas:

1. **Blur / filter effects** — still in flux.
2. **Conflation artifacts** — the antialiasing approach produces visible seams where adjacent primitives meet; a known, unsolved class of visual bug.
3. **GPU memory-allocation strategy** — robust *dynamic* GPU memory allocation has been a recurring pain point. The compute pipeline must size buffers for work whose extent is only known mid-pipeline.
4. **Glyph caching** — no settled glyph-cache strategy.

It also states Vello "needs a GPU with support for compute shaders to run," and "The web is not currently a primary target for Vello, and WebGPU implementations are incomplete, so you might run into issues."

## The load-bearing wart — GPU-compute portability is unsolved at the *spec* level

This is *why Vello must ship `vello_hybrid` + `vello_cpu` at all.* Raph Levien's "Prefix sum on portable compute shaders" documents it verbatim:

- "The Vulkan specification itself is careful to make no forward progress guarantees."
- Apple and ARM GPUs exhibit **forward-progress failures**.
- On Metal "there is simply no way to run decoupled look-back … (unless the payload can be packed into a 32 bit word)."
- WebGPU's uniformity analysis "rejects valid shaders."
- DX11/FXC's "simplistic uniformity analysis rejects common advanced compute patterns."
- Net: portable compute "can work pretty well on Vulkan and DX12, but Metal remains out of reach … as is WebGPU." DX12 additionally needs special handling of the SRV/UAV (readonly vs read-write) descriptor distinction ([vello#125](https://github.com/linebender/vello/issues/125)).

`vello_hybrid` is the pragmatic escape hatch: by rasterizing sparse strips with a *fragment* shader instead of compute, it runs on WebGL2 and low-end GPUs that can't or won't run the compute pipeline ([vello#670](https://github.com/linebender/vello/issues/670)). **This entire problem class does not exist for Buiy** — Buiy is instanced quads + per-fragment SDF, no compute, no prefix-sum, no forward-progress dependency. Buiy sidesteps the wart that forced Vello's architecture to fork.

## Gaps relevant to a greenfield neighbor (Buiy)

1. **CPU fallback exists but is not framed as an authoritative oracle.** `vello_cpu` shares the sparse-strip architecture but DeepWiki positions it "for debugging purposes," **not** as a reference oracle. Buiy's plan to promote its CPU SDF port to a first-class oracle therefore goes *beyond* what Vello formally claims. The upside: Buiy's oracle is a per-pixel eval of the *same analytic function* the GPU runs, which is a more durable correctness basis (against implementation drift) than two independently-written rasterizers agreeing — with the matching limitation that a bug in the shared function escapes both paths, so the golden/reftest tiers stay necessary (see [`lessons.md`](lessons.md)).

2. **No single settled image-diff metric.** Linebender runs `nv_flip` in `vello_tests` and tolerance-16 pixel diff in xilem, with Kompari as an unreleased convergence plan ([`metric-and-kompari.md`](metric-and-kompari.md)). A greenfield neighbor inherits an *unsettled* answer, not a recipe — Buiy must pick deliberately per failure mode.

3. **The CPU-vs-GPU cross-check is treated as transitional.** Vello's tier-3 comparison tests are slated to be "largely phased out in favour of additional snapshot tests" ([`cpu-gpu-testing.md`](cpu-gpu-testing.md)) — because Vello's two pipelines are *different implementations*, so their agreement is a weaker invariant. Buiy should NOT inherit this "phase out" posture; Buiy's oracle is the same function, so the cross-check is durable.

4. **No accessibility, no layout, no widgets — by design.** Vello is purely a rasterizer. None of Buiy's layout-number-snapshot or a11y-contract testing tiers have any analog in Vello; only the rasterization-cross-check tier maps over.

5. **Git LFS reference store cost.** `vello_tests/snapshots/*.png` live in Git LFS ([`cpu-gpu-testing.md`](cpu-gpu-testing.md)). The CPU-oracle approach lets a neighbor *defer* that cost for the rasterization cross-check entirely; LFS is only needed for genuine golden screenshots.

6. **Everything is pre-1.0 and churning.** Flagship alpha, sparse-strips `0.0.x`, glyph/blur/memory strategies in flux, MSRV 1.88 — taking a runtime dependency means tracking a moving target.

## Sources

- Vello README (four open areas, alpha caveat): https://github.com/linebender/vello
- "Prefix sum on portable compute shaders" (Raph Levien, 2021-11-17): https://raphlinus.github.io/gpu/2021/11/17/prefix-sum-portable.html
- DX12 portability polish issue: https://github.com/linebender/vello/issues/125
- Sparse strip path rendering issue: https://github.com/linebender/vello/issues/670
- DeepWiki testing & validation: https://deepwiki.com/linebender/vello/5.2-testing-and-validation
