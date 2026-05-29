**Date:** 2026-05-29
**Status:** active
**Subject:** Servo / Stylo compared against Blink, Gecko, Taffy, and Buiy

# Servo / Stylo — Comparisons

How Servo/Stylo sits relative to the engines and layout libraries that bound
Buiy's design space. Read alongside [critiques.md](critiques.md) (why Servo is
incomplete) and [open-problems.md](open-problems.md) (what it has not solved).
For the components themselves see [stylo.md](stylo.md), [layout.md](layout.md),
[rendering.md](rendering.md).

## 1. Servo vs Blink (the canonical engine)

Blink (Chromium) is the canonical reference implementation of the CSS modules
Buiy cites — when a behavior is ambiguous, "what Blink does" is the de-facto
answer for most of the web. Blink is C++, single-threaded for layout in its
classic core (with parallelism added incrementally), and backed by Google's very
large engineering org. Servo is Rust, was architected around parallelism from
the start, and is funded at a tiny fraction of Blink's scale.

| | Blink | Servo |
|---|---|---|
| Language | C++ | Rust |
| Backing | Google (large) | Igalia / LF Europe / donations (small) |
| Status | Ships to billions | Experimental / embeddable |
| Layout | mature, all formatting contexts | layout 2020, active gaps (inline/multicol/tables) |
| CSS engine | Blink's own | Stylo (parallel cascade) |
| GPU paint | Skia + compositor | WebRender |

For Buiy, Blink is the **specification ground truth** (verify CSS-faithful
behavior against it) and Servo is the **Rust architectural reference** (how to
express these semantics in safe Rust, with which crate boundaries): Blink for
*what* the semantics are, Servo for *how* to structure a Rust implementation.

## 2. Servo vs Gecko (its own downstream consumer)

This is the most instructive comparison. Gecko is Firefox's engine — also
Mozilla-born, also the *home* of the components Servo invented. **Stylo shipped
in Firefox as "Quantum CSS" in Firefox 57 (2017-11-14)**, and **WebRender
shipped with Firefox 67 (2019-05-21)** — though WebRender rolled out *gradually*,
initially disabled by default and enabled progressively to qualified hardware
populations rather than to everyone at once. So Gecko is a downstream *consumer*
of Servo's two biggest successes, while remaining the fully-staffed shipping
engine.

The lesson: Servo's value was realized **by extracting components into a
different engine**, not by shipping Servo itself. Stylo had to satisfy Gecko's
existing DOM, its `nsCSSFrameConstructor`, its FFI boundary, and its enormous
test surface — and it did, which is the strongest possible validation of Stylo's
design (parallel cascade, rule tree, Bloom-filter ancestor matching, style
sharing cache). The integrated Servo browser, by contrast, stalled.

For Buiy: this is the model. Buiy reuses *components* (Taffy, `cosmic-text`,
AccessKit, wgpu) and assembles them, rather than building a monolithic engine.
The Stylo-into-Gecko story is direct evidence that component extraction is the
viable path and monolithic ambition is the trap. See [critiques.md](critiques.md)
§4.

## 3. Servo's layout, Taffy, and Buiy — a three-way contrast (not a dichotomy)

