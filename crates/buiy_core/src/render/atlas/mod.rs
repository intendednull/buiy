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
pub use primitive::{GlyphAlphaInstance, IconInstance};

mod warmup;
pub use warmup::{AtlasWarmupQueue, AtlasWarmupRequest};

use bevy::prelude::*;
use bevy::render::ExtractSchedule;

/// Insert the shared atlas resources into the render world and schedule the
/// pre-paint warmup drain. Called from `BuiyRenderPlugin::build` inside the
/// `RenderApp` branch. Spec § 2.1 (one resource per `RenderApp`), § 2.3.
pub(crate) fn register(render_app: &mut SubApp) {
    render_app
        .insert_resource(BuiyAtlas::new(AtlasConfig::default()))
        .init_resource::<AtlasWarmupQueue>()
        .add_systems(ExtractSchedule, warmup_atlas);
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
