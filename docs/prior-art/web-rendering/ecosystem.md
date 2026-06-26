**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — who actually ships a Bevy/wgpu/winit-class Rust app to the browser

This file surveys the *field evidence* that the Bevy/wgpu/winit browser-rendering path works in practice: who builds on it, at what maturity, and where the honest limits of "shipped" are. It is the demand-side companion to [wgpu-backends](wgpu-backends.md) (the backend mechanics) and [bevy-bootstrap](bevy-bootstrap.md) (the in-engine wiring). Buiy-specific reads are in the marked sub-section and in [lessons](lessons.md).

## Bevy itself ships this path, but mostly at demo scale

Bevy's own example showcase at <https://bevy.org/examples/> compiles every example to wasm and runs it in the browser; the page is explicitly titled "Bevy Examples in WebGL2," and WebGL2 is still the default web backend when you build your own app. A second, separate WebGPU examples page exists for the `webgpu`-feature build. The toolchain is first-party: the Bevy CLI's `bevy build web` / `bevy run web` subcommands compile to wasm, run `wasm-bindgen`, and serve locally (<https://thebevyflock.github.io/bevy_cli/cli/web.html>), and Bevy has run example wasm builds in CI since bevy#4817. So the substrate Buiy inherits is exercised continuously by upstream. The honesty caveat: the *shipped* artifacts are overwhelmingly small games. itch.io's `platform-web` + `bevy` tag lists many jam-scale titles (Bevy Jam #1/#2 entries, plus games like Tunnet, Taipo, Orbital Tactics), not large production line-of-business apps. A "Bevy renders a complex UI in the browser at production scale" claim is **not** verifiable from public evidence; treat the proven envelope as "games and demos render reliably."

## egui / eframe — the most mature wgpu-on-web Rust UI precedent

egui via its `eframe` shell is the closest precedent to "a Rust *UI* (not a 3D game) rendering through wgpu in a browser canvas," and it is the most battle-tested. `eframe` defaults to the `wgpu` backend and, on web, **prefers WebGPU and falls back to WebGL2** automatically, configurable at runtime via `WebOptions::wgpu_options` (<https://docs.rs/eframe>, emilk/egui#5889). egui itself notes it compiles to wasm using "either WebGPU (when available) or WebGL2 for rendering, and almost nothing else from the web tech stack" — i.e. the same canvas-only posture Buiy targets. egui also drove much of the surrounding ecosystem work Buiy depends on: it is an AccessKit consumer, and its hidden-`<input>` IME workaround is the reference cited in the winit web-IME issue (see [open-problems](open-problems.md)). The live demo at <https://www.egui.rs/> runs in-browser. (Rerun is frequently cited as a production egui-on-web app; *unverified here* — left in the flagged list.)

## Makepad — GPU-only Rust UI, ships small to wasm/WebGL

Makepad renders its entire UI on the GPU and compiles to wasm + WebGL (the same WebGL2-class envelope), targeting browsers alongside native Metal/DX11/OpenGL backends (github.com/makepad/makepad). It reached a 1.0 milestone (HN #43971829) and reports wasm artifacts "on the order of just a few hundred kilobytes," a stark contrast to Bevy's tens of megabytes (see [open-problems](open-problems.md) on binary size). Makepad is evidence that a *custom* GPU UI renderer — not a general game engine — runs in the WebGL-downlevel subset, which is the architectural class Buiy is in.

## Vello — the counter-example: WebGPU/compute-only

Vello (linebender/vello) is the case that does **not** generalize to Buiy's WebGL2 plan. It is "a GPU compute-centric 2D renderer": it relies heavily on compute shaders and on WebGPU to run on the web, and is therefore unrunnable on WebGL2 (no compute). Its maintainers report it tested on production Chrome, with Firefox and Safari WebGPU still "experimental" from their perspective. Vello matters here as the explicit contrast to Buiy's [feasibility verdict](../../reports/2026-06-25-wasm-browser-support-feasibility.md): Buiy's pipeline has *zero* compute/storage, so unlike Vello it keeps a WebGL2 fallback on the table. It also anchors the Dioxus and (experimentally) the wider "Rust 2D on WebGPU" story below.

## Dioxus and gpui — adjacent, but NOT this path on the web

Two prominent Rust UI frameworks are easy to miscount as wgpu-on-web precedents; neither is. **Dioxus** on the web renders to the **DOM** via `web-sys` (`dioxus-web`), not to a wgpu canvas; its wgpu path is the *native* `dioxus-native`/Blitz renderer (Blitz layout + Vello/wgpu), which is experimental and runs outside the browser (github.com/DioxusLabs/blitz). So Dioxus is a DOM precedent on web, a wgpu precedent only on native. **gpui** (Zed's renderer) is native-only: Zed "renders through the GPU and does not use Electron, Chromium, or any web technology," and gpui-on-wasm is an unshipped community/experimental effort, not an officially supported target (zed-industries/zed#8203). Buiy should not cite either as proof that *its* class of renderer ships in a browser; egui and Makepad are the load-bearing precedents.

## cosmic-text / swash on the web

Buiy's text stack (cosmic-text 0.19 + swash + fontdb) is the same stack Bevy's text uses, and it reaches the browser through Bevy's wasm example builds — it is pure-Rust shaping/rasterization (rustybuzz/swash/fontdb) with no platform syscalls, so it compiles and runs on `wasm32-unknown-unknown`. Direct, dedicated "cosmic-text in a browser at scale" write-ups were **not** found (flagged), but the transitive precedent (Bevy UI text → wasm examples) is solid, and Buiy further de-risks it by embedding its default font (no fetch/filesystem needed; see the feasibility report §2).

### Implications for Buiy

The ecosystem says: the *substrate* (Bevy + wgpu + winit → canvas) is proven and continuously CI-exercised, but the *closest UI precedents that actually ship to browsers* are egui/eframe and Makepad — both of which (a) target the WebGPU-first-with-WebGL2-fallback envelope Buiy plans, and (b) had to solve the same web-IME and a11y gaps Buiy will hit ([open-problems](open-problems.md)). Buiy should lift egui's posture wholesale: prefer WebGPU, keep a WebGL2 fallback, and copy its hidden-input IME shim rather than wait on winit. Vello is the cautionary tale — a compute-only design forecloses WebGL2 reach; Buiy's compute-free pipeline is what keeps the two-backend strategy open, and that property is worth defending as a hard invariant. Do not over-claim: no public evidence shows this stack carrying a *large production UI* in-browser, so Buiy's web target should be scoped as "reach + parity demo," with production-grade web a11y/IME treated as open (XL) work, not a checkbox.

## Sources

- Bevy examples showcase (WebGL2 default): <https://bevy.org/examples/>
- Bevy + WebGPU (separate webgpu build): <https://bevy.org/news/bevy-webgpu/>
- Bevy CLI web subcommands: <https://thebevyflock.github.io/bevy_cli/cli/web.html>
- Run examples for WASM in CI: <https://github.com/bevyengine/bevy/issues/4817>
- Bevy web games (itch.io): <https://itch.io/games/platform-web/tag-bevy>
- eframe docs (wgpu default, WebGPU-preferred web backend): <https://docs.rs/eframe>
- eframe wgpu-as-default backend: <https://github.com/emilk/egui/issues/5889>
- egui repo + live web demo: <https://github.com/emilk/egui> · <https://www.egui.rs/>
- Makepad (wasm/WebGL, GPU-only): <https://github.com/makepad/makepad> · <https://news.ycombinator.com/item?id=43971829>
- Vello (compute-centric, WebGPU-only): <https://github.com/linebender/vello>
- Dioxus Blitz (native wgpu/Vello, not web): <https://github.com/DioxusLabs/blitz>
- gpui/Zed native-only, gpui-wasm experimental: <https://github.com/zed-industries/zed/discussions/8203>
- Buiy feasibility report: ../../reports/2026-06-25-wasm-browser-support-feasibility.md
