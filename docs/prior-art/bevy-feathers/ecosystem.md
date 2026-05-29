**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — consumers, ecosystem position, relationship to third-party widget kits, relationship to Buiy

# Ecosystem

`bevy_feathers` is a young crate (first release 2025-09-30, ~8 months old as of 2026-05-22), small distribution (191,700 lifetime downloads), and gated behind an explicitly experimental feature flag. The ecosystem is correspondingly thin: one stated primary consumer (the in-development Bevy editor), no known shipping commercial product, and a small cluster of demo / tutorial projects.

## Primary consumer: the Bevy editor (in development)

Per the Bevy 0.17 release notes (verbatim): "Feathers is meant to be Bevy's 'developer tools' widget set, and **it will be used to build the upcoming Bevy Editor**." The editor itself has not shipped a public release; it is being built incrementally inside the Bevy monorepo. The cadence of feathers widget additions (text input → 0.19, number input → 0.19, menu → 0.19, color plane → 0.18) tracks editor needs, not general game-UI demand. See [`history.md`](history.md) § "Bevy 0.19-rc.1" for the controls-directory diff.

A related early consumer is the **World Inspector** — the in-engine entity/component browser tool. The PR #19730 description names both: "intended for use by the Bevy Editor, World Inspector, and other tools."

## Shipping products using feathers

**None verified as of 2026-05-22.** Searching crates.io reverse-dependencies, GitHub code-search, and the curated `awesome-bevy` listings produces:

- Tutorial repos and small examples (e.g., a handful of `getting-started-with-feathers` repos).
- The two in-tree examples: `examples/ui/widgets/feathers_counter.rs` and `feathers_gallery.rs` on `main`; a single `examples/ui/feathers.rs` on `v0.18.1`.
- No commercial-game shipping with feathers as its UI layer.
- No flagship indie title verified — Tiny Glade (the most-cited Bevy commercial release) wrote its own UI renderer and does not use feathers or `bevy_ui` widgets.

The "shipping product" gap is not a critique of feathers per se — the crate is 8 months old, behind an experimental flag, and explicitly not aimed at games. But it does mean **no battle-testing at game-UX scale exists**. The Buiy verification harness ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) commits to its own fixtures rather than relying on a flagship-consumer proof; the absence of a flagship feathers consumer is one reason that commitment matters.

## Position in the Bevy widget-kit landscape

Feathers is the **first-party official option** in a landscape that previously held only third-party kits. Each adjacent kit lives in its own prior-art folder (see [`comparisons.md`](comparisons.md) for the row-by-row breakdown). Brief positioning:

| Project | Paradigm | Relationship to feathers |
|---|---|---|
| **`bevy_ui_widgets`** | Headless primitives (behavior + a11y, no visuals) | **Sibling.** Feathers styles `bevy_ui_widgets` primitives; they ship together (both gated under their respective `experimental_*` features). |
| **`bevy_egui`** | Immediate-mode wrapper around `egui` | **Different paradigm.** Not built on `bevy_ui`; renders its own quads. Mature, widely used for in-game debug UI. Separate prior-art folder (planned). |
| **`bevy_lunex`** | Parallel UI, `Transform`-based | **Parallel stack.** Not built on `bevy_ui`; reimplements layout / rendering. The closest paradigm-cousin to Buiy's "parallel to bevy_ui" stance. Separate prior-art folder (planned). |
| **`sickle_ui`** | Themed widgets built on top of `bevy_ui` | **Direct ancestor.** Closest design-space neighbor — opinionated widget kit on `bevy_ui`, predates feathers. Separate prior-art folder (planned). |
| **`woodpecker_ui`** | Custom declarative API | **Parallel paradigm.** Successor to ideas in the archived `kayak_ui`. Separate prior-art folder (planned). |
| **`kayak_ui`** | Retained widget tree, custom DSL | **Archived 2024.** Cautionary tale: third-party kit with momentum that did not survive its maintainer's bandwidth. See [`comparisons.md`](comparisons.md). |

The "experimental" framing means feathers does not compete head-to-head with the mature third-party kits today — `bevy_egui` and `sickle_ui` continue to be the practical choices for general game / editor UI in late 2026.

## Relationship to Buiy

Buiy is **parallel to `bevy_ui`** (see [architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) — it integrates the same underlying primitives (Taffy, cosmic-text, AccessKit, bevy_picking, Bevy's render graph) directly with its own component model. Since feathers is built on `bevy_ui` + `bevy_ui_widgets` + `bevy_ui_render`, **Buiy is parallel to feathers by transitivity**. The relationship is *not* layered: Buiy does not extend feathers components, does not consume `bevy_ui_widgets` primitives, and does not theme feathers' tokens.

Coexistence model (per [cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)):

- **Per-window only.** A Bevy `App` may have multiple windows; each window is owned by one stack (Buiy or `bevy_ui`/feathers), not shared.
- **No same-tree mixing.** Inside a single Buiy tree, no raw `bevy_ui::Node` content. Feathers widgets are `bevy_ui::Node` descendants — they cannot be embedded in a Buiy hierarchy.
- **Distinct AccessKit adapters.** AccessKit allows exactly one tree per `accesskit_winit::Adapter` per window. Buiy owns the adapter on its windows; `bevy_a11y` owns it on feathers windows. No coordination crate in v1.
- **Migration is per-window.** An app moving from feathers to Buiy replaces a window's entire UI tree; it does not extend feathers components.

**Buiy is therefore not a feathers competitor in the same way feathers competes with `bevy_egui` or `sickle_ui`.** Both kits can run in the same `App` without contention as long as they own different windows. The Buiy-vs-feathers contrast lives at the architectural level (parallel stack vs `bevy_ui`-layer) and the scope level (web-platform-parity + game-UI vs editor/tooling). See [`comparisons.md`](comparisons.md) for the table.

The "Buiy is parallel to bevy_feathers" framing matters for two practical decisions:

1. An app building its own editor on top of Bevy can choose feathers (in-tree, official, editor-targeted) or Buiy (parallel stack, full web parity, game-UI capable). The choice is exclusive per window.
2. A studio with mixed needs (in-game UI + editor tooling) might run Buiy for the game window and feathers for tool windows — both stacks coexist at the `App` level without contention.

## Reverse dependencies on crates.io

`bevy_feathers` shows **0 reverse dependencies** on crates.io as of 2026-05-22. This is consistent with the experimental-flag gating: nothing publishes against an unstable API. Downloads (191,700 lifetime) are CI / tutorial / direct-consumer pulls.

## Sources

- Bevy 0.17 release notes — `https://bevy.org/news/bevy-0-17/`.
- PR #19730 description — `https://github.com/bevyengine/bevy/pull/19730`.
- `bevy_feathers` examples — `https://github.com/bevyengine/bevy/tree/main/examples/ui/widgets`.
- crates.io reverse-deps — `https://crates.io/crates/bevy_feathers/reverse_dependencies`.
- `awesome-bevy` — `https://github.com/bevyengine/awesome-bevy`.
- Tiny Glade UI architecture (community write-ups, various).
