**Date:** 2026-05-22
**Status:** active
**Subject:** Slint — DSL-compiler-as-source-of-truth architecture; retained-mode runtime; native + WASM + JS targets

# Architecture

Slint's load-bearing architectural choice is that the **`.slint` DSL is the source of truth**, not the host language. A `.slint` file is parsed by the Slint compiler at build time (Rust: macro `slint!` or `slint_build` in `build.rs`; C++: CMake invocation; JS / Python: `npm install` / `pip install` ships the compiler as a native binary), and the compiler emits host-language code that constructs and mutates a tree of native UI objects at runtime. The runtime is **retained-mode** — the object tree persists across frames; property bindings re-evaluate when their inputs change; the renderer redraws only dirty regions.

This shape is the *opposite* of Bevy's ECS-first model. In Bevy, the host language (Rust) constructs entities and components; UI is one consumer of the ECS. In Slint, the DSL constructs the UI; the host language is glue. Buiy commits to the Bevy shape ([`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.4, BSN-native goal 3); Slint is the comparison case.

## The compiler pipeline

1. **Parse**: `.slint` source → AST. The grammar is documented in the Slint language reference; the parser is hand-written in Rust.
2. **Lower**: AST → typed IR. Property types are inferred or declared; binding expressions are checked for purity (binding expressions must be side-effect-free; the compiler enforces this).
3. **Optimize**: dead-code elimination, constant folding on binding graphs, layout-cache hoisting, component-instance inlining where statically safe.
4. **Codegen**: emit host-language code. For Rust, the macro / build.rs path emits Rust structs + `impl` blocks that wrap the native runtime. For C++, headers are emitted. For JS / Python, the binding glue is generated.
5. **Link**: the host-language toolchain (cargo / cmake / npm / setuptools) compiles the generated code against `slint`'s runtime crate (Rust) or `libslint_cpp` (C++) or a native `.so` / `.dylib` / `.dll` (JS / Python).

The runtime crate (`slint`) ships the native object model, the property binding evaluator, the layout solver, the renderers, and the platform backends. The compiler is the heavy lift; the runtime is a relatively thin VM-over-Rust.

There is also an **interpreter path**: `slint-interpreter` parses `.slint` files at runtime and constructs UI without ahead-of-time codegen. This is the path used by the Live Preview and the `slint-viewer` tool, and it's available for application use where dynamic UI loading is needed — at the cost of losing some compile-time checks.

## The runtime model

Slint's runtime is **retained-mode** in the textbook sense:

- A **Component** instance owns a tree of **Item**s (e.g. `Rectangle`, `Text`, `TouchArea`, `ListView`).
- Items carry **properties** (typed fields: `length`, `color`, `string`, `bool`, `int`, `float`, `image`, custom structs, enums).
- Properties carry **bindings** — pure expressions over other properties. The runtime maintains a dependency graph; when a property changes, dependent bindings are marked dirty and re-evaluated lazily (on next access, or on next frame for layout-affecting properties).
- **Callbacks** (declared with `callback name(arg)` syntax) are the imperative interaction primitive — items emit callbacks, the host language handles them; bindings can connect callback emissions to property mutations.
- **States** are named groupings of property settings, with transitions between them; declarative animations apply during transitions.
- The **event loop** is owned by the active backend (winit for desktop / mobile, Qt for Qt-styled apps, custom for embedded). The host language hands control to the event loop; the runtime calls back into host-language callback handlers as user input arrives.

## Targets: native + WASM + JS / Python

Slint compiles to three deployment shapes:

1. **Native binary.** Rust → `cargo build`. C++ → CMake build. The runtime crate links statically; the resulting binary embeds the renderer of choice and the platform backend (winit, Qt, Android-NDK, iOS, ESP-IDF, etc.).
2. **WASM.** Rust → `wasm-pack` / `wasm-bindgen` with the winit backend in `wasm32-unknown-unknown` mode. The Live Preview itself is a WASM build. Note: WASM accessibility is gated on AccessKit's web adapter (which **does not exist yet**, per [`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md)) — Slint's WASM target ships visual / input / layout but not full a11y.
3. **JS / Node + Python.** The Slint compiler emits a native module (`napi-rs` for Node.js, PyO3 for Python) that the JS / Python program imports. The compiler is invoked from the host package manager's install step. Python bindings landed in 1.13.0; Node was ported to napi-rs 3.0 in 1.16.0.

