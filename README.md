# Buiy

Buiy is a comprehensive UI library for [Bevy](https://bevyengine.org). It provides a token-themed, accessibility-first widget toolkit on top of Bevy's ECS — Taffy-based layout, an AccessKit-shaped a11y tree, focus and picking, and a small render pipeline that draws into Bevy's render graph. Phase 0 ships the foundation (one `Button`, the plugin scaffolding, and the verification harness); the full widget catalog and feature surface are tracked in `docs/`.

## Status

Pre-0.1. APIs may change. The repo currently lives at version `0.0.1` and has not been tagged or published.

## Quick start

Add Buiy alongside Bevy in your `Cargo.toml`, then add `BuiyPlugin` after `DefaultPlugins`:

```rust
use bevy::prelude::*;
use buiy::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(Button::new("Save"));
}
```

`Button::new` returns a bundle wired up with `Node`, `Style`, `Focusable`, an `A11yRole::Button`, and an `A11yLabel`. See `examples/hello_button/` and `tests/hello_button_e2e.rs` for the full shape.

## Requirements

- **Rust:** stable (see `rust-toolchain.toml`).
- **Bevy:** 0.18.
- **Linux system deps for Bevy:** `libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev` (the CI lint and test jobs install these explicitly).

## Repository layout

```
crates/
├── buiy/         — public umbrella crate; the one users depend on
├── buiy_core/    — components, plugins, layout, a11y, focus, picking, render, theme
├── buiy_widgets/ — widget implementations (Phase 0: Button)
└── buiy_verify/  — verification helpers (a11y snapshots, contrast linter)

examples/
└── hello_button/ — minimal end-to-end runnable scene

tests/            — workspace-level integration tests (e.g. hello_button_e2e)
docs/             — design specs, implementation plans, and reports
```

## Design docs

Architectural specs, migration plans, and audit reports all live under [`docs/`](docs/README.md). Start at the docs index, which is grouped by feature area and tagged with status.

## License

TBD (license placeholder; the workspace `Cargo.toml` currently declares `MIT OR Apache-2.0`, but the crate has not been published).
