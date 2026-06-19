//! Shared test support for the Buiy `buiy_core` integration tests. This module
//! hosts BOTH halves of the test substrate:
//!
//! * The **headless (no-adapter) App builders** — [`bare_layout_app`],
//!   [`headless_layout_app`], [`headless_text_app`], and the shared [`settle`]
//!   condition-poll. These take NO wgpu adapter and create NO `RenderApp`; they
//!   are the canonical layout/text plugin stacks that the headless test files
//!   share instead of re-inlining (audit #35).
//! * The **GPU render-integration harness** — [`gpu_test_app`] & friends, the
//!   adapter-backed siblings of the headless builders (documented in the GPU
//!   cascade sub-section below).
//!
//! ## The GPU harness: why each plugin is here (the "Message not initialized" cascade)
//!
//! [`gpu_test_app`] builds the minimal *complete* plugin set that drives a full
//! Buiy render frame headless on a real wgpu adapter (this host: AMD Radeon RX
//! 6700 XT, RADV/Vulkan — no X server / xvfb needed for render-to-texture).
//!
//! `RenderPlugin::build` pulls in `bevy_render::camera::camera_system`, which
//! reads `MessageReader<WindowResized>`; the minimal probe set never added the
//! sole owner of `add_message::<WindowResized>()`, so the first `app.update()`
//! panicked with *"Parameter `…messages` failed validation: Message not
//! initialized"* (Bevy 0.18 renamed Events→Messages). Resolving that surfaced a
//! short cascade of missing owners — each fixed by adding the **correct owning
//! plugin / init**, not a bare `add_message`:
//!
//! | Missing resource / message            | Owner added                         |
//! |---------------------------------------|-------------------------------------|
//! | `Messages<WindowResized>`             | [`bevy::window::WindowPlugin`]      |
//! | `Assets<Mesh>` + `Messages<AssetEvent<Mesh>>` | `app.init_asset::<Mesh>()` (what the private `MeshPlugin` does internally; `RenderPlugin` extracts meshes but deliberately does **not** add the asset) |
//! | `Res<ClearColor>` (+ visibility/projection) | [`bevy::camera::CameraPlugin`] (the logical-world camera plugin, distinct from `RenderPlugin`'s render-world one) |
//! | `Res<Theme>` / `Res<UserPreferences>` | [`buiy_core::theme::ThemePlugin`] (Buiy's own — intentionally separate from `CorePlugin`; `extract_buiy_nodes` reads `Res<Theme>`) |
//!
//! None of these were Buiy bugs — the panic reproduces with *zero* Buiy plugins.
//! Verified by the panel that established this harness (campaign plan
//! `docs/plans/2026-06-07-render-gpu-verify-campaign.md`).
#![allow(dead_code)]

pub mod extract_harness;

use bevy::asset::{AssetApp, RenderAssetUsages};
use bevy::camera::RenderTarget;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use buiy_core::{CorePlugin, render::BuiyRenderPlugin};
use std::sync::{Arc, OnceLock};

// ---------------------------------------------------------------------------
// Headless (no-adapter) App builders — the shared layout/text plugin stacks.
//
// Audit finding #35: ~40 copy-pasted plugin stacks had silently DRIFTED across
// the headless test files — `fn app()` ×21, `fn text_app()` ×10, and a 2-vs-3
// `fn settle()` split ×8. The three builders below are the canonical headless
// stacks; new tests use these instead of re-inlining a stack (which is how the
// drift accumulated). They take NO wgpu adapter and create NO `RenderApp` — the
// GPU builders above (`gpu_test_app` & friends) are the adapter-backed siblings.
//
// The three stacks form a ladder, each a superset of the one above:
//   bare_layout_app     = MinimalPlugins + CorePlugin + LayoutPlugin
//   headless_layout_app = …            + TransformPlugin   (the Transform bridge)
//   headless_text_app   = …            + ThemePlugin + BuiyTextPlugin (the text stack)
// They are kept as three explicit builders (not one parameterized one) so each
// call site reads as a named intent, and the plugin list each adds is obvious.

