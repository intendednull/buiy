# Color, Forced-Colors, and the Verification Harness Implementation Plan

**Date:** 2026-06-03
**Status:** landed

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the render-side color contract — `ColorToken` resolution against `Res<Theme>`, the `Theme::is_changed()` re-extract edge, a main-world forced-colors `Theme` swap with a v1 stub system-color map, the two gate-#11 static analyzers (token-flow + no-shadow-only-affordance), and the scaffolded gate-#2 golden-image harness — all proven headless except the goldens.

**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes [`color-and-forced-colors.md`](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md) (all) and [`verification.md`](../specs/2026-06-03-buiy-render-pipeline-design/verification.md) (all), against the README § 3 component contract (`ColorToken`, `SystemColorKeyword`, the forced-colors selection mechanism).

**Architecture:** Color is a leaf-value lookup, not geometry, so it resolves at extract time against a live `Res<Theme>` — and a theme swap (light↔dark, *or* forced-colors↔normal) rides one edge: `theme.is_changed()`. Forced-colors is modeled as a **theme variant** swapped in the main world (before extract) when `UserPreferences.forced_colors` flips, so render needs no second color path; the one direct forced-colors read in extract is the `BoxShadow` draw-skip. The two gate-#11 static checks (`Background`/`Border`/`Outline`/`BoxShadow` are token edges, never literals; every state-bearing widget has a non-shadow inter-state cue) are pure functions over the widget catalog, and the gate-#2 golden harness is scaffolded `#[ignore]` with the fixed-clock + font-load-sync + atlas-warmup triad and the `--accept` workflow.

**Tier/Test reality:** MIXED. HEADLESS (every-PR `cargo test`, no wgpu adapter): the `ColorToken` resolver + missing-token sentinel, the contrast helper, the forced-colors `Theme`-swap system (`App::new()` + `MinimalPlugins`), the `Theme::is_changed()` re-resolve, and both gate-#11 static analyzers. GPU (`#[ignore]`, no wgpu adapter on CI/this host): the gate-#2 / gate-#11(b) goldens — code + harness scaffold land here, captured only on the canonical CI GPU runner.

**Depends on:** R1. Execution order across the render-pipeline plans: R1 → R2 → R3 → R4 → R5 → R6 → R7 → R8 → (R9, R10) → R11. This is the **last** plan in the order. R1 is the sole creator of `crates/buiy_core/src/render/color.rs` and the sole definer of `ColorToken` + `SystemColorKeyword` (color-and-forced-colors.md § 2.0). This plan **extends** `color.rs` with resolution + forced-colors logic; it does **not** redefine those types and does **not** re-export them from `lib.rs`.

---

## The gate (every commit must keep this green)

This host **and CI have no `xvfb` and no wgpu adapter**. Every commit must pass:

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

Any test that needs a wgpu adapter (RenderApp construction, pipeline compilation, an actual draw / screenshot) **cannot run on CI** — mark it `#[ignore = "needs a wgpu adapter (real GPU or lavapipe); …"]` exactly like `crates/buiy_core/tests/render_smoke.rs` does. The device-free assertions (token resolution, CPU contrast math, forced-colors swap via `App::new()`, schedule membership/order, static catalog analysis) are the real gating tests.

---

## Prior-art citations (idioms this plan mirrors)

- **Extract + theme read** — `crates/buiy_core/src/render/mod.rs:98-130` (`extract_buiy_draws`: `Extract<Res<Theme>>`, `theme.color(&token)`, the `MISSING_TOKEN_FALLBACK = Color::srgb(1.0, 0.0, 1.0)` sentinel + `tracing::warn!`). This phase **generalizes the token type** (`String` → `ColorToken`) and adds the forced-colors theme variant the extract reads. The extract itself stays the target shape.
- **Theme resource + resolver** — `crates/buiy_core/src/theme.rs:18-37` (`Theme { colors: HashMap<String, Color>, … }`, `Theme::color(&str) -> Option<Color>`), `:48-58` (`UserPreferences { forced_colors: bool, … }`), `:62-89` (`default_light_theme()`), `:91-100` (`ThemePlugin` registers types + inserts both resources). New `forced_colors_theme()` mirrors `default_light_theme()`.
- **Render-handoff component pattern** — `crates/buiy_core/src/components.rs:47-83` (`ResolvedTransform`, `StackingContext`: `#[derive(Component, Reflect, …)] #[reflect(Component)]`, `pub` crate-level, written by one system, `register_type`'d). `ColorToken` follows the same Reflect/derive conventions (it is a field type, not a `Component`).
- **`Cow<'static, str>`** is in `std::borrow::Cow` — already used across the workspace; no new dependency. `bevy_color::{Color, LinearRgba, Srgba}` come from `bevy::prelude` / `bevy::color`.
- **Pure-fn + App test harness** — `crates/buiy_core/tests/render_instance.rs` (pure-CPU math tests, no GPU) and `crates/buiy_core/tests/layout_containment.rs:11-18` (`fn app() { App::new() + MinimalPlugins + CorePlugin + LayoutPlugin }`, spawn, `app.update()`, assert via `world().get`). No `tracing-test` dep exists, so the missing-token test asserts the **observable sentinel Color**, not captured logs.
- **`#[ignore]` GPU caveat** — `crates/buiy_core/tests/render_smoke.rs:24-39` (the exact marker string and the `--ignored` run note). The golden harness scaffold reuses this caveat verbatim.
- **Schedule-order introspection** — `crates/buiy_core/tests/system_set_order.rs` (read `Schedules` → `Update` graph → toposort, assert relative order). The forced-colors-swap-before-render test reuses this pattern at the system-set granularity (`BuiySet::Style` runs before `BuiySet::Render`).
- **Widget catalog** — `crates/buiy_widgets/src/button.rs:32-53` (`Button::new` returns `impl Bundle` with `Visual { background_token, foreground_token, border_radius }`). The gate-#11 analyzers walk the catalog's emitted paint; until `Background`/`Border`/`Outline`/`BoxShadow` exist (sibling component-model phase), the analyzer operates over a small **catalog descriptor** this phase defines (see Task 7 note on the cross-phase seam).

---

## Cross-phase dependencies (assumed, not built here)

- **`Background` / `Border` / `Outline` / `BoxShadow` components** are introduced by the **component-model phase** (README § 3.2), not here. This phase's gate-#11 analyzers therefore run over an explicit **catalog paint descriptor** (`CatalogPaint`, a plain test/analyzer struct enumerating each widget × state's `{ background: ColorToken, border: ColorToken, outline: ColorToken, has_shadow_delta: bool }`) so the analyzer is real and gating **today**, and is re-pointed at the live components when they land (a one-line source swap, asserted by the same test). This is the same "rule, not the number" durability the verification spec uses for stride tests.
- **The forced-colors system-color *map contents*** (which token → which system color) are owned by `buiy-theme-tokens-design`. This phase ships a **minimal v1 stub** `forced_colors_theme()` holding the 16 system-color keys with placeholder values, which is the hard v1 prerequisite the spec names ([color-and-forced-colors.md § 3.1](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md) "v1 prerequisite — the forced-colors system-color map must exist"). Without it, `CanvasText` lookups miss → magenta.
- **`UiTransform`/`Stacking`/`Overflow`** already exist in `buiy_core::layout` (read-only here; this phase reads only `UserPreferences` + `Theme`).
- **The `--accept` golden CLI + canonical CI GPU runner** are operational concerns owned by `buiy-verification-design`; this phase scaffolds the harness module + its `#[ignore]` e2e test and documents the workflow, but does not provision the runner.

