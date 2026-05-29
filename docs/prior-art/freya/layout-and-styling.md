**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — layout engine (Torin) + CSS-like styling props

# Layout and styling

Freya uses **Torin**, its own pure-Rust layout engine, **not Taffy**. Torin is a workspace crate (`crates/torin/`) versioned in lockstep with Freya itself (currently `0.4.0-rc.19`). Its `Cargo.toml` description is *"UI layout Library designed for Freya."* The crate is theoretically usable outside Freya but has no other known production consumers.

This is one of the **most consequential divergences** between Freya's stack and Buiy's. Buiy's foundation [§ 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly) commits to **Taffy** — the same engine Blitz, bevy_ui, and Servo use. Freya rolled its own.

## Why Torin (Marc's reasoning, paraphrased)

The most-cited public reasoning across Freya's discussions is that Torin's model is **specifically tuned for Freya's frame-by-frame VDOM-diff workload** — partial-layout updates when only a few subtrees changed, rather than full-tree recomputation. Taffy's `LayoutPartialTree` trait surface didn't exist when Freya started (2022-07); by the time it landed, Torin was already shipping. There is no committed roadmap to migrate Freya to Taffy.

Whether this reasoning still holds in 2026 is debatable — Taffy 0.7+ has the partial-layout surface that Blitz uses ([`../taffy/architecture.md`](../taffy/architecture.md)). But the migration cost would now be substantial, and Marc has not signalled intent.

## Torin's model

Verified from the crate `Cargo.toml` and surface API:

- **Pure Rust, no allocator surprises.** Dependencies: `euclid` (geometry types), `rustc-hash` (`FxHashMap`), `itertools`, `tracing`. Optional `serde`. No `taffy`, no `cosmic-text`.
- **Independent of Skia or any renderer.** Torin emits sizes and positions; the embedder does the drawing.
- **Not "flexbox-as-CSS-spec".** Torin's model uses concepts like `direction` (`vertical` / `horizontal`), `main_align` / `cross_align`, `width` / `height` (with `%` / `auto` / `calc()` units), `padding`, `margin`, `position` (relative / absolute). This is *flexbox-shaped* in spirit but **does not implement the full CSS Flexbox spec** (no `flex-basis` / `flex-grow` / `flex-shrink` interaction, no `flex-wrap`, no `align-self`-style cross-axis overrides per-child as a full set, no Grid).
- **Not "CSS Grid".** No grid surface.
- **No subgrid, no container queries.** No equivalents.

Freya's element attributes that map to Torin layout:

```rust
rect {
    direction: "vertical",       // main axis
    main_align: "center",        // justify-content equivalent
    cross_align: "start",        // align-items equivalent
    width: "100%",
    height: "auto",
    padding: "20",
    margin: "0",
    spacing: "10",               // gap-like, between children
    position: "stacked",         // or "absolute"
    position_top: "0",
    position_left: "0",
    overflow: "clip",            // or "none"
}
```

This is **CSS-flavored stringly-typed styling props**, similar to React Native's `style={{ flexDirection: 'column' }}` pattern but inline as macro attribute syntax.

## CSS-flavored styling props (the surface)

Beyond layout, Freya exposes a wide CSS-like prop surface on `rect` / `label` / `paragraph`:

| Prop | Maps to |
|---|---|
| `background` | Skia solid color or gradient shader |
| `color` | Text color (Skia paint) |
| `font_family` / `font_size` / `font_weight` / `font_style` | Skia text style |
| `font_align` | Text alignment within paragraph |
| `corner_radius` / `corner_smoothing` | Skia rounded clip |
| `border` (width + style + color) | Skia stroked outline |
| `shadow` | Skia blur + offset paint |
| `opacity` | Skia paint alpha |
| `rotate` / `scale_x` / `scale_y` | Skia `Canvas::concat` matrix |
| `cursor` | winit cursor type |
| `blend_mode` | Skia `BlendMode` |
| `backdrop_blur` | Skia `save_layer` + `ImageFilter::blur` |

