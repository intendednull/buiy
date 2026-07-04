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
    // clip_min.xy + clip_max.zw folded to ONE vec4 (freeing a slot for `style`
    // while staying at WebGL2's 16-attribute cap). -inf/+inf = the full-view
    // sentinel; the two reads become `.xy` / `.zw`.
    @location(13) clip: vec4<f32>,
    // Per-side dash-stipple flag [top, right, bottom, left] (F4b-3): 0 = solid
    // (continuous ring), 1 = dashed, 2 = dotted.
    @location(14) style: vec4<f32>,
    // Affine basis [m00, m10, m01, m11] as ONE vec4. Folded to keep the band
    // layout at 16 vertex attributes — WebGL2's `max_vertex_attributes` cap
    // (downlevel_webgl2_defaults). The two 2-col reads become `.xy` / `.zw`;
    // native/WebGPU behavior is identical.
    @location(15) affine: vec4<f32>,
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
    // Per-corner CIRCULAR radii `min(rx, ry)` for (TL, TR, BR, BL) — outer + inner.
    // A uniform radius packs all four equal, so the SDF reduces to the old
    // single-radius (TL.x) path byte-for-byte; a wide bordered pill (or a
    // per-corner `.radius_corners` wobble) rounds each corner independently instead
    // of drawing the pointed radius LENS (F4b-2). Widening the two f32 varyings to
    // vec4 keeps 12 `@location` slots — well under the WebGL2 varying budget.
    @location(10) outer_r4: vec4<f32>,
    @location(11) inner_r4: vec4<f32>,
    // Per-side dash-stipple flag [t,r,b,l] (F4b-3) + a representative stroke width
    // (max side, logical px) that sizes the dash period. Solid (all-zero) fragments
    // take the byte-identical continuous-ring path.
    @location(12) style4: vec4<f32>,
    @location(13) stroke_w: f32,
};

fn logical_to_clip(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.col0.x * p.x + view.col0.w, view.col1.y * p.y + view.col1.w);
}

@vertex
fn vertex(v: Vertex, i: Instance) -> VertexOut {
    var out: VertexOut;
    let local_corner = v.uv * i.rect_size;                  // box-local, TL at 0
    let logical = i.rect_pos + mat2x2<f32>(i.affine.xy, i.affine.zw) * local_corner;
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
    out.clip_min = i.clip.xy;
    out.clip_max = i.clip.zw;
    out.style4 = i.style;
    // A representative dash period comes from the widest side (uniform borders —
    // the common case — are exact; a mixed-width border approximates).
    out.stroke_w = max(max(i.width.x, i.width.y), max(i.width.z, i.width.w));
    // Per-corner CIRCULAR radius `min(rx, ry)` of each corner (TL, TR, BR, BL).
    // The `*_tl_tr` vec4 packs (TL.xy, TR.xy), `*_br_bl` packs (BR.xy, BL.xy). The
    // circular `sdf_rounded_rect` takes one radius per corner; `min(rx, ry)` is the
    // pill/circle behavior — a wide `border-radius:9999px` box clamps to `rx=half_w`
    // (huge) but `ry=half_h`, and using `rx` alone draws the pointed radius LENS
    // (the W3 lens bug, visible once a pill gains a border). `min` picks the
    // box-fitting radius so it pills; for a uniform circular radius (rx==ry) it is
    // byte-identical to the old TL.x path. Inner radii shrink with the width.
    out.outer_r4 = vec4<f32>(
        min(i.outer_radius_tl_tr.x, i.outer_radius_tl_tr.y),
        min(i.outer_radius_tl_tr.z, i.outer_radius_tl_tr.w),
        min(i.outer_radius_br_bl.x, i.outer_radius_br_bl.y),
        min(i.outer_radius_br_bl.z, i.outer_radius_br_bl.w)
    );
    out.inner_r4 = vec4<f32>(
        min(i.inner_radius_tl_tr.x, i.inner_radius_tl_tr.y),
        min(i.inner_radius_tl_tr.z, i.inner_radius_tl_tr.w),
        min(i.inner_radius_br_bl.x, i.inner_radius_br_bl.y),
        min(i.inner_radius_br_bl.z, i.inner_radius_br_bl.w)
    );
    return out;
}

