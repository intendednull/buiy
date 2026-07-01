# Docs organization — target structure

**Date:** 2026-05-07
**Status:** landed

## Purpose

Define the target structure of `docs/` so agents and humans can find specs,
plans, and reports without grepping. Cement the conventions for what each
document type is, how it is named, where it lives, and how it appears in the
master index. Once realized, this spec is the canonical reference for any
future doc added to the project.

The migration from the current state to this target lives in a separate plan
under `docs/plans/`, per the spec/plan distinction defined below.

## Document types

Five document types, each with a single job. The split is the spine of the
whole structure — if a doc does not fit one of these, the type list is wrong,
not the doc.

### Spec — what we are building toward

Lives in `docs/specs/`. Describes the *target* shape of the code: types,
traits, invariants, public API, architectural boundaries. May briefly note
current state for contrast, but the bulk is the destination, not the journey.

Specs stay small because they do not repeat code that already exists. A spec
is canonical and long-lived; it is the reference plans and reviewers point at.
"Design rationale" and "why approach X over Y" belong here, because they
describe the destination's shape.

### Plan — how we get from current code to the target

Lives in `docs/plans/`. Describes the *migration*: current state, file-by-file
changes, ordering, risks, test strategy, PR-level breakdown. A plan cites the
spec it is realizing.

Plans go stale once shipped — they are an artifact of the journey, not the
destination. "How to refactor the existing 800-line file" belongs here, not
in the spec.

### Report — findings from a one-shot investigation

Lives in `docs/reports/`. Audits, post-mortems, performance investigations
of *our* codebase. Dated, immutable, does not define future direction.

### Prototype — the durable output of a throwaway prototype

Lives in `docs/prototypes/`. The committed **deliverables of prototype-first
work** (the `prototype-first-development` skill): a build-to-learn **journal**
(appended every wave) and a **retrospective** (keep / refine / redesign), plus
any **charter** or build-to-learn **design** the prototype produced.

The load-bearing rule: **the prototype CODE stays unmerged** (a throwaway
worktree, an audited reference for the final) — only these DOCS are carried
over, because a prototype's product is *learning*, not code. A retrospective
that lives only in a worktree is one `git worktree remove` from gone, so it is
promoted to `docs/prototypes/` (carried by the final's PR or a dedicated docs
PR) *before* the worktree is cleaned up. Prototype docs legitimately reference
their unmerged code by path — those are historical / branch references.

Prototype docs are `[active]` while the prototype runs, then `[archived]` once
a final supersedes them. They are indexed under their feature area's *prototype
lineage* line in the catalog (e.g. the MVU prototypes under **State
management**), not in a separate area.

### Prior-art — deep dive on an external system

Lives in `docs/prior-art/<system>/`. Captures an external project (competitor,
integration target, load-bearing dependency, design-space neighbor) as
durable reference material — so future spec authors consult the folder
instead of redoing the research. Each system gets its own subfolder; categories
(Bevy UI ecosystem, Rust GUI, game-engine UI, substrate) live as section
headers in the catalog only, not as on-disk subfolders.

Driven by two skills:

- `researching-prior-art` — the producer-side flow that creates a folder via
  a 7-stage workflow (landscape scan, structured overview, deep-dive
  fan-out, review, polish, second review, framing disclosure).
- `using-prior-art` — the consumer-side flow: surface relevant folders when
  brainstorming a spec, extend with online research, flag worth-promoting
  findings back to the corpus.

Prior-art docs are NOT specs. Do not encode Buiy design decisions in a
prior-art doc; capture lessons in the folder's `lessons.md` (validates / avoid
/ borrow) and promote real decisions into a spec.

### Implications of the split

- A spec can have multiple plans (large target, multiple PR-sized chunks).
- A spec without a plan is fine (target known, path deferred).
- A plan without a spec is suspicious — flag it during review.
- When the target evolves, the spec is updated and a new plan is written;
  old plans are not rewritten retroactively.
- A prior-art folder is a launchpad for specs, not a substitute. A spec that
  consults prior art cites the folder + section, names the runner-up
  paradigm if a choice was made, and flags remaining gaps.

### Specialized kinds — the `Kind:` field

The five types are the spine, but months of use produced recurring *variants*
of specs and plans that a reader needs to recognize on sight. Rather than mint
new types or new filename suffixes (which proliferate), name the variant in an
optional `Kind:` header field. The type (directory) is unchanged; the filename
stays `YYYY-MM-DD-<kebab>`; `Kind:` is the one place the variant is declared,
mirrored into the catalog entry's prose. Recognized kinds:

- **`campaign`** (a *plan*) — a plan-of-plans: the multi-PR umbrella that
  sequences a subsystem's phase-plans, sub-specs, decision notes, and reports.
  It carries `Date`/`Status` like any plan, but its `Spec:` may point at the
  spec **or** the report/audit it realizes, and it lists its member docs. A
  campaign is *coordination*, distinct from a bite-sized migration/TDD plan.
- **`decision`** (a *spec*, top-level or a spec-folder child) — a scoped
  ADR-style note recording one decision + its date + the rejected alternatives.
  This is the same thing the "dated design-note / ADR child" exception already
  blesses inside spec folders; `Kind: decision` names it wherever it lives.
- **`values`** / **`reference`** (a *spec*) — a mined source-of-truth data
  table an implementation is authored against (e.g. an exact-values token table),
  as opposed to a prose design.

`Kind:` is optional and additive: a plain spec/plan/report needs none, and it
does **not** duplicate a type. In particular the prototype-first byproducts
(journal, retrospective, charter, build-to-learn design) are the **Prototype**
type above — not kinds; and a re-decided post-prototype design is just a normal
spec that `Supersedes:` the throwaway one. `Kind:` exists so "what *is* this
doc?" is answerable from the header and catalog without opening the file or
decoding the filename.

## Top-level layout

```
docs/
├── README.md            master index — entry point for agents and humans
├── specs/               target state — what we are building toward
├── plans/               migration steps — how we get there
├── reports/             one-shot audits and investigations of our code
├── prototypes/          prototype-first journals + retrospectives (learning kept; code stays unmerged)
├── prior-art/           deep dives on external systems we learn from
│   └── README.md        the prior-art sub-index (its own catalog; see Non-goals)
├── reference-designs/   archived design bundles fed into specs (immutable)
└── assets/              images referenced by docs (screenshots, diagrams)
```

`docs/design/` does not exist in the target; design documents live in
`docs/specs/` (or as a multi-file spec folder under `docs/specs/`).

`docs/prototypes/` is the home of the **Prototype** type (above).
`reference-designs/` is the one remaining archival *location* rather than an
authored type: immutable design bundles that feed *into* specs (e.g. a source
design HTML a UI is reimplemented against), no date prefix, cataloged under
`## Reference designs`.

## Naming conventions

| Type | Pattern | Example |
|---|---|---|
| Spec | `docs/specs/YYYY-MM-DD-<kebab>-design.md` | `2026-05-07-docs-organization-design.md` |
| Multi-file spec | `docs/specs/YYYY-MM-DD-<kebab>/README.md` + children | `2026-04-19-ui-design/README.md` |
| Plan | `docs/plans/YYYY-MM-DD-<kebab>.md` (no `-design`) | `2026-04-21-e2e-test-architecture.md` |
| Report | `docs/reports/YYYY-MM-DD-<kebab>.md` | `2026-04-13-test-audit.md` |
| Prototype | `docs/prototypes/YYYY-MM-DD-<kebab>-{journal,RETROSPECTIVE}.md` (+ `-charter`/`-design`) | `2026-06-26-mvu-as-core-PROTO3-journal.md` |
| Prior-art | `docs/prior-art/<system>/README.md` (no date prefix) | `docs/prior-art/bevy-feathers/README.md` |
| Prior-art child | `docs/prior-art/<system>/<facet>.md` | `docs/prior-art/bevy-feathers/architecture.md` |

The date is **when the doc was written**, not the implementation target.

The `-design.md` suffix on specs is what visually distinguishes a spec from a
plan in `ls` output. Plans omit it. Prior-art uses the system name (lowercase,
kebab-case) with no date prefix — folders are living docs and revision
history lives in git, not in filenames.

*Exception to the dated-plan rule:* a rolling, intentionally-undated backlog
file (`docs/plans/follow-ups.md`) is permitted. It is a living cross-phase
tracker, not a one-shot dated migration, so it carries no date prefix — its
entries are flipped to `LANDED` in place as work ships rather than being
superseded by a new dated plan.

## Document headers

Every new spec, plan, and report opens with a small header. Existing files
predating this convention are not retrofitted (see *Non-goals*).

```
**Date:** YYYY-MM-DD
**Status:** draft | active | landed | superseded
**Kind:** campaign | decision | values   (optional — names a spec/plan variant; see "Specialized kinds")
**Spec:** docs/specs/...      (plans only — REQUIRED, points at the spec being realized)
**Supersedes:** docs/specs/... (if applicable)
```

