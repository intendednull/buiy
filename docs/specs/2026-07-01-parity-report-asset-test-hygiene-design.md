# Parity report-asset test hygiene — design

**Date:** 2026-07-01
**Status:** target state (spec)
**Area:** verification / render GPU tests
**Owner:** follow-up drain (verification neighbourhood)

## Problem

Running the `buiy_core` GPU `--ignored` lane rewrites **committed** PNG files
under `docs/reports/parity-*-assets/*.png` with adapter-dependent bytes, dirtying
`git status` after an otherwise read-only test run and risking someone committing
adapter-specific rasterizer noise. It is a silent side-effect: the affected tests
never *read* these PNGs back — their verification is entirely programmatic pixel
sampling (channel-rise / bounded-tolerance corner checks chosen precisely because
absolute bytes vary per adapter).

## Root cause

Six `#[ignore]` GPU-lane tests in `crates/buiy_core/tests/render/` each end with
an **unconditional** `img.save(&out)` into a committed report-assets path — no
`BUIY_BLESS` gate, no comparison, no consumer:

| Test (save site)                                          | Writes |
|----------------------------------------------------------|--------|
| `render_gradient_gpu.rs:101`                             | `parity-proto-assets/b1-gradient.png` |
| `render_gradient_gpu.rs:237`                             | `parity-proto-assets/b2-dotgrid.png` |
| `render_icon_gpu.rs:120`                                 | `parity-proto-assets/b3-icons.png` |
| `render_backdrop_blur_gpu.rs:168`                        | `parity-proto-assets/b4-blur.png` |
| `render_top_layer_paint_gpu.rs:139`                      | `parity-proto-assets/fix-m1m6.png` |
| `render_gradient_paint_order_gpu.rs:140`                 | `parity-final-assets/gradient-paint-order-gpu.png` |

