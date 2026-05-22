**Date:** 2026-05-22
**Status:** active
**Subject:** woodpecker_ui — overview, key facts, reading order

# woodpecker_ui

`woodpecker_ui` (crates.io: [`woodpecker_ui`](https://crates.io/crates/woodpecker_ui)) is a third-party reactive UI crate for the Bevy game engine by **StarArawn** (John), maintained as the explicit successor to his earlier **kayak_ui** project. It is **not** part of the Bevy workspace, **not** built on `bevy_ui`, and **not** widely adopted (1,077 lifetime downloads vs kayak_ui's 18,774). What's interesting about it for Buiy is the *architecture stack*: vello-based renderer (via `bevy_vello`), Taffy for layout, **Parley** for text shaping (the same engine Bevy is moving to in 0.19), and an ECS-first declarative widget API with React-style hooks (`use_state`) and Dioxus-style hot reloading (`#[hot]` macro via `dioxus-devtools`). It is a sister-project not-on-bevy_ui that converges on roughly the same substrate Buiy is targeting — and was built by someone who has already shipped (and abandoned) one Bevy UI crate.

## Key facts

| Fact | Value | Source |
|---|---|---|
| Latest stable | `0.1.1` | crates.io API, 2025-05-31 |
| Previous release | `0.1.0` | crates.io API, 2025-05-31 (same day) |
| First commit | 2024-07-18 | GitHub repo `created_at` |
| Last commit pushed | 2025-06-07 | GitHub `pushed_at` |
| Last release | 2025-05-31 | crates.io, `0.1.1` publish time |
| Lifetime downloads | 1,077 | crates.io, fetched 2026-05-22 |
| Recent downloads (90-day) | 6 | crates.io, fetched 2026-05-22 |
| GitHub stars | 70 | GitHub API, 2026-05-22 |
| Forks | 4 | GitHub API |
| Open issues | 8 | GitHub API |
| License | MIT OR Apache-2.0 | `Cargo.toml`, crates.io |
| Repo | https://github.com/StarArawn/woodpecker_ui | GitHub search |
| Maintainer | StarArawn (John, github id 6656977) | crates.io publishers list |
| Bevy version pinned | **0.16** | `Cargo.toml` on `main` |
| Layout engine | Taffy 0.7 (flexbox + grid) | `Cargo.toml` |
| Text engine | Parley 0.4 + skrifa 0.30 | `Cargo.toml` (doc-comment still mentions cosmic-text — pre-migration residue) |
| Renderer | `bevy_vello` 0.9 (vello scenes) | `Cargo.toml` |
| Hot reload | `dioxus-devtools` 0.7.0-alpha.0 (opt-in `hotreload` feature) | `Cargo.toml`, README |
| Picking | own `picking_backend` registered to `bevy_picking` | `src/lib.rs` |
| Crate size | 8.47 MB (embedded fonts + SVG icons) | crates.io |
| Code size | ~9,290 Rust lines (56 files) | crates.io linecounts |
| Workspace crates | `woodpecker_ui` + `woodpecker_ui_macros` (proc-macros) | `Cargo.toml` |

## Honest staleness assessment

woodpecker_ui has been **release-silent for ~12 months** as of 2026-05-22. The last published version is `0.1.1` (2025-05-31). The last commit to the repo was 2025-06-07 — one week after the only release. The repo's GitHub metadata updates (2026-04-01) reflect badge/issue activity, not pushes.

This is less catastrophic than fully abandoned (kayak_ui's last release was Feb 2024 — that pattern would have woodpecker_ui at ~3-year silence by now), but the gap is structurally significant:

- **Bevy 0.16 → 0.18.1 drift.** Pinned at Bevy `0.16` in `Cargo.toml`. Bevy is on 0.18.1 stable + 0.19-rc as of 2026-05-22 ([`bevy-feathers/distribution.md`](../bevy-feathers/distribution.md)). Two Bevy minor releases of unabsorbed migration — and Buiy commits to "rolling latest-stable" (foundation README goal 5). Anyone adopting woodpecker_ui today inherits the migration tax.
- **Parley 0.4 → 0.9 drift.** Bevy's 0.19-dev migration is on Parley `0.9.0` ([`bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Top-of-file finding #2). woodpecker_ui is on `0.4`. Same shaper family, but ABI/API distance is real.
- **Single-maintainer bus factor.** StarArawn is the sole publisher of both kayak_ui and woodpecker_ui crates. No co-maintainers verified. See [`distribution.md`](distribution.md).

The right frame for Buiy is **architectural reference, not adoption candidate** — read it to learn what a vello+Parley+Taffy+`bevy_picking` Bevy UI stack looks like in practice, then build our own.

## Table of contents

- [`architecture.md`](architecture.md) — Plugin shape, render pipeline (vello), reactivity model, module layout.
- [`api.md`](api.md) — `#[derive(Widget)]` macro, hooks, widget vocabulary, style component.
- [`integration.md`](integration.md) — Setup, Bevy compat, coexistence with `bevy_ui`, custom widget extension.
- [`history.md`](history.md) — The **kayak_ui → woodpecker_ui** lineage (verified) and 0.1.0/0.1.1 timeline.
- [`distribution.md`](distribution.md) — License, MSRV, Bevy pin, platform support, governance, bus factor.
- [`critiques.md`](critiques.md) — Pre-1.0 status, small adoption, APG/WCAG coverage gaps, open problems.
- [`ecosystem.md`](ecosystem.md) — Production usage (effectively none), comparisons (vs bevy_ui / lunex / feathers / sickle / kayak / Buiy).
- [`lessons.md`](lessons.md) — **The consult-this-when-designing file.** Validates / Avoid / Borrow for Buiy.
- [`glossary.md`](glossary.md) — woodpecker-specific terms.

## Framing disclosure

These docs are written from a **Buiy-stance** — parallel-to-`bevy_ui`, web-platform-parity, WCAG 2.2 AA-floor, BSN-friendly, AccessKit-first. The "Implications for Buiy" sub-sections frame woodpecker_ui's choices through that lens. The corpus is also written from a **small-adoption third-party** stance: where woodpecker_ui's release silence or solo-maintainer posture matters, the lessons file calls that out instead of soft-pedaling. Future readers auditing whether Buiy's parallel-stack bet is itself correct should weigh this corpus as a learn-from-a-fellow-non-`bevy_ui`-traveler artifact, not a neutral catalog.

## Glossary stub

See [`glossary.md`](glossary.md). Quick anchors:

- **WoodpeckerUIPlugin** — entry-point Bevy plugin (`app.add_plugins(WoodpeckerUIPlugin::default())`).
- **WoodpeckerApp / WoodpeckerView / Element** — root, camera marker, generic widget container.
- **`#[derive(Widget)]`** — proc-macro on a user component; pairs with `#[auto_update(render)]` or `#[widget_systems(update, render)]`.
- **HookHelper / use_state** — React-style state hook keyed off the current widget entity.
- **WoodpeckerStyle** — single-component style struct (~CSS-flavored, ~50 fields).
- **WidgetRender** — leaf-content enum (Text, Image, SVG, Quad, etc.) routed through `bevy_vello`.

## Sources

- crates.io: `woodpecker_ui` — https://crates.io/crates/woodpecker_ui (fetched 2026-05-22)
- crates.io: `kayak_ui` — https://crates.io/crates/kayak_ui (fetched 2026-05-22)
- GitHub repo: https://github.com/StarArawn/woodpecker_ui (fetched 2026-05-22)
- GitHub repo: https://github.com/StarArawn/kayak_ui (predecessor; fetched 2026-05-22)
- `woodpecker_ui` README — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/README.md
- `woodpecker_ui` Cargo.toml — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/Cargo.toml
- `woodpecker_ui` src/lib.rs — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/src/lib.rs
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy bevy_ui lessons — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
