**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — adoption (canonical dev tool, indie games, editor experiments), egui's own ecosystem, adjacent crates

# Ecosystem

bevy_egui is the **most-adopted third-party Bevy UI plugin** by download count (2,020,092 lifetime, 286,785 last-90-day on 2026-05-22 — see [`distribution.md`](distribution.md)). This file walks where it's actually used in production, the broader egui ecosystem above and beside it, and the adjacent crates that depend on bevy_egui specifically.

Honest framing up front: bevy_egui's adoption is **overwhelmingly tools and debug overlays**, not production game UI. The 2M-download number measures dev-loop usage more than shipped-product UI. See [`critiques.md`](critiques.md) § "The 'dev tool, not production UI' framing."

## Canonical consumer: bevy-inspector-egui

The single most important downstream consumer of bevy_egui is **`bevy-inspector-egui`** (Jakob Hellermann).

| | |
|---|---|
| Repository | `https://github.com/jakobhellermann/bevy-inspector-egui` |
| Latest version | 0.36.0 (2026-01-14) |
| License | MIT OR Apache-2.0 |
| Lifetime downloads | 1,224,818 |
| 90-day downloads | 146,688 |
| Bevy compat | 0.18 (matching bevy_egui 0.39) |

This crate is the canonical "dev tool" in the Bevy ecosystem — `WorldInspectorPlugin`, `ResourceInspectorPlugin`, asset/entity inspection, reflection-driven UI for arbitrary `Reflect` values. It is the **reason** most Bevy projects pull in bevy_egui at all: every Bevy tutorial that wants to show a debug overlay introduces bevy-inspector-egui, which transitively pulls in bevy_egui. Roughly **60%** of bevy_egui's 90-day download count is plausibly indirect through bevy-inspector-egui (1.22M lifetime inspector vs 2.02M lifetime egui-plugin — the inspector accounts for ~60% of the integration crate's downloads, and the ratio is consistent over the recent 90-day window).

The dependency relationship is structurally important: bevy-inspector-egui is the *killer app* for bevy_egui. Were it to migrate to a different UI substrate (it has explored `bevy_ui` integration historically, never landing), bevy_egui's adoption would drop substantially.

## Bevy editor experiments

The Bevy Foundation has prototyped several editor / scene-tool experiments over 2022–2026. As of 2026-05-22 there is **no official Bevy editor**, but the prototypes that exist use these UI substrates:

- **Early prototypes (2022–2023)** — used bevy_egui as the most-mature option. The "bevy_editor_pls" community plugin used bevy_egui for its panels.
- **Current direction (2024–2026)** — the foundation has signaled the official editor will be built on `bevy_feathers` + `bevy_ui_widgets` (see [`../bevy-feathers/comparisons.md`](../bevy-feathers/comparisons.md)). bevy_egui is *not* on the official-editor roadmap.

This is significant: bevy_egui's editor-tool dominance is a *historical fact* of 2022–2024 that is being gradually displaced by in-tree retained-mode alternatives. The displacement is incomplete — dev tools like bevy-inspector-egui still ship on bevy_egui, and the path off is not visible — but the long-term trajectory is toward retained-mode in-tree solutions for editor-style use cases.

## Indie / hobbyist games shipping with bevy_egui

Game UI shipped on bevy_egui (not debug overlays, not dev tools — actual menus / HUD / settings screens visible to players) is **rarer than the download count suggests**. Patterns observed:

- **Game jams** — bevy_egui is the de-facto choice for Bevy Jam entries that need any UI beyond a single button. The "ship a UI in 48 hours" use case is where immediate-mode is genuinely faster than retained-mode.
- **Hobbyist projects on itch.io** — many small Bevy projects use bevy_egui for menus, often with the unmodified default egui look (the homogeneity problem — see [`critiques.md`](critiques.md) § "Visual homogeneity").
- **Commercial Bevy releases** — the most-cited commercial Bevy release is **Tiny Glade** (Pounce Light, 2024); it wrote its own UI renderer rather than using either bevy_ui or bevy_egui. **No flagship commercial Bevy game ships its production UI on bevy_egui.** This is the load-bearing critique — see [`critiques.md`](critiques.md) and [`open-problems.md`](open-problems.md) § "The production game UI gap."

Verifying "X game uses bevy_egui" is unreliable at this point — most credits don't itemize UI substrate — but the pattern of egui-styled menus in Bevy showcase reels is widespread enough that "hobbyist game UI" is a real adoption category, even if it's not a flagship one.

## egui's own ecosystem (non-Bevy)

egui upstream has its own substantial ecosystem, separate from bevy_egui:

- **eframe** — the "official egui framework" (Ernerfeldt's term). Native + web shell that hosts egui without any game engine. Most non-Bevy egui apps run on eframe; it's what Rerun's viewer uses. Supports Windows / macOS / Linux / Web / Android.
- **egui_web** — historical browser shell, now folded into eframe's WASM target.
- **egui_glow** — OpenGL backend for egui.
- **egui_wgpu** — wgpu backend (the modern default, also the substrate bevy_egui's render path effectively duplicates).

Non-Bevy companies known to use egui in production:

- **Rerun.io** — the canonical production user; viewer built end-to-end on egui (see [`history.md`](history.md) § "Rerun.io stewardship").
- **Embark Studios** — known to use egui in tooling (their open-source `embark-studios` GitHub org has multiple egui-dependent projects, though no shipped *games* use it for production UI).
- Various scientific / robotics tools — egui is a common choice for control-panel-style UIs in the Rust robotics community.

The "egui in production" story is **strong outside games** (Rerun's viewer, scientific tools, internal corporate tooling) and **weak inside games** (no flagship Bevy or non-Bevy commercial game UI). bevy_egui inherits this asymmetry: its strength is dev tools and overlays; its weakness is shipped player-facing game UI.

(Note on a pre-amble entry: "Fortnight Studios" does not appear in egui's documented adopters list and could not be verified — likely a typo for either "Fortnite" (which is C++ Unreal, not egui) or for a different studio name. Correction noted in the final response.)

## Adjacent crates that depend on bevy_egui

A non-exhaustive list of crates that pull in bevy_egui as a dependency:

- **`bevy-inspector-egui`** — the canonical consumer (described above).
- **`bevy_egui_kbgp`** — keyboard / gamepad navigation overlay for bevy_egui. Solves the focus-on-gamepad gap that vanilla egui handles poorly.
- **`bevy_editor_pls`** — community editor-panel plugin (last major release was during the 2022–2023 era; less active now).
- Various game-jam templates and starter kits that include bevy_egui in their default plugin set.

The dependency graph is broad-but-shallow: many crates depend on bevy_egui, but few are *load-bearing* — bevy-inspector-egui is the only one with significant production weight.

## Adjacent egui-ecosystem crates worth naming

egui's own adjacent crate ecosystem (independent of Bevy):

- **`egui_plot`** — plotting / charting widgets. Production-quality, widely used.
- **`egui_extras`** — extra widgets (date picker, table, syntax highlighter) maintained alongside egui upstream.
- **`egui_tiles`** — tiling layout engine published by `rerun-io`. Powers Rerun's multi-pane viewer.
- **`catppuccin/egui`** — third-party theme for egui matching the Catppuccin color palette (an example of theming over egui's `Visuals` system).
- **`egui_flex`** — Flexbox-style layout for egui (egui's native layout is much simpler than CSS Flexbox; this crate adds it).

These are mostly available to bevy_egui consumers too — egui's API surface is shared — but require the consumer to wire them in by hand. bevy_egui does not curate or republish them.

## Adoption summary

| Layer | Adoption | Strength |
|---|---|---|
| egui (upstream) | Production tools, scientific apps, Rerun viewer | Very strong, multi-company |
| bevy_egui (this crate) | Dev tools (canonically bevy-inspector-egui), game jams, hobbyist games | Strong in dev tooling, weak in production game UI |
| bevy-inspector-egui (transitive consumer) | The default Bevy debug tool | Canonical — Bevy tutorials assume it |
| Editor / IDE | Historical (early Bevy editor prototypes) → shifting away | Declining; in-tree retained-mode (`bevy_feathers`) is the official direction |
| Commercial Bevy games | None confirmed shipping production UI on bevy_egui | The major gap |

For Buiy: the bevy_egui ecosystem demonstrates that **adoption-by-download** and **adoption-as-production-UI-substrate** are different metrics. Buiy's success criterion should not be download count but flagship-deployment count. See [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) "No flagship game = no UX battle-testing" row.

## Sources

- bevy_egui crates.io — `https://crates.io/crates/bevy_egui`.
- bevy-inspector-egui crates.io — `https://crates.io/crates/bevy-inspector-egui`.
- bevy-inspector-egui repo — `https://github.com/jakobhellermann/bevy-inspector-egui`.
- egui repo — `https://github.com/emilk/egui`.
- Rerun.io — `https://www.rerun.io/`.
- Embark Studios open source — `https://embark.dev/`.
- egui_tiles (Rerun-published) — `https://github.com/rerun-io/egui_tiles`.
- egui_plot — `https://github.com/emilk/egui/tree/master/crates/egui_plot`.
- catppuccin/egui — `https://github.com/catppuccin/egui`.
- Tiny Glade (custom UI renderer, not egui) — Pounce Light, 2024.
- Bevy editor direction (toward bevy_feathers, not bevy_egui) — `https://bevy.org/news/bevy-0-17/`.
- Sibling files: [`history.md`](history.md), [`critiques.md`](critiques.md), [`open-problems.md`](open-problems.md), [`comparisons.md`](comparisons.md), [`distribution.md`](distribution.md).
- [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § "No flagship game = no UX battle-testing."