/// The **2-plugin** headless layout stack: `MinimalPlugins + CorePlugin +
/// LayoutPlugin`, with **no** `TransformPlugin`. The self-documenting *weaker*
/// variant: layout resolves so `ResolvedLayout` is populated, but
/// `GlobalTransform` is **never** finalized (nothing runs Bevy's propagation
/// chain).
///
/// **Pick by the verification need, not by which component appears:** use
/// [`headless_layout_app`] when a test needs `GlobalTransform` *finalized* via
/// `TransformPlugin` propagation; use this builder when the test reads only
/// `ResolvedLayout`, OR when it *deliberately* exercises the no-`TransformPlugin`
/// path (e.g. asserting the bridge writes `Transform` in `Update` even with no
/// propagation plugin present — there the absence of `TransformPlugin` is the
/// thing under test, so `headless_layout_app` would defeat the test).
pub fn bare_layout_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(buiy_core::layout::LayoutPlugin);
    app
}

/// The **3-plugin** transform-bridge stack: [`bare_layout_app`]'s stack PLUS
/// `bevy::transform::TransformPlugin`. This is the stack to use whenever a test
/// reads `Transform` or `GlobalTransform`: `TransformPlugin` is what owns and
/// runs the propagation systems (`mark_dirty_trees → propagate_parent_transforms
/// → sync_simple_transforms`) that `CorePlugin` chains into `Update` to finalize
/// `GlobalTransform` from the bridge-written `Transform` (clip-and-transform.md
/// § B.2.1). Without it `GlobalTransform` stays at its identity default.
pub fn headless_layout_app() -> App {
    let mut app = bare_layout_app();
    app.add_plugins(bevy::transform::TransformPlugin);
    app
}

/// The headless **text** stack: `MinimalPlugins + ThemePlugin + CorePlugin +
/// LayoutPlugin + BuiyTextPlugin::default()` — the T2/T3 text pipeline (TextSync
/// → measure → TextCommit), no render half / no adapter. Includes
/// `buiy_core::theme::ThemePlugin` so the `Res<Theme>` / `Res<UserPreferences>`
/// that extract-time color-token resolution reads exist (this is the plugin
/// `text_decoration.rs::text_app` silently added but most other text tests
/// omitted — folding it in here is what removes that drift). No
/// `TransformPlugin`: text tests read `TextBuffer`/`ResolvedLayout`, not
/// `GlobalTransform`.
pub fn headless_text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::theme::ThemePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(buiy_core::layout::LayoutPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app
}

