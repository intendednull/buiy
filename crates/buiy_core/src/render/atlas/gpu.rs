//! The render-world, device-owning half of the atlas: the per-(format, page)
//! GPU `Texture`s, the `@group(1)` coverage bind group the glyph node binds, and
//! the prepare/maintenance systems that drive them. Spec atlas-and-text-seam.md
//! § 2.2 (dirty-gated upload), § 2.5 (pooling), and the GPU-verify design note
//! (fork #1 `write_texture`, fork #3 bind group built in prepare).
//!
//! `BuiyAtlas` stays device-free (the headless allocator tests need no adapter);
//! this resource owns everything that needs the `RenderDevice`/`RenderQueue`. It
//! is filled by [`prepare_atlas_textures`] in `RenderSystems::Prepare`, exactly
//! the effect-group-target precedent (`node.rs`): acquisition needs `&mut` +
//! `RenderDevice`, which `BuiyNode::run(&World)` cannot get, so it lives in
//! prepare and the node only reads.

use bevy::prelude::*;
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, Extent3d, Origin3d, Sampler, SamplerDescriptor,
    TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor,
    TextureDimension, TextureUsages, TextureView, TextureViewDescriptor,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::{AtlasFormat, BuiyAtlas};
use crate::render::pipeline::BuiyPipeline;

/// One page's GPU resources: the `Texture`, its default `TextureView`, and the
/// page-index it mirrors. Persists across `BuiyAtlas` page-pool recycling — the
/// expensive GPU object is reused at the same page index, never realloc'd (spec
/// § 2.5).
struct PageTexture {
    texture: Texture,
    view: TextureView,
}

/// Render-world resource: the device-owning side of the atlas. Holds the
/// per-format page `Texture`s, a shared `Sampler`, and the prebuilt `@group(1)`
/// coverage bind group the glyph node reads. Built/refreshed each prepare from
/// [`BuiyAtlas`]'s dirty pages.
#[derive(Resource)]
pub struct AtlasGpu {
    /// `CoverageR8` page textures, indexed by page (parallel to
    /// `BuiyAtlas.pages[CoverageR8]`). Recycled in place under pooling.
    coverage_pages: Vec<PageTexture>,
    /// Nearest-neighbour, clamp-to-edge sampler shared by every atlas page. A
    /// coverage atlas is sampled at integer-snapped texel rects, so linear
    /// filtering would bleed neighbouring cells across the guillotiere seam.
    sampler: Sampler,
    /// The `@group(1)` bind group for the **CoverageR8** page-0 texture + the
    /// sampler — the glyph node binds this. `None` until a coverage page exists
    /// (no glyphs warmed yet). Rebuilt when page 0's texture is (re)created.
    coverage_bind_group: Option<BindGroup>,
}

impl AtlasGpu {
    /// The prebuilt coverage `@group(1)` bind group, if a coverage page exists.
    pub fn coverage_bind_group(&self) -> Option<&BindGroup> {
        self.coverage_bind_group.as_ref()
    }
}

/// Create the one shared atlas sampler (nearest / clamp). Pulled out so both
/// the initial `FromWorld` and any later device-loss rebuild use one definition.
fn make_sampler(device: &RenderDevice) -> Sampler {
    device.create_sampler(&SamplerDescriptor {
        label: Some("buiy_atlas_sampler"),
        address_mode_u: bevy::render::render_resource::AddressMode::ClampToEdge,
        address_mode_v: bevy::render::render_resource::AddressMode::ClampToEdge,
        address_mode_w: bevy::render::render_resource::AddressMode::ClampToEdge,
        mag_filter: bevy::render::render_resource::FilterMode::Nearest,
        min_filter: bevy::render::render_resource::FilterMode::Nearest,
        mipmap_filter: bevy::render::render_resource::FilterMode::Nearest,
        ..Default::default()
    })
}

impl FromWorld for AtlasGpu {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        Self {
            coverage_pages: Vec::new(),
            sampler: make_sampler(device),
            coverage_bind_group: None,
        }
    }
}

/// Create a fresh atlas page `Texture` of `size × size` in `format`, with
/// `TEXTURE_BINDING | COPY_DST` (sampled by the shader, written by
/// `write_texture`). One mip, one layer, 2D.
fn create_page_texture(device: &RenderDevice, size: u32, format: AtlasFormat) -> PageTexture {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("buiy_atlas_page"),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: format.texture_format(),
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    PageTexture { texture, view }
}

