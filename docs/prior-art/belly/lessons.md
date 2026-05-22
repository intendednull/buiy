**Date:** 2026-05-22
**Status:** active
**Subject:** belly — Validates / Avoid / Borrow synthesis. The consult-this-when-designing decision file.

# Lessons for Buiy

This is the consult-this-when-designing decision file for the belly corpus. Other files are evidence; this file is the synthesis.

belly is **paired with [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md)** — both folders feed the same Buiy open question:

> [foundation README § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions): **CSS-flavored stylesheet.** Never, or as a future layer above tokens? bevy_flair sets one precedent; the right answer depends on user demand.

Where bevy_flair documents *what a published, scoped-to-styles-only CSS layer on Bevy looks like*, belly documents *what a never-published, broader-scope HTML + CSS + bindings framework on Bevy looks like*. Read both files when deciding the stylesheet question.

## Top of file: the bigger picture

belly is two stories layered on top of each other:

1. **A design story** — `eml!` + `.ess` + `from!`/`to!` is a plausible-feeling answer to "what would the web platform's UI trifecta look like on Bevy?" The design works at the small-scale demo level.

2. **An operational story** — single maintainer, no crates.io publication, Bevy 0.13 (April 2024) pin, stalled since 2024-04, 436 stars but zero verifiable production users, no AccessKit, no tokens, no transitions.

The design story validates pattern shapes that Buiy might revisit. The operational story is the cautionary tale for what happens when a Bevy UI plugin tries to be a *framework* (authoring + styling + bindings + widgets, all in one crate) as a single-maintainer hobby project.

For Buiy's open question on a stylesheet layer, the operational story dominates: belly is the prior-art that says "even if the design is right, the bus-factor and crates.io decisions are load-bearing." Combined with bevy_flair's caveats, this strongly suggests: any future Buiy stylesheet layer is **Buiy-team-owned in-tree code**, not a vendor or fork of either precedent.

---

## Validates

These Buiy design choices are confirmed by belly's experience:

- **Cascade engine on bevy_ui-style decomposed components is implementable.** belly and bevy_flair independently converged on the same pattern: a runtime cascade pass walks the entity tree, matches selectors against entities, writes resolved values to small style components. Buiy's commitment to small, public-fielded, decomposed style components (foundation [architecture § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns)) is exactly the substrate that makes the cascade pattern feasible. **Two independent attempts arriving at the same shape is meaningful evidence.**

- **Hot-reload of stylesheets as a first-class feature works on Bevy's asset system.** belly's `.ess` hot-reload + bevy_flair's `.css` hot-reload both work. Buiy's commitment to hot-reloadable theme assets (foundation [architecture § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)) inherits this primitive directly.

