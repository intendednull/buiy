// Buiy rounded-rect shader. Instance inputs are LOGICAL pixels; the view
// uniform (render::view_uniform::BuiyViewUniform) does the logical->clip
// transform in the vertex stage. The y-flip and px->clip scale live ENTIRELY
// in the uniform — the per-instance y-flip / 2/min(w,h) hack is retired.

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
    @location(5) radius: f32,            // logical px
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
    @location(3) radius: f32,            // logical px
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
    // logical->clip view map. The affine maps box-local 0 -> 0 for a pure
    // rotation/scale, so an identity basis [1,0,0,1] yields rect_pos + local
    // (byte-identical to the pre-R1 axis-aligned path).
    let local = v.uv * i.rect_size;                // box-local corner (top-left at 0)
    let logical = i.rect_pos + mat2x2<f32>(i.affine_col0, i.affine_col1) * local;
    out.clip_position = vec4<f32>(logical_to_clip(logical), 0.0, 1.0);
    out.local_uv = v.uv * 2.0 - 1.0;
    out.half_size = i.rect_size * 0.5;             // positive — no abs needed
    out.color = i.color;
    out.radius = i.radius;
    // The affine is linear, so the interpolated frag_logical is the correct
    // transformed window-space point for the clip-AABB discard.
    out.frag_logical = logical;
    out.clip_min = i.clip_min;
    out.clip_max = i.clip_max;
    return out;
}

// Signed distance to a rounded rect centered at the origin.
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    // Per-primitive clip AABB (R8b): discard fragments outside [clip_min,
    // clip_max] in logical-px window space — the same space as ClipRect. The
    // full-view sentinel (±inf) makes this never fire (unclipped / top-layer).
    // frag_logical is the affine-transformed window-logical corner (R1) — the
    // correct post-transform point, not the old axis-aligned box center.
    // WebGPU/Tint requires derivative builtins (`fwidth`) in UNIFORM control
    // flow, so we must NOT early-return on the per-fragment clip test before
    // `fwidth`. Compute the SDF + AA unconditionally and apply the clip as an
    // alpha mask. Behavior-identical on native (clipped -> alpha 0 either way);
    // native naga is lenient and accepts the early return, Tint rejects it.
    let frag_pos = in.frag_logical;
    let clipped = any(frag_pos < in.clip_min) || any(frag_pos > in.clip_max);
    // SDF in logical px; AA from fwidth in logical px (the view uniform keeps
    // logical px well-scaled, so fwidth is meaningful without scale_factor).
    let d = sdf_rounded_rect(in.local_uv * in.half_size, in.half_size, in.radius);
    let aa = fwidth(d);
    let alpha = 1.0 - smoothstep(-aa, aa, d);
    let mask = select(1.0, 0.0, clipped);
    return vec4<f32>(in.color.rgb, in.color.a * alpha * mask);
}
