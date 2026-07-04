//! Textured-quad RASTER primitive — the drawing-canvas seam (Dooduel F1).
//!
//! `buiy_core`'s F-tier fill primitives are solid + gradient only; nothing
//! samples an arbitrary raster texture onto a node. This module adds the
//! smallest such primitive: a [`RasterImage`] component makes a Buiy node draw a
//! textured quad sampling a bevy [`Image`], sized by the node's resolved layout
//! rect.
//!
//! **A distinct pipeline, NOT a new [`BuiyPrimitiveKind`].** This follows the
//! band/gradient precedent (`render::primitive`): a dedicated
//! [`BuiyRasterPipeline`] keyed by record type, with its own `raster.wgsl`
//! shader + [`RasterInstance`] vertex layout, painted in the fill paint slot.
//! The closed `BuiyPrimitiveKind` enum (and the byte-stable quad path) stays
//! untouched. The reserved `Path` slot is for a future vector-path SDF shader,
//! not a sampled raster — a raster is the wrong data model for it.
//!
//! [`BuiyPrimitiveKind`]: crate::render::buckets::BuiyPrimitiveKind
//!
//! **Per-node texture.** Unlike glyphs (one shared coverage atlas bound once),
//! each `RasterImage` samples its OWN texture, so the draw is one bind group +
//! one `draw` per raster node (`@group(1)` = the image's texture + a Nearest
//! sampler). Cheap: an app has a handful of raster nodes (a game canvas, an
//! avatar editor), not thousands.
//!
//! **CPU-authoritative source.** The app owns the [`Image`] asset with
//! `RenderAssetUsages::all()` (`MAIN_WORLD | RENDER_WORLD`), paints into its CPU
//! `data`, and marks it changed; bevy re-extracts a clone and re-uploads the
//! `GpuImage` — so flood-fill / undo / serialization all live on the CPU buffer.
//! This module is the framework consumer: it only samples whatever `GpuImage`
//! the handle currently resolves to.
//!
//! **The `RENDER_WORLD`-only `data.take()` trap.** An `Image` authored with only
//! `RenderAssetUsages::RENDER_WORLD` has its `data` *taken* (moved out) when the
//! render asset is prepared, so the main-world buffer the app keeps painting into
//! goes empty after the first upload. Author the canvas image with
//! `RenderAssetUsages::all()` so the main-world `data` survives the render-world
//! clone and the app retains its CPU-authoritative buffer.

use bevy::asset::uuid::Uuid;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    AddressMode, BindGroup, BindGroupEntries, BlendState, BufferUsages, ColorTargetState,
    ColorWrites, FilterMode, FragmentState, FrontFace, MipmapFilterMode, MultisampleState,
    PolygonMode, PrimitiveState, PrimitiveTopology, RawBufferVec, RenderPipelineDescriptor,
    Sampler, SamplerDescriptor, SpecializedRenderPipeline, TextureFormat, VertexAttribute,
    VertexFormat, VertexState, VertexStepMode,
};
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use bytemuck::{Pod, Zeroable};
use core::marker::PhantomData;

use crate::components::{Node, ResolvedLayout};
use crate::render::components::ClipRect;
use crate::render::pipeline::{
    BuiyPipeline, atlas_layout_descriptor, view_uniform_layout_descriptor,
};

/// A node that paints a textured quad sampling a bevy [`Image`], sized by its
/// resolved layout rect. The app owns the image (typically
/// `RenderAssetUsages::all()` so it can keep painting into the CPU `data`); this
/// primitive samples whatever `GpuImage` the handle currently resolves to with a
/// **Nearest** sampler (crisp for pixel drawing).
///
/// Author it on a layout node (which carries [`Node`]) so extract has a
/// `ResolvedLayout` + `GlobalTransform` to size/place the quad. The whole image
/// is stretched to the node rect (implicit `0..1` uv from the unit quad).
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct RasterImage(pub Handle<Image>);

/// Stable UUID for the raster (textured-quad) shader. Follows the render-asset
/// UUID convention in `pipeline.rs` (`0xB01A_01NN_..._000N`, `NN == N`) — octet
/// `..09` (the backdrop-blur shader took `..08`).
const RASTER_SHADER_UUID: Uuid = Uuid::from_u128(0xB01A_0109_0000_0000_0000_0000_0000_0009u128);

