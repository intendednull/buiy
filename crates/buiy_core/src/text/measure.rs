//! The Taffy measure seam (measure-and-layout §§ 3–4.3): the measure
//! protocol, the intrinsics cache fill, and the one shared compute helper
//! all three layout compute sites call.
//!
//! **Lock discipline (architecture § 1.2 site #1; measure § 3.4):** ONE
//! `SharedFontSystem::lock()` per [`compute_roots_with_text_measure`]
//! invocation, scoped to its body; the closure reborrows the held guard as
//! `&mut FontSystem` and must NEVER lock the resource itself —
//! `std::sync::Mutex` is not reentrant, a nested lock self-deadlocks. The
//! helper runs at most twice per frame (`taffy_compute` + at most one cq
//! re-run — `CqFlipReRanThisFrame` makes sites 2/3 mutually exclusive);
//! each invocation takes and releases the lock independently. The closure
//! is rebuilt per call from current world state, holds no cross-call
//! state, and never issues `Commands` — cq re-entrancy is then free
//! (measure § 4.3).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use cosmic_text::FontSystem;
use taffy::{AvailableSpace, NodeId, Size as TaffySize};

use crate::layout::{BoxModel, LayoutTaffyComputeCount, LayoutTree, Sizing};

use super::components::{IntrinsicWidths, TextBuffer};
use super::font_system::SharedFontSystem;

/// Per-frame count of measure-closure invocations on text leaves (measure
/// § 7 — the `SyncStylesIterCount` precedent). Reset by `taffy_compute`
/// at frame start; the cq re-run sites increment without resetting
/// (mirroring `LayoutTaffyComputeCount`). `tests/text_commit.rs` asserts
/// ZERO on a no-change frame — Taffy's cache holds and the edge-triggered
/// context registration holds (measure § 2.2).
#[derive(Resource, Default, Debug)]
pub struct TextMeasureCallCount(pub usize);

/// The text inputs each compute site threads into the helper — one
/// `SystemParam` so every site grows by exactly one parameter.
/// `fonts`/`call_count` are `Option`: `LayoutPlugin` without
/// `BuiyTextPlugin` (the standing layout tests) has neither resource and
/// degrades to the plain zero-measure compute.
#[derive(SystemParam)]
pub struct TextMeasureParam<'w, 's> {
    fonts: Option<Res<'w, SharedFontSystem>>,
    buffers: Query<'w, 's, (&'static mut TextBuffer, Option<&'static BoxModel>)>,
    call_count: Option<ResMut<'w, TextMeasureCallCount>>,
}

impl TextMeasureParam<'_, '_> {
    /// Frame-start reset (called by `taffy_compute` only — the
    /// `LayoutTaffyComputeCount` reset pattern).
    pub(crate) fn reset_call_count(&mut self) {
        if let Some(count) = self.call_count.as_deref_mut() {
            count.0 = 0;
        }
    }
}

/// The one compute helper, three sites (measure § 4.3): replaces plain
/// `compute_layout` at `taffy_compute`, `cq_flip_rerun`, and
/// `cq_descendant_rerun`. Measure adds ZERO extra Taffy passes — it rides
/// whatever passes layout already runs; `LayoutTaffyComputeCount`
/// semantics are unchanged (reset stays in `taffy_compute` alone).
pub(crate) fn compute_roots_with_text_measure(
    tree: &mut LayoutTree,
    measure: &mut TextMeasureParam,
    window_size: Vec2,
    roots: &[(Entity, NodeId)],
    compute_count: &mut LayoutTaffyComputeCount,
    site: &'static str,
) {
    let available = TaffySize {
        width: AvailableSpace::Definite(window_size.x),
        height: AvailableSpace::Definite(window_size.y),
    };
    // Lock site #1: one lock per invocation, guard scoped to this body,
    // dropped before return. Never stored. (See the module doc.)
    let mut guard = measure.fonts.as_ref().map(|fonts| fonts.lock());
    let buffers = &mut measure.buffers;
    let mut calls = 0usize;
    for &(entity, node) in roots {
        let result = match guard.as_deref_mut() {
            Some(font_system) => tree.tree.compute_layout_with_measure(
                node,
                available,
                |known_dimensions, available_space, _node_id, node_context, _style| {
                    let Some(&mut text_entity) = node_context else {
                        // Childless non-text leaf: no context registered.
                        return TaffySize::ZERO;
                    };
                    calls += 1;
                    measure_text_node(
                        font_system,
                        buffers,
                        text_entity,
                        known_dimensions,
                        available_space,
                    )
                },
            ),
            // No engine ⇒ no text entities can exist either.
            None => tree.tree.compute_layout(node, available),
        };
        match result {
            Ok(()) => compute_count.0 += 1,
            Err(err) => {
                warn!(
                    ?entity,
                    ?err,
                    "buiy: layout compute_layout ({}) failed",
                    site
                );
            }
        }
    }
    drop(guard);
    if let Some(count) = measure.call_count.as_deref_mut() {
        count.0 += calls;
    }
}

