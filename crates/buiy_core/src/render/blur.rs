//! Backdrop-blur (parity Wave B4): a real dual-Kawase blur over the painted
//! window backdrop, composited UNDER a `backdrop-filter` element.
//!
//! ## Why this is NOT an off-screen effect group
//!
//! The effect-group compositor (`compositor.rs` / `node.rs`) renders a group's
//! OWN subtree into a target and composites it over the parent — the right shape
//! for `opacity` / `isolation` / `filter`-on-self. `backdrop-filter` is the
//! INVERSE read: it samples what is painted BEHIND the element. So a
//! backdrop-filter element is handled as an IN-PLACE post-process on the window
//! main texture, run mid-`buiy_pass`:
//!
//! 1. the flat window pass paints everything EXCEPT the backdrop-filter
//!    element's own subtree (the backdrop) — `flat_ranges`;
//! 2. for each backdrop-filter element, [`run_backdrop_blur`](crate::render::node)
//!    samples the window's element region into a half-res scratch, runs the
//!    dual-Kawase down/up pyramid, then blits the blurred result back over the
//!    element rect in the window (`LoadOp::Load` preserves the rest);
//! 3. the element's own fill draws over the blurred backdrop
//!    (`backdrop_flat_ranges`).
//!
//! The Bevy-0.19 seam this depends on: `ViewTarget::main_texture()` is a
//! sampleable `TEXTURE_BINDING | COPY_SRC` texture, so the painted backdrop is
//! readable at the point the element composites (the Wave-B4 spike — see
//! `docs/reports/2026-06-25-parity-prototype-journal.md`). No `post_process_write`
//! ping-pong is needed for the window-parent case: the extract-into-scratch read
//! and the blit-back write target DIFFERENT textures, so they never alias.
//!
//! ## Scope (v1 prototype)
//!
//! WINDOW-parent only (the gallery's two uses — viewport header + modal scrim —
//! are both window-parented). A backdrop-filter element nested INSIDE another
//! effect group (its backdrop is a parent `Rgba16Float` target, not the window)
//! is a documented follow-up; this path leaves it un-blurred (it still paints,
//! just without the blur) rather than sampling the wrong texture.
//!
//! Spec: docs/specs/2026-06-25-widget-catalog-parity-design.md § 3.4 / § 8.

use core::marker::PhantomData;

use bevy::asset::uuid::Uuid;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AddressMode, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState,
    Buffer, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId, ColorTargetState,
    ColorWrites, Extent3d, FilterMode, FragmentState, FrontFace, MipmapFilterMode,
    MultisampleState, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipelineDescriptor,
    Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, SpecializedRenderPipeline,
    SpecializedRenderPipelines, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, VertexAttribute, VertexFormat, VertexState, VertexStepMode,
    binding_types::{sampler, texture_2d, uniform_buffer},
};
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::CachedTexture;
use bevy::shader::Shader;

/// Stable UUID for the blur shader (octet `..08` — the next free octet after the
/// gradient shader `..07`).
const BLUR_SHADER_UUID: Uuid = Uuid::from_u128(0xB01A_0108_0000_0000_0000_0000_0000_0008u128);

/// Weak handle to the blur WGSL shader (octet `..08`), backed by `blur.wgsl`,
/// loaded into the MAIN world by `BuiyRenderPlugin::build`.
pub fn blur_shader_handle() -> Handle<Shader> {
    Handle::Uuid(BLUR_SHADER_UUID, PhantomData)
}

/// The scratch pyramid's pinned format: `Rgba16Float` (linear), the SAME format
/// as the effect-group targets — the blur averages LINEAR light, so the result
/// composites in linear space (effect-compositor.md § 4). The window backdrop
/// the L0 down-pass samples is the view's main texture; its texels are read
/// linearly by the sampler (the down pass is the linear-space entry point).
pub const SCRATCH_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

