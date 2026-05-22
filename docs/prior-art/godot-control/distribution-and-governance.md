**Date:** 2026-05-22
**Status:** active
**Subject:** Godot Control — distribution + governance: Godot Foundation (Dutch Stichting, 2022), MIT license, contributor base, commercial partnerships (W4 Games), release cadence

# Distribution and governance

## License

Godot is **MIT-licensed**. The license has been MIT since the project's public release in January 2014 and has not changed across major versions. The MIT license applies to the engine source, the editor, and the standard library; bundled third-party dependencies have their own licenses (FreeType: FTL/GPLv2, HarfBuzz: MIT-equivalent, ICU: ICU License which is BSD-style, mbedTLS, etc.) — Godot's [LICENSE.txt](https://github.com/godotengine/godot/blob/master/LICENSE.txt) enumerates them.

**Comparison to Buiy:** Buiy commits to dual MIT-or-Apache-2.0 (Rust ecosystem norm). Godot's single-MIT is permissive enough that there is no friction for any commercial use; the absence of Apache-2.0 means no explicit patent grant, which is a real but rarely-blocking difference. For Rust ecosystem code Buiy can vendor or take inspiration from Godot's design without re-licensing concerns since the source is permissively licensed.

## The Godot Foundation

The **Stichting Godot Foundation** is a Dutch non-profit foundation (Stichting is the Netherlands' general non-profit legal form, analogous to a 501(c)(3) in spirit). Formed in **November 2022**, replacing the project's prior fiscal-sponsorship arrangement with the **Software Freedom Conservancy** (which Godot had used since ~2015).

### What the Foundation owns

- The **Godot trademark.**
- Donation infrastructure (the Foundation receives PayPal / Stripe / OpenCollective contributions; the prior SFC arrangement routed donations through SFC).
- Employment contracts for full-time Godot developers (started with 4–6 employees in 2023; expanded since).
- Commercial partnerships (Meta, Microsoft, Khronos, W4 Games) for grant and sponsorship revenue.

### What the Foundation does *not* own

- The **source code** — that's MIT, owned by the contributors (per the MIT license, no CLA is required in Godot; contributions are made under the MIT terms by the contributors themselves).
- **Roadmap decisions** — the Foundation funds developers but the engineering roadmap is decided through the standard open-source process (GitHub Discussions, godot-proposals issues, PR review). The Foundation explicitly does not have unilateral authority to direct the engine.
- **Editorial control over PRs** — merge authority lives with the maintainer team, not the Foundation board.

### Foundation board

The board has rotated since formation. **Juan Linietsky** (co-founder) has held founding and ongoing leadership roles; **Rémi Verschelde** is a long-standing core maintainer with foundation involvement; elected representatives from the contributor base round out the body. The exact membership shifts; consult [godot.foundation](https://godot.foundation/) for current composition.

### Funding model

- **Donations** — individual recurring + one-time donations via the Foundation's website, OpenCollective, GitHub Sponsors.
- **Corporate sponsorships** — tiered partnership with corporate sponsors (Meta has historically been a large sponsor for VR-related work; Microsoft for D3D12 and Windows-platform improvements).
- **Grants** — project-specific grants (e.g., for AccessKit work, Wayland support).
- **W4 Games partnership** — commercial entity (next section) contributes to the Foundation directly.

Annual budgets are published on the Foundation's website.

## W4 Games — the commercial partner

[W4 Games](https://w4games.com/) is a **venture-backed commercial company** founded by ex-Godot core developers, including (at one point) Juan Linietsky and core contributor Rémi Verschelde, plus former Godot Foundation executive Fabio Alessandrelli. W4's business:

- **Console ports** — Nintendo Switch, PlayStation 5, Xbox Series X|S, and other commercial consoles require NDA'd SDKs that incompatible with MIT-licensed open-source distribution. W4 sells per-title or per-studio commercial licenses for the closed-source console-port runtime.
- **Commercial support contracts** for studios using Godot at scale.
- **Cloud services** — multiplayer / matchmaking / leaderboards for Godot games.

W4 is **not a fork** — it consumes upstream Godot and contributes back via the same open-source mechanism as any other contributor. W4's existence solves a real ecosystem problem (Godot games on consoles) without compromising the engine's open-source posture.

VC backing: W4 raised seed funding from a consortium of game-industry angel investors in 2022 and has expanded since. Specific funding amounts are not publicly itemized.

## Release cadence and version policy

Godot's release cadence has evolved:

- **3.x era** — roughly 12–18 months between major versions; 6-month minor versions.
- **4.x era** — ~6-month minor versions (4.1 Jul 2023 → 4.2 Nov 2023 → 4.3 Aug 2024 → 4.4 Mar 2025 → 4.5 Sep 2025 → 4.6 Q1 2026). The cadence has slipped slightly from the 6-month target as features grow.
- **Patch releases (X.Y.Z)** — typically 1–3 per minor version, addressing crashes and major regressions.

**Backward compatibility policy**: Godot 4 deliberately broke compatibility with Godot 3. Within 4.x, the project tries to maintain GDScript / scene-file compatibility across minors; breaking changes are documented in release notes. The C++ source itself (which third-party GDExtension authors consume) has had API drift across 4.x minors that requires plugin updates.

**GDExtension ABI stability**: Godot 4.x introduced GDExtension (the C ABI for third-party plugins, intended to replace GDNative from 3.x). The ABI is documented but not yet fully stable across minor versions; plugin authors (including Rust `gdext` users) typically recompile per Godot minor.

## Contributor base

Godot is one of the largest open-source contributor pools in the game-engine space. Per the [contributor list](https://github.com/godotengine/godot/graphs/contributors), the engine has accepted PRs from **over 3,000 contributors** since 2014. Active core maintainers number in the dozens; the broader contributor base includes occasional contributors, documentation writers, translators, and bug reporters.

Coordination happens via:

- **GitHub Issues** — bug tracking. ~10,000+ open issues; volume is managed via labels + triage rotations.
- **godot-proposals** — separate repo for feature proposals. Long-lived discussions on design directions.
- **GitHub Discussions** — community Q&A.
- **Godot Engine Contributors Conference** (GodotCon) — annual in-person event; talks are recorded.
- **Discord, Reddit, forum** — informal community channels.

No formal RFC process — design state lives across godot-proposals discussions and core-developer PR review. This is similar to Bevy's lightweight design-state model (see [`/home/user/buiy/docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) "Avoid" row 10) with the same upsides (low overhead, contributor-friendly) and the same downsides (design history is scattered).

## Platform support

Godot's official platform matrix (as of 4.6):

- **Desktop:** Windows 7+, macOS 10.13+, Linux. Fully supported.
- **Mobile:** Android 6+ (Vulkan), iOS 12+ (Metal). Fully supported.
- **Web:** WebAssembly + WebGL 2 / WebGPU. Fully supported; sometimes flaky on niche browsers.
- **Consoles:** Switch / PS5 / Xbox via W4 Games (commercial, closed-source per-platform).
- **TV / Console-adjacent:** Apple TV (tvOS) experimental; visionOS export added in 4.5.

The Foundation maintains official Linux + Windows + macOS + Android + iOS + web builds; the commercial console port runtime is W4's product.

## Implications for Buiy

- **Foundation governance is a workable open-source UI ecosystem model.** Stichting form, MIT license, board-led, donation + partnership funded. Buiy doesn't need a foundation today (we're a Bevy plugin under Bevy's foundation) but the precedent matters if Buiy ever becomes its own ecosystem.
- **MIT-permissive scales commercially.** Twelve years of MIT did not prevent W4 Games, Meta, Microsoft, or Khronos from engaging commercially. Validates Buiy's permissive licensing.
- **Console-port-as-commercial-partner is a real pattern.** Buiy doesn't need to solve consoles itself; if Bevy ships through a third-party commercial partner (analogous to W4 for Godot) Buiy inherits whatever that partner can deliver. Foundation-§3 platform-support staging is fine for Buiy v1.
- **No-CLA contribution model** (per-PR MIT) is what Godot uses; it works at the engine scale. Buiy follows the same Bevy-default model.
- **Avoid:** Godot's lightweight RFC process. Design state across godot-proposals + Discord + Discord-recorded-conference-talks reaches the same lossiness as Bevy's. Buiy's `docs/specs/` + `docs/plans/` + `docs/reports/` discipline is the corrective.
- **Borrow:** the engine-eats-its-own-editor dogfooding model. Godot ships its editor on the same Control system game developers use; bugs in the framework are felt immediately by the maintainers. Buiy's [`buiy_devtools`](../../specs/2026-05-07-buiy-foundation/architecture.md) crate should be the same — devtools written in Buiy, not in a separate UI stack.

## Sources

- Godot Foundation — https://godot.foundation/
- Godot license file — https://github.com/godotengine/godot/blob/master/LICENSE.txt
- W4 Games — https://w4games.com/
- Software Freedom Conservancy (prior fiscal sponsor) — https://sfconservancy.org/
- godot-proposals repository — https://github.com/godotengine/godot-proposals
- Godot contributors graph — https://github.com/godotengine/godot/graphs/contributors
- Bevy-UI governance comparison — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
