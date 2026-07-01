# Tier 5 — golden persistence + triage

**Date:** 2026-06-15
**Status:** landed (Phase 3; `crates/buiy_verify/src/golden.rs` + `golden/{check,ledger,report}.rs`; corpus started)
**Spec:** specs/2026-06-15-buiy-verification-design/README.md

> **As-landed reconciliation.** The `GoldenKey` / `Backend` / `BlessLedger` /
> `Positive` / `TriageReport` / `TriageCard` shapes match this spec verbatim, with
> these landed details:
> - **`Backend` gains a `Cpu` variant** (`Backend::Cpu` in `golden.rs`) so the structured
>   Tiers 1-3 coverage cells (`CoverageKey`, no GPU) and the GPU `GoldenKey` key
>   off **one** enum. `Cpu` is never a golden capture backend; it is the
>   coverage-tier marker.
> - **`GoldenKey::slug()` uses `__` field separators** (e.g.
>   `rect-rounded/default/dark__sm__fc0__lavapipe__dpr1` — the forced-colors mode
>   `fc0`/`fc1` is its own field), with `from_slug` the lossless inverse. The
>   directory-per-`widget/state` layout keeps a fixture's whole row of cells
>   together.
> - **The bless policy is a `BlessMode` enum threaded as a parameter, not an env
>   read deep in the comparison.** `check_golden`/`assert_golden` read
>   `BUIY_BLESS`/`BUIY_BLESS_REPLACE` once at the top (`mode_from_env`) and pass
>   `BlessMode { Assert | Bless { replace } }` down; `check_golden_in` /
>   `assert_golden_in` take an explicit corpus root + report dir + mode (no env,
>   no process CWD) so the pure-CPU self-tests run hermetically against a temp
>   corpus.
> - **The corpus is started, not full — 5/6 residue classes are aspirational:**
>   only two cells are blessed — `rect-rounded` (the **SDF corner AA** residue
>   class) and `text-ahem` (the Ahem layout class, not a residue class) — at
>   `dark/sm/fc0/lavapipe/dpr1`, each one positive `.png` + its `.toml` ledger under
>   `crates/buiy_verify/tests/goldens/`. The other **five residue classes
>   enumerated above** — drop-shadow Gaussian kernel, glyph fidelity, color-emoji
>   atlas, the effect compositor, blend/gamma, and the forced-colors *visual*
>   residual — have **no committed golden yet** and are renderer-blocked
>   (e.g. the `BoxShadow` extract/draw path is not landed; color-emoji waits on a
>   pinned bundled emoji font). They are aspirational: the harness is ready (a
>   fixture + one `assert_golden`), only the renderer leg / pinned asset is
>   missing (see § Sources / follow-ups).
> - **Adapter-gated committed comparison (audit #7).** Stored goldens are blessed
>   against the **pinned lavapipe** (Mesa llvmpipe); on any other adapter the
>   rim/AA pixels differ (this host's RX 6700 XT diverges by
>   `max_channel_delta=35`), so a committed-baseline EXACT comparison is **gated**
>   on `support::on_pinned_lavapipe()` (env `WGPU_ADAPTER_NAME=llvmpipe`, else a
>   real `RenderAdapterInfo` probe). On lavapipe it compares EXACT; off it the
>   cell **skips-as-pending** (never a cross-rasterizer hard-fail). This is how
>   `determinism.md`'s "the local lane does not compare against the stored
>   lavapipe baseline" is enforced in code — `golden_sdf_corner` and every
>   `matrix_goldens` cell consult the same probe.
> - **Non-vacuity contract (audit #14) — green-on-lavapipe-with-goldens ⟹ ≥1
>   cell compared.** `coverage_golden::matrix_goldens` iterates the whole
>   `Matrix::ci_default()` over the catalog (today: the single `button` fixture =
>   24 cells), **skip-as-pending** for any cell that is un-blessed *or* captured
>   off lavapipe. A green run no longer passes on `pending` alone: a guard
>   **fails** the test when `on_lavapipe && any_matrix_cell_blessed &&
>   asserted == 0`. So on the canonical rasterizer, if any matrix cell is blessed,
>   green *implies* at least one real comparison happened. The guard stays silent
>   (lane honestly green) in the two legitimate zero-compare cases: no matrix cell
>   blessed yet (the current aspirational state — the catalog's only fixture,
>   `button`, has no golden), or off lavapipe (every blessed cell adapter-skips).
>   The eprintln status line reports `asserted`/`pending`/`on_lavapipe`/
>   `any_cell_blessed` so a reader sees exactly why a run was vacuous. The every-PR
>   **headless** gate is unaffected (it never runs `--ignored`). Blessing the
>   `button` corpus is still gated on the default widget being forced-colors-safe
>   (else the wholesale swap would bless the magenta sentinel as a golden).

Tier 5 is the stored-baseline regression tier for the irreducible rasterization
residue — what Tiers 1–4 provably cannot reach: SDF corner AA (beyond the CPU
cross-check), the drop-shadow Gaussian kernel, glyph/color-emoji atlas output,
the effect compositor, blend/gamma, and the forced-colors *visual* residual.
This file specifies `assert_golden` persistence against a `tests/goldens/`
corpus keyed `widget × state × theme × viewport × backend × dpr` with
set-valued (multi-positive) baselines, the `BUIY_BLESS` accept-FILE workflow
(modeled on `BUIY_ACCEPT_SHAPING`), a self-contained offline HTML triage report
+ diff-PNG emit, the in-git→object-store storage migration, and the
Ahem/obscure-text split that keeps real glyphs out of *layout* goldens. It is
deliberately the smallest tier (report §Tier5); the renderer and capture path
already exist — what is missing is the corpus, the persistence machinery, and
the curated set.

## Contract deviations

None. This file consumes `buiy_verify::metric` (`Diff`/`FuzzBudget`),
`determinism::DeterministicApp`, and the promoted
`buiy_core::render::golden::capture_to_image` exactly as the shared contract
defines them, and extends `GoldenConfig` only as `determinism.md` already
mandates. `assert_golden` matches the contract signature
(`key: &GoldenKey, &RgbaImage, &FuzzBudget`); the `GoldenKey`, `BlessLedger`, and
HTML-report types below are additive and live entirely in `buiy_verify::golden`.

## Module: `buiy_verify::golden`

GPU-coupled (`#[ignore]`, GPU lane — CLAUDE.md). Capture is delegated to
`buiy_core` (the promoted `capture_to_image`); everything else here is pure CPU
and unit-testable without an adapter.

```rust
// crates/buiy_verify/src/golden.rs

use image::RgbaImage;
use crate::metric::{compare, CompareOpts, Diff, FuzzBudget};
use buiy_core::render::golden::Dpr;   // canonical Dpr — defined in determinism.md

/// The trace identity (Skia-Gold "params/traces"; skia-gold/lessons §Borrow 2).
/// FIXED before any golden is generated — retrofitting keys re-baselines the
/// whole corpus. Ordered fields drive a stable on-disk path + the report.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GoldenKey {
    pub widget: String,        // catalog fixture id (BSN gallery entry)
    pub state: String,         // default | hover | focus | pressed | disabled
    pub theme: String,         // light | dark | high-contrast | forced-*
    pub viewport: String,      // named viewport (e.g. "sm" 360x640)
    pub forced_colors: bool,   // forced-colors MODE (fc0/fc1) — a distinct axis
                               // from theme: the same theme renders differently
                               // with forced-colors on, so each mode is its own
                               // baseline (mirrors CoverageKey; coverage.md).
    pub backend: Backend,      // Lavapipe | Vulkan | Gl | Metal | Dx12 | Cpu
    pub dpr: Dpr,              // canonical buiy_core::render::golden::Dpr (milliscale)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
// `Cpu` is the structured-tier marker (Tiers 1-3 share this enum with the GPU
// golden tier); it is never a capture backend.
pub enum Backend { Lavapipe, Vulkan, Gl, Metal, Dx12, Cpu }

// `Dpr` is the canonical type from `buiy_core::render::golden` (defined in
// determinism.md): integer milliscale (1000 = 1×, 2000 = 2×), `Eq + Hash + Ord`
// so the key compares/sorts without float pitfalls. Imported above; NOT
// redefined here. It already derives `serde::Serialize`/`Deserialize` at its
// definition site, so `GoldenKey`'s derives are satisfied.

impl GoldenKey {
    /// `widget/state/theme__viewport__fc__backend__dpr` — directory per widget
    /// keeps a fixture's whole row of cells together for review. The
    /// forced-colors mode is `fc0`/`fc1`. Slug-safe; no raw `Debug`.
    pub fn slug(&self) -> String { /* deterministic, lower-kebab */ }
    /// Corpus directory holding `<slug>.<n>.png` (n = positive index) + the
    /// `<slug>.toml` ledger. Default `crates/buiy_verify/tests/goldens/`.
    pub fn dir(&self, root: &std::path::Path) -> std::path::PathBuf { /* root.join(self.slug parts) */ }
}
```

### `assert_golden` — the public entry point

```rust
/// Compare `actual` against the stored multi-positive baseline set for `key`,
/// gated by `budget`. On `BUIY_BLESS=1` this *blesses* instead of asserting
/// (see below). On a non-bless failure: writes the diff PNG, appends an HTML
/// triage card, and panics with the report path. The contract signature
/// `assert_golden(key: &GoldenKey, &RgbaImage, &FuzzBudget)` takes a pre-built key.
pub fn assert_golden(key: &GoldenKey, actual: &RgbaImage, budget: &FuzzBudget);

/// The same comparison without the panic — for the harness's own tests and for
/// the coverage matrix driver that collects many cells before reporting.
pub fn check_golden(key: &GoldenKey, actual: &RgbaImage, budget: &FuzzBudget) -> GoldenOutcome;

pub enum GoldenOutcome {
    /// `actual` matched at least one stored positive within `budget`.
    Pass { matched_positive: usize, diff: Diff },
    /// No positive matched. Carries the best (smallest-`Diff`) candidate so the
    /// report can show the *closest* baseline, not an arbitrary one.
    Fail { best: Option<(usize, Diff)>, report: std::path::PathBuf },
    /// `BUIY_BLESS=1`: wrote a new/updated positive. Never reached in CI
    /// (`BUIY_BLESS` unset ⇒ env-gated, mirrors `BUIY_ACCEPT_SHAPING`).
    Blessed { positive: usize, was_new: bool },
}
```

**Set-valued match (multi-positive).** A key maps to a *set* of accepted PNGs,
not one (Skia Gold "many positives per config"; skia-gold/lessons §Validates).
`check_golden` compares `actual` against each positive via
`metric::compare(actual, positive, &CompareOpts::default())` and passes if
*any* `Diff::passes(budget)`. This is essential for residual GPU AA jitter that
the determinism pin reduces but does not eliminate. Default budget after the
determinism pin is `FuzzBudget { max_channel_delta: 0, max_diff_pixels: 0 }`
(determinism.md); widen per-fixture with a documented reason in the ledger
(Mozilla `fuzzy-if`, "ranges must not include 0" — report §Cross-cutting).

**Stale-positive guard.** Multi-positive accumulates stale entries silently — a
real regression can match an old wrong positive (skia-gold/lessons §Avoid). The
ledger records, per positive, the blessing commit + timestamp + a one-line
reason; `cargo run -p buiy_verify --bin golden-prune` lists positives unmatched
by any recent run for human removal. Pruning is *advisory*, never automatic.

### The bless ledger (persistence)

```rust
/// `<slug>.toml` beside the PNGs — the durable accept ledger reg-suit lacks
/// (skia-gold/lessons §Avoid "implicit-in-git-history acceptance"; §Borrow 1).
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BlessLedger {
    pub key: GoldenKey,
    pub positives: Vec<Positive>,   // index i ⇒ `<slug>.i.png`
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Positive {
    pub file: String,               // `<slug>.0.png`
    pub blessed_commit: String,     // `git rev-parse HEAD` at bless time
    pub blessed_at: String,         // RFC3339
    pub budget: FuzzBudget,         // per-fixture widened budget + its reason
    pub reason: String,             // why this positive (or why widened)
}
```

### `BUIY_BLESS` accept-FILE workflow

Replaces the inline "re-capture IS the golden" discipline of the current
`#[ignore]` GPU tests (which assert `perceptual_diff < 1e-4` between two fresh
captures — a *determinism* check, not a stored regression). Modeled exactly on
`BUIY_ACCEPT_SHAPING` (`tests/text_shaping_snapshots.rs:296`):

- `BUIY_BLESS` **unset** (CI + default): `assert_golden` reads the baseline set,
  fails closed if the corpus has no positive (`panic!` instructing the dev to
  bless + review + commit — verbatim shape of the shaping panic at
  `text_shaping_snapshots.rs:301`).
- `BUIY_BLESS=1`: `assert_golden` writes `actual` as a new positive (or replaces
  positive 0 when `BUIY_BLESS_REPLACE=<i>`), updates the ledger, and returns
  `Blessed`. **Then the human reviews the PNG diff in the PR and commits it** —
  blessing is an explicit, reviewable, diffable act, never a silent overwrite
  (Flutter `--update-goldens` + pre-submit triage; flutter-golden/lessons
  §Borrow 4). One canonical invocation, documented in the module header:

  ```sh
  BUIY_BLESS=1 cargo test -p buiy_verify --test verify_gpu -- --ignored --test-threads=1 goldens
  ```

### Diff-PNG + self-contained HTML triage report

Offline-first, no SaaS (project ethos; skia-gold/lessons §Borrow 6
reg-cli/x-img-diff-js). On any `Fail`, the harness:

1. Writes `target/buiy-goldens/<slug>.diff.png` — the `Diff::diff_image`
   heatmap from `metric` (already produced by `compare` when
   `CompareOpts::default()` requests it).
2. Appends a card to a single self-contained `target/buiy-goldens/report.html`
   (one file per `cargo test` run, all failing cells accumulated):

```rust
pub struct TriageReport { path: std::path::PathBuf, cards: Vec<TriageCard> }
pub struct TriageCard {
    pub key: GoldenKey,
    pub actual_png: Vec<u8>,    // base64-inlined ⇒ self-contained, CI-artifact-portable
    pub baseline_png: Vec<u8>,  // the closest positive (GoldenOutcome::Fail.best)
    pub diff_png: Vec<u8>,
    pub diff: Diff,             // differing_pixels / max_channel_delta / mssim
    pub budget: FuzzBudget,
}
impl TriageReport {
    pub fn open_or_create(path: &std::path::Path) -> Self;
    pub fn push(&mut self, card: TriageCard);
    /// Emit one HTML file: side-by-side, toggle-overlay (JS opacity slider),
    /// and diff-heatmap views per card, all PNGs base64-inlined. No external
    /// assets, no network — openable straight from CI artifacts.
    pub fn write(&self) -> std::io::Result<()>;
}
```

The HTML embeds three views per card (skia-gold/lessons §Borrow 6):
side-by-side expected|actual, a slider/toggle overlay, and the diff heatmap.
Triage = human eyeballs it, then runs the `BUIY_BLESS=1` command to promote
actual→positive. **Borrowed primitives, deferred to follow-ups** (not v1):
Skia-Gold time-boxed ignore rules (a `[[ignore]]` block in the ledger with an
RFC3339 `expires`, for an expected mass change like a font roll) and Argos-style
flaky auto-ignore (min-occurrences heuristic) — design hooks named, machinery
deferred (skia-gold/lessons §Borrow 5, 8).

## Capture: promote `capture_to_image` into `buiy_core`

The pure/GPU split (shared contract): the device-coupled capture lives in
`buiy_core::render::golden`, callable by `buiy_verify`. Promote the
`render_to_image`/`readback_rgba`/`spawn_capture_camera` triad from
`tests/support/mod.rs` into a library fn:

```rust
// crates/buiy_core/src/render/golden.rs  (new, src — not tests)
/// Render `app` to an offscreen Rgba8UnormSrgb target sized to the window's
/// physical pixels and read it back as an `image::RgbaImage`. Honors
/// `cfg.wait_for_fonts` (drives frames until `fonts_ready`, support/mod.rs)
/// before capture. The single GPU-coupled primitive every Tier-4/5 test shares.
pub fn capture_to_image(app: &mut bevy::app::App, cfg: &GoldenConfig) -> image::RgbaImage;
```

This adds `image = "0.25"` (already a workspace dep) to `buiy_core`; no new
crate. The existing naive `perceptual_diff` (`render/golden.rs`) is deprecated —
its callers move to `buiy_verify::metric::compare` (shared contract).

## The Ahem / obscure-text split — keep real glyphs out of *layout* goldens

Two classes of golden, per the Flutter/Alchemist two-class trick
(flutter-golden/lessons §Validates, §Borrow 1, 3):

- **Layout-determinism class (the bulk).** Text-bearing goldens that test
  *boxes*, not glyph fidelity, render under `BUIY_TEST_FONT` — a clean-room
  box-glyph font (UPM **1024**, pinned ascent/descent 0.75/0.25 em, line-gap 0,
  every glyph a solid em-box). Power-of-2 UPM makes metrics integer-exact and
  font-engine-agnostic — boxes alone are not enough (flutter-golden/lessons
  §Avoid "boxes instead of curves"). This collapses the font axis: any
  layout-class golden is byte-identical across hosts. Wired through the same
  `FontRegistry::register_bytes` path the shaping fixtures use
  (`support/mod.rs`); selected by `DeterministicApp::test_font()`
  (determinism.md). Shadows in this class swap to a flat fill via
  `BUIY_DISABLE_SHADOWS` (engine-side, release-safe — flutter-golden/lessons
  §Avoid "debug-build-only killswitch"; spec'd in determinism.md).

- **Real-font fidelity class (deliberately narrow).** Only goldens that *assert*
  glyph rasterization — hinting/subpixel, decoration position, color-emoji —
  render real `cosmic-text`/`harfrust` glyphs from one pinned bundled OFL font
  per script (the committed fixture fonts, `tests/fixtures/fonts/`), on the
  pinned lavapipe rasterizer, with a documented widened budget. The shaping
  `.snap` fixtures already pin glyph *positions* deterministically for 6
  scripts; this class adds the *pixel* fidelity check the snapshots can't.

**Color emoji is the canonical irreducible golden** (report §Tier5;
flutter-golden/lessons §Avoid "trying to make color emoji deterministic"). It
has no feature-free reference (you cannot re-author a CBDT bitmap or a COLR
layer stack from primitives), is highly font-version-sensitive, and a user
notices tofu/wrong-emoji instantly. It belongs in the real-font class with a
pinned bundled emoji font, captured once on the canonical rasterizer, and a
generous per-fixture budget — never fought with determinism knobs. A font-version
roll is triaged via the time-boxed ignore (deferred primitive above).

## Storage staging + migration trigger

Designed now so migration is mechanical (report §Cross-cutting; skia-gold/lessons
§Borrow 1, 2):

- **Now:** positives live in-git under `crates/buiy_verify/tests/goldens/`,
  reviewed as the PR diff. The box-font layout class produces *tiny* PNGs
  (solid rectangles compress hard), so churn is bounded; git-LFS only if the
  real-font class churn bites. `*.png` under `tests/goldens/` gets `-text` in
  `.gitattributes` (mirrors the `*.snap` pin already present).
- **Later (only if the count explodes):** commit-hash-keyed object storage
  (reg-suit's keygen+publisher split) — a content-addressed bucket
  (local dir → optional S3/GCS) with the baseline fetched as the parent
  commit's snapshot, git stays clean. The `GoldenKey` schema + `BlessLedger`
  are the durable accept ledger that reg-suit lacks; the object store only
  changes *where bytes live*, not the key or the bless contract. Design the
  rebase/squash/merge commit-key edge cases up front (skia-gold/lessons §Avoid
  "naive commit-key resolution").
- **Migration trigger (Open Q for the synthesizer — report §OQ6):** propose
  **total in-git golden bytes > 50 MB OR positive count > 500** as the
  planned threshold. Name it now so migration is a step, not a crisis. Do *not*
  build a Skia-Gold-class service (skia-gold/lessons §Avoid).

## Dependencies

- `image = "0.25"` — already a workspace dep (PNG I/O); now also used in
  `buiy_core`. No add.
- `serde`/`serde_json` — already deps (ledger TOML/JSON). The ledger uses
  `toml` for human-diffable review; **add `toml = "0.8"`** to
  `[workspace.dependencies]` and `buiy_verify`. New dep ⇒ run
  `cargo deny check` before committing (CLAUDE.md); `toml` (MIT/Apache-2.0) is
  license-clean and already transitively present via `cargo` tooling.
- HTML report: hand-written `String` templating + `base64` inlining. **Add
  `base64 = "0.22"`** (MIT/Apache-2.0) to inline PNGs; gate on `cargo deny
  check`. No templating/WASM crate — the report is a static string, offline by
  construction.
- No perceptual-metric crate is added *here* — `metric.md` owns that.
- No object-store/S3 crate now — deferred until the migration trigger fires.

## Verification (how the Tier-5 harness tests itself)

The harness is mostly pure CPU; only capture needs the GPU lane.

1. **Match/mismatch unit tests (no GPU).** Synthesize two `RgbaImage`s in
   memory, write one as a positive via the bless path, assert `check_golden`
   returns `Pass` on an identical image and `Fail` on a one-pixel-over-budget
   image. Proves the set-valued comparison + budget gate without a renderer.
2. **Multi-positive.** Bless two near-identical positives; assert an image
   matching the *second* returns `Pass { matched_positive: 1 }`. Proves the
   any-positive-matches semantics.
3. **Bless round-trip.** With `BUIY_BLESS=1`, bless an image to a temp corpus
   root, re-run without the env, assert it now passes and the ledger records
   commit/timestamp/reason. Mirrors the shaping accept-then-assert test shape.
4. **Fail-closed.** Empty corpus + `BUIY_BLESS` unset ⇒ `assert_golden` panics
   with the bless instruction (assert on the panic message, à la
   `text_shaping_snapshots.rs:301`).
5. **Report self-containment.** Generate a `TriageReport` with one card, assert
   the emitted HTML contains the base64 PNGs and references no external URL
   (grep the string for `http`/`src="./"`). Proves offline-first.
6. **Key/slug stability.** Property test (`proptest`, already a dep): a
   `GoldenKey` round-trips through `slug()`→parse and two distinct keys never
   collide on a slug.
7. **GPU lane (`#[ignore]`).** One end-to-end golden per residue class (SDF
   corner, shadow kernel, real-font glyph, color-emoji) captured via
   `capture_to_image` under `DeterministicApp`, blessed once, asserted on the
   pinned rasterizer. The Ahem layout-class golden additionally asserts
   byte-identity across two fresh captures (re-capture determinism) *and*
   equality to the stored positive — proving the box-font collapse holds.

## Sources

- Code: `crates/buiy_core/src/render/golden.rs` (GoldenConfig,
  deterministic(), perceptual_diff, fonts_ready);
  `crates/buiy_core/tests/support/mod.rs` (render_to_image,
  spawn_capture_camera, wait_for_text_ready, register_fixture_font,
  readback_rgba); `crates/buiy_core/tests/text_shaping_snapshots.rs:296,301`
  (BUIY_ACCEPT_SHAPING accept-FILE precedent + fail-closed panic);
  `crates/buiy_verify/src/visual.rs` (naive RMSE, superseded by `metric`);
  `.gitattributes` (`*.snap -text` pin to mirror for `*.png`).
- Prior-art: `docs/prior-art/skia-gold/lessons.md` (key schema, multi-positive,
  durable accept ledger, commit-keyed store, local HTML report, expiring
  ignores, stale-positive pruning); `docs/prior-art/flutter-golden-testing/lessons.md`
  (box-glyph UPM-1024 font, engine-side shadow killswitch, two-tier
  obscure/real split, `--update-goldens` curated accept, color-emoji as
  irreducible golden).
- Report: `docs/reports/2026-06-14-visual-bug-detection-strategy.md` §Tier 5,
  §Cross-cutting (golden storage strategy, `--accept`/triage UX, Ahem split),
  §Open questions 6 (storage-migration trigger), 7 (Ahem boundary + emoji
  baseline).
