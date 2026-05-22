**Date:** 2026-05-22
**Status:** active
**Subject:** Coherent Gameface — Cohtml / Hummingbird vs embedded Chromium (CEF): performance + memory trade-offs

# Cohtml vs Chromium / CEF

## The case Coherent makes

Coherent's central technical argument — repeated across the product page, the all-features page, the "Using Chromium Embedded Framework" blog post, and the Renoir announcement — is that **embedded Chromium (CEF) is the wrong shape for game UI**, and that an **in-house HTML engine purpose-built for real-time integration with a game engine** is faster, leaner, and more controllable.

Coherent's public technical claims (clearly labeled as **vendor claims** — independently-verified game-by-game benchmarks are not available to this corpus):

| Claim | Coherent's wording (paraphrased) | Status |
|---|---|---|
| Main-thread frame completion | "10× better than Google Chrome on heavy pages" | Vendor marketing claim; no public independent benchmark |
| PS4 frame budget | "Sub-millisecond UI rendering, ~1ms demonstrated" | Vendor demo; conditions not specified |
| Renoir backend uplift | "15–70% rendering improvement depending on UI complexity" vs legacy backend | Vendor self-comparison; internal baseline |
| Memory | "Significantly lower memory footprint than CEF" | Qualitative claim; no public per-app number |
| IPC latency | "CEF's IPC adds several milliseconds for cross-process data flow" | Architectural critique; CEF docs confirm IPC exists |
| Threading | "Multi-threaded command generation; data-oriented design" | Confirmed in Renoir public docs |

Treat the **numbers** as marketing claims unless cross-referenced against an independent third-party benchmark. Treat the **architectural critiques of CEF** as substantively correct — CEF does run an out-of-process renderer with IPC, and that does add latency that single-process integration avoids.

## What Cohtml does structurally differently from Chromium / CEF

