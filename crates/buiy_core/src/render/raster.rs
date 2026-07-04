//! The raster (textured-quad) node primitive — the CPU-authored-image seam.
//!
//! A [`RasterImage`] component makes a Buiy layout node sample a bevy [`Image`]
//! onto its resolved rect (a drawing canvas, an avatar bitmap). The app owns and
//! paints the `Image` (typically `RenderAssetUsages::all()` so it keeps the CPU
//! `data`); this primitive only places + samples it.
//!
//! **Wave F2 note (pending F1).** This file carries ONLY the [`RasterImage`]
//! component so the `buiy_view` `raster()` element (wave F2) can build, place +
//! size a raster node, and patch its handle by identity — the whole render side
//! (the distinct `BuiyRasterPipeline`, the `RasterInstance` record, the
//! extract→prepare→draw glue, and the `register_type` wiring) lands in **wave
//! F1**, which owns this file. The component's public shape is fixed here to
//! match F1's exactly (`RasterImage(pub Handle<Image>)`), so the F1→F2 rebase is
//! a clean "take F1's superset of `render/raster.rs`" — the `buiy_view`
//! reconciler's `use buiy_core::render::RasterImage` stays valid across it.

use bevy::prelude::*;

/// A node that paints a textured quad sampling a bevy [`Image`], sized by its
/// resolved layout rect. The app owns the image (typically
/// `RenderAssetUsages::all()` so it can keep painting into the CPU `data`); the
/// render pipeline (wave F1) samples whatever `GpuImage` the handle currently
/// resolves to with a **Nearest** sampler (crisp for pixel drawing).
///
/// Author it on a layout node (carries [`Node`](crate::components::Node)) so the
/// F1 extract has a `ResolvedLayout` + `GlobalTransform` to size/place the quad.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct RasterImage(pub Handle<Image>);
