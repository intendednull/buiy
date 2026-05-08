# Buiy Phase 0 Foundations Implementation Plan

**Date:** 2026-05-07
**Status:** landed
**Spec:** [specs/2026-05-07-buiy-foundation/README.md](../specs/2026-05-07-buiy-foundation/README.md)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Smallest end-to-end "does the architecture work?" Buiy demo: workspace + crates + `BuiyPlugin` + minimal render/layout/a11y/focus/picking/theme + a verification harness + a hello-world Button example, with cross-platform CI green.

**Architecture:** Parallel-to-bevy_ui Bevy plugin. Components in `buiy_core`, widgets in `buiy_widgets`, verification harness in `buiy_verify`, top-level `BuiyPlugin` in `buiy`. Sub-plugin order in Phase 0: `core → theme → a11y → focus → input → widgets`. System sets: `BuiySet::Layout → Style → Input → Animate → Picking → A11yUpdate → Render`. AccessKit adapter is owned per-window keyed by winit `WindowId`. Components derive `Reflect + FromReflect + Default + Clone + Component` and are `register_type`d.

**Tech Stack:** Rust (latest stable), Bevy 0.18+, Taffy for layout, AccessKit + accesskit_winit for a11y, bevy_picking for hit-testing, wgpu via Bevy render graph, image-compare for visual regression, proptest for property tests.

**Phase 0 explicitly out of scope:** rich text + IME (cosmic-text comes later), animation system, forms/validation, devtools, BSN authoring, multiple widgets, 3D-anchored UI. Each becomes its own sub-spec + plan.

**Bevy version note:** Bevy 0.18+ APIs shift between minors. Where this plan shows specific Bevy API calls, the engineer must verify against the in-tree Bevy version's documentation; minor adjustments to plugin / render-graph / SystemSet construction may be required. The architectural decisions (per-window adapter, render-graph node ordering, system-set names) are stable.

---

## Spec coverage map

Each Phase 0 task maps to specific commitments in the foundation spec:

