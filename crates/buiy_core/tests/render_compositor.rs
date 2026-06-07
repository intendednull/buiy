//! Headless unit tests for the off-screen effect-group compositor's
//! prepare-phase math (effect-compositor.md § 2). Pure CPU — no wgpu adapter.

use bevy::prelude::*;
use buiy_core::render::compositor::{
    EffectReason, InkExpansion, PreparedEffectGroup, RT_POOL_BUDGET_BYTES, bucket_extent,
    composite_src_over, painted_bounds, plan_allocation, post_order_indices, target_bytes,
};

#[test]
fn effect_reason_bits_match_spec() {
    // effect-compositor.md § 1.1 / component-model.md § 10.
    assert_eq!(EffectReason::OPACITY.bits(), 1);
    assert_eq!(EffectReason::ISOLATION.bits(), 2);
    assert_eq!(EffectReason::FILTER.bits(), 4);
    assert_eq!(EffectReason::BACKDROP_FILTER.bits(), 8);
    assert_eq!(EffectReason::MIX_BLEND.bits(), 16);
}

#[test]
fn effect_reason_composes_opacity_and_isolation() {
    let r = EffectReason::OPACITY | EffectReason::ISOLATION;
    assert!(r.contains(EffectReason::OPACITY));
    assert!(r.contains(EffectReason::ISOLATION));
    assert!(!r.contains(EffectReason::FILTER));
}

#[test]
fn prepared_group_carries_index_opacity_reason() {
    let g = PreparedEffectGroup {
        index: 3,
        parent: None,
        bounds: Rect::from_corners(Vec2::ZERO, Vec2::splat(10.0)),
        extent: UVec2::new(16, 16),
        opacity: 0.5,
        reason: EffectReason::OPACITY,
    };
    assert_eq!(g.index, 3);
    assert_eq!(g.parent, None);
    assert!((g.opacity - 0.5).abs() < 1e-6);
    assert_eq!(g.extent, UVec2::new(16, 16));
}

#[test]
fn painted_bounds_union_root_and_descendants() {
    // Root box plus a descendant that overflows to the right/bottom.
    let root = Rect::from_corners(Vec2::ZERO, Vec2::new(20.0, 20.0));
    let descendants = [Rect::from_corners(
        Vec2::new(10.0, 10.0),
        Vec2::new(40.0, 30.0),
    )];
    let b = painted_bounds(root, &descendants, &[], None);
    assert_eq!(b.min, Vec2::ZERO);
    assert_eq!(b.max, Vec2::new(40.0, 30.0));
}

#[test]
fn painted_bounds_grows_by_ink_outset_shadow_and_outline() {
    let root = Rect::from_corners(Vec2::ZERO, Vec2::splat(24.0));
    // shadow: blur 4 + spread 2 = 6 outset; outline width 1 + offset 1 = 2.
    let ink = [
        InkExpansion {
            margin: 6.0,
            around: root,
        },
        InkExpansion {
            margin: 2.0,
            around: root,
        },
    ];
    let b = painted_bounds(root, &[], &ink, None);
    // Largest ink margin (6) expands the box on every side.
    assert_eq!(b.min, Vec2::splat(-6.0));
    assert_eq!(b.max, Vec2::splat(30.0));
}

#[test]
fn painted_bounds_clips_to_group_clip_rect_last() {
    let root = Rect::from_corners(Vec2::ZERO, Vec2::splat(100.0));
    let descendants = [Rect::from_corners(Vec2::splat(-50.0), Vec2::splat(200.0))];
    let clip = Rect::from_corners(Vec2::splat(10.0), Vec2::splat(60.0));
    let b = painted_bounds(root, &descendants, &[], Some(clip));
    // Clipped descendants cannot enlarge the target beyond the clip.
    assert_eq!(b.min, Vec2::splat(10.0));
    assert_eq!(b.max, Vec2::splat(60.0));
}

#[test]
fn painted_bounds_inset_outline_offset_does_not_grow() {
    // Outline width 1 + offset -2 = max(0, -1) = 0: no growth.
    let root = Rect::from_corners(Vec2::ZERO, Vec2::splat(10.0));
    let ink = [InkExpansion {
        margin: 0.0,
        around: root,
    }];
    let b = painted_bounds(root, &[], &ink, None);
    assert_eq!(b.min, Vec2::ZERO);
    assert_eq!(b.max, Vec2::splat(10.0));
}