// Signed distance to a rounded rect centered at the origin (port of
// shader.wgsl::sdf_rounded_rect / render_border_sdf.rs::sdf_rounded_rect).
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// Pick a fragment's corner radius from the per-corner (TL, TR, BR, BL) array by
// which quadrant of the centered box-local point it lies in (+x right, +y down,
// matching `local`). A uniform radius makes every branch equal (byte-identical to
// the old TL-only path); a wobble / pill radius rounds each corner independently.
fn corner_radius(p: vec2<f32>, r: vec4<f32>) -> f32 {
    if p.x < 0.0 {
        return select(r.w, r.x, p.y < 0.0);   // left: top=TL(.x), bottom=BL(.w)
    }
    return select(r.z, r.y, p.y < 0.0);       // right: top=TR(.y), bottom=BR(.z)
}

// Dash / dotted stipple coverage in [0,1] (F4b-3). Picks the fragment's dominant
// side (the same quadrant split the per-side color uses), its per-side style flag
// (`style4` = [top, right, bottom, left]: 0 solid, 1 dashed, 2 dotted), and the
// along-border coordinate; a screen-space arc-length pulse gives an AA'd dash.
// SOLID sides (flag 0) return 1.0 — the byte-identical continuous-ring path.
// Computed unconditionally (no branch before `fwidth`) so derivatives stay in
// uniform control flow (native naga is lenient, Tint/WebGPU strict).
fn dash_stipple(local: vec2<f32>, style4: vec4<f32>, stroke_w: f32) -> f32 {
    let ax = abs(local.x);
    let ay = abs(local.y);
    // Horizontal (top/bottom) sides run along x; vertical (left/right) along y.
    let horizontal = ay >= ax;
    let flag_h = select(style4.z, style4.x, local.y < 0.0);   // top=.x, bottom=.z
    let flag_v = select(style4.w, style4.y, local.x < 0.0);   // left=.w, right=.y
    let flag = select(flag_v, flag_h, horizontal);
    let t = select(local.y, local.x, horizontal);
    // dashed (flag 1): period 4·w, 50% duty. dotted (flag 2): period 2·w, 50% duty.
    let dashed = flag < 1.5;
    let period = max(select(2.0 * stroke_w, 4.0 * stroke_w, dashed), 0.001);
    let dash = 0.5 * period;
    let cell = fract(t / period) * period;   // px within one dash+gap cell
    let aa = max(fwidth(t), 0.001);
    let stipple = clamp(
        smoothstep(-aa, aa, cell) * smoothstep(-aa, aa, dash - cell),
        0.0,
        1.0,
    );
    // Solid side or a degenerate stroke ⇒ full coverage (byte-identical).
    let stippled = flag > 0.5 && stroke_w > 0.0;
    return select(1.0, stipple, stippled);
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    // Per-primitive clip AABB (identical to shader.wgsl): the OUTLINE clip is
    // the AncestorClip, so a ring outside an `overflow:hidden` box still paints.
    // WebGPU/Tint requires `fwidth` in UNIFORM control flow — compute the band
    // unconditionally and apply the clip as an alpha mask (no early return).
    // Behavior-identical on native; native naga is lenient, Tint rejects it.
    let frag_pos = in.frag_logical;
    let clipped = any(frag_pos < in.clip_min) || any(frag_pos > in.clip_max);

    // Band = inside(outer) AND NOT inside(inner). AA via fwidth on each SDF. The
    // corner radius is selected per fragment from the per-corner array (uniform
    // radius ⇒ the old single-radius path; wobble/pill radius ⇒ per-corner
    // rounding, no lens).
    let r_o = corner_radius(in.local, in.outer_r4);
    let r_i = corner_radius(in.local, in.inner_r4);
    let d_outer = sdf_rounded_rect(in.local, in.outer_half, r_o);
    let d_inner = sdf_rounded_rect(in.local, in.inner_half, r_i);

    let aa_o = fwidth(d_outer);
    let aa_i = fwidth(d_inner);
    let inside_outer = 1.0 - smoothstep(-aa_o, aa_o, d_outer);
    let inside_inner = 1.0 - smoothstep(-aa_i, aa_i, d_inner);
    // Coverage of the band = inside outer minus inside inner, clamped to [0,1],
    // then stippled for a dashed/dotted side (F4b-3; solid ⇒ ×1, byte-identical).
    let stipple = dash_stipple(in.local, in.style4, in.stroke_w);
    let band = clamp(inside_outer - inside_inner, 0.0, 1.0) * stipple;

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

    let mask = select(1.0, 0.0, clipped);
    return vec4<f32>(col.rgb, col.a * band * mask);
}
