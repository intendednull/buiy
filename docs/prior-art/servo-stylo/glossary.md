**Date:** 2026-05-29
**Status:** active
**Subject:** Servo / Stylo — glossary of system-specific terms used across this folder.

# Servo / Stylo — Glossary

One-line definitions of the Servo/Stylo/WebRender terms used in the sibling files. Backticked names are crates, types, or APIs.

- **`AnyRender`** — Blitz's renderer-abstraction trait; lets Blitz target multiple GPU backends, with Vello as the primary one (not a hardwired Vello dependency).
- **batching** — WebRender step that groups display-list primitives sharing GPU state (shader, texture atlas, blend) into a few instanced draw calls instead of thousands.
- **Blitz** — DioxusLabs' modular HTML/CSS rendering engine: Stylo (CSS) + Taffy (box layout) + Parley (text) + Vello (GPU paint). The closest external analogue to Buiy's substrate, minus Bevy/ECS. Repo `github.com/DioxusLabs/blitz`.
- **Bloom filter (ancestor matching)** — per-element probabilistic summary of ancestor ids/classes/tags; lets a descendant-combinator selector fast-reject (no false negatives) before walking the ancestor chain. A Stylo mechanism.
- **box tree** — layout's persistent representation of nested formatting contexts; records *what kind* of box each element generates, built once from the DOM, mostly immutable (`servo_arc::Arc`). Distinct from the fragment tree.
- **`BoxFragment`** — a node in the fragment tree representing a laid-out box (one of several fragment types in `components/layout/fragment_tree/`).
- **compositor** — Servo's main-process component that receives display lists, drives WebRender, manages the scene, and handles scrolling/hit-testing/presentation. Used near-interchangeably with "renderer" in Servo's lexicon.
- **`ComputedValues`** — Stylo's output struct: the fully resolved, absolute, inheritance-applied value of every CSS property for one element, grouped into reference-counted "style structs."
- **constellation** — Servo's central per-instance coordinator; owns the set of pipelines, brokers navigation, manages content processes, routes messages. The only component with a global view.
- **`cssparser`** — Servo's CSS Syntax Level 3 tokenizer/parser crate (v0.37.0); repo `github.com/servo/rust-cssparser`. Used far beyond Servo (~51M downloads).
- **display list** — a flat, *unrasterized*, serialized list of paint primitives (rects, text runs, images, gradients, borders, clips, stacking contexts) emitted by layout and consumed by WebRender.
- **fragment tree** — the *result* of laying the box tree out against a containing block; one box can produce many fragments (line splits, columns, pages). Consumed to build the display list.
- **frame builder** — WebRender stage that walks the retained scene for a given viewport/scroll state, resolves clips and transforms, culls off-screen primitives, and assigns each to a render target.
- **flow tree** — layout-2013's single combined tree where internal `Flow` nodes were block/inline formatting contexts and leaves were fragments; boxes and fragments were intermixed. Replaced because it conflated two concepts the CSS spec keeps distinct.
- **formatting context** — the CSS notion of an independent layout region (block/BFC, inline/IFC, flex, grid, table). Servo encodes these as nested enums (`IndependentFormattingContext`); flex and grid are Taffy-backed, the rest are Servo's own.
- **Gecko** — Firefox's (C++) browser engine; the downstream *consumer* of Servo's two biggest successes (Stylo as Quantum CSS, WebRender as Quantum Render).
- **Igalia** — worker-owned consultancy (major Chromium/WebKit contributor) running Servo's day-to-day development since 2023; largest single contributor but not a numeric majority (26% of 2024 merged PRs).
- **IFC (inline formatting context)** — layout for inline-level content: text runs, line breaking, bidi, atomic inlines. Servo writes its own (`flow/inline/`); Taffy has none.
- **`layout` (crate)** — Servo's current single layout crate, formed in 2025 when `layout_2020`/`layout_thread_2020` merged after legacy layout was removed (PR #36613). What "Servo layout" means today.
- **Layout 2013 / legacy layout** — Servo's original flow-tree layout engine; default-off behind a feature flag in 2024 (PR #32759), fully removed in 2025 (PR #35943).
- **Layout 2020** — the from-scratch rewrite separating box tree from fragment tree, matching CSS-spec structure. Now the *only* engine, so the "2020" name was dropped.
- **`libservo`** — Servo's Rust embedding API: a `WebView` plus `WebViewDelegate`/`ServoDelegate` model where the embedder owns the window and event loop and Servo calls back through delegates.
- **MPL-2.0** — Mozilla Public License 2.0; weak/file-level copyleft. Modifications to MPL files must stay MPL, but MPL files may combine with differently-licensed code in a larger work. Servo/Stylo/WebRender's license — diverges from Buiy's MIT/Apache.
- **`Parley`** — Linebender's text-layout crate, used by Blitz (not by Servo, which uses HarfBuzz bindings; not by Buiy, which uses `cosmic-text`).
- **pipeline** — Servo's per-frame unit (top-level document or each `<iframe>`), owned by the constellation; a script thread can own several.
- **Project Quantum** — Mozilla's 2016 plan to fold Servo's production-ready components into Gecko incrementally: Stylo (Quantum CSS), WebRender (Quantum Render), Quantum DOM.
- **Quantum CSS** — the shipping-in-Firefox name for Stylo, enabled by default in Firefox 57 (2017-11-14).
- **`rayon`** — the Rust work-stealing data-parallelism library Stylo uses to run the cascade across cores.
- **retained scene** — WebRender's persistent cross-frame data structure built from the display list; only changed subtrees are rebuilt.
- **rule tree** — Stylo's interned tree of matched-rule lists; paths are shared between elements matching the same rules, so restyle walks pointers instead of re-running selector matching. Inherited from Gecko.
- **script thread** — Servo's content-process component hosting the DOM, running JavaScript (via SpiderMonkey), servicing the event loop, and driving layout.
- **`selectors`** — Servo's selector parsing + matching crate (v0.38.0, repo `github.com/servo/stylo`), including the Bloom-filter logic (~44M downloads).
- **servoshell** — the reference shell / demo browser in the Servo repo; the dogfooding consumer of the embedding API.
- **`StackingContext` / `StackingContextTree`** — Servo's `display_list/stacking_context.rs` types; the tree is sorted by `z_index` and walked in CSS paint order to build the display list. Closest prior art to Buiy's sub-pass-6f `StackingContext { painters_z }`.
- **`StackingContextSection`** — Servo's paint-phase bucket enum (`OwnBackgroundsAndBorders`, `DescendantBackgroundsAndBorders`, `Foreground`, `Outline`) implementing CSS 2.1 Appendix E ordering.
- **style sharing cache** — Stylo's small LRU that lets a second element reuse a first element's `ComputedValues` outright when their style-determining inputs match. Borrowed from WebKit/Blink.
- **`style` crate / `stylo`** — Stylo's cascade engine (published crate name `stylo`, v0.17.0, repo `github.com/servo/stylo`); the CSS engine powering both Servo and Firefox. Standalone repo created 2024-02-07.
- **`stylo_taffy`** — the adapter crate (`TaffyStyloStyle`) that maps Stylo `ComputedValues` onto Taffy's style traits; lets Stylo pair with Taffy. MPL-2.0; now vendored in Blitz.
- **`swgl` / software WebRender** — Mozilla's CPU software rasterizer running the WebRender pipeline where the GPU path is untrusted (bad/old drivers).
- **`TaffyContainer` / `TaffyItemBox`** — Servo's wrappers (`components/layout/taffy/`) that drive Taffy for Servo's flexbox and grid formatting contexts.
- **Tauri-Servo experiment** — NLnet-funded effort to let Tauri use Servo as an embeddable webview instead of the OS webview.
- **TSC (Technical Steering Committee)** — Servo's committee governance: maintains the roadmap and decides donation spending under Linux Foundation Europe's neutral umbrella.
- **`unicode-bidi`** — the UAX #9 bidi crate Servo authored and the broader Rust ecosystem reuses.
- **Verso** — a browser built on Servo by a TSC member, exercising the features Servo needs to back a real browser (wrote its own compositor layer on top of Servo).
- **WebRender** — Servo's GPU renderer: display list → retained scene → frame builder → batching → single-pass GPU raster/composite. Upstreamed to Firefox (canonical home now `mozilla-central/gfx/wr`); `github.com/servo/webrender` is a downstream mirror. `webrender` crate v0.68.0.

## Sources

- Servo architecture / book: https://book.servo.org/architecture/overview.html ; https://book.servo.org/architecture/layout.html
- Stylo mechanisms: https://hacks.mozilla.org/2017/08/inside-a-super-fast-css-engine-quantum-css-aka-stylo/
- Crates: https://crates.io/crates/stylo ; https://crates.io/crates/selectors ; https://crates.io/crates/cssparser ; https://crates.io/crates/stylo_taffy ; https://crates.io/crates/webrender
- Layout source (box/fragment tree, Taffy, stacking context): https://github.com/servo/servo/tree/master/components/layout
- WebRender: https://mozillagfx.wordpress.com/2019/05/21/graphics-team-ships-webrender-mvp/ ; https://github.com/servo/webrender
- Blitz: https://github.com/DioxusLabs/blitz
- License MPL-2.0: https://github.com/servo/servo/blob/main/LICENSE
- Sibling files: [README.md](README.md), [lessons.md](lessons.md), [architecture.md](architecture.md), [stylo.md](stylo.md), [layout.md](layout.md), [rendering.md](rendering.md), [governance.md](governance.md), [history.md](history.md), [critiques.md](critiques.md), [open-problems.md](open-problems.md), [comparisons.md](comparisons.md)