#[test]
fn bucket_rounds_each_axis_to_next_pow2() {
    // 24x24 logical at scale 1 -> 24x24 physical -> 32x32 bucket.
    let e = bucket_extent(Vec2::splat(24.0), 1.0, UVec2::new(1920, 1080));
    assert_eq!(e, UVec2::new(32, 32));
}

#[test]
fn bucket_folds_scale_factor_before_rounding() {
    // 24x24 logical at 2x -> 48x48 physical -> 64x64 bucket.
    let e = bucket_extent(Vec2::splat(24.0), 2.0, UVec2::new(3840, 2160));
    assert_eq!(e, UVec2::new(64, 64));
}

#[test]
fn bucket_caps_at_view_size_not_next_pow2_past_it() {
    // A near-viewport group (1900 wide, view 1920) caps at 1920 on x,
    // NOT 2048 — one stable bucket shared by all overflowing groups (§ 2.2).
    let e = bucket_extent(Vec2::new(1900.0, 40.0), 1.0, UVec2::new(1920, 1080));
    assert_eq!(e.x, 1920);
    assert_eq!(e.y, 64);
}

#[test]
fn bucket_caps_when_bounds_exceed_view() {
    // Bounds larger than view on both axes cap to the view dimensions.
    let e = bucket_extent(Vec2::new(5000.0, 4000.0), 1.0, UVec2::new(1920, 1080));
    assert_eq!(e, UVec2::new(1920, 1080));
}

#[test]
fn bucket_never_zero() {
    // A degenerate empty bound still yields at least a 1x1 target.
    let e = bucket_extent(Vec2::ZERO, 1.0, UVec2::new(800, 600));
    assert_eq!(e, UVec2::new(1, 1));
}

#[test]
fn post_order_places_children_before_parents() {
    // Forest by parent link (None == root group). Group 0 is parent of 1 and 2.
    //   0
    //   ├─ 1
    //   └─ 2 ── 3
    let parents = [None, Some(0usize), Some(0usize), Some(2usize)];
    let order = post_order_indices(&parents);
    let pos = |g: usize| order.iter().position(|&x| x == g).unwrap();
    // Every child precedes its parent.
    assert!(pos(1) < pos(0));
    assert!(pos(2) < pos(0));
    assert!(pos(3) < pos(2));
    assert_eq!(order.len(), 4);
}

#[test]
fn post_order_handles_multiple_roots() {
    // Two independent groups, no nesting.
    let parents = [None, None];
    let order = post_order_indices(&parents);
    assert_eq!(order.len(), 2);
    assert!(order.contains(&0));
    assert!(order.contains(&1));
}

#[test]
fn post_order_deep_chain_is_innermost_first() {
    // 0 -> 1 -> 2 -> 3 (3 deepest). Post-order: 3,2,1,0.
    let parents = [None, Some(0usize), Some(1usize), Some(2usize)];
    let order = post_order_indices(&parents);
    assert_eq!(order, vec![3, 2, 1, 0]);
}

#[test]
fn budget_default_is_64_mib() {
    assert_eq!(RT_POOL_BUDGET_BYTES, 64 * 1024 * 1024);
}

#[test]
fn target_bytes_is_area_times_eight_for_rgba16float() {
    // Rgba16Float = 8 bytes/texel.
    assert_eq!(target_bytes(UVec2::new(64, 64)), 64 * 64 * 8);
}

#[test]
fn plan_allocation_keeps_all_under_budget() {
    // Two small groups well under budget: both composite off-screen.
    let groups = [
        (UVec2::new(32, 32), EffectReason::OPACITY),
        (UVec2::new(64, 64), EffectReason::ISOLATION),
    ];
    let plan = plan_allocation(&groups, RT_POOL_BUDGET_BYTES);
    assert_eq!(plan, vec![true, true]);
}

