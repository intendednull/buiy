# Buiy layout — Phase 8: transforms + containment

**Date:** 2026-05-28
**Status:** landed
**Spec:** [`specs/2026-05-08-buiy-layout-design/transforms-and-containment.md`](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md) § 1 (`UiTransform` + `TransformMatrix`/`TransformOrigin`/`TransformStyle`/`BackfaceVisibility`), § 1.1 (`Translate`/`Rotate`/`Scale` longhands + composition order), § 5 (`Containment` + `ContainFlags`/`ContentVisibility`/`WillChange`/`WillChangeProperty`) + [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md) § 3 (sub-pass 6e), § 6 (error model).
**Supersedes:** none (graduates the unbuilt `transforms-and-containment` spec children; sub-pass 6e extends the Phase-7 `PostTaffyOverrides` chain — `clear → sticky 6a → table 6b → multicol 6c → anchor 6d` — by appending `transform_composition` 6e after `anchor_resolution`).

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development`. Each task lists exact file paths and TDD steps; steps use checkbox (`- [ ]`) tracking. Run the project gate (below) before every commit and resolve every warning.

**Goal:** Land the transform layer (`UiTransform` self-styling component + the `Translate`/`Rotate`/`Scale` decomposed-only longhands + the `TransformMatrix`→`Mat4` composition convention `M = T·R·S·M_transform`) and the containment layer (`Containment` self-styling component carrying `ContainFlags`, `ContentVisibility`, and the tier-E `WillChange` hint). A new `PostTaffyOverrides` sub-pass **6e** (`transform_composition`) composes the resolved matrix and writes it to a new private `ResolvedTransform` component — the render handoff for transforms, mirroring how `ResolvedLayout` is the render handoff for position+size. Sub-pass 6e runs **after** `anchor_resolution` (6d) per spec [§ 1.1](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md#11-longhand-components). The only concrete layout-side containment effect Phase 8 ships is **SIZE / INLINE_SIZE** containment (auto width/height → 0 with a warn-once); `content-visibility` and `will-change` are stored-only (deferred / tier-E). Phase 8 does **not** touch stacking-context formation (Phase 9, sub-pass 6f reads the matrix 6e produces).

**Architecture (3 sentences):**
1. **Transform composition as a pure post-Taffy overlay that does not move the layout box.** Sub-pass 6e `transform_composition` queries entities with `UiTransform` (+ optional decomposed-only `Translate`/`Rotate`/`Scale` longhands), skips `Display::None`, and for each non-identity transform composes `M = T_translate · R_rotate · S_scale · M_transform` via the pure helper `compose_transform(ui, t, r, s) -> Mat4` and writes the result to the private `ResolvedTransform` component. A transform does **not** displace the layout box, so — unlike sticky (6a) / table (6b) / multicol (6c) / anchor (6d) — 6e writes **nothing** to `PostTaffyPositionOverrides`; it owns a separate render-handoff component. For identity transforms it inserts no `ResolvedTransform` (and removes a stale one), mirroring the spec § 7 "identity → no `ResolvedTransform`" contract.
2. **`ResolvedTransform`, not Bevy `Transform` (deliberate, documented divergence from spec § 2 approach (a) at the implementation-timing level).** The spec recommends approach (a) — `write_resolved_layout` composing into the entity's Bevy `Transform` so Bevy's `TransformSystems::Propagate` owns `GlobalTransform`. Phase 8 produces the spec's `ResolvedTransform` artifact but does **not** write Bevy `Transform`/`GlobalTransform`: render reads `ResolvedLayout` directly (`render/mod.rs:98`), `buiy_core` has zero Bevy-`Transform` wiring, and the layout test harness uses `MinimalPlugins` (no `TransformPlugin`), so a `Transform` write today is dead code that nothing consumes (YAGNI). The Bevy-`Transform`-ownership bridge is deferred to render-pipeline / 3D-anchored-UI; a follow-up tracks it (D2).
3. **Containment is a `Style` self-styling field; only SIZE containment has a Phase 8 layout effect.** `Containment { contain, content_visibility, will_change }` is added to the `Style` bundle (self-styling perf opt-in, mirroring `Container`). The one concrete layout effect is `ContainFlags::SIZE` / `INLINE_SIZE`: when the corresponding `BoxModel.width`/`height` is `Sizing::Auto`, `style_to_taffy` treats it as `Sizing::Length(Length::px(0.0))` and emits `warn!` once per (entity, session) via `LayoutWarnOnceKey::SizeContainmentZeroed(Entity)` — this requires threading `Containment` into the `StyleView` / `sync_styles` query. `LAYOUT`/`PAINT`/`STYLE` flags are stored for render/future use (no Phase 8 layout effect beyond what Taffy already does); `content-visibility != Visible` is stubbed (warn-once `ContentVisibilityDeferred(Entity)`, value stored); `will-change` is stored-only with no warn.

**Tech Stack:** Bevy 0.18 (`bevy::math::{Mat4, Quat, Vec3, Vec2}`, `Query<&UiTransform>`, `Option<&Translate>`/`Option<&Rotate>`/`Option<&Scale>`, `Commands::insert`/`remove`, `Resource<HashSet>`). `bitflags` (already a transitive dep via `taffy`? — verify in T8; if not present as a direct dep, add it with `cargo add bitflags` and run `cargo deny check`). `bevy_reflect`'s `impl_reflect_opaque!` macro for the `bitflags` opaque-type registration (renamed from `impl_reflect_value!` in bevy_reflect 0.18). `std::collections::HashSet` (no `bevy::utils::HashSet`, per Phase 6/7 precedent). **One possible new external dependency: `bitflags`** (gated on the T8 verification).

---

## Prior-art citations (used throughout this plan)

Each task below references these. Quoting the file + line here once so individual tasks stay tight.

- **Pipeline sub-pass chain** — `crates/buiy_core/src/layout/mod.rs:180-188`: a `.chain().in_set(BuiyLayoutStep::PostTaffyOverrides)` tuple `(clear_post_taffy_overrides, sticky_offset, table_layout, multicol_pack, anchor_resolution)`. Phase 8 appends `transform_composition` as the **6th** element (sub-pass 6e), after `anchor_resolution` (6d). No new `BuiyLayoutStep` variant — 6e lives inside the existing `PostTaffyOverrides` set (`crates/buiy_core/src/layout/pipeline.rs:16-44`).
- **`PostTaffyPositionOverrides` shape + role** — `crates/buiy_core/src/layout/systems.rs:174` (`pub struct PostTaffyPositionOverrides { by_entity: HashMap<Entity, Vec2> }`). Phase 8's `transform_composition` does **NOT** write to this map (a transform does not move the layout box) — it writes `ResolvedTransform` instead. This is the structural difference from 6a–6d.
- **Render handoff is `ResolvedLayout`** — `crates/buiy_core/src/render/mod.rs:98` (`extract_buiy_draws` reads `ResolvedLayout.position` / `.size` directly). `ResolvedLayout` is at `crates/buiy_core/src/components.rs:22` (`#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component)] pub struct ResolvedLayout { position: Vec2, size: Vec2 }`). Phase 8's `ResolvedTransform` mirrors this exact derive set and home (`components.rs`), as the render handoff for the composed matrix. There is **no** Bevy `Transform`/`GlobalTransform` usage anywhere in `buiy_core`; the layout harness uses `MinimalPlugins` (no `TransformPlugin`) — this is why D2 defers the Bevy-`Transform` write.
- **`write_resolved_layout` writes only `ResolvedLayout`** — `crates/buiy_core/src/layout/systems.rs:1662` (idempotent insert: compares `cur.position == new.position && cur.size == new.size` before `commands.entity(e).insert(new)`). Phase 8 does not modify this system; `ResolvedTransform` is written by 6e, not by step 7.
- **Per-session warn-once dedup** — `crates/buiy_core/src/layout/systems.rs:201` (`pub struct LayoutWarnedOnceSession { set: HashSet<LayoutWarnOnceKey> }`); `LayoutWarnOnceKey` enum at `crates/buiy_core/src/layout/types.rs:975` (currently `TableUnsupported(Entity)` / `MulticolUnsupported` / `StickyFrUnsupported(Entity)` / `StickyCqDeferred(Entity)`). Dedup idiom: `if warned.set.insert(LayoutWarnOnceKey::X) { warn!(...) }`. Phase 8 adds `SizeContainmentZeroed(Entity)` and `ContentVisibilityDeferred(Entity)` (D8). The enum is already `register_type`'d at `mod.rs:157`; `Reflect` picks up the new variants for free.
- **Component-registration chain** — `crates/buiy_core/src/layout/mod.rs:101-157` (one long `app.register_type::<T>()` chain, grouped by phase, terminating in the Phase-7 group `MultiColumn … LayoutWarnOnceKey`). Phase 8 appends a "Phase 8 — transforms + containment" group.
- **`Style` bundle + fluent setters** — `crates/buiy_core/src/layout/style.rs:45` (`#[derive(Bundle, Clone, Debug, Default)] pub struct Style { … }` — every field is always inserted on spawn). The `Container` field + setter (`style.rs:445`, `pub fn container(mut self, c: Container) -> Self`) and the Phase-7 `multi_column` field + setter (`style.rs:457`) are the precedent Phase 8's `ui_transform` / `containment` fields follow.
- **Decomposed-only precedent for `Translate`/`Rotate`/`Scale`** — spec [`architecture.md § 2.4`](../specs/2026-05-08-buiy-layout-design/architecture.md#24-child-side-components-decomposed-only). `FlexItem` (child-side) and `Anchor` (relational) are decomposed-only (spawned alongside `Style`, not bundle fields). The CSS `translate`/`rotate`/`scale` longhands are rare and additive to `UiTransform`; they follow the decomposed-only path. `UiTransform` and `Containment` are self-styling and so are `Style` fields (D4).
- **`StyleView` + `sync_styles` query + `style_to_taffy`** — `crates/buiy_core/src/layout/translate.rs:217` (`pub struct StyleView<'a> { display, box_model, position, … }`), `crates/buiy_core/src/layout/systems.rs:1302` (`sync_styles` query tuple + nested `Or<>` change filter), `crates/buiy_core/src/layout/translate.rs:256` (`pub fn style_to_taffy(view) -> taffy::Style`, with `box_model.width`/`height` flowing through `sizing_to_dim(normalize_cq_sizing(…))`). T10 threads `Containment` through this chain so `style_to_taffy` can zero `Auto` size under SIZE containment.
- **`bitflags` opaque reflect registration** — spec § 5 mandates `impl_reflect_opaque!(ContainFlags(Default, PartialEq))` because `bitflags!` doesn't compose with `#[derive(Reflect)]`. Cross-check whether `bitflags` is already a direct dep in T8.
- **Test harness** — `fn app() { let mut app = App::new(); app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin); app }` (no `TransformPlugin`); spawn `(Node, Style::default()…)`; `app.update()`; assert via `app.world().get::<ResolvedTransform>(e)` / resource reads. Unit tests use `Entity::from_raw_u32(n).unwrap()`. Existing files: `crates/buiy_core/tests/layout_sticky.rs`, `tests/layout_pipeline_order.rs`, `tests/layout_post_taffy_overrides_clear.rs`.

---

## File map (what each task touches)

| File | Touched by tasks |
|---|---|
| `docs/plans/2026-05-28-buiy-layout-transforms-containment.md` | T1 (this file) |
| `docs/README.md` | T1 (Phase 8 entry under "### Layout"), T14 (status tag) |
| `crates/buiy_core/src/layout/types.rs` | T2 (`TransformMatrix`, `TransformOrigin`+manual Default, `TransformStyle`, `BackfaceVisibility`), T8 (`ContainFlags`, `ContentVisibility`, `WillChange`, `WillChangeProperty` + `impl_reflect_opaque!`), T10 (new `LayoutWarnOnceKey` variants `SizeContainmentZeroed`, `ContentVisibilityDeferred`) |
| `crates/buiy_core/src/layout/components.rs` | T3 (`UiTransform`), T4 (`Translate`/`Rotate`/`Scale`), T9 (`Containment`) |
| `crates/buiy_core/src/components.rs` | T5 (`ResolvedTransform` private render-handoff component) |
| `crates/buiy_core/src/layout/systems.rs` | T5 (`compose_transform` helper), T6 (`transform_composition` system 6e), T10 (`Containment` zeroing in `sync_styles`/`style_to_taffy` path), T11 (content-vis/will-change stub warns) |
| `crates/buiy_core/src/layout/translate.rs` | T10 (add `containment: &Containment` to `StyleView`; zero `Auto` size under SIZE/INLINE_SIZE in `style_to_taffy`) |
| `crates/buiy_core/src/layout/style.rs` | T7 (`ui_transform: UiTransform` field + fluent setters), T9 (`containment: Containment` field + setter) |
| `crates/buiy_core/src/layout/mod.rs` | T6 (wire `transform_composition` into the chain after `anchor_resolution`), T13 (`register_type` group + `pub use` re-exports) |
| `crates/buiy_core/src/lib.rs` | T13 (re-export new public types + `ResolvedTransform`) |
| `crates/buiy/src/lib.rs` | T13 (re-export same set from top-level facade) |
| `crates/buiy_core/tests/layout_pipeline_order.rs` | T6 (assert 6e runs — transformed entity gets a `ResolvedTransform`) |
| `crates/buiy_core/tests/layout_transforms.rs` | T12 (new file — composition + layout-flow-unchanged integration tests) |
| `crates/buiy_core/tests/layout_containment.rs` | T12 (new file — SIZE-containment zeroing + content-vis deferred warns) |
| `CHANGELOG.md` | T14 |
| `docs/plans/follow-ups.md` | T14 |

No changes to: `crates/buiy_core/src/render/mod.rs` (render reads `ResolvedLayout`; `ResolvedTransform` consumption is a render-pipeline follow-up), `crates/buiy_core/src/layout/pipeline.rs` (6e lives in the existing `PostTaffyOverrides` set), `crates/buiy_core/src/layout/tree.rs`.

---

## Decision blocks (locked-in choices the implementer must honor)

### D1. Phase 8 scope = transforms + containment, NOT stacking / top-layer

**Decision:** Phase 8 ships (a) the transform layer — `UiTransform` + the `Translate`/`Rotate`/`Scale` decomposed-only longhands + the `compose_transform` matrix composition convention `M = T·R·S·M_transform` + the `transform_composition` sub-pass 6e + the `ResolvedTransform` render handoff — and (b) the containment layer — the `Containment` component (`contain` flags, `content-visibility`, `will-change`), with the single concrete layout effect being SIZE/INLINE_SIZE containment zeroing. Phase 8 does **NOT** implement stacking-context formation or top-layer; those are Phase 9 (sub-pass 6f), which reads the composed matrix 6e produces.

**Why:** Spec [§ 1.1](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md#11-longhand-components) and [§ 3](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md#3-stacking-context-formation) explicitly sequence the work: 6e (transform composition, Phase 8) runs before 6f (stacking detection, Phase 9) "since it reads the composed matrix produced by 6e." Splitting at this seam keeps Phase 8 self-contained — 6e produces an artifact (`ResolvedTransform`) that 6f later consumes — and avoids bundling the entire stacking system (z-index sort, isolation, top-layer escape) into a transform/containment phase.

**How to apply:** Implement only the spec § 1, § 1.1, § 5 types + sub-pass 6e + SIZE containment. Add a follow-up entry pointing Phase 9 at "sub-pass 6f reads `ResolvedTransform`."

**Runner-up rejected:** Bundle stacking into Phase 8 because spec § 3 / § 6 list the SC triggers in the same file. Rejected: the triggers are *documented* in this spec file for completeness, but the spec's own pipeline sequencing (6e then 6f) and the cross-reference to `stacking-and-top-layer.md` make the phase boundary explicit. Bundling would balloon scope past the "bite-sized" bar.

### D2. `ResolvedTransform`, not Bevy `Transform` (KEY decision — deliberate divergence from spec § 2 approach (a) at the implementation-timing level)

**Decision:** Phase 8 writes a new **private** component `ResolvedTransform` holding the composed transform as a `Mat4` (`pub matrix: Mat4`), and consumed by render later — mirroring how `ResolvedLayout` is the render handoff for position+size. Phase 8 does **NOT** write Bevy `Transform` or `GlobalTransform`. Use `Mat4` (not `Affine3A`) for the stored matrix.

**Why (`ResolvedTransform` over Bevy `Transform`):** The spec § 2 recommends approach (a) — `write_resolved_layout` composes the layout position + the resolved transform into the entity's Bevy `Transform`, letting Bevy's `TransformSystems::Propagate` own `GlobalTransform`. But as of Phases 0–7 (spec § 2's own "Not yet in effect" callout): `write_resolved_layout` (`systems.rs:1662`) writes **only** `ResolvedLayout`; render (`render/mod.rs:98`) consumes `ResolvedLayout` directly; there is **zero** Bevy-`Transform` wiring anywhere in `buiy_core`; and the layout test harness uses `MinimalPlugins` (no `TransformPlugin`, so no `Propagate` system, so a `GlobalTransform` would never be recomposed). Writing Bevy `Transform` now would be **dead code** — nothing reads the resulting `GlobalTransform`. The spec's `ResolvedTransform` artifact IS produced by Phase 8 (so the data exists for render and for Phase 9's 6f); only the Bevy-`Transform` *write* is deferred. This keeps Phase 8 consistent with the shipped `ResolvedLayout` render handoff (YAGNI: don't wire a propagation path no consumer needs).

**Why `Mat4` over `Affine3A`:** `Mat4` is the simplest, most direct representation of the spec's `M · p` convention; it is 3D-ready (the spec's `Translate(Length, Length, Length)`, `Rotate(Quat)`, `Scale(f32,f32,f32)`, `Skew`, `Perspective`, `Preserve3d` are all expressible as a 4×4); `bevy::math::Mat4` is in the glob `bevy::prelude::*` already imported in `systems.rs:29`; and `Mat4::IDENTITY`, `Mat4::from_translation`, `Mat4::from_quat`, `Mat4::from_scale`, and `*` (matrix product) map 1:1 onto the spec's composition. `Affine3A` is faster for affine-only transforms but cannot represent a general perspective `Mat4` (`TransformMatrix::Matrix(Mat4)` / future `perspective`), so it would force a representation change the moment perspective lands.

**How to apply:** Define `ResolvedTransform { pub matrix: Mat4 }` in `crates/buiy_core/src/components.rs` next to `ResolvedLayout`, same derive set (`#[derive(Component, Reflect, Clone, Debug)] #[reflect(Component)]`). Give it a `Default` (identity) and a `PartialEq` (for the idempotent-insert gate in 6e). It is `pub` at the crate level (render needs it eventually) but is **not** written by `write_resolved_layout` — only by `transform_composition` (6e). Add the follow-up "Bevy-`Transform` ownership bridge / `GlobalTransform` write (spec § 2 approach (a))" to `follow-ups.md`.

**Runner-up rejected:** Follow spec approach (a) literally and write Bevy `Transform` in Phase 8. Rejected: it is dead code under the current render + harness reality (no `TransformPlugin`, render reads `ResolvedLayout`), and would require pulling `TransformPlugin` into the layout harness and `CorePlugin` just to make `GlobalTransform` meaningful — scope creep with no consumer. The spec's approach (a) is the correct *eventual* design; Phase 8 produces the `ResolvedTransform` it depends on and defers the bridge.

**Second runner-up rejected:** Store `Affine3A` instead of `Mat4`. Rejected: cannot represent perspective / arbitrary `TransformMatrix::Matrix(Mat4)`; forces a later representation churn.

### D3. `transform_composition` is `PostTaffyOverrides` sub-pass 6e, appended after `anchor_resolution`

**Decision:** `transform_composition` is the 6th element of the `PostTaffyOverrides` chained tuple (`mod.rs:180-188`), inserted **after** `anchor_resolution` (6d). It reads `UiTransform` + optional `Translate`/`Rotate`/`Scale` longhands, composes per spec § 1 (`M = T_translate · R_rotate · S_scale · M_transform`), and writes `ResolvedTransform`. It **skips `Display::None`** entities. For identity transforms (`UiTransform` `matrix == None`, default origin/style/etc., AND no/identity longhands), it does **NOT** insert `ResolvedTransform` — and it **removes a stale one** if present — mirroring the spec § 7 "identity → no `ResolvedTransform`" test.

**Why:** Spec [§ 1.1](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md#11-longhand-components): "The composition runs as `PostTaffyOverrides` sub-pass **6e** … extending the shipped sub-pass chain: 6a sticky, 6b table, 6c multicol, 6d anchor." Running after 6d means the composed matrix is available before step 7 (`WriteResolvedLayout`) and before Phase 9's 6f. Skipping `Display::None` matches every other sub-pass (`Display::None` entities are removed from layout). The identity→no-component rule keeps entities clean (Phase-1 "don't pollute entities with components they don't need" invariant — `ResolvedTransform` is an *optional* render handoff, written only when there is a transform to render, unlike the always-present `ResolvedLayout`). Removing a stale `ResolvedTransform` handles the animation case where a transform is set then cleared.

**How to apply:** `transform_composition` is `pub(super) fn` in `systems.rs`; wire it as `systems::transform_composition` after `systems::anchor_resolution` in the `mod.rs` tuple. Inside: iterate `Query<(Entity, &UiTransform, Option<&Translate>, Option<&Rotate>, Option<&Scale>, &Display), With<Node>>`; `continue` on `Display::None`; compute `m = compose_transform(ui, t, r, s)`; if `m == Mat4::IDENTITY` (i.e. identity transform) → `commands.entity(e).remove::<ResolvedTransform>()` only if one exists (use `Option<&ResolvedTransform>` in the query or a `Has<ResolvedTransform>` filter to avoid removing nothing); else idempotent-insert `ResolvedTransform { matrix: m }`.

**Runner-up rejected:** Write `ResolvedTransform` for every `UiTransform`-bearing entity including identity. Rejected: spec § 7 explicitly says "Identity transform … produces no … `ResolvedTransform`," and an always-present identity matrix is wasted storage + a spurious render-side "this entity is transformed" signal.

### D4. `UiTransform` + `Containment` are `Style` fields; longhands are decomposed-only

**Decision:** `UiTransform` and `Containment` are *self-styling, container-agnostic* properties, so they are added to the `Style` bundle as fields (`ui_transform: UiTransform`, `containment: Containment`) with fluent setters (`.ui_transform(t)`, plus ergonomic shortcuts `.translate_px(x, y)`, `.rotate_z(rad)`, `.scale(s)`; `.containment(c)` plus `.contain(flags)`). The `Translate`/`Rotate`/`Scale` **longhands** are **decomposed-only** — spawned alongside `Style` (`commands.spawn((Style::default()…, Translate(…)))`), NOT bundle fields — mirroring the `FlexItem`/`Anchor` decomposed-only precedent.

**Why:** Spec [§ 1](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md#1-uitransform) defines `UiTransform` as the entity's own visual transform (self-styling); spec § 5 defines `Containment` as the entity's own perf opt-in (self-styling). Per [`architecture.md § 2.4`](../specs/2026-05-08-buiy-layout-design/architecture.md#24-child-side-components-decomposed-only), self-styling properties live in `Style`. The CSS `translate`/`rotate`/`scale` longhands are *additive, rare* properties applied independently of `transform`; bundling all three into every `Style` (which derives `Bundle`, inserting every field unconditionally) would pollute every entity with three components that are almost always at their (identity) default. Decomposed-only keeps them off entities that don't use them — the same reasoning that makes `FlexItem` `Option<&FlexItem>` in the `sync_styles` query.

**How to apply:** Add the two `Style` fields + setters (T7, T9). Document the longhands as decomposed-only in their doc comments (T4) and in the `transform_composition` query (`Option<&Translate>` etc. — T6). The composition helper takes the longhands as `Option<&_>` so absence = identity contribution.

**Runner-up rejected:** Make `Translate`/`Rotate`/`Scale` `Style` fields too. Rejected: pollutes every Style-spawned entity with three near-always-default components; the CSS longhands are explicitly the "rare, additive" path; decomposed-only matches the `FlexItem` precedent.

### D5. Containment layout effect = SIZE / INLINE_SIZE zeroing only

**Decision:** The only concrete layout-side effect Phase 8 ships is SIZE / INLINE_SIZE containment: when `Containment.contain` contains `ContainFlags::SIZE` (both axes) or `ContainFlags::INLINE_SIZE` (inline axis only), and the corresponding `BoxModel.width`/`height` is `Sizing::Auto`, `style_to_taffy` treats it as `Sizing::Length(Length::px(0.0))` and emits warn-once `LayoutWarnOnceKey::SizeContainmentZeroed(Entity)`. This requires adding `Containment` to the `StyleView` and `sync_styles` query so `style_to_taffy` can read it. `LAYOUT` / `PAINT` / `STYLE` flags are **stored** for render / future use (no Phase 8 layout effect beyond what Taffy already does for block formatting contexts).

**Why:** Spec [§ 5.1](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md#51-effect-of-contain): "`SIZE` | The entity's size *must be* explicit … If size containment is enabled and width/height are `Sizing::Auto`, treat as `Sizing::Length(Length::px(0.0))` and `warn!`." `INLINE_SIZE` is the "inline-axis variant of `SIZE`." `LAYOUT`/`PAINT`/`STYLE` are "Render-side primarily; layout records" / "Mostly render-side" per the same table — no Taffy-side action in v1. The warn matches the spec's explicit `warn!` requirement; per-(entity, session) dedup is the canonical spec § 6 pattern.

**How to apply:** Thread `Containment` through `sync_styles` query → `StyleView.containment` → `style_to_taffy`. In `style_to_taffy`, before `sizing_to_dim(view.box_model.width)`: if `contain` has `SIZE` (or `INLINE_SIZE` for the inline axis under the resolved writing mode) and the axis sizing is `Sizing::Auto`, substitute `Sizing::Length(Length::px(0.0))`. The warn-once fires from `sync_styles` (which holds `ResMut<LayoutWarnedOnceSession>`), not from the pure `style_to_taffy` (keep it pure); `sync_styles` checks the condition and records the key + warns, then `style_to_taffy` reads a `Containment` already on the `StyleView` and does the substitution deterministically. (Concretely: `style_to_taffy` does the substitution from the flags; `sync_styles` does the warn-once side effect — separation keeps `style_to_taffy` a pure mapping.)

**Inline-axis note:** `INLINE_SIZE` zeroes the *inline* axis only. Under horizontal writing modes inline = width; under vertical writing modes inline = height. Use `view.writing_mode_resolved` (already on `StyleView`) to pick the axis, reusing the same inline/block axis logic the `Cqi`/`Cqb` resolution already uses. If the writing-mode axis mapping helper is non-trivial to reach from `style_to_taffy`, the minimum correct Phase 8 behavior is: `INLINE_SIZE` zeroes width under horizontal modes and height under vertical modes — document the chosen mapping in the code comment.

**Runner-up rejected:** Implement `LAYOUT` containment (force a new block formatting context / isolate descendants from ancestor sizing). Rejected: spec § 5.1 says Taffy "already gets close to this for block formatting contexts" and the strict version is a change-detection / render concern, not a Taffy-emit change — out of Phase 8 scope. Stored for future.

### D6. `content-visibility` is stubbed + deferred in Phase 8

**Decision:** Both `ContentVisibility::Auto` (off-screen paint/layout skip) and `ContentVisibility::Hidden` (Display::None-for-descendants) are **NOT** implemented in Phase 8. The value is **stored** on `Containment`; an entity with `content_visibility != Visible` emits warn-once `LayoutWarnOnceKey::ContentVisibilityDeferred(Entity)`. Full impl is a follow-up.

**Why:** Spec [§ 5.2](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md#52-content-visibility-auto): `Auto` needs "last frame's `ResolvedLayout`" + a viewport check + a `contain-intrinsic-size` opt-in hint (none of which Phase 8 builds), and `Hidden` needs a tree-skip path equivalent to `Display::None` for descendants (a `sync_styles` subtree-prune). Both are substantial pipeline changes (step-1 skip logic, sentinel Taffy sizes) that are tangential to transform composition; bundling them would balloon Phase 8. The warn-once tells authors the value is recognized but not yet enforced.

**How to apply:** In `transform_composition` (which already iterates layout entities) OR a tiny dedicated check inside `sync_styles`, emit the warn-once when `content_visibility != ContentVisibility::Visible`. Prefer `sync_styles` (it already holds `ResMut<LayoutWarnedOnceSession>` and the `Containment` after T10) — colocate with the SIZE-containment warn. Add the follow-up "content-visibility `Auto` + `Hidden` full impl."

**Runner-up rejected:** Implement `Hidden` (it's "just" `Display::None` for descendants). Rejected: it still needs a subtree-prune in `sync_styles` + a snap-back path, and the spec groups it with `Auto` as a unit of deferred work. Half-implementing one of the two is more confusing than deferring both with a clear warn.

### D7. `will-change` is tier-E, stored-only, no warn

**Decision:** The `WillChange` field is **stored** on `Containment` for forward-compat. Phase 8 ships NO layer-promotion and NO stacking-context-trigger behavior for it. No warn fires (it is a valid stored hint, not an error or unimplemented-stub).

**Why:** Spec [§ 5.3](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md#53-will-change): "`will-change` is foundation tier-E … v1 ships the `WillChange` API surface for forward compatibility, but the layer-promotion hint and the SC-forming behavior below are deferred until user demand." SC triggers are Phase 9 regardless. Unlike `content-visibility` (a recognized-but-unenforced layout property → warn), `will-change` is a *hint* that is correctly stored and consumed later by render — warning would be noise on valid usage.

**How to apply:** Just store it (it is a `Containment` field). No system reads it in Phase 8. No `LayoutWarnOnceKey` variant. Add a follow-up "will-change layer promotion + SC trigger."

**Runner-up rejected:** Warn on any non-`Auto` `WillChange`. Rejected: it's a valid hint, not an error; warning on correct usage trains authors to ignore warnings.

### D8. New `LayoutWarnOnceKey` variants: `SizeContainmentZeroed(Entity)`, `ContentVisibilityDeferred(Entity)`

**Decision:** Add two variants to `LayoutWarnOnceKey` (`types.rs:975`): `SizeContainmentZeroed(Entity)` (D5) and `ContentVisibilityDeferred(Entity)` (D6). Per-(entity, session) dedup via `LayoutWarnedOnceSession` (`systems.rs:201`), the canonical spec § 6 pattern. The enum is already `register_type`'d at `mod.rs:157`; `Reflect` picks up the new variants automatically — no registration change needed.

**Why:** Both are per-entity recognized-but-unenforced conditions (SIZE zeroing is enforced + worth telling the author; content-vis is recognized but deferred). The existing per-session dedup resource is the right mechanism — spec § 6 canonical, and consistent with the Phase-7 `Sticky*`/`Table*` keys. Per-entity (not session-wide) because each entity's condition is independently actionable.

**How to apply:** Append the two variants with doc comments citing spec § 5.1 / § 5.2 + D5 / D6. Dedup idiom: `if warned.set.insert(LayoutWarnOnceKey::SizeContainmentZeroed(e)) { warn!(…) }`.

**Runner-up rejected:** Reuse a generic `ContainmentUnsupported(Entity)` for both. Rejected: the two conditions have different meanings (one enforced, one deferred) and different messages; collapsing them loses the actionable distinction in logs.

---

## Tasks

> **Per-task workflow (subagent-driven):**
> 1. Implementer subagent reads the task block.
> 2. Implementer follows TDD: failing test first, then minimal impl to pass, then refactor if needed, then commit.
> 3. Spec-compliance reviewer subagent reads the spec sections + the diff and asserts coverage.
> 4. Code-quality reviewer subagent reads the diff and asserts the code-quality bar.
> 5. Both reviews must be ✅ before moving to the next task.

> **Project gate (run before every commit, exactly):**
> ```sh
> cargo fmt --all -- --check && \
>   cargo clippy --workspace --all-targets -- -D warnings && \
>   RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
>   xvfb-run -a cargo test --workspace
> ```

### Task 1: Plan doc lands + docs/README.md entry

**Files:**
- Create: `docs/plans/2026-05-28-buiy-layout-transforms-containment.md` (this file)
- Modify: `docs/README.md` (Phase 8 entry under "### Layout" → "**Plans**")

- [ ] **Step 1: This plan doc is drafted.** Confirm it covers (a) transforms + containment scope, (b) decision blocks D1–D8, (c) tasks T1–T14 with TDD steps, (d) prior-art citations + integration surface.
- [ ] **Step 2: Add docs/README.md entry.** Under "### Layout" → "**Plans**", append:
  ```markdown
  - [Buiy layout transforms + containment](plans/2026-05-28-buiy-layout-transforms-containment.md) — Phase 8: `UiTransform` + `Translate`/`Rotate`/`Scale` longhands + `compose_transform` (M = T·R·S·M_transform), `transform_composition` sub-pass 6e writing the private `ResolvedTransform` render handoff, `Containment` (contain flags / content-visibility / will-change) with SIZE-containment zeroing. Stacking/top-layer deferred to Phase 9 (6f reads the matrix). `[active]`
  ```
- [ ] **Step 3: Commit.**
  ```bash
  git add docs/plans/2026-05-28-buiy-layout-transforms-containment.md docs/README.md
  git commit -m "docs(layout): Phase 8 plan — transforms + containment"
  ```

### Task 2: Transform value types in `types.rs`

**Spec:** § 1 (`TransformMatrix`, `TransformOrigin`, `TransformStyle`, `BackfaceVisibility`).

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (add 4 transform value types + tests)

- [ ] **Step 1: Failing test.** Add to `types.rs::mod tests`:
  ```rust
  #[test]
  fn transform_matrix_default_is_none() {
      assert_eq!(TransformMatrix::default(), TransformMatrix::None);
  }

  #[test]
  fn transform_origin_default_is_50_50_0() {
      let o = TransformOrigin::default();
      assert_eq!(o.x, Length::Percent(50.0));
      assert_eq!(o.y, Length::Percent(50.0));
      assert_eq!(o.z, Length::ZERO);
  }

  #[test]
  fn transform_style_and_backface_defaults() {
      assert_eq!(TransformStyle::default(), TransformStyle::Flat);
      assert_eq!(BackfaceVisibility::default(), BackfaceVisibility::Visible);
  }
  ```
  Run: `cargo test -p buiy_core transform_matrix_default transform_origin_default transform_style_and_backface` — expected FAIL (types don't exist).

- [ ] **Step 2: Add the types to `types.rs`.** After the existing layout value types (near `AnchorErrorKind` / `LayoutWarnOnceKey`), insert. Use the spec § 1 shapes EXACTLY (`TransformMatrix::None` is `#[default]`; `TransformOrigin` `Default` is hand-written 50%/50%/0):
  ```rust
  // ============================================================
  // Phase 8 — transform value types (transforms-and-containment.md § 1)
  // ============================================================

  /// The transform matrix variant for `UiTransform`. `None` is identity.
  /// `Compose([A, B, …])` is the matrix product `A · B · …` (outermost
  /// first); the rightmost/innermost entry transforms a child point
  /// first. See [`UiTransform`] composition convention.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.
  #[derive(Reflect, Clone, Default, PartialEq, Debug)]
  pub enum TransformMatrix {
      #[default]
      None,                                  // identity
      Translate(Length, Length, Length),     // 3D translate
      Rotate(Quat),                          // arbitrary 3D rotation
      Scale(f32, f32, f32),
      Skew(f32, f32),                        // x, y in radians
      Matrix(Mat4),                          // explicit 4×4
      Compose(Vec<TransformMatrix>),         // matrix product A · B · …
  }

  /// CSS `transform-origin`. Default is `50% 50% 0` (hand-written —
  /// `#[derive(Default)]` would give all-zero `Length`s, which is wrong).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.
  #[derive(Reflect, Clone, Copy, PartialEq, Debug)]
  pub struct TransformOrigin {
      pub x: Length,
      pub y: Length,
      pub z: Length,
  }

  impl Default for TransformOrigin {
      fn default() -> Self {
          Self {
              x: Length::Percent(50.0),
              y: Length::Percent(50.0),
              z: Length::ZERO,
          }
      }
  }

  /// CSS `transform-style`. Render-side concern; layout stores.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 4.
  #[derive(Reflect, Clone, Copy, Default, PartialEq, Eq, Debug)]
  pub enum TransformStyle {
      #[default]
      Flat,
      Preserve3d,
  }

  /// CSS `backface-visibility`. Render-side concern; layout stores.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 4.
  #[derive(Reflect, Clone, Copy, Default, PartialEq, Eq, Debug)]
  pub enum BackfaceVisibility {
      #[default]
      Visible,
      Hidden,
  }
  ```
  **Implementer note:** `Quat`, `Mat4`, `Vec` come from the glob `use bevy::prelude::*` / `use bevy::math::*` — confirm `types.rs` imports them; if not, add `use bevy::math::{Mat4, Quat};`. `Mat4` derives `Reflect` in Bevy 0.18 (it is `#[reflect]`-registered in `bevy_math`); `Quat` likewise. The spec's struct/enum derives in § 1 omit `Debug`; this plan adds `Debug` (consistent with the rest of `types.rs` and needed for `assert_eq!` in tests) — note that `TransformMatrix::Matrix(Mat4)` and `Compose(Vec<_>)` are `Debug` via `Mat4`/`Vec`'s `Debug`. `PartialEq` on `TransformMatrix` requires `Mat4: PartialEq` (yes, exact float eq) — acceptable for the identity comparison in 6e since identity is exact.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core transform_matrix_default transform_origin_default transform_style_and_backface
  ```
  Expected PASS.

- [ ] **Step 4: Project gate.** (Registration of these types happens in T13; here just confirm compile/tests/doc are green for the added types.)
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  ```

- [ ] **Step 5: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/types.rs
  git commit -m "feat(layout): transform value types (Phase 8 — spec § 1)

Adds TransformMatrix (None default), TransformOrigin (hand-written 50%/50%/0
Default), TransformStyle (Flat default), BackfaceVisibility (Visible default)
per transforms-and-containment.md § 1. register_type wiring lands in T13."
  ```

### Task 3: `UiTransform` component in `components.rs`

**Spec:** § 1.

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` (add `UiTransform` component + imports)

- [ ] **Step 1: Failing test.** Add to `components.rs::mod tests`:
  ```rust
  #[test]
  fn ui_transform_default_is_identity() {
      let t = UiTransform::default();
      assert_eq!(t.matrix, TransformMatrix::None);
      assert_eq!(t.origin, TransformOrigin::default());
      assert_eq!(t.style, TransformStyle::Flat);
      assert!(t.perspective.is_none());
      assert_eq!(t.backface_visibility, BackfaceVisibility::Visible);
  }
  ```
  Run: `cargo test -p buiy_core ui_transform_default_is_identity` — expected FAIL.

- [ ] **Step 2: Add `UiTransform` to `components.rs`.** After the `Container` component (or near the Phase-7 `MultiColumn`), add — note the name is `UiTransform`, NOT `Transform`, because `bevy::prelude::Transform` is glob-imported (spec § 1):
  ```rust
  /// Visual transform for an entity's box. Named `UiTransform` (not
  /// `Transform`) to avoid colliding with the glob-imported
  /// `bevy::prelude::Transform`. Does NOT affect Taffy layout (spec
  /// § 1.2) — a transformed element occupies its un-transformed box and
  /// siblings ignore the transform. Composed (with the `Translate` /
  /// `Rotate` / `Scale` longhands) by sub-pass 6e `transform_composition`
  /// into the private `ResolvedTransform` render handoff.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.
  #[derive(Component, Reflect, Clone, Default, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub struct UiTransform {
      pub matrix: TransformMatrix,
      pub origin: TransformOrigin,
      pub style: TransformStyle,
      pub perspective: Option<Length>,
      pub backface_visibility: BackfaceVisibility,
  }
  ```
  Add the imports to the `use super::types::{ … }` block at `components.rs:13`:
  ```rust
  use super::types::{
      // ... existing imports ...
      BackfaceVisibility, TransformMatrix, TransformOrigin, TransformStyle,
  };
  ```
  **Implementer note:** the spec § 1 derive set for `UiTransform` is `#[derive(Component, Reflect, Clone, Default)]`. This plan adds `PartialEq, Debug` for testability + the identity-comparison need in 6e (`UiTransform`'s identity check is via the composed `Mat4`, but a `PartialEq` on the component is still convenient). `Length` is already imported in `components.rs`.

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core ui_transform_default_is_identity
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  git add crates/buiy_core/src/layout/components.rs
  git commit -m "feat(layout): UiTransform component (Phase 8 — spec § 1)

Self-styling visual transform. Named UiTransform to avoid the glob-imported
bevy::prelude::Transform collision (spec § 1). Default is identity. Style-field
integration lands in T7; register_type in T13."
  ```

### Task 4: Longhand components `Translate` / `Rotate` / `Scale`

**Spec:** § 1.1 (decomposed-only; `Scale` hand-written `Default` = `1,1,1`).

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` (add 3 longhand tuple-struct components)

- [ ] **Step 1: Failing test.** Add to `components.rs::mod tests`:
  ```rust
  #[test]
  fn translate_default_is_zero() {
      let t = Translate::default();
      assert_eq!(t.0, Length::ZERO);
      assert_eq!(t.1, Length::ZERO);
      assert_eq!(t.2, Length::ZERO);
  }

  #[test]
  fn rotate_default_is_identity_quat() {
      assert_eq!(Rotate::default().0, Quat::IDENTITY);
  }

  #[test]
  fn scale_default_is_one_one_one() {
      let s = Scale::default();
      assert_eq!(s.0, 1.0);
      assert_eq!(s.1, 1.0);
      assert_eq!(s.2, 1.0);
  }
  ```
  Run: `cargo test -p buiy_core translate_default_is_zero rotate_default_is_identity_quat scale_default_is_one_one_one` — expected FAIL.

- [ ] **Step 2: Add the longhands to `components.rs`.** After `UiTransform`. Use the spec § 1.1 shapes EXACTLY — `Scale` has a hand-written `Default` of `(1, 1, 1)`:
  ```rust
  /// CSS `translate` longhand. **Decomposed-only** — spawn alongside
  /// `Style` (not a `Style` field), composed with `UiTransform.matrix`
  /// by sub-pass 6e per `M = T·R·S·M_transform`.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.1.
  #[derive(Component, Reflect, Clone, Default, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub struct Translate(pub Length, pub Length, pub Length);

  /// CSS `rotate` longhand. **Decomposed-only.** Default is the identity
  /// quaternion.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.1.
  #[derive(Component, Reflect, Clone, Default, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub struct Rotate(pub Quat);

  /// CSS `scale` longhand. **Decomposed-only.** CSS default scale is
  /// identity `(1, 1, 1)`, not derived zeros — hand-written `Default`.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.1.
  #[derive(Component, Reflect, Clone, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub struct Scale(pub f32, pub f32, pub f32);

  impl Default for Scale {
      fn default() -> Self {
          Scale(1.0, 1.0, 1.0)
      }
  }
  ```
  **Implementer note:** `Quat` is `bevy::math::Quat` (in the prelude glob `components.rs:22` `use bevy::prelude::*`). `Quat::IDENTITY` is the default `Quat`. `#[reflect(Component, Default)]` on `Scale` requires the `Default` impl to exist (it does, hand-written) — the reflect macro calls `Scale::default()` not the derive. The spec § 1.1 derive set omits `PartialEq, Debug`; this plan adds them for testability.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core translate_default_is_zero rotate_default_is_identity_quat scale_default_is_one_one_one
  ```
  Expected PASS (note `scale_default_is_one_one_one` PASSES because of the hand-written `Default`).

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  git add crates/buiy_core/src/layout/components.rs
  git commit -m "feat(layout): Translate/Rotate/Scale longhands (Phase 8 — spec § 1.1)

Decomposed-only components (spawned alongside Style, not bundle fields), mirroring
the FlexItem/Anchor precedent. Scale has a hand-written Default of (1,1,1) per CSS.
register_type wiring in T13."
  ```

### Task 5: `ResolvedTransform` component + `compose_transform` helper

**Spec:** § 1 (composition convention), § 1.1 (longhand composition), D2 (`ResolvedTransform` over Bevy `Transform`; `Mat4`).

**Files:**
- Modify: `crates/buiy_core/src/components.rs` (add `ResolvedTransform` next to `ResolvedLayout`)
- Modify: `crates/buiy_core/src/layout/systems.rs` (add the pure `compose_transform` helper + unit tests)

- [ ] **Step 1: Failing test for `ResolvedTransform`.** Add to `components.rs::mod tests` (or create the test module if absent):
  ```rust
  #[test]
  fn resolved_transform_default_is_identity() {
      assert_eq!(ResolvedTransform::default().matrix, Mat4::IDENTITY);
  }
  ```
  Run: `cargo test -p buiy_core resolved_transform_default_is_identity` — expected FAIL.

- [ ] **Step 2: Add `ResolvedTransform` to `components.rs`.** After `ResolvedLayout` (`components.rs:22`), same derive shape + a `Default` of identity:
  ```rust
  /// Resolved composed transform, written by sub-pass 6e
  /// (`transform_composition`) when an entity has a non-identity
  /// `UiTransform` / `Translate` / `Rotate` / `Scale`. The render
  /// handoff for transforms — mirrors how `ResolvedLayout` is the
  /// render handoff for position+size. Absent on entities with an
  /// identity transform (sub-pass 6e inserts it only when non-identity
  /// and removes a stale one otherwise — spec § 7).
  ///
  /// **Not** written into a Bevy `Transform`/`GlobalTransform` in
  /// Phase 8 (deliberate divergence from spec § 2 approach (a): render
  /// reads `ResolvedLayout` directly and `buiy_core` has no
  /// `TransformPlugin` wiring — the Bevy-`Transform` ownership bridge
  /// is a render-pipeline follow-up). Stored as `Mat4` (3D-ready,
  /// represents perspective + arbitrary 4×4).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 1.1, § 2.
  #[derive(Component, Reflect, Clone, Debug, PartialEq)]
  #[reflect(Component)]
  pub struct ResolvedTransform {
      /// The composed transform matrix `M = T·R·S·M_transform`. A child
      /// point `p` is transformed as `M · p`.
      pub matrix: Mat4,
  }

  impl Default for ResolvedTransform {
      fn default() -> Self {
          Self {
              matrix: Mat4::IDENTITY,
          }
      }
  }
  ```
  **Implementer note:** `Mat4` is in the prelude glob already used in `components.rs`; confirm and add `use bevy::math::Mat4;` if the test module needs it explicitly. `Mat4: PartialEq` (exact float eq) — fine for the identity gate. `Mat4` is `Reflect` in Bevy 0.18.

- [ ] **Step 3: Run the `ResolvedTransform` test.**
  ```bash
  cargo test -p buiy_core resolved_transform_default_is_identity
  ```
  Expected PASS.

- [ ] **Step 4: Failing tests for `compose_transform`.** Add to `systems.rs::mod tests`:
  ```rust
  #[test]
  fn compose_identity_is_identity() {
      let ui = UiTransform::default();
      let m = compose_transform(&ui, None, None, None);
      assert_eq!(m, Mat4::IDENTITY);
  }

  #[test]
  fn compose_matrix_translate_only() {
      let ui = UiTransform {
          matrix: TransformMatrix::Translate(Length::px(10.0), Length::px(20.0), Length::ZERO),
          ..Default::default()
      };
      let m = compose_transform(&ui, None, None, None);
      assert_eq!(m, Mat4::from_translation(Vec3::new(10.0, 20.0, 0.0)));
  }

  #[test]
  fn compose_matrix_scale_only() {
      let ui = UiTransform {
          matrix: TransformMatrix::Scale(2.0, 3.0, 1.0),
          ..Default::default()
      };
      let m = compose_transform(&ui, None, None, None);
      assert_eq!(m, Mat4::from_scale(Vec3::new(2.0, 3.0, 1.0)));
  }

  #[test]
  fn compose_longhands_with_matrix_order() {
      // Longhand translate (10,0,0), longhand scale (2,2,1), matrix Rotate(z 90°).
      // M = T_translate · R_rotate · S_scale · M_transform
      //   = T(10) · R_longhand_identity? — NOTE: Rotate longhand absent, so R = IDENTITY.
      // With t = Translate(10,0,0), r = None, s = Scale(2,2,1), matrix = Rotate(z, FRAC_PI_2):
      //   M = T(10,0,0) · IDENTITY · S(2,2,1) · Rz(90°)
      let ui = UiTransform {
          matrix: TransformMatrix::Rotate(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
          ..Default::default()
      };
      let t = Translate(Length::px(10.0), Length::ZERO, Length::ZERO);
      let s = Scale(2.0, 2.0, 1.0);
      let m = compose_transform(&ui, Some(&t), None, Some(&s));
      let expected = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0))
          * Mat4::from_scale(Vec3::new(2.0, 2.0, 1.0))
          * Mat4::from_quat(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2));
      assert_eq!(m, expected);
  }

  #[test]
  fn compose_matrix_compose_product_order() {
      // Compose([A, B]) = A · B (A outermost, B transforms a child point first).
      let a = TransformMatrix::Translate(Length::px(5.0), Length::ZERO, Length::ZERO);
      let b = TransformMatrix::Scale(2.0, 1.0, 1.0);
      let ui = UiTransform {
          matrix: TransformMatrix::Compose(vec![a, b]),
          ..Default::default()
      };
      let m = compose_transform(&ui, None, None, None);
      let expected = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0))
          * Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0));
      assert_eq!(m, expected);
  }
  ```
  Run: `cargo test -p buiy_core compose_` — expected FAIL (`compose_transform` doesn't exist).

- [ ] **Step 5: Implement `compose_transform` + `transform_matrix_to_mat4` in `systems.rs`.** Add as `pub(super) fn` (so unit tests in the module + the 6e system can call it):
  ```rust
  /// Convert a `TransformMatrix` to a `Mat4`. `None` → identity.
  /// `Translate`/`Rotate`/`Scale`/`Skew`/`Matrix` map directly;
  /// `Compose([A, B, …])` folds to the matrix product `A · B · …`
  /// (outermost first; rightmost transforms a child point first).
  ///
  /// `Length`s in `Translate` resolve as px today (percent/cq transform
  /// translates resolve against the entity's own box — deferred to the
  /// render/animation phase; px is the only meaningful unit at compose
  /// time for Phase 8). Non-px `Length` variants resolve to their px
  /// magnitude via `Length::Px` only; other variants contribute 0.0.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.
  fn transform_matrix_to_mat4(m: &TransformMatrix) -> Mat4 {
      match m {
          TransformMatrix::None => Mat4::IDENTITY,
          TransformMatrix::Translate(x, y, z) => {
              Mat4::from_translation(Vec3::new(length_px(x), length_px(y), length_px(z)))
          }
          TransformMatrix::Rotate(q) => Mat4::from_quat(*q),
          TransformMatrix::Scale(x, y, z) => Mat4::from_scale(Vec3::new(*x, *y, *z)),
          TransformMatrix::Skew(ax, ay) => {
              // 2D skew: shear matrix with tan(angle) off-diagonals.
              let mut mat = Mat4::IDENTITY;
              mat.y_axis.x = ax.tan();
              mat.x_axis.y = ay.tan();
              mat
          }
          TransformMatrix::Matrix(mat) => *mat,
          TransformMatrix::Compose(list) => list
              .iter()
              .fold(Mat4::IDENTITY, |acc, item| acc * transform_matrix_to_mat4(item)),
      }
  }

  /// Resolve a `Length` to px for transform translation. Only `Px` is
  /// meaningful at compose time in Phase 8; other units (percent /
  /// cq) resolve against the entity's own box and are deferred to the
  /// render/animation phase — they contribute 0.0 here.
  fn length_px(l: &Length) -> f32 {
      match l {
          Length::Px(p) => *p,
          _ => 0.0,
      }
  }

  /// Compose the final transform matrix per spec § 1:
  /// `M = T_translate · R_rotate · S_scale · M_transform`.
  /// The longhand `Translate`/`Rotate`/`Scale` (absent → identity
  /// contribution) are the outer factors; `UiTransform.matrix` is the
  /// innermost. A child point `p` is transformed as `M · p`, so it
  /// feels the rightmost (innermost) factor first.
  ///
  /// Pure function — no Bevy queries, no Taffy reads. Easy to unit test.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 1.1.
  pub(super) fn compose_transform(
      ui: &UiTransform,
      t: Option<&Translate>,
      r: Option<&Rotate>,
      s: Option<&Scale>,
  ) -> Mat4 {
      let t_mat = match t {
          Some(Translate(x, y, z)) => {
              Mat4::from_translation(Vec3::new(length_px(x), length_px(y), length_px(z)))
          }
          None => Mat4::IDENTITY,
      };
      let r_mat = match r {
          Some(Rotate(q)) => Mat4::from_quat(*q),
          None => Mat4::IDENTITY,
      };
      let s_mat = match s {
          Some(Scale(x, y, z)) => Mat4::from_scale(Vec3::new(*x, *y, *z)),
          None => Mat4::IDENTITY,
      };
      let m_transform = transform_matrix_to_mat4(&ui.matrix);
      t_mat * r_mat * s_mat * m_transform
  }
  ```
  Add the imports at the top of `systems.rs` (`use super::components::{ … }` and `use super::types::{ … }`):
  ```rust
  use super::components::{
      // ... existing ...
      Rotate, Scale, Translate, UiTransform,
  };
  use super::types::{
      // ... existing ...
      TransformMatrix,
  };
  use crate::components::{Node, ResolvedLayout, ResolvedTransform};
  ```
  And ensure `Mat4`, `Vec3` are reachable (from `use bevy::prelude::*` at `systems.rs:29` — `Vec3`/`Mat4` are in the prelude glob; add `use bevy::math::{Mat4, Vec3};` if not).

- [ ] **Step 6: Run the `compose_transform` tests.**
  ```bash
  cargo test -p buiy_core compose_ resolved_transform_default
  ```
  Expected PASS (5 compose tests + 1 default test).

- [ ] **Step 7: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  git add crates/buiy_core/src/components.rs crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): ResolvedTransform + compose_transform helper (Phase 8 — D2, spec § 1)

ResolvedTransform { matrix: Mat4 } is the private render handoff for transforms
(mirrors ResolvedLayout; NOT a Bevy Transform write — see D2). compose_transform
implements M = T·R·S·M_transform with TransformMatrix→Mat4 (None=IDENTITY,
Translate/Rotate/Scale/Skew/Matrix direct, Compose=fold product A·B·…).
register_type in T13; the 6e system that calls it lands in T6."
  ```

### Task 6: `transform_composition` sub-pass 6e + wire into the chain

**Spec:** § 1.1 (sub-pass 6e), D3 (after 6d; identity→no-component; Display::None skip).

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `transform_composition` system)
- Modify: `crates/buiy_core/src/layout/mod.rs:180-188` (append to the `PostTaffyOverrides` chain)
- Modify: `crates/buiy_core/tests/layout_pipeline_order.rs` (assert 6e runs)

- [ ] **Step 1: Add the `transform_composition` system to `systems.rs`.**
  ```rust
  /// Phase 8 — sub-pass 6e of `BuiyLayoutStep::PostTaffyOverrides`.
  /// Composes each entity's `UiTransform` + optional `Translate` /
  /// `Rotate` / `Scale` longhands into the private `ResolvedTransform`
  /// render handoff per spec § 1 (`M = T·R·S·M_transform`).
  ///
  /// Runs AFTER `anchor_resolution` (6d). Unlike 6a–6d, writes NOTHING
  /// to `PostTaffyPositionOverrides` — a transform does not move the
  /// layout box (spec § 1.2). For identity transforms it inserts no
  /// `ResolvedTransform` and removes a stale one (spec § 7). Skips
  /// `Display::None` entities.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.1.
  pub(super) fn transform_composition(
      mut commands: Commands,
      query: Query<
          (
              Entity,
              &UiTransform,
              Option<&Translate>,
              Option<&Rotate>,
              Option<&Scale>,
              &Display,
              Option<&ResolvedTransform>,
          ),
          With<Node>,
      >,
  ) {
      for (e, ui, t, r, s, display, existing) in query.iter() {
          if matches!(display, Display::None) {
              continue;
          }
          let m = compose_transform(ui, t, r, s);
          if m == Mat4::IDENTITY {
              // Identity → no ResolvedTransform; remove a stale one.
              if existing.is_some() {
                  commands.entity(e).remove::<ResolvedTransform>();
              }
              continue;
          }
          // Idempotent insert (mirror write_resolved_layout's gate).
          if existing.map(|rt| rt.matrix) != Some(m) {
              commands.entity(e).insert(ResolvedTransform { matrix: m });
          }
      }
  }
  ```
  **Implementer note:** `Display` is already imported in `systems.rs`. `Commands` from the prelude. The query uses `Option<&ResolvedTransform>` to both gate the idempotent insert and decide whether a `remove` is needed (avoids issuing a `remove` on entities that never had one).

- [ ] **Step 2: Wire into the `PostTaffyOverrides` chain in `mod.rs:180-188`.** Append `systems::transform_composition` after `systems::anchor_resolution`, and extend the comment to mention 6e:
  ```rust
                  // Phase 7 — PostTaffyOverrides chain: clear → sticky 6a →
                  // table 6b → multicol 6c → anchor 6d. Phase 8 appends
                  // transform_composition 6e AFTER anchor (spec § 1.1) —
                  // it composes the matrix and writes ResolvedTransform; it
                  // does NOT write PostTaffyPositionOverrides (a transform
                  // does not move the layout box, spec § 1.2). Phase 9 will
                  // append stacking 6f after 6e (it reads the composed
                  // matrix).
                  (
                      systems::clear_post_taffy_overrides,
                      systems::sticky_offset,
                      systems::table_layout,
                      systems::multicol_pack,
                      systems::anchor_resolution,
                      systems::transform_composition,
                  )
                      .chain()
                      .in_set(BuiyLayoutStep::PostTaffyOverrides),
  ```

- [ ] **Step 3: Failing pipeline-order assertion.** In `crates/buiy_core/tests/layout_pipeline_order.rs`, add (and extend the `use` import to include `UiTransform`, `TransformMatrix`, `ResolvedTransform`):
  ```rust
  #[test]
  fn transform_composition_runs_and_writes_resolved_transform() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(LayoutPlugin);

      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default(),
              UiTransform {
                  matrix: TransformMatrix::Translate(Length::px(10.0), Length::px(0.0), Length::ZERO),
                  ..Default::default()
              },
          ))
          .id();

      app.update();

      let rt = app
          .world()
          .get::<ResolvedTransform>(e)
          .expect("6e should write ResolvedTransform for a non-identity UiTransform");
      assert_eq!(rt.matrix, Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)));
  }

  #[test]
  fn identity_transform_gets_no_resolved_transform() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(LayoutPlugin);

      let e = app
          .world_mut()
          .spawn((Node, Style::default(), UiTransform::default()))
          .id();

      app.update();

      assert!(
          app.world().get::<ResolvedTransform>(e).is_none(),
          "identity transform must not produce a ResolvedTransform (spec § 7)"
      );
  }
  ```
  Run: `cargo test -p buiy_core --test layout_pipeline_order transform_composition_runs identity_transform_gets_no` — expected FAIL until Steps 1+2 compile and `ResolvedTransform`/`UiTransform` are re-exported (re-exports land in T13; for this task the test can import via `buiy_core::layout::{UiTransform, TransformMatrix}` and `buiy_core::ResolvedTransform` once T13 lands — **sequencing note:** run this test after T13, or add a temporary `pub use` locally and finalize in T13. Prefer: implement Steps 1+2 here, add the test, and let it go green after T13 wires the re-exports; if blocked, the implementer may pull the minimal re-exports for `UiTransform`/`TransformMatrix`/`ResolvedTransform` forward into this task and T13 confirms the full set).

- [ ] **Step 4: Run the tests.**
  ```bash
  cargo test -p buiy_core --test layout_pipeline_order
  ```
  Expected PASS (existing order test + the 2 new transform tests).

- [ ] **Step 5: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/mod.rs crates/buiy_core/tests/layout_pipeline_order.rs
  git commit -m "feat(layout): transform_composition sub-pass 6e (Phase 8 — spec § 1.1, D3)

Composes UiTransform + Translate/Rotate/Scale into ResolvedTransform; wired
into the PostTaffyOverrides chain AFTER anchor_resolution (6d). Writes nothing
to PostTaffyPositionOverrides (a transform does not move the box). Identity →
no ResolvedTransform (+removes stale); skips Display::None. Pipeline-order test
asserts 6e runs."
  ```

### Task 7: `Style` integration for `UiTransform` (+ ergonomic setters)

**Spec:** D4 (`UiTransform` is a `Style` field; longhands stay decomposed-only).

**Files:**
- Modify: `crates/buiy_core/src/layout/style.rs` (add `ui_transform: UiTransform` field + fluent setters)

- [ ] **Step 1: Failing test.** Add to `style.rs::mod tests`:
  ```rust
  #[test]
  fn style_default_spawns_ui_transform() {
      let mut world = World::new();
      let e = world.spawn(Style::default()).id();
      assert!(
          world.get::<UiTransform>(e).is_some(),
          "Style derives Bundle; ui_transform inserts unconditionally (matches Container)"
      );
      assert_eq!(world.get::<UiTransform>(e).unwrap().matrix, TransformMatrix::None);
  }

  #[test]
  fn style_translate_px_setter_round_trips() {
      let s = Style::default().translate_px(10.0, 20.0);
      assert_eq!(
          s.ui_transform.matrix,
          TransformMatrix::Translate(Length::px(10.0), Length::px(20.0), Length::ZERO)
      );
  }
  ```
  Run: `cargo test -p buiy_core style_default_spawns_ui_transform style_translate_px_setter` — expected FAIL.

- [ ] **Step 2: Add the `ui_transform` field to the `Style` bundle (`style.rs:45`).**
  ```rust
  #[derive(Bundle, Clone, Debug, Default)]
  pub struct Style {
      pub display: Display,
      pub box_model: BoxModel,
      pub position: Position,
      pub flex_params: FlexParams,
      pub overflow: Overflow,
      pub scroll: Scroll,
      pub grid_params: GridParams,
      pub writing_mode: WritingMode,
      pub container: Container,
      pub multi_column: MultiColumn,
      pub ui_transform: UiTransform,  // NEW (Phase 8)
  }
  ```

- [ ] **Step 3: Add fluent setters.** Near the `container` / `multi_column` setters (`style.rs:445-460`):
  ```rust
  // ---- UiTransform ----

  /// Set the full `UiTransform` for this entity (self-styling visual
  /// transform). Does not affect layout flow (spec § 1.2).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.
  pub fn ui_transform(mut self, t: UiTransform) -> Self {
      self.ui_transform = t;
      self
  }

  /// Ergonomic setter — 2D translate in logical pixels (z = 0).
  pub fn translate_px(mut self, x: f32, y: f32) -> Self {
      self.ui_transform.matrix =
          TransformMatrix::Translate(Length::px(x), Length::px(y), Length::ZERO);
      self
  }

  /// Ergonomic setter — rotate about the z axis (radians).
  pub fn rotate_z(mut self, radians: f32) -> Self {
      self.ui_transform.matrix = TransformMatrix::Rotate(Quat::from_rotation_z(radians));
      self
  }

  /// Ergonomic setter — uniform 2D scale (z = 1).
  pub fn scale(mut self, factor: f32) -> Self {
      self.ui_transform.matrix = TransformMatrix::Scale(factor, factor, 1.0);
      self
  }
  ```
  Add imports to `style.rs`:
  ```rust
  use crate::layout::components::{
      // ... existing ...
      UiTransform,
  };
  use crate::layout::types::TransformMatrix;
  // Quat / Length already reachable (Length is imported; Quat via bevy prelude or add use bevy::math::Quat;).
  ```

- [ ] **Step 4: Document the decomposed-only longhands.** Add a doc comment near the setters noting that `Translate`/`Rotate`/`Scale` longhands are **not** `Style` fields — spawn them alongside `Style` (`commands.spawn((Style::default(), Translate(…)))`) — mirroring the `FlexItem`/`Anchor` decomposed-only convention (D4). No setter for the longhands.

- [ ] **Step 5: Add a non-identity round-trip integration test.** Add to `style.rs::mod tests` (or a `tests/` file — keep in `style.rs` for the unit-level round-trip; the full-pipeline assertion lives in T12):
  ```rust
  #[test]
  fn style_non_identity_ui_transform_field_inserts() {
      let mut world = World::new();
      let e = world
          .spawn(Style::default().rotate_z(std::f32::consts::FRAC_PI_4))
          .id();
      let ui = world.get::<UiTransform>(e).expect("ui_transform inserted");
      assert!(matches!(ui.matrix, TransformMatrix::Rotate(_)));
  }
  ```
  (The "non-identity ui_transform yields a ResolvedTransform after update" full-pipeline assertion is covered by `transform_composition_runs_and_writes_resolved_transform` in T6 and the T12 integration tests.)

- [ ] **Step 6: Run the tests.**
  ```bash
  cargo test -p buiy_core style_default_spawns_ui_transform style_translate_px style_non_identity_ui_transform
  ```
  Expected PASS.

- [ ] **Step 7: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  git add crates/buiy_core/src/layout/style.rs
  git commit -m "feat(layout): Style.ui_transform field + ergonomic setters (Phase 8 — D4)

UiTransform is a self-styling Style field (always inserted via derived Bundle,
matches Container). Adds .ui_transform()/.translate_px()/.rotate_z()/.scale()
setters. Translate/Rotate/Scale longhands stay decomposed-only (documented)."
  ```

### Task 8: Containment value types (`ContainFlags`, `ContentVisibility`, `WillChange`, `WillChangeProperty`)

**Spec:** § 5 (bitflags with `CONTENT`/`STRICT` bit-unions + `impl_reflect_opaque!`; `ContentVisibility`; `WillChange`; `WillChangeProperty`).

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (add the containment value types)
- Possibly modify: `crates/buiy_core/Cargo.toml` (add `bitflags` if not a direct dep)

- [ ] **Step 1: Verify `bitflags` availability.**
  ```bash
  grep -n "bitflags" crates/buiy_core/Cargo.toml
  cargo tree -p buiy_core -i bitflags 2>/dev/null | head
  ```
  If `bitflags` is NOT a direct dependency of `buiy_core`, add it:
  ```bash
  cargo add bitflags -p buiy_core
  cargo deny check
  ```
  Expected: either it's already direct, or `cargo add` + `cargo deny check` succeeds (no advisory / license violation).

- [ ] **Step 2: Failing tests.** Add to `types.rs::mod tests`:
  ```rust
  #[test]
  fn contain_content_includes_paint_layout_style() {
      assert!(ContainFlags::CONTENT.contains(ContainFlags::PAINT));
      assert!(ContainFlags::CONTENT.contains(ContainFlags::LAYOUT));
      assert!(ContainFlags::CONTENT.contains(ContainFlags::STYLE));
      assert!(!ContainFlags::CONTENT.contains(ContainFlags::SIZE));
  }

  #[test]
  fn contain_strict_includes_size() {
      assert!(ContainFlags::STRICT.contains(ContainFlags::SIZE));
      assert!(ContainFlags::STRICT.contains(ContainFlags::PAINT));
      assert!(ContainFlags::STRICT.contains(ContainFlags::LAYOUT));
      assert!(ContainFlags::STRICT.contains(ContainFlags::STYLE));
  }

  #[test]
  fn contain_flags_default_is_empty() {
      assert_eq!(ContainFlags::default(), ContainFlags::empty());
  }

  #[test]
  fn content_visibility_and_will_change_defaults() {
      assert_eq!(ContentVisibility::default(), ContentVisibility::Visible);
      assert_eq!(WillChange::default(), WillChange::Auto);
  }
  ```
  Run: `cargo test -p buiy_core contain_ content_visibility_and_will_change` — expected FAIL.

- [ ] **Step 3: Add the containment value types to `types.rs`.** Use the spec § 5 shapes EXACTLY (`CONTENT`/`STRICT` are bit-UNIONS of primitive bits; `impl_reflect_opaque!` registers the bitflags):
  ```rust
  // ============================================================
  // Phase 8 — containment value types (transforms-and-containment.md § 5)
  // ============================================================

  bitflags::bitflags! {
      /// CSS `contain` flags. `CONTENT` and `STRICT` are unions of the
      /// primitive bits (not standalone bits), so `.contains(PAINT)` is
      /// true for a `CONTENT`- or `STRICT`-contained entity.
      ///
      /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.
      #[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
      pub struct ContainFlags: u8 {
          const LAYOUT      = 1 << 0;
          const PAINT       = 1 << 1;
          const SIZE        = 1 << 2;
          const STYLE       = 1 << 3;
          const INLINE_SIZE = 1 << 4;
          const CONTENT = Self::LAYOUT.bits() | Self::PAINT.bits() | Self::STYLE.bits();
          const STRICT  = Self::LAYOUT.bits()
              | Self::PAINT.bits()
              | Self::SIZE.bits()
              | Self::STYLE.bits();
      }
  }

  // `bitflags!` doesn't compose with `#[derive(Reflect)]` — register the
  // opaque type manually (`impl_reflect_value!` → `impl_reflect_opaque!`
  // in bevy_reflect 0.18).
  impl_reflect_opaque!(ContainFlags(Default, PartialEq));

  /// CSS `content-visibility`. Phase 8 stores the value; `Auto` /
  /// `Hidden` enforcement is deferred (warn-once
  /// `LayoutWarnOnceKey::ContentVisibilityDeferred`).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5, § 5.2.
  #[derive(Reflect, Clone, Copy, Default, PartialEq, Eq, Debug)]
  pub enum ContentVisibility {
      #[default]
      Visible,
      Auto,
      Hidden,
  }

  /// CSS `will-change`. Tier-E forward-compat hint; Phase 8 stores only
  /// (no layer promotion, no SC trigger — those are render / Phase 9).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.3.
  #[derive(Reflect, Clone, Default, PartialEq, Debug)]
  pub enum WillChange {
      #[default]
      Auto,
      Properties(Vec<WillChangeProperty>),
  }

  /// Properties an author hints will change (`will-change: <prop>`).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.3.
  #[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)]
  pub enum WillChangeProperty {
      Transform,
      Opacity,
      Filter,
      ZIndex,
      ScrollPosition,
  }
  ```
  Add the `impl_reflect_opaque!` import. Confirm the correct path in bevy_reflect 0.18:
  ```bash
  grep -rn "impl_reflect_opaque" ~/.cargo/registry/src/*/bevy_reflect-0.18*/src/ 2>/dev/null | head
  ```
  Then add (most likely) `use bevy::reflect::impl_reflect_opaque;` at the top of `types.rs` (or the fully-qualified `bevy::reflect::impl_reflect_opaque!` at the macro site). Verify with the project gate.

- [ ] **Step 4: Run the tests.**
  ```bash
  cargo test -p buiy_core contain_ content_visibility_and_will_change
  ```
  Expected PASS.

- [ ] **Step 5: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  git add crates/buiy_core/src/layout/types.rs crates/buiy_core/Cargo.toml Cargo.lock
  git commit -m "feat(layout): containment value types (Phase 8 — spec § 5)

ContainFlags bitflags (CONTENT/STRICT are bit-unions of the primitive bits;
registered via impl_reflect_opaque!), ContentVisibility (Visible default),
WillChange (Auto default), WillChangeProperty. Adds bitflags as a direct dep
if not already present (cargo deny clean)."
  ```

### Task 9: `Containment` component + `Style` field

**Spec:** § 5, D4 (`Containment` is a `Style` field).

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` (add `Containment` component)
- Modify: `crates/buiy_core/src/layout/style.rs` (add `containment: Containment` field + setters)

- [ ] **Step 1: Failing test.** Add to `components.rs::mod tests`:
  ```rust
  #[test]
  fn containment_default_is_empty_visible_auto() {
      let c = Containment::default();
      assert_eq!(c.contain, ContainFlags::empty());
      assert_eq!(c.content_visibility, ContentVisibility::Visible);
      assert_eq!(c.will_change, WillChange::Auto);
  }
  ```
  Run: `cargo test -p buiy_core containment_default_is_empty_visible_auto` — expected FAIL.

- [ ] **Step 2: Add the `Containment` component to `components.rs`.** Use the spec § 5 shape EXACTLY:
  ```rust
  /// CSS containment — a performance opt-in describing how this
  /// entity's subtree is isolated from the rest of the layout/paint
  /// tree. Self-styling (`Style` field). Phase 8 implements only
  /// SIZE / INLINE_SIZE containment (auto width/height → 0 with a
  /// warn-once); LAYOUT/PAINT/STYLE flags are stored for render/future;
  /// `content_visibility != Visible` is stored + deferred (warn-once);
  /// `will_change` is stored-only (tier-E).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.
  #[derive(Component, Reflect, Clone, Default, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub struct Containment {
      pub contain: ContainFlags,
      pub content_visibility: ContentVisibility,
      pub will_change: WillChange,
  }
  ```
  **Implementer note:** `#[reflect(Component, Default)]` requires the field types to be `Reflect`. `ContainFlags` is registered via `impl_reflect_opaque!` (T8); `ContentVisibility`/`WillChange` derive `Reflect`. Add imports:
  ```rust
  use super::types::{
      // ... existing ...
      ContainFlags, ContentVisibility, WillChange,
  };
  ```

- [ ] **Step 3: Add the `containment` field + setters to `Style` (`style.rs`).**
  ```rust
  // in the Style bundle struct, after ui_transform:
      pub containment: Containment,  // NEW (Phase 8)
  ```
  Setters near the `ui_transform` setters:
  ```rust
  // ---- Containment ----

  /// Set the full `Containment` for this entity.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.
  pub fn containment(mut self, c: Containment) -> Self {
      self.containment = c;
      self
  }

  /// Set just the `contain` flags (e.g. `ContainFlags::SIZE`).
  pub fn contain(mut self, flags: ContainFlags) -> Self {
      self.containment.contain = flags;
      self
  }
  ```
  Imports:
  ```rust
  use crate::layout::components::{ /* ... */ Containment };
  use crate::layout::types::ContainFlags;
  ```

- [ ] **Step 4: Style round-trip test.** Add to `style.rs::mod tests`:
  ```rust
  #[test]
  fn style_default_spawns_containment() {
      let mut world = World::new();
      let e = world.spawn(Style::default()).id();
      assert!(world.get::<Containment>(e).is_some());
  }

  #[test]
  fn style_contain_setter_round_trips() {
      let s = Style::default().contain(ContainFlags::SIZE);
      assert_eq!(s.containment.contain, ContainFlags::SIZE);
  }
  ```

- [ ] **Step 5: Run the tests.**
  ```bash
  cargo test -p buiy_core containment_default style_default_spawns_containment style_contain_setter
  ```
  Expected PASS.

- [ ] **Step 6: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  git add crates/buiy_core/src/layout/components.rs crates/buiy_core/src/layout/style.rs
  git commit -m "feat(layout): Containment component + Style field (Phase 8 — spec § 5, D4)

Self-styling containment opt-in (always inserted via derived Bundle, matches
Container). Adds .containment()/.contain() setters. Layout enforcement (SIZE
zeroing) lands in T10; content-vis/will-change handling in T11."
  ```

### Task 10: SIZE / INLINE_SIZE containment enforcement (thread `Containment` through `style_to_taffy`)

**Spec:** § 5.1 (SIZE → auto width/height treated as 0 + `warn!`), D5, D8.

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (add `SizeContainmentZeroed(Entity)` to `LayoutWarnOnceKey`)
- Modify: `crates/buiy_core/src/layout/translate.rs` (add `containment: &Containment` to `StyleView`; zero `Auto` under SIZE/INLINE_SIZE in `style_to_taffy`)
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `Containment` to `sync_styles` query + the warn-once side effect)

- [ ] **Step 1: Add the `LayoutWarnOnceKey::SizeContainmentZeroed(Entity)` variant (`types.rs:975`).** Append to the enum (after `StickyCqDeferred`):
  ```rust
      /// `Containment.contain` includes `SIZE` / `INLINE_SIZE` and the
      /// corresponding axis sizing is `Sizing::Auto`. Per spec § 5.1 the
      /// auto size is treated as `0px`. One warn per (entity, session).
      ///
      /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.1.
      SizeContainmentZeroed(Entity),
  ```

- [ ] **Step 2: Add `containment: &'a Containment` to `StyleView` (`translate.rs:217`).**
  ```rust
  pub struct StyleView<'a> {
      pub display: &'a Display,
      pub box_model: &'a BoxModel,
      // ... existing fields ...
      pub containment: &'a Containment,  // NEW (Phase 8)
      // ... nearest_container, viewport_size ...
  }
  ```
  Import `Containment` in `translate.rs`.

- [ ] **Step 3: Apply SIZE/INLINE_SIZE zeroing in `style_to_taffy` (`translate.rs:256`).** Before the width/height mapping (`translate.rs:268-269`), add a pure helper + use it:
  ```rust
  /// Under SIZE / INLINE_SIZE containment, `Sizing::Auto` on a contained
  /// axis is treated as `0px` (spec § 5.1). Pure — the warn-once side
  /// effect lives in `sync_styles` (which holds the warn resource);
  /// this just performs the deterministic substitution from the flags.
  fn apply_size_containment(sizing: Sizing, contained_axis: bool) -> Sizing {
      if contained_axis && matches!(sizing, Sizing::Auto) {
          Sizing::Length(Length::px(0.0))
      } else {
          sizing
      }
  }
  ```
  Determine per-axis containment inside `style_to_taffy`:
  ```rust
  let contain = view.containment.contain;
  let size_all = contain.contains(ContainFlags::SIZE);
  // INLINE_SIZE contains only the inline axis. Inline = width under
  // horizontal writing modes, height under vertical modes. Reuse the
  // writing-mode axis decision already used for Cqi/Cqb.
  let inline_is_horizontal = view.writing_mode_resolved.is_horizontal(); // or equivalent
  let contain_width = size_all
      || (contain.contains(ContainFlags::INLINE_SIZE) && inline_is_horizontal);
  let contain_height = size_all
      || (contain.contains(ContainFlags::INLINE_SIZE) && !inline_is_horizontal);
  ```
  Then wrap the width/height sizings:
  ```rust
      size: taffy::Size {
          width: sizing_to_dim(normalize_cq_sizing(
              apply_size_containment(view.box_model.width, contain_width), &view)),
          height: sizing_to_dim(normalize_cq_sizing(
              apply_size_containment(view.box_model.height, contain_height), &view)),
      },
  ```
  **Implementer note:** confirm the writing-mode horizontal/vertical accessor name on `WritingModeResolved` (search `is_horizontal` / `WritingModeKind` in `types.rs` / `components.rs`; the Cqi/Cqb resolution path already makes this decision — reuse it). If no accessor exists, the minimum correct Phase 8 behavior per D5 is: `INLINE_SIZE` zeroes width under horizontal modes, height under vertical modes — implement that mapping and document it. Import `ContainFlags`, `Length` in `translate.rs`.

- [ ] **Step 4: Add `Containment` to the `sync_styles` query + warn-once side effect (`systems.rs:1302`).** Add `&Containment` to the query tuple, add `Changed<Containment>` to the nested `Or<>` change filter (the nested inner `Or<>` has room — Phase 7 left it at 6/15), add `ResMut<LayoutWarnedOnceSession>` to `sync_styles`'s params (it may already be present — confirm), and pass `containment` into the `StyleView`. Then add the warn-once side effect at the per-entity translation site (`systems.rs:1534`, where the `StyleView` is built):
  ```rust
  // SIZE / INLINE_SIZE containment with auto size → treated as 0px
  // (spec § 5.1). Warn once per (entity, session). The substitution
  // itself happens in style_to_taffy (pure); this is just the log.
  let contain = containment.contain;
  let size_all = contain.contains(ContainFlags::SIZE);
  let inline_is_horizontal = writing_mode_resolved.is_horizontal();
  let zeroed_width = (size_all
      || (contain.contains(ContainFlags::INLINE_SIZE) && inline_is_horizontal))
      && matches!(box_model.width, Sizing::Auto);
  let zeroed_height = (size_all
      || (contain.contains(ContainFlags::INLINE_SIZE) && !inline_is_horizontal))
      && matches!(box_model.height, Sizing::Auto);
  if (zeroed_width || zeroed_height)
      && warned.set.insert(LayoutWarnOnceKey::SizeContainmentZeroed(entity))
  {
      bevy::log::warn!(
          "Entity {:?} has size containment (contain: size/inline-size) with an \
           auto size on a contained axis; treating the auto size as 0px (spec § 5.1). \
           Declare an explicit width/height.",
          entity,
      );
  }
  ```
  **Implementer note:** the exact variable names (`containment`, `writing_mode_resolved`, `box_model`, `entity`, `warned`) must match the `sync_styles` loop bindings — adapt to the actual destructuring. `sync_styles` writes Taffy styles and may not currently hold `ResMut<LayoutWarnedOnceSession>`; add it as a system param. Confirm the nested `Or<>` filter isn't at the 15-cap (Phase 7 self-review noted the inner nested `Or<>` was at 6/15 — there is room; if the *outer* tuple is at 15, add `Changed<Containment>` to the *inner nested* `Or<>`).

- [ ] **Step 5: Failing integration test.** Create `crates/buiy_core/tests/layout_containment.rs`:
  ```rust
  //! Phase 8 — containment layout effects.
  //!
  //! Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.

  use bevy::prelude::*;
  use buiy_core::{
      CorePlugin, Node, ResolvedLayout,
      layout::{ContainFlags, Containment, LayoutPlugin, LayoutWarnOnceKey, LayoutWarnedOnceSession, Style},
  };

  fn app() -> App {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(LayoutPlugin);
      app
  }

  #[test]
  fn size_containment_zeroes_auto_width_and_warns() {
      let mut app = app();
      // contain: size, width: auto (default) → Taffy width 0.
      let e = app
          .world_mut()
          .spawn((Node, Style::default().contain(ContainFlags::SIZE)))
          .id();
      app.update();

      let rl = app.world().get::<ResolvedLayout>(e).expect("resolved");
      assert_eq!(rl.size.x, 0.0, "size containment zeroes auto width");
      assert_eq!(rl.size.y, 0.0, "size containment zeroes auto height");

      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert!(
          warned
              .set
              .contains(&LayoutWarnOnceKey::SizeContainmentZeroed(e)),
          "size-containment-zeroed warn recorded"
      );
  }
  ```
  Run: `cargo test -p buiy_core --test layout_containment size_containment_zeroes` — expected FAIL until Steps 1-4 land (and re-exports of `ContainFlags`/`Containment` from T13 — sequencing per T6 note).

- [ ] **Step 6: Run the test.**
  ```bash
  cargo test -p buiy_core --test layout_containment
  ```
  Expected PASS.

- [ ] **Step 7: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  git add crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/translate.rs crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_containment.rs
  git commit -m "feat(layout): SIZE/INLINE_SIZE containment zeroing (Phase 8 — spec § 5.1, D5)

Threads Containment into StyleView + sync_styles. Under contain:size/inline-size,
auto width/height on a contained axis is treated as 0px (spec § 5.1) with a
warn-once LayoutWarnOnceKey::SizeContainmentZeroed(Entity). Substitution is pure
(style_to_taffy); the warn-once side effect lives in sync_styles."
  ```

### Task 11: `content-visibility` deferred stub + `will-change` stored-only

**Spec:** § 5.2 (content-visibility deferred), § 5.3 (will-change tier-E), D6, D7, D8.

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (add `ContentVisibilityDeferred(Entity)` to `LayoutWarnOnceKey`)
- Modify: `crates/buiy_core/src/layout/systems.rs` (emit the warn-once in `sync_styles`)

- [ ] **Step 1: Add the `LayoutWarnOnceKey::ContentVisibilityDeferred(Entity)` variant (`types.rs`).** Append (after `SizeContainmentZeroed`):
  ```rust
      /// `Containment.content_visibility != Visible`. Phase 8 stores the
      /// value but does NOT enforce `Auto` (off-screen skip) or `Hidden`
      /// (Display::None-for-descendants) — both deferred. One warn per
      /// (entity, session).
      ///
      /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.2.
      ContentVisibilityDeferred(Entity),
  ```

- [ ] **Step 2: Emit the content-visibility warn-once in `sync_styles`.** Colocate with the SIZE-containment warn (T10 Step 4), reading the same `containment` binding:
  ```rust
  // content-visibility != Visible is recognized but deferred in Phase 8
  // (Auto needs last-frame ResolvedLayout + viewport + contain-intrinsic-size;
  // Hidden needs a tree-skip path). Store the value; warn once per entity.
  if !matches!(containment.content_visibility, ContentVisibility::Visible)
      && warned
          .set
          .insert(LayoutWarnOnceKey::ContentVisibilityDeferred(entity))
  {
      bevy::log::warn!(
          "Entity {:?} sets content-visibility != visible; Phase 8 stores the value \
           but does not yet skip off-screen layout/paint (deferred). The value is \
           recognized and will be honored in a follow-up.",
          entity,
      );
  }
  ```
  Import `ContentVisibility` in `systems.rs`. **will-change is stored-only — NO warn (D7).** Do not add any code path for `will_change`; it is simply a stored `Containment` field.

- [ ] **Step 3: Failing tests.** Add to `crates/buiy_core/tests/layout_containment.rs`:
  ```rust
  #[test]
  fn content_visibility_auto_warns_once() {
      use buiy_core::layout::ContentVisibility;
      let mut app = app();
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default().containment(Containment {
                  content_visibility: ContentVisibility::Auto,
                  ..Default::default()
              }),
          ))
          .id();
      app.update();
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert!(warned
          .set
          .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e)));
  }

  #[test]
  fn content_visibility_hidden_also_warns() {
      use buiy_core::layout::ContentVisibility;
      let mut app = app();
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default().containment(Containment {
                  content_visibility: ContentVisibility::Hidden,
                  ..Default::default()
              }),
          ))
          .id();
      app.update();
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert!(warned
          .set
          .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e)));
  }

  #[test]
  fn will_change_does_not_warn() {
      use buiy_core::layout::{WillChange, WillChangeProperty};
      let mut app = app();
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default().containment(Containment {
                  will_change: WillChange::Properties(vec![WillChangeProperty::Transform]),
                  ..Default::default()
              }),
          ))
          .id();
      app.update();
      // will-change is a valid stored hint — no warn-once key for it.
      // (Negative assertion: no ContentVisibilityDeferred / SizeContainmentZeroed
      // fires because content_visibility = Visible and size is not contained.)
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert!(!warned
          .set
          .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e)));
      assert!(!warned
          .set
          .contains(&LayoutWarnOnceKey::SizeContainmentZeroed(e)));
  }
  ```
  Run: `cargo test -p buiy_core --test layout_containment content_visibility will_change` — expected FAIL until Steps 1-2 land.

- [ ] **Step 4: Run the tests.**
  ```bash
  cargo test -p buiy_core --test layout_containment
  ```
  Expected PASS.

- [ ] **Step 5: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  git add crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_containment.rs
  git commit -m "feat(layout): content-visibility deferred stub + will-change stored-only (Phase 8 — D6, D7)

content_visibility != Visible warns once (LayoutWarnOnceKey::ContentVisibilityDeferred);
the value is stored but Auto/Hidden enforcement is deferred (spec § 5.2). will-change
is a valid stored tier-E hint — NO warn (spec § 5.3, D7)."
  ```

