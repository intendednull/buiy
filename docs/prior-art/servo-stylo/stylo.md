**Date:** 2026-05-29
**Status:** active
**Subject:** Stylo — the parallel Rust CSS style system (cascade, rule tree, selector matching) and what shipping it in Firefox proved about production Rust CSS.

# Stylo: the only production Rust CSS cascade engine

Stylo is Servo's CSS style system, extracted into the `style`, `selectors`, and `cssparser` crates. It computes, for every element, the set of CSS declarations that apply and the resulting `ComputedValues` — the input that [layout.md](layout.md) and [rendering.md](rendering.md) consume. It is the one piece of prior art that answers the question Buiy's whole layout effort implicitly raises: *can a CSS cascade run in safe, parallel Rust at production scale?* The answer shipped to hundreds of millions of users in **Firefox 57 "Quantum"** (released **2017-11-14**), branded "Quantum CSS." For Buiy this is the existence proof — not a design to copy, but a demonstration that the typed-Rust-subset-of-CSS bet is sound.

Buiy is *not* building a cascade. Buiy's `SyncStyles` pass reads decomposed, public-fielded ECS components (no string CSS parse, no specificity sort) and resolves a typed subset of CSS semantics. So Stylo is relevant less as an architecture template and more as evidence: the hard, irregular, pointer-chasing work of CSS resolution was made parallel and memory-safe in Rust, and the bugs that fell out were the *interesting* kind (correctness, perf), not the use-after-free kind C++ engines fight. See [architecture.md](architecture.md) for how Stylo fits the wider Servo pipeline.

## The four mechanisms

The canonical reference is Lin Clark's Mozilla Hacks post "Inside a super fast CSS engine: Quantum CSS (aka Stylo)" (2017-08-22). Four mechanisms carry the engine:

**1. Parallel style computation via `rayon` work-stealing.** Style resolution is "embarrassingly parallel": each element's computed style depends mostly on its parent's, so the DOM tree can be styled top-down across all cores. Stylo uses `rayon`'s work-stealing scheduler rather than statically partitioning the tree — when a core drains its queue it steals subtrees from busy cores, which keeps load balanced on the lopsided trees real pages produce. Rust's `Send`/`Sync` and borrow checker are what made this tractable: data races that would be latent UB in a C++ engine are compile errors here. This is the load-bearing claim for Buiy, whose layout passes run inside Bevy's parallel ECS schedule.

**2. The rule tree (shared cascade).** Inherited from Gecko's old engine. Rather than store every element's matched-rule list independently, Stylo interns matched rules into a tree whose paths are shared between elements that match the same rules. Restyling walks pointers instead of re-running selector matching, and branches only where elements diverge. This is a structural-sharing optimization; Buiy's analogue is far cheaper because there is no selector list to match — components *are* the resolved declarations.

**3. The style sharing cache.** Borrowed (Stylo's own framing) from WebKit and Blink. When two sibling-ish elements would compute identical styles — same tag, classes, id, inline style, and parent computed style — the second reuses the first's `ComputedValues` outright. It is a small LRU of recently computed styles keyed on the inputs that determine the result.

**4. Bloom-filter ancestor selector matching.** To answer "does any ancestor match `.foo`?" without walking the ancestor chain per selector, each element carries a Bloom filter summarizing its ancestors' ids/classes/tags. A descendant-combinator selector first probes the filter; only a possible-match triggers the real walk. Bloom filters give false positives but never false negatives, so this is a sound fast-reject. Buiy has no descendant combinators, so it needs none of this — but container queries (`CqActivate`) do walk ancestors, and the Bloom-filter-as-fast-reject pattern is worth keeping in mind if that walk ever shows up in a profile.

These four interlock: the rule tree caches *what matched*, the sharing cache caches *whole results*, the Bloom filter makes the *matching* it can't cache cheap, and `rayon` runs the whole thing wide. The traversal itself is a parallel pre-order walk — a node cannot be styled before its parent (it inherits the parent's computed style), so the tree is processed in depth waves and `rayon` fans each node's children out as independent tasks. That parent-before-child dependency is the same shape as Buiy's top-down layout-flow invariance, which is why Bevy's parallel schedule is a defensible host for the same kind of work.

