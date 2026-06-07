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
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_uv: vec2<f32>,   // -1..+1 across the rect
    @location(1) half_size: vec2<f32>,  // logical px
    @location(2) color: vec4<f32>,
    @location(3) radius: f32,            // logical px
    @location(4) rect_center: vec2<f32>, // logical px, window-relative
    @location(5) clip_min: vec2<f32>,   // logical px (clip AABB, ClipRect space)
    @location(6) clip_max: vec2<f32>,   // logical px (clip AABB, ClipRect space)
};

fn logical_to_clip(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.col0.x * p.x + view.col0.w, view.col1.y * p.y + view.col1.w);
}

@vertex
fn vertex(v: Vertex, i: Instance) -> VertexOut {
    var out: VertexOut;
    let logical = i.rect_pos + v.uv * i.rect_size; // logical-px corner
    out.clip_position = vec4<f32>(logical_to_clip(logical), 0.0, 1.0);
    out.local_uv = v.uv * 2.0 - 1.0;
    out.half_size = i.rect_size * 0.5;             // positive — no abs needed
    out.color = i.color;
    out.radius = i.radius;
    out.rect_center = i.rect_pos + out.half_size;  // logical px, window-relative
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
    let frag_pos = in.rect_center + in.local_uv * in.half_size;
    if any(frag_pos < in.clip_min) || any(frag_pos > in.clip_max) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    // SDF in logical px; AA from fwidth in logical px (the view uniform keeps
    // logical px well-scaled, so fwidth is meaningful without scale_factor).
    let d = sdf_rounded_rect(in.local_uv * in.half_size, in.half_size, in.radius);
    let aa = fwidth(d);
    let alpha = 1.0 - smoothstep(-aa, aa, d);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