### Task 12: Integration tests — transform composition + layout-flow invariance + containment

**Spec:** § 7 (test surface).

**Files:**
- Create: `crates/buiy_core/tests/layout_transforms.rs`
- Extend: `crates/buiy_core/tests/layout_containment.rs` (content-vis across 3 entities)

- [ ] **Step 1: Create `crates/buiy_core/tests/layout_transforms.rs`.**
  ```rust
  //! Phase 8 — transform composition + layout-flow invariance.
  //!
  //! Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 7.

  use bevy::prelude::*;
  use buiy_core::{
      CorePlugin, Node, ResolvedLayout, ResolvedTransform,
      layout::{
          BoxModel, Display, FlexAxis, Length, Sizing, Style, TransformMatrix, UiTransform,
      },
  };

  fn app() -> App {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(LayoutPlugin);
      app
  }
  use buiy_core::layout::LayoutPlugin;

  #[test]
  fn translate_transform_composes_to_resolved_transform() {
      let mut app = app();
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default().translate_px(15.0, 25.0),
          ))
          .id();
      app.update();
      let rt = app.world().get::<ResolvedTransform>(e).expect("non-identity → ResolvedTransform");
      assert_eq!(rt.matrix, Mat4::from_translation(Vec3::new(15.0, 25.0, 0.0)));
  }

  #[test]
  fn transform_does_not_change_own_resolved_layout_position() {
      // A transformed element occupies its un-transformed box (spec § 1.2).
      let mut app = app();
      // Build the SAME box twice: once with a transform, once without,
      // under identical parents; assert ResolvedLayout.position matches.
      let parent_plain = app
          .world_mut()
          .spawn((Node, Style::default().flex_axis(FlexAxis::Row)))
          .id();
      let child_plain = app
          .world_mut()
          .spawn((Node, Style::default()))
          .id();
      app.world_mut().entity_mut(parent_plain).add_child(child_plain);

      let parent_xf = app
          .world_mut()
          .spawn((Node, Style::default().flex_axis(FlexAxis::Row)))
          .id();
      let child_xf = app
          .world_mut()
          .spawn((Node, Style::default().translate_px(100.0, 100.0)))
          .id();
      app.world_mut().entity_mut(parent_xf).add_child(child_xf);

      app.update();

      let p = app.world().get::<ResolvedLayout>(child_plain).unwrap().position;
      let x = app.world().get::<ResolvedLayout>(child_xf).unwrap().position;
      assert_eq!(p, x, "transform must NOT move the layout box (spec § 1.2)");
  }

  #[test]
  fn transform_does_not_change_sibling_positions() {
      // Flex row with three children; middle child rotated; assert the
      // siblings' ResolvedLayout positions match the un-rotated case.
      fn build(app: &mut App, rotate_middle: bool) -> [Entity; 3] {
          let parent = app
              .world_mut()
              .spawn((Node, Style::default().flex_axis(FlexAxis::Row)))
              .id();
          let mut kids = [Entity::PLACEHOLDER; 3];
          for i in 0..3 {
              let mut s = Style::default();
              // give each child a fixed size so positions are deterministic
              s.box_model.width = Sizing::Length(Length::px(50.0));
              s.box_model.height = Sizing::Length(Length::px(50.0));
              if i == 1 && rotate_middle {
                  s = s.rotate_z(std::f32::consts::FRAC_PI_4);
              }
              let c = app.world_mut().spawn((Node, s)).id();
              app.world_mut().entity_mut(parent).add_child(c);
              kids[i] = c;
          }
          kids
      }

      let mut plain = app();
      let kp = build(&mut plain, false);
      plain.update();

      let mut rot = app();
      let kr = build(&mut rot, true);
      rot.update();

      for i in 0..3 {
          let pp = plain.world().get::<ResolvedLayout>(kp[i]).unwrap().position;
          let rp = rot.world().get::<ResolvedLayout>(kr[i]).unwrap().position;
          assert_eq!(pp, rp, "child {i} position must be unaffected by a sibling's transform");
      }
  }

  #[test]
  fn display_none_transformed_entity_gets_no_resolved_transform() {
      let mut app = app();
      let mut s = Style::default().translate_px(10.0, 10.0);
      s.display = Display::None;
      let e = app.world_mut().spawn((Node, s)).id();
      app.update();
      assert!(
          app.world().get::<ResolvedTransform>(e).is_none(),
          "Display::None is skipped by sub-pass 6e"
      );
  }
  ```
  **Implementer note:** `Mat4`/`Vec3`/`Entity::PLACEHOLDER` from the bevy prelude. The `flex_axis(FlexAxis::Row)` and `box_model` field-set are existing `Style` API. If `add_child` requires the `Children`/`ChildOf` relation be set via `EntityWorldMut::add_child`, that is the Bevy 0.18 API — confirm. Move the duplicate `use … LayoutPlugin` to the top import block (shown split here only for clarity).

