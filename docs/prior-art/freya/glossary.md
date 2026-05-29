**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — glossary

# Glossary

Terms specific to Freya, its substrate, or commonly confused in the surrounding ecosystem.

- **Freya.** A cross-platform native (non-web) GUI library for Rust, powered by Skia for rendering and Dioxus for reactivity. Created 2022-07-27 by Marc Espín Sanz. MIT-licensed. Current pre-release: `0.4.0-rc.19` (2026-04-23).
- **`freya` (the meta-crate).** The top-level workspace crate that re-exports common API surface and ships `launch()` / `launch_cfg()` entry points.
- **`freya-core`.** The renderer + scheduler that bridges Dioxus VDOM mutations into the retained scene tree.
- **`freya-engine`.** The Skia-abstraction layer; the seam where Freya talks to `freya-skia-safe`.
- **`freya-elements`.** Defines primitive elements (`rect`, `label`, `paragraph`, `image`, `svg`, `text`) usable inside `rsx!`.
- **`freya-components`.** Pre-built widgets composed from elements: `Button`, `Slider`, `VirtualScrollView`, `Calendar`, etc.
- **`freya-hooks`.** Freya-specific reactive hooks: `use_focus`, `use_animation`, `use_canvas`, `use_node`, `use_theme`, `use_platform`.
- **`freya-winit`.** winit-integration crate; handles event loop, IME, clipboard, cursor.
- **`freya-skia-safe`.** A fork (or vendored variant) of `rust-skia`'s `skia-safe` crate that Freya depends on. Current version `0.96.1` with `textlayout`, `svg`, `webp` features.
- **Torin.** Freya's own pure-Rust layout engine. Lives at `crates/torin/`. Not Taffy. Description: *"UI layout Library designed for Freya."* Custom flexbox-flavored model (own model, not CSS-spec-conformant).
- **`rsx!`.** Dioxus's procedural macro for declarative UI. Freya reuses it, with Freya-specific element schema (`rect` not `div`).
- **Element.** A primitive node in Freya's scene tree (e.g. `rect`, `label`). Maps to a render-tree node. Defined in `freya-elements`.
- **Component.** A Dioxus function component that returns `Element`. Composes elements + other components. Stateful via hooks.
- **Skia.** Google's 2D graphics engine, written in C++. Powers Chrome, Android, Flutter. Provides Freya's entire rendering surface.
- **Skia textlayout.** Skia's text layout / paragraph builder API. Handles BiDi, line breaking, shaping. Pulled in via the `textlayout` feature on `freya-skia-safe`. **NOT** cosmic-text.
- **Dioxus.** The Rust UI framework whose reactivity primitives (signals, components, scopes, `rsx!`) Freya consumes as a library. Dioxus is *not* Freya's renderer — Dioxus has its own renderers (web/desktop-webview/native-Blitz/mobile); Freya is a community-maintained alternative renderer outside DioxusLabs.
- **Signal.** A `Copy` handle backed by a generational-arena slot, providing fine-grained reactivity. From `dioxus-signals` (via `generational-box`). See [`../dioxus/signals-and-state.md`](../dioxus/signals-and-state.md).
- **AccessKit.** Cross-platform accessibility-tree library (UIA / NSAccessibility / AT-SPI). Freya depends on `accesskit 0.24.0` + `accesskit_winit 0.32.0`. See [`../accesskit/`](../accesskit/).
- **Subsecond.** Dioxus's hot-reload runtime. Freya inherits hot-reload from Dioxus 0.6+. See [`../dioxus/history.md`](../dioxus/history.md).
- **`use_focus`.** Freya hook returning a `FocusManager` for the current scene node. Wires focus state into AccessKit + keyboard navigation.
- **`use_canvas`.** Freya hook giving a component a raw Skia `Canvas` for custom paint. The escape hatch from the declarative model.
- **`marc2332`.** GitHub username of Marc Espín Sanz, Freya's creator and primary maintainer. Based in Barcelona, Spain. Self-describes as web frontend developer working on Rust in spare time. Member of `@tauri-apps` and `@dioxus-community` GitHub organizations.
- **PR #1351.** The "0.4 rewrite" — a substantial Freya internals overhaul referenced on the official Freya site. The reason `main` differs significantly from `0.3.x` stable.
- **`launch()` / `launch_cfg()`.** Top-level entry points in the `freya` meta-crate to start a Freya app, analogous to Bevy's `App::new().run()`.
- **Common misattribution: "Freya uses cosmic-text."** **FALSE.** Freya uses Skia's `textlayout`. cosmic-text is used by Iced, COSMIC, Bevy (until 0.19-dev), and a long tail of editors. See [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) line 35.
- **Common misattribution: "Freya uses Taffy."** **FALSE.** Freya uses Torin (its own engine). Taffy is used by Blitz, bevy_ui, Servo, Buiy.

## Sources

- Freya workspace `Cargo.toml` — https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml
- Freya docs.rs — https://docs.rs/freya/latest/freya/
- Cross-references: [`../dioxus/glossary.md`](../dioxus/glossary.md), [`../cosmic-text/glossary.md`](../cosmic-text/glossary.md) (if present), [`../accesskit/`](../accesskit/), [`README.md`](README.md).