/// Weak handle to the raster WGSL shader (octet `..09`). Backed by `raster.wgsl`,
/// loaded into the MAIN world by `BuiyRenderPlugin::build` (`load_internal_asset!`);
/// [`BuiyRasterPipeline::specialize`] resolves it through the `PipelineCache`
/// mirror.
pub fn raster_shader_handle() -> Handle<Shader> {
    Handle::Uuid(RASTER_SHADER_UUID, PhantomData)
}

/// Full-view clip sentinel for an unclipped instance (mirrors the private
/// constants in `render::instance`): any finite fragment position is inside
/// `[-inf, +inf]`, so the fragment discard never fires.
const CLIP_SENTINEL_MIN: [f32; 2] = [f32::NEG_INFINITY, f32::NEG_INFINITY];
const CLIP_SENTINEL_MAX: [f32; 2] = [f32::INFINITY, f32::INFINITY];

/// Stride of [`RasterInstance`] in bytes (12 × f32 = 48 B). Must equal the
/// per-instance `array_stride` the raster pipeline declares.
pub const RASTER_INSTANCE_STRIDE_BYTES: usize = std::mem::size_of::<RasterInstance>();

/// One raster-quad instance in LOGICAL-pixel units (the view-uniform handoff, the
/// quad-family convention). No color/radius — the fragment IS the sampled texel.
/// The uv is implicit `0..1` from the unit-quad VBO, so a `RasterImage` always
/// samples the whole image stretched to the node rect.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct RasterInstance {
    /// Top-left in logical px (window-relative, y-down) — `GlobalTransform` xy.
    pub rect_pos: [f32; 2],
    /// Width / height in logical px (positive; the y-flip lives in the view
    /// uniform) — `ResolvedLayout.size`.
    pub rect_size: [f32; 2],
    /// Clip AABB min in logical px; `[-inf; 2]` = the full-view sentinel.
    pub clip_min: [f32; 2],
    /// Clip AABB max in logical px; `[+inf; 2]` = the full-view sentinel.
    pub clip_max: [f32; 2],
    /// The 2D affine basis `[m00, m10, m01, m11]` (folded to ONE `Float32x4`
    /// vertex attribute, the band's WebGL2-thrifty precedent). Identity
    /// `[1, 0, 0, 1]` paints axis-aligned.
    pub affine: [f32; 4],
}

/// `true` iff [`RASTER_INSTANCE_STRIDE_BYTES`] equals the actual struct size and
/// the 48 B the pipeline declares (the parallel of `gradient_stride_agrees`).
pub fn raster_stride_agrees() -> bool {
    RASTER_INSTANCE_STRIDE_BYTES == std::mem::size_of::<RasterInstance>()
        && RASTER_INSTANCE_STRIDE_BYTES == 12 * std::mem::size_of::<f32>()
}

/// Render-world list of raster quads to draw this frame, rebuilt every extract
/// (an app has a handful of raster nodes, so no damage-gating in v1). `images[i]`
/// is the texture the instance `instances[i]` samples — a parallel vec so the
/// draw can look each up in `RenderAssets<GpuImage>` per frame.
#[derive(Resource, Default)]
pub struct ExtractedRasters {
    /// One instance per `RasterImage` node, in query order.
    pub instances: Vec<RasterInstance>,
    /// The image each instance samples (parallel to `instances`).
    pub images: Vec<AssetId<Image>>,
}

/// Persistent per-view raster instance buffer (grow-in-place, the
/// `BuiyInstanceBuffers` idiom) + the parallel image ids the draw resolves.
#[derive(Resource)]
pub struct RasterBuffers {
    /// Raster-quad instances (grows in place).
    pub instances: RawBufferVec<RasterInstance>,
    /// The image each instance samples (parallel to the uploaded buffer).
    pub images: Vec<AssetId<Image>>,
    /// Instance count written this frame (the draw range upper bound).
    pub count: u32,
}

impl Default for RasterBuffers {
    fn default() -> Self {
        Self {
            instances: RawBufferVec::new(BufferUsages::VERTEX),
            images: Vec::new(),
            count: 0,
        }
    }
}

/// The device-owning raster half (`finish`-time): the Nearest sampler every
/// raster `@group(1)` bind group uses. The `@group(1)` layout itself is reused
/// from [`BuiyPipeline::atlas_layout`] (same `texture_2d<f32>` + sampler shape),
/// so this resource only owns the sampler.
#[derive(Resource)]
pub struct RasterGpu {
    /// Nearest, clamp-to-edge — crisp pixel drawing with no bleed at the rim.
    pub sampler: Sampler,
}

impl FromWorld for RasterGpu {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("buiy_raster_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: MipmapFilterMode::Nearest,
            ..Default::default()
        });
        Self { sampler }
    }
}