/// Advance `app.update()` until the layout-and-text pipeline **converges** — two
/// consecutive frames produce an identical *settle snapshot*, where a snapshot is
/// the per-entity resolved geometry (every `ResolvedLayout`'s `(position, size)`
/// plus its `GlobalTransform` translation when present) PLUS the text-shaping
/// quiescence signal (`TextSyncAppliedCount`, `TextCommitReshapeCount`,
/// `FontsGeneration`).
///
/// **Why both signals, not geometry alone** (audit #35, the 2-vs-3-frame split).
/// A geometry-only poll is BLIND to text-shaping state: a reshape that re-lays a
/// buffer's glyphs without moving its box (the FOUT / `FontsGeneration` echo) is
/// invisible to it, so it returns one frame early and the caller reads stale
/// shaped state. The fix is to widen the convergence signal to observe shaping
/// directly via the three resources `BuiyTextPlugin` owns:
///
/// * `TextSyncAppliedCount` (sync.rs) — per-frame count of buffers `TextSync`
///   re-applied the lazy setters to; **reset to 0** at the top of every
///   invocation, so an idle frame reads 0.
/// * `TextCommitReshapeCount` (commit.rs) — per-frame count of buffers
///   `TextCommit` actually reshaped; likewise **reset to 0** each invocation.
/// * `FontsGeneration` (font_system.rs) — the **cumulative** font-set generation,
///   bumped once per font-set change (the system-scan swap / runtime FOUT). This
///   is the resource the legacy 3-update text settles existed to flush.
///
/// Each is read via [`World::get_resource`], so an app WITHOUT `BuiyTextPlugin`
/// (e.g. [`bare_layout_app`] / [`headless_layout_app`]) yields `None` for all
/// three — `None` is treated as a quiescent sentinel, so pure-layout apps still
/// converge on geometry alone.
///
/// **Why "two consecutive identical snapshots" + "no shaping on the final frame"
/// rather than "all counters == 0".** Snapshot equality is the robust core: a
/// `FontsGeneration` bump differs from the prior frame and forces another
/// iteration; the per-frame counters reset to 0 on an idle frame, so the
/// active→idle transition (nonzero → 0) is always an inequality that forces the
/// confirming frame. We additionally require both per-frame counts to be 0 on the
/// converging frame to close the one false-convergence hole this leaves: two
/// back-to-back frames that re-apply the SAME nonzero count with identical
/// geometry would otherwise compare equal even though shaping is still running.
/// Requiring 0 on the final frame makes "converged" mean *genuinely quiescent*,
/// not merely *unchanged-this-pair*. (Mirrors the bounded-poll shape of
/// [`wait_for_text_ready`] / [`readback_rgba`] — condition-based, not
/// frame-count-based.)
///
/// **What the legacy 2-vs-3 split actually was.** A pure layout box settles in
/// ~2 frames (spawn frame + the steady frame). *Display* text populates its
/// asserted content — glyphs and decoration spans — **synchronously in the spawn
/// frame** (`TextSync` builds and fully populates the buffer the same frame),
/// which is why a geometry-only poll *happened* to read correct content on the
/// migrated decoration tests. But the shaping pipeline is not yet quiescent then:
/// the spawn-frame `Added<TextBuffer>` idempotent re-apply echo (`sync.rs`) keeps
/// the per-frame counts nonzero for a second frame, so `shaping_idle` is what
/// actually decides convergence on *every* display-text settle (it reaches ~4
/// updates, not 2). A runtime font load adds a `FontsGeneration` / FOUT reshape
/// echo on top — a reshape that re-lays shaped glyphs while moving NO box
/// geometry. The widened signal observes both echoes (the nonzero per-frame
/// counts and the generation bump), so it iterates the needed extra frames on its
/// own — correct whether the entity is pure-layout, display-text, or mid-FOUT,
/// without the caller knowing which.
///
/// Bounded at [`SETTLE_MAX_FRAMES`]; panics past it (a pipeline that never reaches
/// a fixed point is a real bug, not something to silently tolerate).
pub fn settle(app: &mut App) {
    use buiy_core::text::{FontsGeneration, TextCommitReshapeCount, TextSyncAppliedCount};

    /// The text-shaping quiescence half of the snapshot. `None` for each field
    /// an app lacks `BuiyTextPlugin` (so a pure-layout app's text signal is a
    /// constant `(None, None, None)` — always "stable"). The two per-frame
    /// counts reset to 0 each invocation; `FontsGeneration` is cumulative.
    #[derive(PartialEq)]
    struct ShapingSignal {
        sync_applied: Option<usize>,
        commit_reshaped: Option<usize>,
        fonts_generation: Option<u64>,
    }

    fn shaping(app: &App) -> ShapingSignal {
        let world = app.world();
        ShapingSignal {
            sync_applied: world.get_resource::<TextSyncAppliedCount>().map(|c| c.0),
            commit_reshaped: world.get_resource::<TextCommitReshapeCount>().map(|c| c.0),
            fonts_generation: world.get_resource::<FontsGeneration>().map(|g| g.0),
        }
    }

    /// True when no shaping work happened on the frame just run: both per-frame
    /// counts are 0 (or absent). A converged frame must be quiescent by this
    /// measure, not merely equal to its predecessor — see the doc rationale.
    fn shaping_idle(signal: &ShapingSignal) -> bool {
        signal.sync_applied.unwrap_or(0) == 0 && signal.commit_reshaped.unwrap_or(0) == 0
    }

    /// One entity's resolved geometry: `(position, size)` plus its
    /// `GlobalTransform` translation when present. (`ResolvedLayout` derives no
    /// `PartialEq`, but its two `Vec2` fields do, so the tuple is comparable
    /// without touching the production type.)
    type EntityGeometry = (Entity, Vec2, Vec2, Option<Vec3>);

    /// The full settle snapshot: per-entity geometry (sorted by `Entity` so two
    /// snapshots are comparable) plus the text-shaping quiescence signal.
    #[derive(PartialEq)]
    struct SettleSnapshot {
        geometry: Vec<EntityGeometry>,
        shaping: ShapingSignal,
    }

    fn snapshot(app: &mut App) -> SettleSnapshot {
        let mut q = app
            .world_mut()
            .query::<(Entity, &buiy_core::ResolvedLayout, Option<&GlobalTransform>)>();
        let mut geometry: Vec<EntityGeometry> = q
            .iter(app.world())
            .map(|(e, layout, gt)| {
                (
                    e,
                    layout.position,
                    layout.size,
                    gt.map(|gt| gt.translation()),
                )
            })
            .collect();
        geometry.sort_by_key(|(e, ..)| *e);
        SettleSnapshot {
            geometry,
            shaping: shaping(app),
        }
    }

    app.update();
    let mut prev = snapshot(app);
    // Runs up to SETTLE_MAX_FRAMES iterations AFTER the initial update, so up to
    // SETTLE_MAX_FRAMES + 1 total updates run before the panic fires — the
    // advertised bound below matches that actual update count.
    for _ in 0..SETTLE_MAX_FRAMES {
        app.update();
        let next = snapshot(app);
        // Converged: identical to the previous frame AND no shaping ran on this
        // frame (a stable-but-still-reshaping pair is not yet quiescent).
        if next == prev && shaping_idle(&next.shaping) {
            return;
        }
        prev = next;
    }
    panic!(
        "layout/text pipeline never reached a quiescent fixed point within \
         {} updates (geometry + text-shaping signal still moving — a genuine \
         non-convergence / oscillation bug)",
        SETTLE_MAX_FRAMES + 1
    );
}

