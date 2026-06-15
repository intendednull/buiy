//! The e2e golden-image harness (gate #2). The only proof of pixels, so its
//! reliability is load-bearing (verification.md § 4). This module owns the
//! device-free pieces — the flake-mitigation triad config (§ 4.3), the
//! perceptual-diff metric + tolerance-budget seam (§ 4.2), and the human-
//! curated `--accept` workflow flag (§ 4.4). The capture itself runs only on
//! the canonical CI GPU class (§ 4.1) behind an `#[ignore]` test.
//!
//! Per-fixture tolerance/perf/leak *numbers* are owned by
//! `buiy-verification-design`; this module commits to *having* a budget, not
//! its value.

use crate::render::atlas::{AtlasKey, AtlasWarmupQueue, BuiyAtlas};

/// Deterministic-capture configuration. The three flake sources of § 4.3 are
/// *necessary together*: a golden captured without all three is not
/// reproducible. `accept` is the § 4.4 human-curated golden-update gate —
/// never an automatic overwrite.
#[derive(Clone, Copy, Debug)]
pub struct GoldenConfig {
    /// Drive time from a fixed/virtual clock, not wall time, so any time-
    /// dependent visual is captured at a deterministic instant (§ 4.3.1).
    pub fixed_clock: bool,
    /// Block capture until every referenced font is loaded and its glyphs are
    /// resident (§ 4.3.2) — a half-loaded font flips the diff.
    pub wait_for_fonts: bool,
    /// Warm the texture atlas (glyphs/icons/gradients) before capture (§ 4.3.3)
    /// so first-frame upload latency does not perturb the image. Also
    /// establishes the gate-#15 steady-state baseline.
    pub warm_atlas: bool,
    /// `--accept`: update the stored golden instead of failing on mismatch.
    /// Off by default; gated behind human PR review (§ 4.4).
    pub accept: bool,
}

impl GoldenConfig {
    /// The capture config with the full flake-mitigation triad pinned and
    /// `accept` off — the configuration every golden is captured under.
    pub fn deterministic() -> Self {
        Self {
            fixed_clock: true,
            wait_for_fonts: true,
            warm_atlas: true,
            accept: false,
        }
    }
}

/// **Canonical device-pixel-ratio type.** Integer *milliscale* (1000 = 1.0×,
/// 2000 = 2.0×) so it is `Eq + Hash + Ord` without float pitfalls — it is a
/// *fixture axis* that keys a golden / coverage cell, **never** a tolerance.
///
/// Defined ONCE here; `buiy_verify::golden::GoldenKey.dpr` and
/// `buiy_verify::coverage::{Matrix.dprs, CoverageKey.dpr}` import this type,
/// they do **not** redefine it (verification-design `determinism.md`). The
/// capture boundary converts the window's `f32` `scale_factor` via
/// [`Dpr::from_f32`] and back via [`Dpr::as_f32`] when sizing the offscreen
/// target. Derives `serde` so the golden bless ledger can persist it directly.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Dpr(pub u32);

impl Dpr {
    /// 1.0× device-pixel-ratio (the headless capture default).
    pub const X1: Self = Dpr(1000);
    /// 2.0× device-pixel-ratio (the HiDPI fixture axis).
    pub const X2: Self = Dpr(2000);

    /// Round an `f32` scale factor to integer milliscale (`1.0 → Dpr(1000)`).
    /// Rounds to nearest so a `1.5×` window maps to `Dpr(1500)` exactly.
    pub fn from_f32(scale: f32) -> Self {
        Dpr((scale * 1000.0).round() as u32)
    }

    /// Back to the `f32` scale factor the window / extract path consumes.
    pub fn as_f32(&self) -> f32 {
        self.0 as f32 / 1000.0
    }
}

/// Single-sampled capture: a 4× MSAA resolve antialiases edges
/// nondeterministically across drivers, while Buiy's in-shader analytic AA is
/// deterministic given identical FP — so MSAA buys nothing here and costs
/// determinism. Mirrors the capture camera's landed `Msaa::Off`
/// (verification-design `determinism.md`).
pub const CAPTURE_MSAA: bevy::render::view::Msaa = bevy::render::view::Msaa::Off;

