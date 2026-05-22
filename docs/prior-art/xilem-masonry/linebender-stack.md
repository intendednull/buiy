**Date:** 2026-05-22
**Status:** active
**Subject:** The full Linebender stack — Vello, Parley, Skrifa, Fontique, Kurbo, Peniko, Color, Masonry, Xilem, Druid (legacy)

# The Linebender stack

Linebender is a **volunteer-run collective**, organized by Raph Levien, that ships an unbundled Rust UI substrate. The decomposition is the point: each crate solves one problem, and any one of them can be adopted independently by anyone (Bevy, woodpecker_ui, Iced, GPUI, etc.). This is the **opposite of monolithic** — and it's the closest existing-art reference for Buiy's "parallel-to-bevy_ui stack of focused primitives" approach.

## The layer cake

From lowest substrate to highest:

```
┌─────────────────────────────────────────────────────┐
│ Xilem            (reactive layer; views, diffing)   │  ← application authors
├─────────────────────────────────────────────────────┤
│ Masonry          (retained widget tree, lifecycle)  │  ← framework authors
├─────────────────────────────────────────────────────┤
│ Parley           (text layout: line breaking, BiDi) │
│ Fontique         (font enumeration, fallback)       │
│ Skrifa           (font data parsing; read-fonts)    │
│ Vello            (GPU 2D rendering; compute shader) │
│ Peniko           (paint / brush / image primitives) │
│ Kurbo            (2D curves / paths / Bézier)       │
│ Color            (color spaces, OKLab, ICC)         │
├─────────────────────────────────────────────────────┤
│ wgpu             (GPU abstraction, not Linebender)  │
│ winit            (windowing, not Linebender)        │
│ AccessKit        (a11y bridge, Linebender-adjacent) │
└─────────────────────────────────────────────────────┘
```

Plus archived / legacy:

- **Druid** — original retained-mode framework (Raph started at Google ~2018, moved under Linebender). Officially discontinued. Replaced by Xilem/Masonry.
- **Piet** — abstract 2D rendering API. Superseded by Vello + Peniko.
- **Druid-shell** — windowing layer. Superseded by direct winit use.
- **Skribo** — early text shaper. Superseded by Parley + harfrust.
- **Runebender** — font editor. Maintained but quiet.

And ancillaries:

- **Velato** — Lottie animation runtime over Vello.
- **Vello SVG** — SVG renderer over Vello.
- **Norad** — UFO font format library (font tooling).
- **Interpoli, Kompari** — smaller utilities.

## Why the unbundling matters

Linebender's bet is that **the UI substrate is too complex for one crate to own**. Splitting it into 10+ focused crates means:

- Bevy can adopt Vello (`bevy_vello`) and Parley (Bevy 0.19-dev) independently without taking on a UI framework.
- woodpecker_ui can ship a Vello renderer via `bevy_vello` without taking on Linebender's UI framework.
- GPUI / Iced / Slint can pick which Linebender pieces they want.
- Embedded / no_std targets can use Kurbo or Color alone.
- Buiy can study the substrate without taking a dependency.

The same unbundling argument is **exactly** Buiy's foundation rationale ([`architecture.md § 2.2`](../../specs/2026-05-07-buiy-foundation/architecture.md)) — pick the primitives directly, don't depend on a framework that bundles them. The closest existing-art demonstration that this works at scale is Linebender's own example: their primitives are *adopted by their competitors* (Bevy, GPUI, woodpecker_ui). That's the strongest possible signal that the unbundled-substrate posture is viable.

## Per-crate snapshot

### Vello (~0.6.0 at 0.4.0 release; 0.8.0 in workspace HEAD)
GPU-accelerated 2D renderer using compute shaders. Renders arbitrary path-fills (anti-aliased), strokes, gradients (linear / radial / sweep), images, blurs, blends, clips. The compute-shader-based approach (rather than triangulation) means complex paths render correctly without subdivision tricks. **Used by:** Masonry (paint pass), Bevy via `bevy_vello`, woodpecker_ui via `bevy_vello`, several embedded experiments.

For Buiy: Vello is the *feasibility witness* for "complex 2D primitives can render on top of wgpu without going through a triangulator." Buiy's own render pipeline doesn't depend on Vello but the capability set Vello demonstrates (rounded clipping, `clip-path` shapes, backdrop-filter, gradients in any color space) is the Buiy-spec target.

### Parley (~0.6.0 at 0.4.0 release; 0.8.0 in HEAD)
Text layout engine. Takes a text + style spans, returns laid-out lines with glyph positions. Uses **harfrust** under the hood for shaping (the Rust port of HarfBuzz; same shaper cosmic-text uses). Layered on Skrifa for font data and Fontique for font selection.