/// One `SpecializedRenderPipeline` variant of the raster pipeline: built for a
/// specific target color-attachment format + sample count (the `BuiyBandKey`
/// precedent). A distinct pipeline keyed by record — NOT a `BuiyPrimitiveKind`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BuiyRasterKey {
    /// The bound attachment's format (view format / `Rgba16Float` group target).
    pub format: TextureFormat,
    /// The bound attachment's sample count (the per-view `Msaa` count).
    pub samples: u32,
}

/// The raster (textured-quad) `SpecializedRenderPipeline`. Its own vertex layout
/// ([`RasterInstance`]) + shader (`raster.wgsl`), sharing the quad family's
/// `@group(0)` view uniform AND declaring the atlas-shaped `@group(1)`
/// (`texture_2d<f32>` + sampler) for the per-node image.
#[derive(Default)]
pub struct BuiyRasterPipeline;

impl BuiyRasterPipeline {
    /// The two interleaved vertex-buffer layouts: the shared static unit quad
    /// (stride 16, VBO 0) and the per-instance [`RasterInstance`] record
    /// (VBO 1, `step_mode: Instance`). Offsets/formats MUST match
    /// `RasterInstance`'s `#[repr(C)]` fields and `raster.wgsl`'s `@location`s:
    ///
    /// | field             | offset | format    | `@location` |
    /// |-------------------|--------|-----------|-------------|
    /// | (vertex) position | 0      | Float32x2 | 0           |
    /// | (vertex) uv       | 8      | Float32x2 | 1           |
    /// | rect_pos          | 0      | Float32x2 | 2           |
    /// | rect_size         | 8      | Float32x2 | 3           |
    /// | clip_min          | 16     | Float32x2 | 4           |
    /// | clip_max          | 24     | Float32x2 | 5           |
    /// | affine            | 32     | Float32x4 | 6           |
    ///
    /// Total instance stride 48 B = [`RASTER_INSTANCE_STRIDE_BYTES`] (7 vertex
    /// attributes total — well under the WebGL2 16-attribute cap).
    fn raster_vertex_buffers() -> Vec<VertexBufferLayout> {
        vec![
            VertexBufferLayout {
                array_stride: 16,
                step_mode: VertexStepMode::Vertex,
                attributes: vec![
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 1,
                    },
                ],
            },
            VertexBufferLayout {
                array_stride: RASTER_INSTANCE_STRIDE_BYTES as u64,
                step_mode: VertexStepMode::Instance,
                attributes: vec![
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 2,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 3,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 16,
                        shader_location: 4,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 24,
                        shader_location: 5,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 32,
                        shader_location: 6,
                    },
                ],
            },
        ]
    }
}