- [ ] **Step 2: Extend `layout_containment.rs` with the 3-entity content-vis dedup test.**
  ```rust
  #[test]
  fn content_visibility_deferred_warns_once_per_entity_across_three() {
      use buiy_core::layout::ContentVisibility;
      let mut app = app();
      let mk = |app: &mut App| {
          app.world_mut()
              .spawn((
                  Node,
                  Style::default().containment(Containment {
                      content_visibility: ContentVisibility::Auto,
                      ..Default::default()
                  }),
              ))
              .id()
      };
      let a = mk(&mut app);
      let b = mk(&mut app);
      let c = mk(&mut app);
      app.update();
      // run a second frame — dedup must hold (no panic / re-warn observable
      // via the set, which persists per session).
      app.update();

      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert!(warned.set.contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(a)));
      assert!(warned.set.contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(b)));
      assert!(warned.set.contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(c)));
      // Exactly three content-vis keys (one per entity), no duplicates.
      let count = warned
          .set
          .iter()
          .filter(|k| matches!(k, LayoutWarnOnceKey::ContentVisibilityDeferred(_)))
          .count();
      assert_eq!(count, 3, "one warn-once key per entity, deduped across frames");
  }
  ```

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core --test layout_transforms
  cargo test -p buiy_core --test layout_containment
  ```
  Expected PASS (after T13 re-exports land — sequence per the T6 note; the implementer may finalize T13 before running these if re-exports are missing).

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  git add crates/buiy_core/tests/layout_transforms.rs crates/buiy_core/tests/layout_containment.rs
  git commit -m "test(buiy_core): transform composition + layout-flow invariance + containment (Phase 8 — spec § 7)

layout_transforms.rs: translate composes to ResolvedTransform; a transform does
NOT change the entity's own ResolvedLayout position or its siblings' (flex row,
middle child rotated); Display::None skipped. layout_containment.rs: SIZE zeroing
+ warn, content-vis deferred warns once across 3 entities."
  ```

