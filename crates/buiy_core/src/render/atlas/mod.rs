//! Buiy's shared texture atlas: the warehouse all coverage-and-image
//! primitives (glyph / icon / gradient / mask) sample. One render-world
//! `BuiyAtlas` resource; allocation via `guillotiere`, content-addressed
//! entries, LRU eviction, page-budget pressure, warmup, and pooling.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md.
//! This spec owns the atlas + the glyph-alpha / icon primitive *shapes*;
//! `buiy-text-rendering-design` owns shaping and produces coverage bitmaps,
//! plugging in through the § 3 `get_or_insert` API only.
//!
//! ## The seam (spec § 1)
//! - **This module owns:** the shared atlas (allocation, warmup, eviction,
//!   pooling), the glyph-alpha + icon primitive *shapes*, and the reserved
//!   gradient/mask entry kinds.
//! - **`buiy-text-rendering-design` owns:** glyph shaping, line layout, font
//!   fallback, BiDi, coverage-bitmap rasterization, and *emitting* primitives.
//!   It plugs in through `get_or_insert`/`AtlasEntry` (inbound) and
//!   `GlyphAlphaInstance`/`IconInstance` (outbound) only — no cosmic-text type
//!   ever crosses into this module.

/// The two backing-texture formats. Glyph/mask coverage is single-channel
/// `R8`; icon/sprite and baked gradient stops are full-color `Rgba8`. A
/// `guillotiere` page is one format — the two never share a page
/// (spec § 2.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AtlasFormat {
    /// `TextureFormat::R8Unorm` — alpha-as-color coverage (spec § 2.2, § 4.1).
    CoverageR8,
    /// `TextureFormat::Rgba8UnormSrgb` — full color; hardware sRGB→linear
    /// decode on sample keeps the all-linear shading invariant (spec § 2.2).
    ColorRgba8,
}

impl AtlasFormat {
    /// Bytes per texel for this format: `CoverageR8` is single-channel `R8`
    /// (1 byte), `ColorRgba8` is 4-channel `Rgba8` (4 bytes). Drives the blit
    /// row-stride math and the `write_texture` `bytes_per_row` (spec § 2.2).
    pub fn bytes_per_texel(self) -> u32 {
        match self {
            AtlasFormat::CoverageR8 => 1,
            AtlasFormat::ColorRgba8 => 4,
        }
    }

    /// The wgpu `TextureFormat` this atlas format maps to: `R8Unorm` for
    /// coverage (stores no color — modulates the per-instance linear tint),
    /// `Rgba8UnormSrgb` for color (hardware sRGB→linear decode on sample keeps
    /// the all-linear shading invariant). Spec § 2.2.
    pub fn texture_format(self) -> bevy::render::render_resource::TextureFormat {
        use bevy::render::render_resource::TextureFormat;
        match self {
            AtlasFormat::CoverageR8 => TextureFormat::R8Unorm,
            AtlasFormat::ColorRgba8 => TextureFormat::Rgba8UnormSrgb,
        }
    }
}

mod types;
pub use types::{AtlasBitmap, AtlasConfig, AtlasEntry, AtlasEntryKind, AtlasKey};

mod page;
pub use page::AtlasPage;

mod lru;
pub use lru::LruQueue;

// The resource lives in `atlas.rs` so the file name matches the type it owns
// (`BuiyAtlas`); the spec pins both names, so the self-named submodule is
// intentional.
#[allow(clippy::module_inception)]
mod atlas;
pub use atlas::BuiyAtlas;

mod primitive;
pub use primitive::{
    GLYPH_ALPHA_FLOAT_OFFSET, GLYPH_ALPHA_INSTANCE_STRIDE_BYTES, GLYPH_IDENTITY_AFFINE,
    GlyphAlphaInstance, IconInstance,
};

mod warmup;
pub use warmup::{AtlasWarmupQueue, AtlasWarmupRequest};

mod gpu;
pub use gpu::{AtlasGpu, maintain_atlas, prepare_atlas_textures};

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, Render, RenderSystems};

/// Insert the shared atlas resources into the render world and schedule the
/// pre-paint warmup drain, the dirty-page GPU upload, and per-frame maintenance.
/// Called from `BuiyRenderPlugin::build` inside the `RenderApp` branch. Spec
/// § 2.1 (one resource per `RenderApp`), § 2.3.
///
/// `AtlasGpu` (the device-owning half) is NOT inserted here — it is `FromWorld`
/// on the `RenderDevice`, which `RenderPlugin` only materializes in its `finish`,
/// so [`register_gpu`] inits it at finish (mirroring `pipeline::register`).
/// Scheduling the systems is device-free and stays in `build`.
pub(crate) fn register(render_app: &mut SubApp) {
    render_app
        .insert_resource(BuiyAtlas::new(AtlasConfig::default()))
        .init_resource::<AtlasWarmupQueue>()
        // Pre-paint warmup drain, THEN per-frame maintenance (begin_frame +
        // grace drain + page pooling) — both in ExtractSchedule. Maintenance
        // runs AFTER warmup so a just-warmed entry is not immediately drained,
        // and its begin_frame advances the LRU clock the next warmup touches.
        .add_systems(ExtractSchedule, (warmup_atlas, maintain_atlas).chain())
        // The dirty-page GPU upload + `@group(1)` bind-group build, in Prepare
        // (alongside `prepare_buiy_instances`). Reads the atlas's dirty pages,
        // uploads via `write_texture`, stashes the bind group on `AtlasGpu`.
        .add_systems(
            Render,
            prepare_atlas_textures.in_set(RenderSystems::Prepare),
        );
}

/// Device-dependent atlas setup: insert the `AtlasGpu` render resource (its
/// `FromWorld` needs the `RenderDevice`). Called from `BuiyRenderPlugin::finish`
/// after `pipeline::register`, so `BuiyPipeline.atlas_layout` exists for the
/// first `prepare_atlas_textures` run.
pub(crate) fn register_gpu(render_app: &mut SubApp) {
    render_app.init_resource::<AtlasGpu>();
}

/// Pre-paint warmup drain (spec § 2.3): force every queued residency request
/// resident before the first paint, so golden frames never race a cold atlas
/// (gate #2). Producers (text/icon owners) push to the queue; this drains it.
fn warmup_atlas(mut atlas: ResMut<BuiyAtlas>, mut queue: ResMut<AtlasWarmupQueue>) {
    if queue.is_empty() {
        return;
    }
    atlas.drain_warmup(&mut queue);
}
