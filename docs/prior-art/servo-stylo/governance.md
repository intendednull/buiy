**Date:** 2026-05-29
**Status:** active
**Subject:** Servo / Stylo — project governance: Mozilla origins, the 2020 layoffs, the Linux Foundation Europe move, Igalia stewardship, funding, and bus-factor

# Servo / Stylo — Governance

Servo is the experimental browser engine in Rust; Stylo is its CSS style system (now also Firefox's). Their governance story is the single most load-bearing reason this folder exists: a corporate steward (Mozilla) built a large Rust codebase, withdrew funding overnight, and a much smaller community-plus-consultancy arrangement kept it alive. For a project like Buiy — built on a stack of independently-governed crates (`taffy`, `cosmic-text`, `accesskit`, `wgpu`) — the Servo arc is a concrete case study in what happens when the entity paying the engineers leaves. See [history.md](history.md) for the dated timeline; this file covers the *who-owns-what* and *who-pays* picture.

## Origin: Mozilla Research (2012–2020)

Servo began at Mozilla Research in 2012 as an R&D effort to build "an independent, modular, embeddable web engine" and to stress-test the then-young Rust language — Servo and Rust co-evolved, and Servo was the first large non-compiler Rust codebase. For roughly eight years Mozilla funded a dedicated team. The most durable output was not the browser but **Stylo**: Servo's parallel CSS engine was upstreamed into Firefox's Gecko and shipped as "Quantum CSS" in Firefox 57 (2017-11-14), and **WebRender** (Servo's GPU renderer) shipped in Firefox from version 67 (2019). Mozilla extracted production value from Servo's components while the engine itself stayed experimental.

## The withdrawal: August 2020 layoffs

On **2020-08-11** Mozilla announced layoffs of roughly **250 employees — about 25% of its workforce** — "to adapt its finances to a post-COVID-19 world and re-focus the organization on new commercial services." The Servo team was among the teams cut wholesale (alongside MDN, DevTools, and others). This is the corporate-steward-withdrawal event: a single funder's strategy change zeroed out the paid engineering on the project in one day. The codebase did not die, but day-to-day development effectively stopped for over two years.

## The rescue: Linux Foundation, then Linux Foundation Europe + Igalia

Two distinct governance moves are easy to conflate (Wikipedia conflates them; this corpus does not):

1. **2020 — Linux Foundation.** After the layoffs, "stewardship of Servo moved from Mozilla Research to the Linux Foundation in 2020" (Servo's own wording). This was a custodial transfer — a neutral home for the trademark, repos, and copyright — not a re-funding. Activity stayed near-zero through 2021–2022.
2. **2023 — Linux Foundation Europe + Igalia.** In January 2023 the project announced "new external funding" reactivating the team, and on **2023-09-07** Servo "officially joined Linux Foundation Europe." The renewed activity is "led by Igalia, a Linux Foundation Europe member that now has a team of engineers working on the project." (LF Europe itself was only founded in late 2022, which is why the 2020 transfer cannot have been to LF Europe.)

**Igalia's role — stated precisely.** Igalia is a worker-owned consultancy with deep prior browser-engine experience (it is a major Chromium and WebKit contributor). Since 2023 it runs Servo's day-to-day development and is the largest single contributor — **but it is not a numeric majority**, and no source formally designates it the sole "owner." In 2024 Igalia made **679 commits** and authored **26% of merged PRs**; **40%** came from other (non-Igalia) human contributors and **34%** from bots (Servo, 2024 report). The honest framing: Igalia is the steward and engine of the revival, but the project is structurally a multi-party effort governed by a **Technical Steering Committee (TSC)** that sets the roadmap and decides spending.

## Funding (Open Collective + GitHub Sponsors)

Servo set up community funding in early 2024 (announced 2024-03-12): a **Servo Open Collective** (via Open Source Collective) and **GitHub Sponsors**. Reported intake:

| Year | Raised (USD) | Donors | Source |
|---|---|---|---|
| 2024 | **$33,632.64** | 500 people/orgs | [servo.org, 2024 report](https://servo.org/blog/2025/01/31/servo-in-2024/) |
| 2025 | **$59,430.02** (project-reported "+62.5%") | — | [Servo Open Collective](https://opencollective.com/servo) (also cited in the [Igalia "Servo 2025 Stats" post](https://blogs.igalia.com/mrego/servo-2025-stats/)) |

(The 2025 figure is the exact total shown on the Servo Open Collective page — verified there, not in the older "Servo Revival: 2023-2024" Igalia post, which only runs through 2024 and pre-dates the 2025 total. The stated "+62.5%" does not arithmetically reconcile against $33,632.64 → $59,430.02, which is ~+76.7%; treat the percentage as project-reported, not independently checked, even though the dollar amount is exact.) Community donations cover **infrastructure, not salaries** — the funds bought 3 self-hosted CI runners (Linux/macOS/Windows) cutting build times from over an hour to under 30 minutes. The TSC decides spending transparently. Crucially, donation income (~$33k–$60k/yr) is **orders of magnitude below** the cost of the paid engineering team — Igalia's contract/consulting funding (including a 2025 **Sovereign Tech Fund** commission, below), not the Open Collective, is what pays the engineers. The donations are real but they are a CI-and-goodwill layer, not a salary base.

### Sovereign Tech Fund (2025)

In addition to the Open Collective and GitHub Sponsors, Igalia secured a **2025 commission from Germany's Sovereign Tech Fund** to advance Servo (announced 2025-10-09). Per Igalia, the commission funds **initial accessibility support, the `WebView` embedding API, and project maintenance** (issue triage, PR review, version releases, and governance support). This is a grant-funding source distinct from the donation pool: it pays for directed engineering work, not CI, and is the kind of multi-party funding the TSC structure is meant to enable (see "Bus-factor picture"). It is also why Servo's a11y — historically thin (see [open-problems.md](open-problems.md) §5) — finally has a dedicated funder.

## Governance mechanics: the Technical Steering Committee

Day-to-day direction runs through a **Technical Steering Committee (TSC)** rather than a benevolent-dictator model. The TSC:

- maintains the public roadmap on the project wiki (the 2025 roadmap was discussed and updated there);
- decides how donation income is spent, transparently in committee (this is how the self-hosted CI runners were approved);
- sits under Linux Foundation Europe's neutral umbrella, so no single member — including Igalia — can unilaterally redirect the trademark or the repositories.

This is a deliberately less-concentrated structure than the one that failed in 2020, where a single corporation held both the funding *and* the decision-making. The TSC separates those: funding can come from multiple parties (Igalia contracts, donations, sponsors), while direction is committee-held. For Buiy — currently a single-author project — the lesson is less "adopt a TSC now" and more "the entity paying for the work and the entity steering it should be separable, so the loss of one funder is survivable."

## Contributor breakdown (2024)

The 2024 numbers are the clearest published bus-factor signal:

| Slice | Share of merged PRs | Note |
|---|---|---|
| Igalia | 26% | largest single contributor; 679 commits |
| Other humans | 40% | the community remainder |
| Bots | 34% | dependency/automation PRs, not feature work |

Read carefully: humans authored ~66% of PRs, split 26% Igalia / 40% everyone-else. Igalia is the indispensable *coordinating* contributor — it employs the engineers who do the architectural work and review the rest — but it is not a numeric majority of merged changes. The "everyone else" 40% is real but diffuse; it is unlikely to self-organise into a funded team if Igalia exited, which is precisely what the 2020→2023 dormancy demonstrated.

## License: MPL-2.0 (diverges from Buiy)

Servo and Stylo are **MPL-2.0** (Mozilla Public License 2.0), inherited from their Mozilla origin and shared with Firefox/Gecko. Verified on crates.io: the `stylo` crate (v0.17.0, repo `github.com/servo/stylo`) declares `"license": "MPL-2.0"`. **This diverges from Buiy's MIT OR Apache-2.0.** MPL-2.0 is a weak/file-level copyleft: modifications to MPL-covered *files* must be shared, but the license permits combining MPL files with differently-licensed code in a larger work. Practical implication for Buiy: Buiy can *study* and *cite* Stylo's algorithms freely (algorithms aren't copyrightable), but Buiy **cannot vendor Stylo source** into its MIT/Apache tree without dragging MPL obligations onto those files. This is one reason Buiy implements a typed-Rust CSS subset itself rather than depending on `stylo` — see [../taffy/governance.md](../taffy/governance.md) for the contrasting MIT story of Buiy's actual layout dependency.

## Bus-factor picture

- **Concentration risk shifted, not eliminated.** Mozilla was a single corporate point of failure; today Igalia is a single-consultancy point of failure for the *paid* engineering. If Igalia's Servo contracts lapse, the 40%-other-contributors + 34%-bots remainder would not sustain the current velocity. The 2020→2023 dead period shows the failure mode is real.
- **Trademark/IP custody is safe.** Holding the project under Linux Foundation Europe means the name and repos cannot be unilaterally killed by any one funder — the lesson Mozilla's withdrawal taught.
- **Component longevity beats engine longevity.** Stylo and WebRender outlived the original Servo team because they were upstreamed into a shipping product (Firefox). Buiy's analogue: its components (the layout passes, the `Style` builder) gain longevity by being useful inside Bevy's ecosystem, not by the standalone library's survival.

## Implications for Buiy

- **Single-funder dependence is the named risk.** Buiy depends on `taffy` (one maintainer, see [../taffy/governance.md](../taffy/governance.md)), `cosmic-text`, and `accesskit`. The Servo case shows that even a deep-pocketed corporate steward can exit overnight; design contingencies (fork plans, vendored snapshots) for each load-bearing crate. Buiy's substrate commitments are documented in [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md).
- **License hygiene.** Stylo's MPL-2.0 is the reason Buiy reimplements CSS semantics as a typed-Rust subset rather than depending on Stylo. Keep every load-bearing dependency MIT/Apache-compatible; treat MPL/GPL code as reference-only.
- **Stewardship survives via usefulness-to-a-host.** Stylo lives because Firefox needed it. Buiy lives or dies by being useful inside Bevy. Tie the project's survival to a host ecosystem's needs, not to standalone donations.

## Sources

- Servo (software), Wikipedia: https://en.wikipedia.org/wiki/Servo_(software)
- "Servo to Advance in 2023", servo.org: https://servo.org/blog/2023/01/16/servo-2023/
- "Servo web rendering engine joins Linux Foundation Europe", linuxfoundation.eu: https://linuxfoundation.eu/newsroom/servo-web-rendering-engine-joins-linux-foundation-europe
- "Servo in 2024: stats, features and donations", servo.org: https://servo.org/blog/2025/01/31/servo-in-2024/
- "You can now sponsor Servo on GitHub and Open Collective!", servo.org: https://servo.org/blog/2024/03/12/sponsoring-servo/
- "Servo Revival: 2023-2024", Igalia (M. Rego): https://blogs.igalia.com/mrego/servo-revival-2023-2024/
- "Servo 2025 Stats", Igalia (M. Rego): https://blogs.igalia.com/mrego/servo-2025-stats/
- "Igalia, Servo, and the Sovereign Tech Fund" (2025-10-09): https://www.igalia.com/2025/10/09/Igalia,-Servo,-and-the-Sovereign-Tech-Fund.html
- Servo Open Collective (2025 total $59,430.02, +62.5%): https://opencollective.com/servo
- "Mozilla lays off 250 employees…", gHacks (2020-08-11): https://www.ghacks.net/2020/08/11/mozilla-lays-off-250-employees-in-massive-company-reorganization/
- `stylo` crate metadata (license MPL-2.0, v0.17.0): https://crates.io/crates/stylo
- Buiy foundation spec: [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling: [history.md](history.md), [stylo.md](stylo.md), [rendering.md](rendering.md), [../taffy/governance.md](../taffy/governance.md), [../dioxus/governance.md](../dioxus/governance.md)