/// Deband dither perturbs the low bits of the tonemapped output; the capture
/// camera pins it off. A `true` sentinel the capture path documents (the
/// camera spawns with no `DebandDither::Enabled`).
pub const CAPTURE_DITHER_OFF: bool = true;

/// Build the canonical headless painting App at a logical viewport size,
/// promoted from `tests/support/mod.rs` into src so `buiy_verify`'s reftest /
/// golden tiers build their app without the test crate. NOT finished:
/// [`capture_to_image`] finishes + drives to quiescence + reads back.
pub fn capture_app(logical_w: u32, logical_h: u32) -> bevy::app::App {
    capture_app_scaled(logical_w, logical_h, 1.0)
}

/// [`capture_app`] at an explicit window scale factor (the DPR-pin builder
/// determinism.md sizes the offscreen target through). Bevy 0.18
/// `WindowResolution::new` takes PHYSICAL units; pass `logical × scale` plus
/// the override so `resolution.size()` reads back the logical size the view
/// uniform is built from.
pub fn capture_app_scaled(logical_w: u32, logical_h: u32, scale_factor: f32) -> bevy::app::App {
    use bevy::window::WindowResolution;
    let resolution = WindowResolution::new(
        (logical_w as f32 * scale_factor).round() as u32,
        (logical_h as f32 * scale_factor).round() as u32,
    )
    .with_scale_factor_override(scale_factor);
    capture_app_with_resolution(resolution)
}

/// The one shared plugin stack behind [`capture_app`] / [`capture_app_scaled`]
/// (and, via delegation, the test-support `gpu_render_app*` builders) — a
/// single body so the scaled / test-support builders cannot drift. The plugin
/// set + init order MUST stay byte-identical to the documented capture stack
/// (the offscreen `Core2d` graph `BuiyRenderPlugin` wires into requires
/// `CorePipelinePlugin` before it).
pub fn capture_app_with_resolution(resolution: bevy::window::WindowResolution) -> bevy::app::App {
    use bevy::app::App;
    use bevy::prelude::*;
    use bevy::window::{Window, WindowPlugin};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(WindowPlugin {
            primary_window: Some(Window {
                resolution,
                ..default()
            }),
            ..default()
        })
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::render::RenderPlugin::default())
        .add_plugins(bevy::image::ImagePlugin::default())
        .add_plugins(bevy::camera::CameraPlugin)
        .add_plugins(bevy::core_pipeline::CorePipelinePlugin)
        .add_plugins(crate::theme::ThemePlugin)
        .add_plugins(crate::layout::LayoutPlugin)
        .add_plugins(crate::CorePlugin)
        .add_plugins(crate::text::BuiyTextPlugin::default())
        .add_plugins(crate::render::BuiyRenderPlugin);
    app.init_asset::<Mesh>();
    app
}

/// **The shared capture seam** (verification-design README § Architecture):
/// render the already-built, fixture-populated `app` into an offscreen target
/// sized to the window's PHYSICAL pixel grid and read it back as an
/// `image::RgbaImage`. Re-runnable against one `App` (a reftest calls it twice
/// on one device; spec § "Resolved during synthesis" #4).
///
/// Phase-0 scope: the capture mechanics (size-to-physical, paint, readback,
/// assemble). The four-condition quiescence flush and the
/// `scale_factor == cfg.dpr` assertion are Phase 3.3's hardening of this same
/// function (`determinism.md` § Async-asset flush).
///
/// Drives `MAX_CAPTURE_FRAMES` update frames after finishing the app (pipeline
/// async-compile + extract + prepare + paint settle), then reads back the
/// offscreen target's un-padded RGBA8 bytes.
pub fn capture_to_image(app: &mut bevy::app::App, _cfg: &GoldenConfig) -> image::RgbaImage {
    use bevy::asset::RenderAssetUsages;
    use bevy::camera::RenderTarget;
    use bevy::image::Image;
    use bevy::prelude::*;
    use bevy::render::render_resource::{TextureFormat, TextureUsages};

    // Physical pixel grid the offscreen target must match: the primary
    // window's physical size (logical × scale_factor), which the view uniform
    // is built from (extract fills `logical_size` from the primary window).
    let (phys_w, phys_h) = {
        let window = app
            .world_mut()
            .query::<&bevy::window::Window>()
            .single(app.world())
            .expect("primary window for capture sizing");
        let r = window.resolution.physical_size();
        (r.x, r.y)
    };

    // Offscreen Rgba8UnormSrgb target with COPY_SRC for the readback copy and
    // RenderAssetUsages::all() so the GpuImage exists in the render world.
    let target = {
        let mut image =
            Image::new_target_texture(phys_w, phys_h, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        image.asset_usage = RenderAssetUsages::all();
        app.world_mut().resource_mut::<Assets<Image>>().add(image)
    };

    // Capture camera: opaque-black clear, CAPTURE_MSAA (single-sampled),
    // dither off (bare Camera2d at Msaa::Off carries no DebandDither::Enabled).
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::from(target.clone()),
        CAPTURE_MSAA,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
    ));

    // Finish materializes the device + pipelines; drive frames so layout →
    // extract → prepare → paint settle before the readback poll.
    const MAX_CAPTURE_FRAMES: usize = 3;
    app.finish();
    app.cleanup();
    for _ in 0..MAX_CAPTURE_FRAMES {
        app.update();
    }

    let bytes = readback_rgba_into(app, &target, phys_w, phys_h);
    image::RgbaImage::from_raw(phys_w, phys_h, bytes)
        .expect("readback byte count matches phys_w * phys_h * 4")
}

