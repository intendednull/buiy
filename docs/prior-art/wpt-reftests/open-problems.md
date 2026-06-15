**Date:** 2026-06-14
**Status:** active
**Subject:** What reftests structurally cannot do — the "impossible to reftest" category, the vacuous-pass / reference-independence failure mode, fuzzy masking, and cross-backend variance

# Open problems

What the reftest methodology structurally does *not* solve. These are not bugs in the harnesses — they are limits intrinsic to comparing two renderings of the same engine, and they define exactly where Buiy's Tier-4 reftest tier ends and Tier-5 goldens begin.

## The irreducible limit: effects with no feature-free reference

A reftest requires a reference file that "use[s] a different method to produce the same rendering as the test file." Where **no such alternative exists**, reftests are impossible and goldens are mandatory. The CSS-WG enumerates these verbatim:

- "there is no way to create a reference for underlining, since the position and thickness of the underline depends on the UA, the font, and/or the platform."
- "The following border-styles are impossible to reftest: dotted, dashed, ridge, groove, inset, outset, double. Only solid, none, hidden (and sometimes inherit) are reftestable."

WPT issue #7676 (labeled `type:untestable`) reinforces this from the spec side — gsnedders: "It's impossible to write any automated test for these values of border-style as the specification doesn't define their behaviour sufficiently (i.e., all values except none, hidden, and solid)."

The category is broader than borders: **any effect whose pixels depend on font metrics, a UA-chosen line position/thickness, or unspecified dash/dot geometry** has no feature-free twin — underline-position/thickness, `text-decoration-style` dotted/dashed/wavy/double, dashed/dotted/ridge/groove/double/inset/outset borders, focus-ring rendering. **The boundary is structural, not a gap in effort.**

### Implications for Buiy

These are precisely the visual effects Buiy's reftest tier cannot cover and must hand to Tier-5 goldens. For Buiy specifically, the analogous irreducible residue is: the **drop-shadow Gaussian falloff, glyph rasterization fidelity (hinting/subpixel), color-emoji compositing (CBDT/COLR/bitmap), blend-mode math, and gamma/sRGB encode** — all of which render on `origin/main` today. A reftest can confirm a shadow is *translation-invariant* or *symmetric* (and Tier-3 property tests should), but not that the falloff is *correct*. The one effect Buiy can pull *back* from goldens is SDF corner AA — not via a feature-free reference (none exists) but via the **CPU-vs-GPU cross-check** (Buiy's CPU SDF port as an independent rasterization oracle), a Tier-4.5 the strategy report places between reftests and goldens.

## The vacuous-pass failure mode: reference independence

The load-bearing wart. If the reference exercises the *same code path* as the test, a shared bug makes both render identically wrong and the test **passes vacuously**. The CSS-WG states it: a reference may "itself fail in such a manner as to cause the reference to render identically to a failed test." This is the exact symmetry of the golden-test weakness ("a golden can't catch a bug present when the golden was captured") — a reftest is equally blind when test and reference share the buggy path.

**Mitigations the browsers use:**
- The reference must use a *different technique* than the test ("The reference file must not use the same features that are being tested").
- Where one disjoint reference is impossible, use **multiple reference files**, each using a different technique, so two independent techniques must agree (WPT: "if there are any match references, at least one must match").
- Keep references human-legible and self-describing so a reviewer can confirm test and reference are not *both* wrong.

### Implications for Buiy

Concretely: do **not** write a flex reference using flex; do not write an `@container` reference using `@container`. Route references through Buiy's **primitive/absolute layer** (literal-positioned boxes that bypass Taffy and the CSS-subset entirely) or a second, independent style mechanism. This is Open Question #1 in the strategy report — *who* reviews reference independence, and *can it be lint-enforced* (e.g. "a reference scene must not contain a `@container` rule")? Without that discipline, reftests silently lose their teeth.

## Fuzzy matching masks real regressions

Exact equality is unattainable in some cases even within one run (gradient dithering, transform-rotated edges), so fuzzy tolerance exists — but **over-broad fuzz ranges silently mask real regressions**, and intermittently-failing tests are forced into a `0`-inclusive form that *loses* the regression-catching property (a fixed bug, or a further regression landing back in the window, goes unreported). The discipline (pin both ends, never include 0 when a difference is expected) mitigates but does not eliminate this; non-deterministic tests are the acknowledged casualty. Full treatment in [fuzzy-matching.md](fuzzy-matching.md).

## Cross-backend variance reintroduces the noise reftests cancel

The "one run cancels variance" benefit assumes a **fixed backend per CI lane**. Both halves of a reftest cancel GPU/driver/font variance only because they share it. A reftest whose test renders on Vulkan and reference on Metal would reintroduce exactly the variance reftests are designed to cancel, forcing fuzz back in. For Buiy this means: keep a reftest's two captures on the *same* `wgpu` backend in the *same* process; do not attempt cross-backend `==`. Cross-platform confidence comes from running the *whole* reftest suite on each pinned backend independently, not from cross-backend pairings.

## Unreftestable renderings belong in lower tiers, not just goldens

Some renderings are simply unreftestable *and* poor golden candidates (font-metric-dependent, scrollbar-dependent geometry). For Buiy these belong in **Tiers 1–3** (layout-number snapshots, structured display-list snapshots, metamorphic/property invariants) where the assertion is on *structured data* the engine produced, not on pixels — e.g. a glyph's `(line, glyph_id, x, y)` shaping snapshot, not a screenshot of the rendered text. The pyramid is the answer: each untestable-by-reftest effect routes to whichever tier *can* observe it deterministically.

## Sources

- CSSWG wiki, test/reftest (underline + border-style "impossible to reftest"; vacuous-pass) — https://wiki.csswg.org/test/reftest
- WPT issue #7676 (`type:untestable`, border-style) — https://github.com/w3c/web-platform-tests/issues/7676
- web-platform-tests, writing reftests (underline wart; multiple references) — https://web-platform-tests.org/writing-tests/reftests.html
- Firefox Source Docs, Reftest (fuzzy masking) — https://firefox-source-docs.mozilla.org/layout/Reftest.html
- Sibling files: [methodology.md](methodology.md), [fuzzy-matching.md](fuzzy-matching.md), [lessons.md](lessons.md), [consumers.md](consumers.md)
- Buiy strategy report (Open Question #1, Tier-4/Tier-5 boundary, CPU-vs-GPU cross-check) — [../../reports/2026-06-14-visual-bug-detection-strategy.md](../../reports/2026-06-14-visual-bug-detection-strategy.md)
