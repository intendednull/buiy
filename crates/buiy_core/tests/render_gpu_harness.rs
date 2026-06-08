//! Keystone GPU harness smoke. Proves [`support::gpu_test_app`] drives a full
//! Buiy render frame on a real wgpu adapter and that `BuiyPipeline` materializes
//! in `BuiyRenderPlugin::finish`. Every other GPU `#[ignore]` test builds on this
//! same harness (campaign plan `docs/plans/2026-06-07-render-gpu-verify-campaign.md`).
//!
//! Run: `cargo test -p buiy_core --test render_gpu_harness -- --ignored --nocapture`.

mod support;

use bevy::render::RenderApp;
use buiy_core::render::pipeline::BuiyPipeline;

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn canonical_harness_drives_a_full_frame() {
    let mut app = support::gpu_test_app();
    // Drive two frames: finish materializes the device + pipeline; the second
    // update proves the render schedule runs without the "Message not
    // initialized" param-validation panic the minimal probe set hit.
    support::finish_and_run(&mut app, 2);

    let render_app = app
        .get_sub_app(RenderApp)
        .expect("RenderApp exists after finish");
    assert!(
        render_app.world().get_resource::<BuiyPipeline>().is_some(),
        "BuiyPipeline registers during BuiyRenderPlugin::finish",
    );
}
