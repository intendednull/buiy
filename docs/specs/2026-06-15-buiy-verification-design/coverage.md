# Coverage-by-construction

**Date:** 2026-06-15
**Status:** landed (Phase 4; `crates/buiy_verify/src/coverage/`; see § Landed)
**Spec:** specs/2026-06-15-buiy-verification-design/README.md

Stop hand-writing per-widget tests; derive them from the BSN/widget catalog. This
section specifies `buiy_verify::coverage`: a `Matrix` of global axes
(themes × viewports × forced_colors × dprs), a Cartesian product taken at
test-collection time over a fixture directory, so that adding **one** widget
fixture auto-enrolls it across **every** tier (layout snapshot, display-list
snapshot, invariant scenes, reftest pairings, golden corpus). It also re-points
`forced_colors_analyzer` from hand-built `CatalogPaint` at the live widget catalog
(follow-ups.md:462–481), making gate #11 fall out of the same enrollment.

## Contract deviations

None. This module is additive over the contract's `coverage` slot. One
clarification flagged for the synthesizer: the contract lists axes as
`{themes, viewports, forced_colors, dprs}`; this spec models `forced_colors`
and `dpr` as the two **`Mode`** axes (Chromatic-style, each cell gets its own
baseline) and `theme`/`viewport` as ordinary axes — a presentation grouping, not
a type change. The Cartesian product is over all four.

## The fixture as single source of truth

A **fixture** is a BSN scene factory plus a name — the catalog row, authored once.
It is the same `fn(&mut App)` shape every other tier consumes (`reftest::RefCase`,
`golden`, `snapshot`), so a fixture is enrollable everywhere with no adapter.

```rust
// crates/buiy_verify/src/coverage/fixture.rs
pub struct Fixture {
    /// Stable identity. Becomes the `widget` key component and the insta
    /// snapshot stem. `lower-kebab`, unique within the corpus.
    pub name: &'static str,
    /// Spawns the scene into a deterministic app. MUST spawn a `Camera2d`,
    /// MUST tag the widget root with a `Name` (entities are keyed by Name in
    /// every dump — never by `Entity` bits; snapshot.md). One fixture = one
    /// widget × state (the `state` key component is carried separately, below).
    pub state: &'static str,
    pub spawn: fn(&mut App),
}

/// The corpus: every fixture, collected once. `inventory`-registered so a new
/// fixture file enrolls with zero edits to a central list (see Enrollment).
pub fn catalog() -> &'static [Fixture];
```

Fixtures live under `crates/buiy_verify/fixtures/<widget>/<state>.rs` and register
via the `fixture!` macro (below). The BSN factory is the same code the
`hello_button` / `hello_text` examples already use (`examples/hello_button/src/main.rs`
spawns `Button::new("Save")`); the catalog is those spawns, named and enumerated.

```rust
fixture! {
    name  = "button",
    state = "resting",
    spawn = |app| { app.world_mut().spawn((Name::new("button"), Button::new("Save"))); },
}
```

The `state` axis (resting / hover / focus / pressed / disabled) is **per-fixture**,
not a global `Matrix` axis, because state is widget-specific (a `Button` has
`hover`; a static `Label` does not). It is encoded by spawning the widget already
in that state (e.g. inserting `Hovered`, `Focusable { focused: true }`, the
`Disabled` marker — all live components). One file per state keeps each fixture a
single scene.

## The Matrix — global axes, Cartesian product

```rust
// crates/buiy_verify/src/coverage/matrix.rs
use buiy_core::render::golden::Dpr;   // canonical Dpr (determinism.md); NOT a local f32
pub struct Matrix {
    pub themes:        Vec<ThemeAxis>,    // light, forced_colors (dark when it lands)
    pub viewports:     Vec<Viewport>,    // logical (w,h): phone, tablet, desktop
    pub forced_colors: Vec<bool>,        // Mode axis: false, true
    pub dprs:          Vec<Dpr>,         // Mode axis: Dpr::X1, Dpr::X2 (milliscale)
}

#[derive(Clone, Copy)]
pub enum ThemeAxis { Light, ForcedColors }  // -> theme.rs constructors
impl ThemeAxis {
    pub fn build(self) -> Theme {            // default_light_theme / forced_colors_theme
        match self { Self::Light => default_light_theme(), Self::ForcedColors => forced_colors_theme() }
    }
    pub fn key(self) -> &'static str { /* "light" | "forced" */ }
}

#[derive(Clone, Copy)]
pub struct Viewport { pub w: u32, pub h: u32, pub key: &'static str }

impl Matrix {
    /// The CI default. Conservative product; widen per axis with a documented
    /// reason, never silently (mirrors the metric's fuzz-budget discipline).
    pub fn ci_default() -> Self;
    /// Cartesian product → one `Cell` per combination. Stable iteration order
    /// (axis declaration order) so snapshot stems are deterministic.
    pub fn cells(&self) -> impl Iterator<Item = Cell>;
}

/// One enrolled combination. The product `Matrix × Fixture` is the full corpus.
#[derive(Clone, Copy)]
pub struct Cell {
    pub theme:         ThemeAxis,
    pub viewport:      Viewport,
    pub forced_colors: bool,
    pub dpr:           Dpr,              // canonical buiy_core::render::golden::Dpr
}
```

