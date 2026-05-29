**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — license, publishing, multi-author team, funding model, bus factor

# Distribution and governance

## Licensing

Makepad ships under permissive licensing — and there's a **discrepancy** worth pinning between two authoritative sources:

| Source | License | Note |
|---|---|---|
| **crates.io** (every version of `makepad-widgets`, 0.3.0 through 1.0.0) | **`MIT OR Apache-2.0`** | Dual-permissive. Matches Bevy's licensing model. Verified per-version via the crates.io API metadata. |
| **GitHub repo** (`license.spdx_id` per GitHub API) | **`MIT`** | Single MIT. GitHub's license detection ran against the repo's LICENSE-or-equivalent file and resolved to MIT only. |

The README at the repo root does not state the license in plain prose. The presence of the dual license in crates metadata across **all 7 published versions** is the canonical claim — Makepad is treated as `MIT OR Apache-2.0` by consumers (crates resolved this way during `cargo build`). The GitHub-side `MIT`-only classification is likely an artifact of how the LICENSE file is named or formatted versus GitHub's classifier expectations.

**Implication:** Consumers should rely on the crates.io metadata (`MIT OR Apache-2.0`) for compliance purposes. This is **fully compatible with Buiy's MIT/Apache-2.0 dual licensing**. Contrast with Slint's GPL-3 + commercial gate; Makepad has no commercial license track and no royalty terms.

This is one place where Makepad is *more* friendly to Buiy's preferred audience than Slint is. The dual-permissive license means a downstream Bevy / Buiy project could in principle vendor or learn from Makepad code without procurement-level licensing concerns.

## Crates published under the Makepad organization

Inferred from the `makepad-widgets` dependency graph and the repo's `Cargo.toml` workspace listings:

| Crate | Role |
|---|---|
| `makepad-widgets` | Public API + widget catalog. The 1.0.0 stability marker. |
| `makepad-platform` | Runtime, windowing, GPU backends, event loop. |
| `makepad-live-compiler` | Live DSL parser + expander + LiveRegistry. |
| `makepad-live-tokenizer` | Live DSL tokenizer. |
| `makepad-derive-live` | `#[derive(Live)]` and related macros. |
| `makepad-shader-compiler` | Inline-GLSL → MSL / HLSL / GLSL ES code generation. |
| `makepad-math` | vec2/vec3/vec4/mat4 math primitives. |
| `makepad-html` | HTML / markdown rendering subset. |
| `makepad-studio` | The Makepad Studio IDE app. |
| `cargo-makepad` | Cargo subcommand for cross-target tooling. |
| `makepad-zune-*`, `makepad-rusttype`, etc. | Vendored / forked dependencies for the platform layer. |

All crates publish under the single `makepaddev` crates.io account (a shared bot / project account, not a personal account). This is the right shape for a multi-author project — no individual maintainer is the single publisher of record.

## Maintainers and team

The de facto core team (from public sources):

- **Rik Arends** (`rikarends` on X/Twitter) — lead architect, public face, founder. Author of pre-Makepad live-coding work (Cloud9 IDE; AWS Cloud9 lineage). Based in the Netherlands.
- **Eddy Bruël** (Dutch spelling: `Bruël`; GitHub `eddybruel`) — co-architect. Mozilla / SpiderMonkey background.
- **Sebastian Michailidis** (Germany; GitHub `okapii`; Twitter `@SebMichailidis`, Bluesky) — co-architect. **#1 by raw GitHub contributions on the current `dev` branch** (154 commits).

Contributors beyond the core team are mostly from the **Project Robius downstream community**:

- **Kevin Boos** (`kevinaboos`, Principal Architect at Futurewei Technologies) — 104 commits. Robius lead. PhD Rice University; creator of Theseus OS.
- **offline-ant** (47 commits), **yulnr** (34 commits), **wheregmis** (21 commits) — additional contributors.

The shape: **3 core architects + a community-driven contributor pool, much of which is Robius-affiliated**. No foundation, no charter, no formal RFC process. Funding model is unclear from public sources — possibly Arends self-funded, possibly mixed with Robius / Futurewei collaboration.

## Bus factor

Concentrated. The de facto core team is three people; remove any one and the project's progress is materially impacted. There is no governance document, no published bus-factor mitigation, no acknowledged succession plan. Contrast with:

- **Bevy** — formally-organized Bevy Foundation, multiple paid contributors, public governance.
- **Slint** — SixtyFPS GmbH (a legal entity); commercial license revenue funds full-time engineers; still single-vendor but legally formalized.
- **AccessKit** — single-architect-with-NLnet-funding model. Comparable bus factor concentration.

Makepad's bus factor is closer to AccessKit's than to Bevy's or Slint's — a small team with deep competence, no formal foundation, depending on continuous founder commitment.

## Funding model (inferred)

Public sources don't enumerate a funding model. Inferences:

- **Rik Arends's involvement.** Self-funded or supported via consulting; no public sponsor disclosed.
- **Eddy Bruël and Sebastian Michailidis.** Unclear; full-time commitment level not publicly disclosed.
- **Project Robius / Kevin Boos.** Funded via **Futurewei Technologies** (a US-incorporated Huawei subsidiary). Robius is described as a "fully open-source, decentralized, community-driven" project; the practical maintenance heft is Futurewei-employed Kevin Boos's commits. Robius-funded contributions accrue to Makepad indirectly when they require Makepad-core changes.
- **No GitHub Sponsors or OpenCollective backing** publicly visible on the `makepad/` org.

The Futurewei angle is worth flagging: Futurewei is the US arm of Huawei, with the geopolitical tail that implies. Robius's open-source commitment is real, but the funding chain's terminus is a single corporate sponsor. This is *not* an issue for Makepad itself (which is independently maintained by Arends et al.), but it's a structural risk for Project Robius adoption — if Futurewei pulled the funding tomorrow, Robrix's primary maintainer would lose paid-time-on-project.

## Governance comparison

| Aspect | Makepad | Slint | AccessKit | Bevy |
|---|---|---|---|---|
| Legal entity | None (informal) | SixtyFPS GmbH | None (informal, NLnet-grant-funded) | Bevy Foundation (since 2024) |
| License | MIT OR Apache-2.0 | GPL-3 OR royalty-free OR commercial | MIT OR Apache-2.0 | MIT OR Apache-2.0 |
| Funding | Self-funded (inferred); Futurewei via Robius | Commercial license revenue | NLnet grants + Pneuma Solutions | Bevy Foundation + Sponsors |
| RFC process | None | None (single-vendor decisions) | None (single-architect decisions) | Yes (formal Bevy RFCs) |
| Bus factor | 3 core architects | SixtyFPS GmbH employees (small) | 1 lead (Matt Campbell) | Distributed (multiple paid maintainers) |
| Foundation | No | No | No | Yes (Bevy Foundation) |

Buiy's commitment (per [foundation README](../../specs/2026-05-07-buiy-foundation/README.md)): MIT/Apache-2.0 dual-permissive, Bevy-Foundation-adjacent, community-driven. Closer to Bevy than to Makepad in governance shape.

## Platform-support matrix (distribution view)

| Target | Status | Tooling |
|---|---|---|
| macOS (Metal, aarch64 / x86_64) | Stable | `cargo build` |
| Windows (DX11, x86_64) | Stable | `cargo build` |
| Linux (OpenGL, aarch64 / x86_64) | Stable | `cargo build` + system deps in `tools/linux_deps.sh` |
| Web (WebGL, WASM) | Stable | `cargo makepad wasm build` |
| iOS / iPadOS | Beta-stable (Robrix ships) | `cargo makepad ios install-toolchain` + `run` |
| Android | Beta-stable (Robrix ships) | `cargo makepad android install-toolchain` + `run` |
| tvOS | Experimental | `cargo makepad apple-tv install-toolchain` |
| OpenHarmony | Builds, doesn't run | `cargo makepad openharmony install-toolchain` |
| Embedded (MCU) | Not in scope | — |
| WebGPU (vs WebGL) | Not yet | — |

Compare with Slint's matrix: Slint adds bare-metal MCU (RGB565 framebuffer) and explicit MCU-grade software-rendering paths but lacks tvOS / OpenHarmony. Makepad's matrix is the closest existing-art for Buiy's desktop + mobile + web ambition.

## Implications for Buiy

- **License compatibility is the easy win.** Makepad's `MIT OR Apache-2.0` matches Buiy's expected licensing exactly. No procurement gate, no commercial-track friction. (Contrast Slint.)
- **Foundation-style governance is the right structural choice.** Buiy targeting Bevy-Foundation alignment is the structurally lower-risk path than Makepad's informal-core-team model. The cost is slower decision-making; the benefit is bus-factor distribution.
- **Single-publisher account is fine.** Makepad's `makepaddev` shared bot account is a clean pattern for multi-author projects. Buiy can follow the same shape.
- **Beware downstream-funded-by-single-corporation patterns.** Project Robius's Futurewei funding is the kind of "single sponsor de facto runs the community" arrangement Buiy should structure away from. Multiple Buiy-app contributors from multiple companies is a better long-term shape than one Futurewei-equivalent.

## Sources

- crates.io API (license per version): https://crates.io/api/v1/crates/makepad-widgets
- GitHub API (repo metadata): https://api.github.com/repos/makepad/makepad
- GitHub contributors: https://api.github.com/repos/makepad/makepad/contributors
- Rik Arends X profile: https://x.com/rikarends
- Eddy Bruël GitHub: https://github.com/eddybruel
- Sebastian Michailidis GitHub: https://github.com/okapii
- Kevin Boos GitHub: https://github.com/kevinaboos
- Project Robius: https://github.com/project-robius
- Robrix conference talks: Rust China Conf 2025, GOSIM Europe / China 2024 (per Robrix README)
- Sibling files: [`history.md`](history.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`open-problems.md`](open-problems.md)
- Slint comparison: [`../slint/governance-and-distribution.md`](../slint/governance-and-distribution.md)
- AccessKit comparison: [`../accesskit/governance.md`](../accesskit/governance.md)
