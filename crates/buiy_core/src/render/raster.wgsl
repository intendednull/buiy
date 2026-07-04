// Buiy raster (textured-quad) shader — the drawing-canvas seam. Samples a bevy
// `Image` onto the node's layout rect. Instance inputs are LOGICAL pixels; the
// per-view uniform (render::view_uniform::BuiyViewUniform) does the
// logical->clip transform in the vertex stage, exactly like the quad family.
//
// Unlike the quad/glyph pipelines this samples a PER-NODE texture bound at
// `@group(1)` (each `RasterImage` node is its own texture + its own draw), not
// the one shared coverage atlas. The color is the sampled texel — no per-instance
// tint in v1. The image is authored as `Rgba8UnormSrgb`, so the sample decodes
// sRGB->linear and the sRGB view attachment re-encodes on write (round-tripping
// the authored bytes).

struct BuiyView {
    // col0 = [sx, 0, 0, tx]; col1 = [0, sy, 0, ty]; clip = M*logical + t.
    col0: vec4<f32>,
    col1: vec4<f32>,
    // [scale_factor, pad, pad, pad]
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> view: BuiyView;

// The per-node sampled image + its sampler (Nearest — crisp pixel drawing).
@group(1) @binding(0) var image_tex: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct Instance {
    @location(2) rect_pos: vec2<f32>,   // logical px, top-left
    @location(3) rect_size: vec2<f32>,  // logical px, POSITIVE height
    @location(4) clip_min: vec2<f32>,   // logical px, clip AABB min (-inf = none)
    @location(5) clip_max: vec2<f32>,   // logical px, clip AABB max (+inf = none)
    @location(6) affine: vec4<f32>,     // 2D affine basis [m00, m10, m01, m11]
    @location(7) radius: f32,           // uniform corner radius, logical px (0 = square clip)
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,           // 0..1 across the rect (texture coords)
    @location(1) frag_logical: vec2<f32>, // affine-transformed window-logical corner
    @location(2) clip_min: vec2<f32>,
    @location(3) clip_max: vec2<f32>,
    @location(4) local: vec2<f32>,        // box-local point, centered px (-half..+half)
    @location(5) half_size: vec2<f32>,    // box-local SDF half-extent, px
    @location(6) radius: f32,             // uniform corner radius, logical px
};

fn logical_to_clip(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.col0.x * p.x + view.col0.w, view.col1.y * p.y + view.col1.w);
}

@vertex
fn vertex(v: Vertex, i: Instance) -> VertexOut {
    var out: VertexOut;
    // Transform the box-local corner by the 2D affine BEFORE the logical->clip
    // view map (the quad-family convention). Identity `[1,0,0,1]` paints
    // axis-aligned.
    let local = v.uv * i.rect_size;
    let affine = mat2x2<f32>(i.affine.xy, i.affine.zw);
    let logical = i.rect_pos + affine * local;
    out.clip_position = vec4<f32>(logical_to_clip(logical), 0.0, 1.0);
    out.uv = v.uv;
    out.frag_logical = logical;
    out.clip_min = i.clip_min;
    out.clip_max = i.clip_max;
    out.local = (v.uv - vec2<f32>(0.5, 0.5)) * i.rect_size;  // centered box-local px
    out.half_size = i.rect_size * 0.5;
    out.radius = i.radius;
    return out;
}

// Signed distance to a rounded rect centered at the origin (the shared SDF, port
// of shader.wgsl / band.wgsl `sdf_rounded_rect`).
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    // Per-primitive clip AABB (R8b): the full-view sentinel (±inf) never fires.
    // `textureSample` needs UNIFORM control flow (implicit derivatives), so we
    // sample UNCONDITIONALLY and apply the clip as an alpha mask — never an early
    // return before the sample (native naga is lenient, Tint/WebGPU is strict).
    let clipped = any(in.frag_logical < in.clip_min) || any(in.frag_logical > in.clip_max);
    let texel = textureSample(image_tex, image_sampler, in.uv);
    let mask = select(1.0, 0.0, clipped);
    // Rounded corner clip (F4b-4): AA'd rounded-rect coverage in box-local space
    // (rotation-invariant, computed UNCONDITIONALLY so `fwidth` stays uniform).
    // `radius == 0` BYPASSES it (mask 1.0) so the square F1 raster path is
    // byte-identical; a positive radius clips a custom avatar to a circle/pill.
    let d = sdf_rounded_rect(in.local, in.half_size, in.radius);
    let corner_cov = 1.0 - smoothstep(-fwidth(d), fwidth(d), d);
    let corner_mask = select(1.0, corner_cov, in.radius > 0.0);
    return vec4<f32>(texel.rgb, texel.a * mask * corner_mask);
}