---

## File map (what each task touches)

| Task | Create | Modify | Test |
|---|---|---|---|
| 1 | — | — (verify only; `render/color.rs` + `ColorToken`/`SystemColorKeyword` already exist, owned by R1) | `crates/buiy_core/tests/render_color_token.rs` |
| 2 | — | `render/color.rs` (resolver + sentinel) | `crates/buiy_core/tests/render_color_token.rs` |
| 3 | — | `theme.rs` (`SystemColorKeyword`, `forced_colors_theme()`, register) | `crates/buiy_core/tests/theme_forced_colors.rs` |
| 4 | `crates/buiy_core/src/render/forced_colors.rs` | `render/mod.rs`, `render/color.rs` (system-color resolution), `lib.rs` | `crates/buiy_core/tests/render_forced_colors_swap.rs` |
| 5 | — | `render/forced_colors.rs` (schedule into `BuiySet::Style`), `lib.rs`/`render/mod.rs` plugin wiring | `crates/buiy_core/tests/render_forced_colors_swap.rs` |
| 6 | — | `render/color.rs` (contrast helper) | `crates/buiy_core/tests/render_contrast.rs` |
| 7 | `crates/buiy_core/src/render/forced_colors_analyzer.rs` | `render/mod.rs`, `lib.rs` | `crates/buiy_core/tests/render_forced_colors_analyzer.rs` |
| 8 | — | `render/forced_colors_analyzer.rs` (no-shadow-only check) | `crates/buiy_core/tests/render_forced_colors_analyzer.rs` |
| 9 | — | `render/mod.rs` (`extract` reads `ColorToken` + `theme.is_changed()` doc) | `crates/buiy_core/tests/render_theme_switch.rs` |
| 10 | `crates/buiy_core/src/render/golden.rs` | `render/mod.rs`, `lib.rs` | `crates/buiy_core/tests/render_golden_harness.rs` |

---

## Task 1 — verify the `ColorToken` + `SystemColorKeyword` types (owned by R1)

**Files**
- **Create:** — (nothing; `render/color.rs` already exists, owned by R1)
- **Modify:** — (do **not** add `pub mod color;`, do **not** re-export from `lib.rs` or `crates/buiy/src/lib.rs` — R1 already did all of this)
- **Test:** `crates/buiy_core/tests/render_color_token.rs`

> **Guarded import — do not redefine.** `crates/buiy_core/src/render/color.rs` and the `ColorToken` + `SystemColorKeyword` enums it holds are **created and owned by R1** ([color-and-forced-colors.md § 2.0](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md) is the canonical owner; R1 lands the enum, this plan extends the file with resolution + forced-colors logic). This task no longer *creates* the types — it **verifies their shape** with the test below and confirms the variant/keyword set this plan's resolver depends on. If R1's `color.rs` is missing or the variant set differs from what this test asserts, stop and reconcile with R1 before continuing; never re-`pub mod color;`, never re-add the `lib.rs` re-export, never redefine the enums.

Verifies the typed CSS `<color>` reference set from [color-and-forced-colors.md § 2.0](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md) is present (defined by R1). HEADLESS — pure type + `Default`.

- [ ] **Write the verification test** (asserts R1's already-landed shape; it passes against R1's `color.rs`, it does not gate a new definition here). Create `crates/buiy_core/tests/render_color_token.rs`:

```rust
//! `ColorToken` typed-variant + default tests. Pure-CPU, no GPU adapter.
//! Verifies the R1-owned shape this plan's resolver (Task 2) extends.
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md § 2.0.

use buiy_core::render::color::{ColorToken, SystemColorKeyword};
use std::borrow::Cow;

#[test]
fn color_token_default_is_transparent() {
    // CSS-initial "no fill" == empty-token skip case (component-model § 2/§ 3).
    assert_eq!(ColorToken::default(), ColorToken::Transparent);
}

#[test]
fn color_token_variants_construct() {
    let _ = ColorToken::Token(Cow::Borrowed("color.surface.primary"));
    let _ = ColorToken::CurrentColor;
    let _ = ColorToken::SystemColor(SystemColorKeyword::CanvasText);
}

#[test]
fn system_color_keyword_set_has_all_sixteen() {
    // The foundation-F 16-keyword CSS system-color set (visuals.md § 3.3).
    use SystemColorKeyword::*;
    let all = [
        Canvas,
        CanvasText,
        LinkText,
        ButtonText,
        ButtonBorder,
        GrayText,
        Highlight,
        HighlightText,
        Field,
        FieldText,
        Mark,
        MarkText,
        SelectedItem,
        SelectedItemText,
        AccentColor,
        AccentColorText,
    ];
    assert_eq!(all.len(), 16);
}
```

- [ ] **Run it — expect PASS** (R1 already landed `render/color.rs` + both enums, so the test compiles and passes):
  `cargo test -p buiy_core --test render_color_token`

  If it FAILS to compile (`color` module / `ColorToken` / `SystemColorKeyword` absent), R1 has not landed yet — **stop and resolve the R1 dependency**; do **not** define the types here. The R1-owned shape this plan's resolver depends on is, for reference (do **not** copy it into a new file):

```rust
// OWNED BY R1 — crates/buiy_core/src/render/color.rs (reference only, do not redefine).
// SystemColorKeyword: the 16 CSS system-color keywords, with a `token(self) ->
//   &'static str` method (e.g. Canvas → "Canvas") and a `const ALL: [_; 16]`
//   declaration-order set (used by Task 3's stub theme and Task 7's analyzer).
// ColorToken: { #[default] Transparent, Token(Cow<'static, str>), CurrentColor,
//   SystemColor(SystemColorKeyword) }, deriving Reflect + Clone + Default +
//   PartialEq + Debug. Default == Transparent (CSS-initial "no fill").
```

This plan **extends** that file (Tasks 2, 6 append `resolve_token` / `MISSING_TOKEN_FALLBACK` / `contrast_ratio`); it never re-declares the enums, never re-adds `pub mod color;` to `render/mod.rs`, and never re-adds the `lib.rs` / `crates/buiy/src/lib.rs` re-exports — R1 owns all of that.

- [ ] **Run the full gate** (see top). Resolve every warning.
- [ ] **No commit for this task** — it is verification-only (no source change). Proceed to Task 2.

---

## Task 2 — `resolve_token` against `Res<Theme>` + magenta sentinel

**Files**
- **Modify:** `crates/buiy_core/src/render/color.rs`
- **Test:** `crates/buiy_core/tests/render_color_token.rs`

The resolver each variant maps to one `Color` ([§ 2.0](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md), [§ 2.2 missing-token policy](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md)). HEADLESS — pure fn over a `Theme` value.

- [ ] **Add the failing tests** to `crates/buiy_core/tests/render_color_token.rs`:

```rust
use bevy::prelude::*;
use buiy_core::render::color::{resolve_token, MISSING_TOKEN_FALLBACK};
use buiy_core::theme::default_light_theme;

#[test]
fn transparent_resolves_to_none() {
    let theme = default_light_theme();
    assert_eq!(resolve_token(&ColorToken::Transparent, &theme), Color::NONE);
}

