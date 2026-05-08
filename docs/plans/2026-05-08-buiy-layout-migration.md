**Date:** 2026-05-08
**Status:** draft
**Spec:** docs/specs/2026-05-08-buiy-layout-design.md

# Buiy layout migration — Phase 0 → decomposed components

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate Phase 0's monolithic `Style` and `Node` marker into the decomposed component model from `2026-05-08-buiy-layout-design.md` § 1, preserving all current behavior. Single-stage layout pipeline retained.

**Architecture:** Pull layout-relevant fields out of `Style` into per-concern components (`Display`, `Size`, `BoxSpace`, `FlexLayout`); rename the `Node` marker to `LayoutNode`; introduce the `Length` enum (variants needed in this plan only: `Px`, `Percent`, `Auto`, `Fr`); add the `gc_layout_tree` system that closes the existing `LayoutTree` GC TODO. Visual fields (`background_token`, `foreground_token`, `border_radius`) move into a temporary `VisualStyle` component that lives until `buiy-theme-tokens-design` and `buiy-render-pipeline-design` land — explicitly out of scope here.

**Tech Stack:** Rust stable, Bevy 0.18, Taffy 0.10. No new dependencies. All work in branch `claude/buiy-layout-design`.

**Plan-of-plans context.** This is plan 1 of an expected ~5–7 covering the spec. Subsequent plans (no commitment to ordering yet): full `Length` (em/rem/vw/cqw/calc), pipeline stage decomposition (A→G), container queries, anchor positioning, sticky + writing-mode, grid features.

**Out of scope (call out and resist).**
- `Position` component, `Position::Sticky`, sticky offset pass.
- `WritingMode` component, logical edges, RTL/vertical writing modes.
- `Length::Em` / `Rem` / `Vw` / `Cqw` / `Token` / `Calc` (only `Px`/`Percent`/`Auto`/`Fr` here).
- Container queries (`ContainerContext`, `ContainerQueries`, Stage D).
- Anchor positioning (`AnchorName`, `AnchorTo`, `AnchorRegistry`, Stage E).
- Multi-stage pipeline. The current single-pass `sync_and_compute_layout` keeps its single-pass shape, just queries the new components.

If a task starts to drag in any of the above, stop and add a follow-up plan.

---

### Task 1: Rename `Node` → `LayoutNode`

The Phase 0 marker is `Node`, which collides with `taffy::Node` and `bevy::ecs::Node`. Rename across the workspace before introducing new components.

**Files:**
- Modify: `crates/buiy_core/src/components.rs`
- Modify: `crates/buiy_core/src/lib.rs`
- Modify: `crates/buiy_core/src/layout.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`
- Modify: `crates/buiy_core/tests/components.rs`
- Modify: `crates/buiy_core/tests/layout.rs`
- Modify: `crates/buiy_core/tests/picking.rs`
- Modify: `crates/buiy_widgets/src/button.rs`
- Modify: `crates/buiy/src/lib.rs`
- Modify: `tests/hello_button_e2e.rs` (if it imports `Node`)

- [ ] **Step 1: Search for every reference**

Run: `grep -rn --include="*.rs" "\bNode\b" crates/ tests/ | grep -v "taffy::Node\|bevy::.*Node\|//\|/\*"`

Expected: lines under `crates/buiy_core/src/components.rs`, `lib.rs`, `layout.rs`, `render/mod.rs`, the three test files, `crates/buiy_widgets/src/button.rs`, `crates/buiy/src/lib.rs`. No matches under `tests/hello_button_e2e.rs` is fine.

- [ ] **Step 2: Rename the type**

In `crates/buiy_core/src/components.rs`, change `pub struct Node;` (and its doc comment header) to:

```rust
/// Marker that this entity participates in Buiy's layout / render / a11y trees.
/// Renamed from `Node` (Phase 0) to avoid collision with `taffy::Node` and
/// `bevy::ecs::Node`. The previous name was internal — no public-API
/// deprecation surface to manage.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct LayoutNode;
```

- [ ] **Step 3: Rename every use site**

