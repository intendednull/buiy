**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — Bevy's most prominent third-party parallel UI stack; the closest existing-art neighbor to Buiy

# bevy_lunex

`bevy_lunex` is a retained UI layout engine for the Bevy game engine that runs **parallel to `bevy_ui`** rather than on top of it. It positions Bevy entities by writing their `Transform`s, depends on `bevy_sprite` + `bevy_text` + `bevy_pbr` + `bevy_picking` + `cosmic-text` directly (and optionally on `bevy_rich_text3d`), and ships with **no `bevy_ui` dependency, no `bevy_a11y` dependency, no Taffy dependency, and no AccessKit dependency**. That dependency profile makes it the single closest design-space neighbor to Buiy in the public Bevy ecosystem — both projects share the parallel-stack stance, both treat the underlying Bevy primitives as direct dependencies, and both make worldspace UI a first-class capability. They diverge sharply everywhere else: Buiy commits to Taffy, web-platform parity, and AccessKit-first; bevy_lunex chose transform-based anchored layout, "no flexbox by design," and no accessibility integration at all.

**Honest assessment.** bevy_lunex is real, durable, and the only third-party Bevy UI stack with first-class **worldspace UI** — UI panels anchored to entities in 3D space, hit-tested through the 3D scene, animated like game objects. The 3D-anchored story is its biggest single differentiator and the most directly useful prior art for Buiy's planned `buiy_3d` sub-spec. But the project carries four load-bearing weaknesses that any consumer must weigh: (1) **bus factor 1** — IDEDARY is the sole maintainer, self-identifies as a university student in the official book, and has carried two release gaps of 4–5 months around semesters; the most recent Bevy 0.17 bump was done by an external contributor (S4ndf1re, PR #122) because the maintainer was unavailable. (2) **No flagship game ships with bevy_lunex** — 2.7 years on crates.io, 40,504 downloads, 913 stars, and zero publicly-named commercial Steam or itch.io title outside Bevypunk (the maintainer's own demo). (3) **No accessibility integration whatsoever** — no AccessKit, no `bevy_a11y`, no role / label / state / relation model, no focus ring, no screen-reader path; a bevy_lunex UI structurally **cannot** conform to WCAG 2.2 at any level. (4) **The "blazingly fast" marketing claim has no published benchmarks** — no `benches/` directory, no comparison numbers against `bevy_ui` / `bevy_egui` / `sickle_ui`, no scaling figures. The retained-mode performance argument is plausible but unverified.

**Adoption math.** 40,504 lifetime downloads / 2,126 recent vs `bevy_ui`'s 4,901,387 / 943,255 — bevy_lunex is approximately **0.8%** of `bevy_ui`'s reach by download volume. It sits in the second tier of third-party Bevy UI options (ahead of `sickle_ui` and `woodpecker_ui` on raw downloads, behind `bevy_egui` by ~75×). The stars-to-downloads ratio (≈22.5) is high, consistent with "admired design, niche application." For Buiy this is informative: the parallel-stack idea attracts attention but the specific worldspace-first execution does not produce a flagship adopter.

## Key facts (verified 2026-05-22)