A `Cell` is **not** itself a key — it is half of one. The full key is
`Cell × Fixture`, which is exactly the contract's storage schema and Skia Gold's
params/traces identity (skia-gold/lessons.md §Borrow.2,
`(widget, state, theme, viewport, backend, dpr)`):

```rust
// crates/buiy_verify/src/coverage/key.rs — the shared key for golden + snapshot stems
use buiy_core::render::golden::Dpr;   // canonical Dpr (determinism.md)

/// `dpr: Dpr` (milliscale, `Eq + Hash`) lets `CoverageKey` itself derive
/// `Eq + Hash` — so the `verify_keys_unique` self-test can collect keys into a
/// `HashSet` directly. The old `dpr: f32` made this impossible (`f32` is neither
/// `Eq` nor `Hash`), which is the bug that fix unblocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoverageKey {
    pub widget:        &'static str,  // Fixture::name
    pub state:         &'static str,  // Fixture::state
    pub theme:         &'static str,  // ThemeAxis::key
    pub viewport:      &'static str,  // Viewport::key
    pub forced_colors: bool,
    pub dpr:           Dpr,           // canonical buiy_core::render::golden::Dpr (Eq+Hash)
    pub backend:       Backend,       // golden.rs Backend; "cpu" for snapshot tiers
}
impl CoverageKey {
    pub fn for_cell(fx: &Fixture, cell: &Cell, backend: Backend) -> Self;
    /// Canonical filename stem, e.g. `button.resting.forced.desktop.fc1.dpr2.lavapipe`.
    /// Stable, lossless, ordered — retrofitting keys means re-baselining
    /// everything (skia-gold/lessons.md §Avoid). Used as the golden PNG stem
    /// and the insta snapshot suffix (`assert_snapshot!(key.stem(), …)`).
    pub fn stem(&self) -> String;
}
```

`backend` enumerates `cpu` (Tiers 1–3, no GPU) and the GPU rasterizer name
(`lavapipe` in CI, `radv` locally); reserving it now avoids the painful retrofit
(skia-gold/lessons.md §Avoid: "retrofitting a single-golden assumption is painful").

## Enrollment — one body per tier, applied across the product

Enrollment is the verb: each tier provides **one** generic body; the harness drives
it across `catalog() × Matrix::cells()`. No per-widget test code exists anywhere.

```rust
// crates/buiy_verify/src/coverage/enroll.rs
/// Build a deterministic app for (fixture, cell): DeterministicApp builder
/// (determinism.md) with theme installed, viewport + DPR pinned, forced_colors
/// set on UserPreferences, then the fixture spawned. The `Dpr`→`f32` conversion
/// happens HERE at the capture boundary: `DeterministicApp::dpr(cell.dpr)` feeds
/// the pinned scale_factor as `cell.dpr.as_f32()` (the milliscale axis stays the
/// key; the window's `scale_factor` is the derived f32).
pub fn build_app(fx: &Fixture, cell: &Cell) -> App;

/// Drive a tier body over the entire corpus. `body` receives the built app and
/// the key; it does the tier-specific assert (snapshot / invariant / golden).
pub fn enroll_all(matrix: &Matrix, body: impl Fn(App, CoverageKey));
```

Each tier is a thin caller of `enroll_all`. The `insta` snapshot tiers use
`glob!` over the fixture directory as the contract requires — `glob!` is the
collection-time fan-out, and `enroll_all` multiplies each globbed fixture by the
`Matrix` cells:

```rust
// crates/buiy_verify/tests/coverage_layout.rs   (Tier 1, gate #5, pure CPU)
#[test]
fn layout_snapshots() {
    enroll_all(&Matrix::ci_default(), |app, key| {
        // snapshot.md::assert_layout_snapshot — ResolvedLayout dump, keyed
        assert_layout_snapshot(&key.stem(), &app);
    });
}

// crates/buiy_verify/tests/coverage_display_list.rs  (Tier 2, pure CPU)
//   body -> assert_display_list_snapshot(&key.stem(), &app)   (snapshot.md)
// crates/buiy_verify/tests/coverage_invariants.rs    (Tier 3, pure CPU)
//   body -> for each invariant fn: assert on the built scene  (invariant.md)
// crates/buiy_verify/tests/coverage_golden.rs        (Tier 5, #[ignore], GPU)
//   body -> let img = capture_to_image(&mut app, &cfg);
//           assert_golden(&key.stem(), &img, &budget_for(&key))   (golden.rs)
```

The decisive property: **adding `fixtures/slider/resting.rs` enrolls a slider into
all five tiers at once** — layout snapshot, display-list snapshot, every Tier-3
invariant, and (once a budget is set) a golden cell per `Matrix` combination — with
no edit to any test file. This is Chromatic's "modes multiply, each cell gets its
own baseline" made native to BSN (report §Cross-cutting → Coverage auto-generation).

### The `glob!` ↔ `inventory` choice

`insta::glob!` discovers fixtures by walking a directory of `.rs` (or `.ron` BSN)
files; `inventory` discovers them by link-time registration. The spec uses **both,
non-redundantly**: `inventory` builds `catalog()` (the typed `&[Fixture]` that the
GPU/invariant tiers iterate, since they are not file-driven), and `glob!` drives
the two `insta` snapshot tiers (its `cargo insta review` UX is the required accept
loop). The `fixture!` macro emits *both* an `inventory::submit!` and a
discoverable file, so the two views never drift. A `verify_catalog_matches_glob`
self-test (below) asserts they enumerate the identical set.

## Wiring `forced_colors_analyzer` to the live catalog

