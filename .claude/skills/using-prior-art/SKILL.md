---
name: using-prior-art
description: Use when starting spec/plan/impl/review work that touches a researched area — surface relevant prior-art folders from `docs/prior-art/`, treat them as launchpads for online research, and flag worth-promoting findings back to the corpus.
---

# Using Prior Art

Buiy maintains a researched prior-art corpus under `docs/prior-art/`. This skill is the consumer-side flow. The producer-side flow is `researching-prior-art` (which is what built the corpus).

## When to invoke

- **Brainstorming a spec** — primary case. Before drafting the design, surface relevant prior-art and extend with online research.
- **Writing a plan** — verify migration steps don't contradict consulted prior-art.
- **Implementing** — when a design references a borrowed pattern, re-read the source folder.
- **Reviewing** — when a PR touches an area with researched prior-art, check that consulted folders are cited and remain accurate.

Skip when the work is local or mechanical — rename, format, single-bug fix unrelated to runtime semantics.

## Stance: resource, not gate

This skill is not a mandatory checklist. It is an enabling resource. Agents decide depth based on context. The corpus is load-bearing for spec authoring; ignoring it when relevant wastes that work, but mechanical consultation when not relevant wastes time.

## Three-step flow

### 1. Discover

Read `docs/README.md` Prior-art section. Most entries close with a "Consult before any spec on X, Y, Z" clause — that is the primary topic surface. Some entries describe applicability inline instead; read the whole entry when the closing clause is absent.

For matching folders, read in this order:
1. `<folder>/README.md` — decision summary, key facts at a glance, why-this-folder-exists, honest assessment.
2. `<folder>/lessons.md` — validates / avoid / borrow.
3. `<folder>/open-problems.md` — gaps (especially relevant when judging whether the corpus covers your current work).

If unsure whether a folder is relevant, read its README — the cost is one Read tool call.

### 2. Extend

Prior-art is a launchpad, not a destination. After consulting, do targeted online research:

- **WebFetch** on URLs cited in the prior-art folder if facts may have changed since the folder's `Date:` frontmatter.
- **WebSearch** for developments after that date.

Look specifically for:
- New releases and version bumps
- Deprecations and end-of-life announcements
- Security advisories
- Governance changes — acquisitions, license flips, maintainer departures, project renames

The corpus is a snapshot. Current state always needs verification.

### 3. Promote

If online research surfaces material worth a *new* prior-art folder, or a major correction to an existing one, flag it inline in the conversation:

```
**Prior-art suggestion** — <one-line name>
- Why it matters: <load-bearing for X / invalidates Y / fills gap Z>
- Source(s) found so far: <URL1>, <URL2>
- Suggested action: spawn `researching-prior-art` <now | after current work lands | low-priority>
- Not blocking: current work proceeds
```

The skill does not spawn `researching-prior-art` itself. The user decides.

**Trigger conditions for promotion** (any one suffices):
- New project / library / protocol that is a load-bearing dependency Buiy will lean on
- Existing folder has a major commercial change (acquisition, license flip, project rename) that invalidates conclusions
- Existing folder has a structural gap — current work needs coverage that does not exist
- New academic paper or W3C spec that reframes a paradigm already covered

## Output expectation

When invoked during spec brainstorming, the resulting spec should:

- **Cite consulted folders by path AND section.** `prior-art/bevy-feathers/lessons.md §3` is a citation. `see prior-art/bevy-feathers` is not — uncheckable.
- **Name the runner-up paradigm** if a choice was made. Example: "retained-mode-with-megacomponents — see `prior-art/bevy-ui/`; rejected because BSN-incompatibility surfaced as bevy issue #17644."
- **Flag remaining gaps** the prior-art does not cover. These become candidate trigger conditions for future `researching-prior-art` spawns.

## Anti-patterns

- **Reading prior-art and not extending with online research.** Corpus is dated; current state is not.
- **Citing without specifying file/section.** Uncheckable citation = no citation.
- **Spawning `researching-prior-art` mid-spec without flagging first.** Derails the work. Flag the gap, finish the current work, let the user triage.
- **Mandatory consult on local/mechanical work.** Skill is a resource; using it on a one-line rename wastes time.

## Sibling skill

`researching-prior-art` is the producer-side flow — it builds new folders. When this skill identifies a gap and the user decides to spawn research, that is what they spawn.
