**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — system-specific terms used across this corpus

# Glossary

Definitions for bevy_ui-specific identifiers, type names, and ecosystem terms used throughout this corpus. Cross-link liberally; do not duplicate definitions in evidence files — point at this glossary instead.

## Component / authoring model

- **`Node`** — bevy_ui's *authoring* component. Holds layout fields (`display`, `position_type`, `width`, `height`, `flex_*`, `grid_*`, `padding`, `margin`, `border`, `overflow`) and, since Bevy 0.18, `border_radius`. Acts as a Required-Components hub: spawning `Node` auto-inserts `ComputedNode`, `ComputedNodeTarget`, `UiTransform`, `UiGlobalTransform`, `Visibility`. Confusingly, also the layout tree's per-entity layout-node identity. Context disambiguates which is meant. See [`component-model.md`](component-model.md) and [`architecture.md`](architecture.md).
- **`Style`** — pre-0.15 *megastyle* component containing all of `Node`'s current layout fields. Folded into `Node` in 0.15; the name persists in older docs and migration notes. Not present in 0.18.1 / 0.19-rc.1.
- **`ComputedNode`** — laid-out *output*. Resolved size, position, content size, computed border, scroll position. Read-only from user code; written by the layout pass.
- **`BackgroundColor`, `BorderColor`, `BorderRadius`, `Outline`** — visual decoration as separate components. `BorderRadius` was a separate component in 0.17 then *demoted to a field on `Node`* in 0.18 (the canonical non-monotonic-decomposition example). `BorderColor` is per-side (top/right/bottom/left) as of 0.17.
- **`UiImage`** — image rendering component, renamed to **`ImageNode`** in Bevy 0.16. Optional 9-slice / texture-sliced via auxiliary fields.
- **`Text`** — text content component. As of Bevy 0.15, a `String` newtype (`Text("Save")`). Rich text is *multi-entity*: a `Text` parent with `TextSpan` children, each child carrying its own `TextFont`, `TextColor`, `LineHeight`, `Strikethrough`, `Underline`.
- **`UiTransform`, `UiGlobalTransform`** — 2D-only transform components (Bevy 0.17+) replacing the world `Transform` / `GlobalTransform` for UI hierarchies.
- **`UiMaterial`** — extension point for custom shader effects on UI nodes. Plug a custom shader into the UI pipeline for materials beyond the built-in node shader. See [`architecture.md`](architecture.md) § "Render pipeline" (#6).
- **`Val`** — bevy_ui's length type. Variants: `Auto`, `Px(f32)`, `Percent(f32)`, `Vw(f32)`, `Vh(f32)`, `VMin(f32)`, `VMax(f32)`. The CSS `<length>` analog. Buiy's `Length` is the parallel type.
- **`RequiredComponents`** — Bevy 0.15+ mechanism (`#[require(...)]` on a `Component` derive) that auto-inserts default-constructed companion components when the marked component is inserted. The replacement for `*Bundle` types, which were deprecated in 0.15. See [`component-model.md`](component-model.md) § "RequiredComponents: the mechanism."
- **`AccessibilityNode`** — `bevy_a11y`'s a11y component. Wraps `accesskit::Node`. The *megacomponent* that issue #17644 flagged as BSN-incompatible (private fields, inconsistent method-style setters). PR #24308 broke it up. Still not Buiy's a11y model.

## Layout

- **`Display`** — *value*, not a component. Lives as the `display: Display` field on `Node`, with variants `Flex` / `Grid` / `Block` / `None`. `Display::None` short-circuits layout for the subtree entirely.
- **`Position`** — `PositionType` field on `Node`. Variants `Relative` (default; participates in normal flow) and `Absolute` (out-of-flow, positioned against `inset`).
- **`MeasureFunc`** — Taffy concept: a callback that lets a leaf widget (text, image) measure its intrinsic content size during layout. bevy_ui registers measure funcs for text and image leaves. Buiy uses the same pattern.
- **`OverrideClip`** — component (Bevy 0.18+) that opts a descendant out of inherited clipping from a scrolling ancestor. Used by Feathers popovers/menus to escape their scrolling parent. Narrower than CSS top layer.
- **`IgnoreScroll`** — component (Bevy 0.18+) that exempts marked descendants from scroll-position translation. Approximates "sticky" headers/columns. Narrower than CSS `position: sticky`.
- **Ghost node** — entity-only layout node that participates in layout but doesn't render. Gated behind the experimental `ghost_nodes` Cargo feature on `bevy_ui`. Used for grouping without visual artifacts.

## Stacking and rendering

- **`ZIndex`** — local z-order within the same stacking context. Sibling-only ordering.
- **`GlobalZIndex`** — global z-order that escapes the local stacking context. Bevy's analog of CSS stacking contexts. The two-stage pattern is borrowable for Buiy's stacking-and-top-layer design.
- **`UiPass`** — bevy_ui's render-graph node. Runs after the main 3D/2D passes and before tonemapping (within the UI camera's view).
- **`ExtractedUiNode`** — render-app representation of a UI node, pulled from the main world during the `Extract` phase. Carries a `z_order: f32` (renamed from `stack_index: u32` in 0.18 for finer-grained ordering and texture-slice fixes).
- **`UiPickingPlugin`** — `bevy_ui`'s `bevy_picking` backend, added conditionally behind the `bevy_picking` Cargo feature.

## Text and input

