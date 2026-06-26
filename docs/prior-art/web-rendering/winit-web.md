**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — winit 0.30 web backend: event loop, canvas, input, IME

winit is the windowing/event layer Bevy (and therefore Buiy) runs on. On `wasm32-unknown-unknown` it backs a winit `Window` with an HTML `<canvas>` via `web-sys`, and rewires the event loop and input around the browser's constraints. Buiy writes none of this — it inherits it through Bevy's `DefaultPlugins` — but the seams below shape what works on web. Version on disk: winit **0.30.13** (the Bevy 0.19.0 lock pin).

## Non-blocking event loop: `spawn_app`, not `run_app`

Native winit drives the loop with `EventLoop::run_app(&mut app)`, which blocks the calling thread until exit. A browser tab cannot block its main thread, so winit's web extension `EventLoopExtWebSys::spawn_app(self, app)` exists instead: it *returns immediately* and hands control back to the browser, scheduling event delivery off browser callbacks rather than a blocking loop (`winit-0.30.13/src/platform/web.rs:158-230`). The doc comment is explicit: "Unlike `run_app()` […] this returns immediately, and doesn't throw an exception in order to satisfy its `!` return type" (`web.rs:162-184`). `run_app` is *not even available* on wasm when the target enables exception-handling.

Bevy selects this at `bevy_winit/src/state.rs:895-903`: `event_loop.spawn_app(runner_state)` on `target_arch = "wasm32"` vs `event_loop.run_app(&mut runner_state)` on native. That is why `App::run()` can be the last call in `main()` and still hand back to the browser. Poll cadence is governed by `PollStrategy` (default `Scheduler` — the Prioritized Task Scheduling API, falling back to `setTimeout`) and `WaitUntilStrategy` (`web.rs:317-364`).

## Canvas binding: `WindowAttributesExtWebSys`

The web window is a `<canvas>`. `WindowAttributesExtWebSys` (`web.rs:105-155`) adds four builder methods on `WindowAttributes`:
- `with_canvas(Option<HtmlCanvasElement>)` — adopt an existing canvas; `None` makes winit create one (which it does **not** auto-insert into the DOM).
- `with_prevent_default(bool)` — call `event.preventDefault()` on canvas events with side effects (wheel, etc.); **enabled by default**. Without it, mouse-wheel over the canvas scrolls the *page* instead of the app.
- `with_focusable(bool)` — make the canvas tab-focusable (sets `tabindex`); **enabled by default**, and *necessary to capture keyboard events* (`web.rs:123-127`).
- `with_append(bool)` — append the canvas to the page on creation; disabled by default.

Bevy wires these at `bevy_winit/src/winit_windows.rs:279-301`: it reads the `Window` component's `canvas: Option<String>` CSS selector, `document.query_selector(selector)`, `dyn_into::<HtmlCanvasElement>()`, then `.with_canvas(canvas)`; it sets `.with_prevent_default(window.prevent_default_event_handling)` and `.with_append(true)`. You point Bevy at an existing `<canvas id="…">` in `index.html`; you never construct a window.

winit's own docs warn **not** to apply `transform`, `border`, or `padding` CSS to the canvas — `Resized`, `CursorMoved`, `Touch`, and position events "can't take them into account and will therefore provide inaccurate results" (`web.rs:23-44`).

## Device-pixel-ratio, sizing, and resize

winit's web `scale_factor` is `window.device_pixel_ratio()` directly (`platform_impl/web/web_sys/mod.rs:52-54`). Pointer/wheel coordinates are converted through it (`web_sys/pointer.rs`, `web_sys/event.rs:156`), so winit hands Bevy *physical* pixels matching the canvas backing store. A `ResizeScaling` observer (`web_sys/resize_scaling.rs`) watches both element resizes and `devicePixelRatio` changes and re-emits `WindowEvent::Resized` / `ScaleFactorChanged`. Bevy's `fit_canvas_to_parent` (handled at `bevy_winit/src/system.rs:103-107`, reading `winit_window.canvas().style()`) makes the canvas track its parent element's size. Net: the canvas backing store is `logical_size × devicePixelRatio` — which on high-DPR mobile can exceed `max_texture_dimension_2d` and fail surface/target allocation (see [the feasibility report](../../reports/2026-06-25-wasm-browser-support-feasibility.md) §6 high-DPR crash).

## Raw-window-handle → wgpu surface (same path as native)

The canvas reaches wgpu as a raw handle. On web, `raw-window-handle` 0.6.2 yields `RawWindowHandle::Web(WebWindowHandle)`, a numeric id; winit's web backend tags the canvas with a matching `data-raw-handle` attribute so wgpu can locate it. Bevy's `create_surfaces` (`bevy_render/src/view/window/mod.rs:362-386`) builds `SurfaceTargetUnsafe::RawHandle` from the `RawHandleWrapper` and calls `instance.create_surface_unsafe(...)` — the **identical** code path as native; the canvas is just another raw handle. See [wgpu backends](wgpu-backends.md) for how that surface negotiates WebGL2 vs WebGPU.

