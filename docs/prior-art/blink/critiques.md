**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium) — honest critique of the engine monoculture, the C++ monolith, the `ComputedStyle` megastruct, and legacy-layout migration debt

# Critiques

This file enumerates Blink's structural costs as they appear from a 2026-05-29 outside-the-project vantage. Blink is the canonical reference implementation of the CSS modules Buiy implements a typed-Rust subset of — so the critiques here are *not* "do not use Blink"; they are "here is what the reference implementation pays for its completeness, and which of those costs Buiy's substrate avoids by construction." Companion to [`open-problems.md`](open-problems.md) (forward-looking structural gaps) and [`comparisons.md`](comparisons.md) (Blink vs Gecko / WebKit / Servo / Buiy). Architecture and feature detail live in [`architecture.md`](architecture.md), [`layout.md`](layout.md), [`style.md`](style.md), [`stacking-and-paint.md`](stacking-and-paint.md), and [`containment-and-queries.md`](containment-and-queries.md).

## The engine monoculture

Blink is shared by Chrome, Edge (since Edge 79, 2020-01-15), Brave, Opera, Vivaldi, and Samsung Internet. After Microsoft retired EdgeHTML and rebased Edge on Chromium, the only non-Blink engines with material share are Gecko (Firefox) and WebKit (Safari, and — by App Store policy until the 2024 EU DMA changes — every iOS browser). The practical consequence is that *Blink's implementation choices become the de-facto web standard*, ahead of or in place of the W3C spec text.

This is the central third-party critique of Blink, and it is not hypothetical:

- "Whatever Chrome does is what the web does" is the recurring framing from the Gecko and WebKit teams and from standards-body participants. Features ship in Blink behind the Intent process (see [`governance.md`](governance.md)) and become load-bearing on the live web before the other engines implement them, which pressures the others to match Blink's behavior rather than the spec's.
- Mozilla's own framing of Servo and of Firefox's continued existence is explicitly anti-monoculture: a single engine means a single set of bugs, a single security-surface monoculture, and a single vendor's roadmap deciding what "the web platform" is.

**Implications for Buiy.** Buiy does not add an engine to this count — it is a Bevy plug-in, not a browser. But the monoculture is *why* Blink is load-bearing prior art: when the W3C spec text is ambiguous, the question "what does the platform actually do?" almost always resolves to "what Blink does." Buiy's CSS-faithful subset (it cites Display 3, Positioned Layout, Containment 3, Writing Modes 4, Anchor Positioning 1) therefore checks behavior against Blink as the reference, while keeping Servo/Stylo (the Rust reference implementation) as the second witness so Buiy is not importing Blink-specific quirks. See [`comparisons.md`](comparisons.md).

## The C++ monolith

Blink is a multi-million-line C++ codebase inside the even larger Chromium tree. The size and the language carry well-documented costs:

- **Memory-safety surface.** The Chrome security team has repeatedly reported that around 70% of Chrome's high-severity security bugs are memory-safety bugs (use-after-free, out-of-bounds) — the figure that motivated the `MiraclePtr`/`*Scan` heap-hardening work and the long-running interest in Rust for new Chromium components. A C++ rendering engine of this size cannot be made memory-safe by review alone.
- **Build and onboarding cost.** A full Chromium build is hours on commodity hardware; the engine is not approachable for casual contribution, and the contributor pool is dominated by paid Google and (secondarily) Microsoft/Igalia engineers.
- **Coupling.** Blink is not shipped as a standalone library. Embedding Blink means embedding Chromium (via the Content API or CEF), not linking a focused renderer.

**Implications for Buiy.** Buiy is Rust (`MIT OR Apache-2.0`) and inherits memory safety from the language, not from heap-hardening retrofits. It is also *not* a monolith: it composes Taffy (layout), `cosmic-text` (text), AccessKit (a11y), and Bevy's `wgpu` render graph, each a separately-versioned crate. Buiy adds anchor positioning, container queries, sticky, writing-modes, stacking + top-layer, and transforms + containment as passes *above* Taffy (never forking Taffy) — the inverse of Blink's "one engine owns everything" structure. The cost Buiy pays instead is integration seams across crates; the cost it avoids is a single un-decomposable C++ tree.

## The `ComputedStyle` megastruct

Blink's per-element resolved style is `ComputedStyle`: a single large object holding the resolved value of every CSS property for an element. To keep this affordable at scale, Blink layers in:

- **Field grouping** into sub-`DataRef<>` groups (background, surround, box, rare-inherited, rare-non-inherited, …) so unrelated properties live in separately copy-on-write-shared sub-objects.
- **Sharing** — identical `ComputedStyle` instances are deduplicated across elements so a page of 10,000 similarly-styled nodes does not allocate 10,000 distinct full styles.
- **`ComputedStyleBase` generated from `computed_style_field_aliases`/the property database**, because hand-maintaining hundreds of accessor/setter/diff methods is infeasible.

The well-known maintenance critique: `ComputedStyle` is the canonical "god object." Adding a property touches the generated base, the diffing logic (what invalidates layout vs paint vs nothing), the sharing/dedup logic, and the field-group placement decision. Every property is globally entangled with every other property's storage and invalidation. This is the price of representing all of CSS in one resolved-style type.

