// Buiy effect-group composite shader (octet ..05). Samples one group's off-screen
// Rgba16Float target and blends it SrcOver into the PARENT (a nested group's
// target, or the window) with `sampled.a * opacity` (effect-compositor.md § 3
// step 2). The whole group was composed as ONE unit in the target, so its
// translucency applies ONCE here — overlapping children inside an `opacity < 1`
// group do not double-darken (the correct semantics, § 4).
//
// The group target stores STRAIGHT-alpha LINEAR color (the quad shader's output,
// blended by ALPHA_BLENDING into the linear Rgba16Float). This pass outputs
// straight-alpha linear too and relies on the pipeline's ALPHA_BLENDING to SrcOver
// into the parent — so the blend happens in the PARENT's space (the window
// re-encodes linear→sRGB8 on write, exactly like the flat path).

// Composite params (`@group(0) @binding(0)`): the PARENT view transform to place
// the quad + the quad's logical bounds + the target's used uv sub-rect + opacity.
struct Composite {
    // Parent logical→clip columns: col0 = [sx,0,0,tx]; col1 = [0,sy,0,ty].
    col0: vec4<f32>,
    col1: vec4<f32>,
    // bounds.min.xy, bounds.max.zw — the composite quad in PARENT logical px.
    bounds: vec4<f32>,
    // uv_max.xy (used sub-rect of the pow2-bucketed target), opacity in .z.
    uv_params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> comp: Composite;

// The group's off-screen target + its sampler (`@group(1)`).
@group(1) @binding(0) var src_tex: texture_2d<f32>;
@group(1) @binding(1) var src_samp: sampler;

struct Vertex {
    @location(0) position: vec2<f32>,   // unit quad (unused; uv drives the corner)
    @location(1) uv: vec2<f32>,         // unit-quad uv in [0,1]
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) src_uv: vec2<f32>,     // uv into the group target's used region
};

fn logical_to_clip(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(comp.col0.x * p.x + comp.col0.w, comp.col1.y * p.y + comp.col1.w);
}

@vertex
fn vertex(v: Vertex) -> VertexOut {
    var out: VertexOut;
    // The quad corner = bounds.min + uv * (bounds.max - bounds.min), in PARENT
    // logical px; place it via the parent view transform.
    let min = comp.bounds.xy;
    let max = comp.bounds.zw;
    let logical = min + v.uv * (max - min);
    out.clip_position = vec4<f32>(logical_to_clip(logical), 0.0, 1.0);
    // Sample only the used sub-rect of the (pow2-bucketed) target — the target's
    // texel (0,0) holds the painted-bounds min, so uv [0,uv_max] is the content.
    out.src_uv = v.uv * comp.uv_params.xy;
    return out;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    let sampled = textureSample(src_tex, src_samp, in.src_uv);
    let opacity = comp.uv_params.z;
    // Straight-alpha SrcOver in the parent's space: scale the group's coverage by
    // the group opacity, leave rgb straight (the pipeline's ALPHA_BLENDING does
    // the `src*a + dst*(1-a)` blend). `composite_src_over` (compositor.rs) is the
    // CPU port of exactly this.
    return vec4<f32>(sampled.rgb, sampled.a * opacity);
}
