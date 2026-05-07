// Buiy Phase 0 rounded-rect shader. Vertex stage emits a unit quad (one
// quad per draw). Fragment stage computes signed distance from the
// rounded rect interior and outputs the per-instance color, with anti-
// aliased edges.
//
// TODO(Task 11): rect_pos / rect_size are currently fed in clip-space
// units (-1..+1) but the layout system produces logical pixels. Either
// (a) Task 11 pre-multiplies by the inverse window size on the CPU
// before writing the instance buffer, or (b) introduce a view uniform
// (window-size or full view-projection matrix) and apply the transform
// here in the vertex stage. Decide before instance-buffer construction
// lands. The pipeline descriptor's `layout: vec![]` will need to grow
// to include the bind-group layout if (b) is chosen.

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct Instance {
    @location(2) rect_pos: vec2<f32>,    // see TODO above re: units
    @location(3) rect_size: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(5) radius: f32,
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_uv: vec2<f32>,    // -1..+1 across the rect
    @location(1) half_size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) radius: f32,
};

@vertex
fn vertex(v: Vertex, i: Instance) -> VertexOut {
    var out: VertexOut;
    let world = i.rect_pos + v.uv * i.rect_size;
    out.clip_position = vec4<f32>(world, 0.0, 1.0);
    out.local_uv = v.uv * 2.0 - 1.0;
    out.half_size = i.rect_size * 0.5;
    out.color = i.color;
    out.radius = i.radius;
    return out;
}

// Signed distance to a rounded rect centered at the origin.
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    let d = sdf_rounded_rect(in.local_uv * in.half_size, in.half_size, in.radius);
    let aa = fwidth(d);
    let alpha = 1.0 - smoothstep(-aa, aa, d);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