### Task 13: `register_type` wiring + re-exports + full project gate

**Spec:** § 1, § 1.1, § 5 (all new public types must be reflected + re-exported).

**Files:**
- Modify: `crates/buiy_core/src/layout/mod.rs` (`register_type` group + `pub use` for `layout` types)
- Modify: `crates/buiy_core/src/components.rs` is re-exported via `crates/buiy_core/src/lib.rs:17` (`pub use components::{Node, ResolvedLayout, Visual}`) → add `ResolvedTransform`
- Modify: `crates/buiy_core/src/lib.rs` (layout re-export block + components re-export)
- Modify: `crates/buiy/src/lib.rs` (top-level facade re-export)

- [ ] **Step 1: Add the Phase 8 `register_type` group in `mod.rs` (after the Phase-7 group ending at `mod.rs:157`).** Change the trailing `.register_type::<LayoutWarnOnceKey>();` to continue the chain:
  ```rust
              .register_type::<LayoutWarnOnceKey>()
              // Phase 8 — transforms + containment.
              .register_type::<UiTransform>()
              .register_type::<Translate>()
              .register_type::<Rotate>()
              .register_type::<Scale>()
              .register_type::<TransformMatrix>()
              .register_type::<TransformOrigin>()
              .register_type::<TransformStyle>()
              .register_type::<BackfaceVisibility>()
              .register_type::<ResolvedTransform>()
              .register_type::<Containment>()
              .register_type::<ContainFlags>()
              .register_type::<ContentVisibility>()
              .register_type::<WillChange>()
              .register_type::<WillChangeProperty>();
  ```
  **Implementer note:** `ResolvedTransform` lives in `crate::components` — import it at the top of `mod.rs` (`use crate::components::ResolvedTransform;`) or qualify it. `ContainFlags` is registered via `impl_reflect_opaque!` (T8) which makes `register_type::<ContainFlags>()` valid. `register_type` of an enum (`TransformMatrix`, etc.) auto-covers its variants. Bring the new `types`/`components` names into scope in `mod.rs` as needed for the `register_type::<T>()` calls.

