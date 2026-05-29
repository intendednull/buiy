**Date:** 2026-05-22
**Status:** active
**Subject:** woodpecker_ui — setup, Bevy compat, coexistence, extension

# Integration

## Setup

Minimal startup:

```rust
use bevy::prelude::*;
use woodpecker_ui::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WoodpeckerUIPlugin::default())
        .add_systems(Startup, startup)
        .run();
}

fn startup(mut commands: Commands, mut ui_context: ResMut<WoodpeckerContext>) {
    commands.spawn((Camera2d, WoodpeckerView));
    let root = commands.spawn((WoodpeckerApp, WidgetChildren::default())).id();
    ui_context.set_root_widget(root);
}
```

Three required gestures:

1. `app.add_plugins(WoodpeckerUIPlugin::default())` — installs the runtime.
2. Spawn a 2D camera with the `WoodpeckerView` marker — tells `bevy_vello` which view to render into.
3. `ui_context.set_root_widget(entity)` — until this is called, the entire runner short-circuits (via the `has_root()` run-condition).

For custom register: every user-defined `#[derive(Widget)]` type also needs `app.register_widget::<MyWidget>()` (see [`api.md`](api.md)).

## Plugin configuration

`WoodpeckerUIPlugin { render_settings: RenderSettings { layer, antialiasing, use_cpu } }`:

- `layer: RenderLayers` — Bevy render layers to render into. Defaults to layer 0.
- `antialiasing: AaConfig` — `Area` (default), `Msaa8x`, `Msaa16x`, `None`.
- `use_cpu: bool` — force CPU rasterization (testing/headless).

## Bevy version compatibility

| woodpecker_ui | Bevy | Notes |
|---|---|---|
| 0.1.0 (2025-05-31) | 0.16 | First crates.io release |
| 0.1.1 (2025-05-31) | 0.16 | Bug-fix same day |
| (unreleased; main HEAD as of 2025-06-07) | 0.16 | Cargo.toml still pinned to `bevy = "0.16"` |

Bevy 0.16 stable shipped 2025-04-24. Bevy 0.17 → 0.18 → 0.18.1 → 0.19-rc all landed during the woodpecker_ui silent period. **woodpecker_ui is on a 2-minor-version-old Bevy as of 2026-05-22.** Anyone adopting it inherits the unmigrated 0.16 → 0.18 transition cost; major changes during that window include:

- The `Bundle` → component-based spawn redesign (Bevy 0.17 +).
- `Required Components` API maturity (PR #14791 landed in 0.15; usage stabilized through 0.17).
- AccessKit upgrade cadence ([`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Avoid-row "AccessKit version pin drift") — `bevy_a11y` in 0.18.1 pins different AccessKit majors than 0.16 did.
- `bevy_picking` itself has API churn between 0.16 and 0.18.

The lift to bring woodpecker_ui forward is not trivial; the *user* would do it, since there are no published patches.

## Coexistence with bevy_ui

woodpecker_ui does not consume `bevy_ui`, `bevy_text`, or `bevy_ui_widgets`. It does consume:

- `bevy` with `default-features = false` + `bevy_picking` + `bevy_log` features. No `bevy_ui` feature.
- `bevy_picking` directly (woodpecker registers its own backend).

**Same-window coexistence with `bevy_ui` is technically possible but unverified.** Both stacks register `bevy_picking` backends; `bevy_picking` supports multiple backends with priority ordering, so input *should* route correctly. No example or test in the woodpecker_ui repo exercises this configuration. Render-pass ordering is not coordinated; layered behaviour would depend on vello-scene Z relative to `bevy_ui`'s render pass.

This is the same coexistence question Buiy's foundation defers via `buiy-coexistence-design` (`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/README.md` § 4) — woodpecker_ui has no production data point on the answer.

**Multi-window coexistence** (separate windows, one running woodpecker_ui, one running `bevy_ui`) should work cleanly because `bevy_picking`, `bevy_a11y`, and AccessKit are all keyed by winit `WindowId`. Again, no example verifies it.

## Custom widget extension

Authoring a new widget:

1. Define the marker component (`struct MyWidget { /* props */ }`) with `#[derive(Widget, Component, Reflect, PartialEq, Default, Debug, Clone)]`.
2. Annotate `#[auto_update(render)]` + `#[props(MyWidget)]` (and optionally `#[state(...)]`, `#[context(...)]`, `#[resource(...)]`).
3. Annotate `#[require(...)]` to auto-insert companion components (typically `WoodpeckerStyle` and `WidgetChildren`).
4. Write the `render(...)` system using `Res<CurrentWidget>` + `HookHelper` + `Query<&MyWidget>`.
5. `app.register_widget::<MyWidget>()` at startup.

The `register_widget` call uses `bevy-trait-query` 0.16 to register the component as a `dyn Widget`, which is how the runner enumerates widget systems polymorphically. This is a *runtime* trait-object dispatch, not compile-time — the cost is per-frame overhead per widget type registered.

Custom rendering primitives: implement `WidgetRenderCustom` and attach via `WidgetRender::Custom(Box::new(MyDrawer))`. This receives a `&mut vello::Scene` and arbitrary user closure logic.

## Hot reload

Opt-in via the `hotreload` Cargo feature:

```sh
cargo install dioxus-cli --version 0.7.0-alpha.0
dx serve --example counter --hotpatch --features="hotreload"
```

woodpecker_ui leverages **dioxus-devtools 0.7.0-alpha.0** for live patching. The `#[hot]` macro can be applied to any widget render system; dioxus-cli detects code changes, recompiles the affected system, and live-patches the running binary.

README claim: *"Hot reloading is very lightweight and won't hinder your performance in release mode at all! Currently only the todo example is wired up for hot reloading but any widget render system can be hot reloaded with the #[hot] macro!"*

**Caveat:** `dioxus-devtools 0.7.0-alpha.0` is itself a pre-release of an unrelated framework's dev-tools subsystem. Stability is therefore alpha-of-alpha. Useful idea, not a maintenance commitment.

This is the *only* current third-party demonstration of dioxus-style code-hot-patch in a Bevy UI crate. The pattern is worth recording even if woodpecker_ui itself isn't a runnable adoption target.

## Platform support

- **Desktop:** Windows / macOS / Linux. Verified via `arboard` clipboard dep (cross-platform).
- **WASM:** First-class. `Cargo.toml` has a dedicated `cfg(target_arch = "wasm32")` block depending on `web-sys`, `wasm-bindgen-futures`, `futures-channel`. WebGPU feature enabled on Bevy. The README has explicit `wasm-server-runner` instructions.
- **Mobile (Android / iOS):** Build profile `android-dev` exists; not verified working. No iOS-specific code. No IME / keyboard adaptation for mobile (see [`critiques.md`](critiques.md)).

## Sources

- README setup section — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/README.md
- `src/lib.rs` (plugin shape) — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/src/lib.rs
- `Cargo.toml` (target cfg blocks, hotreload feature) — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/Cargo.toml
- dioxus-devtools — https://crates.io/crates/dioxus-devtools
- Buiy coexistence design pointer — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 4
- Sibling: [`architecture.md`](architecture.md), [`distribution.md`](distribution.md), [`critiques.md`](critiques.md)
