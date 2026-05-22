**Date:** 2026-05-22
**Status:** active
**Subject:** Xilem + Masonry AccessKit integration — how the producer side works, what Buiy borrows

# Accessibility integration

This file documents how Xilem + Masonry plug into AccessKit. Verified by reading `masonry_core/Cargo.toml` (direct `accesskit` dep) and the source-level `Widget::accessibility` method shape. Cross-link to [`../accesskit/lessons.md`](../accesskit/lessons.md) is load-bearing — most of the *AccessKit-side* lessons live there; this file is the *Xilem/Masonry-side* read.

## How Masonry exposes a11y to widgets

Each `Widget` impl is asked to populate its accessibility info per frame via:

```rust
fn accessibility(
    &mut self,
    ctx: &mut AccessCtx<'_>,
    props: &PropertiesRef<'_>,
    node: &mut accesskit::Node,
);
```

Plus:

```rust
fn accessibility_role(&self) -> accesskit::Role;
```

This is *exactly* the AccessKit-correct producer shape: the widget gets a mutable `accesskit::Node` and populates the properties that matter for its role. No megacomponent wraps the node; the producer-toolkit talks to AccessKit directly.

This is **the model Buiy's accessibility layer also adopts**, with one important difference: Buiy's components are decomposed (`A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, `A11yRelations`), and the per-frame `BuiySet::A11yUpdate` system walks the ECS entities and builds the `TreeUpdate` from those components — there's no per-widget trait method, because Buiy isn't a trait-based widget toolkit. Functionally equivalent; mechanistically different.

## The platform-adapter wiring

Masonry's `masonry_winit` crate owns the `accesskit_winit::Adapter` lifecycle. The adapter is created in the window-construction flow, before the window is shown (per AccessKit's "panics if window already visible" constraint — see [`../accesskit/lessons.md`](../accesskit/lessons.md) Avoid row).

On Linux, `masonry_winit` enables `accesskit_winit`'s `async-io` feature (lighter dep closure than `tokio`). This matches the Buiy spec's open question in [`accesskit/lessons.md`](../accesskit/lessons.md) Avoid row "Forgetting the Unix async-runtime feature flag" — Linebender lands on `async-io`, which is a useful signal for Buiy's choice.

The adapter wiring lives in `masonry_winit::app_driver` and the window-driver event loop. Action requests from AccessKit flow back into Masonry's `on_access_event` widget callback, scoped to the widget whose `WidgetId` matches the action's `NodeId`.

## Stable NodeId from WidgetId

Masonry's `WidgetId` is a `NonZeroU64`. Stable across the widget's lifetime in the tree. Masonry uses this directly as the `accesskit::NodeId` — same scheme Buiy uses (`Entity::to_bits()` → `NodeId`). The id-stability matches AccessKit's diff-model expectation.

## Lazy tree gating

`accesskit_winit`'s `update_if_active` short-circuits unless an AT is attached. Masonry's accessibility pass uses this gate, so per-frame a11y cost is paid only when an AT is listening. Same gating Buiy uses (`AccessibilityRequested` resource → `BuiySet::A11yUpdate` short-circuits).

## Parley's AccessKit integration

Parley has an `accesskit` feature flag. When enabled, Parley's text-layout output includes per-run boundaries that map onto AccessKit's text-node properties (`set_text_run`, `set_character_lengths`, `set_character_positions`, etc.). This is how a Masonry text widget surfaces correctly-segmented text to a screen reader.

This is **the closest existing-art reference** for what Buiy's `buiy_text` needs to do with cosmic-text. cosmic-text doesn't ship an `accesskit` feature; `buiy_text` will have to build the equivalent text-run-to-AccessKit-node mapping ourselves. Reading Parley's `accesskit` module is the closest reference for what that mapping looks like in practice.

## Version pin lag

| Crate | Released 0.4.0 pin | Workspace HEAD pin |
|---|---|---|
| `accesskit` | 0.21.1 | 0.24.0 |
| `accesskit_winit` | (matching) | 0.33.0 |

The released 0.4.0 (2025-10-29) pinned AccessKit 0.21.1; workspace HEAD has moved to 0.24.0 in the months since. This **isn't unusual for pre-1.0 substrate** but matters for Buiy's planning:

- Buiy's foundation [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to "AccessKit major release between Bevy minors triggers a Buiy patch release with a documented migration note."
- Looking at Linebender, the same release would bump from `accesskit 0.21` → `accesskit 0.24` in a single Xilem-side release. That's three AccessKit minor bumps absorbed into one Xilem minor.
- For Buiy: if AccessKit ships 0.25 mid-Bevy-cycle and breaks the producer API, Buiy should *patch-release* with the bump, not bundle it with the next Bevy migration. The cadence-decoupling open question in Buiy's foundation README is **already settled in Linebender's direction** by Linebender's lived experience: AccessKit moves on its own clock, downstream UIs absorb the bumps as they ship.

## What Linebender + Xilem do *not* solve in their a11y story

- **ACCNAME 1.2 name computation.** AccessKit deliberately doesn't compute names; the consuming toolkit (Masonry) does. Masonry's name-computation lives inside individual widget `accessibility` methods, not as a centralized algorithm. Buiy's spec ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)) puts ACCNAME 1.2 in `buiy_core` as a centralized algorithm — that's a stronger position than Masonry's per-widget approach because consistency across widgets is enforced centrally.
- **`aria-relevant`.** AccessKit doesn't model it. Masonry doesn't ship a live-region-mutation-filtering layer; Buiy's global-announcer spec does. See [`../accesskit/lessons.md`](../accesskit/lessons.md) Avoid row "Hand-spelling `aria-relevant` semantics via AccessKit."
- **APG widget conformance.** Masonry ships a small widget set (button, text input, label, slider, etc.) — far short of the ~60 APG patterns Buiy's foundation [`media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) catalogs. Each Masonry widget has an AccessKit role assignment but no published evidence of full APG keyboard-contract conformance.
- **Real-AT verification.** Masonry's test harness snapshots the AccessKit tree; there's no published evidence of real-NVDA / real-VoiceOver / real-TalkBack utterance verification. This matches Buiy's stance (foundation [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) — real-AT testing is manual-release-gate only.

