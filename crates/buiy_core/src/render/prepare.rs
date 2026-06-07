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

use crate::render::buckets::pack_view;
use crate::render::extract::{ExtractedNodes, ExtractedNodesView};
use crate::render::view_uniform::BuiyViewUniform;

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
    /// Instance count written this frame (the instanced draw range).
    pub quad_count: u32,
}

impl Default for BuiyInstanceBuffers {
    fn default() -> Self {
        Self {
            quad: RawBufferVec::new(BufferUsages::VERTEX),
            view_uniform: UniformBuffer::default(),
            quad_count: 0,
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
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    nodes: Res<ExtractedNodesView>,
    buffers: Option<ResMut<BuiyInstanceBuffers>>,
) {
    // Consume R5's ExtractedNodes: pack its per-view records into the flat quad
    // blob and build the view uniform (logical_size + scale_factor are R5's).
    let (instances, uniform) = pack_extracted_nodes(&nodes.0);

    // Get-or-insert the persistent buffers resource shim.
    let mut buffers = match buffers {
        Some(b) => b,
        None => {
            commands.init_resource::<BuiyInstanceBuffers>();
            // Skip this frame's upload; next frame the resource exists.
            // (Acceptable one-frame warmup; documented, not a hack —
            // grow-in-place buffers are created lazily on first sight.)
            return;
        }
    };

    // Repack the quad buffer in place: clear + extend (the Vec backing
    // grows; the GPU buffer grows only on capacity overflow).
    buffers.quad.clear();
    for inst in &instances {
        buffers.quad.push(*inst);
    }
    buffers.quad_count = instances.len() as u32;
    buffers.quad.write_buffer(&render_device, &render_queue);

    // Upload the std140 uniform (col0 ++ col1 ++ [scale_factor, 0, 0, 0]).
    // Regroup the flat 12 floats into the three `vec4` columns the WGSL
    // `BuiyView` reads; `[Vec4; 3]` is a valid std140 payload (16-byte stride),
    // unlike the bare `[f32; 12]` which would panic encase's compat assert.
    buffers.view_uniform.set(as_view_columns(uniform));
    buffers
        .view_uniform
        .write_buffer(&render_device, &render_queue);
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