/// Upload one page's full CPU texels into its GPU `Texture` via
/// `RenderQueue::write_texture` (design fork #1). The page buffer is the whole
/// `size × size` image, tightly packed, so `bytes_per_row = size * bpt` with no
/// 256-byte alignment dance (`write_texture` handles its own staging).
fn upload_page(queue: &RenderQueue, tex: &Texture, size: u32, format: AtlasFormat, pixels: &[u8]) {
    let bpt = format.bytes_per_texel();
    queue.write_texture(
        TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        pixels,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size * bpt),
            rows_per_image: Some(size),
        },
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
}

/// `RenderSystems::Prepare` system (design fork #1 + #3): for every dirty
/// CoverageR8 page, ensure a GPU `Texture` exists (creating one when the page
/// set grew — pooled recycles reuse the slot, spec § 2.5), upload its texels via
/// `write_texture`, then (re)build the `@group(1)` coverage bind group into
/// [`AtlasGpu`] so [`BuiyNode::run`] only reads. Clears the atlas dirty flags
/// after reading so an unchanged page does not re-upload (spec § 2.2).
///
/// [`BuiyNode::run`]: crate::render::node::BuiyNode
pub fn prepare_atlas_textures(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    pipeline: Res<BuiyPipeline>,
    mut atlas: ResMut<BuiyAtlas>,
    mut gpu: ResMut<AtlasGpu>,
) {
    let pages = match atlas.pages_of(AtlasFormat::CoverageR8) {
        Some(p) if !p.is_empty() => p,
        // No coverage pages: nothing to upload. Leave the bind group as-is (a
        // page that existed and emptied keeps its texture pooled in `gpu`).
        _ => return,
    };

    // Grow the GPU page set to match the CPU page set. New textures are created
    // for appended pages; existing slots (including pooled-and-reused pages) keep
    // their `Texture`, so pooling reuses the GPU object (spec § 2.5).
    let mut bind_group_stale = false;
    if gpu.coverage_pages.len() < pages.len() {
        let size = pages[0].size();
        for _ in gpu.coverage_pages.len()..pages.len() {
            let pt = create_page_texture(&device, size, AtlasFormat::CoverageR8);
            gpu.coverage_pages.push(pt);
        }
        bind_group_stale = true; // page-0 texture may be freshly created.
    }

    // Upload every dirty page (spec § 2.2). Collect indices first so the `pages`
    // borrow ends before the mutable `gpu`/`atlas` writes below.
    let dirty: Vec<(usize, u32)> = pages
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_dirty())
        .map(|(i, p)| (i, p.size()))
        .collect();
    for (idx, size) in &dirty {
        // Re-borrow per page to keep the borrow scoped.
        let pixels = atlas
            .page_pixels(AtlasFormat::CoverageR8, *idx)
            .expect("dirty page index is in range");
        upload_page(
            &queue,
            &gpu.coverage_pages[*idx].texture,
            *size,
            AtlasFormat::CoverageR8,
            pixels,
        );
    }
    atlas.clear_all_dirty();

    // (Re)build the `@group(1)` coverage bind group against page-0's texture +
    // the shared sampler when it is missing or page 0's texture changed. The
    // layout is the concrete `BuiyPipeline.atlas_layout` — the SAME layout the
    // coverage pipeline descriptor declares (one source of truth, so the bind
    // group is layout-compatible with the pipeline).
    if gpu.coverage_bind_group.is_none() || bind_group_stale {
        let view = &gpu.coverage_pages[0].view;
        let bg = device.create_bind_group(
            "buiy_atlas_coverage_bind_group",
            &pipeline.atlas_layout,
            &BindGroupEntries::sequential((view, &gpu.sampler)),
        );
        gpu.coverage_bind_group = Some(bg);
    }
}

/// Per-frame atlas maintenance (gate #15): advance the atlas frame counter,
/// drain grace-expired entries, and pool emptied pages. Runs in `ExtractSchedule`
/// after the warmup drain so an idle fixture's transient entries leave the atlas
/// within the grace window and their pages return to the pool (spec § 2.4 step 3,
/// § 2.5). Kept separate from the GPU upload so the leak logic stays device-free.
pub fn maintain_atlas(mut atlas: ResMut<BuiyAtlas>) {
    atlas.begin_frame();
    atlas.drain_grace_expired();
    atlas.collect_emptied_pages();
}
