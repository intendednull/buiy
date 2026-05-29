**Date:** 2026-05-22
**Status:** active
**Subject:** Glossary — Xilem, Masonry, and the Linebender stack terms used across this folder

# Glossary

System-specific terms used in this folder. Cross-reference with `../accesskit/glossary.md` and `../cosmic-text/glossary.md` for substrate-shared terms.

| Term | Definition |
|---|---|
| **Xilem** | Linebender's reactive UI framework. Latest 0.4.0 (2025-10-29). View-tree-diffing architecture on top of Masonry. Apache-2.0. |
| **Masonry** | Linebender's lower-level retained-mode widget toolkit. Latest 0.4.0 (2025-10-29). Widget tree, paint passes, AccessKit integration. Apache-2.0. |
| **Linebender** | Volunteer-run Rust UI collective. Started by Raph Levien (~2018). Stewards Xilem, Masonry, Vello, Parley, Skrifa, Fontique, Kurbo, Peniko, Color, Druid (legacy), and more. |
| **Vello** | Linebender's GPU-accelerated 2D renderer (compute-shader-based). Used by Masonry and `bevy_vello`. |
| **Parley** | Linebender's text layout engine (paragraph layout, line breaking, BiDi). Uses harfrust for shaping. Bevy 0.19-dev migrated bevy_text to Parley. |
| **Skrifa** | Font data parser (loads TTF/OTF tables). Part of `googlefonts/fontations`. Used by Parley. |
| **Fontique** | Font enumeration + fallback library. Part of the Parley workspace. |
| **Kurbo** | Linebender's 2D curve / path / Bézier library. *The* Rust 2D-curve library. |
| **Peniko** | Linebender's 2D paint primitives (brushes, colors, gradients, images). Sits between Kurbo and Vello. |
| **Linebender Color** | Modern color-space library (sRGB, OKLab, Display-P3, ICC). Separate Linebender crate. |
| **Druid** | Linebender's original retained-mode UI framework (2018–2023). Officially discontinued; replaced by Xilem/Masonry. |
| **Piet** | Druid-era 2D rendering abstraction. Superseded by Vello + Peniko. |
| **View** | Xilem's central abstraction: a value implementing `View<State, Action, Element>` with `build` / `rebuild` / `teardown` / `message` methods. |
| **ViewSequence** | Trait implemented by tuples / vectors / options of views. Lets a parent contain N children of varying types. |
| **ViewMarker** | Marker trait Xilem uses to identify view types in the type system. |
| **ViewCtx** | Context passed to view methods; carries the runtime's bookkeeping (id paths, state access, etc.). |
| **Adapt** | Xilem view that projects parent state to child state via a closure. Druid-Lens's descendant. |
| **Memoize** | Xilem view wrapper that skips inner build/rebuild when input `Data: PartialEq` is unchanged. |
| **Id path** | The Xilem-paper concept of a path from root to view-tree-leaf. Used for routing messages back from leaf events to the root state with mutable access at every level. |
| **Pod** | Container holding a Masonry widget instance + its bookkeeping; used by Xilem to interact with the widget. |
| **WidgetPod** | Masonry's internal widget container (id, layout-rect, paint flag, accessibility flag). |
| **WidgetId** | Stable `NonZeroU64` Masonry assigns to each widget. Used as the `accesskit::NodeId`. |
| **BoxConstraints** | Masonry's layout primitive: `(min_width, max_width, min_height, max_height)` passed parent-to-child. Flutter/Druid lineage. |
| **AccessCtx** | Context passed to `Widget::accessibility`; carries window state, AccessKit adapter handle, etc. |
| **PropertiesRef** | Read-only widget-property bundle Masonry passes through during accessibility and layout calls. |
| **tree_arena** | Separately versioned Linebender crate; stores Masonry's retained widget tree. |
| **Placehero** | Mastodon client example in `linebender/xilem` repo; the closest thing to a non-trivial Xilem app at 0.4.0. |
| **xilem_web** | Separate Xilem crate using the DOM (not Masonry/Vello) for web. Architecturally a different framework with shared paradigm. |
| **harfrust** | Rust port of HarfBuzz (text shaper). Used by both Parley (Linebender) and cosmic-text (Buiy / Iced / Lapce / Zed / COSMIC). |
| **Velato** | Linebender Lottie animation runtime over Vello. |
| **Vello SVG** | Linebender SVG renderer over Vello. |
| **MSRV** | Minimum Supported Rust Version. Xilem/Masonry 0.4.0 released with 1.88; workspace HEAD at 1.92. |
| **AccessKit** | Cross-platform accessibility bridge (UIA, NSAccessibility, AT-SPI, Android, iOS). Used by Masonry, Buiy, bevy_a11y, Iced, Slint, Freya. See [`../accesskit/`](../accesskit/). |

## Sources

- Xilem docs.rs (0.4.0): https://docs.rs/xilem/0.4.0/xilem/
- Masonry docs.rs (0.4.0): https://docs.rs/masonry/0.4.0/masonry/
- Xilem paper (Raph Levien, 2022-05-07): https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html
- Linebender about: https://linebender.org/about/
- Cross-link glossaries: [`../accesskit/glossary.md`](../accesskit/glossary.md), [`../cosmic-text/glossary.md`](../cosmic-text/glossary.md)
