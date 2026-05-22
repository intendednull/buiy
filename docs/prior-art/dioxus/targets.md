**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — render targets: Web, Desktop (Webview / Blitz-WGPU), Mobile, SSR; per-target maturity

# Render targets

Dioxus's pitch is **"one codebase, every platform."** The mechanism is: a single VDOM-and-mutation-stream protocol consumed by per-target backend crates. The reality is that the targets are at very different maturity levels and carry meaningfully different feature sets. This file enumerates each.

## Maturity matrix (verified 2026-05-22)

| Target | Backend crate | Renderer | Status | A11y? | First-class since |
|---|---|---|---|---|---|
| Web (DOM/WASM) | `dioxus-web` | Browser DOM | **Production** — most mature | **Yes** (browser AT inheritance) | 0.1 |
| Desktop (Webview) | `dioxus-desktop` | wry/tao + system Webview | **Production** | Yes (via webview) | 0.1 |
| Desktop (WGPU/Blitz) | `dioxus-native` | WGPU + Blitz + Vello | **Pre-alpha** (DioxusLabs' own designation) | **No** (planned) | 0.7 (Oct 2025) |
| Mobile (Webview) | `dioxus-mobile` | wry + native shell | **Beta** | Partial | 0.6 (Dec 2024) — first-class CLI in 0.6 |
| Mobile (Native via Blitz) | `dioxus-native` + JNI/Swift bridge | WGPU + Blitz | **Experimental** | No | 0.7 |
| SSR | `dioxus-ssr` | String output | **Production** | n/a | 0.2 |
| Fullstack (SSR + hydrate) | `dioxus-fullstack` | Axum server functions | **Production** | Yes (web target post-hydrate) | 0.4 |

## Web (DOM/WASM)

The most mature backend and the historical center of gravity. Compiles to WASM via `wasm-bindgen`; mutations are applied to the browser DOM via a thin JS shim (the `sledgehammer` library, DioxusLabs-owned, does batched DOM mutation through tagged-pointer encoded commands for performance). Accessibility comes free from the browser's AT integration — semantic HTML elements (`button`, `nav`, `dialog`, `input`) are rendered as actual DOM elements and inherit the AT bindings.

**Strengths.** Mature, fast (close to Solid.js in JS frameworks benchmark per Dioxus blog), browser AT for free, mature dev experience.

**Weaknesses.** WASM bundle size — Dioxus 0.7 advertises "under 50kb" for simple apps but real-world apps with router + signals + a UI library are >300 KB compressed. Hot-reload via Subsecond is great, but the dev cycle starts slower than JS frameworks. WASM startup time is real on slow networks.

## Desktop (Webview)

`dioxus-desktop` wraps Tauri's underlying wry/tao stack: a native OS window with a system webview (WebKit on macOS, WebView2 on Windows, WebKitGTK on Linux) running the WASM bundle. The desktop binary is small (the WASM compiled in, plus the webview is system-supplied) but inherits webview-platform-specificity (WebView2 install state on Windows, WebKitGTK packaging on Linux).

**Strengths.** Small binary, native OS chrome, mature dev experience (same code as web), real native menus / tray / file dialogs via the desktop crate's APIs.

**Weaknesses.** Webview version skew across platforms — what renders on WebKit may not render identically on WebView2 (and vice versa). IPC overhead for any system call. Inherits webview a11y story (good on macOS/Windows; weaker on Linux WebKitGTK).

## Desktop (WGPU / Dioxus Native / Blitz) — pre-alpha

The big architectural bet of 0.7 (October 2025): **dioxus-native** runs WGPU directly, with no webview. Rendering is via **Blitz** — a DioxusLabs-stewarded HTML/CSS engine that combines:

- **Stylo** (Mozilla MPL-2.0 component, Firefox's CSS engine extracted by Servo) for CSS parsing and resolution.
- **Taffy** (cross-reference [`prior-art/taffy/`](../taffy/)) for box-level layout.
- **Parley** (Linebender) for text shaping and layout.
- **Vello** (Linebender) via the **Anyrender** abstraction for GPU rendering.

**Crucial correction to the brief.** Blitz is **not** a Servo-fork. The v0.1 line (pre-2025) was a Servo experiment; that line is archived. The current v0.2+ Blitz is an **independent** engine that reuses Stylo (the standalone CSS component) and Parley (Linebender's text crate) but is otherwise newly authored. The `stylo_taffy` glue crate uses MPL-2.0 for "easier interop with the Servo project" but Blitz itself is dual-licensed under MIT OR Apache-2.0.

**Status (own designation, per upstream README crawl 2026-05-22):** "**pre-alpha**. A very capable renderer, but there are also still many bugs and missing features. We do not recommend building production apps with it yet."

**Strengths.** GPU-driven rendering means no webview install dependency, smaller distributable, deterministic rendering across OSes, full control of the visual stack (gradients, effects, blending). Diegetic UI (rendering into 3D scenes) is possible via render-to-texture. AccessKit integration is on the roadmap.

**Weaknesses.** Pre-alpha by author admission. Feature gaps: many CSS properties incomplete, complex layout edge cases unfilled, animation primitives basic, no a11y yet. Stylo is heavy (Firefox-derived CSS engine; not trivial to ship in a small Rust binary). Vello requires a wgpu-supported GPU (no software fallback for old hardware).

## Mobile (Webview)

`dioxus-mobile` wraps `dioxus-desktop`'s webview approach with iOS/Android shell scaffolding (the `dx` CLI 0.6+ does `dx serve --platform ios/android`). The webview is iOS WKWebView or Android System WebView; native APIs (JNI on Android, ObjC on iOS) accessible via interop crates.

**Strengths.** Same codebase as web/desktop. First-class CLI support since 0.6 (Dec 2024). Single `main.rs` entrypoint.

**Weaknesses.** Inherits webview-on-mobile pain (Android WebView updates are user-/OEM-controlled; iOS WKWebView has its own quirks). App-Store ergonomics for hybrid apps are uneven. A11y is webview-dependent (better on iOS than Android).

## Mobile (Native / WGPU)

The same WGPU+Blitz path on iOS/Android via JNI/Swift bridges. **Experimental**; the same pre-alpha caveats as desktop-WGPU apply, plus mobile-GPU constraints (battery, thermal, driver variance).

## SSR

`dioxus-ssr` renders a `VirtualDom` to an HTML string. No JS runtime, no signals are re-runnable. Used by `dioxus-fullstack` for the server-render-first-then-hydrate-on-client flow.

## Fullstack

`dioxus-fullstack` (since 0.4, overhauled in 0.7 onto Axum) combines:
- Server-side rendering of the initial HTML
- Client hydration (the WASM bundle takes over the DOM rendered by the server)
- Server functions (Rust functions callable from the client; serialized over fetch)
- Suspense boundaries for async data
- Streaming HTML (since 0.6)

The 0.7 release brings WebSocket, SSE, streaming, and typed forms to server functions.

## Cross-target inconsistencies

Same `rsx!` code on different targets does not produce identical output. Concrete examples drawn from upstream issues and release notes:

- HTML elements supported in web/desktop-webview but unsupported (or partially supported) in Blitz-native — `<video>`, `<canvas>`, `<svg>`-with-filters, `<details>`/`<summary>` interactions, form-control native styling.
- Event-model differences — touch vs mouse vs pen handling diverges between webview-on-mobile and Blitz-on-mobile.
- Animation — CSS animations work in web/desktop-webview but Blitz's animation support is limited.
- Accessibility — DOM targets get AT integration; Blitz targets have **no AT integration as of 0.7.9** (AccessKit integration roadmapped but not shipped).
- Bundle size — Blitz embeds a CSS engine and a renderer; the binary is significantly larger than a pure-webview build for the same app.

These inconsistencies are the **multi-target fragmentation cost** — see [`open-problems.md`](open-problems.md) § "Multi-target fragmentation."

## Implications for Buiy

- **Single-codebase-multi-target is expensive.** Dioxus is the most credible attempt in Rust and it still ships a pre-alpha for its flagship 0.7 backend after a year-plus of work. Buiy's choice to target *only* Bevy (and inherit Bevy's WASM/desktop/mobile target story rather than build its own) is validated. See [foundation README § 1.3 non-goals — "non-Bevy frontends"](../../specs/2026-05-07-buiy-foundation/README.md).
- **Per-target a11y stories diverge.** DOM-target inherits browser AT for free; native targets do not. Buiy commits to AccessKit-first ([foundation architecture § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)) precisely because Bevy targets are all native — there is no "AT for free" tier.
- **Renderer-per-target multiplies maintenance.** Dioxus's web/desktop/native/mobile rendering paths each carry independent bugs. Buiy ships a single render-graph integrated into Bevy's; this is the correct simplification for Bevy.
- **WGPU is the right shape for the GPU substrate.** Blitz uses WGPU via Vello; Bevy uses wgpu directly. The substrate convergence is real — Linebender's Vello and Bevy's wgpu stack share enough that a future "Buiy uses Vello for vector rasterization" world is conceptually plausible (though not specced).

## Sources

- Dioxus repo README + targets list: https://github.com/DioxusLabs/dioxus/
- Blitz repo README (pre-alpha status, dependencies): https://github.com/DioxusLabs/blitz
- Dioxus 0.7 release notes (Dioxus Native / Blitz introduction): https://dioxuslabs.com/blog/release-070
- Dioxus 0.6 release notes (mobile first-class): https://dioxuslabs.com/blog/release-060
- Stylo (Mozilla/Servo): https://github.com/servo/stylo
- Vello (Linebender): https://github.com/linebender/vello
- Parley (Linebender): https://github.com/linebender/parley
- Sibling [`integration-with-taffy.md`](integration-with-taffy.md), [`open-problems.md`](open-problems.md)
- Cross-reference: [`../taffy/README.md`](../taffy/README.md) — Blitz is the single largest non-Bevy Taffy consumer.
