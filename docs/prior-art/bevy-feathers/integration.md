**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — Integration: adding the plugin, dependencies, spawning, coexistence with custom UI and with Buiy

# Integration

How a host app uses bevy_feathers, the dependency contract it imposes, and how it can coexist (per-window) with bevy_ui custom content, non-Bevy UI, or Buiy.

## Adding the plugin

```rust
use bevy::prelude::*;
use bevy_feathers::FeathersPlugins;

App::new()
    .add_plugins(DefaultPlugins)
    .add_plugins(FeathersPlugins)  // PluginGroup: TabNavigationPlugin + FeathersCorePlugin
    .run();
```

`FeathersPlugins` is a `PluginGroup`. Behind the scenes it adds:

- `bevy_input_focus::tab_navigation::TabNavigationPlugin` (sequential Tab focus).
- `FeathersCorePlugin`, which transitively adds `ControlsPlugin`, `CursorIconPlugin`, theme observers, font propagation, embedded assets, and `init_resource::<UiTheme>()` (defaulting to dark).

Apps that want only a subset of the controls can add `FeathersCorePlugin` directly without `FeathersPlugins`, then opt into individual control plugins (`ButtonPlugin`, `SliderPlugin`, etc.) — though in practice the cost of the full set is negligible.

**Note on naming:** Pre-0.18 the crate exported a feature flag `experimental_bevy_feathers` (per the Bevy 0.17 release notes); by 0.18 / 0.19 the feature gate has loosened but the README still warns the API is experimental. See [history.md](history.md).

## Cargo features — verified against `Cargo.toml` HEAD

```toml
[features]
default = []
custom_cursor = ["bevy_window/custom_cursor"]
webgl = []
webgpu = []
```

Four features total, three of them empty markers. `custom_cursor` propagates a Bevy window feature for image-backed cursors. `webgl` / `webgpu` are present but appear to be markers consumers can use; they don't gate substantive code in feathers itself as of `main`.

## Cargo dependencies — verified

Internal Bevy crates pulled in:

- `bevy_a11y`, `bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_ecs`, `bevy_input`, `bevy_input_focus`, `bevy_log`, `bevy_math`, `bevy_picking`, `bevy_shader`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_scene`, `bevy_text`, `bevy_ui` (with `bevy_picking` feature), `bevy_ui_render`, `bevy_ui_widgets`, `bevy_window`, `bevy_derive`.
- External: `smol_str = "0.2"`, `accesskit = "0.24"`.

The accesskit pin is exact (`0.24`); compare to bevy 0.17.3 which pinned `accesskit = "0.21"`. AccessKit majors drift mid-Bevy-minor (see [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) "AccessKit version pin drift"). A consumer pinning bevy_feathers also implicitly pins AccessKit.

## Resource dependencies

- **`UiTheme`** — initialized by `FeathersCorePlugin` as the dark theme. App can replace by `app.insert_resource(custom_theme)` before/after plugin add (test order; resource init is `init_resource` which is idempotent in the not-already-inserted sense).
- **`DefaultCursor`** — fallback cursor when nothing is hovered.
- **`OverrideCursor`** — temporary cursor override (loading states).
- **`InputFocus`** + **`InputFocusVisible`** — from `bevy_input_focus`, used by the `manage_focus_indicators` system.
- **Asset loading:** fonts and icons are **embedded** via `embedded_asset!` — no filesystem dependency at runtime. The five fonts (Fira Sans Regular/Italic/Bold/BoldItalic + Fira Mono Medium) ship inside the crate binary.

## Spawning widgets — ECS vs BSN

Two conventions exist in the source (see [widgets.md](widgets.md)):

1. **Scene-component (modern):** the `FeathersFoo` component carries a `Scene` impl that fans out children. Spawned by:

   ```rust
   commands.spawn((FeathersButton, FeathersButtonProps {
       caption: /* boxed scene */,
       variant: ButtonVariant::Primary,
       corners: RoundedCorners::All,
   }));
   ```

2. **Bundle function (deprecated):** `button_bundle(...)` returns a `Bundle` for direct spawn. Every widget has a deprecated `_bundle()` form; the module suppresses the deprecation warning crate-wide.

**BSN compatibility status:** BSN itself (PR [#20158](https://github.com/bevyengine/bevy/pull/20158)) is **still draft / unmerged as of 2026-05-22** (cart: `"not intended to be merged in current form"` — see [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) top-of-file finding). The Bevy 0.17 release notes said feathers would be ported to BSN when BSN lands in 0.18; 0.18 shipped without BSN. The 0.18 release notes don't mention BSN authoring for feathers.

Once BSN lands, the scene-component widgets are the BSN target — `FeathersFoo` is small, public-fielded, decomposed enough to be BSN-authorable. The deprecated `_bundle()` form is the BSN-hostile path being phased out. The decomposition tax has been paid; it's now waiting for BSN itself.

The `AccessibleLabel` decomposition (PR #24308, merged 2026-05-21 for 0.19) is a separate matter — it breaks up `bevy_a11y::AccessibilityNode` into decomposed components a BSN template can patch. Feathers widgets that currently don't set their a11y role (the majority, per [accessibility.md](accessibility.md)) will inherit the decomposed surface; once they wire it, BSN authoring of feathers a11y becomes possible too.

## Coexistence with bevy_ui custom UI

Inside a bevy_ui-owned window, feathers and raw bevy_ui content compose cleanly:

- Feathers entities ARE bevy_ui entities (`Node`, `BackgroundColor`, `BorderColor`, etc., plus feathers's themed extensions).
- An app can interleave feathers widgets with custom `Node` hierarchies in the same tree.
- Picking / hit-testing / focus all flow through the same bevy_ui infrastructure.

This is the intended path for the upcoming Bevy Editor: editor chrome built from feathers widgets, custom content (gizmos, scene viewport overlays) built from raw bevy_ui.

## Coexistence with non-Bevy UI

For an app that needs to host (say) a native menu bar or a webview pane alongside feathers, the model is **per-window**, not per-tree:

- Native OS chrome → use the OS toolkit on its own window.
- Webview → use `wry` / equivalent on its own window.
- Bevy + feathers → one or more bevy-owned windows.

Mixing UI toolkits in a single window is out of scope (and the bevy_picking backend filter would conflict). This mirrors Buiy's foundation [cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) per-window coexistence rule, applied symmetrically: feathers also wants the window, just like Buiy does.

## Coexistence with Buiy — the load-bearing question

Per Buiy foundation [cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md), coexistence with bevy_ui (and therefore with bevy_feathers, which sits on bevy_ui) is **per-window**, not per-tree. The supported model:

- An app may have multiple windows. Each window is owned by **exactly one stack** — either Buiy or bevy_ui (with or without feathers).
- On a Buiy-owned window: Buiy owns the `accesskit_winit::Adapter`, the render-graph nodes, the `bevy_picking` backend, the focus model, the IME consumer. `bevy_a11y` is suppressed for that window. bevy_ui systems do not render or interact on that window. Feathers's theme observers may still fire (they operate on Components), but the bevy_ui rendering that would consume their output does not run on that window.
- On a bevy_ui-owned window: bevy_ui (+ feathers, if added) retains its full behavior. Buiy is absent.
- **Window stack assignment is fixed at window creation**; no runtime stack switching for an existing window in v1.

Concrete app shape:

```rust
App::new()
    .add_plugins(DefaultPlugins)           // bevy_ui's UiPickingPlugin runs on bevy_ui-owned windows
    .add_plugins(FeathersPlugins)          // feathers UI on bevy_ui-owned windows
    .add_plugins(buiy::BuiyPlugin)         // Buiy UI on Buiy-owned windows
    .add_systems(Startup, |mut commands: Commands| {
        commands.spawn(Window { title: "Editor".into(), ..default() })
            .insert(BevyUiOwned);          // bevy_ui + feathers window
        commands.spawn(Window { title: "Game".into(), ..default() })
            .insert(BuiyOwned);            // Buiy window
    })
    .run();