/// One blur pass's `BlurParams` uniform (`@group(0) @binding(0)`), byte-identical
/// to the WGSL `BlurParams` struct (2 × `vec4` = 32 B):
/// - `texel_and_offset` = `[1/src_w, 1/src_h, kawase_offset, pad]`;
/// - `src_rect` = the source sub-rect in normalized uv `[min.x, min.y, max.x,
///   max.y]` (the L0 down pass reads the element window region; deeper passes
///   read the full previous level, `[0,0,1,1]`).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlurParams {
    pub texel_and_offset: [f32; 4],
    pub src_rect: [f32; 4],
}

/// `@group(0)` blur-params uniform layout entries (vertex + fragment): one
/// `var<uniform>` of 2 × `vec4` (32 B), byte-identical to [`BlurParams`].
/// Shared by the pipeline descriptor and the concrete bind-group layout.
/// `[Vec4; 2]` (not `BlurParams`) is the layout type because
/// `BindGroupLayoutEntries` needs `ShaderType`, which `[Vec4; 2]` implements; the
/// upload struct is plain POD (the `CompositePipeline` precedent).
fn uniform_layout_entries() -> [bevy::render::render_resource::BindGroupLayoutEntry; 1] {
    BindGroupLayoutEntries::single(
        ShaderStages::VERTEX_FRAGMENT,
        uniform_buffer::<[Vec4; 2]>(false),
    )
}

/// `@group(1)` sampled-source layout: a `texture_2d<f32>` + a FILTERING sampler.
/// Filtering is load-bearing — each Kawase tap reads BETWEEN texels, so the
/// bilinear fetch widens the footprint far beyond the literal tap count.
fn source_layout_entries() -> BindGroupLayoutEntries<2> {
    BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
        ),
    )
}

fn uniform_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new("buiy_blur_uniform_layout", &uniform_layout_entries())
}

fn source_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new("buiy_blur_source_layout", &source_layout_entries())
}

/// Which dual-Kawase tap a blur pass runs, and into which attachment format. The
/// down/up passes always write `Rgba16Float` scratch; the final blit-back writes
/// the WINDOW format (so it specializes per the view's main-texture format).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlurStage {
    /// Dual-Kawase downsample tap (13-sample). Shrinks the pyramid.
    Down,
    /// Dual-Kawase upsample tap (8-sample tent). Grows the pyramid.
    Up,
}

impl BlurStage {
    fn entry_point(self) -> &'static str {
        match self {
            BlurStage::Down => "down",
            BlurStage::Up => "up",
        }
    }
}

/// Specialization key: the tap stage plus the destination attachment's format
/// and sample count. The down/up taps write single-sampled `Rgba16Float`
/// scratch, so they key `samples = 1`. The final blit-back reuses the `up` tap
/// but writes the WINDOW attachment, whose format and sample count are the
/// view's — a bare `Camera2d` defaults to `Msaa::Sample4`, so the blit pipeline
/// must match the multisampled window pass or wgpu rejects it at `set_pipeline`
/// (the `BuiyViewPipelines` precedent).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BlurKey {
    pub stage: BlurStage,
    pub format: TextureFormat,
    pub samples: u32,
}

/// The device-owning blur pipeline resources: the two bind-group layouts, a
/// LINEAR clamp sampler, the unit-quad VBO, and the `SpecializedRenderPipeline`.
/// Mirrors `CompositePipeline` (composite.rs) — same shape, distinct shader +
/// the filtering sampler the Kawase taps require.
#[derive(Resource)]
pub struct BlurPipeline {
    /// `@group(0)` blur-params uniform layout (vertex + fragment).
    pub uniform_layout: BindGroupLayout,
    /// `@group(1)` source-texture + sampler layout (fragment).
    pub source_layout: BindGroupLayout,
    /// The shared LINEAR clamp sampler (bilinear taps; clamp avoids wrapping the
    /// blur at the scratch edge).
    pub sampler: Sampler,
    /// The unit-quad VBO (pos, uv) — same TL,BL,TR,BR TriangleStrip winding as
    /// the composite VBO.
    pub vertex_buffer: Buffer,
}

