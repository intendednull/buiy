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
/// does). Idempotent: re-registering the same family is a no-op rebuild.
pub fn register_ahem(app: &mut App) {
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes(AHEM_FAMILY, ahem_bytes(), FontFaceDescriptors::default());
    app.update();
}
