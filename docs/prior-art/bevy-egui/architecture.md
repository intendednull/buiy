**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — plugin shape, render integration, and per-window contexts

# Architecture

bevy_egui is a Bevy plugin wrapping the upstream `egui` immediate-mode UI framework by Emil Ernerfeldt. The Bevy side does three things: forward winit input into egui's `RawInput`, run the user's egui code each frame inside a Bevy schedule, and submit egui's `ClippedPrimitive` mesh output through Bevy's render graph. It does not own a component model or a layout solver — egui itself owns those. See [immediate-mode-paradigm.md](immediate-mode-paradigm.md) for the conceptual contrast with retained-mode stacks like Buiy or bevy_ui.

## Crate facts (verified 2026-05-22 against `main`)

| Field | Value |
|---|---|
| Crate | `bevy_egui` |
| Latest stable | **0.39.1** (2026-02-06) |
| Bevy version | 0.18.0 |
| egui version | 0.33 (default-features off) |
| MSRV (`rust-version`) | 1.89.0 |
| Edition | 2024 |
| License | **MIT** (single — upstream egui itself is **MIT OR Apache-2.0** dual; bevy_egui chose MIT only) |
| First release | 2020-08-14 |
| Versions published | 70 |
| Total downloads | 2,020,092 |
| Recent downloads (90 d) | 286,785 |
| Maintainer | Vladyslav Batyrenko (vladbat00) |
| Repo | https://github.com/vladbat00/bevy_egui |

The MIT-only license was already in the verified-facts pre-amble; confirmed via direct read of root `Cargo.toml`. Upstream egui is dual MIT/Apache-2.0; bevy_egui is the strict subset.

## The plugin struct

The user-facing entry point:

```rust
pub struct EguiPlugin {
    pub enable_multipass_for_primary_context: bool,
    #[cfg(feature = "bevy_ui")]
    pub ui_render_order: UiRenderOrder,
    #[cfg(feature = "render")]
    pub bindless_mode_array_size: Option<std::num::NonZero<u32>>,
}
```

Three knobs: whether the primary context runs in multi-pass mode, where egui's pass sits relative to bevy_ui's pass, and the bindless-texture array size (0.37+).

## System-set ordering — per-frame flow

`EguiPlugin::build` registers a fixed pipeline keyed off Bevy's `PreUpdate` and `PostUpdate` schedules.

**PreUpdate** (input forwarding):

1. `EguiPreUpdateSet::InitContexts` — for any camera lacking an `EguiContext` component, create one. As of 0.35.0 every active camera carries its own context (see [integration.md](integration.md)).
2. `EguiPreUpdateSet::ProcessInput` (subdivided into `EguiInputSet::InitReading` → `FocusContext` → `ReadBevyMessages` → `WriteEguiEvents`) — drain Bevy's `Events<KeyboardInput>`, `Events<MouseMotion>`, `Events<MouseButtonInput>`, `Events<Touch>`, IME events, etc., and translate them into `egui::RawInput` on each `EguiInput` component.
3. `EguiPreUpdateSet::BeginPass` — call `egui::Context::begin_pass(raw_input)`.

**Between PreUpdate and PostUpdate** the user's UI systems run. In multi-pass mode they run inside the `EguiPrimaryContextPass` schedule (or a custom `EguiMultipassSchedule` per non-primary context). The "pass loop" system in PostUpdate may run the schedule **multiple times per frame** if egui requests a relayout (this is why it's called multi-pass).

**PostUpdate** (output extraction):

1. `EguiPostUpdateSet::EndPass` — call `egui::Context::end_pass()`, producing `FullOutput` (textures-delta + `ClippedPrimitive`s + platform output).
2. `EguiPostUpdateSet::ProcessOutput` — copy textures-delta into `EguiManagedTextures` (Bevy `Assets<Image>`), stash paint jobs in `EguiRenderOutput`.
3. `EguiPostUpdateSet::PostProcessOutput` — apply `egui::PlatformOutput` to Bevy: cursor icon, clipboard write, IME state, open-URL, copy-to-clipboard, virtual-keyboard hint.

## Render-graph integration

The render side is gated behind the `render` cargo feature (default on). bevy_egui registers a render sub-graph `SubGraphEgui` and a node `NodeEgui::EguiPass`, inserted into **both** `Core2d` and `Core3d` core graphs. The node draws after the main pass and before upscaling. With `bevy_ui` feature on, the `ui_render_order` plugin field decides whether egui draws **above** (`AfterUi`) or **below** (`BeforeUi`) bevy_ui's pass; the default is above.