- [ ] **Step 2: Extend `mod.rs` `pub use` blocks.** In `pub use components::{ … }` (`mod.rs:13`) add `Containment, Rotate, Scale, Translate, UiTransform`. In `pub use types::{ … }` (`mod.rs:25`) add `BackfaceVisibility, ContainFlags, ContentVisibility, TransformMatrix, TransformOrigin, TransformStyle, WillChange, WillChangeProperty`.

- [ ] **Step 3: Re-export `ResolvedTransform` from `crates/buiy_core/src/lib.rs:17`.**
  ```rust
  pub use components::{Node, ResolvedLayout, ResolvedTransform, Visual};
  ```
  And add the new layout types to the `pub use layout::{ … }` block (`lib.rs:19`): `BackfaceVisibility, ContainFlags, Containment, ContentVisibility, Rotate, Scale, Translate, TransformMatrix, TransformOrigin, TransformStyle, UiTransform, WillChange, WillChangeProperty`.

- [ ] **Step 4: Re-export the same set from the top-level facade `crates/buiy/src/lib.rs`.** Add `ResolvedTransform` to the `components::{Node, ResolvedLayout, Visual}` line (`buiy/src/lib.rs:10`) and the new layout types to the `layout::{ … }` block (`buiy/src/lib.rs:12`), keeping alphabetical order.

