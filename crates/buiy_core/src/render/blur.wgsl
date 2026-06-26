// Buiy backdrop-blur shader (octet ..08, parity Wave B4). Implements a
// dual-Kawase blur (Kawase 2003 / Marius Bjørge, ARM GDC 2015): a downsample
// pyramid then an upsample pyramid, each tap a fixed 4–8 sample box-ish kernel
// whose effective radius doubles per pyramid level — O(log r) passes for an
// r-px blur instead of O(r) Gaussian taps.
//
// Three entry points share ONE uniform layout (`@group(0)`) + ONE sampled-source
// layout (`@group(1)`, the previous level's texture + a linear clamp sampler):
//
//   * `vertex`     — a full-target unit-quad pass-through (the same TL,BL,TR,BR
//                    TriangleStrip the composite VBO supplies). `uv` ∈ [0,1] over
//                    the DESTINATION; the source `texel` (1/source_size) scales
//                    the tap offsets.
//   * `down`       — dual-Kawase DOWNSAMPLE tap (13-sample, the ARM kernel):
//                    center + 4 diagonal (weight .5 total) + 4 axis-midpoint
//                    (weight .5 total). Run while shrinking the pyramid.
//   * `up`         — dual-Kawase UPSAMPLE tap (8-sample tent): 4 diagonal +
//                    4 axis, the standard reconstruction kernel. Run while
//                    growing back up.
//
// All sampling is in the source's own space (LINEAR `Rgba16Float` for the
// scratch pyramid), so the blur averages linear light — correct compositing
// (effect-compositor.md § 4: linear-space group math). The caller scales the
// per-pass `offset` by the pyramid level so the effective Gaussian sigma reaches
// the requested blur radius (see `render/blur.rs::kawase_offsets`).

// Blur params (`@group(0) @binding(0)`).
struct BlurParams {
    // 1/source_size in texels (xy); `.z` = the per-pass Kawase sample offset in
    // source-texel units (the half-texel step); `.w` padding.
    texel_and_offset: vec4<f32>,
    // The sub-rect of the SOURCE this pass reads, in normalized source uv
    // (min.xy, max.zw). The L0 down pass reads only the element's window region
    // (`min..max`); every deeper down/up pass reads the full previous level
    // (`0,0,1,1`). The destination's unit-quad uv [0,1] maps into this rect.
    src_rect: vec4<f32>,
};
@group(0) @binding(0) var<uniform> blur: BlurParams;

// The source texture (previous pyramid level) + a LINEAR clamp sampler
// (`@group(1)`) — linear filtering is load-bearing: each tap reads BETWEEN
// texels, so 13/8 bilinear fetches cover a far wider footprint than their count.
@group(1) @binding(0) var src_tex: texture_2d<f32>;
@group(1) @binding(1) var src_samp: sampler;

struct Vertex {
    @location(0) position: vec2<f32>,  // unit quad (unused; uv drives the corner)
    @location(1) uv: vec2<f32>,        // unit-quad uv in [0,1] over the DEST
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(v: Vertex) -> VertexOut {
    var out: VertexOut;
    // Map unit-quad uv [0,1] → NDC [-1,1] (y up): the pass fills the whole
    // destination attachment. `uv` passes through to the fragment to sample src.
    let ndc = vec2<f32>(v.uv.x * 2.0 - 1.0, 1.0 - v.uv.y * 2.0);
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = v.uv;
    return out;
}

// Convenience: one bilinear fetch of the source. `uv` is the DESTINATION's unit
// uv [0,1]; it is remapped into the source sub-rect (`src_rect`) before the tap
// offset `o` (in source texels) is applied. For a full-source pass `src_rect` is
// (0,0,1,1), so the remap is identity; for the L0 down pass it restricts the
// read to the element's window region.
fn fetch(uv: vec2<f32>, o: vec2<f32>) -> vec4<f32> {
    let src_uv = blur.src_rect.xy + uv * (blur.src_rect.zw - blur.src_rect.xy);
    return textureSample(src_tex, src_samp, src_uv + o * blur.texel_and_offset.xy);
}

// Dual-Kawase DOWNSAMPLE (Bjørge ARM 2015, 13-sample): center weighted .5,
// the 4 diagonal half-texel taps weighted .125 each (.5 total), the 4
// one-texel axis taps weighted .0625 each (... summing to 1.0 with the .5
// center + .5 diagonals; the canonical ARM weights). `off` is the per-pass
// offset scale (source texels).
@fragment
fn down(in: VertexOut) -> @location(0) vec4<f32> {
    let off = blur.texel_and_offset.z;
    let uv = in.uv;
    var sum = fetch(uv, vec2<f32>(0.0, 0.0)) * 4.0;
    // Half-texel diagonals (the dense inner ring).
    sum += fetch(uv, vec2<f32>(-off, -off));
    sum += fetch(uv, vec2<f32>( off, -off));
    sum += fetch(uv, vec2<f32>(-off,  off));
    sum += fetch(uv, vec2<f32>( off,  off));
    // One-texel axis taps (the outer ring), half-weighted via the 2× offset.
    sum += fetch(uv, vec2<f32>(-2.0 * off, 0.0)) * 0.5;
    sum += fetch(uv, vec2<f32>( 2.0 * off, 0.0)) * 0.5;
    sum += fetch(uv, vec2<f32>(0.0, -2.0 * off)) * 0.5;
    sum += fetch(uv, vec2<f32>(0.0,  2.0 * off)) * 0.5;
    // Total weight = 4 + 4 + 4·0.5 = 10.
    return sum / 10.0;
}

// Dual-Kawase UPSAMPLE (Bjørge ARM 2015, 8-sample tent): 4 axis taps weighted
// 2 (the "+" of the tent) + 4 diagonal taps weighted 1 (the "×"). Total 12.
@fragment
fn up(in: VertexOut) -> @location(0) vec4<f32> {
    let off = blur.texel_and_offset.z;
    let uv = in.uv;
    var sum = vec4<f32>(0.0);
    // Diagonal (×) — half-texel, weight 1.
    sum += fetch(uv, vec2<f32>(-off, -off));
    sum += fetch(uv, vec2<f32>( off, -off));
    sum += fetch(uv, vec2<f32>(-off,  off));
    sum += fetch(uv, vec2<f32>( off,  off));
    // Axis (+) — one-texel, weight 2.
    sum += fetch(uv, vec2<f32>(-2.0 * off, 0.0)) * 2.0;
    sum += fetch(uv, vec2<f32>( 2.0 * off, 0.0)) * 2.0;
    sum += fetch(uv, vec2<f32>(0.0, -2.0 * off)) * 2.0;
    sum += fetch(uv, vec2<f32>(0.0,  2.0 * off)) * 2.0;
    // Total weight = 4·1 + 4·2 = 12.
    return sum / 12.0;
}
