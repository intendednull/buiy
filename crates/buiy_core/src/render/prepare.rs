//! The prepare phase (architecture.md § 3.2 / § 4): per-view persistent GPU
//! instance buffers + the view uniform, written in `RenderSystems::Prepare`.
//!
//! Why prepare, not extract (architecture.md § 1.1 / § 4): `ViewTarget` (and a
//! settled `GlobalTransform`) do not exist until `prepare_view_targets`
//! (`RenderSystems::ManageViews`), which runs AFTER `ExtractSchedule`. So the
//! CPU-side per-view record (`ExtractedNodes`, owned by R5 in `render::extract`)
//! is an extract product, but the GPU buffers + view uniform are a PREPARE
//! product.
//!
//! v1 carrier shape (matches what R5 actually landed). The architecture target
//! (§ 4) stores BOTH the CPU record and the GPU buffers as COMPONENTS on the
//! resolved per-view render entity (per-window isolation). Resolving that
//! entity needs the render world and is deferred to R6/R8's GPU e2e wiring; R5
//! therefore exposes its `ExtractedNodes` through the single render-world
//! resource shim [`ExtractedNodesView`] (extract.rs), and R6's prepare reads
//! that resource and maintains its [`BuiyInstanceBuffers`] as the matching
//! render-world resource shim. The carrier flips from resource to per-view
//! component for both halves together when R6/R8 wires the view-entity routing
//! (the GPU `#[ignore]` round-trip is the gate for that step); the
//! `BuiyInstanceBuffers` *type* does not change.
//!
//! `ExtractedNodes` is **not redefined here** — it is owned by R5 and imported
//! from `crate::render::extract`. This module owns only `BuiyInstanceBuffers`
//! (the persistent GPU buffers) and the `prepare_buiy_instances` system.

use bevy::prelude::*;
use bevy::render::render_resource::{BufferUsages, RawBufferVec, UniformBuffer};
use bevy::render::renderer::{RenderDevice, RenderQueue};

use std::ops::Range;

use crate::render::atlas::GlyphAlphaInstance;
use crate::render::buckets::{pack_view, pack_view_partitioned};
use crate::render::extract::{ExtractedEffectGroups, ExtractedNodes, ExtractedNodesView};
use crate::render::view_uniform::BuiyViewUniform;

/// Render-world list of glyph-alpha instances to draw this frame, in paint
/// order. Produced by `text::extract_buiy_glyphs` in `ExtractSchedule` (T4):
/// it shapes glyphs, inserts coverage into the atlas, and pushes one
/// [`GlyphAlphaInstance`] per visible glyph here. Retained across steady
/// frames — `is_changed()` is the § 6.2 damage signal the glyph gate in
/// [`prepare_buiy_instances`] reads before packing it into
/// [`BuiyInstanceBuffers::glyph`].
#[derive(Resource, Default)]
pub struct ExtractedGlyphs {
    /// One instance per visible glyph, in paint order (the node draws them in
    /// this order, after the quad draw — shadow < quad < glyph < path).
    pub glyphs: Vec<GlyphAlphaInstance>,
}

