//! Offscreen **shell** capture (parity Wave C1 verification artifact). Renders the
//! unified gallery shell at the design preview size (1280×800) to an offscreen
//! texture on a real wgpu adapter (no on-screen window needed), reads the pixels
//! back, and writes `docs/reports/parity-proto-assets/c1-shell.png` — so the shell
//! can be eyeballed against the design. Mirrors `examples/capture`'s render-to-
//! texture + GPU readback path (the canonical `buiy_core` GPU golden path) but adds
//! the widget / animation / scroll / picking plugins the shell needs and boots the
//! dark theme.
//!
//! Run on a GPU host (this prototype's RX 6700 XT / lavapipe):
//!
//! ```sh
//! cargo run -p buiy_gallery --bin capture_shell
//! ```
//!
//! Not a CI gate — the headless `shell_layout` / `shell_router` tests are the
//! regression guards; this is the human-eyeball artifact.

use std::sync::{Arc, Mutex};

use bevy::asset::{AssetApp, RenderAssetUsages};
use bevy::camera::{CameraPlugin, RenderTarget};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Msaa;

use buiy_core::theme::{SetAccent, default_dark_theme};
use buiy_gallery::composites::ToastPlugin;
use buiy_gallery::inspector::{InspectorPlugin, build_inspector_content};
use buiy_gallery::shell::{
    Screen, ScreenRouter, build_shell, mount_screens, reflect_active_screen,
};
use buiy_gallery::{
    ModalPlugin, OverlayMenuPlugin, ScrollListPlugin, ShowcasePlugin, TodoMvcPlugin,
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
/// The default output path (the C1 whole-shell artifact). Override with the
/// `CAPTURE_SHELL_OUT` env var to write a per-wave artifact (e.g. C3's
/// `c3-todo.png`) without clobbering the C1 image.
const DEFAULT_OUT: &str = "docs/reports/parity-proto-assets/c1-shell.png";

/// Resolve the active screen from the `CAPTURE_SHELL_SCREEN` env var (default
/// `todo`). Used to capture a specific screen (e.g. `scroll` for the C3 Virtual
/// List artifact) without clobbering the default Todo capture.
fn capture_screen() -> Screen {
    match std::env::var("CAPTURE_SHELL_SCREEN")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "scroll" => Screen::Scroll,
        "menu" => Screen::Menu,
        "modal" => Screen::Modal,
        "showcase" => Screen::Showcase,
        _ => Screen::Todo,
    }
}

