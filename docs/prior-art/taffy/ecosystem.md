**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — downstream users, comparison to Yoga, test corpus, community

# Taffy — ecosystem

Taffy is the only Rust-native layout engine that ships Flexbox + Grid + Block + Float in one crate. It sits in a neighborhood with Facebook's Yoga (C++, Flexbox-only) and a handful of single-purpose Rust crates (`morphorm`, `cassowary-rs`, `azul-layout`). This file catalogs who uses Taffy, how it compares to Yoga, and the supporting infrastructure around it.

## 1. Downstream users (production)

Verified via [crates.io reverse-dependencies](https://crates.io/crates/taffy/reverse_dependencies). Top by relevance:

| Crate | Pin | Domain |
|---|---|---|
| `bevy_ui` | `^0.10` (`^0.9` in 0.18.1) | Bevy game engine UI |
| `i-slint-core` | `^0.9` | Slint declarative GUI toolkit |
| `blitz-dom` | `=0.11.0-experimental-cache-fix.3` | Browser engine (Dioxus Native) |
| `blitz-paint` | `^0.9` | Browser paint layer |
| `blitz-renderer-vello` | `^0.8` | Browser renderer |
| `stylo_taffy` | `^0.9` | Servo Stylo → Taffy bridge |
| `servo-layout` | `^0.10` | Servo's layout engine |
| `gpui` | `=0.9.0` (exact) | Zed's UI framework |
| `floem` | `^0.4` | Lapce's UI framework |
| `dioxus-native-core` | `^0.3.12` | Dioxus native renderer (maintenance) |
| `dioxus-tui` | `^0.3.12` | Dioxus terminal renderer |
| `azul-layout` | `^0.9.1` | Azul's layout module |
| `egui_taffy` | `^0.9.2` | egui + Taffy bridge |
| `takumi` | `^0.10` | React → image SSR |
| `iocraft` | `^0.5.2` | Terminal UI |
| `inlyne` | `^0.3.19` | Markdown viewer |
| `rioterm` | `^0.10.1` | Terminal emulator |

The list spans web (Servo, Blitz), game (Bevy), production editors (Zed via GPUI, Lapce via Floem), declarative toolkits (Slint, Azul, Dioxus/Blitz), and a long tail of niche renderers (terminal, image, markdown). This is the broadest production footprint of any Rust layout crate.

**Brief-correction:** the brief listed Iced as a Taffy user. Verified false — Iced has its own per-widget `Widget::layout` protocol, no Taffy dependency in `iced/Cargo.toml` ([master Cargo.toml](https://github.com/iced-rs/iced/blob/master/Cargo.toml)). The Taffy README's "Iced uses Taffy" claim appears stale; corrected here and in [integration.md § 4](integration.md#4-iced-does-not-use-taffy).

**Freya:** the brief asked. Freya is a `react-native`-style native Rust UI framework. It does NOT depend on Taffy directly (no `taffy` in `freya/Cargo.toml`); it uses its own `freya-engine` layout. Not a Taffy user.

## 2. Comparison to Yoga

[Yoga](https://github.com/facebook/yoga) (Facebook, 2014) is Taffy's closest peer. Latest version 3.2.1 (2024-12-13). 18.7k stars, broadly deployed.

| | Yoga | Taffy |
|---|---|---|
| Language | C++ 20 (bindings: Java, TS, Swift, .NET) | Rust |
| License | MIT | MIT |
| Flexbox | ✓ | ✓ |
| CSS Grid | ✗ | ✓ |
| Block layout | ✗ | ✓ |
| Float / clear | ✗ | ✓ (0.10) |
| Direction (LTR/RTL) | ✓ | ✓ (0.10) |
| Writing modes (vertical) | ✗ | ✗ |
| `calc()` | ✗ | ✓ (0.8) |
| Container queries | ✗ | ✗ |
| Anchor positioning | ✗ | ✗ |
| Subgrid | ✗ | ✗ |
| Masonry | ✗ | ✗ |
| Bindings | npm, Maven Central, SwiftPM | crates.io (C/Wasm WIP) |
| Embedder model | C++ wrapper class | Rust traits or `TaffyTree` |
| Used by | React Native, Litho, ComponentKit | Bevy, Servo, Blitz, Zed, Lapce |

**Benchmark posture** (per Taffy README, M1 Pro, criterion):

- 1,000-node deep trees: Taffy `1.7400 ms` vs Yoga `2.2333 ms` (Taffy faster).
- 100,000-node deep trees: Taffy `63.778 ms` vs Yoga `76.755 ms` (Taffy faster).
- 100,000-node *wide* trees at depth 1: Taffy `247.42 ms` vs Yoga `135.78 ms` (Yoga faster).

The wide-tree case is the README's honest admission that Taffy is not uniformly faster than Yoga. Practical implication: deeply-nested UIs (typical) favor Taffy; very flat trees with tens of thousands of siblings favor Yoga. See [critiques.md § 2](critiques.md#2-performance-critiques) for what's behind this.

**Algorithmic scope:** Yoga is Flexbox-only. Taffy implementing Block + Grid + Float in addition is the strategic-distinction line. For Buiy, that's the load-bearing reason Taffy is the choice over Yoga-via-bindings: Buiy needs Grid + Block.

## 3. WPT compliance corpus

Web Platform Tests (WPT) is the W3C-hosted browser-conformance suite. Taffy ships a WPT-derived subset of layout tests as part of its CI:

- **Roadmap:** [issue #639](https://github.com/DioxusLabs/taffy/issues/639) (closed) was the umbrella for hooking up WPT. The script lives in `scripts/import-yoga-tests` (the name is historic — it imports yoga-derived tests; WPT-derived tests came in separately).
- **Test fixtures:** under `test_fixtures/` (Block + Flexbox + Grid). Generated via a `gentest` harness (issue #546 extends it for image nodes).
- **Pass-rate:** not published. The CHANGELOG carries per-WPT-test fix entries (e.g. "Fix resolving flexible lengths (WPT css/flexbox-multiline-min-max test)" in 0.6.0), but no aggregate percentage. Asking "how WPT-conformant is Taffy?" requires reading the issue tracker for individual `wpt`-labelled failures.
- **Compared to Chromium:** Taffy is a Rust port. Some bugs are tracked against Chromium's behavior changes ("CSS Grid aspect-ratio behaviour changed in Chrome 124" — [issue #653](https://github.com/DioxusLabs/taffy/issues/653)). The pattern is "Chromium ships a spec-tracking fix, Taffy ports it after, sometimes months later."

The implicit test corpus is large — `cargo test --workspace` runs thousands of layout fixtures. Per the README, "more than 1500 generated tests, plus several thousand more imported from CSS WG / Chromium / Firefox / WebKit test suites."

## 4. Tooling and surrounding crates

Within the Taffy GitHub org:

- **`scripts/import-yoga-tests`** — workspace member that imports test fixtures.
- **`scripts/gentest`** — generation harness for WPT-derived tests.
- **`benches/`** — criterion benchmarks; the source of the README's numbers.

External crates:

- **`stylo_taffy`** — Servo's Stylo (CSS parsing/cascade engine) bridged to Taffy's `Style`. The standard path when an embedder wants real CSS parsing.
- **`egui_taffy`** — community bridge to egui.
- **`compose-taffy`** — Compose-style declarative wrapper.
- **`cssengine`** — a small CSS engine using Taffy.

No officially-sponsored bindings exist for C, Python, or Wasm as of 2026-05; the README's "C bindings (work in progress)" and "WebAssembly bindings (work in progress)" reference [issue #404](https://github.com/DioxusLabs/taffy/issues/404), an open draft PR for C bindings. The `stretchable` Python wrapper bridges via stretch2's bindings, not current Taffy.

## 5. Community

- **GitHub Discussions** — primary Q&A surface. ~1 thread per week.
- **Discord** — channel `#taffy` in the Dioxus Discord; no dedicated server.
- **Issue tracker** — 89 open issues, 25 open PRs as of 2026-05. Triage by Burns + Cecile.

There's no mailing list, no IRC, no Matrix. No dedicated forum.

## 6. Notable production deployments

The shipping artifacts that demonstrate Taffy works at scale:

- **Bevy** — every shipped Bevy app since `0.8` (2022-07-30, [PR #4716](https://github.com/bevyengine/bevy/pull/4716)). The ecosystem is wide but per-app UI is usually small.
- **Servo** — replaced its own layout engine with Taffy in 2024. Servo is preview-grade, not mainstream-shipped, but it's the most demanding correctness exposure.
- **Blitz** — alpha. The Dioxus Native target. Render-Wikipedia-correctly is the stretch goal.
- **Zed** — released editor. GPUI uses Taffy traits against its own element arena. Zed's UI layouts are simple-flexbox-heavy, not load-bearing on Grid.
- **Lapce** — released editor. Floem-based. Similar to Zed.
- **Slint** — declarative UI toolkit. Taffy underpins flex/grid in Slint's compiled output.

The "in-production-shipping-with-Taffy" set is meaningful (Zed, Lapce, Slint apps, every Bevy app). The "in-production-stressing-Taffy-correctness" set is narrower (Servo + Blitz).

## Sources

- crates.io reverse-dependencies: https://crates.io/crates/taffy/reverse_dependencies
- Taffy README (benchmark numbers, integration list): https://github.com/DioxusLabs/taffy/blob/main/README.md
- Iced Cargo.toml (no taffy): https://github.com/iced-rs/iced/blob/master/Cargo.toml
- Iced layout primitive: https://github.com/iced-rs/iced/blob/master/core/src/layout.rs
- Yoga repo: https://github.com/facebook/yoga
- Yoga 3.2.1 release: https://github.com/facebook/yoga/releases
- Servo's layout migration to Taffy (servo-layout crate): https://crates.io/crates/servo-layout
- stylo_taffy bridge: https://crates.io/crates/stylo_taffy
- Issue #639 (WPT umbrella): https://github.com/DioxusLabs/taffy/issues/639
- Issue #404 (C bindings WIP): https://github.com/DioxusLabs/taffy/pull/404
- Issue #546 (gentest image extension): https://github.com/DioxusLabs/taffy/issues/546
- Issue #653 (Chrome 124 grid aspect-ratio): https://github.com/DioxusLabs/taffy/issues/653
- Sibling: [integration.md](integration.md), [critiques.md](critiques.md), [layout-algorithms.md](layout-algorithms.md)
