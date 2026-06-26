**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — WASM build + serve toolchain (wasm-bindgen, trunk, getrandom, size)

Getting a Bevy/Buiy app into a browser is a build-and-serve pipeline, not a code change. This file covers the toolchain: compiling to wasm, the bindgen step, the dev/prod serve tools, runtime crates (getrandom, web-time), diagnostics, and binary-size hygiene. Versions are the Bevy 0.19.0 lock pins: **wasm-bindgen 0.2.125**, web-sys 0.3.102, web-time 1.1.0, getrandom 0.3.4 + 0.4.3.

## Compile target + wasm-bindgen

The target is `wasm32-unknown-unknown` (`rustup target add wasm32-unknown-unknown`). `cargo build --target wasm32-unknown-unknown` emits a `.wasm` that exports/imports raw numeric ABI symbols — not usable from JS directly. **wasm-bindgen** post-processes it: it reads the `#[wasm_bindgen]`-generated custom sections and produces a JS shim (`*_bg.js`) + a trimmed `*_bg.wasm` that the browser can `import`. The crate `wasm-bindgen` and the `wasm-bindgen-cli` (which provides the `wasm-bindgen` binary) **must be the same version** — a mismatch yields runtime `LinkError: import object field '__wbindgen_…' is not a Function` in the browser console. Pin the CLI to the lock's crate version:

```sh
cargo install -f wasm-bindgen-cli --version 0.2.125   # match Cargo.lock exactly
```

(Latest published is 0.2.126 as of mid-2026; pin to whatever the lock resolves, not "latest".) The higher-level tools below run wasm-bindgen for you, but only the right *version* — keeping them updated is the usual fix for the LinkError.

## Dev vs prod serve tools

Three tools, three roles — the Bevy cheatbook is explicit that they are different layers:

- **`wasm-server-runner`** (dev loop, de-facto). "The easiest and most automatic way to get started" per the cheatbook — register it as the runner for the wasm target in `.cargo/config.toml` and `cargo run --target wasm32-unknown-unknown` builds, bindgens, and serves on a localhost port in one step. Best for the inner dev loop; it does no size optimization.
- **`trunk`** (the de-facto *app* bundler). Driven by a `Trunk.toml` + an `index.html` with a `<link data-trunk rel="rust"/>`; it runs cargo + wasm-bindgen, can chain `wasm-opt`, hashes assets, and serves with live-reload. The cheatbook lists it among "feature-rich […] but opinionated" higher-level alternatives. This is the usual choice for a deployable Buiy web build.
- **`wasm-pack`** — packages a wasm crate as an **npm module** for consumption by a JS/bundler app (webpack/vite). It is the library-publishing path, **not** the way you ship a standalone Bevy *app*; reach for it only if Buiy is embedded inside an existing JS frontend.

## index.html, the canvas, and assets

The app needs an HTML host with a `<canvas>` whose CSS selector matches the Bevy `Window { canvas: Some("#bevy".into()), … }` (see [winit-web](winit-web.md) for the binding). Minimal shape: a `<canvas id="bevy">`, a `<script type="module">` that imports the bindgen JS glue and calls its `init`/start export. Assets Bevy loads via the `AssetServer` are fetched over HTTP relative to the page; with trunk you copy the `assets/` dir next to the bundle (`<link data-trunk rel="copy-dir" href="assets"/>`). Note: Bevy's wasm asset reader uses `fetch`, and **directory/folder loading is unsupported on wasm** — load explicit file paths. http/https sources need `WebAssetPlugin` added *before* `AssetPlugin` (`bevy_asset/src/io/web.rs`). Buiy embeds its default font (`include_bytes!`), so text renders with no fetch at all.

## getrandom on wasm

getrandom has no ambient OS RNG on `wasm32-unknown-unknown`; it needs an explicit web backend. **As of getrandom 0.3.4, enabling the `wasm_js` Cargo feature alone selects the backend** — the old `RUSTFLAGS=--cfg getrandom_backend="wasm_js"` (or the `.cargo/config.toml` rustflag) is **no longer required** for the crate to compile. That rustflag still works as an explicit override even when the `wasm_js` feature is on, and `--cfg getrandom_backend=custom`/`=unsupported` remain the way to override the source for non-JS builds (getrandom 0.3.4 CHANGELOG). 0.4.x behaves the same way. Per the docs, `getrandom = { version = "0.3", features = ["wasm_js"] }`. At runtime the web backend calls `crypto.getRandomValues`, which — unlike `crypto.subtle` — is the one Web Crypto member usable from an **insecure context**, so it works over plain `http://` as well as https/`localhost` (no secure-context requirement).

Crucial Buiy detail (from [the feasibility report](../../reports/2026-06-25-wasm-browser-support-feasibility.md) §7): **getrandom is currently absent from Buiy's wasm *production* graph.** `cargo tree -p buiy --target wasm32-unknown-unknown -e no-dev -i getrandom` prints nothing — on web, winit drops `ahash` and `uuid` pulls no getrandom; the only getrandom is via `proptest` under `buiy_verify`, a dev-dependency that never ships. So the `wasm_js` feature is **not** needed today and would be premature to add. It becomes relevant only if future code re-activates getrandom on web (enabling `uuid`'s `js`/`v4`, ahash runtime-rng, or `rand`'s OS RNG) — at which point enable `wasm_js` for the relevant major.

