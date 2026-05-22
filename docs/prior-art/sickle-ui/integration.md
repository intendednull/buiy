**Date:** 2026-05-22
**Status:** archived
**Subject:** sickle_ui — integration: plugin setup, feature flags, Bevy compat, coexistence

# Integration

This file covers what it takes to drop sickle_ui into an existing Bevy app, what feature flags are available, what Bevy version it pins, and how it coexists (or fails to coexist) with adjacent Bevy UI crates.

## Setup — the entire onboarding

```rust
use bevy::prelude::*;
use sickle_ui::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SickleUiPlugin)  // MUST come after DefaultPlugins
        .add_systems(Startup, setup_ui)
        .run();
}

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    commands.ui_builder(UiRoot).column(|column| {
        column.label(LabelConfig::from("Hello"));
        column.button(...);
    });
}
```

That is, mechanically, the entirety of the integration. The plugin auto-registers the widget plugins, the theming engine, the dynamic-style engine, the interaction state machines. Custom widgets the app defines add their own `ComponentThemePlugin::<MyWidget>::default()` separately.

The `"Must be added after DefaultPlugins"` ordering constraint is documented on `SickleUiPlugin` itself. The reason: sickle wires observers and resources that depend on `bevy_ui`'s plugin set being present.

## Cargo features (verified against the 0.4.0 Cargo.toml)

| Feature | Behavior |
|---|---|
| `default` | Enables `bevy_default_font` + `observable`. |
| `bevy_default_font` | Forwards to `bevy/default_font` — registers a default font asset so `Label` widgets render before the app loads its own fonts. |
| `observable` | Empty feature flag (gates code paths around observer-driven event handling). |
| `dev` | Enables `bevy/dynamic_linking` — fast iteration during development. |
| `dev_panels` | Empty feature flag (intended for additional editor-style panels in development builds; the implementation is sparse). |
| `disable-ui-context-placeholder-warn` | Forwarded to `sickle_ui_scaffold` — suppresses a runtime warning when a `UiContext::get(name)` lookup falls through to a placeholder. Useful when intentionally omitting some `UiContext` targets. |

The feature surface is small. There is no feature for "without widgets" (you cannot use the scaffold layer alone via `sickle_ui` — you must depend on `sickle_ui_scaffold` directly), no feature for "without theming," no a11y feature flag (there is no a11y to enable). The `dev_panels` flag hints at an ambition that did not materialize before the project went silent.

## Bevy version pin — and why this is the central integration blocker

`Cargo.toml` at 0.4.0 pins:

```
bevy = { version = "0.14", features = [...] }
```

Verified-working with `bevy = "0.14.2"` per the surviving fork's README:

> This is the last release, compatible with Bevy 0.14.2.

**No 0.15+ migration exists.** Concretely:

- No `bevy_main` branch on the surviving fork. `danec020/sickle_ui`, the most-recently-committed-to fork, is also still on Bevy 0.14.
- No PR (in either the deleted upstream — verified by the dead links from crates.io — or the surviving forks) attempting Bevy 0.15 / 0.16 / 0.17 / 0.18 migration.
- The maintainer's stated reason is that Bevy 0.15 introduced `RequiredComponents` (PR #14791) which made sickle's hand-rolled "spawn this bundle, then attach these companions" pattern redundant and structurally awkward. The retrofit would have been large enough that the maintainer chose to declare the project obsolete rather than rewrite. See [`history.md` § "The Bevy 0.15 cliff"](history.md) for the specifics.

For an app on Bevy 0.14, sickle works. For an app on Bevy 0.15 or later, sickle does not work, and there is no path to make it work without forking and porting.

## Coexistence with other Bevy UI crates

### Coexistence with `bevy_ui`

**By design — sickle is bevy_ui.** Every sickle entity is a `bevy_ui::Node` with sickle markers + interaction state on top. There is no parallel render pass, no parallel layout solver. The integration is fully transparent: any `bevy_ui` system or query that walks `Node` entities will find sickle widgets too.

This is also the trap: sickle inherits every `bevy_ui` renderer limitation by construction. Non-rectangular clipping, backdrop-filter, mix-blend-mode, true top-layer compositing — none of these are available to sickle widgets because they are not available to `bevy_ui`. See [`../bevy-ui/critiques.md` § "Renderer caps"](../bevy-ui/critiques.md).

### Coexistence with `bevy_egui`

**Orthogonal, mechanically.** `bevy_egui` is an immediate-mode UI library that renders to its own surface; sickle is retained-mode on `bevy_ui`. They do not conflict in the ECS sense. In practice the only friction is **pointer-event arbitration** — both libraries want to consume mouse input, and the host app has to arrange `EguiContext.wants_pointer_input()` checks against sickle's `Interaction` query before deciding which library handles a given event.

This is the same arbitration story any Bevy app needs for any combination of two pointer-consuming layers; sickle does not make it worse, but it also does not help.

### Coexistence with `bevy_feathers` / `bevy_ui_widgets`

**Cannot coexist in practice.** `bevy_feathers` and `bevy_ui_widgets` are Bevy 0.17+ crates. sickle is Bevy 0.14. An app cannot depend on both, because their Bevy version requirements are incompatible — they are not coexistence-blocked at the API level; they are coexistence-blocked at the dependency-resolution level.

