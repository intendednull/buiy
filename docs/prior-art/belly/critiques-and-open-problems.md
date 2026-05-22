**Date:** 2026-05-22
**Status:** active
**Subject:** belly — substantive critiques and structural open problems, framed for Buiy's stylesheet open question

# Critiques and open problems

This file combines third-party critiques (what observers have said about belly's design) and open problems (what belly structurally does not solve). The honest read: belly's design is interesting, but the project's *operational* problems dominate any architectural lessons.

## Critiques

### 1. The no-crates.io situation limits adoption — terminally

The README is explicit: `"As far as the project has no cargo release yet, the only way to discover all the features it has is to clone the repo and check out the examples."`

This is belly's biggest structural problem, and the discussion is in [`distribution.md`](distribution.md). Summary:

- No `cargo add belly` path.
- Any consumer crate that wants to publish to crates.io cannot have belly as a hard dep (Cargo's policy).
- No docs.rs hosting → no easy discovery via standard Rust workflow.
- No download metrics → no public adoption signal.
- 436 GitHub stars and 0 verifiable production users.

The decision to not publish has hardened into the project's stall. There is no public statement from the maintainer planning a `0.5.0` crates.io publication.

### 2. Single maintainer + dormancy = bus factor 1, realized

`jkb0o` is the sole maintainer. No commits to `main` since 2024-04-20. Issue [#83](https://github.com/jkb0o/belly/issues/83) ("Bevy 0.14 migration") has been open since 2024-07-19, unanswered. The contributor who shipped v0.5.0 (`Threadzless`) does not have merge rights.

This is the canonical Bevy-ecosystem bus-factor failure mode: a single developer's hobby project attracts users, the user base outgrows the maintainer's bandwidth, life events intervene, the project freezes mid-Bevy-migration. belly is the most prominent example of this pattern in the Bevy UI space.

For Buiy: this is **the** reason belly cannot be a runtime dependency. The dependency would inherit the bus factor.

### 3. HTML-as-DSL fights the ECS-native mental model

belly's `eml!` macro asks Bevy developers to switch authoring metaphors mid-flight: ECS-spawn syntax outside the macro, HTML syntax inside it. This is a cognitive overhead the Bevy community has consistently identified ([discussion #1522](https://github.com/bevyengine/bevy/discussions/1522), [#9652](https://github.com/bevyengine/bevy/discussions/9652)) as a non-starter.

The BSN proposal (PR #20158) is the community's chosen direction — a Bevy-reflection-driven scene format, not an HTML clone. belly's `eml!` is an interesting *implementation proof* but stands against community direction. Even with the macro fully working, the community fit isn't there. (The 436-star count reflects "this is cool to look at," not "I'd use this in production.")

### 4. Macro hygiene + compile-times

A procedural macro that parses a custom grammar inside Rust source has two well-known costs:

- **Compile-time impact.** Heavy macros expand to large `TokenStream`s. belly's `eml!` is non-trivial; rebuild times on bigger UIs are correspondingly heavy. No published benchmark exists, but the pattern is observable in the example apps.
- **IDE / rust-analyzer support.** Identifiers inside a macro body are not necessarily resolved by rust-analyzer. `eml!` references like `<button>` map to widget functions via macro lookup, not direct calls — IDE jump-to-def is degraded.

bevy_flair's `.css` files are external to the Rust source and avoid both costs entirely. The split is structurally cleaner.

### 5. Cascade resolution cost — never benchmarked

belly's cascade pass is a per-frame walk over the entity tree, matching selectors against entities, applying winning rules. The performance characteristics are:

- **No published benchmark.** Neither belly's docs nor any community report measures cascade cost.
- **No documented optimization story.** Unlike bevy_flair's `StyleMarkers` dirty-tracking ([`../bevy-flair/architecture.md`](../bevy-flair/architecture.md)), belly's cascade does not document an incremental-recompute strategy.
- **Test fixtures stop at small scale.** The example apps are 10–50 entity trees. The 1000-node fixture that the Buiy verification harness ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) requires is untested.

belly's cascade may be fine at small scale — but the absence of measurement is the critique, not the performance itself.

### 6. WCAG / APG coverage absent

belly does not integrate with AccessKit. There is no documented:

- ARIA role mapping for widgets.
- Accessible name computation.
- Focus tree (belly relies on bevy_ui's pre-`bevy_input_focus` model).
- `:focus-visible` semantics.
- Live regions.
- Keyboard interaction contracts for the widget set.

A belly-built UI is **not** WCAG 2.2-conformant out of the box. The Buiy foundation [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md) makes accessibility a foundation-tier requirement; belly's complete absence here is a hard mismatch.

### 7. Cascade-vs-programmatic precedence undocumented

belly's cascade pass writes to bevy_ui style components every frame the cascade is dirty. This **clobbers** programmatic writes (`commands.entity(e).insert(BackgroundColor(…))`) for any field the cascade controls. This is the same pitfall flagged in [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md) ("Clobber semantics undocumented"), and belly inherits it without documentation.

Debugging the symptom — "I set this color in code and the cascade overrides it" — requires reading source. No spec doc, no troubleshooting page.

## Open problems

### Open problem 1: Bevy version migration tax

How does belly catch up to current Bevy (0.18)? Each minor release between 0.13 and 0.18 includes breaking changes to bevy_ui's component model. Migration would touch every belly crate. With one dormant maintainer, the answer is: it doesn't, until someone else forks.

### Open problem 2: APG / WCAG coverage

The widget set has zero documented APG conformance. The `<button>` widget does not declare its keyboard contract, the `<slider>` does not declare its `aria-valuenow` / `aria-valuemin` / `aria-valuemax` mapping, the `<textinput>` has no IME composition path. A WCAG-conformant version of belly would require rewriting every widget around AccessKit — a project larger than the current belly.

### Open problem 3: BSN compatibility

belly's `.eml` asset format is HTML-shaped text. BSN's `.bsn` asset format is Rust-reflection-shaped data. The two are not interoperable, and there is no glue layer. If BSN lands upstream (still draft per [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)), belly users would need to choose: stay in `.eml` and lose BSN ecosystem compatibility, or migrate authoring to BSN and lose belly's stylesheet + bindings system.

### Open problem 4: Theme tokens

belly has no token system. Styles are written as concrete values (`background-color: #ffffff;` style literals). There are no CSS custom properties (`--name`), no `var()`, no semantic-token abstraction. A "designed UI" in belly has every theme decision hard-coded into the stylesheet.

This is the cleanest single demonstration that belly is *not* a full theming substrate — it's a styling substrate. Buiy's [foundation architecture § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system) puts tokens at the foundation; belly puts them nowhere. bevy_flair has `var()` + `calc()` since its 0.3 release; belly never added them.

### Open problem 5: AccessKit integration

There is no AccessKit code in belly. None. The repo doesn't take a dependency on `accesskit` or on Bevy's `bevy_a11y`. A screen reader connected to a belly app sees the bevy_ui tree (whatever AccessKit nodes bevy_ui itself emits) — belly contributes nothing to the a11y tree.

For Buiy this matters because Buiy commits to AccessKit-first ([architecture § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)). belly is not a precedent on this dimension; it's a negative example.

### Open problem 6: Text editing / IME / BiDi / complex script

belly's `<textinput>` widget is a basic single-line text field. There is no:

- IME composition surface.
- BiDi caret behavior.
- Complex-script shaping (cosmic-text was added to bevy_ui in 0.15, after belly's last release).
- Multi-line / rich-text editing.
- Selection model.

For Buiy's "app-and-game both" goal, this is one of belly's largest gaps. Buiy's text editing requirements ([text.md](../../specs/2026-05-07-buiy-foundation/text.md)) cannot be met by belly's substrate.

### Open problem 7: Animation / transitions / motion

The README's "Coming soon" list at v0.5.0 included transitions. Transitions never arrived. There is no `transition-property`, no `@keyframes` analogue, no spring system. belly UIs are static (style-wise) after the cascade resolves.

bevy_flair shipped Oklab transitions in 0.3 ([`../bevy-flair/lessons.md`](../bevy-flair/lessons.md) § Borrow). belly never closed the gap.

### Open problem 8: Devtools

No inspector, no contrast checker, no focus visualizer, no theme editor, no devtool overlays. The CSS-in-the-browser ecosystem assumes devtools exist; belly inherits zero of that ecosystem and didn't ship its own.

## Synthesis

The critiques pattern: belly's *design* (markup + cascade + bindings) is sound and validates the pattern shape. belly's *execution* (no crates.io, single maintainer, stalled on Bevy 0.13, no AccessKit, no tokens, no transitions, no APG conformance) makes it unusable as a runtime dependency and incomplete as a feature reference.

The open-problems pattern: belly stopped where the easy 80% of declarative-UI ergonomics ended. The hard 20% — a11y, IME, transitions, tokens, BSN compat, devtools — is where Buiy starts. None of those problems are belly's fault as a *design*, but the resulting feature set means belly cannot be a foundation, only a reference.

## Implications for Buiy

1. **Treat belly purely as a design reference.** Adopting any of its code, depending on its crates (impossible — they aren't published), or trusting its API stability are all non-starters.

2. **A future Buiy stylesheet sub-spec must close every gap in this file.** Specifically: crates.io publication (mandatory from 0.0.1); AccessKit integration; tokens-as-substrate (not strings); BSN-compat authoring path; APG conformance per widget; OS-pref support (`prefers-contrast`, `prefers-reduced-motion`, `forced-colors`); documented cascade precedence; documented `!important` handling; performance benchmarks at 1000+ nodes.

3. **Don't ship a markup macro alongside BSN.** belly's `eml!` is feasible but fragments the authoring story. Buiy commits to BSN-native authoring. A markup macro is at best a downstream ergonomic layer, at worst a community-fit failure.

4. **The bus-factor critique applies to bevy_flair too.** bevy_flair is also single-maintainer (eckz). The difference is bevy_flair is actively releasing; belly is dormant. Both are bad runtime-dependency choices for Buiy. Both are good design references.

## Sources

- belly v0.5.0 README ("no cargo release yet") — https://github.com/jkb0o/belly/blob/v0.5.0/README.md
- belly issue #83 — https://github.com/jkb0o/belly/issues/83
- belly v0.5.0 `docs/style-properties.md` — https://github.com/jkb0o/belly/blob/v0.5.0/docs/style-properties.md
- Bevy discussion #1522 (CSS skepticism) — https://github.com/bevyengine/bevy/discussions/1522
- Bevy discussion #9652 (CSS skepticism) — https://github.com/bevyengine/bevy/discussions/9652
- Bevy BSN draft PR #20158 — https://github.com/bevyengine/bevy/pull/20158
- bevy_flair lessons (clobber + `!important` warnings) — [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md)
- bevy_flair architecture (StyleMarkers + cascade pipeline) — [`../bevy-flair/architecture.md`](../bevy-flair/architecture.md)
- Buiy foundation accessibility — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation architecture (tokens) — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.5
- Buiy foundation verification — [`../../specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)
- bevy_ui lessons (BSN draft status) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
