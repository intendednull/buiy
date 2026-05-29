# Phase 11: content-visibility auto + hidden enforcement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development`. Each task lists exact file paths and TDD steps; steps use checkbox (`- [ ]`) tracking. Run the project gate (below) before every commit and resolve every warning.

**Goal:** Turn the stored-but-deferred `ContentVisibility::Auto` / `ContentVisibility::Hidden` arms into real layout behavior. (a) **Auto** = off-screen skip — when a `ContentVisibility::Auto` entity is fully off-screen (last frame's `ResolvedLayout` vs the primary-window viewport, with a hysteresis margin so the skip does not thrash at the edge) **and** the author supplied a `ContainIntrinsicSize` hint, feed Taffy a sentinel size equal to the hint and exclude the entity's descendants from the Taffy child list (no-op descendant style sync), snapping back to full layout when on-screen. (b) **Hidden** = the entity's descendants are skipped exactly like `Display::None` (descendants excluded from the Taffy tree), snapping back when toggled. Remove the now-obsolete `ContentVisibilityDeferred` warn for these two arms; keep the contract clean.

**Architecture:** All of the skip logic lives in pipeline step 1 (`sync_styles`), per spec § 5.2 ("during step 1, check if the entity is `ContentVisibility::Auto` and currently off-screen … mark the subtree for skip — Taffy receives a sentinel size and the descendants' style sync is no-op"). A new pure helper `content_visibility_skip(...)` classifies each entity into one of three `SkipKind`s (None / AutoSentinel / HiddenPrune) from its `Containment.content_visibility`, its optional `ContainIntrinsicSize`, its last-frame `ResolvedLayout`, and the viewport rect (expanded by a hysteresis margin). `sync_styles` builds a per-frame `HashSet<Entity>` of "skip-children" entities (Auto-sentinel or Hidden) and feeds it to the existing children-sync pass so those entities' Taffy child lists are emptied — the descendants keep their own Taffy nodes (cheap to re-attach on snap-back) but are detached from the layout tree, so Taffy never lays them out. For Auto-sentinel entities, `style_to_taffy` additionally overrides the entity's own Taffy size with the intrinsic-size hint via a new `StyleView` field. This stays inside the "layout writes, render reads" contract: no re-entrancy, the off-screen decision reads only *previous* frame's `ResolvedLayout` (already the established pattern for `Length::Cq*` resolution).

**Tech Stack:** Bevy 0.18 (`bevy::prelude::{Component, Reflect, Query, Entity, Children, ChildOf, With, Changed, Or}`, `bevy::math::{Vec2, Rect}`). `std::collections::HashSet` (no `bevy::utils::*`, per Phase 6/7/8/9 precedent). Taffy 0.10 via the existing `LayoutTree` (`set_children` / `set_style`). No new external dependency. Reads the primary window for the viewport (same read `sync_styles` already does for `viewport_size`).

**Date:** 2026-05-29
**Status:** active
**Spec:** [`specs/2026-05-08-buiy-layout-design/transforms-and-containment.md`](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md) § 5.2 (`content-visibility: auto` / `hidden`), § 5 (`Containment` / `ContentVisibility`), § 7 (test surface) + [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md) § 3 (step 1 `sync_styles`), § 6 (error model). Chartered by [`follow-ups.md`](follow-ups.md) "Layout — `content-visibility: auto` off-screen skip" + "Layout — `content-visibility: hidden` descendant skip" (both Phase-8 D6 deferrals).

---

## Prior-art citations (used throughout this plan)

- **Blink `content-visibility` — the reference implementation (Chrome 85, 2020-08-05).** `docs/prior-art/blink/containment-and-queries.md` § 2: `auto` "gains layout, style, and paint containment unconditionally; **when off-screen and not user-relevant** … it *additionally* gains size containment and skips laying out, painting, and hit-testing its descendants. When it scrolls on-screen, full rendering snaps back." `hidden` "skips rendering of contents (layout + paint), like `display: none` for the subtree but cheaper to toggle back." The same file's **Buiy mapping** confirms our exact approach ("during pipeline step 1, check `Auto` + last-frame off-screen `ResolvedLayout`, mark the subtree for skip, feed Taffy a sentinel size, no-op descendant style sync … which is structurally Blink's approach").
- **The intrinsic-size dependency is load-bearing.** `docs/prior-art/blink/containment-and-queries.md` § 2 + `docs/prior-art/blink/lessons.md` row "Un-stubbing `content-visibility: auto` without `contain-intrinsic-size`": Blink's ~7× win is real "but a skipped subtree with no intrinsic-size hint forces the engine to lay it out anyway to learn its size → scrollbar jumping and reflows on scroll. … When un-stubbing, ship `contain-intrinsic-size` *first* or the skip causes the same scroll jank." Phase 11 therefore predicates the Auto Taffy-skip on `ContainIntrinsicSize` being present (D2) — without it, the entity lays out normally (Auto still skips *paint* off-screen, which is a render concern Phase 11 does not own).
- **"Layout writes, render reads" — no re-entrancy.** `docs/prior-art/blink/lessons.md` § first row + `docs/prior-art/blink/open-problems.md`: the off-screen check reads *previous*-frame `ResolvedLayout` only, exactly like Buiy's bounded `Length::Cq*` feedback edge (`systems.rs:1360-1378`). No new Taffy re-entry; the skip is a pure function of last-frame geometry + current style.
- **Step-1 skip location** — `crates/buiy_core/src/layout/systems.rs:1496-1553` (`sync_styles`'s per-entity loop already holds the `Containment` + the SIZE-containment-zeroing warn-once that this plan's content-visibility check sits next to) and `:1555-1569` (the children-sync pass, `sync_children_pass`, which this plan extends to honor the skip set).
- **Children-sync exclusion-set precedent** — `crates/buiy_core/src/layout/systems.rs:1700-1807` (`sync_children_pass` / `sync_children_for_entity` already take a `fixed_set: &HashSet<Entity>` and *exclude* those entities from a parent's Taffy child list). Phase 11's "skip-children" set reuses this exact mechanism, but the semantics differ: `fixed_set` excludes the *entity itself* from its parent and re-homes it on the root; the content-visibility skip set empties the *entity's own* child list (its descendants vanish from the tree).
- **Sentinel-size injection** — `crates/buiy_core/src/layout/translate.rs:280-322` (`style_to_taffy`) + `:177-186` (`apply_size_containment`, which already substitutes a `Sizing::Length` for an `Auto` axis under SIZE containment). Phase 11 adds a `StyleView.content_visibility_intrinsic: Option<Vec2>` field that, when `Some`, overrides the resolved `size` with the intrinsic-size hint — the same "override the size axis" pattern.
- **`StyleView` construction** — `crates/buiy_core/src/layout/systems.rs:1633-1650` (`translate_one_entity` builds `StyleView`); `crates/buiy_core/src/layout/translate.rs:237-278` (`StyleView` struct). Phase 11 threads one new field through both.
- **Last-frame `ResolvedLayout` read** — `crates/buiy_core/src/components.rs:20-29` (`ResolvedLayout { position, size }`, written by step 7, read in step 1 as last-frame). `crates/buiy_core/src/layout/systems.rs:1486-1490` (`sync_styles` already reads the primary window for `viewport_size`).
- **Warn-once enum + idiom** — `crates/buiy_core/src/layout/types.rs:979-1031` (`LayoutWarnOnceKey`, including `ContentVisibilityDeferred(Entity)` at `:1025`, to be repurposed in T7). Dedup idiom `if warned.set.insert(key) { warn!(…) }` (`systems.rs:1516-1527`).
- **Component-registration + facade re-export** — `crates/buiy_core/src/layout/mod.rs:165-185` (the Phase-8 / Phase-9 `register_type` groups), `:13-37` (`pub use components::{…}` + `pub use types::{…}`), `crates/buiy/src/lib.rs` (top-level facade). Phase 11 adds `ContainIntrinsicSize`.
- **Test harness** — `crates/buiy_core/tests/layout_containment.rs:13-18` (`app()` = `MinimalPlugins + CorePlugin + LayoutPlugin`; window-less, so `taffy_compute` falls back to `800×600` — `systems.rs:1835-1839`; `sync_styles`'s `viewport_size` falls back to `Vec2::ZERO` when no `PrimaryWindow` — `systems.rs:1486-1490`). The viewport for the off-screen check must therefore come from the **same** primary-window read that already exists, and tests that exercise off-screen behavior must spawn a `PrimaryWindow` (D5).

---

## File map (what each task touches)

| File | Touched by tasks |
|---|---|
| `crates/buiy_core/src/layout/components.rs` | T1 (`ContainIntrinsicSize` component) |
| `crates/buiy_core/src/layout/systems.rs` | T2 (`SkipKind` + `content_visibility_skip` pure helper), T3 (`viewport_rect` + off-screen helper), T5 (`sync_styles` wiring + skip set + warn removal), T6 (`cq_flip_rerun` parity) |
| `crates/buiy_core/src/layout/translate.rs` | T4 (`StyleView.content_visibility_intrinsic` field + size override) |
| `crates/buiy_core/src/layout/style.rs` | T8 (`contain_intrinsic_size` field + setter) |
| `crates/buiy_core/src/layout/mod.rs` | T9 (`register_type` + `pub use` re-exports) |
| `crates/buiy/src/lib.rs` | T9 (top-level facade re-export) |
| `crates/buiy_core/tests/layout_containment.rs` | T7 (rewrite the obsolete deferred-warn tests) |
| `crates/buiy_core/tests/layout_content_visibility.rs` | T10 (new file — spec § 5.2 + § 7 integration tests) |

No changes to: `crates/buiy_core/src/components.rs` (`ResolvedLayout` is read, not changed), `crates/buiy_core/src/layout/tree.rs`, `crates/buiy_core/src/layout/pipeline.rs` (no new step — the skip lives inside step 1 `SyncStyles`), `crates/buiy_core/src/render/*`.

---

## Decision blocks (locked-in choices the implementer must honor)

### D1. `ContainIntrinsicSize` is a separate decomposed component, not a `Containment` field

**Decision:** Add a new component `ContainIntrinsicSize { width: Option<f32>, height: Option<f32> }` (logical-px hints, `None` = no hint for that axis) in `crates/buiy_core/src/layout/components.rs`, and a `Style` field + `.contain_intrinsic_size(w, h)` setter (T8). It is **optional** — most entities never carry it.

**Why:** CSS `contain-intrinsic-size` is a distinct property from `content-visibility` / `contain`; it takes per-axis lengths and is meaningful independently (it also pairs with bare `contain: size`). Spec § 5.2 calls it "an opt-in size hint" — opt-in maps cleanly to a separate, usually-absent component, mirroring how `Translate`/`Rotate`/`Scale` are decomposed-only longhands rather than fields baked onto every entity. Putting per-axis lengths inside `Containment` (which every `Style`-spawned entity carries at its default) would bloat the always-present struct with a rarely-used pair of `Option<f32>`.

**How to apply:** T1 defines the component (default = both `None`). T8 adds the `Style` field + setter. The Auto skip reads it via `Option<&ContainIntrinsicSize>` (T5).

**Runner-up rejected:** A `contain_intrinsic_size: Option<Vec2>` field on `Containment`. Rejected: bloats the always-present `Containment`; loses the per-axis `Option` (CSS allows a hint on one axis only); breaks the "decomposed by concern" convention (`components.rs` doc header).

### D2. Auto skip requires BOTH off-screen AND a `ContainIntrinsicSize` hint; otherwise lay out normally

**Decision:** A `ContentVisibility::Auto` entity gets the Taffy skip (sentinel size + descendants detached) **only when** it is off-screen (D3) **and** it has a `ContainIntrinsicSize` with at least one axis set. If off-screen but no hint, it lays out normally (no skip). If on-screen, it always lays out normally.

**Why:** Spec § 5.2: "*Skips Taffy compute* on its descendants when both off-screen AND its `contain-intrinsic-size` … is set. Without `contain-intrinsic-size`, the engine has to lay out to determine size — defeats the purpose." Blink hit exactly this (`prior-art/blink/containment-and-queries.md` § 2, `lessons.md`): a skipped subtree with no intrinsic-size hint forces a layout anyway → scrollbar jank. Predicating the skip on the hint is the lesson Buiy adopts.

**How to apply:** `content_visibility_skip` (T2) returns `SkipKind::AutoSentinel { intrinsic }` only when both conditions hold; `SkipKind::None` otherwise. The paint-only off-screen skip for Auto-without-hint is a *render* concern (Phase 11 owns layout, not paint) and is not implemented here.

**Runner-up rejected:** Skip on off-screen alone, laying out descendants once to learn the size and caching it (the Blink `contain-intrinsic-size: auto` "remembered size" behavior). Rejected: that needs a measured-size cache + invalidation, far beyond this follow-up's scope; the spec explicitly gates the skip on the explicit hint for v1.

### D3. Off-screen = last-frame `ResolvedLayout` border box does NOT intersect the viewport expanded by a hysteresis margin (`ContentVisibilityMargin`)

**Decision:** "Off-screen" means the entity's *last-frame* `ResolvedLayout` rect (`Rect::from_corners(position, position + size)`) does **not** intersect the viewport rect `Rect { min: 0, max: viewport_size }` **expanded outward** by a margin `m` on all sides. The margin is a resource `ContentVisibilityMargin(pub f32)` defaulting to `200.0` logical px. Hysteresis: an entity becomes skipped only once it is fully outside the *expanded* viewport, and snaps back as soon as it intersects the *expanded* viewport — because the same expanded rect is used for both directions, the dead-band is the margin `m`, so an entity oscillating by less than `m` px around the edge does not flip skip-state every frame. An entity with no last-frame `ResolvedLayout` (first frame, never resolved) is treated as **on-screen** (never skip on the first frame — we have no geometry yet).

**Why:** Spec § 5.2 says the check is "using last frame's `ResolvedLayout`" and the follow-up charter explicitly warns "the off-screen check is per-frame and must not thrash (define the snap-back hysteresis the spec implies)." A single expanded rect used symmetrically for both enter-skip and exit-skip is the simplest correct hysteresis: there is no separate skip-in / skip-out threshold to keep consistent, and the margin doubles as the "render slightly-off-screen content early" pre-roll Blink uses. Reading only last-frame geometry preserves the no-re-entrancy contract (`prior-art/blink/lessons.md`).

**How to apply:** T3 adds `viewport_rect(viewport_size, margin)` + `is_off_screen(resolved: Option<&ResolvedLayout>, expanded_viewport: Rect) -> bool`. `sync_styles` reads `ContentVisibilityMargin` (init in T9) and the per-entity last-frame `ResolvedLayout` (new query item) to compute it.

**Runner-up rejected:** Two distinct thresholds (skip when fully outside `viewport + m_out`, un-skip when intersecting `viewport + m_in` with `m_in < m_out`). Rejected: equivalent dead-band behavior but two tunables to keep ordered and a stored per-entity skip-state to compare against; the single-expanded-rect form is stateless and equally non-thrashing.

### D4. The skip detaches descendants via the existing children-sync exclusion set; descendant Taffy nodes are kept (cheap snap-back)

**Decision:** Both `AutoSentinel` and `HiddenPrune` cause the entity to be added to a per-frame `skip_children: HashSet<Entity>`. `sync_children_pass` / `sync_children_for_entity` empty the Taffy child list of any entity in that set (set `&[]` children). The descendants' own Taffy nodes are **not** removed from `LayoutTree` — they are simply detached from their parent, so Taffy does not traverse them. When the entity comes back on-screen / toggles visible, it leaves the set and the next `sync_styles` rebuilds its real child list (the descendants are still in `LayoutTree`, so re-attach is a `set_children` call — no `new_leaf` churn).

**Why:** Spec § 5.2: "the descendants' style sync is no-op." Detaching the child list is exactly that — Taffy never lays the descendants out. Reusing the `fixed_set`-style exclusion mechanism (`sync_children_pass` already threads a `HashSet<Entity>`) keeps the topology a pure per-frame function of style + last-frame geometry (no stored "is-skipped" flag, matching the Phase-10 `is_fixed_root` D3 precedent). Keeping the descendant nodes alive makes snap-back O(set_children), not O(rebuild subtree) — the "cheaper to toggle back than `display:none`" property Blink advertises (`prior-art/blink/containment-and-queries.md` § 2).

**How to apply:** T5 builds `skip_children` and passes it to `sync_children_pass` (signature gains the set). `sync_children_for_entity` empties the list for members. T6 mirrors it in `cq_flip_rerun`.

**Runner-up rejected:** Remove the descendant Taffy nodes (`tree.tree.remove`) on skip and `new_leaf` them on snap-back. Rejected: defeats the "cheaper toggle" goal, churns node ids, and fights `gc_removed_nodes`; the entities still exist as Bevy entities so their nodes should persist.

### D5. The viewport comes from the primary window; off-screen tests must spawn a `PrimaryWindow`

**Decision:** The off-screen check uses the **same** primary-window read `sync_styles` already performs for `viewport_size` (`systems.rs:1486-1490`), which falls back to `Vec2::ZERO` when no `PrimaryWindow` exists. Under `MinimalPlugins` (no window), `viewport_size = ZERO`, so the *expanded* viewport is `Rect { min: -m, max: m }` (a `2m × 2m` box at the origin). Tests that assert off-screen skip behavior therefore spawn an explicit `Window` + `PrimaryWindow` so the viewport is well-defined; tests that only assert Hidden behavior (geometry-independent) need no window.

**Why:** `taffy_compute` sizes the layout root from `windows.iter().next()` with an `800×600` fallback, but `sync_styles`'s viewport read is `PrimaryWindow`-filtered and falls back to `ZERO` — they are two different reads (confirmed `systems.rs:1486` vs `:1835`). Rather than introduce a third viewport source, Phase 11 reuses the existing `sync_styles` `viewport_size` (it is already in scope at the exact point the skip is decided). The test-harness consequence (spawn a window for geometry tests) is documented so the implementer does not chase a `ZERO`-viewport false "everything is off-screen."

**How to apply:** T5 reuses the existing `viewport_size` local. T10's off-screen fixtures spawn `(Window { resolution: (W,H).into(), .. }, PrimaryWindow)` and place the tested child far outside `(W + margin, …)`. The Hidden fixtures (T10) need no window.

**Runner-up rejected:** Add `800×600` fallback to the `sync_styles` viewport read to match `taffy_compute`. Rejected: changes existing Phase-5 `Length::Cq*` viewport-fallback behavior (a behavior change outside this follow-up's scope); spawning a window in the handful of geometry tests is cleaner and explicit.

### D6. Remove the `ContentVisibilityDeferred` warn for the now-implemented arms; repurpose the variant for the residual "Auto off-screen but no intrinsic-size hint" case

**Decision:** The blanket "content-visibility != visible is deferred" warn (`systems.rs:1532-1543`) is removed. The `LayoutWarnOnceKey::ContentVisibilityDeferred(Entity)` variant is **repurposed** (not deleted) to fire once per (entity, session) for the one residual degenerate case: a `ContentVisibility::Auto` entity that is off-screen but has **no** `ContainIntrinsicSize` hint — i.e. the case where the author asked for the skip but the engine cannot perform it (D2), so it lays out anyway. The doc comment is rewritten to describe this new meaning; `Hidden` never warns (it is fully implemented).

**Why:** The variant name still reads true ("the auto skip is *deferred* for this entity because the hint is missing"), and it is the actionable diagnostic Blink's lesson recommends surfacing ("ship `contain-intrinsic-size` first"). Reusing the existing registered variant avoids a registry change and keeps the warn-once test surface stable. Deleting the variant would force a `register_type` edit and break the existing `layout_containment.rs` tests harder than rewriting them.

**How to apply:** T5 deletes the blanket warn and adds the targeted warn inside the Auto branch when off-screen + no hint. T7 rewrites the three obsolete `layout_containment.rs` tests (the two that assert the blanket warn fires for Auto/Hidden, and the three-entity dedup test) to assert the *new* semantics (no warn for Hidden; no warn for on-screen Auto; warn only for off-screen Auto without a hint).

**Runner-up rejected:** Delete `ContentVisibilityDeferred` entirely and add a new `ContentVisibilityAutoNoIntrinsicSize(Entity)` variant. Rejected: needs a `register_type` line change + a wider test churn for no semantic gain; the existing variant's name fits the repurposed meaning.

### D7. Hidden = descendants detached only; the entity itself still lays out and resolves its own box

**Decision:** `ContentVisibility::Hidden` excludes the entity's *descendants* from the Taffy tree (D4) but the entity itself is laid out normally and gets a normal `ResolvedLayout`. It is **not** the same as the entity setting `Display::None` on itself — only its subtree is skipped. (CSS: a `content-visibility: hidden` box still generates a box; its *contents* are skipped.)

**Why:** Spec § 5 table + § 5.2: Hidden "skips paint AND layout for **descendants** (treated as `Display::None` for layout)" — the word is descendants, not self. The spec's `ContentVisibility` doc comment (`components.rs` / `transforms-and-containment.md:186`) says "skips paint AND layout for descendants." The entity's own box is real (it is what the author scrolls to / sizes); only the subtree vanishes.

**How to apply:** T2's `content_visibility_skip` returns `HiddenPrune` for the *entity*, which T5 maps into `skip_children` (empties *its* child list). The entity's own `style_to_taffy` is unchanged for Hidden (no sentinel override — that is Auto-only).

**Runner-up rejected:** Treat Hidden as `Display::None` on the entity itself (map to `taffy::Display::None`). Rejected: contradicts the spec wording ("for descendants") and CSS — the Hidden box still occupies space and is scrollable-to.

### D8. The skip is computed in `sync_styles` AND mirrored in `cq_flip_rerun`

**Decision:** Both `sync_styles` (step 1) and `cq_flip_rerun` (step 5 — the same-frame re-translation after a container-query flip) must apply the content-visibility skip identically, because `cq_flip_rerun` re-runs the per-entity translation + children-sync with the union of `sync_styles`'s params (the established pattern, `systems.rs:2317-2456`).

**Why:** `cq_flip_rerun` is documented as "INTENTIONALLY duplicative with `sync_styles`" (`systems.rs:2329`); every behavior `sync_styles` applies to the Taffy tree, the re-run must reproduce, or a CQ flip frame would re-lay-out the skipped descendants and undo the skip. The `fixed_set` re-parenting is already mirrored there (`systems.rs:2450-2456`), so the content-visibility skip set follows the same mirror.

**How to apply:** T5 factors the skip-set computation into a small helper or inline block usable by both; T6 wires the identical block into `cq_flip_rerun` and the parity test asserts a skipped subtree stays skipped across a CQ flip frame.

**Runner-up rejected:** Compute the skip only in `sync_styles` and let `cq_flip_rerun` re-attach descendants. Rejected: a CQ flip would transiently re-lay-out the skipped subtree (a correctness bug + the exact thrash the charter warns against).

---

## Tasks

> **Per-task workflow (subagent-driven):**
> 1. Implementer subagent reads the task block.
> 2. Implementer follows TDD: failing test first, then minimal impl to pass, then refactor if needed, then commit.
> 3. Spec-compliance reviewer subagent reads the spec sections + the diff and asserts coverage.
> 4. Code-quality reviewer subagent reads the diff and asserts the code-quality bar.
> 5. Both reviews must be ✅ before moving to the next task.

> **Project gate (run before every commit, exactly — drop `xvfb-run -a` on this host, which has no xvfb; `MinimalPlugins` runs headless):**
> ```sh
> cargo fmt --all -- --check && \
>   cargo clippy --workspace --all-targets -- -D warnings && \
>   RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
>   cargo test --workspace
> ```

### Task 1: `ContainIntrinsicSize` component

**Spec:** § 5.2 (the `contain-intrinsic-size` hint), D1.

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` (add `ContainIntrinsicSize` near `Containment`)

- [ ] **Step 1: Failing test.** Add to `components.rs::mod tests` (the existing `#[cfg(test)] mod tests` block):
  ```rust
  #[test]
  fn contain_intrinsic_size_default_is_none_none() {
      let c = ContainIntrinsicSize::default();
      assert_eq!(c.width, None);
      assert_eq!(c.height, None);
  }

  #[test]
  fn contain_intrinsic_size_has_hint_reports_axes() {
      assert!(!ContainIntrinsicSize::default().has_hint());
      assert!(ContainIntrinsicSize { width: Some(100.0), height: None }.has_hint());
      assert!(ContainIntrinsicSize { width: None, height: Some(50.0) }.has_hint());
  }
  ```
  Run: `cargo test -p buiy_core contain_intrinsic_size` — expected FAIL (type does not exist).

- [ ] **Step 2: Add the component.** In `crates/buiy_core/src/layout/components.rs`, immediately after the `Containment` struct (the `pub struct Containment { … }` ending at `components.rs:420`):
  ```rust
  /// CSS `contain-intrinsic-size` — an author-supplied placeholder size
  /// (logical px, per axis) used when the entity's descendants are
  /// skipped under `ContentVisibility::Auto` (off-screen). `None` on an
  /// axis = no hint for that axis. Optional and usually absent — only
  /// `content-visibility: auto` authors who want the off-screen Taffy
  /// skip need it (spec § 5.2: without it the engine must lay the
  /// subtree out to learn its size, defeating the skip).
  ///
  /// Self-styling (a `Style` field, default both-`None`). Read by step 1
  /// (`sync_styles`) when classifying the content-visibility skip.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.2.
  #[derive(Component, Reflect, Clone, Copy, Default, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub struct ContainIntrinsicSize {
      pub width: Option<f32>,
      pub height: Option<f32>,
  }

  impl ContainIntrinsicSize {
      /// True if at least one axis carries a hint.
      pub fn has_hint(&self) -> bool {
          self.width.is_some() || self.height.is_some()
      }
  }
  ```
  **Implementer note:** `Option<f32>: Reflect` in Bevy 0.18. `#[reflect(Component, Default)]` matches the `Containment` / `Stacking` precedent (`components.rs:414-415`). Confirm `Component`, `Reflect` are in scope (the file's existing components use them).

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core contain_intrinsic_size
  ```
  Expected PASS.

- [ ] **Step 4: Project gate.** (Registration + `Style` field land in T9 / T8; here confirm compile/tests/doc green.)
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```

- [ ] **Step 5: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/components.rs
  git commit -m "feat(layout): ContainIntrinsicSize component (Phase 11 — spec § 5.2)

Per-axis Option<f32> size hint for content-visibility:auto off-screen skip.
Default both-None; has_hint() reports presence. Style field + register in T8/T9."
  ```

### Task 2: `SkipKind` + `content_visibility_skip` pure helper

**Spec:** § 5.2 (Auto/Hidden classification), D2, D7.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `SkipKind` enum + `content_visibility_skip` pure fn + unit tests)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  use crate::layout::components::{ContainIntrinsicSize, Containment};
  use crate::layout::types::ContentVisibility;

  fn cvis(cv: ContentVisibility) -> Containment {
      Containment { content_visibility: cv, ..Default::default() }
  }

  #[test]
  fn skip_none_when_visible() {
      let c = cvis(ContentVisibility::Visible);
      // off-screen + hint present, but Visible never skips.
      let hint = ContainIntrinsicSize { width: Some(100.0), height: Some(50.0) };
      assert_eq!(
          content_visibility_skip(&c, Some(&hint), /*off_screen=*/ true),
          SkipKind::None
      );
  }

  #[test]
  fn skip_hidden_always_prunes() {
      let c = cvis(ContentVisibility::Hidden);
      // Hidden prunes descendants regardless of geometry / hint.
      assert_eq!(
          content_visibility_skip(&c, None, /*off_screen=*/ false),
          SkipKind::HiddenPrune
      );
      assert_eq!(
          content_visibility_skip(&c, None, /*off_screen=*/ true),
          SkipKind::HiddenPrune
      );
  }

  #[test]
  fn skip_auto_on_screen_is_none() {
      let c = cvis(ContentVisibility::Auto);
      let hint = ContainIntrinsicSize { width: Some(100.0), height: Some(50.0) };
      assert_eq!(
          content_visibility_skip(&c, Some(&hint), /*off_screen=*/ false),
          SkipKind::None
      );
  }

  #[test]
  fn skip_auto_off_screen_without_hint_is_none() {
      // D2: Auto + off-screen but NO intrinsic-size hint → lay out normally.
      let c = cvis(ContentVisibility::Auto);
      assert_eq!(
          content_visibility_skip(&c, None, /*off_screen=*/ true),
          SkipKind::None
      );
      // a present-but-empty hint (both None) also does not qualify.
      let empty = ContainIntrinsicSize::default();
      assert_eq!(
          content_visibility_skip(&c, Some(&empty), /*off_screen=*/ true),
          SkipKind::None
      );
  }

  #[test]
  fn skip_auto_off_screen_with_hint_is_sentinel() {
      let c = cvis(ContentVisibility::Auto);
      let hint = ContainIntrinsicSize { width: Some(120.0), height: Some(40.0) };
      assert_eq!(
          content_visibility_skip(&c, Some(&hint), /*off_screen=*/ true),
          SkipKind::AutoSentinel { intrinsic: ContainIntrinsicSize { width: Some(120.0), height: Some(40.0) } }
      );
  }
  ```
  Run: `cargo test -p buiy_core content_visibility_skip skip_none_when_visible skip_hidden skip_auto` — expected FAIL.

- [ ] **Step 2: Add the enum + helper to `systems.rs`.** Near the other pure helpers (e.g. after `is_fixed_root`, `systems.rs:1678-1680`, or next to `forms_stacking_context`):
  ```rust
  /// How step 1 should treat an entity's subtree for `content-visibility`
  /// (spec § 5.2). Pure classification produced by
  /// [`content_visibility_skip`].
  #[derive(Clone, Copy, PartialEq, Debug)]
  pub(super) enum SkipKind {
      /// No skip — lay the entity and its descendants out normally.
      None,
      /// `content-visibility: auto`, off-screen, with a `contain-intrinsic-size`
      /// hint (D2): give the entity the intrinsic-size as its Taffy size and
      /// detach its descendants from the Taffy tree.
      AutoSentinel { intrinsic: ContainIntrinsicSize },
      /// `content-visibility: hidden` (D7): detach the entity's descendants
      /// from the Taffy tree (the entity itself still lays out).
      HiddenPrune,
  }

  /// Classify an entity's `content-visibility` skip for step 1 (spec § 5.2).
  ///
  /// - `Visible` → never skip.
  /// - `Hidden` → always `HiddenPrune` (descendants detached; entity box
  ///   still resolves — D7).
  /// - `Auto` → `AutoSentinel` only when BOTH off-screen AND a
  ///   `ContainIntrinsicSize` with at least one axis hint is present (D2);
  ///   otherwise `None` (lay out normally — the off-screen *paint* skip is
  ///   a render concern Phase 11 does not own).
  ///
  /// `off_screen` is computed by the caller from last-frame `ResolvedLayout`
  /// vs the hysteresis-expanded viewport ([`is_off_screen`], D3).
  pub(super) fn content_visibility_skip(
      containment: &Containment,
      intrinsic: Option<&ContainIntrinsicSize>,
      off_screen: bool,
  ) -> SkipKind {
      match containment.content_visibility {
          ContentVisibility::Visible => SkipKind::None,
          ContentVisibility::Hidden => SkipKind::HiddenPrune,
          ContentVisibility::Auto => {
              match intrinsic {
                  Some(h) if off_screen && h.has_hint() => {
                      SkipKind::AutoSentinel { intrinsic: *h }
                  }
                  _ => SkipKind::None,
              }
          }
      }
  }
  ```
  **Implementer note:** `ContainIntrinsicSize` derives `Copy` (T1), so `intrinsic: *h` copies. Confirm `ContentVisibility` is already imported in `systems.rs` (it is — `systems.rs:27`). `Containment` / `ContainIntrinsicSize` come from `crate::layout::components` — add `ContainIntrinsicSize` to the existing `use` if needed (the imports live at the top of `systems.rs`).

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core content_visibility_skip skip_none_when_visible skip_hidden skip_auto
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): SkipKind + content_visibility_skip pure helper (Phase 11 — spec § 5.2)

Classifies Visible/Hidden/Auto into None/HiddenPrune/AutoSentinel. Auto skips
only when off-screen AND a contain-intrinsic-size hint is present (D2). Wired
into sync_styles in T5."
  ```

### Task 3: `viewport_rect` + `is_off_screen` hysteresis helpers

**Spec:** § 5.2 (last-frame `ResolvedLayout` vs viewport), D3.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `viewport_rect` + `is_off_screen` pure fns + unit tests)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  use crate::components::ResolvedLayout;
  use bevy::math::{Rect, Vec2};

  #[test]
  fn viewport_rect_expands_by_margin() {
      let r = viewport_rect(Vec2::new(800.0, 600.0), 200.0);
      assert_eq!(r.min, Vec2::new(-200.0, -200.0));
      assert_eq!(r.max, Vec2::new(1000.0, 800.0));
  }

  #[test]
  fn on_screen_box_is_not_off_screen() {
      let vp = viewport_rect(Vec2::new(800.0, 600.0), 200.0);
      let rl = ResolvedLayout { position: Vec2::new(100.0, 100.0), size: Vec2::new(50.0, 50.0) };
      assert!(!is_off_screen(Some(&rl), vp));
  }

  #[test]
  fn box_beyond_expanded_viewport_is_off_screen() {
      let vp = viewport_rect(Vec2::new(800.0, 600.0), 200.0);
      // x starts at 1100 > max.x (1000) → fully outside the expanded rect.
      let rl = ResolvedLayout { position: Vec2::new(1100.0, 100.0), size: Vec2::new(50.0, 50.0) };
      assert!(is_off_screen(Some(&rl), vp));
  }

  #[test]
  fn box_within_margin_is_still_on_screen_hysteresis() {
      let vp = viewport_rect(Vec2::new(800.0, 600.0), 200.0);
      // x = 900: past the 800 viewport edge but inside the +200 margin → on-screen.
      let rl = ResolvedLayout { position: Vec2::new(900.0, 100.0), size: Vec2::new(50.0, 50.0) };
      assert!(!is_off_screen(Some(&rl), vp), "within the hysteresis margin counts as on-screen");
  }

  #[test]
  fn no_last_frame_layout_is_on_screen() {
      let vp = viewport_rect(Vec2::new(800.0, 600.0), 200.0);
      assert!(!is_off_screen(None, vp), "never skip without last-frame geometry (D3)");
  }
  ```
  Run: `cargo test -p buiy_core viewport_rect is_off_screen on_screen_box box_beyond box_within no_last_frame` — expected FAIL.

- [ ] **Step 2: Add the helpers to `systems.rs`.** Next to `content_visibility_skip` (T2):
  ```rust
  /// The viewport rectangle for the content-visibility off-screen test,
  /// expanded outward by `margin` on every side (the hysteresis dead-band,
  /// D3). Origin is the layout root's top-left `(0, 0)`; `viewport_size`
  /// is the primary window size (or `Vec2::ZERO` when window-less).
  pub(super) fn viewport_rect(viewport_size: bevy::math::Vec2, margin: f32) -> bevy::math::Rect {
      bevy::math::Rect {
          min: bevy::math::Vec2::new(-margin, -margin),
          max: viewport_size + bevy::math::Vec2::splat(margin),
      }
  }

  /// Whether an entity is "off-screen" for `content-visibility: auto`
  /// (spec § 5.2, D3): its *last-frame* `ResolvedLayout` border box does
  /// NOT intersect the hysteresis-expanded viewport. An entity with no
  /// resolved layout yet (first frame) is treated as on-screen — we have
  /// no geometry to skip against.
  pub(super) fn is_off_screen(
      resolved: Option<&ResolvedLayout>,
      expanded_viewport: bevy::math::Rect,
  ) -> bool {
      let Some(rl) = resolved else {
          return false;
      };
      let box_rect = bevy::math::Rect::from_corners(rl.position, rl.position + rl.size);
      // Off-screen iff the boxes do not overlap. `Rect::intersect` returns
      // an empty rect (zero area) when there is no overlap.
      expanded_viewport.intersect(box_rect).is_empty()
  }
  ```
  **Implementer note:** `bevy::math::Rect::intersect` returns the overlap rect; `Rect::is_empty()` is true when `min.x >= max.x || min.y >= max.y` (no positive-area overlap) — confirm against Bevy 0.18 `bevy_math` (`Rect::is_empty` exists; if the exact predicate differs, fall back to an explicit AABB-overlap test: `box.min.x < vp.max.x && box.max.x > vp.min.x && box.min.y < vp.max.y && box.max.y > vp.min.y` negated). Add `use crate::components::ResolvedLayout;` only inside the test module — the system file already imports `ResolvedLayout` at the top (`systems.rs:31`), so the non-test helper uses the crate path directly or the existing import.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core viewport_rect is_off_screen on_screen_box box_beyond box_within no_last_frame
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): viewport_rect + is_off_screen hysteresis helpers (Phase 11 — spec § 5.2, D3)

Margin-expanded viewport rect (single rect used symmetrically = stateless dead-band)
+ last-frame ResolvedLayout intersection test. No last-frame layout = on-screen."
  ```

### Task 4: `StyleView.content_visibility_intrinsic` size override in `style_to_taffy`

**Spec:** § 5.2 ("Taffy receives a sentinel size"), D2.

**Files:**
- Modify: `crates/buiy_core/src/layout/translate.rs` (add the `StyleView` field + apply the override in `style_to_taffy`; unit test)

- [ ] **Step 1: Failing test.** Add to `translate.rs::mod tests` (the existing `#[cfg(test)] mod tests`, which constructs `StyleView` directly — see `translate.rs:1082`+). Add a small constructor-style test:
  ```rust
  #[test]
  fn content_visibility_intrinsic_overrides_size() {
      use crate::layout::components::*;
      use crate::layout::types::*;
      // Build a default-ish StyleView with an auto size, then assert the
      // intrinsic override replaces the Taffy size with the sentinel.
      let display = Display::Block;
      let box_model = BoxModel { width: Sizing::Auto, height: Sizing::Auto, ..Default::default() };
      let containment = Containment::default();
      let position = Position::default();
      let flex_params = FlexParams::default();
      let overflow = Overflow::default();
      let scroll = Scroll::default();
      let grid_params = GridParams::default();
      let writing_mode_resolved = WritingModeResolved::default();

      let view = StyleView {
          display: &display,
          box_model: &box_model,
          containment: &containment,
          position: &position,
          flex_params: &flex_params,
          flex_item: None,
          overflow: &overflow,
          scroll: &scroll,
          grid_params: &grid_params,
          grid_item: None,
          parent_areas: None,
          writing_mode_resolved: &writing_mode_resolved,
          nearest_container: None,
          viewport_size: bevy::math::Vec2::ZERO,
          content_visibility_intrinsic: Some(bevy::math::Vec2::new(120.0, 40.0)),
      };
      let s = style_to_taffy(view);
      assert_eq!(s.size.width, taffy::Dimension::length(120.0));
      assert_eq!(s.size.height, taffy::Dimension::length(40.0));
  }
  ```
  **Implementer note:** match the exact `StyleView` field set + the existing test module's import style (the module already constructs `StyleView` — see `translate.rs:1082` region for the canonical field list; copy it and append the new field). `taffy::Dimension::length` is the constructor used by `length_to_dim` (`translate.rs:622`).
  Run: `cargo test -p buiy_core content_visibility_intrinsic_overrides_size` — expected FAIL (field does not exist).

- [ ] **Step 2: Add the field + override.**
  - In `crates/buiy_core/src/layout/translate.rs`, add to the `StyleView` struct (after `viewport_size`, `translate.rs:277`):
    ```rust
    /// `content-visibility: auto` off-screen sentinel size (logical px),
    /// or `None` when the entity is not skipping. When `Some`, the
    /// entity's resolved Taffy `size` is replaced by this `contain-intrinsic-size`
    /// hint and its descendants are detached from the Taffy tree by the
    /// caller (`sync_styles`), so Taffy never lays the subtree out.
    /// Set by `sync_styles` (step 1) from `content_visibility_skip`'s
    /// `AutoSentinel` result. Spec § 5.2.
    pub(super) content_visibility_intrinsic: Option<bevy::math::Vec2>,
    ```
  - In `style_to_taffy` (`translate.rs:280`), after the `let mut s = taffy::Style { … };` block (`translate.rs:301-342`), apply the override:
    ```rust
    // content-visibility: auto off-screen sentinel (spec § 5.2): replace
    // the resolved size with the contain-intrinsic-size hint so Taffy
    // reserves the placeholder box without measuring the (detached)
    // descendants.
    if let Some(intrinsic) = view.content_visibility_intrinsic {
        s.size = taffy::Size {
            width: taffy::Dimension::length(intrinsic.x),
            height: taffy::Dimension::length(intrinsic.y),
        };
    }
    ```
  **Implementer note:** the override is applied *after* the normal size computation so it wins. The caller (`sync_styles`, T5) packs the per-axis `ContainIntrinsicSize` `Option<f32>` into a `Vec2` for the skip case, falling back to `0.0` on an unset axis (so a width-only hint reserves `width × 0`); this matches CSS where a missing axis falls to `0` under the skip. Confirm `taffy::Size` / `taffy::Dimension` are already in scope in `translate.rs` (they are — used throughout).

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core content_visibility_intrinsic_overrides_size
  ```
  Expected PASS.

- [ ] **Step 4: Update the other `StyleView` construction sites to set `None`.** `StyleView` is built in `translate_one_entity` (`systems.rs:1634-1649`) and in `style_to_taffy`'s own test module (and possibly `cq_flip_rerun`'s path — it reuses `translate_one_entity`, so only the one site). T5 sets the real value there; for THIS task, add `content_visibility_intrinsic: None,` to `translate_one_entity`'s `StyleView { … }` so the workspace compiles. (T5 replaces the `None` with the computed value.) Run the gate:
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Expected PASS (everything compiles; the new field is wired with `None` placeholders that T5 will populate).

- [ ] **Step 5: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/translate.rs crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): StyleView content_visibility_intrinsic sentinel-size override (Phase 11 — spec § 5.2)

style_to_taffy overrides the resolved size with the contain-intrinsic-size hint
when the auto-skip sentinel is set. Construction sites default to None; populated
by sync_styles in T5."
  ```

### Task 5: Wire the skip into `sync_styles` (sentinel size + children-detach set + warn removal)

**Spec:** § 5.2 (the step-1 skip), D2, D3, D4, D6, D7.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (`sync_styles`: add the last-frame-layout + intrinsic queries, compute the skip, populate `skip_children`, override the sentinel size, remove the blanket warn, add the targeted warn; extend `sync_children_pass` / `sync_children_for_entity` to honor the skip set)

- [ ] **Step 1: Failing test.** Add to `tests/layout_content_visibility.rs` (create the file — T10 fills it out; minimal here). Two behaviors: Hidden detaches a descendant (window-less, geometry-independent), and Auto off-screen with a hint applies the sentinel size + detaches descendants (window-backed).
  ```rust
  // tests/layout_content_visibility.rs
  use bevy::prelude::*;
  use bevy::window::{PrimaryWindow, Window, WindowResolution};
  use buiy_core::{
      CorePlugin, Node, ResolvedLayout,
      layout::{
          ContainIntrinsicSize, ContentVisibility, Containment, LayoutPlugin, Length, Sizing, Style,
      },
  };

  fn app() -> App {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(LayoutPlugin);
      app
  }

  /// Spawn a primary window so `sync_styles`' viewport read is well-defined
  /// (D5). Without this the window-less viewport is `ZERO` and the off-screen
  /// test degenerates.
  fn with_window(app: &mut App, w: f32, h: f32) {
      app.world_mut().spawn((
          Window { resolution: WindowResolution::new(w, h), ..Default::default() },
          PrimaryWindow,
      ));
  }

  #[test]
  fn hidden_detaches_descendant_from_layout() {
      let mut app = app();
      // child has an explicit 50x50 size; if it were laid out it would resolve to 50x50.
      let child = app
          .world_mut()
          .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
          .id();
      let hidden = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .width_px(200.0)
                  .height_px(100.0)
                  .containment(Containment {
                      content_visibility: ContentVisibility::Hidden,
                      ..Default::default()
                  }),
          ))
          .add_child(child)
          .id();
      let _root = app.world_mut().spawn((Node, Style::default())).add_child(hidden).id();
      app.update();
      app.update(); // second frame so detach is stable

      // The Hidden entity itself still resolves its own box (D7).
      let hr = app.world().get::<ResolvedLayout>(hidden).expect("hidden box resolves");
      assert_eq!(hr.size, Vec2::new(200.0, 100.0), "hidden entity keeps its own box");
      // The detached child is not laid out by Taffy: it has no ResolvedLayout,
      // or a stale/zero one — assert it is NOT a live 50x50 child of `hidden`.
      // (Detached nodes are not traversed; their ResolvedLayout is absent or
      // retains a default. Assert absence of a meaningful resolved size.)
      let cr = app.world().get::<ResolvedLayout>(child);
      assert!(
          cr.map(|r| r.size == Vec2::ZERO).unwrap_or(true),
          "descendant of a content-visibility:hidden node is not laid out"
      );
  }

  #[test]
  fn auto_off_screen_with_hint_applies_sentinel_and_detaches() {
      let mut app = app();
      with_window(&mut app, 800.0, 600.0);
      let child = app
          .world_mut()
          .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
          .id();
      // Auto entity positioned far off-screen (x = 5000, well past 800 + margin),
      // absolutely positioned so its ResolvedLayout.position reflects the inset.
      let auto = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(buiy_core::layout::PositionKind::Absolute)
                  .inset(buiy_core::layout::Inset {
                      left: Sizing::Length(Length::px(5000.0)),
                      top: Sizing::Length(Length::px(0.0)),
                      ..Default::default()
                  })
                  .containment(Containment {
                      content_visibility: ContentVisibility::Auto,
                      ..Default::default()
                  })
                  .contain_intrinsic_size(Some(120.0), Some(40.0)),
          ))
          .add_child(child)
          .id();
      let _root = app.world_mut().spawn((Node, Style::default())).add_child(auto).id();
      // Frame 1 establishes the off-screen ResolvedLayout (no skip yet — no last
      // frame). Frame 2 reads frame-1 geometry and applies the skip.
      app.update();
      app.update();

      // Auto entity took the sentinel size (120x40), not its measured size.
      let ar = app.world().get::<ResolvedLayout>(auto).expect("auto box resolves");
      assert_eq!(ar.size, Vec2::new(120.0, 40.0), "off-screen auto uses contain-intrinsic-size");
      // child detached → not laid out.
      let cr = app.world().get::<ResolvedLayout>(child);
      assert!(
          cr.map(|r| r.size == Vec2::ZERO).unwrap_or(true),
          "descendant of an off-screen content-visibility:auto node is not laid out"
      );
  }
  ```
  **Implementer note:** confirm the import surface after T8/T9 (`ContainIntrinsicSize`, `.contain_intrinsic_size(..)`, `ContentVisibility`, `Inset`, `Length`, `Sizing`, `PositionKind`); the `.contain_intrinsic_size` setter lands in T8 — if writing this test before T8, set the component directly in the spawn tuple instead. `WindowResolution::new` is the Bevy 0.18 constructor; adjust if the import path differs. The "child not laid out" assertion is necessarily soft (a detached node has no fresh `ResolvedLayout`); T10 strengthens it with a Taffy-child-count probe if a reviewer wants a harder assertion.
  Run: `cargo test -p buiy_core --test layout_content_visibility hidden_detaches auto_off_screen` — expected FAIL.

- [ ] **Step 2: Extend `sync_styles`.**
  - **2a. Add the last-frame-layout + intrinsic lookups.** `sync_styles`'s main `nodes` query is `Changed`-filtered, but the skip needs *every* entity's last-frame `ResolvedLayout` + `ContainIntrinsicSize` (an off-screen entity whose only change is its ancestor's resize must still skip). Add two read-only side queries to `sync_styles`' signature (conflict-free, read-only, like `parent_grid_lookup`):
    ```rust
    resolved_lookup: Query<&ResolvedLayout>,
    intrinsic_lookup: Query<&ContainIntrinsicSize>,
    content_vis_margin: Res<ContentVisibilityMargin>,
    ```
    Add `ContainIntrinsicSize` to the `crate::layout::components` import and `ContentVisibilityMargin` is defined in this same step (2c).
  - **2b. Compute the per-entity skip + sentinel inside the existing per-entity loop.** In the `for item in nodes.iter()` loop (`systems.rs:1496-1553`), the per-entity tuple already binds `containment` (`systems.rs:1503`). Replace the blanket content-visibility warn block (`systems.rs:1529-1543`, the `if !matches!(containment.content_visibility, ContentVisibility::Visible) && warned.set.insert(ContentVisibilityDeferred…) { warn!(…) }`) with:
    ```rust
    // content-visibility skip (spec § 5.2). Off-screen uses last-frame
    // ResolvedLayout vs the hysteresis-expanded viewport (D3); Auto needs
    // both off-screen AND a contain-intrinsic-size hint (D2).
    let expanded_viewport = viewport_rect(viewport_size, content_vis_margin.0);
    let off_screen = is_off_screen(resolved_lookup.get(entity).ok(), expanded_viewport);
    let skip = content_visibility_skip(
        containment,
        intrinsic_lookup.get(entity).ok(),
        off_screen,
    );
    match skip {
        SkipKind::None => {
            // D6: the residual diagnostic — Auto + off-screen but no usable
            // intrinsic-size hint, so the requested skip cannot run.
            if matches!(containment.content_visibility, ContentVisibility::Auto)
                && off_screen
                && warned
                    .set
                    .insert(LayoutWarnOnceKey::ContentVisibilityDeferred(entity))
            {
                bevy::log::warn!(
                    "Entity {:?} has content-visibility: auto and is off-screen, but no \
                     contain-intrinsic-size hint — the off-screen layout skip is disabled \
                     for it (spec § 5.2). Set contain-intrinsic-size to enable the skip.",
                    entity,
                );
            }
        }
        SkipKind::AutoSentinel { intrinsic } => {
            skip_children.insert(entity);
            sentinel_size.insert(
                entity,
                bevy::math::Vec2::new(
                    intrinsic.width.unwrap_or(0.0),
                    intrinsic.height.unwrap_or(0.0),
                ),
            );
        }
        SkipKind::HiddenPrune => {
            skip_children.insert(entity);
        }
    }
    ```
    Declare `skip_children` + `sentinel_size` before the loop:
    ```rust
    let mut skip_children: HashSet<Entity> = HashSet::new();
    let mut sentinel_size: HashMap<Entity, bevy::math::Vec2> = HashMap::new();
    ```
    (`HashSet` / `HashMap` are already imported in `systems.rs`.)
  - **2c. Thread the sentinel size into `translate_one_entity`.** `translate_one_entity` builds the `StyleView` (`systems.rs:1634-1649`) and currently sets `content_visibility_intrinsic: None` (T4). It needs the per-entity sentinel. Add a parameter:
    ```rust
    pub(super) fn translate_one_entity(
        item: NodeQueryItem<'_>,
        parent_areas_for: &HashMap<Entity, GridAreas>,
        container_index: &HashMap<Entity, ContainerSnapshot>,
        cq_parent_chain: &Query<&ChildOf>,
        viewport_size: bevy::math::Vec2,
        content_visibility_intrinsic: Option<bevy::math::Vec2>,
        tree: &mut LayoutTree,
    ) {
    ```
    and set `content_visibility_intrinsic` in the `StyleView { … }` literal from the parameter. At the `sync_styles` call site (`systems.rs:1545-1552`), pass `sentinel_size.get(&entity).copied()`. (NOTE: the per-entity loop computes `skip`/`sentinel_size` BEFORE calling `translate_one_entity` for that entity — keep the order: classify, then translate.)
  - **2d. Define the margin resource.** Near `PostTaffyPositionOverrides` (`systems.rs:176`):
    ```rust
    /// Hysteresis margin (logical px) for the `content-visibility: auto`
    /// off-screen test (spec § 5.2, D3). The viewport is expanded by this
    /// margin on every side; an entity is "off-screen" only once its
    /// last-frame box is fully outside the expanded rect, and snaps back
    /// as soon as it re-enters — so an entity oscillating by less than
    /// this margin around the edge does not thrash skip-state. Also serves
    /// as the pre-roll distance (slightly-off-screen content is kept laid
    /// out). Default 200px.
    #[derive(Resource, Debug, Clone, Copy)]
    pub struct ContentVisibilityMargin(pub f32);

    impl Default for ContentVisibilityMargin {
        fn default() -> Self {
            ContentVisibilityMargin(200.0)
        }
    }
    ```
  - **2e. Detach skipped entities' children.** `sync_styles` calls `sync_children_pass(&rows, tree)` (`systems.rs:1569`). Extend `sync_children_pass` + `sync_children_for_entity` to take `skip_children: &HashSet<Entity>` and empty the child list for members:
    - `sync_children_pass(rows, skip_children, tree)` — pass it through to each `sync_children_for_entity` call (and to `attach_fixed_to_root`? — NO: `attach_fixed_to_root` only rebuilds the ROOT's list; a skipped root is degenerate. Skip is checked in `sync_children_for_entity` per parent).
    - In `sync_children_for_entity`, after computing `child_ids`, if `skip_children.contains(&entity)` then set `child_ids = Vec::new()` before the `set_children` call:
      ```rust
      let child_ids: Vec<TaffyNodeId> = if skip_children.contains(&entity) {
          // content-visibility skip (D4): detach descendants — Taffy never
          // lays the subtree out. Descendant nodes stay in LayoutTree for a
          // cheap re-attach on snap-back.
          Vec::new()
      } else {
          children
              .into_iter()
              .flatten()
              .filter(|c| !fixed_set.contains(c))
              .filter_map(|c| tree.by_entity.get(c).copied())
              .collect()
      };
      ```
    Update the `sync_children_pass` signature and its single body change (thread `skip_children` to the per-entity calls), and update the `attach_fixed_to_root` call to remain unchanged.
  - **2f. Update the `sync_children_pass` call in `sync_styles`** to pass `&skip_children`.
  **Implementer note:** the call site at `sync_styles` (`systems.rs:1563-1569`) builds `rows` from `fixed_sync_nodes` then calls `sync_children_pass(&rows, tree)`. Change to `sync_children_pass(&rows, &skip_children, tree)`. `cq_flip_rerun` ALSO calls `sync_children_pass` (`systems.rs:2456`) — T6 supplies its skip set; for THIS task, pass an empty set there (`&HashSet::new()`) so the workspace compiles, with a `// T6: real skip set` comment. Do NOT leave `cq_flip_rerun` re-attaching skipped subtrees silently — T6 is mandatory follow-through (D8).

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core --test layout_content_visibility hidden_detaches auto_off_screen
  ```
  Expected PASS.

- [ ] **Step 4: Init the margin resource.** In `crates/buiy_core/src/layout/mod.rs`, in the `init_resource` block (`mod.rs:75-77`, the Phase-9 block), add:
  ```rust
  app.init_resource::<systems::ContentVisibilityMargin>();
  ```
  (The `pub use` re-export of `ContentVisibilityMargin` lands in T9; `init_resource` here references it via `systems::`.)

- [ ] **Step 5: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/mod.rs crates/buiy_core/tests/layout_content_visibility.rs
  git commit -m "feat(layout): content-visibility skip in sync_styles (Phase 11 — spec § 5.2)

Auto off-screen + intrinsic-size hint → sentinel Taffy size + detached descendants;
Hidden → detached descendants (entity box still resolves). ContentVisibilityMargin
hysteresis resource (default 200px). Blanket deferred-warn removed; repurposed for
the off-screen-auto-without-hint residual case (D6). cq_flip_rerun parity in T6."
  ```

### Task 6: `cq_flip_rerun` parity — same-frame re-run honors the skip

**Spec:** § 5.2, D8.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (`cq_flip_rerun`: compute the identical skip set + sentinel + pass to `sync_children_pass` / `translate_one_entity`)

- [ ] **Step 1: Failing test.** Add to `tests/layout_content_visibility.rs`:
  ```rust
  #[test]
  fn skip_survives_container_query_flip_frame() {
      use buiy_core::layout::{Container, ContainerQuery, ContainerType, QueryCondition};
      let mut app = app();
      with_window(&mut app, 800.0, 600.0);

      // A query container whose flip triggers cq_flip_rerun in the same frame.
      // Inside it, an off-screen content-visibility:auto node with a hint and a
      // sized child. The CQ flip must NOT re-lay-out the detached child.
      let child = app
          .world_mut()
          .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
          .id();
      let auto = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(buiy_core::layout::PositionKind::Absolute)
                  .inset(buiy_core::layout::Inset {
                      left: Sizing::Length(Length::px(5000.0)),
                      ..Default::default()
                  })
                  .containment(Containment {
                      content_visibility: ContentVisibility::Auto,
                      ..Default::default()
                  })
                  .contain_intrinsic_size(Some(120.0), Some(40.0)),
          ))
          .add_child(child)
          .id();
      // Container query on the auto node's sibling that flips on frame 1
      // (forces a same-frame cq_flip_rerun).
      let queried = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(400.0).height_px(100.0).container_size(),
              ContainerQuery {
                  container: None,
                  conditions: vec![QueryCondition::MinWidth(Length::px(100.0))],
                  ..Default::default()
              },
          ))
          .id();
      let _root = app
          .world_mut()
          .spawn((Node, Style::default().width_px(400.0).height_px(600.0).container_size()))
          .add_children(&[auto, queried])
          .id();
      app.update();
      app.update();

      let ar = app.world().get::<ResolvedLayout>(auto).expect("auto resolves");
      assert_eq!(ar.size, Vec2::new(120.0, 40.0), "sentinel survives the CQ flip frame");
      let cr = app.world().get::<ResolvedLayout>(child);
      assert!(
          cr.map(|r| r.size == Vec2::ZERO).unwrap_or(true),
          "detached child stays detached across the CQ flip re-run"
      );
  }
  ```
  **Implementer note:** the exact `ContainerQuery` / `QueryCondition` field/variant names must match the Phase-5 surface (`container-queries`); confirm via `crates/buiy_core/src/layout/components.rs` (`ContainerQuery`) + `types.rs` (`QueryCondition`). The point of the test is only that the skip is reproduced in the re-run; if constructing a guaranteed-flip CQ fixture is fiddly, the reviewer may accept a simpler fixture that calls `cq_flip_rerun` directly via `world.run_system_once` after seeding the skip state — but the integration form above is preferred.
  Run: `cargo test -p buiy_core --test layout_content_visibility skip_survives_container_query_flip` — expected FAIL if `cq_flip_rerun` re-attaches the child (it passes an empty skip set from T5).

- [ ] **Step 2: Mirror the skip in `cq_flip_rerun`.** `cq_flip_rerun` (`systems.rs:2317`+) already rebuilds `parent_areas_for` / `container_index` / `viewport_size` and loops `for item in nodes.iter() { translate_one_entity(item, …) }` then `sync_children_pass(&rows, tree)`. Apply the identical content-visibility logic:
  - Add the same side queries to `cq_flip_rerun`'s signature: `resolved_lookup: Query<&ResolvedLayout>`, `intrinsic_lookup: Query<&ContainIntrinsicSize>`, `content_vis_margin: Res<ContentVisibilityMargin>`. (No `warned` resource here — `cq_flip_rerun` has no warn resource, and the D6 diagnostic already fired in `sync_styles` this frame; do NOT re-warn.)
  - Before the translate loop, compute `skip_children` + `sentinel_size` over the FULL tree the re-run touches. The re-run's `nodes` is `Changed`-filtered, but children-detach needs every parent's skip status — iterate the unfiltered `roots` query (which `cq_flip_rerun` already has, `systems.rs:2390`) for the skip classification, reading `containment` via a new read-only `Query<&Containment>` param (add `containment_lookup: Query<&Containment>`), OR — simpler and consistent — compute `skip_children`/`sentinel_size` from a dedicated unfiltered pass mirroring `sync_styles`. Concretely:
    ```rust
    let expanded_viewport = viewport_rect(viewport_size, content_vis_margin.0);
    let mut skip_children: HashSet<Entity> = HashSet::new();
    let mut sentinel_size: HashMap<Entity, bevy::math::Vec2> = HashMap::new();
    for (entity, _children, _parent, _position) in roots.iter() {
        let Ok(containment) = containment_lookup.get(entity) else { continue; };
        let off_screen = is_off_screen(resolved_lookup.get(entity).ok(), expanded_viewport);
        match content_visibility_skip(containment, intrinsic_lookup.get(entity).ok(), off_screen) {
            SkipKind::None => {}
            SkipKind::AutoSentinel { intrinsic } => {
                skip_children.insert(entity);
                sentinel_size.insert(
                    entity,
                    bevy::math::Vec2::new(intrinsic.width.unwrap_or(0.0), intrinsic.height.unwrap_or(0.0)),
                );
            }
            SkipKind::HiddenPrune => { skip_children.insert(entity); }
        }
    }
    ```
    **Implementer note:** `roots` is `Query<(Entity, Option<&Children>, Option<&ChildOf>, &Position), With<Node>>` (`systems.rs:2390`) — it already covers the full tree (it is the unfiltered query used for children-sync + compute). It does NOT bind `Containment`, so add `containment_lookup: Query<&Containment>` as a new read-only param. (Confirm no query-conflict: it is read-only and disjoint from the `&mut`-free `nodes`; Bevy 0.18 allows overlapping read-only queries.)
  - In the translate loop, pass the sentinel: `translate_one_entity(item, &parent_areas_for, &container_index, &cq_parent_chain, viewport_size, sentinel_size.get(&entity).copied(), tree)`. (`item.0` is the entity; bind it or use the tuple's first field.)
  - Change the `sync_children_pass(&rows, tree)` call (`systems.rs:2456`) to `sync_children_pass(&rows, &skip_children, tree)`.

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core --test layout_content_visibility skip_survives_container_query_flip
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_content_visibility.rs
  git commit -m "feat(layout): cq_flip_rerun honors content-visibility skip (Phase 11 — D8)

The same-frame CQ-flip re-run reproduces the sentinel size + descendant detach
so a flip frame cannot re-lay-out a skipped subtree. No re-warn (sync_styles owns
the D6 diagnostic this frame)."
  ```

### Task 7: Rewrite the obsolete `layout_containment.rs` deferred-warn tests

**Spec:** § 5.2, D6.

**Files:**
- Modify: `crates/buiy_core/tests/layout_containment.rs` (replace the three tests that assert the removed blanket warn)

- [ ] **Step 1: Replace the obsolete tests.** The blanket `ContentVisibilityDeferred` warn is gone (T5), so these three tests are now wrong and will FAIL after T5:
  - `content_visibility_auto_warns_once` (`layout_containment.rs:44-65`)
  - `content_visibility_hidden_also_warns` (`:67-88`)
  - `content_visibility_deferred_warns_once_per_entity_across_three` (`:121-170`)
  Replace ALL THREE with tests of the new D6 semantics. The replacements:
  ```rust
  #[test]
  fn content_visibility_hidden_does_not_warn() {
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
      // Hidden is fully implemented (descendants detached) — no warn (D6).
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert!(
          !warned
              .set
              .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e)),
          "content-visibility: hidden is implemented, not deferred"
      );
  }

  #[test]
  fn content_visibility_auto_on_screen_does_not_warn() {
      use buiy_core::layout::ContentVisibility;
      let mut app = app();
      // No window → viewport ZERO, but the entity has no last-frame layout on
      // frame 1 and resolves at/near the origin (on-screen within the +margin).
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(10.0).height_px(10.0).containment(Containment {
                  content_visibility: ContentVisibility::Auto,
                  ..Default::default()
              }),
          ))
          .id();
      let _root = app.world_mut().spawn((Node, Style::default())).add_child(e).id();
      app.update();
      app.update();
      // On-screen Auto (no hint needed) → no skip, no warn (D6).
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert!(
          !warned
              .set
              .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e)),
          "on-screen content-visibility: auto does not warn"
      );
  }

  #[test]
  fn content_visibility_auto_off_screen_without_hint_warns_once() {
      use bevy::window::{PrimaryWindow, Window, WindowResolution};
      use buiy_core::layout::{ContentVisibility, Inset, Length, PositionKind, Sizing};
      let mut app = app();
      app.world_mut().spawn((
          Window { resolution: WindowResolution::new(800.0, 600.0), ..Default::default() },
          PrimaryWindow,
      ));
      // Auto + off-screen + NO contain-intrinsic-size hint → D6 diagnostic.
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .width_px(10.0)
                  .height_px(10.0)
                  .position(PositionKind::Absolute)
                  .inset(Inset {
                      left: Sizing::Length(Length::px(5000.0)),
                      ..Default::default()
                  })
                  .containment(Containment {
                      content_visibility: ContentVisibility::Auto,
                      ..Default::default()
                  }),
          ))
          .id();
      let _root = app.world_mut().spawn((Node, Style::default())).add_child(e).id();
      app.update(); // establishes off-screen geometry
      app.update(); // frame 2 sees last-frame off-screen → D6 warn
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert!(
          warned
              .set
              .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e)),
          "off-screen auto without contain-intrinsic-size warns (D6 repurposed)"
      );
      // dedup: a third frame does not add a duplicate.
      app.update();
      let count = app
          .world()
          .resource::<LayoutWarnedOnceSession>()
          .set
          .iter()
          .filter(|k| matches!(k, LayoutWarnOnceKey::ContentVisibilityDeferred(_)))
          .count();
      assert_eq!(count, 1, "one D6 warn per entity, deduped across frames");
  }
  ```
  **Implementer note:** keep the `will_change_does_not_warn` test (`layout_containment.rs:90-119`) — it stays valid (it asserts neither `ContentVisibilityDeferred` nor `SizeContainmentZeroed` fires for a will-change-only entity; content_visibility is `Visible` there, so still no warn). Confirm the imports at the top of `layout_containment.rs` cover the new uses (`Inset`, `Length`, `PositionKind`, `Sizing`, window types) — add them.

- [ ] **Step 2: Run.**
  ```bash
  cargo test -p buiy_core --test layout_containment
  ```
  Expected PASS (the rewritten tests + the surviving ones).

- [ ] **Step 3: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/tests/layout_containment.rs
  git commit -m "test(layout): retire deferred content-visibility warn tests (Phase 11 — D6)

Hidden no longer warns (implemented); on-screen auto no longer warns; off-screen
auto WITHOUT a contain-intrinsic-size hint warns once (repurposed
ContentVisibilityDeferred). Matches the un-stubbed § 5.2 behavior."
  ```