/// The [`settle`] poll bound (iterations after the initial update; the panic
/// reports `SETTLE_MAX_FRAMES + 1` total updates). The deepest *observed*
/// spawn-settle chain here is short: instrumenting `settle` against
/// `text_font_display`'s asset-path tests showed convergence at **4 total
/// updates** — the spawn/Added<TextBuffer> echo and the FOUT/`FontsGeneration`
/// reshape both take an active frame plus the quiescence-confirming frame. Eight
/// iterations is a generous margin over that, chosen so the panic only ever
/// fires on genuine non-convergence (an oscillation), never on a legitimately
/// deep but finite chain.
pub const SETTLE_MAX_FRAMES: usize = 8;

/// Build the canonical headless-GPU Buiy app. The returned [`App`] is **not yet
/// finished** — the caller must `finish()` it (or use [`finish_and_run`]) before
/// reading any render-world resource: `RenderPlugin` inserts the `RenderDevice` /
/// `PipelineCache` and `BuiyRenderPlugin::finish` registers `BuiyPipeline`
/// **during `finish`**, never `build`.
pub fn gpu_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        // Owns `Messages<WindowResized>`, read by `RenderPlugin`'s camera_system.
        .add_plugins(bevy::window::WindowPlugin::default())
        // `Assets<Shader>` lives here (main world); shaders load into it.
        .add_plugins(bevy::asset::AssetPlugin::default())
        // Creates the RenderApp + the wgpu device/adapter (block_on initialize).
        .add_plugins(bevy::render::RenderPlugin::default())
        // Hard transitive requirement of `RenderPlugin`'s GpuImage path.
        .add_plugins(bevy::image::ImagePlugin::default())
        // Logical-world camera: owns `Res<ClearColor>` + visibility/projection.
        .add_plugins(bevy::camera::CameraPlugin)
        // Buiy's `Res<Theme>` / `Res<UserPreferences>`, read by extract.
        .add_plugins(buiy_core::theme::ThemePlugin)
        .add_plugins(CorePlugin)
        // The text engine + the T4 glyph producer (render half registers
        // against the live RenderApp created by RenderPlugin above).
        .add_plugins(buiy_core::text::BuiyTextPlugin::default())
        .add_plugins(BuiyRenderPlugin);
    // `Assets<Mesh>` + `Messages<AssetEvent<Mesh>>` — `RenderPlugin` extracts
    // meshes but does not add the asset (its doc: "Use MeshPlugin for that").
    app.init_asset::<Mesh>();
    app
}

/// `finish()` + `cleanup()` the app (materializing the render device, pipeline
/// cache, and `BuiyPipeline`), then drive `frames` render frames. Render plugins
/// insert their device-dependent resources only during `finish`, so this MUST
/// run before any render-world resource is read. The first frame may not paint;
/// pass `frames >= 2` when asserting on painted output.
pub fn finish_and_run(app: &mut App, frames: usize) {
    app.finish();
    app.cleanup();
    for _ in 0..frames {
        app.update();
    }
}

/// [`gpu_test_app`] + [`buiy_core::layout::LayoutPlugin`] — for tests that spawn
/// real `(Node, Style)` entities and need the full layout → stacking → transform
/// bridge → extract path. Sub-pass 6f writes the `StackingContext` that
/// `extract_buiy_nodes` walks; without it extract emits nothing, so a painted
/// node never reaches `BuiyInstanceBuffers`. Kept SEPARATE from `gpu_test_app`
/// so the resource/structural GPU tests on the base harness stay untouched.
pub fn gpu_test_app_with_layout() -> App {
    let mut app = gpu_test_app();
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    app
}