/// Persistent per-view GPU instance buffers (architecture.md § 3.2): one
/// growable buffer per primitive, allocated once and reused frame-to-frame
/// (grow-in-place; never reallocated per frame), plus the view-uniform UBO.
///
/// v1 carrier: stored as the render-world resource shim that mirrors R5's
/// [`ExtractedNodesView`] (see the module docs). The architecture target (§ 4)
/// is a per-view-entity COMPONENT for per-window isolation; R6/R8 flips both
/// carriers to components together when the view-entity routing lands.
///
/// The quad instance store is a [`RawBufferVec`] (not a `BufferVec`): the
/// instance record is a raw `[f32; 13]` POD vertex blob (the pipeline-descriptor
/// layout), which is `NoUninit` but **not** a `ShaderType`, so it rides the
/// raw, CPU-readable vertex path rather than the std140/encase `BufferVec` path.
#[derive(Resource)]
pub struct BuiyInstanceBuffers {
    /// Quad-family instances (the v1 primitive set). Grows in place.
    pub quad: RawBufferVec<[f32; 13]>,
    /// Coverage-glyph instances (the alpha-as-color primitive,
    /// atlas-and-text-seam.md § 4.1). A `RawBufferVec<GlyphAlphaInstance>` for
    /// the same reason as `quad`: `GlyphAlphaInstance` is a raw `#[repr(C)]`
    /// vertex POD (the Glyph pipeline-descriptor layout), `NoUninit` but not a
    /// `ShaderType`, so it rides the raw, CPU-readable vertex path. Grows in
    /// place; the node draws it after the quad draw (paint order glyph > quad).
    pub glyph: RawBufferVec<GlyphAlphaInstance>,
    /// The per-view logical->clip + scale_factor uniform (`col0 ++ col1 ++
    /// [scale_factor, 0, 0, 0]`, [`BuiyViewUniform::as_std140_array`]).
    ///
    /// Carried as `[Vec4; 3]` — the WGSL `BuiyView` (3 × `vec4` = 48 B). A bare
    /// `[f32; 12]` is NOT a valid std140 uniform payload (a scalar array has a
    /// 4-byte stride, violating std140's 16-byte array-stride rule), so encase's
    /// `UNIFORM_COMPAT_ASSERT` panics inside `UniformBuffer::write_buffer` on the
    /// first GPU frame. `Vec4` has a 16-byte stride, so `[Vec4; 3]` encodes to a
    /// tight 48 B with no panic — mirroring how `bevy_render::view::ViewUniform`
    /// is a derived `ShaderType` of `vec4`/`mat4` fields, never a scalar array.
    /// The flat `[f32; 12]` from [`BuiyViewUniform::as_std140_array`] is regrouped
    /// into the three columns at the `set(...)` boundary in `prepare_buiy_instances`.
    pub view_uniform: UniformBuffer<[Vec4; 3]>,
    /// Quad instance count written this frame (the instanced draw range).
    pub quad_count: u32,
    /// Glyph instance count written this frame (the glyph instanced draw range).
    pub glyph_count: u32,
    /// Per-effect-group contiguous quad-instance ranges (`group_ranges[g]` =
    /// group `g`'s members), recomputed each quad-buffer upload from
    /// `ExtractedNode.group` (effect-compositor.md § 1.1 / decided fork 3). The
    /// node draws each range into its off-screen target in step 1 — NOT in the
    /// flat window draw. Empty (and so a no-op partition) when no group is live.
    pub group_ranges: Vec<Range<u32>>,
    /// The complement of `group_ranges`: maximal runs of non-group quad
    /// instances. The flat window draw covers exactly these so a group member is
    /// never painted twice (once flat, once composited — the double-paint TODO).
    /// When no group is live this is the single full `0..quad_count` range, so
    /// the flat path is byte-for-byte the pre-compositor draw.
    pub flat_ranges: Vec<Range<u32>>,
}

impl Default for BuiyInstanceBuffers {
    fn default() -> Self {
        Self {
            quad: RawBufferVec::new(BufferUsages::VERTEX),
            glyph: RawBufferVec::new(BufferUsages::VERTEX),
            view_uniform: UniformBuffer::default(),
            quad_count: 0,
            glyph_count: 0,
            group_ranges: Vec::new(),
            flat_ranges: Vec::new(),
        }
    }
}

/// Pure CPU half of the prepare phase: pack one view's [`ExtractedNodes`] into
/// the flat raw quad-instance blob (every batch concatenated in
/// `(primitive, layer)` order) and build the std140 view-uniform array. Split
/// out from [`prepare_buiy_instances`] so the carrier→batch wiring is testable
/// without a GPU device (the upload via `write_buffer` is the only GPU part).
///
/// R5's `ExtractedNodes.nodes` is fed to [`pack_view`] directly — no `DrawData`
/// adapter — so the prepare phase consumes R5's component with no parallel
/// carrier (the packing seam after Task 6's flip).
pub fn pack_extracted_nodes(nodes: &ExtractedNodes) -> (Vec<[f32; 13]>, [f32; 12]) {
    let buckets = pack_view(&nodes.nodes);
    let instances: Vec<[f32; 13]> = buckets
        .batches()
        .flat_map(|(_key, batch)| batch.iter().copied())
        .collect();
    let uniform = BuiyViewUniform::for_view(nodes.logical_size, nodes.scale_factor);
    (instances, uniform.as_std140_array())
}

