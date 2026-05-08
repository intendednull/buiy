// Buiy rounded-rect shader. Inputs are clip-space units; CPU-side
// conversion lives in `render::instance::to_instance`.

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct Instance {
    @location(2) rect_pos: vec2<f32>,
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
    // SDF expects a positive half-extent. `i.rect_size.y` is intentionally
    // negative (CPU-side y-flip in `render::instance::to_instance`); without
    // the abs, the SDF would treat every interior fragment as outside the
    // rect and the alpha collapses to 0. The signed `rect_size` is still
    // load-bearing for `world` above and for `local_uv * half_size` in the
    // fragment stage, where both factors flip sign together.
    out.half_size = abs(i.rect_size) * 0.5;
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
