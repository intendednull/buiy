---
name: organizing-buiy-docs
description: Conventions for adding, naming, and organizing specs, plans, reports, and prior-art folders in docs/. Use when creating a new doc, modifying the docs structure (adding a feature area, splitting into nested folder, adding a prior-art folder, superseding a doc), or reorganizing the catalog. Mirrors docs/README.md and docs/specs/2026-05-07-docs-organization-design.md.
---

# Organizing Buiy docs

Project-local skill mirroring the cemented conventions for the `docs/` tree.

**Source of truth:** [`docs/README.md`](../../../docs/README.md) (master index, self-documenting) and [`docs/specs/2026-05-07-docs-organization-design.md`](../../../docs/specs/2026-05-07-docs-organization-design.md) (the design spec). If this skill ever drifts from those files, the README and spec are right. Update this skill in the same commit when conventions change.

## When to use this skill

Invoke before:

- Adding a new spec, plan, report, or prior-art folder.
- Modifying the docs structure (adding a feature area to the catalog, splitting a spec into a nested folder, adding a prior-art category, superseding or deprecating a doc, archiving a prior-art folder).
- Reorganizing the catalog.

If you are *reading* docs (not changing them), go to `docs/README.md` directly — that is the catalog.

## Document types

Four types, each with one job. If a doc does not fit one of these, the type list is wrong, not the doc.

- **Spec** (`docs/specs/`) — *what we are building toward.* Target shape of the code: types, traits, invariants, public API, architectural boundaries. May briefly note current state for contrast, but the bulk is the destination, not the journey. Long-lived, canonical.
- **Plan** (`docs/plans/`) — *how we get from current code to the target.* Migration steps, file-by-file changes, ordering, risks, test strategy, PR-level breakdown. Cites the spec it realizes. Goes stale once shipped.
- **Report** (`docs/reports/`) — *findings from a one-shot investigation of our codebase.* Audits, post-mortems, performance investigations. Dated, immutable.
- **Prior-art** (`docs/prior-art/<system>/README.md`) — *deep dive on an external system we want to learn from.* Living documents; updated as the external system evolves and as our framing of what's relevant shifts. Each system gets its own subfolder so it can grow supporting children (sub-deep-dives, captured diagrams, archived snapshots) without the top-level prior-art directory turning into noise. Category grouping (Bevy UI ecosystem, Rust GUI, game-engine UI, substrate, etc.) lives in the catalog index only — the on-disk layout stays flat.

Implications:

- A spec can have multiple plans (large target, multiple PR-sized chunks).
- A spec without a plan is fine (target known, path deferred).
- A plan without a spec is suspicious — flag it during review.
- Prior-art docs are NOT specs. Do not encode Buiy design decisions in a prior-art doc. Capture lessons in the doc's `lessons.md` (validates / avoid / borrow) and promote real decisions into a spec.

## Naming

| Type | Pattern | Example |
|---|---|---|
| Spec | `docs/specs/YYYY-MM-DD-<kebab>-design.md` | `2026-05-07-docs-organization-design.md` |
| Multi-file spec | `docs/specs/YYYY-MM-DD-<kebab>/README.md` + children | `2026-05-07-example-design/README.md` |
| Plan | `docs/plans/YYYY-MM-DD-<kebab>.md` (no `-design`) | `2026-05-07-example-plan.md` |
| Report | `docs/reports/YYYY-MM-DD-<kebab>.md` | `2026-05-07-example-audit.md` |
| Prior-art | `docs/prior-art/<system>/README.md` (no date prefix) | `docs/prior-art/bevy-feathers/README.md` |
| Prior-art child | `docs/prior-art/<system>/<facet>.md` | `docs/prior-art/bevy-feathers/architecture.md` |

The date is **when the doc was written**, not the implementation target. The `-design.md` suffix on specs is what visually distinguishes specs from plans in `ls` output. Plans omit it. Prior-art files use the system name (no date prefix) because they're living docs — track revision history via git, not filename.

**Exception to the dated-plan rule:** a rolling, intentionally-undated backlog file (`docs/plans/follow-ups.md`) is permitted. It is a living cross-phase tracker, not a one-shot dated migration, so it carries no date prefix — its entries are flipped to `LANDED` in place as work ships rather than being superseded by a new dated plan.

**Existing files predating these rules are not renamed.** The convention applies to new docs only; the master index labels older entries explicitly so the missing suffix does not affect discovery.

## Document headers

Every new spec, plan, and report opens with:

```
**Date:** YYYY-MM-DD
**Status:** draft | active | landed | superseded
**Spec:** docs/specs/...      (plans only — REQUIRED, points at the spec being realized)
**Supersedes:** docs/specs/... (if applicable)
```

Status semantics for specs/plans/reports:

- `draft` — being written, target not yet stable.
- `active` — current target / in-flight migration.
- `landed` — realized in code; canonical reference.
- `superseded` — replaced; header links to successor.