Today the gate-#11 analyzers consume hand-built `CatalogPaint` descriptors
(`render/forced_colors_analyzer.rs:21`; tests construct them literally,
`tests/render_forced_colors_analyzer.rs:11`). The seam is documented for
re-pointing once real painted components land (follow-ups.md:469–473) — and they
have: `Button::bundle` now spawns live `Background`/`Border`/`Corners`/`Radius`
(`buiy_widgets/src/button.rs:18,47`). The wiring is a **producer** that derives
`CatalogPaint` from the live catalog, leaving the analyzer and its tests unchanged
(the seam's stated contract — `forced_colors_analyzer.rs:10`):

```rust
// crates/buiy_verify/src/coverage/forced_colors.rs
/// Walk the live catalog: for each fixture, build its app, query the spawned
/// `Background`/`Border`/`Outline` (+ shadow-only-delta) components off the
/// `Name`-tagged root, and project them into the existing `CatalogPaint`.
/// The analyzer (`analyze_forced_colors` / `analyze_shadow_only`) is called
/// unchanged — only its *input source* moves from fixtures to the live tree.
pub fn live_catalog_paint() -> Vec<CatalogPaint>;
```

```rust
// crates/buiy_verify/tests/coverage_forced_colors.rs   (Tier 2, gate #11, pure CPU)
#[test]
fn live_catalog_has_no_forced_colors_violations() {
    let catalog = live_catalog_paint();              // from the SAME fixtures
    let theme = forced_colors_theme();
    assert!(analyze_forced_colors(&catalog, &theme).is_empty());
    assert!(analyze_shadow_only(&catalog).is_empty());
}
```

Because `live_catalog_paint` reads the *same* fixture corpus as every other tier,
gate #11 auto-enrolls every new widget by construction — the report's stated goal
("wiring the existing `forced_colors_analyzer` to the live catalog makes #11 fall
out of the same enrollment", report §Cross-cutting). The residual forced-colors
*visual* half (the `BoxShadow` draw-skip, follow-ups.md:474–478) is a reftest, not
coverage's concern (reftests.md); coverage only enrolls the forced-colors **mode**
(`forced_colors: true` cell) into every tier so the visual reftest is itself
matrixed. **That visual reftest is BLOCKED on the unlanded `BoxShadow`
extract/draw path** (`extract_buiy_nodes` has no `BoxShadow` branch yet — reftests.md
§ authoring patterns); the structured `analyze_forced_colors` / `analyze_shadow_only`
gate here covers the rest now and does not depend on it.

## Storage at scale — staged, designed now

Per skia-gold/lessons.md, the corpus is in-repo PNGs (golden.md owns persistence)
until scale hurts; coverage's job is to **make migration mechanical** by fixing the
`CoverageKey` schema now (skia-gold/lessons.md §Borrow.2: "fix the schema before
generating any goldens"). The matrix is the natural place to enforce
*combinatorial budget*: `Matrix::ci_default` is deliberately small (≈ 2 themes × 3
viewports × 2 fc × 2 dpr = 24 cells/fixture), and a `cell_count()` assertion in the
self-test fails the build if the product exceeds a named ceiling — a planned
storage-migration trigger (report Open Q #6), not a surprise. Multi-positive
baselines and pruning are golden.md's concern; coverage only guarantees the key is
set-valued-ready (the stem is the key, one PNG per accepted digest).

## Dependencies

| Crate | Version | Status | `cargo deny` note |
|---|---|---|---|
| `insta` | `1` (workspace) | **new** (added by snapshot.md) | reuse; permissive (Apache-2.0/MIT). `glob!` feature already needed by snapshot tier. |
| `inventory` | `0.3` | **new** | distributed link-time registration for `catalog()`. MIT/Apache-2.0 — clears `cargo deny check`. Re-verify advisories before bump. Alternative considered: a hand-maintained `&[Fixture]` const (rejected — defeats "zero edits to enroll"). |

No GPU-only or copyleft deps. `image`/`proptest` are already workspace deps and
consumed via the metric/invariant tiers, not added here.

## Verification — testing the harness itself

The coverage layer is meta-machinery, so it is tested by asserting its
*enumeration and keying*, independent of any tier's pass/fail:

1. **`verify_catalog_matches_glob`** — `catalog()` (inventory) and the `glob!`
   fixture-directory walk enumerate the identical `name×state` set. Guards the
   dual-source-of-truth drift named above.
2. **`verify_keys_unique`** — over `catalog() × Matrix::ci_default().cells()`,
   every `CoverageKey::stem()` is unique and round-trips (parse-back ≡ identity).
   A collision means two cells would share a baseline — the silent-overwrite bug.
   `CoverageKey` now derives `Eq + Hash` (because `dpr: Dpr` is `Eq + Hash`, not
   the old `f32`), so the keys themselves — not just their stems — collect into a
   `HashSet` for the duplicate check.
3. **`verify_cell_count_under_ceiling`** — the product size is below the named CI
   ceiling; tripping it forces an explicit budget decision (storage-migration
   trigger, report Open Q #6).
4. **A deliberately-broken fixture** (`#[cfg(test)]` only) that paints a brand
   token under forced-colors **must** produce a `NonSystemColor` violation through
   `live_catalog_paint` → proves the live-catalog producer actually observes paint,
   not a stale hand-built descriptor (the failure mode the re-pointing fixes). It
   is excluded from the real `catalog()` so it never reds the production gate.
5. **Enrollment fan-out** — a stub tier body that pushes its `CoverageKey` into a
   `Vec` asserts `enroll_all` invokes the body exactly `fixtures × cells` times
   with no duplicate key — the Cartesian product is total and non-redundant.

All five are pure-CPU and run under the headless gate; only `coverage_golden`
(which consumes the corpus) is `#[ignore]` GPU.

## Landed (Phase 4, plan tasks 4.1–4.6)

The module is implemented and gates green. As-built details and honest
deviations:

- **`coverage/{mod,fixture,matrix,key,enroll,forced_colors}.rs`** — `Fixture` +
  the `fixture!` macro (emits an `inventory::submit!`), `catalog()` /
  `sorted_catalog()`; `Matrix` / `ThemeAxis` / `Viewport` / `Cell` with
  `Matrix::ci_default()` (2×3×2×2 = **24 cells/fixture**), `cells()` Cartesian
  product, and `CELL_CEILING_PER_FIXTURE = 32` asserted by a self-test;
  `CoverageKey` deriving `Eq + Hash` (because `dpr` is the canonical milliscale
  `Dpr`, not `f32`), with `for_cell`, `stem()`, and a `from_stem() -> ParsedStem`
  **lossless round-trip** (`ParsedStem` is the owned-fields parse result, kept
  distinct from the `'static`-borrowed `CoverageKey`).
- **`build_app(fx, cell) -> App`** — a CPU-only deterministic app: the cell theme
  installed, a synthetic `PrimaryWindow` sized to viewport × dpr, `forced_colors`
  on `UserPreferences`. `enroll_all(matrix, body)` drives the body over
  `catalog × cells`; `enroll_fixtures(slice, …)` is the subset driver.
- **Auto-enroll-by-construction is proven by `adding_one_fixture_grows_corpus_by_axes`**:
  a new fixture grows the corpus by exactly |axes| (24) cells, and the per-tier
  enrollment drivers (`coverage_{layout, display_list, invariants, golden,
  forced_colors}.rs`) are thin `enroll_all` callers with zero per-widget test
  code.
- **`forced_colors::live_catalog_paint()`** builds each fixture's app, queries
  the spawned `Background`/`Border`/`Outline` (+ `BoxShadow`-presence delta) off
  the `Name`-tagged root, and projects them into the existing `CatalogPaint`. The
  analyzers (`analyze_forced_colors` / `analyze_shadow_only`) run **unchanged** —
  only the input source moved from hand-built descriptors to the live tree
  (closes follow-ups.md:469–473). Teeth: `broken_fixture_produces_violation` (a
  `#[cfg(test)]` brand-token fixture, excluded from the real catalog, MUST flag
  `NonSystemColor`) proves the producer reads real paint;
  `safe_fixture_produces_no_violation` proves it is not a constant-violation
  function.
- **Honest deviation — the default `Button::new()` is not forced-colors-safe.**
  It paints the brand token `color.surface.secondary`, which under Buiy's
  *wholesale* forced-colors theme swap resolves to the magenta missing-token
  sentinel (a genuine gate-#11 violation per `color-and-forced-colors.md § 3.1`).
  Making the default widget forced-colors-safe is a `buiy-widget-catalog-design`
  concern, not this campaign. The catalog fixture therefore inserts
  forced-colors-safe **system-color** paint (the catalog's target), and the
  producer reads those live components. Consequence recorded faithfully: because
  the swap is wholesale, no token resolves in *both* light and forced themes, so
  the system-color button renders the magenta sentinel under the *light* theme —
  captured in the committed `*.light.*` display-list baselines (48
  CPU-deterministic `.snap`s: 24 layout + 24 display-list). Documented as
  expected, not a harness bug.
- **The forced-colors `BoxShadow` *visual* reftest stays BLOCKED.**
  `boxshadow_visual_reftest_is_blocked` is an `#[ignore]`'d, assertion-free
  placeholder documenting the dependency on the unlanded `BoxShadow` extract/draw
  path (`extract_buiy_nodes` has no `BoxShadow` branch — follow-ups.md:474–478).
  It is **not** authored as a green test. The structured
  `analyze_forced_colors` / `analyze_shadow_only` scan covers the rest of gate
  #11 today, with no dependency on that path.
- **Added `Backend::Cpu` to the golden `Backend` enum** so CPU (Tiers 1-3) and
  GPU golden cells key off one enum (`goldens.md` § As-landed).

## Sources

- Code: `render/forced_colors_analyzer.rs:10,21,51,89` (the `CatalogPaint` seam + analyzers);
  `tests/render_forced_colors_analyzer.rs:11` (hand-built descriptors today);
  `buiy_widgets/src/button.rs:18,47` (live `Background`/`Border` catalog);
  `theme.rs:62,110` (`default_light_theme` / `forced_colors_theme`);
  `theme.rs:56` (`UserPreferences.forced_colors`); `components.rs:25` (`ResolvedLayout`);
  `render/extract.rs:139` (`ExtractedNodes.scale_factor` = DPR axis);
  `examples/hello_button/src/main.rs` (the spawn that fixtures generalize);
  follow-ups.md:462–481 (the forced-colors live-catalog seam).
- Prior art: `prior-art/skia-gold/lessons.md` §Borrow.2 (params/traces key schema),
  §Borrow.3 (set-valued baselines), §Avoid (retrofit-keys / stale-positive pitfalls).
- Report: `reports/2026-06-14-visual-bug-detection-strategy.md`
  §Cross-cutting → "Coverage auto-generation from the catalog/BSN" (Chromatic modes,
  matrix enrollment, #11 falling out of the same enrollment); Open Q #6 (storage trigger).
- Sibling specs: `snapshot.md` (`assert_layout_snapshot` / `assert_display_list_snapshot`,
  `glob!`), `invariant.md` (Tier-3 predicates), `golden.md` (`assert_golden`, persistence,
  `Backend`, multi-positive), `determinism.md` (`DeterministicApp`, DPR/clock pin),
  `metric.md` (`FuzzBudget`).