impl SpecializedRenderPipeline for BuiyRasterPipeline {
    type Key = BuiyRasterKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let shader = raster_shader_handle();
        RenderPipelineDescriptor {
            label: Some("buiy_raster_pipeline".into()),
            // `@group(0)` view uniform (shared) + `@group(1)` the per-node image
            // texture + sampler (the same shape the coverage atlas declares —
            // one source of truth via `atlas_layout_descriptor`).
            layout: vec![view_uniform_layout_descriptor(), atlas_layout_descriptor()],
            immediate_size: 0,
            vertex: VertexState {
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: Self::raster_vertex_buffers(),
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: key.samples,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                shader,
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}

/// Map one raster node's propagated transform + resolved layout (+ optional clip)
/// to a [`RasterInstance`] — the pure per-node mapping [`extract_buiy_rasters`]
/// applies to every `RasterImage`, factored out so it is unit-testable
/// device-free (the `extracted_node_for` convention). Position + size are in
/// LOGICAL px; the affine basis is the 2D linear part of `global_transform`
/// (columns `[m00, m10, m01, m11]`, exactly as `extracted_node_for` derives it).
pub fn raster_instance_for(
    global_transform: &GlobalTransform,
    layout: &ResolvedLayout,
    clip: Option<&ClipRect>,
) -> RasterInstance {
    let m = global_transform.affine().matrix3;
    let affine = [m.x_axis.x, m.x_axis.y, m.y_axis.x, m.y_axis.y];
    let (clip_min, clip_max) = match clip {
        Some(c) => ([c.min.x, c.min.y], [c.max.x, c.max.y]),
        None => (CLIP_SENTINEL_MIN, CLIP_SENTINEL_MAX),
    };
    let pos = global_transform.translation().truncate();
    RasterInstance {
        rect_pos: [pos.x, pos.y],
        rect_size: [layout.size.x, layout.size.y],
        clip_min,
        clip_max,
        affine,
    }
}

/// `ExtractSchedule` system: mirror every [`RasterImage`] layout node into the
/// render-world [`ExtractedRasters`] list. Reads `GlobalTransform` (the
/// propagated transform, like `extracted_node_for`), `ResolvedLayout.size`, and
/// the entity's own [`ClipRect`]. Rebuilt wholesale each frame (a handful of
/// raster nodes, so no damage gate in v1).
#[allow(clippy::type_complexity)]
pub fn extract_buiy_rasters(
    mut out: ResMut<ExtractedRasters>,
    query: Extract<
        Query<
            (
                &GlobalTransform,
                &ResolvedLayout,
                &RasterImage,
                Option<&ClipRect>,
            ),
            With<Node>,
        >,
    >,
) {
    out.instances.clear();
    out.images.clear();
    for (global_transform, layout, raster, clip) in &query {
        out.instances
            .push(raster_instance_for(global_transform, layout, clip));
        out.images.push(raster.0.id());
    }
}

/// `RenderSystems::Prepare` system: copy [`ExtractedRasters`] into the persistent
/// [`RasterBuffers`] (grow-in-place) and upload. Unconditional (no damage gate) —
/// a handful of 48 B instances is negligible; the heavy cost (the image texture
/// re-upload) is bevy's `RenderAsset` path, not this system.
pub fn prepare_buiy_rasters(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    extracted: Res<ExtractedRasters>,
    mut buffers: ResMut<RasterBuffers>,
) {
    buffers.instances.clear();
    for inst in &extracted.instances {
        buffers.instances.push(*inst);
    }
    buffers.count = extracted.instances.len() as u32;
    buffers.images.clone_from(&extracted.images);
    if buffers.count > 0 {
        buffers
            .instances
            .write_buffer(&render_device, &render_queue);
    }
}

/// One prepared raster draw: the `@group(1)` bind group (image texture + the
/// Nearest sampler) paired with the instance index it draws.
pub struct RasterDraw {
    /// `@group(1)` = (image `texture_view`, the Nearest sampler).
    pub bind_group: BindGroup,
    /// The instance index in [`RasterBuffers::instances`] this draw covers.
    pub instance: u32,
}

/// Build the per-image `@group(1)` bind groups for this frame's raster draws
/// (called by `buiy_pass` BEFORE the window pass opens — bind-group creation
/// needs the device, which the open pass borrows; the `composite_bindings`
/// precedent). Returns empty if the raster resources are absent or no `GpuImage`
/// has uploaded yet (the draw is simply skipped that frame — the established
/// async-upload skip class). An instance whose image is not yet resident is
/// dropped from this frame's draw list (it lands once the upload completes).
pub fn build_raster_draws(world: &World, render_context: &mut RenderContext) -> Vec<RasterDraw> {
    let (Some(buffers), Some(gpu), Some(pipeline)) = (
        world.get_resource::<RasterBuffers>(),
        world.get_resource::<RasterGpu>(),
        world.get_resource::<BuiyPipeline>(),
    ) else {
        return Vec::new();
    };
    if buffers.count == 0 {
        return Vec::new();
    }
    let Some(images) = world.get_resource::<RenderAssets<GpuImage>>() else {
        return Vec::new();
    };
    let device = render_context.render_device();
    let mut draws = Vec::new();
    for (i, image_id) in buffers.images.iter().enumerate() {
        let Some(gpu_image) = images.get(*image_id) else {
            continue; // not uploaded yet — skip this frame (resolves once ready).
        };
        // Build against `BuiyPipeline::atlas_layout` — the SAME
        // `texture_2d<f32>` + sampler shape the raster pipeline declares for
        // `@group(1)` (`atlas_layout_descriptor`), so the bind group is
        // layout-compatible with the pipeline.
        let bind_group = device.create_bind_group(
            "buiy_raster_image_bind_group",
            &pipeline.atlas_layout,
            &BindGroupEntries::sequential((&gpu_image.texture_view, &gpu.sampler)),
        );
        draws.push(RasterDraw {
            bind_group,
            instance: i as u32,
        });
    }
    draws
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_instance_stride_is_48_bytes() {
        assert!(raster_stride_agrees());
        assert_eq!(RASTER_INSTANCE_STRIDE_BYTES, 48);
    }

    #[test]
    fn raster_vertex_layout_stays_within_webgl2_16_attribute_cap() {
        // The band/glyph WebGL2 guard, applied to the raster layout: 7 attributes,
        // max location 6 — comfortably under the 16-attribute / loc<=15 cap.
        let buffers = BuiyRasterPipeline::raster_vertex_buffers();
        let locations: Vec<u32> = buffers
            .iter()
            .flat_map(|b| b.attributes.iter().map(|a| a.shader_location))
            .collect();
        assert_eq!(locations.len(), 7);
        assert!(locations.iter().copied().max().unwrap() <= 15);
    }

    #[test]
    fn raster_vertex_layout_offsets_match_repr_c() {
        // The instance VBO offsets must equal the `#[repr(C)]` field offsets of
        // `RasterInstance`, or the GPU reads garbage for the later attributes.
        let buffers = BuiyRasterPipeline::raster_vertex_buffers();
        let instance = &buffers[1];
        let offsets: Vec<u64> = instance.attributes.iter().map(|a| a.offset).collect();
        assert_eq!(offsets, vec![0, 8, 16, 24, 32]);
        assert_eq!(instance.array_stride, RASTER_INSTANCE_STRIDE_BYTES as u64);
    }

    #[test]
    fn raster_instance_is_pod_zeroable() {
        // The raw-vertex path requires POD; a zeroed instance is the identity-ish
        // degenerate (zero size, sentinel-free) — just proves the derive holds.
        let z = RasterInstance::zeroed();
        assert_eq!(z.rect_size, [0.0, 0.0]);
        assert_eq!(z.affine, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn default_raster_image_is_a_default_handle() {
        // Author convenience / reflect surface: `RasterImage::default()` is a
        // default (dangling) handle — a real author always sets a live handle.
        let r = RasterImage::default();
        assert_eq!(r.0, Handle::default());
    }

    // --- The device-free display-list mapping (`raster_instance_for`) ---------
    // These are the headless display-list assertions: the record a `RasterImage`
    // node extracts to, proven without a wgpu adapter (the `extracted_node_for`
    // convention). The end-to-end system wiring is exercised in
    // tests/render/render_raster.rs.

    fn layout(w: f32, h: f32) -> ResolvedLayout {
        ResolvedLayout {
            position: Vec2::ZERO,
            size: Vec2::new(w, h),
        }
    }

    #[test]
    fn raster_instance_forwards_size_and_identity_affine() {
        let inst = raster_instance_for(&GlobalTransform::IDENTITY, &layout(720.0, 450.0), None);
        assert_eq!(inst.rect_size, [720.0, 450.0]);
        // An identity transform paints axis-aligned at the origin.
        assert_eq!(inst.rect_pos, [0.0, 0.0]);
        assert_eq!(inst.affine, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn raster_instance_position_follows_global_transform() {
        // Position is `GlobalTransform.translation().xy` (pillar 5), exactly as
        // the node quad path takes it — not `ResolvedLayout.position`.
        let gt = GlobalTransform::from_translation(Vec3::new(30.0, 60.0, 0.0));
        let inst = raster_instance_for(&gt, &layout(220.0, 220.0), None);
        assert_eq!(inst.rect_pos, [30.0, 60.0]);
        assert_eq!(inst.rect_size, [220.0, 220.0]);
    }

    #[test]
    fn raster_instance_absent_clip_is_the_full_view_sentinel() {
        let inst = raster_instance_for(&GlobalTransform::IDENTITY, &layout(10.0, 10.0), None);
        assert_eq!(inst.clip_min, [f32::NEG_INFINITY, f32::NEG_INFINITY]);
        assert_eq!(inst.clip_max, [f32::INFINITY, f32::INFINITY]);
    }

    #[test]
    fn raster_instance_present_clip_carries_the_aabb() {
        let clip = ClipRect {
            min: Vec2::new(5.0, 6.0),
            max: Vec2::new(105.0, 86.0),
        };
        let inst = raster_instance_for(
            &GlobalTransform::IDENTITY,
            &layout(200.0, 100.0),
            Some(&clip),
        );
        assert_eq!(inst.clip_min, [5.0, 6.0]);
        assert_eq!(inst.clip_max, [105.0, 86.0]);
    }

    #[test]
    fn raster_instance_carries_nonuniform_scale_basis() {
        // A scaled transform packs its 2D linear part into the affine basis, so a
        // scaled canvas paints scaled (the same basis the node quad path uses).
        let gt = GlobalTransform::from_scale(Vec3::new(2.0, 3.0, 1.0));
        let inst = raster_instance_for(&gt, &layout(10.0, 10.0), None);
        assert_eq!(inst.affine, [2.0, 0.0, 0.0, 3.0]);
    }
}
