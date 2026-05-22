**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — system-specific terms used across the corpus

# Glossary

| Term | Definition |
|---|---|
| **`VirtualDom`** | Dioxus's runtime root type (`dioxus_core::VirtualDom`); owns scope arena, dirty queue, scheduler, mutation output buffer. |
| **Scope** | Per-component-instance slot in the `VirtualDom` arena; owns hook state. Indexed by position in the tree. |
| **VNode** | A node in the virtual-DOM tree; bump-allocated per render. |
| **Mutation** | A single edit emitted by the diff (`AppendChildren`, `CreateElement`, `SetText`, `SetAttribute`, etc.). Backends drain a `Vec<Mutation>` and apply to the host. |
| **`rsx!`** | Dioxus's authoring proc-macro. Brace-shaped JSX-like syntax that compiles to VNode constructors. See [`rsx-macro.md`](rsx-macro.md). |
| **`Element`** | Opaque handle type returned by component functions (`fn Component() -> Element`). Represents a rendered `rsx!` tree. |
| **Signal (`Signal<T>`)** | Dioxus 0.5+ reactive primitive. `Copy` cell backed by generational-arena storage; reads in the render path are tracked. See [`signals-and-state.md`](signals-and-state.md). |
| **Store** | Dioxus 0.7+ `#[derive(Store)]`-generated nested-reactivity primitive. Per-field signal accessors; per-collection-entry subscriptions. |
| **generational-box** | DioxusLabs-stewarded crate that implements the `Copy`-handle arena underlying signals. Carved out of `dioxus-signals` in 0.5. |
| **Hook** | A function called during component render that interacts with scope state (`use_signal`, `use_effect`, `use_memo`, `use_context`, `use_resource`, etc.). Follows React's positional-hook rule. |
| **Subsecond** | Dioxus 0.7's runtime code-modification system. Uses incremental linking + explicit `subsecond::call()` integration points to hot-patch Rust code across WASM + desktop + mobile. |
| **Manganis** | DioxusLabs's asset-bundling system. Stabilized in 0.6. Automatic optimization (AVIF/WebP conversion, asset hashing). |
| **`dx`** | The Dioxus CLI tool (`dioxus-cli` crate). `dx serve`, `dx build`, `dx bundle`, `dx fmt`. |
| **Backend / Renderer crate** | A consumer of `VirtualDom`'s mutation stream: `dioxus-web`, `dioxus-desktop`, `dioxus-native`, `dioxus-mobile`, `dioxus-ssr`, `dioxus-fullstack`. See [`targets.md`](targets.md). |
| **Blitz** | DioxusLabs-stewarded HTML/CSS rendering engine (pre-alpha). Uses Stylo (CSS) + Taffy (layout) + Parley (text) + Vello (GPU draw). Powers `dioxus-native`. |
| **`blitz-dom`** | Blitz's DOM-tree crate; implements Taffy's `LayoutPartialTree` / `TraversePartialTree` / `CacheTree` traits against its own node arena. |
| **`stylo_taffy`** | Glue crate translating Stylo's `ComputedValues` into Taffy's `Style`. MPL-2.0 for Servo-interop. |
| **Stylo** | Mozilla/Servo standalone CSS engine. Used by Blitz for parsing + cascade. MPL-2.0. |
| **Vello** | Linebender GPU vector-rasterization library. Used by Blitz via the Anyrender abstraction. |
| **Parley** | Linebender text-shaping and layout library. Used by Blitz for text. |
| **Anyrender** | 2D drawing abstraction used by Blitz; Vello is the current backend. |
| **`use_signal`** | The dominant state hook; replaces `use_state` / `use_ref` from 0.4. |
| **`use_effect`** | Side-effect hook; re-runs when read signals change. |
| **`use_memo`** | Derived-value hook with caching. |
| **`use_resource`** | Async data hook integrated with Suspense. |
| **Suspense** | Boundary that collects pending async work; re-renders when futures resolve. SSR variant supports streaming. |
| **Server function** | Rust function decorated with `#[server]`; callable from client code, executes server-side via Axum. Part of `dioxus-fullstack`. |
| **DioxusLabs** | The company; San Francisco-based US corp, YC S23. Stewards Dioxus, Taffy, Blitz, dioxus-cli, manganis, sledgehammer, generational-box. |
| **Jonathan Kelley** (@jkelleyrtp) | Dioxus founder + lead. Sole publisher of `dioxus` crate versions on crates.io. |
| **FutureWei** | US R&D subsidiary of Huawei. Sponsor of Dioxus (not an equity investor). |
| **YC S23** | Y Combinator Summer 2023 batch; the cohort Dioxus Labs joined. |
| **Sledgehammer** | DioxusLabs's web-target DOM-mutation library; tagged-pointer batched-command encoding. |

## Cross-reference glossary

For terms from sibling folders:

- **Taffy** terms (`TaffyTree`, `LayoutPartialTree`, `Style`, `CompactLength`, `MeasureFunc`) — see [`../taffy/glossary.md`](../taffy/glossary.md).
- **Bevy UI** terms (`Node`, `ComputedNode`, `BSN`, `RequiredComponents`, `bevy_a11y`) — see [`../bevy-ui/`](../bevy-ui/) (no glossary file yet; terms appear in [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)).
- **bevy_egui** terms (`EguiContext`, `EguiPickingOrder`, immediate-mode `Id`) — see [`../bevy-egui/glossary.md`](../bevy-egui/) (if present) or [`../bevy-egui/README.md`](../bevy-egui/README.md).

## Sources

- Dioxus repo: https://github.com/DioxusLabs/dioxus
- Dioxus docs: https://docs.rs/dioxus/
- Blitz repo: https://github.com/DioxusLabs/blitz
- dioxus-signals crate: https://crates.io/crates/dioxus-signals
- generational-box crate: https://crates.io/crates/generational-box