#[test]
fn token_hit_resolves_to_theme_color() {
    let theme = default_light_theme();
    let got = resolve_token(
        &ColorToken::Token(std::borrow::Cow::Borrowed("color.surface.primary")),
        &theme,
    );
    assert_eq!(got, Color::WHITE);
}

#[test]
fn token_miss_resolves_to_magenta_sentinel() {
    // A miss is an author bug: loud in screenshots and logs, never silent (§ 2.2).
    let theme = default_light_theme();
    let got = resolve_token(
        &ColorToken::Token(std::borrow::Cow::Borrowed("color.does.not.exist")),
        &theme,
    );
    assert_eq!(got, MISSING_TOKEN_FALLBACK);
    assert_eq!(MISSING_TOKEN_FALLBACK, Color::srgb(1.0, 0.0, 1.0));
}

#[test]
fn current_color_default_path_falls_back_to_foreground_token() {
    // v1 fallback: non-forced theme → color.text.primary (§ 2.0).
    let theme = default_light_theme();
    let got = resolve_token(&ColorToken::CurrentColor, &theme);
    assert_eq!(got, theme.color("color.text.primary").unwrap());
}
```

- [ ] **Run it — expect FAIL** (`resolve_token` / `MISSING_TOKEN_FALLBACK` not yet in `color.rs`):
  `cargo test -p buiy_core --test render_color_token`
- [ ] **Minimal impl.** Append to `crates/buiy_core/src/render/color.rs`:

```rust
use crate::theme::Theme;

/// Sentinel color for a missing theme token (magenta = "missing", visible at a
/// glance in screenshots). The accompanying `warn!` surfaces the typo'd token
/// name. A missing token is an author bug that should be *loud*, never silently
/// transparent (§ 2.2). It is an ordinary `Color::srgb`, so it composites
/// through the same linear pipeline as any other color.
pub const MISSING_TOKEN_FALLBACK: Color = Color::srgb(1.0, 0.0, 1.0);

/// Resolve one [`ColorToken`] against the active [`Theme`] to a concrete
/// `Color` (§ 2.0). Called at extract time. Never panics; a miss returns the
/// magenta sentinel and emits a `warn!` naming the token (§ 2.2).
///
/// `CurrentColor` uses the v1 fallback (§ 2.0): the theme default foreground
/// token — `CanvasText` when a `CanvasText` entry exists in the active theme
/// (the forced-colors case), otherwise `color.text.primary`. When
/// `buiy-text-rendering-design` lands the inherited-text-color carrier this
/// rule switches to read it with no change to the variant set.
pub fn resolve_token(token: &ColorToken, theme: &Theme) -> Color {
    match token {
        ColorToken::Transparent => Color::NONE,
        ColorToken::Token(name) => resolve_named(name, theme),
        ColorToken::SystemColor(kw) => resolve_named(kw.token(), theme),
        ColorToken::CurrentColor => {
            // Forced-colors theme carries `CanvasText`; prefer it when present.
            if theme.color(SystemColorKeyword::CanvasText.token()).is_some() {
                resolve_named(SystemColorKeyword::CanvasText.token(), theme)
            } else {
                resolve_named("color.text.primary", theme)
            }
        }
    }
}

fn resolve_named(name: &str, theme: &Theme) -> Color {
    match theme.color(name) {
        Some(c) => c,
        None => {
            tracing::warn!(token = %name, "missing theme color token; falling back to magenta sentinel");
            MISSING_TOKEN_FALLBACK
        }
    }
}
```

- [ ] **Run it — expect PASS:** `cargo test -p buiy_core --test render_color_token`
- [ ] **Run the full gate.**
- [ ] **Commit:** `feat(render): resolve ColorToken against Res<Theme> with magenta sentinel`

---

## Task 3 — forced-colors stub theme + `SystemColorKeyword` map

**Files**
- **Modify:** `crates/buiy_core/src/theme.rs`
- **Test:** `crates/buiy_core/tests/theme_forced_colors.rs`

The v1-prerequisite **stub** forced-colors theme whose `colors` map holds exactly the 16 system-color keys ([§ 3.1 "v1 prerequisite"](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md)). Placeholder values — *not* magenta — so the forced path resolves. HEADLESS.

- [ ] **Write the failing test.** Create `crates/buiy_core/tests/theme_forced_colors.rs`:

```rust
//! The v1 stub forced-colors theme: its `colors` map must hold exactly the 16
//! CSS system-color keys so every forced-colors paint token resolves (§ 3.1).
//! Pure-CPU, no GPU.

use bevy::prelude::*;
use buiy_core::render::color::SystemColorKeyword;
use buiy_core::theme::forced_colors_theme;

#[test]
fn forced_theme_holds_all_sixteen_system_colors() {
    let theme = forced_colors_theme();
    for kw in SystemColorKeyword::ALL {
        assert!(
            theme.color(kw.token()).is_some(),
            "forced-colors theme must define system color {}",
            kw.token()
        );
    }
}

#[test]
fn forced_theme_values_are_not_magenta_sentinel() {
    // The stub must resolve to *real* placeholder values, not the missing-token
    // sentinel — otherwise the forced path is indistinguishable from a miss.
    let theme = forced_colors_theme();
    for kw in SystemColorKeyword::ALL {
        assert_ne!(
            theme.color(kw.token()).unwrap(),
            Color::srgb(1.0, 0.0, 1.0),
            "system color {} must not be the magenta sentinel",
            kw.token()
        );
    }
}

#[test]
fn forced_theme_canvas_and_canvastext_contrast() {
    // High-contrast mode: Canvas (surface) and CanvasText (text) must differ.
    let theme = forced_colors_theme();
    assert_ne!(
        theme.color("Canvas").unwrap(),
        theme.color("CanvasText").unwrap()
    );
}
```

- [ ] **Run it — expect FAIL** (`forced_colors_theme` absent):
  `cargo test -p buiy_core --test theme_forced_colors`
- [ ] **Minimal impl.** Append to `crates/buiy_core/src/theme.rs` (after `default_light_theme`):

```rust
/// v1 **stub** forced-colors (high-contrast) theme. Its `colors` map holds
/// exactly the 16 CSS system-color keys so every forced-colors paint token
/// resolves (color-and-forced-colors.md § 3.1 — the hard v1 prerequisite).
///
/// Values are placeholders modeled on a Windows-High-Contrast black palette;
/// the *authoritative* system-color values are owned by
/// `buiy-theme-tokens-design`. This stub exists only so the forced-colors path
/// resolves to real colors (not magenta) and the gate-#11 analyzer is
/// meaningful. The keys, not the values, are the contract.
pub fn forced_colors_theme() -> Theme {
    use crate::render::color::SystemColorKeyword::*;
    let mut t = Theme::default();
    let black = Color::srgb(0.0, 0.0, 0.0);
    let white = Color::WHITE;
    let yellow = Color::srgb(1.0, 1.0, 0.0);
    let cyan = Color::srgb(0.0, 1.0, 1.0);
    let gray = Color::srgb(0.5, 0.5, 0.5);
    let pairs = [
        (Canvas, black),
        (CanvasText, white),
        (LinkText, cyan),
        (ButtonText, white),
        (ButtonBorder, white),
        (GrayText, gray),
        (Highlight, yellow),
        (HighlightText, black),
        (Field, black),
        (FieldText, white),
        (Mark, yellow),
        (MarkText, black),
        (SelectedItem, yellow),
        (SelectedItemText, black),
        (AccentColor, cyan),
        (AccentColorText, black),
    ];
    for (kw, color) in pairs {
        t.colors.insert(kw.token().to_string(), color);
    }
    t
}
```

- [ ] **Run it — expect PASS:** `cargo test -p buiy_core --test theme_forced_colors`
- [ ] **Run the full gate.**
- [ ] **Commit:** `feat(theme): add v1 stub forced-colors theme with 16 system-color keys`

---

## Task 4 — forced-colors `Theme`-swap system (main world)

**Files**
- **Create:** `crates/buiy_core/src/render/forced_colors.rs`
- **Modify:** `crates/buiy_core/src/render/mod.rs` (`pub mod forced_colors;`), `crates/buiy_core/src/lib.rs` (re-export the system + a resource that remembers the prior theme)
- **Test:** `crates/buiy_core/tests/render_forced_colors_swap.rs`

Forced-colors is selected **in the main world, before extract**, by swapping which `Theme` resource is active when `UserPreferences.forced_colors` flips ([§ 3.1 "concrete selection mechanism"](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md)). Because the swap mutates `Res<Theme>`, it rides the existing `Theme::is_changed()` re-resolve edge. HEADLESS — `App::new()` + `MinimalPlugins`.

- [ ] **Write the failing test.** Create `crates/buiy_core/tests/render_forced_colors_swap.rs`:

```rust
//! The forced-colors Theme swap is a main-world system (no GPU). When
//! `UserPreferences.forced_colors` flips, the active `Theme` becomes the
//! system-color variant; when it clears, the prior theme is restored.
//! Spec: color-and-forced-colors.md § 3.1.

