**Date:** 2026-06-14
**Status:** active
**Subject:** What Flutter's golden system structurally does NOT solve — host-rasterization leakage, the flaky-fix blind spot, the hosted-service floor, and the irreducible color-emoji golden

# Open problems

What the Flutter golden ecosystem — even at Google scale, with Gold content-addressing, `goldctl`, per-PR human triage, and Docker pinning — structurally does *not* solve. These are the limits Buiy should not expect any golden tier to exceed.

## 1. Host-OS rasterization leaks through containers

Pixel goldens remain platform-sensitive even when the OS is pinned. Issue [#131559](https://github.com/flutter/flutter/issues/131559) (Open, P2): with **one identical Ubuntu Docker image**, generating on a Windows host and verifying on a Mac host yields *"a random smattering of mismatched pixels"* ranging *"from single pixels to 30-90 pixel mismatches."* Host-OS font/AA rasterization leaks through the container boundary. **Implication for Buiy:** a Docker image is not a determinism guarantee for the pixel tier; the real fix is upstream (box-glyph font, flat shadows) so the comparison has nothing host-dependent in it. The irreducible pixel residue genuinely needs *one* canonical host, pinned.

## 2. Skipping a flaky golden makes the fix unverifiable

When a framework golden flakes it is simply skipped, and "when we skip a test we stop sending to Skia Gold entirely," so there is "no way to verify flaky golden test fixes" short of speculatively un-skipping — "The cost of a mistake is closed tree, P0s, wasted time, and other sadness" ([#111325](https://github.com/flutter/flutter/issues/111325), 2022-09-10). The act of disabling the flake destroys the signal you need to confirm the flake is gone. **Implication for Buiy:** flake at the pixel tier is not just noise — it actively erodes the ability to fix it. Another argument for keeping that tier minimal and deterministic-by-construction rather than fighting flake after the fact.

## 3. The hosted-service operational floor

Flutter Gold is a hosted Skia Gold instance on Google Cloud — a GCS bucket plus a frontend, Google-operated. It carries an operational floor: the `flutter-gold` check gets stuck pending on force-push (remedy: *"Try rebasing again. This side-effect is flaky"*), and unapproved-image post-submit failures have historically been mis-reported as flaky. A hosted triage service is a standing cost and a standing dependency. **Implication for Buiy:** for a local-first, offline, MIT/Apache library, this central service is the wrong dependency *and* a flake source. Buiy commits goldens to the repo and reserves a host-pinned local rasterizer for the residue — no hosted service. (For the storage/triage tradeoffs in depth, see the [`skia-gold`](../skia-gold/README.md) folder.)

## 4. Color emoji is the irreducible golden

Color-emoji (CBDT/CBLC, COLR/CPAL, sbix) rendering is genuinely platform-divergent and unstable even within Flutter — e.g. "Color emoji renders as question mark boxes on iOS simulator (Impeller, Skia removed)" ([#183828](https://github.com/flutter/flutter/issues/183828), closed, filed 2026-03-18). A box-glyph font cannot collapse it, and it resists metric-only assertion (the *point* is the rasterized color bitmap). **Implication for Buiy:** treat Buiy's color-emoji path as the one case that *must* be a real golden screenshot on pinned hardware/font, with generous diff tolerance. Do not try to make it deterministic — accept it as the irreducible residue.

## 5. The oracle problem persists

Goldens (like all snapshot tests) assert "matches the blessed image," not "is correct." A wrong-but-blessed golden silently passes forever. Gold's content-addressing makes a triaged-wrong digest *auto-approve* indefinitely. The triage human is the only oracle, and human triage at scale is itself a known fatigue/error source. **Implication for Buiy:** the cheaper tiers (reftests assert *relations* — `render-A == render-B` — that encode a correctness oracle without a blessed image; property invariants assert laws with no oracle at all) are structurally better than goldens here, which is the strategy report's core thesis. Goldens are the last resort, not the first.

## Sources

- flutter/flutter#131559 (Docker rasterization leak): https://github.com/flutter/flutter/issues/131559
- flutter/flutter#111325 (no way to verify flaky golden fixes): https://github.com/flutter/flutter/issues/111325
- flutter/flutter#183828 (color emoji as tofu on iOS sim; closed, filed 2026-03-18): https://github.com/flutter/flutter/issues/183828
- Skia Gold service model and storage tradeoffs: [`docs/prior-art/skia-gold/`](../skia-gold/README.md)
- Buiy visual-bug-detection strategy: [`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md)
