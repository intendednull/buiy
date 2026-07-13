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
    TextureDimension, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::{AtlasFormat, BuiyAtlas};
use crate::render::pipeline::BuiyPipeline;

/// Render-world resource: the device-owning side of the atlas. Holds the
/// `CoverageR8` page texels as ONE array texture (a layer per page), a shared
/// `Sampler`, and the prebuilt `@group(1)` coverage bind group the glyph node
/// reads. Built/refreshed each prepare from [`BuiyAtlas`]'s dirty pages.
///
/// **Why one array texture, not a Vec of single-layer textures.** The GPU binds
/// all resident coverage pages at once as a `texture_2d_array` and the shader
/// samples the per-instance layer (`coverage.wgsl`), so a glyph/icon on any page
/// renders from its own texels. Design
/// `docs/specs/2026-07-09-multipage-coverage-atlas-bind-design.md`. The raster
/// (drawing-canvas) path is unaffected — it owns its own per-image textures +
/// bind groups in `raster.rs`.
#[derive(Resource)]
pub struct AtlasGpu {
    /// The `CoverageR8` array texture — one layer per resident page (`origin.z`
    /// = page index). `None` until the first page exists (a 0-layer array
    /// texture is invalid). Grow-only: recreated (never resized in place) when
    /// the page count exceeds [`Self::coverage_array_layers`] (§ 3.2).
    coverage_array: Option<Texture>,
    /// The `D2Array` view of [`Self::coverage_array`] bound at
    /// `@group(1) @binding(0)`. Rebuilt with the texture on a grow-recreate.
    coverage_array_view: Option<TextureView>,
    /// The array texture's layer count — grow-only "high-water". Invariant:
    /// `coverage_array_layers >= pages.len()` whenever a coverage page exists.
    /// A transient page-count shrink-then-regrow within the high-water needs no
    /// recreate (the re-grown page re-uploads into its already-allocated layer).
    coverage_array_layers: u32,
    /// Nearest-neighbour, clamp-to-edge sampler shared by every atlas page. A
    /// coverage atlas is sampled at integer-snapped texel rects, so linear
    /// filtering would bleed neighbouring cells across the guillotiere seam.
    sampler: Sampler,
    /// The `@group(1)` bind group for the **CoverageR8** array texture + the
    /// sampler — the glyph node binds this. `None` until a coverage page exists
    /// (no glyphs warmed yet). Rebuilt when the array texture is (re)created.
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
        // wgpu 28+: `mipmap_filter` is `MipmapFilterMode` (mag/min stay `FilterMode`).
        mipmap_filter: bevy::render::render_resource::MipmapFilterMode::Nearest,
        ..Default::default()
    })
}

impl FromWorld for AtlasGpu {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        Self {
            coverage_array: None,
            coverage_array_view: None,
            coverage_array_layers: 0,
            sampler: make_sampler(device),
            coverage_bind_group: None,
        }
    }
}

/// Upload one page's full CPU texels into its array LAYER via
/// `RenderQueue::write_texture` (design fork #1). The page buffer is the whole
/// `size × size` image, tightly packed, so `bytes_per_row = size * bpt` with no
/// 256-byte alignment dance (`write_texture` handles its own staging). The layer
/// is selected by `origin.z = layer`, copy extent depth 1. Coverage-only
/// (`CoverageR8`); the raster path uploads its own images in `raster.rs`.
fn upload_page_layer(queue: &RenderQueue, tex: &Texture, layer: u32, size: u32, pixels: &[u8]) {
    let bpt = AtlasFormat::CoverageR8.bytes_per_texel();
    queue.write_texture(
        TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
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

/// `RenderSystems::Prepare` system (design fork #1 + #3): package every resident
/// CoverageR8 page as a layer of ONE array texture (growing the array to a new
/// high-water when the page count exceeds it — § 3.2), upload page texels to
/// their layers via `write_texture`, then (re)build the `@group(1)` coverage
/// bind group into [`AtlasGpu`] so [`buiy_pass`] only reads. Clears the atlas
/// dirty flags after reading so an unchanged page does not re-upload (spec
/// § 2.2) — except on a grow-recreate frame, where EVERY resident page
/// re-uploads (a fresh array texture has no residual contents; the dirty-gated
/// loop would silently drop clean pages).
///
/// [`buiy_pass`]: crate::render::node::buiy_pass
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
        // page that existed and emptied keeps its array texture in `gpu`).
        _ => return,
    };
    let page_size = pages[0].size();
    let page_count = pages.len();

    // Grow-to-high-water (§ 3.2): a `texture_2d_array`'s layer count is fixed at
    // creation, so recreate the array whenever the page count exceeds the
    // current high-water (`array_layers >= pages.len()`). A transient
    // shrink-then-regrow within the high-water needs no recreate — the re-grown
    // page re-uploads into its already-allocated layer. Because the bind-group
    // *layout* is `D2Array` (a shape carrying no layer count), a recreate needs
    // no pipeline recompile, only a new bind group.
    let recreated = gpu.coverage_array.is_none() || page_count as u32 > gpu.coverage_array_layers;
    if recreated {
        let layers = page_count as u32;
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("buiy_coverage_atlas_array"),
            size: Extent3d {
                width: page_size,
                height: page_size,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            // Array layers are D2; the D2Array view (below) is what the shader
            // samples by layer.
            dimension: TextureDimension::D2,
            format: AtlasFormat::CoverageR8.texture_format(),
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor {
            label: Some("buiy_coverage_atlas_array_view"),
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        });
        gpu.coverage_array = Some(texture);
        gpu.coverage_array_view = Some(view);
        gpu.coverage_array_layers = layers;
    }

    // Which pages to upload: on a recreate frame ALL of them (the fresh array
    // texture has no residual contents — the dirty-gated loop would silently
    // drop clean pages, blanking a page-0 node across a later growth), otherwise
    // only the dirty ones (spec § 2.2). Collect first so the `pages` borrow ends
    // before the per-page pixel re-borrow + the mutable `atlas`/`gpu` writes.
    let uploads: Vec<(usize, u32)> = pages
        .iter()
        .enumerate()
        .filter(|(_, p)| recreated || p.is_dirty())
        .map(|(i, p)| (i, p.size()))
        .collect();

    let texture = gpu
        .coverage_array
        .as_ref()
        .expect("coverage array texture created above");
    for (idx, size) in &uploads {
        // Re-borrow per page to keep the borrow scoped.
        let pixels = atlas
            .page_pixels(AtlasFormat::CoverageR8, *idx)
            .expect("upload page index is in range");
        upload_page_layer(&queue, texture, *idx as u32, *size, pixels);
    }
    atlas.clear_all_dirty();

    // (Re)build the `@group(1)` coverage bind group against the D2Array view +
    // the shared sampler when it is missing or the array was recreated (new
    // view). The layout is the concrete `BuiyPipeline.coverage_atlas_layout` —
    // the SAME layout the glyph pipeline descriptor declares (one source of
    // truth, so the bind group is layout-compatible with the pipeline).
    if gpu.coverage_bind_group.is_none() || recreated {
        let view = gpu
            .coverage_array_view
            .as_ref()
            .expect("coverage array view created above");
        let bg = device.create_bind_group(
            "buiy_atlas_coverage_bind_group",
            &pipeline.coverage_atlas_layout,
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