Replace `Node` with `LayoutNode` in:
- All `use ...components::{..., Node, ...}` paths.
- All `pub use components::{..., Node, ...}` re-exports.
- Every type position: `With<Node>` → `With<LayoutNode>`, `Node::default()` → `LayoutNode::default()` (or just `LayoutNode` since it's a unit struct), `(Node, Style { ... })` → `(LayoutNode, Style { ... })`.
- The `register_type::<Node>()` call in `crates/buiy_core/src/lib.rs` becomes `register_type::<LayoutNode>()`.

- [ ] **Step 4: Build, format, lint, test**

Run:
```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
xvfb-run -a cargo test --workspace
```
Expected: all green. The migration is mechanical — any failure here is a missed rename.

- [ ] **Step 5: Commit**

```
git add -A
git commit -m "refactor: rename Node marker to LayoutNode

Avoids collision with taffy::Node and bevy::ecs::Node. Phase 0 Node was
internal — no public deprecation surface. Migration step 1 of the
layout-design plan; component decomposition follows in subsequent commits."
```

---

### Task 2: Introduce the `Length` enum (subset)

This plan needs four variants: `Px`, `Percent`, `Auto`, `Fr`. The full enum (em/rem/vw/cqw/token/calc) lands in a follow-up plan.

**Files:**
- Create: `crates/buiy_core/src/length.rs`
- Modify: `crates/buiy_core/src/lib.rs`
- Test: `crates/buiy_core/tests/length.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/length.rs`:

```rust
use buiy_core::length::{IntoLength, Length};

#[test]
fn px_constructor_produces_px_variant() {
    let l: Length = 200.0.px();
    assert_eq!(l, Length::Px(200.0));
}

#[test]
fn percent_constructor_produces_percent_variant() {
    let l: Length = 50.0.percent();
    assert_eq!(l, Length::Percent(50.0));
}

#[test]
fn fr_constructor_produces_fr_variant() {
    let l: Length = 1.0.fr();
    assert_eq!(l, Length::Fr(1.0));
}

#[test]
fn default_is_auto() {
    let l = Length::default();
    assert_eq!(l, Length::Auto);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p buiy_core --test length`
Expected: FAIL — `buiy_core::length` module does not exist.

- [ ] **Step 3: Write the implementation**

Create `crates/buiy_core/src/length.rs`:

```rust
//! Length values for the layout subsystem.
//!
//! See: docs/specs/2026-05-08-buiy-layout-design.md § 2.
//!
//! This module ships a subset of the spec's `Length` enum: `Px`, `Percent`,
//! `Auto`, `Fr`. The full set (em/rem/vw/vh/cqw/cqh/token/calc) lands in a
//! follow-up plan; the variants here cover everything the layout system reads
//! today.

use bevy::prelude::Reflect;

/// A length value. Subset of the full `Length` enum from the layout spec —
/// other variants are added by follow-up plans without breaking these.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// Logical pixels.
    Px(f32),
    /// Percent of the containing block (resolved by Taffy at compute time).
    Percent(f32),
    /// Automatic — defer to layout intrinsic / parent / Taffy default.
    Auto,
    /// Grid track flexible factor. Only meaningful inside a grid template.
    Fr(f32),
}

impl Default for Length {
    fn default() -> Self {
        Length::Auto
    }
}

/// Construct `Length` values from numbers: `200.0.px()`, `50.0.percent()`.
pub trait IntoLength: Sized + Copy {
    fn px(self) -> Length;
    fn percent(self) -> Length;
    fn fr(self) -> Length;
}

impl IntoLength for f32 {
    fn px(self) -> Length { Length::Px(self) }
    fn percent(self) -> Length { Length::Percent(self) }
    fn fr(self) -> Length { Length::Fr(self) }
}

impl IntoLength for i32 {
    fn px(self) -> Length { Length::Px(self as f32) }
    fn percent(self) -> Length { Length::Percent(self as f32) }
    fn fr(self) -> Length { Length::Fr(self as f32) }
}
```

In `crates/buiy_core/src/lib.rs`, add `pub mod length;` near the existing `pub mod components;` declaration.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p buiy_core --test length`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```
git add crates/buiy_core/src/length.rs crates/buiy_core/src/lib.rs crates/buiy_core/tests/length.rs
git commit -m "feat(layout): introduce Length enum (Px/Percent/Auto/Fr subset)

Subset of the full Length type from the layout spec § 2. Variants needed
beyond this set (em/rem/vw/cqw/token/calc) land in follow-up plans without
breaking these. IntoLength trait gives 200.0.px() / 50.0.percent() ergonomics."
```

---

### Task 3: Length → Taffy conversion

Add `length_to_taffy_dimension` (for `Size`/`flex_basis`), `length_to_taffy_lp` (for padding/border/gap — no `Auto`/`Fr`), and `length_to_taffy_lpa` (for margin/inset — `Auto` allowed, no `Fr`).

**Files:**
- Modify: `crates/buiy_core/src/length.rs`
- Modify: `crates/buiy_core/tests/length.rs`

- [ ] **Step 1: Add failing tests for the three conversions**

Append to `crates/buiy_core/tests/length.rs`:

```rust
use buiy_core::length::{length_to_taffy_dimension, length_to_taffy_lp, length_to_taffy_lpa};
use taffy::{Dimension, LengthPercentage, LengthPercentageAuto};

#[test]
fn dimension_round_trip() {
    assert!(matches!(length_to_taffy_dimension(Length::Px(10.0)), Dimension::Length(x) if (x - 10.0).abs() < 1e-6));
    assert!(matches!(length_to_taffy_dimension(Length::Percent(50.0)), Dimension::Percent(x) if (x - 0.5).abs() < 1e-6));
    assert!(matches!(length_to_taffy_dimension(Length::Auto), Dimension::Auto));
    // Fr is not valid in Dimension context — coerces to Auto with a warn (we only assert the value).
    assert!(matches!(length_to_taffy_dimension(Length::Fr(1.0)), Dimension::Auto));
}

#[test]
fn length_percentage_coerces_invalid_to_zero() {
    assert!(matches!(length_to_taffy_lp(Length::Px(10.0)), LengthPercentage::Length(x) if (x - 10.0).abs() < 1e-6));
    assert!(matches!(length_to_taffy_lp(Length::Percent(25.0)), LengthPercentage::Percent(x) if (x - 0.25).abs() < 1e-6));
    // Auto and Fr are not valid in LengthPercentage context — coerce to Length(0.0).
    assert!(matches!(length_to_taffy_lp(Length::Auto), LengthPercentage::Length(x) if x.abs() < 1e-6));
    assert!(matches!(length_to_taffy_lp(Length::Fr(1.0)), LengthPercentage::Length(x) if x.abs() < 1e-6));
}

#[test]
fn length_percentage_auto_keeps_auto() {
    assert!(matches!(length_to_taffy_lpa(Length::Auto), LengthPercentageAuto::Auto));
    assert!(matches!(length_to_taffy_lpa(Length::Px(8.0)), LengthPercentageAuto::Length(x) if (x - 8.0).abs() < 1e-6));
    assert!(matches!(length_to_taffy_lpa(Length::Percent(10.0)), LengthPercentageAuto::Percent(x) if (x - 0.1).abs() < 1e-6));
    // Fr is not valid here — coerces to Length(0.0).
    assert!(matches!(length_to_taffy_lpa(Length::Fr(1.0)), LengthPercentageAuto::Length(x) if x.abs() < 1e-6));
}
```

- [ ] **Step 2: Run tests, verify they fail to compile**

Run: `cargo test -p buiy_core --test length`
Expected: compile error — `length_to_taffy_dimension` etc. not found.

- [ ] **Step 3: Implement the conversions**

Append to `crates/buiy_core/src/length.rs`:

```rust
use taffy::{Dimension, LengthPercentage, LengthPercentageAuto};
use tracing::warn;

/// Buiy `Length` → `taffy::Dimension`. Used for `Size`, `min_size`, `max_size`,
/// `flex_basis`. `Fr` is not valid here — coerces to `Auto` and warns.
pub fn length_to_taffy_dimension(l: Length) -> Dimension {
    match l {
        Length::Px(v) => Dimension::length(v),
        Length::Percent(v) => Dimension::percent(v / 100.0),
        Length::Auto => Dimension::auto(),
        Length::Fr(_) => {
            warn!(target: "buiy::layout::length_coerce", "Fr is not valid in Dimension context; coerced to Auto");
            Dimension::auto()
        }
    }
}

/// Buiy `Length` → `taffy::LengthPercentage`. Used for padding, border, gap.
/// `Auto` and `Fr` are not valid here — coerce to 0.0 and warn.
pub fn length_to_taffy_lp(l: Length) -> LengthPercentage {
    match l {
        Length::Px(v) => LengthPercentage::length(v),
        Length::Percent(v) => LengthPercentage::percent(v / 100.0),
        Length::Auto => {
            warn!(target: "buiy::layout::length_coerce", "Auto is not valid in LengthPercentage context; coerced to 0.0");
            LengthPercentage::length(0.0)
        }
        Length::Fr(_) => {
            warn!(target: "buiy::layout::length_coerce", "Fr is not valid in LengthPercentage context; coerced to 0.0");
            LengthPercentage::length(0.0)
        }
    }
}

/// Buiy `Length` → `taffy::LengthPercentageAuto`. Used for margin and `inset`.
/// `Fr` is not valid here — coerces to 0.0 and warns.
pub fn length_to_taffy_lpa(l: Length) -> LengthPercentageAuto {
    match l {
        Length::Px(v) => LengthPercentageAuto::length(v),
        Length::Percent(v) => LengthPercentageAuto::percent(v / 100.0),
        Length::Auto => LengthPercentageAuto::auto(),
        Length::Fr(_) => {
            warn!(target: "buiy::layout::length_coerce", "Fr is not valid in LengthPercentageAuto context; coerced to 0.0");
            LengthPercentageAuto::length(0.0)
        }
    }
}
```

- [ ] **Step 4: Verify the tests pass**

Run: `cargo test -p buiy_core --test length`
Expected: 7 tests pass.

- [ ] **Step 5: Commit**

```
git add crates/buiy_core/src/length.rs crates/buiy_core/tests/length.rs
git commit -m "feat(layout): add Length-to-Taffy conversions

Three conversions per the spec § 2 tradeoff: invalid context (e.g. Auto in
LengthPercentage) coerces to a sensible default (Length(0.0) or Auto) and
emits a warn!. Trades compile-time correctness for ergonomic single-Length
flow through builders/BSN/calc."
```

---

### Task 4: `Edges<T>` and `Size2D<T>`

Generic four-sided and two-axis containers. Logical accessors (`block_start` etc.) are deferred until the writing-mode plan — for now `Edges<T>` exposes only physical sides.

**Files:**
- Create: `crates/buiy_core/src/geometry.rs`
- Modify: `crates/buiy_core/src/lib.rs`
- Test: `crates/buiy_core/tests/geometry.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/geometry.rs`:

```rust
use buiy_core::geometry::{Edges, Size2D};
use buiy_core::length::{IntoLength, Length};

#[test]
fn edges_uniform_constructs_all_sides_equal() {
    let e = Edges::uniform(10.0.px());
    assert_eq!(e.top, 10.0.px());
    assert_eq!(e.right, 10.0.px());
    assert_eq!(e.bottom, 10.0.px());
    assert_eq!(e.left, 10.0.px());
}

#[test]
fn edges_default_is_zero() {
    let e: Edges<Length> = Edges::default();
    assert_eq!(e.top, Length::Px(0.0));
}

#[test]
fn size2d_default_zero() {
    let s: Size2D<Length> = Size2D::default();
    assert_eq!(s.row, Length::Px(0.0));
    assert_eq!(s.column, Length::Px(0.0));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p buiy_core --test geometry`
Expected: compile error — module missing.

- [ ] **Step 3: Implement**

Create `crates/buiy_core/src/geometry.rs`:

```rust
//! Geometry helpers: `Edges<T>` (four-sided) and `Size2D<T>` (two-axis).
//! Used by box-model and gap fields. Logical-edge accessors (`block_start`
//! etc.) are added when the writing-mode plan lands; for now, physical sides
//! only.

use crate::length::Length;
use bevy::prelude::Reflect;

/// Four physical sides. Logical aliases (block_start / inline_start) are added
/// later, gated on the writing-mode plan. `Reflect` requires `T: Reflect`.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub struct Edges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Copy> Edges<T> {
    pub fn uniform(v: T) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }
}

