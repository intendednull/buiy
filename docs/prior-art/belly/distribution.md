**Date:** 2026-05-22
**Status:** active
**Subject:** belly — distribution channel, Bevy version compatibility, license, governance

# Distribution and governance

## Distribution channel — git-only

belly is **not published to crates.io**. The only supported way to depend on it is via Cargo's git-dep syntax:

```toml
[dependencies]
belly = { git = "https://github.com/jkb0o/belly", tag = "v0.5.0" }
```

The README's authoritative wording: `"As far as the project has no cargo release yet, the only way to discover all the features it has is to clone the repo and check out the examples."`

Verified at write-time (2026-05-22):

```
$ curl https://crates.io/api/v1/crates/belly
{"errors":[{"detail":"crate `belly` does not exist"}]}
```

The name `belly` is not registered to any other project on crates.io either — the slot is free, but unused.

### Consequences of git-only distribution

1. **No semver guarantees enforced by Cargo.** Cargo's resolver applies semver constraints only to crates.io versions. Git refs are opaque — Cargo treats `tag = "v0.5.0"` and `rev = "abc1234"` as pinpoint pins, no `^0.5` resolution.

2. **Diamond-dependency hazard.** If two consumers in the same workspace depend on belly via different git refs, Cargo errors. crates.io publishing solves this through version unification; git pinning does not.

3. **No transitive depend-on-belly path.** Cargo blocks publishing a crates.io crate that has any git-only dep. Anything wanting to publish *cannot* take belly as a hard dep. This excludes belly from being a foundation for other Bevy ecosystem crates.

4. **No `cargo search belly`, no docs.rs.** The standard Rust discovery surfaces don't index belly.

5. **No download metrics.** crates.io tracks usage via download counts. belly's adoption is invisible — `git clone` counts aren't published. Star count (436) is the only public signal.

## Bevy version compatibility

| belly version | Bevy version | Released | Status |
|---|---|---|---|
| v0.1.1 | 0.9 | 2023-03-01 | EOL |
| v0.2.0 | 0.10 | 2023-04-01 | EOL |
| v0.4.0 | 0.12 | 2024-03-13 | EOL |
| v0.4.1 | 0.12 | 2024-03-16 | EOL |
| **v0.5.0** | **0.13** | **2024-04-20** | **Latest. ~5 majors stale.** |
| (none) | 0.14, 0.15, 0.16, 0.17, 0.18 | 2024-07 → 2026-01 | No belly support |

The Bevy version belly is pinned to (0.13, April 2024) predates several Bevy primitives Buiy depends on: Required Components (0.15+), the decomposed `BackgroundColor` / `BorderColor` / `Outline` / `BoxShadow` components (drifted through 0.14–0.17), `bevy_input_focus` (0.16+), `bevy_a11y` post-#17644 PR #24308 changes (0.17+), `bevy_feathers` (0.18+). A migration to current Bevy would touch every belly crate.

## License

Dual MIT OR Apache-2.0. Standard for the Rust + Bevy ecosystem. License files:

- `LICENSE-MIT.md` — MIT License text
- `LICENSE-APACHE.md` — Apache License 2.0 text

No Contributor License Agreement (CLA) is required. Contributions inherit the dual license.

The dual MIT/Apache-2.0 license is **fully compatible** with anything Buiy would want to do — borrow code, fork, or vendor. If the Buiy team ever needed to vendor belly code (for a specific algorithm or test fixture), the license permits it without restriction. (Buiy's current corpus uses belly only as a design reference, not vendored code.)

## Maintainer + governance

| Role | Person | Notes |
|---|---|---|
| Maintainer / owner | `jkb0o` | Single-maintainer project. Bus factor 1. |
| Last-known active contributor | `Threadzless` | Authored PR #82 (the Bevy 0.13 migration that shipped as v0.5.0). |
| Other contributors | Several minor PRs from drive-by contributors | None with merge rights. |

belly has **no governance structure**, no RFC process, no published roadmap beyond the README's "Coming soon" sections (which haven't been touched since 2024-04). All decisions flow through `jkb0o` — and as of 2024-04, no decisions are flowing.

### Bus factor 1

The maintainer's GitHub activity has been quiet since the 2024 stall. There is no co-maintainer with merge rights. If `jkb0o` does not resume, no person inside the project's history has standing to revive it. A community fork is possible (the license permits it; the codebase is reasonably small), but no widely-recognized fork exists as of 2026-05-22.

For Buiy's purposes: the bus factor of 1, combined with the no-crates.io-presence, combined with the 5-major-versions-stale Bevy pin, makes belly **infeasible as a foundation dependency**. The corpus treats it strictly as a design reference.

## Comparison to bevy_flair on distribution

| Dimension | belly | bevy_flair |
|---|---|---|
| crates.io | not published | published, actively releasing |
| Bevy pin | 0.13 (2024-04) | tracks current Bevy (0.18 era) |
| Maintainer | jkb0o | eckz (also single-maintainer, but actively releasing) |
| License | MIT OR Apache-2.0 | MIT OR Apache-2.0 |
| Repo activity | dormant since 2024-04 | actively committing |

bevy_flair wins every distribution dimension. The two prior-art folders frame the same design question (CSS-on-Bevy-UI) with starkly different artifacts: bevy_flair is "what an in-production version of this idea looks like," belly is "what an abandoned-on-stale-Bevy version of this idea looks like."

## Implications for Buiy

1. **Don't depend on belly.** Not from `Cargo.toml`, not as an extension target, not as a test fixture. Treat it as a design reference, full stop.

2. **If Buiy ever publishes extension crates, publish them to crates.io early.** belly's "no cargo release yet" stance hardened into permanent unavailability. Even `0.0.1` placeholder publishes secure the name and exercise the publication pipeline before life events stall the project.

3. **Track belly's Bevy-version stall as the central case study of the Bevy-minor-release migration tax.** Buiy commits to rolling-latest-stable; belly's history is the negative example of what happens to a single-maintainer ecosystem crate that falls one version behind. Foundation [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md) should include a release-gate that explicitly tests current-Bevy compatibility on every cut.

4. **The license is not a constraint.** Dual MIT/Apache-2.0 means Buiy can borrow patterns freely. The constraint is the *bus-factor* and the *crates.io-absence*, not the legal terms.

## Sources

- belly v0.5.0 README ("no cargo release yet") — https://github.com/jkb0o/belly/blob/v0.5.0/README.md
- belly releases — https://github.com/jkb0o/belly/releases
- crates.io 404 for `belly` — https://crates.io/api/v1/crates/belly
- belly LICENSE-MIT.md / LICENSE-APACHE.md — https://github.com/jkb0o/belly/tree/v0.5.0
- Cargo git-dep + publishing policy — https://doc.rust-lang.org/cargo/reference/publishing.html#packages-with-git-dependencies
- bevy_flair governance — [`../bevy-flair/governance.md`](../bevy-flair/governance.md)
- Buiy foundation verification — [`../../specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)
