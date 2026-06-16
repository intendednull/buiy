//! The determinism substrate (verification-design `determinism.md`): the one
//! public seam every GPU tier (reftest, golden) constructs its capture app
//! through, with every nondeterminism knob pinned at the source.
//!
//! This module owns the *setup* — the [`FontMode::Ahem`] box-font substitution
//! (so text-bearing captures are host-stable), the fixed virtual clock, the DPR
//! pin, and the MSAA/dither pin — while `buiy_core::render::golden`'s
//! [`capture_to_image`](buiy_core::render::golden::capture_to_image) owns the
//! *capture* (size-to-physical, quiescence flush, readback).
//!
//! `FontMode` / `Dpr` are **re-exported** from their canonical home in
//! `buiy_core::render::golden` (where `GoldenConfig` carries them), never
//! redefined here.

use bevy::prelude::*;
use buiy_core::text::{FontFaceDescriptors, FontRegistry};
use std::sync::Arc;

// Re-export the canonical config types from their home in buiy_core. Tiers
// import `FontMode` / `Dpr` from here OR from `buiy_core::render::golden` —
// they are the same types (this is a re-export, not a redefinition).
pub use buiy_core::render::golden::{Dpr, FontMode, GoldenConfig};

/// The family name the Ahem box-font registers under and that fixture text
/// must name (`font-family: Ahem`) to resolve to it under [`FontMode::Ahem`].
pub const AHEM_FAMILY: &str = "Ahem";

/// The committed Ahem face — the W3C/WPT public-domain em-box font, baked into
/// the test binary so the box-font substitution needs no filesystem read at
/// capture time. Every glyph is a solid em-square, so any non-fidelity golden
/// is byte-identical across hosts (`determinism.md` § "Ahem font mode").
static AHEM_TTF: &[u8] = include_bytes!("../../buiy_core/tests/fixtures/fonts/Ahem.ttf");

/// The Ahem face's raw bytes, ready for the production registration path.
/// `Arc`-wrapped to match [`FontRegistry::register_bytes`]'s signature without
/// copying the ~21 KB face on every call.
fn ahem_bytes() -> Arc<Vec<u8>> {
    Arc::new(AHEM_TTF.to_vec())
}

/// Register the Ahem box-font through the **production bytes path**
/// ([`FontRegistry::register_bytes`]) under family [`AHEM_FAMILY`], then settle
/// one update so `apply_font_registry` rebuilds the engine + `FontMatchIndex`
/// and the resolver can see it. This is the capture-time substitution
/// `FontMode::Ahem` performs; combined with system fonts being off (the
/// headless capture stack runs bundled-only), Ahem is the only resolvable
/// family for fixture text that names it — fallback cannot reintroduce a
/// host-specific platform font.
///
/// The `app` must already carry a `FontRegistry` (any `BuiyTextPlugin` app
/// does). Settles one `app.update()` so the engine + `FontMatchIndex` see the
/// face immediately — use on a NON-render app (the headless resolver tests). On
/// a render app, `app.update()` before `app.finish()` trips a render system, so
/// the [`DeterministicApp`] build path uses [`stage_ahem`] instead and lets the
/// capture's post-finish quiescence loop settle it. Idempotent.
pub fn register_ahem(app: &mut App) {
    stage_ahem(app);
    app.update();
}

/// Stage the Ahem registration through the production bytes path WITHOUT
/// settling — `apply_font_registry` drains it on the next `app.update()`. The
/// settle-free twin of [`register_ahem`] for the capture build path, where the
/// first update happens inside `capture_to_image` after `app.finish()`.
pub fn stage_ahem(app: &mut App) {
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes(AHEM_FAMILY, ahem_bytes(), FontFaceDescriptors::default());
}

