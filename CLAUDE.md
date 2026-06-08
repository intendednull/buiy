# CLAUDE.md — Buiy Development Guide

## Project Overview

_TODO: one-paragraph description of Buiy — what it is, the language/stack, the core concepts. Replace this placeholder before merging real work._

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
├── README.md           — Master index of specs, plans, reports, and prior-art (start here)
├── specs/              — Target state — what we are building toward (YYYY-MM-DD-<name>-design.md)
├── plans/              — Migration steps — how we get to the target (YYYY-MM-DD-<name>.md)
├── reports/            — One-shot audits and investigations of our code
├── prior-art/          — Deep dives on external systems we learn from (one folder per system)
└── reference-designs/  — Archived design bundles (immutable)
```

_TODO: add the source-tree layout (e.g. `src/`, `crates/`, `packages/`) once Buiy has code._

## Build & Test

The "run all checks" command (mirrors what CI runs):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace
```

On macOS / Windows drop the `xvfb-run -a` prefix; tests run headlessly without it.

If the test step link-OOMs under full `mold` parallelism, add `-j 2` to the
`cargo test` step (the large bevy test binaries link in parallel otherwise).

### GPU lane (`#[ignore]` tests — needs a real wgpu adapter)

The headless gate above runs WITHOUT `--ignored`, so it never instantiates a wgpu
adapter and never exercises the render GPU path. The render-pipeline GPU tests
(pipeline creation, the extract→prepare→node draw spine, render-to-texture +
pixel readback, atlas, compositor) are `#[ignore]` and run on a host with a real
GPU (or `lavapipe`). Vulkan render-to-texture needs **no** X server, so this works
headless on any machine with an adapter — it does **not** require a display:

```sh
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
```

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
- `cargo run --example hello_button` — visual smoke test of the Phase 0 widget.

## Code Conventions

- **Docs entry point:** `docs/README.md` is the master index of specs, plans, reports, and prior-art folders, grouped by area. Read it before adding any new doc or before searching for an existing one. The `organizing-buiy-docs` skill mirrors the conventions for on-demand loading. Cemented in `docs/specs/2026-05-07-docs-organization-design.md`.
- **Prior-art workflow:** the `researching-prior-art` skill drives the 7-stage parallel-agent creation of a `docs/prior-art/<system>/` folder; the `using-prior-art` skill is the consumer-side flow that surfaces relevant folders during spec/plan/review work.

_TODO: add language- and project-specific conventions (naming, error handling, testing, serialization, etc.) as they are established._
