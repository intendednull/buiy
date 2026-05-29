**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — Styling: color, materials, themability status, animations, render features

# Styling

This file covers what bevy_lunex 0.6.0 ships for visual styling. The short version: bevy_lunex's styling surface is **narrow**, by design — it has one styling component (`UiColor`), defers all other visual styling to whatever sprite / mesh / material the user attaches to the node, and ships no theme system. This is internally consistent with its "position entities, don't render them" architecture (see [`architecture.md`](architecture.md) § "Render path: there is no render pipeline").

## Core styling primitives

### `UiColor`

The single styling component bevy_lunex ships. Verified via `lib.rs` exports.

- Holds a color value (per Bevy `Color`) per state — `base`, `hover`, `clicked`, `selected`, `intro`, `outro` — mirroring the state structure of `UiLayout`.
- The state-driven color interpolation uses the same `UiState::value()` lerp as `UiLayout`. Hovering smoothly transitions color from base to hover; the speed/easing are configured on the state component (e.g. `UiHover::forward_speed`).
- Applied to the sprite / mesh / text on the same entity. There is no separate background/border/foreground color — `UiColor` modulates whichever drawable is present.

### Colors come from Bevy's `Color` type

bevy_lunex inherits Bevy's color story: linear RGBA, with conversions for sRGB, HSL, HWB, LCH, OKLAB, OKLCH built into `bevy_color`. There is no special "UI color" type and no color management beyond what Bevy itself does.

### Borders, radii, shadows, outlines

**There are no border, border-radius, box-shadow, or outline components in bevy_lunex.** If you want rounded corners, you put a rounded sprite or a custom-shader material on the entity. If you want a shadow, you put a shadow sprite behind the entity (or a custom material). If you want a border, you put a border-image sprite.

This is a meaningful contrast with bevy_ui's decomposed visual components (`BackgroundColor`, `BorderColor`, `BorderRadius`, `Outline`, `BoxShadow` — see [bevy-ui lessons.md Borrow #4](../bevy-ui/lessons.md)) and a much larger contrast with Buiy's planned scope ([visuals.md § 3.3](../../specs/2026-05-07-buiy-foundation/visuals.md) commits to all of these as **F** or **C** tier).

The honest framing: in bevy_lunex you don't "style a UI node" in the CSS sense — you assemble a sprite-and-text composition and let bevy_lunex position the pieces. This works because the rendering responsibility is delegated to `bevy_sprite` / `bevy_text` / `bevy_pbr`, which are competent renderers with mature material/shader stories of their own.

## Images and materials

Image rendering goes through `bevy_sprite`'s standard `Sprite` component (in 2D) or `MeshMaterial3d<StandardMaterial>` (in 3D) on a quad reconstructed by `UiMeshPlane2d` / `UiMeshPlane3d`. bevy_lunex provides:

- **`UiImageSize`** to bridge an image's intrinsic size into the layout unit system (mirror of `UiTextSize` — see [`component-model.md`](component-model.md)).

The `UiMeshPlane2d` / `UiMeshPlane3d` markers reconstruct a quad mesh sized to the node's `Dimension`. The user then attaches whatever material they want to that mesh. This is the "custom rendering hook" the docs reference — there is no `UiMaterial`-style trait; the user just uses Bevy's existing material system.

## Themability: is there a theme system?

**No.** There is no `Theme` component, no theme resource, no token system, no semantic-color palette, no scale system (spacing / typography / radius / motion scales).

The maintainer's `Bevypunk` example demonstrates a "themed" UI by setting per-state colors and image sources explicitly on each component spawn. There is no inheritance, no override-by-subtree, no swap-the-theme-at-runtime mechanism.

If a downstream user wants theming, they roll it themselves — either by abstracting over component-spawning code or by maintaining their own resource that supplies values into `UiColor::base` / `UiColor::hover` at spawn time. The pattern is not blocked but is also not supported.

## Token / palette support

None. There is no analog to CSS custom properties, design tokens, or a typed-token system. Spacing values are written as `Ab(8.0)` or `Rl(1.0)` literals inline.

For Buiy's purposes the relevant lesson is what's *missing* — Buiy's foundation commits to a token-based design system ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md)) with semantic tokens, palettes + scales, variants, OS-preference binding, and hot-reloadable theme assets ([theming and user preferences § 3.14](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)). bevy_lunex hits none of these, and the gap is structural — there's no place in the component model for a token to live.

## Animations and transitions

Already covered in [`layout.md`](layout.md) § "Animations and transitions". Recap for the styling perspective:

