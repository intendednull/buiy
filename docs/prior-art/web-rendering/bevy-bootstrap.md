**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — how Bevy boots a wasm app, end to end

A Buiy app on the web is a Bevy app on the web. Buiy builds on `DefaultPlugins`, so the **entire** browser bootstrap — event loop, canvas attach, surface creation, async device, backend selection — is Bevy's code, gated on one cargo feature (`webgl` or `webgpu`). Buiy writes none of it. This file traces that bootstrap against the Bevy 0.19.0 source on disk; line numbers are from `bevy_*-0.19.0` in the crates.io registry.

## 1. The event loop does not block the browser main thread

Native apps call `event_loop.run_app(...)`, which blocks the calling thread for the program's lifetime. A browser main thread cannot be blocked — you must return control to the JS event loop or the tab hangs. winit exposes `EventLoopExtWebSys::spawn_app`, which registers the app as a set of callbacks and returns immediately.

Bevy's runner picks the right one by target (`bevy_winit/src/state.rs:885-913`):

```rust
// winit_runner, state.rs:896-912
cfg_select! {
    target_arch = "wasm32" => {
        event_loop.spawn_app(runner_state);   // returns; browser drives the loop
        AppExit::Success
    }
    _ => { event_loop.run_app(&mut runner_state) /* blocks */ }
}
```

The in-source comment (`state.rs:894-895`) cites the winit docs: "use `spawn` instead of `run` on Wasm." This is why `App::run()` is the last line of a Bevy `main()` and still "returns" on web — `spawn_app` hands the loop back to the browser. Buiy's app shape (`App::new().add_plugins(DefaultPlugins).add_plugins(BuiyPlugin)…run()`) is already correct; nothing changes.

## 2. The canvas is bound by CSS selector

On wasm Bevy never opens an OS window; it attaches winit to an existing `<canvas>` you put in `index.html`. The `Window` component carries a `canvas: Option<String>` CSS selector. `bevy_winit/src/winit_windows.rs:279-301` (web-only block):

- `web_sys::window().document().query_selector(selector)` finds the element,
- `dyn_into::<web_sys::HtmlCanvasElement>()` casts it,
- `WindowAttributesExtWebSys::with_canvas(canvas)` binds winit to it,
- `.with_prevent_default(window.prevent_default_event_handling)` and `.with_append(true)` finish setup.

`fit_canvas_to_parent` is applied separately (`bevy_winit/src/system.rs:103`). You point Bevy at a canvas; you never construct a window — and Buiy constructs none of its own.

## 3. The surface comes from a raw handle — the same path as native

Surface creation is backend-agnostic. `bevy_render/src/view/window/mod.rs:362-388` (`create_surfaces`) builds a `SurfaceTargetUnsafe::RawHandle` from the window's `RawHandleWrapper` and calls `instance.create_surface_unsafe(...)`. On web the handle is `RawWindowHandle::Web` (the canvas), but it flows through the **identical** code as a native `Win32`/`Xlib`/`AppKit` handle — the canvas is just another raw handle. The only web-specific note is in the source itself (`mod.rs:384`): the call is "only fallible if the given window is a HTML canvas and obtaining a WebGPU or WebGL2 context fails."

## 4. Adapter + device init is async on the web; the handoff is `FutureRenderResources`

Requesting a GPU adapter and device returns a future on the web (the browser negotiates the WebGPU/WebGL2 context asynchronously). Native Bevy just blocks on it; web Bevy cannot. `bevy_render/src/settings.rs:259-301` (`create_render`) builds one `async_renderer` future wrapping `initialize_renderer(...).await`, then dispatches it by target:

```rust
// settings.rs:289-296
#[cfg(target_arch = "wasm32")]
bevy_tasks::IoTaskPool::get().spawn_local(async_renderer).detach();  // fire-and-forget on the main thread
#[cfg(not(target_arch = "wasm32"))]
bevy_tasks::block_on(async_renderer);                               // native: block until done
```

On wasm the future is detached onto the single-threaded browser executor (it cannot block the main thread); see [threading](threading.md) for what `spawn_local` resolves to (`web_task::spawn_local`). When it completes it writes its result into a shared `FutureRenderResources` cell (`bevy_render/src/lib.rs:501` `insert_future_resources` → `create_render` → `*future_resources.lock() = Some(resources)`, `settings.rs:286`).

