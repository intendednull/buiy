**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_flair — governance + distribution: single-maintainer, no commercial backing, dual-license, bus factor of 1

# Governance & distribution

## Maintenance

bevy_flair is a one-person project. Verified:

- **Sole `published_by`** on every crates.io release from 0.1.0 (2025-01-24) through 0.7.0 (2026-02-03): GitHub user `eckz`, ID 273043 (`Erick Z`).
- **GitHub profile:** [@eckz](https://github.com/eckz) — 3 followers, 10 repos, no bio, no employer / location listed.
- **No co-maintainers** visible in commit history, issue triage, or crate ownership.
- **No sponsoring company / foundation backing.** Not in the Bevy organization. Not in the Linux Foundation, CNCF, or Rust Foundation ecosystems. No GitHub Sponsors / Open Collective / Patreon links visible.

**Bus factor: 1.** If eckz stops working on bevy_flair, there is no fallback maintainer, no organization owning the crate name on crates.io, and no foundation positioned to fork. The closest analog in the Bevy ecosystem with comparable bus factor would be a single-developer side project — not a load-bearing dependency. Three GitHub followers is itself a notable signal: bevy_flair's 130 stars come from drive-by interest, not from a sustaining community of contributors.

For Buiy: **do not adopt bevy_flair as a load-bearing dependency.** If the design lessons motivate Buiy's own stylesheet layer, write that layer in-tree against Buiy's own components — don't take a hard dependency on `bevy_flair_core` / `bevy_flair_style` / `bevy_flair_css_parser`. The reflection-bridge and the cascade engine are vendorable concepts; the crate itself is too thin a bus factor to bake against. See [`lessons.md`](lessons.md) "Avoid" row on dependency posture.

## Cadence policy

No formal RFC process, no policy doc. The implicit policy from history:

- Tracks Bevy minor releases (~3-month cadence). Each Bevy minor triggers a bevy_flair minor.
- Bug-fix releases happen ad hoc within a Bevy minor (0.4.0 → 0.4.1, 0.5.0 → 0.5.1).
- No deprecation policy on user-facing API (the `NodeStyleSheet` → `Styled` rename in 0.8 is a hard break, not deprecated-then-removed).

## License

**MIT OR Apache-2.0** (dual). Verified per-version on crates.io. The README header line says `MIT` alone, but the workspace `Cargo.toml` declares `MIT OR Apache-2.0` consistently from 0.1 forward. Treat the dual-license declaration as authoritative; the README is a documentation lag.

This license matches:
- Bevy itself (Apache-2.0 OR MIT)
- bevy_ui, Taffy, cosmic-text, AccessKit
- The Buiy commitment ([architecture.md § 2.9](../../specs/2026-05-07-buiy-foundation/architecture.md#29-compatibility--policy) implies same license as Bevy).

So license compatibility is not a barrier to studying / borrowing patterns from bevy_flair. The barrier is the bus factor + the API coupling to bevy_ui (not Buiy) component types.

## Distribution

- **crates.io only.** No alternate registry. `bevy_flair`, `bevy_flair_core`, `bevy_flair_style`, `bevy_flair_css_parser` are all published as separate crates.
- **No docs.rs / mdBook custom site.** Documentation lives in the README, the CHANGELOG, the `examples/` directory, and inline rustdoc. docs.rs build is presumed working (no manual override visible) but not extensively prose-documented.
- **No public Discord / forum.** Discussions happen on GitHub Issues + PRs. The repo has ~28 closed issues + handful open as of mid-2026.
- **Cargo features:**
  - `default = []` (no default features).
  - `experimental_ghost_nodes` — forwards to `bevy_flair_style/experimental_ghost_nodes`, opting into Bevy 0.18's `GhostNode` support inside the styling pipeline. 0.6 added basic `GhostNode` support unconditionally; the experimental feature gates further integration.

## Versioning

Semver: pre-1.0 (still on 0.x). Each minor bump is potentially breaking. The 0.8 `Styled` rename is a representative example — pre-1.0 affords renames without deprecation.

No published roadmap to 1.0. Comparing to bevy_ui's pre-1.0 trajectory (Bevy 0.18 ↔ bevy_ui 0.18, still on 0.x): the Bevy ecosystem culturally treats 0.x as "production-usable but unstable," and bevy_flair fits the same posture.

## Trademark / branding

No project logo, no trademark filings, no governance org. The crate name `bevy_flair` follows the `bevy_*` ecosystem naming convention but is **not** endorsed by the Bevy organization. Bevy's ecosystem-naming-policy permits this (community crates can use `bevy_*`), but adoption of the prefix does not imply Bevy-team sponsorship.

## Implications for Buiy

| Risk | Mitigation |
|---|---|
| Bus factor 1 — eckz steps away, crate freezes against Bevy 0.18, can't track 0.19+. | Don't take a hard dependency. Treat bevy_flair as a *design reference*, not a runtime dependency. |
| API churn pre-1.0 — major renames between 0.7 and 0.8 (`NodeStyleSheet` → `Styled`). | Pin to a specific version, or vendor needed concepts into Buiy. |
| Single-author bandwidth — fixes for novel use cases (forced-colors, container queries, prefers-contrast) may not land. | If Buiy needs these, implement them in Buiy's own stylesheet layer, not by upstreaming requests to bevy_flair. |
| No formal cascade-correctness audit — the Servo `selectors` crate is browser-tested, but the bevy_flair *bridge* over it is not independently verified. | Buiy verification harness (foundation [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) would test any Buiy stylesheet layer independently. |
| License compatibility | Not a concern — dual MIT/Apache-2.0 matches the rest of the Bevy stack. |

## Sources

- bevy_flair crate ownership — https://crates.io/crates/bevy_flair (owners endpoint)
- bevy_flair sub-crates — https://crates.io/crates/bevy_flair_core, https://crates.io/crates/bevy_flair_style, https://crates.io/crates/bevy_flair_css_parser
- eckz GitHub profile — https://github.com/eckz
- bevy_flair Cargo features — `Cargo.toml` on `main` https://github.com/eckz/bevy_flair/blob/main/Cargo.toml
- Bus-factor framing pattern — Buiy `bevy-ui` prior-art [`../bevy-ui/governance.md`](../bevy-ui/governance.md) for contrast
- Sibling: [`critiques.md`](critiques.md), [`lessons.md`](lessons.md)
