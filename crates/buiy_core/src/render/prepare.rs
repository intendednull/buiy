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

use bevy::ecs::entity::{EntityHashMap, EntityHashSet};
use bevy::prelude::*;
use bevy::render::render_resource::{BufferUsages, RawBufferVec, UniformBuffer};
use bevy::render::renderer::{RenderDevice, RenderQueue};

use std::collections::HashMap;
use std::ops::Range;

use crate::render::atlas::GlyphAlphaInstance;
use crate::render::buckets::{
    pack_band_instances, pack_gradient_instances, pack_rounded_shadow_instances,
    pack_shadow_instances, pack_view, pack_view_partitioned, packed_to_raw, partition_glyph_ranges,
};
use crate::render::extract::{
    ExtractedEffectGroups, ExtractedNodes, ExtractedNodesView, ExtractedTextQuads, NodeDamage,
    RetainedNodeIndex,
};
use crate::render::icon_producer::ExtractedIcons;
use crate::render::instance::{
    BorderBandInstance, GradientInstance, RoundedShadowInstance, pack_extracted,
};
use crate::render::view_uniform::BuiyViewUniform;
use crate::text::GlyphDamage;

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
    /// One run per emitting entity, in paint order, covering `glyphs`
    /// exactly (empty ⇔ `glyphs` empty). Published in lockstep with
    /// `glyphs` under ONE change tick (D4).
    pub entity_runs: Vec<GlyphEntityRun>,
}

