**Date:** 2026-05-22
**Status:** active
**Subject:** woodpecker_ui — plugin shape, render pipeline, layout, reactivity model

# Architecture

woodpecker_ui is a **parallel UI stack** that does not build on `bevy_ui`. It sits on `bevy_vello` for rendering, Taffy for layout, Parley for text, and Bevy ECS for state. The widget system itself is a small reactive runtime: a per-widget `update` system returns a `bool` flagging "did anything change?"; if true, the widget's `render` system runs and patches children. This is the same lineage as kayak_ui (see [`history.md`](history.md)) but the README's Q3 advertises a deliberately simpler runtime — *"the primary system that runs the UI was over 1k lines in Kayak and in Woodpecker it's less than 200."*

## The layer cake

```
+------------------------------------------------------------+
| User widgets       — #[derive(Widget)] on user components  |
+------------------------------------------------------------+
| woodpecker_ui      — runner, hooks, layout, picking,       |
|                      vello-renderer, font, svg, focus       |
+------------------------------------------------------------+
| bevy_vello 0.9     — vello scene compositor                |
| Taffy 0.7          — flexbox + grid layout                 |
| Parley 0.4         — text shaping (replaces cosmic-text    |
|                      in lib.rs doc-comment as of 2025)     |
| skrifa 0.30        — glyph rasterization                   |
| usvg 0.44          — SVG parsing                           |
| bevy_picking       — input hit-testing (own backend)       |
| bevy 0.16          — ECS, asset, render-graph              |
+------------------------------------------------------------+
```

Verified from `Cargo.toml` on `main` (2025-06-07 push) — see [`distribution.md`](distribution.md) for version pinning details.

## Plugin shape

```rust
app.add_plugins(WoodpeckerUIPlugin::default())
```

`WoodpeckerUIPlugin` is a single-plugin entry point that internally installs:

- **`WoodpeckerLayoutPlugin`** — Taffy layout pipeline.
- **`VelloPlugin`** (from `bevy_vello`) — render compositor; `canvas_render_layers`, `use_cpu`, and `antialiasing` are forwarded from `RenderSettings`.
- **`WoodpeckerUIWidgetPlugin`** — registers the built-in widget set (see [`api.md`](api.md)).
- **`ConvertRenderTargetPlugin`** — render-target translation for embedding woodpecker in non-default cameras.
- **`ExtractResourcePlugin<ImageManager>`** — extracts image-handle state to the render world.

The plugin initializes these resources: `FontManager`, `HookHelper`, `WoodpeckerContext`, `WidgetMapper`, `DefaultFont` (embedded `Poppins-Regular.ttf`), `WidgetMetrics`, `SvgManager`, `ImageManager`, `ObserverCache`, `CurrentFocus`. It registers events `WidgetFocus` / `WidgetBlur`. It registers `WoodpeckerStyle` and several style-enum types for reflection (used by `bevy-inspector-egui` integration).

The runner system (`runner::system`) and the vello scene renderer (`vello_renderer::run`) are gated by `run_if(has_root())` — the entire stack short-circuits until `WoodpeckerContext::set_root_widget(entity)` has been called. This is the canonical setup gesture (see [`api.md`](api.md)).

## Render pipeline

woodpecker_ui does **not** use `bevy_ui`'s render passes, `bevy_sprite`, or any of Bevy's built-in 2D renderers. It emits one **vello scene** per frame.

Mechanics:

1. After layout completes, `vello_renderer::run` (ordered `.after(layout::system::run)`) walks the widget tree.
2. For each widget with a `WidgetRender` leaf component, it appends draw commands to the vello scene.
3. `bevy_vello` rasterizes the scene to a render layer (default = standard 2D camera layer; overridable via `RenderSettings.layer: RenderLayers`).
4. Antialiasing is controlled by `vello::AaConfig` (default `Area`); `use_cpu` toggles CPU fallback.

`WidgetRender` variants (from `src/render/`, abridged):
- `Text { content, word_wrap }`
- `Image` / image atlas
- `Svg { handle }`
- `Quad` (filled rectangle with rounded corners via `Corner` styling)
- `Custom(WidgetRenderCustom)` — escape hatch to emit raw vello scene fragments.
- Layer / Clip — push/pop vello layers; the `Clip` widget uses this.

**Implication for Buiy.** This is a useful proof point that **vello is a viable Bevy UI renderer**, but it ships a single-scene-per-frame compositor — not a render-graph node integration the way Buiy's foundation architecture commits to (`docs/specs/2026-05-07-buiy-foundation/architecture.md` § 2.3). Vello can do rounded clip, `clip-path`, gradients, drop-shadow, blur — the features `bevy_ui` lacks ([`bevy-ui/lessons.md`](../bevy-ui/lessons.md) "Renderer that caps web-parity features"). woodpecker_ui doesn't *expose* most of those capabilities through `WoodpeckerStyle`, but the underlying vello primitives are available via `WidgetRenderCustom`. See [`lessons.md`](lessons.md) entry on "borrow vello as renderer substrate."

## Layout

Taffy 0.7 is used directly with `flexbox` + `grid` features. No float, no block (Taffy 0.4 / 0.10 features) are enabled. `WoodpeckerStyle` exposes a CSS-flavored field set (see [`api.md`](api.md) § Style component): `display`, `position`, `flex_direction`, `align_items`, `justify_content`, `padding`, `margin`, `width`/`height`/`min_*`/`max_*`, `gap`, `overflow`, `top`/`right`/`bottom`/`left`. The `WoodpeckerLayoutPlugin` builds a Taffy tree mirroring the widget hierarchy; `WidgetLayout` and `WidgetPreviousLayout` components hold the laid-out output.