impl Default for Edges<Length> {
    fn default() -> Self { Edges::uniform(Length::Px(0.0)) }
}

/// Two-axis sizing — used for `gap` (row gap, column gap).
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub struct Size2D<T> {
    pub row: T,
    pub column: T,
}

impl Default for Size2D<Length> {
    fn default() -> Self { Self { row: Length::Px(0.0), column: Length::Px(0.0) } }
}
```

In `crates/buiy_core/src/lib.rs`, add `pub mod geometry;` next to the other `pub mod` lines.

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p buiy_core --test geometry`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```
git add crates/buiy_core/src/geometry.rs crates/buiy_core/src/lib.rs crates/buiy_core/tests/geometry.rs
git commit -m "feat(layout): add Edges<T> and Size2D<T> geometry helpers

Four-sided and two-axis containers used by BoxSpace and gap fields.
Logical-edge accessors (block_start etc.) are gated on the writing-mode plan."
```

---

### Task 5: `Display` component + `DisplayKind`

Always-present component opting into a layout mode. `DisplayKind::None` excludes the subtree from layout, focus, picking, and a11y (the spec § 7 kill switch). For this plan, only `Block`, `Flex`, and `None` are exercised — other variants compile as no-ops on the Taffy side until later plans.

**Files:**
- Modify: `crates/buiy_core/src/components.rs`

- [ ] **Step 1: Add the type**

Append to `crates/buiy_core/src/components.rs`:

```rust
use crate::length::Length;
use crate::geometry::{Edges, Size2D};

/// Display mode. Always present on a `LayoutNode`. See spec § 1.1.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct Display { pub kind: DisplayKind }

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub enum DisplayKind {
    #[default]
    Block,
    Inline,
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    FlowRoot,
    None,
    Contents,
}
```

- [ ] **Step 2: Write the conversion test**

Create `crates/buiy_core/tests/display.rs`:

```rust
use buiy_core::components::{Display, DisplayKind, display_to_taffy};
use taffy::Display as TaffyDisplay;

#[test]
fn block_maps_to_taffy_block() {
    assert!(matches!(display_to_taffy(DisplayKind::Block), TaffyDisplay::Block));
}

#[test]
fn flex_maps_to_taffy_flex() {
    assert!(matches!(display_to_taffy(DisplayKind::Flex), TaffyDisplay::Flex));
}

#[test]
fn none_maps_to_taffy_none() {
    assert!(matches!(display_to_taffy(DisplayKind::None), TaffyDisplay::None));
}

#[test]
fn default_is_block() {
    assert_eq!(Display::default().kind, DisplayKind::Block);
}
```

- [ ] **Step 3: Run, verify failure**

Run: `cargo test -p buiy_core --test display`
Expected: compile error — `display_to_taffy` not found.

- [ ] **Step 4: Implement `display_to_taffy`**

Append to `crates/buiy_core/src/components.rs`:

