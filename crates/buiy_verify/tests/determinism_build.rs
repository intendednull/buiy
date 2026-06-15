//! `DeterministicApp::build` knob-application tripwires (Phase 3.4,
//! verification-design `determinism.md` § "DeterministicApp builder").
//! HEADLESS — these inspect the built app's CPU-side configuration (window
//! scale factor, virtual-clock strategy, the pinned MSAA constant). The
//! pixel-level idempotent/knob-sensitivity proofs are GPU `#[ignore]` in
//! `determinism_capture.rs`.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use buiy_core::render::golden::CAPTURE_MSAA;
use buiy_verify::determinism::{DeterministicApp, Dpr, FontMode};

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
fn build_applies_dpr_and_msaa() {
    // 2× DPR through the builder: the window must carry scale_factor 2.0 (the
    // offscreen target is sized logical × dpr) …
    let mut app = DeterministicApp::new(64, 64).dpr(Dpr::X2).build();
    assert_eq!(
        window_scale_factor(&mut app),
        2.0,
        "dpr(X2) pins the window scale_factor to 2.0×"
    );

    // … and the capture MSAA is pinned single-sampled (a module constant, never
    // a per-fixture knob — a 4× resolve antialiases nondeterministically).
    assert_eq!(
        CAPTURE_MSAA,
        bevy::render::view::Msaa::Off,
        "the capture path pins MSAA off for determinism"
    );
}

#[test]
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

#[test]
fn default_dpr_is_one_x() {
    // Without an explicit dpr() the builder is 1× (the deterministic() default).
    let mut app = DeterministicApp::new(48, 48).build();
    assert_eq!(window_scale_factor(&mut app), 1.0);
}

#[test]
fn font_mode_override_flows_into_cfg() {
    // font_mode() overrides the config (default Ahem); fidelity work pins Real.
    let a = DeterministicApp::new(16, 16);
    assert_eq!(a.config().font_mode, FontMode::Ahem, "default is Ahem");
    let b = a.font_mode(FontMode::Real);
    assert_eq!(b.config().font_mode, FontMode::Real);
}