| Axis | Cohtml | Chromium / CEF |
|---|---|---|
| Process model | **Single-process**, in the host game's address space | Multi-process; renderer in a separate sandbox process; IPC for everything |
| Memory allocator | **Embedder-supplied** — Cohtml routes every allocation through host's allocator | Chromium's allocator; harder to attribute to a UI subsystem |
| GPU integration | **Direct** — Renoir emits commands into host's GPU command queue | OS-buffer compositor → texture share → game engine samples the texture |
| Threading | Embedder-controlled; UI lane on a thread the engine schedules | Renderer-thread + compositor-thread + GPU-thread inside CEF; engine waits on a texture |
| JS runtime | **V8 where licensing permits + alternate VM elsewhere**; one runtime per Cohtml view; in-process | V8; one renderer-process per page; cross-process |
| HTTP / networking | **None** by design; host engine owns | Full HTTP stack, cookies, cache, IndexedDB, service workers |
| Sandboxing | **None** — Cohtml runs with the game's process privileges | Site-isolation + sandbox; designed for untrusted web content |
| Browser chrome / nav | **None** | Full browser UI possible |
| Code size shipped | Small (Cohtml + Renoir are MB-scale per Coherent's marketing) | Chromium is 100s of MB embedded; CEF binary is large |
| Update cadence | Per Coherent's release cadence (typically quarterly minors + LTS) | Chrome's 4-week release cadence drives upstream pressure |

## Why a custom HTML engine?

Coherent's argument distilled:

1. **Game UI doesn't need a browser.** Game UI doesn't navigate URLs, doesn't display untrusted content, doesn't need a sandbox, doesn't need cross-origin isolation, doesn't need cookies, doesn't need a download manager. ~80% of the browser is dead weight.
2. **Game UI does need tight engine integration.** Direct GPU command sharing, custom allocator routing, host-controlled threading, frame-synchronized rendering — none of these are first-class in CEF, all of these are core to Cohtml.
3. **Game UI is performance-sensitive in different ways than a browser tab.** A browser optimizes for "smooth scrolling, low input lag." A game engine optimizes for "fits in the frame budget alongside the renderer + physics + AI + audio." Single-millisecond margins matter.

This case is **architecturally sound**. The same argument was made by Scaleform (Flash → game UI in the 2000s era), by NoesisGUI (XAML, see [`noesisgui/architecture.md`](../noesisgui/architecture.md)), and by RmlUi (HTML subset, see [`rmlui/architecture.md`](../rmlui/architecture.md)). Every successful game UI middleware accepts the "in-process, host-allocator, host-thread-scheduled" trade-off in exchange for being shippable in a 16ms frame budget.

The harder case is what Coherent uniquely sells: **a standards-compliant HTML5 + CSS3 + JS engine** with all the modern web-platform niceties Scaleform / NoesisGUI lack. That's the value-add. That's also where the maintenance cost lives — keeping Cohtml current with the web platform is an indefinite engineering commitment.

## What the trade-offs cost Coherent

1. **Maintenance pace.** The web platform moves quickly. Container queries (2022–2023), `:has()` (2023), CSS Nesting (2023), anchor positioning (2024), `oklch()` (2023), view transitions (2024), scroll-driven animations (2024). Each of these is a major spec absorbed by Chromium / WebKit / Gecko on their schedule; Cohtml must track or fall behind. The current Cohtml coverage (Flexbox full, Grid via custom-element, no container queries, no anchor positioning) reflects the lag. **Coherent's 14-year history (2012–2026) of operating an HTML engine reflects this cost as recurring.**
2. **Browser-engine bug parity.** Cohtml is a separate implementation; web content that "works in Chrome" may behave subtly differently. The "use standard tools, you can develop in Chrome" pitch (Coherent's marketing) has caveats — Coherent docs flag specific divergences (e.g., the all-elements-have-`flex` behavior; `calc()` not supported in `@keyframes`).
3. **JavaScript engine licensing.** V8 has a redistribution license that constrains binary distribution on some platforms (notably some console SDKs). Cohtml's documented dual-VM strategy (V8 + an alternate VM) reflects this. The non-V8 path is presumed less feature-complete and possibly slower.
4. **WebGL / canvas implementations.** Cohtml supports both, but a separately-implemented canvas + WebGL on a non-Blink engine is a multi-year project on its own.
5. **Accessibility tooling that depends on browser internals.** OS-AT bridges (UIA, AT-SPI, AXAPI) consume accessibility-tree data via the OS interfaces. Chromium exposes this. Cohtml uses **in-process TTS** instead (the `CohtmlARIA*` plugins) — a different shape, not directly bridged to OS AT. See [`html5-coverage.md`](html5-coverage.md) and [`critiques-and-open-problems.md`](critiques-and-open-problems.md).

## What this means for Buiy

Buiy's decision is **not "Cohtml or Chromium" — it's "no HTML engine at all."**

The Coherent vs Chromium argument is the right one **if you commit to an HTML-engine-driven UI**. The argument Buiy makes is one level up: **don't commit to an HTML engine.** Instead:

- BSN (`.bsn` asset format, foundation [`architecture.md` § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class)) is a Bevy-native reflection-driven asset, not an HTML/CSS/JS bundle. No parser, no DOM, no JS VM.
- Layout uses **Taffy** directly (Flexbox + Grid + Block + Float — better Grid story than Cohtml ships). No layout engine to maintain in-house.
- Text uses **cosmic-text** (harfrust + skrifa + unicode-bidi; better complex-script + BiDi story than Cohtml's docs surface). No text shaper to maintain in-house.
- Rendering uses **wgpu via Bevy's render graph**. No Renoir-equivalent abstraction layer needed; Bevy already abstracts wgpu, wgpu already abstracts the native graphics APIs.
- Logic uses **Rust ECS systems + observers + change detection**. No JavaScript runtime.
- Accessibility uses **AccessKit**. OS-AT bridges (UIA, AT-SPI, AXAPI, UI Automation on Android, AXSpeechRecognizer on iOS) are real. See foundation [`architecture.md` § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first).

This is a **fundamentally different architectural answer** to the same problem domain. Coherent's answer says "ship the web platform inside the engine, faster than CEF." Buiy's answer says "ship a feature-parallel UI library against curated Rust substrates, faster than embedding anything."

Both answers are reasonable. Coherent's answer scales to multi-engine + commercial-license customers. Buiy's answer scales to Bevy + open-source customers.

## The Cohtml-vs-Chromium argument is not the Buiy-vs-Cohtml argument

A reader skimming Coherent's marketing might conclude "OK, custom HTML engine beats embedded Chromium for game UI." That's a defensible position. **It does not follow that custom HTML engine beats native-Rust UI library** — the comparison is one axis over.

The closest analog to "Buiy vs Cohtml" is "egui vs Cohtml" or "iced vs Cohtml" or "GPUI vs Cohtml" — native UI library vs HTML-engine-based UI library. The native side typically wins on memory, on integration, on render-graph control; the HTML side typically wins on author productivity (familiar tools), on third-party-asset ecosystem (npm), on dynamic content (HTML templates feed by JS data).

Buiy's bet is that **BSN authoring + ECS-native widgets reach the author-productivity bar of HTML/CSS/JS** because (a) BSN is intentionally shaped like a declarative markup, (b) the LLM-tooling ecosystem already understands BSN-like patterns, (c) AccessKit and APG patterns give widgets the same semantic richness HTML does, and (d) the Bevy ecosystem aggregates third-party widgets into a shared discoverable surface. None of those are proven at AAA scale yet. Coherent Gameface is the existence proof that the *HTML-engine* path works at AAA scale; Buiy is the open-source attempt at a different path.

## Sources

- Coherent Labs CEF critique post — https://coherent-labs.com/posts/what-developers-should-consider-when-using-chromium-embedded-framework-cef-in-their-games/
- Renoir Graphics Library introduction — https://coherent-labs.com/posts/introducing-renoir-graphics-library/
- Coherent Gameface all-features page — https://coherent-labs.com/all-features-gameface/
- Coherent Gameface product page — https://coherent-labs.com/products/coherent-gameface/
- "How to get 60 fps UI on mobile?" — https://coherent-labs.com/posts/get-60-fps-ui-on-mobile/
- "Mobile game UI technology flies higher with Hummingbird" — https://coherent-labs.com/posts/why-are-we-developing-an-entirely-new-technology-for-mobile-game-ui/
- Coherent performance-benchmark community forum — https://coherentlabs.zendesk.com/hc/en-us/community/posts/16767604519197
- CEF Forum CEF performance discussion — https://magpcss.org/ceforum/viewtopic.php?f=10&t=181
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