If a hypothetical "sickle_ui ported to Bevy 0.17" existed, it would mechanically coexist with `bevy_feathers` (both are styled-widgets-on-bevy_ui kits; no ECS conflict). But that port does not exist, the maintainer declared it explicitly not coming, and no community fork has produced it as of this doc's writing.

### Coexistence with `bevy_lunex`

**Mechanically possible, semantically pointless.** `bevy_lunex` is a parallel-to-`bevy_ui` UI stack (its own Node-equivalent component, its own layout). An app could depend on both — sickle widgets in `bevy_ui` windows, lunex widgets in separate windows — and they would not conflict at the ECS level. But the whole point of choosing lunex is to avoid `bevy_ui`'s renderer caps; pulling in sickle re-introduces them. No production app combines them, to our knowledge.

### Coexistence with `bevy_cobweb_ui` (the spiritual successor)

**Significant: cobweb internally salvaged sickle's scaffold layer** as `cob_sickle_math` / `cob_sickle_macros` / `cob_sickle_ui_scaffold` (all v0.8.0, vendored under `crates/` inside the cobweb repository). The cobweb design carries sickle's `Theme<C>` / `PseudoTheme<C>` / `DynamicStyle` / `FluxInteraction` shape forward. So an app that depended on sickle 0.4 (Bevy 0.14) could in theory migrate to cobweb (Bevy 0.17) and find the underlying styling vocabulary familiar — though the COB asset format and the cobweb widget catalog are different.

**Caveat:** `bevy_cobweb_ui` is itself archived as of 2026-01-13. Sickle's scaffold survived the project but not the maintainer pipeline.

## Custom widget extension pattern

A third-party crate that wants to add widgets to a sickle-using app:

```rust
// my_widget_crate/src/lib.rs
use sickle_ui_scaffold::prelude::*;
use sickle_ui::prelude::*;

#[derive(Component, DefaultTheme, UiContext)]
pub struct MyWidget;

pub trait UiMyWidgetExt {
    fn my_widget(&mut self, ...) -> UiBuilder<Entity>;
}

impl UiMyWidgetExt for UiBuilder<Entity> {
    fn my_widget(&mut self, ...) -> UiBuilder<Entity> {
        // Spawn an Entity bundle, attach MyWidget + visual companions,
        // return a UiBuilder<Entity> rooted at the new entity.
    }
}

pub struct MyWidgetPlugin;
impl Plugin for MyWidgetPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ComponentThemePlugin::<MyWidget>::default());
    }
}
```

The user then imports both `sickle_ui::prelude::*` and `my_widget_crate::prelude::*`. `UiMyWidgetExt` is in scope, `ui.my_widget(...)` becomes available. This is the *intended* extension story; in practice the wider ecosystem never produced more than a handful of community widgets because (a) sickle's surface was already large, (b) the Bevy 0.15 cliff foreclosed investment, (c) `sickle_ui_scaffold`'s API is BSN-hostile in the same way the catalog is, so widgets written against it become migration cost when Bevy moves.

## Documentation completeness

docs.rs reports `"2.9% of the crate is documented"` on the 0.4.0 docs page. The README (surviving-fork version) is the substantive user-facing documentation; the example app `simple_editor` is the second resource. There is no published mdBook, no tutorial site, no migration guide. For an app already using sickle, the working code in `simple_editor` is the canonical reference.

## Implications for Buiy

1. **Single-`plugin` onboarding is good.** `app.add_plugins(SickleUiPlugin)` is the entire setup. Buiy's `BuiyPlugins` should match this ergonomic level — one `add_plugins` call gets the full stack.
2. **Bevy 0.14 pin is the cautionary tale.** sickle's death-on-0.15 is exactly the failure mode Buiy's policy of "tracks latest Bevy stable, each minor is a migration event" is designed to avoid. The lesson: a UI library that doesn't migrate within one Bevy cycle dies. Buiy commits to migrating with the engine.
3. **Feature-flag minimalism is good.** sickle's six-feature surface is well-scoped. Buiy's feature flags should not balloon — the dev-mode flag (`dev`) and the optional-asset flags (`bevy_default_font`) cover the legitimate use-cases.
4. **The "we are bevy_ui" coexistence story makes sickle's renderer fate inescapable.** Buiy's parallel-stack choice (foundation [architecture.md § 2](../../specs/2026-05-07-buiy-foundation/architecture.md)) avoids this by owning the render pipeline. The coexistence policy Buiy commits to is per-window, not in-tree (foundation [cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) — that's a deliberate response to the sickle pattern.

## Sources

- 0.4.0 Cargo.toml — https://docs.rs/crate/sickle_ui/0.4.0/source/Cargo.toml
- crates.io API (versions / dates) — https://crates.io/api/v1/crates/sickle_ui
- Surviving fork README (Bevy 0.14.2 pin, no migration) — https://github.com/UkoeHB/sickle_ui
- `SickleUiPlugin` doc page — https://docs.rs/sickle_ui/0.4.0/sickle_ui/struct.SickleUiPlugin.html
- docs.rs documentation coverage — https://docs.rs/sickle_ui/0.4.0/sickle_ui/
- bevy_cobweb_ui Cargo.toml (cob_sickle_ui_scaffold salvage) — https://github.com/UkoeHB/bevy_cobweb_ui
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation cross-cutting (coexistence) — [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