```

The Buiy plugin filters its `bevy_picking` backend to `BuiyOwned`-tagged windows; `UiPickingPlugin` filters to `BevyUiOwned`. AccessKit adapter ownership splits the same way — Buiy owns the adapter on Buiy windows; `bevy_a11y` owns it on bevy_ui windows.

This is the committed coexistence model. **Buiy does not ship migration adapters from feathers widgets in v1** (per foundation README § 5 open questions — "whether Buiy ships migration adapters from bevy_ui widgets is open"). Apps that want a Buiy version of feathers's catalog get it from `buiy_widgets`, not by reskinning feathers.

## Game-loop interaction

Feathers widgets live in the standard Bevy update cycle:

- **`PreUpdate`**: `update_cursor` resolves the cursor icon based on hovered entity.
- **`Update`**: app systems read `ValueChange<*>` / `Activate` events and mutate app state.
- **`PostUpdate`**: `propagate_text_fonts` (before `UiSystems::Content`), `manage_focus_indicators` (in `UiSystems::Content`). Theme observers run on component-change events whenever they fire.
- **Render extract**: standard bevy_ui render extraction; feathers contributes no render-graph nodes of its own (apart from the alpha-pattern shader for color widgets).

Feathers does not introduce its own schedule, set, or run condition. This is appropriate for its scope (it's a widget kit, not a UI runtime) but limits its ability to enforce ordering — a custom system that mutates `BackgroundColor` after the theme observer can desync visuals from theme state. Buiy's foundation [architecture.md § 2.8](../../specs/2026-05-07-buiy-foundation/architecture.md) addresses this with explicit `BuiySet::*` ordering.

## Implications for Buiy

- **The per-window coexistence story is symmetric and unambiguous.** Apps that want both an editor (feathers) and a game UI (Buiy) get them in separate windows. Document this in Buiy's getting-started, not as an afterthought.
- **No adapter from feathers widgets to Buiy widgets in v1** — every feathers widget that an app needs in a Buiy window must be reimplemented as a `buiy_widgets` widget. The migration tax is real but bounded.
- **Plugin-group convention is borrowable.** `FeathersPlugins` as a `PluginGroup` adding a base plugin + a focus plugin is exactly the shape `BuiyPlugin`'s sub-plugin ordering takes (foundation [architecture.md § 2.8](../../specs/2026-05-07-buiy-foundation/architecture.md)).
- **Embedded fonts via `embedded_asset!`** is the right default for a tooling kit — no filesystem dependency, version-pinned. Buiy's default theme can do the same for its bundled fonts.
- **The deprecated `_bundle()` shadow API is a transitional artifact** of the scene-component migration. Buiy should not ship a similar shadow — pick the scene-component / BSN-friendly form from day one (per [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) "BSN has not landed. Design *for* it landing, not *because* it has.").

## Sources

- `Cargo.toml` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/Cargo.toml
- `lib.rs` (`FeathersCorePlugin`, `FeathersPlugins`) — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/lib.rs
- `controls/mod.rs` (`ControlsPlugin`) — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/controls/mod.rs
- Bevy 0.17 release notes — https://bevy.org/news/bevy-0-17/
- Bevy 0.18 release notes — https://bevy.org/news/bevy-0-18/
- PR #19730 (feathers introduction, merged 2025-06-28) — https://github.com/bevyengine/bevy/pull/19730
- PR #20158 (BSN, still draft) — https://github.com/bevyengine/bevy/pull/20158
- PR #24308 (`AccessibleLabel`, merged 2026-05-21) — https://github.com/bevyengine/bevy/pull/24308
- Buiy foundation cross-cutting (coexistence) — [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) § 3.18
- Buiy foundation architecture (sub-plugin ordering) — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.8
- bevy_ui lessons — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- Cross-link: [architecture.md](architecture.md), [widgets.md](widgets.md), [theming.md](theming.md), [accessibility.md](accessibility.md)
