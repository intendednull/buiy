**Date:** 2026-05-22
**Status:** archived
**Subject:** kayak_ui — system-specific terms used in this corpus.

# Glossary

- **`rsx!`** — kayak_ui's React-style JSX-analog proc-macro for declaring widget trees. Exported from the `kayak_ui_macros` crate alongside a `constructor` macro. Expands to ECS spawn calls against kayak_ui-known `Bundle` tag types.
- **`KayakContextPlugin`** — the entry-point Bevy `Plugin` consumers added to `App::add_plugin(...)`. Registered systems and resources for kayak_ui's render + layout + input + focus subsystems.
- **`KayakWidgets`** — separate Bevy `Plugin` that registered the bundled default widget set (`KButton`, `KWindow`, `TextBox`, `KImage`, `KSvg`, `NinePatch`, `Accordion`, `ScrollBox`, ...). Opt-in; consumers shipping their own widget set could omit it.
- **`KayakUIPlugin`** — a kayak_ui-internal trait (NOT a Bevy `Plugin`) used to extend a `KayakRootContext` with additional widgets / systems. `fn build(&self, context: &mut KayakRootContext);`. The naming collision with `Plugin` was a documented confusion source.
- **`KayakWidgetsContextPlugin`** — the `KayakUIPlugin` impl that registered the bundled default widget set into a specific `KayakRootContext`.
- **`KayakRootContext`** — the per-app kayak_ui state container. Held the widget tree, focus tree, layout cache, render data. Roughly analogous to a React root or a virtual-DOM root.
- **`KStyle`** — the styling struct (CSS-like fields), passed via the `styles={...}` attribute on every widget. The widget-level styling primitive.
- **`OnEvent`** — wrapper for a single widget's event-handler closure. Took an `EventDispatcher` + a `KEvent` payload.
- **`KEvent`** — the kayak_ui event payload enum: `MouseDown`, `MouseUp`, `Click`, `Hover`, `MouseLeave`, `CharEvent`, `KeyEvent`. Routed through the kayak widget tree, **not** through Bevy's entity tree.
- **`EventDispatcher`** — the per-`KayakRootContext` event-routing resource. One per kayak_ui context.
- **`FocusTree`** — the focus-management resource added in 0.5.0. Held the canonical focus state for a kayak_ui context. Operated against `Focusable`-marked entities.
- **`Focusable`** — marker struct opting a widget into the kayak_ui focus tree.
- **`MaterialUI`** — extension point for custom UI shaders. The kayak_ui analogue of bevy_ui's later `UiMaterial`.
- **`CameraUIKayak`** — marker component placed on the camera entity that should render kayak_ui's UI. Identifies which camera the kayak_ui render pass targets.
- **`KayakUICameraPlugin`** — the camera-side wiring plugin.
- **`WindowSize`** — resource tracking the Bevy window dimensions for kayak_ui's render scale calculations.
- **`DrawUiGraph` / `KayakUiPass`** — kayak_ui's custom render graph node, added to the Bevy render graph in parallel to bevy_ui's render.
- **`DEFAULT_FONT`** — constant `"Kayak-Default"`, the bundled MSDF font asset key.
- **MSDF** — Multi-channel Signed Distance Field. The font-rendering technique kayak_ui chose: sharp at arbitrary scale but requires pre-baked MSDF font assets. Not the same as bevy_ui's bitmap-atlas approach.
- **morphorm** — the one-pass layout engine kayak_ui chose. Maintained by the [vizia](https://github.com/vizia) project. Pinned at version 0.3. Not Taffy.
- **`bevy-track` branch** — kayak_ui's `main`-tracking development branch for Bevy `main` consumers. Never released to crates.io; never reached Bevy 0.13 compat in any usable form.
- **`KayakUIPlugin` trait** — see entry above. Distinct from a Bevy `Plugin` despite the name.
- **passive abandonment** — the failure mode where a project's maintainer stops working on it without issuing an archive banner, deprecation notice, or `cargo yank`. Contrast with **deliberate archive** (e.g. `bevy_cosmic_edit`'s 2025-03-21 archive). kayak_ui exemplifies passive abandonment.

## Sources

- kayak_ui prelude — https://docs.rs/kayak_ui/0.5.0/kayak_ui/prelude/index.html
- kayak_ui widgets module — https://docs.rs/kayak_ui/0.5.0/kayak_ui/widgets/index.html
- kayak_ui book chapter 1 — https://github.com/StarArawn/kayak_ui/blob/main/book/src/chapter_1.md
- morphorm crate — https://github.com/vizia/morphorm