#[test]
fn plan_allocation_degrades_lowest_cost_opacity_first_when_over_budget() {
    // Budget only fits the big isolation group; the small opacity group
    // degrades to forward compositing (lowest-cost, OPACITY-only first).
    let big = UVec2::new(2048, 2048); // 2048*2048*8 = 32 MiB
    let small = UVec2::new(32, 32); // 8 KiB
    let groups = [
        (small, EffectReason::OPACITY),
        (big, EffectReason::ISOLATION),
    ];
    let budget = target_bytes(big); // exactly fits the isolation group only
    let plan = plan_allocation(&groups, budget);
    assert!(!plan[0], "small opacity-only group degrades first");
    assert!(plan[1], "structural isolation group keeps its target");
}

#[test]
fn plan_allocation_isolation_degrades_last() {
    // Two groups both individually fit, together exceed budget. The
    // OPACITY-only group yields before the ISOLATION group (§ 2.3 ranking).
    let a = UVec2::new(1024, 1024); // 8 MiB, OPACITY
    let b = UVec2::new(1024, 1024); // 8 MiB, ISOLATION
    let groups = [(a, EffectReason::OPACITY), (b, EffectReason::ISOLATION)];
    let budget = target_bytes(a) + target_bytes(b) / 2; // fits one, not both
    let plan = plan_allocation(&groups, budget);
    assert!(!plan[0]);
    assert!(plan[1]);
}

#[test]
fn plan_allocation_keeps_large_structural_and_degrades_small_opacity_only() {
    // Strict-priority eviction (§ 2.3), NOT a utilization-maximizing bin-pack:
    // the budget holds the large structural ISOLATION group OR the small
    // OPACITY-only group, but not both. A greedy "keep highest priority that
    // fits, then fill remaining budget" would keep the structural group and
    // then ALSO fit the tiny opacity-only one only if room remained — but the
    // failure mode is the inverse case: when the structural group alone nearly
    // fills the budget, a fit-the-front greedy that processes the structural
    // group first leaves no room, then a degrade-first policy must drop the
    // OPACITY-only group, never the structural one. Strict priority degrades
    // the opacity-only group FIRST, so the large structural group keeps its
    // target and the small opacity-only group degrades — the structural
    // boundary survives because it is structural, not an alpha multiply.
    let big = UVec2::new(2048, 2048); // 32 MiB, ISOLATION (structural)
    let small = UVec2::new(64, 64); // 32 KiB, OPACITY-only
    let groups = [
        (big, EffectReason::ISOLATION),
        (small, EffectReason::OPACITY),
    ];
    // Fits the structural group alone, but not the structural + opacity pair.
    let budget = target_bytes(big);
    let plan = plan_allocation(&groups, budget);
    assert!(
        plan[0],
        "large structural isolation group keeps its target (degrades last)"
    );
    assert!(
        !plan[1],
        "small opacity-only group degrades first under strict priority"
    );
}

#[test]
fn plan_allocation_degrades_all_when_even_top_priority_alone_exceeds_budget() {
    // When the single highest-priority (structural) group alone exceeds the
    // budget, strict-priority eviction degrades EVERY group — it never keeps a
    // lower-priority opacity-only group as a consolation survivor (§ 2.3: a
    // structural group degrades only after all opacity-only groups have, and
    // here even doing so cannot make the structural group fit).
    let big = UVec2::new(2048, 2048); // 32 MiB, ISOLATION
    let small = UVec2::new(64, 64); // 32 KiB, OPACITY-only
    let groups = [
        (big, EffectReason::ISOLATION),
        (small, EffectReason::OPACITY),
    ];
    // Too small for the structural group even on its own.
    let budget = target_bytes(small) + 1024;
    let plan = plan_allocation(&groups, budget);
    assert!(
        !plan[0],
        "structural group cannot fit alone, so it degrades"
    );
    assert!(
        !plan[1],
        "opacity-only group is not kept as a consolation survivor"
    );
}