/// Read a render-world resource back from the `RenderApp` after a frame — `None`
/// if the `RenderApp` or the resource is absent. DRYs the
/// `get_sub_app(RenderApp).world().get_resource::<R>()` idiom the spine / readback
/// tests share.
pub fn render_world_resource<R: Resource>(app: &App) -> Option<&R> {
    app.get_sub_app(bevy::render::RenderApp)?
        .world()
        .get_resource::<R>()
}

// ---------------------------------------------------------------------------
// Render-to-texture + readback capture infra (gate-#2 keystone).
//
// `gpu_test_app` proves the harness drives a frame and packs the instance
// buffers, but it deliberately omits `CorePipelinePlugin`, so the `Core2d`
// sub-graph never exists — `node::register`'s `add_render_graph_node(Core2d, …)`
// only *warns* when the sub-graph is missing (bevy_render render_graph/app.rs),
// so the Buiy node is never wired into a graph and never executes. NO pixels are
// painted. This builder is the painting-capable sibling: it adds
// `CorePipelinePlugin` (→ `Core2dPlugin`, which `add_render_sub_graph(Core2d)`s
// the 2D graph) BEFORE `BuiyRenderPlugin`, so the Buiy node lands inside a live
// `Core2d` graph and its `StartMainPassPostProcessing → BuiyRenderLabel →
// Tonemapping` edges resolve. A `CameraDriverNode` then runs that graph for the
// offscreen view, painting into the render-target image read back below.
//
// The primary window resolution is set to the capture size so the per-view
// `BuiyViewUniform` (built in prepare from `ExtractedNodes.logical_size`, which
// `extract_buiy_nodes` fills from the primary window — architecture § 4, D2:
// every Node resolves to the primary view) matches the offscreen target's pixel
// grid. Without that match the logical→clip transform would scale geometry to
// the window, not the image.

/// Painting-capable headless-GPU app: `gpu_test_app`'s stack PLUS
/// `CorePipelinePlugin` (the `Core2d` graph) and `LayoutPlugin`, with the
/// primary window sized to `width`×`height` so the view uniform matches the
/// capture image. Like [`gpu_test_app`] the returned [`App`] is NOT finished —
/// use [`finish_and_run`]. Spawn `(Node, Style, Background)` entities + a
/// capture camera ([`spawn_capture_camera`]) before driving frames.
pub fn gpu_render_app(width: u32, height: u32) -> App {
    // `WindowResolution::new` takes PHYSICAL units; at the default scale
    // factor 1.0 physical == logical, so `width`×`height` is both.
    gpu_render_app_with_resolution(bevy::window::WindowResolution::new(width, height))
}

/// [`gpu_render_app`] at an explicit window scale factor — the viewport-axis
/// builder (text campaign T9): the SAME plugin stack, with the primary window
/// LOGICAL `logical_w`×`logical_h` at `scale_factor` device pixels per
/// logical pixel.
///
/// Bevy 0.18's `WindowResolution::new` takes **physical** units (verified at
/// implementation, bevy_window-0.18.1 window.rs `new(physical_width,
/// physical_height)`), and `with_scale_factor_override` does not touch the
/// physical size — so this builder passes `logical × scale_factor` physical
/// plus the override, and `resolution.size()` reads back the logical size the
/// view uniform is built from.
///
/// **Contract:** the capture image must be sized to the window's **physical**
/// size (`logical_w × scale_factor`, `logical_h × scale_factor`) — the view
/// uniform maps logical (0,0)..(w,h) to the full clip square, so the
/// offscreen target supplies the physical pixel grid.
pub fn gpu_render_app_scaled(logical_w: u32, logical_h: u32, scale_factor: f32) -> App {
    let resolution = bevy::window::WindowResolution::new(
        (logical_w as f32 * scale_factor).round() as u32,
        (logical_h as f32 * scale_factor).round() as u32,
    )
    .with_scale_factor_override(scale_factor);
    gpu_render_app_with_resolution(resolution)
}

