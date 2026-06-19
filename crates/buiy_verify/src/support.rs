//! GPU-capture glue for the reftest/golden tiers — the ONE place that names
//! the concrete app builder, so Phase 3 swaps it for `DeterministicApp` in a
//! single edit. `pub` so `tests/` integration tests reach it.
//!
//! It also owns the **adapter probe** ([`on_pinned_lavapipe`]): the single
//! source of truth for "is the SELECTED wgpu adapter the pinned lavapipe
//! (llvmpipe)?" that the committed-baseline golden gates consult. Stored goldens
//! are blessed against the pinned lavapipe (Mesa llvmpipe); on any other adapter
//! the pixels are non-comparable (`determinism.md` § "CI software-rasterizer
//! pin": *the local lane does not compare against the stored lavapipe
//! baseline*). So a committed-baseline EXACT comparison runs ONLY on lavapipe;
//! off it, the cell skips-as-pending instead of cross-rasterizer-failing.

use bevy::prelude::*;
use std::sync::OnceLock;

/// Build the headless painting app both reftest captures share. Phase 3 swapped
/// this single line from the bare `capture_app` seam to the
/// [`DeterministicApp`](crate::determinism::DeterministicApp) builder — the
/// `&mut App → RgbaImage` capture contract is identical, but every
/// nondeterminism knob (fixed virtual clock, Ahem sole-family, DPR pin,
/// MSAA/dither) is now pinned at the source. A reftest renders both halves in
/// one app run, so the staged Ahem registration drains in the first capture's
/// quiescence loop and the second half shares it.
pub fn reftest_app(logical_w: u32, logical_h: u32) -> App {
    crate::determinism::DeterministicApp::new(logical_w, logical_h).build()
}

/// Despawn the previous scene's spawned roots between the two captures so the
/// second scene renders alone. Keeps the camera + render-target entities.
pub fn clear_reftest_scene(app: &mut App) {
    let roots: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, (With<buiy_core::components::Node>, Without<ChildOf>)>()
        .iter(app.world())
        .collect();
    for e in roots {
        app.world_mut().entity_mut(e).despawn();
    }
}

/// The substring (case-insensitive) that identifies the pinned Mesa software
/// rasterizer in a wgpu adapter name/driver AND in the CI `WGPU_ADAPTER_NAME`
/// env contract. The device reports `llvmpipe (LLVM …)`; the CI install-mesa
/// action exports `WGPU_ADAPTER_NAME=llvmpipe`. (lavapipe is the *driver* name;
/// llvmpipe is the *device* name wgpu reports and the env contract pins.)
const LAVAPIPE_MARKER: &str = "llvmpipe";

/// Memoized result of [`on_pinned_lavapipe`] — the probe instantiates a wgpu
/// adapter (or reads the env), so a serialized GPU lane that calls it per cell
/// (`matrix_goldens`) pays it once.
static ON_PINNED_LAVAPIPE: OnceLock<bool> = OnceLock::new();