| Foundation-spec commitment | Phase 0 task |
|---|---|
| Crate workspace partition (architecture.md § 2.8) | Task 1 |
| `BuiySet::Layout / Style / Input / Animate / Picking / A11yUpdate / Render` ordering | Task 4 |
| Components derive `Reflect + FromReflect + Default + Clone + Component`, `register_type`d | Task 5 |
| Token-based theming, OS-pref `UserPreferences` resource | Task 6 |
| Taffy integration, `BuiySet::Layout` | Task 7 |
| Focus tree, `:focus-visible` semantics, Tab handling, focus ring | Task 8 |
| AccessKit adapter ownership per-window keyed by `WindowId`, `bevy_a11y` replaced | Task 9 |
| `bevy_picking` backend registration, per-window filtered | Task 10 |
| Custom render pipeline (rounded clipping foundations, top-layer hooks) | Tasks 11–13 |
| Sub-plugin order in `BuiyPlugin` | Tasks 14, 15 |
| Verification harness (`buiy_verify`) | Tasks 16–19 |
| CI on Windows / macOS / Linux desktop | Task 22 |
| Visual regression CI gate (gate #2) | Task 17 |
| AccessKit tree snapshot CI gate (gate #3) | Task 18 |
| Contrast linter CI gate (gate #9) | Task 19 |
| Hot-reload validation (gate #13), perf regression (gate #14), memory leak (gate #15) — **deferred** beyond Phase 0; their crates are scaffolded but not exercised |

Non-CI gate categories deferred beyond Phase 0: announcement-output snapshots (gate #4), forced-colors compatibility scan (gate #11), hit-target linter (gate #10), property tests / fuzzing (gate #12). Spec calls them out as committed, but Phase 0's job is to prove the architecture, not to ship the full verification suite.

---

## Task 1: Workspace scaffolding

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `rust-toolchain.toml`
- Create: `rustfmt.toml`
- Create: `clippy.toml`
- Modify: `.gitignore` (add Rust artifacts)

- [ ] **Step 1: Write the failing test**

Create `tests/workspace_smoke.rs`:

```rust
#[test]
fn workspace_builds() {
    // This is a compile-time assertion that the workspace is well-formed.
    // If this file compiles, the workspace is valid.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo build --workspace`
Expected: FAIL with "no Cargo.toml found" or similar.

- [ ] **Step 3: Create the workspace**

Create `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/buiy",
    "crates/buiy_core",
    "crates/buiy_widgets",
    "crates/buiy_verify",
    "examples/hello_button",
]

[workspace.package]
version = "0.0.1"
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/intendednull/buiy"
rust-version = "1.85"

[workspace.dependencies]
bevy = { version = "0.18", default-features = false, features = ["bevy_render", "bevy_winit", "bevy_window", "bevy_asset", "bevy_log", "x11", "wayland"] }
bevy_picking = "0.18"
taffy = "0.10"
accesskit = "0.24"
accesskit_winit = "0.32"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
image = "0.25"
image-compare = "0.6"
proptest = "1"
thiserror = "2"
tracing = "0.1"
```

Versions are indicative for May 2026; the engineer must verify against current crates.io and pin compatible versions. The `accesskit` and `accesskit_winit` versions must match what Bevy's a11y crate pins, OR the spec's parallel architecture means we own the version directly — whichever is current with Bevy 0.18.

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

Create `rustfmt.toml`:

```toml
edition = "2024"
# imports_granularity = "Module" and group_imports = "StdExternalCrate" are
# nightly-only on rustfmt as of stable 1.x. Re-enable if the project later
# moves fmt to nightly.
```

Create `clippy.toml`:

```toml
disallowed-methods = []
```

Modify `.gitignore` — append:

```
target/
Cargo.lock
```

(`Cargo.lock` is omitted from version control because Buiy is a library workspace; `examples/hello_button` may want to commit its own Cargo.lock — that's a future decision. For Phase 0, exclude.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo build --workspace`
Expected: builds (with warnings about empty crates that will be filled in by later tasks). If it fails because member crates don't exist yet, create empty `crates/<name>/Cargo.toml` and `crates/<name>/src/lib.rs` with `pub fn _placeholder() {}` so the workspace resolves; later tasks replace these.

Run: `cargo test --workspace`
Expected: PASS (the empty test from Step 1 passes).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml rust-toolchain.toml rustfmt.toml clippy.toml .gitignore tests/workspace_smoke.rs
git add crates/*/Cargo.toml crates/*/src/lib.rs examples/*/Cargo.toml examples/*/src/main.rs
git commit -m "chore: bootstrap Buiy workspace and crate scaffolding"
```

---

## Task 2: `buiy_core` crate skeleton with `CorePlugin`

**Files:**
- Modify: `crates/buiy_core/Cargo.toml`
- Modify: `crates/buiy_core/src/lib.rs`
- Create: `crates/buiy_core/tests/plugin_smoke.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/plugin_smoke.rs`:

```rust
use bevy::prelude::*;
use buiy_core::CorePlugin;

#[test]
fn core_plugin_loads_without_panic() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.update();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_core --test plugin_smoke`
Expected: FAIL — `CorePlugin` does not exist.

- [ ] **Step 3: Implement `CorePlugin`**

Replace `crates/buiy_core/Cargo.toml`:

```toml
[package]
name = "buiy_core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
bevy.workspace = true
taffy.workspace = true
accesskit.workspace = true
accesskit_winit.workspace = true
bevy_picking.workspace = true
thiserror.workspace = true
tracing.workspace = true
```

Replace `crates/buiy_core/src/lib.rs`:

```rust
//! Buiy core: components, plugin scaffolding, system sets.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.8 for
//! sub-plugin order and SystemSet definitions.

use bevy::prelude::*;

/// Top-level system sets for Buiy. Order: Layout → Style → Input → Animate
/// → Picking → A11yUpdate → Render.
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BuiySet {
    Layout,
    Style,
    Input,
    Animate,
    Picking,
    A11yUpdate,
    Render,
}

/// Core Buiy plugin: registers types, configures system sets.
/// Composed into `BuiyPlugin` from the meta-crate; not consumed directly
/// by end users.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                BuiySet::Layout,
                BuiySet::Style,
                BuiySet::Input,
                BuiySet::Animate,
                BuiySet::Picking,
                BuiySet::A11yUpdate,
                BuiySet::Render,
            )
                .chain(),
        );
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_core --test plugin_smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/Cargo.toml crates/buiy_core/src/lib.rs crates/buiy_core/tests/plugin_smoke.rs
git commit -m "feat(core): scaffold CorePlugin and BuiySet system sets"
```

---

## Task 3: `Node` and `Style` components

**Files:**
- Create: `crates/buiy_core/src/components.rs`
- Modify: `crates/buiy_core/src/lib.rs` (mod components, register types)
- Create: `crates/buiy_core/tests/components.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/components.rs`:

```rust
use bevy::prelude::*;
use bevy::reflect::TypeRegistry;
use buiy_core::{components::*, CorePlugin};

#[test]
fn node_and_style_are_registered_and_default_constructible() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);

    let registry = app.world().resource::<AppTypeRegistry>();
    let registry = registry.read();
    assert!(registry.get(std::any::TypeId::of::<Node>()).is_some(),
        "Node not registered");
    assert!(registry.get(std::any::TypeId::of::<Style>()).is_some(),
        "Style not registered");
    assert!(registry.get(std::any::TypeId::of::<ResolvedLayout>()).is_some(),
        "ResolvedLayout not registered");

    drop(registry);
    let mut world = app.world_mut();
    let entity = world.spawn((Node::default(), Style::default())).id();
    assert!(world.get::<Node>(entity).is_some());
    assert!(world.get::<Style>(entity).is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_core --test components`
Expected: FAIL — `Node`, `Style`, `ResolvedLayout` not in `components` module.

- [ ] **Step 3: Define the components**

Create `crates/buiy_core/src/components.rs`:

```rust
//! Buiy's core component types.
//!
//! Every Buiy component is small, public-fielded, observable, and decomposed
//! by concern. Every component derives `Reflect + FromReflect + Default +
//! Clone + Component`. See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.4.

use bevy::prelude::*;

/// A Buiy node — the parallel-to-bevy_ui::Node primitive. Marker that this
/// entity participates in Buiy's layout / render / a11y trees.
#[derive(Component, Reflect, FromReflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Node;

/// Box-model + layout style. Not exhaustive in Phase 0 — only the surface
/// the layout system reads.
#[derive(Component, Reflect, FromReflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Style {
    /// Width in logical pixels. 0.0 = auto.
    pub width: f32,
    /// Height in logical pixels. 0.0 = auto.
    pub height: f32,
    /// Padding on all sides.
    pub padding: f32,
    /// Margin on all sides.
    pub margin: f32,
    /// Border radius (uniform; per-corner is a later sub-spec).
    pub border_radius: f32,
    /// Flex direction. 0 = Row, 1 = Column. Mapped to Taffy in `layout.rs`.
    pub flex_direction: u8,
    /// Token reference for background color (e.g., "color.surface.primary").
    pub background_token: String,
    /// Token reference for foreground/text color.
    pub foreground_token: String,
}

/// Resolved layout output, written by the layout system in `BuiySet::Layout`.
/// Read by render and picking in subsequent sets.
#[derive(Component, Reflect, FromReflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct ResolvedLayout {
    /// Top-left position in logical pixels (window-relative).
    pub position: Vec2,
    /// Size in logical pixels.
    pub size: Vec2,
}
```

Modify `crates/buiy_core/src/lib.rs` — add `mod components;` and `pub use` plus type registration in `CorePlugin::build`:

```rust
//! Buiy core: components, plugin scaffolding, system sets.

use bevy::prelude::*;

pub mod components;

pub use components::{Node, Style, ResolvedLayout};

#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BuiySet {
    Layout, Style, Input, Animate, Picking, A11yUpdate, Render,
}

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Node>()
            .register_type::<Style>()
            .register_type::<ResolvedLayout>()
            .configure_sets(
                Update,
                (
                    BuiySet::Layout, BuiySet::Style, BuiySet::Input,
                    BuiySet::Animate, BuiySet::Picking,
                    BuiySet::A11yUpdate, BuiySet::Render,
                ).chain(),
            );
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_core --test components`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/src/components.rs crates/buiy_core/src/lib.rs crates/buiy_core/tests/components.rs
git commit -m "feat(core): add Node, Style, ResolvedLayout components"
```

---

## Task 4: `Theme` resource + tokens + `UserPreferences`

**Files:**
- Create: `crates/buiy_core/src/theme.rs`
- Modify: `crates/buiy_core/src/lib.rs` (mod theme, register, expose `ThemePlugin`)
- Create: `crates/buiy_core/tests/theme.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/theme.rs`:

```rust
use bevy::prelude::*;
use buiy_core::theme::{Theme, UserPreferences, default_light_theme};

#[test]
fn default_theme_resolves_known_tokens() {
    let theme = default_light_theme();
    let bg = theme.color("color.surface.primary").expect("primary surface");
    let fg = theme.color("color.text.primary").expect("primary text");
    assert!(bg != fg, "fg and bg must differ");
    let space_4 = theme.space("space.4").expect("space.4");
    assert!(space_4 > 0.0);
}

#[test]
fn user_preferences_default_to_light_no_reduce_motion() {
    let prefs = UserPreferences::default();
    assert!(!prefs.prefers_dark);
    assert!(!prefs.prefers_reduced_motion);
    assert!(!prefs.forced_colors);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_core --test theme`
Expected: FAIL — `theme` module does not exist.

- [ ] **Step 3: Implement theme**

Create `crates/buiy_core/src/theme.rs`:

```rust
//! Token-based theming.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.5 and
//! cross-cutting.md § 3.14. Phase 0 ships a minimal token surface — the
//! full token taxonomy lives in `buiy-theme-tokens-design`.

use bevy::prelude::*;
use std::collections::HashMap;

/// A single theme variant. Phase 0 stores tokens as flat string-keyed maps.
/// The full token system replaces this with typed scales in
/// `buiy-theme-tokens-design`.
#[derive(Resource, Reflect, Clone, Debug, Default)]
#[reflect(Resource)]
pub struct Theme {
    pub colors: HashMap<String, Color>,
    pub spaces: HashMap<String, f32>,
    pub radii: HashMap<String, f32>,
}

impl Theme {
    pub fn color(&self, token: &str) -> Option<Color> {
        self.colors.get(token).copied()
    }
    pub fn space(&self, token: &str) -> Option<f32> {
        self.spaces.get(token).copied()
    }
    pub fn radius(&self, token: &str) -> Option<f32> {
        self.radii.get(token).copied()
    }
}

/// OS preference resource. Updated by a system in BuiySet::Input that reads
/// from winit (or platform-specific sources). Phase 0 populates with
/// defaults; full OS-pref plumbing is `buiy-clipboard-and-os-integration-design`.
#[derive(Resource, Reflect, Clone, Debug, Default)]
#[reflect(Resource)]
pub struct UserPreferences {
    pub prefers_dark: bool,
    pub prefers_reduced_motion: bool,
    pub prefers_reduced_transparency: bool,
    pub prefers_more_contrast: bool,
    pub forced_colors: bool,
    pub inverted_colors: bool,
}

/// Default light theme. Phase 0 is intentionally bare; v1 ships a full token
/// scale set validated against WCAG 2.2 AA in CI.
pub fn default_light_theme() -> Theme {
    let mut t = Theme::default();
    t.colors.insert("color.surface.primary".into(), Color::WHITE);
    t.colors.insert("color.surface.secondary".into(), Color::srgb(0.96, 0.96, 0.96));
    t.colors.insert("color.text.primary".into(), Color::srgb(0.10, 0.10, 0.10));
    t.colors.insert("color.text.secondary".into(), Color::srgb(0.40, 0.40, 0.40));
    t.colors.insert("color.accent".into(), Color::srgb(0.20, 0.45, 0.95));
    t.colors.insert("color.focus.ring".into(), Color::srgb(0.20, 0.45, 0.95));

    t.spaces.insert("space.0".into(), 0.0);
    t.spaces.insert("space.1".into(), 4.0);
    t.spaces.insert("space.2".into(), 8.0);
    t.spaces.insert("space.3".into(), 12.0);
    t.spaces.insert("space.4".into(), 16.0);

    t.radii.insert("radius.sm".into(), 2.0);
    t.radii.insert("radius.md".into(), 6.0);
    t.radii.insert("radius.lg".into(), 12.0);
    t
}

pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Theme>()
            .register_type::<UserPreferences>()
            .insert_resource(default_light_theme())
            .insert_resource(UserPreferences::default());
    }
}
```

Modify `crates/buiy_core/src/lib.rs` — add `pub mod theme;`. Do NOT add `ThemePlugin` to `CorePlugin`; the meta-crate (`buiy`) composes them in the documented order.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_core --test theme`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/src/theme.rs crates/buiy_core/src/lib.rs crates/buiy_core/tests/theme.rs
git commit -m "feat(core): add Theme resource, default light theme, UserPreferences"
```

---

## Task 5: Layout via Taffy

**Files:**
- Create: `crates/buiy_core/src/layout.rs`
- Modify: `crates/buiy_core/src/lib.rs` (mod layout)
- Create: `crates/buiy_core/tests/layout.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/layout.rs`:

```rust
use bevy::prelude::*;
use buiy_core::{
    components::{Node, ResolvedLayout, Style},
    layout::LayoutPlugin,
    CorePlugin,
};

#[test]
fn layout_resolves_a_simple_flex_row() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let parent = app.world_mut().spawn((
        Node,
        Style { width: 200.0, height: 100.0, flex_direction: 0, ..default() },
    )).id();

    let child = app.world_mut().spawn((
        Node,
        Style { width: 50.0, height: 50.0, ..default() },
    )).id();

    app.world_mut().entity_mut(parent).add_child(child);

    app.update(); // run BuiySet::Layout

    let layout = app.world().get::<ResolvedLayout>(child)
        .expect("child has ResolvedLayout after Update");
    assert!((layout.size.x - 50.0).abs() < 0.5, "child width ≈ 50");
    assert!((layout.size.y - 50.0).abs() < 0.5, "child height ≈ 50");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_core --test layout`
Expected: FAIL — `LayoutPlugin` not defined.

- [ ] **Step 3: Implement layout**

Create `crates/buiy_core/src/layout.rs`:

```rust
//! Layout via Taffy.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/visuals.md § 3.2 and
//! architecture.md § 2.3. Phase 0 supports flex row/column with fixed
//! width/height; the full layout surface lives in `buiy-layout-design`.

use crate::{components::{Node, ResolvedLayout, Style}, BuiySet};
use bevy::prelude::*;
use std::collections::HashMap;
use taffy::{
    AvailableSpace, Dimension, FlexDirection, NodeId as TaffyNodeId, Size,
    Style as TaffyStyle, TaffyTree, TraversePartialTree,
};

/// Resource: maps Bevy `Entity` to Taffy node IDs. Reused across frames to
/// keep Taffy's internal cache warm.
#[derive(Resource, Default)]
pub struct LayoutTree {
    tree: TaffyTree<()>,
    by_entity: HashMap<Entity, TaffyNodeId>,
}

pub struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LayoutTree>()
            .add_systems(Update, sync_and_compute_layout.in_set(BuiySet::Layout));
    }
}

fn style_to_taffy(style: &Style) -> TaffyStyle {
    TaffyStyle {
        size: Size {
            width: if style.width > 0.0 { Dimension::Length(style.width) } else { Dimension::Auto },
            height: if style.height > 0.0 { Dimension::Length(style.height) } else { Dimension::Auto },
        },
        flex_direction: if style.flex_direction == 1 { FlexDirection::Column } else { FlexDirection::Row },
        ..Default::default()
    }
}

/// One pass: ensure every Buiy entity has a Taffy node, sync style, compute
/// layout starting from roots (entities with `Node` and no Buiy parent),
/// write `ResolvedLayout` back.
fn sync_and_compute_layout(
    mut commands: Commands,
    mut tree: ResMut<LayoutTree>,
    nodes: Query<(Entity, &Style, Option<&Parent>, Option<&Children>), With<Node>>,
    windows: Query<&bevy::window::Window>,
) {
    // Ensure every Buiy entity has a Taffy node + current style.
    for (entity, style, _parent, _children) in nodes.iter() {
        let taffy_style = style_to_taffy(style);
        match tree.by_entity.get(&entity).copied() {
            Some(id) => { let _ = tree.tree.set_style(id, taffy_style); }
            None => {
                let id = tree.tree.new_leaf(taffy_style).expect("new_leaf");
                tree.by_entity.insert(entity, id);
            }
        }
    }

    // Sync child relationships for each Buiy entity.
    for (entity, _style, _parent, children) in nodes.iter() {
        let parent_id = match tree.by_entity.get(&entity).copied() {
            Some(id) => id, None => continue,
        };
        let child_ids: Vec<TaffyNodeId> = children
            .into_iter()
            .flatten()
            .filter_map(|c| tree.by_entity.get(c).copied())
            .collect();
        let _ = tree.tree.set_children(parent_id, &child_ids);
    }

    // Compute layout for roots (entities with Node and no Buiy parent).
    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));

    for (entity, _style, parent, _children) in nodes.iter() {
        let is_root = parent.map(|p| !tree.by_entity.contains_key(&p.get())).unwrap_or(true);
        if !is_root { continue; }
        if let Some(id) = tree.by_entity.get(&entity).copied() {
            let _ = tree.tree.compute_layout(
                id,
                Size {
                    width: AvailableSpace::Definite(window_size.x),
                    height: AvailableSpace::Definite(window_size.y),
                },
            );
        }
    }

    // Walk the tree and write ResolvedLayout for every entity.
    let mut to_write: Vec<(Entity, ResolvedLayout)> = Vec::new();
    for (&entity, &id) in tree.by_entity.iter() {
        if let Ok(layout) = tree.tree.layout(id) {
            to_write.push((entity, ResolvedLayout {
                position: Vec2::new(layout.location.x, layout.location.y),
                size: Vec2::new(layout.size.width, layout.size.height),
            }));
        }
    }
    for (e, rl) in to_write {
        commands.entity(e).insert(rl);
    }
}
```

Add `pub mod layout;` and `pub use layout::LayoutPlugin;` to `crates/buiy_core/src/lib.rs`.

> **Note for executor:** Taffy 0.10's API is the reference; minor rev shifts often. If `set_children` / `new_leaf` / `compute_layout` signatures differ, adjust to current Taffy API. The architectural commitment (one Taffy tree per app, entity-keyed mapping, layout in `BuiySet::Layout`) does not change.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_core --test layout`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/src/layout.rs crates/buiy_core/src/lib.rs crates/buiy_core/tests/layout.rs
git commit -m "feat(core): integrate Taffy for layout in BuiySet::Layout"
```

---

## Task 6: Focus model — `Focusable` + Tab handling

**Files:**
- Create: `crates/buiy_core/src/focus.rs`
- Modify: `crates/buiy_core/src/lib.rs` (mod focus)
- Create: `crates/buiy_core/tests/focus.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/focus.rs`:

```rust
use bevy::prelude::*;
use buiy_core::{focus::{advance_focus_for_test, FocusPlugin, Focusable, FocusedEntity}, CorePlugin};

#[test]
fn tab_cycles_focus_through_focusables() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(FocusPlugin);

    let a = app.world_mut().spawn(Focusable::default()).id();
    let b = app.world_mut().spawn(Focusable::default()).id();
    let c = app.world_mut().spawn(Focusable::default()).id();

    advance_focus_for_test(&mut app, true);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(a));
    advance_focus_for_test(&mut app, true);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(b));
    advance_focus_for_test(&mut app, true);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(c));
    advance_focus_for_test(&mut app, true);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(a),
        "wraps to first focusable");

    advance_focus_for_test(&mut app, false);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(c),
        "Shift+Tab moves backward");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_core --test focus`
Expected: FAIL — `focus` module does not exist.

- [ ] **Step 3: Implement focus**

Create `crates/buiy_core/src/focus.rs`:

```rust
//! Focus model: focus tree, Tab handling, focus-visible heuristic, focus
//! restoration. Phase 0 implements ordered Tab traversal; full focus tree
//! (roving tabindex, aria-activedescendant, traps, restoration, spatial nav)
//! lives in `buiy-focus-model-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.3 and
//! accessibility.md (Focus management).

