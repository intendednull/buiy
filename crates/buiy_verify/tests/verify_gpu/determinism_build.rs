//! `DeterministicApp` knob tripwires (Phase 3.4, verification-design
//! `determinism.md` § "DeterministicApp builder").
//!
//! TWO tiers, because [`DeterministicApp::build`] instantiates the capture
//! render stack (`capture_app_scaled` → `RenderPlugin`), which **requires a
//! wgpu adapter** — `build()` is NOT headless (an earlier version wrongly ran
//! these in the every-PR gate; they pass on any machine WITH an adapter — local
//! GPU, macOS/Windows CI — but panic "Unable to find a GPU" on adapter-less
//! Linux CI, the gate that must stay green without one):
//!
//!   * **HEADLESS** config-level tripwires (no `build()`) inspect the resolved
//!     [`GoldenConfig`] knobs + the pinned MSAA constant — every-PR gate, no
//!     adapter.
//!   * **`#[ignore]`** built-app tripwires assert the knobs LAND on the built
//!     app (window scale_factor, the manual `TimeUpdateStrategy`); they need the
//!     capture adapter, so they run on the GPU lane next to
//!     `determinism_capture.rs`.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use buiy_core::render::golden::CAPTURE_MSAA;
use buiy_verify::determinism::{DeterministicApp, Dpr, FontMode};

// --- HEADLESS: config-level knob application (no GPU adapter) ---------------

#[test]
fn default_config_is_one_x_ahem() {
    // Without explicit overrides the builder is 1× DPR + Ahem font (the
    // `deterministic()` default) — readable straight off `config()`, no build.
    let a = DeterministicApp::new(48, 48);
    assert_eq!(a.config().dpr, Dpr::X1, "default DPR is 1×");
    assert_eq!(
        a.config().font_mode,
        FontMode::Ahem,
        "default font mode is Ahem"
    );
}

#[test]
fn dpr_override_flows_into_config() {
    let a = DeterministicApp::new(64, 64).dpr(Dpr::X2);
    assert_eq!(a.config().dpr, Dpr::X2, "dpr(X2) overrides the config DPR");
}

#[test]
fn font_mode_override_flows_into_cfg() {
    // font_mode() overrides the config (default Ahem); fidelity work pins Real.
    let a = DeterministicApp::new(16, 16);
    assert_eq!(a.config().font_mode, FontMode::Ahem, "default is Ahem");
    let b = a.font_mode(FontMode::Real);
    assert_eq!(b.config().font_mode, FontMode::Real);
}

#[test]
fn capture_msaa_is_pinned_off() {
    // A module constant, never a per-fixture knob — a 4× resolve antialiases
    // nondeterministically. A pure constant check; no build() needed.
    assert_eq!(
        CAPTURE_MSAA,
        bevy::render::view::Msaa::Off,
        "the capture path pins MSAA off for determinism"
    );
}

// --- GPU (#[ignore]): the knobs LAND on the BUILT app ----------------------
// build() instantiates `capture_app_scaled` (RenderPlugin → wgpu adapter), so
// these are NOT headless; they run on the GPU lane (CLAUDE.md GPU lane):
//     cargo test -p buiy_verify --test determinism_build -- --ignored

/// The built app's primary-window scale factor (the DPR pin's observable).
fn window_scale_factor(app: &mut App) -> f32 {
    app.world_mut()
        .query::<&Window>()
        .single(app.world())
        .expect("the built app carries a primary window")
        .resolution
        .scale_factor()
}

#[test]
#[ignore = "GPU: build() instantiates the capture render stack (needs a wgpu adapter)"]
fn build_applies_dpr_to_window() {
    // 2× DPR through the builder: the window carries scale_factor 2.0 (the
    // offscreen target is sized logical × dpr).
    let mut app = DeterministicApp::new(64, 64).dpr(Dpr::X2).build();
    assert_eq!(
        window_scale_factor(&mut app),
        2.0,
        "dpr(X2) pins the window scale_factor to 2.0×"
    );
}

#[test]
#[ignore = "GPU: build() instantiates the capture render stack (needs a wgpu adapter)"]
fn build_defaults_window_to_one_x() {
    let mut app = DeterministicApp::new(48, 48).build();
    assert_eq!(window_scale_factor(&mut app), 1.0);
}

#[test]
#[ignore = "GPU: build() instantiates the capture render stack (needs a wgpu adapter)"]
fn build_pins_the_virtual_clock() {
    // The fixed-clock knob: the built app drives time by a fixed ZERO virtual
    // delta, never wall time, so every frame sees the same instant and the
    // quiescence loop terminates deterministically.
    let app = DeterministicApp::new(32, 32).build();
    let strategy = app
        .world()
        .get_resource::<TimeUpdateStrategy>()
        .expect("DeterministicApp installs a manual TimeUpdateStrategy");
    assert!(
        matches!(strategy, TimeUpdateStrategy::ManualDuration(d) if d.is_zero()),
        "the clock advances by a fixed zero virtual delta (no wall-time read)"
    );
}
