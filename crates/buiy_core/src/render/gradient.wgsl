// Buiy background-gradient shader (octet ..07). Paints a 2-stop gradient fill
// inside a rounded-rect box — parity Wave B1 (linear) + B2 (radial / dotted
// grid). The fill geometry is the same rounded-rect SDF as shader.wgsl; the only
// difference is the fragment COLOR:
//   - LINEAR (B1): the 2-stop interpolation along the projected gradient axis.
//   - RADIAL (B2): a distance-to-center field. `axis` carries the TILE size —
//     nonzero repeats the radial once per tile (the viewport DOTTED-GRID bg: a
//     hard-edged 1px dot of color0 centered in every cell, color1 between),
//     zero is a single radial over the box. The 1px dot edge is smoothstep-AA'd
//     across a ~1px band at the radius, like the rounded-rect SDF rim.
// The two CPU-resolved, CPU-linearized stop colors are color0/color1.
//
// The instance record is the DISTINCT `GradientInstance` (NOT `PackedInstance`)
// — its own vertex layout — so R1/R2's 68 B quad stride is untouched and a
// non-gradient quad carries ZERO gradient bytes. The CPU precomputes the
// gradient axis (linear: a unit vector in the box's y-DOWN fragment space;
// radial: the tile size) and the gradient extent (line length / dot radius), so
// the shader does NO trig.
//
// Instance inputs are LOGICAL pixels; the shared view uniform
// (render::view_uniform::BuiyViewUniform) does the logical->clip transform in
// the vertex stage, identical to shader.wgsl / band.wgsl.

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

// MUST match `GradientInstance`'s #[repr(C)] field offsets + the
// `gradient_vertex_buffers` VertexBufferLayout byte-for-byte.
struct Instance {
    @location(2) rect_pos: vec2<f32>,    // box top-left, logical px
    @location(3) rect_size: vec2<f32>,   // box size, logical px (positive)
    @location(4) color0: vec4<f32>,      // stop 0 linear RGBA (start)
    @location(5) color1: vec4<f32>,      // stop 1 linear RGBA (end)
    @location(6) stops: vec2<f32>,       // [pos0, pos1] normalized 0..1
    @location(7) axis: vec2<f32>,        // unit gradient axis, y-down (sinθ,-cosθ)
    @location(8) gparams: vec2<f32>,     // [kind (0=linear,1=radial), line_len]
    @location(9) clip_min: vec2<f32>,    // logical px (-inf = none)
    @location(10) clip_max: vec2<f32>,   // logical px (+inf = none)
    @location(11) affine_col0: vec2<f32>,
    @location(12) affine_col1: vec2<f32>,
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local: vec2<f32>,       // box-local centered point, px (-half..+half)
    @location(1) half_size: vec2<f32>,   // SDF half-extent, px
    @location(2) color0: vec4<f32>,
    @location(3) color1: vec4<f32>,
    @location(4) frag_logical: vec2<f32>,// affine-transformed window-logical corner (clip discard)
    @location(5) clip_min: vec2<f32>,
    @location(6) clip_max: vec2<f32>,
    @location(7) stops: vec2<f32>,
    @location(8) axis: vec2<f32>,
    @location(9) gparams: vec2<f32>,
};

fn logical_to_clip(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.col0.x * p.x + view.col0.w, view.col1.y * p.y + view.col1.w);
}

@vertex
fn vertex(v: Vertex, i: Instance) -> VertexOut {
    var out: VertexOut;
    // Transform the box-local corner by the 2D affine before the logical->clip
    // view map (identity basis -> rect_pos + local). Same as shader.wgsl.
    let local_corner = v.uv * i.rect_size;                  // box-local, TL at 0
    let logical = i.rect_pos + mat2x2<f32>(i.affine_col0, i.affine_col1) * local_corner;
    out.clip_position = vec4<f32>(logical_to_clip(logical), 0.0, 1.0);
    out.local = (v.uv - vec2<f32>(0.5, 0.5)) * i.rect_size;  // centered px
    out.half_size = i.rect_size * 0.5;
    out.color0 = i.color0;
    out.color1 = i.color1;
    out.frag_logical = logical;
    out.clip_min = i.clip_min;
    out.clip_max = i.clip_max;
    out.stops = i.stops;
    out.axis = i.axis;
    out.gparams = i.gparams;
    return out;
}