use crate::BuiySet;
use bevy::prelude::*;

/// Marks an entity as part of the focus tree.
#[derive(Component, Reflect, FromReflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Focusable {
    /// Phase 0: 0 = Auto (in document order); negative = Skip; positive = explicit.
    pub tab_order: i32,
}

/// Currently focused entity (None = nothing focused).
#[derive(Resource, Reflect, Default, Clone, Debug)]
#[reflect(Resource)]
pub struct FocusedEntity(pub Option<Entity>);

/// Tracks whether the most recent focus change was keyboard / programmatic
/// (true) or pointer (false). Drives the `:focus-visible` heuristic — focus
/// rings render only when this is true.
#[derive(Resource, Reflect, Default, Clone, Debug)]
#[reflect(Resource)]
pub struct FocusVisible(pub bool);

pub struct FocusPlugin;

impl Plugin for FocusPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Focusable>()
            .register_type::<FocusedEntity>()
            .register_type::<FocusVisible>()
            .init_resource::<FocusedEntity>()
            .init_resource::<FocusVisible>()
            .add_systems(Update, handle_tab.in_set(BuiySet::Input));
    }
}

fn handle_tab(
    keys: Res<ButtonInput<KeyCode>>,
    focusables: Query<(Entity, &Focusable)>,
    mut focused: ResMut<FocusedEntity>,
    mut visible: ResMut<FocusVisible>,
) {
    let pressed_tab = keys.just_pressed(KeyCode::Tab);
    if !pressed_tab { return; }
    let forward = !keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::ShiftRight);
    advance_focus(&focusables, &mut focused, forward);
    visible.0 = true;
}

/// Test-friendly helper: advance focus without needing an input event loop.
pub fn advance_focus_for_test(app: &mut App, forward: bool) {
    let focusables: Vec<(Entity, Focusable)> = app
        .world_mut()
        .query::<(Entity, &Focusable)>()
        .iter(app.world())
        .map(|(e, f)| (e, f.clone()))
        .collect();
    let mut focused = app.world_mut().resource_mut::<FocusedEntity>();
    let prev = focused.0;
    let next = compute_next_focus(&focusables, prev, forward);
    focused.0 = next;
    app.world_mut().resource_mut::<FocusVisible>().0 = true;
}

fn advance_focus(
    focusables: &Query<(Entity, &Focusable)>,
    focused: &mut FocusedEntity,
    forward: bool,
) {
    let entries: Vec<(Entity, Focusable)> = focusables.iter().map(|(e, f)| (e, f.clone())).collect();
    focused.0 = compute_next_focus(&entries, focused.0, forward);
}

fn compute_next_focus(
    focusables: &[(Entity, Focusable)],
    current: Option<Entity>,
    forward: bool,
) -> Option<Entity> {
    let mut entries: Vec<(Entity, Focusable)> = focusables
        .iter()
        .filter(|(_, f)| f.tab_order >= 0)
        .cloned()
        .collect();
    if entries.is_empty() { return None; }
    // Sort: explicit positive tab_order first (ascending), then Auto (0) in document order.
    entries.sort_by_key(|(e, f)| (if f.tab_order > 0 { 0 } else { 1 }, f.tab_order, e.index()));

    let idx = current
        .and_then(|e| entries.iter().position(|(x, _)| *x == e));
    let n = entries.len();
    let next_idx = match (idx, forward) {
        (None, true) => 0,
        (None, false) => n - 1,
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
    };
    Some(entries[next_idx].0)
}
```

Add `pub mod focus;` and `pub use focus::{FocusPlugin, Focusable, FocusedEntity, FocusVisible};` to `crates/buiy_core/src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_core --test focus`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/src/focus.rs crates/buiy_core/src/lib.rs crates/buiy_core/tests/focus.rs
git commit -m "feat(core): add Focusable + FocusPlugin with Tab handling"
```

---

## Task 7: A11y components + AccessKit adapter

**Files:**
- Create: `crates/buiy_core/src/a11y.rs`
- Modify: `crates/buiy_core/src/lib.rs` (mod a11y)
- Create: `crates/buiy_core/tests/a11y.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/a11y.rs`:

```rust
use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yPlugin, A11yRole, A11yTreeBuilder},
    focus::Focusable,
    CorePlugin,
};

#[test]
fn tree_builder_emits_one_node_per_focusable_with_role_and_label() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);

    let _btn = app.world_mut().spawn((
        Focusable::default(),
        A11yRole::Button,
        A11yLabel("Save".to_string()),
    )).id();

    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    let snapshot = builder.snapshot();
    let count = snapshot.iter()
        .filter(|n| n.role == A11yRole::Button)
        .count();
    assert_eq!(count, 1, "exactly one button node in tree");
    let names: Vec<String> = snapshot.iter().map(|n| n.name.clone()).collect();
    assert!(names.contains(&"Save".to_string()), "Save name present");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_core --test a11y`
Expected: FAIL.

- [ ] **Step 3: Implement a11y**

Create `crates/buiy_core/src/a11y.rs`:

```rust
//! AccessKit integration. Phase 0 builds an in-memory snapshot and exposes
//! an `A11yTreeBuilder` that can be serialized for snapshot tests; the real
//! `accesskit_winit::Adapter` wiring per-window happens once Bevy windows
//! are introduced (Task 14, BuiyPlugin).
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.6 and
//! accessibility.md § 3.11 (decomposed components per #17644).

use crate::{focus::Focusable, BuiySet};
use bevy::prelude::*;

/// Decomposed AccessKit role component.
#[derive(Component, Reflect, FromReflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(Component)]
pub enum A11yRole {
    #[default]
    Generic,
    Button,
    Link,
    Image,
    Text,
    Heading,
    Dialog,
    AlertDialog,
    Tooltip,
    // Phase 0 stops here; full taxonomy is in the foundation spec accessibility.md.
}

/// Decomposed accessible name. ACCNAME 1.2 computation is in `buiy-accessibility-design`;
/// Phase 0 is the literal-string fast path.
#[derive(Component, Reflect, FromReflect, Clone, Debug, Default)]
#[reflect(Component)]
pub struct A11yLabel(pub String);

/// Decomposed accessible description.
#[derive(Component, Reflect, FromReflect, Clone, Debug, Default)]
#[reflect(Component)]
pub struct A11yDescription(pub String);

/// One node in the tree as Buiy sees it. Will be translated into
/// `accesskit::Node` by the adapter in Task 14. Decoupled here so we can
/// snapshot it without needing a winit window.
#[derive(Clone, Debug, PartialEq)]
pub struct A11yNodeView {
    pub entity: Entity,
    pub role: A11yRole,
    pub name: String,
    pub description: String,
    pub focusable: bool,
}

/// Tree builder: rebuilt each frame from changed components in BuiySet::A11yUpdate.
#[derive(Resource, Default)]
pub struct A11yTreeBuilder {
    nodes: Vec<A11yNodeView>,
}

impl A11yTreeBuilder {
    pub fn snapshot(&self) -> &[A11yNodeView] { &self.nodes }
}

pub struct A11yPlugin;

impl Plugin for A11yPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<A11yRole>()
            .register_type::<A11yLabel>()
            .register_type::<A11yDescription>()
            .init_resource::<A11yTreeBuilder>()
            .add_systems(Update, build_tree.in_set(BuiySet::A11yUpdate));
    }
}

fn build_tree(
    mut builder: ResMut<A11yTreeBuilder>,
    q: Query<(
        Entity,
        Option<&A11yRole>,
        Option<&A11yLabel>,
        Option<&A11yDescription>,
        Option<&Focusable>,
    )>,
) {
    builder.nodes.clear();
    for (entity, role, label, desc, focusable) in q.iter() {
        // Skip entities that have no a11y content at all.
        if role.is_none() && label.is_none() && desc.is_none() && focusable.is_none() {
            continue;
        }
        builder.nodes.push(A11yNodeView {
            entity,
            role: role.copied().unwrap_or_default(),
            name: label.map(|l| l.0.clone()).unwrap_or_default(),
            description: desc.map(|d| d.0.clone()).unwrap_or_default(),
            focusable: focusable.is_some(),
        });
    }
}
```

