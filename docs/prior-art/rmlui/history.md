**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — chronological history; libRocket genesis, decline, RmlUi fork, release timeline

# History

The combined libRocket + RmlUi history spans **~18 years** (2008 → 2026), divided into a libRocket era (2008–2014, ~6 years active), a dormant period (2014–2018, ~4 years), and an RmlUi era (2018–present, ~8 years active). The fork is a clean case study in **open-source project revival** — a maintainer with no formal organizational tie to the original team picks up an abandoned codebase, rebrands, and ships consistently for ~8 years.

## Era 1 — libRocket (2008–2014)

### Origin

libRocket was founded by **CodePoint Ltd** and **Shift Technology Ltd** (UK + games industry — the original copyright header reads `Copyright 2008-2014 CodePoint Ltd, Shift Technology Ltd, and contributors`). The library was branded as *"the C++ user interface package based on the HTML and CSS standards"* — the same one-line description RmlUi inherits today.

The lead figure most commonly cited from the libRocket era is **Lloyd Weehuizen** (CodePoint Ltd), though the project had a small group of regular contributors. The exact founding-date attribution beyond "2008" is not well-recorded in the public history.

### Design choices established in this era

Almost every architectural fundamental that RmlUi inherits comes from the libRocket era:

- The **embedder-interface pattern** (RenderInterface, SystemInterface, FileInterface) — the load-bearing portability primitive that survived the 2018+ revival intact.
- The **own-layout-engine** decision (no Yoga, no Stretch — neither existed yet).
- **RML + RCSS** as the markup + style languages, with CSS 2.1 + XHTML 1.0 as the spec target.
- **Decorators** as the CSS-`background-image` replacement.
- **Spatial controller navigation** as a first-class feature (the only a11y-adjacent feature in the codebase).
- **Python and Lua scripting bindings** — both shipped in the libRocket era; both subsequently dropped in the RmlUi era as the maintenance cost outweighed usage.

### Notable libRocket-era users

The libRocket era saw adoption in indie + AA studios. **Unvanquished** (a Tremulous-derived open-source first-person shooter) adopted libRocket and has continued onto RmlUi. The id Tech-derived game engine community picked up libRocket for HUD and menu work.

### Decline (~2014)

Activity in `libRocket/libRocket` slows after 2014. The `release-1.2.0` tag (2014-08-25) is the last numbered release; `release-1.3.0.0` (2014-07-10) is described in the repo as *"Pseudo release for dependent projects requiring a static tag to build against"* — i.e., a tag-only convenience, not a feature release.

By 2018 the project is in a state often described in open-source archeology as **soft-dormant**: code exists, repo has issues + PRs that go unanswered, no scheduled release, no formal end-of-life announcement. The community continues to use it because there is no obvious successor.

### What libRocket left RmlUi

A mature codebase with a settled architecture, multiple shipping commercial users, a documented embedder pattern, an MIT license that survives the fork intact, and ~6 years of bug-fixes-against-real-games baked into the layout engine. The libRocket-era codebase is the single most-important reason RmlUi could ship a *"new project"* (rebranded as 2.0 in 2019) in under 18 months from fork to first release — most of the engineering was already done.

## Era 2 — Dormancy (2014–2018)