/// Is the SELECTED wgpu adapter the pinned lavapipe (Mesa llvmpipe)?
///
/// This is the gate for every **committed-baseline** golden comparison. Stored
/// goldens are blessed against the canonical CI rasterizer (pinned lavapipe);
/// on any other adapter the rim/AA pixels differ (commit `b869eba` records
/// `max_channel_delta=35` for this host's RX 6700 XT / RADV vs lavapipe), so an
/// EXACT stored-baseline comparison would cross-rasterizer-fail. `determinism.md`
/// § "CI software-rasterizer pin" cements that the local lane runs
/// rasterizer-internal checks, **not** the stored baseline — this predicate is
/// how a test honors that: compare against the committed corpus only when
/// `true`, otherwise skip-as-pending.
///
/// Two signals, in order (first decisive one wins):
///
///  1. **The CI env contract.** `.github/actions/install-mesa` exports
///     `WGPU_ADAPTER_NAME=llvmpipe` (and `VK_DRIVER_FILES` so the loader sees
///     *only* lavapipe). If `WGPU_ADAPTER_NAME` contains `llvmpipe`
///     (case-insensitive), the pin is active — return `true` without
///     instantiating an adapter (the CI fast path).
///  2. **A real adapter probe.** Otherwise build a minimal render app, finish it
///     (materializing the device + `RenderAdapterInfo` exactly as a capture
///     does — same plugin stack), and check whether the selected adapter's
///     `name`/`driver` contains `llvmpipe`. This is the source of truth on a
///     developer box: it returns `false` on the RX (RADV) and `true` if someone
///     locally points `VK_DRIVER_FILES` at lavapipe.
///
/// Memoized: the adapter is probed at most once per process.
pub fn on_pinned_lavapipe() -> bool {
    *ON_PINNED_LAVAPIPE.get_or_init(|| {
        // 1) The CI env contract — `WGPU_ADAPTER_NAME=llvmpipe` is the explicit
        //    signal the pin took effect; trust it without spinning up an adapter.
        if let Some(name) = std::env::var_os("WGPU_ADAPTER_NAME")
            && name
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains(LAVAPIPE_MARKER)
        {
            return true;
        }

        // 2) Probe the actually-selected adapter. Build the same render stack a
        //    capture uses (1×1 is enough — we only read RenderAdapterInfo), finish
        //    to materialize the device, then inspect the adapter name + driver.
        probe_selected_adapter_is_lavapipe()
    })
}

/// Build a minimal render app, finish it (materializing the wgpu device +
/// `RenderAdapterInfo`), and report whether the selected adapter is lavapipe.
/// Returns `false` if no render sub-app / adapter info materializes (no adapter
/// available) — the conservative answer, since "not provably lavapipe" must gate
/// OFF the stored-baseline comparison.
fn probe_selected_adapter_is_lavapipe() -> bool {
    use bevy::render::RenderApp;
    use bevy::render::renderer::RenderAdapterInfo;

    // Reuse the canonical capture stack so the probed adapter is byte-identical
    // to the one captures select (same RenderPlugin config, same env).
    let mut app = crate::determinism::DeterministicApp::new(1, 1).build();
    app.finish();
    app.cleanup();

    let Some(render_app) = app.get_sub_app(RenderApp) else {
        return false;
    };
    let Some(info) = render_app.world().get_resource::<RenderAdapterInfo>() else {
        return false;
    };
    adapter_info_is_lavapipe(&info.name, &info.driver)
}

/// Pure predicate: does this adapter `name`/`driver` pair identify lavapipe
/// (Mesa llvmpipe)? Split out so it is unit-testable without an adapter. The
/// device reports `name = "llvmpipe (LLVM 18.1.0, 256 bits)"` and
/// `driver = "llvmpipe"`; matching either (case-insensitive) on the marker
/// covers both how the software rasterizer surfaces.
fn adapter_info_is_lavapipe(name: &str, driver: &str) -> bool {
    name.to_ascii_lowercase().contains(LAVAPIPE_MARKER)
        || driver.to_ascii_lowercase().contains(LAVAPIPE_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lavapipe_marker_matches_real_device_strings() {
        // The exact strings the Mesa software rasterizer reports.
        assert!(adapter_info_is_lavapipe(
            "llvmpipe (LLVM 18.1.0, 256 bits)",
            "llvmpipe"
        ));
        // Driver-only match (defensive — the device-name field could vary).
        assert!(adapter_info_is_lavapipe("software", "llvmpipe"));
        // Case-insensitive.
        assert!(adapter_info_is_lavapipe("LLVMpipe", ""));
    }

    #[test]
    fn lavapipe_marker_rejects_hardware_adapters() {
        // This host's RX 6700 XT (RADV) — must NOT be mistaken for lavapipe, so
        // the stored-baseline comparison gates OFF (the #7 fix).
        assert!(!adapter_info_is_lavapipe(
            "AMD Radeon RX 6700 XT (RADV NAVI22)",
            "radv"
        ));
        assert!(!adapter_info_is_lavapipe(
            "NVIDIA GeForce RTX 3080",
            "nvidia"
        ));
        assert!(!adapter_info_is_lavapipe("", ""));
    }
}