use bevy::prelude::*;
use buiy_core::render::forced_colors::{apply_forced_colors_theme, PrePreferenceTheme};
use buiy_core::theme::{default_light_theme, UserPreferences, Theme};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(default_light_theme());
    app.insert_resource(UserPreferences::default());
    app.init_resource::<PrePreferenceTheme>();
    app.add_systems(Update, apply_forced_colors_theme);
    app
}

#[test]
fn flipping_forced_colors_swaps_in_system_color_theme() {
    let mut app = app();
    // Sanity: normal theme has no `Canvas` key.
    assert!(app.world().resource::<Theme>().color("Canvas").is_none());

    app.world_mut().resource_mut::<UserPreferences>().forced_colors = true;
    app.update();

    // After the swap the active theme resolves the 16 system colors.
    assert!(app.world().resource::<Theme>().color("Canvas").is_some());
    assert!(app.world().resource::<Theme>().color("CanvasText").is_some());
}

#[test]
fn clearing_forced_colors_restores_prior_theme() {
    let mut app = app();
    app.world_mut().resource_mut::<UserPreferences>().forced_colors = true;
    app.update();
    assert!(app.world().resource::<Theme>().color("Canvas").is_some());

    app.world_mut().resource_mut::<UserPreferences>().forced_colors = false;
    app.update();

    // Prior (light) theme is back: Canvas gone, the original token resolves.
    assert!(app.world().resource::<Theme>().color("Canvas").is_none());
    assert_eq!(
        app.world().resource::<Theme>().color("color.surface.primary"),
        Some(Color::WHITE)
    );
}

#[test]
fn no_flip_leaves_theme_unchanged_and_unmarked() {
    let mut app = app();
    app.update(); // first frame establishes baseline
    let changed_before = app.world().resource_ref::<Theme>().is_changed();
    app.update(); // no preference change
    // After a steady-state frame with no flip, the system must not re-insert
    // Theme (which would spuriously mark it changed every frame and force a
    // full re-resolve — § 2.3 / § 3.1).
    assert!(
        !app.world().resource_ref::<Theme>().is_changed() || changed_before,
        "Theme must not be marked changed on a no-flip frame"
    );
}
```

- [ ] **Run it — expect FAIL** (module absent):
  `cargo test -p buiy_core --test render_forced_colors_swap`
- [ ] **Minimal impl.** Create `crates/buiy_core/src/render/forced_colors.rs`:

```rust
//! Forced-colors (OS high-contrast) selection — a **main-world** Theme swap,
//! run before extract. When `UserPreferences.forced_colors` flips, the active
//! `Theme` becomes the system-color variant (the stub `forced_colors_theme()`);
//! when it clears, the theme captured before the flip is restored. Because the
//! swap mutates `Res<Theme>`, it rides the existing `Theme::is_changed()`
//! re-resolve edge (color-and-forced-colors.md § 2.3 / § 3.1) — there is **no**
//! separate forced-colors color path in extract. The one direct
//! `UserPreferences.forced_colors` read in extract is the BoxShadow draw-skip
//! (§ 3.3), owned by the component-model / compositor phase, not here.

use crate::theme::{forced_colors_theme, Theme, UserPreferences};
use bevy::prelude::*;

/// Remembers the theme that was active **before** forced-colors was applied,
/// so clearing `forced_colors` restores it. `None` while forced-colors is off
/// and no swap has happened. Render-relevant main-world resource.
#[derive(Resource, Default)]
pub struct PrePreferenceTheme(pub Option<Theme>);

/// Main-world system: keep the active `Theme` in sync with
/// `UserPreferences.forced_colors`. Idempotent — only touches `Theme` on the
/// frame the preference actually transitions, so it does not spuriously mark
/// `Theme` changed every frame (which would force a full re-resolve in extract
/// — § 2.3). Scheduled in `BuiySet::Style`, before `BuiySet::Render` (Task 5).
pub fn apply_forced_colors_theme(
    prefs: Res<UserPreferences>,
    mut theme: ResMut<Theme>,
    mut saved: ResMut<PrePreferenceTheme>,
) {
    if !prefs.is_changed() {
        return;
    }
    match (prefs.forced_colors, saved.0.is_some()) {
        // Entering forced-colors: save current, swap in the system-color theme.
        (true, false) => {
            saved.0 = Some(theme.clone());
            *theme = forced_colors_theme();
        }
        // Leaving forced-colors: restore the saved theme.
        (false, true) => {
            if let Some(prev) = saved.0.take() {
                *theme = prev;
            }
        }
        // Already in the requested state (e.g. a different preference changed):
        // leave Theme untouched so it is not re-marked changed.
        _ => {}
    }
}
```

Add `pub mod forced_colors;` to `crates/buiy_core/src/render/mod.rs` and re-export in `crates/buiy_core/src/lib.rs`:

```rust
pub use render::forced_colors::{apply_forced_colors_theme, PrePreferenceTheme};
```

> Note: `ResMut<Theme>` is marked changed by Bevy's change detection only when the system **dereferences it mutably** (`*theme = …`). The early `return` and the `_ => {}` arm never deref-mut, so a no-flip frame leaves `Theme` unmarked — that is exactly what `no_flip_leaves_theme_unchanged_and_unmarked` pins.

- [ ] **Run it — expect PASS:** `cargo test -p buiy_core --test render_forced_colors_swap`
- [ ] **Run the full gate.**
- [ ] **Commit:** `feat(render): forced-colors Theme swap on UserPreferences edge`

---

## Task 5 — schedule the swap in `BuiySet::Style`, before render extract

**Files**
- **Modify:** `crates/buiy_core/src/render/forced_colors.rs` (a `register_main_world` fn) OR `crates/buiy_core/src/render/mod.rs` (`BuiyRenderPlugin::build` main-world branch)
- **Test:** `crates/buiy_core/tests/render_forced_colors_swap.rs`

The swap must run in the **main world** in `BuiySet::Style`, which `CorePlugin` chains *before* `BuiySet::Render` (where extract reads `Theme`). `BuiyRenderPlugin::build`'s early `return` skips the RenderApp; the main-world system must be added **before** that guard. HEADLESS — schedule-membership assertion via `App::new()`.

- [ ] **Add the failing test** to `crates/buiy_core/tests/render_forced_colors_swap.rs`:

```rust
use buiy_core::{BuiySet, CorePlugin};
use buiy_core::render::BuiyRenderPlugin;