**vs cosmic-text:** Both use harfrust. Parley is *layout-focused* (paragraphs, line-breaking, BiDi); cosmic-text is *layout-focused too*, but with a different API shape (cosmic-text exposes a `Buffer` model designed for text editing). Bevy 0.19-dev migrated to Parley (issue #21765); Buiy stays on cosmic-text. The technical differences are documented in [`text-and-rendering.md`](text-and-rendering.md).

### Skrifa
Font data parser (loads TTF / OTF tables, exposes glyphs, font metrics, variation axes). Built on **read-fonts**, a zero-copy font-table reader. Used by Parley, Fontique, can also be used standalone.

### Fontique
Font enumeration + fallback. Given a script, language, style, weight, generic family name, returns a candidate font from the system's installed fonts. The fallback chain handles missing-glyph scenarios (Latin text with a Chinese glyph mid-string).

### Kurbo
2D curve / path / Bézier library. Path-flattening, intersection, area, bounding-box. **Used by:** Vello (path input), Peniko (path-flattening). Used standalone by many graphics libraries — kurbo is **the** Rust 2D-curve library.

### Peniko
2D paint / brush / image primitives. `Brush`, `Color`, `Gradient`, `Image`, `Stroke`. Sits between Kurbo (paths) and Vello (renderer). Provides the data types Vello consumes.

### Color
Modern color-space library. sRGB, OKLab, Display-P3, HSL, ICC profiles, color interpolation in arbitrary spaces. Linebender published this as a separate crate after concluding that color is its own deep subject.

Buiy's foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) commits to "gradients in any color space" — Linebender Color is the closest published Rust library that handles arbitrary color-space interpolation correctly. Worth studying when Buiy's render pipeline implements gradient sampling.

### Masonry
See [`masonry-toolkit.md`](masonry-toolkit.md).

### Xilem
See [`xilem-architecture.md`](xilem-architecture.md).

### Druid (legacy)
See [`history.md`](history.md). Officially discontinued.

## The Linebender-vs-Bevy mirror

Both Bevy and Linebender are doing the same architectural move at different ecosystem levels:

- Bevy: ECS at the bottom, render-graph above, bevy_ui / bevy_text / bevy_render as integrated subsystems.
- Linebender: Vello / Parley / Kurbo / etc. as standalone crates, Masonry on top, Xilem on top of that.

Buiy sits at the intersection: ECS-as-substrate (Bevy's choice) + parallel render pipeline (Linebender's depth of primitives) + cosmic-text (where Buiy diverges from Linebender's Parley). The dependency graph is:

```
Buiy
├── Bevy (ECS, render-graph, wgpu access, winit)
├── Taffy (layout — neither Bevy nor Linebender; DioxusLabs)
├── cosmic-text (text — pop-os/COSMIC, not Linebender)
├── AccessKit (Linebender-adjacent — Matt Campbell is associated)
└── (Vello not used; Buiy's own wgpu render passes)
```

So Buiy is **adjacent to Linebender** without being downstream of it. The intersection point is AccessKit. The non-intersection points (Taffy, cosmic-text) are Buiy's deliberate divergences.

## How to read this when designing Buiy

When designing a Buiy subsystem, check whether Linebender ships a focused crate for the same problem:

| Buiy subsystem | Linebender analog | Buiy posture |
|---|---|---|
| Render passes | Vello | Don't depend; study capability set |
| Text shaping + layout | Parley (uses harfrust) | Use cosmic-text (also uses harfrust); divergence is API shape, not shaper |
| Font enumeration | Fontique | Use cosmic-text's font enum; if it grows, reconsider Fontique |
| Font data | Skrifa (read-fonts) | Use cosmic-text's; transitively similar |
| Color spaces | Linebender Color | Worth studying for gradients-in-any-space |
| 2D curves | Kurbo | Study; potential direct dep for path utilities |
| Paint primitives | Peniko | Study; not directly applicable (Buiy's render pipeline owns its own types) |
| A11y | AccessKit | Direct dep (same as everyone) |
| Reactive layer | Xilem | Out of scope v1; reference for future signal sub-spec |
| Retained widget tree | Masonry | Out of scope (ECS is the tree); reference for `Widget::accessibility` shape |

## Sources

- Linebender about page: https://linebender.org/about/
- Vello: https://github.com/linebender/vello
- Parley: https://github.com/linebender/parley
- Skrifa: https://github.com/googlefonts/fontations (Skrifa is one crate in fontations; Linebender contributes)
- Fontique: part of `linebender/parley` workspace
- Kurbo: https://github.com/linebender/kurbo
- Peniko: https://github.com/linebender/peniko
- Color: https://github.com/linebender/color
- Druid (legacy): https://github.com/linebender/druid
- Cross-link: [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md), [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md), [`../woodpecker-ui/lessons.md`](../woodpecker-ui/lessons.md)
