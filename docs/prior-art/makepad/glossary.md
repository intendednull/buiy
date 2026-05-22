**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — terminology used in this prior-art folder

# Glossary

Makepad-specific terms and the conventions this corpus uses for them. When the same concept has different names across the prior-art folders, the cross-reference is named.

- **`.live` file.** A file containing Live DSL source, loaded at startup or via `dep()` paths. Hot-reloadable on disk change. Contrast with **`live_design!` macro** which embeds Live syntax inside a Rust source file.
- **`#[derive(Live)]`.** Derive macro that binds Rust `#[live]` fields to LiveRegistry values, enabling property assignment from Live DSL. Pairs with **`#[derive(Widget)]`** which registers the struct as a Makepad widget.
- **`#[live]` field.** A Rust struct field that receives its value from a Live DSL property binding. Contrast with **`#[rust]` field**, internal state not bound from Live.
- **action.** A Rust enum sent up the widget tree via the `Cx` action queue; the Makepad equivalent of Slint's `callback` or BSN's events / observers. Routed via `widget_uid` and `cast()` matching.
- **animator.** Live DSL block declaring named states and declarative transitions for animation. Shape: `animator: { hover = { default: off, off = { from, apply }, on = { from, apply } } }`. Comparable in spirit to Slint's `states + transitions`.
- **`cargo-makepad`.** Cargo subcommand installing cross-target toolchains (Android NDK, iOS signing, tvOS, OpenHarmony) and driving per-target builds + on-device runs.
- **`Cx` (context).** The per-frame god-object threaded through every widget method by reference. Holds draw state, event queue, asset registry, animation timeline, GPU backend handle, LiveRegistry reference. Buiy's nearest equivalent is the ECS `World` (different paradigm).
- **`Cx2d`.** The 2D drawing context, threaded through `Widget::draw_walk`. Exposes `DrawQuad`, `DrawText`, `DrawShader` primitives.
- **`dep("crate://self/icons/foo.svg")`.** Live syntax for resolving an asset path through the LiveRegistry's dependency resolver. Used for fonts, shaders, images, `.live` sub-files.
- **`DrawQuad` / `DrawShader` / `DrawText`.** Primitive draw widgets in `Cx2d`. Configured via Live properties; emit GPU draw calls.
- **Futurewei Technologies.** US-incorporated Huawei subsidiary; funds Kevin Boos's work on Project Robius / Robrix. See [`distribution-and-governance.md`](distribution-and-governance.md).
- **hot-reload.** The runtime mechanism re-parsing changed `.live` source and re-binding live struct fields without recompilation. Demonstrated in the `hotload_ui` example.
- **`live_design!` macro.** Proc-macro embedding Live DSL syntax inside a Rust source file. Compile-time-parsed; runtime-loaded into the LiveRegistry.
- **Live language.** Makepad's DSL — own syntax, embedded in `live_design!` macro or external `.live` files. Compiles via `makepad-live-compiler`.
- **`LiveHook`.** Trait with lifecycle callbacks (`after_apply`, `after_update_from_doc`, `before_apply`) triggered by the runtime when LiveRegistry data is applied to or updated against a `#[derive(Live)]` struct.
- **`LiveNode` / `LiveValue` / `LiveType`.** Core data types in the LiveRegistry. A `LiveNode` is one entry in the flat node array; `LiveValue` is the typed value; `LiveType` is a Rust type reference.
- **`LiveRegistry`.** The runtime data structure produced by the Live compiler — a flat `Vec<LiveNode>` holding all parsed Live data. Mutable; supports hot-reload by re-expansion.
- **`makepad-live-compiler`.** The crate that parses and expands `.live` source into a `LiveRegistry`. 0% documented per docs.rs at folder-write.
- **`makepad-platform`.** The crate containing the runtime, windowing, GPU backends, event loop. Equivalent role to "what wgpu + winit + a render-graph would do for a wgpu-based UI" but in-house.
- **`makepad-shader-compiler`.** The crate generating backend-specific shader source (MSL / HLSL / GLSL ES / GLSL) from inline Live shader snippets. 0% documented per docs.rs.
- **`makepad-widgets`.** The flagship public-API crate (1.0.0 at folder-write). Contains the widget catalog.
- **`makepaddev`.** The shared crates.io publisher account used by all 7 `makepad-widgets` releases. A project-level publisher, not a personal account.
- **Makepad Studio.** The Makepad IDE, itself built on Makepad. The framework's canonical dogfooded application. `cargo run -p makepad-studio --release`.
- **`okapii`.** GitHub handle for Sebastian Michailidis; #1 contributor on the current `dev` branch (154 commits per GitHub contributors API).
- **`pixel(self) -> vec4` function.** Inline GLSL-flavoured pixel shader inside a Live `draw_*` block. Compiled to backend-specific shader code by `makepad-shader-compiler`.
- **Project Robius.** The community led by Kevin Boos (Futurewei) that builds cross-platform apps on Makepad. Repo organization `github.com/project-robius`. Largest visible Makepad downstream consumer.
- **Robrix.** Matrix chat client built by Project Robius on Makepad. Most-visible Makepad downstream app. v1.0.0-alpha.1 (2026-05-05). 448 stars. Presented at Rust China Conf 2025, GOSIM 2024.
- **`Widget` trait.** The trait that every Makepad widget implements. Methods: `handle_event`, `draw_walk`, `widget_uid` etc.
- **`widget_uid`.** Unique identifier for a widget instance, used to route actions and address widgets across the tree. Buiy's analog is `Entity::to_bits()` (different mechanism, same role).

Cross-folder vocabulary correspondences:

| Makepad term | Slint term | Buiy term |
|---|---|---|
| Live DSL / `.live` | Slint DSL / `.slint` | BSN / `.bsn` (Rust syntax) |
| `live_design!` macro | `slint!` macro | `bsn!` macro |
| `#[derive(Live)]` + `#[live]` field | `in property` / `out property` qualifiers | ECS component with `#[derive(Reflect)]` |
| action (Rust enum) | `callback name(args)` | observer / event on entity |
| `LiveRegistry` | Slint runtime object tree | ECS `World` |
| Makepad Studio | Slint VSCode + Live Preview | (no canonical IDE yet) |
| `cargo-makepad` | `slint-build` + `slint-viewer` | (Bevy + `cargo-mobile2`) |
| `Cx` (context) | Slint runtime context | Bevy `World` |
| Animator (Live block) | `states + transitions` block | (planned in `buiy-animation-design`) |
| `dep("crate://self/...")` | `slint::include_modules!()` + `@import` | Bevy `AssetServer::load` |
| `<StackNavigation>` widget | (no direct equivalent) | (would be in `buiy-widget-catalog-design`) |
| `FingerDown` / `FingerMove` / `FingerUp` event | TouchEvent | (planned in `buiy-input-events-design`) |

## Sources

- Sibling files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`live-language.md`](live-language.md), [`gpu-rendering.md`](gpu-rendering.md), [`mobile-targets.md`](mobile-targets.md), [`history.md`](history.md), [`distribution-and-governance.md`](distribution-and-governance.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`open-problems.md`](open-problems.md), [`critiques.md`](critiques.md), [`lessons.md`](lessons.md)
- Makepad repo: https://github.com/makepad/makepad
- `makepad-widgets` docs.rs: https://docs.rs/makepad-widgets/1.0.0/
- Slint glossary (cross-reference): [`../slint/glossary.md`](../slint/glossary.md)
