//! GPU lane (`--ignored`): the raster (textured-quad) primitive samples a
//! CPU-authored bevy `Image` byte-exactly onto a layout node. Authors an
//! `Rgba8UnormSrgb` canvas with distinct opaque texels (a red brush stroke, a
//! green fill, the paper ground), renders a `RasterImage` node covering the
//! capture view 1:1, reads the framebuffer back, and asserts each texel arrives
//! byte-exact at its pixel.
//!
//! **Adapter-independent.** Unlike the SDF/AA goldens, this is exact on ANY
//! conformant adapter (no pinned-lavapipe gate): a fully-opaque texel sampled
//! Nearest from an `Rgba8UnormSrgb` source, blended SrcOver over the opaque-black
//! clear, then sRGB-encoded to the `Rgba8UnormSrgb` target, round-trips to its
//! authored byte. So the exact assertions run on this host's real adapter.
//!
//! Run locally with: `cargo test -p buiy_core --test render raster -- --ignored`.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use buiy_core::components::Node;
use buiy_core::layout::Style;
use buiy_core::render::raster::RasterImage;

/// The canvas is `N×N` logical px, captured 1:1 so pixel `(x, y)` samples texel
/// `(x, y)` under the Nearest sampler (uv center `(x+0.5)/N` → texel `x`).
const N: u32 = 8;

const PAPER: [u8; 4] = [240, 240, 235, 255]; // the erased ground
const RED: [u8; 4] = [220, 40, 40, 255]; // the brush stroke
const GREEN: [u8; 4] = [40, 200, 60, 255]; // the fill

/// Author an `N×N` `Rgba8UnormSrgb` canvas: paper ground, a red texel at (1,1),
/// a green texel at (6,6). `RenderAssetUsages::all()` so the `GpuImage` is
/// created in the render world (and the main-world `data` survives the clone —
/// the `data.take()` trap the raster module documents).
fn author_canvas(app: &mut App) -> Handle<Image> {
    let mut image = Image::new_fill(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &PAPER,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    let data = image.data.as_mut().expect("new_fill populates image.data");
    let mut put = |x: u32, y: u32, rgba: [u8; 4]| {
        let i = ((y * N + x) * 4) as usize;
        data[i..i + 4].copy_from_slice(&rgba);
    };
    put(1, 1, RED);
    put(6, 6, GREEN);
    app.world_mut().resource_mut::<Assets<Image>>().add(image)
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); raster texel readback"]
fn raster_node_samples_authored_texels_byte_exact() {
    // `gpu_render_app` is the painting-capable stack (Core2d graph + LayoutPlugin)
    // sized to the capture view, so a `(Node, Style)` raster node lays out at
    // (0,0) and paints 1:1 into the offscreen target.
    let mut app = crate::support::gpu_render_app(N, N);
    let handle = author_canvas(&mut app);
    app.world_mut().spawn((
        Node,
        Style::default().width_px(N as f32).height_px(N as f32),
        RasterImage(handle),
    ));

    let target = crate::support::render_to_image(&mut app, N, N);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    // finish + several frames: the image uploads to a `GpuImage`, the raster
    // pipeline async-compiles, and the draw lands once the texture is resident
    // (the documented skip-until-resident resolves within a couple frames).
    crate::support::finish_and_run(&mut app, 5);

    let pixels = crate::support::readback_rgba(&mut app, target);
    assert_eq!(pixels.len(), (N * N * 4) as usize, "un-padded N×N readback");
    let px = |x: u32, y: u32| crate::support::px(&pixels, N, x, y);

    // Non-vacuous first: the canvas actually painted its distinct texels (a blank
    // / all-paper frame — e.g. the image never uploaded, or the draw never fired
    // — fails HERE on any adapter, independent of the exact byte checks below).
    assert!(
        pixels.chunks_exact(4).any(|p| p == RED),
        "the red brush texel painted somewhere (the raster draw fired)"
    );
    assert!(
        pixels.chunks_exact(4).any(|p| p == GREEN),
        "the green fill texel painted somewhere"
    );

    // Byte-exact per-texel mapping: pixel (x,y) == the authored texel (x,y). An
    // opaque Nearest texel over the opaque-black clear round-trips through the
    // sRGB encode to its authored byte — exact on any conformant adapter.
    assert_eq!(px(1, 1), RED, "brush texel at (1,1)");
    assert_eq!(px(6, 6), GREEN, "fill texel at (6,6)");
    assert_eq!(px(0, 0), PAPER, "paper ground at (0,0) — the erased state");
    assert_eq!(
        px(4, 4),
        PAPER,
        "paper ground at an untouched interior texel"
    );
}