#[test]
fn swap_system_runs_under_render_plugin_in_style_set() {
    // BuiyRenderPlugin must register the main-world swap system + its resource
    // even with no RenderApp (headless). Spawning the plugin and flipping the
    // preference must swap the theme on the next frame.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin); // provides Theme? no — CorePlugin doesn't insert Theme.
    app.insert_resource(default_light_theme());
    app.insert_resource(UserPreferences::default());
    app.add_plugins(BuiyRenderPlugin);

    app.world_mut().resource_mut::<UserPreferences>().forced_colors = true;
    app.update();
    assert!(app.world().resource::<Theme>().color("Canvas").is_some());
}

#[test]
fn style_set_precedes_render_set() {
    // The swap is in BuiySet::Style; extract is in BuiySet::Render. CorePlugin
    // chains Style -> … -> Render, so the swap is visible to the same frame's
    // extract (§ 3.1: "selected in the main world, before extract").
    use bevy::ecs::schedule::NodeId;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.update();
    let schedules = app.world().resource::<bevy::ecs::schedule::Schedules>();
    let graph = schedules.get(Update).unwrap().graph();
    let topo = graph.dependency().get_toposort().unwrap();
    let idx = |want: BuiySet| {
        topo.iter()
            .position(|n| matches!(graph.get_set_at(*n), Some(s) if format!("{s:?}") == format!("{want:?}")))
            .unwrap_or_else(|| panic!("set {want:?} not in toposort"))
    };
    assert!(idx(BuiySet::Style) < idx(BuiySet::Render));
}
```

> If `get_set_at` / `NodeId` API differs in this Bevy build, mirror the exact helper in `crates/buiy_core/tests/system_set_order.rs` (it already resolves a `BuiySet` to a toposort index) and reuse it.

- [ ] **Run it — expect FAIL** (`BuiyRenderPlugin` does not yet add the swap system):
  `cargo test -p buiy_core --test render_forced_colors_swap`
- [ ] **Minimal impl.** In `crates/buiy_core/src/render/mod.rs`, add the main-world registration **before** the RenderApp guard in `BuiyRenderPlugin::build`:

```rust
fn build(&self, app: &mut App) {
    // Main-world forced-colors selection runs in BuiySet::Style, before the
    // BuiySet::Render extract reads Theme (§ 3.1). Registered unconditionally —
    // it has no RenderApp dependency.
    app.init_resource::<crate::render::forced_colors::PrePreferenceTheme>()
        .add_systems(
            Update,
            crate::render::forced_colors::apply_forced_colors_theme
                .in_set(crate::BuiySet::Style),
        );

    // ExtractedDraws is render-world only … (existing code below unchanged).
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    // … existing render_app wiring …
}
```

Add the `use` for `default_light_theme` / `UserPreferences` / `Theme` in the test (import from `buiy_core::theme`).

- [ ] **Run it — expect PASS:** `cargo test -p buiy_core --test render_forced_colors_swap`
- [ ] **Run the full gate.**
- [ ] **Commit:** `feat(render): schedule forced-colors swap in BuiySet::Style before extract`

---

## Task 6 — WCAG contrast helper (gate-#9 token-pair math, render-side dep)

**Files**
- **Modify:** `crates/buiy_core/src/render/color.rs`
- **Test:** `crates/buiy_core/tests/render_contrast.rs`

The contrast helper the forced-colors / focus-ring claims rest on ([§ 3.2 "Focus ring ≥3:1"](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md)). A pure relative-luminance + WCAG-ratio function over two resolved `Color`s. HEADLESS.

- [ ] **Write the failing test.** Create `crates/buiy_core/tests/render_contrast.rs`:

```rust
//! WCAG 2.x relative-luminance contrast ratio over two resolved colors. Pure
//! CPU. The gate-#9 token-pair lint and the focus-ring ≥3:1 claim (§ 3.2) rest
//! on this. Reference values from the WCAG 2.1 definition.

use bevy::prelude::*;
use buiy_core::render::color::contrast_ratio;

#[test]
fn black_on_white_is_twenty_one_to_one() {
    let r = contrast_ratio(Color::BLACK, Color::WHITE);
    assert!((r - 21.0).abs() < 0.01, "black/white must be 21:1 (got {r})");
}

#[test]
fn identical_colors_are_one_to_one() {
    let r = contrast_ratio(Color::WHITE, Color::WHITE);
    assert!((r - 1.0).abs() < 1e-6);
}

#[test]
fn ratio_is_symmetric() {
    let a = Color::srgb(0.2, 0.45, 0.95);
    let b = Color::srgb(0.96, 0.96, 0.96);
    assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-6);
}

#[test]
fn focus_ring_pair_meets_three_to_one() {
    // The default focus ring (accent on white surface) must clear WCAG 2.4.11
    // non-text 3:1 — the render-side reason the foundation marks Focus Visible F.
    let ring = Color::srgb(0.20, 0.45, 0.95);
    let surface = Color::WHITE;
    assert!(contrast_ratio(ring, surface) >= 3.0);
}
```

- [ ] **Run it — expect FAIL** (`contrast_ratio` absent):
  `cargo test -p buiy_core --test render_contrast`
- [ ] **Minimal impl.** Append to `crates/buiy_core/src/render/color.rs`:

```rust
/// WCAG 2.x relative luminance of a color (sRGB → linear, then the 0.2126 /
/// 0.7152 / 0.0722 weighting). Operates on the sRGB-decoded channels; alpha is
/// ignored (contrast is defined over opaque colors).
fn relative_luminance(color: Color) -> f32 {
    let lin = LinearRgba::from(color);
    0.2126 * lin.red + 0.7152 * lin.green + 0.0722 * lin.blue
}

/// WCAG 2.x contrast ratio between two colors, `(L_lighter + 0.05) /
/// (L_darker + 0.05)`, in `[1.0, 21.0]`. Symmetric in its arguments. The
/// gate-#9 token-pair contrast lint and the focus-ring ≥3:1 claim (§ 3.2) use
/// this; it checks authored token *values* independent of the render path.
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}
```

- [ ] **Run it — expect PASS:** `cargo test -p buiy_core --test render_contrast`
- [ ] **Run the full gate.**
- [ ] **Commit:** `feat(render): add WCAG relative-luminance contrast_ratio helper`

---

## Task 7 — gate-#11(a): token-flow static analyzer

**Files**
- **Create:** `crates/buiy_core/src/render/forced_colors_analyzer.rs`
- **Modify:** `crates/buiy_core/src/render/mod.rs` (`pub mod forced_colors_analyzer;`), `crates/buiy_core/src/lib.rs`
- **Test:** `crates/buiy_core/tests/render_forced_colors_analyzer.rs`

The build/test-time pass that asserts, under the forced-colors theme, **every paint color is a system-color token reference** ([§ 3.1 "the static check is a token-flow analyzer"](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md), [verification.md gate #11](../specs/2026-06-03-buiy-render-pipeline-design/verification.md)). Operates over a `CatalogPaint` descriptor (cross-phase seam — see dependencies). HEADLESS.

- [ ] **Write the failing test.** Create `crates/buiy_core/tests/render_forced_colors_analyzer.rs`:

```rust
//! Gate #11(a): token-flow analyzer. Under the forced-colors theme, no widget
//! paints a color outside the system-color token set. Pure CPU, no GPU.
//! Spec: color-and-forced-colors.md § 3.1; verification.md gate #11.