### Task 8: `ContainIntrinsicSize` `Style` field + `contain_intrinsic_size` setter

**Spec:** § 5.2, D1.

**Files:**
- Modify: `crates/buiy_core/src/layout/style.rs` (add `contain_intrinsic_size: ContainIntrinsicSize` field + setter)

- [ ] **Step 1: Failing test.** Add to `style.rs::mod tests`:
  ```rust
  #[test]
  fn style_contain_intrinsic_size_setter() {
      let s = Style::default().contain_intrinsic_size(Some(120.0), Some(40.0));
      assert_eq!(s.contain_intrinsic_size.width, Some(120.0));
      assert_eq!(s.contain_intrinsic_size.height, Some(40.0));
  }

  #[test]
  fn style_contain_intrinsic_size_default_is_none() {
      let s = Style::default();
      assert_eq!(s.contain_intrinsic_size, ContainIntrinsicSize::default());
  }

  #[test]
  fn style_contain_intrinsic_size_single_axis() {
      let s = Style::default().contain_intrinsic_size(Some(80.0), None);
      assert_eq!(s.contain_intrinsic_size.width, Some(80.0));
      assert_eq!(s.contain_intrinsic_size.height, None);
  }
  ```
  Run: `cargo test -p buiy_core style_contain_intrinsic_size` — expected FAIL.

