**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — distribution, licensing, governance, single-maintainer model, bus factor

# Distribution and governance

RmlUi is the **clean open-source counterpart** to the proprietary commercial UI middleware market (NoesisGUI, Coherent Gameface, Scaleform-of-old). It ships under a single permissive license, no foundation, no corporate steward, no formal RFC process — a single primary maintainer with community PRs. This file documents the distribution + governance shape and surfaces the **bus-factor** as the project's structural risk.

## License

- **MIT** — single permissive license, no Apache-2.0 alternative (compare: Buiy, Bevy, AccessKit ship dual MIT/Apache-2.0 for patent-grant coverage).
- License header preserves libRocket's copyright (`Copyright 2008-2014 CodePoint Ltd, Shift Technology Ltd, and contributors`) **plus** RmlUi additions (`Copyright 2019-present The RmlUi Team`).
- **No CLA, no DCO formally documented.** Contributors retain copyright on their additions per standard open-source practice.
- **Commercial usage is unrestricted.** No royalty, no per-platform fee, no revenue threshold (compare: NoesisGUI's tiered indie / pro / premium licensing). This is RmlUi's single largest commercial advantage over proprietary cousins.

## Distribution

### Source

Primary distribution channel is **GitHub source releases** (`mikke89/RmlUi`). Tagged releases include a tarball + zip; users typically check out a tag or pin to a commit in their build system.

### No package-manager presence

- **Not on Vcpkg, Conan, or Hunter as a first-party-maintained package.** Third-party Vcpkg / Conan recipes exist but are community-maintained.
- **Not on Homebrew.**
- **No official Debian / Ubuntu / Arch / Fedora package.** Distro packagers maintain their own ports.
- **No crates.io / pip / npm presence** (irrelevant — C++ library — but worth noting the asymmetry vs Rust UI libraries Buiy compares with).

### Build system

CMake is the canonical build system. The `CMakeLists.txt` provides options to select:

- Reference backend (GL2 / GL3 / Vulkan / DX12 / SDL / SFML / GLFW / Win32 / X11).
- Optional integrations (FreeType — required; HarfBuzz — sample-only opt-in; Lottie via `rlottie`; SVG via `lunasvg`).
- Sample suite enable/disable.
- Build flavor (static / shared library).

The build system is well-documented and unsurprising for a 2008-era C++ project. No Bazel, no Buck, no Meson.

## Platform matrix

Verified shipping platforms across the RmlUi era:

| Platform | Status |
|---|---|
| Windows | First-class, all backends supported |
| Linux | First-class, X11 and SDL backends |
| macOS | Supported; GLFW / SDL / Vulkan(MoltenVK) backends |
| Android | Supported; embedder provides the integration |
| iOS | Supported; embedder provides the integration |
| Nintendo Switch | Documented as shipping (production users) |
| Web / WASM | Not officially supported as a target |
| PlayStation 4 / 5, Xbox One / Series X|S | Not officially documented; commercial users likely roll their own |

The platform breadth is **wider than Buiy's v1 commitment** (Windows / macOS / Linux desktop) but narrower than NoesisGUI's (which adds UWP, PS4/5, Xbox, Switch, WebGL as first-class). This is a fair indicator of the resource asymmetry — RmlUi has one maintainer; NoesisGUI has a small team funded by commercial licensing.

## Cargo / build features (analogue)

CMake options the integrator typically chooses:

- `RMLUI_FONT_ENGINE` (default: `freetype`; alternative: custom or HarfBuzz sample).
- `RMLUI_SVG_PLUGIN`, `RMLUI_LOTTIE_PLUGIN` — optional integrations.
- `BUILD_SAMPLES` — opt-in the reference backends + sample applications.
- `BUILD_TESTING` — opt-in the test suite.

## Governance

### Single primary maintainer

**Michael Ragazzon** (`mikke89`) is the single primary maintainer. He has been the de facto release manager since 2.0 (2019-10-13). All releases are tagged by him. Major architectural decisions (Flexbox in 5.0, render interface redesign in 6.0) are designed and led by him.

### Contributor pattern

The project accepts community PRs and has accumulated a contributor list over the RmlUi era. The contribution shape is **incremental fixes + occasional feature additions** rather than a co-maintainer model. There is no second person with named release-management responsibility.

### No foundation, no corporate steward

- No formal foundation analog to Bevy Foundation / Linux Foundation / CNCF.
- No corporate steward analog to System76 (cosmic-text), Pneuma Solutions (AccessKit), Linebender (Vello / Parley).
- No documented RFC process, design-discussions forum (Discord exists but design-state is not durably captured), or governance bylaws.

### Decision-making

Effectively *benevolent-dictator* — Michael Ragazzon decides. Community PRs go through standard code review on GitHub. Major redesigns (Flexbox, render interface) appear to be planned in private + landed as large PRs rather than RFC'd in public.

## Bus factor

**Bus factor 1.** RmlUi's continued maintenance depends materially on Michael Ragazzon's continued availability. There is no documented succession plan. If `mikke89` stops working on the project, RmlUi enters the same dormancy phase libRocket entered after 2014.

Risk indicators:

- **Maintenance cadence is slowing.** 6.0 (Aug 2024) → 6.1 (Apr 2025): 8 months. 6.1 → 6.2 (Jan 2026): 9 months. Not alarming but consistent with a single maintainer reducing throughput over time.
- **No public roadmap** beyond the changelog.
- **No funded contributor outside `mikke89`.** Community contributions are volunteer.

Compare:

- **Bevy** has the Bevy Foundation (501(c)(3) public charity), 5 board members, multiple paid SMEs, regular SME hires.
- **AccessKit** has Pneuma Solutions sponsorship.
- **NoesisGUI** has Noesis Technologies as a small commercial company with paid staff.
- **RmlUi** has neither corporate steward nor foundation; survives on `mikke89`'s personal commitment.

This is the single largest risk factor for any project considering RmlUi as a strategic dependency. The project has been remarkably durable through 8 years of mikke89's stewardship — but the structural risk is real and unmitigated.

## Versioning + release notes

Semantic versioning by intent. Major releases (2.0 → 3.0 → 4.0 → 5.0 → 6.0) carry breaking changes; minor releases (`X.1`, `X.2`) are additive. The `changelog.md` file in the repo is the canonical release-notes record, updated per release; there are no separate release-notes blog posts.

**Breaking changes that consumers must handle:**

- 2.0 → 3.0: C++14 baseline (was C++11), significant API cleanup.
- 5.x → 6.0: render interface redesign — every embedder must port their `RenderInterface` implementation.
- Minor versions: source compatibility preserved within a major.

## Build / packaging ecosystem outside the repo

- **Unreal Engine plugins**: third-party plugins integrate RmlUi into UE projects (e.g., `Mind-Overflow/RmlUiUnrealPlugin`); none are first-party.
- **Unity plugins**: third-party plugins exist; less common than the UE side.
- **Godot bindings**: community attempts exist; not officially maintained.
- **Engine-specific forks**: several large studios maintain internal forks (Cfx.re for FiveM, Nightdive Studios for their KEX engine).

## Funding

- **No corporate sponsorship publicly documented** for RmlUi or `mikke89` directly.
- **No GitHub Sponsors / Patreon / OpenCollective** prominently linked from the repo (verify per-visit).
- **No paid tier** of the library (compare: NoesisGUI's pro / premium).

This is consistent with the "open-source side project + community contributions" model. It is the model that produces both RmlUi's strengths (no commercial-licensing friction, full source access, free to use commercially) and its weaknesses (bus factor 1, slow feature pace, no funded a11y / platform / spec-conformance work).

## Implications for Buiy

- **Permissive license + commercial-unrestricted-use is the open-source competitive position.** Buiy (MIT OR Apache-2.0 dual) shares this and gains the additional patent-grant coverage of Apache-2.0. RmlUi's MIT-only is mildly inferior on patent grant but functionally equivalent for commercial users.
- **Single-maintainer bus factor is real.** Buiy is positioned within Bevy ecosystem (Bevy Foundation, multi-maintainer governance); the lesson is to **name** the bus-factor explicitly. The RmlUi single-maintainer model has produced 8 years of durable stewardship — but the structural risk is the project's largest threat.
- **No foundation / corporate steward shapes the contribution flow.** RmlUi accepts community PRs but does not have funded SMEs working on a11y, platform support, or spec conformance. Buiy's positioning within Bevy Foundation + the Bevy contributor ecosystem materially improves this.
- **Cargo features parallel:** RmlUi's CMake options (font engine, SVG, Lottie, samples) are the equivalent of Buiy's foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.8 crate split. The pattern of "optional features behind build flags, with sensible defaults" is shared and validated.
- **Platform breadth requires resources.** RmlUi ships Windows / macOS / Linux / Android / iOS / Switch on one maintainer's effort. Buiy's v1 commitment is desktop-only (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.9 staged platform support). The data is consistent: more platforms = more work; pick what you can support, then expand.
- **MIT-only vs MIT/Apache-2.0**: minor difference but worth noting. RmlUi's MIT-only is fine for permissive-license consumers but Apache-2.0's explicit patent grant is the marginal upgrade. Buiy's dual MIT/Apache-2.0 is the contemporary Rust default.
- **No CLA, no DCO** matches Buiy's likely default (no CLA, contributor copyright). The lesson: this is the conventional open-source position for permissive-licensed UI libraries; no need to over-engineer.

## Sources

- RmlUi GitHub repository (license, readme) — https://github.com/mikke89/RmlUi
- RmlUi releases (cadence) — https://github.com/mikke89/RmlUi/releases
- RmlUi changelog — https://github.com/mikke89/RmlUi/blob/master/changelog.md
- libRocket GitHub repository (copyright lineage) — https://github.com/libRocket/libRocket
- NoesisGUI prior-art (commercial-licensing contrast) — [`../noesisgui/`](../noesisgui/)
- bevy_ui prior-art (Bevy Foundation contrast) — [`../bevy-ui/governance.md`](../bevy-ui/governance.md)
- AccessKit prior-art (Pneuma Solutions stewardship contrast) — [`../accesskit/`](../accesskit/)
- Buiy foundation architecture (crate split, platform support) — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
