**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — WebGPU vs WebGL2 browser availability (mid-2026)

Which wgpu backend ([wgpu-backends](wgpu-backends.md)) you can rely on is dictated
by what browsers ship. As of mid-2026 the two web backends sit at very different
reach: WebGPU is broad-but-not-universal, WebGL2 is effectively everywhere.

## WebGPU availability (mid-2026)

WebGPU requires a **secure context** (HTTPS or `localhost`) everywhere. Per-engine:

- **Chrome / Edge desktop** — default-on since **v113** (April/May 2023), ChromeOS,
  macOS, Windows; stable continuously since. Edge tracks Chromium identically.
- **Chrome for Android** — default-on since **v121** (Jan 2024), initially Android 12+
  on Qualcomm/ARM GPUs, widening since.
- **Firefox** — Windows shipped default-on in **v141** (22 Jul 2025); macOS
  (Apple Silicon) followed in **v145**. **Linux and Android were still in progress
  as of early 2026** (Mozilla targeting Android sometime in 2026). So a Firefox
  user on Linux or Android cannot be assumed to have WebGPU.
- **Safari** — shipped in **Safari 26.0** (2025) on macOS Tahoe 26, iOS 26,
  iPadOS 26, visionOS 26. Earlier Safari (17.4–18.x) had it behind a flag only.
  So WebGPU on Apple devices requires the **2025 OS generation**.
- **Samsung Internet** — v24+.

Global coverage is roughly **82–85%** per caniuse in early 2026 — high, but the
missing slice (older Safari/iOS, Firefox on Linux/Android, older Android devices,
any browser on dated hardware/OS) is exactly the "long tail" a UI library cannot
assume away.

## WebGL2 availability

WebGL2 is **near-universal**. It is OpenGL ES 3.0 exposed to the canvas, shipped in
Chrome/Firefox since 2017; Safari was the long holdout and added it in **Safari 15**
(Sept 2021, macOS + iOS). Once Safari 15 landed, Khronos declared WebGL2 had
"pervasive support from all major web browsers." In practice caniuse lists it as
supported across effectively all current browsers (high-90s % globally). A WebGL2
fallback build therefore reaches essentially every browser a WebGPU build misses.

## The single-binary dual-backend gap

wgpu 29 *can* host both backends in one wasm binary and runtime-select via
`navigator.gpu` ([wgpu-backends](wgpu-backends.md), instance.rs:71-91). **Bevy 0.19
cannot** — it resolves one backend at compile time from the `webgpu`/`webgl` cargo
features, so each Bevy wasm artifact carries exactly one backend. Wiring wgpu's
runtime selection into Bevy is tracked upstream and **still open**:

- bevy#13168 — "Support WebGL2 and WebGPU in the same WASM file" (open).
- bevy#8315 — "Official WebGPU Support" (the umbrella issue; fallback-to-WebGL is
  discussed there).

Until that lands, **broad reach = two wasm artifacts + a tiny JS loader** that
feature-detects `navigator.gpu` and loads the WebGPU build when present, else the
WebGL2 build. This is the standard Bevy-on-web pattern today, not a Buiy-specific
workaround.

## Support table (mid-2026)

| Browser / platform | WebGPU | WebGL2 |
|---|---|---|
| Chrome / Edge desktop | ✅ since 113 (2023) | ✅ |
| Chrome Android | ✅ since 121 (2024), Android 12+ | ✅ |
| Firefox Windows | ✅ since 141 (Jul 2025) | ✅ |
| Firefox macOS (Apple Silicon) | ✅ since 145 | ✅ |
| Firefox Linux / Android | ⏳ in progress (2026) | ✅ |
| Safari macOS | ✅ Safari 26 (macOS Tahoe 26, 2025) | ✅ since 15 (2021) |
| Safari iOS / iPadOS | ✅ iOS/iPadOS 26 (2025) | ✅ since 15 (2021) |
| Samsung Internet | ✅ v24+ | ✅ |
| Older Safari/iOS, older Android, dated hardware | ❌ | ✅ (mostly) |

## Implications for Buiy

- **WebGPU-first MVP.** A WebGPU-only build reaches current Chrome/Edge/Firefox-
  Windows/Safari-26 — the fastest path to a widget painting in a canvas. It works
  as-is on Chrome/Dawn (which reports ~30 vertex attributes), but the 17-attribute
  band pipeline exceeds the WebGPU **spec baseline** of 16, so a conservative
  adapter (notably mobile, and possibly baseline Firefox/Safari) can fail to create
  it. Packing the band to ≤16 attributes (B2) is therefore advisable for
  cross-browser WebGPU reach, not only for the WebGL2 fallback.
- **WebGL2 fallback for reach.** To cover Firefox-on-Linux/Android, older Safari/
  iOS, and older Android, ship a second `webgl`-feature artifact. That build is
  what forces the two real WebGL2 fixes: the band pipeline must drop to ≤16 vertex
  attributes, and the `Rgba16Float` compositor targets need `EXT_color_buffer_float`
  or an `Rgba8Unorm` fallback (see [wgpu-backends](wgpu-backends.md) and
  [lessons](lessons.md)).
- **Plan for two artifacts + a JS feature-detect loader** from the start; do not
  expect a single binary to cover both until bevy#13168 lands.
- **Mobile caveat unrelated to the backend:** even where WebGPU/WebGL2 is present,
  a bare GPU canvas does not raise a soft keyboard and AccessKit reaches no web
  screen reader — reach is not the same as usability. See
  `docs/reports/2026-06-25-wasm-browser-support-feasibility.md` §6.

## Sources

- caniuse, WebGPU: https://caniuse.com/webgpu ; WebGL2: https://caniuse.com/webgl2
- web.dev, "WebGPU is now supported in major browsers": https://web.dev/blog/webgpu-supported-major-browsers
- gpuweb implementation status: https://github.com/gpuweb/gpuweb/wiki/Implementation-Status
- Mozilla Gfx blog, "Shipping WebGPU on Windows in Firefox 141": https://mozillagfx.wordpress.com/2025/07/15/shipping-webgpu-on-windows-in-firefox-141/ ; Firefox 141 release notes: https://developer.mozilla.org/en-US/docs/Mozilla/Firefox/Releases/141
- Chrome 121 (Android WebGPU): https://developer.chrome.com/blog/new-in-webgpu-121
- Khronos, WebGL 2 pervasive support: https://www.khronos.org/blog/webgl-2-achieves-pervasive-support-from-all-major-web-browsers
- bevy#13168 (single-WASM dual-backend): https://github.com/bevyengine/bevy/issues/13168 ; bevy#8315 (official WebGPU): https://github.com/bevyengine/bevy/issues/8315
- `docs/reports/2026-06-25-wasm-browser-support-feasibility.md` (§5, §6)
