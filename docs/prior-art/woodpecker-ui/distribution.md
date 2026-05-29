**Date:** 2026-05-22
**Status:** active
**Subject:** woodpecker_ui — distribution, license, platform support, governance, bus factor

# Distribution & governance

## License

**MIT OR Apache-2.0** (dual-licensed, the Rust-ecosystem de-facto standard). Verified in crates.io metadata for both `0.1.0` and `0.1.1`, in the GitHub `LICENSE-MIT` / `LICENSE-APACHE` files, and in the README's License section.

Note: GitHub's repo-level license-key API returns `apache-2.0` only (because GitHub's auto-detection picks one when multiple `LICENSE-*` files are present). The crate metadata is the canonical source — both licenses apply at the consumer's option.

The kayak_ui predecessor crate has license `"non-standard"` on crates.io (the repo says MIT but the crate metadata wasn't filled correctly). woodpecker_ui's dual-licensing is a tidier setup.

## Versions

| Version | Published | Bevy pin | Downloads | Yanked |
|---|---|---|---|---|
| `0.1.0` | 2025-05-31 15:50 UTC | 0.16 | 464 | no |
| `0.1.1` | 2025-05-31 22:42 UTC | 0.16 | 613 | no |

Total lifetime downloads: **1,077**. Recent (90-day): **6**. Both numbers fetched 2026-05-22 from `https://crates.io/api/v1/crates/woodpecker_ui`.

For reference, comparable Bevy UI crates fetched the same day:
- `bevy_lunex`: see [`../bevy-lunex/`](../bevy-lunex/) (folder partial)
- `kayak_ui`: 18,774 lifetime downloads (predecessor)
- `bevy_feathers`: 191,700 lifetime downloads ([`../bevy-feathers/distribution.md`](../bevy-feathers/distribution.md))

woodpecker_ui is **~5–200× smaller than its sibling third-party crates** depending on what's compared. Its adoption is in the long tail.

## MSRV

Not declared (`rust_version` is null in `Cargo.toml`). edition = `2021`. Effective MSRV is whatever Bevy 0.16 + Parley 0.4 + bevy_vello 0.9 + Taffy 0.7 cumulatively require — roughly Rust 1.75+ (Bevy 0.16's published MSRV).

## Cargo features

```toml
[features]
default = []
debug-render = []
hotreload = ["dioxus-devtools", "woodpecker_ui_macros/hotreload"]
metrics = []
```

Four features:
- `debug-render` — show layout/widget bounding boxes.
- `hotreload` — opt-in dioxus-devtools live-patch (see [`integration.md`](integration.md)).
- `metrics` — runtime widget-count + system-timing counters (`WidgetMetrics` resource).

No widget-subset features, no theme-source toggles, no accessibility toggle (because there's no accessibility integration to toggle — see [`critiques.md`](critiques.md)).

## Dependencies

Verified from `Cargo.toml` on `main` (2025-06-07 push):

| Group | Crate | Version |
|---|---|---|
| Bevy | `bevy` (default-features = false) | 0.16 |
| Render | `bevy_vello` | 0.9 (`text` + `default_font` features) |
| Layout | `taffy` | 0.7 (`flexbox` + `grid`) |
| Text | `parley` | 0.4 |
| Text | `skrifa` | 0.30.0 |
| Text | `ttf-parser` | 0.25.1 |
| Vector | `usvg` | 0.44 |
| Animation | `interpolation` | 0.2 |
| Color | `palette` | 0.7.6 |
| Image | `image` | 0.24 |
| Strings | `smol_str` | 0.2 (no default features) |
| Trait query | `bevy-trait-query` | 0.16 |
| Hot-reload (opt) | `dioxus-devtools` | 0.7.0-alpha.0 |
| Cast | `bytemuck` | 1.23.0 |
| Errors | `thiserror` | 1.0 |
| Time | `web-time` | 1.1 |
| Syntax (code blocks) | `autumnus` | 0.3.2 |
| ANSI | `ansi-parser` | 0.9.1 |
| Macros | `woodpecker_ui_macros` (workspace) | 0.1 |
| Native clipboard | `arboard` | 3.4 |
| WASM | `web-sys`, `wasm-bindgen-futures`, `futures-channel` | various |
| Dev | `bevy-inspector-egui` | 0.31 |
| Dev | `calc` | 0.4.0 |

A handful of these are pinned old (`image` 0.24 is 2-3 years out of date; `thiserror` 1.0 was superseded by 2.0 in late 2024). The dep pinning has not been refreshed alongside the silent period.

## Platform support

- **Linux / macOS / Windows native:** verified by `arboard` cross-platform clipboard dep and default Bevy feature set. Examples run with `cargo run --example todo`.
- **Web (WASM):** explicit `cfg(target_arch = "wasm32")` dependency block. Uses Bevy's `webgpu` feature. Build flags documented: `RUSTFLAGS="--cfg=web_sys_unstable_apis" cargo run --example todo --target wasm32-unknown-unknown --release` + `wasm-server-runner`.
- **Android:** profile `android-dev` defined in `Cargo.toml` but no Android-specific code. Not verified working.
- **iOS:** not mentioned, not verified.
- **Headless / CI:** `RenderSettings { use_cpu: true }` enables vello CPU rasterization (slow but functional). No CI workflows in the repo to confirm headless test runs.

## Governance

**Solo maintainer.** Both crates.io publishers list and GitHub commit history are dominated by `StarArawn`. Recent verified data (2026-05-22):

| Metric | Value |
|---|---|
| GitHub stars | 70 |
| Forks | 4 |
| Open issues | 8 |
| Open PRs | unknown (rate-limited during research) |
| Discussions enabled | yes |
| Last commit | 2025-06-07 |

**Bus factor: 1.** If StarArawn stops working on woodpecker_ui (which the 11-month silent period suggests has effectively happened), there is no second maintainer to pick it up. The 4 forks are unverified — typically community forks of small Bevy crates are personal pins, not active continuations.

The pattern matches kayak_ui's: solo author, ~15-month active development, silent → effective abandonment. See [`history.md`](history.md) § "Pattern: the second-system trap?"

## Release cadence

- **0.1.0:** 2025-05-31 15:50 UTC
- **0.1.1:** 2025-05-31 22:42 UTC (7 hours later — bug-fix patch)
- **(none since)**

There is no release schedule. The crate is effectively single-release. The README's experimental hot-reload section depends on `dioxus-devtools 0.7.0-alpha.0` which is itself a pre-release of an unrelated framework's tooling — the entire stability story is pre-1.0 on pre-1.0.

## Distribution observations relevant to Buiy

1. **Single-crate workspace** (woodpecker_ui + woodpecker_ui_macros). Buiy's foundation README § 5 lists "Final crate split" as an open question; woodpecker_ui's single-crate decision is a data point that the simpler approach is viable for a starter set, but the surface here is small (~9.3 KLOC) — Buiy's APG-coverage commitment is multiples larger.
2. **Hot reload via dioxus-devtools** is a real demonstration that **someone else's hot-patch infrastructure can be borrowed** rather than written from scratch. See [`lessons.md`](lessons.md) Borrow entry.
3. **No reverse-dependency on `bevy_ui`** confirms that a parallel UI stack on `bevy_picking` + `bevy_vello` is technically achievable without depending on the bevy_ui crate surface. This validates the Buiy parallel-stack architecture from a different angle than `bevy_lunex` does.

## Sources

- crates.io API — https://crates.io/api/v1/crates/woodpecker_ui (fetched 2026-05-22, with proper User-Agent)
- crates.io API — https://crates.io/api/v1/crates/kayak_ui (fetched 2026-05-22)
- GitHub repo metadata — https://github.com/StarArawn/woodpecker_ui (fetched 2026-05-22)
- `Cargo.toml` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/Cargo.toml
- README — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/README.md
- Sibling: [`history.md`](history.md), [`integration.md`](integration.md), [`critiques.md`](critiques.md), [`ecosystem.md`](ecosystem.md)