**Implications for Buiy.** This is a direct and deliberate divergence. Buiy is built on ECS + *decomposed, public-fielded components* — explicitly "NO megacomponents" — plus a hybrid `Style` builder (BSN-native) for authoring ergonomics. A Buiy node's transform lives in `UiTransform` (+ `Translate`/`Rotate`/`Scale` longhands), its containment in a `Containment` component, its stacking in a `Stacking` component (Phase 9, next). There is no single resolved-style god object: adding a feature adds a component and a pipeline sub-pass, local to that feature. The `bevy-ui` prior art records the opposite failure mode (see [`../bevy-ui/`](../bevy-ui/)) — Buiy's decomposition is informed by both Blink's megastruct *and* bevy_ui's component-model history.

## Legacy-layout migration debt

Blink's modern layout engine is LayoutNG (fragment-tree, immutable physical fragments). It did not replace the legacy layout engine in one step — it replaced it *primitive by primitive over years*:

- Block and inline layout (plus floats and out-of-flow positioning) shipped first, in **Chrome 77 (2019)**.
- Flexbox, then CSS Grid, then table layout, then block fragmentation followed in subsequent releases.

For the entire multi-year transition, Blink carried *two* layout engines in-tree and had to keep them behaviorally consistent at the boundary (a legacy subtree inside an NG subtree, and vice versa). The migration is a textbook case of "rewrite the engine under a live, web-compatibility-constrained product without a flag day."

**Implications for Buiy.** Buiy starts on Taffy, which already provides Flexbox + Grid + Block + Float in one consistent solver, so Buiy never carries two layout engines and never owns a legacy/NG boundary. Where Blink had to *migrate to* an immutable fragment tree, Buiy's contract is fixed from the start: **layout writes, render reads** — `TaffyCompute` plus the `PostTaffyOverrides` sub-passes (`6a` sticky, `6b` table stub, `6c` multicol stub, `6d` anchor, `6e` transform-composition, `6f` stacking + top-layer) write resolved values, and render never recomputes layout/stacking/paint order. The LayoutNG lesson Buiy *does* take is the immutability discipline: Phase 8's `ResolvedTransform` and Phase 9's planned private `StackingContext { painters_z: Vec<Entity> }` are write-once render hand-offs, the same shape as an immutable fragment. See [`layout.md`](layout.md) and the Buiy layout spec [`../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md`](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md).

## Not shippable as a focused library

Blink is not distributed as a standalone rendering crate the way Taffy, `cosmic-text`, or AccessKit are. Embedding Blink means embedding *Chromium* — through the Content API or the Chromium Embedded Framework (CEF) — and inheriting the multi-process model, the V8 JavaScript engine, the network stack, the GPU process, and the build system along with the renderer. The commercial game-UI engines that wrap an HTML/CSS renderer (Coherent Gameface, historically Coherent UI / Berkelium / Awesomium) exist *because* you cannot link "just the layout and style parts" of a browser:

- There is no `blink-layout` you can `cargo add` or link as a focused dependency.
- The dependency surface is the whole browser, so binary size, update cadence, and security-patch obligations are the whole browser's.
- Behavior is excellent but the integration boundary is "host an entire browser," not "call a layout function."

**Implications for Buiy.** This is the structural reason Buiy is built on *decomposed, separately-versioned crates* (Taffy for layout, `cosmic-text` for text, AccessKit for a11y, `wgpu` via Bevy for render) instead of embedding a browser. Buiy is itself a focused library: a Bevy plug-in that a game can `cargo add`. The commercial-embedding prior art ([`../coherent-gameface/`](../coherent-gameface/), [`../rmlui/`](../rmlui/)) is the evidence that "just embed a browser" is expensive enough that a whole product category exists to avoid it. Buiy chooses the focused-substrate path that browser-embedding products cannot.

## Completeness is not free anywhere

Every critique above is the shadow of Blink's actual achievement: a complete, web-compatible implementation of essentially all of CSS. Buiy does not attempt that — it is a *subset*, by design (foundation [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)). The honest reading is that Blink pays the monolith/megastruct/migration costs *because* it must support the entire open web bug-for-bug; Buiy can decompose and stay typed precisely because it gets to choose its subset and cite the spec rather than chase legacy quirks.

## Sources

- Google forks WebKit, launches Blink (2013-04-03) — https://techcrunch.com/2013/04/03/google-forks-webkit-and-launches-blink-its-own-rendering-engine-that-will-soon-power-chrome-and-chromeos/
- Microsoft Edge 79 (Chromium) released 2020-01-15 — https://blogs.windows.com/msedgedev/2020/01/15/upgrading-new-microsoft-edge-79-chromium/
- LayoutNG (block/inline in Chrome 77, 2019) — https://www.chromium.org/blink/layoutng/
- RenderingNG deep-dive: LayoutNG — https://developer.chrome.com/docs/chromium/layoutng
- Chrome memory-safety / ~70% of high-severity bugs — https://www.chromium.org/Home/chromium-security/memory-safety/
- Chromium `LICENSE` (top-level BSD-3-Clause, Google copyright holder; WebKit-inherited LGPL/MIT/MPL per-file headers) — https://chromium.googlesource.com/chromium/src/+/main/LICENSE
- ComputedStyle source (Blink core/style) — https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/renderer/core/style/computed_style.h
- Chromium Embedded Framework (CEF) — https://github.com/chromiumembedded/cef
- Buiy foundation README — ../../specs/2026-05-07-buiy-foundation/README.md
- Buiy layout: stacking + top layer spec — ../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md
- Buiy bevy-ui prior art — ../bevy-ui/
- Buiy Taffy prior art — ../taffy/
- Buiy Coherent Gameface prior art (HTML/CSS game-UI embedding) — ../coherent-gameface/
- Buiy RmlUi prior art (C++ HTML/CSS UI library) — ../rmlui/