impl SpecializedRenderPipeline for BlurPipeline {
    type Key = BlurKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("buiy_blur_pipeline".into()),
            layout: vec![uniform_layout_descriptor(), source_layout_descriptor()],
            immediate_size: 0,
            vertex: VertexState {
                shader: blur_shader_handle(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: vec![VertexBufferLayout {
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
                }],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: None,
            // Match the destination attachment's sample count: 1 for the
            // `Rgba16Float` scratch pyramid; the view's `Msaa` samples for the
            // blit-back into the (possibly multisampled) window pass.
            multisample: MultisampleState {
                count: key.samples,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                shader: blur_shader_handle(),
                shader_defs: vec![],
                entry_point: Some(key.stage.entry_point().into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    // The blur pyramid REPLACES each level (no blend); the final
                    // blit-back also replaces the element region (the backdrop is
                    // already painted, we overwrite it with its blurred copy).
                    blend: if key.format == SCRATCH_FORMAT {
                        None
                    } else {
                        // Window blit-back: opaque replace too (we own the rect).
                        Some(BlendState::REPLACE)
                    },
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}

impl FromWorld for BlurPipeline {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        let uniform_layout =
            device.create_bind_group_layout("buiy_blur_uniform_layout", &uniform_layout_entries());
        let source_layout =
            device.create_bind_group_layout("buiy_blur_source_layout", &source_layout_entries());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("buiy_blur_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            // LINEAR mag/min — the Kawase taps depend on bilinear in-between fetches.
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Nearest,
            ..Default::default()
        });

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct QuadVertex {
            pos: [f32; 2],
            uv: [f32; 2],
        }
        let quad: [QuadVertex; 4] = [
            QuadVertex {
                pos: [0.0, 0.0],
                uv: [0.0, 0.0],
            },
            QuadVertex {
                pos: [0.0, 1.0],
                uv: [0.0, 1.0],
            },
            QuadVertex {
                pos: [1.0, 0.0],
                uv: [1.0, 0.0],
            },
            QuadVertex {
                pos: [1.0, 1.0],
                uv: [1.0, 1.0],
            },
        ];
        let vertex_buffer = device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("buiy_blur_unit_quad_vbo"),
            contents: bytemuck::cast_slice(&quad),
            usage: BufferUsages::VERTEX,
        });

        Self {
            uniform_layout,
            source_layout,
            sampler,
            vertex_buffer,
        }
    }
}

/// The scratch `Rgba16Float` target descriptor for one pyramid level of
/// `extent` physical texels. `RENDER_ATTACHMENT` (a pass writes into it) |
/// `TEXTURE_BINDING` (the next pass samples it). Descriptor-keyed `TextureCache`
/// reuse depends on this being byte-stable per extent.
pub fn scratch_descriptor(extent: UVec2) -> TextureDescriptor<'static> {
    TextureDescriptor {
        label: Some("buiy_backdrop_blur_scratch"),
        size: Extent3d {
            width: extent.x.max(1),
            height: extent.y.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: SCRATCH_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    }
}

// ---------------------------------------------------------------------------
// CPU pyramid planner (headless-testable — pure math, no device).
// ---------------------------------------------------------------------------

/// One level of the dual-Kawase pyramid: the scratch extent (physical texels)
/// at this level. Level 0 is the half-res capture of the element's window
/// region; each deeper level halves again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlurLevel {
    pub extent: UVec2,
}

/// The CPU plan for one backdrop-filter element's blur: the per-level scratch
/// extents (down then back up reuse the same level textures) and the per-pass
/// sample-offset scale that reaches the requested blur radius.
#[derive(Clone, Debug, PartialEq)]
pub struct BlurPlan {
    /// Pyramid levels, level 0 (largest, half-res) first. `down` walks 0→N-1
    /// shrinking; `up` walks N-1→0 growing. A plan with 0 levels means "no blur"
    /// (radius rounded to nothing) — the caller skips the element.
    pub levels: Vec<BlurLevel>,
    /// The per-pass Kawase sample-offset, in SOURCE texels. A constant 0.5 (the
    /// canonical half-texel dual-Kawase offset) — the EFFECTIVE radius grows
    /// because each pyramid level is half the resolution of the one above, so a
    /// half-texel step there covers twice the screen distance.
    pub offset: f32,
}

