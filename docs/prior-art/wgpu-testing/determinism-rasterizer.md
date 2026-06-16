**Date:** 2026-06-14
**Status:** active
**Subject:** wgpu's pinned-software-rasterizer determinism recipe — lavapipe/llvmpipe/WARP frozen at one Mesa version, vendored via gfx-rs/ci-build, selected via VK_DRIVER_FILES

# The pinned software rasterizer (the determinism contract)

## The problem: real GPUs cannot be a reference image

wgpu's image-comparison tests need a renderer that produces **bit-stable output across machines and across time**. Real GPUs can't: driver versions, vendor quirks, and undefined behavior in shaders all perturb pixels. The chosen reference is Mesa's software stack:

- **lavapipe** — the `swrast` Vulkan driver (library `libvulkan_lvp.so`, ICD `lvp_icd.x86_64.json`), used on the **Vulkan** backend.
- **llvmpipe** — the Gallium OpenGL/GLES software rasterizer, used on the **GL** backend.
- **WARP** (`d3d10warp.dll`) — Microsoft's software D3D rasterizer, the Windows **DX12** analogue, installed via a separate `install-warp` composite action that shells out to `cargo xtask install-warp`. *(The exact WARP/NuGet package version is not visible from the action.yml alone — **unverified**.)*

**No software-Metal reference exists.** The recipe covers Vulkan/GL/DX12 only — there is no CPU Metal rasterizer, so **macOS Metal goldens are not deterministic under this model** (they'd run on a real Apple GPU/driver). This is load-bearing for Buiy, which targets macOS; see [open-problems.md § 8](open-problems.md).

Source: [`.github/actions/install-mesa/action.yml`](https://github.com/gfx-rs/wgpu/blob/trunk/.github/actions/install-mesa/action.yml).

## The load-bearing wart: why the daily PPA was abandoned

Originally wgpu CI `apt`-installed llvmpipe from `ppa:oibaf/graphics-drivers`, a **rolling daily** build. jimblandy filed [gfx-rs/wgpu#2594](https://github.com/gfx-rs/wgpu/issues/2594) (opened **April 13, 2022**):

> "Because `.github/workflows/ci.yaml` pulls the latest `llvmpipe` from `ppa:oibaf/graphics-drivers`, the specific version of llvmpipe we get varies from day to day, and when we happen to get a buggy version, we get CI failures that have nothing to do with the PR in question. We should instead pull a specific known-good (or known-adequate) version of LLVM pipe to run our CI tests against."

That is the entire rationale for pinning: **a rolling software-rasterizer is a moving reference image.** An unrelated llvmpipe regression landing upstream turns every PR's CI red overnight, decoupling test failures from the change under review. The fix was to stop pulling distro packages entirely and consume a **frozen, self-built Mesa**.

## The pinning mechanism: gfx-rs/ci-build

[gfx-rs/ci-build](https://github.com/gfx-rs/ci-build) ("Automated action for building/hosting components we need in CI") compiles Mesa from source on a tag push and publishes the result as a GitHub Release asset. From its `.github/workflows/artifacts.yml`, it downloads `https://archive.mesa3d.org/mesa-$MESA_VERSION.tar.xz` and runs:

```
meson setup builddir/ --buildtype=release -Dgallium-drivers=llvmpipe \
  -Dvulkan-drivers=swrast -Dplatforms= -Dglx=disabled
```

then tars `install/` into `mesa-$MESA_VERSION-linux-x86_64.tar.xz` and attaches it to the release (`softprops/action-gh-release@v1`). **Both rasterizers come from one source-pinned build with no DRI/GLX platform deps.** As of writing, `MESA_VERSION: "25.2.7"`, published as release **build26** (Nov 18); earlier `build20` = Mesa 24.3.4.

## How wgpu consumes the pinned build

The `install-mesa` action hardcodes the pin and documents the coupling verbatim:

```yaml
# Sourced from https://archive.mesa3d.org/. Bumping this requires
# updating the mesa build in https://github.com/gfx-rs/ci-build and creating a new release.
version:
  default: "25.2.7"
ci-binary-build:
  default: "build26"
```

On **Linux** it `curl`s `…/ci-build/releases/download/build26/mesa-25.2.7-linux-x86_64.tar.xz`, then — because "*The ICD provided by the mesa build is hardcoded to the build environment*" — **writes its own ICD JSON** pointing at the unpacked `libvulkan_lvp.so`, and exports:

```
VK_DRIVER_FILES=$PWD/icd.json
LD_LIBRARY_PATH=$PWD/mesa/lib/x86_64-linux-gnu/:$LD_LIBRARY_PATH
LIBGL_DRIVERS_PATH=$PWD/mesa/lib/x86_64-linux-gnu/dri
```

On **Windows** it pulls the prebuilt `mesa3d-$MESA_VERSION-release-msvc.7z` from a **third-party** repo, [pal1000/mesa-dist-win](https://github.com/pal1000/mesa-dist-win) (**not** a gfx-rs-controlled build — a supply-chain trust wart), extracts `vulkan_lvp.dll` + `lvp_icd.x86_64.json`, sets `VK_DRIVER_FILES` (via `cygpath --windows`) and `GALLIUM_DRIVER=llvmpipe`.

## Environment-driven adapter selection

`VK_DRIVER_FILES` (the modern replacement for the now-deprecated `VK_ICD_FILENAMES`; [Mesa envvars](https://docs.mesa3d.org/envvars.html)) forces the Vulkan loader to *only* see lavapipe, so the test harness **cannot accidentally pick a hardware GPU**. On the GL side `GALLIUM_DRIVER=llvmpipe` (paired with `LIBGL_ALWAYS_SOFTWARE=true` per Mesa docs) forces software GL. Within wgpu, `WGPU_ADAPTER_NAME` does a case-insensitive substring match over enumerated adapters ([`wgpu::util::initialize_adapter_from_env`](https://docs.rs/wgpu/latest/wgpu/util/fn.initialize_adapter_from_env.html)) to nail the exact device.

## The `LP_NUM_THREADS` myth — flagged, do NOT copy

A common external claim is that wgpu sets `LP_NUM_THREADS` to force single-threaded, deterministic FP accumulation. This is **not present** in the current `install-mesa/action.yml` (no such export). Mesa documents `LP_NUM_THREADS` verbatim as "*an integer indicating how many threads to use for rendering. Zero turns off threading completely. The default value is the number of CPU cores present*" — but does **not** characterize it as a determinism knob. llvmpipe tiles the framebuffer per-thread, so for a fixed tile assignment results are stable regardless of thread count. **No primary source shows wgpu pinning `LP_NUM_THREADS` for FP determinism** — treat that claim as **unverified / likely not how wgpu achieves determinism**. The determinism comes from the **pinned Mesa version**, not thread count.

## Recent churn: the warts stay live

Pinning trades day-to-day flakes for a **manual upgrade treadmill**: each bump requires a new ci-build release *and* an action edit. [gfx-rs/wgpu#8544](https://github.com/gfx-rs/wgpu/issues/8544) "Upgrade LLVMPipe in CI" (closed via PR #8582) shows the cost — a `Limits::blas_max_primitive_count` workaround (PR #8446) for an llvmpipe ray-tracing bug had to wait until **Mesa 25.2.7** fixed it before the limit could be restored. [#8727](https://github.com/gfx-rs/wgpu/issues/8727) ("SPIR-V writing for mesh shaders is broken on llvmpipe") shows the reference rasterizer itself still has feature gaps wgpu must route around. Earlier bump-tracking issues: "[Upgrade Mesa to 24.3.4 in CI #6988](https://github.com/gfx-rs/wgpu/issues/6988)".

## Implications for Buiy

The directly reusable pattern is three pieces:

1. A separate **"build-and-host-a-frozen-Mesa" repo** keyed by a single `MESA_VERSION` + release tag — and Buiy can consume `gfx-rs/ci-build`'s artifacts **directly** rather than building its own.
2. A composite action that downloads it and **writes its own ICD** (the upstream ICD path is build-host-absolute and unusable).
3. `VK_DRIVER_FILES` + `WGPU_ADAPTER_NAME` to make adapter choice deterministic and hardware-proof.

Bump the pin **deliberately, in a tracked issue**, and regenerate any golden images in that same PR. Do **not** copy a `LP_NUM_THREADS` determinism story. See [lessons.md](lessons.md) for the full Borrow/Avoid, and [open-problems.md](open-problems.md) for the supply-chain and non-conformance warts this carries.

## Sources

- `install-mesa/action.yml`: https://github.com/gfx-rs/wgpu/blob/trunk/.github/actions/install-mesa/action.yml (raw: https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/.github/actions/install-mesa/action.yml)
- `gfx-rs/ci-build`: https://github.com/gfx-rs/ci-build · releases: https://github.com/gfx-rs/ci-build/releases
- pal1000/mesa-dist-win (Windows source, third-party): https://github.com/pal1000/mesa-dist-win
- wgpu issue #2594 (abandon daily PPA): https://github.com/gfx-rs/wgpu/issues/2594
- wgpu issue #8544 (upgrade LLVMPipe) / #6988 / #8727: https://github.com/gfx-rs/wgpu/issues/8544 · https://github.com/gfx-rs/wgpu/issues/6988 · https://github.com/gfx-rs/wgpu/issues/8727
- Mesa envvars (`VK_DRIVER_FILES`, `LP_NUM_THREADS`, `GALLIUM_DRIVER`): https://docs.mesa3d.org/envvars.html
- `wgpu::util::initialize_adapter_from_env`: https://docs.rs/wgpu/latest/wgpu/util/fn.initialize_adapter_from_env.html
- Sibling files: [gpu-test-harness.md](gpu-test-harness.md), [image-compare.md](image-compare.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md), [glossary.md](glossary.md)
