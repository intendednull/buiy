//! Headless WGSL validation of Buiy's render shaders. Parses each shader
//! source with `naga` (no wgpu adapter needed) and asserts the expected
//! entry points exist. This is the device-free half of pipeline coverage;
//! actual GPU compilation rides the `#[ignore]` e2e path (render_smoke.rs).

/// Parse WGSL source with naga; panics with the naga diagnostic on error.
fn parse_wgsl(label: &str, src: &str) -> naga::Module {
    naga::front::wgsl::parse_str(src)
        .unwrap_or_else(|e| panic!("{label}: WGSL parse failed: {e:?}"))
}

/// True iff the module declares an entry point with this name.
fn has_entry_point(module: &naga::Module, name: &str) -> bool {
    module.entry_points.iter().any(|ep| ep.name == name)
}

const QUAD_WGSL: &str = include_str!("../../src/render/shader.wgsl");
const SHADOW_WGSL: &str = include_str!("../../src/render/shadow.wgsl");
const COVERAGE_WGSL: &str = include_str!("../../src/render/coverage.wgsl");
const COMPOSITE_WGSL: &str = include_str!("../../src/render/composite.wgsl");

#[test]
fn quad_shader_parses_and_has_entry_points() {
    let m = parse_wgsl("quad", QUAD_WGSL);
    assert!(has_entry_point(&m, "vertex"), "quad shader has `vertex`");
    assert!(
        has_entry_point(&m, "fragment"),
        "quad shader has `fragment`"
    );
}

#[test]
fn shadow_shader_parses_and_has_entry_points() {
    let m = parse_wgsl("shadow", SHADOW_WGSL);
    assert!(has_entry_point(&m, "vertex"), "shadow shader has `vertex`");
    assert!(
        has_entry_point(&m, "fragment"),
        "shadow shader has `fragment`"
    );
}

#[test]
fn quad_shader_with_clip_parses() {
    // The R8b clip AABB rides the instance at `@location(6)`/`(7)` and the
    // fragment discards outside `[clip_min, clip_max]`. naga rejects a malformed
    // attribute index, type, or discard expression, so a clean parse + present
    // entry points proves the clip additions are well-formed WGSL.
    let m = parse_wgsl("quad", QUAD_WGSL);
    assert!(has_entry_point(&m, "vertex"), "quad shader has `vertex`");
    assert!(
        has_entry_point(&m, "fragment"),
        "quad shader has `fragment`"
    );
    assert!(
        QUAD_WGSL.contains("clip_min") && QUAD_WGSL.contains("clip_max"),
        "quad shader declares the clip AABB inputs"
    );
    assert!(
        QUAD_WGSL.contains("@location(6)") && QUAD_WGSL.contains("@location(7)"),
        "quad clip inputs bound at @location(6)/(7) (matches the vertex layout)"
    );
}

#[test]
fn shadow_shader_with_clip_parses() {
    // Same clip AABB as the quad shader, with the shadow's `@location(5)` being
    // `blur` (not `radius`); the clip fields still append at `@location(6)`/`(7)`.
    let m = parse_wgsl("shadow", SHADOW_WGSL);
    assert!(has_entry_point(&m, "vertex"), "shadow shader has `vertex`");
    assert!(
        has_entry_point(&m, "fragment"),
        "shadow shader has `fragment`"
    );
    assert!(
        SHADOW_WGSL.contains("clip_min") && SHADOW_WGSL.contains("clip_max"),
        "shadow shader declares the clip AABB inputs"
    );
    assert!(
        SHADOW_WGSL.contains("@location(6)") && SHADOW_WGSL.contains("@location(7)"),
        "shadow clip inputs bound at @location(6)/(7) (matches the vertex layout)"
    );
}

#[test]
fn coverage_shader_parses_and_has_entry_points() {
    // The alpha-as-color glyph shader (coverage.wgsl) is loaded via
    // `Shader::from_wgsl` (render/mod.rs) but only device-compiled in the
    // `#[ignore]` GPU lane, so naga is the only headless guard against a
    // syntax/binding regression. naga rejects a malformed attribute index,
    // type, or binding, so a clean parse + present entry points proves the
    // shader is well-formed WGSL.
    let m = parse_wgsl("coverage", COVERAGE_WGSL);
    assert!(
        has_entry_point(&m, "vertex"),
        "coverage shader has `vertex`"
    );
    assert!(
        has_entry_point(&m, "fragment"),
        "coverage shader has `fragment`"
    );
    // Pin the additive atlas binding (the quad/shadow pipelines never bind
    // `@group(1)`; coverage adds the R8 atlas texture + sampler there — a
    // dropped or moved binding is a real regression the parse alone misses).
    assert!(
        COVERAGE_WGSL.contains("@group(1) @binding(0) var atlas")
            && COVERAGE_WGSL.contains("@group(1) @binding(1) var atlas_samp"),
        "coverage shader binds the atlas texture+sampler on @group(1)"
    );
    // The view uniform is byte-identically shared with the quad shader at
    // @group(0) @binding(0) (the same `BuiyView`).
    assert!(
        COVERAGE_WGSL.contains("@group(0) @binding(0) var<uniform> view"),
        "coverage shader shares the quad view uniform at @group(0) @binding(0)"
    );
}

