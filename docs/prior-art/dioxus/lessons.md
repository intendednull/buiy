**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — Validates / Avoid / Borrow decisions for Buiy

# Lessons for Buiy

This is the consult-this-when-designing decision file. The other files in this corpus are evidence; this file is the synthesis. **Dioxus is not a substrate Buiy could be built on** — it's a separate runtime with its own scheduler, VDOM, and renderer abstractions. The lessons here are about (a) the shape of a Rust signal layer if Buiy ever adds one, (b) authoring-DSL ergonomics for BSN, (c) what multi-target ambition costs, and (d) Dioxus's Taffy integration as a sibling reference.

## Top of file: two findings that reframe Buiy decisions

### 1. The Rust signal-framework convergence is real.

Dioxus (signals 0.5+, Stores 0.7+) and Leptos (`RwSignal` 0.6+, `Store` 0.7+) independently arrived at the same primitive: **generational-arena-stored, Copy-by-default, subscribe-in-render-path, batched-effect-commit**. Sycamore's older `Rc<RefCell<T>>` shape is the negative example. The Solid.js model (Ryan Carniato) is the common conceptual ancestor.

**Restated rule for Buiy:** If Buiy adds signals (foundation [open question § 5](../../specs/2026-05-07-buiy-foundation/README.md)), don't invent a new model. The settled pattern is generational-arena + Copy + Solid-derived subscription rules. Read Carniato's writeups before reading the Dioxus or Leptos source.

### 2. Multi-target ambition costs more than the marketing implies.

Dioxus's *"one codebase, every platform"* slogan translates, after five years and ~$500K + sponsorships of investment, into: production on web/webview targets; **pre-alpha on the flagship native (Blitz) target by the authors' own admission** ([`open-problems.md`](open-problems.md) § "Blitz is pre-alpha"). Each renderer is a separate maintenance line with its own bug list.

**Restated rule for Buiy:** [Foundation non-goal § 1.3 — "non-Bevy frontends"](../../specs/2026-05-07-buiy-foundation/README.md) is validated by every Dioxus target-maturity story. The single-substrate-Bevy commitment trades scope for quality. Resist any future temptation to add a non-Bevy renderer — the cost is years, the gain is rarely worth it.

## Validates

These Buiy design choices are confirmed by Dioxus's experience:

- **Single substrate (Bevy ECS + render graph)** ([foundation README § 1.3](../../specs/2026-05-07-buiy-foundation/README.md)). Dioxus's multi-target tax is the existence proof that owning multiple renderers is a years-long quality-per-target problem. Buiy inherits Bevy's WASM/desktop/mobile target story for free; this is the load-bearing simplification.
- **AccessKit-first on every target** ([foundation architecture § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)). Dioxus gets DOM/AT-for-free on webview targets but **no AT integration on Blitz/Native as of 0.7.9**. For Bevy targets (all native), there is no "AT-for-free" tier — Buiy's AccessKit commitment is the correct policy. See [`open-problems.md`](open-problems.md) § "Accessibility gap on native targets."
- **No signals/computed/effects in v1** ([foundation README § 1.3 non-goals](../../specs/2026-05-07-buiy-foundation/README.md)). A real signal layer is a several-engineer-year investment (Dioxus 0.5 was a 100K-line, 1,400-commit, multi-quarter rewrite). Doing it post-foundation, with the foundation's invariants known, is cheaper than doing it inline. The current "Bevy observers + change detection" v1 stance is correct; the eventual signal layer can borrow the settled Rust-framework convergence.
- **BSN-friendly decomposed components by construction** ([foundation architecture § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)). The `rsx!` macro's component-and-typed-props authoring is the closest existing Rust UI DSL to BSN, and its iteration history (rust-analyzer partial-parse in 0.6, hot-patchable token formatting in 0.7) reveals what authoring-DSL users care about. The lesson: rust-analyzer integration and DSL hot-reload are day-one quality, not v2 polish. See [`rsx-macro.md`](rsx-macro.md).
- **Taffy as the layout substrate** ([foundation architecture § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)). Blitz (DioxusLabs's native-renderer flagship) uses Taffy directly through the `LayoutPartialTree` trait surface against a non-ECS DOM-node arena. Multi-embedder Taffy is genuine; Buiy's Taffy bet is doubly validated. See [`integration-with-taffy.md`](integration-with-taffy.md).
- **Tracking-latest-Bevy / no multi-version-compat-promise** ([foundation README § 1.5](../../specs/2026-05-07-buiy-foundation/README.md)). Dioxus's release cadence (annual minors, monthly patches) and its 0.4→0.5 100K-line rewrite illustrate that pre-1.0 Rust UI frameworks churn substantially between minor versions. Buiy aligning to Bevy's release tempo and not promising cross-version compatibility is realistic.
- **Docs discipline (`docs/specs/`, `docs/plans/`, `docs/prior-art/`, `organizing-buiy-docs` skill)**. Dioxus has no public RFC repo or design-spec corpus; its design rationale lives across GitHub Discussions, Discord, and per-release blog posts. This makes deliberate "learn-from-Dioxus" research expensive (this corpus took several hours to assemble). Buiy's commitment to a `docs/`-tree corpus is a deliberate improvement over the Rust UI ecosystem norm.

## Avoid

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **Multi-target single-codebase ambition.** Dioxus's "one codebase, every platform" promise after five years is "production on web, pre-alpha on the flagship native target." Per-target maintenance scales linearly with target count. | [`open-problems.md`](open-problems.md) § "Multi-target fragmentation"; [`targets.md`](targets.md) maturity matrix. | Buiy targets Bevy only. Foundation [non-goal § 1.3](../../specs/2026-05-07-buiy-foundation/README.md) is the policy. Inherit Bevy's target story; don't ship a "non-Bevy frontend." |
| **Renderer-per-target maintenance.** Dioxus ships dioxus-web, dioxus-desktop (webview), dioxus-native (WGPU/Blitz), dioxus-mobile, dioxus-ssr — five live renderers + the SSR string renderer. Each carries its own bugs. | [`targets.md`](targets.md). | Buiy ships one renderer integrated with Bevy's render graph. Foundation [architecture § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md). |
| **A flagship "production" feature in pre-alpha by author admission.** Dioxus Native / Blitz was the headline of the year-defining 0.7 release; Blitz's own README says "we do not recommend building production apps with it yet." This is reputational debt — users adopt under the marketing premise and hit pre-alpha bugs. | [`open-problems.md`](open-problems.md) § "Blitz is pre-alpha"; [`history.md`](history.md) § "0.7." | Buiy's tier-F/C/E/O language ([foundation README § "Tier legend"](../../specs/2026-05-07-buiy-foundation/README.md)) makes per-feature maturity explicit. CI gates + manual release gates ([foundation verification spec](../../specs/2026-05-07-buiy-foundation/verification.md)) prevent the "flagship feature is pre-alpha" outcome by making the gate explicit. |
| **Webview-AT-for-free shortcut.** Webview targets inherit AT from the platform; native targets don't. Dioxus's web target a11y is good, Blitz's a11y is non-existent. The shortcut creates uneven cross-target quality. | [`open-problems.md`](open-problems.md) § "Accessibility gap on native targets." | Bevy targets are all native; Buiy's AccessKit-first policy applies uniformly. Foundation [architecture § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md). |
| **Signals layered onto a non-ECS scheduler.** Dioxus's signal scheduler is single-threaded and topologically ordered; Bevy's is parallel and access-pattern ordered. A Buiy signal layer cannot just port the Dioxus shape — it needs a scheduler bridge. | [`signals-and-state.md`](signals-and-state.md) § "Schedule-alignment is the open question." | If signals are added to Buiy (open question § 5), the sub-spec must address scheduler integration explicitly. Don't try to make Dioxus-shape signals work inside a Bevy world without designing the bridge. |
| **Informal-design-process cost.** Dioxus's deliberation lives in Discord + GitHub Discussions; learning *why* a decision was made requires archaeology. The Blitz architecture took multiple quarters of scattered discussion before culminating in a release post. | [`governance.md`](governance.md) § "RFC / design process." | Buiy commits to `docs/specs/` + `docs/plans/` + `docs/reports/` + `docs/prior-art/`. No design state in chat. The `docs/README.md` index is the only entry point. |
| **`Rc<RefCell<T>>`-shape signals.** Sycamore's older Rust signal model used `Rc/RefCell`; it stalled in adoption because the shape is ergonomically inferior to the Copy-arena approach. | [`signals-and-state.md`](signals-and-state.md) § "Comparison." | If signals are added to Buiy, use the generational-arena + Copy shape (Dioxus's `Signal<T>` and Leptos's `RwSignal<T>`). Do not invent a new shape. |
| **DSL ergonomics deferred to "later."** Dioxus iterated `rsx!` over 5 years — partial-parse autocomplete in 0.6 was a major release feature, not day-one polish. Users complain about IDE integration before they complain about runtime performance. | [`rsx-macro.md`](rsx-macro.md) § "Tooling integration." | BSN's foundation work should include rust-analyzer integration and hot-reload-of-the-DSL from day one, not as a "v2" feature. Foundation `buiy-bsn-integration-design` sub-spec ([open question § Hot-reload of components](../../specs/2026-05-07-buiy-foundation/README.md)). |
| **Pseudo-HTML element naming in the DSL.** Dioxus uses lowercase `div`/`button`/`span` because it ships an HTML element schema. BSN's vocabulary is Bevy components (`Node`, `Text`, `Button`). The temptation to make BSN look HTML-shaped is real but wrong — it isn't HTML. | [`rsx-macro.md`](rsx-macro.md) § "Implications for Buiy." | Resist HTML cosmetics in BSN. Component types are Rust identifiers; cases match Rust idiom. |
| **Hot-reload completeness ambition.** Dioxus Subsecond is the most aggressive Rust UI hot-reload story and still hits reliability edge cases (lifetimes-heavy generics, certain closures). Don't promise "everything hot-reloads" — promise an explicit coverage matrix. | [`open-problems.md`](open-problems.md) § "Hot-reload reliability." | Buiy's BSN hot-reload story should design an explicit "what reloads / what requires restart" matrix. Foundation [open question § Hot-reload of components](../../specs/2026-05-07-buiy-foundation/README.md). |
| **Single-vendor brief facts.** The brief for this corpus said "FutureWei + Khosla Ventures Series A funding" — neither claim is verifiable in public funding records (Crunchbase, YC, Tracxn). The actual story is YC S23 seed (~$500K) + Pioneer Fund + sponsors. | [`governance.md`](governance.md) § "Funding"; [`README.md`](README.md) § "Brief corrections." | For load-bearing-dep prior-art folders, verify every funding/governance claim against crates.io publisher data, Crunchbase, YC pages. The `researching-prior-art` skill's verification-before-stage-7 rule applies to organization-level facts, not just version pins. |
| **Treating Blitz as Servo.** Blitz v0.2+ is **not a Servo fork**. It is an independent engine that reuses Stylo (the standalone CSS component) and Parley (Linebender's text crate). The v0.1 Servo-fork-shape branch is archived. | [`targets.md`](targets.md) § "Desktop (WGPU/Blitz)"; [`README.md`](README.md) § "Brief corrections." | When citing dependency lineage for a Buiy spec, verify against the upstream README on the date of the citation. Pre-amble inheritance from older sources drifts. |

## Borrow

Concrete primitives worth studying (and possibly adapting into Buiy's own layers):

1. **`Signal<T>: Copy` via generational-box arena.** The defining ergonomic primitive of modern Rust signals. Implemented in DioxusLabs's `generational-box` crate, reused by `dioxus-signals`. The same arena pattern is independently shipped by Leptos. If Buiy ever adds signals, this is the right shape — `Copy` handles, arena-stored values, drop-on-scope-unmount. See [`signals-and-state.md`](signals-and-state.md) § "Signal API."

2. **The `Store` derive macro pattern (Dioxus 0.7).** Per-field signal accessors on a struct + per-key accessors on collections. The application-scale unlock without which signals don't compose past trivial state. Leptos has the same. If Buiy's signal layer ships, Store-shape must ship simultaneously, not as a v2. See [`signals-and-state.md`](signals-and-state.md) § "Stores."

3. **`rsx!` statement-shape control flow (`if`/`for`/`match` inside the DSL).** Cleaner than JSX's expression-only `{cond && jsx}` / `{arr.map(...)}` shapes. BSN's draft tracks this. Worth a closer study of how Dioxus's proc-macro handles control flow tokens vs Bevy's BSN approach. See [`rsx-macro.md`](rsx-macro.md) § "Surface syntax."

4. **Format-string interpolation `"{name}"` inside attribute and text positions.** The single highest-leverage DX feature in `rsx!`. Compresses string concat with bindings to one line and is hot-reloadable via Subsecond. BSN should support this. See [`rsx-macro.md`](rsx-macro.md) § "Implications for Buiy."

5. **Blitz's `LayoutPartialTree` integration pattern against a non-Taffy-owned arena.** The reference implementation of the Taffy-trait-surface integration path. If Buiy ever migrates off `TaffyTree`-wrapping (the current bridge) and onto the trait surface, Blitz's `blitz-dom` is the cleanest pre-existing reference. See [`integration-with-taffy.md`](integration-with-taffy.md) and [`../taffy/architecture.md`](../taffy/architecture.md).

6. **Subsecond's "framework cleanup at sync points" contract.** Subsecond doesn't auto-detect when to hot-patch — it relies on the framework defining safe sync points (between scheduler ticks, after event commit) where pending hot-patches apply. This is the load-bearing pragmatic concession that makes Subsecond work at all. Buiy's BSN hot-reload story will face the same issue: there must be explicit safe points (system-set boundaries, end-of-frame, between substages) where hot-patches apply. See [`history.md`](history.md) § "0.7 — Subsecond."

7. **Per-release blog-post discipline.** Dioxus ships a detailed long-form release post for every minor version. The posts retrospectively explain design choices, list breaking changes, give migration paths, and link to relevant discussions. This is the **only** public deliberation trace for major Dioxus design decisions and it's load-bearing for external researchers. Buiy should adopt the same release-note depth — release-notes-as-design-artifact is a cheap improvement over typical Rust crate release-note brevity.

8. **Cargo feature carve-out per target.** Dioxus's umbrella crate exposes `web`/`desktop`/`mobile`/`native`/`fullstack`/`router`/`document` features so app authors only compile what they use. Buiy's foundation crate-split decision ([foundation open question § Crate-split refinement](../../specs/2026-05-07-buiy-foundation/README.md)) can borrow the same shape — even if Buiy is single-substrate, feature-gating per subsystem (render / a11y / layout / focus / theme) gives the same compile-time-control without renderer-per-target maintenance. See [`governance.md`](governance.md) § "Cargo features."

9. **Solid.js as the conceptual ancestor for reactivity.** When designing any Buiy reactive layer, read Ryan Carniato's signal-rendering writeups before reading Dioxus or Leptos source. The conceptual model is upstream of both Rust frameworks; understanding it from Solid is cheaper than reverse-engineering from a Rust implementation. See [`signals-and-state.md`](signals-and-state.md) § "Comparison."

10. **The `dx` CLI's single-binary tooling story.** One CLI binary handles serve, build, bundle, hot-reload, format, test for all targets. Buiy's tooling story (foundation [verification](../../specs/2026-05-07-buiy-foundation/verification.md) + [devtools](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) sub-specs) can adopt the same "single binary, multiple subcommands" shape. The Bevy ecosystem currently has scattered tooling (cargo-bevy doesn't yet exist as a flagship); Buiy can promote a single integrated binary for its own workflows.

## How to use this file

When designing a Buiy feature:

1. **Find the row in `Avoid`** that names a pitfall close to your design. Read the linked file for the original incident.
2. **Find the entry in `Borrow`** that names a primitive close to what you're designing. Read the linked file for the shape, then adapt for Buiy's ECS-on-Bevy substrate.
3. **Promote any decision into a Buiy spec** under `docs/specs/` — this file is for capturing what we learn from Dioxus, not for encoding Buiy's own decisions.

## Sources

- Sibling evidence files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`rsx-macro.md`](rsx-macro.md), [`signals-and-state.md`](signals-and-state.md), [`targets.md`](targets.md), [`integration-with-taffy.md`](integration-with-taffy.md), [`history.md`](history.md), [`governance.md`](governance.md), [`ecosystem.md`](ecosystem.md), [`open-problems.md`](open-problems.md), [`glossary.md`](glossary.md).
- Cross-references: [`../taffy/lessons.md`](../taffy/lessons.md), [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md), [`../bevy-egui/README.md`](../bevy-egui/README.md).
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md).
- Dioxus 0.5 / 0.6 / 0.7 release notes: https://dioxuslabs.com/blog/
- Dioxus repo: https://github.com/DioxusLabs/dioxus/
- Blitz repo: https://github.com/DioxusLabs/blitz
- Ryan Carniato's signals introduction: https://dev.to/this-is-learning/a-hands-on-introduction-to-fine-grained-reactivity-3ndf