Internally each frame the extract-schedule pulls `EguiRenderOutput` from the main world into the render world; the node converts each `ClippedPrimitive` (egui's tessellated triangle list with a per-primitive clip rect and texture ID) into a wgpu draw call. egui ships its own tessellator (`epaint::Tessellator`) — bevy_egui does not re-tessellate.

## Texture / font atlas

egui owns the font atlas; bevy_egui mirrors it as a Bevy `Image` asset registered with a stable `egui::TextureId::Managed(...)`. The `EguiManagedTextures` resource holds the mapping. User-supplied textures (e.g. an in-game RenderTarget piped into an egui image widget) go through `EguiUserTextures` — the user calls `egui_user_textures.add_image(handle)` to receive a `TextureId::User(...)` and uses that in `egui::Image::new(texture_id, size)`.

0.38.0 added **partial texture update** support — egui can dirty a sub-rect of the font atlas (e.g. when a new glyph is shaped) and bevy_egui uploads only the changed region.

## Multi-pass vs single-pass

Two modes (single-pass is deprecated as of 0.35.0; the README says it "may become deprecated" but the changelog already marks `EguiPlugin::default()` as multi-pass):

- **Single-pass** (legacy): UI systems run as ordinary Bevy systems in `Update`. The user calls `ctx.ctx_mut()` to get the egui `Context` and emits widgets. Cannot iterate inside one frame.
- **Multi-pass** (current): UI systems live in the `EguiPrimaryContextPass` schedule. The pass-loop system in PostUpdate calls `world.run_schedule(EguiPrimaryContextPass)` once, checks `egui::Context::wants_to_repaint_immediately()`, and re-runs if egui needs another pass to settle layout. Required for widgets whose size depends on a value computed during the same frame (e.g. auto-sizing tooltips, popovers that need to know their content size before placing themselves).

Each non-primary context must use its own schedule via `EguiMultipassSchedule::new(MyCustomSchedule)`. Sharing one schedule across multiple contexts is not supported.

## Per-window / per-camera contexts

Since 0.35.0 egui contexts attach to **cameras**, not windows. The first camera added gets a `PrimaryEguiContext` marker; secondary cameras get their own `EguiContext` components. This supports:

- Multi-window apps (one camera per window; the `examples/two_windows.rs` demo).
- Split-screen apps (multiple cameras targeting the same window — each gets its own egui surface).
- Render-to-texture (`EguiRenderToImage` component on a camera that targets a `RenderTarget::Image`, for diegetic / in-world UI panels).

This is a notable shift: pre-0.35 the model was one egui context per window. Cameras are now the carrier, which lines up with Bevy's general render model. Multi-pass mode and the `EguiMultipassSchedule` component compose with this — each context can run on its own schedule. See [integration.md](integration.md) for setup mechanics.

## Input forwarding: winit → egui

bevy_egui consumes Bevy input events (which themselves are translated from winit), not raw winit. The chain:

1. `bevy_winit` polls winit, emits `KeyboardInput`, `MouseMotion`, etc. into Bevy events.
2. `EguiPreUpdateSet::ProcessInput` reads those events and writes to `EguiInput::events` and the modifier/cursor fields.
3. egui sees the input on its next `begin_pass`.

IME has dedicated handling — egui has its own composition state (independent of Bevy's `Ime` events), driven by `EguiInput::events::Ime`. 0.37 / 0.38 / 0.39 each shipped IME fixes (the Linux backspace/arrow-key fix in 0.39.0, the `ime_enable` opt-out in 0.38.0). Mobile virtual-keyboard support landed in 0.35.0 but is documented as "still rough around the edges."

## Picking integration

When the `picking` feature is on (default), bevy_egui inserts a system `capture_pointer_input_system` that **suppresses bevy_picking events** when a pointer is over an egui widget. The `EguiPickingOrder` resource sets the priority — default 0.6 (above bevy_ui's default backend), or 0.4 (below) depending on `ui_render_order`. The deprecated `PICKING_ORDER` constant was removed in 0.39.0 in favor of this dynamic resource.

For diegetic UI (egui rendered onto a 3D mesh), 0.35.0 added bevy_picking mesh-picking integration: a pointer hit on a 3D mesh whose surface is an egui render-target can be forwarded into that mesh's egui context as a virtual pointer position. The `examples/render_to_image_widget.rs` and `examples/render_egui_to_mesh.rs` demonstrate this.

## What this architecture buys, what it costs

bevy_egui is a thin bridge. It does not extend egui's component model (because egui doesn't have one — see [immediate-mode-paradigm.md](immediate-mode-paradigm.md)), and it does not extend Bevy's ECS-side UI (because Bevy's ECS doesn't see egui widgets — they exist only during the schedule run). The cost is structural: every egui frame rebuilds the entire widget tree from scratch, which is fine for the dev-tools workload egui is designed for ([use-cases.md](use-cases.md)) but inappropriate for production UI with many thousands of widgets, layout caching across frames, animation state held by widgets, or an accessibility tree consumed by external ATs without the egui-AccessKit hop.

See also: [api-surface.md](api-surface.md) for the egui widget vocabulary, [integration.md](integration.md) for setup mechanics, sibling docs `distribution.md`, `ecosystem.md`, `comparisons.md` for the broader landscape.

## Sources

- bevy_egui repo — https://github.com/vladbat00/bevy_egui
- Cargo.toml @ main — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/Cargo.toml
- README @ main — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/README.md
- CHANGELOG @ main — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/CHANGELOG.md
- docs.rs — https://docs.rs/bevy_egui/latest/bevy_egui/
- crates.io API — https://crates.io/api/v1/crates/bevy_egui
- Releases — https://github.com/vladbat00/bevy_egui/releases
- egui upstream — https://github.com/emilk/egui