## Input coverage on web

- **Pointer** — mouse, touch, and pen all arrive as `PointerEvent`s (`web_sys/pointer.rs`), surfaced as winit `CursorMoved`/`MouseInput`/`Touch`. Coordinates are DPR-scaled to physical pixels.
- **Keyboard** — delivered only while the canvas is focused, which is why `with_focusable` defaults on. winit maps `KeyboardEvent` to its `Key`/`KeyCode` model (`platform_impl/web/keyboard.rs`).
- **Focus** — canvas focus/blur become `WindowEvent::Focused`.
- **Wheel** — `WheelEvent` → `MouseWheel`; needs the canvas `prevent_default` (above) or the page scrolls.

### The IME gap (winit issue #4424, open)

winit emits **no** `Ime::Preedit` / `Ime::Commit` on web. The web `Window`'s `set_ime_allowed`, `set_ime_cursor_area`, and `set_ime_purpose` are all empty no-ops (`platform_impl/web/window.rs:327-337`), and the canvas backend installs no `CompositionEvent` listener. Tracking issue [rust-windowing/winit#4424 "Add IME support for Web (WASM)"](https://github.com/rust-windowing/winit/issues/4424) is open as of mid-2026: "IME input is not supported on Web (WASM) because the canvas element cannot receive `CompositionEvent`." The known workaround (used by egui) is a **hidden DOM `<input>` / `EditContext`** that captures composition outside winit and forwards committed text. Consequence: CJK, dead-key, and accented composition do not work through winit on web; only direct Latin keydown survives. A canvas also does not raise a mobile soft-keyboard without a focused DOM input, so phone text entry needs the same shim.

## Clipboard is not a winit concern on web

winit exposes no clipboard API on any platform; clipboard is out of scope for the windowing layer. Native apps reach the OS clipboard through a separate crate (Buiy uses `arboard`). On web the only path is `web-sys` `navigator.clipboard`, which is **async and permission/gesture-gated** — fundamentally incompatible with a synchronous `read()`/`write()` shape. So winit gives you nothing here; the web clipboard must be bridged separately. See [toolchain](toolchain.md) for the wasm build wiring and [lessons](lessons.md) for the cross-cutting takeaways.

## Implications for Buiy

- Buiy constructs no window and sets no `EventLoop` runner — Bevy's `spawn_app` path applies unchanged. Buiy's `App::new().add_plugins(DefaultPlugins).add_plugins(BuiyPlugin)…run()` shape is already correct for web.
- Buiy's wheel scrolling rides `bevy_picking` (`On<Pointer<Scroll>>` in `scroll.rs`), so it works on web **only if** the canvas `prevent_default`s wheel/touch — otherwise the page scrolls. Set `prevent_default_event_handling: true` on the `Window` (Bevy's default) and confirm touch is covered too.
- Buiy's E5 IME composition path is **inert on web** until a hidden-input/`EditContext` shim is built outside winit; this degrades safely (no crash) but blocks CJK/dead-key input and mobile soft-keyboards. This is a Buiy-owned effort, not an upstream winit fix to wait on for the MVP.
- Buiy's `macos`-modifier branch keys off `cfg!(target_os = "macos")`, which is `"unknown"` on wasm — a Mac user in a browser silently gets Ctrl-not-Cmd. Fixing it needs **runtime** platform detection (winit/web does not surface this); the keyboard events themselves arrive fine.
- Buiy's clipboard facade (`ClipboardProvider` + `MemClipboard`) is the right shape: on web, `arboard` cannot be the backend, and winit offers nothing, so the in-app `MemClipboard` is the honest v1; a real bridge over async `navigator.clipboard` is later, separate work.

## Sources

- winit 0.30.13 web platform module: `winit-0.30.13/src/platform/web.rs:23-44,105-230,317-364`
- winit web window no-op IME: `winit-0.30.13/src/platform_impl/web/window.rs:327-337`
- winit web scale factor / resize: `winit-0.30.13/src/platform_impl/web/web_sys/mod.rs:52-54`, `web_sys/resize_scaling.rs`, `web_sys/pointer.rs`, `web_sys/event.rs:156`
- Bevy event-loop runner select: `bevy_winit-0.19.0/src/state.rs:895-903`
- Bevy canvas binding: `bevy_winit-0.19.0/src/winit_windows.rs:279-301`; fit-canvas: `bevy_winit-0.19.0/src/system.rs:103-107`
- Bevy surface creation: `bevy_render-0.19.0/src/view/window/mod.rs:362-386`
- winit IME-on-web tracking issue: https://github.com/rust-windowing/winit/issues/4424
- Buiy feasibility report: `docs/reports/2026-06-25-wasm-browser-support-feasibility.md` (§6 IME/soft-keyboard/clipboard/high-DPR)