- [ ] **Step 2: Add the field + setter.**
  - In `crates/buiy_core/src/layout/style.rs`, add `pub contain_intrinsic_size: ContainIntrinsicSize,` to the `Style` struct (after `stacking: Stacking,`, `style.rs:61`).
  - Add `ContainIntrinsicSize` to the `use super::components::{ … }` block (`style.rs:15-18`).
  - Add the setter (near the `containment` / `contain` setters, `style.rs:513-525`):
    ```rust
    /// Set `contain-intrinsic-size` (spec § 5.2) — the placeholder size
    /// (logical px, per axis; `None` = no hint) used when a
    /// `content-visibility: auto` subtree is skipped off-screen. Without
    /// it, the off-screen Taffy skip is disabled (the engine would have to
    /// lay the subtree out to learn its size).
    pub fn contain_intrinsic_size(mut self, width: Option<f32>, height: Option<f32>) -> Self {
        self.contain_intrinsic_size = ContainIntrinsicSize { width, height };
        self
    }
    ```
  **Implementer note:** `Style` derives `Bundle`, so `ContainIntrinsicSize` is inserted on every spawn at its both-`None` default. That is intended (matches `Containment` / `Stacking` always-present fields, D1) — a both-`None` `ContainIntrinsicSize` never enables a skip (`has_hint()` is false), so the default is inert.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core style_contain_intrinsic_size
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/style.rs
  git commit -m "feat(layout): ContainIntrinsicSize Style field + contain_intrinsic_size setter (Phase 11 — D1)"
  ```

### Task 9: Register type + facade re-exports

**Spec:** § 5.2.

**Files:**
- Modify: `crates/buiy_core/src/layout/mod.rs` (`register_type` + `pub use`)
- Modify: `crates/buiy/src/lib.rs` (top-level facade re-export)

- [ ] **Step 1: Failing test.** Add to `tests/layout_content_visibility.rs`:
  ```rust
  #[test]
  fn phase11_types_are_registered() {
      let mut app = app();
      app.update();
      let registry = app.world().resource::<AppTypeRegistry>().read();
      assert!(
          registry
              .get_with_type_path("buiy_core::layout::components::ContainIntrinsicSize")
              .is_some(),
          "ContainIntrinsicSize not registered"
      );
  }
  ```
  **Implementer note:** confirm the exact `type_path` via `std::any::type_name::<ContainIntrinsicSize>()` if the assert fails. `AppTypeRegistry` is in `bevy::prelude`.
  Run: `cargo test -p buiy_core --test layout_content_visibility phase11_types_are_registered` — expected FAIL.

- [ ] **Step 2: Register + re-export.**
  - In `crates/buiy_core/src/layout/mod.rs`, append to the "Phase 9 — stacking + top layer" group of the `register_type` chain (after `.register_type::<crate::components::StackingContext>()`, `mod.rs:185`) a new group, and re-terminate the chain with `;`:
    ```rust
            // Phase 9 — stacking + top layer.
            .register_type::<Stacking>()
            .register_type::<ZIndex>()
            .register_type::<Isolation>()
            .register_type::<TopLayer>()
            .register_type::<crate::components::StackingContext>()
            // Phase 11 — content-visibility enforcement.
            .register_type::<ContainIntrinsicSize>();
    ```
    (i.e. drop the `;` after `StackingContext` and add the `ContainIntrinsicSize` line ending with `;`.)
  - In `mod.rs`, add `ContainIntrinsicSize` to the `pub use components::{ … }` block (`mod.rs:13-18`) and `ContentVisibilityMargin` to the `pub use systems::{ … }` block (`mod.rs:21-24`).
  - In `crates/buiy/src/lib.rs`, re-export `ContainIntrinsicSize` (and `ContentVisibilityMargin`) from the top-level facade, mirroring how the Phase-9 `Stacking` / `TopLayerActivation` are re-exported. **Implementer note:** grep `crates/buiy/src/lib.rs` for `Stacking` to find the exact re-export block + style; add the two names alongside.
  **Implementer note:** `ContentVisibilityMargin` is a `Resource` (not `Component`) — it is NOT `register_type`'d (resources are not reflected in this codebase unless needed; `PostTaffyPositionOverrides` etc. are not registered). Only `ContainIntrinsicSize` (a `Component`) is registered.

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core --test layout_content_visibility phase11_types_are_registered
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/mod.rs crates/buiy/src/lib.rs
  git commit -m "feat(layout): register + re-export ContainIntrinsicSize / ContentVisibilityMargin (Phase 11)"
  ```