```rust
/// Map `DisplayKind` to Taffy's display.
///
/// Taffy 0.10 has only `Block`, `Flex`, `Grid`, `None`. Buiy's other variants
/// (`Inline`, `InlineBlock`, `InlineFlex`, `InlineGrid`, `FlowRoot`,
/// `Contents`) coerce to the closest Taffy mode for now; a follow-up plan
/// adds proper inline-formatting support when text rendering lands.
pub fn display_to_taffy(d: DisplayKind) -> taffy::Display {
    match d {
        DisplayKind::Block | DisplayKind::Inline | DisplayKind::InlineBlock | DisplayKind::FlowRoot | DisplayKind::Contents => taffy::Display::Block,
        DisplayKind::Flex | DisplayKind::InlineFlex => taffy::Display::Flex,
        DisplayKind::Grid | DisplayKind::InlineGrid => taffy::Display::Grid,
        DisplayKind::None => taffy::Display::None,
    }
}
```

- [ ] **Step 5: Verify**

Run: `cargo test -p buiy_core --test display`
Expected: 4 tests pass.

- [ ] **Step 6: Commit**

```
git add crates/buiy_core/src/components.rs crates/buiy_core/tests/display.rs
git commit -m "feat(layout): add Display component + DisplayKind enum

Always-present component opting into a layout mode (spec § 1.1).
Inline-formatting variants (Inline/InlineBlock/FlowRoot/Contents) coerce
to taffy::Display::Block until a follow-up plan adds inline support."
```

---

### Task 6: `Size` component

```rust
pub struct Size {
    pub width: Length, pub height: Length,
    pub min_width: Length, pub min_height: Length,
    pub max_width: Length, pub max_height: Length,
    pub aspect_ratio: Option<f32>,
}
```

**Files:**
- Modify: `crates/buiy_core/src/components.rs`
- Test: extend `crates/buiy_core/tests/components.rs`

- [ ] **Step 1: Add type**

Append to `crates/buiy_core/src/components.rs`:

```rust
/// Sizing constraints. See spec § 1.1. Default = all `Auto`, no aspect ratio.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct Size {
    pub width: Length, pub height: Length,
    pub min_width: Length, pub min_height: Length,
    pub max_width: Length, pub max_height: Length,
    pub aspect_ratio: Option<f32>,
}

impl Default for Size {
    fn default() -> Self {
        Self {
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            aspect_ratio: None,
        }
    }
}
```

- [ ] **Step 2: Add reflection-registration test**

Append to `crates/buiy_core/tests/components.rs`:

```rust
#[test]
fn size_is_reflection_registered() {
    use buiy_core::CorePlugin;
    use bevy::reflect::TypeRegistry;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    let registry = app
        .world()
        .resource::<bevy::reflect::AppTypeRegistry>()
        .read();
    assert!(
        registry.get(std::any::TypeId::of::<buiy_core::components::Size>()).is_some(),
        "Size not registered"
    );
}
```

- [ ] **Step 3: Run, verify failure**

Run: `cargo test -p buiy_core --test components size_is_reflection_registered`
Expected: FAIL — `Size` not in registry (we haven't wired registration yet — that's Task 12). The compile passes.

We'll come back to make this test green in Task 12. For now, let it fail. Remove `#[test]` annotation temporarily or mark `#[ignore]`. Use `#[ignore = "wired in Task 12"]` for clarity.

```rust
#[test]
#[ignore = "wired in Task 12 (reflection registration)"]
fn size_is_reflection_registered() { ... }
```

- [ ] **Step 4: Verify build still passes**

Run: `cargo build -p buiy_core`
Expected: success.

- [ ] **Step 5: Commit**

```
git add crates/buiy_core/src/components.rs crates/buiy_core/tests/components.rs
git commit -m "feat(layout): add Size component

Sizing constraints (width/height + min/max + aspect_ratio) per spec § 1.1.
Reflection registration test is #[ignore]'d until Task 12 wires registration."
```

---

### Task 7: `BoxSpace` component

Padding + margin + border + box-sizing.

**Files:**
- Modify: `crates/buiy_core/src/components.rs`

- [ ] **Step 1: Add types**

Append to `crates/buiy_core/src/components.rs`:

```rust
/// Padding, margin, border, and box-sizing model. See spec § 1.1.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct BoxSpace {
    pub padding: Edges<Length>,
    pub margin: Edges<Length>,
    pub border: Edges<Length>,
    pub box_sizing: BoxSizing,
}

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub enum BoxSizing {
    #[default]
    BorderBox,
    ContentBox,
}

impl Default for BoxSpace {
    fn default() -> Self {
        Self {
            padding: Edges::default(),
            margin: Edges::default(),
            border: Edges::default(),
            box_sizing: BoxSizing::BorderBox,
        }
    }
}
```

- [ ] **Step 2: Add a smoke test**

Create `crates/buiy_core/tests/box_space.rs`:

```rust
use buiy_core::components::{BoxSizing, BoxSpace};
use buiy_core::geometry::Edges;
use buiy_core::length::{IntoLength, Length};

#[test]
fn default_box_space_is_zero_borderbox() {
    let bs = BoxSpace::default();
    assert_eq!(bs.padding, Edges::uniform(Length::Px(0.0)));
    assert_eq!(bs.margin, Edges::uniform(Length::Px(0.0)));
    assert_eq!(bs.border, Edges::uniform(Length::Px(0.0)));
    assert_eq!(bs.box_sizing, BoxSizing::BorderBox);
}

#[test]
fn uniform_padding() {
    let bs = BoxSpace { padding: Edges::uniform(8.0.px()), ..Default::default() };
    assert_eq!(bs.padding.left, 8.0.px());
}
```

- [ ] **Step 3: Run, verify, commit**

Run: `cargo test -p buiy_core --test box_space`
Expected: 2 tests pass.

```
git add crates/buiy_core/src/components.rs crates/buiy_core/tests/box_space.rs
git commit -m "feat(layout): add BoxSpace component (padding/margin/border + box-sizing)"
```

---

### Task 8: `FlexLayout` component

Replaces Phase 0's `Style.flex_direction`. Opt-in; entities without it default to whatever Taffy uses for non-flex displays.

**Files:**
- Modify: `crates/buiy_core/src/components.rs`

- [ ] **Step 1: Replace the existing Phase-0 `FlexDirection` definition**

The Phase 0 `FlexDirection` (in `components.rs`) covers only `Row` / `Column`. Widen it to all four CSS variants and add the surrounding `FlexLayout` component. Replace the existing `FlexDirection` enum definition with:

```rust
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustifyContent {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Stretch,
}

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignItems {
    #[default]
    Stretch,
    Start,
    End,
    Center,
    Baseline,
}

pub type AlignContent = JustifyContent;
pub type AlignSelf = AlignItems;

/// Flex container properties + per-item placement on the same component.
/// Opt-in: present only when an entity actually uses flex.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct FlexLayout {
    pub direction: FlexDirection,
    pub wrap: FlexWrap,
    pub justify: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    pub gap: Size2D<Length>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Length,
    pub align_self: Option<AlignSelf>,
}

impl Default for FlexLayout {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            wrap: FlexWrap::NoWrap,
            justify: JustifyContent::Start,
            align_items: AlignItems::Stretch,
            align_content: AlignContent::Start,
            gap: Size2D::default(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
            align_self: None,
        }
    }
}
```

- [ ] **Step 2: Build and confirm `FlexDirection::RowReverse` etc. compile**

Run: `cargo build -p buiy_core`
Expected: success.

- [ ] **Step 3: Smoke test**

Append to `crates/buiy_core/tests/components.rs`:

```rust
#[test]
fn flex_layout_default_is_row_nowrap() {
    use buiy_core::components::{FlexDirection, FlexLayout, FlexWrap};
    let f = FlexLayout::default();
    assert_eq!(f.direction, FlexDirection::Row);
    assert_eq!(f.wrap, FlexWrap::NoWrap);
}
```

- [ ] **Step 4: Run, commit**

Run: `cargo test -p buiy_core --test components flex_layout_default_is_row_nowrap`
Expected: pass.

```
git add crates/buiy_core/src/components.rs crates/buiy_core/tests/components.rs
git commit -m "feat(layout): add FlexLayout component, widen FlexDirection

FlexLayout subsumes Phase 0 Style.flex_direction and adds the rest of the
flex container + item surface (justify/align/gap/grow/shrink/basis).
Opt-in: queries use Option<&FlexLayout> with sensible Taffy defaults
when the component is absent."
```

---

### Task 9: Carve `VisualStyle` from `Style`

`background_token`, `foreground_token`, and `border_radius` are not layout concerns. They move to a temporary `VisualStyle` component until `buiy-theme-tokens-design` and `buiy-render-pipeline-design` land. Renderer reads `VisualStyle` instead of `Style`.

**Files:**
- Modify: `crates/buiy_core/src/components.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`

- [ ] **Step 1: Add `VisualStyle`**

Append to `crates/buiy_core/src/components.rs`:

```rust
/// **Transitional.** Visual / theming fields that lived on Phase 0's `Style`
/// but are not layout concerns. Will move into theme + render specs
/// (`buiy-theme-tokens-design`, `buiy-render-pipeline-design`) when those
/// land. Renderer reads this in the meantime.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct VisualStyle {
    pub background_token: String,
    pub foreground_token: String,
    pub border_radius: f32,
}
```

- [ ] **Step 2: Update render extraction**

In `crates/buiy_core/src/render/mod.rs`, replace the `&Style` query with `&VisualStyle` and `&ResolvedLayout`. Concretely, change the import line:

```rust
use crate::components::{LayoutNode, ResolvedLayout, VisualStyle};
```

(Removing `Style` from the import. `LayoutNode` was renamed in Task 1, so this just keeps it in the import.) Then in the extract system body, replace `&Style` with `&VisualStyle` in the `Extract<Query<...>>` and replace every `style.background_token` / `style.foreground_token` / `style.border_radius` with `visual.background_token` / `visual.foreground_token` / `visual.border_radius`. Rename the binding from `style` to `visual` for clarity.

- [ ] **Step 3: Build, fix downstream errors, do not commit yet**

Run: `cargo build -p buiy_core`
Expected: errors elsewhere — `Style` is still referenced. We'll fix those in subsequent tasks. To temporarily keep the world buildable, leave `Style` defined for now (it's removed in Task 13).

This task ends with the codebase in a transient state. Commit anyway — the next tasks bring it back to green.

- [ ] **Step 4: Commit**

```
git add crates/buiy_core/src/components.rs crates/buiy_core/src/render/mod.rs
git commit -m "feat(layout): introduce transitional VisualStyle component

Moves background_token / foreground_token / border_radius off Style and
onto VisualStyle. Renderer extraction now reads VisualStyle. Style still
exists with only its layout fields until later tasks finish migrating
callers; deletion lands in Task 13."
```

---

### Task 10: Migrate `layout.rs` to read decomposed components

The single-pass pipeline shape stays. Only the query and the style-builder change.

**Files:**
- Modify: `crates/buiy_core/src/layout.rs`

- [ ] **Step 1: Replace the `style_to_taffy` helper**

Replace the existing `style_to_taffy` function with a `build_taffy_style` that takes the decomposed components by reference (using `Option` for opt-in components):

```rust
use crate::components::{BoxSpace, Display, DisplayKind, FlexLayout, LayoutNode, ResolvedLayout, Size, display_to_taffy};
use crate::length::{length_to_taffy_dimension, length_to_taffy_lp, length_to_taffy_lpa};
use taffy::{
    AvailableSpace, NodeId as TaffyNodeId, Size as TaffySize, Style as TaffyStyle, TaffyTree,
    LengthPercentage,
};

fn build_taffy_style(
    display: &Display,
    size: &Size,
    box_space: Option<&BoxSpace>,
    flex: Option<&FlexLayout>,
) -> TaffyStyle {
    let bs_default = BoxSpace::default();
    let bs = box_space.unwrap_or(&bs_default);
    let fl_default = FlexLayout::default();
    let fl = flex.unwrap_or(&fl_default);

    TaffyStyle {
        display: display_to_taffy(display.kind),
        size: TaffySize {
            width: length_to_taffy_dimension(size.width),
            height: length_to_taffy_dimension(size.height),
        },
        min_size: TaffySize {
            width: length_to_taffy_dimension(size.min_width),
            height: length_to_taffy_dimension(size.min_height),
        },
        max_size: TaffySize {
            width: length_to_taffy_dimension(size.max_width),
            height: length_to_taffy_dimension(size.max_height),
        },
        aspect_ratio: size.aspect_ratio,
        padding: taffy::Rect {
            top: length_to_taffy_lp(bs.padding.top),
            right: length_to_taffy_lp(bs.padding.right),
            bottom: length_to_taffy_lp(bs.padding.bottom),
            left: length_to_taffy_lp(bs.padding.left),
        },
        margin: taffy::Rect {
            top: length_to_taffy_lpa(bs.margin.top),
            right: length_to_taffy_lpa(bs.margin.right),
            bottom: length_to_taffy_lpa(bs.margin.bottom),
            left: length_to_taffy_lpa(bs.margin.left),
        },
        border: taffy::Rect {
            top: length_to_taffy_lp(bs.border.top),
            right: length_to_taffy_lp(bs.border.right),
            bottom: length_to_taffy_lp(bs.border.bottom),
            left: length_to_taffy_lp(bs.border.left),
        },
        flex_direction: match fl.direction {
            crate::components::FlexDirection::Row => taffy::FlexDirection::Row,
            crate::components::FlexDirection::Column => taffy::FlexDirection::Column,
            crate::components::FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            crate::components::FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        },
        flex_wrap: match fl.wrap {
            crate::components::FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
            crate::components::FlexWrap::Wrap => taffy::FlexWrap::Wrap,
            crate::components::FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
        },
        flex_grow: fl.flex_grow,
        flex_shrink: fl.flex_shrink,
        flex_basis: length_to_taffy_dimension(fl.flex_basis),
        gap: TaffySize {
            width: length_to_taffy_lp(fl.gap.column),
            height: length_to_taffy_lp(fl.gap.row),
        },
        ..Default::default()
    }
}
```

