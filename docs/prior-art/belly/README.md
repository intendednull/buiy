**Date:** 2026-05-22
**Status:** active (with staleness flag — see § Maintenance status)
**Subject:** belly — third-party Bevy UI plugin with HTML-like `eml!` markup, CSS-like `ess` stylesheets, and data binding. Strongest existing design precedent for "stylesheet on Bevy UI."

# belly

`belly` is a declarative-UI plugin for the Bevy game engine. It packages three ideas in one crate: an HTML-like markup macro (`eml!`), a CSS-like stylesheet asset (`.ess`), and a reactive data-binding system (`from!` / `to!` / `connect`). It is the closest existing implementation in the Bevy ecosystem to "the web platform's authoring trifecta" — markup + stylesheet + bindings.

It matters to Buiy for one reason. Foundation [README.md § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) keeps the question **"CSS-flavored stylesheet — never, or as a future layer above tokens?"** open. `bevy_flair` is one published precedent for the stylesheet side; `belly` is the older, broader-scope precedent. Where bevy_flair documents "what CSS in Bevy looks like as a standalone layer," belly documents "what CSS *plus* HTML *plus* reactive bindings looks like as a single framework." Both folders feed the same decision.

## Key facts

| Fact | Value |
|---|---|
| Repository | https://github.com/jkb0o/belly |
| Maintainer | `jkb0o` (single-maintainer) |
| License | MIT OR Apache-2.0 (dual) |
| Latest release tag | **v0.5.0** (2024-04-20) |
| Latest commit on `main` | 2024-04-20 (same as v0.5.0) — no activity since |
| Stars / forks | 436 / 32 (as of 2026-05-22) |
| Crates.io | **Not published.** Git dep only. README: `"As far as the project has no cargo release yet…"` |
| Bevy version supported | **0.13** (April 2024). Bevy is now at 0.18 → belly is ~5 majors stale |
| Open issue tracking newer Bevy | #83 "Need help with updating to Bevy 0.14" (2024-07-19), unresolved |
| Workspace | `belly` + `belly_core` + `belly_macro` + `belly_widgets` + `bevy_stylebox` + `tagstr` |
| Example count | 27 (`button`, `color-picker`, `connections`, `counter-binds`, `counter-signals`, `grid`, `for-loop`, `selectors`, `slider`, `style-sheet`, `tabview`, `text-input`, …) |

## Maintenance status

**Honest read: effectively unmaintained as a runtime dependency.**

- No commits to `main` since 2024-04-20 (~25 months as of 2026-05-22).
- v0.5.0 (April 2024) was the last release. It tracks Bevy **0.13**.
- Bevy has since shipped 0.14, 0.15, 0.16, 0.17, 0.18. None are supported by any tagged or unreleased belly.
- The only known migration request (issue #83, Bevy 0.14, opened 2024-07-19) is open and unanswered.
- Repo is **not** formally archived; the GitHub flag is unset and the README contains no archive notice.
- Single maintainer (`jkb0o`) — bus factor 1.

The corpus carries `Status: active` to match upstream's own GitHub flag, but every consuming file in this folder marks the staleness explicitly. A future reader **must not** treat belly as a working Bevy-0.18 dependency without independent migration work.

## Contents

- [`architecture.md`](architecture.md) — Workspace shape, `BellyPlugin`, how `eml!` / `ess` / bindings compose, what belly adds on top of bevy_ui.
- [`eml-macro.md`](eml-macro.md) — The HTML-like authoring macro: elements, attributes, children, slots, `for` loops, `bind:` / `on:` attributes, lack of scripting.
- [`ess-stylesheets.md`](ess-stylesheets.md) — The CSS-like styling system: parser scope, selectors, properties, cascade, hot-reload, `c:` class notation. Compared to bevy_flair throughout.
- [`data-binding.md`](data-binding.md) — `from!` / `to!`, `connect()` / `on()` / `handle()`, the `run!` macro, signals, transformers.
- [`history.md`](history.md) — Project genesis, 0.x evolution from Bevy 0.10 → 0.13, the crates.io question, the 2024 stall.
- [`distribution.md`](distribution.md) — Git-only-dep status, Bevy version compatibility, license, single maintainer, bus factor.
- [`critiques-and-open-problems.md`](critiques-and-open-problems.md) — No crates.io presence; single maintainer; HTML-as-DSL friction in ECS; macro hygiene; cascade resolution cost; the Bevy version migration tax; APG/WCAG coverage absent; BSN-compat absent; AccessKit absent.
- [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md) — Production usage (none verifiable); vs bevy_flair (published, scoped to styles only); vs sickle_ui (archived, ECS-builder); vs bevy_ui programmatic; vs Buiy's parallel-stack stance.
- [`lessons.md`](lessons.md) — **The decision file.** Validates / Avoid / Borrow for Buiy.
- [`glossary.md`](glossary.md) — `eml!`, `ess`, `BellyPlugin`, `from!`, `to!`, `connect`, `run!`, `c:` notation, `s:`-prefixed attributes, transformer.

## Recommended reading order

1. This README — get oriented.
2. [`lessons.md`](lessons.md) — if you only have 10 minutes, this is the file.
3. [`architecture.md`](architecture.md) — how the three pieces compose.
4. [`eml-macro.md`](eml-macro.md) + [`ess-stylesheets.md`](ess-stylesheets.md) — the authoring surfaces.
5. [`critiques-and-open-problems.md`](critiques-and-open-problems.md) — what doesn't work.
6. [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md) — placement vs siblings.
7. [`data-binding.md`](data-binding.md), [`history.md`](history.md), [`distribution.md`](distribution.md), [`glossary.md`](glossary.md) — reference depth.

## Framing disclosure

These docs are written from a **parallel-to-bevy_ui, BSN-native, AccessKit-first, token-based-theming** stance — the Buiy foundation spec's commitments. Most "Implications for Buiy" framing in [`lessons.md`](lessons.md) and the critiques file reads belly's choices through that lens. Specifically, the corpus treats single-maintainer git-dep status as a strict avoid (because Buiy is a foundation library, not an app), and reads belly's `eml!` HTML-as-DSL choice through the Buiy commitment to BSN authoring. Future readers auditing whether the Buiy stance is itself the right primitive should weigh the corpus accordingly: it's a learn-from-belly-into-Buiy artifact, not a neutral catalog of declarative-UI options.

## Sources

- belly repository — https://github.com/jkb0o/belly
- belly v0.5.0 README — https://github.com/jkb0o/belly/blob/v0.5.0/README.md
- belly releases — https://github.com/jkb0o/belly/releases
- belly issue #83 (Bevy 0.14 migration request) — https://github.com/jkb0o/belly/issues/83
- crates.io check (`belly` does not exist) — https://crates.io/api/v1/crates/belly returns `crate does not exist`
- Buiy foundation README open question — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 5
- bevy_flair prior-art folder — [`../bevy-flair/`](../bevy-flair/)
- bevy_ui prior-art folder — [`../bevy-ui/`](../bevy-ui/)
- sickle_ui prior-art folder — [`../sickle-ui/`](../sickle-ui/)