| Fact | Value |
|---|---|
| Crate | `bevy_lunex` (underscored) |
| Repo | `https://github.com/bytestring-net/bevy-lunex` (hyphenated) |
| License | MIT OR Apache-2.0 |
| Latest stable | **0.6.0** (published 2026-01-22) |
| Edition / MSRV | `edition = "2024"`; no `rust-version` declared — effective Rust 1.85+ |
| Lifetime downloads | 40,504 |
| 90-day downloads | 2,126 |
| GitHub stars | 913 |
| Total releases | 28 versions (0.0.1 → 0.6.0) |
| First crates.io publish | 2023-08-24 (0.0.1) |
| Maintainer | IDEDARY (`bytestring-net` org; Czechia; self-identified university student) |
| Bus factor | **1** |
| Funding | None declared — no Sponsors, no Open Collective, no commercial backing |
| External contributors with merged PRs | 1 (S4ndf1re — Bevy 0.17 bump, PR #122) |
| Last commit on `main` | 2026-02-24 (~3 months silent at time of writing) |
| Cargo features | `default = ["text3d"]`, `text3d`, `wasm` |
| Bevy deps | `bevy_app`, `bevy_ecs`, `bevy_sprite`, `bevy_text`, `bevy_pbr`, `bevy_render`, `bevy_camera`, `bevy_picking`, `bevy_winit`, `bevy_transform`, ~22 sibling crates |
| Non-Bevy deps | `cosmic-text 0.16`, `bevy_rich_text3d` (optional), `radsort`, `colored`, `rand`, `getrandom` (opt-in WASM) |
| **Notably absent deps** | `accesskit`, `bevy_a11y`, `bevy_ui`, `taffy` |
| `UiLayoutType` enum | `Window` / `Solid` / `Boundary` (3 variants total) |
| Flagship demo | **Bevypunk** (`IDEDARY/Bevypunk`, 218 stars; maintainer's own demo) |
| Commercial games shipping it | **None publicly named** |
| AccessKit integration | **None** |
| Theme system | **None** |
| Widget catalog | **None** |
| Documentation | The Lunex Book (`bytestring-net.github.io/bevy_lunex/`) + inline docs |
| Adoption vs `bevy_ui` | **~0.8%** by lifetime downloads |

## Contents

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, key facts, ToC, framing disclosure. |
| [`lessons.md`](lessons.md) | **The consult-this-when-designing decision file.** Validates / avoid / borrow. |
| [`glossary.md`](glossary.md) | bevy_lunex-specific type names and identifiers. |
| [`architecture.md`](architecture.md) | Transform-based layout model, system ordering, `UiLayoutRoot` tree, the deliberate absence of a render pipeline. |
| [`layout.md`](layout.md) | The transform-based solver, the three `UiLayoutType` variants, the `UiValue<T>` unit system, the explicit "no flexbox by design" stance. |
| [`component-model.md`](component-model.md) | Shipped components (`UiLayoutRoot`, `UiLayout`, `Dimension`, `UiColor`, state components), BSN-friendliness, comparison to bevy_ui. |
| [`styling.md`](styling.md) | The minimal styling surface (`UiColor` only), the absence of a theme system, custom-shader integration via `bevy_sprite`. |
| [`3d-and-worldspace.md`](3d-and-worldspace.md) | bevy_lunex's load-bearing differentiator — `UiRoot3d`, `UiMeshPlane3d`, anchoring to 3D entities, hit-testing through the 3D scene, render-to-texture surfaces. |
| [`history.md`](history.md) | Chronological release timeline (0.0.1 → 0.6.0), the 0.3 rewrite, the Bevy version coupling. |
| [`distribution.md`](distribution.md) | Cargo features, dependency graph, platform matrix, MSRV, release cadence, Bevy migration tax. |
| [`governance.md`](governance.md) | IDEDARY profile, bytestring-net org, funding, bus factor analysis, kayak_ui precedent. |
| [`ecosystem.md`](ecosystem.md) | Adoption numbers, showcase community, reverse-dependencies, comparative landscape vs bevy_egui / sickle_ui / woodpecker_ui. |
| [`comparisons.md`](comparisons.md) | Head-to-head against `bevy_ui`, `sickle_ui`, `woodpecker_ui`, `bevy_egui`, `kayak_ui`, and Buiy. |
| [`critiques.md`](critiques.md) | Honest critique of the "blazingly fast" marketing claim, transform-vs-Taffy tradeoffs, documentation gaps, accessibility absence, BSN-friendliness. |
| [`open-problems.md`](open-problems.md) | Structural absences — accessibility, themes, animation, drag-and-drop, text editing, mobile, WCAG SC coverage. |

## How to use this corpus

1. **Designing a Buiy feature with overlap to bevy_lunex?** Start at [`lessons.md`](lessons.md). Find the `Avoid` row that names a pitfall close to your design, or the `Borrow` entry that names a primitive worth studying.
2. **Auditing the `buiy_3d` sub-spec?** Start at [`3d-and-worldspace.md`](3d-and-worldspace.md) — this is where bevy_lunex's best prior art lives. Cross-reference with [foundation cross-cutting.md § 3.17](../../specs/2026-05-07-buiy-foundation/cross-cutting.md).
3. **Weighing transform-based vs Taffy-based layout?** Start at [`layout.md`](layout.md) and [`critiques.md`](critiques.md) § "Transform-based vs Taffy-based: the honest assessment."
4. **Considering bevy_lunex as a long-term runtime dependency?** Start at [`governance.md`](governance.md). The short answer: don't — the bus-factor-1 + kayak_ui precedent + no-funding posture make it suitable as design influence only.
5. **Tracking the Bevy version coupling?** Start at [`history.md`](history.md) and [`distribution.md`](distribution.md) § "Bevy version coupling."

## Cross-document inconsistencies surfaced

- **Crate name vs repo path.** The crate is `bevy_lunex` (underscored); the GitHub repository is `bytestring-net/bevy-lunex` (hyphenated). This is the standard Cargo / GitHub disambiguation but easy to miss.
- **MSRV.** No `rust-version` field declared in `crate/Cargo.toml`. The workspace declares `edition = "2024"`, requiring Rust 1.85+. Treat 1.85 as the effective floor; treat all MSRV bumps as silent.
- **Cargo feature shape changes between minors.** `wasm` first appeared post-0.3, `text3d` first appeared at 0.3.2 and became default-on at 0.5+. Consumers pinning to `bevy_lunex = "0.X"` cannot trust feature names to survive minor bumps. Not semver-stable by Cargo's strict reading.
- **"Bevypunk = production ready example."** The Bevypunk README frames itself as a `"production ready example"` but it is a tech demo, not a shipped product. Subtle but important distinction when reading bevy_lunex's flagship claims.
- **0.5.0 attribution.** The 0.5.0 release exists because S4ndf1re did the Bevy 0.17 bump (PR #122). The release-notes author is the maintainer, but the load-bearing work was external. This is the empirical bus-factor signal.

## Framing disclosure

This corpus is written from a **Buiy-commits-to-Taffy + web-platform-parity + WCAG 2.2 AA + AccessKit-first** stance. Most `Implications for Buiy` lines treat bevy_lunex's choices through Buiy's stance: bevy_lunex's transform-based bet becomes the case study for what *not* to do at the layout layer, its accessibility absence becomes the cautionary tale for what "we'll add it later" looks like at 2.7 years in, and its bus-factor-1 governance becomes the precedent that argues for Buiy to plan for fork-strategy contingencies.

A future reader auditing whether *transform-based parallel UI* is itself viable for a different design space — a worldspace-first / game-HUD-only / pixel-perfect-not-required project — should weigh the corpus accordingly. bevy_lunex's design bets are internally consistent and defensible for its scope. This corpus is a learn-from-bevy_lunex-into-Buiy artifact, not a neutral catalog of "is bevy_lunex good?"

A secondary disclosure: bevy_lunex is the closest *architectural posture* match to Buiy of any project examined in this prior-art corpus. There is an incentive to over-cite its design wins (parallel-stack viability, `bevy_picking` integration pattern, `Transform`-based 3D-anchored UI) and under-cite its design failures (no flexbox, no a11y, no flagship). Where this corpus is silent on a bevy_lunex risk that Buiy structurally inherits (e.g. solo-maintainer fragility, "blazingly fast" without bench data), default-assume the silence is bias and pressure-test against the Buiy verification harness ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)).

## Sources

- bevy_lunex on crates.io — https://crates.io/crates/bevy_lunex
- bevy_lunex crates.io API metadata (fetched 2026-05-22) — https://crates.io/api/v1/crates/bevy_lunex
- bevy_lunex repository — https://github.com/bytestring-net/bevy-lunex
- bevy_lunex `crate/Cargo.toml` (main) — https://raw.githubusercontent.com/bytestring-net/bevy-lunex/main/crate/Cargo.toml
- The Lunex Book — https://bytestring-net.github.io/bevy_lunex/
- docs.rs — https://docs.rs/bevy_lunex/0.6.0/bevy_lunex/
- Bevypunk (flagship demo) — https://github.com/IDEDARY/Bevypunk, https://idedary.itch.io/bevypunk
- bytestring-net org — https://github.com/bytestring-net
- IDEDARY profile — https://github.com/IDEDARY
- PR #122 (Bevy 0.17 bump by S4ndf1re) — https://github.com/bytestring-net/bevy-lunex/pull/122
- kayak_ui (archived precedent) — https://github.com/StarArawn/kayak_ui
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation cross-cutting — [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- bevy-ui prior-art lessons (cross-link) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
