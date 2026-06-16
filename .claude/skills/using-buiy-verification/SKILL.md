---
name: using-buiy-verification
description: How to USE the Buiy visual-bug verification harness (crate buiy_verify) — pick the right tier, add a fixture, write a layout/display-list snapshot, a reftest, an invariant, or bless a golden, and run the headless + GPU gates. Use whenever adding or changing a visual/layout/render test, adding a widget fixture, debugging a flaky snapshot, or blessing golden images. Mirrors docs/specs/2026-06-15-buiy-verification-design/.
---

# Using the Buiy verification harness

Crate `buiy_verify` is Buiy's defence against visual bugs (misplaced boxes, wrong
colors, broken paint order, AA seams, BiDi caret drift) as the library scales. It
is a **five-tier pyramid**, reftests-first: catch bugs in cheap, deterministic,
structured tiers and shrink the expensive flaky pixel tier to the irreducible
rasterization residue.

**Source of truth:** the design spec
[`docs/specs/2026-06-15-buiy-verification-design/`](../../../docs/specs/2026-06-15-buiy-verification-design/)
(README + one file per tier) and the strategy report
[`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../../docs/reports/2026-06-14-visual-bug-detection-strategy.md).
If this skill drifts from those, they win — update this skill in the same commit.
The crate root doc (`crates/buiy_verify/src/lib.rs`) is the code-proximate twin.

## When to use this skill

Before: adding/changing any visual, layout, paint-order, color, or render test;
adding a widget fixture; writing a reftest; adding an invariant predicate;
blessing or re-blessing golden images; debugging a flaky snapshot. If you are
only *running* the gates, jump to [Running the gates](#running-the-gates).

## The five tiers — and which one to add a test at

Add a test at the **lowest tier that can observe the bug**. Lower tiers are
cheaper, deterministic, headless (no GPU), and name the bug precisely; goldens
are flaky and only say "N pixels changed".

| Tier | Module | Catches | GPU? |
|---|---|---|---|
| **1 Layout snapshot** | `snapshot::assert_layout_snapshot` | wrong position/size, wrong tree | no (headless) |
| **2 Display-list snapshot** | `snapshot::assert_display_list_snapshot[_at]` | wrong resolved color, clip, instance packing, paint membership | no (headless) |
| **3 Invariant / metamorphic** | `invariant::*` predicates + proptest | properties that must hold for ALL scenes (paint order total, transform round-trips, top-layer dominance, finiteness, BiDi caret round-trip) | no (headless) |
| **4 Reftest + SDF cross-check** | `reftest!` macro, `run_sdf_cross_check` | "two equivalent inputs render identically (==) or differ (!=)"; CPU-vs-GPU SDF agreement | **yes (`#[ignore]`)** |
| **5 Golden** | `golden::assert_golden` | the irreducible rasterization residue: SDF corner AA, drop-shadow kernel, glyph/emoji atlas, compositor, forced-colors *visual* | **yes (`#[ignore]`)** |

Decision: a number wrong → Tier 1. A color/clip/paint-membership wrong → Tier 2.
A property that must hold for every scene → Tier 3. "These two ways of expressing
the same thing must match" → Tier 4 reftest (no stored image). Only pixels a
rasterizer alone produces → Tier 5 golden.

## Coverage-by-construction: add ONE fixture, enroll everywhere

The decisive property: a **fixture** (`widget × state` BSN scene factory) authored
once auto-enrolls across **every** tier and the full `Matrix` of
themes × viewports × forced-colors × DPRs — **no edits to any per-tier test
list** (no tier body changes). Two steps to add one:

1. Author `crates/buiy_verify/fixtures/<widget>/<state>.rs` (note: under the
   crate root, **not** `tests/`) with the `fixture!` macro:

```rust
buiy_verify::fixture! {
    name  = "button",          // lower-kebab, unique widget id; becomes the Name + stem
    state = "resting",         // resting | hover | focus | pressed | disabled (one file per state)
    spawn = |app| {
        app.world_mut().spawn(bevy::prelude::Camera2d);  // a GPU capture needs a view
        // spawn the widget already in `state`, and Name-tag its root:
        // every dump keys entities by Name, never by Entity bits.
    },
}
```

2. Declare it once in `crates/buiy_verify/src/coverage/mod.rs` so the
   `inventory::submit!` is compiled into the crate:
   `#[path = "../../fixtures/button/resting.rs"] mod fixture_button_resting;`
   (the registry is link-time, so this `#[path] mod` line is the only wiring —
   no central fixture *list*, no per-tier edits).

The fixture **contract** (a doc-comment MUST, only partly backstopped — there is
no assertion that checks it): `spawn` should spawn a `Camera2d` (a missing one
merely fails the later GPU capture) and `Name`-tag the widget root (a missing
`Name` falls back to an `entity#<index>` label — diff-unstable). The one case
that DOES fail loudly is two same-`Name` siblings with the same box (the
content-tiebreak panic). `(name, state)` is the unique corpus key. Iterate via
`coverage::sorted_catalog()` (stable `(name, state)` order); `Matrix::ci_default()`
+ `enroll_all` multiply a tier body over `catalog × cells`.

## How to add each kind of test

### Tier 1 — layout snapshot
```rust
let mut app = /* MinimalPlugins + CorePlugin + LayoutPlugin + your scene */;
buiy_verify::snapshot::assert_layout_snapshot(&mut app, "my_case"); // runs one update, dumps boxes
```
Dump = `(Name, position, size)` per `ResolvedLayout` entity, content-keyed (Name
then box), floats rounded — host-stable. Stored as an `insta` `.snap`. A number
change ⇒ snapshot diff ⇒ RED.

### Tier 2 — display-list snapshot
```rust
buiy_verify::snapshot::assert_display_list_snapshot(&nodes, "my_case", &names);
// or, for a time-driven (animated) fixture, sampling logical timestamps:
buiy_verify::snapshot::assert_display_list_snapshot_at(&mut app, "blink", &[Duration::ZERO, Duration::from_millis(500)]);
```
Dumps `painters_z` node order + packed `InstanceBuckets` draw order; color as
`#rrggbbaa`. Use `assert_instance_hex_snapshot` for a byte-exact `PackedInstance`
check (catches a 1-LSB packing drift).

### Tier 3 — invariant / metamorphic
Predicates in `invariant::` take a realized scene and return `Result<(), Violation>`:
`paint_order_is_total`, `transform_roundtrips`, `top_layer_dominates`,
`all_finite`, `bidi_caret_roundtrips`. Drive them with the proptest generators
(`invariant::scene`). **Every predicate MUST have a mutation fixture** — a
hand-built BROKEN scene asserted to return `Err` — else the property is vacuous
(a passing test that can't fail is the worst bug in a verifier). Add the mutation
fixture in the same change as the predicate.

### Tier 4 — reftest (no stored image)
```rust
// match: the two inputs must render IDENTICALLY; mismatch: they must DIFFER.
buiy_verify::reftest!(match,    flex_justify_end, flex_test,  literal_offsets_ref);
buiy_verify::reftest!(mismatch, cv_hidden_hides,  cv_visible, cv_hidden);
buiy_verify::reftest!(match,    transform_xy,     xfm_test,   literal_ref, fuzz = (1, 8));
```
Generates one `#[test] #[ignore]` GPU case each. The reference MUST reach the
result by a DIFFERENT code path than the test input (the independence lint fails a
reference that re-uses the feature under test — else the comparison passes
vacuously). A non-`(0,0)` fuzz floor on a `mismatch` **fails to compile** (a
fuzzy "they differ" is meaningless). For SDF corner AA, `run_sdf_cross_check`
compares the GPU output against an independent CPU oracle.

### Tier 5 — golden
```rust
buiy_verify::golden::assert_golden(&key, &captured_image, &FuzzBudget::EXACT);
```
`GoldenKey { widget, state, theme, viewport, forced_colors, backend, dpr }` is the
trace identity — **fixed before any golden is generated** (adding a field
re-baselines the whole corpus). Baselines are **multi-positive** (any committed
positive matching ⇒ pass) and each positive is gated by **its own recorded
budget** (widen per-fixture for known SDF/shadow jitter; default `EXACT`). Only
add a golden for residue Tiers 1–4 provably cannot reach.

## Blessing goldens (the accept workflow)

Goldens are **never** auto-overwritten. To create/update a baseline, capture on a
real GPU host, then **review the PNG diff** and commit:
```sh
# assert against the committed corpus (GPU lane):
cargo test -p buiy_verify --test goldens -- --ignored --test-threads=1
# bless / re-bless, then REVIEW the diff PNG before committing:
BUIY_BLESS=1 cargo test -p buiy_verify --test goldens -- --ignored --test-threads=1
```
`BUIY_BLESS=1` writes the PNG + a TOML `BlessLedger` entry (commit, timestamp,
budget, reason). The corpus matrix driver (`coverage_golden`) is
**bless-on-demand**: an un-blessed cell is *pending* (skipped), a blessed cell
must still match. On a failure the harness writes a self-contained offline HTML
triage report (diff PNG + cards) and points at it.

## Determinism (why the pixel tiers are reproducible)

Use `determinism::DeterministicApp` to build a capture app: it pins a fixed
virtual clock, atlas warmup, `Dpr` (integer milliscale), MSAA/dither off, and
`FontMode::Ahem` (a bundled em-box font so non-fidelity text is byte-identical
across hosts — use `FontMode::Real` only for the narrow glyph-fidelity suite). CI
pins the **lavapipe** software rasterizer. Capture itself is
`buiy_core::render::golden::capture_to_image`.

## Running the gates

Headless gate (every-PR CI; **must stay green without a GPU** — never runs `--ignored`):
```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace      # drop xvfb-run on macOS/Windows
```
GPU lane (Tiers 4–5, the `#[ignore]` tests — needs a real wgpu adapter or lavapipe;
additive, run on a GPU host):
```sh
cargo test -p buiy_verify -- --ignored --test-threads=1
```
`--test-threads=1` serializes the single adapter context. Keep new GPU tests
`#[ignore]`. **Never run two GPU/cargo jobs in parallel on one `target/`** — build-cache
contention can produce a spurious `SIGSEGV`/lock-stall that looks like a failure.

## Gotchas (each one cost a real bug — see the 2026-06-15 review report)

- **Dumps key by `Name`, never `Entity` index.** Two siblings sharing a `Name`
  with the SAME position+size make a dump non-deterministic and **fail loudly** —
  give list rows distinct Names or distinct positions.
- **A fixture's colors must be ASYMMETRIC** for a color mutation to be observable:
  white `#ffffffff` and the magenta sentinel `#ff00ffff` are both invariant under
  an R↔B swap. The default `Button` paints the magenta missing-token sentinel
  (it is not yet forced-colors-safe) — don't bless that verbatim.
- **`forced_colors` is a golden key axis** (`fc0`/`fc1`): the same theme renders
  differently with forced-colors on. Never collapse it.
- **Tier-3 invariants do NOT catch a production paint-order-ASSEMBLY bug**:
  `invariant::scene::realize` re-implements layout sub-pass 6f (the `painters_z`
  z-tier sort) rather than calling it, so a bug there is caught by buiy_core's own
  `z_index_*` tests today (and, once a relevant widget golden is blessed, the GPU
  golden tier — only 2 residue goldens are committed now), not the metamorphic
  suite. Verified by fault injection 2026-06-15. (Hardening follow-up open in
  `docs/plans/follow-ups.md`.)
- **`compare` returns a saturated `Diff` on a dimension mismatch** (a `0×0`
  capture vs a real baseline) that fails EVERY budget — a blank/failed render is
  loud, never a silent pass.
- **A vacuous test is the worst defect in a verifier.** New predicates need a
  mutation fixture; new known-answer tests must demonstrably fail on the wrong
  answer. Prove RED before trusting GREEN.

## Verify before claiming a visual test "works"

Run the actual gate (headless and, for Tiers 4–5, the GPU lane) and read the
output. For a new detection test, prove it goes RED on the bug it targets (inject
the bug, watch it fail, revert) — green-by-construction tells you nothing. See
`superpowers:verification-before-completion` and the fault-injection method in
`docs/reports/2026-06-15-verification-harness-adversarial-review.md`.