## web-time replaces std::time

`std::time::Instant`/`SystemTime` panic or misbehave on wasm. **web-time** (1.1.0) is a drop-in that backs `Instant` with `performance.now()` and `SystemTime` with `Date.now()` on wasm, and re-exports `std::time` unchanged on native. Bevy already routes its `Time` through this via `bevy_platform`, so Buiy — which reads time only through Bevy's `Time` — needs no change. The rule for any new wasm-reachable code: never import `std::time` directly; go through `bevy_platform`/`web-time`.

## Diagnostics on web

- **`console_error_panic_hook`** — install it at startup (`console_error_panic_hook::set_once()`) so Rust panics print a readable message + stack to the browser console instead of a bare `RuntimeError: unreachable`.
- **Console tracing** — route `tracing`/`log` to the dev console. Bevy's `LogPlugin` does this on wasm (via `tracing-wasm` / a console layer); for a custom setup `tracing-wasm` or `console_log` works. Without it, `info!`/`warn!` go nowhere visible.
- **panic = abort is forced.** `wasm32-unknown-unknown` ships a std precompiled with `-Cpanic=abort` (historically there was no wasm unwinding; the exception-handling proposal only stabilized mid-2025 and still needs `-Zbuild-std` to use). So a panic **aborts the whole wasm instance** — there is no `catch_unwind` recovery. This raises the stakes on the panic paths Buiy can hit on web (e.g. an unshaped buffer at render-extract): one panic kills the app, not one frame.

## Binary-size hygiene

Bevy wasm binaries are large — **upwards of ~30 MB before trimming, reducible to ~15 MB with `wasm-opt`** per the cheatbook/community reports (figures vary by feature set; treat as order-of-magnitude). The standard stack:

- `Cargo.toml` release profile: `opt-level = "z"` (or `"s"`), `lto = true` (fat), `codegen-units = 1`, `strip = true`, `panic = "abort"`.
- **`wasm-opt -Oz`** (Binaryen) as a post-bindgen pass — "goes much further than the compiler can"; slow but the single biggest win. trunk can run it automatically.
- Serve **brotli/gzip-compressed** — wasm compresses well over the wire even though the decompressed module is large.
- A **loading screen** in `index.html` so the multi-MB fetch+compile is not a blank canvas.
- Audit features: drop default Bevy features you don't use; Buiy already runs Bevy `default-features = false` + single-threaded, which avoids the atomics/COOP-COEP path entirely.

## Implications for Buiy

- **P0 is a build-wiring milestone, not renderer work.** The one *compile* blocker is `arboard` (an ungated dep with no wasm backend); gate it under `cfg(not(wasm32))` and default `Clipboard` to `MemClipboard`. After that, `cargo build --target wasm32-unknown-unknown -p buiy` is the first gate.
- **Do not add the getrandom `wasm_js` feature pre-emptively** — it is latent (absent from the production graph). Adding it now is dead config; add it only when a dep re-activates getrandom on web.
- **Pin the wasm-bindgen CLI to the lock (0.2.125 here)** in any wasm CI lane and dev docs; version drift is the #1 cause of the browser-side `LinkError`.
- **panic=abort means every web-reachable panic is fatal.** The headless gate never runs the render world, so render-extract panics (historically a Buiy footgun) only show up in-browser. A wasm smoke-test lane (`cargo build --target wasm32` + a headless-browser load) belongs in CI before claiming web support; add `wasm32-unknown-unknown` to `deny.toml` `graph.targets` too.
- Use **trunk** for the deployable build (size pipeline + asset copy) and **wasm-server-runner** for the dev loop. wasm-pack is not the Buiy-app path. See [winit-web](winit-web.md) for the canvas/event side and [lessons](lessons.md) for cross-cutting takeaways.

## Sources

- Bevy cheatbook — Browser (WebAssembly): https://bevy-cheatbook.github.io/platforms/wasm.html (wasm-server-runner "easiest"; trunk/wasm-pack as higher-level alternatives)
- Bevy cheatbook — Optimize for Size: https://bevy-cheatbook.github.io/platforms/wasm/size-opt.html ; size figures: bevyengine/bevy#3800, #3978
- getrandom docs (wasm_js feature, 0.3.4 behavior, secure context): https://docs.rs/getrandom ; backend-auto discussion: https://github.com/TheBevyFlock/bevy_cli/issues/546
- wasm-bindgen / wasm-bindgen-cli version-match: https://crates.io/crates/wasm-bindgen-cli ; LinkError on mismatch: https://github.com/bevyengine/bevy/discussions/4888
- panic=abort default on wasm32-unknown-unknown: https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html
- Bevy web asset reader (fetch, no folder loading): `bevy_asset-0.19.0/src/io/web.rs`
- Buiy feasibility report (getrandom absent §7; binary-size/CI §8 P4; arboard B1 §4): `docs/reports/2026-06-25-wasm-browser-support-feasibility.md`