/// The single public seam every GPU tier (reftest, golden) constructs its
/// capture app through, with **every** nondeterminism knob pinned at the source
/// (`determinism.md` § "DeterministicApp builder"):
///
///   * the DPR pin — built via `capture_app_scaled(w, h, cfg.dpr.as_f32())`;
///   * the fixed virtual clock — `TimeUpdateStrategy::ManualDuration(ZERO)`, so
///     every `app.update()` advances `Time` by a fixed zero delta, never wall
///     time, and the capture's quiescence loop terminates deterministically;
///   * the Ahem box-font as the sole resolvable family when
///     `cfg.font_mode == Ahem` (host-stable text);
///   * the MSAA / dither pin — applied by [`capture_to_image`] when it spawns
///     the capture camera (`CAPTURE_MSAA`, dither off).
///
/// It owns the *setup*; `buiy_core::render::golden::capture_to_image` owns the
/// *capture* (size-to-physical, quiescence flush, readback). The single-call
/// [`DeterministicApp::capture`] path tiers use is `build` + spawn-fixture +
/// `capture_to_image`.
///
/// [`capture_to_image`]: buiy_core::render::golden::capture_to_image
#[derive(Clone, Copy, Debug)]
pub struct DeterministicApp {
    cfg: GoldenConfig,
    logical: (u32, u32),
}

impl DeterministicApp {
    /// Default-deterministic at a logical viewport size: the full flake triad,
    /// `FontMode::Ahem`, `Dpr::X1`, MSAA/dither off (the `deterministic()`
    /// config). Override individual knobs with [`with`](Self::with) /
    /// [`font_mode`](Self::font_mode) / [`dpr`](Self::dpr).
    pub fn new(logical_w: u32, logical_h: u32) -> Self {
        Self {
            cfg: GoldenConfig::deterministic(),
            logical: (logical_w, logical_h),
        }
    }

    /// Replace the whole capture config (e.g. `GoldenConfig::fidelity()` for the
    /// real-glyph suite). The logical viewport size is unchanged.
    pub fn with(mut self, cfg: GoldenConfig) -> Self {
        self.cfg = cfg;
        self
    }

    /// Override the font axis only (default [`FontMode::Ahem`]).
    pub fn font_mode(mut self, mode: FontMode) -> Self {
        self.cfg.font_mode = mode;
        self
    }

    /// Override the DPR axis only (default [`Dpr::X1`]).
    pub fn dpr(mut self, dpr: Dpr) -> Self {
        self.cfg.dpr = dpr;
        self
    }

    /// The capture config this builder applies (the value `capture` passes to
    /// `capture_to_image`). Lets a caller read back the resolved knobs.
    pub fn config(&self) -> GoldenConfig {
        self.cfg
    }

    /// Build a painting-capable headless `App` with every knob applied (see the
    /// type docs). A thin, **single-bodied** wrapper over the landed
    /// `capture_app_scaled` so the plugin stack cannot drift from the canonical
    /// capture stack. Returns an `App` ready for fixture spawn; the offscreen
    /// target + capture camera + readback are added by `capture_to_image`.
    pub fn build(self) -> App {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let (w, h) = self.logical;
        // The DPR pin: size the window to logical × dpr with the scale-factor
        // override, exactly as the capture path expects (the single landed
        // builder — no drift).
        let mut app = buiy_core::render::golden::capture_app_scaled(w, h, self.cfg.dpr.as_f32());

        // The fixed virtual clock: advance time by a fixed ZERO delta each
        // frame so the capture reads a deterministic instant, never wall time.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));

        // The font pin: under Ahem mode, STAGE the box-font through the
        // production bytes path (system fonts are already off in the capture
        // stack). We must not settle here — `app.update()` before `finish()`
        // trips a render system — so the registration drains on the first
        // update inside `capture_to_image`'s post-finish quiescence loop.
        if self.cfg.font_mode == FontMode::Ahem {
            stage_ahem(&mut app);
        }

        app
    }

    /// `build` + spawn the fixture + `capture_to_image(&app, &cfg)` — the
    /// one-call path the GPU tiers use. The capture internally drives the app to
    /// quiescence (asset/atlas/font/pipeline) and asserts the DPR pin before
    /// readback.
    pub fn capture(self, fixture: impl FnOnce(&mut App)) -> image::RgbaImage {
        let cfg = self.cfg;
        let mut app = self.build();
        fixture(&mut app);
        buiy_core::render::golden::capture_to_image(&mut app, &cfg)
    }
}