/// Resolve an optional accent swap from `CAPTURE_SHELL_ACCENT`
/// (blue|green|violet|coral). `None` keeps the boot accent (blue). Used by the
/// `c4-accent-green` artifact to prove the whole-app live re-theme.
fn capture_accent() -> Option<Color> {
    match std::env::var("CAPTURE_SHELL_ACCENT")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "blue" => Some(Color::srgb_u8(0x5b, 0x86, 0xf5)),
        "green" => Some(Color::srgb_u8(0x45, 0xc0, 0x7d)),
        "violet" => Some(Color::srgb_u8(0xb9, 0x8a, 0xff)),
        "coral" => Some(Color::srgb_u8(0xf0, 0x65, 0x5b)),
        _ => None,
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        // A window sized to the capture target so the primary-window-derived view
        // uniform matches the offscreen pixel grid (support/mod.rs note); the
        // shell root's `100%` also resolves against this window.
        .add_plugins(bevy::window::WindowPlugin {
            primary_window: Some(Window {
                resolution: bevy::window::WindowResolution::new(WIDTH, HEIGHT),
                ..default()
            }),
            ..default()
        })
        .add_plugins(bevy::asset::AssetPlugin::default())
        // `mount_screens` → `spawn_scene` needs the scene infrastructure.
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::render::RenderPlugin::default())
        .add_plugins(bevy::image::ImagePlugin::default())
        .add_plugins(CameraPlugin)
        // The `Core2d` graph `BuiyRenderPlugin` wires into — must precede it.
        .add_plugins(bevy::core_pipeline::CorePipelinePlugin)
        // `bevy::input` so `Res<ButtonInput<KeyCode>>` exists for any input-reading
        // widget/core system the paint path touches.
        .add_plugins(bevy::input::InputPlugin)
        // The Buiy headless render subset (theme → layout → core → text → focus →
        // a11y → widgets → render) the shell PAINT path needs, as ONE plugin — no
        // winit/picking/scroll/animation (a static capture forces widget state
        // directly; `focus` + `a11y` ARE included because the widgets `#[require]`
        // a11y components + read `FocusedEntity`).
        .add_plugins(buiy::BuiyHeadlessPlugin)
        // The shell's per-screen app logic (so the mounted screens are well-formed
        // — the same plugins the real binary adds).
        .add_plugins(TodoMvcPlugin)
        .add_plugins(ScrollListPlugin)
        .add_plugins(OverlayMenuPlugin)
        .add_plugins(ModalPlugin)
        .add_plugins(ShowcasePlugin)
        .add_plugins(ToastPlugin)
        // The C4 inspector logic (the switch-rebuild, live-state refresh, the
        // accent-swatch press → `SetAccent`, the swatch-ring reflect).
        .add_plugins(InspectorPlugin);

    // Reflect the captured screen into the viewport-header + status-bar labels.
    // The capture builds the world directly without `ScreenRouterPlugin` (which
    // owns this system in the live app), and `mount_screens`' `set_active_screen`
    // only toggles the screen-subtree visibility — not the header TEXT. Without
    // this, a non-Todo capture (`CAPTURE_SHELL_SCREEN=scroll`) shows the right
    // content but the boot screen's "TodoMVC" header. The router is fresh-inserted
    // (so `is_changed()` fires frame 0); this idempotent reflect makes the
    // verification artifact's header faithful to the captured screen.
    app.add_systems(bevy::app::Update, reflect_active_screen);

    app.init_asset::<Mesh>();
    // Bevy 0.19 `CameraPlugin` reads `Assets<SkinnedMeshInverseBindposes>` as a
    // non-`Option` param (panics if absent); real apps get it via `MeshPlugin`.
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();

    // Dark theme so the design tokens resolve.
    app.insert_resource(default_dark_theme());
    // The router resource `mount_screens` reads for the initial active screen.
    // Default Todo; `CAPTURE_SHELL_SCREEN` selects another (e.g. `scroll` for the
    // C3 Virtual List artifact) so the mount sets that screen active from frame 0.
    app.insert_resource(ScreenRouter(capture_screen()));

    // Offscreen `Rgba8UnormSrgb` target with `COPY_SRC` for readback.
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    image.asset_usage = RenderAssetUsages::all();
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);

    // Capture camera → offscreen target, clearing to the app background.
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::from(target.clone()),
        Msaa::Sample4,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(0x0b, 0x0c, 0x0e)),
            ..default()
        },
    ));

    // Build the shell tree (the same `build_shell` + `mount_screens` the binary
    // boots) + fill the C4 inspector content for the boot screen.
    {
        let world = app.world_mut();
        build_shell(world);
        mount_screens(world);
        build_inspector_content(world);
    }

    // Optional accent swap (the c4-accent-green artifact): `CAPTURE_SHELL_ACCENT`
    // = blue|green|violet|coral writes a `SetAccent` so the whole app re-themes
    // live (proving the swap re-extracts every accent-bearing paint). Default =
    // none (the boot blue).
    if let Some(accent) = capture_accent() {
        app.world_mut().write_message(SetAccent(accent));
    }

    // For the menu screen, capture it WITH THE DROPDOWN OPEN so the artifact shows
    // the design's `menuOpen:true` state: force the button `A11yExpanded(true)` +
    // the menu `CssVisibility::Visible`, then let the LIVE `Popover`/`Anchor`
    // pipeline place the dropdown below the ⋮ trigger. The dropdown's descendant
    // `Background` fills + `Text` glyphs now extract through its own stacking
    // context (the M1/M6 top-layer descendant-paint fix — a top-layer member forms
    // an SC, `layout/systems.rs` trigger 7), so no `Translate` stand-in is needed.
    app.finish();
    app.cleanup();
    if matches!(capture_screen(), Screen::Menu) {
        use buiy_core::a11y::A11yExpanded;
        use buiy_core::render::components::CssVisibility;
        let world = app.world_mut();
        let button = {
            let mut q = world.query_filtered::<Entity, With<buiy_widgets::MenuButton>>();
            q.iter(world).next()
        };
        let menu = {
            let mut q = world.query_filtered::<Entity, With<buiy_widgets::Menu>>();
            q.iter(world).next()
        };
        if let Some(button) = button {
            world.entity_mut(button).insert(A11yExpanded(true));
        }
        if let Some(menu) = menu {
            world.entity_mut(menu).insert(CssVisibility::Visible);
        }
    }

    // For the modal screen, capture it WITH THE CREATE MODAL OPEN (the design's
    // `modalOpen:true, modalMode:'create'` state): the centered `TopLayer::Modal`
    // overlay — its descendants paint through the same top-layer SC descent the
    // menu dropdown now uses (and the positioned dialog card paints behind its own
    // contents — the M6 paint-order fix) — so flipping `CssVisibility::Visible` +
    // setting the create body is enough.
    if matches!(capture_screen(), Screen::Modal) {
        use buiy_core::render::components::CssVisibility;
        use buiy_gallery::{ModalDialog, ModalMode, set_modal_mode};
        let world = app.world_mut();
        let dialog = {
            let mut q = world.query_filtered::<Entity, With<ModalDialog>>();
            q.iter(world).next()
        };
        if let Some(dialog) = dialog {
            world.entity_mut(dialog).insert(CssVisibility::Visible);
            // Author the create body open (title/sub/body/confirm = create face).
            set_modal_mode(world, ModalMode::Create);
        }
    }

    // Materialize the device + pipelines, then settle enough frames for layout →
    // extract → prepare → paint and the glyph/icon atlas to fill.
    for _ in 0..64 {
        app.update();
    }

    let out = std::env::var("CAPTURE_SHELL_OUT").unwrap_or_else(|_| DEFAULT_OUT.to_string());
    let pixels = readback_rgba(&mut app, target, WIDTH, HEIGHT);
    let img = image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels)
        .expect("readback buffer matches width*height*4");
    if let Some(parent) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(parent).expect("create output dir");
    }
    img.save(&out)
        .unwrap_or_else(|e| panic!("write {out}: {e}"));
    println!("wrote {out} ({WIDTH}x{HEIGHT})");
}