/// The one shared plugin stack behind [`gpu_render_app`] /
/// [`gpu_render_app_scaled`] — delegates to the promoted src builder
/// `buiy_core::render::golden::capture_app_with_resolution` so the canonical
/// plugin stack lives in exactly one place (anti-drift: the reftest / golden
/// tiers and these test-support builders are now the SAME body).
fn gpu_render_app_with_resolution(resolution: bevy::window::WindowResolution) -> App {
    buiy_core::render::golden::capture_app_with_resolution(resolution)
}

/// Create an offscreen `Rgba8UnormSrgb` render-target image of `width`×`height`,
/// add the `COPY_SRC` usage the readback copy needs (the constructor sets
/// `RENDER_ATTACHMENT | COPY_DST | TEXTURE_BINDING` but not `COPY_SRC`), force
/// `RenderAssetUsages::all()` so the `GpuImage` exists in the render world, and
/// insert it into `Assets<Image>`. Returns the handle.
pub fn render_to_image(app: &mut App, width: u32, height: u32) -> Handle<Image> {
    // `Rgba8UnormSrgb` == `ViewTarget::main_texture_format()` for a non-HDR
    // Camera2d (== `BuiyPipeline`'s target format), so the Buiy pipeline binds
    // to this view without a format mismatch.
    let mut image = Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
    // The GpuReadback copy is a texture→buffer COPY_SRC; the constructor omits it.
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    // Without RENDER_WORLD the GpuImage is never created, so the readback (which
    // looks the handle up in `RenderAssets<GpuImage>`) finds nothing. `all()`
    // keeps MAIN_WORLD too — harmless here, and the documented render-target idiom.
    image.asset_usage = RenderAssetUsages::all();
    app.world_mut().resource_mut::<Assets<Image>>().add(image)
}

/// Spawn a `Camera2d` whose render target is `target`, so `BuiyNode::run` paints
/// into the offscreen image's `ViewTarget`. The clear color is forced opaque
/// black (`ClearColorConfig::Custom`) — a deterministic backdrop the SrcOver
/// composite is asserted against (the global default `ClearColor` is an opaque
/// dark gray, not a clean zero).
///
/// `Msaa::Off` — single-sampled for pixel determinism (most readback tests
/// assert exact pixel values, and a 4x resolve antialiases edges). The Buiy
/// pipelines specialize per-view on the sample count (`prepare_buiy_view_pipelines`),
/// so both `Off` and `Sample4` views work; the multisampled path is covered by
/// `tests/render_msaa.rs` via [`spawn_capture_camera_with_msaa`].
pub fn spawn_capture_camera(app: &mut App, target: Handle<Image>) {
    spawn_capture_camera_with_msaa(app, target, bevy::render::view::Msaa::Off);
}

/// [`spawn_capture_camera`] with an explicit per-view [`Msaa`](bevy::render::view::Msaa)
/// mode — the MSAA regression tests (`tests/render_msaa.rs`) spawn the capture
/// camera at `Msaa::Sample4` (the bare-`Camera2d` default a real app gets) to
/// prove the per-view sample-count pipeline specialization.
pub fn spawn_capture_camera_with_msaa(
    app: &mut App,
    target: Handle<Image>,
    msaa: bevy::render::view::Msaa,
) {
    app.world_mut().spawn((
        Camera2d,
        // `RenderTarget` is a standalone component in Bevy 0.18 (no longer a
        // `Camera` field); spawning it overrides the default primary-window target.
        RenderTarget::from(target),
        msaa,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
    ));
}

/// Drive frames until the text fixture's `wait_for_fonts` predicate holds
/// (verification § 3.2): the producer has emitted (`ResidentTextKeys`
/// non-empty), the warmup queue is drained, and every emitted key is
/// resident. Returns frames driven; panics past `max_frames`.
pub fn wait_for_text_ready(app: &mut App, max_frames: usize) -> usize {
    use buiy_core::render::atlas::{AtlasWarmupQueue, BuiyAtlas};
    use buiy_core::render::golden::fonts_ready;
    use buiy_core::text::ResidentTextKeys;

    for frame in 0..max_frames {
        app.update();
        let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
        let world = render_app.world();
        let resident = world.resource::<ResidentTextKeys>();
        if !resident.keys.is_empty()
            && fonts_ready(
                world.resource::<BuiyAtlas>(),
                world.resource::<AtlasWarmupQueue>(),
                &resident.keys,
            )
        {
            return frame + 1;
        }
    }
    panic!("text never became atlas-resident within {max_frames} frames");
}

