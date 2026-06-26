**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — asset and font loading in the browser

In a browser there is no filesystem and no system font directory. Every byte an app reads at runtime arrives one of two ways: **fetched over HTTP** (the page and its assets are served together; the loader issues a `fetch`), or **embedded at compile time** (`include_bytes!` / Bevy's `embedded_asset!`, baked into the `.wasm`). This file documents how the Bevy + wgpu stack supplies assets and fonts on web, and what Buiy inherits.

## Asset loading: `WebAssetPlugin` (HTTP sources)

Bevy 0.19 ships an HTTP asset source in `bevy_asset/src/io/web.rs` (upstreamed from the third-party `bevy_web_asset`). `WebAssetPlugin` registers `http://` and `https://` asset sources, gated behind the `http`/`https` cargo features:

- **Transport split by target.** A path beginning with `http`/`https` is loaded "via `fetch` (wasm) or `ureq` (native)" (`web.rs:17-18`). The wasm reader is `#[cfg(target_arch = "wasm32")]` and calls `.fetch_bytes(path)` (`web.rs:112-118`); the native reader uses `ureq` (`web.rs:122+`). Same `AssetReader` trait, two backends.
- **Ordering is load-bearing.** "Make sure to add this plugin *before* `AssetPlugin` to properly register http asset sources" (`web.rs:11`); adding it late only earns a runtime `warn!` (`web.rs:69-70`). In practice you set it on `DefaultPlugins` (`DefaultPlugins.set(WebAssetPlugin { .. })`, `web.rs:36`) so it lands before the default `AssetPlugin`.
- **No folder loading on wasm.** The HTTP source cannot enumerate a directory: its `is_directory`/`read_directory` (`web.rs:206-218`) cannot list a remote path, so `AssetServer::load_folder` does not work against an HTTP/web source. Asset discovery on web must be by explicit path.
- **Security note in-tree.** The plugin warns it is "potentially insecure" and asks callers to verify URLs before loading (`web.rs:65-67`).

Separately, Bevy's *default* asset source (the `assets/` folder) also resolves over HTTP on wasm — the bytes are fetched relative to the served page root. The practical pattern is therefore: **serve `assets/` next to `index.html`**, or embed the critical few. Embedding (`include_bytes!`) avoids an extra round trip and the 404-on-missing-path failure mode, at the cost of `.wasm` size — see [lessons](lessons.md) on binary size.

## Fonts: no system fonts in the browser

The browser exposes no font directory to wasm. `fontdb`'s system scan is unavailable; on wasm `load_system_fonts()` is a no-op. Fonts must arrive as **in-memory bytes** and be registered with `fontdb` via `load_font_data` / `Source::Binary` — sourced from `include_bytes!` or fetched `.ttf`/`.otf`.

**The CPU text engine runs on wasm.** cosmic-text shapes and lays out, and rasterizes each glyph with **swash**, entirely on the CPU — no platform text API, so it compiles to wasm cleanly. The reference cosmic-text→wgpu integration, **glyphon**, "shapes/calculates layout/rasterizes glyphs (with cosmic-text), packs glyphs into a texture atlas (with etagere), and samples the atlas (with wgpu)," and explicitly runs "on the web, compiled to WebAssembly." Bevy's own `bevy_text` is also cosmic-text-backed. So the exact engine Buiy depends on — cosmic-text 0.19 + swash 0.2.9 + fontdb 0.23 — is field-proven on web. swash emits 8-bit alpha images for monochrome glyphs and subpixel-RGBA images for color/emoji; all of it runs on the single web main thread (web is single-threaded by default — see [wgpu backends](wgpu-backends.md)).

**woff2 is not loadable directly.** Browsers ship web fonts as `.woff2` (whole-font Brotli compression + reverse table transforms on `glyf`/`loca`/`hmtx`). `fontdb` does **not** decompress them — its docs list "Font types support other than TrueType" as an explicit *non-goal*; it accepts only TrueType/OpenType (`.ttf`/`.otf`/`.ttc`). A `.woff2` must be decompressed back to an sfnt (TTF/OTF) — ahead of time, or at runtime via a Brotli/woff2 crate — **before** `load_font_data`. You cannot hand the browser's `.woff2` bytes straight to the stack.

**Color/emoji fallback.** swash can rasterize color glyphs (subpixel RGBA), but the browser supplies no emoji face, so a color-emoji font must be registered explicitly for emoji to render; the cosmic-text fallback chain selects it per-script.

## Implications for Buiy

- **Text works on web with zero font-supply work for the MVP.** Buiy embeds its default font: `include_bytes!("../../assets/fonts/FiraSans-Regular-latin.ttf")` under the default-on `default_font` feature (`crates/buiy_core/src/text/font_system.rs:22`), registered as `"Fira Sans"` and **pinned to all five generic families** (`registered_fonts_db`, `font_system.rs:117-134`). System-font scanning is opt-in and off by default — and `load_system_fonts` is a no-op on wasm regardless. So Latin text renders in the browser with no fetch, no filesystem, no font wiring (feasibility report §2).
- **Non-Latin coverage is a font-supply task, not an engine task.** Buiy's `BuiyFallback` names per-script families (Arabic/Hebrew/Devanagari/Han, `font_system.rs:76-87`) that resolve only if those faces are registered. On web there are no system fonts, so non-Latin scripts need their faces delivered as bytes (embed or fetch). The engine already handles them; the bytes have to arrive.
- **If Buiy ever fetches assets, mind the two web constraints.** `WebAssetPlugin` must precede `AssetPlugin`, and folder-loading does not work on wasm. Today Buiy embeds, so neither bites; both matter the moment a runtime `Source::Binary` font or an image is loaded from a URL.
- **A woff2 seam is optional, not required.** The feasibility report's P4 lists an optional woff2 path in a future `BuiyFontLoader`; because Buiy ships raw `.ttf`, no Brotli decompress is needed for the MVP.

## Sources

- `/home/intendednull/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_asset-0.19.0/src/io/web.rs` (lines 11, 17-18, 36, 65-70, 112-118, 122+, 206-218)
- `crates/buiy_core/src/text/font_system.rs` (lines 22, 76-87, 117-134)
- `docs/reports/2026-06-25-wasm-browser-support-feasibility.md` (§2, §7, P4)
- glyphon — fast 2D text renderer for wgpu (cosmic-text + swash + etagere; wasm support): https://github.com/grovesNL/glyphon and https://docs.rs/glyphon
- fontdb crate docs (non-goal: "Font types support other than TrueType"): https://docs.rs/fontdb
- WOFF File Format 2.0 (Brotli whole-font compression + table transforms → reconstructed sfnt): https://w3c.github.io/woff/woff2/
- fontTools woff2 (`decompress` WOFF2 → OpenType): https://fonttools.readthedocs.io/en/stable/ttLib/woff2.html
- Related: [`../cosmic-text/`](../cosmic-text/README.md), [`../bevy-egui/`](../bevy-egui/README.md)