/// Resource cell the `ReadbackComplete` observer writes the captured bytes
/// into. `Arc<Mutex<…>>` so the observer (which `move`s its capture) and the
/// poll loop share one slot. The src twin of the test-support `CapturedBytes`.
#[derive(bevy::ecs::resource::Resource, Clone, Default)]
struct CapturedBytes(std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>);

/// Spawn `Readback::texture(target)`, observe its `ReadbackComplete`, and POLL
/// `app.update()` until the bytes arrive — condition-based, NOT a fixed frame
/// count: the pipeline async-compiles, prepares, paints, copies, and maps
/// across several frames, so the number of frames is not knowable up front.
/// Bounded by `MAX_FRAMES`; panics with a clear message if the readback never
/// fires.
///
/// Returns the un-padded `w*h*4` RGBA8 bytes. The raw readback buffer keeps
/// wgpu's 256-byte ROW PADDING whenever `w * 4` is not already 256-aligned;
/// the padding is stripped HERE so callers can index `chunks_exact(4)` safely.
/// The src twin of `tests/support/mod.rs`'s `readback_rgba`; the support
/// helper delegates here so the readback body lives in exactly one place.
pub fn readback_rgba_into(
    app: &mut bevy::app::App,
    target: &bevy::asset::Handle<bevy::image::Image>,
    w: u32,
    h: u32,
) -> Vec<u8> {
    use bevy::prelude::*;
    use bevy::render::gpu_readback::{Readback, ReadbackComplete};

    const MAX_FRAMES: usize = 60;
    let (width, height) = (w as usize, h as usize);

    let cell = CapturedBytes::default();
    app.insert_resource(cell.clone());

    let sink = cell.0.clone();
    app.world_mut()
        .spawn(Readback::texture(target.clone()))
        .observe(move |trigger: On<ReadbackComplete>| {
            // `ReadbackComplete` derefs to its `data: Vec<u8>`; clone the raw
            // RGBA8 into the shared slot. First completion wins (the readback
            // re-fires every frame until its entity is despawned, but the poll
            // loop stops at the first non-empty slot).
            let mut slot = sink.lock().expect("readback sink mutex");
            if slot.is_none() {
                slot.replace(trigger.event().data.clone());
            }
        });

    for _ in 0..MAX_FRAMES {
        app.update();
        if cell.0.lock().expect("readback sink mutex").is_some() {
            break;
        }
    }

    let data = cell
        .0
        .lock()
        .expect("readback sink mutex")
        .take()
        .unwrap_or_else(|| {
            panic!(
                "GPU readback never delivered bytes within {MAX_FRAMES} frames — \
                 the texture→buffer copy or buffer map never completed (check that \
                 the image carries COPY_SRC + RenderAssetUsages::all() and that a \
                 capture camera targets it)"
            )
        });

    // Strip wgpu's 256-byte row padding if present (see the doc comment).
    let unpadded_row = width * 4;
    let padded_row = unpadded_row.div_ceil(256) * 256;
    if data.len() == unpadded_row * height {
        data
    } else if data.len() == padded_row * height {
        let mut out = Vec::with_capacity(unpadded_row * height);
        for row in 0..height {
            let start = row * padded_row;
            out.extend_from_slice(&data[start..start + unpadded_row]);
        }
        out
    } else {
        panic!(
            "readback returned {} bytes for a {width}x{height} RGBA8 target — \
             expected {} (unpadded) or {} (256-byte-padded rows)",
            data.len(),
            unpadded_row * height,
            padded_row * height,
        );
    }
}

