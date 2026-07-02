// Buiy coverage-glyph shader (octet ..03) — the alpha-as-color primitive
// (atlas-and-text-seam.md § 4.1). The atlas stores single-channel R8 coverage;
// color is applied per-instance, so one resident copy serves any tint and a
// theme color change never touches the atlas. The fragment is one line of
// intent: out = color * textureSample(atlas, samp, uv).r.
//
// Reuses the quad shader's `@group(0)` view uniform BYTE-IDENTICALLY (the same
// `BuiyView`, render::view_uniform::BuiyViewUniform) and the same clip-AABB
// fragment discard. The atlas texture+sampler are ADDITIVE on `@group(1)`,
// declared only by this (and the future icon) pipeline — quad/shadow never bind
// it (GPU-verify design fork #2).

struct BuiyView {
    // col0 = [sx, 0, 0, tx]; col1 = [0, sy, 0, ty]; clip = M*logical + t.
    col0: vec4<f32>,
    col1: vec4<f32>,
    // [scale_factor, pad, pad, pad]
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> view: BuiyView;

// The atlas page texture + its sampler (`@group(1)`). R8Unorm coverage sampled
// as a float; `.r` is the coverage in [0,1].
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_samp: sampler;

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,         // unit-quad uv in [0,1]
};

// GlyphAlphaInstance (render/atlas/primitive.rs), stride 84. Field order/offsets
// are the byte-level contract: rect @0, uv @16, color @32, clip @48, page @64,
// affine @68.
struct Instance {
    @location(2) rect: vec4<f32>,       // logical px: pos.xy (top-left), size.zw (positive)
    @location(3) uv_rect: vec4<f32>,    // atlas UV: min.xy, max.zw (normalized [0,1])
    @location(4) color: vec4<f32>,      // linear-light pre-linearized STRAIGHT-alpha tint (alpha-as-color; NOT premultiplied — frag scales only alpha)
    @location(5) clip: vec4<f32>,       // clip AABB: min.xy (-inf=none), max.zw (+inf=none)
    @location(6) page: u32,             // CoverageR8 page index (v1: single page bound)
    @location(7) affine: vec4<f32>,     // 2D affine basis cols [m00,m10,m01,m11]; identity [1,0,0,1] = axis-aligned
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) atlas_uv: vec2<f32>,   // interpolated UV into the atlas page
    @location(1) color: vec4<f32>,
    @location(2) frag_pos: vec2<f32>,   // logical px, window-relative (for the clip test)
    @location(3) clip_min: vec2<f32>,   // logical px (clip AABB, ClipRect space)
    @location(4) clip_max: vec2<f32>,   // logical px (clip AABB, ClipRect space)
};

fn logical_to_clip(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.col0.x * p.x + view.col0.w, view.col1.y * p.y + view.col1.w);
}

@vertex
fn vertex(v: Vertex, i: Instance) -> VertexOut {
    var out: VertexOut;
    let pos = i.rect.xy;
    let size = i.rect.zw;
    // Apply the 2D affine to the box-local corner, mirroring the quad/band path
    // (shader.wgsl / band.wgsl). The PRODUCER pre-rotates the instance origin
    // (`rect.xy = gt.transform_point(box_local_topleft)`), so `rect.xy + A*(v.uv*size)`
    // = `transform_point(box_local_corner)` — the whole run/icon rotates rigidly
    // about the entity's transform-origin. Identity `[1,0,0,1]` ⇒ `pos + v.uv*size`
    // (byte-identical to the pre-affine axis-aligned path).
    let logical = pos + mat2x2<f32>(i.affine.xy, i.affine.zw) * (v.uv * size);
    out.clip_position = vec4<f32>(logical_to_clip(logical), 0.0, 1.0);
    // Interpolate the atlas UV across the quad: v.uv in [0,1] maps the cell's
    // [uv_min, uv_max] rect (so the glyph's coverage cell is sampled, not the
    // whole page).
    out.atlas_uv = mix(i.uv_rect.xy, i.uv_rect.zw, v.uv);
    out.color = i.color;
    out.frag_pos = logical;
    out.clip_min = i.clip.xy;
    out.clip_max = i.clip.zw;
    // `page` rides the instance for multi-page selection; v1 binds a single
    // CoverageR8 page, so it is not yet consumed here (no array binding).
    _ = i.page;
    return out;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    // Per-primitive clip AABB: discard fragments outside [clip_min, clip_max] in
    // logical-px window space — the same encoding as shader.wgsl (±inf sentinel =
    // unclipped / top-layer, never fires).
    // WebGPU/Tint requires `textureSample` (implicit-LOD = a derivative op) in
    // UNIFORM control flow, so sample unconditionally and apply the clip as an
    // alpha mask rather than an early return. Behavior-identical on native;
    // native naga accepts the early return, Tint rejects it.
    let clipped = any(in.frag_pos < in.clip_min) || any(in.frag_pos > in.clip_max);
    // Alpha-as-color (§ 4.1): the R8 coverage modulates the per-instance linear
    // tint. The color is straight-alpha linear (matching the quad path); the
    // pipeline's ALPHA_BLENDING blends it SrcOver in linear space.
    let coverage = textureSample(atlas, atlas_samp, in.atlas_uv).r;
    let mask = select(1.0, 0.0, clipped);
    return vec4<f32>(in.color.rgb, in.color.a * coverage * mask);
}