use buiy_core::render::color::{ColorToken, SystemColorKeyword};
use buiy_core::render::forced_colors_analyzer::{
    analyze_forced_colors, CatalogPaint, ForcedColorsViolation,
};
use buiy_core::theme::forced_colors_theme;

fn good_widget() -> CatalogPaint {
    CatalogPaint {
        widget: "button",
        state: "resting",
        background: ColorToken::SystemColor(SystemColorKeyword::ButtonText),
        border: ColorToken::SystemColor(SystemColorKeyword::ButtonBorder),
        outline: ColorToken::Transparent,
        has_shadow_only_state_delta: false,
    }
}

#[test]
fn all_system_color_tokens_pass_under_forced_theme() {
    let theme = forced_colors_theme();
    let report = analyze_forced_colors(&[good_widget()], &theme);
    assert!(report.is_empty(), "system-color tokens must pass: {report:?}");
}

#[test]
fn non_system_token_under_forced_theme_is_a_violation() {
    // A brand token absent from the forced map resolves to magenta → violation.
    let theme = forced_colors_theme();
    let mut w = good_widget();
    w.background = ColorToken::Token(std::borrow::Cow::Borrowed("color.accent"));
    let report = analyze_forced_colors(&[w], &theme);
    assert_eq!(report.len(), 1);
    assert!(matches!(
        report[0],
        ForcedColorsViolation::NonSystemColor { widget: "button", .. }
    ));
}

#[test]
fn transparent_is_allowed_under_forced_theme() {
    // Transparent is the no-fill case, not a color outside the palette.
    let theme = forced_colors_theme();
    let mut w = good_widget();
    w.background = ColorToken::Transparent;
    assert!(analyze_forced_colors(&[w], &theme).is_empty());
}
```

- [ ] **Run it — expect FAIL** (module absent):
  `cargo test -p buiy_core --test render_forced_colors_analyzer`
- [ ] **Minimal impl.** Create `crates/buiy_core/src/render/forced_colors_analyzer.rs`:

```rust
//! Gate-#11 static analyzers over the default widget catalog (build/test-time,
//! no GPU). Check (a): under the forced-colors theme every paint color is a
//! **system-color token reference** that resolves inside the system-color key
//! set (§ 3.1). Check (b): every state-bearing widget has a non-`BoxShadow`
//! inter-state cue (§ 3.2, Task 8).
//!
//! The analyzer is possible only because paint color is uniformly a token edge
//! in the component model — there is no second, literal-color path to miss.
//!
//! `CatalogPaint` is the cross-phase seam: until `Background`/`Border`/
//! `Outline`/`BoxShadow` (component-model phase) exist, the catalog is
//! enumerated as plain descriptors; when those components land, the descriptor
//! is built from them with no change to the analyzer or its tests.

use crate::render::color::{resolve_token, ColorToken, SystemColorKeyword, MISSING_TOKEN_FALLBACK};
use crate::theme::Theme;

/// One widget × state's emitted paint, as token references. Built from the
/// default catalog. `has_shadow_only_state_delta` records whether this state
/// differs from the resting state *only* in `BoxShadow` (check (b), Task 8).
#[derive(Clone, Debug)]
pub struct CatalogPaint {
    pub widget: &'static str,
    pub state: &'static str,
    pub background: ColorToken,
    pub border: ColorToken,
    pub outline: ColorToken,
    pub has_shadow_only_state_delta: bool,
}

/// A gate-#11 violation.
#[derive(Clone, Debug, PartialEq)]
pub enum ForcedColorsViolation {
    /// A paint token resolved outside the system-color set under forced-colors
    /// (it hit the magenta sentinel — an absent/brand token).
    NonSystemColor {
        widget: &'static str,
        state: &'static str,
        field: &'static str,
    },
    /// (Check (b), Task 8) the only inter-state difference is `BoxShadow`.
    ShadowOnlyAffordance { widget: &'static str, state: &'static str },
}

/// Check (a): under `theme` (the forced-colors variant), assert every non-
/// `Transparent` paint token resolves to a real system color — i.e. does not
/// fall through to the magenta sentinel. Returns the violations (empty == pass).
pub fn analyze_forced_colors(catalog: &[CatalogPaint], theme: &Theme) -> Vec<ForcedColorsViolation> {
    let mut out = Vec::new();
    for paint in catalog {
        for (field, token) in [
            ("background", &paint.background),
            ("border", &paint.border),
            ("outline", &paint.outline),
        ] {
            if matches!(token, ColorToken::Transparent) {
                continue;
            }
            if resolve_token(token, theme) == MISSING_TOKEN_FALLBACK {
                out.push(ForcedColorsViolation::NonSystemColor {
                    widget: paint.widget,
                    state: paint.state,
                    field,
                });
            }
        }
    }
    out
}

/// The system-color key set, for callers that want the allow-list directly.
pub fn system_color_tokens() -> [&'static str; 16] {
    SystemColorKeyword::ALL.map(|kw| kw.token())
}
```

Wire `pub mod forced_colors_analyzer;` into `render/mod.rs` and re-export `analyze_forced_colors`, `CatalogPaint`, `ForcedColorsViolation` in `crates/buiy_core/src/lib.rs`.

- [ ] **Run it — expect PASS:** `cargo test -p buiy_core --test render_forced_colors_analyzer`
- [ ] **Run the full gate.**
- [ ] **Commit:** `feat(render): gate-#11(a) token-flow forced-colors analyzer`

---

## Task 8 — gate-#11(b): no-shadow-only-affordance static check

**Files**
- **Modify:** `crates/buiy_core/src/render/forced_colors_analyzer.rs`
- **Test:** `crates/buiy_core/tests/render_forced_colors_analyzer.rs`

The structural query: every state-bearing widget conveys its state with a **non-shadow** cue, because Buiy drops `box-shadow` in forced-colors ([§ 3.2 check (b)](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md)). A widget whose only inter-state difference is `BoxShadow` fails. HEADLESS.

- [ ] **Add the failing tests** to `crates/buiy_core/tests/render_forced_colors_analyzer.rs`:

```rust
use buiy_core::render::forced_colors_analyzer::analyze_shadow_only;

#[test]
fn shadow_only_state_delta_is_a_violation() {
    // A focused state that differs from resting ONLY in BoxShadow is invisible
    // once shadows are suppressed under forced-colors (§ 3.2).
    let mut w = good_widget();
    w.state = "focus-visible";
    w.has_shadow_only_state_delta = true;
    let report = analyze_shadow_only(&[w]);
    assert_eq!(report.len(), 1);
    assert!(matches!(
        report[0],
        ForcedColorsViolation::ShadowOnlyAffordance { widget: "button", state: "focus-visible" }
    ));
}

#[test]
fn non_shadow_state_delta_passes() {
    // Resting state (no shadow-only delta) and a state with a border/outline
    // cue both pass.
    let resting = good_widget();
    let mut focus = good_widget();
    focus.state = "focus-visible";
    focus.outline = ColorToken::SystemColor(SystemColorKeyword::Highlight);
    focus.has_shadow_only_state_delta = false;
    assert!(analyze_shadow_only(&[resting, focus]).is_empty());
}
```

