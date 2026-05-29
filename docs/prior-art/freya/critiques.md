**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — critiques and open problems

# Critiques and open problems

The critiques below are not partisan — Freya is one of the more impressive solo-maintainer Rust GUI projects shipping today. The list is calibrated for **Buiy's decision-making**: which Freya weaknesses are *structural* (informing what Buiy must avoid) vs *operational* (informing what Buiy can do better with similar headcount).

## Structural critiques

### 1. Skia C++ dependency

The single biggest architectural commitment Freya cannot reverse. Implications:

- **Build complexity.** `freya-skia-safe` requires CMake + Clang + Python during build. First builds take 10–30 minutes depending on hardware.
- **Binary size.** A Freya hello-world binary is ~20–40MB after release-stripping. Most of that is Skia.
- **Mobile / WASM tax.** Skia *can* build for mobile and WASM in principle, but the integration cost is high enough that Freya has not done it.
- **Audit surface.** Skia is ~500K lines of C++ from Google. Supply-chain auditability is meaningfully harder than a pure-Rust stack.
- **Not Bevy-compatible.** Freya could not become a Bevy plugin even hypothetically — Skia's surface model conflicts with Bevy's render graph.

**Buiy mitigation:** wgpu via Bevy's render graph (foundation [§ 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly)). Pure-Rust at the boundary; the win is borrowable to mobile/WASM, slimmer binaries, easier audit.

### 2. Pre-1.0 with rc churn

`0.4.0-rc.19` after several months of rc-tagging signals an ongoing API in flux. For production users this means:

- **Pin to exact rc** because rc-N to rc-N+1 can be breaking.
- **No long-term support** — once 0.4.0 stable ships (whenever), 0.3.x patches stop.
- **Migrations are real and undocumented in long-form** (per [`distribution.md`](distribution.md) — no per-release blog posts).

**Buiy mitigation:** Foundation [README § 4](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap) sub-specs and `docs/specs/` corpus → migration notes ship as part of each sub-spec. Per-release docs discipline.

### 3. Dioxus coupling

Freya is married to Dioxus 0.6.x (workspace pin: `dioxus ^0.6.3`). Every Dioxus major release is a Freya migration event. Practically:

- Dioxus's API changes flow downstream regardless of Freya's roadmap.
- Freya cannot upgrade Dioxus on its own schedule — must wait for stability + feature completeness.
- Two pre-1.0 ecosystems compose multiplicatively, not additively.

