**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — Epic Games stewardship, EULA, royalty model, source access

# Distribution and governance

## Steward

**Epic Games, Inc.** — Cary, North Carolina; founded 1991 by Tim Sweeney. Privately held. Tencent owns ~40% (since 2012, non-controlling). Sony, KIRKBI (Lego's parent), and Disney are minority investors.

Epic single-handedly maintains Slate and UMG. There is no foundation, no committee, no formal RFC process. Direction is set internally and shipped via Unreal Engine releases. Community PRs to the Unreal Engine GitHub mirror are accepted but the bar is high and turnaround is slow — the engine code is treated as Epic's product, not a community project.

This is the opposite end of the governance spectrum from Buiy:

- **Bevy** — Bevy Foundation; SME working groups; open RFCs and discussions; PRs from the community are the primary code intake.
- **Linebender (Xilem/Masonry)** — volunteer collective; Raph Levien informally leads.
- **Slate/UMG** — single corporate steward; community contribution is exceptional, not normal.

This single-steward model is the load-bearing reason Slate has remained architecturally stable for fifteen years (one steward, one direction). It is also the reason no community-driven "Slate 2.0" has emerged.

## License — the Unreal Engine EULA

**Not** an OSI-approved open-source license. Unreal Engine is **source-available** under Epic's proprietary EULA (one of: the *Publishing License Agreement* for games / sold software, or the *Creators License Agreement* for non-game uses up to $1M revenue).

Key terms (as of [unrealengine.com/eula](https://www.unrealengine.com/eula/unreal) and [unrealengine.com/license](https://www.unrealengine.com/license)):

- **Free to use** for development, prototyping, education, internal/private projects, and any project under $1M lifetime gross revenue.
- **5% royalty** on gross revenue **above $1M lifetime** per "Royalty Product." Calculated globally, not per platform.
- **Royalty-free** for revenue from sales on the **Epic Games Store**.
- **3.5% reduced rate** under the "Launch Everywhere with Epic" program when a game ships on the Epic Games Store at the same time as (or before) other PC stores.
- **No royalty** for non-game uses up to $1M revenue under the Creators License (film, broadcast, archviz, in-house tools).
- **Source access** to the engine code via the [EpicGames/UnrealEngine](https://github.com/EpicGames/UnrealEngine) private GitHub repo — accessible only to accounts that have:
  1. Created an Epic account.
  2. Linked a GitHub username to it through Epic's developer portal.
  3. Accepted the EULA.

Source forks remain private — you cannot fork the engine and re-host it publicly. You can distribute modifications **only to other Epic licensees** of the same engine version.

### Copyleft incompatibility

A specific term that matters for code reuse: Unreal Engine code **cannot** be combined with GPL/LGPL/CC-BY-SA code. Acceptable side-by-side licenses include BSD, MIT, MS-PL, and Apache. This means a Buiy-style MIT/Apache library can technically be embedded in an Unreal project; the reverse direction (lifting Unreal code into a permissively-licensed project) is forbidden by the EULA.

For Buiy: **no Slate code is reusable.** Buiy can study Slate's shape, document its patterns, even reimplement its semantics — but it cannot lift a single line of Slate source. This is enforced by the EULA, not by copyright alone, and Epic actively enforces it.

## Royalty calculation specifics

The 5% royalty applies to "Royalty Revenue" — defined in the EULA as:

> "All worldwide gross revenue actually attributable to a Royalty Product, regardless of whether that revenue is received by you or any other person or legal entity ... from in-app purchasing, advertising, subscription fees, in-game advertising, sponsorships, payments from any source for early access, and ... transfer or sale of a Royalty Product to a third party."

Per-product, lifetime-cumulative, first $1M excluded. The royalty is owed quarterly. Failure to report is grounds for license termination plus an audit clause.

## What this means for licensing-adjacent decisions

A few load-bearing implications:

- **No fork-and-rehost.** A community "Slate 2.0" cannot legally exist outside Epic's umbrella. Even Slate's bug fixes from the community come back via PR into Epic's repo, not as a public fork.
- **No long-tail community.** Unlike Bevy (where bevy_ui, bevy_feathers, sickle_ui, kayak_ui, belly, woodpecker_ui all coexist as independent crates), Slate has essentially no ecosystem of independent reimplementations or extensions. Plugins exist (CommonUI, MVVM, third-party UMG add-ons) but always under Epic's umbrella.
- **Royalty taxes adoption.** AAA studios with revenues > $1M lifetime per title pay the royalty as a cost of doing business — Slate/UMG is competitive with the alternative (build your own) for them. Indie studios under $1M and OSS projects in adjacent spaces (Bevy, Godot, app frameworks) cannot use Slate code as a primitive without entering the royalty world. This is a structural reason Slate's design influences UI in commercial games but not in the OSS UI space.

## Comparison with Buiy

| | Slate + UMG | Buiy |
|---|---|---|
| Steward | Epic Games (single corporate) | TBD; OSS license model |
| License | Unreal EULA (source-available, royalty) | **MIT OR Apache-2.0** (dual permissive) |
| Royalty | 5% above $1M / 3.5% with Epic Games Store | None |
| Source access | Private GitHub, EULA-gated | Public, fork-friendly |
| Code reuse from this project | **Forbidden** for non-UE projects | Granted unconditionally (MIT/Apache) |
| Ecosystem | Single trunk, plugins under Epic umbrella | Community ecosystem expected |
| Governance | Internal at Epic | TBD (community + maintainer model) |

Buiy's commitment to MIT-or-Apache dual licensing is partly informed by exactly this contrast. The OSS Rust UI space has multiple precedents (egui MIT, Iced MIT, Bevy MIT-or-Apache, Floem MIT/Apache, Masonry Apache); for Buiy to be reusable as a primitive in turn, it must clear the same bar.

## CommonUI's distribution shape

CommonUI is also Epic-owned, shipping as a built-in plugin (`/Engine/Plugins/Runtime/CommonUI/`) under the same Unreal EULA. There is no standalone CommonUI distribution. Third-party "CommonUI for Bevy" or "CommonUI for Unity" reimplementations do not exist.

## Mac, Linux, and PC platform parity

Unreal's PC platform support is uneven for UI:

- **Windows** — first-class.
- **macOS** — supported but lower-priority; Epic shipped the macOS engine but Unreal Editor on macOS lags features quarterly.
- **Linux** — engine runs (Vulkan); editor builds but is uncommon. Linux a11y is non-existent. Many UE features (Datasmith, some VR pipelines) are Windows-only.

This is why an open Linux Vulkan-targeting Rust UI library (which Buiy is) addresses an underserved corner that even Unreal's reach hasn't filled.

## Sources

- Unreal Engine EULA — https://www.unrealengine.com/eula/unreal
- Unreal Engine License options — https://www.unrealengine.com/license
- Unreal Engine on GitHub (access flow) — https://www.unrealengine.com/en-US/ue-on-github
- Is Unreal Engine 5 Free? Costs, Licensing, and Royalty Explained — https://blog.3sfarm.com/is-unreal-engine-5-free-costs-licensing-and-royalty-explained
- Unreal Engine 5 Blueprint vs C++ performance (context) — https://www.spongehammer.com/unreal-engine-5-blueprint-vs-cpp-performance/
- Unreal Engine — Wikipedia — https://en.wikipedia.org/wiki/Unreal_Engine
