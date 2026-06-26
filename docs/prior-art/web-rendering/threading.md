**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — threading model on wasm

The browser's default execution model is single-threaded: one JS event loop on the main thread. Bevy's wasm story follows that grain. Multi-threading on the web is possible but expensive (atomics, cross-origin isolation, nightly), and — crucially — **Bevy does not run its multi-threaded ECS scheduler on the web at all** as of 0.19. This file documents the model and what it costs; the bootstrap that depends on it is in [bevy bootstrap](bevy-bootstrap.md).

## Single-thread is the default *and* it is hard-wired on wasm

`bevy_tasks` default features are `async_executor` + `futures-lite` (`bevy_tasks-0.19.0/Cargo.toml`); `multi_threaded` is opt-in. But even when a downstream app *does* enable `multi_threaded`, it has no effect on wasm. The internal `multi_threaded` cfg-alias is defined as (`bevy_tasks-0.19.0/src/lib.rs:21-24`):

```rust
#[cfg(all(not(target_arch = "wasm32"), feature = "multi_threaded"))] => { multi_threaded }
```

The `not(target_arch = "wasm32")` guard means: **on wasm32 the multi-threaded pool is never compiled, regardless of the feature flag.** `lib.rs:93-104` then selects `single_threaded_task_pool` whenever that alias is off. So a wasm build always gets the single-threaded `TaskPool`, whose own doc comment (`single_threaded_task_pool.rs:30-33`) states it "just calls `wasm_bindgen_futures::spawn_local` for spawning which just runs tasks on the main thread." `Scope::spawn` drives futures inline on the calling thread. There is no parallel system execution.

## Real wasm threads are a heavy, separate opt-in

Getting actual OS-style threads in the browser is not a flag flip; it is a different build mode:

- **Recompile std with atomics.** `-C target-feature=+atomics,+bulk-memory,+mutable-globals`, on a **nightly** toolchain, with `-Z build-std=panic_abort,std`. (`std`'s prebuilt binaries are non-atomic.)
- **Cross-origin isolation.** `SharedArrayBuffer` — the backbone of wasm threads — is only exposed to a page that is cross-origin isolated, which requires the server to send `Cross-Origin-Opener-Policy: same-origin` **and** `Cross-Origin-Embedder-Policy: require-corp`. This is a Spectre mitigation, not a Bevy choice.
- **An adapter to spawn the workers.** `wasm-bindgen-rayon` taps Rayon's hooks to spawn wasm threads as Web Workers over `SharedArrayBuffer`; it requires wasm-bindgen's `--target web`.

Each of these leaks into deployment: nightly-only builds, special server headers, and an artifact that simply will not start threads on a non-isolated origin (e.g. an itch.io embed without SharedArrayBuffer enabled).

## Bevy 0.19 still does not multi-thread its scheduler on the web

Even with all of the above wired, Bevy's ECS scheduler stays single-threaded on web. The official Bevy CLI docs are explicit (current as of 2026): *"This **does not enable Bevy's multi-threaded scheduler**. The Bevy engine does not yet take advantage of multi-threading on the web, only certain 3rd-party crates do."* The wasm-multithreading support exists so that third-party crates (the docs name web-audio crates such as `firewheel-web-audio` / `bevy_seedling`) can use Web Workers — not to parallelize Bevy systems. The feature is marked **unstable / experimental**. So the threading effort buys a UI app essentially nothing today: the scheduler that runs Buiy's systems would not parallelize anyway.

## Bevy 0.19's cancellable-web-task fix (relevant even single-threaded)

Single-threaded does not mean async-free — the device init future (see [bevy bootstrap](bevy-bootstrap.md) §4) and any `IoTaskPool` work run on the browser executor. Bevy 0.19 fixed a long-standing wasm-task correctness bug here (PR #21795, *"Use `web_task` for cancellable wasm tasks"*). Previously the wasm pool handed futures straight to `wasm_bindgen_futures::spawn_local`, which gives back no handle, so a Bevy `Task` was a receipt with no power to cancel — dropping a task **silently leaked** its work on web while correctly cancelling it on native. 0.19 routes spawning through the `web-task` crate instead (`bevy_tasks-0.19.0/src/single_threaded_task_pool.rs:196-197`, `web_task::spawn_local(future)` under `cfg::web`; dependency pinned at `Cargo.toml:134`, `web-task = "1"`). `web-task` layers cooperative cancellation on the JS event loop — spawned tasks check an abort flag at each yield point — so dropping a `Task` now cancels it (matching the `spawn` doc at `single_threaded_task_pool.rs:182-187`). wasm `Task`s are now functionally equivalent to native ones.

## Cost / benefit for a UI library

A 3D engine has throughput-bound, embarrassingly parallel per-frame work (culling, animation, physics) that multi-threading speeds up. A UI library does not: its per-frame work — layout, extract, prepare, encode draws — is small, latency-sensitive, and dominated by the GPU and the browser's own compositor, not by CPU system parallelism. Against that, the wasm-threads tax is steep: nightly + `build-std`, COOP/COEP server headers, no startup on non-isolated origins, larger/duplicated artifacts, `Send`/`Sync` pressure on every shared type — to parallelize a scheduler that Bevy does not even parallelize on web yet. The cost/benefit is firmly negative for a UI lib in the browser.

## Implications for Buiy

**Single-thread is both correct and free for Buiy — it is already there.** Buiy builds Bevy with `default-features = false` and does not enable `multi_threaded` (feasibility report §2), so it is single-threaded on native today; on wasm the `bevy_tasks` cfg-alias above would force single-thread regardless of any future feature choice. This means:

- Buiy sidesteps the entire atomics + COOP/COEP + nightly + `build-std` rabbit hole for the wasm MVP — no special server headers, no nightly toolchain, no SharedArrayBuffer dependency.
- The one async path Buiy actually depends on (Bevy's async adapter/device init, [bevy bootstrap](bevy-bootstrap.md) §4) runs on that same single browser-main-thread executor and now inherits the 0.19 `web-task` cancellation fix for free.
- No Buiy code change is required for the threading model. The action item is the opposite of work: do **not** reach for `multi_threaded` on web. See [lessons](lessons.md) for the wasm items that *do* need work.

## Sources

- `bevy_tasks-0.19.0/src/lib.rs:14-24` — `multi_threaded` cfg-alias gated on `not(target_arch = "wasm32")`; `:93-104` pool selection
- `bevy_tasks-0.19.0/src/single_threaded_task_pool.rs:30-33` — wasm pool runs tasks on the main thread; `:182-187` `spawn` cancellation doc; `:196-197` `web_task::spawn_local` under `cfg::web`
- `bevy_tasks-0.19.0/Cargo.toml:134` — `web-task = "1"` (wasm32-only dependency)
- Bevy CLI docs, Wasm Multi-Threading (Unstable) — https://thebevyflock.github.io/bevy_cli/cli/web/multi-threading.html — "does not enable Bevy's multi-threaded scheduler … only certain 3rd-party crates do"
- Bevy PR #21795 "Use `web_task` for cancellable wasm tasks" — https://github.com/bevyengine/bevy/pull/21795
- web.dev, Using WebAssembly threads from C, C++ and Rust — https://web.dev/articles/webassembly-threads — `+atomics,+bulk-memory,+mutable-globals`, nightly, `build-std`, COOP/COEP
- wasm-bindgen-rayon — https://docs.rs/wasm-bindgen-rayon — Rayon over Web Workers + SharedArrayBuffer, `--target web`
- The wasm-bindgen Guide, Parallel Raytracing — https://rustwasm.github.io/docs/wasm-bindgen/examples/raytrace.html
- `docs/reports/2026-06-25-wasm-browser-support-feasibility.md` §2 — Buiy already single-threaded (bevy `default-features=false`, `multi_threaded` off)
