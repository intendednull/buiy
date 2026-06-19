**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — Node/Style/visual components, the 0.15 required-components migration, BSN-hostility lesson (#17644), feathers / ui_widgets extension surface, current authoring patterns

## Today's component surface (0.18.1 / 0.19-rc.1)

The component-level vocabulary an app touches when authoring a bevy_ui tree, as of HEAD:

- **`Node`** — the authoring component. Holds layout fields (`display`, `position_type`, `width`, `height`, `flex_*`, `grid_*`, `padding`, `margin`, `border`, `overflow`) and, since 0.18, `border_radius`. Acts as a required-components hub: spawning `Node` auto-inserts `ComputedNode`, `ComputedNodeTarget`, `UiTransform`, `UiGlobalTransform`, `Visibility`, and the inherited-visibility components ([Bevy 0.15 notes](https://bevy.org/news/bevy-0-15/), [0.17→0.18 migration](https://bevy.org/learn/migration-guides/0-17-to-0-18/)).
- **`ComputedNode`** — laid-out output. Resolved size, position, content size, computed border, scroll position. Read-only from user code; written by the layout pass.
- **`BackgroundColor`** — flat fill color. Separate component, attached as needed.
- **`BorderColor`** — per-side border colors as of 0.17 ([Bevy 0.17 notes](https://bevy.org/news/bevy-0-17/)). Pre-0.17 it was a single color.
- **`BackgroundGradient`, `BorderGradient`** — 0.17+. Linear, Conic, Radial; interpolation color-space configurable.
- **`Outline`** — outline (CSS `outline`, drawn outside the border edge, does not affect layout).
- **`UiTransform`, `UiGlobalTransform`** — 2D-only transform components (0.17+) replacing the world `Transform`/`GlobalTransform` for UI hierarchies.
- **`UiImage`** (renamed to `ImageNode` in 0.16 — [0.15→0.16 migration](https://bevy.org/learn/migration-guides/0-15-to-0-16/)) — image rendering, optionally 9-slice/texture-sliced.
- **`Text`** — text content. As of 0.15, a `String` newtype ("`Text (the UI text component) and Text2d (the world-space 2D text component) became literally just a String newtype`" — Bevy 0.15). Rich text is multi-entity: a `Text` parent with `TextSpan` children.
- **`TextFont`, `TextColor`, `LineHeight`, `Strikethrough`, `Underline`, `TextShadow`, `TextBackgroundColor`** — text styling, each its own component. `LineHeight` was split out of `TextFont` in 0.18 ([0.17→0.18 migration](https://bevy.org/learn/migration-guides/0-17-to-0-18/)).
- **`FocusPolicy`, `Interaction`** — input-state companions ([text-and-input.md](text-and-input.md)).
- **`AutoDirectionalNavigation`** — 0.18+ opt-in for spatial nav.
- **`AccessibilityNode`** — wraps `accesskit::Node` for a11y; the megacomponent that #17644 critiques.
- **`Display`** — *value*, not a component. Lives as the `display: Display` field on `Node`, with variants `Flex` / `Grid` / `Block` / `None`.

## The decomposition migration: a three-release arc

The current shape did not arrive in one release. It is the residue of a multi-version drift from bundle-heavy authoring to required-components authoring:

| Release | Date | Key UI component-model change |
|---|---|---|
| 0.14 | 2024-07 | `NodeBundle` still canonical; cosmic-text replaces ab_glyph internally ([PR #10193](https://github.com/bevyengine/bevy/pull/10193), merged 2024-07-04, shipped in 0.15) |
| 0.15 | 2024-12 | **Required components ship.** `NodeBundle` deprecated. `Style` fields merged into `Node`. `ComputedNode` introduced. `Text` → `String` newtype, `TextSpan` for rich-text children. `UiImage` → `ImageNode` rename ([0.14→0.15 migration](https://bevy.org/learn/migration-guides/0-14-to-0-15/)) |
| 0.16 | 2025-04 | Minor UI churn — multiple `BoxShadow`, physical-coord-only internals, custom-rounding removal. No big component reshape |
| 0.17 | 2025-08 | `UiTransform`/`UiGlobalTransform` (UI-only 2D transforms). `BackgroundGradient`/`BorderGradient` shipped. Per-side `BorderColor`. `bevy_feathers` (experimental) and `bevy_ui_widgets` (experimental headless) introduced ([Bevy 0.17 notes](https://bevy.org/news/bevy-0-17/)) |
| 0.18 | 2025-12 | `BorderRadius` *demoted from component to field on `Node`*. `LineHeight` split out of `TextFont`. `TextLayoutInfo.section_rects` → `run_geometry`. Text-section picking lands ([0.17→0.18 migration](https://bevy.org/learn/migration-guides/0-17-to-0-18/)) |
| 0.19-rc.1 | 2026-02 | Text stack switches from cosmic-text 0.16 to parley 0.9.0 + swash 0.2.6 ([text-and-input.md](text-and-input.md)) |

The 0.15 split was deliberate: see [Bevy 0.15 notes](https://bevy.org/news/bevy-0-15/), which frame the move from bundles to required components as the foundational rearchitecture that BSN/scenes (discussion [#14437](https://github.com/bevyengine/bevy/discussions/14437)) would build on. The 0.18 `BorderRadius`-back-into-`Node` move is a counter-decomposition — the field was small enough that keeping it as a separate component cost more than it bought.

## RequiredComponents: the mechanism

Bevy 0.15 introduced `#[require(...)]` on `Component` derives. Spawning a component with `#[require(Foo, Bar)]` auto-inserts `Foo::default()` and `Bar::default()` if not already present. `Node` is the canonical heavy user ([Bevy 0.15 notes](https://bevy.org/news/bevy-0-15/)):

```rust
// Conceptual shape; actual decls are spread across bevy_ui sources.
#[derive(Component, Default)]
#[require(ComputedNode, UiTransform, UiGlobalTransform, Visibility, /* ... */)]
pub struct Node { /* ... */ }
```

The mechanism replaces bundles as the entity-construction primitive. It also feeds BSN: a scene asset can list `Node` alone and the runtime will fill in the required companions, so scene files stay terse.

Required-components-for-UI has had documented gotchas — see [issue #18779](https://github.com/bevyengine/bevy/issues/18779) ("Required components behave unexpectedly for UI"), which is still open in 2026. The pattern is sound but the corner cases haven't all been ironed.

## Issue #17644: the BSN-hostility lesson

[Bevy issue #17644](https://github.com/bevyengine/bevy/issues/17644) — *"Design of bevy_a11y is BSN-unfriendly"* — is the canonical write-up of the megacomponent / private-setter pattern as a BSN antipattern. It targets `bevy_a11y`'s `AccessibilityNode`, not bevy_ui's own components, but the diagnosis generalises.

Two specific complaints, verbatim:

> "`The properties are private, and only accessible through methods. The methods use different calling conventions depending on the type of the property.`"

> "`All of the accessibility properties are combined together in a single ECS component.`"

The mechanical consequence: BSN patches/templates can't merge into private-setter components. You can't write `bsn! { Button { aria_disabled: true } }` if `aria_disabled` is reachable only via `.set_disabled()`. Layered scene composition — the whole BSN value proposition — collapses to read-modify-write-via-method calls.

The recommendation in #17644 is to keep an idiomatic decomposed Bevy-side API and *transform* it into AccessKit's structure at the boundary, rather than exposing AccessKit's tree shape as the component model.

This is the single most-cited lesson in the Buiy foundation spec — see `docs/specs/2026-05-07-buiy-foundation/architecture.md` § 2.4 and § 2.6. Every Buiy component is therefore mandated small, public-fielded, observable, decomposed; AccessKit is reached via Buiy's own decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` components.

bevy_ui's own components are *closer* to the BSN-friendly shape than `AccessibilityNode` is — `Node` has public fields, `BackgroundColor` is decomposed, `Text` is a newtype — but the residual coupling inside `Node` (layout fields + `border_radius` + position-type all bundled together) is the same pattern at smaller scale. A BSN patch that wants to change only `border_radius` still pulls the whole `Node` into the patch.

## How `bevy_feathers` and `bevy_ui_widgets` extend the model

Both shipped experimental in 0.17:

- **`bevy_ui_widgets`** ([Cargo.toml](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui_widgets/Cargo.toml)) — `"Unstyled common widgets for Bevy Engine."` Provides behavioral primitives — `Button`, `Slider`, `Scrollbar`, `Checkbox`, `RadioButton`/`RadioGroup` ([Bevy 0.17 notes](https://bevy.org/news/bevy-0-17/)) — without visual styling. Each widget is a marker component plus state components plus required-components on `Node`. Apps add their own `BackgroundColor`/`BorderColor`/font to taste. This is the "headless widget" architecture, explicitly modeled on Headless UI / Radix patterns ([discussion #16900](https://github.com/bevyengine/bevy/discussions/16900)).
- **`bevy_feathers`** ([Cargo.toml](https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/Cargo.toml)) — `"A collection of UI widgets for building editors and utilities in Bevy."` Sits on top of `bevy_ui_widgets`, adding a styled, themed widget kit aimed at the Bevy editor itself. Depends on `accesskit 0.24` and `bevy_ui` directly. 0.18 added more standard widgets: `Popover`, `MenuPopup`, improved `RadioButton`/`RadioGroup`, and a `Color Plane` 2D color picker ([Bevy 0.18 notes](https://bevy.org/news/bevy-0-18/)).

The split is deliberate: `bevy_ui_widgets` is the contract surface games can reuse, `bevy_feathers` is opinionated styling for the editor. Apps building their own visual identity are expected to extend `bevy_ui_widgets` directly, not theme `bevy_feathers`.

## Authoring today

Two practical patterns dominate:

**Direct ECS spawn with `Children::spawn`:**

```rust
commands.spawn((
    Node {
        width: Val::Px(200.0),
        height: Val::Px(40.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    },
    BackgroundColor(GRAY),
    Button,  // from bevy_ui_widgets
    Children::spawn(SpawnIter([
        // child text node
    ])),
));
```

**`bsn!` macro (Bevy 0.18+, draft; full BSN asset format still landing — see [PR #20158](https://github.com/bevyengine/bevy/pull/20158)):**

```rust
bsn! {
    Button {
        Node { width: Val::Px(200.0), height: Val::Px(40.0) }
        BackgroundColor(GRAY)
        [ Text("Save") ]
    }
}
```

BSN's full asset-format loader (`asset_server.load("button.bsn")`) is not yet shipped as of 0.18; it is the next major BSN milestone per discussion #14437.

## Sources

- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/lib.rs
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/Cargo.toml
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui_widgets/Cargo.toml
- https://github.com/bevyengine/bevy/issues/17644
- https://github.com/bevyengine/bevy/issues/18779
- https://github.com/bevyengine/bevy/discussions/14437
- https://github.com/bevyengine/bevy/discussions/16900
- https://github.com/bevyengine/bevy/pull/10193
- https://github.com/bevyengine/bevy/pull/20158
- https://bevy.org/news/bevy-0-15/
- https://bevy.org/news/bevy-0-17/
- https://bevy.org/news/bevy-0-18/
- https://bevy.org/learn/migration-guides/0-14-to-0-15/
- https://bevy.org/learn/migration-guides/0-15-to-0-16/
- https://bevy.org/learn/migration-guides/0-17-to-0-18/