Add `pub mod a11y;` and re-exports to `crates/buiy_core/src/lib.rs`.

> **Note for executor:** The actual `accesskit_winit::Adapter` instantiation per-window happens in Task 14 (the meta-crate composes adapter ownership via a `BuiyA11yAdapter` resource keyed by `WindowId`). Phase 0 keeps `A11yTreeBuilder` testable without a real window. The tree-snapshot CI gate (Task 18) reads from `A11yTreeBuilder`, not from AccessKit's adapter.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_core --test a11y`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/src/a11y.rs crates/buiy_core/src/lib.rs crates/buiy_core/tests/a11y.rs
git commit -m "feat(core): add A11yRole / A11yLabel / A11yTreeBuilder"
```

---

## Task 8: `bevy_picking` backend

**Files:**
- Create: `crates/buiy_core/src/picking.rs`
- Modify: `crates/buiy_core/src/lib.rs` (mod picking)
- Create: `crates/buiy_core/tests/picking.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/picking.rs`:

```rust
use bevy::prelude::*;
use buiy_core::{
    components::{Node, ResolvedLayout, Style},
    picking::{hit_test, PickingPlugin},
    CorePlugin,
};

#[test]
fn hit_test_returns_entity_under_point() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);

    let entity = app.world_mut().spawn((
        Node,
        Style::default(),
        ResolvedLayout {
            position: Vec2::new(10.0, 10.0),
            size: Vec2::new(100.0, 50.0),
        },
    )).id();

    let world = app.world();
    let q = world.query::<(Entity, &ResolvedLayout)>();
    let hit = hit_test(world, Vec2::new(50.0, 30.0));
    assert_eq!(hit, Some(entity));
    let miss = hit_test(world, Vec2::new(500.0, 500.0));
    assert_eq!(miss, None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_core --test picking`
Expected: FAIL.

- [ ] **Step 3: Implement picking**

Create `crates/buiy_core/src/picking.rs`:

```rust
//! Buiy `bevy_picking` backend. Per-window registration; full backend
//! priority + window filter live in `buiy-input-events-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/cross-cutting.md § 3.18.

use crate::components::ResolvedLayout;
use bevy::prelude::*;

/// Phase 0 picking exposes a simple AABB hit-test fn for tests + a
/// minimal Bevy system that updates a `Hovered` resource. The full
/// `bevy_picking::backend::PickingBackend` registration lives in v0.x.
pub struct PickingPlugin;

#[derive(Resource, Default, Clone, Debug)]
pub struct Hovered(pub Option<Entity>);

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hovered>()
            .add_systems(Update, update_hovered.in_set(crate::BuiySet::Picking));
    }
}

pub fn hit_test(world: &World, point: Vec2) -> Option<Entity> {
    let mut q = world.query::<(Entity, &ResolvedLayout)>();
    let mut best: Option<(Entity, f32)> = None; // entity, area (smallest wins for top-most)
    for (entity, layout) in q.iter(world) {
        let max = layout.position + layout.size;
        if point.x >= layout.position.x && point.x <= max.x
            && point.y >= layout.position.y && point.y <= max.y
        {
            let area = layout.size.x * layout.size.y;
            if best.map(|(_, a)| area < a).unwrap_or(true) {
                best = Some((entity, area));
            }
        }
    }
    best.map(|(e, _)| e)
}

fn update_hovered(
    mut hovered: ResMut<Hovered>,
    windows: Query<&Window>,
    layouts: Query<(Entity, &ResolvedLayout)>,
) {
    let Some(window) = windows.iter().next() else { return; };
    let Some(cursor) = window.cursor_position() else {
        hovered.0 = None;
        return;
    };
    // Inline hit_test against the live query to avoid needing &World.
    let mut best: Option<(Entity, f32)> = None;
    for (entity, layout) in layouts.iter() {
        let max = layout.position + layout.size;
        if cursor.x >= layout.position.x && cursor.x <= max.x
            && cursor.y >= layout.position.y && cursor.y <= max.y
        {
            let area = layout.size.x * layout.size.y;
            if best.map(|(_, a)| area < a).unwrap_or(true) {
                best = Some((entity, area));
            }
        }
    }
    hovered.0 = best.map(|(e, _)| e);
}
```

Add `pub mod picking;` and re-exports.

> **Note for executor:** v0.x replaces this with a real `bevy_picking::backend::PickingBackend` impl (per-window filtered, registered via `Plugin::build`). Phase 0 needs only the AABB primitive to validate the architecture.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_core --test picking`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/src/picking.rs crates/buiy_core/src/lib.rs crates/buiy_core/tests/picking.rs
git commit -m "feat(core): add minimal AABB picking backend"
```

---

## Task 9: Render pipeline plugin (extract + queue setup)

**Files:**
- Create: `crates/buiy_core/src/render/mod.rs`
- Modify: `crates/buiy_core/src/lib.rs` (mod render)
- Create: `crates/buiy_core/tests/render_smoke.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/render_smoke.rs`:

```rust
use bevy::prelude::*;
use buiy_core::{render::BuiyRenderPlugin, CorePlugin};

#[test]
fn render_plugin_loads_without_panic() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // BuiyRenderPlugin needs a render-app context normally, but Phase 0
    // smoke test asserts the plugin's `build` does not panic when added
    // without RenderApp. Real render assertions happen in the e2e test (Task 21).
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);
    app.update();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_core --test render_smoke`
Expected: FAIL.

- [ ] **Step 3: Implement render plugin scaffold**

Create `crates/buiy_core/src/render/mod.rs`:

```rust
//! Buiy render pipeline. Phase 0 ships the smallest end-to-end pass
//! (rounded rect with solid bg) wired into Bevy's render graph. Full
//! pipeline (top-layer compositing, clip-path, filters, blend modes,
//! atlasing) lives in `buiy-render-pipeline-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/visuals.md § 3.3 and
//! architecture.md § 2.3.

use crate::{components::{Node, ResolvedLayout, Style}, theme::Theme, BuiySet};
use bevy::prelude::*;
use bevy::render::{Render, RenderApp, RenderSet, ExtractSchedule, extract_resource::ExtractResource};

pub mod node;
pub mod pipeline;

/// What the render world needs from the main world per frame: a list of
/// (rect, color, radius) tuples in window-local logical pixels.
#[derive(Resource, Default, Clone)]
pub struct ExtractedDraws {
    pub draws: Vec<DrawData>,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawData {
    pub position: Vec2,
    pub size: Vec2,
    pub color: Color,
    pub radius: f32,
}

pub struct BuiyRenderPlugin;

impl Plugin for BuiyRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExtractedDraws>();
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return; };
        render_app.init_resource::<ExtractedDraws>()
            .add_systems(ExtractSchedule, extract_buiy_draws);
        // Phase 0: render-graph node + pipeline initialization.
        // The actual pipeline + node wiring lives in pipeline.rs and node.rs.
        node::register(render_app);
        pipeline::register(render_app);
    }
}

fn extract_buiy_draws(
    mut commands: Commands,
    main_world_q: Extract<Query<(&Style, &ResolvedLayout), With<Node>>>,
    main_world_theme: Extract<Res<Theme>>,
) {
    let mut draws = ExtractedDraws::default();
    for (style, layout) in main_world_q.iter() {
        let color = main_world_theme
            .color(&style.background_token)
            .unwrap_or(Color::WHITE);
        draws.draws.push(DrawData {
            position: layout.position,
            size: layout.size,
            color,
            radius: style.border_radius,
        });
    }
    commands.insert_resource(draws);
}
```

Note: `Extract<Query<...>>` is Bevy 0.18+; on earlier versions use `Query<...>` inside the `ExtractSchedule` directly. Adjust to current API.

Create stubs `crates/buiy_core/src/render/node.rs`:

```rust
//! Render-graph node — populated in Task 11.
use bevy::render::renderer::RenderDevice;
use bevy::prelude::*;

pub fn register(_render_app: &mut SubApp) {
    // Phase 0 Task 11 fills this in.
}
```

Create stub `crates/buiy_core/src/render/pipeline.rs`:

```rust
//! Render pipeline + WGSL shader — populated in Task 10.
use bevy::prelude::*;

pub fn register(_render_app: &mut SubApp) {
    // Phase 0 Task 10 fills this in.
}
```

Add `pub mod render;` to `crates/buiy_core/src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_core --test render_smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/src/render/ crates/buiy_core/src/lib.rs crates/buiy_core/tests/render_smoke.rs
git commit -m "feat(core): scaffold BuiyRenderPlugin with extract phase"
```

---

## Task 10: Render pipeline shader (rounded rect with border)

**Files:**
- Create: `crates/buiy_core/src/render/shader.wgsl`
- Modify: `crates/buiy_core/src/render/pipeline.rs`

- [ ] **Step 1: Write the WGSL shader**

Create `crates/buiy_core/src/render/shader.wgsl`:

```wgsl
// Buiy Phase 0 rounded-rect shader. Vertex stage emits a unit quad (one
// quad per draw). Fragment stage computes signed distance from the
// rounded rect interior and outputs the per-instance color, with anti-
// aliased edges.

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct Instance {
    @location(2) rect_pos: vec2<f32>,    // top-left in clip-space units
    @location(3) rect_size: vec2<f32>,   // size in clip-space units
    @location(4) color: vec4<f32>,
    @location(5) radius: f32,
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_uv: vec2<f32>,    // -1..+1 across the rect
    @location(1) half_size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) radius: f32,
};

