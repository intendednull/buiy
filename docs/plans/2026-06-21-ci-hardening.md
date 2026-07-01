# CI Security & Code-Quality Hardening

**Basis:** CI security + code-quality audit (2026-06-21, multi-agent: 21 findings raised,
20 confirmed, 1 rejected as a false positive). This plan implements the confirmed findings
**except** the Dependabot config (deliberately declined — see "Out of scope").

**Status:** landed — merged in PR #78 (branch protection staged as a manual follow-up).

## Why

The CI gate set is strong (fmt + clippy `-D warnings` + doc `-D warnings` + cargo-deny +
3-OS test matrix + a pinned-lavapipe GPU lane), the `pull_request` trigger is safe (no
secrets, read-only fork token), and secret scanning + push protection are on. The residual
gaps are **reproducibility / supply-chain** and **governance / ops hardening** — none are
secret-exfil holes (the token is read-only with no secrets on the safe trigger); the
realistic blast radius is non-reproducible / poisoned builds and false-green drift.

## Resolved pins (captured 2026-06-21)

| Artifact | Pin | Note |
|---|---|---|
| `actions/checkout` | `9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0` | v7.0.0 (bumped from v4) |
| `Swatinem/rust-cache` | `c19371144df3bb44fab255c43d04cbc2ab54d1c4` | v2.9.1 |
| `taiki-e/install-action` | `9e1e5806d4a4822de933115878265be9aaa786d9` | v2.82.2 |
| `dtolnay/rust-toolchain` | `67ef31d5b988238dd797d409d6f9574278e20537` | master @ 2026-06-20; needs `with: toolchain:` |
| `wgpu-info` | `29.0.3` | not in install-action → pinned `cargo install` |
| Mesa tarball (24.3.4/build20) | sha256 `8e6b565703f856c8aaf654cee18ade38fcb8c032b2d7dd609a9df6411e5aed0d` | verify before extract |

## Slices

Each slice maps to a confirmed audit finding.

1. **Commit `Cargo.lock` + `--locked` everywhere** (High). Un-ignore `Cargo.lock`,
   `cargo generate-lockfile`, commit it, add `--locked` to every workspace cargo step
   (clippy, doc, deny, test, gpu, msrv). Makes builds reproducible and pins the exact
   graph cargo-deny audits and tests run against.
2. **SHA-pin every action** (High/Medium). Replace mutable tag/branch refs with full-SHA
   pins + version comments. `dtolnay/rust-toolchain@stable` (a mutable *branch*) → master
   SHA with explicit `with: toolchain: stable` (or `1.95` for the MSRV job, forced past the
   root `rust-toolchain.toml` stable pin via `RUSTUP_TOOLCHAIN`). Bump checkout v4 → v7 while
   pinning.
3. **Verify the Mesa tarball by SHA256** (Medium). Add a `sha256` input to the
   `install-mesa` composite action and a `sha256sum -c` gate between download and extract.
4. **Branch protection on `main`** (Medium) — repo setting, *not* in the PR diff. Required
   status checks (the CI jobs) + block force-push/delete + linear history. Applied via
   `gh` **after** this PR merges (so the new job names exist and have run). Commands in the
   PR body.
5. **`concurrency` + `timeout-minutes`** (Medium). Top-level concurrency group cancelling
   superseded PR runs (never main); per-job timeouts so a hung job doesn't ride the 6h
   ceiling.
6. **Optional-feature compile coverage** (Medium). CI only ever builds default features, so
   the `default_font`-off and `clipboard-image`-on paths of `buiy_core` are uncompiled (a
   cfg typo / unused import would pass even `clippy -D warnings`). Add `--no-default-features`
   and `--all-features` clippy steps for `buiy_core` to the lint job.
7. **MSRV job** (Low). `rust-version = 1.85` was declared but never built — and is wrong.
   Verification showed the workspace does **not** build on 1.85 or 1.87: bevy 0.19 /
   bevy_ecs declare `rust-version 1.95`, which dominates the graph (next-highest:
   bevy_math/bevy_input_focus 1.94, cosmic-text/smol_str 1.89, image 1.88). The `1.85` was
   stale from before the bevy 0.18→0.19 bump. **Corrected `rust-version` to `1.95`** (the
   real, CI-verified floor) and added a `cargo check --workspace --locked` `msrv` job
   pinned to it. Note: 1.95 == current stable today, so the job is ≈ the stable build now;
   its standing value is enforcing the declared floor and catching the next dep-driven
   floor bump deliberately.
8. **`SECURITY.md` + enable private vulnerability reporting** (Low). Coordinated-disclosure
   path for a public library. PVR enabled via `gh` (repo setting).
9. **Pin `wgpu-info`** (Low). Replace unpinned `cargo install --locked wgpu-info` with a
   version-pinned install.
10. **Explicit `permissions: contents: read`** (Low). Defense-in-depth — least privilege
    in-workflow, not just relying on the mutable repo default.
11. **De-duplicate the "Free disk space" steps** (Low). Two drifted hand-rolled `rm -rf`
    blocks → one `./.github/actions/free-disk-space` composite action.
12. **`CODEOWNERS`** (Low). Added in tandem with branch protection so supply-chain config
    (`/.github/`, `deny.toml`, `Cargo.*`) always gets owner review.

## Out of scope (deliberate)

- **Dependabot / Renovate** — declined by the maintainer. Consequence: the SHA pins above
  will not auto-update and need **periodic manual bumps** (checkout/rust-cache/install-action
  releases, the rust-toolchain master SHA, the Mesa pin, wgpu-info). Noted in the PR.
- **`step-security/harden-runner`** — audit rated Info; low payoff with no secrets + a
  read-only token. Not adopting.
- The deny.toml `RUSTSEC-2024-0436` (`paste`) ignore is **load-bearing and correct** (a
  rejected finding wrongly called it redundant); left untouched.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo clippy -p buiy_core --no-default-features --all-targets --locked -- -D warnings`
- `cargo clippy -p buiy_core --all-features --all-targets --locked -- -D warnings`
- `cargo doc --workspace --no-deps --locked` (RUSTDOCFLAGS=-D warnings)
- `cargo deny check` (against the committed lock)
- `cargo +1.95 check --workspace --locked` (verified: builds clean; 1.85 and 1.87 fail)
- `actionlint` on the workflow; YAML well-formedness.
- Adversarial diff review (fresh-context agents): SHA pins valid, `--locked` coverage
  complete, no dangling action references, `with: toolchain:` present on every pinned
  rust-toolchain use, Mesa checksum wired.
- The PR's own CI runs the full 3-OS matrix + the GPU lavapipe lane.