Bevy gates plugin finalization on that cell. `RenderPlugin::ready()` (`lib.rs:446-449`) returns `true` only once the cell is populated; `RenderPlugin::finish()` (`lib.rs:452-466`) then `remove_resource::<FutureRenderResources>()`, `lock().take().unwrap()`s the resolved `RenderResources`, and `unpack_into(main_world, render_world, …)` — inserting `RenderDevice`, `RenderQueue`, `RenderAdapter`, `PipelineCache`, etc. (`settings.rs:184-219`). So on web the device simply appears later than on native, but it is guaranteed to exist by the time any plugin's `finish()` runs.

## 5. Backend + limits are one compile-time switch

`WgpuSettings::default()` (`bevy_render/src/settings.rs:71-97`) resolves the backend at compile time from cargo features:

- `webgl` + `wasm32` + not `webgpu` → `Backends::GL` (and forces `wgpu::Limits::downlevel_webgl2_defaults()`, `settings.rs:91-97`),
- `webgpu` + `wasm32` → `Backends::BROWSER_WEBGPU`,
- otherwise → `Backends::all()` (native auto-select).

`Backends::from_env()` (`WGPU_BACKEND`) overrides all of it (`settings.rs:84`). The bevy feature is named **`webgl`**, not `webgl2` — it maps to `wgpu/webgl` (`bevy_render/Cargo.toml:65-66`) and applies WebGL2 limits. Bevy's own "am I on WebGL2?" litmus is storage-buffer support: `storage_buffers_are_unsupported()` returns `max_storage_buffers_per_shader_stage == 0` (`bevy_render/src/lib.rs:584-586`). The two backends are mutually exclusive in one wasm binary; see [wgpu backends](wgpu-backends.md) for the WebGPU-vs-WebGL2 split and the downlevel limit table.

## Implications for Buiy

**Buiy adds zero bootstrap code, and that is by design.** Verified against `fdb8dda` (feasibility report §2-§3): Buiy never constructs a `Window`, never sets `WgpuSettings`/`Backends`, and never requests an adapter or device (grep across crates + examples = 0 hits). Bevy owns every step above. Buiy only ever *reads* `Res<RenderDevice>` — in its own `BuiyPlugin::finish()` and in its per-frame prepare systems — never a blocking adapter request.

That read-only stance is what makes Buiy safe on the async web path: `finish()` (step 4) is precisely where Bevy guarantees the async-initialized device exists, so Buiy reading `RenderDevice` there works identically on wasm and native. Buiy introduces **no** synchronous-adapter blocker.

The only Buiy-side bootstrap surface is configuration, not code: a web example crate sets the `Window` component's `canvas` selector / `fit_canvas_to_parent` / `prevent_default_event_handling` (step 2), and the build picks the `webgl` or `webgpu` cargo feature (step 5). See [lessons](lessons.md) for the wasm gotchas this inheritance does *not* paper over (arboard compile blocker, the 17-attribute band pipeline, screen-reader reach).

## Sources

- `bevy_winit-0.19.0/src/state.rs:885-913` — `winit_runner`, `spawn_app` vs `run_app`
- `bevy_winit-0.19.0/src/winit_windows.rs:279-301` — canvas binding by CSS selector
- `bevy_winit-0.19.0/src/system.rs:103` — `fit_canvas_to_parent`
- `bevy_render-0.19.0/src/view/window/mod.rs:362-388` — `create_surfaces`, `SurfaceTargetUnsafe::RawHandle`
- `bevy_render-0.19.0/src/settings.rs:71-97` — `WgpuSettings::default()` backend + limit selection
- `bevy_render-0.19.0/src/settings.rs:259-301` — `create_render`, async device init dispatch
- `bevy_render-0.19.0/src/settings.rs:184-219` — `RenderResources::unpack_into`
- `bevy_render-0.19.0/src/lib.rs:446-466` — `RenderPlugin::ready` / `finish` handoff; `:501` `insert_future_resources`
- `bevy_render-0.19.0/src/lib.rs:584-586` — `storage_buffers_are_unsupported`
- `bevy_render-0.19.0/Cargo.toml:65-66` — `webgl`/`webgpu` feature → `wgpu/webgl`,`wgpu/webgpu`
- `docs/reports/2026-06-25-wasm-browser-support-feasibility.md` §2-§3 — Buiy adds no bootstrap; reads `RenderDevice` only