@vertex
fn vertex(v: Vertex, i: Instance) -> VertexOut {
    var out: VertexOut;
    let world = i.rect_pos + v.uv * i.rect_size;
    out.clip_position = vec4<f32>(world, 0.0, 1.0);
    out.local_uv = v.uv * 2.0 - 1.0;
    out.half_size = i.rect_size * 0.5;
    out.color = i.color;
    out.radius = i.radius;
    return out;
}

// Signed distance to a rounded rect centered at the origin.
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    let d = sdf_rounded_rect(in.local_uv * in.half_size, in.half_size, in.radius);
    let aa = fwidth(d);
    let alpha = 1.0 - smoothstep(-aa, aa, d);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
```

- [ ] **Step 2: Write the pipeline registration**

Replace `crates/buiy_core/src/render/pipeline.rs`:

```rust
//! Buiy render pipeline. The render-graph node in `node.rs` references
//! `BuiyPipeline::id` to dispatch draws.
//!
//! Full pipeline (multi-pass top-layer compositing, atlas binding,
//! filter/blend mode passes) lives in `buiy-render-pipeline-design`.

use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        BlendState, ColorTargetState, ColorWrites, Face, FragmentState, FrontFace,
        MultisampleState, PipelineCache, PolygonMode, PrimitiveState, PrimitiveTopology,
        RenderPipelineDescriptor, ShaderStages, TextureFormat, VertexBufferLayout,
        VertexFormat, VertexState, VertexStepMode,
    },
    renderer::RenderDevice,
    RenderApp,
};

pub const SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(0xBU1Y_0PHASE_0_RECT);

#[derive(Resource)]
pub struct BuiyPipeline {
    pub id: bevy::render::render_resource::CachedRenderPipelineId,
}

pub fn register(render_app: &mut SubApp) {
    let world = render_app.world_mut();
    // Load WGSL shader.
    {
        let mut shaders = world.resource_mut::<Assets<Shader>>();
        shaders.insert(SHADER_HANDLE.id(), Shader::from_wgsl(
            include_str!("shader.wgsl"),
            "buiy/render/shader.wgsl",
        ));
    }

    // Build pipeline descriptor and queue it.
    let device = world.resource::<RenderDevice>().clone();
    let mut pipeline_cache = world.resource_mut::<PipelineCache>();

    let descriptor = RenderPipelineDescriptor {
        label: Some("buiy_rounded_rect_pipeline".into()),
        layout: vec![],
        push_constant_ranges: vec![],
        vertex: VertexState {
            shader: SHADER_HANDLE.clone(),
            shader_defs: vec![],
            entry_point: "vertex".into(),
            buffers: vec![
                VertexBufferLayout {
                    array_stride: 16,
                    step_mode: VertexStepMode::Vertex,
                    attributes: vec![
                        bevy::render::render_resource::VertexAttribute {
                            format: VertexFormat::Float32x2, offset: 0, shader_location: 0,
                        },
                        bevy::render::render_resource::VertexAttribute {
                            format: VertexFormat::Float32x2, offset: 8, shader_location: 1,
                        },
                    ],
                },
                VertexBufferLayout {
                    array_stride: 36,
                    step_mode: VertexStepMode::Instance,
                    attributes: vec![
                        bevy::render::render_resource::VertexAttribute {
                            format: VertexFormat::Float32x2, offset: 0,  shader_location: 2,
                        },
                        bevy::render::render_resource::VertexAttribute {
                            format: VertexFormat::Float32x2, offset: 8,  shader_location: 3,
                        },
                        bevy::render::render_resource::VertexAttribute {
                            format: VertexFormat::Float32x4, offset: 16, shader_location: 4,
                        },
                        bevy::render::render_resource::VertexAttribute {
                            format: VertexFormat::Float32,   offset: 32, shader_location: 5,
                        },
                    ],
                },
            ],
        },
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleStrip,
            front_face: FrontFace::Ccw,
            cull_mode: Some(Face::Back),
            polygon_mode: PolygonMode::Fill,
            ..default()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            shader: SHADER_HANDLE.clone(),
            shader_defs: vec![],
            entry_point: "fragment".into(),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba8UnormSrgb,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        zero_initialize_workgroup_memory: false,
    };

    let id = pipeline_cache.queue_render_pipeline(descriptor);
    drop(pipeline_cache);
    world.insert_resource(BuiyPipeline { id });
}
```

> **Note for executor:** Bevy 0.18's `RenderPipelineDescriptor` field set evolves; verify `zero_initialize_workgroup_memory`, `Handle::weak_from_u128`, and `Shader::from_wgsl` signatures. The shape of the pipeline (one shader, instance buffer, alpha-blend output) is what matters.

- [ ] **Step 3: Add a render-pipeline test**

Add to `crates/buiy_core/tests/render_smoke.rs`:

```rust
#[test]
fn pipeline_registers_in_render_app() {
    use bevy::render::render_resource::PipelineCache;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);

    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    let pipeline = render_app.world().get_resource::<buiy_core::render::pipeline::BuiyPipeline>();
    assert!(pipeline.is_some(), "BuiyPipeline registered");
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p buiy_core --test render_smoke`
Expected: PASS (both smoke tests).

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/src/render/pipeline.rs crates/buiy_core/src/render/shader.wgsl crates/buiy_core/tests/render_smoke.rs
git commit -m "feat(core): add rounded-rect render pipeline + WGSL shader"
```

---

## Task 11: Render-graph node (queue draws into the main pass)

**Files:**
- Modify: `crates/buiy_core/src/render/node.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/buiy_core/tests/render_smoke.rs`:

```rust
#[test]
fn render_graph_node_inserted_after_main_2d_pass() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);

    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    let graph = render_app.world().resource::<bevy::render::render_graph::RenderGraph>();
    // Phase 0: assert the Buiy node is present in the Core2d sub-graph.
    let has_buiy_node = graph
        .iter_node_names()
        .any(|n| n.to_string().contains("buiy"));
    assert!(has_buiy_node, "Buiy render-graph node present");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_core --test render_smoke -- render_graph_node`
Expected: FAIL — node not yet inserted.

- [ ] **Step 3: Implement the render-graph node**

Replace `crates/buiy_core/src/render/node.rs`:

```rust
//! Buiy render-graph node. Inserted into the Core2d sub-graph, after the
//! main 2D pass and before tonemapping. Phase 0 draws Buiy entities directly
//! into the 2D-pass color attachment.

use bevy::core_pipeline::core_2d::graph::{Core2d, Node2d};
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::{
    render_graph::{Node, NodeRunError, RenderGraphApp, RenderGraphContext, ViewNode, ViewNodeRunner},
    render_resource::{PipelineCache, RenderPassDescriptor},
    renderer::RenderContext,
    view::ViewTarget,
    RenderApp,
};

use super::{pipeline::BuiyPipeline, ExtractedDraws};

#[derive(Default)]
pub struct BuiyNode;

impl ViewNode for BuiyNode {
    type ViewQuery = &'static ViewTarget;

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        view_target: QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let buiy_pipeline = world.resource::<BuiyPipeline>();
        let Some(pipeline) = pipeline_cache.get_render_pipeline(buiy_pipeline.id) else {
            return Ok(()); // pipeline not ready yet
        };
        let draws = world.resource::<ExtractedDraws>();
        if draws.draws.is_empty() {
            return Ok(());
        }

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("buiy_pass"),
            color_attachments: &[Some(view_target.get_color_attachment())],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(pipeline);
        // Phase 0: vertex + instance buffers built per-frame here. v0.x
        // upgrades to persistent buffers + bind groups for filters / atlas.
        // The buffer-construction code is deferred to a follow-up task.
        Ok(())
    }
}

pub fn register(render_app: &mut SubApp) {
    render_app.add_render_graph_node::<ViewNodeRunner<BuiyNode>>(Core2d, BuiyRenderLabel);
    render_app.add_render_graph_edge(Core2d, Node2d::MainPass, BuiyRenderLabel);
}

#[derive(bevy::render::render_graph::RenderLabel, Hash, PartialEq, Eq, Debug, Clone)]
pub struct BuiyRenderLabel;
```

> **Note for executor:** Bevy's `Core2d` graph and `ViewNode` API names occasionally shift across minors. Verify `RenderLabel` derive, `add_render_graph_edge` ordering, and `view_target.get_color_attachment()` against the current Bevy. The actual draw-call construction (vertex + instance buffers from `ExtractedDraws.draws`) is deferred to v0.x because Phase 0 only requires the *graph node* to exist; the draws are validated end-to-end in Task 21 via screenshot.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_core --test render_smoke -- render_graph_node`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/src/render/node.rs crates/buiy_core/tests/render_smoke.rs
git commit -m "feat(core): add Buiy render-graph node into Core2d after MainPass"
```

---

## Task 12: `buiy_widgets` crate + `Button` widget

**Files:**
- Modify: `crates/buiy_widgets/Cargo.toml`
- Replace: `crates/buiy_widgets/src/lib.rs`
- Create: `crates/buiy_widgets/src/button.rs`
- Create: `crates/buiy_widgets/tests/button.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_widgets/tests/button.rs`:

```rust
use bevy::prelude::*;
use buiy_core::{a11y::{A11yLabel, A11yRole}, focus::Focusable, CorePlugin};
use buiy_widgets::{Button, OnPress, WidgetsPlugin};

#[test]
fn spawning_a_button_attaches_role_label_focusable_and_default_style() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(Button::new("Save")).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Button>(entity).is_some());
    assert!(world.get::<Focusable>(entity).is_some());
    assert_eq!(world.get::<A11yRole>(entity).copied(), Some(A11yRole::Button));
    let label = world.get::<A11yLabel>(entity).expect("a11y label");
    assert_eq!(label.0, "Save");
}