- [ ] **Step 5: Full project gate (the CLAUDE.md "run all checks" command, exactly).**
  ```bash
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    xvfb-run -a cargo test --workspace
  ```
  Expected: all green. This is the first task where the entire workspace (all integration test files, all re-exports) must compile and pass together.

- [ ] **Step 6: Supply-chain check (only if T8 added `bitflags`).**
  ```bash
  cargo deny check
  ```
  Expected: clean.

- [ ] **Step 7: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/mod.rs crates/buiy_core/src/lib.rs crates/buiy/src/lib.rs
  git commit -m "feat(layout): register + re-export Phase 8 transform/containment types

register_type for UiTransform, Translate, Rotate, Scale, TransformMatrix,
TransformOrigin, TransformStyle, BackfaceVisibility, ResolvedTransform,
Containment, ContainFlags, ContentVisibility, WillChange, WillChangeProperty.
Re-exported from buiy_core + the buiy facade. Full workspace gate green."
  ```

### Task 14: Closeout — CHANGELOG + follow-ups + status tag

**Spec:** all of the above.

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/plans/follow-ups.md`
- Modify: `docs/README.md` (status tag)

- [ ] **Step 1: Final whole-branch review.** Dispatch one fresh `code-reviewer` agent over the full diff from `main`. Address any BLOCKERs. Re-run the full project gate to confirm green.

