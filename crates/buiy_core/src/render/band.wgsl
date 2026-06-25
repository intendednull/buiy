// Buiy border/outline BAND shader (octet ..06). Paints an outer-minus-inner
// rounded-rect band — the GPU form of the `render_border_sdf.rs` oracle: a
// fragment is painted iff it is inside the OUTER rounded rect AND outside the
// INNER (content) rounded rect. C6-a feeds the OUTLINE channel through this
// (the focus ring / selection outline); C6-b will feed per-side borders through
// the SAME record (BorderBandInstance) and shader.
//
// Instance inputs are LOGICAL pixels; the shared view uniform
// (render::view_uniform::BuiyViewUniform) does the logical->clip transform in
// the vertex stage, identical to shader.wgsl. The instance record is the
// DISTINCT `BorderBandInstance` (NOT `PackedInstance`) — its own vertex layout,
// so R1/R2's 68 B quad stride is untouched.

struct BuiyView {
    col0: vec4<f32>,
    col1: vec4<f32>,
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> view: BuiyView;

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

// MUST match `BorderBandInstance`'s #[repr(C)] field offsets + the
// `band_vertex_buffers` VertexBufferLayout byte-for-byte.
struct Instance {
    @location(2) rect_pos: vec2<f32>,      // outer box top-left, logical px
    @location(3) rect_size: vec2<f32>,     // outer box size, logical px
    @location(4) color_top: vec4<f32>,
    @location(5) color_right: vec4<f32>,
    @location(6) color_bottom: vec4<f32>,
    @location(7) color_left: vec4<f32>,
    @location(8) width: vec4<f32>,         // [top, right, bottom, left] px
    // Per-corner outer/inner radii (rx,ry) x4, flattened to vec4 pairs:
    // outer = (TLxy, TRxy, BRxy, BLxy); same for inner.
    @location(9) outer_radius_tl_tr: vec4<f32>,
    @location(10) outer_radius_br_bl: vec4<f32>,
    @location(11) inner_radius_tl_tr: vec4<f32>,
    @location(12) inner_radius_br_bl: vec4<f32>,
    @location(13) clip_min: vec2<f32>,     // logical px (-inf = none)
    @location(14) clip_max: vec2<f32>,     // logical px (+inf = none)
    @location(15) affine_col0: vec2<f32>,
    @location(16) affine_col1: vec2<f32>,
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local: vec2<f32>,         // box-local point in px, centered (-half..+half)
    @location(1) outer_half: vec2<f32>,    // outer SDF half-extent, px
    @location(2) inner_half: vec2<f32>,    // inner SDF half-extent, px
    @location(3) color_top: vec4<f32>,
    @location(4) color_right: vec4<f32>,
    @location(5) color_bottom: vec4<f32>,
    @location(6) color_left: vec4<f32>,
    @location(7) frag_logical: vec2<f32>,  // affine-transformed window-logical corner (clip discard)
    @location(8) clip_min: vec2<f32>,
    @location(9) clip_max: vec2<f32>,
    // The uniform corner radii (TL.x used; outline rings are uniform in C6-a).
    @location(10) outer_r: f32,
    @location(11) inner_r: f32,
};

fn logical_to_clip(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.col0.x * p.x + view.col0.w, view.col1.y * p.y + view.col1.w);
}

@vertex
fn vertex(v: Vertex, i: Instance) -> VertexOut {
    var out: VertexOut;
    let local_corner = v.uv * i.rect_size;                  // box-local, TL at 0
    let logical = i.rect_pos + mat2x2<f32>(i.affine_col0, i.affine_col1) * local_corner;
    out.clip_position = vec4<f32>(logical_to_clip(logical), 0.0, 1.0);

    let outer_half = i.rect_size * 0.5;
    // Symmetric inner half: outer minus the average L/R and T/B widths. For an
    // outline all four widths are equal, so this is `outer_half - width`.
    let inner_half = vec2<f32>(
        outer_half.x - 0.5 * (i.width.y + i.width.w),  // right + left
        outer_half.y - 0.5 * (i.width.x + i.width.z),  // top + bottom
    );
    out.local = (v.uv - vec2<f32>(0.5, 0.5)) * i.rect_size; // centered px
    out.outer_half = outer_half;
    out.inner_half = inner_half;
    out.color_top = i.color_top;
    out.color_right = i.color_right;
    out.color_bottom = i.color_bottom;
    out.color_left = i.color_left;
    out.frag_logical = logical;
    out.clip_min = i.clip_min;
    out.clip_max = i.clip_max;
    out.outer_r = i.outer_radius_tl_tr.x;                   // TL.x (uniform in C6-a)
    // Inner radius shrinks with the border width (the oracle's load-bearing
    // shrink); for a square ring (outer_r == 0) this stays 0.
    out.inner_r = i.inner_radius_tl_tr.x;
    return out;
}

// Signed distance to a rounded rect centered at the origin (port of
// shader.wgsl::sdf_rounded_rect / render_border_sdf.rs::sdf_rounded_rect).
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    // Per-primitive clip AABB (identical to shader.wgsl): the OUTLINE clip is
    // the AncestorClip, so a ring outside an `overflow:hidden` box still paints.
    let frag_pos = in.frag_logical;
    if any(frag_pos < in.clip_min) || any(frag_pos > in.clip_max) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Band = inside(outer) AND NOT inside(inner). AA via fwidth on each SDF.
    let d_outer = sdf_rounded_rect(in.local, in.outer_half, in.outer_r);
    let d_inner = sdf_rounded_rect(in.local, in.inner_half, in.inner_r);

    let aa_o = fwidth(d_outer);
    let aa_i = fwidth(d_inner);
    let inside_outer = 1.0 - smoothstep(-aa_o, aa_o, d_outer);
    let inside_inner = 1.0 - smoothstep(-aa_i, aa_i, d_inner);
    // Coverage of the band = inside outer minus inside inner, clamped to [0,1].
    let band = clamp(inside_outer - inside_inner, 0.0, 1.0);

    // Per-side color: pick the dominant edge by the centered local point. For an
    // outline all four colors are equal, so this reduces to the ring color; the
    // selection logic is here so C6-b's per-side borders reuse it unchanged.
    var col = in.color_top;
    let ax = abs(in.local.x);
    let ay = abs(in.local.y);
    if ay >= ax {
        if in.local.y < 0.0 { col = in.color_top; } else { col = in.color_bottom; }
    } else {
        if in.local.x < 0.0 { col = in.color_left; } else { col = in.color_right; }
    }

    return vec4<f32>(col.rgb, col.a * band);
}