/// `RenderSystems::Prepare` system: pack R5's [`ExtractedNodesView`] into
/// typed-primitive buckets, upload the persistent [`BuiyInstanceBuffers`]
/// (grow-in-place), and write the view uniform. `ViewTarget` is available in
/// this set (architecture.md § 4), unlike in extract.
///
/// v1 reads the single render-world [`ExtractedNodesView`] resource shim and
/// maintains `BuiyInstanceBuffers` as the matching resource shim (see module
/// docs); R6/R8 flips both to per-view-entity components together.
pub fn prepare_buiy_instances(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    nodes: Res<ExtractedNodesView>,
    groups: Res<ExtractedEffectGroups>,
    glyphs: Res<ExtractedGlyphs>,
    mut buffers: ResMut<BuiyInstanceBuffers>,
) {
    // Damage gate (architecture.md § 3.1): extract overwrites `ExtractedNodesView`
    // ONLY on a frame where a paint input actually changed (a despawn, a theme
    // swap, or a `Changed` paint component); on a steady-state frame it leaves the
    // resource resident, so `is_changed()` is the exact per-frame damage signal.
    // When nothing changed, RETAIN the persistent buffer — `BuiyNode::run` re-binds
    // and re-draws it as-is — and skip the GPU re-upload (the gate-#14 budget the
    // spec protects). `BuiyInstanceBuffers` is `init_resource`'d in the plugin
    // build, so it always exists here (no one-frame warmup).
    //
    // The quad and glyph buffers are gated INDEPENDENTLY: a frame that re-tints a
    // glyph (gate #2 test) changes only `ExtractedGlyphs`, so the quad buffer is
    // retained and only the glyph buffer re-uploads — and vice versa.
    if nodes.is_changed() || groups.is_changed() {
        // Consume R5's ExtractedNodes: pack its per-view records into the flat
        // quad blob, the per-group instance-range partition, and build the view
        // uniform (logical_size + scale_factor are R5's). The view uniform rides
        // the quad gate because R5's `ExtractedNodes` carries the logical_size/
        // scale_factor it is built from. The partition keys off `ExtractedNode.group`
        // (effect-compositor.md § 1.1): each group's contiguous range renders into
        // its own off-screen target (the node's step 1), the flat ranges into the
        // window — so a group member is never double-painted.
        let partition = pack_view_partitioned(&nodes.0.nodes, groups.0.len());
        let uniform =
            BuiyViewUniform::for_view(nodes.0.logical_size, nodes.0.scale_factor).as_std140_array();

        // Repack the quad buffer in place: clear + extend (the Vec backing
        // grows; the GPU buffer grows only on capacity overflow).
        buffers.quad.clear();
        for inst in &partition.instances {
            buffers.quad.push(*inst);
        }
        buffers.quad_count = partition.instances.len() as u32;
        buffers.quad.write_buffer(&render_device, &render_queue);
        // When NO group is live, the whole buffer is the flat draw — `pack_view_
        // partitioned` returns it as the single non-group run, so the node's flat
        // path stays byte-for-byte the pre-compositor draw.
        buffers.group_ranges = partition.group_ranges;
        buffers.flat_ranges = partition.flat_ranges;

        // Upload the std140 uniform (col0 ++ col1 ++ [scale_factor, 0, 0, 0]).
        // Regroup the flat 12 floats into the three `vec4` columns the WGSL
        // `BuiyView` reads; `[Vec4; 3]` is a valid std140 payload (16-byte
        // stride), unlike the bare `[f32; 12]` which would panic encase's
        // compat assert.
        buffers.view_uniform.set(as_view_columns(uniform));
        buffers
            .view_uniform
            .write_buffer(&render_device, &render_queue);
    }

    // Glyph buffer (the coverage-glyph primitive). Gated on its own change
    // signal so a re-tint-only frame re-uploads glyphs without touching quads.
    if glyphs.is_changed() {
        buffers.glyph.clear();
        for inst in &glyphs.glyphs {
            buffers.glyph.push(*inst);
        }
        buffers.glyph_count = glyphs.glyphs.len() as u32;
        buffers.glyph.write_buffer(&render_device, &render_queue);
    }
}

/// Regroup the flat std140 view-uniform array ([`BuiyViewUniform::as_std140_array`])
/// into the three `vec4` columns of the WGSL `BuiyView` (`col0`, `col1`,
/// `params`). The byte layout is identical (12 contiguous `f32` = 3 × `vec4`);
/// this only restates the type so the carrier is a valid std140 uniform.
fn as_view_columns(uniform: [f32; 12]) -> [Vec4; 3] {
    [
        Vec4::new(uniform[0], uniform[1], uniform[2], uniform[3]),
        Vec4::new(uniform[4], uniform[5], uniform[6], uniform[7]),
        Vec4::new(uniform[8], uniform[9], uniform[10], uniform[11]),
    ]
}