/// One emitting entity's contiguous slice of [`ExtractedGlyphs::glyphs`]
/// (T8 / D1). The producer emits each entity's glyph-tier instances (run
/// glyphs, line-through stamps, the caret stamp) inside one walk
/// iteration, so one run per entity is exact; runs are gapless from 0 and
/// cover the instance vec. The prepare partition maps `entity` to its
/// `ExtractedNode.group` off the FRESH node list — group membership is
/// never recorded here (it would go stale against node-walk rebuilds, the
/// § 4.6 rejected runner-up).
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphEntityRun {
    /// The source main-world entity — the group-lookup key.
    pub entity: Entity,
    /// This entity's instance indices in [`ExtractedGlyphs::glyphs`].
    pub instances: Range<u32>,
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
/// instance record is a raw `[f32; 17]` POD vertex blob (the pipeline-descriptor
/// layout), which is `NoUninit` but **not** a `ShaderType`, so it rides the
/// raw, CPU-readable vertex path rather than the std140/encase `BufferVec` path.
#[derive(Resource)]
pub struct BuiyInstanceBuffers {
    /// Quad-family instances (the v1 primitive set). Grows in place.
    pub quad: RawBufferVec<[f32; 17]>,
    /// Coverage-glyph instances (the alpha-as-color primitive,
    /// atlas-and-text-seam.md § 4.1). A `RawBufferVec<GlyphAlphaInstance>` for
    /// the same reason as `quad`: `GlyphAlphaInstance` is a raw `#[repr(C)]`
    /// vertex POD (the Glyph pipeline-descriptor layout), `NoUninit` but not a
    /// `ShaderType`, so it rides the raw, CPU-readable vertex path. Grows in
    /// place; the node draws it after the quad draw (paint order glyph > quad).
    pub glyph: RawBufferVec<GlyphAlphaInstance>,
    /// Vector-ICON coverage instances (parity Wave B3). A
    /// `RawBufferVec<GlyphAlphaInstance>` — an icon record IS a
    /// `GlyphAlphaInstance` (R8 coverage + per-instance tint), so it rides the
    /// EXACT same raw-vertex path and the EXACT same coverage pipeline + atlas
    /// bind group as `glyph`, drawn through a SEPARATE buffer/draw only so the
    /// wholesale-rebuilt glyph carrier stays decoupled from the icon carrier
    /// (§ 3.5; no new GPU shader). Grows in place; the node draws it right after
    /// the glyph draw (both coverage tier). Gated on its OWN `ExtractedIcons`
    /// change signal, independent of the glyph/quad gates.
    pub icon: RawBufferVec<GlyphAlphaInstance>,
    /// Border/outline BAND instances (styling-f-tier.md § 2.3 — C6-a feeds the
    /// OUTLINE channel, C6-b adds the per-side BORDER). A
    /// `RawBufferVec<BorderBandInstance>` for the same reason as `quad`/`glyph`:
    /// `BorderBandInstance` is a raw `#[repr(C)]` vertex POD (the band
    /// pipeline-descriptor layout), `NoUninit` but not a `ShaderType`, so it
    /// rides the raw, CPU-readable vertex path. Grows in place; the node draws it
    /// AFTER the quad/glyph draw so the band sits on top.
    pub band: RawBufferVec<BorderBandInstance>,
    /// Box-shadow instances (styling-f-tier.md § 2.2 — C6-b). A
    /// `RawBufferVec<[f32; 17]>` — the shadow reuses the frozen 68 B
    /// [`PackedInstance`](crate::render::instance::PackedInstance) layout (radius
    /// slot → blur sigma, zero stride change), so it shares the quad's raw
    /// `[f32; 17]` vertex POD. Grows in place; the node draws it FIRST (before
    /// the quad), so a shadow paints BEHIND its caster (shadow < quad in
    /// `paint_order`).
    pub shadow: RawBufferVec<[f32; 17]>,
    /// Background-GRADIENT instances (parity Wave B1). A
    /// `RawBufferVec<GradientInstance>` — `GradientInstance` is a raw `#[repr(C)]`
    /// vertex POD (the gradient pipeline-descriptor layout), `NoUninit` but not a
    /// `ShaderType`, so it rides the raw, CPU-readable vertex path like
    /// `quad`/`glyph`/`band`. Grows in place; the node draws it AFTER the quad
    /// (over the solid fill), BEFORE glyphs/bands. Packed from the SAME node walk
    /// as the quad, so it rides the quad gate.
    pub gradient: RawBufferVec<GradientInstance>,
    /// ROUNDED box-shadow instances (F4b-6). A `RawBufferVec<RoundedShadowInstance>`
    /// — a raw `#[repr(C)]` vertex POD like `band`/`gradient`. Grows in place; the
    /// node draws it in the SHADOW tier (before the flat quads, alongside the
    /// square shadow blob). Packed from the same node walk, so it rides the quad
    /// gate. The SQUARE shadow blob (`shadow`) is disjoint and byte-stable.
    pub rounded_shadow: RawBufferVec<RoundedShadowInstance>,
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
    /// Vector-icon instance count written this frame (parity Wave B3). Gated on
    /// the `ExtractedIcons` change signal, independent of the glyph gate.
    pub icon_count: u32,
    /// Border/outline band instance count written this frame (C6-a/C6-b). Rides
    /// the quad gate (the band is packed from the same node walk).
    pub band_count: u32,
    /// Box-shadow instance count written this frame (C6-b). Rides the quad gate
    /// (shadows are packed from the same node walk).
    pub shadow_count: u32,
    /// Background-gradient instance count written this frame (parity Wave B1).
    /// Rides the quad gate (gradients are packed from the same node walk).
    pub gradient_count: u32,
    /// Rounded box-shadow instance count written this frame (F4b-6). Rides the
    /// quad gate (packed from the same node walk).
    pub rounded_shadow_count: u32,
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
    /// Per-effect-group contiguous GLYPH-instance ranges (T8 —
    /// `glyph_group_ranges[g]` = group `g`'s glyph members), the glyph mirror
    /// of [`group_ranges`](Self::group_ranges). Recomputed CPU-only under the
    /// UNION of the quad and glyph gates (D2): membership derives from the
    /// fresh node list, instance indices from the (possibly retained) glyph
    /// carrier — either side changing re-derives. The node's step-1 group
    /// pass draws each range into the group's off-screen target via the
    /// `Glyph@Rgba16Float` specialization.
    pub glyph_group_ranges: Vec<Range<u32>>,
    /// The complement: maximal runs of non-group glyph instances — the flat
    /// window glyph draw covers exactly these (a group's glyph is never
    /// painted twice). When no group is live: the single full
    /// `0..glyph_count` run, so the flat path is byte-for-byte the pre-T8
    /// draw.
    pub glyph_flat_ranges: Vec<Range<u32>>,
    /// Per-effect-group contiguous ICON-instance ranges (parity Wave B3 — the
    /// glyph mirror), recomputed from `ExtractedIcons::entity_runs` ×
    /// `ExtractedNode.group`. The node's step-1 group pass draws each into the
    /// group's off-screen target via the same Glyph specialization.
    pub icon_group_ranges: Vec<Range<u32>>,
    /// The complement: maximal runs of non-group icon instances — the flat
    /// window icon draw covers exactly these. No live group ⇒ the single full
    /// `0..icon_count` run.
    pub icon_flat_ranges: Vec<Range<u32>>,
    /// Per-gradient-instance PAINT-ORDER anchors (parity gradient-bleed fix):
    /// `gradient_anchors[i]` is the quad-blob index just after gradient `i`'s
    /// node's own quad. CPU-side draw metadata (NOT uploaded — the byte-stable
    /// `GradientInstance` layout is untouched): the node draws the flat quad runs
    /// and the gradient blob INTERLEAVED by these so a node's gradient paints
    /// after its own fill and before its descendants' quads (an ancestor's
    /// gradient never overpaints a descendant's opaque fill). One entry per
    /// gradient instance, in node-walk (paint) order, so it is non-decreasing.
    /// Rides the quad gate (gradients + anchors are packed from the same node
    /// walk as the quad partition).
    pub gradient_anchors: Vec<u32>,
    /// #2 Stage D1: entity -> its quad-instance slot (`PackedPartition::quad_slot_of`),
    /// stored on every full quad repack. A subsequent Patch frame (stable paint order)
    /// reads it to overwrite only the changed entities' quad slots via `quad.set` +
    /// `write_buffer_range`, instead of re-uploading the whole blob.
    pub quad_slot_of: EntityHashMap<u32>,
    /// F4a: entity -> its `node_quad_anchor` (`PackedPartition::node_quad_anchor_of`),
    /// stored on every full quad repack. `node.rs`'s `build_raster_draws` joins each
    /// extracted raster (which knows only its entity) to its paint-order splice
    /// position through this, so a raster paints at its true stacking position.
    /// Retained across a Patch (a Patch never reorders — the anchors stay valid).
    pub node_quad_anchor_of: EntityHashMap<u32>,
    /// Glyph partial-reextract D6 guard: `true` while the glyph CPU mirror
    /// carries a degraded-group alpha-fold (`prepare_effect_groups` /
    /// `fold_degraded_groups` multiplied member alphas IN PLACE, diverging
    /// the mirror from `ExtractedGlyphs`). Cleared TWO ways: (1) the next full
    /// glyph repack from source (below), and (2) the un-degrade edge in
    /// `prepare_effect_groups`, which — when the degradation lifts on a
    /// glyph-CLEAN frame — rebuilds the mirror from `ExtractedGlyphs` and sets
    /// this to whether any glyph fold still applies (so a full un-degrade / group
    /// DROP clears it, a surviving degraded group keeps it true). The suffix
    /// ranged upload retains the mirror's prefix, so a folded prefix would
    /// freeze stale dimmed alpha on the GPU — the Patch fast path falls back to
    /// a full repack while this holds. The fold cannot run ON a Patch frame
    /// (Patch frames are provably effect-group-free: `write_effect_groups`
    /// re-inserts the `EffectGroup` marker every frame a former holds, so the
    /// classifier's `Changed<EffectGroup>` probe escalates every group-bearing
    /// dirty frame to Full); this flag covers the CROSS-frame residue — a fold
    /// on an earlier Full frame whose group dropped without a glyph-dirty frame
    /// in between (test-only in practice: nothing degrades at the default
    /// 64 MiB RT budget).
    pub glyph_mirror_folded: bool,
}

impl Default for BuiyInstanceBuffers {
    fn default() -> Self {
        Self {
            quad: RawBufferVec::new(BufferUsages::VERTEX),
            glyph: RawBufferVec::new(BufferUsages::VERTEX),
            icon: RawBufferVec::new(BufferUsages::VERTEX),
            band: RawBufferVec::new(BufferUsages::VERTEX),
            shadow: RawBufferVec::new(BufferUsages::VERTEX),
            gradient: RawBufferVec::new(BufferUsages::VERTEX),
            rounded_shadow: RawBufferVec::new(BufferUsages::VERTEX),
            view_uniform: UniformBuffer::default(),
            quad_count: 0,
            glyph_count: 0,
            icon_count: 0,
            band_count: 0,
            shadow_count: 0,
            gradient_count: 0,
            rounded_shadow_count: 0,
            group_ranges: Vec::new(),
            flat_ranges: Vec::new(),
            glyph_group_ranges: Vec::new(),
            glyph_flat_ranges: Vec::new(),
            icon_group_ranges: Vec::new(),
            icon_flat_ranges: Vec::new(),
            gradient_anchors: Vec::new(),
            quad_slot_of: EntityHashMap::default(),
            node_quad_anchor_of: EntityHashMap::default(),
            glyph_mirror_folded: false,
        }
    }
}

/// Observable render-world stat (the `RtPoolStats` idiom): cumulative
/// per-buffer GPU upload counts from [`prepare_buiy_instances`]. The
/// caret-blink GPU damage test (verification § 1.3) reads it through the
/// test harness to assert a blink frame re-uploads the glyph buffer ONLY
/// (decoration-and-paint § 6.3); `buiy-verification-design` may grow it
/// (byte counts, percentiles) for the gate-#14 budget wiring.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferUploadStats {
    /// Quad-buffer `write_buffer` calls (the quad gate fired).
    pub quad_uploads: u64,
    /// Glyph-buffer upload EVENTS — every glyph-dirty frame that refreshed
    /// the GPU draw state, whether via a full `write_buffer` or a suffix
    /// `write_buffer_range` (glyph partial-reextract D6). Counting both
    /// keeps the caret-blink pin's semantics stable ("a blink edge uploads
    /// the glyph buffer exactly once") across the Stage-D flip from full to
    /// ranged uploads; [`glyph_partial_uploads`](Self::glyph_partial_uploads)
    /// splits out the ranged ones.
    pub glyph_uploads: u64,
    /// #2 Stage D2: quad INSTANCES written this frame — `N` (the whole blob) on a Full
    /// repack, but only the changed-entity count on a pure-bg-quad Patch partial upload.
    /// The audit-#2 upload-rate signal: a single hover should move 1 instance, not N.
    pub instances_uploaded: u64,
    /// Glyph-buffer SUFFIX ranged uploads (glyph partial-reextract D6): the
    /// subset of [`glyph_uploads`](Self::glyph_uploads) that wrote only
    /// `[first_dirty_slot..len)` instead of the whole buffer. A
    /// `GlyphDamage::Patch` frame that hits any fallback (growth past GPU
    /// capacity, cold buffer, folded mirror, ranged-write error) counts in
    /// `glyph_uploads` only.
    pub glyph_partial_uploads: u64,
    /// Glyph INSTANCES written — the whole carrier length on a full upload,
    /// only the suffix length on a ranged one. The D6 upload-rate signal the
    /// GPU reftest gates on: a 1-entity mid-scene edit moves the suffix, not
    /// the full buffer.
    pub glyph_instances_uploaded: u64,
}