- [ ] **Step 2: CHANGELOG.md additions.**
  ```markdown
  ## Phase 8 — Layout: transforms + containment

  ### Added
  - `UiTransform` component (self-styling visual transform; named `UiTransform` to avoid the glob-imported `bevy::prelude::Transform` collision) + value types `TransformMatrix` (None default), `TransformOrigin` (50%/50%/0 default), `TransformStyle` (Flat), `BackfaceVisibility` (Visible). Spec § 1.
  - `Translate` / `Rotate` / `Scale` decomposed-only longhand components (`Scale::default()` is `(1,1,1)` per CSS). Spec § 1.1.
  - `compose_transform(ui, t, r, s) -> Mat4` — pure helper implementing `M = T·R·S·M_transform` (None=IDENTITY, Translate/Rotate/Scale/Skew/Matrix direct, Compose=fold product A·B·…). Spec § 1.
  - `transform_composition` system — `PostTaffyOverrides` sub-pass **6e**, runs after `anchor_resolution` (6d). Writes the composed matrix to `ResolvedTransform`; writes nothing to `PostTaffyPositionOverrides` (a transform does not move the layout box). Identity → no `ResolvedTransform` (+removes stale); skips `Display::None`. Spec § 1.1, § 7.
  - `ResolvedTransform { matrix: Mat4 }` — private render handoff for transforms, mirroring `ResolvedLayout`. Spec § 1.
  - `Style.ui_transform` field + `.ui_transform()` / `.translate_px()` / `.rotate_z()` / `.scale()` fluent setters.
  - `Containment` component (`contain: ContainFlags`, `content_visibility: ContentVisibility`, `will_change: WillChange`) + value types `ContainFlags` bitflags (CONTENT/STRICT are bit-unions; `impl_reflect_opaque!`), `ContentVisibility` (Visible default), `WillChange` (Auto default), `WillChangeProperty`. Spec § 5.
  - `Style.containment` field + `.containment()` / `.contain()` setters.
  - SIZE / INLINE_SIZE containment enforcement: under `contain: size` / `inline-size`, an auto width/height on a contained axis is treated as `0px` with a warn-once `LayoutWarnOnceKey::SizeContainmentZeroed(Entity)`. Spec § 5.1.
  - `LayoutWarnOnceKey::SizeContainmentZeroed(Entity)` + `ContentVisibilityDeferred(Entity)` variants.
  - Integration tests `tests/layout_transforms.rs` + `tests/layout_containment.rs`; pipeline-order test extended to assert 6e runs.

  ### Changed
  - `BuiyLayoutStep::PostTaffyOverrides` chain now has six elements: `clear → sticky 6a → table 6b → multicol 6c → anchor 6d → transform 6e`.

  ### Deferred / divergences
  - **Bevy `Transform`/`GlobalTransform` write — deferred (deliberate divergence from spec § 2 approach (a) at the implementation-timing level).** Phase 8 produces the spec's `ResolvedTransform` artifact but does NOT write Bevy `Transform`: render reads `ResolvedLayout` directly and `buiy_core` has no `TransformPlugin` wiring (the harness uses `MinimalPlugins`), so a `Transform` write would be dead code. The Bevy-`Transform` ownership bridge (spec approach (a)) is a render-pipeline / 3D-anchored-UI follow-up. Tracked in `follow-ups.md`.
  - **`content-visibility` `Auto` + `Hidden` — stored, not enforced.** `Auto` needs last-frame `ResolvedLayout` + viewport + `contain-intrinsic-size`; `Hidden` needs a tree-skip path. Both deferred; value is stored and `content_visibility != Visible` warns once (`ContentVisibilityDeferred`). Spec § 5.2. Tracked in `follow-ups.md`.
  - **`will-change` — stored-only (tier-E).** No layer promotion, no SC trigger in Phase 8 (SC triggers are Phase 9). Valid stored hint; no warn. Spec § 5.3. Tracked in `follow-ups.md`.
  - **`LAYOUT` / `PAINT` / `STYLE` contain flags — stored, no Phase 8 layout effect** beyond what Taffy already does (spec § 5.1: "render-side primarily; layout records").
  - **Non-px translate units in transforms** — `compose_transform` resolves only `Length::Px` for translate; percent/cq translate (resolved against the entity's own box) contributes `0.0` and is deferred to the render/animation phase.
  - **Stacking-context formation / top-layer — Phase 9 (sub-pass 6f reads `ResolvedTransform`).** A non-identity transform forms a stacking context (spec § 3), but detection is Phase 9. Not in Phase 8.

  ### Removed
  - None.

  ### Performance contract
  - Steady-state O(0) preserved: `transform_composition` is `O(UiTransform-bearing entities)`; identity transforms insert nothing. SIZE-containment substitution is `O(1)` per contained entity inside the existing `sync_styles` pass.
  ```