#[test]
fn composite_shader_parses_and_has_entry_points() {
    // The effect-group composite shader (composite.wgsl) samples a group's
    // off-screen target and blends it into the parent. Same headless-only
    // exposure as coverage.wgsl: device compilation rides the `#[ignore]`
    // lane, so naga is the merge-gate guard.
    let m = parse_wgsl("composite", COMPOSITE_WGSL);
    assert!(
        has_entry_point(&m, "vertex"),
        "composite shader has `vertex`"
    );
    assert!(
        has_entry_point(&m, "fragment"),
        "composite shader has `fragment`"
    );
    // Pin the source-target binding (the off-screen group target + its
    // sampler ride `@group(1)`; the composite params uniform rides
    // `@group(0)`) so a binding regression is caught, not just a syntax error.
    assert!(
        COMPOSITE_WGSL.contains("@group(0) @binding(0) var<uniform> comp")
            && COMPOSITE_WGSL.contains("@group(1) @binding(0) var src_tex")
            && COMPOSITE_WGSL.contains("@group(1) @binding(1) var src_samp"),
        "composite shader binds the params uniform (@group(0)) + the source target+sampler (@group(1))"
    );
}

#[test]
fn quad_shader_applies_affine_via_mat2x2() {
    // R1: the quad shader declares the 2D affine basis instance inputs at
    // @location(8)/(9), builds the window-logical corner via a `mat2x2`, and
    // interpolates `frag_logical` for the clip discard — `rect_center` is GONE
    // (it was the axis-aligned corner, wrong under rotation). naga PARSES (not
    // string-grep) so a malformed VertexOut/fragment-input mismatch is rejected.
    let m = parse_wgsl("quad", QUAD_WGSL);
    assert!(has_entry_point(&m, "vertex"));
    assert!(has_entry_point(&m, "fragment"));
    assert!(
        QUAD_WGSL.contains("@location(8)") && QUAD_WGSL.contains("@location(9)"),
        "quad affine inputs bound at @location(8)/(9) (matches the vertex layout)"
    );
    assert!(
        QUAD_WGSL.contains("affine_col0") && QUAD_WGSL.contains("affine_col1"),
        "quad shader declares the affine basis columns"
    );
    assert!(
        QUAD_WGSL.contains("mat2x2"),
        "quad vertex builds the logical corner via a mat2x2 affine"
    );
    assert!(
        QUAD_WGSL.contains("frag_logical"),
        "quad carries the affine-transformed window-logical corner for the clip discard"
    );
    assert!(
        !QUAD_WGSL.contains("rect_center"),
        "rect_center (the axis-aligned corner) is dropped — replaced by frag_logical"
    );
}

#[test]
fn shadow_shader_applies_affine_via_mat2x2() {
    // The shadow shader mirrors the quad shader's affine path identically.
    let m = parse_wgsl("shadow", SHADOW_WGSL);
    assert!(has_entry_point(&m, "vertex"));
    assert!(has_entry_point(&m, "fragment"));
    assert!(
        SHADOW_WGSL.contains("@location(8)") && SHADOW_WGSL.contains("@location(9)"),
        "shadow affine inputs bound at @location(8)/(9)"
    );
    assert!(
        SHADOW_WGSL.contains("affine_col0") && SHADOW_WGSL.contains("affine_col1"),
        "shadow shader declares the affine basis columns"
    );
    assert!(
        SHADOW_WGSL.contains("mat2x2"),
        "shadow vertex builds the logical corner via a mat2x2 affine"
    );
    assert!(
        SHADOW_WGSL.contains("frag_logical"),
        "shadow carries the affine-transformed window-logical corner"
    );
    assert!(
        !SHADOW_WGSL.contains("rect_center"),
        "rect_center is dropped in the shadow shader too"
    );
}