/// The ONE source of truth for "which tiers were repacked from source this
/// frame" (the H6 fix, `docs/reports/2026-06-30-mt-safety-followups.md` § H6):
/// [`prepare_buiy_instances`] publishes the per-tier dirty bits it ACTUALLY
/// used — overwritten unconditionally every run, so stale values never
/// persist — and `prepare_effect_groups` reads THIS instead of re-deriving
/// the same `is_changed()` gates from its own change ticks. The two systems
/// previously had to agree on two independent `is_changed()` evaluations
/// (correct only while order-pinned with nothing writable in between); the
/// published bits are immune to reorder/`run_if` drift by construction.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedDamage {
    /// The quad gate fired (nodes | groups | text quads changed) — the quad-
    /// family buffers were repacked from source this frame.
    pub quad_dirty: bool,
    /// The glyph gate fired (`ExtractedGlyphs` changed) — the glyph buffer was
    /// refreshed FROM SOURCE this frame. TRUE on glyph-Patch frames too
    /// (partial-reextract D6): the suffix ranged upload leaves mirror == GPU
    /// == carrier exactly like a full repack, so both consumers keep their
    /// contracts — `prepare_effect_groups`' alpha-fold gate reads "buffer
    /// state equals unfolded source, fold at most once" (moot on Patch
    /// frames, which are provably group-free — the fold can't run with no
    /// extracted groups), and the `partition_glyph_ranges` re-derivation
    /// must still run because a splice moves run boundaries.
    pub glyph_dirty: bool,
    /// The icon gate fired (`ExtractedIcons` changed) — the icon buffer was
    /// repacked from source this frame.
    pub icon_dirty: bool,
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
pub fn pack_extracted_nodes(nodes: &ExtractedNodes) -> (Vec<[f32; 17]>, [f32; 12]) {
    let buckets = pack_view(&nodes.nodes);
    let instances: Vec<[f32; 17]> = buckets
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
#[allow(clippy::too_many_arguments)]
pub fn prepare_buiy_instances(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    nodes: Res<ExtractedNodesView>,
    groups: Res<ExtractedEffectGroups>,
    glyphs: Res<ExtractedGlyphs>,
    icons: Res<ExtractedIcons>,
    text_quads: Res<ExtractedTextQuads>,
    // #2 Stage D2: the partial-upload inputs. `damage` says whether this frame was a
    // Patch + which entities; `index` maps an entity to its record slot in
    // `ExtractedNodesView` (to re-pack just that record).
    index: Res<RetainedNodeIndex>,
    damage: Res<NodeDamage>,
    // Glyph partial-reextract D6: the glyph tier's Full|Patch verdict + first
    // dirty slot, published by `extract_buiy_glyphs`. `Option` per the
    // `RetainedNodeIndex`/`RenderWorkCounters` precedent — a render setup
    // without the text plugin (which owns the registration) simply never
    // takes the glyph fast path.
    glyph_damage: Option<Res<GlyphDamage>>,
    mut buffers: ResMut<BuiyInstanceBuffers>,
    mut stats: ResMut<BufferUploadStats>,
    // H6 fix: the per-tier dirty bits this run uses, published for
    // `prepare_effect_groups` (see the `PreparedDamage` doc).
    mut prepared_damage: ResMut<PreparedDamage>,
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
    // The quad gate (§ 4.6): nodes OR groups OR text quads — text's
    // quad-tier visuals (underline/overline, T6) ride the SAME buffer, so a
    // decoration-only frame (e.g. a TextDecorations color edit: text probe
    // fires, node probe doesn't) must re-pack it. Quad and glyph buffers stay
    // INDEPENDENTLY gated — a caret blink (T7) re-uploads glyphs only.
    let quad_dirty = nodes.is_changed() || groups.is_changed() || text_quads.is_changed();
    let glyph_dirty = glyphs.is_changed();
    // Vector icons (parity Wave B3) are gated on their OWN carrier, independent of
    // the glyph gate: an accent-swatch re-tint re-extracts only `ExtractedIcons`,
    // so the icon buffer re-uploads without touching the glyph buffer.
    let icon_dirty = icons.is_changed();

    // H6 fix: publish the gate values this run ACTUALLY used, unconditionally
    // (an overwrite every run, so a stale bit can never persist into a later
    // frame). `prepare_effect_groups` reads these instead of re-deriving the
    // same `is_changed()` gates — the one source of truth (`PreparedDamage`).
    *prepared_damage = PreparedDamage {
        quad_dirty,
        glyph_dirty,
        icon_dirty,
    };

    // #2 Stage D2: the partial-upload fast path. When extract published a Patch (a
    // group-free, footprint-stable value change) and the ONLY dirty carrier is the node
    // quad blob, overwrite just the changed entities' quad slots in place + upload only
    // the spanned range — instead of repacking + re-uploading the whole O(N) blob. Only
    // PURE bg-quad nodes qualify: their bg quad is their sole instance across every
    // buffer (no band/shadow/gradient, no own text quads), so a single `set` is correct
    // for any value change. Anything else falls through to the Full repack below.
    let partial_done = if quad_dirty {
        'partial: {
            let NodeDamage::Patch(ents) = &*damage else {
                break 'partial false;
            };
            if ents.is_empty()
                || groups.is_changed()
                || text_quads.is_changed()
                || glyphs.is_changed()
                || icons.is_changed()
            {
                break 'partial false;
            }
            // Entities whose OWN text quads live (interleaved) in the quad buffer — a
            // value change would move those too, so they are not pure-bg-quad.
            let text_ents: EntityHashSet = text_quads.quads.iter().map(|q| q.entity).collect();
            let mut slots: Vec<u32> = Vec::with_capacity(ents.len());
            for &e in ents {
                let (Some(&qslot), Some(&rslot)) = (buffers.quad_slot_of.get(&e), index.0.get(&e))
                else {
                    break 'partial false;
                };
                let Some(rec) = nodes.0.nodes.get(rslot as usize) else {
                    break 'partial false;
                };
                let pure_bg_quad = rec.color != Color::NONE
                    && rec.border.is_none()
                    && rec.shadows.is_empty()
                    && rec.gradients.is_empty()
                    && rec.outline.is_none()
                    && !text_ents.contains(&e);
                if !pure_bg_quad {
                    break 'partial false;
                }
                slots.push(qslot);
            }
            // Overwrite each changed entity's quad slot in place (siblings untouched).
            for (&e, &qslot) in ents.iter().zip(slots.iter()) {
                let rslot = index.0[&e] as usize;
                let raw = packed_to_raw(&pack_extracted(&nodes.0.nodes[rslot]));
                debug_assert!(qslot < buffers.quad_count, "#2 D2: patch slot in range");
                buffers.quad.set(qslot, raw);
            }
            // Upload only the spanned element range (one slot for a single-entity hover;
            // any unchanged slots inside the span keep their prior, still-correct values).
            let lo = *slots.iter().min().unwrap() as usize;
            let hi = *slots.iter().max().unwrap() as usize;
            if buffers
                .quad
                .write_buffer_range(&render_queue, lo..hi + 1)
                .is_err()
            {
                break 'partial false; // uninitialised/cold buffer -> fall back to Full
            }
            stats.quad_uploads += 1;
            stats.instances_uploaded += ents.len() as u64;
            true
        }
    } else {
        false
    };

    if quad_dirty && !partial_done {
        // Consume R5's ExtractedNodes: pack its per-view records into the flat
        // quad blob, the per-group instance-range partition, and build the view
        // uniform (logical_size + scale_factor are R5's). The view uniform rides
        // the quad gate because R5's `ExtractedNodes` carries the logical_size/
        // scale_factor it is built from. The partition keys off `ExtractedNode.group`
        // (effect-compositor.md § 1.1): each group's contiguous range renders into
        // its own off-screen target (the node's step 1), the flat ranges into the
        // window — so a group member is never double-painted. Text quads splice
        // in by the § 4.6 fresh-node-list walk (each entity's quads land right
        // after its node instance, adopting its group).
        let mut partition =
            pack_view_partitioned(&nodes.0.nodes, groups.0.len(), &text_quads.quads);
        let uniform =
            BuiyViewUniform::for_view(nodes.0.logical_size, nodes.0.scale_factor).as_std140_array();
        // D1: cache entity -> quad slot from this full pack so a later Patch frame (D2)
        // can overwrite just the changed slots. Additive — stored but unused until D2.
        buffers.quad_slot_of = std::mem::take(&mut partition.quad_slot_of);
        // F4a: cache entity -> paint-order anchor from this full pack so
        // `build_raster_draws` can splice each raster at its stacking position.
        buffers.node_quad_anchor_of = std::mem::take(&mut partition.node_quad_anchor_of);

        // Repack the quad buffer in place: clear + extend (the Vec backing
        // grows; the GPU buffer grows only on capacity overflow).
        buffers.quad.clear();
        for inst in &partition.instances {
            buffers.quad.push(*inst);
        }
        buffers.quad_count = partition.instances.len() as u32;
        buffers.quad.write_buffer(&render_device, &render_queue);
        stats.quad_uploads += 1;
        stats.instances_uploaded += partition.instances.len() as u64;
        // When NO group is live, the whole buffer is the flat draw — `pack_view_
        // partitioned` returns it as the single non-group run, so the node's flat
        // path stays byte-for-byte the pre-compositor draw.
        buffers.group_ranges = partition.group_ranges;
        buffers.flat_ranges = partition.flat_ranges;

        // Border/outline band buffer (C6-a outline + C6-b per-side border).
        // Packed from the SAME node walk, so it rides the quad gate; a node with
        // no border/outline contributes nothing, so a band-free frame uploads an
        // empty band buffer (band_count = 0) and the node skips the band draw.
        // The band draws flat (after quad/glyph) and is NOT effect-group-
        // partitioned in v1 (styling-f-tier.md § 2.3).
        let (bands, _band_top_layer_boundary) = pack_band_instances(&nodes.0.nodes);
        buffers.band.clear();
        for band in &bands {
            buffers.band.push(*band);
        }
        buffers.band_count = bands.len() as u32;
        buffers.band.write_buffer(&render_device, &render_queue);

        // Box-shadow buffer (C6-b). Packed from the SAME node walk (it rides the
        // quad gate); a node with no shadow — or every shadow suppressed under
        // forced-colors — contributes nothing, so a shadow-free frame uploads an
        // empty buffer (shadow_count = 0) and the node skips the shadow draw. The
        // shadow reuses the 68 B `[f32; 17]` quad layout (radius → blur sigma), so
        // it shares the `packed_to_raw` flatten. Drawn FIRST in `node.rs` (before
        // the quad), so a shadow paints BEHIND its caster.
        // The boundary is retained on `buffers.top_layer` in Task 1.6 (the
        // per-block draw consumes it in W2); dropped here to keep this task's
        // signature change compiling without the retention plumbing.
        let (shadows, _shadow_top_layer_boundary) = pack_shadow_instances(&nodes.0.nodes);
        buffers.shadow.clear();
        for shadow in &shadows {
            buffers
                .shadow
                .push(crate::render::buckets::packed_to_raw(shadow));
        }
        buffers.shadow_count = shadows.len() as u32;
        buffers.shadow.write_buffer(&render_device, &render_queue);

        // Rounded box-shadow buffer (F4b-6). Packed from the SAME node walk (it
        // rides the quad gate); only a shadow term of a ROUNDED caster (radius > 0)
        // lands here — a square caster's terms went to `shadow` above — so a scene
        // with no rounded shadow uploads an empty buffer (rounded_shadow_count = 0)
        // and the node skips the draw. Its OWN `RoundedShadowInstance` layout (the
        // 68 B quad stride + square-shadow path are untouched). Drawn in the SHADOW
        // tier in `node.rs`, alongside the square shadow blob.
        let (rounded_shadows, _rounded_shadow_top_layer_boundary) =
            pack_rounded_shadow_instances(&nodes.0.nodes);
        buffers.rounded_shadow.clear();
        for rs in &rounded_shadows {
            buffers.rounded_shadow.push(*rs);
        }
        buffers.rounded_shadow_count = rounded_shadows.len() as u32;
        buffers
            .rounded_shadow
            .write_buffer(&render_device, &render_queue);

        // Background-gradient buffer (parity Wave B1). Packed from the SAME node
        // walk (it rides the quad gate); a node with no gradient layers
        // contributes nothing, so a gradient-free frame uploads an empty buffer
        // (gradient_count = 0) and the node skips the gradient draw. Its OWN
        // `GradientInstance` layout (the 68 B quad stride is untouched). The
        // `anchors` are the per-instance PAINT-ORDER positions (the quad-blob
        // index after each gradient's node's own quad — `partition.node_quad_anchors`
        // from the SAME walk): `node.rs` interleaves the gradient blob with the
        // flat quad runs by these, so a node's gradient paints over its own fill
        // and BEFORE its descendants' quads (an ancestor gradient never
        // overpaints a descendant's opaque fill). Anchors are CPU-side draw
        // metadata, not uploaded (the byte-stable layout is untouched).
        let (gradients, gradient_anchors) =
            pack_gradient_instances(&nodes.0.nodes, &partition.node_quad_anchors);
        buffers.gradient.clear();
        for gradient in &gradients {
            buffers.gradient.push(*gradient);
        }
        buffers.gradient_count = gradients.len() as u32;
        buffers.gradient.write_buffer(&render_device, &render_queue);
        buffers.gradient_anchors = gradient_anchors;

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
    //
    // Glyph partial-reextract D6: the suffix ranged fast path. When extract
    // EXECUTED a Patch (splices confined to resident entities) it published
    // the first dirty instance slot — the prefix `[0..slot)` is byte-identical
    // to the previously uploaded carrier by construction — so the CPU mirror
    // keeps its prefix (truncate + re-push the suffix from the carrier) and
    // ONE `write_buffer_range(slot..len)` refreshes the GPU. Splice semantics
    // make this a SUFFIX, not a slot overwrite (run lengths change on the
    // flagship triggers — typing, caret blink), unlike the node tier's
    // fixed-slot D2 path above.
    //
    // Every fallback runs the full clear+push+`write_buffer` repack below:
    //  • `GlyphDamage::Full` (or the resource unregistered / no slot
    //    published) — whole-set damage by definition;
    //  • growth past the GPU buffer's capacity — `write_buffer_range` cannot
    //    grow a buffer; the full path's `reserve` recreates it (an EXPECTED
    //    path on any net-growth edit, so no warn);
    //  • a cold/uninitialized buffer (first dirty frame in a fresh app) —
    //    likewise expected, no warn;
    //  • a shorter-than-prefix mirror (defensive: the mirror should always
    //    hold last frame's carrier here);
    //  • the group-free premise violated, or a degraded-group fold left the
    //    mirror diverged from the carrier (`glyph_mirror_folded`) — see the
    //    debug_assert below;
    //  • a `write_buffer_range` error after the guards (unreachable by
    //    inspection — kept as a loud belt-and-suspenders).
    let glyph_partial_done = if glyph_dirty {
        'glyph_partial: {
            let Some(GlyphDamage::Patch {
                first_dirty_slot: Some(first_dirty),
                ..
            }) = glyph_damage.as_deref()
            else {
                break 'glyph_partial false;
            };
            let first_dirty = *first_dirty as usize;
            let new_len = glyphs.glyphs.len();
            // The group-free premise (D6): `write_effect_groups` re-inserts
            // the `EffectGroup` marker every frame a former holds, so the
            // extract classifier's `Changed<EffectGroup>` probe escalates
            // every group-bearing dirty frame to Full — a Patch frame can
            // carry no live group, hence no degraded-group alpha-fold can
            // touch glyph ranges this frame.
            debug_assert!(
                groups.0.is_empty(),
                "GlyphDamage::Patch on a frame with live effect groups — the \
                 extract classifier's Changed<EffectGroup> probe should have \
                 escalated this frame to Full (glyph partial-reextract D6)"
            );
            if !groups.0.is_empty() {
                bevy::log::warn_once!(
                    "buiy: glyph Patch frame with live effect groups — the \
                     D6 group-free premise is violated; falling back to the \
                     full glyph upload (warned once)"
                );
                break 'glyph_partial false;
            }
            // Cross-frame fold residue: a degraded-group fold on an EARLIER
            // frame diverged the mirror's prefix from the carrier — only a
            // full repack (which clears the flag) may run until then.
            if buffers.glyph_mirror_folded {
                break 'glyph_partial false;
            }
            if first_dirty > new_len
                || first_dirty > buffers.glyph.len()
                || new_len > buffers.glyph.capacity()
                || buffers.glyph.buffer().is_none()
            {
                break 'glyph_partial false; // growth / cold / short mirror
            }
            // Mirror update: retain the byte-identical prefix, re-push the
            // suffix from the carrier — the mirror equals the carrier after
            // this (the same end state as the full path's clear+push).
            buffers.glyph.truncate(first_dirty);
            for inst in &glyphs.glyphs[first_dirty..] {
                buffers.glyph.push(*inst);
            }
            debug_assert_eq!(
                buffers.glyph.len(),
                new_len,
                "D6: the patched mirror must equal the carrier"
            );
            // A pure tail-shrink publishes `first_dirty == new_len`: nothing
            // to write — the retained prefix IS the new draw range and
            // `glyph_count` below shortens the instanced draw.
            if first_dirty < new_len
                && buffers
                    .glyph
                    .write_buffer_range(&render_queue, first_dirty..new_len)
                    .is_err()
            {
                bevy::log::warn_once!(
                    "buiy: glyph suffix write_buffer_range failed after the \
                     capacity/cold guards; falling back to the full glyph \
                     upload (warned once)"
                );
                break 'glyph_partial false;
            }
            buffers.glyph_count = new_len as u32;
            stats.glyph_uploads += 1;
            stats.glyph_partial_uploads += 1;
            stats.glyph_instances_uploaded += (new_len - first_dirty) as u64;
            true
        }
    } else {
        false
    };

    if glyph_dirty && !glyph_partial_done {
        buffers.glyph.clear();
        for inst in &glyphs.glyphs {
            buffers.glyph.push(*inst);
        }
        buffers.glyph_count = glyphs.glyphs.len() as u32;
        buffers.glyph.write_buffer(&render_device, &render_queue);
        // A full repack restores mirror == carrier, repairing any
        // degraded-group fold residue (see `glyph_mirror_folded`).
        buffers.glyph_mirror_folded = false;
        stats.glyph_uploads += 1;
        stats.glyph_instances_uploaded += glyphs.glyphs.len() as u64;
    }

    // Icon buffer (parity Wave B3 — the SAME coverage primitive as glyphs, drawn
    // through a separate buffer so the two producers stay decoupled). Gated on
    // `ExtractedIcons` independently of glyphs/quads, so an accent-swatch re-tint
    // re-uploads icons ONLY.
    if icon_dirty {
        buffers.icon.clear();
        for inst in &icons.icons {
            buffers.icon.push(*inst);
        }
        buffers.icon_count = icons.icons.len() as u32;
        buffers.icon.write_buffer(&render_device, &render_queue);
    }

    // T8 (D1/D2): derive the glyph partition from the FRESH node list — group
    // membership is the entity's ExtractedNode.group (the § 4.6 discipline;
    // never recorded into the carrier). Recompute under the UNION: a group
    // can form/drop on a node-only frame while glyphs are retained
    // (Changed<EffectGroup>/<Opacity> ride the nodes probe), and a glyph-only
    // rebuild (a caret blink) moves the run boundaries. CPU-only — no upload
    // rides this branch, so the independent buffer gating (and the
    // blink-reuploads-glyphs-only property) is untouched.
    if (quad_dirty && !partial_done) || glyph_dirty {
        let group_count = groups.0.len();
        let group_by_entity: HashMap<Entity, Option<usize>> =
            nodes.0.nodes.iter().map(|n| (n.entity, n.group)).collect();
        // The top-layer stacking composite (§ 3.2): the parallel entity→top_layer
        // map, off the fresh node list's `ExtractedNode.top_layer` (mirroring
        // `group_by_entity`). The boundary is retained on `buffers.top_layer` in
        // Task 1.6; dropped here so the closure lands without the retention.
        let top_layer_by_entity: HashMap<Entity, bool> =
            nodes.0.nodes.iter().map(|n| (n.entity, n.top_layer)).collect();
        let (group_ranges, flat_ranges, _glyph_top_layer_boundary) = partition_glyph_ranges(
            glyphs
                .entity_runs
                .iter()
                .map(|r| (r.entity, r.instances.clone())),
            // The carrier's count, not `buffers.glyph_count` — always
            // consistent with `entity_runs` even on a quad-dirty-only frame.
            glyphs.glyphs.len() as u32,
            group_count,
            |e| group_by_entity.get(&e).copied().flatten(),
            |e| top_layer_by_entity.get(&e).copied().unwrap_or(false),
        );
        buffers.glyph_group_ranges = group_ranges;
        buffers.glyph_flat_ranges = flat_ranges;
    }

    // Icon partition (parity Wave B3 — the glyph-partition mirror over the icon
    // carrier's own entity_runs). Recompute under the icon-or-quad union: a group
    // can form/drop on a node-only frame while icons are retained, and an
    // icon-only rebuild moves run boundaries. `ExtractedIcons::entity_runs` is its
    // OWN contiguous-from-0 source, so the partition's contiguity assert holds.
    if (quad_dirty && !partial_done) || icon_dirty {
        let group_count = groups.0.len();
        let group_by_entity: HashMap<Entity, Option<usize>> =
            nodes.0.nodes.iter().map(|n| (n.entity, n.group)).collect();
        // Icon mirror of the glyph partition's parallel top-layer map (§ 3.2). The
        // boundary is retained on `buffers.top_layer` in Task 1.6.
        let top_layer_by_entity: HashMap<Entity, bool> =
            nodes.0.nodes.iter().map(|n| (n.entity, n.top_layer)).collect();
        let (group_ranges, flat_ranges, _icon_top_layer_boundary) = partition_glyph_ranges(
            icons
                .entity_runs
                .iter()
                .map(|r| (r.entity, r.instances.clone())),
            icons.icons.len() as u32,
            group_count,
            |e| group_by_entity.get(&e).copied().flatten(),
            |e| top_layer_by_entity.get(&e).copied().unwrap_or(false),
        );
        buffers.icon_group_ranges = group_ranges;
        buffers.icon_flat_ranges = flat_ranges;
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