- **cosmic-text** — text shaping / layout library (https://github.com/pop-os/cosmic-text). bevy_ui's text shaper from Bevy 0.15 through 0.18. Buiy commits to cosmic-text.
- **parley + swash** — text shaping (parley, https://github.com/linebender/parley) + glyph rasterization (swash, https://github.com/dfrg/swash). bevy_ui's text shaper as of 0.19-dev (issue #21765). Post-0.19, bevy_ui and Buiy diverge on text shaper.
- **ab_glyph** — bevy_ui's pre-0.15 text shaper. No complex script shaping, no BiDi, no font fallback. Replaced by cosmic-text.
- **`Interaction`** — bevy_ui's mouse/touch hit-test state enum (`Pressed` / `Hovered` / `None`). **Not keyboard focus.**
- **`FocusPolicy`** — input-state companion to `Interaction`. Variants `Block` (default; consumes hits) and `Pass` (transparent to hits).
- **`InputFocus`** — `bevy_input_focus` resource (Bevy 0.16+) holding the currently-focused entity. The keyboard-focus primitive, decoupled from `bevy_ui`.
- **`AutoDirectionalNavigation`** — bevy_ui marker (Bevy 0.18+) opting an entity into the spatial nav graph. Paired with `DirectionalNavigationMap` (manual edges) and `FocusableArea` (navigable node).
- **`CompassOctant`** — 8-direction enum (N, NE, E, SE, S, SW, W, NW) used by spatial navigation to filter candidates by direction.

## Widgets

- **`bevy_ui_widgets`** — first-party headless widget primitives crate (Bevy 0.17+). `CoreButton`, `CoreSlider`, `CoreScrollbar`, `CoreCheckbox`, `CoreRadioButton`. Unstyled; apps add their own visuals. The output of discussion #16900 ("Standard Headless Widgets").
- **Headless widgets** — widget primitives that provide behavior and a11y contracts but no visual styling. The pattern explicitly modeled on Headless UI (React) and Radix (web). bevy_ui_widgets is the first-party implementation.
- **`bevy_feathers`** — first-party opinionated widget kit (Bevy 0.17+). Sits on top of `bevy_ui_widgets`, adds Bevy-editor-flavored styling, theming, and additional widgets (`Popover`, `MenuPopup`, `ColorPlane`). Gated behind the `experimental_bevy_feathers` umbrella Cargo feature.

## Scenes and authoring

- **BSN (Bevy Scene Notation)** — text-based scene format proposed in PR [#20158](https://github.com/bevyengine/bevy/pull/20158) (cart, 2025-07-16). Compose UI from layered component patches. **Still draft / unmerged as of 2026-05-22**; cart explicitly wrote it is `"not intended to be merged in current form."` Discussion #14437 tracks the design.
- **`bsn!` macro** — proposed Rust macro for inline BSN authoring (`bsn! { Button [ Text("Save") ] }`). Part of PR #20158. Status follows BSN itself.
- **`.bsn` asset** — proposed hot-reloadable scene asset file (`asset_server.load("button.bsn")`). Part of PR #20158, not yet shipped.
- **`Construct` trait** — proposed mechanism (discussion #14437) for components that need `World` state (asset handles, OS-pref resources) at construction time, beyond what Required Components' `Default` constraint allows. Not yet merged.

## A11y / external

- **AccessKit** — cross-platform accessibility tree library (https://accesskit.dev). Provides `TreeUpdate` diffs that platform adapters (`accesskit_windows`, `accesskit_macos`, `accesskit_unix`) push to OS accessibility APIs (UI Automation, NSAccessibility, AT-SPI). Both bevy_ui and Buiy integrate AccessKit.
- **`bevy_a11y`** — Bevy crate that bridges Bevy entities to AccessKit. Owns the `AccessibilityNode` component and the AccessKit tree update pump. **Buiy explicitly does not layer over `bevy_a11y`** — it replaces it on any window where Buiy is present.
- **`bevy_picking`** — Bevy crate for pointer / picking. UI-, sprite-, and mesh-picking are all backends to a common abstraction. Backend priority is configurable per pointer. Buiy registers its own backend in parallel.
- **`bevy_text`** — Bevy crate wrapping the active text shaper (cosmic-text through 0.18, parley + swash from 0.19-dev). bevy_ui consumes it. Buiy uses cosmic-text directly without going through `bevy_text`.

## Project / governance

- **SME (Subject-Matter Expert)** — Bevy's review-authority model. UI SMEs are inferred from PR review activity, not formally enumerated on a public page. As of 2026-05-22, the UI SME set centers on @alice-i-cecile, @viridia, @ickshonpe, @nicoburns (Taffy upstream), @bushrat011899.
- **Bevy Foundation** — Washington-state non-profit with 501(c)(3) federal public-charity status. Mission: `"promote, protect, and advance the free and open source Bevy Engine."` Board: cart (President + Interim Treasurer), Alice Cecile (Secretary), François Mockers, Robert Swain, James Liu.
- **`cart`** — Carter Anderson. Bevy founder, project lead, Foundation President, BSN designer.

## Sources

- See [`architecture.md`](architecture.md), [`component-model.md`](component-model.md), [`layout.md`](layout.md), [`styling.md`](styling.md), [`text-and-input.md`](text-and-input.md), [`history.md`](history.md), [`distribution.md`](distribution.md), [`governance.md`](governance.md), [`ecosystem.md`](ecosystem.md) for primary sourcing of each term.
- AccessKit project — https://accesskit.dev
- cosmic-text — https://github.com/pop-os/cosmic-text
- parley — https://github.com/linebender/parley
- swash — https://github.com/dfrg/swash
- Taffy — https://github.com/DioxusLabs/taffy
- BSN PR #20158 — https://github.com/bevyengine/bevy/pull/20158
- Discussion #14437 (BSN tracking) — https://github.com/bevyengine/bevy/discussions/14437
- Discussion #16900 (Standard Headless Widgets) — https://github.com/bevyengine/bevy/discussions/16900
