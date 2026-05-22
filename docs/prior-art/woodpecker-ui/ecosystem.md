**Date:** 2026-05-22
**Status:** active
**Subject:** woodpecker_ui — production usage, comparisons to peer Bevy UI crates

# Ecosystem & comparisons

## Production usage

**Effectively none verified.** Search of crates.io reverse-dependencies on 2026-05-22 returns no published crates depending on `woodpecker_ui`. Community references (Bevy Discord, /r/bevy, Bevy assets directory) place woodpecker_ui in the "tried it for a game jam" or "interesting experiment" category — not the "shipping a product" category.

Even the predecessor kayak_ui (18,774 lifetime downloads) had only sparse shipping uses, none flagship-class — see [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Avoid-row "No flagship game = no UX battle-testing." woodpecker_ui has 17× fewer downloads.

The honest framing: **woodpecker_ui is a single-author exploratory crate**, not a deployment-ready library. Treat it as architectural reference (good substrate choices: vello + Parley + Taffy) and lineage data (kayak_ui → woodpecker_ui transition; see [`history.md`](history.md)) — not as an adoption target.

## Where it sits in the third-party UI landscape

| Crate | Renderer | Layout | Text | Authoring | Adoption (DLs) | Status (2026-05-22) |
|---|---|---|---|---|---|---|
| `bevy_ui` | bevy_render UI pass | Taffy | Parley (0.19-dev) / cosmic-text (≤0.18) | ECS + BSN (in-flight) | huge (in-tree) | active in-tree |
| `bevy_feathers` | bevy_ui | Taffy (via bevy_ui) | bevy_text | bevy_ui_widgets | 191,700 | active in-tree, experimental |
| `bevy_lunex` | bevy_render (custom 2D / 3D anchored) | own layout | bevy_text | ECS + builder | mid (~k-class) | active third-party |
| `sickle_ui` | bevy_ui | Taffy (via bevy_ui) | bevy_text | builder DSL | mid (~k-class) | dormant (last release 2024) |
| `kayak_ui` | bevy_render (custom MSDF) | morphorm | MSDF custom | rsx-style proc-macro | 18,774 | effectively abandoned |
| `woodpecker_ui` | bevy_vello (vello scenes) | Taffy 0.7 | Parley 0.4 | derive-macro + hooks | 1,077 | release-silent since 2025-05 |
| `bevy_egui` | bevy_render | egui (immediate-mode) | egui fonts | egui macros | very high | active third-party |
| `iyes_ui` (suite) | bevy_ui or its own | varies | varies | varies | small | small-author crates |

woodpecker_ui's distinctive position: **the only mainline-ish Bevy UI crate using `bevy_vello` as its renderer.** That choice — leveraging vello's path-rendering capabilities — is what makes the crate architecturally interesting to Buiy even though its adoption is small.

## Comparisons

### vs `bevy_ui` (the in-tree default)

| Dimension | woodpecker_ui | bevy_ui |
|---|---|---|
| Renderer | vello scenes via `bevy_vello` | custom UI render pass with rect-only clip |
| Rounded clip / clip-path / backdrop-filter | available via vello primitives (not exposed) | absent, blocked on architectural redesign (issue #22345) |
| Authoring | `#[derive(Widget)]` + `WidgetChildren` builder | spawn `Node` + companion components; BSN in-flight |
| Reactivity | dirty-bit `update() -> bool` + hooks | observers + change detection |
| Accessibility | none (no AccessKit) | `bevy_a11y` adapter (megacomponent, see [`bevy-ui/lessons.md`](../bevy-ui/lessons.md)) |
| Theme system | per-widget `*Styles` structs | none in `bevy_ui`; `bevy_feathers` layers tokens |
| Widget set | ~12 game-UI widgets | bevy_ui itself ships node primitives; `bevy_feathers` adds 8 styled widgets |
| Bevy version compat | locked at 0.16 | tracks main |
| Adoption | 1,077 DLs | engine-bundled |

**Implication for Buiy:** woodpecker_ui validates the *parallel-stack feasibility* — you can ship a usable Bevy UI without consuming `bevy_ui`. It does not validate the *parallel-stack maintenance cost* — woodpecker_ui's release silence is the warning sign there.

### vs `bevy_lunex` (the closest design-space peer)

Both are parallel-to-`bevy_ui` third-party stacks. Differences:

- **Lunex** focuses on **game-style UI with 3D-anchored / worldspace UI as a first-class concern**. Its own layout engine; its own rendering through `bevy_render`. Stronger production use in indie games.
- **woodpecker_ui** focuses on **declarative reactive authoring**, taking standard substrate components (Taffy, Parley) and a different rendering backend (vello). Smaller production use.

Both share the maintainer-bus-factor risk. Neither addresses AccessKit; neither targets WCAG.

For Buiy: lunex's lesson is "worldspace UI is achievable in a parallel stack" (see [`../bevy-lunex/architecture.md`](../bevy-lunex/architecture.md)); woodpecker_ui's lesson is "vello-as-Bevy-UI-renderer is feasible." Different findings, both confirmatory of the parallel-stack architecture.

### vs `bevy_feathers` / `bevy_ui_widgets` (the in-tree styled widget set)

`bevy_feathers` runs *on* `bevy_ui` and inherits its render-graph caps ([`../bevy-feathers/`](../bevy-feathers/)). woodpecker_ui doesn't, and gets vello's path-render capabilities for free — but ships no APG-compliant widget surface.

The tradeoff is symmetric:
- Feathers: ~8 styled widgets, accessibility-integrated (via `bevy_ui_widgets`), in-tree update cadence, but renderer-capped.
- woodpecker_ui: ~12 widgets, no accessibility, solo-maintained, but renderer-capable.

Neither is the right target for Buiy. Buiy's commitment is to **own the render pipeline + ship AccessKit-first widgets** — taking the renderer side from somewhere closer to woodpecker_ui's vello stance and the accessibility side from somewhere closer to feathers' (or to AccessKit directly).

### vs `sickle_ui` (the prior third-party tokens-and-builder play)

sickle_ui (last release 2024, also effectively dormant) ran on `bevy_ui` and provided a tokens-driven builder DSL with a substantial widget catalog and theming model. Its abandonment is a separate cautionary tale — see [`../sickle-ui/`](../sickle-ui/) once it lands (not yet present in this repo).

woodpecker_ui is its near-contemporary on a *different* substrate (`bevy_vello`) but with a *smaller* widget catalog and *no* theming. Both are now release-silent. The base rate is consistent: solo-author Bevy UI crates do not survive past ~15 months of active development.

### vs `kayak_ui` (the predecessor)

See [`history.md`](history.md). The most important comparison:

| Subsystem | kayak_ui | woodpecker_ui |
|---|---|---|
| Author | StarArawn | StarArawn (same person) |
| Total downloads | 18,774 | 1,077 |
| Last release | 2024-02-11 (Bevy 0.12) | 2025-05-31 (Bevy 0.16) |
| Lifecycle | ~15 months active | ~11 months active to date |

The Q3 of the woodpecker_ui README ([`history.md`](history.md)) is the author's own evaluation of what went wrong with kayak_ui (*"overly complicated internals"*) and what's better about woodpecker_ui (*"the primary system that runs the UI was over 1k lines in Kayak and in Woodpecker its less than 200"*). The rewrite is real and architecturally improved. The adoption signal does **not** suggest the rewrite has fixed the abandonment dynamic.

### vs Buiy

| Dimension | woodpecker_ui | Buiy (target state) |
|---|---|---|
| Scope | game-UI helper widgets | full web platform parity + APG + WCAG 2.2 AA |
| Renderer | `bevy_vello` (vello as compositor) | own render pipeline on Bevy render graph; vello as inspiration for capability set |
| Layout | Taffy | Taffy (same substrate) |
| Text | Parley | cosmic-text (foundation arch § 2.2; see [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) finding #2) |
| Accessibility | none | AccessKit-first; replaces `bevy_a11y` per-window |
| Theme | per-widget structs | semantic tokens, OS-preference binding, contrast lint |
| BSN | structurally hostile (megacomponent style) | BSN-friendly by construction |
| Reactivity | `update()` dirty bit + hooks | observers + change detection (signal layer optional later) |
| Bevy compat | locked at 0.16 | rolling latest-stable |
| Authoring | `#[derive(Widget)]` macro | plain Bevy components + BSN templates |
| Maintenance | solo, release-silent | TBD |

**Useful borrowable elements** from woodpecker_ui for Buiy: vello-as-render-substrate inspiration, dioxus-devtools hot-reload pattern, ECS-first widget-typing via `bevy-trait-query`. See [`lessons.md`](lessons.md) Borrow section.

## Sources

- crates.io reverse-dependency listing (none verified) — https://crates.io/crates/woodpecker_ui/reverse_dependencies
- Bevy ecosystem references (informal, Discord / r/bevy)
- woodpecker_ui README Q1, Q3, Q4 (positioning) — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/README.md
- bevy_ui megacomponent / renderer-cap lessons — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- bevy_feathers comparison — [`../bevy-feathers/`](../bevy-feathers/)
- bevy_lunex (partial folder) — [`../bevy-lunex/`](../bevy-lunex/)
- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling: [`history.md`](history.md), [`critiques.md`](critiques.md), [`lessons.md`](lessons.md)