## Mobile / web

- **Android:** `masonry_winit` examples include `cdylib` builds (the workspace `Cargo.toml` calls out Android targets), but the `accesskit_android` adapter is pre-1.0 (per [`../accesskit/lessons.md`](../accesskit/lessons.md)). Masonry's Android a11y is incidental at best.
- **iOS:** No published Masonry-on-iOS examples. `accesskit_ios` 0.1.0 shipped recently per [`../accesskit/lessons.md`](../accesskit/lessons.md). Not in scope for Xilem 0.4.0.
- **Web:** `xilem_web` exists, but uses **the DOM** (not Masonry, not Vello). Web accessibility is inherited from the DOM's ARIA support. The Masonry-Vello path has no web target.

Buiy's foundation stance matches: desktop a11y in CI, mobile / web at manual release gate until adapter coverage matures.

## What Buiy borrows from this a11y story

(Detail in [`lessons.md`](lessons.md) Borrow #4.)

1. **The per-widget `accessibility(&mut node)` shape.** Buiy's decomposed-components model isn't trait-based but lands on the same producer-shape: the producer (widget or component-bundle) populates an `accesskit::Node` directly.
2. **`tree_arena`-style stable WidgetId → NodeId.** Buiy uses `Entity::to_bits()` for the same role.
3. **Linux `async-io` feature on `accesskit_winit`.** Linebender's choice is evidence for Buiy's same choice.
4. **Parley's `accesskit` text-run integration.** Buiy's `buiy_text` builds the equivalent for cosmic-text.

## What Buiy doesn't borrow

- **Per-widget ACCNAME computation.** Buiy centralizes ACCNAME 1.2 in `buiy_core`.
- **Per-widget role assignment in code.** Buiy assigns roles via the `A11yRole` component, which keeps roles BSN-authorable (the whole point of issue #17644's lesson).

## Cross-references

- [`../accesskit/lessons.md`](../accesskit/lessons.md) — the load-bearing decision file for AccessKit. Buiy decisions here defer to that file's Validates / Avoid / Borrow rows.
- [`text-and-rendering.md`](text-and-rendering.md) — Parley's `accesskit` feature in context.
- [`masonry-toolkit.md`](masonry-toolkit.md) — the `Widget` trait shape.

## Sources

- Masonry source: https://github.com/linebender/xilem/tree/main/masonry
- `masonry_core` `Widget` trait: https://docs.rs/masonry_core/latest/masonry_core/core/trait.Widget.html
- `masonry_winit` adapter wiring: https://github.com/linebender/xilem/tree/main/masonry_winit
- Parley's `accesskit` feature: https://github.com/linebender/parley
- AccessKit docs: https://docs.rs/accesskit/latest/accesskit/
- Cross-link: [`../accesskit/lessons.md`](../accesskit/lessons.md)