- Built-in: per-state lerps on `UiColor` and `UiLayout` properties.
- The lerp is driven by per-state interpolated values (e.g. `UiHover::value`) with configurable speed and easing curve.
- No keyframe animations, no spring physics, no animation timelines, no scroll-driven animations, no view transitions.
- No reduced-motion gating (the application must read the OS preference and adjust).

The shape covers the common "smooth hover/click transition" need that most game HUDs require. It does **not** cover the broader CSS Transitions / WAAPI surface that Buiy commits to ([visuals.md § 3.3](../../specs/2026-05-07-buiy-foundation/visuals.md), [interaction.md](../../specs/2026-05-07-buiy-foundation/interaction.md)).

## Render features

Because bevy_lunex doesn't own the renderer, "what render features bevy_lunex supports" reduces to "what `bevy_sprite` / `bevy_text` / `bevy_pbr` support":

| Feature | Status in the stack bevy_lunex composes |
|---|---|
| Solid colors | `bevy_sprite` |
| Gradients | **Not built in** — use a custom-shader material |
| Drop shadows | **Not built in** — composite a shadow sprite |
| Backdrop blur | **Not supported** — no compositor; would require a custom render pass |
| Mix-blend-mode | **Not supported** — Bevy's blend mode story is per-material |
| Rounded clipping | **Not supported** — use a pre-rounded sprite or a custom-shader material with SDF |
| `clip-path` shapes | **Not supported** |
| True top-layer compositing | **Not supported** — z-ordering only via `UiDepth` and `Transform.translation.z` |
| Custom shaders | **Yes** — apply any Bevy `Material2d` / `Material` to a `UiMeshPlane*d` |
| Render-to-texture | **Yes** — via `UiEmbedding` (see [`3d-and-worldspace.md`](3d-and-worldspace.md)) |
| HDR | Inherits Bevy's HDR pipeline state |
| sRGB / Display-P3 color management | Inherits Bevy's wgpu color management |

The features marked **Not built in** are achievable by writing custom shaders / materials and stacking sprites — bevy_lunex doesn't block them — but the user assembles them, and they don't compose with bevy_lunex's state-driven transition system.

## Custom shader integration

The integration point is the same as any `bevy_sprite` / `bevy_pbr` user: write a `Material2d` or `Material` impl, register it with the appropriate plugin, attach an instance to a `UiMeshPlane2d` / `UiMeshPlane3d` entity. bevy_lunex's layout system positions and sizes the mesh; the material draws whatever the shader does.

No "Buiy material slot" analog — there's no `LunexMaterial` trait or registration system. This works because bevy_lunex doesn't own the renderer; it doesn't need to. The downside: any visual primitive you'd reach for in CSS (gradients, shadows, rounded corners, blurs) is the user's shader-writing problem in bevy_lunex.

## Comparison summary

bevy_lunex's styling story is: **"we don't have one, on purpose."** The library is honest about being a positioning engine, not a visual framework. The visual layer is whatever you assemble out of Bevy's existing renderers. This is internally consistent and a reasonable bet for game HUD authors who already think in sprite-and-mesh terms.

It is not a reasonable bet for a comprehensive UI library — and the bevy_lunex maintainer doesn't claim it is. The README's framing ("Lunex is designed to support ALL window sizes out of the box without deforming") is about layout robustness, not visual capability. Quote the marketing language verbatim:

> "Blazingly fast retained _**layout engine**_ for Bevy entities, built around vanilla **Bevy ECS**."

Note: the **"blazingly fast"** framing is a marketing smell — there is no published benchmark on the bevy_lunex repo, no comparison to bevy_ui under load, no profiler trace, no scaling number ("blazingly fast at N nodes"). The phrase is the same Rust-ecosystem boilerplate (ripgrep, sled, fastcdc, dozens of others) that has lost meaning through overuse. The actual perf story is reasonable — the solver is O(N) over visible nodes per frame, with no expensive global solve like flexbox/grid — but "blazingly fast" doesn't tell us anything more than that. Treat it as marketing, not evidence.

## Sources

- `crate/src/lib.rs`, `crate/src/states.rs` (main branch, 2026-05-22)
- docs.rs — https://docs.rs/bevy_lunex/0.6.0/bevy_lunex/
- The Lunex Book — https://bytestring-net.github.io/bevy_lunex/
- README — https://github.com/bytestring-net/bevy-lunex (main, 2026-05-22)
- Bevypunk example — https://github.com/IDEDARY/Bevypunk
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation visuals — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
- Buiy foundation cross-cutting — [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- bevy-ui prior-art — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
