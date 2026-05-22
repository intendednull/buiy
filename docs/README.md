# Buiy docs

Master index of Buiy's design specs, implementation plans, and reports. Grouped by feature area for discovery and onboarding.

For build/test/dev commands, see `../CLAUDE.md`. This file does not duplicate that content.

## Where to start (new agents and humans)

Reading order for newcomers:

1. [Buiy foundation design](specs/2026-05-07-buiy-foundation/README.md) — the target shape of the library: feature inventory, architectural foundation, sub-spec roadmap. Multi-file folder; start at the README, then read children in the order it lists.
2. [Docs organization design](specs/2026-05-07-docs-organization-design.md) — how this docs tree is structured.

## Document types

Four document types, each with one job. If a doc does not fit one of these, the type list is wrong, not the doc.

- **Spec** (`specs/`) — *what we are building toward.* Target shape of the code: types, traits, invariants, public API. Long-lived, canonical.
- **Plan** (`plans/`) — *how we get from current code to the target.* Migration steps, file-by-file changes, PR breakdown. Cites the spec it realizes. Goes stale once shipped.
- **Report** (`reports/`) — *findings from a one-shot investigation of our codebase.* Audits, post-mortems. Dated, immutable.
- **Prior-art** (`prior-art/<system>/`) — *deep dive on an external system we want to learn from.* Living documents; updated as the external system evolves. One folder per system; categories live in the catalog only.

## Status tags

Specs / plans / reports carry one of:

- `[draft]` — being written, target not yet stable.
- `[active]` — current target / in-flight migration.
- `[landed]` — realized in code; canonical reference.
- `[superseded]` — replaced; entry links to successor.

Prior-art docs carry `[active]` or `[archived]`.

## Catalog

Areas appear here as soon as there is a real doc to slot under them. Each area is a `### ` header with **Specs** and **Plans** subsections; entries are one line each:

```markdown
- [Title](specs/YYYY-MM-DD-name-design.md) — 5–15 word summary. `[draft]`
```

If a doc spans areas, file it under its primary area only. Reference any adjacent topics in the summary.

### Foundation

**Specs**

- [Buiy foundation design](specs/2026-05-07-buiy-foundation/README.md) — feature inventory, architectural foundation, sub-spec roadmap (multi-file). `[draft]`

**Plans**

- [Phase 0 foundations](plans/2026-05-07-buiy-phase-0-foundations.md) — workspace, BuiyPlugin, system sets, minimal render/layout/a11y/focus/picking/theme, verification harness skeleton, hello-world Button. `[landed]`
- [Phase 0 closeout](plans/2026-05-08-buiy-phase-0-closeout.md) — render-pipeline draws, AccessKit per-window adapter, `bevy_picking` backend; closes the three substantive deferrals from the Phase 0 self-review. `[landed]`

### Layout

**Specs**

- [Buiy layout design](specs/2026-05-08-buiy-layout-design/README.md) — Taffy bridge, hybrid `Style` builder + decomposed components, anchor positioning, container queries, writing modes, stacking + top layer, transforms + containment (multi-file). `[active]`

**Plans**

- [Buiy layout foundation](plans/2026-05-08-buiy-layout-foundation.md) — Phase 1: 8-step pipeline skeleton, decomposed components for the Phase-0 surface, hybrid `Style` builder, `Button` migration. `[landed]`
- [Buiy layout overflow and scrolling](plans/2026-05-08-buiy-layout-overflow-and-scrolling.md) — Phase 2: `Overflow` / `Scroll` / `ScrollOffset` / `ScrollSnapItem` components, Taffy overflow mapping, scroll-position-doesn't-invalidate invariant. `[landed]`
- [Buiy layout grid](plans/2026-05-09-buiy-layout-grid.md) — Phase 3: `GridParams` + `GridItem`, `TrackSize` / `GridLine` / `GridAreas` value types, `Display::Grid` → Taffy, Subgrid + Masonry warn-once stubs. `[landed]`
- [Buiy layout writing modes](plans/2026-05-10-buiy-layout-writing-modes.md) — Phase 4: `WritingMode` + `WritingModeResolved`, inheritance pass, `LogicalBoxModel` / `LogicalInset` builders, sideways-* warn-once stubs. `[landed]`
- [Buiy layout container queries](plans/2026-05-21-buiy-layout-container-queries.md) — Phase 5: `Container` + `ContainerQuery`, `Length::Cq{w,h,i,b,min,max}`, `cq_activate` / `cq_flip_check` / `cq_flip_rerun` pipeline systems, same-frame re-layout capped at 2× Taffy. `[landed]`

