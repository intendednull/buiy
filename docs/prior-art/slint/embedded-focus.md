**Date:** 2026-05-22
**Status:** active
**Subject:** Slint — embedded target: MCU support (ESP-IDF, STM32), software renderer, RGB565 framebuffers, no_std, low-allocation rendering

# Embedded focus

Slint's identity is "embedded-first," in a way no other production Rust GUI toolkit claims. The crates.io categories include `no-std`; the partner page leads with Espressif, STMicroelectronics, Toradex, and Raspberry Pi; the Rust Foundation member spotlight describes customers in industrial automation, medical devices, automotive, and aerospace. This file walks the embedded story so a Buiy designer can extract the allocation-discipline lessons.

Buiy itself is **not** an embedded target — the foundation spec ([`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)) targets Bevy desktop + mobile (web/WASM open). But the embedded-grade allocation discipline Slint had to develop carries over directly to large-tree productivity-app UIs, and is the technical-discipline lesson worth taking forward.

## The MCU target

Slint compiles to bare-metal microcontrollers via the `no_std` Cargo configuration. The runtime crate's MCU profile drops all `std`-dependent code: no thread pool, no async runtime, no dynamic loader, no filesystem; statically-sized arenas for the few cases that would have used `Vec` / `Box`; compile-time-bounded layout arenas where possible.

Documented MCU stack examples:

- **ESP-IDF** (Espressif's ESP32 family) — first-class support since 1.4 (early 2024); RGB565 framebuffer rendering; covers ESP32-S3 dev boards with built-in displays. CHANGELOG entries reference RGB565 in 1.5.0 onward.
- **STM32** — first-class support added in 1.8 (September 2024). STM32H7 and STM32F7 family typical targets.
- **LinuxKMS** — direct framebuffer rendering on Linux without a window manager, useful on embedded Linux devices. GPU support added in 1.16.0 (April 2026).
- **Zephyr / QNX** — supported as partner OS targets per [https://slint.dev/partners](https://slint.dev/partners). QNX is the safety-critical real-time OS deployed in 255M+ vehicles; Slint's Rust bindings on QNX enables in-vehicle HMI without C++ Qt.

## The software renderer

Slint ships a software (CPU) renderer that doesn't require a GPU at all. The renderer:

- Operates on a frame buffer in user-chosen pixel format (RGB565, RGB888, ARGB8888). RGB565 is the dominant MCU choice — 2 bytes per pixel halves framebuffer cost.
- Implements **dirty-region partial updates**: only the rectangles containing changed properties are re-rasterized and re-uploaded. The full-screen-redraw cost an MCU can't afford never happens.
- Uses CPU-friendly algorithms: integer-only blitting where possible; fixed-point math for transforms; no per-pixel allocations.
- Supports SDF (signed distance field) glyph rendering, added in 1.16.0 (April 2026), as the path for high-quality text on MCUs without a GPU.

A 1MB-flash / 256KB-RAM MCU can run a small Slint UI. The constraint shows in two places: (a) the standard widgets are simpler than their desktop counterparts (no rich text in `TextEdit`; ListView is virtualized but doesn't support arbitrary item types in MCU mode); (b) the styles (Fluent / Material / Cupertino) ship in trimmed forms for embedded.

## Allocation discipline

The embedded constraint forces an allocation discipline across the whole codebase that benefits desktop too:

- **Property values are stored inline** in the component instance struct, not heap-boxed. The codegen emits one struct per component with one field per declared property.
- **Bindings are dispatch tables, not closures**. Each binding is compiled to a function pointer + dependency mask; no per-binding heap allocation.
- **Layout solver is arena-based**. Layout state lives in a per-window arena reset each layout pass — no Vec-per-node.
- **Strings are reference-counted** (`SharedString`) with COW semantics, not always heap-copied. Static strings (literals from `.slint` source) live in `.rodata`.

The result: a Slint application's per-frame allocation count is often zero (in steady state, when properties aren't structurally changing — text edits, scroll-list grows / shrinks force some allocation).

## Tradeoffs

The embedded focus has costs on the desktop side:

- **No async runtime in the core.** Async-callback support (Python's `asyncio` integration in 1.9.0) is layered on top of the synchronous core. Apps wanting native async event handling fight the grain.
- **Custom layout solver (not Taffy).** Slint can't benefit from Taffy's CSS-grid / block-layout / float / named-line-grid improvements upstream. Buiy's choice to integrate Taffy directly ([`../bevy-ui/layout.md`](../bevy-ui/layout.md), [`../taffy/`](../taffy/)) means inheriting those improvements — at the cost of Taffy's allocation profile (which is generally good but not MCU-arena-grade).
- **No rich-text editing in `TextEdit`.** Cosmic-text-equivalent rich shaping / IME / BiDi caret is not the embedded baseline; the desktop story has caught up partially via Parley / Fontique (added in 1.14, October 2025) but Buiy's commitment to full text editing ([`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)) is broader than what Slint's runtime is sized for.
- **Standard widgets are simpler than desktop equivalents.** Slint's `ScrollView` doesn't have momentum-scroll; `ListView` virtualization is coarse; complex form controls (date pickers, time pickers) are recent additions (1.7+).

## Implications for Buiy

- **Embedded-grade allocation discipline applies to large-tree desktop UIs.** Productivity apps with 1000+ nodes (the [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) Avoid §5 case) benefit from the same discipline that lets Slint run on a STM32: inline property storage, no per-frame allocation in steady state, arena layout. Buiy's foundation README goal 7 commits to "productivity-app fixtures at 1000+ nodes" in the verification harness — the engineering target is what Slint delivers on MCUs.
- **No `no_std` for Buiy.** Buiy depends on Bevy, which depends on `std` and dynamic dispatch heavily. Buiy is not pursuing the MCU market. But the *per-frame-allocation budget* discipline transfers.
- **Software renderer with dirty-region partial updates is a reference pattern.** Bevy's renderer is wgpu-based and assumes GPU. If Buiy ever needs a CPU-fallback path (e.g., for headless testing, server-side rendering of UI, or a "render to bitmap for accessibility tools" pipeline), Slint's software renderer is the closest Rust-ecosystem reference. Not a v1 commitment; worth noting.
- **RGB565 / framebuffer color management is a different problem domain.** Buiy commits to wide-gamut color (gradients in any color space — foundation README goal 1). The MCU constraint Slint operates under doesn't translate; Buiy's color story is closer to web-platform-parity (`color()`, `color-mix()`, OKLab/OKLCH) than to RGB565 framebuffers.
- **Partner-driven adoption is the embedded-toolkit business model.** Slint's industrial adopters (OTIV, KDAB's clients) come through partner channels — SixtyFPS GmbH sells commercial licenses, KDAB / Spyrosoft / Crossware do the integration work. This is the embedded ecosystem's revenue model and would not transfer to a game-engine UI library if Buiy ever considered commercial licensing.

## Sources

- Slint partners page: https://slint.dev/partners
- Slint CHANGELOG (RGB565, STM32, ESP-IDF entries): https://github.com/slint-ui/slint/blob/master/CHANGELOG.md
- Slint Rust Foundation spotlight: https://rustfoundation.org/media/member-spotlight-slint/
- crates.io `slint` categories (`no-std`): https://crates.io/crates/slint
- "Slint on Microcontrollers" documentation: https://docs.slint.dev/latest/docs/slint/guide/platforms/mcu/
- Buiy foundation README goal 7 (verifiability): [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling files: [`architecture.md`](architecture.md), [`history.md`](history.md), [`open-problems.md`](open-problems.md)
- Sibling prior-art: [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md), [`../taffy/`](../taffy/)
