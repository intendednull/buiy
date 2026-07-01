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
const BAND_WGSL: &str = include_str!("../../src/render/band.wgsl");
const GRADIENT_WGSL: &str = include_str!("../../src/render/gradient.wgsl");

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
fn band_shader_parses_and_has_entry_points() {
    // The border/outline band shader (band.wgsl, octet ..06 — C6-a feeds the
    // OUTLINE channel). Device compilation rides the `#[ignore]` GPU lane, so
    // naga is the merge-gate guard against a syntax/binding/attribute regression.
    let m = parse_wgsl("band", BAND_WGSL);
    assert!(has_entry_point(&m, "vertex"), "band shader has `vertex`");
    assert!(
        has_entry_point(&m, "fragment"),
        "band shader has `fragment`"
    );
    // It shares the quad family's view uniform at @group(0) @binding(0) (no
    // @group(1) — the band samples no texture).
    assert!(
        BAND_WGSL.contains("@group(0) @binding(0) var<uniform> view"),
        "band shader shares the quad view uniform at @group(0) @binding(0)"
    );
    assert!(
        !BAND_WGSL.contains("@group(1)"),
        "the band pipeline binds no @group(1) (it samples no texture)"
    );
    // The outer-minus-inner SDF band + the AncestorClip discard are the two
    // load-bearing pieces (styling-f-tier.md § 2.3 / § 2.4).
    assert!(
        BAND_WGSL.contains("sdf_rounded_rect") && BAND_WGSL.contains("inside_inner"),
        "band fragment is inside(outer) AND NOT inside(inner)"
    );
    assert!(
        BAND_WGSL.contains("clip_min") && BAND_WGSL.contains("clip_max"),
        "band shader discards outside the (outline/ancestor) clip AABB"
    );
    // The per-side color inputs ride @location(4)..(7) and the affine basis the
    // LAST location as a single vec4 (folded from two vec2 cols to keep the band
    // layout at 16 vertex attributes — WebGL2's max_vertex_attributes cap) —
    // matching the BorderBandInstance vertex layout.
    assert!(
        BAND_WGSL.contains("affine: vec4<f32>"),
        "band shader declares the affine basis as one vec4 (loc 15)"
    );
}

#[test]
fn gradient_shader_parses_and_has_entry_points() {
    // The background-gradient shader (gradient.wgsl, octet ..07 — parity Wave B1).
    // Device compilation rides the `#[ignore]` GPU lane, so naga is the merge-gate
    // guard against a syntax/binding/attribute regression.
    let m = parse_wgsl("gradient", GRADIENT_WGSL);
    assert!(
        has_entry_point(&m, "vertex"),
        "gradient shader has `vertex`"
    );
    assert!(
        has_entry_point(&m, "fragment"),
        "gradient shader has `fragment`"
    );
    // Shares the quad family's view uniform at @group(0) @binding(0) (no
    // @group(1) — the gradient is computed in-shader, samples no texture).
    assert!(
        GRADIENT_WGSL.contains("@group(0) @binding(0) var<uniform> view"),
        "gradient shader shares the quad view uniform at @group(0) @binding(0)"
    );
    assert!(
        !GRADIENT_WGSL.contains("@group(1)"),
        "the gradient pipeline binds no @group(1) (it samples no texture)"
    );
    // The clip discard + the affine basis match the GradientInstance layout.
    assert!(
        GRADIENT_WGSL.contains("clip_min") && GRADIENT_WGSL.contains("clip_max"),
        "gradient shader discards outside the clip AABB"
    );
    assert!(
        GRADIENT_WGSL.contains("affine_col0") && GRADIENT_WGSL.contains("affine_col1"),
        "gradient shader declares the affine basis columns"
    );
    // The two stop colors + the axis projection (linear) / center distance
    // (radial branch) are the load-bearing gradient pieces.
    assert!(
        GRADIENT_WGSL.contains("color0") && GRADIENT_WGSL.contains("color1"),
        "gradient shader interpolates two stop colors"
    );
    assert!(
        GRADIENT_WGSL.contains("dot(in.local, in.axis)"),
        "gradient shader projects the fragment onto the precomputed axis (linear)"
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
