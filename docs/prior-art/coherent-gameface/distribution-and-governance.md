**Date:** 2026-05-22
**Status:** active
**Subject:** Coherent Gameface — pricing, distribution, ownership, team, long-term viability

# Distribution and governance

## Pricing model

Coherent Gameface is **proprietary, commercial, quote-based**. There is no public price list. The pricing page states (paraphrased from public sources):

- **Pricing is determined by project scope and budget** — "a fair and scalable model for studios of all sizes."
- **Every license includes**: full Gameface SDK + regular updates, comprehensive documentation, first year of support and maintenance.
- **Indie pricing**: "special pricing for indie requests" — no fixed indie tier publicly documented.
- **Trial**: 30-day evaluation with watermark; cannot ship a product.

This is **the most opaque pricing model among major game-UI middleware vendors**. By comparison:

| Vendor | Pricing model |
|---|---|
| **Coherent Gameface** | Fully quote-based; no public tiers; "special pricing for indies on request" |
| **NoesisGUI** | Tiered with public prices: Indie €195 (rev < €100K) / Pro €9K + €3.6K/extra platform / Premium €18K + €7.2K/extra (March 2024 restructure) — see [`noesisgui/distribution-and-governance.md`](../noesisgui/distribution-and-governance.md) |
| **RmlUi** | MIT-licensed, free, no royalties — see [`rmlui/distribution-and-governance.md`](../rmlui/distribution-and-governance.md) |
| **Buiy** | MIT-OR-Apache-2.0 dual permissive; no fee, no royalty, no quote — foundation [`README.md` non-goals](../../specs/2026-05-07-buiy-foundation/README.md#non-goals) |

The opacity is a deliberate commercial choice — every prospective AAA studio gets a custom-priced engagement. The Coherent sales motion is consultative and includes pre-sales technical engagement (per the PocketGamer.biz interview, Coherent staff fly out to studios to help with integration). The trade-off is that **indie / hobbyist / open-source projects effectively can't budget for Coherent**, since the price is unknown until quoted.

## Distribution channels

- **Direct sales** is the primary channel. Customers contact Coherent Labs, receive a quote, sign a license, get a download portal.
- **No Unreal Marketplace SKU** for Gameface (verified — searching the marketplace as of 2026-05-22 doesn't surface it). Contrast with NoesisGUI which does list on UE Marketplace.
- **No Unity Asset Store SKU** for Gameface (verified). Contrast with NoesisGUI which does maintain an Asset Store version (with documented drift issues per [`noesisgui/critiques-and-open-problems.md`](../noesisgui/critiques-and-open-problems.md)).
- **GitHub OSS components** — `CoherentLabs/GameUIComponents` (custom Web Components), `CoherentLabs/CoherentUIMobileOpenSource` (Coherent UI Mobile, the original mobile product, MIT-released as part of the EOL motion), and several Gameface-flavored open utilities. These are **supplementary tooling, not the core engine** — Cohtml + Renoir remain closed-source.
- **npm registry** — Coherent maintains the `@coherent-labs` npm scope and several `coherent-gameface-*` packages (`-grid`, `-automatic-grid`, `-components`, etc.) for the custom-element library. Free to use.
- **Figma plugin** — "Coherent Gameface Exporter" on the Figma community marketplace; exports Figma designs to Gameface-ready HTML/CSS.

The deliberate split: **core runtime closed-source + commercial; surrounding tooling open-source + free.** This pattern matches Unity's "engine commercial, packages on UPM mostly open" approach.

## Ownership

**Could not be verified** as of 2026-05-22:

- Coherent Labs AD (Dun & Bradstreet registry, Sofia) — appears to remain an independent privately-held Bulgarian company.
- Crunchbase reports ~$260K total funding (Dare to Scale + LAUNCHub Ventures) — modest, no signs of large VC or M&A in the public record.
- The pre-amble claim of "acquired by Hellbender / Streamline Group ~2021" **could not be confirmed**. Searches for that acquisition return no public coverage. Most likely sources of confusion: (a) **Coherent, Inc.** (laser/photonics, unrelated company) was acquired by II-VI Incorporated in 2022 and renamed Coherent Corp; (b) the pre-amble was speculative.
- Leadership as of the current About page: George Petrov (CEO), Dimitar Trendafilov (CTO), Nick Vasilev (R&D Director), Aleksandra Ivanov (Customer Success Director), Annie Atanasova (Director of Operations and Finance). Two of four founders remain in named senior leadership; Stoyan Nikolov departed ~2019.

The corpus should not propagate the Hellbender claim without independent verification. If future research surfaces a real acquisition record, update this file.

## Team size

- **Founded**: 2012 with ~5 people (per PocketGamer.biz interview).
- **Mid-2010s**: ~50 people (per PocketGamer.biz interview).
- **As of 2026**: estimate ~50–100 based on LinkedIn employee counts (LinkedIn typically reports ranges; Crunchbase reports "11-50 employees"; Tracxn reports "100+").

A team of ~50–100 is enough to:

- Maintain an in-house HTML engine + GPU renderer.
- Ship Unreal + Unity + custom-C++ bindings, tracking engine cadences.
- Maintain LTS + Feature release tracks.
- Run consultative pre-sales engagement.
- Operate documentation, support, customer success.
- Compete with NoesisGUI (Madrid, smaller team estimated 2-9 per Crunchbase but the figures are dated).

A team of 50–100 is roughly **the floor for proprietary middleware that ships an in-house browser engine** — anything less and the web-platform-tracking workload outpaces the team. This sets a benchmark for what doing this work "right" requires in headcount.

Buiy's contrast: Bevy + the Buiy maintainer cluster. The maintainer cluster's headcount is necessarily much smaller in the near term. **The way Buiy stays viable at lower headcount is by NOT shipping an HTML engine** — Taffy + cosmic-text + AccessKit + wgpu are maintained by other communities Buiy reuses.

## Long-term viability

Positive signals:

- **14-year continuous operation** (2012–2026). Multiple AAA shipped titles. Multiple generations of substrate (WebKit → Hummingbird → Cohtml/Renoir).
- **Sustained release cadence**: quarterly minor + LTS + Feature tracks visible in the changelog.
- **Engine support staying current**: UE 5.x + Unity 2020.2+ as of 2026 docs.
- **Customer brand strength**: Borderlands 4, Marvel's Spider-Man 2, Civilization 7, Alan Wake 2 are 2024–2025 AAA titles — recency confirms ongoing studio adoption.
- **Two founders remain in named leadership** — corporate memory + technical depth preserved.

Risk signals:

- **Pricing opacity** discourages broad adoption; the lower funnel is gated by sales engagement. If a competitor (or open-source alternative) makes the floor more accessible, Coherent's growth ceiling depends on retaining AAA studios specifically.
- **Single-vendor risk for shipping titles**. A studio that ships on Cohtml has an engine + a renderer + a JS VM bridge built and owned by Coherent. Coherent EOL'd Coherent UI in 2017; the same could happen to Gameface if Coherent pivots. Existing customers have source-code-escrow language in their contracts (standard for AAA middleware), but switching cost is real.
- **Web platform pace pressure**. Container queries (2022–2023), anchor positioning (2024), modern CSS color, view transitions, scroll-driven animations — every year Cohtml falls further behind a moving Chromium target unless the engineering team can absorb. ~50–100 engineers vs Chromium's hundreds; the relative position is what it is.
- **Open-source competition emerging**. RmlUi (open-source MIT, HTML-flavored, 18-year cumulative) and Buiy (open-source MIT/Apache, Bevy-only) cover overlapping problem space at zero license cost. Coherent's commercial moat is "we ship at AAA scale today" — that moat depends on continued AAA adoption.
- **Game-industry headcount volatility 2023–2025**. Layoffs swept the game industry during this period. Coherent's customer base is, by definition, game studios. Customer churn affects revenue. (No specific public information about Coherent Labs's own financial position — independent + privately-held companies don't publish quarterly numbers.)

## What Coherent's posture tells Buiy

- **Commercial proprietary middleware at this scope requires ~50–100 engineers and a commercial revenue stream.** Buiy can't replicate that headcount and shouldn't try. Buiy's strategy is to **defer the maintenance-intensive substrate work** (HTML parsing, JS runtime, custom CSS parser, custom layout engine, custom shaper) **to upstream communities** (Taffy, cosmic-text, AccessKit, wgpu, Bevy), and own only the integration layer + widget catalog + a11y bridge + render pipeline.
- **Opaque pricing is a market-segmentation tool.** Coherent prices for AAA. Indies / mid-tier studios / open-source projects are gated out. Buiy's MIT/Apache positioning targets exactly the segment Coherent doesn't serve — the same way RmlUi has filled that niche on the C++ side.
- **The "consultative pre-sales engagement" model is fundamentally incompatible with open-source distribution.** Buiy ships via crates.io + GitHub; there's no sales engagement, no quote. The discoverability + low-friction-adoption upside is what offsets the lack of hand-holding.

## Sources

- Coherent Labs pricing page — https://coherent-labs.com/pricing/
- Coherent Labs FAQ — https://coherent-labs.com/frequently-asked-questions/
- Coherent Labs About page — https://coherent-labs.com/about-us/
- PocketGamer.biz interview ("new standard for game UI") — https://www.pocketgamer.biz/interview/67816/can-coherent-labs-rise-as-the-new-standard-for-game-ui/
- The Recursive company profile — https://therecursive.com/company/coherent-labs/
- Crunchbase profile — https://www.crunchbase.com/organization/coherent-labs
- Dun & Bradstreet (COHERENT LABS AD, Sofia) — https://www.dnb.com/business-directory/company-profiles.coherent_labs_ad.fef0f7437aa2c5dff6d99516a7ad0dcf.html
- GameUIComponents OSS repo — https://github.com/CoherentLabs/GameUIComponents
- Coherent UI Mobile OSS repo — https://github.com/CoherentLabs/CoherentUIMobileOpenSource
- npm `@coherent-labs` scope — https://www.npmjs.com/~coherent-labs
- Figma Coherent Gameface Exporter — https://www.figma.com/community/plugin/1595556380268929590/coherent-gameface-exporter
- LinkedIn company page — https://bg.linkedin.com/company/coherent-labs