// Signed distance to a rect centered at the origin (square corners — the B1
// gradient boxes the design uses carry their rounding via a separate Border;
// the gradient fill fills the box rectangle, matching the quad fill's coverage).
fn sdf_rect(p: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let q = abs(p) - half_size;
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0);
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    // Per-primitive clip AABB (identical to shader.wgsl). WebGPU/Tint requires
    // `fwidth` in UNIFORM control flow, so compute coverage unconditionally and
    // apply the clip as an alpha mask below (no early return). Behavior-identical
    // on native; native naga is lenient, Tint rejects the early-return-before-fwidth.
    let frag_pos = in.frag_logical;
    let clipped = any(frag_pos < in.clip_min) || any(frag_pos > in.clip_max);

    // Box coverage (analytic AA via fwidth on the rect SDF).
    let d = sdf_rect(in.local, in.half_size);
    let aa = fwidth(d);
    let cov = 1.0 - smoothstep(-aa, aa, d);

    let kind = in.gparams.x;
    var col: vec4<f32>;
    if kind < 0.5 {
        // LINEAR: project the centered box-local point onto the unit axis and
        // normalize across the CSS gradient-line length. The box center is
        // t=0.5; the 0%-stop / 100%-stop ends sit at ±line_len/2 along axis.
        let line_len = max(in.gparams.y, 1e-4);
        let t = clamp(0.5 + dot(in.local, in.axis) / line_len, 0.0, 1.0);
        // Map t through the two stop positions, then interpolate. (Design stops
        // are [0,1], so this is the identity; the remap supports off-edge stops.)
        let denom = max(in.stops.y - in.stops.x, 1e-4);
        let f = clamp((t - in.stops.x) / denom, 0.0, 1.0);
        col = mix(in.color0, in.color1, f);
    } else {
        // RADIAL. `line_len` (gparams.y) is the gradient extent in px — the DOT
        // RADIUS for the dotted-grid, else the box farthest-corner extent. The
        // `axis` slot carries the TILE size in px: nonzero = repeat the radial
        // once per tile (the viewport dotted-grid bg), zero = a single radial
        // over the box.
        let radius = max(in.gparams.y, 1e-4);
        let tile = in.axis;
        var dist: f32;
        if tile.x > 0.5 && tile.y > 0.5 {
            // Dotted-grid: map the box-local point (TL at -half_size) into its
            // tile cell, then measure the distance to the TILE CENTER (CSS
            // radial-gradient default center = the cell center). `frac` of the
            // tile-normalized coord gives the in-cell position; subtract 0.5 to
            // center, scale back to px.
            let tl = in.local + in.half_size;             // box-local, TL at 0
            let cell = (fract(tl / tile) - vec2<f32>(0.5, 0.5)) * tile;
            dist = length(cell);
        } else {
            // Single radial: distance from the box center over the extent.
            dist = length(in.local);
        }
        // Hard-edged dot of `color0` inside `radius`, `color1` (transparent for
        // the dotted-grid) outside — AA'd across a tight 1px-wide band at the
        // radius (the standard analytic disc coverage `clamp(radius - dist +
        // 0.5)`: full inside `radius-0.5`, zero past `radius+0.5`, a 1px linear
        // ramp between). A 1px-wide transition matches how the browser
        // rasterizes the CSS hard-edged 1px dot — wide enough to avoid jaggies,
        // tight enough that a 1px-radius dot still reaches near-full color at its
        // center (a `±1px smoothstep` would over-soften so small a dot).
        let inside = clamp(radius - dist + 0.5, 0.0, 1.0);
        col = mix(in.color1, in.color0, inside);
    }

    let mask = select(1.0, 0.0, clipped);
    return vec4<f32>(col.rgb, col.a * cov * mask);
}