**Servo does not have a layout engine that is an *alternative* to Taffy — it
*embeds* Taffy.** Servo's layout 2020 owns its own block, inline, table, and
float algorithms but delegates **flexbox and CSS grid** to Taffy via
`components/layout/taffy/` and the `stylo_taffy` adapter (CSS-grid WPT pass rate
jumped ~18.6% → 38.3% in landing PR #32619; see [layout.md](layout.md) §1). So
"Servo layout *vs* Taffy" — as if they were competing whole-engine alternatives —
is wrong. The real contrast is three-way, about *layering*:

- **Servo** calls Taffy *inside* a formatting context — Taffy is one leaf
  algorithm (flex/grid) among several, with block/inline/table being Servo's own.
  Servo owns the surrounding box tree.
- **Buiy** runs Taffy as the *whole* `TaffyCompute` pass and layers its own
  passes (sticky, anchor, transforms, stacking + top-layer) **above** Taffy in
  `PostTaffyOverrides`, never forking it.
- **Taffy alone** is a *separate* dual MIT/Apache crate (DioxusLabs) scoped to
  box-level algorithms — Flexbox, CSS Grid, Block, Float — and explicitly does
  *not* do inline/text layout, fragmentation, tables, or paint. It carries no MPL
  obligation. Servo and Buiy are both Taffy embedders.

| | Servo layout 2020 | Taffy (alone) | Buiy |
|---|---|---|---|
| Scope | full CSS layout (a browser's job) | box-level only (Flexbox/Grid/Block/Float) | typed-Rust CSS subset above Taffy |
| Flex/grid | **Taffy-backed** (`stylo_taffy`) | its own (the box-level core) | **Taffy-backed** (`TaffyCompute`) |
| Block/inline/table | Servo's own (incl. its own IFC) | block only; no inline/table | Taffy block; text via `cosmic-text` leaves; tables stubbed |
| Taffy role | embedded leaf | n/a (is Taffy) | whole-tree solver |
| License | MPL-2.0 | MIT OR Apache-2.0 | MIT OR Apache-2.0 |
| Used by Buiy | no (reference only) | yes | — |

That two independent production engines (Servo, plus Blitz in §5) both sit Taffy
under their flex/grid is the strongest evidence the Taffy choice is load-bearing,
not a toy. Servo, having to be a browser, still had to write inline/table/float
itself and paid for the box-vs-fragment structure with a layout rewrite (see
[critiques.md](critiques.md) §3 and [../taffy/](../taffy/)). Buiy adds the
semantics Taffy omits — anchor, container queries, sticky, writing-modes,
stacking + top-layer, transforms + containment — *above* Taffy in
`PostTaffyOverrides`, never forking it.

## 4. Servo/Stylo vs Buiy

Buiy and Servo overlap in being Rust and CSS-faithful, but differ in almost
every commitment that matters.

| | Servo / Stylo | Buiy |
|---|---|---|
| Goal | embeddable web engine (a browser) | retained-mode UI library for Bevy |
| CSS coverage | aims at the full platform | typed-Rust *subset* of named modules |
| Layout | own block/inline/table/float; flex/grid via Taffy | Taffy whole-tree + passes above it |
| Style model | CSS cascade (selectors, specificity, rule tree) | ECS components + hybrid `Style` builder (BSN-native) |
| Text | own + Gecko's | `cosmic-text` |
| Accessibility | thin | AccessKit-first, WCAG 2.2 AA floor |
| Rendering | WebRender | wgpu via Bevy render graph |
| Concurrency model | parallel cascade (`rayon`) | Bevy ECS scheduler |
| License | MPL-2.0 | MIT OR Apache-2.0 |

The sharpest divergences:

- **Cascade vs ECS.** Servo resolves style through CSS selector matching and the
  cascade. Buiy has no selector engine; style is *decomposed, public-fielded ECS
  components* (no megacomponents) authored via a hybrid `Style` builder. Buiy
  borrows the *semantics* of CSS properties, not the *delivery mechanism*
  (selectors). See [stylo.md](stylo.md) for what Stylo does that Buiy
  deliberately does not.
- **Layout ownership.** Servo owns its block/inline/table/float solver (and
  rewrote it) while delegating flex/grid to Taffy; Buiy owns *none* of the box
  solver and layers semantics above whole-tree Taffy. Buiy's `PostTaffyOverrides`
  chain (6a sticky, 6b table-stub, 6c multicol-stub, 6d anchor, 6e
  transform-composition [Phase 8, landed], 6f stacking + top-layer [Phase 9,
  next]) is the seam where Buiy expresses CSS semantics Taffy omits. The
  layout-writes / render-reads contract means render never recomputes stacking or
  paint order — a discipline Servo's display-list construction shares in spirit
  (layout produces, paint consumes). See
  [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md).
- **License.** Buiy must not vendor MPL-2.0 source.

## 5. Blitz — the near-miss "Buiy substrate without Bevy"

Blitz (DioxusLabs, `github.com/DioxusLabs/blitz`) is the most direct point of
comparison for Buiy's *substrate choice*. Blitz is a modular HTML/CSS renderer
that combines **Stylo (CSS parsing/resolution) + Taffy (box-level layout) +
Parley (text layout) + Vello (GPU paint, via an `AnyRender` abstraction)**. (The
pre-Blitz prototype repo `jkelleyrtp/stylo-dioxus` described itself as "Render
HTML and CSS with Servo, Taffy, and Vello.")

Blitz is therefore *almost exactly* Buiy's substrate — minus Bevy and ECS, plus a
DOM. Where Buiy and Blitz diverge:

- Blitz keeps **Stylo + a DOM + an HTML parser** and renders documents; Buiy
  drops Stylo and the DOM entirely, expressing style as ECS components and
  authoring via Bevy/BSN. Blitz takes the MPL-2.0 Stylo dependency; Buiy declines
  it (see [open-problems.md](open-problems.md) §4).
- Blitz uses **Parley** for text; Buiy uses **`cosmic-text`**.
- Blitz targets `winit`/standalone windows + Dioxus apps; Buiy targets the **Bevy
  render graph** and integrates with Bevy's ECS and scheduler, sitting parallel
  to `bevy_ui` (see [../bevy-ui/](../bevy-ui/) and [../dioxus/](../dioxus/)).

Blitz is the proof that "reuse Taffy + a GPU vector renderer + a text crate to
render CSS without being a browser" is a real, working architecture — which is
the bet Buiy makes, with Bevy/ECS substituted for DOM/Dioxus and `cosmic-text`
substituted for Parley, and *without* the Stylo/MPL dependency.

## Sources

- Blink as canonical engine / Chromium: https://www.chromium.org/blink/
- Stylo in Firefox 57 "Quantum CSS" (2017-11-14): https://developer.mozilla.org/en-US/docs/Mozilla/Firefox/Releases/57 , https://hacks.mozilla.org/2017/08/inside-a-super-fast-css-engine-quantum-css-aka-stylo/
- WebRender in Firefox 67 (2019-05-21), gradual rollout: https://hacks.mozilla.org/2019/05/firefox-67-dark-mode-css-webrender/ , https://mozillagfx.wordpress.com/2019/05/21/graphics-team-ships-webrender-mvp/
- Servo layout 2020 vs Taffy scope: https://servo.org/blog/2023/04/13/layout-2013-vs-2020/ , https://github.com/servo/servo/wiki/Layout-2020
- Taffy (MIT/Apache, box-level only): https://github.com/DioxusLabs/taffy , https://crates.io/crates/taffy
- Blitz = Stylo + Taffy + Parley + Vello: https://github.com/DioxusLabs/blitz , https://github.com/DioxusLabs/blitz/blob/main/README.md , https://github.com/jkelleyrtp/stylo-dioxus
- License (MPL-2.0): https://github.com/servo/servo/blob/main/LICENSE
- Buiy specs: [../../specs/2026-05-08-buiy-layout-design/README.md](../../specs/2026-05-08-buiy-layout-design/README.md) , [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md) , [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling prior-art: [../taffy/](../taffy/) , [../bevy-ui/](../bevy-ui/) , [../dioxus/](../dioxus/) , [../xilem-masonry/](../xilem-masonry/)
