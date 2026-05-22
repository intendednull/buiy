**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — production users, third-party crates, ecosystem health

## Production users

| App | Status | Notes |
|---|---|---|
| **Lapce** (editor) | Production, confirmed | The flagship. Substantive UI surface: editor view, file tree, panels, status bar, command palette, settings UI, terminal, debugger UI. |
| **Lapdev** | Production, Lapce-team | Cloud dev environment built by the Lapce team using Floem. Mentioned in Lapce ecosystem materials. |
| Others | Unverified | No widely-known external production apps. The community wiki / showcase is thin. |

The accurate frame: **Floem has one major production user, and that user is the same team that maintains the library.** This is single-flagship dogfooding — useful (catches real bugs) but not the same as ecosystem traction.

For comparison:

- **egui**: dozens of production apps (Rerun, Rust Analyzer's experimental UI, many indie tools).
- **Iced**: COSMIC desktop apps (significant), various indie tools.
- **Dioxus**: production web/desktop apps from Dioxus Labs customers; several public showcases.
- **Slint**: industrial-embedded customers (SlintPad, Edge UI shipping in commercial products).
- **Floem**: Lapce, Lapdev. That's the list.

## Third-party crates

The Floem ecosystem outside Lapce is small but exists:

- **`floem-ui-kit`** (pieterdd) — ready-to-use widgets on top of Floem. Independent of the Lapce team.
- **`understory`** — Lapce-team utility crates (box-tree, focus). Used by Floem itself.
- **`floem_*_renderer`** sister crates — published alongside Floem; usable independently in narrow cases but practically only useful with Floem.

That is essentially the discoverable ecosystem. There is no equivalent of Dioxus's plugin gallery, Iced's `iced_aw`, or egui's extension crate explosion.

## Comparison to peer ecosystems

| Library | Third-party widget crates | Plugins | Themes |
|---|---|---|---|
| **egui** | Many (`egui_extras`, `egui_plot`, `egui_node_graph`, etc.) | Many | Several |
| **Iced** | `iced_aw`, several themes | Some | Several |
| **Dioxus** | `dioxus-charts`, `dioxus-router`, ecosystem packages | Many | Some |
| **Floem** | `floem-ui-kit` (one) | None | Built-in themes only |

This isn't a value judgment — Floem is younger and smaller. But it does mean a Buiy designer looking for "what's the typical Floem-user experience for X widget?" will not find a community answer. The answer is "look at Lapce, or write it yourself."

## What Lapce demonstrates about Floem

Lapce is a substantial proof point. The editor includes:

- Multi-pane layout with splits, tabs, scrolling.
- Tree views (file explorer, problems panel).
- Modal command palette with fuzzy filter.
- Settings UI with hundreds of options across categories.
- Embedded terminal.
- LSP-driven completions, hovers, signature help (rich tooltips).
- Vim mode with modal editing.
- Multi-cursor editing.
- Debugger UI (variables, watches, breakpoints).
- Status bar with extension indicators.

All of this is built in Floem. Anyone evaluating Floem can install Lapce, exercise these surfaces, and learn what Floem can do at scale. **This is the single best signal Floem has** — and it's why "active" is the right status verdict even with the dead-looking crates.io trajectory.

## Documentation and learning resources

- **docs.rs API reference** — exists, version-pinned to 0.2.0 (so it lags `main`).
- **Examples folder** — 27 examples covering core patterns (counter, todo, editor, themes, animations, virtual_list, etc.). The de facto tutorial.
- **README** — feature list + getting-started snippet. ~100 lines.
- **No book.** No long-form tutorial. No video courses.
- **GitHub Discussions** — active enough for ad-hoc Q&A.

This is thin for a UI library intended for outside reuse. Compare to Leptos's full book, Dioxus's docs.dioxuslabs.com guide, egui's docs.rs structure with running examples.

## Cross-link: comparisons

See [`comparisons.md`](comparisons.md) for the side-by-side with Dioxus / Xilem / Iced / Solid.js / Buiy.

## Sources

- Lapce — https://github.com/lapce/lapce
- Lapdev — https://lapdev.net (Lapce-team's cloud dev environment)
- `floem-ui-kit` — https://github.com/pieterdd/floem-ui-kit
- Floem examples — https://github.com/lapce/floem/tree/main/examples
- docs.rs/floem — https://docs.rs/floem/latest/floem/