### Task 10: Integration tests (spec § 5.2 + § 7 surface)

**Spec:** § 5.2 (Auto skip + snap-back, Hidden), § 7 (`content-visibility: auto skips off-screen` test bullet).

**Files:**
- Modify: `crates/buiy_core/tests/layout_content_visibility.rs` (extend with the remaining § 5.2 / § 7 fixtures)

- [ ] **Step 1: Add the remaining fixtures.** Append to `tests/layout_content_visibility.rs` (the file created in T5). Cover snap-back (the explicit hysteresis behavior the charter demanded) + the spec § 7 "auto skips off-screen" assertion in its strongest form + the margin resource:
  ```rust
  #[test]
  fn auto_snaps_back_on_screen() {
      // spec § 5.2: snaps back to full layout when on-screen.
      let mut app = app();
      with_window(&mut app, 800.0, 600.0);
      let child = app
          .world_mut()
          .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
          .id();
      let auto = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(buiy_core::layout::PositionKind::Absolute)
                  .inset(buiy_core::layout::Inset {
                      left: Sizing::Length(Length::px(5000.0)),
                      ..Default::default()
                  })
                  .containment(Containment {
                      content_visibility: ContentVisibility::Auto,
                      ..Default::default()
                  })
                  .contain_intrinsic_size(Some(120.0), Some(40.0)),
          ))
          .add_child(child)
          .id();
      let _root = app.world_mut().spawn((Node, Style::default())).add_child(auto).id();
      app.update();
      app.update();
      // skipped (sentinel) while off-screen.
      assert_eq!(
          app.world().get::<ResolvedLayout>(auto).unwrap().size,
          Vec2::new(120.0, 40.0)
      );
      // Move it on-screen.
      {
          let mut e = app.world_mut().entity_mut(auto);
          let mut pos = e.get_mut::<buiy_core::layout::Position>().unwrap();
          pos.inset.left = Sizing::Length(Length::px(10.0));
      }
      app.update();
      app.update();
      // Snapped back: child is laid out again (50x50), and the auto box is no
      // longer the sentinel (it sizes to its content / explicit size).
      let cr = app.world().get::<ResolvedLayout>(child).expect("child re-laid-out");
      assert_eq!(cr.size, Vec2::new(50.0, 50.0), "descendant snaps back on-screen");
  }

  #[test]
  fn auto_skips_off_screen_spec_section_7() {
      // spec § 7: "content-visibility: auto skips off-screen — tall scroll
      // container, off-screen child has ContentVisibility::Auto; assert child
      // is not in step 1's translation set when off-screen." We assert the
      // observable consequence: the off-screen auto child's descendants are
      // detached (not laid out) while an identical on-screen sibling's are not.
      let mut app = app();
      with_window(&mut app, 800.0, 600.0);

      let mk_auto = |app: &mut App, left: f32| -> (Entity, Entity) {
          let grandchild = app
              .world_mut()
              .spawn((Node, Style::default().width_px(30.0).height_px(30.0)))
              .id();
          let auto = app
              .world_mut()
              .spawn((
                  Node,
                  Style::default()
                      .position(buiy_core::layout::PositionKind::Absolute)
                      .inset(buiy_core::layout::Inset {
                          left: Sizing::Length(Length::px(left)),
                          ..Default::default()
                      })
                      .containment(Containment {
                          content_visibility: ContentVisibility::Auto,
                          ..Default::default()
                      })
                      .contain_intrinsic_size(Some(60.0), Some(60.0)),
              ))
              .add_child(grandchild)
              .id();
          (auto, grandchild)
      };
      let (off_auto, off_gc) = mk_auto(&mut app, 5000.0); // off-screen
      let (on_auto, on_gc) = mk_auto(&mut app, 10.0); // on-screen
      let _root = app
          .world_mut()
          .spawn((Node, Style::default()))
          .add_children(&[off_auto, on_auto])
          .id();
      app.update();
      app.update();

      // Off-screen: sentinel size + detached grandchild.
      assert_eq!(
          app.world().get::<ResolvedLayout>(off_auto).unwrap().size,
          Vec2::new(60.0, 60.0),
          "off-screen auto uses the sentinel"
      );
      assert!(
          app.world()
              .get::<ResolvedLayout>(off_gc)
              .map(|r| r.size == Vec2::ZERO)
              .unwrap_or(true),
          "off-screen auto's descendant is not laid out"
      );
      // On-screen: descendant laid out at its real size.
      assert_eq!(
          app.world().get::<ResolvedLayout>(on_gc).unwrap().size,
          Vec2::new(30.0, 30.0),
          "on-screen auto lays out its descendant"
      );
  }

  #[test]
  fn margin_resource_controls_hysteresis_band() {
      use buiy_core::layout::ContentVisibilityMargin;
      let mut app = app();
      with_window(&mut app, 800.0, 600.0);
      // Shrink the margin to 0 so an entity just past the viewport edge skips.
      app.insert_resource(ContentVisibilityMargin(0.0));
      let child = app
          .world_mut()
          .spawn((Node, Style::default().width_px(20.0).height_px(20.0)))
          .id();
      let auto = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(buiy_core::layout::PositionKind::Absolute)
                  .inset(buiy_core::layout::Inset {
                      left: Sizing::Length(Length::px(900.0)), // past 800, margin 0 → off-screen
                      ..Default::default()
                  })
                  .containment(Containment {
                      content_visibility: ContentVisibility::Auto,
                      ..Default::default()
                  })
                  .contain_intrinsic_size(Some(50.0), Some(50.0)),
          ))
          .add_child(child)
          .id();
      let _root = app.world_mut().spawn((Node, Style::default())).add_child(auto).id();
      app.update();
      app.update();
      assert_eq!(
          app.world().get::<ResolvedLayout>(auto).unwrap().size,
          Vec2::new(50.0, 50.0),
          "with margin 0, an entity past the viewport edge skips"
      );
  }
  ```
  **Implementer note:** the off-screen detection relies on the absolutely-positioned auto entity's `ResolvedLayout.position` reflecting its `inset.left` (Phase 10 `Absolute`/`Fixed` resolves insets against the containing block). Confirm `Position { kind, inset, .. }` is the field shape (`get_mut::<Position>().inset.left`) — adjust the snap-back mutation if the field path differs. `add_children(&[..])` orders children. The § 7 spec test phrasing ("not in step 1's translation set") is asserted via its *observable* consequence (detached descendant) rather than instrumenting the internal translation set, matching how the other layout suites assert behavior end-to-end.

