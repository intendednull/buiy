**Date:** 2026-05-22
**Status:** active
**Subject:** Slint — system-specific terms and named entities

# Glossary

System-specific vocabulary. See [`README.md`](README.md) for the corpus overview and [`lessons.md`](lessons.md) for synthesis.

## DSL and runtime

**`.slint`** — file extension for the Slint declarative DSL source files. Each file contains one or more `component` declarations; compiled at build time (Rust `slint!` macro or `slint_build` in `build.rs`; C++ via CMake; JS/Python via package-manager install hooks). See [`dsl-language.md`](dsl-language.md).

**`slint-build`** — the Rust build-time crate that drives `.slint` → Rust codegen from `build.rs`. The non-macro path; preferred over `slint!` for multi-file `.slint` projects. The compiler emits typed Rust structs that wrap the runtime's native object model.

**`slint_interpreter`** — runtime DSL evaluator. Parses `.slint` source at runtime and constructs UI without ahead-of-time codegen. Used by the Live Preview and the `slint-viewer` tool; available for application use where dynamic UI loading matters. Tradeoff: loses compile-time type checks. See [`architecture.md`](architecture.md).

**Compile-time vs runtime DSL evaluation** — Slint supports both. Compile-time is the default (typed codegen via `slint!` / `slint_build`); runtime is `slint_interpreter`. Choose at the level of the consuming application; mixing within one app is supported but uncommon.

**Property** — typed field on a component or item. Declared with `property <T>` plus optional direction qualifier (`in`, `out`, `in-out`). Can carry a binding expression (re-evaluated lazily when dependencies change). See [`dsl-language.md`](dsl-language.md).

**Property binding** — pure expression assigned to a property; tracked in a dependency graph; re-evaluated when inputs change. Bindings must be side-effect-free; the compiler enforces this. The load-bearing reactive primitive — `architecture.md` explains the binding-graph evaluator in depth.

**Callback** — declarative event-emission declaration on a component (`callback name(arg) -> ReturnType;`). Emitted from inside the component; handled by parent components (via `=>` blocks) or by host-language code. The imperative escape hatch — inside a callback handler, mutations and side effects are allowed; outside (in property bindings), only pure expressions.

**Two-way binding (`<=>`)** — bidirectional property connection with cycle detection. Used for form state — `TextInput { text <=> root.user-input; }` propagates changes both directions.

**States + transitions** — the declarative animation model. Named states (`states [ pressed when ...: { ... } ]`) group property settings; transitions (`transitions [ in pressed: { animate background { duration: 100ms; } } ]`) define how property changes are animated when entering or leaving each state. See [`dsl-language.md`](dsl-language.md).

**Component / Item** — runtime structural units. A *Component* is the compiled product of one `component` declaration in `.slint` source. An *Item* is a built-in primitive (`Rectangle`, `Text`, `TouchArea`, `Path`); components are composed of items and nested components.

## Renderers and backends

**Femtovg** — OpenGL ES 2.0 renderer. Lighter than Skia; used on Linux and embedded with GPU. Slint's pre-Skia default desktop renderer.

**Skia** — the Google-maintained 2D graphics library; default Slint renderer on Windows and macOS since 1.14 (October 2025). Hardware-accelerated; full feature parity; largest binary footprint of the renderer set.

**Wgpu** — Rust cross-platform GPU API library. FemtoVG-WGPU (added Slint 1.12, June 2025) is a `wgpu`-backed FemtoVG variant targeting Vulkan / Metal / DX12 / WebGPU. The renderer most directly comparable to the Bevy / Buiy stack, since Bevy is wgpu-based.

**Software renderer** — CPU rasterizer. No GPU required; supports RGB565 framebuffers; the MCU / embedded target. See [`embedded-focus.md`](embedded-focus.md).

**MCU target** — microcontroller targets (STM32, ESP32, Cortex-M devices). Bare-metal `no_std` Rust + Slint's software renderer + ESP-IDF / STM32 / Zephyr platform backends. The market that funds Slint's commercial-license revenue.

## Project organization and people

**SixtyFPS GmbH** — the legal entity behind Slint. Registered in Brandenburg state, Germany. Founded 2020 (originally as SixtyFPS the project; the company retained the legal name after the 2022 rebrand). Copyright holder on every Slint commit and the named licensor on the commercial license terms. See [`governance-and-distribution.md`](governance-and-distribution.md).

**Olivier Goffart** — Slint co-founder; ex-Trolltech / Qt Company. Long-time Qt core maintainer; primary maintainer of the Qt meta-object compiler (moc); co-founder of Woboq (consulting + the Woboq Code Browser, 2011). Most active committer on the Slint codebase.