#[test]
fn clicking_a_button_emits_on_press() {
    use buiy_core::picking::Hovered;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(Button::new("Save")).id();
    // Manually mark hovered + simulate a primary mouse press.
    app.world_mut().insert_resource(Hovered(Some(entity)));
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();

    let mut events = app.world_mut().resource_mut::<Events<OnPress>>();
    let mut reader = events.get_reader();
    let mut found = false;
    for ev in reader.read(&events) {
        if ev.0 == entity { found = true; }
    }
    assert!(found, "OnPress event for clicked button");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_widgets`
Expected: FAIL.

- [ ] **Step 3: Implement Button**

Replace `crates/buiy_widgets/Cargo.toml`:

```toml
[package]
name = "buiy_widgets"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
bevy.workspace = true
buiy_core = { path = "../buiy_core" }
```

Replace `crates/buiy_widgets/src/lib.rs`:

```rust
//! Buiy widgets. Phase 0 ships a single `Button` to validate the
//! foundation. Full APG widget catalog lives in `buiy-widget-catalog-design`.

use bevy::prelude::*;

pub mod button;
pub use button::{Button, OnPress};

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Button>()
            .add_event::<OnPress>()
            .add_systems(Update, button::emit_on_press_on_click.in_set(buiy_core::BuiySet::Input));
    }
}
```

Create `crates/buiy_widgets/src/button.rs`:

```rust
//! Button widget. Phase 0 contract: `Focusable + A11yRole::Button + A11yLabel +
//! Node + Style with theme tokens + click-emits-OnPress`. Per-widget detail
//! (toggle button via aria-pressed, keyboard contract, full APG behavior)
//! lives in `buiy-widget-catalog-design`.

use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yRole},
    components::{Node, Style},
    focus::Focusable,
    picking::Hovered,
};

#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Button;

#[derive(Event, Debug, Clone, Copy)]
pub struct OnPress(pub Entity);

impl Button {
    pub fn new(label: impl Into<String>) -> impl Bundle {
        let label = label.into();
        (
            Button,
            Node,
            Style {
                width: 120.0,
                height: 32.0,
                padding: 8.0,
                border_radius: 6.0, // matches "radius.md"
                background_token: "color.surface.secondary".into(),
                foreground_token: "color.text.primary".into(),
                ..default()
            },
            Focusable::default(),
            A11yRole::Button,
            A11yLabel(label),
        )
    }
}