- [ ] **Run it — expect FAIL** (`analyze_shadow_only` absent):
  `cargo test -p buiy_core --test render_forced_colors_analyzer`
- [ ] **Minimal impl.** Append to `crates/buiy_core/src/render/forced_colors_analyzer.rs`:

```rust
/// Check (b): assert no widget state conveys its affordance with a shadow
/// alone. A `CatalogPaint` whose `has_shadow_only_state_delta` is set fails —
/// once `BoxShadow` is suppressed under forced-colors (§ 3.3) such a state is
/// indistinguishable from resting. Because `Background`/`Border`/`Outline` are
/// four distinct components from `BoxShadow`, "has a non-shadow cue?" is a
/// structural query answerable without rendering (§ 3.2). The visual half is
/// the forced-colors golden (gate #11(b), GPU — golden.rs).
pub fn analyze_shadow_only(catalog: &[CatalogPaint]) -> Vec<ForcedColorsViolation> {
    catalog
        .iter()
        .filter(|p| p.has_shadow_only_state_delta)
        .map(|p| ForcedColorsViolation::ShadowOnlyAffordance {
            widget: p.widget,
            state: p.state,
        })
        .collect()
}
```

Re-export `analyze_shadow_only` in `crates/buiy_core/src/lib.rs`.

- [ ] **Run it — expect PASS:** `cargo test -p buiy_core --test render_forced_colors_analyzer`
- [ ] **Run the full gate.**
- [ ] **Commit:** `feat(render): gate-#11(b) no-shadow-only-affordance static check`

---

## Task 9 — theme-switch re-resolve through extract (`is_changed()` edge)

