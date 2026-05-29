**Date:** 2026-05-22
**Status:** active
**Subject:** woodpecker_ui — the kayak_ui lineage, transition, and release timeline

# History

## The kayak_ui → woodpecker_ui lineage

**Verified.** Both crates are published by the same crates.io user (StarArawn / John, GitHub user id `6656977`). The woodpecker_ui README's Q3 (verbatim) makes the lineage explicit:

> *"Q3. What about Kayak UI?*
> *You might notice the syntax used here is quite similar to Kayak UI, but Kayak UI suffered from overly complicated internals. It made contributing to Kayak UI much too difficult and caused quite a few fundamental bugs. In Woodpecker UI I took what made Kayak UI great and made the backend much much simpler. As an example the primiary system that runs the UI was over 1k lines in Kayak and in Woodpecker its less than 200! This should help foster collaborative development and encourage people to help fix bugs!"*
>
> — `https://github.com/StarArawn/woodpecker_ui/blob/main/README.md`

The transition is a deliberate rewrite, not a fork: the GitHub repo `StarArawn/woodpecker_ui` was created 2024-07-18 (fresh repo; not a `git clone` of `kayak_ui`), five months after kayak_ui's last release (2024-02-11, version `0.5.0`).

## kayak_ui timeline (predecessor)

| Date | Event | Source |
|---|---|---|
| 2022-11-13 | `kayak_ui` 0.1.0 first published to crates.io | crates.io API |
| 2022-12-11 | `kayak_ui` 0.2.0 (Bevy 0.9 era) | crates.io API |
| 2023-04-16 | `kayak_ui` 0.3.0 (Bevy 0.10) | crates.io API |
| 2023-04-30 | `kayak_ui` 0.4.0 | crates.io API |
| 2023-05-02 | `kayak_ui` 0.4.1 | crates.io API |
| 2024-02-11 | `kayak_ui` 0.5.0 — **last published version**; Bevy 0.12 | crates.io API |
| (silent) | Bevy ships 0.13, 0.14, 0.15, 0.16 without a kayak_ui release | — |
| (open) | kayak_ui repo not archived; effectively abandoned | GitHub repo (verified not archived as of 2026-05-22) |

**kayak_ui lifetime downloads:** 18,774 (fetched 2026-05-22). Roughly 17× woodpecker_ui's count — kayak_ui was small but had real adoption during 2022–2024. The custom proc-macro KAYAK had (markup-style `rsx!`-flavored macros) attracted some game-jam usage.

**kayak_ui architecture (per repo README, for context):** custom rsx-like proc-macro, **morphorm** layout (not Taffy), MSDF text rendering, opacity layers, custom render passes through `bevy_render`. None of this stack is in woodpecker_ui — every subsystem was rewritten.

## Why a new project instead of continuing kayak_ui

The README Q3 names two reasons:
1. **Overly complicated internals.** The 1k-line widget runner in kayak_ui was load-bearing and difficult to change; rewriting from scratch was less work than refactoring.
2. **Fundamental bugs.** Long-tail bugs in kayak_ui were attributable to the runtime complexity itself — not feature-by-feature failures.

The architectural deltas are large enough that "rewrite" is correct framing, not "rebrand":

| Subsystem | kayak_ui | woodpecker_ui |
|---|---|---|
| Authoring | `rsx!`-style proc-macro | `#[derive(Widget)]` + `WidgetChildren` builder |
| Layout | morphorm | Taffy 0.7 |
| Text | MSDF font renderer (custom) | Parley 0.4 + skrifa |
| Renderer | Bevy `bevy_render` pipeline (custom UI pass) | `bevy_vello` (vello scenes) |
| State | Kayak's own context system | React-style `use_state` hooks |
| Children | Markup nested via macro | `WidgetChildren` fluent builder |
| Picking | Custom event listener plugin | `bevy_picking` backend |
| Bevy compat | Tied to Bevy minor releases (followed 0.9 → 0.12) | Pinned to Bevy 0.16 (since 2025-05) |

The README Q2 also rejects the broader category of non-ECS UI crates: *"They tend to want ownership of the data which means it must live outside of bevy's ECS world. I have problems with this."* This positions woodpecker_ui inside the ECS-first design space (alongside `bevy_ui`) and against egui-/iced-flavored runtime-owned trees.