Prior-art docs use a different header:

```
**Date:** YYYY-MM-DD       (last meaningful update — bump on revision)
**Status:** active | archived
**Subject:** <System name + one-line scope>
```

Prior-art `active` = the system is still relevant to our framing. `archived` = we've concluded the system isn't worth tracking further; doc kept for historical context.

The status tag is a discovery aid, not a project-management tool. Stale tags are tolerable; missing entries in the master index are not.

## Nested folders

Use a folder (`docs/specs/YYYY-MM-DD-<topic>/`) only when one logical document is too large for a single file *and* its children are tightly coupled — they lose meaning without the parent.

Rules:

- The parent `README.md` is **required**. It states the folder's purpose and links every child.
- Children use kebab-case topic names with **no date prefix** — they inherit the parent's date. *Exception:* a dated **design-note / ADR** child that records its own decision date and status is permitted inside a spec folder (it reads like a miniature dated report scoped to the parent design — e.g. the render-pipeline folder's `2026-06-*` children — where the date marks when that decision was made). These are the only children that may carry a date prefix.
- Children are facets of one design, not phase numbers. Phases imply ordering; children do not.
- Maximum one level deep. If a child needs its own children, promote it to a top-level spec.

Multiple independent documents that share a topic are flat siblings, not children. Phased plans (`...phase-1a`, `...phase-1b`, ...) ship independently and stay flat in `docs/plans/`.

## Adding a new spec, plan, or report

1. **Pick the right type.** Spec = target. Plan = migration. Report = audit.
2. **Name it.** `YYYY-MM-DD-<kebab>-design.md` (spec) or `YYYY-MM-DD-<kebab>.md` (plan/report). Date is today.
3. **Write the header.** All four fields where applicable. `Spec:` is required for plans.
4. **Add a catalog entry.** One line under the right area in `docs/README.md`:
    ```markdown
    - [Title](specs/YYYY-MM-DD-name-design.md) — 5–15 word summary. `[draft]`
    ```
5. **Pick the area.** Use an existing area in the catalog. If none fit, add a new area in the same commit (see below). If a doc spans areas, file it under its primary area.
6. **Commit the doc and the README entry together.** The catalog must not lag the file.

## Adding a new prior-art doc

1. **Confirm it deserves its own folder.** A prior-art doc is for systems we expect to consult repeatedly. One-shot research notes belong in `docs/reports/` instead. The `researching-prior-art` skill drives the full creation flow.
2. **Create the folder.** `docs/prior-art/<system>/`. Lowercase, kebab-case system name. No date in the path.
3. **Write the main doc** at `docs/prior-art/<system>/README.md`.
4. **Add the header** (see ## Document headers, prior-art variant).
5. **Cover the required sections** (see ## Prior-art structure below).
6. **Add a catalog entry** under the right category sub-section of `## Prior art` in `docs/README.md`. Categories are organizational groupings in the index only — they do NOT exist as on-disk subfolders.
7. **Commit the doc and the README entry together.**

Children (focused sub-deep-dives, captured diagrams, archived screenshots, raw research notes worth preserving) go in the same folder as siblings to `README.md` — e.g. `docs/prior-art/<system>/architecture.md`. The README links them. No date prefix on children.

## Prior-art structure

Each prior-art doc covers, at minimum:

- **What it is** — one-paragraph summary + key facts table.
- **Architecture** — components, data model, control flow. Diagrams welcome.
- **Strengths** — what the system gets right that we should learn from.
- **Weaknesses / open problems** — what's broken, what's unsolved, what cost time.
- **Lessons for Buiy** — split into `Validates` (our existing choices), `Avoid` (specific pitfalls + our mitigation), `Borrow` (primitives worth studying). Lives in `lessons.md` for multi-file folders.
- **Sources** — URLs cited inline + collected at the end of each file.

Optional but encouraged: glossary of system-specific terms, recommended reading order, history/governance/critiques files for larger folders. The `researching-prior-art` skill enumerates the full file partition.

## Modifying the structure

- **Adding a feature area:** adds an `### ` header to the catalog plus the area name to step 5 above. Update the spec at `docs/specs/2026-05-07-docs-organization-design.md` and this skill in the same commit.
- **Adding a prior-art category:** add a new sub-section under `## Prior art` in `docs/README.md`. Categories are catalog-only; do NOT create category subfolders on disk.
- **Promoting a spec to a nested folder:** rename `<topic>-design.md` → `<topic>/README.md`. Children are added later as kebab-case files (no date). Update the catalog entry to point at the folder's `README.md`.
- **Superseding a doc:** add `**Supersedes:**` to the new doc's header and `[superseded]` plus a link to the successor in the old doc's catalog entry. Do NOT delete the old doc.
- **Archiving a prior-art doc:** flip its `Status:` from `active` to `archived` and add a one-line "Why archived" note under the header. Move catalog entry under an `### Archived` sub-section of `## Prior art`. The folder stays where it is.