/// A committed per-script fixture font (verification § 2.2; produced ONLY
/// by `tools/fonts/subset_fixture_fonts.sh` — pinned upstreams + sha256 +
/// pinned fonttools, never hand-edited).
pub fn fixture_font_bytes(file_name: &str) -> Arc<Vec<u8>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fonts")
        .join(file_name);
    Arc::new(std::fs::read(&path).unwrap_or_else(|e| {
        panic!("fixture font {file_name} missing ({e}); run tools/fonts/subset_fixture_fonts.sh")
    }))
}

/// Register a fixture font through the production bytes path
/// (`FontRegistry::register_bytes` → `apply_font_registry`) and settle one
/// update so the engine + `FontMatchIndex` see it. `family` must be the
/// subset's declared family name verbatim (the resolver queries by name;
/// a mismatch will not match — T5 plan decision 4).
pub fn register_fixture_font(app: &mut App, family: &str, file_name: &str) {
    use buiy_core::text::{FontFaceDescriptors, FontRegistry};

    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes(
            family,
            fixture_font_bytes(file_name),
            FontFaceDescriptors::default(),
        );
    app.update();
}

// ---------------------------------------------------------------------------
// Adapter probe: is the SELECTED wgpu adapter the pinned lavapipe (llvmpipe)?
//
// `buiy_verify::support::on_pinned_lavapipe` is the workspace source of truth
// for this (the golden gates consult it), but `buiy_core` CANNOT depend on
// `buiy_verify` (wrong dep direction — `buiy_verify` depends on `buiy_core`),
// so the GPU `#[ignore]` tests in `buiy_core/tests/` carry their OWN small twin
// here. It is a faithful mirror: the same env fast-path + the same
// RenderAdapterInfo name/driver substring check, over `buiy_core`'s OWN capture
// stack (`gpu_render_app`), and memoized once per process.
//
// Why a probe at all: stored-baseline / SDF-AA pixel claims are blessed against
// the canonical CI rasterizer (pinned lavapipe / Mesa llvmpipe). On any other
// adapter the rim/AA pixels diverge (this host's RX 6700 XT / RADV hard-edges
// the SDF-AA quad band where lavapipe leaves a 0.84375-alpha row), so a
// lavapipe-specific pixel assertion must run ONLY when this returns `true` and
// skip-as-pending otherwise. See `determinism.md` § "CI software-rasterizer
// pin" and `buiy_verify::support::on_pinned_lavapipe` (the twin this mirrors).
// ---------------------------------------------------------------------------

/// The case-insensitive substring identifying the pinned Mesa software
/// rasterizer in a wgpu adapter name/driver AND in the CI `WGPU_ADAPTER_NAME`
/// env contract (the device reports `llvmpipe (LLVM …)`; the CI install-mesa
/// action exports `WGPU_ADAPTER_NAME=llvmpipe`).
const LAVAPIPE_MARKER: &str = "llvmpipe";

/// Memoized [`on_pinned_lavapipe`] result — the probe instantiates a wgpu
/// adapter (or reads the env), so a serialized GPU lane pays it at most once.
static ON_PINNED_LAVAPIPE: OnceLock<bool> = OnceLock::new();

/// Is the SELECTED wgpu adapter the pinned lavapipe (Mesa llvmpipe)?
///
/// The gate for every lavapipe-specific PIXEL assertion in the `buiy_core` GPU
/// `#[ignore]` tests (the SDF-AA band signature, exact rim encodes — pixels
/// blessed against the pinned CI rasterizer that do not hold on real hardware).
/// Compare against the lavapipe-specific value only when `true`; otherwise skip
/// it as pending after the rasterizer-internal legs (band count, re-capture
/// determinism) have run. Mirror of
/// [`buiy_verify::support::on_pinned_lavapipe`] (`buiy_core` cannot depend on
/// `buiy_verify`). Two signals, first decisive wins:
///
///  1. **CI env contract** — `WGPU_ADAPTER_NAME` contains `llvmpipe`
///     (`.github/actions/install-mesa` exports it): the pin is active, return
///     `true` without instantiating an adapter.
///  2. **Real adapter probe** — otherwise build the canonical capture stack,
///     finish it (materializing `RenderAdapterInfo`), and check the selected
///     adapter's `name`/`driver` for `llvmpipe`. Returns `false` on the RX
///     (RADV) and `true` if `VK_DRIVER_FILES` points at lavapipe locally.
///
/// Conservative: any failure to materialize an adapter returns `false` —
/// "not provably lavapipe" must gate OFF the lavapipe-specific assertion.
pub fn on_pinned_lavapipe() -> bool {
    *ON_PINNED_LAVAPIPE.get_or_init(|| {
        if let Some(name) = std::env::var_os("WGPU_ADAPTER_NAME")
            && name
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains(LAVAPIPE_MARKER)
        {
            return true;
        }
        probe_selected_adapter_is_lavapipe()
    })
}

