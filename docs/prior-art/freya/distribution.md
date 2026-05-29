**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — license, governance, platform support

# Distribution and governance

## License

**MIT.** Single permissive license, no dual-license / commercial-tier model (unlike Slint's GPL + commercial). No CLA visible on the repo. Pure open-source.

## Governance

**Effectively single-maintainer.** Marc Espín Sanz (`marc2332`) owns:

- The GitHub repo (`marc2332/freya` — not under an organization).
- The crates.io publisher account.
- Strategic direction (no public RFC process; design discussions live in the project's Discord).
- Release cadence and rc tagging.

External contributors land PRs (visible in the repo's commit history) but no documented co-maintainer with merge / release authority. Marc is a member of the `@tauri-apps` and `@dioxus-community` GitHub organizations but **not** of `@DioxusLabs` (the company-equivalent for Dioxus).

**Funding model.** Marc has 7 GitHub Sponsors. He self-describes Freya as *"Rust projects in my spare time"* alongside web frontend developer work. There is no:

- Backing company (compare to Slint's SixtyFPS GmbH).
- VC / grant funding (compare to Dioxus's YC + Pioneer Fund seed).
- Foundation backing (compare to Iced's loose community + Linux Foundation niche).
- Commercial-license revenue stream (none — pure MIT).

This is the **purest hobby/sponsorship-funded shape** in the Rust GUI prior-art surveyed. It is also the highest single-point-of-failure shape.

## Contribution model

The repo accepts PRs through the standard GitHub flow. There is no:

- Published contributor guide beyond a basic `CONTRIBUTING.md` (lightweight).
- RFC / design-spec process (designs land via Discord + PR description).
- Documented review SLA or merge cadence guarantee.
- CLA.

In practice, drive-by PRs land; substantial architectural changes go through Marc directly.

## Versioning policy

Pre-1.0 semver, with substantial breaks between minor versions:

- `0.x → 0.y` is a breaking-change boundary. Migration notes ship in release posts.
- `0.x.y → 0.x.z` is patch-level — bug fixes and additive changes.
- `0.x.0-rc.N` is the iterating-toward-stable form. Currently `0.4.0-rc.19`.
- **No long-term support branch.** Once a minor ships, only the latest minor receives fixes.

The implication for production users: pin to an exact version (`= 0.3.4`, not `^0.3`) because patch-version bumps occasionally include behavioral changes.

## Platform support matrix

Verified from the workspace and documentation:

| Platform | Status |
|---|---|
| **Windows** | Supported (winit + Skia with GL backend) |
| **macOS** | Supported (winit + Skia with Metal backend via `metal-rs`) |
| **Linux (X11)** | Supported (winit + Skia with GL) |
| **Linux (Wayland)** | Supported (winit + Skia with GL) |
| **Android** | Not supported |
| **iOS** | Not supported |
| **WASM / web** | **Not supported** — `freya-skia-safe` does not build for WASM |
| **Bare-metal / embedded** | Not supported (Skia C++ requires a real OS allocator) |

The desktop-only focus is **deliberate**, mirrored on the official site: *"Cross-platform and non-web GUI Library."* The "cross-platform" claim is desktop-cross-platform (Win/Mac/Linux), not mobile or web.

## Bevy compatibility

**N/A.** Freya is not a Bevy library. It is a *peer* native GUI framework that does not depend on Bevy, does not coexist in a Bevy app, and has no Bevy bindings. The comparison axis for Buiy is **what would Buiy have looked like if it were not a Bevy plugin** — Freya is one possible answer.

## Distribution artifacts

- **crates.io publishes:** `freya`, `freya-core`, `freya-engine`, `freya-elements`, `freya-components`, `freya-hooks`, `freya-winit`, `torin`, `freya-devtools`, and several others — all under marc2332's publisher account.
- **Cargo features:** Each crate exposes per-feature flags; the meta-crate (`freya`) re-exports common surface. No platform-specific feature gates required (Skia builds for all desktop platforms by default).
- **Pre-built binaries:** None. All users build from source, which means **a Skia C++ build occurs on first compile** (~minutes; cached after).
- **Docker / dev-container** publishing: None.

## Release-note discipline

Marc ships release notes via GitHub releases. The notes for rc versions are typically short (one-line summaries per release); stable releases (0.3.x) get longer write-ups. There is **no per-release blog post** equivalent to the Dioxus 0.5 / 0.6 / 0.7 long-form posts ([`../dioxus/lessons.md`](../dioxus/lessons.md) Borrow #7). Freya's design-rationale archaeology requires reading commit messages + Discord, not blog posts.

## What this means for Buiy

- **MIT-only with no CLA** is the simplest model and the appropriate baseline. Buiy can follow.
- **Single-maintainer is a known bus-factor risk** — Buiy must structure governance around multiple committers from the start. Foundation does not yet commit to a governance model; this is an [open question](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions).
- **Desktop-only is a constraint Buiy explicitly does not accept.** Buiy targets Bevy's full platform matrix (foundation [non-goal § 1.3](../../specs/2026-05-07-buiy-foundation/README.md) excludes non-Bevy frontends but inherits Bevy's targets — desktop + mobile + WASM as Bevy ships them).
- **Pre-built binary / Skia-C++-on-every-build** is a UX wart Buiy avoids by being pure-Rust + wgpu.

## Sources

- Freya repo + license — https://github.com/marc2332/freya/blob/main/LICENSE
- crates.io publisher data — https://crates.io/users/marc2332
- Marc Espín GitHub — https://github.com/marc2332 (sponsor count, organization memberships)
- Cross-references: [`../slint/governance-and-distribution.md`](../slint/governance-and-distribution.md), [`../dioxus/governance.md`](../dioxus/governance.md), [`history.md`](history.md), [`critiques.md`](critiques.md).
- Buiy foundation — [`README.md § 5 Open questions`](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions).