- [ ] **Step 2: Replace the system query**

Update `sync_and_compute_layout` to query decomposed components instead of `&Style`. The signature becomes:

```rust
#[allow(clippy::type_complexity)]
fn sync_and_compute_layout(
    mut commands: Commands,
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<
        (
            Entity,
            &Display,
            &Size,
            Option<&BoxSpace>,
            Option<&FlexLayout>,
            Option<&ChildOf>,
            Option<&Children>,
        ),
        With<LayoutNode>,
    >,
    windows: Query<&bevy::window::Window>,
) {
    let tree = &mut *tree;

    // Stage 1: ensure Taffy node + sync style
    for (entity, display, size, box_space, flex, _parent, _children) in nodes.iter() {
        let taffy_style = build_taffy_style(display, size, box_space, flex);
        match tree.by_entity.get(&entity).copied() {
            Some(id) => {
                if let Err(err) = tree.tree.set_style(id, taffy_style) {
                    warn!(?entity, ?err, "buiy: layout set_style failed");
                }
            }
            None => match tree.tree.new_leaf(taffy_style) {
                Ok(id) => { tree.by_entity.insert(entity, id); }
                Err(err) => {
                    warn!(?entity, ?err, "buiy: layout new_leaf failed; entity will be skipped this frame");
                }
            },
        }
    }

    // Stage 2: sync child relationships (unchanged)
    for (entity, _, _, _, _, _, children) in nodes.iter() {
        let parent_id = match tree.by_entity.get(&entity).copied() {
            Some(id) => id,
            None => continue,
        };
        let child_ids: Vec<TaffyNodeId> = children
            .into_iter()
            .flatten()
            .filter_map(|c| tree.by_entity.get(c).copied())
            .collect();
        if let Err(err) = tree.tree.set_children(parent_id, &child_ids) {
            warn!(?entity, ?err, "buiy: layout set_children failed");
        }
    }

    // Stage 3: compute layout for roots (unchanged logic, query shape adjusted)
    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));

    for (entity, _, _, _, _, parent, _) in nodes.iter() {
        let is_root = parent
            .map(|p| !tree.by_entity.contains_key(&p.parent()))
            .unwrap_or(true);
        if !is_root { continue; }
        if let Some(id) = tree.by_entity.get(&entity).copied()
            && let Err(err) = tree.tree.compute_layout(
                id,
                TaffySize {
                    width: AvailableSpace::Definite(window_size.x),
                    height: AvailableSpace::Definite(window_size.y),
                },
            )
        {
            warn!(?entity, ?err, "buiy: layout compute_layout failed");
        }
    }

    // Stage 4: writeback (unchanged)
    let mut to_write: Vec<(Entity, ResolvedLayout)> = Vec::new();
    for (&entity, &id) in tree.by_entity.iter() {
        if let Ok(layout) = tree.tree.layout(id) {
            to_write.push((
                entity,
                ResolvedLayout {
                    position: Vec2::new(layout.location.x, layout.location.y),
                    size: Vec2::new(layout.size.width, layout.size.height),
                },
            ));
        }
    }
    for (e, rl) in to_write {
        commands.entity(e).insert(rl);
    }
}
```

- [ ] **Step 3: Build (will fail downstream — that is fine)**

Run: `cargo build -p buiy_core`
Expected: layout.rs compiles. Other crates / tests still reference `Style`; they're fixed in the next tasks.

- [ ] **Step 4: Commit**

```
git add crates/buiy_core/src/layout.rs
git commit -m "refactor(layout): query decomposed components in layout system

Replaces &Style query with Display + Size + Option<BoxSpace> + Option<FlexLayout>.
Pipeline shape stays single-pass; multi-stage decomposition lands in a
follow-up plan."
```

---

### Task 11: Update `Button` widget and tests

**Files:**
- Modify: `crates/buiy_widgets/src/button.rs`
- Modify: `crates/buiy_core/tests/layout.rs`
- Modify: `crates/buiy_core/tests/picking.rs`
- Modify: `crates/buiy_core/tests/components.rs`
- Modify: `tests/hello_button_e2e.rs`

- [ ] **Step 1: Update `Button::bundle`**

In `crates/buiy_widgets/src/button.rs`, replace the `Style { ... }` literal with the decomposed bundle. Update the imports first:

```rust
use buiy_core::{
    components::{BoxSpace, Display, DisplayKind, FlexDirection, FlexLayout, LayoutNode, Size, VisualStyle},
    geometry::Edges,
    length::IntoLength,
    ...
};
```

Then replace the spawn bundle:

```rust
(
    LayoutNode,
    Display { kind: DisplayKind::Flex },
    Size {
        width: 120.0.px(),
        height: 32.0.px(),
        ..Default::default()
    },
    BoxSpace {
        padding: Edges::uniform(8.0.px()),
        ..Default::default()
    },
    FlexLayout {
        direction: FlexDirection::Row,
        ..Default::default()
    },
    VisualStyle {
        background_token: "color.surface.secondary".into(),
        foreground_token: "color.text.primary".into(),
        border_radius: 6.0,
    },
    Focusable::default(),
    A11yRole::Button,
    A11yLabel(label),
)
```

- [ ] **Step 2: Update `crates/buiy_core/tests/layout.rs`**

Replace the existing test body's `Style { ... }` literals with decomposed components. The new file content:

