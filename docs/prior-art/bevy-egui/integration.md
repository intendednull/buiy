**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — setup, per-frame mechanics, coexistence, multi-window, WASM and mobile

# Integration

This file covers the practical mechanics of putting bevy_egui into a Bevy app: how the plugin is added, how per-frame UI is emitted, how it coexists with bevy_ui and other parallel UI stacks, how multi-window and render-to-texture surfaces work, and what platforms ship today.

## Setup

`Cargo.toml`:

```toml
[dependencies]
bevy = "0.18.0"
bevy_egui = "0.39.1"
```

`main.rs`:

```rust
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);   // at least one camera is required
        })
        .add_systems(EguiPrimaryContextPass, ui_system)
        .run();
}

fn ui_system(mut contexts: EguiContexts) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    egui::Window::new("Hello").show(ctx, |ui| {
        ui.label("World");
    });
}
```

`EguiPlugin::default()` is multi-pass; the user system runs in the `EguiPrimaryContextPass` schedule. The "at least one camera" requirement is a 0.35.0 change — egui contexts now attach to cameras, not to the `App` globally.

## Cargo features

From the verified `Cargo.toml`, default-on features:

- `manage_clipboard` — wire `EguiClipboard` to the OS clipboard via `arboard` on desktop / `web-sys` on WASM.
- `open_url` — wire `egui::output().open_url` to platform URL opening.
- `default_fonts` — include egui's bundled fonts (Ubuntu Mono / Hack / etc.).
- `render` — register the wgpu render pass. Drops to zero rendering cost when off (useful for headless tests).
- `bevy_ui` — enable the `ui_render_order` plugin field for coexistence with bevy_ui.
- `picking` — register the bevy_picking hover-suppression system.

Off-by-default:

- `accesskit` — enable AccessKit a11y tree forwarding. Pulls `bevy_a11y`. **Disabled by default since 0.38.0.**
- `immutable_ctx` / `serde` / others — niche.

## Per-frame usage in detail

The user authoring surface is one of:

**Multi-pass (recommended, default):** Add systems to `EguiPrimaryContextPass`. The schedule may run multiple times per frame if egui requests a relayout.

```rust
.add_systems(EguiPrimaryContextPass, (system_a, system_b))
```

**Single-pass (deprecated since 0.35.0):** Add systems to `Update`. The user calls `ctx.ctx_mut()` manually. Cannot iterate.

```rust
.add_systems(Update, ui_system)
```

The deprecation has shipped — `EguiPlugin::default()` enables `enable_multipass_for_primary_context: true`. Users still wanting single-pass set it to `false` explicitly. Per the 0.35.0 changelog: *"Single-pass rendering mode deprecated."* The README note "may become deprecated" was authored before the deprecation actually landed.

## Coexistence with bevy_ui

bevy_egui is designed to coexist with bevy_ui in the same app, in the same window. Mechanics:

- **Rendering.** The `ui_render_order` plugin field (when `bevy_ui` feature on) decides whether egui draws above or below bevy_ui. Default is `AfterUi` (above). The egui render pass and the bevy_ui render pass live in the same Core2d / Core3d graph; their ordering is configurable.
- **Picking.** When `picking` feature on, bevy_egui's `capture_pointer_input_system` suppresses bevy_picking events that hit egui widgets. The `EguiPickingOrder` resource sets priority (default 0.6, above bevy_ui's default 0.5). bevy_ui still handles its own non-egui-covered area.
- **Input.** Both stacks consume Bevy events from the same source. egui marks input as "consumed" via `egui::Context::wants_pointer_input()` / `wants_keyboard_input()`; well-behaved apps check these before forwarding to other consumers.
- **Focus.** bevy_egui does not integrate with `bevy_input_focus`. egui's own focus model (internal to `Context`) is independent. Focus does not cross the boundary — Tab inside an egui window cycles egui widgets; Tab in bevy_ui cycles bevy_ui widgets; the user is expected to click into one or the other.

This coexistence model is **less strict than Buiy's** ([cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) commits to one stack per window). bevy_egui can mix with bevy_ui in one window because egui doesn't push to AccessKit by default (so no adapter-ownership conflict) and because egui's render output is a single flat tessellated layer that composites cleanly above other Bevy passes. Buiy can't follow this pattern because Buiy *does* push to AccessKit by default and *does* own a render-graph node group — adapter ownership becomes the constraint.

## Coexistence with bevy_lunex and other parallel UI stacks

The same rules apply: bevy_egui draws on top via the render-graph node ordering, and consumes pointer hits via picking suppression. bevy_lunex / bevy_dioxus / bevy_cobweb / iced_bevy each ship their own render passes; bevy_egui's default ordering puts it above all of them. None of these stacks ship an AccessKit integration that would collide with egui's, so AccessKit-adapter ownership is not a coexistence issue in practice (though the cohabiting trees would be confusing to ATs in the rare app that turned them all on).

## Multi-window and multi-camera contexts