- [ ] **Step 3: Add Phase 8 follow-ups to `docs/plans/follow-ups.md`.**
  - **Layout / render — Bevy `Transform` ownership bridge (`GlobalTransform` write).** Implement spec § 2 approach (a): `write_resolved_layout` (or a dedicated render-prep system) composes `ResolvedLayout.position` + `ResolvedTransform.matrix` into the entity's Bevy `Transform`, so `TransformSystems::Propagate` owns `GlobalTransform`. Requires pulling `TransformPlugin` into the relevant app + render reading `GlobalTransform` instead of (or alongside) `ResolvedLayout`. Phase 8 deliberately defers this (D2) because render currently reads `ResolvedLayout` directly and the layout harness has no `TransformPlugin`.
  - **Layout — `content-visibility: auto` off-screen skip.** Implement the spec § 5.2 step-1 skip: check `ContentVisibility::Auto` + off-screen (last-frame `ResolvedLayout` vs viewport) + `contain-intrinsic-size` hint; feed Taffy a sentinel size and no-op the descendants' style sync; snap back on-screen. Needs a `contain-intrinsic-size` component.
  - **Layout — `content-visibility: hidden` descendant skip.** Equivalent to `Display::None` for descendants (tree-prune in `sync_styles`); snap back on toggle.
  - **Layout / render — `will-change` layer promotion + SC trigger.** Honor `WillChange::Properties` as a render layer-promotion hint and a stacking-context trigger when the list mentions an SC-forming property (`WillChangeProperty::Transform` etc.) — coordinates with Phase 9 stacking.
  - **Render — `UiTransform` paint + `Containment` PAINT clip rect + perspective / backface.** `perspective`, `TransformStyle::Preserve3d`, `BackfaceVisibility::Hidden` are render-side (spec § 4); render consumes `ResolvedTransform` + the containment flags.
  - **Layout — Phase 9 stacking sub-pass 6f reads `ResolvedTransform`.** A non-identity transform forms a stacking context (spec § 3); 6f runs after 6e and reads the composed matrix it produced.
  - **Layout — non-px translate units in `compose_transform`.** Resolve percent / `Cq*` translate against the entity's own resolved box (currently `0.0`); coordinate with the animation phase.

- [ ] **Step 4: Commit closeout docs.**
  ```bash
  git add CHANGELOG.md docs/plans/follow-ups.md
  git commit -m "docs(layout): Phase 8 CHANGELOG + follow-ups (transforms + containment)"
  ```

- [ ] **Step 5: Open PR, wait for CI, merge if green.** Push the branch, open the PR with a summary mirroring the CHANGELOG, wait for the CI gates, fix any failures inline, squash-merge once green.

- [ ] **Step 6: Flip plan + README to `[landed]` on main.**
  ```bash
  git checkout main && git pull --ff-only origin main
  # Edit docs/plans/2026-05-28-buiy-layout-transforms-containment.md: Status: active → landed
  # Edit docs/README.md: [active] → [landed]
  git add docs/plans/2026-05-28-buiy-layout-transforms-containment.md docs/README.md
  git commit -m "docs: mark Phase 8 layout plan [landed]"
  git push origin main
  ```

---

## Self-review

1. **Spec coverage:**
   - § 1 `UiTransform` + `TransformMatrix` (None default) + `TransformOrigin` (50%/50%/0 manual Default) + `TransformStyle` (Flat) + `BackfaceVisibility` (Visible) → T2 (value types), T3 (component).
   - § 1 composition convention `M = T·R·S·M_transform` + `Compose` product order → T5 (`compose_transform` + tests).
   - § 1.1 `Translate`/`Rotate`/`Scale` longhands (Scale Default 1,1,1; decomposed-only) → T4.
   - § 1.1 sub-pass 6e + `ResolvedTransform` handoff → T5 (`ResolvedTransform`), T6 (`transform_composition` + wiring + pipeline test).
   - § 1.2 transform does not affect layout flow → T12 (own-position + sibling-position invariance tests).
   - § 2 Bevy `Transform` mapping → D2 (deliberate defer; `ResolvedTransform` produced, Bevy write deferred) + T14 follow-up.
   - § 5 `Containment` + `ContainFlags` (CONTENT/STRICT bit-unions + `impl_reflect_opaque!`) + `ContentVisibility` + `WillChange` + `WillChangeProperty` → T8 (value types), T9 (component + Style field).
   - § 5.1 SIZE containment auto→0 + `warn!` → T10. § 5.2 content-visibility deferred → T11 (D6). § 5.3 will-change tier-E stored-only → T11 (D7).
   - § 7 test surface → T6 (identity→no-transform; non-identity→transform), T12 (layout-flow invariance, SIZE warn), T11/T12 (content-vis).
   - § 3 stacking-context formation → explicitly OUT of scope (D1, Phase 9 6f) — see "could-not-map" note.
   - `architecture.md § 3` sub-pass 6e ordering → T6. `architecture.md § 6` error model → T10/T11 (warn-once keys via `LayoutWarnedOnceSession`).

2. **Placeholder scan:** No "TBD" / "implement later" in code steps. Every code step shows real code; every test step shows a real test + a real `cargo test …` command + expected PASS/FAIL. Two implementer-judgment points remain, both bounded with a documented fallback: (a) the writing-mode horizontal/vertical accessor name in T10 (fallback mapping spelled out in D5 + T10 Step 3); (b) the exact `impl_reflect_opaque!` import path in T8 (a `grep` command + the most-likely path given).

3. **Type / name consistency:** `UiTransform`, `Translate`/`Rotate`/`Scale`, `TransformMatrix`/`TransformOrigin`/`TransformStyle`/`BackfaceVisibility`, `ResolvedTransform`, `compose_transform`, `transform_composition`, `Containment`, `ContainFlags`/`ContentVisibility`/`WillChange`/`WillChangeProperty`, and `LayoutWarnOnceKey::{SizeContainmentZeroed, ContentVisibilityDeferred}` are spelled identically across T2–T14 (definition, registration at T13, re-exports at T13, tests at T6/T10/T11/T12). The `PostTaffyOverrides` chain references match `mod.rs:180-188`. `compose_transform` signature matches its callsite in `transform_composition` (T6) and its unit tests (T5).

4. **Decision-block coverage:** D1 (scope) → T1/T6/T14; D2 (`ResolvedTransform` over Bevy `Transform`, `Mat4`) → T5/T14; D3 (6e after 6d, identity→none, Display::None) → T6; D4 (Style fields + decomposed-only longhands) → T4/T7/T9; D5 (SIZE/INLINE_SIZE zeroing) → T10; D6 (content-vis deferred) → T11; D7 (will-change stored-only, no warn) → T11; D8 (warn-once variants) → T10/T11.

5. **Total task count:** 14 (T1–T14). Each ends with a commit step; T13 carries the full workspace gate; T14 is the closeout (review + CHANGELOG + follow-ups + status flip).

**Spec requirement I could NOT map to an implementation task (intentional, documented):**
- **§ 3 stacking-context formation** (a non-identity `UiTransform` / `ContainFlags::PAINT` / `will-change` forms a stacking context; § 6's full SC trigger list). This is explicitly OUT of Phase 8 scope per D1 and the spec's own pipeline sequencing (§ 1.1 / § 3: detection is sub-pass **6f**, Phase 9, which reads the matrix 6e produces). Phase 8 produces the `ResolvedTransform` artifact 6f will consume; the SC detection itself is deferred to Phase 9 with a follow-up entry (T14). The § 7 test "Non-identity transform forms SC" and "will-change: transform forms SC" therefore land in Phase 9, not Phase 8.
- **§ 7 "`UiTransform` composes into Bevy `Transform`" test** — not mappable because Phase 8 deliberately defers the Bevy-`Transform` write (D2). The Phase-8-applicable half of that test surface (composition correctness) is covered by T5/T6/T12 via `ResolvedTransform`; the Bevy-`Transform`/`GlobalTransform` assertion lands with the approach-(a) follow-up.