/// The canonical dual-Kawase half-texel sample offset (source texels). The
/// effective blur radius is driven by the PYRAMID DEPTH, not this constant.
pub const KAWASE_OFFSET: f32 = 0.5;

/// Number of pyramid levels for a CSS `blur(radius_px)` over a region of
/// `rect_physical` texels. Each level halves the resolution and roughly doubles
/// the effective blur footprint, so `levels ≈ log2(radius)`. The level count maps
/// to the spec's pass budget: each level is one DOWN pass, plus the UP chain
/// (levels-1) and the final blit (1) — so `N` levels == `2N` passes. The design's
/// two uses land where the spec expects: blur(2px) → 1 level (2 passes, the soft
/// header-style blur), blur(6px) → 2 levels (4 passes, "blur(6px) ≈ 4 passes"
/// from the design). Clamped so a level never shrinks below 1 texel on either
/// axis (a degenerate scratch produces no blur).
///
/// `radius_px` is in PHYSICAL px (the caller folds `scale_factor` in). A radius
/// below 0.5 px yields 0 levels (sub-texel blur is a no-op).
pub fn blur_levels(rect_physical: UVec2, radius_px: f32) -> Vec<BlurLevel> {
    if radius_px < 0.5 || rect_physical.x == 0 || rect_physical.y == 0 {
        return Vec::new();
    }
    // floor(log2(radius)) levels, at least 1 for any radius >= 0.5 px:
    // blur(2px)→1, blur(6px)→2, blur(8px)→3, blur(16px)→4 — `2N` passes each.
    let target = (radius_px.log2().floor() as i32).max(1) as u32;

    let mut levels = Vec::new();
    // Level 0 is HALF the element-rect resolution (the first downsample), each
    // deeper level halves again — but never below 1 texel on an axis.
    let mut extent = (rect_physical / 2).max(UVec2::ONE);
    for _ in 0..target {
        levels.push(BlurLevel { extent });
        let next = (extent / 2).max(UVec2::ONE);
        // Stop growing the pyramid once a level would be a 1×1 (or stuck) — no
        // useful blur beyond that, and equal extents would alias in the cache.
        if next == extent {
            break;
        }
        extent = next;
    }
    levels
}

/// Build the full [`BlurPlan`] for a `blur(radius_px)` over `rect_physical`.
pub fn plan_blur_pyramid(rect_physical: UVec2, radius_px: f32) -> BlurPlan {
    BlurPlan {
        levels: blur_levels(rect_physical, radius_px),
        offset: KAWASE_OFFSET,
    }
}

// ---------------------------------------------------------------------------
// Per-view prepared backdrop blurs (the node reads this off the view entity).
// ---------------------------------------------------------------------------

/// One backdrop-filter element's prepared blur: where in the window to sample +
/// blit (physical-px rect), the pyramid plan, and the acquired scratch level
/// textures (level 0 first). The node samples the window's `region` into level
/// 0, runs down/up over the levels, then blits level 0 back over `region`.
#[derive(Clone)]
pub struct PreparedBackdropBlur {
    /// The element's rect in the WINDOW, physical texels (origin top-left). The
    /// down-pass L0 samples this sub-rect of the main texture; the blit-back
    /// writes it back. Already clamped to the view's physical bounds.
    pub region: URect,
    /// The element's rect in NORMALIZED window UV (`region / view_physical`) — the
    /// sub-rect of the main texture the L0 down-pass reads.
    pub src_uv_min: Vec2,
    pub src_uv_max: Vec2,
    /// The per-pass Kawase offset (source texels) — [`BlurPlan::offset`].
    pub offset: f32,
    /// The WINDOW physical size in texels (the L0 down pass's source-texel pitch
    /// is `1/window`, since it samples a sub-rect of the full-size main texture).
    pub window_physical: UVec2,
    /// The acquired scratch level textures, level 0 (largest) first, in lockstep
    /// with [`BlurPlan::levels`]. Empty == the blur was skipped (sub-texel radius
    /// or a degenerate rect); the node leaves the backdrop un-blurred.
    pub levels: Vec<CachedTexture>,
    /// The per-level scratch extents (physical texels), in lockstep with `levels`
    /// — the node reads these for each pass's `1/source_size` instead of querying
    /// the wgpu `Texture` (the plan owns the sizing).
    pub level_extents: Vec<UVec2>,
    /// Whether this backdrop-filter former is in a top-layer subtree (top-layer
    /// stacking composite, § 3.3 / rev-4/M1). Stamped from the former entity's
    /// `ExtractedNode.top_layer`; `PreparedBackdropBlur` carries no `entity`, so
    /// the node's per-block draw filters the blur slice on THIS flag (base blurs
    /// run in the base block, top-layer blurs in the top block, so a top-layer
    /// backdrop samples the base beneath it — not the un-drawn top block).
    pub top_layer: bool,
}