Values parse at **runtime**; invalid strings produce runtime warnings, not compile errors. This is the inverse of the BSN-typed approach Buiy's foundation [§ 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class) commits to.

## Themability

Freya ships **`freya-hooks::use_theme()` + `use_init_theme()`** for theming. Themes are Rust struct values, scoped via Dioxus context. Per-widget tokens (button colors, slider rail color, etc.) come from the active theme; user code can override via context provider.

This is **closer to React-Context-themes than to bevy_flair / Buiy token themes**. There is no:

- Hot-reloadable theme assets.
- OS-preference-driven variant auto-binding (`prefers-color-scheme`, `prefers-contrast`, etc.).
- Asset-format for theme files.
- Per-subtree override via component data (Buiy's `Theme` component pattern).

Themes are pure-code. Switching a theme is a context replacement + re-render.

## How this compares to Buiy's plan

| Concern | Freya | Buiy ([foundation § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)) |
|---|---|---|
| Layout engine | Torin (own) | Taffy (shared with rest of Rust ecosystem) |
| Layout features | flexbox-flavored subset | Flexbox + Grid + Block + (future: anchor positioning, container queries) |
| Style props | Stringly-typed attrs in `rsx!` | Typed Bevy components (`buiy::Style`) with semantic token references |
| Type safety | Runtime parse errors | Compile-time + `Reflect`-based BSN validation |
| Theming | Rust struct via Dioxus context | Token assets, hot-reloadable, OS-pref-bound variants |
| Variants | None automatic | `light`/`dark`/`high-contrast` + user-defined |
| CSS-flavored stylesheet | The whole API is CSS-flavored | Not in foundation; future sub-spec ([open question](../../specs/2026-05-07-buiy-foundation/README.md)) |

## What Buiy can borrow

- **The CSS-like attribute surface as a familiarity hook for users.** Freya's prop names (`background`, `corner_radius`, `padding`, `main_align`, `cross_align`) mirror CSS — users transferring from web feel at home. Buiy's BSN component fields can adopt similar naming where the underlying concept maps cleanly. Foundation [§ 3.2](../../specs/2026-05-07-buiy-foundation/visuals.md) absorbs CSS naming for the most part.
- **Per-component-instance theme overrides** as a context-or-component pattern. Buiy's `Theme` component on a subtree (foundation [§ 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)) is the BSN-flavored version.

## What Buiy should NOT borrow

- **Torin's own-layout-engine choice.** Building a new layout engine is a multi-year investment that Taffy already serves. The cosmic-text / Taffy / AccessKit prior-art folders all reinforce this — Buiy's "integrate primitives directly" principle holds.
- **Stringly-typed styling props.** Buiy's BSN reflection-based typed components are the more correct path. Compile-time errors + IDE completion beat runtime warnings for non-trivial UIs. Foundation [§ 2.4 / architecture § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class).
- **Theme-as-Rust-struct.** Token-asset themes (foundation [§ 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)) get hot-reload, OS-pref binding, and asset-pipeline integration for free.

## Sources

- `freya-skia-safe`-using element render — Freya `crates/freya-elements/` (workspace).
- Torin `Cargo.toml` — https://raw.githubusercontent.com/marc2332/freya/main/crates/torin/Cargo.toml
- Torin `README.md` — https://github.com/marc2332/freya/tree/main/crates/torin
- Freya docs.rs (modules: `elements`, `hooks::use_theme`) — https://docs.rs/freya/latest/freya/
- Cross-references: [`../taffy/architecture.md`](../taffy/architecture.md), [`../taffy/lessons.md`](../taffy/lessons.md), [`../dioxus/integration-with-taffy.md`](../dioxus/integration-with-taffy.md), [`skia-rendering.md`](skia-rendering.md), [`lessons.md`](lessons.md).
- Buiy foundation — [`architecture.md § 2.2 / § 2.5`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md).