**Buiy mitigation:** Foundation explicitly does not depend on any external reactivity layer (foundation [§ 2.7](../../specs/2026-05-07-buiy-foundation/architecture.md#27-reactivity)). Bevy observers + change detection in v1; if signals are added later, build them directly on Bevy ECS rather than depending on Dioxus.

### 4. Single-maintainer bus factor

Marc Espín is the sole strategic owner. The 7 GitHub Sponsors are appreciated patrons, not co-maintainers. Risks:

- If Marc stops, the codebase becomes dormant.
- Design decisions live in Discord, not in `docs/` — succession requires Discord history scraping.
- No documented "what happens to the crate if Marc moves on" plan (no published deprecation policy or repo-transfer arrangement).

**Buiy mitigation:** Open governance question in [foundation README § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions). Buiy must structure for multiple maintainers from day one.

### 5. Small adoption / no flagship app

After 3.5 years and 33K+ downloads, no flagship application is publicly cited as built on Freya. Implications:

- The framework is *unproven* at production-app scale.
- Edge cases that only emerge in real apps (multi-window IME composition, locale-specific text editing, complex form validation, large list virtualization performance) have not been stress-tested.
- The 0.4 rewrite is happening without flagship-app feedback pressure.

**Buiy mitigation:** Buiy's verification spec ([foundation § verification](../../specs/2026-05-07-buiy-foundation/verification.md)) commits to a synthetic-app harness that exercises real production patterns from CI day one. Doesn't substitute for real users but de-risks edge cases substantially.

### 6. Torin layout vs Taffy ecosystem

Torin is a one-consumer crate. By picking Torin, Freya excluded itself from:

- Shared layout-engine improvements (subgrid, container queries, anchor positioning) landing in Taffy.
- Cross-framework layout test corpora (Blitz, bevy_ui, Servo, and many smaller embedders all share Taffy bug reports + fixes).
- Layout-engine maintenance amortized across the ecosystem.

Marc bears Torin's entire maintenance burden alone.

**Buiy mitigation:** Foundation [§ 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly) commits to Taffy. Shares the maintenance + features with the rest of the Rust ecosystem.

## Operational critiques

### 7. Stringly-typed styling props

Every Freya attribute (`width: "100%"`, `background: "rgb(...)"`, `direction: "vertical"`) is a string parsed at runtime. Errors surface as runtime warnings, not compile errors. IDE autocomplete is limited to "is this a known attribute name" — it cannot validate value shape.

This is a deliberate web-familiarity tradeoff but it costs:

- **Type safety.** Typos and value-shape bugs slip past compilation.
- **Refactor safety.** Renaming a prop requires string-find across the codebase.
- **IDE support.** No rust-analyzer "go to definition" on a CSS string.

**Buiy mitigation:** BSN typed components (foundation [§ 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class)) — all styling is Bevy components with typed fields, `Reflect`-able for asset hydration.

### 8. No CSS-stylesheet-from-file

Freya's styling is inline on each element via `rsx!`. There is no `.css`-equivalent file format. Themes are Rust struct values, not asset files. This means:

- No designer-friendly theme-edit workflow.
- No hot-reloadable theme file (themes hot-reload only insofar as Subsecond hot-reloads the Rust source they live in).
- No CSS-class-equivalent for sharing styling across components.

**Buiy mitigation:** Foundation [§ 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system) — token themes as hot-reloadable assets. (CSS-flavored stylesheet remains [open question § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) for a future layer.)

### 9. AccessKit integration depth unverified

Per [`accessibility.md`](accessibility.md): the AccessKit dependency is present, but per-widget APG conformance, ACCNAME 1.2 compliance, live regions, and forced-colors-mode response are unverified in the public record. Freya could have any depth-of-implementation from "AccessKit adapter wired but most widgets ship without correct roles" to "fully APG-conformant."

**Buiy mitigation:** Foundation [verification spec](../../specs/2026-05-07-buiy-foundation/verification.md) — per-widget APG conformance tests in CI. Buiy's claim is the *verified-in-CI* claim, not the "we have AccessKit" claim.

### 10. Hot-reload depends on Subsecond stability

Freya uses Dioxus's Subsecond hot-reload (per the 0.6+ Dioxus integration). Subsecond is the most-aggressive Rust hot-reload story shipping anywhere and still hits edge cases — see [`../dioxus/open-problems.md`](../dioxus/open-problems.md) § "Hot-reload reliability."

Freya does not document its own "what hot-reloads / what requires restart" matrix. Behavior is inherited from Subsecond.

**Buiy mitigation:** Foundation [open question § Hot-reload of components](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) and [`buiy-bsn-integration-design`](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap) sub-spec will define an explicit hot-reload coverage matrix.

### 11. Theme system lacks OS-pref binding

No automatic `prefers-color-scheme` / `prefers-contrast` / `forced-colors` / `prefers-reduced-motion` propagation into themes. The user code must wire OS preferences manually if they want them.

**Buiy mitigation:** Foundation [§ 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system) — `UserPreferences` resource auto-binds OS prefs to theme variants.

### 12. No documented performance budgets

Freya has no public per-fixture frame-time budget, no CI regression gate on UI render performance, and no published "what we test on what hardware" matrix. Performance discussion lives in Discord + GitHub issues.

**Buiy mitigation:** Foundation [verification.md CI gate](../../specs/2026-05-07-buiy-foundation/verification.md) — performance budget per fixture, documented + CI-enforced.

## Open problems (things Freya itself acknowledges or that observers can identify)

- **Mobile / WASM.** Skia builds on both, but the work has not been done. Freya is desktop-only for foreseeable future.
- **Advanced text features.** Vertical writing modes, hyphenation, complex justification — same gaps as cosmic-text + Parley share, but Freya's reliance on Skia textlayout puts it in a different position re: extensibility (Skia is more opaque to embedders).
- **1.0 timeline.** No public commitment.
- **Co-maintainer succession.** No documented plan.
- **Real-app validation.** No flagship.
- **Theme tokenization story.** Hardcoded Rust-struct themes don't compose well with cross-app design systems.

## What Buiy carries forward

The structural critiques (1–6) are all reasons Buiy made the substrate / governance / layout / reactivity choices it did. Cross-link to [`lessons.md`](lessons.md) for the Validates / Avoid / Borrow synthesis.

The operational critiques (7–12) are reasons Buiy invests in BSN-typed components, token-asset theming, verified APG conformance, explicit hot-reload matrix, OS-pref binding, and CI performance budgets. Each is an above-baseline commitment relative to where Freya sits today.

## Sources

- Freya repo + issue tracker — https://github.com/marc2332/freya
- Freya releases (rc cadence) — https://github.com/marc2332/freya/releases
- Workspace dependency declarations — https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml
- Cross-references: [`../dioxus/open-problems.md`](../dioxus/open-problems.md), [`../cosmic-text/critiques.md`](../cosmic-text/critiques.md), [`history.md`](history.md), [`accessibility.md`](accessibility.md), [`distribution.md`](distribution.md), [`lessons.md`](lessons.md).
- Buiy foundation — [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md), [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md).