- **HTML-like authoring syntax in a Rust procedural macro is feasible.** `eml!` proves the lexer + parser fits inside one crate. If a Buiy markup macro is ever wanted (it shouldn't be, given BSN), the implementation cost is bounded by belly's existence proof.

- **Attribute namespacing (`s:` / `c:` / `bind:` / `on:`) cleanly separates concerns.** Inline styles, classes, value bindings, and event handlers each get their own attribute prefix. The pattern is portable to any extensible markup system — including BSN's metadata fields. Buiy's authoring layer can borrow the namespacing convention without borrowing the markup macro.

- **Procedural-macro-based bindings (`from!` / `to!` / `run!`) work over Bevy's change detection.** belly's reactive bindings layer is a thin macro DSL over Bevy's existing `Changed<T>` / `Added<T>` machinery. It validates that Buiy's "observers + change detection are the v1 reactivity primitive" stance ([README non-goals](../../specs/2026-05-07-buiy-foundation/README.md#non-goals)) is enough to ship a usable declarative UI. A signal layer is genuinely deferrable.

- **A widget + cascade + binding plugin is end-to-end demonstrable** at the example-app scale. 27 examples shipped; the pattern works for "hello world" through "tab view with persistent state." Buiy's foundation can ship a similarly scoped widget+token+observer surface that's similarly demonstrable.

## Avoid

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **Git-only deps for production code.** belly's "no cargo release yet" became permanent unavailability. No `cargo add`, no docs.rs, no transitive depend-on path for crates.io crates. | [`distribution.md`](distribution.md) | Every Buiy crate is published to crates.io from `0.0.1`. The publication path is exercised before any project stall risk. |
| **Bus factor 1 for a foundational stylesheet layer.** Single maintainer, no co-maintainers with merge rights, life events stall the project mid-Bevy-migration. | [`distribution.md`](distribution.md), [`history.md`](history.md) | Buiy is in-tree, owned by the Buiy team, with the project's own contributors. No external single-maintainer dep on the critical path. |
| **HTML-as-DSL when the host engine has its own scene model (BSN).** `eml!` fragments the authoring story; community direction is ECS + BSN. | [`eml-macro.md`](eml-macro.md), Bevy discussions [#1522](https://github.com/bevyengine/bevy/discussions/1522) / [#9652](https://github.com/bevyengine/bevy/discussions/9652) | Buiy commits to ECS-spawn + BSN as the authoring path (foundation [architecture § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class)). No HTML-shaped macro. |
| **Cascading-CSS resolution overhead per frame, never benchmarked.** belly's cascade walks the entity tree every frame the cascade is dirty; no published performance characterization at scale. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) Critique 5 | Any Buiy stylesheet sub-spec ships benchmarks at 1000+-node fixtures as a CI gate (foundation [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md) CI gate #14). |
| **Hand-rolling the CSS parser.** belly's hand-rolled parser is a maintenance debt and is narrower than CSS. bevy_flair leasing Servo `cssparser` + `selectors` is the right answer. | [`ess-stylesheets.md`](ess-stylesheets.md) "Selectors" + "Properties" tables | If a Buiy stylesheet ships, parser substrate is Servo `cssparser` + `selectors`. See [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md) § Borrow. |
| **Cascade-vs-programmatic precedence undocumented.** belly clobbers programmatic style writes for any field the cascade controls. No spec doc, debugging requires reading source. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) Critique 7 | Buiy stylesheet sub-spec (if shipped) documents precedence among stylesheet / inline / programmatic / BSN / token in the spec, not as a footnote. |
| **`!important` undocumented.** belly's parser may or may not accept `!important`; behavior is undefined. bevy_flair parses it but silently ignores it — also bad. | [`ess-stylesheets.md`](ess-stylesheets.md), [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md) | Buiy stylesheet sub-spec either honors `!important` or errors-on-parse. No silent discard. |
| **AccessKit absence.** belly has zero a11y integration. A belly UI is not WCAG-conformant. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) Open problem 5 | AccessKit is a Buiy *foundation*-tier commitment ([architecture § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)). Every widget ships with role, label, states, relations, and a documented keyboard contract. |
| **No tokens — literal values in stylesheets.** belly UIs hardcode every color, every size. No `var()`, no semantic-token layer. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) Open problem 4 | Tokens are the Buiy foundation. A stylesheet, if shipped, resolves `var(--token)` against the typed Buiy token registry — not as opaque strings. See [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md) "string-keyed `var()`". |
| **Bevy minor-release migration tax compounds.** belly fell one version behind in 2024-07 and never caught up. Every subsequent Bevy minor compounded the migration cost. | [`history.md`](history.md), [`distribution.md`](distribution.md) | Foundation [README goal 5](../../specs/2026-05-07-buiy-foundation/README.md) commits to rolling-latest-stable. Verification harness includes a release gate testing current-Bevy compat on every cut. |
| **Pseudo-class set narrower than `:focus-visible` requirement.** belly has `:hover` / `:active` / `:focus`, no `:focus-visible`. WCAG 2.4.7 fails out of the box. | [`ess-stylesheets.md`](ess-stylesheets.md) "Selectors" | Buiy ships its own focus model with `:focus-visible` semantics ([architecture § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns)). Any Buiy stylesheet layer **must** expose `:focus-visible`. |
| **Inline-handler ergonomics via custom macro (`run!`).** belly invented `run!` because writing out `SystemParam` tuples for inline closures is verbose. Bevy has since improved on this; duplicating belly's macro is wrong. | [`data-binding.md`](data-binding.md) "run! macro" | Buiy's inline-handler ergonomics ride on current Bevy primitives (`IntoSystem`, observers). No bespoke macro layer. |

## Borrow

Concrete patterns worth studying if Buiy ever revisits the markup / cascade / bindings problem space:

1. **The `eml!` HTML-like syntax shape — as a *reference* for the attribute-namespacing pattern.** `s:padding="50px"` / `c:red` / `bind:value=…` / `on:press=…` cleanly separates four orthogonal concerns. Even if Buiy never ships an `eml!`-equivalent, the prefix-namespacing pattern is portable to BSN metadata fields, to a future stylesheet's inline-style syntax, and to any extensible attribute system. The pattern is the lesson, not the macro.

2. **The `.ess` selector + cascade resolution approach — confirmed feasible.** belly + bevy_flair both implement it; the pattern is sound. If a Buiy stylesheet ships, the cascade engine sits on the same primitive: walk entity tree, match selectors, write to decomposed style components. The implementation template is bevy_flair (Servo cssparser, 11-stage pipeline, marker-driven recalc) rather than belly (hand-rolled, monolithic).

3. **Data-binding observer patterns — `from!` / `to!` as ergonomic surface over change detection.** belly's bindings runtime is a useful proof that a thin macro DSL over Bevy's change detection is enough for the demo-scale reactive-UI case. Buiy's v1 commits to observers + change detection without a macro DSL ([README non-goals](../../specs/2026-05-07-buiy-foundation/README.md#non-goals)). If a future Buiy sub-spec adds reactive bindings as a downstream ergonomic layer, the `from!`/`to!` shape is a candidate API — minus the inline-in-markup coupling.

4. **The "HTML + CSS + observers" trifecta as a future Buiy stylesheet sub-spec consideration.** belly is the prior-art that says these three pieces are independently designable and can compose. If Buiy ships a stylesheet sub-spec, the same question arises: does it also bundle an authoring macro and a bindings DSL? The honest answer is **no** — keep each axis as a separately graduable sub-spec, because the operational lesson from belly is that bundling them into one crate forces a single maintainer to carry three responsibilities and stall on all three at once. Modularity is bus-factor insurance.

5. **`s:`-prefixed inline styles as a precedent for theme-override syntax.** Even if Buiy uses BSN authoring rather than markup, a future BSN-extension or BSN-companion syntax for per-instance overrides could borrow the `s:` prefix shape. The pattern works because it parses cleanly inside a procedural macro and namespaces away from regular attributes.

6. **Stylebox (nine-slice image rendering) as a missing-from-bevy_ui primitive.** `bevy_stylebox` is the one place belly does something bevy_ui can't natively. Nine-slice rendering is a real need (button backgrounds with non-rectangular borders, scaling panels with rounded corners). Buiy's render pipeline owns this kind of feature first-class ([visuals.md § border-image](../../specs/2026-05-07-buiy-foundation/visuals.md) at C-tier) — but the algorithm + asset format belly settled on is a useful starting point.

7. **A 27-example test suite as the demonstration baseline.** belly's examples cover the full surface (color picker, counter binds, signals, sliders, tabs, image rendering, scene loading). Buiy's verification harness should ship a comparable example set; "27 demonstrable example apps" is a reasonable scale signal that the framework is end-to-end usable.

## How to use this file

When the question "should Buiy ship a CSS-flavored stylesheet layer?" is on the table:

1. Read [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md) "Top of file" first — that has the structured arguments-for + arguments-against trade-off table.
2. Then read this file's **Top of file** for the operational caveat: even a well-designed stylesheet layer fails as a runtime dependency if it's single-maintainer + not on crates.io + Bevy-version stale. The design is necessary but not sufficient.
3. Read **Validates** to confirm what belly proves about the substrate Buiy is already committed to (decomposed components, hot-reload, change-detection-based bindings).
4. Read **Avoid** before writing any spec. Each row is a constraint a future `buiy-css-stylesheet-design` sub-spec must address.
5. Read **Borrow** as the implementation cookbook for the *shape* of authoring + cascade + bindings, paired with bevy_flair's lessons for the *engineering*.

When the question "should Buiy ship an HTML-like authoring macro?" is on the table:

1. The answer is **no**. The Buiy foundation commits to BSN-native authoring; an `eml!`-shaped macro fragments the story.
2. Borrow the attribute-namespacing convention (`s:` / `c:` / `bind:` / `on:`) if needed for BSN metadata.
3. Re-read belly's history — the macro is the most-developed part and didn't save the project.

## Sources

- belly repository — https://github.com/jkb0o/belly
- bevy_flair lessons (paired prior-art) — [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md)
- Buiy foundation README open question — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 5
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation visuals — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
- Buiy foundation accessibility — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation verification — [`../../specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)
- bevy_ui lessons (BSN draft status, megacomponent anti-pattern) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- Sibling evidence files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`eml-macro.md`](eml-macro.md), [`ess-stylesheets.md`](ess-stylesheets.md), [`data-binding.md`](data-binding.md), [`history.md`](history.md), [`distribution.md`](distribution.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`glossary.md`](glossary.md)
- Bevy CSS-skepticism discussions — https://github.com/bevyengine/bevy/discussions/1522, https://github.com/bevyengine/bevy/discussions/9652
- Bevy BSN draft PR — https://github.com/bevyengine/bevy/pull/20158