/// Perceptual difference between two RGBA8 frames, as a normalized mean
/// per-channel difference in `[0.0, 1.0]` (0 == identical). Comparison is
/// *perceptual*, not exact byte equality (§ 4.2): sub-LSB float jitter in the
/// SDF and linear→sRGB encode is invisible but not bit-stable, so the caller
/// compares this against an explicit per-fixture tolerance budget (owned by
/// `buiy-verification-design`) — the budget is the line between jitter and
/// regression. Frames must be the same length (same dimensions); mismatched
/// lengths return `1.0` (maximal difference).
#[deprecated(
    note = "use buiy_verify::metric::compare; kept only for unmigrated ignored GPU re-capture tests"
)]
pub fn perceptual_diff(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let sum: f64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| (x as f64 - y as f64).abs())
        .sum();
    (sum / (a.len() as f64 * 255.0)) as f32
}

/// verification § 3.2 — [`GoldenConfig::wait_for_fonts`], flipped from
/// declared flag to implemented predicate. With embedded deterministic
/// fonts, registration is synchronous at `FontSystem` construction (nothing
/// asynchronous exists to wait on), so "fonts ready" reduces to: the warmup
/// queue is drained AND every glyph key the fixture's producer emitted is
/// resident — probed via the **no-LRU-touch** [`BuiyAtlas::get`], so the
/// check never perturbs eviction order.
///
/// § 3.3 (`warm_atlas`) is satisfied STRUCTURALLY for text fixtures: the
/// producer inserts at extract, before Prepare's upload and the node's draw
/// (glyph-pipeline § 6.4), so by the time this predicate holds the atlas is
/// warm. `AtlasWarmupQueue` remains the seam for the optional production
/// ASCII pre-warm (rejected — text campaign T9; architecture § 2.3) and T6's
/// solid stamp.
pub fn fonts_ready(
    atlas: &BuiyAtlas,
    warmup: &AtlasWarmupQueue,
    visible_keys: &[AtlasKey],
) -> bool {
    warmup.is_empty() && visible_keys.iter().all(|key| atlas.get(key).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpr_milliscale_round_trips_f32() {
        // The canonical fixture axis: integer milliscale so it is Eq+Hash+Ord,
        // but it must convert losslessly to/from the f32 scale_factor the
        // window/extract path carries (determinism.md § Extending GoldenConfig).
        assert_eq!(Dpr::from_f32(1.0), Dpr::X1);
        assert_eq!(Dpr::from_f32(2.0), Dpr::X2);
        assert_eq!(Dpr::X1.as_f32(), 1.0);
        assert_eq!(Dpr::X2.as_f32(), 2.0);
        // Round-trip through both directions for a fractional ratio (1.5×).
        assert_eq!(Dpr::from_f32(1.5), Dpr(1500));
        assert_eq!(Dpr(1500).as_f32(), 1.5);
        // from_f32 rounds to nearest milliscale (no truncation drift).
        assert_eq!(Dpr::from_f32(1.2345), Dpr(1235));
    }

    #[test]
    fn dpr_is_ord_and_hashable() {
        // It keys a golden/coverage cell, so Ord + Hash must hold (the reason
        // for milliscale over f32). A plain compile-and-run proof.
        use std::collections::HashSet;
        assert!(Dpr::X1 < Dpr::X2);
        let mut set = HashSet::new();
        assert!(set.insert(Dpr::X1));
        assert!(!set.insert(Dpr::X1)); // already present — Hash + Eq agree
        assert!(set.insert(Dpr::X2));
    }
}