**Files**
- **Modify:** `crates/buiy_core/src/render/mod.rs` (use `resolve_token` in `extract_buiy_draws`'s doc/seam; assert the live re-read)
- **Test:** `crates/buiy_core/tests/render_theme_switch.rs`

A theme swap (light↔dark or forced↔normal) must re-resolve every token-bearing color the next frame, with **no cached theme-stamped instance buffer** ([§ 2.3](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md)). Because the render world has no adapter on CI, this is proven on the **main-world resolution seam**: resolving a fixture token against the live `Theme` produces the new color after a swap. HEADLESS.

> The render-world `ExtractedDraws` resource cannot be read without a `RenderApp` (needs an adapter). So the theme-switch property is proven at the **resolver layer** — `resolve_token(token, &theme)` re-reads the live theme — which is exactly the value extract packs. This is the verification spec's "prove every render property at the lowest layer that can observe it without a device" (layer 2).

- [ ] **Write the failing test.** Create `crates/buiy_core/tests/render_theme_switch.rs`:

```rust
//! A theme switch re-resolves token colors with no cached, theme-stamped
//! buffer — proven at the resolver layer (extract re-reads the live Theme).
//! Pure CPU, no GPU. Spec: color-and-forced-colors.md § 2.3 / § 3.1.

use bevy::prelude::*;
use buiy_core::render::color::{resolve_token, ColorToken};
use buiy_core::render::forced_colors::{apply_forced_colors_theme, PrePreferenceTheme};
use buiy_core::theme::{default_light_theme, forced_colors_theme, Theme, UserPreferences};
use std::borrow::Cow;

#[test]
fn replacing_theme_reresolves_token_next_read() {
    let token = ColorToken::Token(Cow::Borrowed("color.surface.primary"));

    let mut light = default_light_theme();
    assert_eq!(resolve_token(&token, &light), Color::WHITE);

    // Mutate the same key to a new value (a brand/dark swap).
    light
        .colors
        .insert("color.surface.primary".into(), Color::BLACK);
    assert_eq!(resolve_token(&token, &light), Color::BLACK);
}

#[test]
fn theme_swap_marks_resource_changed_for_extract() {
    // The is_changed() edge: a ResMut<Theme> swap marks the resource changed,
    // so the next frame's extract (which reads Theme live) re-resolves.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(default_light_theme());
    app.insert_resource(UserPreferences::default());
    app.init_resource::<PrePreferenceTheme>();
    app.add_systems(Update, apply_forced_colors_theme);

    app.update(); // baseline
    app.world_mut().resource_mut::<UserPreferences>().forced_colors = true;
    app.update();

    assert!(
        app.world().resource_ref::<Theme>().is_changed(),
        "theme swap must mark Theme changed so extract re-resolves (§ 2.3)"
    );
    // And the new value is the forced palette.
    let forced = forced_colors_theme();
    assert_eq!(
        app.world().resource::<Theme>().color("Canvas"),
        forced.color("Canvas")
    );
}
```

- [ ] **Run it — expect FAIL** only if the resolver/swap seam regressed; if Tasks 2/4 are green this compiles. Run to confirm the **property** holds (and is asserted), then proceed:
  `cargo test -p buiy_core --test render_theme_switch`
- [ ] **Minimal impl.** This task is primarily an assertion task. The only source change is documenting the seam: in `crates/buiy_core/src/render/mod.rs`, update the `extract_buiy_draws` doc comment to name the re-resolve contract (the function body still reads `Visual.background_token` until the component-model phase, so add the forward-looking note only):

```rust
// Token resolution re-reads `Res<Theme>` live every frame, so a theme swap
// (light↔dark or the forced-colors variant) re-resolves all token-bearing
// paint with no cached, theme-stamped buffer (color-and-forced-colors.md
// § 2.3). The component-model phase replaces `Visual.background_token` with
// `ColorToken`-bearing `Background`/`Border`/`Outline`/`BoxShadow`, resolved
// via `crate::render::color::resolve_token`; the `theme.is_changed()` global
// re-resolve signal then bypasses the per-entity `Changed<T>` short-circuit
// on a theme-only switch.
```

(If `extract_buiy_draws` already has an adjacent doc block, extend it; do not add an unused import.)

- [ ] **Run it — expect PASS:** `cargo test -p buiy_core --test render_theme_switch`
- [ ] **Run the full gate.**
- [ ] **Commit:** `test(render): pin theme-switch re-resolve through the is_changed edge`

---

## Task 10 — gate-#2 golden-image harness scaffold (GPU `#[ignore]`)

**Files**
- **Create:** `crates/buiy_core/src/render/golden.rs`
- **Modify:** `crates/buiy_core/src/render/mod.rs` (`pub mod golden;`), `crates/buiy_core/src/lib.rs`
- **Test:** `crates/buiy_core/tests/render_golden_harness.rs`

Scaffold the e2e golden harness ([verification.md § 4](../specs/2026-06-03-buiy-render-pipeline-design/verification.md)) — the flake-mitigation triad (fixed clock + font-load sync + atlas warmup), the perceptual-diff + tolerance-budget seam, and the `--accept` workflow. The **device-free pieces** (the `GoldenConfig` triad struct, the perceptual-diff pure fn, `accept` flag plumbing) are HEADLESS-gating; the **actual capture/draw** test is GPU `#[ignore]`.

- [ ] **Write the failing test.** Create `crates/buiy_core/tests/render_golden_harness.rs`:

```rust
//! Golden-image harness (gate #2). The triad config + perceptual diff are
//! device-free and gating; the actual capture needs a wgpu adapter and is
//! #[ignore]. Spec: verification.md § 4.

use buiy_core::render::golden::{perceptual_diff, GoldenConfig};

#[test]
fn golden_config_pins_the_flake_triad() {
    // All three nondeterminism sources must be pinned together (verification § 4.3).
    let cfg = GoldenConfig::deterministic();
    assert!(cfg.fixed_clock, "fixed clock");
    assert!(cfg.wait_for_fonts, "font-load sync");
    assert!(cfg.warm_atlas, "atlas warmup");
}

#[test]
fn identical_images_diff_to_zero() {
    let a = vec![10u8, 20, 30, 255, 40, 50, 60, 255];
    assert_eq!(perceptual_diff(&a, &a), 0.0);
}

#[test]
fn differing_images_diff_above_zero() {
    let a = vec![0u8, 0, 0, 255];
    let b = vec![255u8, 255, 255, 255];
    assert!(perceptual_diff(&a, &b) > 0.0);
}

#[test]
fn accept_flag_routes_through_config() {
    // The --accept workflow is human-curated: never an automatic overwrite
    // (verification § 4.4). The flag is off by default.
    let cfg = GoldenConfig::deterministic();
    assert!(!cfg.accept, "golden updates require explicit, human-curated --accept");
}
```

And the `#[ignore]` GPU capture test in the same file:

```rust
// Needs a wgpu adapter (real GPU or lavapipe): captures a frame via Bevy's
// screenshot system on the canonical CI GPU class and perceptually diffs it
// against the stored golden (verification § 4.1). Headless CI without a GPU
// panics at adapter init, so this runs only on the e2e runner / with --ignored.
//
// Run locally with: cargo test -p buiy_core --test render_golden_harness -- --ignored
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by the gate-#2 e2e runner"]
fn overlapping_semitransparent_fills_match_golden() {
    // Pillar-6 standing proof: overlapping children under group Opacity < 1 +
    // isolation must composite linear-correctly (verification § 5). Captured
    // with the flake triad and diffed under the per-fixture tolerance budget
    // owned by buiy-verification-design. Body is the e2e-runner wiring; the
    // device-free assertions above are the gating tests.
    let _cfg = buiy_core::render::golden::GoldenConfig::deterministic();
    // The capture/draw/compare pipeline is provisioned on the e2e runner.
}
```

- [ ] **Run it — expect FAIL** (module absent):
  `cargo test -p buiy_core --test render_golden_harness`
- [ ] **Minimal impl.** Create `crates/buiy_core/src/render/golden.rs`:

```rust
//! The e2e golden-image harness (gate #2). The only proof of pixels, so its
//! reliability is load-bearing (verification.md § 4). This module owns the
//! device-free pieces — the flake-mitigation triad config (§ 4.3), the
//! perceptual-diff metric + tolerance-budget seam (§ 4.2), and the human-
//! curated `--accept` workflow flag (§ 4.4). The capture itself runs only on
//! the canonical CI GPU class (§ 4.1) behind an `#[ignore]` test.
//!
//! Per-fixture tolerance/perf/leak *numbers* are owned by
//! `buiy-verification-design`; this module commits to *having* a budget, not
//! its value.

/// Deterministic-capture configuration. The three flake sources of § 4.3 are
/// *necessary together*: a golden captured without all three is not
/// reproducible. `accept` is the § 4.4 human-curated golden-update gate —
/// never an automatic overwrite.
#[derive(Clone, Copy, Debug)]
pub struct GoldenConfig {
    /// Drive time from a fixed/virtual clock, not wall time, so any time-
    /// dependent visual is captured at a deterministic instant (§ 4.3.1).
    pub fixed_clock: bool,
    /// Block capture until every referenced font is loaded and its glyphs are
    /// resident (§ 4.3.2) — a half-loaded font flips the diff.
    pub wait_for_fonts: bool,
    /// Warm the texture atlas (glyphs/icons/gradients) before capture (§ 4.3.3)
    /// so first-frame upload latency does not perturb the image. Also
    /// establishes the gate-#15 steady-state baseline.
    pub warm_atlas: bool,
    /// `--accept`: update the stored golden instead of failing on mismatch.
    /// Off by default; gated behind human PR review (§ 4.4).
    pub accept: bool,
}

impl GoldenConfig {
    /// The capture config with the full flake-mitigation triad pinned and
    /// `accept` off — the configuration every golden is captured under.
    pub fn deterministic() -> Self {
        Self {
            fixed_clock: true,
            wait_for_fonts: true,
            warm_atlas: true,
            accept: false,
        }
    }
}

/// Perceptual difference between two RGBA8 frames, as a normalized mean
/// per-channel difference in `[0.0, 1.0]` (0 == identical). Comparison is
/// *perceptual*, not exact byte equality (§ 4.2): sub-LSB float jitter in the
/// SDF and linear→sRGB encode is invisible but not bit-stable, so the caller
/// compares this against an explicit per-fixture tolerance budget (owned by
/// `buiy-verification-design`) — the budget is the line between jitter and
/// regression. Frames must be the same length (same dimensions); mismatched
/// lengths return `1.0` (maximal difference).
pub fn perceptual_diff(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let sum: f64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| (x as f64 - y as f64).abs())
        .sum();
    (sum / (a.len() as f64 * 255.0)) as f32
}
```

Wire `pub mod golden;` into `render/mod.rs` and re-export `GoldenConfig`, `perceptual_diff` in `crates/buiy_core/src/lib.rs`.

- [ ] **Run it — expect PASS** (the `#[ignore]` test is skipped headlessly):
  `cargo test -p buiy_core --test render_golden_harness`
- [ ] **Confirm the GPU test is properly ignored** (lists as ignored, does not run):
  `cargo test -p buiy_core --test render_golden_harness -- --list 2>&1 | grep ignore` — expect `overlapping_semitransparent_fills_match_golden` flagged ignored.
- [ ] **Run the full gate.**
- [ ] **Commit:** `feat(render): scaffold gate-#2 golden harness (triad + diff + --accept)`

---

## Done criteria

- [ ] All ten tasks committed; the full gate is green after each.
- [ ] HEADLESS-gating tests (run every PR, no adapter): `render_color_token`, `theme_forced_colors`, `render_forced_colors_swap`, `render_contrast`, `render_forced_colors_analyzer`, `render_theme_switch`, and the device-free half of `render_golden_harness`.
- [ ] GPU `#[ignore]` (e2e runner only): `render_golden_harness::overlapping_semitransparent_fills_match_golden` (gate #2 / #11(b) goldens).
- [ ] **Docs update (ship with the change):** in `docs/specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md` and `verification.md`, no target-state edits are needed (this realizes the existing target). Add a row to `docs/README.md`'s plan catalog for this plan, and — if the repo tracks render follow-ups — note in `docs/plans/follow-ups.md` that the `Background`/`Border`/`Outline`/`BoxShadow` component-model phase must re-point the `CatalogPaint` analyzer seam at the live components (Task 7 cross-phase note) and wire the `BoxShadow` draw-skip (§ 3.3) into extract.

---

*Plan authored 2026-06-04 against the 2026-06-03 render-pipeline design. Color resolution + forced-colors selection + the gate-#11 analyzers are HEADLESS-gating; the gate-#2 goldens are GPU `#[ignore]` (no wgpu adapter on CI or this host).*