These PNGs are one-time **proof artifacts** from the parity campaign (PR #83) and
the gradient paint-order fix (PR #111) — an eyeball aid a human wanted while
building those changes, committed once and thereafter re-dumped on every GPU run.
They are **not** goldens: the real golden corpus lives under
`crates/buiy_verify/tests/goldens/` and is written **only** behind `BUIY_BLESS=1`
with a review ledger. The `parity-final-assets/c-*.png` curated per-screen
illustrations are written by **no** code and are out of scope (they stay frozen).

A parallel `docs/reports/2026-06-30-demos-mvu-migration-assets/` dir is written by
the same unconditional-`img.save`-into-committed-path pattern, but from `src/bin/`
**capture binaries** (`capture_todomvc.rs` / `capture_counter.rs`) that the test
lane never runs (`cargo test` does not build `src/bin` targets). It is a known,
deliberately out-of-scope sibling of this fix — the GPU **test lane** is the thing
that dirties the tree on a plain run. The broadened `docs/reports/` CI guard
(below) nonetheless keeps the whole report tree honest against any future
lane-run writer.

## Decision

**Remove the side-effect PNG writes** from all six tests; keep every programmatic
assertion untouched. The committed PNGs freeze at their current bytes (they remain
valid one-time illustrations — `b3-icons.png` is referenced by a report markdown;
the rest are historical proof captures). Add a **durable CI regression guard** so
recurrence fails the lane instead of silently churning the tree.

Concretely:

1. Delete the `let out = …; create_dir_all; img.save(&out)…` block at each of the
   six sites; reword the **five** module doc-comments that claim the test "writes
   the PNG to …" (`render_gradient_gpu.rs:5` + the path-mentioning docs at
   `render_gradient_gpu.rs:162`, `render_icon_gpu.rs:8`,
   `render_backdrop_blur_gpu.rs:11`, `render_top_layer_paint_gpu.rs:17`) to state
   the check is programmatic. `render_gradient_paint_order_gpu.rs` makes no such
   claim (its `img.save` is undocumented) — leave its doc-comment alone.
2. Add, immediately after the GPU-lane step in `.github/workflows/ci.yml`
   (`ci.yml:342-343`, the `--run-ignored=only` step), a guard that fails if the
   lane mutated any committed report asset:
   `git diff --exit-code -- docs/reports/`.
   Scoped to the whole `docs/reports/` tree (not just the two touched dirs): the
   true invariant is "the GPU lane mutates **no** committed report asset," and a
   plain `--run-ignored=only` run writes nothing tracked there — goldens are
   `BUIY_BLESS`-gated (off), shaping snapshots `BUIY_ACCEPT_SHAPING`-gated (off),
   `Cargo.lock` is `--locked`-pinned, and diff/reftest artifacts land under
   `target/`/tempdir (untracked). The broader pathspec future-proofs against a new
   proof subdir at zero false-positive risk. `git diff --exit-code` is
   **tracked-only** by design — exactly the "don't mutate committed assets"
   invariant (untracked scratch is fine).
3. Log + close the follow-up in `docs/plans/follow-ups.md` (currently unlogged).

## Why this is the right fix (not a band-aid)

- **Root-cause, lowest-tier:** the write is a gratuitous side-effect no assertion
  consumes; deleting it removes the churn at its source with no new machinery.
- **Correct layering:** a `buiy_core` render test must READ/assert, never rewrite
  a committed report illustration. The project already has the right mechanism for
  deliberately regenerating committed PNGs (`buiy_verify`'s `BUIY_BLESS` gate +
  ledger); report captures are point-in-time snapshots, correctly frozen.
- **Durable:** the CI guard converts "someone notices a dirty tree" into a hard
  lane failure, adapter-agnostically, catching any future recurrence directly.

## Rejected alternatives

- **Env-gate the writes behind `BUIY_BLESS`** (mirror `buiy_verify`). Rejected as
  the default: these are not goldens and have no comparison/consumer, so a
  regeneration path is machinery without a customer. It stays the natural fallback
  *if* the team later wants to refresh the illustrations — but the un-gated
  every-run write is the actual bug, so the minimum correct change is to stop
  writing, not to gate a write nobody needs.
- **Redirect writes to a scratch/temp dir (or `git rm` + `.gitignore`).** Rejected:
  more moving parts than deletion, and `git rm`-ing the assets would drop the
  markdown-referenced `b3-icons.png` and conflate this hygiene fix with an
  asset-curation decision that isn't ours to make here (scope creep).
- **Delete the committed PNGs outright.** Rejected: they are valid one-time
  illustrations (one is referenced by report markdown); freezing beats deleting,
  and deletion is an orthogonal curation call.

## Verification

A real wgpu adapter is required (GPU `--ignored` lane; RX 6700 XT / RADV locally,
pinned lavapipe in CI).

- **RED (pre-fix):** run a writer test → the target PNG shows modified in
  `git status`; restore with `git checkout --`.
- **GREEN (post-fix):** run all six writer tests →
  `git status --porcelain docs/reports/` stays empty; the tests still pass
  (assertions intact). Headless gate and the `buiy_verify` GPU leg unaffected.
  (The manual `git status --porcelain` check catches tracked **and** untracked;
  the CI `git diff --exit-code` guard is tracked-only by design — the two coincide
  for this lane since the tests produce no untracked report files.)
- **Guard proof:** the new CI step passes post-fix; would fail if any write
  returned.

## Doc touchpoints

- `docs/plans/follow-ups.md` — add the entry (near the stored-PNG golden machinery
  section) and mark it landed with this change.
- The **five** test files that claim it in their doc-comments — drop the "writes
  the PNG" wording (`render_gradient_paint_order_gpu.rs` makes no such claim).
- `docs/README.md` — register this spec.
- No verification-design spec change: `docs/specs/2026-06-15-buiy-verification-design/`
  governs the golden corpus, not these report assets.