/// Per-view carrier for the prepared backdrop blurs (parallel to
/// `PreparedEffectGroups` — a COMPONENT on the view render entity, so the node's
/// `ViewQuery` resolves it). Built by `prepare_backdrop_blurs`. An empty vec
/// (or absent component) means no backdrop-filter element this frame, and the
/// node's flat path runs unchanged.
///
/// The three pipeline ids are specialized in prepare (the node's `&World` cannot
/// get the mutable specialization cache — the `PreparedEffectGroups` precedent);
/// the node only reads them. `None` until they async-compile (a skipped blur that
/// frame, the established render behavior class).
#[derive(Component, Default, Clone)]
pub struct PreparedBackdropBlurs {
    pub blurs: Vec<PreparedBackdropBlur>,
    /// `down` tap → `Rgba16Float` scratch.
    pub down_pipeline: Option<CachedRenderPipelineId>,
    /// `up` tap → `Rgba16Float` scratch.
    pub up_pipeline: Option<CachedRenderPipelineId>,
    /// `up` tap → WINDOW format (the final blit-back over the element region).
    pub blit_pipeline: Option<CachedRenderPipelineId>,
}

/// Per-view prepare pass (`RenderSystems::Prepare`): build each view's
/// [`PreparedBackdropBlurs`] from the extracted backdrop-filter groups. For each
/// former carrying a px blur, it folds `scale_factor` into the radius + the
/// element box, clamps the box to the view's physical bounds, plans the
/// dual-Kawase pyramid ([`plan_blur_pyramid`]), acquires the pooled
/// `Rgba16Float` scratch level textures (`&mut TextureCache`, the node's `&World`
/// cannot), specializes the down/up pipelines, and INSERTS the carrier onto the
/// view render entity (a COMPONENT, so `buiy_pass`'s `ViewQuery` resolves it).
///
/// Runs AFTER `prepare_effect_groups` so it shares the same `TextureCache` frame
/// (both acquire pooled targets) and the same view-physical derivation; the two
/// are independent otherwise (backdrop blur is NOT an off-screen group — see the
/// module doc). v1: WINDOW-parent backdrop filters only — a former with a
/// `parent` (nested inside another effect group) is SKIPPED here (its backdrop is
/// the parent target, not the window; a documented follow-up).
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_backdrop_blurs(
    mut commands: Commands,
    render_device: Res<bevy::render::renderer::RenderDevice>,
    mut texture_cache: ResMut<bevy::render::texture::TextureCache>,
    extracted: Res<crate::render::extract::ExtractedEffectGroups>,
    nodes: Res<crate::render::extract::ExtractedNodesView>,
    pipeline_cache: Res<bevy::render::render_resource::PipelineCache>,
    blur_pipeline: Res<BlurPipeline>,
    mut specialized: ResMut<BlurSpecializedPipelines>,
    views: Query<(
        Entity,
        &bevy::render::view::ViewTarget,
        &bevy::render::view::Msaa,
    )>,
) {
    let scale_factor = nodes.0.scale_factor;
    let view_physical = (nodes.0.logical_size * scale_factor).ceil().as_uvec2();

    // The top-layer stacking composite (§ 3.3 / M1): the former entity → its
    // `ExtractedNode.top_layer`, mirroring the `top_layer_by_entity` map prepare.rs
    // builds for the glyph/icon partition. `PreparedBackdropBlur` has no `entity`,
    // so this stamps the flag now for the node's per-block blur filter.
    let top_layer_by_entity: std::collections::HashMap<bevy::prelude::Entity, bool> = nodes
        .0
        .nodes
        .iter()
        .map(|n| (n.entity, n.top_layer))
        .collect();

    // The window main-texture format + sample count the final blit-back writes
    // (per-view; v1/D2 single primary view — specialize for the first view, reuse
    // for the rest, mirroring `prepare_effect_groups`). No view ⇒ nothing to
    // prepare.
    let Some((window_format, window_samples)) = views
        .iter()
        .next()
        .map(|(_, vt, msaa)| (vt.main_texture_format(), msaa.samples()))
    else {
        return;
    };

    // Specialize the three pipelines ONCE per frame (the node only reads the
    // ids): down + up into the SINGLE-sampled `Rgba16Float` scratch, and the up
    // tap into the WINDOW format AT the view's sample count for the blit-back.
    let down_pipeline = Some(specialized.pipelines.specialize(
        &pipeline_cache,
        &blur_pipeline,
        BlurKey {
            stage: BlurStage::Down,
            format: SCRATCH_FORMAT,
            samples: 1,
        },
    ));
    let up_pipeline = Some(specialized.pipelines.specialize(
        &pipeline_cache,
        &blur_pipeline,
        BlurKey {
            stage: BlurStage::Up,
            format: SCRATCH_FORMAT,
            samples: 1,
        },
    ));
    let blit_pipeline = Some(specialized.pipelines.specialize(
        &pipeline_cache,
        &blur_pipeline,
        BlurKey {
            stage: BlurStage::Up,
            format: window_format,
            samples: window_samples,
        },
    ));

    let mut blurs: Vec<PreparedBackdropBlur> = Vec::new();
    for g in extracted.0.iter() {
        // Only WINDOW-parented backdrop-filter formers with a real px blur.
        let (Some(radius_logical), Some(box_logical)) = (g.backdrop_blur_px, g.backdrop_box) else {
            continue;
        };
        if g.parent.is_some() {
            // Nested backdrop filter: its backdrop is a parent Rgba16Float target,
            // not the window (v1 follow-up). Skip — leaves it un-blurred.
            continue;
        }

        // Fold scale_factor into the radius + the element box; clamp the box to
        // the view's physical bounds (the blur samples only on-screen texels).
        let radius_physical = radius_logical * scale_factor;
        let min = (box_logical.min * scale_factor).floor().max(Vec2::ZERO);
        let max = (box_logical.max * scale_factor)
            .ceil()
            .min(view_physical.as_vec2());
        if !(max.x > min.x && max.y > min.y) {
            continue; // fully off-screen / degenerate.
        }
        let region = URect::new(min.x as u32, min.y as u32, max.x as u32, max.y as u32);
        let rect_physical = region.size();

        let plan = plan_blur_pyramid(rect_physical, radius_physical);
        if plan.levels.is_empty() {
            continue; // sub-texel blur — nothing to do (the fill still paints).
        }

        // Acquire one pooled `Rgba16Float` scratch per pyramid level.
        let levels: Vec<CachedTexture> = plan
            .levels
            .iter()
            .map(|lvl| texture_cache.get(&render_device, scratch_descriptor(lvl.extent)))
            .collect();
        let level_extents: Vec<UVec2> = plan.levels.iter().map(|l| l.extent).collect();

        let vp = view_physical.as_vec2().max(Vec2::ONE);
        blurs.push(PreparedBackdropBlur {
            region,
            src_uv_min: min / vp,
            src_uv_max: max / vp,
            offset: plan.offset,
            window_physical: view_physical,
            levels,
            level_extents,
            // A former with no node record (a transient impossibility) is treated
            // as base — the byte-stable default (§ 3.3).
            top_layer: top_layer_by_entity.get(&g.entity).copied().unwrap_or(false),
        });
    }

    let prepared = PreparedBackdropBlurs {
        blurs,
        down_pipeline,
        up_pipeline,
        blit_pipeline,
    };
    for (view, _, _) in &views {
        commands.entity(view).insert(prepared.clone());
    }
}

