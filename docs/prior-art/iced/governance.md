**Date:** 2026-05-22
**Status:** active
**Subject:** iced — lead author, organization, funding, license posture, RFC process

# Governance

This file captures who decides what in iced: who Héctor Ramón is, how the iced-rs GitHub organization is structured, how decisions are made, and how the project is funded. Companion to [`history.md`](history.md) (what shipped when) and [`ecosystem.md`](ecosystem.md) (who builds on top).

## Lead architect

**Héctor Ramón** ([hecrj](https://github.com/hecrj)) is the founder, lead architect, and BDFL-by-fact of iced. From his GitHub Sponsors page: *"Hi! My name is Héctor, but you may know me as hecrj. I am the creator of Iced, a cross-platform GUI library for Rust."*

He framed his trajectory there: *"5 years ago, I decided to create my own games while contributing to open-source."* iced spun out of his earlier `coffee` game-engine experiments around 2019.

iced's philosophy chapter is unambiguous about the autonomy posture: *"Complexity is bad. It creeps in. Silently. Unnoticed. And then it kills everything."* and on the rejection of DSLs: *"You will write everything in plain Rust"* with *"Rust itself is powerful and elegant enough to express user interface code."* The chapter notably also flags *"personal project autonomy over community-driven development, directly contradicting conventional open-source expectations"* — Héctor reserves the right to reject contributions that don't fit his architectural taste, a stance he has reaffirmed several times in GitHub discussions.

This is structurally similar to Bevy's relationship to cart, and stands in deliberate contrast to the more committee-shaped governance of, e.g., `egui` (Emil Ernerfeldt + Rerun.io team) or GTK-rs. The bus-factor risk is real (see [`open-problems.md`](open-problems.md) § "Single-maintainer risk").

## The iced-rs organization

The [iced-rs](https://github.com/iced-rs) GitHub organization holds the main repo and adjacent crates:

- `iced-rs/iced` — the main repo.
- `iced-rs/cryoglyph` — the glyphon fork (since March 2025).
- `iced-rs/awesome-iced` — community-curated list of apps and widgets.
- `iced-rs/iced_aw` — community-contributed widget extras (badges, color pickers, date pickers, drop-downs not in core, etc.).

Héctor is the organization owner and the only person with commit authority on the main repo. Major releases are tagged and published by him; CHANGELOG entries credit individual PR authors but the final integration pass is single-hand.

## Contributors

iced has 400+ unique contributors per the GitHub contributor graph. Substantial repeat contributors (verified by recent commit history in the 0.13–0.14 cycle) include:

- **Héctor Ramón** (`hecrj`) — lead, ~half of all commits.
- Multiple System76 engineers via the COSMIC desktop integration — bug reports and PRs across `iced_winit`, theming, and Wayland integration.
- Long-tail individual contributors — most PRs end up rewritten by Héctor before merge, a pattern that has both quality-control upsides and contributor-friction downsides (see [`critiques.md`](critiques.md) § "Maintainer review style").

## Funding

iced runs on a combination of:

- **GitHub Sponsors via [hecrj's page](https://github.com/sponsors/hecrj).** Four tiers: $5 / $10 / $20 / $50 per month, all coffee-themed. The page shows 14 current sponsors and 40 past sponsors as of 2026-05-22. The page does **not** list any current corporate-sponsor employment.
- **Kraken / Cryptowatch corporate sponsorship.** The iced README states verbatim: *"The development of Iced is sponsored by the [Cryptowatch](https://cryptowat.ch/charts) team at [Kraken.com](https://kraken.com/)"*. This is the largest single financial backer and has been continuous since around the 0.6 era. The Cryptowatch desktop application is one of iced's flagship in-production users.
- **System76 via COSMIC desktop adoption.** Not direct cash sponsorship, but System76 employs engineers who contribute upstream improvements as part of their COSMIC work. This is the closest iced has to a "vendor" relationship — see [`ecosystem.md`](ecosystem.md) § "COSMIC desktop."

The brief that produced this folder asked whether Héctor is employed by System76. **No public source confirms this.** His GitHub Sponsors page lists no employer; the iced README credits Kraken/Cryptowatch as a sponsor, not System76 as an employer. If he is employed by either company, that fact is not public.

## License

**MIT, single license.** Every version since `0.0.0` in 2019 has shipped under MIT.

This is a deliberate divergence from the Rust ecosystem norm of `MIT OR Apache-2.0` dual-licensing. The Bevy ecosystem (including `bevy_ui`), the rest of the Linebender stack (`xilem`, `parley`, `vello`), Slint (GPL-3 / commercial / Royalty-free), `egui` (MIT OR Apache-2.0), and most foundational Rust crates dual-license.

Implications:

- **For users:** MIT-only means slightly weaker patent-grant language than Apache-2.0 provides. Most consumers do not care; some legal teams in patent-active companies do. iced has not received a patent claim in its history.
- **For dependency-graph hygiene:** mixing iced (MIT) with Apache-2.0 dependencies in a downstream license report produces a multi-license bundle. This is not a technical problem but is a paperwork tax for some commercial users.
- **For contributors:** patches default to MIT under iced's CLA-free contribution model. The repo has no formal CLA.

For Buiy (which inherits Bevy's `MIT OR Apache-2.0` ecosystem norm), iced's MIT-only is a footnote, not a blocker.

## RFC / decision process

There is **no formal RFC process**. Major architectural decisions are discussed in:

- **GitHub Issues** and **Discussions** on the main repo.
- The **Zulip forum** at https://iced.zulipchat.com — the canonical place for design conversation between contributors.
- The **Discord server** — more casual community Q&A.
- The **Whimsical graphical roadmap** at https://whimsical.com/roadmap-iced-7vhq6R35Lp3TmYH4WeYwLM — Héctor's public planning surface.
- **The iced book** at https://book.iced.rs/ — long-form essay-style documentation of design choices (Philosophy, The Elm Architecture, etc.). The book is still in active drafting as of 2026-05-22 and explicitly notes it remains incomplete.

The 14-month gap between 0.13 and 0.14 is partly explained by the absence of an RFC discipline: there is no public "we will ship X next" milestone tracker tied to dates. The Whimsical roadmap is the closest analog and is informal.

**Lesson for Buiy:** the lightweight-process choice has cost iced legibility — feature requests sit unresolved for years (issue #552 since 2020), and outsiders cannot predict what lands when. Buiy's commitment to `docs/specs/` + `docs/plans/` (foundation [README § Code Conventions](../../specs/2026-05-07-buiy-foundation/README.md)) is informed by exactly this gap. See [`docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) Avoid row "Lightweight RFC process" for the matched pattern in Bevy.

## Sources

- hecrj on GitHub — https://github.com/hecrj
- GitHub Sponsors page for hecrj — https://github.com/sponsors/hecrj
- iced README (Sponsors section) — https://github.com/iced-rs/iced
- iced book, Philosophy chapter — https://book.iced.rs/philosophy.html
- iced organization — https://github.com/iced-rs
- iced Zulip — https://iced.zulipchat.com
- iced Whimsical roadmap — https://whimsical.com/roadmap-iced-7vhq6R35Lp3TmYH4WeYwLM
