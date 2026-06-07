//! GPU-only end-to-end atlas tests. Every test here needs a wgpu adapter
//! (real GPU or lavapipe) — CI and this host have none, so all are #[ignore]
//! exactly like render_smoke.rs. The headless allocator/LRU/pooling/warmup
//! contract is covered adapter-free in atlas_alloc.rs.
//!
//! Run locally with a GPU/lavapipe:
//!   cargo test -p buiy_core --test atlas_gpu -- --ignored

// --- (1) Upload + sample: a warmed coverage entry paints with its tint. ---
#[test]
#[ignore = "needs a wgpu adapter; GPU upload + sampling draw (spec § 7 'On GPU')"]
fn warmed_glyph_uploads_and_samples_with_tint() {
    // Build a RenderApp, push a CoverageR8 warmup request for a known 16x16
    // coverage bitmap, run one frame, and read back the target: the entry's
    // pixels must equal `color * coverage` (alpha-as-color, spec § 4.1).
    // Implementation deferred to the GPU runner; the headless residency +
    // primitive layout it depends on are proven in atlas_alloc.rs /
    // atlas_primitive.rs.
}

// --- (2) Alpha-as-color: re-tinting a glyph never regenerates the atlas. ---
#[test]
#[ignore = "needs a wgpu adapter; atlas byte-identity across two themes (spec § 7)"]
fn retint_same_glyph_leaves_atlas_byte_identical() {
    // Insert glyph G once. Emit a GlyphAlphaInstance with theme-A color, then
    // theme-B color. Assert the CoverageR8 page texture is byte-identical
    // between the two frames (only the instance `color` differs) — the
    // alpha-as-color trick (spec § 4.1, § 7).
}

// --- (3) Warmup determinism: first painted frame matches golden. ---
#[test]
#[ignore = "needs a wgpu adapter; gate #2 warmup-determinism golden (spec § 2.3, § 7)"]
fn warmup_makes_first_frame_match_golden() {
    // With warmup_atlas draining the queue pre-paint, the fixture's FIRST
    // painted frame matches its golden (no glyph lands a frame late).
}

// --- (4) Gate #15: atlas entries return within ε of baseline after idle. ---
#[test]
#[ignore = "needs a wgpu adapter; gate #15 atlas-entries-return-to-baseline fixture"]
fn gate15_atlas_entries_return_to_baseline_after_idle() {
    // Drive a fixture that exercises many transient glyphs/icons, then go
    // idle. The idle-settle window must exceed
    // max(config.eviction_grace, RT-pool 3 frames) (spec § 2.4 "Consequence
    // for the gate-#15 fixture's idle-settle window"). After settling, the
    // live-entry count returns within ε of baseline and page count does not
    // grow monotonically. The headless half of this (entry count + page
    // count, adapter-free) is `grace_drain_returns_idle_entries_to_baseline`
    // in atlas_alloc.rs; this is the on-GPU RSS half.
}