## woodpecker_ui timeline

| Date | Event | Source |
|---|---|---|
| 2024-07-18 | GitHub repo `StarArawn/woodpecker_ui` created | GitHub API `created_at` |
| 2024-07 → 2025-05 | ~10 months of development on `main`, no crates.io releases | inferred from repo `created_at` vs publish date |
| 2025-05-31 15:50 UTC | `woodpecker_ui` 0.1.0 published to crates.io | crates.io API |
| 2025-05-31 22:42 UTC | `woodpecker_ui` 0.1.1 published (same day, bug fix) | crates.io API |
| 2025-06-07 | Last commit pushed to `main` (`pushed_at`) | GitHub API |
| 2025-06 → 2026-05 | ~12 months silent on commits and releases | inferred |
| 2026-04-01 | Repo `updated_at` (badge/issue metadata, not commits) | GitHub API |
| 2026-05-22 | This corpus written | — |

**`0.1.0` → `0.1.1` delta:** crates.io diff between the two versions is small (`0.1.0` 9,288 LoC; `0.1.1` 9,289 LoC). Functionally a same-day patch, not a significant point release.

## Pattern: the second-system trap?

The Q4 of the README is also worth recording — it's the maintainer's stated reason for *not* waiting for upstream Bevy UI improvements:

> *"Q4. Why not wait for the next-gen Bevy UI? Why make your own?*
> *1. There is no timeline for when this might come out.*
> *2. There are a lot of conflicting opinions about how the next-gen Bevy UI should work. In my opinion there isn't a clear direction(yet although its starting to form). [...]*
> *3. So far I'm personally not a huge fan of using scenes and also the new BSN macro. From what I've seen it has some problems around not using rust syntax, data management, [...]*
> *4. I apparently really like writing UI crates."*

The honest fourth bullet is the operative one: woodpecker_ui exists because its author enjoys building UI crates, not because there's an outside demand strong enough to sustain it. This is consistent with the small lifetime download count (1,077 vs kayak_ui's 18,774 over a comparable time window) and the ~12-month commit silence after the first release.

**Pattern in the Bevy ecosystem.** Solo-maintainer Bevy UI crates have a recurring lifecycle: enthusiastic 6–18 month development burst → first release → maintenance fatigue / Bevy-version-migration tax / author moves on → effective abandonment. kayak_ui (2022-11 → 2024-02, ~15 months) and woodpecker_ui (2024-07 → 2025-06, ~11 months active) both fit this shape. For Buiy adoption decisions, this is the relevant base rate. See [`critiques.md`](critiques.md) and [`lessons.md`](lessons.md).

## A note on `kayak-ui/` prior-art folder

This corpus cross-references `kayak-ui/` for predecessor context. As of 2026-05-22, **`docs/prior-art/kayak-ui/` does not exist** in this repo. If kayak_ui is later added as its own prior-art folder, this file should be updated to cross-link `../kayak-ui/history.md` and merge timeline overlaps. The kayak_ui-specific facts in this file (license "non-standard" on crates.io, morphorm/MSDF stack, downloads count) are the minimum needed to make the lineage navigable without that folder.

## Sources

- woodpecker_ui crates.io — https://crates.io/crates/woodpecker_ui (versions 0.1.0, 0.1.1; both published 2025-05-31)
- kayak_ui crates.io — https://crates.io/crates/kayak_ui (versions 0.1.0 through 0.5.0; last release 2024-02-11)
- woodpecker_ui README Q3 (kayak_ui lineage statement, verbatim) — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/README.md
- kayak_ui README — https://raw.githubusercontent.com/StarArawn/kayak_ui/main/README.md
- woodpecker_ui GitHub repo metadata (`created_at` 2024-07-18, `pushed_at` 2025-06-07, stars 70, forks 4) — https://api.github.com/repos/StarArawn/woodpecker_ui
- Sibling: [`architecture.md`](architecture.md), [`distribution.md`](distribution.md), [`ecosystem.md`](ecosystem.md), [`critiques.md`](critiques.md)