/// Register the blur specialization cache (device-free) + the prepare system in
/// `build`. The prepare pass reads `Res<BlurPipeline>` (a finish-time resource,
/// like `prepare_effect_groups`'s `CompositePipeline`), inserted before the first
/// Render run; it is pinned AFTER `prepare_effect_groups` so the two pooled-target
/// acquirers share one `TextureCache` frame and the same view-physical derivation.
pub(crate) fn register(render_app: &mut bevy::app::SubApp) {
    use bevy::render::{Render, RenderSystems};
    render_app.init_resource::<BlurSpecializedPipelines>();
    render_app.add_systems(
        Render,
        prepare_backdrop_blurs
            .in_set(RenderSystems::Prepare)
            .after(crate::render::compositor::prepare_effect_groups),
    );
}

/// `finish`-time half: the device-owning [`BlurPipeline`].
pub(crate) fn register_gpu(render_app: &mut bevy::app::SubApp) {
    render_app.init_resource::<BlurPipeline>();
}

/// The blur pipeline specialization cache (mirrors `BuiySpecializedPipelines`).
/// A distinct resource so the blur pipelines do not entangle with the
/// effect-group composite cache.
#[derive(Resource, Default)]
pub struct BlurSpecializedPipelines {
    pub pipelines: SpecializedRenderPipelines<BlurPipeline>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_texel_radius_yields_no_levels() {
        // A radius below half a texel is a no-op (no blur pyramid).
        assert!(blur_levels(UVec2::new(100, 40), 0.4).is_empty());
        assert!(blur_levels(UVec2::new(100, 40), 0.0).is_empty());
    }

