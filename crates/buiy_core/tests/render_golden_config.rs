//! `GoldenConfig` default-config tripwires (Phase 3.1, verification-design
//! `determinism.md` § "Extending GoldenConfig"). Pure-CPU, headless — no
//! adapter, no `#[ignore]`. Pins that `deterministic()` collapses the font
//! axis to the Ahem box-font + a 1× DPR, while `fidelity()` is the narrow
//! real-glyph variant with every other knob still pinned.

use buiy_core::render::golden::{Dpr, FontMode, GoldenConfig};

#[test]
fn deterministic_defaults_collapse_font_axis() {
    let cfg = GoldenConfig::deterministic();
    // The bulk of text-bearing goldens test boxes, not glyphs: default to the
    // Ahem em-box font so they are byte-identical across hosts.
    assert_eq!(cfg.font_mode, FontMode::Ahem);
    // 1× is the headless capture default; 2× is an explicit fixture axis.
    assert_eq!(cfg.dpr, Dpr::X1);
    // The landed flake triad stays pinned.
    assert!(cfg.fixed_clock);
    assert!(cfg.wait_for_fonts);
    assert!(cfg.warm_atlas);
    assert!(!cfg.accept);
}

#[test]
fn fidelity_uses_real_font() {
    let cfg = GoldenConfig::fidelity();
    // The narrow real-glyph fidelity suite: Ahem off …
    assert_eq!(cfg.font_mode, FontMode::Real);
    // … but every other determinism knob is still pinned (it differs from
    // `deterministic()` in exactly the font axis).
    assert_eq!(cfg.dpr, Dpr::X1);
    assert!(cfg.fixed_clock);
    assert!(cfg.wait_for_fonts);
    assert!(cfg.warm_atlas);
    assert!(!cfg.accept);
}

#[test]
fn config_is_copy() {
    // `GoldenConfig` must stay `Copy` (every field is `Copy`) so the capture
    // path can pass it by value without ceremony.
    let cfg = GoldenConfig::deterministic();
    let a = cfg;
    let b = cfg;
    assert_eq!(a.font_mode, b.font_mode);
}