### Docs infrastructure

**Specs**

- [Docs organization design](specs/2026-05-07-docs-organization-design.md) — target structure of `docs/`, naming, headers, nesting. `[draft]`

## Prior art

External systems we learn from. Living documents — update on revision, archive when no longer worth tracking. Each system has its own subfolder under [`prior-art/`](prior-art/); categories below are organizational groupings in the index only — they do NOT exist as on-disk subfolders. Driven by the `researching-prior-art` skill (creation) and the `using-prior-art` skill (consumption).

### Bevy UI ecosystem

*(closest relatives — bevy_ui, bevy_feathers, bevy_ui_widgets, bevy_lunex, sickle_ui, woodpecker_ui, kayak_ui, belly, bevy_egui, bevy_flair, bevy_cosmic_edit, bevy_a11y, bevy_picking, iyes_ui_navigation, quill, polako)*

_(empty)_

### Non-Bevy Rust GUI

*(egui, iced, slint, dioxus, leptos, gpui, freya, makepad, xilem/masonry, druid, cushy, floem, vello-gui, relm4, gtk-rs, fltk-rs, tauri)*

_(empty)_

### Game engine UI systems

*(Unity UGUI, Unity UI Toolkit, Unreal Slate, Unreal UMG, Godot Control, NoesisGUI, RmlUi, Coherent Gameface, Scaleform, Flutter-in-Flame, Defold GUI)*

_(empty)_

### Substrate primitives

*(load-bearing dependencies — Taffy, Yoga, Stretch, cosmic-text, Parley, AccessKit, AT-SPI / UIA / NSAccessibility, WAI-ARIA APG, CSS Houdini)*

_(empty)_

## Reference designs

Archived design bundles (immutable inputs to specs, not specs themselves) live in [`reference-designs/`](reference-designs/) when they exist.

## Conventions

Cemented in [`specs/2026-05-07-docs-organization-design.md`](specs/2026-05-07-docs-organization-design.md). Mirrored on demand by the `organizing-buiy-docs` skill. Summary:

### Naming

| Type | Pattern | Example |
|---|---|---|
| Spec | `specs/YYYY-MM-DD-<kebab>-design.md` | `2026-05-07-docs-organization-design.md` |
| Multi-file spec | `specs/YYYY-MM-DD-<kebab>/README.md` + children | `2026-05-07-example-design/README.md` |
| Plan | `plans/YYYY-MM-DD-<kebab>.md` | `2026-05-07-example-plan.md` |
| Report | `reports/YYYY-MM-DD-<kebab>.md` | `2026-05-07-example-audit.md` |
| Prior-art | `prior-art/<system>/README.md` (no date prefix) | `prior-art/bevy-feathers/README.md` |
| Prior-art child | `prior-art/<system>/<facet>.md` | `prior-art/bevy-feathers/architecture.md` |

The date is when the doc was written, not the implementation target. The `-design.md` suffix on specs is what visually distinguishes specs from plans in `ls` output. Prior-art uses the system name (no date prefix) because the folders are living docs — track revision via git, not filename.

### Document headers

Every new spec, plan, and report opens with:

```
**Date:** YYYY-MM-DD
**Status:** draft | active | landed | superseded
**Spec:** specs/...      (plans only — REQUIRED, points at the spec being realized)
**Supersedes:** specs/... (if applicable)
```

Prior-art docs use a different header:

```
**Date:** YYYY-MM-DD       (last meaningful update — bump on revision)
**Status:** active | archived
**Subject:** <System name + one-line scope>
```

### Nested folders

Use a folder (`specs/YYYY-MM-DD-<topic>/README.md` + children) only when one logical document is too large for a single file *and* the children are tightly coupled. Children use kebab-case topic names (no date prefix — they inherit the parent's date). Maximum one level deep. Multiple independent docs sharing a topic stay flat.

### Adding a new spec, plan, or report

1. Pick the right type (spec = target, plan = migration, report = audit).
2. Name with `YYYY-MM-DD-<kebab>-design.md` (spec) or `YYYY-MM-DD-<kebab>.md` (plan/report).
3. Add a one-line entry to this README under the right area, with a 5–15 word summary and `[draft]` tag.
4. Plans must include `**Spec:** specs/...` in their header.
5. Multi-file specs nest under `YYYY-MM-DD-<topic>/` with a required `README.md`.