The `Units` enum supports `Pixels`, `Percentage`, `Auto`. Taffy is bridged via custom measure functions for leaf text/image content (same pattern as `bevy_ui`; see [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Borrow #6).

## Text and input

- **Text shaping:** Parley 0.4. The `lib.rs` doc-comment still mentions cosmic-text — that is residue from the pre-migration codebase. The `Cargo.toml` (verified 2026-05-22) is the authoritative source. Glyph rasterization is via `skrifa` 0.30.
- **Font management:** `FontManager` resource; users must `font_manager.add(&handle)` for any font they load. An embedded default font (`Poppins-Regular.ttf`) is asset-baked.
- **Rich text:** `rich_text` module ships a span-based rich-text type; syntax highlighting via the `autumnus` crate for code blocks.
- **Text input:** `TextBox` widget with `TextBoxState`; keyboard input plumbed through `keyboard_input::runner` (separate code path for WASM via `read_paste_events` for clipboard).
- **IME:** **No IME support verified.** No `bevy_input::ime` integration, no `Compose` event handling in `keyboard_input/`. Latin-keyboard-only.
- **BiDi / RTL:** Parley does the shaping, but no `direction` field in `WoodpeckerStyle`. Authoring is single-direction.
- **Clipboard:** `arboard` 3.4 on native; `web_sys::Clipboard` on WASM.

## Picking and focus

- **Picking:** woodpecker_ui registers its own backend with `bevy_picking` (Bevy 0.16's standard picking framework). Hit-testing follows the laid-out widget tree.
- **Focus:** Single `CurrentFocus` resource (`Entity::PLACEHOLDER` when no focus). `WidgetFocus` and `WidgetBlur` events fire on transitions. No `:focus-visible` distinction, no focus trap primitive, no inert subtrees, no roving-tabindex, no spatial/gamepad navigation. This is well short of the Buiy focus model (`docs/specs/2026-05-07-buiy-foundation/architecture.md` § 2.3, "Buiy owns a single focus tree with `:focus-visible`, traps, restoration, inert subtrees, roving tabindex").

## Reactivity model

Per-widget `update(...) -> bool` + conditional `render(...)`:

```rust
fn render(/* ... */) -> bool { /* return true if children need re-rendering */ }
```

The runtime calls `update` every frame; `render` runs only when `update` returned `true` *or* when a tracked input changed. Inputs tracked:
- `#[props(...)]` — props are the marker component's fields; equality-checked across frames.
- `#[state(...)]` — state components (declared via the `state` attribute) are equality-checked.
- `#[context(...)]` — context entities (declared via the `context` attribute) feed nearest-ancestor lookups.
- `#[resource(...)]` — declared Bevy resources are equality-checked.

The `#[auto_update(render)]` macro generates the `update` function automatically by diffing the declared props/state/context/resource.

State is plumbed via the **`HookHelper` resource + `use_state(...)` method**: state entities are created on first call, keyed off the current widget entity, and tracked across re-renders via `PreviousWidget` mapping. This is a direct port of React-style hooks (the API surface matches `useState`).

Children are managed via `WidgetChildren`, a vector-of-bundles structure built fluently:

```rust
WidgetChildren::default().with_child::<MyWidget>((MyWidget { /* props */ }, /* required components */))
```

`WidgetChildren::apply(parent)` reconciles the actual entity hierarchy with the declarative description, despawning removed children.

**Compared to BSN.** Where Bevy's BSN (PR #20158, still draft per [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Top-of-file finding #1) intends to provide an asset-driven scene-patch authoring layer, woodpecker_ui's `WidgetChildren` is a Rust-builder in-code authoring style. The Q4 of the README explicitly opposes the BSN direction: *"I'm personally not a huge fan of using scenes and also the new BSN macro."* This is a useful third-party stance to record but does not change Buiy's BSN-friendly commitment (foundation goal 3).

## Module layout

From `src/lib.rs`:

```
mod children;          // WidgetChildren, Mounted, PassedChildren
mod context;           // Widget trait, WoodpeckerContext, root entity
mod convert_render_target;
mod entity_mapping;    // WidgetMapper — entity-to-render-tree mapping
mod focus;             // CurrentFocus, WidgetFocus/Blur events
mod font;              // FontManager, TextAlign, font loading
mod hook_helper;       // HookHelper, use_state, PreviousWidget
mod image;             // ImageManager
mod keyboard_input;    // keyboard runner
mod layout;            // WoodpeckerLayoutPlugin (Taffy bridge)
mod metrics;           // WidgetMetrics
mod observer_cache;
mod on_change;         // Change<T> events (TextChanged, ToggleChanged, ...)
mod picking_backend;   // bevy_picking backend
mod render;            // WidgetRender enum, WidgetRenderCustom trait
mod rich_text;
mod runner;            // the main 200-line widget runner
mod styles;            // WoodpeckerStyle, Corner, Edge, Units, layout enums
mod svg;               // SvgAsset, SvgLoader, SvgManager
mod vello_renderer;    // walks tree, emits vello scene
mod vello_svg;
mod widgets;           // the built-in widget set
```

The compactness is real. The runner (`src/runner.rs`) is the load-bearing reactive scheduler and is short.

## Sources

- `src/lib.rs` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/src/lib.rs
- `Cargo.toml` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/Cargo.toml
- README — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/README.md
- `bevy_vello` — https://github.com/linebender/bevy_vello
- Parley — https://github.com/linebender/parley
- Taffy — https://github.com/DioxusLabs/taffy
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling: [`api.md`](api.md), [`integration.md`](integration.md), [`history.md`](history.md), [`bevy-ui/lessons.md`](../bevy-ui/lessons.md)
