**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — chronological history, Lapce origin, version timeline, the 18-month crates.io silence

## Lapce: the parent project (2018 → present)

Lapce is the project Floem was extracted from. The Lapce timeline as best reconstructed from public sources:

- **~2018** — Dongdong Zhou starts Lapce as a personal project. (Confirmed via the InfoQ piece below; the first commit predates the 2020-public-launch by an unclear margin.)
- **~2020-2021** — Lapce gains public attention. Hacker News discussions; first contributors join.
- **2022-2023** — Lapce shifts UI substrate. Earlier iterations used Druid (the Linebender pre-Xilem predecessor); a transition to a custom UI layer begins.
- **2023** — The custom UI layer is extracted as `lapce/floem` (the first GitHub commits on the floem repo are early 2023).
- **2024-03** — InfoQ piece "Lapce is a Native Open-Source Code Editor Written in Rust" published; Lapce HN thread (Feb 2024) confirms project has no full-time maintainers.
- **2024-Nov** — Floem 0.2.0 published; Lapce continues on git-dependency basis.
- **2025-2026** — Active development on Floem `main` continues; no new published crates.io versions; Lapce remains alive.

The "Floem extracted from Lapce" framing is the standard one: Lapce's UI was first an embedded part of the editor, then was promoted to its own crate when the team realized the UI layer was reusable. This pattern (editor team builds custom UI, then extracts it) traces a line through TextMate / Sublime / VS Code (editor builds UI primitives) but Lapce + Floem is the first instance where the UI primitives ship as a separate Rust crate intended for outside reuse.

## Floem's own version timeline

| Version | Date | Notes |
|---|---|---|
| **0.1.0** | (early 2024, before 0.1.1) | First public crate. Minimal release notes. |
| **0.1.1** | 2024-01-13 | Tagged "First release" on GitHub. |
| **0.2.0** | **2024-11-15** | Major release: "nearly a year of work." Highlights: editor integration (PR #296), experimental WebAssembly, keyframe + spring animations, Vello renderer integration (behind feature flag), ECS view improvements, new logo. |
| (none since) | **17+ months** | Active development on `main` continues but no published version. |

Only **three** versions in nearly four years of public existence. Compare to:

- **egui**: 30+ releases in the same period.
- **Iced**: ~15 releases.
- **Dioxus**: 0.1 → 0.6 over the same period with frequent point releases.

Floem's release cadence is an outlier among comparably-mature Rust UI projects.

## Why the 18-month silence?

Direct evidence is incomplete; the inferred explanation is:

1. **Lapce consumes Floem as a git dependency**, pinned to a specific revision. Lapce doesn't need a crates.io release to ship.
2. **Cutting a crates.io release requires curating breaking changes, writing migration docs, and committing to stability surface.** A no-full-time-devs project routinely defers this.
3. **The 0.2.0 release notes themselves say** "nearly a year of work" — suggesting that even 0.2.0 represented a backlog of changes finally cut into a release after an extended delay.
4. **The `lapce/winit` fork and `understory_*` sister crates** lock external Floem users into the broader Lapce-team ecosystem. There's no pressure from an independent Floem user base.

This is **not the same** as project death. It is project-not-managed-for-external-users. For Buiy: the lesson is about *release discipline being a separate concern from code health.* See [`critiques.md`](critiques.md).

## Floem's name and identity

"Floem" is the Dutch word for the *phloem* of a plant (the tissue that transports nutrients). The naming convention follows Lapce's plant theme (Lapce = "lapis lazuli"-adjacent; the Lapce team uses plant / mineral metaphors). The logo (redesigned for 0.2.0) features a stylized leaf.

## Lineage in the broader Rust UI space

Floem's design draws from three named sources:

1. **Xilem** — Linebender's reactive UI experiment. The view-tree-as-functions pattern.
2. **Leptos** — Rust web framework with `leptos_reactive` (the signal runtime Floem ports).
3. **rui** — Audulus's experimental Rust GUI (immediate-mode-ish with reactive elements).

All three lineages converge in Floem: signals + view functions + native rendering. The combination is the contribution; the individual ideas are not novel.

## Active-vs-archived final word

As stated in [`README.md`](README.md): the repo is **active** (last commit 2026-05-11). The crates.io trajectory looks dead. The accurate frame is "active for Lapce; effectively a private-dependency for everyone else." A Buiy designer should treat Floem the way one treats a library held captive to a single downstream — useful to learn from, risky to depend on.

## Sources

- Floem repo — https://github.com/lapce/floem
- Floem releases page — https://github.com/lapce/floem/releases
- Floem 0.2.0 release notes — https://github.com/lapce/floem/releases/tag/v0.2.0
- Lapce repo — https://github.com/lapce/lapce
- Lapce site — https://lapce.dev
- InfoQ piece (2024-03) — https://www.infoq.com/news/2024/03/lapce-rust-editor/
- HN thread (2024-02) "Lapce dev here" — https://news.ycombinator.com/item?id=39423493
- HN thread Lapce general — https://news.ycombinator.com/item?id=39421090