/// Build a minimal capture app, finish it (materializing the wgpu device +
/// `RenderAdapterInfo`), and report whether the selected adapter is lavapipe.
/// `false` if no `RenderApp` / adapter info materializes (no adapter).
fn probe_selected_adapter_is_lavapipe() -> bool {
    use bevy::render::RenderApp;
    use bevy::render::renderer::RenderAdapterInfo;

    // The same capture stack a GPU test selects (1×1 — we only read the info),
    // so the probed adapter is byte-identical to the one captures use.
    let mut app = gpu_render_app(1, 1);
    app.finish();
    app.cleanup();

    let Some(render_app) = app.get_sub_app(RenderApp) else {
        return false;
    };
    let Some(info) = render_app.world().get_resource::<RenderAdapterInfo>() else {
        return false;
    };
    adapter_info_is_lavapipe(&info.name, &info.driver)
}

/// Pure predicate: does this adapter `name`/`driver` identify lavapipe (Mesa
/// llvmpipe)? Split out so it is unit-testable without an adapter. The device
/// reports `name = "llvmpipe (LLVM …)"`, `driver = "llvmpipe"`; matching either
/// (case-insensitive) covers both surfaces.
fn adapter_info_is_lavapipe(name: &str, driver: &str) -> bool {
    name.to_ascii_lowercase().contains(LAVAPIPE_MARKER)
        || driver.to_ascii_lowercase().contains(LAVAPIPE_MARKER)
}

/// Index one RGBA8 pixel out of an un-padded `w*h*4` readback buffer.
pub fn px(pixels: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

/// The sRGB8 the target stores for a FULL-coverage texel of linear
/// straight-alpha `color` over the opaque-black clear: SrcOver in linear
/// (dst = 0), then the Rgba8UnormSrgb linear→sRGB encode.
pub fn expected_full_coverage_srgb(color: [f32; 4]) -> [u8; 4] {
    let a = color[3];
    let lin = LinearRgba::new(color[0] * a, color[1] * a, color[2] * a, 1.0);
    let s = Srgba::from(lin);
    [
        (s.red * 255.0).round() as u8,
        (s.green * 255.0).round() as u8,
        (s.blue * 255.0).round() as u8,
        255,
    ]
}

/// Spawn `Readback::texture(target)`, observe its `ReadbackComplete`, and POLL
/// `app.update()` until the bytes arrive — condition-based, NOT a fixed frame
/// count: the pipeline async-compiles, prepares, paints, copies, and maps across
/// several frames, so the number of frames is not knowable up front. Bounded by
/// `MAX_FRAMES`; panics with a clear message if the readback never fires.
///
/// Returns the un-padded `width*height*4` RGBA8 bytes. The raw readback buffer
/// keeps wgpu's 256-byte ROW PADDING whenever `width * 4` is not already
/// 256-aligned (a 32-px-wide target comes back as 256-byte rows = 2× the
/// pixels; every 64-px-wide test was aligned by luck, which hid this). The
/// padding is stripped HERE so callers can index `chunks_exact(4)` safely —
/// padding bytes are `[0,0,0,0]`, which would otherwise satisfy a
/// `px != clear` probe and false-green a "something painted" assertion.
pub fn readback_rgba(app: &mut App, target: Handle<Image>) -> Vec<u8> {
    // The target's true extent — the promoted readback needs it to detect +
    // strip wgpu's 256-byte row padding.
    let (width, height) = {
        let images = app.world().resource::<Assets<Image>>();
        let image = images.get(&target).expect("readback target Image exists");
        (
            image.texture_descriptor.size.width,
            image.texture_descriptor.size.height,
        )
    };
    // Delegate to the promoted src twin so the readback poll + row-padding
    // strip live in exactly one place (Phase 0.4 anti-drift).
    buiy_core::render::golden::readback_rgba_into(app, &target, width, height)
}
