// Buiy ROUNDED box-shadow shader (octet ..0A, F4b-6). A blurred ROUNDED-rect
// coverage — the distinct-record sibling of `shadow.wgsl` (the square shadow),
// carrying BOTH a blur sigma AND a corner radius so a rounded caster's shadow
// rounds its corners to match instead of drawing a rectangular blur that pokes
// past the box. The crisp zero-blur 3D-press "sticker" edge is `sigma == 0`.
//
// Coverage = the erf of the signed distance to the rounded-rect (a Gaussian CDF):
// EXACT for a straight edge, a good approximation around the rounded corners, and
// it degrades to a crisp fwidth-AA edge as sigma → 0. Like the quad family, inputs
// are LOGICAL pixels and the shared view uniform does the logical→clip transform.

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

// MUST match `RoundedShadowInstance`'s #[repr(C)] field offsets + the
// `rounded_shadow_vertex_buffers` VertexBufferLayout byte-for-byte.
struct Instance {
    @location(2) rect_pos: vec2<f32>,     // logical px, top-left
    @location(3) rect_size: vec2<f32>,    // logical px, POSITIVE height
    @location(4) color: vec4<f32>,        // pre-linearized shadow color
    @location(5) blur_radius: vec2<f32>,  // (sigma, corner radius), logical px
    @location(6) clip: vec4<f32>,         // (clip_min.xy, clip_max.zw), logical px
    @location(7) affine: vec4<f32>,       // 2D affine basis [m00, m10, m01, m11]
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local: vec2<f32>,        // box-local point, centered px (rotation-invariant)
    @location(1) half_size: vec2<f32>,    // logical px (box-local SDF half-extent)
    @location(2) color: vec4<f32>,
    @location(3) blur_radius: vec2<f32>,  // (sigma, corner radius)
    @location(4) frag_logical: vec2<f32>, // affine-transformed window-logical corner (clip discard)
    @location(5) clip_min: vec2<f32>,
    @location(6) clip_max: vec2<f32>,
};

fn logical_to_clip(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.col0.x * p.x + view.col0.w, view.col1.y * p.y + view.col1.w);
}

@vertex
fn vertex(v: Vertex, i: Instance) -> VertexOut {
    var out: VertexOut;
    let local_corner = v.uv * i.rect_size;                 // box-local corner (top-left at 0)
    let logical = i.rect_pos + mat2x2<f32>(i.affine.xy, i.affine.zw) * local_corner;
    out.clip_position = vec4<f32>(logical_to_clip(logical), 0.0, 1.0);
    out.local = (v.uv - vec2<f32>(0.5, 0.5)) * i.rect_size; // centered px
    out.half_size = i.rect_size * 0.5;                      // positive
    out.color = i.color;
    out.blur_radius = i.blur_radius;
    out.frag_logical = logical;
    out.clip_min = i.clip.xy;
    out.clip_max = i.clip.zw;
    return out;
}

// Abramowitz & Stegun 7.1.26 erf approximation (max abs error ~1.5e-7) — the same
// approximation `shadow.wgsl` uses, so the two shadow pipelines share a blur model.
fn erf(x: f32) -> f32 {
    let s = sign(x);
    let a = abs(x);
    let t = 1.0 / (1.0 + 0.3275911 * a);
    let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741)
        * t - 0.284496736) * t + 0.254829592) * t * exp(-a * a);
    return s * y;
}

// Signed distance to a rounded rect centered at the origin (port of
// shader.wgsl / band.wgsl `sdf_rounded_rect`).
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    // Per-primitive clip AABB (R8b): the full-view sentinel (±inf) never fires.
    // `fwidth` needs UNIFORM control flow, so compute coverage UNCONDITIONALLY and
    // apply the clip as an alpha mask — never an early return (Tint/WebGPU strict).
    let clipped = any(in.frag_logical < in.clip_min) || any(in.frag_logical > in.clip_max);
    let sigma = in.blur_radius.x;
    let radius = in.blur_radius.y;
    let d = sdf_rounded_rect(in.local, in.half_size, radius);
    let aa = fwidth(d);
    // erf of the signed distance = a Gaussian CDF. `sigma_eff` floors at the
    // fwidth AA so a zero-blur shadow is a crisp 1px AA edge (the 3D-press case),
    // and a positive sigma gives a soft Gaussian falloff.
    let sigma_eff = max(sigma, aa * 0.5);
    let cov = 0.5 * (1.0 - erf(d / (1.4142135 * sigma_eff)));
    let mask = select(1.0, 0.0, clipped);
    return vec4<f32>(in.color.rgb, in.color.a * cov * mask);
}