#[test]
fn composite_group_opacity_scales_sampled_alpha() {
    // Opaque group sample (a=1) composited at group opacity 0.5 over a
    // black backdrop: dst = src * 0.5 (premultiplied SrcOver).
    let src = LinearRgba::new(1.0, 0.0, 0.0, 1.0); // straight-alpha red
    let dst = LinearRgba::new(0.0, 0.0, 0.0, 1.0);
    let out = composite_src_over(src, dst, 0.5);
    // Effective src alpha 0.5: out.rgb = 0.5*red + 0.5*black.
    assert!((out.red - 0.5).abs() < 1e-6);
    assert!((out.alpha - 1.0).abs() < 1e-6);
}

#[test]
fn composite_opacity_one_is_plain_src_over() {
    // group opacity 1.0 == the group composites identically to no scaling.
    let src = LinearRgba::new(0.2, 0.4, 0.6, 1.0);
    let dst = LinearRgba::new(0.0, 0.0, 0.0, 0.0);
    let out = composite_src_over(src, dst, 1.0);
    assert!((out.red - 0.2).abs() < 1e-6);
    assert!((out.green - 0.4).abs() < 1e-6);
    assert!((out.blue - 0.6).abs() < 1e-6);
    assert!((out.alpha - 1.0).abs() < 1e-6);
}

#[test]
fn composite_overlap_does_not_double_darken() {
    // The correctness point (§ 4 / § 5.1): a fully-composed group sample
    // applied ONCE at 0.5 over an identical backdrop yields a single 50%
    // blend, not a doubled one. Two opaque reds composited as ONE group
    // sample at 0.5 over red == red (no darkening of the overlap).
    let group_sample = LinearRgba::new(1.0, 0.0, 0.0, 1.0); // fully composed
    let backdrop = LinearRgba::new(1.0, 0.0, 0.0, 1.0);
    let out = composite_src_over(group_sample, backdrop, 0.5);
    assert!(
        (out.red - 1.0).abs() < 1e-6,
        "overlap stays single-layer red"
    );
}

#[test]
fn group_opacity_overlap_math_matches_golden_expectation() {
    // Two opaque reds composed inside the group = fully-opaque red sample;
    // that ONE sample at 0.5 over a white backdrop = 50% red + 50% white.
    let group_sample = LinearRgba::new(1.0, 0.0, 0.0, 1.0);
    let backdrop = LinearRgba::new(1.0, 1.0, 1.0, 1.0);
    let out = composite_src_over(group_sample, backdrop, 0.5);
    assert!((out.red - 1.0).abs() < 1e-6);
    assert!((out.green - 0.5).abs() < 1e-6);
    assert!((out.blue - 0.5).abs() < 1e-6);
}

#[test]
fn group_target_descriptor_is_pinned_rgba16float_linear() {
    use bevy::render::render_resource::{TextureFormat, TextureUsages};
    use buiy_core::render::compositor::group_target_descriptor;

    let d = group_target_descriptor(UVec2::new(64, 64));
    assert_eq!(d.label, Some("buiy_effect_group_target"));
    assert_eq!(d.format, TextureFormat::Rgba16Float);
    assert_eq!(d.size.width, 64);
    assert_eq!(d.size.height, 64);
    assert_eq!(d.size.depth_or_array_layers, 1);
    assert_eq!(d.mip_level_count, 1);
    assert_eq!(d.sample_count, 1);
    assert!(d.usage.contains(TextureUsages::RENDER_ATTACHMENT));
    assert!(d.usage.contains(TextureUsages::TEXTURE_BINDING));
}

#[test]
fn churn_never_exceeds_rt_pool_budget() {
    // 1000 groups (an adversarial open/close churn) all want targets, but
    // plan_allocation caps the live allocated bytes at the budget (§ 2.3):
    // the count of allocated targets * their bytes never exceeds the budget.
    let groups: Vec<_> = (0..1000)
        .map(|_| (UVec2::new(256, 256), EffectReason::OPACITY))
        .collect();
    let plan = plan_allocation(&groups, RT_POOL_BUDGET_BYTES);
    let live: u64 = groups
        .iter()
        .zip(&plan)
        .filter(|&(_, &alloc)| alloc)
        .map(|((extent, _), _)| target_bytes(*extent))
        .sum();
    assert!(
        live <= RT_POOL_BUDGET_BYTES,
        "live target bytes within budget"
    );
    assert!(plan.iter().any(|&a| !a), "some groups degraded under churn");
}