- [ ] **Step 2: Run.**
  ```bash
  cargo test -p buiy_core --test layout_content_visibility
  ```
  Expected PASS (all fixtures).

- [ ] **Step 3: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/tests/layout_content_visibility.rs
  git commit -m "test(layout): Phase 11 content-visibility integration suite (spec § 5.2, § 7)

Snap-back on-screen, off-screen-skip (§ 7), and ContentVisibilityMargin hysteresis
band coverage."
  ```

---

## Self-review (against the spec)

**Spec coverage** (`transforms-and-containment.md § 5.2`):
- "Skips paint when fully outside the viewport" → paint is render-side; Phase 11 owns the *layout* skip. The off-screen *layout* skip (the spec's bullet 2) is implemented. ✓ (paint-skip noted as render concern, D2.)
- "Skips Taffy compute on descendants when both off-screen AND contain-intrinsic-size is set" → `content_visibility_skip` `AutoSentinel` requires both (T2, D2); `ContainIntrinsicSize` component (T1); descendants detached (T5, D4). ✓
- "Snaps back to full layout + paint when on-screen" → snap-back via stateless per-frame reclassification (T5) + `auto_snaps_back_on_screen` fixture (T10). ✓
- "during step 1, check Auto + off-screen (last frame's ResolvedLayout); Taffy receives a sentinel size and descendants' style sync is no-op" → in `sync_styles` step 1 (T5); sentinel via `StyleView` override (T4); descendant detach (T5). ✓
- "Hidden is harsher — equivalent to Display::None for descendants, doesn't snap back unless toggled" → `HiddenPrune` detaches descendants (T5, D7); entity box still resolves (D7); toggles back via reclassification (T5). ✓
- § 5 `ContentVisibility` table semantics (`Hidden` = descendants only) → D7. ✓
- § 7 "content-visibility: auto skips off-screen" test → `auto_skips_off_screen_spec_section_7` (T10). ✓
- Charter "off-screen check must not thrash; define hysteresis" → `ContentVisibilityMargin` single-expanded-rect dead-band (D3, T3); `margin_resource_controls_hysteresis_band` + `box_within_margin_is_still_on_screen_hysteresis` (T3/T10). ✓
- Charter "remove/repurpose the ContentVisibilityDeferred warn" → blanket warn removed, variant repurposed for off-screen-auto-without-hint (D6, T5, T7). ✓
- Charter "needs a NEW contain-intrinsic-size component" → `ContainIntrinsicSize` (T1, T8). ✓
- D8 `cq_flip_rerun` parity (no re-entrancy correctness) → T6. ✓

**Placeholder scan:** every code step shows full code. T4 Step 4 + T5 Step 2e use explicit `None` / `&HashSet::new()` placeholders that are *named and removed by a later task* (T5 populates the T4 `None`; T6 populates the T5 empty-set in `cq_flip_rerun`) — each is flagged inline as mandatory follow-through, not a silent stub. No "TBD", no "similar to Task N", no "add error handling".

**Type consistency:** `ContainIntrinsicSize { width: Option<f32>, height: Option<f32> }` + `has_hint()` (T1) used in T2/T5/T6/T8; `SkipKind::{None, AutoSentinel { intrinsic }, HiddenPrune }` + `content_visibility_skip(&Containment, Option<&ContainIntrinsicSize>, bool) -> SkipKind` (T2) used in T5/T6; `viewport_rect(Vec2, f32) -> Rect` + `is_off_screen(Option<&ResolvedLayout>, Rect) -> bool` (T3) used in T5/T6; `StyleView.content_visibility_intrinsic: Option<Vec2>` (T4) set in T5/T6 via the new `translate_one_entity` param; `ContentVisibilityMargin(pub f32)` resource (T5) init in T5/mod.rs + read in T5/T6 + re-exported in T9; `sync_children_pass(rows, skip_children, tree)` / `sync_children_for_entity(entity, children, fixed_set, skip_children, tree)` extended signatures (T5) called in T5/T6. `LayoutWarnOnceKey::ContentVisibilityDeferred(Entity)` repurposed (T5) asserted in T7. All consistent across T1–T10.
