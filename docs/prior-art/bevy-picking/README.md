**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — Bevy's first-party screen-picking / pointer-event primitive (Buiy hit-testing substrate)

# bevy_picking

`bevy_picking` is the Bevy workspace crate that provides pointer abstractions, backend-pluggable hit-testing, and high-level pointer events (hover, click, drag) for entities in a Bevy `World`. It is the **canonical hit-testing primitive Buiy builds on**: Buiy registers its own `bevy_picking` backend per [`architecture.md` § 2.9](../../specs/2026-05-07-buiy-foundation/architecture.md) and routes Buiy interaction off the same `Pointer<E>` observer events bevy_ui consumes (see [`interaction.md` § 3.7](../../specs/2026-05-07-buiy-foundation/interaction.md), [`cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).

Historically, this functionality lived in the **`bevy_mod_picking`** external crate (by Aevyrie, 2020–2024); it was upstreamed into Bevy 0.15 (PR series including #15800) and the external crate was archived 2025-03-04.

## Key facts

| Field | Value |
| --- | --- |
| Crate | `bevy_picking` (Bevy monorepo workspace crate) |
| Repo | https://github.com/bevyengine/bevy/tree/main/crates/bevy_picking |
| Latest stable | **0.18.1** (published 2026-03-04) |
| Pre-release | `0.19.0-rc.1` (2026-05-13) |
| First "modern-era" release | `0.15.0` (2024-11-29) — the upstreaming release |
| Placeholder squat | `0.0.1` (2020-09-17) — reserved name, not functional |
| Total versions on crates.io | 26 |
| License | MIT OR Apache-2.0 |
| Downloads (cumulative) | 1,676,103 (558,828 recent — 90-day) |
| Predecessor | [`bevy_mod_picking`](https://github.com/aevyrie/bevy_mod_picking) — archived 2025-03-04, last version 0.20.1 (2024-07-09), 44 versions |
| Original architect | Aevyrie (`@aevyrie`) |
| 0.15 upstreaming work | @aevyrie + @NthTensor + @TotalKrill + @jnhyatt + @Jondolf |
| Edition | Rust 2024 |
| External deps (runtime) | `uuid`, `tracing`, `crossbeam-channel` (optional, mesh_picking feature) |
| Cargo features | `mesh_picking` (off by default) |
| Bundled backends | `mesh_picking` (in-crate, feature-gated); UI / sprite / window backends live in **sibling** Bevy crates (`bevy_ui`, `bevy_sprite`, and `bevy_picking::window`) |

## Reading order

1. [`README.md`](README.md) (this file) — orient.
2. [`architecture.md`](architecture.md) — the four-stage pipeline (Input → Backend → Hover → Events), schedules, and system sets.
3. [`backends.md`](backends.md) — what backends ship in-tree, how they register, how Buiy's backend will slot in.
4. [`api.md`](api.md) — the public surface: `Pickable`, `Pointer<E>`, observer events, drag lifecycle.
5. [`capabilities.md`](capabilities.md) — what bevy_picking does and does not do today.
6. [`integration.md`](integration.md) — Buiy's backend registration, `bevy_a11y` interaction, coexistence with `bevy_ui`'s backend.
7. [`history.md`](history.md) — `bevy_mod_picking` → `bevy_picking`, API churn 0.15 → 0.18.
8. [`distribution.md`](distribution.md) — repo, license, release cadence, platform support.
9. [`open-problems.md`](open-problems.md) — critiques and structural gaps (gamepad picking, a11y-driven focus, multi-window pointer, sub-pixel hit, backend-priority API).
10. [`lessons.md`](lessons.md) — **the consult-this file**: validates / avoid / borrow for Buiy designers.
11. [`glossary.md`](glossary.md) — quick term lookup.

## Glossary stub

- **Backend** — A plugin that reads `PointerLocation` and emits `PointerHits` events. Independent backends compose; bevy_ui, bevy_sprite, and Buiy each register their own.
- **Pointer** — An abstract screen-located input source (mouse, finger, pen, gamepad-driven virtual, custom). Identified by `PointerId`. Multiple pointers coexist.
- **HitData** — Per-hit payload: depth (f32, semantic ordering), optional world position, optional normal, camera entity.
- **PointerHits** — A backend's per-frame report: pointer + list of `(Entity, HitData)`, plus a backend-supplied `order` f32 (higher = on top across backends).
- **Pickable** — Opt-in component with `should_block_lower` + `is_hoverable` knobs. Absent ⇒ default behaviour (block lower + hoverable). `Pickable::IGNORE` ⇒ pretend the entity isn't there.
- **Picking** — The whole pipeline; also colloquially the act of selecting an entity under the cursor.
- **Pointer<E>** — The high-level event wrapper: `{ entity, pointer_id, pointer_location, event: E }` where `E` is one of `Over`, `Out`, `Move`, `Press`, `Release`, `Click`, `Drag*`, `Scroll`, `Cancel`. Delivered as `EntityEvent`s that bubble.

See [`glossary.md`](glossary.md) for the full term list.

## Framing disclosure

These docs are written from a **Buiy-as-parallel-stack-to-bevy_ui** stance — most "Implications for Buiy" sub-sections frame bevy_picking's choices through that lens. The corpus is intentionally **load-bearing-dependency** prior art: Buiy hard-bakes against bevy_picking as its hit-testing substrate (per [`architecture.md` § 2.2 / § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md)), so the lessons file is biased toward "what do we inherit, what do we need to extend, what do we work around." Future readers auditing whether the parallel-stack stance is itself the right primitive should weigh the corpus accordingly: it's a learn-from-bevy_picking-into-Buiy artifact, not a neutral catalog. The corpus also has the standard load-bearing-dependency conflict-of-interest: there is incentive to soft-pedal problems Buiy will inherit because it cannot easily swap the dependency. [`open-problems.md`](open-problems.md) and [`critiques`](open-problems.md#critiques) deliberately surface those.

## Sources

- https://crates.io/crates/bevy_picking (versions, license, downloads)
- https://crates.io/api/v1/crates/bevy_picking/versions
- https://crates.io/crates/bevy_mod_picking
- https://github.com/bevyengine/bevy/tree/main/crates/bevy_picking
- https://github.com/aevyrie/bevy_mod_picking (archived 2025-03-04)
- https://docs.rs/bevy_picking/latest/bevy_picking/
- https://bevy.org/news/bevy-0-15/ (upstreaming announcement)
- Buiy foundation spec: `docs/specs/2026-05-07-buiy-foundation/{architecture,interaction,cross-cutting}.md`