```rust
use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Display, DisplayKind, FlexDirection, FlexLayout, LayoutNode, ResolvedLayout, Size},
    layout::LayoutPlugin,
    length::IntoLength,
};

#[test]
fn layout_resolves_a_simple_flex_row() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let parent = app
        .world_mut()
        .spawn((
            LayoutNode,
            Display { kind: DisplayKind::Flex },
            Size { width: 200.0.px(), height: 100.0.px(), ..Default::default() },
            FlexLayout { direction: FlexDirection::Row, ..Default::default() },
        ))
        .id();

    let child = app
        .world_mut()
        .spawn((
            LayoutNode,
            Display { kind: DisplayKind::Block },
            Size { width: 50.0.px(), height: 50.0.px(), ..Default::default() },
        ))
        .id();

    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    let layout = app
        .world()
        .get::<ResolvedLayout>(child)
        .expect("child has ResolvedLayout after Update");
    assert!((layout.size.x - 50.0).abs() < 0.5, "child width ~ 50");
    assert!((layout.size.y - 50.0).abs() < 0.5, "child height ~ 50");
}
```

- [ ] **Step 3: Update `crates/buiy_core/tests/picking.rs`**

Replace `Style::default()` and any `Node` with decomposed components. The test only needs a `LayoutNode` + sized `ResolvedLayout` for picking — it doesn't run a layout pass — so:

```rust
use buiy_core::{
    components::{Display, DisplayKind, LayoutNode, ResolvedLayout, Size},
    length::IntoLength,
    ...
};

// ...inside the test body, replace the existing spawn:
let entity = app.world_mut().spawn((
    LayoutNode,
    Display { kind: DisplayKind::Block },
    Size { width: 100.0.px(), height: 50.0.px(), ..Default::default() },
    ResolvedLayout { position: Vec2::ZERO, size: Vec2::new(100.0, 50.0) },
)).id();
```

(Use the actual Vec2 values from the existing test; preserve its assertions.)

- [ ] **Step 4: Update `crates/buiy_core/tests/components.rs`**

The existing test that asserts `Style` is registered must become an assertion about `Display + Size`:

```rust
#[test]
fn layout_components_are_reflection_registered() {
    use bevy::reflect::AppTypeRegistry;
    use buiy_core::components::{Display, Size};
    use buiy_core::CorePlugin;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    let registry = app.world().resource::<AppTypeRegistry>().read();
    assert!(registry.get(std::any::TypeId::of::<Display>()).is_some(), "Display not registered");
    assert!(registry.get(std::any::TypeId::of::<Size>()).is_some(), "Size not registered");
}
```

Delete the old `Style` registration test. The previously `#[ignore]`'d `size_is_reflection_registered` test from Task 6 is now subsumed by this combined test — delete the ignored placeholder.

The spawn smoke test that did `world.spawn((Node::default(), Style::default()))` becomes:

```rust
let entity = world.spawn((LayoutNode, Display::default(), Size::default())).id();
assert!(world.get::<LayoutNode>(entity).is_some());
assert!(world.get::<Display>(entity).is_some());
assert!(world.get::<Size>(entity).is_some());
```

- [ ] **Step 5: Update `tests/hello_button_e2e.rs` if it imports `Style`**

Run: `grep -n "Style\|background_token\|foreground_token" tests/hello_button_e2e.rs`. If anything matches, replace usages following the same pattern as the Button widget (Step 1). If nothing matches, skip.

- [ ] **Step 6: Build**

Run: `cargo build --workspace`
Expected: success.

- [ ] **Step 7: Run tests**

Run: `xvfb-run -a cargo test --workspace`
Expected: all green.

- [ ] **Step 8: Commit**

```
git add crates/buiy_widgets/src/button.rs crates/buiy_core/tests/layout.rs crates/buiy_core/tests/picking.rs crates/buiy_core/tests/components.rs tests/hello_button_e2e.rs
git commit -m "refactor: migrate Button widget and tests to decomposed components"
```

---

### Task 12: Wire reflection registration

**Files:**
- Modify: `crates/buiy_core/src/lib.rs`

- [ ] **Step 1: Add registrations**

In `crates/buiy_core/src/lib.rs`, find the existing `register_type::<...>()` chain inside `CorePlugin::build`. Add registrations for every new type. After the change, the chain should include at least:

```rust
.register_type::<LayoutNode>()
.register_type::<Display>()
.register_type::<DisplayKind>()
.register_type::<Size>()
.register_type::<BoxSpace>()
.register_type::<BoxSizing>()
.register_type::<FlexLayout>()
.register_type::<FlexDirection>()
.register_type::<FlexWrap>()
.register_type::<JustifyContent>()
.register_type::<AlignItems>()
.register_type::<VisualStyle>()
.register_type::<ResolvedLayout>()
.register_type::<Length>()
.register_type::<Edges<Length>>()
.register_type::<Size2D<Length>>()
```

Remove the now-stale `register_type::<Style>()` and `register_type::<Node>()` calls.

Add the necessary imports at the top of `lib.rs`:

```rust
use crate::components::{
    AlignItems, BoxSizing, BoxSpace, Display, DisplayKind, FlexDirection, FlexLayout, FlexWrap,
    JustifyContent, LayoutNode, ResolvedLayout, Size, VisualStyle,
};
use crate::geometry::{Edges, Size2D};
use crate::length::Length;
```

- [ ] **Step 2: Update re-exports**

In `crates/buiy_core/src/lib.rs`, the `pub use components::{...}` line currently re-exports `{FlexDirection, Node, ResolvedLayout, Style}`. Replace with the new public surface:

```rust
pub use components::{
    AlignItems, BoxSizing, BoxSpace, Display, DisplayKind, FlexDirection, FlexLayout, FlexWrap,
    JustifyContent, LayoutNode, ResolvedLayout, Size, VisualStyle,
};
pub use geometry::{Edges, Size2D};
pub use length::{IntoLength, Length};
```

In `crates/buiy/src/lib.rs`, mirror the same re-exports (the umbrella crate re-exports from `buiy_core`).

- [ ] **Step 3: Build, test**

Run:
```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
xvfb-run -a cargo test --workspace
```

Expected: all green. The `layout_components_are_reflection_registered` test from Task 11 should pass.

- [ ] **Step 4: Commit**

```
git add crates/buiy_core/src/lib.rs crates/buiy/src/lib.rs
git commit -m "feat(layout): register reflection for decomposed layout components

LayoutNode, Display, DisplayKind, Size, BoxSpace, BoxSizing, FlexLayout,
FlexDirection, FlexWrap, JustifyContent, AlignItems, VisualStyle, Length,
Edges<Length>, Size2D<Length>. Drops the stale Style and Node registrations.
Re-exports updated through both buiy_core and the buiy umbrella crate."
```

---

### Task 13: Delete `Style`

Now that nothing reads it, remove the type.