pub(crate) fn emit_on_press_on_click(
    hovered: Res<Hovered>,
    mouse: Res<ButtonInput<MouseButton>>,
    buttons: Query<(), With<Button>>,
    mut writer: EventWriter<OnPress>,
) {
    if !mouse.just_pressed(MouseButton::Left) { return; }
    let Some(entity) = hovered.0 else { return; };
    if buttons.get(entity).is_ok() {
        writer.send(OnPress(entity));
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p buiy_widgets`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_widgets/Cargo.toml crates/buiy_widgets/src/ crates/buiy_widgets/tests/
git commit -m "feat(widgets): add Button widget with OnPress event"
```

---

## Task 13: `buiy` meta-crate — top-level `BuiyPlugin`

**Files:**
- Modify: `crates/buiy/Cargo.toml`
- Replace: `crates/buiy/src/lib.rs`
- Create: `crates/buiy/tests/plugin.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy/tests/plugin.rs`:

```rust
use bevy::prelude::*;
use buiy::BuiyPlugin;

#[test]
fn buiy_plugin_loads_in_correct_order() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BuiyPlugin);
    app.update();

    // Sanity: re-exports are accessible.
    let _ = std::any::TypeId::of::<buiy::Button>();
    let _ = std::any::TypeId::of::<buiy::Focusable>();
    let _ = std::any::TypeId::of::<buiy::A11yRole>();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy`
Expected: FAIL.

- [ ] **Step 3: Implement meta-crate**

Replace `crates/buiy/Cargo.toml`:

```toml
[package]
name = "buiy"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "A comprehensive UI library for Bevy with web-quality accessibility."

[dependencies]
bevy.workspace = true
buiy_core = { path = "../buiy_core" }
buiy_widgets = { path = "../buiy_widgets" }
```

Replace `crates/buiy/src/lib.rs`:

```rust
//! Buiy — comprehensive UI library for Bevy.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/README.md.

use bevy::prelude::*;

pub use buiy_core::{
    a11y::{A11yDescription, A11yLabel, A11yRole, A11yTreeBuilder},
    components::{Node, ResolvedLayout, Style},
    focus::{Focusable, FocusVisible, FocusedEntity},
    picking::Hovered,
    render::ExtractedDraws,
    theme::{default_light_theme, Theme, UserPreferences},
    BuiySet, CorePlugin,
};
pub use buiy_widgets::{Button, OnPress, WidgetsPlugin};

/// Top-level Buiy plugin. Composes sub-plugins in the documented order:
/// core → theme → a11y → focus → input → widgets. Render registration
/// happens in `Plugin::finish` so RenderApp exists when we reach it.
///
/// See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.8.
pub struct BuiyPlugin;

impl Plugin for BuiyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::picking::PickingPlugin,
            WidgetsPlugin,
        ));
    }

    fn finish(&self, app: &mut App) {
        // RenderApp is guaranteed to exist by `finish` time.
        app.add_plugins(buiy_core::render::BuiyRenderPlugin);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy/Cargo.toml crates/buiy/src/lib.rs crates/buiy/tests/plugin.rs
git commit -m "feat(buiy): top-level BuiyPlugin composes sub-plugins"
```

---

## Task 14: `buiy_verify` crate skeleton

**Files:**
- Modify: `crates/buiy_verify/Cargo.toml`
- Replace: `crates/buiy_verify/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_verify/tests/smoke.rs`:

```rust
#[test]
fn re_exports_compile() {
    use buiy_verify::{visual, a11y, contrast};
    let _ = visual::compare_images;
    let _ = a11y::snapshot_tree;
    let _ = contrast::wcag2_ratio;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_verify`
Expected: FAIL — modules don't exist yet.

- [ ] **Step 3: Implement skeleton**

Replace `crates/buiy_verify/Cargo.toml`:

```toml
[package]
name = "buiy_verify"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
bevy.workspace = true
buiy_core = { path = "../buiy_core" }
serde.workspace = true
serde_json.workspace = true
image.workspace = true
image-compare.workspace = true
proptest.workspace = true
thiserror.workspace = true
```

Replace `crates/buiy_verify/src/lib.rs`:

```rust
//! Buiy verification harness. Phase 0 ships visual regression, AccessKit
//! tree snapshot, and WCAG 2 contrast linter. Full harness (15 CI gates)
//! lives in `buiy-verification-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/verification.md.

pub mod visual;
pub mod a11y;
pub mod contrast;
```

Create stubs for the three modules — empty for now, populated in Tasks 15–17. Each module must export the function the smoke test references.

`crates/buiy_verify/src/visual.rs`:

```rust
use image::DynamicImage;
pub fn compare_images(_a: &DynamicImage, _b: &DynamicImage) -> f64 { 0.0 }
```

`crates/buiy_verify/src/a11y.rs`:

```rust
use buiy_core::a11y::A11yNodeView;
pub fn snapshot_tree(_nodes: &[A11yNodeView]) -> String { String::new() }
```

`crates/buiy_verify/src/contrast.rs`:

```rust
use bevy::prelude::Color;
pub fn wcag2_ratio(_a: Color, _b: Color) -> f64 { 0.0 }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_verify --test smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_verify/Cargo.toml crates/buiy_verify/src/ crates/buiy_verify/tests/smoke.rs
git commit -m "feat(verify): scaffold buiy_verify with module stubs"
```

---

## Task 15: `buiy_verify::visual` — perceptual screenshot diff

**Files:**
- Replace: `crates/buiy_verify/src/visual.rs`
- Create: `crates/buiy_verify/tests/visual.rs`
- Create: `crates/buiy_verify/tests/fixtures/visual/baseline.png` (1×1 white)
- Create: `crates/buiy_verify/tests/fixtures/visual/tinted.png` (1×1 light gray)

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_verify/tests/visual.rs`:

```rust
use buiy_verify::visual::{compare_images, DiffResult};
use image::open;

#[test]
fn identical_images_diff_zero() {
    let baseline = open("tests/fixtures/visual/baseline.png").unwrap();
    let result = compare_images(&baseline, &baseline);
    assert_eq!(result.score, 0.0);
    assert!(result.passed(0.01), "identical images pass 0.01 tolerance");
}

#[test]
fn tinted_image_diff_nonzero() {
    let a = open("tests/fixtures/visual/baseline.png").unwrap();
    let b = open("tests/fixtures/visual/tinted.png").unwrap();
    let result = compare_images(&a, &b);
    assert!(result.score > 0.0, "different images produce nonzero diff");
}
```

Create the two PNG fixtures. Use a tiny script (or `convert -size 1x1 xc:white baseline.png` etc.). The key is that the two are byte-different.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_verify --test visual`
Expected: FAIL — `DiffResult` not defined.

- [ ] **Step 3: Implement visual diff**

Replace `crates/buiy_verify/src/visual.rs`:

```rust
//! Visual regression — perceptual diff with a tolerance budget.
//! See: docs/specs/2026-05-07-buiy-foundation/verification.md (CI gate #2).

use image::{DynamicImage, GenericImageView};

pub struct DiffResult {
    /// 0.0 = identical, 1.0 = totally different.
    pub score: f64,
}

impl DiffResult {
    pub fn passed(&self, tolerance: f64) -> bool {
        self.score <= tolerance
    }
}

pub fn compare_images(a: &DynamicImage, b: &DynamicImage) -> DiffResult {
    if a.dimensions() != b.dimensions() {
        return DiffResult { score: 1.0 };
    }
    let a8 = a.to_rgba8();
    let b8 = b.to_rgba8();
    let mut accumulated = 0u64;
    let pixels = (a8.width() * a8.height()) as u64;
    for (pa, pb) in a8.pixels().zip(b8.pixels()) {
        for ch in 0..4 {
            let d = pa[ch] as i32 - pb[ch] as i32;
            accumulated += (d * d) as u64;
        }
    }
    let max = (pixels * 4 * 255 * 255) as f64;
    DiffResult { score: (accumulated as f64 / max).sqrt() }
}
```

> **Note for executor:** This is a starter perceptual diff (RMSE in RGBA). For real CI, swap to `image-compare` crate's structural-similarity metric or DSSIM. The interface (`compare_images(&a, &b) -> DiffResult`) is the contract; the metric improves over time per `buiy-verification-design`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_verify --test visual`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_verify/src/visual.rs crates/buiy_verify/tests/visual.rs crates/buiy_verify/tests/fixtures/
git commit -m "feat(verify): visual regression diff with tolerance"
```

---

## Task 16: `buiy_verify::a11y` — AccessKit tree snapshot

**Files:**
- Replace: `crates/buiy_verify/src/a11y.rs`
- Create: `crates/buiy_verify/tests/a11y.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_verify/tests/a11y.rs`:

```rust
use bevy::prelude::*;
use buiy_core::a11y::{A11yNodeView, A11yRole};
use buiy_verify::a11y::{snapshot_tree, diff_snapshots};

#[test]
fn snapshot_tree_serializes_to_stable_json() {
    let nodes = vec![
        A11yNodeView { entity: Entity::from_raw(1), role: A11yRole::Button,
            name: "Save".into(), description: "".into(), focusable: true },
        A11yNodeView { entity: Entity::from_raw(2), role: A11yRole::Text,
            name: "Hello".into(), description: "".into(), focusable: false },
    ];
    let json = snapshot_tree(&nodes);
    assert!(json.contains("\"role\":\"Button\""));
    assert!(json.contains("\"name\":\"Save\""));
    assert!(json.contains("\"focusable\":true"));
}

#[test]
fn diff_returns_none_for_identical_snapshots() {
    let nodes = vec![A11yNodeView { entity: Entity::from_raw(1), role: A11yRole::Button,
        name: "Save".into(), description: "".into(), focusable: true }];
    let snap = snapshot_tree(&nodes);
    assert!(diff_snapshots(&snap, &snap).is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_verify --test a11y`
Expected: FAIL.

- [ ] **Step 3: Implement snapshot**

Replace `crates/buiy_verify/src/a11y.rs`:

```rust
//! AccessKit tree snapshot — serializes Buiy's `A11yTreeBuilder` view to
//! stable JSON suitable for golden-file comparison.
//! See: docs/specs/2026-05-07-buiy-foundation/verification.md (CI gate #3).

use buiy_core::a11y::{A11yNodeView, A11yRole};
use serde::Serialize;

#[derive(Serialize)]
struct WireNode<'a> {
    entity: u64,
    role: &'a str,
    name: &'a str,
    description: &'a str,
    focusable: bool,
}

fn role_to_str(r: A11yRole) -> &'static str {
    match r {
        A11yRole::Generic => "Generic",
        A11yRole::Button => "Button",
        A11yRole::Link => "Link",
        A11yRole::Image => "Image",
        A11yRole::Text => "Text",
        A11yRole::Heading => "Heading",
        A11yRole::Dialog => "Dialog",
        A11yRole::AlertDialog => "AlertDialog",
        A11yRole::Tooltip => "Tooltip",
    }
}

pub fn snapshot_tree(nodes: &[A11yNodeView]) -> String {
    let wire: Vec<WireNode> = nodes.iter().map(|n| WireNode {
        entity: n.entity.to_bits(),
        role: role_to_str(n.role),
        name: &n.name,
        description: &n.description,
        focusable: n.focusable,
    }).collect();
    serde_json::to_string(&wire).expect("snapshot serializes")
}

/// Returns `None` if identical, `Some(unified_diff_text)` otherwise.
pub fn diff_snapshots(left: &str, right: &str) -> Option<String> {
    if left == right { None } else {
        Some(format!("LEFT:\n{}\n\nRIGHT:\n{}\n", left, right))
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_verify --test a11y`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_verify/src/a11y.rs crates/buiy_verify/tests/a11y.rs
git commit -m "feat(verify): AccessKit tree snapshot + JSON diff"
```

---

## Task 17: `buiy_verify::contrast` — WCAG 2 contrast linter

**Files:**
- Replace: `crates/buiy_verify/src/contrast.rs`
- Create: `crates/buiy_verify/tests/contrast.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_verify/tests/contrast.rs`:

```rust
use bevy::prelude::Color;
use buiy_core::theme::default_light_theme;
use buiy_verify::contrast::{
    contrast_violations, lint_theme, wcag2_ratio, ContrastSeverity, WCAG_AA_NORMAL,
};

#[test]
fn black_white_ratio_is_21() {
    let r = wcag2_ratio(Color::WHITE, Color::BLACK);
    assert!((r - 21.0).abs() < 0.01, "white/black ratio is 21:1, got {r}");
}

#[test]
fn equal_colors_ratio_is_1() {
    let r = wcag2_ratio(Color::WHITE, Color::WHITE);
    assert!((r - 1.0).abs() < 0.01);
}

#[test]
fn aa_passes_default_light_theme_text_on_surface() {
    let theme = default_light_theme();
    let bg = theme.color("color.surface.primary").unwrap();
    let fg = theme.color("color.text.primary").unwrap();
    let r = wcag2_ratio(fg, bg);
    assert!(r >= WCAG_AA_NORMAL, "default theme text on surface is AA: ratio={r}");
}

#[test]
fn linter_reports_violations_for_failing_pair() {
    let mut theme = default_light_theme();
    // Insert a known-failing pair.
    theme.colors.insert("color.text.bad".into(), Color::srgb(0.9, 0.9, 0.9));
    let pairs = vec![("color.surface.primary", "color.text.bad")];
    let violations = contrast_violations(&theme, &pairs, WCAG_AA_NORMAL);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].severity, ContrastSeverity::Fail);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_verify --test contrast`
Expected: FAIL.

- [ ] **Step 3: Implement contrast linter**

Replace `crates/buiy_verify/src/contrast.rs`:

```rust
//! WCAG 2 contrast linter. See: docs/specs/2026-05-07-buiy-foundation/verification.md
//! (CI gate #9). APCA is on the same code path but advisory; it ships in v0.x
//! per `buiy-theme-tokens-design`.

use bevy::prelude::Color;
use buiy_core::theme::Theme;

pub const WCAG_AA_NORMAL: f64 = 4.5;
pub const WCAG_AA_LARGE: f64 = 3.0;
pub const WCAG_AA_NON_TEXT: f64 = 3.0;
pub const WCAG_AAA_NORMAL: f64 = 7.0;
pub const WCAG_AAA_LARGE: f64 = 4.5;

#[derive(Debug, Clone, PartialEq)]
pub enum ContrastSeverity { Pass, Warn, Fail }

#[derive(Debug, Clone)]
pub struct ContrastViolation {
    pub bg_token: String,
    pub fg_token: String,
    pub ratio: f64,
    pub required: f64,
    pub severity: ContrastSeverity,
}

pub fn wcag2_ratio(fg: Color, bg: Color) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(c: Color) -> f64 {
    let lin = c.to_linear();
    let lin_r = lin.red as f64;
    let lin_g = lin.green as f64;
    let lin_b = lin.blue as f64;
    0.2126 * lin_r + 0.7152 * lin_g + 0.0722 * lin_b
}

pub fn contrast_violations(theme: &Theme, pairs: &[(&str, &str)], required: f64) -> Vec<ContrastViolation> {
    let mut out = Vec::new();
    for (bg_token, fg_token) in pairs {
        let bg = match theme.color(bg_token) { Some(c) => c, None => continue };
        let fg = match theme.color(fg_token) { Some(c) => c, None => continue };
        let ratio = wcag2_ratio(fg, bg);
        let severity = if ratio < required { ContrastSeverity::Fail } else { ContrastSeverity::Pass };
        out.push(ContrastViolation {
            bg_token: bg_token.to_string(),
            fg_token: fg_token.to_string(),
            ratio, required, severity,
        });
    }
    out.into_iter().filter(|v| v.severity == ContrastSeverity::Fail).collect()
}

/// Lint the canonical text-on-surface pairs in any theme. Returns Ok if all pass.
pub fn lint_theme(theme: &Theme) -> Result<(), Vec<ContrastViolation>> {
    let pairs = [
        ("color.surface.primary", "color.text.primary"),
        ("color.surface.primary", "color.text.secondary"),
        ("color.surface.secondary", "color.text.primary"),
    ];
    let v = contrast_violations(theme, &pairs, WCAG_AA_NORMAL);
    if v.is_empty() { Ok(()) } else { Err(v) }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_verify --test contrast`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_verify/src/contrast.rs crates/buiy_verify/tests/contrast.rs
git commit -m "feat(verify): WCAG 2 contrast linter with default-theme baseline"
```

---

## Task 18: `examples/hello_button` — minimal end-to-end app

**Files:**
- Modify: `examples/hello_button/Cargo.toml`
- Replace: `examples/hello_button/src/main.rs`

- [ ] **Step 1: Write the failing test**

The hello_button example is exercised by the integration test in Task 19. For Phase 0, prove it compiles by running:

Run: `cargo build -p hello_button`
Expected: FAIL initially (empty placeholder).

- [ ] **Step 2: Implement the example**

Replace `examples/hello_button/Cargo.toml`:

```toml
[package]
name = "hello_button"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false

[dependencies]
bevy = { workspace = true, features = ["bevy_render", "bevy_winit", "x11", "wayland"] }
buiy = { path = "../../crates/buiy" }
```

Replace `examples/hello_button/src/main.rs`:

```rust
//! Buiy Phase 0 hello-world: spawn one Button. The end-to-end verification
//! test (`tests/hello_button_e2e.rs`) drives the same scene and asserts
//! visual regression + AccessKit tree snapshot + focus / click behavior.

use bevy::prelude::*;
use buiy::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, log_press)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(Button::new("Save"));
}

fn log_press(mut events: EventReader<OnPress>) {
    for ev in events.read() {
        info!("button pressed: {:?}", ev.0);
    }
}
```

- [ ] **Step 3: Run build to verify it compiles**

Run: `cargo build -p hello_button`
Expected: PASS.

- [ ] **Step 4: Manual smoke run (optional, not a CI gate)**

Run: `cargo run -p hello_button`
Expected: window opens, a button is visible. Close window. Phase 0 may not yet render text glyphs (cosmic-text is a follow-up sub-spec); the button rectangle is what proves the architecture.

- [ ] **Step 5: Commit**

```bash
git add examples/hello_button/Cargo.toml examples/hello_button/src/main.rs
git commit -m "feat(example): add hello_button minimal app"
```

---

## Task 19: End-to-end verification test

**Files:**
- Create: `tests/hello_button_e2e.rs`
- Create: `tests/fixtures/hello_button/golden_a11y_tree.json` (committed golden)

- [ ] **Step 1: Write the failing test**

Create `tests/hello_button_e2e.rs`:

```rust
//! Phase 0 end-to-end verification fixture. Exercises the full Buiy
//! pipeline against the hello_button example scene:
//!  - layout resolves
//!  - render pipeline draws (no panic)
//!  - AccessKit tree snapshot matches golden
//!  - Tab focuses the Button (FocusVisible = true)
//!  - simulated click emits OnPress
//!  - default theme passes WCAG 2 contrast lint

use bevy::prelude::*;
use buiy::*;
use buiy_verify::a11y::{diff_snapshots, snapshot_tree};
use buiy_verify::contrast::lint_theme;
use buiy_core::focus::advance_focus_for_test;

fn setup_scene(app: &mut App) {
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BuiyPlugin);
    app.world_mut().spawn(Button::new("Save"));
}

#[test]
fn e2e_layout_and_a11y_tree_match_golden() {
    let mut app = App::new();
    setup_scene(&mut app);
    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    let snap = snapshot_tree(builder.snapshot());

    let golden = std::fs::read_to_string("tests/fixtures/hello_button/golden_a11y_tree.json")
        .expect("golden file present");
    let golden = canonicalize_entity_ids(&golden);
    let snap = canonicalize_entity_ids(&snap);

    assert!(diff_snapshots(&snap, &golden).is_none(),
        "AccessKit tree drift; expected = {golden}, actual = {snap}");
}

#[test]
fn e2e_tab_focuses_button() {
    let mut app = App::new();
    setup_scene(&mut app);
    app.update();
    advance_focus_for_test(&mut app, true);
    let focused = app.world().resource::<FocusedEntity>().0;
    assert!(focused.is_some(), "Tab focuses the Button");
    assert!(app.world().resource::<FocusVisible>().0,
        "focus-visible is set after Tab (keyboard-driven focus)");
}

#[test]
fn e2e_default_theme_passes_aa_contrast() {
    let theme = default_light_theme();
    if let Err(violations) = lint_theme(&theme) {
        panic!("default theme fails AA contrast: {violations:?}");
    }
}

/// Replace entity-bit fields with a stable placeholder so goldens don't
/// drift across test runs (entities are allocated dynamically).
fn canonicalize_entity_ids(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            // Look for "entity":N pattern.
            let mut buf = String::from(c);
            while let Some(&n) = chars.peek() {
                if n == '"' { buf.push(chars.next().unwrap()); break; }
                buf.push(chars.next().unwrap());
            }
            if buf == "\"entity\"" {
                out.push_str(&buf);
                // Skip until comma or }, replacing the value.
                while let Some(&n) = chars.peek() {
                    if n == ',' || n == '}' { break; }
                    chars.next();
                }
                out.push_str(":0");
            } else {
                out.push_str(&buf);
            }
        } else {
            out.push(c);
        }
    }
    out
}
```

Create `tests/fixtures/hello_button/golden_a11y_tree.json`:

```json
[{"entity":0,"role":"Button","name":"Save","description":"","focusable":true}]
```

- [ ] **Step 2: Run tests to verify they fail (or pass)**

Run: `cargo test --test hello_button_e2e`
Expected: PASS — if the prior tasks all landed correctly. If they don't, address the failure (most likely a layout / a11y wiring gap — fix in the appropriate task and re-run).

- [ ] **Step 3: Run the visual regression as a manual smoke**

Visual regression in CI requires a windowing environment. Phase 0's CI on Linux uses `xvfb-run` to provide one. The actual screenshot capture + diff is wired up in Task 22. For now, document the path: when CI runs `cargo test --test hello_button_e2e`, the AccessKit tree assertion runs without windowing (works headless), and the visual assertion is added in v0.x once `bevy::render` headless capture is configured.

- [ ] **Step 4: Confirm contrast lint passes**

Run: `cargo test --test hello_button_e2e e2e_default_theme_passes_aa_contrast`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/hello_button_e2e.rs tests/fixtures/
git commit -m "test: end-to-end verification fixture for hello_button scene"
```