/// One measure invocation (measure § 3.2). Taffy already subtracted the
/// content-box inset from `available_space` and adds it back to the
/// return (leaf.rs:111–146) — no BoxModel math here. Under
/// `RunMode::PerformLayout` taffy zeroes `known_dimensions` and passes
/// the resolved width as `AvailableSpace::Definite`, so the fold's
/// Definite arm answers at layout time.
fn measure_text_node(
    font_system: &mut FontSystem,
    buffers: &mut Query<(&'static mut TextBuffer, Option<&'static BoxModel>)>,
    entity: Entity,
    known_dimensions: TaffySize<Option<f32>>,
    available_space: TaffySize<AvailableSpace>,
) -> TaffySize<f32> {
    let Ok((mut text, box_model)) = buffers.get_mut(entity) else {
        // The context outlived its TextBuffer within this frame (the
        // removal edge races the compute): measure as empty; the cleared
        // context lands at the next sync point.
        return TaffySize::ZERO;
    };
    // measure § 7: a width probe is not a semantic change — never tick.
    let text = text.bypass_change_detection();
    let intrinsics = cached_intrinsics(text, font_system);
    // § 3.3 — width-axis intrinsic keywords answer from the cache
    // regardless of the probe. A parent-resolved known width still
    // overrides the measured size downstream (leaf.rs:143–146) — the
    // documented under-stretch fidelity limit. `FitContent` and
    // height-axis keywords stay auto-equivalent (named deferrals).
    let keyword_width = box_model.and_then(|bm| match bm.width {
        Sizing::MinContent => Some(intrinsics.min_content),
        Sizing::MaxContent => Some(intrinsics.max_content),
        _ => None,
    });
    let width = known_dimensions
        .width
        .or(keyword_width)
        .unwrap_or(match available_space.width {
            AvailableSpace::MinContent => intrinsics.min_content,
            AvailableSpace::MaxContent => intrinsics.max_content,
            AvailableSpace::Definite(w) => w,
        });
    // The definite-width relayout: `set_size` invalidates only
    // `layout_opt` (per-line `shape_opt` survives — the amortization the
    // protocol rides, § 3.2). Height stays None: measure never crops, and
    // the None is the catch-all signal TextCommit uses to recognize a
    // probe-left buffer (commit always sets Some — decision 7).
    text.buffer.set_size(Some(width), None);
    text.buffer.shape_until_scroll(font_system, false);
    let (max_w, total_h) = fold_runs(&text.buffer);
    // Ceil: taffy's whole-px rounding must never round the final box
    // below the measured content (a <1px deficit re-wraps the last word
    // at commit — the bevy_text precedent; decision 5).
    TaffySize {
        width: max_w.ceil(),
        height: total_h.ceil(),
    }
}

/// (max line_w, Σ line_height) over the laid-out runs (§ 3.2's fold).
fn fold_runs(buffer: &cosmic_text::Buffer) -> (f32, f32) {
    buffer
        .layout_runs()
        .fold((0.0_f32, 0.0_f32), |(w, h), run| {
            (w.max(run.line_w), h + run.line_height)
        })
}

/// Serve or fill the per-content-version intrinsics cache (§ 3.2):
/// min-content via width-0 layout (every wrap opportunity breaks; under
/// `Wrap::None` nothing breaks ⇒ min == max — the CSS nowrap behavior),
/// max-content via unconstrained layout. `TextSync` invalidates on every
/// content change — that invalidation IS the cache key.
fn cached_intrinsics(text: &mut TextBuffer, font_system: &mut FontSystem) -> IntrinsicWidths {
    if let Some(cached) = text.intrinsics() {
        return cached;
    }
    let buffer = &mut text.buffer;
    buffer.set_size(Some(0.0), None);
    buffer.shape_until_scroll(font_system, false);
    let min_content = fold_runs(buffer).0;
    buffer.set_size(None, None);
    buffer.shape_until_scroll(font_system, false);
    let max_content = fold_runs(buffer).0;
    let widths = IntrinsicWidths {
        min_content,
        max_content,
    };
    text.cache_intrinsics(widths);
    widths
}
