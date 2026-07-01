# Contributing to Buiy

> ⚠️ Buiy is an experimental, largely-unreviewed project (see the note at the top of
> [`README.md`](README.md)). It is pre-0.1 and APIs break in any commit.

Thanks for your interest. This is the human-facing front door; the detailed,
always-current engineering guide is [`CLAUDE.md`](CLAUDE.md) — it is written for automated
contributors but every command in it applies to humans too.

## Getting oriented

- **What Buiy is and how to use it:** [`README.md`](README.md) → the [getting-started
  guide](docs/guide/getting-started.md).
- **Architecture, design decisions, and prior art:** [`docs/README.md`](docs/README.md) — the
  master index of specs (target state), plans (migrations), reports (audits), and prior-art.
- **Dev commands and conventions:** [`CLAUDE.md`](CLAUDE.md) (§ Build & Test, § Code Conventions).

## Building & testing

The canonical "run all checks" command (mirrors CI) lives in
[`CLAUDE.md` § Build & Test](CLAUDE.md#build--test). In short, from the workspace root:

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets --locked -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked && \
  xvfb-run -a cargo test --workspace --locked
```

Drop `xvfb-run -a` on macOS / Windows. `--locked` is load-bearing (CI commits `Cargo.lock`
and runs every step locked). CI actually runs the tests through `cargo nextest` with
`--unreferenced=reject` (an orphaned snapshot fails CI), so if you touch snapshots, run
nextest locally too.

**The GPU lane is additive and required.** The render/verify GPU paths are `#[ignore]`-gated
and need a real wgpu adapter (or `lavapipe`); the headless gate above must stay green *without*
one. Both legs run on a GPU host:

```sh
cargo test -p buiy_core   -j 2 -- --ignored --test-threads=1
cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1
```

Before bumping any dependency, run `cargo deny check`.

## Workflow

- **Branch from the latest `origin/main`.** `git fetch` first; cut your branch from
  `origin/main`, not a stale local `main`.
- **Open a PR and wait for green CI.** CI runs the headless gate on 3 OSes, the GPU lane
  (pinned `lavapipe`), an MSRV job (Rust 1.95), a web-smoke lane, and `cargo deny`. Don't
  self-merge; squash-merge once approved.
- **Docs ship with the change.** If your change advances or alters the target state, update the
  relevant spec/plan/report and the [`docs/README.md`](docs/README.md) index in the same PR.
  See the `organizing-buiy-docs` conventions ([docs/specs/2026-05-07-docs-organization-design.md](docs/specs/2026-05-07-docs-organization-design.md)).
- **Tests at the lowest tier that observes the behavior.** For visual/layout/render/a11y tests,
  follow the verification how-to (the `using-buiy-verification` skill; mirrors
  [docs/specs/2026-06-15-buiy-verification-design/](docs/specs/2026-06-15-buiy-verification-design/README.md)).

## Reporting issues

Include the platform, whether a GPU adapter was present, and the exact command + output. For
security-sensitive reports, see [`.github/SECURITY.md`](.github/SECURITY.md).

## License

By contributing you agree your contributions are dual-licensed under
[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at the user's option.
