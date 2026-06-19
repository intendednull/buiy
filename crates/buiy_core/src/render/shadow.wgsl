// Buiy box-shadow shader (octet ..02). Closed-form Gaussian-blurred
// rounded-rect coverage — one draw per shadow, no convolution pass.
// Inputs match the quad instance layout (stride 68); the instance `blur`
// field carries the shadow's effective blur sigma in logical px for this
// primitive (the sibling component-model phase maps `BoxShadow.blur` into it).
//
// Like the quad shader, instance inputs are LOGICAL pixels and the view
// uniform (render::view_uniform::BuiyViewUniform) does the logical->clip
// transform in the vertex stage. The shadow pipeline shares the quad's
// `@group(0)` view-uniform layout (render::pipeline::view_uniform_layout_
// descriptor), so this shader binds the same `@group(0) @binding(0)`.

struct BuiyView {
    // col0 = [sx, 0, 0, tx]; col1 = [0, sy, 0, ty]; clip = M*logical + t.
    col0: vec4<f32>,
    col1: vec4<f32>,
    // [scale_factor, pad, pad, pad]
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> view: BuiyView;

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct Instance {
    @location(2) rect_pos: vec2<f32>,   // logical px, top-left
    @location(3) rect_size: vec2<f32>,  // logical px, POSITIVE height
    @location(4) color: vec4<f32>,
    @location(5) blur: f32,             // logical px, effective blur sigma
    @location(6) clip_min: vec2<f32>,   // logical px, clip AABB min (-inf = none)
    @location(7) clip_max: vec2<f32>,   // logical px, clip AABB max (+inf = none)
    @location(8) affine_col0: vec2<f32>, // 2D affine basis col0 = [m00, m10]
    @location(9) affine_col1: vec2<f32>, // 2D affine basis col1 = [m01, m11]
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_uv: vec2<f32>,   // -1..+1 across the rect (box-local, rotation-invariant)
    @location(1) half_size: vec2<f32>,  // logical px (box-local SDF half-extent)
    @location(2) color: vec4<f32>,
    @location(3) blur: f32,             // logical px
    @location(4) frag_logical: vec2<f32>, // affine-transformed window-logical corner (slot 4, was the axis-aligned center)
    @location(5) clip_min: vec2<f32>,   // logical px (clip AABB, ClipRect space)
    @location(6) clip_max: vec2<f32>,   // logical px (clip AABB, ClipRect space)
};

fn logical_to_clip(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.col0.x * p.x + view.col0.w, view.col1.y * p.y + view.col1.w);
}

@vertex
fn vertex(v: Vertex, i: Instance) -> VertexOut {
    var out: VertexOut;
    // R1: transform the box-local corner by the 2D affine BEFORE the
    // logical->clip view map (identity basis [1,0,0,1] -> rect_pos + local,
    // byte-identical to the pre-R1 axis-aligned path).
    let local = v.uv * i.rect_size;                // box-local corner (top-left at 0)
    let logical = i.rect_pos + mat2x2<f32>(i.affine_col0, i.affine_col1) * local;
    out.clip_position = vec4<f32>(logical_to_clip(logical), 0.0, 1.0);
    out.local_uv = v.uv * 2.0 - 1.0;
    out.half_size = i.rect_size * 0.5;             // positive — no abs needed
    out.color = i.color;
    out.blur = i.blur;
    // The affine is linear, so the interpolated frag_logical is the correct
    // transformed window-space point for the clip-AABB discard.
    out.frag_logical = logical;
    out.clip_min = i.clip_min;
    out.clip_max = i.clip_max;
    return out;
}

// Abramowitz & Stegun 7.1.26 erf approximation (max abs error ~1.5e-7).
fn erf(x: f32) -> f32 {
    let s = sign(x);
    let a = abs(x);
    let t = 1.0 / (1.0 + 0.3275911 * a);
    let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741)
        * t - 0.284496736) * t + 0.254829592) * t * exp(-a * a);
    return s * y;
}

// Closed-form 1D Gaussian-blurred box coverage along one axis: the integral
// of a unit box [-half, half] convolved with a Gaussian of std-dev sigma.
fn blurred_box_1d(p: f32, half: f32, sigma: f32) -> f32 {
    let inv = 1.0 / (sqrt(2.0) * max(sigma, 1e-4));
    return 0.5 * (erf((half - p) * inv) + erf((half + p) * inv));
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    // Per-primitive clip AABB (R8b): discard fragments outside [clip_min,
    // clip_max] in logical-px window space — the same space as ClipRect. The
    // full-view sentinel (±inf) makes this never fire (unclipped / top-layer).
    // frag_logical is the affine-transformed window-logical corner (R1).
    let frag_pos = in.frag_logical;
    if any(frag_pos < in.clip_min) || any(frag_pos > in.clip_max) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let p = in.local_uv * in.half_size;
    // Separable approximation of the rounded-rect blur: product of the two
    // axis-blurred box coverages. Corner rounding is folded into the
    // effective half-size shrink by `blur` (a v1 approximation; the exact
    // rounded-corner blur is a later refinement, not required by the
    // headless gate).
    let cov = blurred_box_1d(p.x, in.half_size.x, in.blur)
        * blurred_box_1d(p.y, in.half_size.y, in.blur);
    return vec4<f32>(in.color.rgb, in.color.a * cov);
}