    #[test]
    fn degenerate_rect_yields_no_levels() {
        assert!(blur_levels(UVec2::new(0, 40), 6.0).is_empty());
        assert!(blur_levels(UVec2::new(100, 0), 6.0).is_empty());
    }

    #[test]
    fn radius_drives_pyramid_depth() {
        // blur(2px) → 1 level (2 passes), blur(6px) → 2 levels (4 passes) — the
        // design's two uses — on a region big enough not to bottom out at 1×1.
        let big = UVec2::new(1024, 256);
        assert_eq!(blur_levels(big, 2.0).len(), 1);
        assert_eq!(blur_levels(big, 6.0).len(), 2);
        assert_eq!(blur_levels(big, 8.0).len(), 3);
        // Monotonic: a larger radius never yields fewer levels.
        assert!(blur_levels(big, 16.0).len() >= blur_levels(big, 6.0).len());
    }

    #[test]
    fn level_zero_is_half_res_and_each_halves() {
        let levels = blur_levels(UVec2::new(800, 200), 8.0);
        assert_eq!(levels[0].extent, UVec2::new(400, 100));
        // Each deeper level halves again (until it would stick at 1).
        for w in levels.windows(2) {
            assert_eq!(w[1].extent, (w[0].extent / 2).max(UVec2::ONE));
        }
    }

    #[test]
    fn pyramid_never_shrinks_below_one_texel() {
        // A thin strip (the viewport header is 1px tall in the limit) must not
        // produce a 0-height scratch; every level is at least 1×1.
        let levels = blur_levels(UVec2::new(2000, 3), 16.0);
        for l in &levels {
            assert!(l.extent.x >= 1 && l.extent.y >= 1);
        }
        // Equal-extent dedup: no two adjacent levels are identical (would alias
        // the descriptor-keyed cache and waste a pass).
        for w in levels.windows(2) {
            assert_ne!(w[0].extent, w[1].extent);
        }
    }

    #[test]
    fn plan_carries_canonical_offset() {
        let plan = plan_blur_pyramid(UVec2::new(512, 128), 6.0);
        assert_eq!(plan.offset, KAWASE_OFFSET);
        assert_eq!(plan.levels.len(), 2);
    }
}