`libRocket/libRocket` continues as a public archive. Forks accumulate. The community gradually disperses to other UI libraries (Coherent Gameface for AAA, Scaleform's death drives some toward Slate, NoesisGUI rises). The Unity / Unreal ecosystem solidifies around in-engine UI tools.

## Era 3 — RmlUi (2018–present)

### Genesis

**Michael Ragazzon** (GitHub `mikke89`) creates the `mikke89/RmlUi` repository as a fork of libRocket on or around **2018** (exact creation date verifiable via GitHub repo metadata, not fetched here but consistent with the gap to 2019-10 first release). The decision to rebrand as **RmlUi** (Rocket Markup Language UI) rather than continue `libRocket` is a clean break: new name, new maintainer, no formal organizational continuity, but the same MIT license and the same RML + RCSS spec target.

### Release timeline (verified via GitHub releases)

| Release | Date | Highlights |
|---|---|---|
| **2.0** | 2019-10-13 | First official RmlUi release. "Last C++11-compatible release." Repackage + initial cleanup of the libRocket codebase. |
| **3.0** | 2019-11-27 | Major refactor; C++14 baseline. Animations + transitions + transforms. |
| **3.1** | 2019-12-10 | Incremental. |
| **3.2** | 2020-02-01 | Incremental. |
| **3.3** | 2020-06-28 | Incremental. |
| **4.0** | 2021-05-09 | Major. Data binding system expansion; lots of API cleanup. |
| **4.1** | 2021-06-19 | Incremental. |
| **4.2** | 2021-08-23 | Incremental. |
| **4.3** | 2021-12-11 | Incremental. |
| **4.4** | 2022-05-13 | Incremental. |
| **5.0** | 2022-12-11 | **Flexbox layout** added — the single largest layout feature addition in RmlUi's history. |
| **5.1** | 2023-04-07 | Stability + flexbox fixes. |
| **6.0** | 2024-08-26 | **Render interface redesign**: filters, gradients, box-shadows, masks, render layers, custom shaders. CSS custom properties (`--var` / `var(...)`). Largest breaking change since 2.0. |
| **6.1** | 2025-04-20 | Quality: fractional-pixel gaps fixed; `<handle>` edge constraints; new `text` decorator; HarfBuzz kerning. |
| **6.2** | **2026-01-11** | **Latest stable.** Native touch input + inertial scrolling. SVG caching. `font-kerning` and `text-overflow` RCSS properties. Data-model debugging. Emoji rendering improvements. |

### Release cadence observations

- **First two years (2019–2021)**: rapid releases, ~5 per year. Codebase modernization (C++11 → C++14, naming cleanups, API surface settling).
- **Middle period (2022–2024)**: ~2 releases per year. Major feature work concentrated in 5.0 (Flexbox) and 6.0 (effects).
- **Recent (2025–2026)**: ~1 release per year. The 6.x line has shipped one major (6.0) + two minor (6.1, 6.2) over ~17 months.
- The cadence has **slowed**. 6.0 → 6.1 was ~8 months; 6.1 → 6.2 was ~9 months. This is slower than the early RmlUi pace and similar to what libRocket showed before its dormancy.

### Key abandoned features

- **Python bindings** — libRocket shipped these; RmlUi 2.0 dropped them. Reason: maintenance cost vs usage.
- **Lua bindings** — libRocket shipped these; RmlUi 2.0 retained them initially but they're now community-maintained / external.
- Several legacy renderers and platforms from libRocket are not represented in RmlUi's reference backends (e.g., older OpenGL ES targets dropped or merged).

### Key new features (RmlUi-era only)

- **Animations / transitions / transforms** (3.0, 2019-11-27).
- **Data binding system** in current form (4.0, 2021-05-09).
- **Flexbox** (5.0, 2022-12-11).
- **`border-radius`** with full elliptical per-corner support (4.x).
- **Filters / box-shadow / masks / gradients / custom properties** (6.0, 2024-08-26).
- **`text` decorator** + flexbox improvements (6.1, 2025-04-20).
- **Native touch + inertial scrolling + SVG caching + `font-kerning` + `text-overflow`** (6.2, 2026-01-11).

## What is NOT in RmlUi's history

To understand the project's trajectory it helps to enumerate features that never landed:

- **CSS Grid** — never added. No tracking issue committing to it.
- **AccessKit / screen-reader integration** — never started. No mention in any release.
- **HarfBuzz as a built-in font engine** — remains a sample, ~7 years after the RmlUi fork.
- **BiDi paragraph algorithm** — never added to the core path.
- **Container queries** — never added.
- **Anchor positioning** — never added.
- **`backdrop-filter`, `mix-blend-mode`, true CSS top layer** — never added even after 6.0's effects work.
- **`:focus-visible`, `:has()`, `:is()`, `:where()`** — never added.
- **Scroll-driven animations** — never added.
- **Logical properties** — never added.
- **Modern color spaces (lab, lch, oklab, oklch, color())** — never added.

## Implications for Buiy

- **Project revival is possible but slow.** 4 years dormant + 8 years active = 12 years to reach 6.2's feature surface. Buiy's foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) goals (web-platform parity + WCAG 2.2 AA + game and app both) are *substantially* more ambitious than RmlUi's after 18 years of cumulative work. The realistic engineering cost is the foundation spec's tier list itself — anything **F/C** must ship, anything **E** can wait, anything **O** is explicitly excluded.
- **Architectural commitments are sticky.** Every load-bearing decision in RmlUi today (RML + RCSS, decorators, own-layout-engine, embedder pattern, no-a11y) traces to libRocket 2008–2014. The RmlUi fork inherited all of them. Buiy must commit to its foundation choices (Taffy substrate, cosmic-text, AccessKit-first, decomposed components, parallel-to-bevy_ui) **knowing** they will be sticky for the same decade-plus horizon.
- **Single-maintainer revival is fragile.** RmlUi's revival depends on Michael Ragazzon's continued availability. Buiy's reliance on a single primary author (Buiy itself + Bevy itself, both have foundations + multiple maintainers) is structurally less brittle, but the lesson is to *name* the bus-factor explicitly (see [`distribution-and-governance.md`](distribution-and-governance.md) § "Bus factor").
- **Major releases are infrequent and expensive.** RmlUi 5.0 (Flexbox) was 3 years after 4.0. RmlUi 6.0 (effects) was 2 years after 5.0. When Buiy ships a major release with a render-pipeline redesign or a font-shaper migration, expect 6–18 months of breaking change for downstream users. Foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 2.9 "rolling latest-stable Bevy" already implies this; RmlUi's data is consistent with that pace.
- **The 6.0 render-interface redesign is a cautionary tale** for `buiy-render-pipeline-design`. RmlUi's original interface (designed in 2008) couldn't accommodate filters, masks, shadows. Adding them in 6.0 (2024) required a breaking redesign. Buiy must design its render-interface surface (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.3) to accommodate the **full effects vocabulary day one** — filters, masks, `backdrop-filter`, `mix-blend-mode`, top layer compositing.

## Sources

- libRocket GitHub repository — https://github.com/libRocket/libRocket (copyright header, last release tags)
- RmlUi GitHub repository — https://github.com/mikke89/RmlUi
- RmlUi releases (verified 2026-05-22) — https://github.com/mikke89/RmlUi/releases
- RmlUi tags (verified 2026-05-22) — https://github.com/mikke89/RmlUi/tags
- RmlUi changelog — https://github.com/mikke89/RmlUi/blob/master/changelog.md
- RmlUi 2.0 release notes (first official RmlUi release) — https://github.com/mikke89/RmlUi/releases/tag/2.0
- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
