**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — commercial licensing tiers, governance, long-term viability

# Distribution & governance

NoesisGUI is **closed-source proprietary commercial software** distributed under a tiered license model. The runtime is owned by Noesis Technologies S.L. (Madrid, Spain); the source code is not publicly available except as an optional add-on to certain paid tiers; the project does not accept external code contributions to its core. This is a deliberate stance — Noesis Technologies is a small commercial entity that exists by selling licenses, and its governance model reflects that.

This file documents the licensing structure, the indie threshold, and the long-term-viability question that any potential adopter has to answer for themselves.

## Licensing tiers (2026 current)

Pricing structure as published on [the licensing page](https://www.noesisengine.com/licensing.php) and updated in [the March 2024 forum post](https://www.noesisengine.com/forums/viewtopic.php?t=3260):

| Tier | Price | Eligibility | Source code |
|---|---|---|---|
| **INDIE** | **€195 / project** (all platforms) | Gross revenue < €100K/year **and** project budget < €250K | No |
| **PRO** | €9,000 first platform + €3,600 / extra | Project budget < €1.5M | Optional (extra cost) |
| **PREMIUM** | €18,000 first platform + €7,200 / extra | Project budget €1.5M – €12M | Optional |
| **Enterprise** | Contact required | Project budget > €12M, or non-gaming applications | Optional |

All tiers include:

- All platforms (Desktop, Mobile, all consoles — Xbox One/Series, PS4/PS5, Nintendo Switch / Switch 2).
- All minor and major releases.
- Fully featured (no feature-gated tiers).
- Royalty-free.
- Perpetual license (the license itself does not expire; maintenance subscription is separate).
- Noesis Studio (the visual editor).

Differences:

- **Forums support** is the only support for INDIE. PRO and PREMIUM include priority private-ticket support (mandatory first year: €3,200 / $1,300 PRO, €6,300 / $2,600 PREMIUM, yearly).
- **Source code access** is not available at INDIE; available at additional cost on PRO and PREMIUM.

The **30-day trial** is available on request at any tier.

### What changed in March 2024

Per the [forum announcement](https://www.noesisengine.com/forums/viewtopic.php?t=3260):

- INDIE price: **€395 → €195** (51% reduction).
- INDIE project-budget threshold: **€100K → €250K** (2.5× increase).
- Studio access added to all licenses.
- Console support included in all licenses (previously may have been an add-on).
- Pricing for higher tiers made public (previously contact-only).

The 2024 changes shifted Noesis toward a more accessible indie tier while keeping the enterprise pricing intact. This is consistent with how the product has been marketed: "we want indies to use it cheaply but the AAA revenue is the company's foundation."

## Governance

- **Company:** Noesis Technologies S.L., privately held, ~2-9 employees, headquartered in Madrid.
- **Funding:** Bootstrapped / customer-funded. No public funding rounds listed on Crunchbase.
- **Roadmap:** Closed; no public RFC process or community design feedback channel. Feature requests live in the [forums](https://www.noesisengine.com/forums/).
- **Source contribution:** The runtime is not accepted from external contributors. The **LanguageServer** repo and **Lottie-Noesis** repo are MIT-licensed and accept community PRs, but these are auxiliary tools, not the core runtime.
- **License changes:** Noesis has historically broadcast license-structure changes via forum announcements and direct email to license holders. The March 2024 indie-price change is the most recent major adjustment.

## Long-term viability

The viability question for any potential adopter: **what happens if Noesis Technologies S.L. goes out of business?**

The technical risk: a customer with a perpetual license retains the right to use whatever version they had at the time, but loses access to future engine compatibility, future platform SDK bumps, and bug fixes. A game shipping on Noesis 3.2.x today and continuing past 2030 would face:

- New Unity / Unreal minor releases — Noesis-maintained plugin no longer updated; customer must port plugin to new engine API.
- New console SDKs — Noesis ships per-console NDA'd code; customer cannot easily port.
- Bugs in the core runtime — customer cannot fix without source.

**Source-code add-on** is the standard mitigation. PRO and PREMIUM tiers offer source code access at additional cost; customers shipping on Noesis typically bundle source-code access into their procurement so that if Noesis vanishes, the customer can fork and continue. Anecdotally (Larian, Ninja Theory, etc.) AAA customers do buy the source-code option.

For indie tier (€195, no source), the viability question is sharper: the indie pays a low one-time fee and inherits long-term dependency risk. The €195 is not refundable if the company ceases trading.

Noesis Technologies has been profitable and continuously operating since ~2013 — 13 years of cash flow. The release cadence (one minor every 2-3 months across 2024-2026) suggests the company is currently healthy. There is no public indication of an imminent change. But the structural risk — a small bootstrapped company in a niche market — is real and any adopter should price it in.

## Console-platform support

Noesis ships per-console support packages for:

- Xbox One, Xbox Series X|S
- PlayStation 4, PlayStation 5
- Nintendo Switch, Nintendo Switch 2

These require platform NDAs (Sony, Microsoft, Nintendo) and are gated behind those agreements. Noesis manages the NDA relationships on its customers' behalf; customers get the platform support without negotiating directly with the platform holder for UI middleware approval. **This is a significant value-add** for a small studio that would otherwise face years of platform-relationship work.

Buiy has no analog. As open-source MIT/Apache code, Buiy can in principle target any platform Bevy targets (which is Desktop + WASM today; Bevy itself does not have console support as a first-class target). Console UI on Bevy + Buiy would require a customer's existing platform relationship + per-platform Bevy-on-console work, which is a separate question entirely.

## Comparison: distribution model

| Project | License | Source | Console support | Indie threshold |
|---|---|---|---|---|
| **NoesisGUI** | Proprietary commercial | Optional paid add-on | Yes (NDA managed) | €195 < €100K rev |
| **Coherent Gameface** | Proprietary commercial | Source included (Premium) | Yes | Contact (not public) |
| **Scaleform GFx** | Discontinued (Autodesk) | n/a | Was supported | n/a |
| **Unity UI Toolkit** | Proprietary (Unity Editor required) | Closed | Via Unity Pro | Free tier (small studios) |
| **Unreal Slate/UMG** | Proprietary (Unreal EULA, royalty-based) | Yes (source available) | Via Unreal | 5% royalty above $1M |
| **Buiy** | MIT OR Apache-2.0 dual | Yes (open) | n/a (Bevy-driven) | n/a (free) |

Buiy and Noesis sit at opposite ends of the distribution-model spectrum. Buiy is permissive open source; Noesis is closed commercial. The lesson is *not* "Buiy should be commercial" but rather "Buiy and Noesis serve different markets" — Noesis serves studios that want a turnkey commercial solution with NDA-managed console support; Buiy serves the Bevy community that wants permissive open-source with full source access by default.

## Implication for Buiy

The Noesis licensing structure validates that **commercial UI middleware is a viable business**. It does not validate that Buiy should be commercial. Buiy's foundation is dual MIT/Apache-2.0 (the Bevy ecosystem norm); the open-source license is a hard constraint of the project. The lesson from Noesis is on the *product surface* side — the comprehensive feature set + console support + platform breadth — not the distribution model.

Specific borrowable practices:

- **Console support discipline.** Noesis ships per-console-SDK builds and manages the NDA relationships. Buiy can't replicate this directly, but the **pattern** of "the UI library is the single integration point for console TRC / TCR compliance (UI scaling, safe areas, controller affordances)" is right. Buiy's foundation accessibility goals already include controller / gamepad nav; the console-UI extensions (e.g., Xbox safe areas, PS5 trigger affordances) belong as a future sub-spec.
- **Indie tier ergonomics.** Noesis's free-for-indies threshold is friction-free for small developers — €195 is in indie-tool-pricing territory. Buiy's MIT/Apache license is friction-zero; this is *strictly better* than Noesis for indie adoption. The lesson is to keep that advantage visible: never gate features behind a "premium tier" pattern.

## Sources

- NoesisGUI licensing page — https://www.noesisengine.com/licensing.php
- Updated licensing forum post (March 2024) — https://www.noesisengine.com/forums/viewtopic.php?t=3260
- Crunchbase profile — https://www.crunchbase.com/organization/noesis-technologies
- Coherent Gameface site — https://coherent-labs.com/products/coherent-gameface/
- Unreal EULA / royalty terms — https://www.unrealengine.com/en-US/eula/publishing
- Buiy foundation goals — ../../specs/2026-05-07-buiy-foundation/README.md