Since 0.35.0, each camera carries its own `EguiContext`. Setup for two windows:

```rust
fn setup(mut commands: Commands) {
    let primary_window = commands.spawn(Window::default()).id();
    let secondary_window = commands.spawn(Window {
        title: "Secondary".into(),
        ..default()
    }).id();

    commands.spawn((
        Camera2d,
        Camera {
            target: RenderTarget::Window(WindowRef::Entity(primary_window)),
            ..default()
        },
        // PrimaryEguiContext added automatically to the first camera
    ));
    commands.spawn((
        Camera2d,
        Camera {
            target: RenderTarget::Window(WindowRef::Entity(secondary_window)),
            order: 1,
            ..default()
        },
        EguiMultipassSchedule::new(SecondaryContextPass),
    ));
}

#[derive(ScheduleLabel, ...)]
struct SecondaryContextPass;
```

Each non-primary context that runs in multi-pass mode must declare its own schedule via `EguiMultipassSchedule::new(label)`. The example `examples/two_windows.rs` ships this pattern verbatim.

For split-screen (two cameras one window), the same setup works — both cameras target the same window with different viewports; each gets its own egui surface.

## WASM target

bevy_egui ships WASM support. Mechanics:

- **Render.** The wgpu render pass works under `wgpu`'s WebGL2 or WebGPU backend. egui's tessellated output ships through unchanged.
- **Input.** `bevy_winit` on WASM translates browser events into Bevy `KeyboardInput` / `MouseMotion` / `TouchInput`; bevy_egui sees them like native.
- **Clipboard.** The `manage_clipboard` feature on WASM uses `web-sys` to call `navigator.clipboard.readText/writeText`. Browser security gates apply (user gesture required).
- **URL opening.** `web-sys::Window::open` for `open_url`.
- **Text input / IME.** egui handles its own IME composition; on WASM this needs a hidden `<input>` element managed by egui-winit-style glue. bevy_egui inherits this.

WASM examples ship and are linked from the README. The "still rough around the edges" disclaimer attached to mobile-web virtual-keyboard applies — IME on WASM is functional but has known issues.

## Mobile support

- **Render.** Works on Android (via wgpu/Vulkan/GLES) and iOS (via wgpu/Metal) wherever Bevy itself works.
- **Touch.** Single touch translates to pointer input. Multi-touch is partial — egui exposes `MultiTouchInfo` but most stock widgets ignore secondary touches.
- **Virtual keyboard.** Native Android / iOS support landed in 0.35.0 but is documented as "still rough around the edges and only works without `prevent_default_event_handling` set to `false`."
- **Gestures.** No native gesture recognizer. Pinch / rotate / long-press are not built in; the app implements them on raw `TouchInput`.

## Render-to-texture surfaces (diegetic UI)

For "egui rendered onto a 3D mesh in the world" (a computer terminal in-game, a holographic UI), 0.35.0 added the pattern:

1. Spawn a camera with `RenderTarget::Image(handle)` — its egui context paints into that image.
2. Attach the image to a `MeshMaterial3d<StandardMaterial>` on a mesh in the 3D scene.
3. With the `picking` feature on, bevy_picking forwards mesh-surface pointer hits to the egui camera's virtual pointer position.

Examples: `render_to_image_widget.rs`, `render_egui_to_mesh.rs`. This is the use case Buiy's `buiy_3d` subsystem also targets ([cross-cutting.md § 3.17](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)), and bevy_egui is currently the only Bevy UI stack that ships it as a documented example.

## What integration does *not* cover

- **No persistent component model.** You can't query egui widgets via `Query<&Button, With<Settings>>` — they don't exist as entities.
- **No BSN authoring.** BSN reflects components onto entities; egui has no entities.
- **No bevy_input_focus integration.** egui's focus model is independent.
- **No bevy_animation integration.** egui has its own `animate_value` / `animate_bool` API, internal to `Context`.
- **No bevy_asset integration for fonts.** Fonts are bundled or loaded via egui's own `egui::FontData` API; they don't live as `Handle<Font>`.

These are not bugs — they're consequences of immediate-mode (see [immediate-mode-paradigm.md](immediate-mode-paradigm.md)). egui owns its world; bevy_egui owns the bridge.

## Sources

- bevy_egui README — https://github.com/vladbat00/bevy_egui
- bevy_egui CHANGELOG — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/CHANGELOG.md
- bevy_egui Cargo.toml — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/Cargo.toml
- Example: two_windows — https://github.com/vladbat00/bevy_egui/blob/main/examples/two_windows.rs
- Example: render_to_image_widget — https://github.com/vladbat00/bevy_egui/blob/main/examples/render_to_image_widget.rs
- bevy_picking — https://docs.rs/bevy_picking
- Sibling files: [architecture.md](architecture.md), [api-surface.md](api-surface.md), [immediate-mode-paradigm.md](immediate-mode-paradigm.md), [use-cases.md](use-cases.md)
