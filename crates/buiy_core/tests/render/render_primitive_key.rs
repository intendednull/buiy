//! Device-free tests for the typed-primitive specialization key. No wgpu
//! adapter required — these assert the pure key logic that drives
//! `SpecializedRenderPipeline` variant selection (architecture.md § 1.4).

use bevy::render::render_resource::TextureFormat;
// `BuiyPrimitiveKind` is owned by R6 (render::buckets); `BuiyPrimitiveKey`
// is owned here (render::primitive). Import each from its real owner.
use buiy_core::render::buckets::BuiyPrimitiveKind;
use buiy_core::render::primitive::BuiyPrimitiveKey;

#[test]
fn kind_variants_are_distinct() {
    use BuiyPrimitiveKind::*;
    // The landed R6 enum (render::buckets) — Shadow/Quad/Glyph/Path. Border
    // folds into Quad and Outline is a clip-suppressed Quad, so neither is a
    // variant here. This phase ships the Quad and Shadow pipelines.
    let all = [Shadow, Quad, Glyph, Path];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            assert_eq!(i == j, a == b, "{a:?} vs {b:?} distinctness");
        }
    }
}

#[test]
fn key_equality_is_by_kind_format_and_samples() {
    let a = BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
        samples: 1,
    };
    let b = BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
        samples: 1,
    };
    let diff_format = BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba16Float,
        samples: 1,
    };
    let diff_kind = BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Shadow,
        format: TextureFormat::Rgba8UnormSrgb,
        samples: 1,
    };
    // The MSAA seam: a 4x view-pass variant is a distinct pipeline from the 1x
    // baseline (wgpu validates the pipeline's MultisampleState.count against
    // the pass attachments at set_pipeline).
    let diff_samples = BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
        samples: 4,
    };
    assert_eq!(a, b);
    assert_ne!(a, diff_format);
    assert_ne!(a, diff_kind);
    assert_ne!(a, diff_samples);
}

#[test]
fn key_is_hashable_and_dedupes_in_a_set() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
        samples: 1,
    });
    // Same key inserted twice → one entry (Hash + Eq).
    set.insert(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
        samples: 1,
    });
    // Different format → distinct entry.
    set.insert(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba16Float,
        samples: 1,
    });
    // Different sample count → distinct entry (the Msaa::Sample4 view variant).
    set.insert(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
        samples: 4,
    });
    assert_eq!(set.len(), 3);
}