The DSL is **once-authored, deployed-everywhere** — the same `.slint` file produces native, WASM, JS, and Python UIs without source changes. This is the load-bearing pitch and it largely works in practice; cross-language differences live in how callbacks are exposed (sync vs async; futures vs promises) and in renderer feature coverage by backend.

## Renderer backends

Slint ships four renderers (selected at compile time via Cargo features in Rust; CMake options in C++):

- **Skia** — default on Windows / macOS since 1.14 (October 2025). Hardware-accelerated; full feature parity; largest binary footprint.
- **FemtoVG** — OpenGL ES 2.0 renderer. Lighter than Skia; cross-platform; used on Linux and embedded with GPU.
- **FemtoVG-WGPU** — `wgpu`-backed FemtoVG, added in 1.12 (June 2025). Modern GPU API; works on Vulkan / Metal / DX12 / WebGPU.
- **Software** — CPU rasterizer. The MCU / embedded story. No GPU required; supports RGB565 framebuffers; dirty-region partial updates; the renderer Buiy's [embedded-focus.md](embedded-focus.md) discussion centers on.
- **Qt** — uses Qt's native style engine when Qt is installed. The "look native on each desktop OS" backend. Optional.

The renderer choice is orthogonal to the platform backend (winit, Qt, ESP-IDF, etc.) — winit + Skia is the default desktop combo; winit + software is the embedded combo.

## Property-binding graph: the load-bearing primitive

The single most-studied piece of Slint's runtime is the property binding evaluator. Properties form a directed dependency graph; binding expressions are pure (compiler-enforced); evaluation is lazy and memoized; updates propagate via dirty marking. This is the "reactive" core that lets the DSL look declarative without paying for per-frame full re-evaluation.

The property binding model is what Buiy's foundation spec ([`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) § state/data/reactivity, currently observers+change-detection-only-in-v1) explicitly *defers* — Bevy's observers + change-detection are Buiy's primitive instead. But "should a signal/computed/effect layer ship as a follow-up sub-spec?" is open (foundation README § 5), and Slint's binding evaluator is the canonical Rust-ecosystem reference for what that would look like.

## Implications for Buiy

- **DSL-as-source-of-truth is a load-bearing architectural commitment, not a tooling layer.** Slint's "the `.slint` file is the truth, the host language is glue" is the inverse of Buiy's "the ECS world is the truth, BSN is one of many authoring surfaces over it." A future Buiy spec proposing "let's ship a DSL" would be proposing to flip this — which would conflict with the foundation spec's goal 3 (BSN-native, components-are-the-API).
- **Retained-mode runtime is the right model for productivity-app UI.** Slint validates Buiy's parallel-stack choice on this point — the alternative (immediate-mode, like egui) loses on text editing, IME, focus model, and accessibility tree continuity. Buiy's retained-mode commitment (foundation README goal 1) is shared with Slint.
- **Cross-language emission of a single source is real but expensive to maintain.** Slint's host-language bindings (C++, JS, Python) are a continuing cost — Python landed in 1.13 (September 2025), napi-rs 3.0 port for Node landed in 1.16 (April 2026). Buiy's "Rust-only, ECS-and-BSN-native" choice is a smaller surface and that's a feature, not a missing capability.
- **Property-binding graph is a useful study target for a future Buiy reactivity sub-spec.** If Buiy adds a signal/computed/effect layer (foundation README § 5 open question), Slint's binding evaluator is the closest Rust-ecosystem reference. The purity constraint (binding expressions must be side-effect-free, compiler-enforced) is the most-borrowable property.

## Sources

- Slint repo: https://github.com/slint-ui/slint
- Slint language reference: https://docs.slint.dev/latest/docs/slint/
- Slint docs.rs (runtime crate): https://docs.rs/slint/1.16.1/slint/
- Slint blog (architecture posts): https://slint.dev/blog/
- `slint-interpreter` crate: https://crates.io/crates/slint-interpreter
- Buiy foundation architecture: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation cross-cutting (reactivity): [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- Sibling files: [`dsl-language.md`](dsl-language.md), [`embedded-focus.md`](embedded-focus.md), [`accessibility.md`](accessibility.md)
