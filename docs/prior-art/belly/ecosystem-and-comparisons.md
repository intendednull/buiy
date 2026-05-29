**Date:** 2026-05-22
**Status:** active
**Subject:** belly — adoption and direct comparison to other Bevy UI options (bevy_flair, sickle_ui, bevy_ui programmatic, Buiy)

# Ecosystem and comparisons

## Production usage

**None verifiable.** belly has 436 GitHub stars, ~14 months of active development (2023-03 → 2024-04), 4 tagged releases, ~30 example apps, and zero known production deployments.

Searches across crates.io reverse-deps (impossible — belly isn't on crates.io), GitHub code search for `belly = { git = "https://github.com/jkb0o/belly" }`, and Bevy community case studies turn up no shipping app, game, tool, or commercial product using belly as its UI layer. The 436 stars are best read as "interesting to look at" rather than "in production."

A few hobby-scale demos exist in personal repos that depend on belly via git ref, but none of these are commercial titles, and most are pinned to the v0.4 era (Bevy 0.12). The v0.5 (Bevy 0.13) era saw less community uptake before belly went dormant.

The most-cited Bevy commercial release of the belly era (Tiny Glade) wrote its own UI renderer — same pattern reported in [`../bevy-ui/ecosystem.md`](../bevy-ui/ecosystem.md). The "no flagship game" pattern that bevy_ui exhibits is even more pronounced for belly.

## Direct comparisons

### belly vs bevy_flair

The single most important comparison for Buiy, because both projects address the "CSS on Bevy" design problem.

| Dimension | belly | bevy_flair |
|---|---|---|
| **Scope** | Authoring (`eml!`) + styling (`ess`) + bindings (`from!`/`to!`) | Styling only (`.css` on bevy_ui components) |
| **crates.io** | Not published | Published, actively releasing |
| **Stars** | 436 | ~27 (smaller adoption) |
| **Bevy pin** | 0.13 (April 2024, ~5 majors stale) | Tracks current Bevy (0.18 era) |
| **Maintainer** | jkb0o (dormant) | eckz (actively committing) |
| **CSS parser** | Hand-rolled | Servo `cssparser` + `selectors` |
| **`var()` / `calc()`** | Absent | Present (F-tier coverage) |
| **`@media` queries** | Absent | `prefers-color-scheme` only |
| **Transitions / animations** | Absent (was "Coming soon") | Oklab interpolation since 0.3 |
| **Cascade docs** | Sparse | Thorough |
| **AccessKit integration** | Absent | Absent (out of scope for both) |
| **Pipeline decomposition** | Monolithic cascade pass | 11-stage `StyleSystems` |
| **Hot-reload** | Works | Works |
| **License** | MIT/Apache-2.0 dual | MIT/Apache-2.0 dual |

**Net read for Buiy:** if Buiy ever ships a stylesheet layer, the implementation template is bevy_flair (published, narrower scope, current Bevy, Servo `cssparser`, decomposed pipeline). The *what-to-include-from-the-broader-design* template is belly (`s:`-prefixed inline styles, `c:`-class notation, `bind:` / `on:` attribute namespacing). But neither is a runtime dependency.

### belly vs sickle_ui

sickle_ui took a different design path entirely — fluent ECS builders (`commands.ui_builder(root).column(|column| { … })`) rather than HTML markup. See [`../sickle-ui/`](../sickle-ui/).

| Dimension | belly | sickle_ui |
|---|---|---|
| **Authoring metaphor** | HTML-like markup macro | ECS fluent builder |
| **Stylesheet** | `.ess` cascade engine | Pseudo-theme `Style` struct (programmatic) |
| **Bindings** | `from!` / `to!` macros | Imperative (write through builder context) |
| **Status** | Dormant since 2024-04 | **Archived** (officially) |
| **Bevy pin** | 0.13 | Stalled around similar era |
| **crates.io** | Not published | Was published |
| **Stars** | 436 | ~600 |
| **Production usage** | None verifiable | None verifiable |

Both projects are now non-options as dependencies, and both demonstrate the bus-factor failure mode in Bevy UI ecosystem crates. The design-space contrast is interesting — sickle_ui sat closer to "ECS-native," belly sat closer to "web-platform-native" — and both attracted Bevy users for opposite reasons. Neither survived. The lesson is on the *operational* side, not the design side.

### belly vs bevy_ui programmatic

The default Bevy UI authoring path is direct ECS spawn:

```rust
commands.spawn((
    NodeBundle {
        style: Style {
            padding: UiRect::all(Val::Px(50.0)),
            ..default()
        },
        ..default()
    },
    children![(
        TextBundle::from_section("Hello, world!", TextStyle { … }),
    )],
));
```

belly's `eml!` is sugar over exactly this pattern. The same UI in `eml!`:

```rust
commands.add(eml! { <body s:padding="50px">"Hello, world!"</body> });
```

The line-count savings are real. But the bevy_ui programmatic path:

- Has no compile-time cost beyond normal Rust.
- Has full rust-analyzer support (jump-to-def works on every type).
- Inherits any Bevy migration without belly-mediated changes.
- Is the path the Bevy community is converging on (with BSN as the future scene-format complement).

belly's authoring-ergonomics win is offset by the operational risks (compile-time, IDE, migration tax). For Buiy: the ECS-native + BSN authoring path is the foundation commitment ([architecture § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class)).

### belly vs Buiy

Buiy is not yet implemented — the comparison is between belly (as of 2026-05-22) and Buiy's foundation spec.

| Dimension | belly | Buiy foundation |
|---|---|---|
| **Authoring** | `eml!` HTML markup | ECS spawn + BSN, no HTML markup |
| **Styling** | `.ess` cascade engine | Tokens (mandatory) + optional future stylesheet sub-spec |
| **Bindings** | `from!` / `to!` macros | Observers + change detection (v1); signals deferred |
| **Layout** | bevy_ui's Taffy (via bevy_ui) | Taffy directly, parallel to bevy_ui |
| **Text** | bevy_ui's text system (ab_glyph era) | cosmic-text directly |
| **A11y** | None | AccessKit-first, decomposed components |
| **Theme tokens** | None (literal values only) | First-class, hot-reloadable, OS-pref-driven |
| **Hot-reload** | `.ess` + `.eml` | Tokens + BSN + (optional future stylesheets) |
| **Bevy version** | 0.13 (frozen) | Rolling latest-stable |
| **Crates.io** | Never | Mandatory from 0.0.1 |
| **Production-grade WCAG** | No | Yes (foundation tier) |

The only dimensions where belly leads Buiy's plan are *authoring ergonomics for a designer-from-web-bg* (which is a deliberate Buiy non-goal — see [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md) "Top of file") and *bindings inline-in-markup* (which Buiy defers to a follow-up sub-spec).

## What belly's adoption tells us

The 436-star, zero-production-user pattern is informative:

1. **Bevy developers find belly's design *appealing*.** The star count is real interest, not noise.
2. **Bevy developers don't *use* belly in production.** No flagship app, no commercial title, no other crates.io crate depending on it (impossible regardless of intent).
3. **The barrier is operational, not aesthetic.** "Cool to look at" + "not on crates.io" + "stalled on Bevy 0.13" + "no AccessKit" = "not in production."

For Buiy: this confirms that solving the operational problems (crates.io, current Bevy, AccessKit, tokens, devtools) is the hard part. The aesthetic / authoring problem (markup vs builder vs BSN) is less load-bearing than internet discussion suggests. Ship the operational substrate; the authoring layer is the easy decorative tier.

## Implications for Buiy

1. **belly is design precedent, not engineering precedent.** Look at it for the shape of `eml!` + `ess` + bindings; don't look at it for "how to build a production UI plugin on Bevy."

2. **The bevy_flair + belly pair is the canonical case study** for "what happens when you ship a stylesheet-on-Bevy as a single-maintainer side project." Both single-maintainer; one published + actively maintained, the other not published + dormant. Both have failed to attract production adoption. The pattern matches Buiy's [foundation README § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) caution that "the right answer depends on user demand."

3. **The Bevy UI niche needs a foundation crate, not another framework.** belly tried to be a framework (markup + cascade + bindings + widgets in one crate); the result is a fragmented authoring story that competes with BSN. Buiy's foundation commitment ([README § 1.3 BSN-native](../../specs/2026-05-07-buiy-foundation/README.md)) avoids this by being a *library* whose components author cleanly in ECS + BSN, not its own authoring DSL.

## Sources

- belly repository — https://github.com/jkb0o/belly
- bevy_flair prior-art folder — [`../bevy-flair/`](../bevy-flair/)
- sickle_ui prior-art folder — [`../sickle-ui/`](../sickle-ui/)
- bevy_ui prior-art folder — [`../bevy-ui/`](../bevy-ui/)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Bevy BSN PR #20158 — https://github.com/bevyengine/bevy/pull/20158
- Tiny Glade UI write-up (custom UI renderer) — referenced via bevy_ui ecosystem doc — [`../bevy-ui/ecosystem.md`](../bevy-ui/ecosystem.md)