## ComputedValues and the crate split

`ComputedValues` is Stylo's output struct: the fully resolved, absolute, inheritance-applied value of every CSS property for one element, grouped into reference-counted "style structs" (e.g. `Font`, `Background`, `Position`) so unchanged groups are shared across elements. Buiy's equivalent is not one struct but the *set of resolved ECS components* an entity carries after `SyncStyles` plus the private render-handoff components (`ResolvedTransform`, the Phase 9 `StackingContext`). The shared-struct idea — only the changed slice is recomputed and the rest is shared by pointer — echoes Buiy's contract that layout writes private resolved components once and render only reads them.

The crates, verified on crates.io (2026-05):

- **`cssparser`** (v0.37.0; repo `servo/rust-cssparser`) — a tokenizer/parser for CSS Syntax Level 3. ~51M all-time downloads; used far beyond Servo.
- **`selectors`** (v0.38.0; repo `servo/stylo`) — selector parsing and the matching engine (including the Bloom-filter logic). ~44M downloads.
- **`stylo`** (the published name of the `style` crate, v0.17.0; repo `servo/stylo`) — "The Stylo CSS engine," the cascade itself. ~129K downloads — far lower, because the cascade is hard to use standalone; it assumes a DOM-shaped host.

The **standalone `servo/stylo` repository was created 2024-02-07**, splitting Stylo out of the Servo monorepo so Firefox (Gecko) and Servo can both vendor it and so downstreams like Blitz can depend on a versioned crate rather than a git pin. This split is itself a Buiy-relevant signal: a CSS engine reusable enough to be a library is a recent, deliberate, still-immature outcome — `stylo`'s API is not a stable public contract.

## What Quantum CSS proved (and the licensing catch)

Quantum CSS replaced Gecko's sequential C++ style system in a shipping browser and survived. The Mozilla Hacks post reports the parallel cascade hitting near-linear speedups with core count on style-heavy pages. The significance for Buiy is narrow and real: **a Rust CSS resolution stage is viable in production, in parallel, without the memory-safety tax** — exactly the regime Buiy's parallel ECS layout passes occupy.

The catch Buiy must call out explicitly: **Servo and Stylo are licensed MPL-2.0** (verified on the `servo/servo` repo). Buiy is **MIT OR Apache-2.0**. MPL-2.0 is file-level copyleft — incompatible with vendoring Stylo source into Buiy's permissively licensed tree without subjecting those files to MPL terms. So Stylo is prior art to *learn from and cite*, not code to lift. (Notably, Blitz's `stylo_taffy` glue crate is triple-licensed MIT/Apache-2.0/MPL-2.0 precisely to bridge this gap — see below.) This divergence is also flagged in [governance.md](governance.md).

## Blitz: the closest thing to Buiy's substrate

`DioxusLabs/blitz` is the sharpest external validation of Buiy's component choices. Blitz is a modular HTML/CSS rendering engine that composes:

- **Stylo** for CSS parsing and style resolution,
- **Taffy** for box-level layout (the same engine Buiy builds on — see [../taffy/](../taffy/)),
- **Parley** (Linebender) for text layout, rendered through an **`AnyRender`** abstraction whose primary backend is **Vello** on `wgpu`.