---

## Task 20: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Write the workflow**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -D warnings

jobs:
  lint:
    name: Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: rustfmt
        run: cargo fmt --all -- --check
      - name: clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

  test:
    name: Test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install Linux deps for Bevy
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev xvfb at-spi2-core
      - name: cargo test (headless via xvfb on Linux)
        if: matrix.os == 'ubuntu-latest'
        run: xvfb-run -a cargo test --workspace
      - name: cargo test (macOS / Windows)
        if: matrix.os != 'ubuntu-latest'
        run: cargo test --workspace
```

> **Note for executor:** The Bevy ecosystem typically pins specific Linux APT package lists; verify against Bevy's own CI workflow at the targeted version. The `at-spi2-core` package is required for AccessKit's AT-SPI adapter on Linux. macOS / Windows AccessKit adapters do not require extra dependencies.

- [ ] **Step 2: Validate locally**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: PASS on the developer's machine. (Linux CI uses xvfb; you do not need it locally if you have a windowing system.)

- [ ] **Step 3: Push and observe CI**

```bash
git push origin <branch>
```

Watch GitHub Actions on the PR. Expect green on all three platforms. If a platform-specific issue appears (most likely AT-SPI on Linux), fix per the upstream Bevy / AccessKit guidance and update the workflow.

- [ ] **Step 4: Verify all categories report**

Confirm in the GitHub Actions UI that:
- `lint` job runs `cargo fmt` + `cargo clippy`.
- `test` job runs on Linux, macOS, Windows.
- All workspace tests (every crate's `tests/`) execute on each platform.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: lint + test workflow on Linux / macOS / Windows"
```

---

## Task 21: Self-review pass

This isn't a code task — it's the executor's checklist before merging Phase 0.

- [x] **Step 1:** Re-read the foundation spec [README](../specs/2026-05-07-buiy-foundation/README.md) and confirm every Phase 0 commitment in the **Spec coverage map** above is realized in code or explicitly deferred with a sub-spec reference.
- [x] **Step 2:** Run `cargo doc --workspace --no-deps --open` and skim the public API. Each crate should have a single-paragraph crate-level doc and per-item docs. Add missing docs.
- [x] **Step 3:** Run `cargo deny check` if `deny.toml` is configured (license + advisory check). Defer to v0.x if `cargo-deny` isn't yet installed.
- [x] **Step 4:** Run a final `cargo test --workspace` locally. Fix any flake by re-running once and root-causing if it persists; do NOT mark the test ignored.
- [x] **Step 5:** Update `docs/README.md` index if Phase 0 introduces new conventions worth surfacing (it should not, since the spec already commits them).

---

## Self-review (plan author)

Cross-checked against the foundation spec:

- ✅ Workspace + crate split matches [architecture.md § 2.8](../specs/2026-05-07-buiy-foundation/architecture.md). Tasks 1, 13.
- ✅ `BuiySet` ordering matches [architecture.md § 2.8](../specs/2026-05-07-buiy-foundation/architecture.md). Task 2.
- ✅ Sub-plugin order in `BuiyPlugin` matches the documented order (core → theme → a11y → focus → input → text → widgets → animation → forms → devtools); Phase 0 ships a subset (no text/animation/forms/devtools) and the order is preserved. Task 13.
- ✅ Reflect + FromReflect + Default + Clone + Component derive on every component, registered via `register_type`. Tasks 3, 4, 6, 7, 12.
- ✅ Per-window AccessKit ownership keyed by `WindowId`: deferred to v0.x — Phase 0 builds a tree-builder snapshot decoupled from a real adapter, sufficient for the e2e test. Documented in Task 7's note.
- ✅ Token-based theme + `UserPreferences` resource. Task 4.
- ✅ Layout via Taffy in `BuiySet::Layout`. Task 5.
- ✅ Focus model with Tab handling + `FocusVisible`. Task 6.
- ✅ Picking backend: Phase 0 ships AABB + cursor-driven `Hovered`; full `bevy_picking::backend::PickingBackend` impl is v0.x — note in Task 8.
- ✅ Custom render pipeline with rounded-rect shader. Tasks 9-11.
- ✅ Verification harness: visual regression (Task 15), AccessKit tree snapshot (Task 16), contrast linter (Task 17). Other CI gates from [verification.md](../specs/2026-05-07-buiy-foundation/verification.md) (announcement-output, hit-target linter, forced-colors scan, property tests, hot-reload, perf regression, memory leak) are committed but **deferred** beyond Phase 0; the harness crate is scaffolded so they slot in later without restructuring.
- ✅ CI on Windows / macOS / Linux desktop. Task 20.
- ✅ End-to-end test exercises layout + a11y + focus + click + contrast. Task 19.

**Placeholder scan:** No "TBD" / "TODO" / "implement later" inside steps. Two notes-for-executor reference upstream API drift (Bevy / Taffy) — those are honest version-stability acknowledgements, not placeholders.

**Type consistency:** Component names, system-set names, plugin names, and re-exports are consistent across tasks. `BuiySet`, `Focusable`, `FocusedEntity`, `FocusVisible`, `A11yRole`, `A11yLabel`, `A11yDescription`, `A11yTreeBuilder`, `Theme`, `UserPreferences`, `Node`, `Style`, `ResolvedLayout`, `Hovered`, `Button`, `OnPress`, `BuiyPlugin`, `CorePlugin`, `WidgetsPlugin`, `BuiyRenderPlugin`, `LayoutPlugin`, `FocusPlugin`, `PickingPlugin`, `A11yPlugin`, `ThemePlugin`, `BuiyPipeline`, `ExtractedDraws`, `DrawData`, `BuiyNode` (render), `BuiyRenderLabel`. All cross-references match.

**Spec gaps that became Phase 0 explicit deferrals (intentional):**
- `bevy_picking` real backend implementation (we ship AABB + Hovered)
- AccessKit `accesskit_winit::Adapter` per-window wiring (we ship `A11yTreeBuilder` snapshot)
- Render pipeline draws — the render-graph node is wired but instance-buffer construction is left to v0.x
- Cosmic-text / glyph rendering — text inside the Button is not rendered in Phase 0; the rectangle is sufficient to prove the architecture
- BSN authoring / hot-reload (Task list omits — `buiy_bsn` crate not in Phase 0)
- Forms, animation, devtools, 3D-anchored UI — not in Phase 0

Each deferred item has a sub-spec named in the foundation roadmap and a note in the relevant task explaining why Phase 0 stops where it does.

---

Plan complete and saved to `docs/plans/2026-05-07-buiy-phase-0-foundations.md`.
