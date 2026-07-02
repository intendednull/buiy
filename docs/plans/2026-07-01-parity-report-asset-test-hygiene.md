# Parity report-asset test hygiene — plan

**Spec:** [`docs/specs/2026-07-01-parity-report-asset-test-hygiene-design.md`](../specs/2026-07-01-parity-report-asset-test-hygiene-design.md)
**Branch:** `fix/parity-assets-test-hygiene` (off `origin/main`)
**Gate:** GPU `--ignored` lane (RX 6700 XT / RADV locally; pinned lavapipe in CI).

One small, self-contained change: stop six `#[ignore]` GPU tests from rewriting
committed `docs/reports/parity-*-assets/*.png`, and add a durable CI guard.

## S1 — RED: reproduce the dirt (verification-first)

Run one writer test on the real adapter and confirm it rewrites a committed PNG.
The render GPU tests are submodules of the single `render` integration target
(there are no per-file `[[test]]` binaries), so filter by test name:

```sh
cargo test -p buiy_core --test render -- --ignored --test-threads=1 linear_gradient_paints_ac_to_ac2
```

On an adapter whose bytes differ from the committed capture (CI's lavapipe), this
shows `M docs/reports/parity-proto-assets/b1-gradient.png` in `git status`. On the
RX 6700 XT host the baselines were blessed on, the bytes are identical, so use the
**adapter-independent** proof — the write still `touch`es the file:

```sh
f=docs/reports/parity-proto-assets/b1-gradient.png
stat -c %Y $f                                   # mtime BEFORE
cargo test -p buiy_core --test render -- --ignored --test-threads=1 linear_gradient_paints_ac_to_ac2
stat -c %Y $f                                   # mtime AFTER — CHANGED == the write fired (RED)
```

## S2 — Remove the side-effect writes

In each of the six sites, delete the `let out = …; create_dir_all(parent); img.save(&out)…`
block (keep every programmatic assertion above it):

- `crates/buiy_core/tests/render/render_gradient_gpu.rs:95-102` (b1) and `:231-238` (b2)
- `crates/buiy_core/tests/render/render_icon_gpu.rs:114-121` (b3)
- `crates/buiy_core/tests/render/render_backdrop_blur_gpu.rs:162-169` (b4)
- `crates/buiy_core/tests/render/render_top_layer_paint_gpu.rs:133-140` (fix-m1m6)
- `crates/buiy_core/tests/render/render_gradient_paint_order_gpu.rs:134-141` (gradient-paint-order)

Then reword the **five** module doc-comments that claim the test "writes the PNG
to …" so they describe the programmatic check instead
(`render_gradient_gpu.rs:5` + `:162`, `render_icon_gpu.rs:8`,
`render_backdrop_blur_gpu.rs:11`, `render_top_layer_paint_gpu.rs:17`). Leave
`render_gradient_paint_order_gpu.rs`'s doc-comment untouched (no such claim).

Check: no remaining *write* into `docs/reports/` from test code —
`grep -rn "\.save(" crates/buiy_core/tests/render/` returns nothing. (A plain
`grep "docs/reports"` still matches unrelated doc-comment mentions, e.g. in
`tests/crosscut/picking_backend.rs`; the invariant is "no write", not "no
mention".)

## S3 — Add the CI regression guard

In `.github/workflows/ci.yml`, immediately after the GPU-lane step
(`ci.yml:342-343`, `cargo nextest run --profile gpu … --run-ignored=only`), add a
step in the same `gpu` job:

```yaml
      - name: Assert the GPU lane mutated no committed report asset
        run: git diff --exit-code -- docs/reports/
```

Tracked-only, adapter-agnostic; fails the lane if any test ever rewrites a
committed report asset again. (Rationale + no-false-positive proof: spec § Decision.)

## S4 — GREEN: prove clean + still-passing

```sh
cargo test -p buiy_core --test render -- --ignored --test-threads=1 \
  linear_gradient_paints_ac_to_ac2 dotted_grid_paints_lit_dot \
  vector_icons_paint_in_accent backdrop_filter_blurs_the_window_backdrop \
  fix_m1m6_top_layer_descendants ancestor_gradient_does_not_overpaint
git status --porcelain docs/reports/    # expect: EMPTY
```

(libtest OR-matches multiple name filters, so this runs exactly the six writer
tests. Also snapshot the PNG mtimes before/after to prove they are never
touched.)

All five test binaries pass (assertions intact) AND `docs/reports/` stays clean.
Then the mechanical gate on the touched crate:

```sh
cargo fmt --all -- --check
cargo clippy -p buiy_core --all-targets --locked -- -D warnings
```

(The full headless `cargo test --workspace` gate is unaffected — the writers are
all `#[ignore]` — but run it if cheap to confirm no collateral.)

## S5 — Docs

- `docs/plans/follow-ups.md` — add an entry near the stored-PNG golden machinery
  section documenting the bug + fix, marked **LANDED** with this change.
- `docs/README.md` — register the spec + this plan under the verification area.

## Done when

- The five GPU test binaries pass on the adapter with `git status --porcelain
  docs/reports/` empty afterwards.
- No `docs/reports` write remains in `crates/buiy_core/tests/`.
- CI guard step present after the GPU-lane step.
- `fmt` + `clippy -p buiy_core` clean; follow-ups.md + docs/README updated.
- Fresh-context review gate passed.