(Correction to this file's brief: the often-repeated "Stylo + Taffy + Vello" shorthand is close but imprecise — text is Parley, not cosmic-text, and rendering is abstracted behind `AnyRender` with Vello as one backend, not hardwired.) `blitz-dom` holds the DOM + style + layout + events; `blitz-paint` lowers a `blitz-dom` tree into `anyrender` draw commands. Dual-licensed MIT/Apache-2.0.

Blitz is nearly the Buiy substrate with two swaps: it sits on a DOM and a generic renderer rather than **Bevy's ECS and render graph**, and it uses Parley rather than **cosmic-text**. That overlap is the point — an independent project reached almost the same Rust-UI stack (Stylo-or-equivalent style + Taffy layout + GPU paint) for general HTML/CSS, while Buiy deliberately diverges by (a) skipping the cascade in favor of typed ECS components and (b) putting its own passes *above* Taffy (sticky, anchor, transforms, the Phase 9 stacking/top-layer work) rather than forking layout. Blitz proves the substrate; Buiy's bet is the ECS-native authoring and the Taffy-superset passes on top.

## Implications for Buiy

- **The cascade is the part Buiy is right to skip.** Stylo's rule tree, sharing cache, and Bloom filter all exist to make *string-CSS selector matching* fast. Buiy's decomposed public-fielded components erase that entire problem class: there is no specificity sort, no selector match, no `!important` ordering to resolve at runtime. The lesson is inverted prior art — Stylo shows exactly how much machinery a real cascade costs, which justifies not having one.
- **Parallel resolution in safe Rust is proven.** Buiy's `SyncStyles`/`PostTaffyOverrides` passes running in Bevy's parallel schedule are not a research gamble; Stylo did the harder version (irregular tree, work-stealing) at browser scale.
- **Browser engines are the reference implementations of the W3C modules Buiy cites.** Where Buiy implements a typed subset of Display 3, Positioned Layout, Containment 3, etc. (see [../../specs/2026-05-08-buiy-layout-design/README.md](../../specs/2026-05-08-buiy-layout-design/README.md)), Stylo (the Rust one) and Blink (the canonical one — see [comparisons.md](comparisons.md)) are where the prose specs become observable behavior. When Buiy must decide an underspecified edge of stacking-context formation ([../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)) or containment, Stylo's source is a legal-to-read, hard-to-copy reference.
- **License hygiene.** Read Stylo, cite Stylo, do not vendor Stylo. MPL-2.0 ≠ MIT/Apache-2.0.

See also: sibling [layout.md](layout.md), [rendering.md](rendering.md), [history.md](history.md), [critiques.md](critiques.md), [open-problems.md](open-problems.md); and prior-art folders [../taffy/](../taffy/), [../dioxus/](../dioxus/), [../bevy-ui/](../bevy-ui/), [../xilem-masonry/](../xilem-masonry/).

## Sources

- Lin Clark, "Inside a super fast CSS engine: Quantum CSS (aka Stylo)," Mozilla Hacks, 2017-08-22 — https://hacks.mozilla.org/2017/08/inside-a-super-fast-css-engine-quantum-css-aka-stylo/
- "Firefox 57 (Quantum) for developers," MDN — https://developer.mozilla.org/en-US/docs/Mozilla/Firefox/Releases/57
- `stylo` crate (v0.17.0), crates.io — https://crates.io/crates/stylo
- `selectors` crate (v0.38.0), crates.io — https://crates.io/crates/selectors
- `cssparser` crate (v0.37.0), crates.io — https://crates.io/crates/cssparser
- `servo/stylo` repository (created 2024-02-07; "CSS engine that powers Servo and Firefox") — https://github.com/servo/stylo
- `servo/servo` repository (license MPL-2.0; created 2012-02-08) — https://github.com/servo/servo
- `DioxusLabs/blitz` — modular HTML/CSS engine (Stylo + Taffy + Parley + AnyRender/Vello) — https://github.com/DioxusLabs/blitz
- "Experience Report: Developing the Servo Web Browser Engine using Rust," arXiv:1505.07383 — https://arxiv.org/abs/1505.07383
- Buiy layout design specs — ../../specs/2026-05-08-buiy-layout-design/README.md
- Buiy stacking & top-layer spec — ../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md
- Buiy foundation spec — ../../specs/2026-05-07-buiy-foundation/README.md