Status semantics for specs/plans/reports:

- `draft` — being written, target not yet stable
- `active` — current target / in-flight migration
- `landed` — realized in code; canonical reference
- `superseded` — replaced; header links to successor

Prior-art docs use a different header:

```
**Date:** YYYY-MM-DD       (last meaningful update — bump on revision)
**Status:** active | archived
**Subject:** <System name + one-line scope>
```

Prior-art `active` = the system is still relevant to our framing. `archived`
= we've concluded the system isn't worth tracking further; the folder is
kept for historical context and the catalog entry moves under an
`### Archived` sub-section of `## Prior art`.

## Nested folder convention

A spec may be split across multiple files when one logical document is too
large for a single file *and* the children are tightly coupled — they lose
meaning without the parent.

```
docs/specs/2026-04-19-ui-design/
├── README.md         REQUIRED. Parent doc: purpose, scope, non-goals, child links.
├── foundation.md     children use kebab-case, NO date prefix
├── composer.md
└── ...
```

Rules:

- The parent `README.md` is required. It states the folder's purpose and links
  every child.
- Children do not carry their own date — they inherit the parent's date. If a
  child genuinely needs its own date, it is a separate spec, not a child.
  *Exception:* a dated **design-note / ADR** child that records its own
  decision date and status is permitted inside a spec folder. These read like a
  miniature dated report scoped to the parent design (e.g. the render-pipeline
  folder's `2026-06-*` children), and the date is load-bearing — it marks when
  that decision was made. They are the only children that may carry a date
  prefix.
- Children are kebab-case topic names, not phase numbers. Phases imply
  ordering; children are facets of one design.
- The same nesting rule applies to plans, but use is rare — plans usually
  stay flat. A nested plan folder represents one large migration broken into
  chapters, not multiple independent plans.
- Do not nest more than one level deep. If a child grows large enough to need
  its own children, promote it to a top-level spec.

Multiple independent documents that share a topic are flat siblings, not
children. Phased plans (`...phase-1a`, `...phase-1b`, ...) ship independently
and stay flat in `docs/plans/`.

## Master index — `docs/README.md`

The entry point. Four sections, in this order.

### 1. Orientation

Eight or so lines that tell a new agent or human:

- What this file is.
- Where to start if new (a curated reading list of 3–5 foundational specs).
- Pointer to `CLAUDE.md` for build/test/dev commands. The README does not
  duplicate that content.

### 2. Document type primer

Five short blocks recapping the spec / plan / report / prototype / prior-art distinction. Self-
contained so an agent landing here does not need to read this design doc or
`CLAUDE.md` to understand the catalog.

### 3. Catalog by feature area

Areas are introduced as the catalog grows. New areas are added when there is
a real doc to slot under them — not speculatively. Keep the ordering
foundations-first: lower-level primitives before higher-level features,
infrastructure before product, target specs before tooling.

Each area is a `## ` header containing a **Specs** subsection and a **Plans**
subsection. Entries are one line each:

```markdown
- [Title](specs/YYYY-MM-DD-name-design.md) — 5–15 word summary. `[status]`
```

If a doc spans areas, it appears in its primary area only. The summary tells
the reader if it is also relevant to adjacent topics.

A nested-folder spec appears as one entry pointing at its `README.md`. The
index does not enumerate children — that is the parent README's job.

Beyond the feature areas, the master index ends with a few standing sections:
`## Prior art` (a **category map** linking the split-out
[`prior-art/README.md`](../prior-art/README.md) sub-index — not the full
entries), `## Reference designs`, and the `## Reports` bucket. Prototype records
are cataloged under the feature area they belong to (e.g. a state-management
prototype under that area), tagged `[archived]`.

### 4. Conventions

The cemented rules — naming, document types, when to nest, how to add a new
spec/plan/report. Same content as this design, distilled into reference form.
Includes a short "Adding a new spec/plan" checklist:

1. Pick the right type (spec = target, plan = migration, report = audit,
   prototype = a prototype's carried-over journal + retrospective, prior-art =
   external-system deep-dive). Prototype docs and prior-art folders have their
   own add-flows (see the skill); this checklist is for spec/plan/report.
2. Name with `YYYY-MM-DD-<kebab>-design.md` (spec) or `YYYY-MM-DD-<kebab>.md`
   (plan/report).
3. Add an entry to `docs/README.md` under the right area with a 5–15 word
   summary and `[draft]` tag.
4. Plans must reference their spec in the header.
5. Multi-file specs nest under `YYYY-MM-DD-<topic>/` with a required `README.md`.

## Discovery surfaces

The conventions in this spec are surfaced to agents and humans through three
deliberately redundant channels. Each has a different access pattern; together
they ensure an agent always finds the rules without grepping.

### Primary — `docs/README.md`

The master index. Self-documenting: the conventions live in section 4
alongside the catalog so anyone browsing the docs sees them in passing. This
is the canonical copy. If the other two surfaces drift, the README is right.

### Skill mirror — `organizing-buiy-docs`

A project-local skill at `.claude/skills/organizing-buiy-docs/SKILL.md`
that mirrors the conventions and the "Adding a new spec/plan" checklist. An
agent loads this skill on demand when:

- Adding a new spec, plan, or report.
- Modifying the structure (adding a feature area, splitting a spec into a
  nested folder, deprecating/superseding a doc).
- Reorganizing the catalog.

The skill exists for two reasons:

1. **Discoverability via metadata.** Skills surface in the agent's
   available-skills list with their `description`; `docs/README.md` does not.
   An agent with no prior context can find the skill from its trigger
   description alone.
2. **Token efficiency.** The skill loads only the rules — not the catalog —
   so an agent making structural changes does not need to read 60+ catalog
   entries to find the conventions.

The skill is not the source of truth. It points back at this spec and at
`docs/README.md` for the canonical text. When the canonical text changes,
the skill is updated in the same commit.

### Pointer — `CLAUDE.md`

`CLAUDE.md` retains all build/test/dev/architecture content. The "Specs &
Plans" line in *Code Conventions* is replaced with a short pointer to both
of the surfaces above:

> **Docs entry point:** `docs/README.md` is the master index of specs and
> plans, grouped by feature area. Read it before adding any new spec or plan,
> or before searching for an existing one. The `organizing-buiy-docs` skill
> mirrors the conventions for on-demand loading.

Existing CLAUDE.md sections that duplicate doc-discovery information (e.g.
the per-task "see `docs/specs/...`" pointers in *Architecture Notes*) are
left in place — they are useful inline context, not redundant with the index.

## Reconciling when work lands

The "adding a new doc" checklist covers a doc's *birth*. Rot accrues because
nothing covered its *landing*: docs are written pre-merge on long-lived
branches and never reconciled once the work ships, so campaigns leave plans
uncataloged and statuses frozen at `draft`/`active`. Close the loop at
**branch-finish / PR time** (the `finishing-a-development-branch` flow), in the
same change that lands the work:

1. **Catalog every new doc the work introduced** — every plan, report,
   prototype record, or reference bundle. This is **required**: it is the one
   hard invariant ("missing entries are not tolerable"), not advisory.
2. **Reconcile status** of every spec/plan the work realized — flip `draft`/
   `active` → `landed` (or `superseded`, with the successor link) in **both**
   the in-doc header and the catalog `[tag]`. This is *expected hygiene*, not a
   gate — stale status tags remain tolerable (see Non-goals), but a landed
   campaign is the moment they are cheapest to fix.
3. **Fix references the work invalidated** — a renamed/removed doc, a link to a
   file that stayed on a throwaway worktree.

This is a *habit at the merge boundary*, deliberately not a CI gate: the tag
stays a discovery aid, not an enforced project-management signal.

- **Renaming existing files.** Files that predate this convention (e.g.
  specs without the `-design.md` suffix) stay as-is to preserve git history
  and existing links. The index resolves any ambiguity by labeling them
  explicitly. The convention applies to *new* docs only.
- **A status-tracking system.** The `[status]` tag is a discovery aid, not a
  project-management tool. Stale tags are tolerable; missing entries are not.
  The *"Reconciling when work lands"* habit tightens this at the merge boundary
  (reconcile statuses when a campaign ships) but keeps it a habit, not a CI
  gate — the tag never becomes an enforced signal.
- **Per-area sub-READMEs.** Single-file index *until volume justifies a split*.
  Prior art crossed that line first: at ~48 rich entries it had grown to roughly
  half the master index, so its catalog now lives in its own
  [`prior-art/README.md`](../prior-art/README.md) and the master keeps only a
  category map. This is the sanctioned split, not a reversal — apply the same
  test before splitting any other area.

## Open questions

- The set of feature areas in `docs/README.md` will be defined as the buiy
  catalog grows. The area list is load-bearing for navigation, so introduce
  areas only when there is a real doc to slot under them rather than
  speculatively.