**Files:**
- Modify: `crates/buiy_core/src/components.rs`

- [ ] **Step 1: Verify nothing references `Style`**

Run: `grep -rn --include="*.rs" "\bStyle\b" crates/ tests/ | grep -v "TaffyStyle\|taffy::Style\|VisualStyle\|//\|/\*"`
Expected: no matches.

- [ ] **Step 2: Delete the type**

In `crates/buiy_core/src/components.rs`, delete the `pub struct Style { ... }` definition entirely.

- [ ] **Step 3: Build, lint, test**

Run:
```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
xvfb-run -a cargo test --workspace
```
Expected: all green.

- [ ] **Step 4: Commit**

```
git add crates/buiy_core/src/components.rs
git commit -m "refactor(layout): remove Phase 0 Style component

Subsumed by Display + Size + BoxSpace + FlexLayout (layout fields) and
VisualStyle (transitional theme/visual fields). Pre-0.1, no public API
deprecation surface."
```

---

### Task 14: `gc_layout_tree` system — closes the Phase 0 TODO

LayoutTree currently grows monotonically across despawns. Add a system that drops Taffy nodes when their `LayoutNode` component is removed (which happens on despawn or explicit `remove::<LayoutNode>()`).

**Files:**
- Modify: `crates/buiy_core/src/layout.rs`
- Test: `crates/buiy_core/tests/layout_gc.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/buiy_core/tests/layout_gc.rs`:

```rust
use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Display, DisplayKind, LayoutNode, Size},
    layout::{LayoutPlugin, LayoutTree},
    length::IntoLength,
};

#[test]
fn despawn_removes_taffy_node() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let entity = app
        .world_mut()
        .spawn((
            LayoutNode,
            Display { kind: DisplayKind::Block },
            Size { width: 100.0.px(), height: 100.0.px(), ..Default::default() },
        ))
        .id();

    app.update(); // sync_and_compute_layout creates the Taffy node

    {
        let tree = app.world().non_send_resource::<LayoutTree>();
        assert!(tree.contains_entity(entity), "entity has Taffy node after first update");
    }

    app.world_mut().despawn(entity);
    app.update(); // gc_layout_tree should drop it

    let tree = app.world().non_send_resource::<LayoutTree>();
    assert!(!tree.contains_entity(entity), "entity's Taffy node was GC'd after despawn");
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p buiy_core --test layout_gc`
Expected: compile error — `LayoutTree::contains_entity` does not exist; `gc_layout_tree` not added.

- [ ] **Step 3: Implement**

In `crates/buiy_core/src/layout.rs`:

a) Add a `contains_entity` accessor on `LayoutTree`:

```rust
impl LayoutTree {
    pub fn contains_entity(&self, e: Entity) -> bool {
        self.by_entity.contains_key(&e)
    }
}
```

b) Add the `gc_layout_tree` system:

```rust
fn gc_layout_tree(
    mut tree: NonSendMut<LayoutTree>,
    mut removed: RemovedComponents<LayoutNode>,
) {
    for entity in removed.read() {
        if let Some(id) = tree.by_entity.remove(&entity)
            && let Err(err) = tree.tree.remove(id)
        {
            warn!(?entity, ?err, "buiy: layout tree remove failed");
        }
    }
}
```

c) Register it in `LayoutPlugin::build`:

```rust
app.init_non_send_resource::<LayoutTree>()
    .add_systems(
        Update,
        (sync_and_compute_layout, gc_layout_tree).chain().in_set(BuiySet::Layout),
    );
```

`gc_layout_tree` runs after `sync_and_compute_layout` so a freshly-despawned entity that was visible to layout this frame still gets its Taffy node, then dropped — simpler invariant than racing the events.

Also remove the `TODO(buiy-layout-design)` comment block from `LayoutTree`'s definition (it's now done).

- [ ] **Step 4: Verify**

Run: `cargo test -p buiy_core --test layout_gc`
Expected: pass.

Run: `xvfb-run -a cargo test --workspace`
Expected: all other tests still pass.

- [ ] **Step 5: Commit**

```
git add crates/buiy_core/src/layout.rs crates/buiy_core/tests/layout_gc.rs
git commit -m "feat(layout): GC LayoutTree on LayoutNode removal

Closes the Phase 0 TODO: LayoutTree no longer grows monotonically across
despawns. New gc_layout_tree system reads RemovedComponents<LayoutNode>
and drops the matching Taffy node. Spec § 3 Stage G."
```

---

### Task 15: Final verification

- [ ] **Step 1: Format**

Run: `cargo fmt --all -- --check`
Expected: success.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: success.

- [ ] **Step 3: Tests**

Run: `xvfb-run -a cargo test --workspace`
Expected: all green.

- [ ] **Step 4: Doc build**

Run: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
Expected: success. Fix any rustdoc warnings inline (broken intra-doc links to renamed types are most likely).

- [ ] **Step 5: Push**

Run:
```
git push
```

The branch (`claude/buiy-layout-design`) already tracks origin from the spec push, so a plain `git push` is enough.

- [ ] **Step 6: Open the PR (only when the human asks)**

Do NOT auto-open the PR. The plan is "ready for PR review" once Tasks 1–15 land; the human directs when to open it.

---

## Summary table

| Task | What | Behavior change | Test added |
|---|---|---|---|
| 1 | Rename `Node` → `LayoutNode` | None | n/a (mechanical) |
| 2 | `Length` enum + helpers | None | unit |
| 3 | Length → Taffy conversions | None | unit |
| 4 | `Edges<T>` + `Size2D<T>` | None | unit |
| 5 | `Display` component | None | unit |
| 6 | `Size` component | None | (deferred to Task 11) |
| 7 | `BoxSpace` component | None | unit |
| 8 | `FlexLayout` component, widen `FlexDirection` | None | unit |
| 9 | `VisualStyle` component, render reads it | Renderer reads new component | (covered by existing visual tests) |
| 10 | Layout queries decomposed components | Same output | (covered by Task 11 layout test) |
| 11 | Migrate Button + tests | Same output | updates existing |
| 12 | Reflection registration | None visible | reflection-registered test |
| 13 | Delete `Style` | None | n/a |
| 14 | `gc_layout_tree` system | Closes Phase 0 TODO | unit |
| 15 | Final verify + push | n/a | n/a |

After this plan: the codebase has the new component shape; the layout pipeline is still single-pass; visual fields ride on `VisualStyle` until later specs land. No public 0.1 promise yet, so no migration shim needed.

The next plan in the sequence widens `Length` (em/rem/vw/vh/cqw/cqh/token/calc) and decomposes the pipeline into Stages A–G.
