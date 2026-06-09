//! Device-free proof of the per-format-distinct-id mapping contract that the
//! render-world `SpecializedRenderPipelines<BuiyPrimitives>` cache enforces.
//! We model the cache's `entry(key).or_insert_with(counter)` allocation with
//! a HashMap + monotonic counter (no PipelineCache, no RenderDevice), so the
//! key→id contract is asserted with no wgpu adapter. The live-cache version
//! rides the `#[ignore]` GPU path (Task 9). See architecture.md § 1.4.

use std::collections::HashMap;

use bevy::render::render_resource::TextureFormat;
use buiy_core::render::buckets::BuiyPrimitiveKind;
use buiy_core::render::primitive::BuiyPrimitiveKey;

/// Mirror of `SpecializedRenderPipelines::specialize`'s allocation: each new
/// key gets the next id; a repeated key returns its existing id.
#[derive(Default)]
struct StubPipelineIds {
    map: HashMap<BuiyPrimitiveKey, u32>,
    next: u32,
}
impl StubPipelineIds {
    fn id_for(&mut self, key: BuiyPrimitiveKey) -> u32 {
        let next = &mut self.next;
        *self.map.entry(key).or_insert_with(|| {
            let id = *next;
            *next += 1;
            id
        })
    }
}

fn key(kind: BuiyPrimitiveKind, format: TextureFormat) -> BuiyPrimitiveKey {
    BuiyPrimitiveKey {
        kind,
        format,
        samples: 1,
    }
}

#[test]
fn each_kind_x_format_x_samples_gets_a_distinct_id() {
    use BuiyPrimitiveKind::*;
    let mut ids = StubPipelineIds::default();
    let formats = [TextureFormat::Rgba8UnormSrgb, TextureFormat::Rgba16Float];
    let mut seen = Vec::new();
    // The landed R6 enum's four kinds; the key is `(kind, format, samples)` so
    // each tuple is a distinct variant regardless of which kinds' shaders this
    // phase ships. Sample counts 1 and 4 mirror the Msaa::Off / Msaa::Sample4
    // (the bare-Camera2d default) view variants.
    for kind in [Shadow, Quad, Glyph, Path] {
        for format in formats {
            for samples in [1u32, 4] {
                seen.push(ids.id_for(BuiyPrimitiveKey {
                    kind,
                    format,
                    samples,
                }));
            }
        }
    }
    // 4 kinds × 2 formats × 2 sample counts = 16 distinct variants → 16 ids.
    let mut uniq = seen.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(
        uniq.len(),
        16,
        "every (kind, format, samples) variant is distinct"
    );
}

#[test]
fn identical_key_dedupes_to_one_id() {
    let mut ids = StubPipelineIds::default();
    let k = key(BuiyPrimitiveKind::Quad, TextureFormat::Rgba8UnormSrgb);
    let a = ids.id_for(k);
    let b = ids.id_for(k);
    assert_eq!(a, b, "same key → same cached id (no duplicate pipeline)");
}

#[test]
fn hdr_view_and_group_target_share_the_rgba16float_variant() {
    // The key is the *format*; an HDR view's main_texture_format() is
    // Rgba16Float, identical to the effect-group target format, so they
    // collapse onto the same variant id (no redundant pipeline).
    let mut ids = StubPipelineIds::default();
    let hdr_view = ids.id_for(key(BuiyPrimitiveKind::Quad, TextureFormat::Rgba16Float));
    let group_target = ids.id_for(key(BuiyPrimitiveKind::Quad, TextureFormat::Rgba16Float));
    assert_eq!(hdr_view, group_target);
}