/// Spawn a `Readback`, observe its completion, poll frames until the bytes arrive,
/// then strip wgpu's 256-byte row padding. Returns un-padded RGBA8. (Mirrors
/// `examples/capture::readback_rgba` / `support::readback_rgba`.)
fn readback_rgba(app: &mut App, target: Handle<Image>, width: u32, height: u32) -> Vec<u8> {
    let slot: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let sink = slot.clone();
    app.world_mut().spawn(Readback::texture(target)).observe(
        move |trigger: On<ReadbackComplete>| {
            let mut s = sink.lock().expect("readback sink");
            if s.is_none() {
                s.replace(trigger.event().data.clone());
            }
        },
    );

    for _ in 0..60 {
        app.update();
        if slot.lock().expect("readback sink").is_some() {
            break;
        }
    }
    let raw = slot
        .lock()
        .expect("readback sink")
        .take()
        .expect("GPU readback delivered bytes within 60 frames");

    let unpadded = (width * 4) as usize;
    let padded = unpadded.div_ceil(256) * 256;
    let h = height as usize;
    if raw.len() == unpadded * h {
        raw
    } else if raw.len() == padded * h {
        let mut out = Vec::with_capacity(unpadded * h);
        for row in 0..h {
            let start = row * padded;
            out.extend_from_slice(&raw[start..start + unpadded]);
        }
        out
    } else {
        panic!(
            "readback returned {} bytes for {width}x{height} — expected {} or {}",
            raw.len(),
            unpadded * h,
            padded * h
        );
    }
}
