# CLAUDE.md — Buiy Development Guide

## Project Overview

Buiy is an accessible, web-quality UI library for the [Bevy](https://bevyengine.org) game
engine, written in Rust as a **parallel UI stack to `bevy_ui`** — not built on top of it. It
wires the primitives Bevy uses (Taffy for layout, cosmic-text for shaping + editing, AccessKit
for the accessibility tree, `bevy_picking` for hit-testing) behind its own decomposed,
ECS-native component model and a **custom wgpu render pipeline** that runs as a system in
Bevy's `Core2d` schedule. Widget state flows through an Elm-style **MVU** funnel
(`buiy_core::mvu`), which buys record/replay and agent-drive. The north star is modern-web
feature parity — a CSS-subset layout/styling engine, complex text with BiDi/IME, ARIA roles,
WCAG 2.2 AA — for both game and application UIs, with every machine-testable claim gated in CI.
Status: pre-0.1 (`0.0.1`), pre-alpha; APIs are unstable and may break in any commit. A fuller
overview is in [`README.md`](README.md).

## Dev Guidelines

Quality + longevity beat speed + convenience.

- **Choose right solution, not easy one.** Ask: which approach makes most sense long-term, causes least future confusion, lasts? Pick that.
- **No hacky workarounds, no shortcuts.** If obvious fix is band-aid, keep digging for real fix.
- **Root-cause every bug.** No patching symptoms. No disabling failing tests. No swallowing errors. Find why, fix why.
- **Scope creep OK when warranted, not speculative.** Doing it right means touching more files / refactoring abstraction — do it. Don't add features, abstractions, error handling task didn't ask for.
- **Answer not obvious? Stop, design.** Two+ reasonable approaches? Brief note in `docs/specs/YYYY-MM-DD-<name>-design.md` before coding. Plan in `docs/plans/YYYY-MM-DD-<name>.md`. Cheap up front, expensive later.
- **Surface tradeoffs explicit.** Picking between approaches, name runner-up + why rejected. Commit body or PR description. Future-you needs reasoning, not just result.
- **Mechanical rigor before commit.** Run the project's check command (lint + format + tests) and resolve every warning before committing. Fill in the exact command under `## Build & Test` once it exists.
- **Semantic rigor: verify before claiming done.** Run actual test, hit actual UI, read actual output. No "should work" assertions. See `superpowers:verification-before-completion`.
- **Process skills before implementation skills.** Brainstorming + debugging determine *how*. Don't skip to feel productive.
- **Tests at lowest tier covering behavior.** Unit before integration before end-to-end.

## Repository Structure

```
docs/
├── README.md           — Master index of specs, plans, reports, prototypes, and prior-art (start here)
├── specs/              — Target state — what we are building toward (YYYY-MM-DD-<name>-design.md)
├── plans/              — Migration steps — how we get to the target (YYYY-MM-DD-<name>.md)
├── reports/            — One-shot audits and investigations of our code
├── prototypes/         — Prototype-first journals + retrospectives (learning kept; code stays unmerged)
├── prior-art/          — Deep dives on external systems we learn from (one folder per system)
└── reference-designs/  — Archived design bundles (immutable)
```

Source tree:

```
crates/
├── buiy/              — public umbrella crate (BuiyPlugin / BuiyHeadlessPlugin + re-exports + buiy::bsn)
├── buiy_core/         — components, layout, render, text + editing, a11y, focus, picking, theme, animation, and the MVU state substrate
├── buiy_widgets/      — widget implementations (Button, TextInput, Checkbox, Switch, Slider, Disclosure, Dialog, Tooltip, Popover, Menu, ScrollArea) + composites
├── buiy_bsn/          — BSN (`bsn!`) authoring re-exports
├── buiy_verify/       — verification harness (visual goldens, AccessKit snapshots, contrast linter)
└── buiy_bench_support/ — dev-only perf-measurement harness (never in the production graph)

examples/              — hello_button, hello_text, hello_bsn, hello_mvu, buiy_gallery, gallery_web, buiy_web, capture
```

## Build & Test

The "run all checks" command (mirrors what CI runs):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets --locked -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked && \
  xvfb-run -a cargo test --workspace --locked
```

On macOS / Windows drop the `xvfb-run -a` prefix; tests run headlessly without it.

`--locked` mirrors CI: `Cargo.lock` is committed and every CI cargo step runs
`--locked`, so a build that needs to change the lockfile is a real failure to
surface, not silently paper over (audit finding #1). Run `cargo update` (or
`generate-lockfile`) deliberately in its own commit when a bump is intended.

If the test step link-OOMs under full `mold` parallelism, add `-j 2` to the
`cargo test` step (the large bevy test binaries link in parallel otherwise).

Local debuginfo: root `Cargo.toml` sets `[profile.dev]`/`[profile.test]`
`debug = 0` (audit finding #10 — the bevy dep artifacts' debuginfo is what
overran the CI runner disk). The cost is that **local** dev/test backtraces lose
line numbers. If you want them back locally without re-bloating CI, switch those
profiles to `debug = "line-tables-only"` (keeps line info at a larger target
dir); don't set full `debug = 2` or CI will OOM-link again.

### GPU lane (`#[ignore]` tests — needs a real wgpu adapter)

The headless gate above runs WITHOUT `--ignored`, so it never instantiates a wgpu
adapter and never exercises the render GPU path. The render-pipeline GPU tests
(pipeline creation, the extract→prepare→node draw spine, render-to-texture +
pixel readback, atlas, compositor, the text pipeline (glyph producer,
decorations, selection/caret, effect groups, golden suite)) are `#[ignore]` and
run on a host with a real GPU (or `lavapipe`). Vulkan render-to-texture needs
**no** X server, so this works headless on any machine with an adapter — it
does **not** require a display:

```sh
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1
```

The GPU lane has **two legs**, both of which CI runs: `buiy_core` (the render
GPU path above) and `buiy_verify` (the visual-bug verification suite — goldens,
reftests, the perceptual metric). Run both before pushing, or you skip the
`buiy_verify` GPU tests and can land a regression that fails CI's GPU lane.

`--test-threads=1` serializes the GPU work (one adapter context at a time). This
lane is **additive** — it must pass on a GPU host, and the headless gate above
must stay green independently (CI has no adapter). When adding a render GPU test,
keep `#[ignore]` on it and build it on `crates/buiy_core/tests/support/mod.rs`
(`gpu_test_app` / `gpu_render_app` / `render_to_image` / `readback_rgba`). The
campaign that established this lane: `docs/plans/2026-06-07-render-gpu-verify-campaign.md`.

Supply-chain check (run before bumping any dep):

```sh
cargo deny check
```

Other useful one-offs:

- `cargo test -p buiy_core` — fast loop on the core crate.
- `cargo run -p hello_button` — visual smoke test of the Phase 0 widget.
- `cargo run -p hello_text` — visual smoke test of the text stack.
- `cargo run -p hello_bsn` — visual smoke test of the `bsn!` authoring path.
- `cargo run -p capture` — regenerate the README screenshots headlessly (offscreen render-to-texture + GPU readback; needs a real wgpu adapter).
- `BUIY_ACCEPT_SHAPING=1 cargo test -p buiy_core --test text_shaping_snapshots`
  — regenerate the `.snap` shaping snapshots (curated: review the diff before
  committing).
- `cargo test -p buiy_core --features clipboard-image` — exercise the clipboard
  image flavor (`ClipboardImage`, `get_image`/`set_image`). The default
  workspace gate runs with this feature **OFF** (the image module compiles out),
  so this gated lane must be run separately to keep the image path from rotting.

## Code Conventions

- **Docs entry point:** `docs/README.md` is the master index of specs, plans, reports, prototypes, and prior-art folders, grouped by area. Read it before adding any new doc or before searching for an existing one. The `organizing-buiy-docs` skill mirrors the conventions for on-demand loading. Cemented in `docs/specs/2026-05-07-docs-organization-design.md`.
- **Prior-art workflow:** the `researching-prior-art` skill drives the 7-stage parallel-agent creation of a `docs/prior-art/<system>/` folder; the `using-prior-art` skill is the consumer-side flow that surfaces relevant folders during spec/plan/review work.
- **Visual-bug verification (`buiy_verify`):** before adding/changing any visual, layout, paint-order, color, or render test — or adding a widget fixture, writing a reftest, or blessing a golden — use the `using-buiy-verification` skill (the task-oriented how-to: pick a tier, add a fixture, run the gates, gotchas). It mirrors the design spec `docs/specs/2026-06-15-buiy-verification-design/` and the crate root doc `crates/buiy_verify/src/lib.rs`. Rule of thumb: add a test at the **lowest tier that can observe the bug** (layout snapshot → display-list snapshot → invariant → reftest → golden); goldens are the last resort for the rasterization residue only. The GPU `--ignored` lane (Tiers 4–5) is additive and must pass on a GPU host; the headless gate must stay green without an adapter.
- **BSN authoring (`buiy_bsn`):** the thin `buiy_bsn` crate re-exports Bevy 0.19's `bsn!` / `bsn_list!` + spawn ext traits (no new syntax); it is reached via `buiy::bsn` and folded into `buiy::prelude`, so `use buiy::prelude::*;` brings `bsn!` into scope. Author the **decomposed components directly** (`bsn! { BoxModel { … } Background(…) }`) — `Style` is a `Bundle` builder, not a Component, so it is not `bsn!`-authorable. Widgets carry `#[require(...)]` contracts; **style them via the parameterized scene-fns in `buiy_widgets`** (`button("…")`, `text_input_*`, re-exported through `buiy::prelude`), never a single-field patch of a `#[require]`'d component (that drops the widget's other defaults — the § 4.1c suppression gotcha). Pin: `docs/specs/2026-06-18-buiy-bsn-integration-design.md`.

_TODO: add language- and project-specific conventions (naming, error handling, testing, serialization, etc.) as they are established._