**Simon Hausmann** — Slint co-founder; ex-Trolltech / Qt Company. Lead developer and maintainer of the QtQml engine; one of the canonical authors of QML's reactive bindings. The DSL expertise that underwrites Slint's binding evaluator.

**Aurindam Jana** — Slint co-founder; ex-Qt Company. Qt engineering manager background; technical and partner-relationship work. Listed as co-founder per Rust Foundation member spotlight and Slint about-us page.

**Tobias Hunger** (GitHub `@hunger`) — Slint software engineer, **not** a co-founder. (The original prior-art brief mis-listed Hunger as a founder; corrected per Slint's about-us page.) One of the most active contributors. (See history.md for PR #2865, which was authored by co-founder Simon Hausmann @tronical, not Hunger.)

## Licenses

**`GPL-3.0-only`** — the open-source gate. Slint code is GPL-3.0; binaries that link Slint are GPL-3.0 unless covered by one of the other two licenses. Open-source consumers under permissive licenses (MIT, Apache-2.0, BSD) must bump to GPL-3.0 to use Slint unless they qualify for the royalty-free terms.

**`LicenseRef-Slint-Royalty-free-2.0`** — the royalty-free option, added in Slint 1.1 (June 2023). Lifts the GPL gate for *desktop* proprietary applications under specific terms (excluding embedded and mobile, with revenue-threshold and attribution requirements). Terms can change between versions at SixtyFPS GmbH's discretion. See [`governance-and-distribution.md`](governance-and-distribution.md).

**`LicenseRef-Slint-Software-3.0`** — the commercial license. Mandatory for proprietary embedded and mobile deployment; mandatory for any proprietary use that needs guarantees beyond the royalty-free terms. The procurement-conversation path; this is the revenue stream that funds SixtyFPS GmbH. See [`open-problems.md`](open-problems.md) for Buiy's avoid-this stance and [`lessons.md`](lessons.md) for the borrowable transparency lesson.

## Tooling

**Slint Live Preview** — the WASM-rendered hot-reloading preview tool. Embedded in the VSCode extension as a split-pane that re-renders on save; also available as the standalone web editor at https://slint.dev/editor. Uses `slint_interpreter` to evaluate `.slint` source at runtime. See [`dsl-language.md`](dsl-language.md).

**Slint LSP / `slint-lsp`** — Language Server Protocol implementation for `.slint` files. Editor-agnostic — supports VSCode, Vim, Emacs, Helix, Sublime, etc. Provides autocomplete, go-to-definition, refactoring, diagnostics, and Live-Preview hooks.

**`slint-viewer`** — standalone binary that renders any `.slint` file. Used for sharing UI prototypes without a host application.

**Figma plugin** — added in Slint 1.10 (February 2025). Exports Figma designs as `.slint` code; supports Figma variables → Slint structs/enums/globals; honors Figma modes for theming. The design-to-code workflow.

## Accessibility

**AccessKit integration** — Slint has been an AccessKit producer since PR [#2865](https://github.com/slint-ui/slint/pull/2865) merged 2023-06-15 (before Slint 1.1). Producer-side wiring through `accesskit_winit`; per-window `Adapter` ownership; `TreeUpdate` push on property change. Slint is a named verified adopter in [`../accesskit/ecosystem.md`](../accesskit/ecosystem.md). See [`accessibility.md`](accessibility.md).

**`accessible-*` properties** — item-level DSL fields that map to AccessKit `Node` properties: `accessible-role`, `accessible-label`, `accessible-description`, `accessible-checked`, `accessible-expanded`, `accessible-action-default`, etc. The author-facing accessibility surface; the runtime stitches them into the AccessKit tree.

## Sources

- Slint About Us page: https://slint.dev/about-us
- Slint repo: https://github.com/slint-ui/slint
- Slint AccessKit PR #2865: https://github.com/slint-ui/slint/pull/2865
- Slint Online Editor: https://slint.dev/editor
- Slint VSCode extension: https://marketplace.visualstudio.com/items?itemName=Slint.slint
- Slint Figma plugin docs: https://docs.slint.dev/latest/docs/slint/guide/tooling/figma-inspector/
- Sibling files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`dsl-language.md`](dsl-language.md), [`accessibility.md`](accessibility.md), [`embedded-focus.md`](embedded-focus.md), [`history.md`](history.md), [`governance-and-distribution.md`](governance-and-distribution.md), [`lessons.md`](lessons.md), [`open-problems.md`](open-problems.md)
