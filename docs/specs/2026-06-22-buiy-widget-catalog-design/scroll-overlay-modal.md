# Scroll / Overlay / Modal + Focus-Scope containers — child C5 of the widget-catalog campaign

`2026-06-22` · `[draft]` · Wave 4 · realizes foundation `interaction.md §3.7` (wheel/scroll, event flow), `accessibility.md` (focus-trap/restoration/roving §3.11 — the *traversal geometry*; the inert/live-region/announcer *a11y semantics* are owned by the agent-interface campaign, see "## Coordination"), `media-and-widgets.md §3.10` (Dialog/Popover/Menu/Tooltip — the *positioning + container* layer), `visuals.md §3.2` (overflow + true top-layer) · depends on C0, C1, C3, C4, C7, **and the agent-interface campaign's P1a/P1b/P1d**

> **Scope discipline.** This child owns the **container-and-traversal geometry** the gallery needs — scroll input routing + scroll container, overlay positioning + light-dismiss, the modal focus-trap *traversal* (scoped `compute_next_focus`) + focus restoration, and the `Hidden` (resolved-display) override that supplies the geometry predicate for filter visibility. It does **not** own: the `Pointer<E>` taxonomy and the stacking-aware `emit_picks`/`hit_test` (C3 defines; this consumes and supplies the inert/hidden *exclusion predicates*); the coordinate-space/`GlobalTransform` fix (C1 defines; this depends on it for overlay picking correctness); the leaf widget-state components `A11yToggled`/`A11ySelected`/`A11yExpanded` (the **agent-interface campaign** owns; this consumes for menu/option *visual* state); the box-shadow/border/outline paint (C6 wires; overlays *use* shadows but this child paints nothing); and — newly ceded by the 2026-06-22 coordination decision — **the Dialog/AlertDialog/Menu/MenuItem/Tooltip/Disclosure a11y contracts + roles, the `Inert`/`A11yHidden` a11y-prune marker, the live-region/`Announcer`, and roving/`aria-activedescendant` *lowering*** (the **agent-interface campaign** owns all of these; this child *consumes* them and *supplies* the focus-traversal + hit-test exclusion predicates + populates the modal/active-descendant/live fields for its containers). Per umbrella §6 + §2.7, shared contracts are **referenced**, not redefined.

This document spells the three slices A (scroll), B (overlay), C (modal/focus) as one spec with explicit slice headers. §7 holds open the option to split into A/B/C sub-children if any slice proves too large to implement as one unit; the recommendation (§3.7) is to keep it as one document and stage the *implementation* A→B→C.

---

## 1. Problem & current state

### 1.1 Scroll — the entire downstream pipeline is built and idle; no producer exists

The `ScrollOffset → bridge → GlobalTransform → render/clip/paint-skip` chain is fully wired. What is missing is the one upstream system that writes `ScrollOffset`.

- `ScrollOffset{x: f32, y: f32}` is defined, `Reflect`+`Default`, with the load-bearing invariant *"Mutating `ScrollOffset` must NOT invalidate `ResolvedLayout`"* documented and enforced (`crates/buiy_core/src/layout/components.rs:509-526`; test `tests/layout_scroll_offset_no_invalidate.rs`). Its doc comment says *"Mutated by the scroll-input handler in `buiy-input-events-design`"* — **that spec/handler does not exist.**
- `Overflow{x, y, scrollbar_*, scroll_behavior, overscroll_x, overscroll_y}` (`components.rs:162-186`) + `OverflowMode{Visible,Hidden,Clip,Scroll,Auto}` (`types.rs:317-325`) + `Overflow::is_scroll_container()` (`components.rs:182-185`, true iff either axis is `Scroll`/`Auto`) are all defined. `OverscrollBehavior{Auto,Contain,None}` (`types.rs:375-381`) and `ScrollBehavior{Auto,Smooth}` (`types.rs:366-371`) are stored-only with doc comments deferring honoring to *"buiy-input-events-design's scroll handler"* / *"`BuiySet::Animate`"*.
- The bridge **already folds** `ScrollOffset` into `GlobalTransform`: `write_buiy_transform`'s `walk` computes `base = from_translation(position − acc)` and a scroll-container pushes `child_acc = acc + (off.x, off.y)` to descendants (`crates/buiy_core/src/render/bridge.rs:135-163`); `seed_scroll_dirty` reacts to `Changed<ScrollOffset>` on scroll-containers (`bridge.rs:40-56`). So render **and** picking inherit scroll for free the instant `ScrollOffset` is written — provided picking reads `GlobalTransform` (the C1 fix).
- **Missing:** any `MouseWheel`/`Pointer<Scroll>` consumer (grep over core+widgets = zero, audit §5 / report line 42, 162). No `scroll_to`. No smooth-scroll system. No scroll-container widget. No bounds-clamping. A long list is unscrollable today.
- **Virtualization substrate is already built (Phase 11).** `ContentVisibility{Visible,Auto,Hidden}` (`types.rs:1202-1213`) is enforced — off-screen layout skip with a `contain-intrinsic-size` hint, and render off-screen culling via `write_paint_skip → ComputedPaintSkip`/`OffscreenAuto` (`crates/buiy_core/src/render/visibility.rs`), with a 200px hysteresis margin (`ContentVisibilityMargin`, `systems.rs:242-247`). This is the scaling relief valve, not a stub.

### 1.2 `apply_filter`-owns-`Display` (audit W15) — desyncs `FlexParams.direction`

The prototype's `apply_filter` rewrote `Display` directly to hide rows. The audit (report line 151) found `Display` and `FlexParams.direction` are **decoupled components** (`style.rs:73-77`), so a direct `Display` rewrite desyncs *direction*, not just non-flex rows, and stomps the author's `Display`. There is a clean render-owned alternative already on `main`: `CssVisibility::Hidden` (`crates/buiy_core/src/render/components.rs:337-345, 440-455`) is a paint-skip that **keeps the layout box and a11y presence** (explicitly contrasted with `Display::None` in its doc). The gallery's filter/hide needs a `Hidden` marker leaving `Display`+`FlexParams` intact.

### 1.3 Overlay/menu picking — paints correctly, picks wrong

Top-layer is built (Phase 9): `Stacking{z_index, isolation, top_layer}` (`components.rs:456-462`), `TopLayer{None,Modal,Popover,Tooltip,Fullscreen}` (`types.rs:1298-1306`), `StackingContext.painters_z` (`components.rs:103-123`), `TopLayerActivation{order: VecDeque<Entity>}` (`systems.rs:266-269`), and the stable render partition `partition_top_layer` (`crates/buiy_core/src/render/top_layer.rs`). So an overlay **paints** on top correctly today. But picking ranks by **smallest area** with a **placeholder camera**:

- `emit_picks` (`crates/buiy_core/src/picking/backend.rs:27-76`) collects every node under the cursor, sorts by `area` ascending ("smallest = top"), and emits `HitData::new(Entity::PLACEHOLDER, area_rank, …)`. Pick-order is **not** paint-order. A small node *behind* a large overlay still picks first.
- `hit_test` (`crates/buiy_core/src/picking/mod.rs:37-49`) is the same smallest-area model.
- There is no `Pointer<E>`/`Pickable`/bubbling consumption; only a single `Hovered` resource (`mod.rs:20-30`). No light-dismiss channel.

This is the C3-owned shared pick-depth rewrite (umbrella §6.1). This child **consumes** `painters_z` pick-depth for overlay correctness and light-dismiss interception; it does not own the `emit_picks` rewrite.

### 1.4 Focus / modal / live-region — flat-global, no trap, no inert, no announcer

- `compute_next_focus` (`crates/buiy_core/src/focus.rs:85-110`) iterates the **global** `Focusable` set with **no scope, no trap, no container boundary**. Tab order tiebreaks on `entity.index()` — a documented Phase-0 deferral (`focus.rs:9-15`) that is wrong under despawn/respawn (audit W16 / report line 153). `FocusVisible` is set true on Tab and never reset (`focus.rs:16-19`, `handle_tab:72`). **C5 owns the fix** (scoped traversal + document-order tab-order, §C.1).
- **The a11y substrate for inert/live/activedescendant is built by the agent-interface campaign, not here (2026-06-22 coordination).** The agent-interface campaign's P1b prunes `A11yHidden` + inert subtrees from the AccessKit tree (semantic-tree.md §7.4); P1a adds `A11yLive { politeness, atomic }` + role-implied live (`resolve_live`, semantic-tree.md §5) and `A11yRelations.active_descendant` (semantic-tree.md §3). C5 does **not** define a competing `Inert`/`Announcer`/`LiveRegion`/`Roving`-lowering — it *consumes* those and *supplies* the focus-traversal + hit-test exclusion predicates and *populates* the live/active-descendant/modal fields for its containers. The remaining as-built gaps C5 still owns:
- **No focus restoration** — closing an overlay does not restore the previously-focused entity. **C5 owns** `FocusReturn` (§C.4).
- **No focus-scope/trap traversal** — there is no scoped `compute_next_focus`. **C5 owns** `FocusScope` + the scoped traversal (§C.1). (The *roving-container intra-navigation* a11y behavior + `aria-activedescendant` *lowering* is the agent-interface campaign's consumer-side APG keyboard + relation lowering; C5 supplies the focus-scope geometry it sits in.)

### 1.5 Widgets present: button + text_input only

`crates/buiy_widgets/src/` = `button.rs`, `text_input.rs`, `scene.rs`, `lib.rs`. No scroll container, scrollbar, menu, popover, dialog, tooltip, listbox. All greenfield, re-derived on bevy 0.19.0-rc.3 / accesskit 0.24.

---

## 2. Target design

The system order is fixed: `Layout → Style → Input → Animate → Picking → A11yUpdate → Render` (`crates/buiy_core/src/lib.rs:62-92`). Scroll input, focus traversal, and light-dismiss all run in `BuiySet::Input`; smooth-scroll interpolation runs in `BuiySet::Animate`. The a11y lowering (active-descendant / inert-prune / live) runs in `BuiySet::A11yUpdate` and is **owned by the agent-interface campaign** (its `build_tree`/`to_accesskit_node` fold); C5 *populates* the source components and *supplies* the inert/hidden exclusion predicates. Inert/hidden hit-test exclusion is enforced inside C3's `emit_picks` in `BuiySet::Picking` (this child contributes the predicate; the marker is the agent-interface campaign's `A11yHidden`/inert).

### 2.1 Crate / module placement

Per umbrella §7 (state types in `buiy_core` so lowering+verify co-locate; widget markers+behavior in `buiy_widgets`):

- **`buiy_core`** gets (C5-owned): the scroll input pipeline (a `ScrollInputPlugin` — the canonical home named by the layout spec as `buiy-input-events-design`, realized here, see §3.6), the generalized `compute_next_focus` + `FocusScope` (in `focus.rs`), the **focus-traversal + hit-test exclusion predicates** that read the agent-interface campaign's `A11yHidden`/inert markers, the `Hidden` (resolved-display override) marker, and `FocusReturn`.
- **Owned by the agent-interface campaign, NOT here** (C5 consumes/populates): the `A11yHidden`/inert marker + the AccessKit-tree prune (semantic-tree.md §7.4); the `A11yLive`/`Announcer`/live-region lowering (P1a `resolve_live` + role-implied live); `A11yRelations.active_descendant` + roving's `aria-activedescendant` lowering; the `A11yNodeView` state fields (already widened by P1a). C5 must **not** add a competing marker/resource/field for any of these.
- **`buiy_widgets`** gets (the meeting point — coordinated, sequenced after the agent-interface P1d bundle for each widget): `ScrollArea`, `Popover` (positioning primitive), and the **container/positioning + focus-scope + rendering layer** of `MenuPopup`/`MenuItem`/`MenuButton`, `Tooltip`, `Dialog`/`AlertDialog`. The agent-interface campaign's P1d owns each widget's **bundle + `A11yContract` + APG keyboard + role + a11y state**; C5 adds the **positioning (`Popover`), `Stacking`/`TopLayer` membership, `FocusScope` trap, light-dismiss wiring, focus restoration, and visible rendering** on top. Coordinate per-widget so neither campaign re-builds the other's layer.

This child does **not** create standalone empty `buiy-input-events-design` / `buiy-focus-model-design` spec stubs (umbrella §7: "the C-children *are* the realization"). It is the realization of those roadmap rows; §5 flips the layout-spec deferral notes to point here.

---

### SLICE A — Scroll input + scrollable container

#### A.1 `ScrollInputPlugin` — `Pointer<Scroll>` → clamped `ScrollOffset`

A single observer-based system in `buiy_core`, consuming C3's `Pointer<Scroll>` (umbrella §6.3: C3 owns the *entry*, C5 owns nearest-container routing + clamp + overscroll). `Pointer<Scroll>` carries `{unit: MouseScrollUnit, x: f32, y: f32, hit, phase}` (bevy_picking 0.19.0-rc.3 `events.rs`), fed from winit `MouseWheel`. Because C3 propagates `Pointer<E>` capture→target→bubble, the observer is attached to scroll-container entities and the event bubbles to the nearest scroll-container ancestor for free — no manual ancestor walk.

```rust
/// buiy_core::scroll — realizes overflow-and-scrolling.md §2's deferred
/// "scroll handler". Registered in BuiySet::Input.
pub struct ScrollInputPlugin;

/// Observer body (attached by the scroll-container widget, or globally
/// filtered to is_scroll_container()):
fn on_scroll(
    ev: On<Pointer<Scroll>>,                 // C3 event type, re-exported via buiy::prelude
    mut q: Query<(&Overflow, &mut ScrollOffset, &ResolvedLayout, &ScrollExtent)>,
    prefs: Res<UserPreferences>,             // reduced-motion gate for inertial deltas
) {
    let target = ev.target();                // nearest scroll-container ancestor (bubbled)
    let Ok((overflow, mut offset, _layout, extent)) = q.get_mut(target) else { return };
    let delta = normalize_delta(ev.unit, ev.x, ev.y);  // Line → line_height*k, Pixel → as-is
    let new = clamp_to_extent(*offset + delta, extent, overflow);
    // overscroll Contain/None: if new == clamped at a bound AND the axis is
    // Contain/None, mark the event consumed so it does NOT bubble further
    // (no scroll-chaining). Auto allows the residual to bubble to an
    // outer scroll-container.
    if should_contain(overflow, *offset, new, delta) { ev.stop_propagation(); }
    *offset = new;   // bridge folds into GlobalTransform next frame; no layout invalidation
}
```

- **Unit normalization.** `MouseScrollUnit::Line` → multiply by a line-height constant (default 16px line × a small scroll-step factor `k≈3`, matching CSS `WHEEL_DELTA` conventions); `Pixel` → as-is. Trackpad pixel deltas pass through.
- **Clamp.** `clamp_to_extent` clamps each axis to `[0, max(0, content − viewport)]` once the extent is valid; while the extent is not yet populated (spawn frame) it clamps only the lower bound — see the §A.3 initialization/ordering note for the unknown-extent rule. Content extent comes from `ScrollExtent` (A.3); viewport from the container `ResolvedLayout.size` minus scrollbar gutter. No layout invalidation — only `ScrollOffset` is written (preserves the `components.rs:516` invariant).
- **Overscroll.** `OverscrollBehavior::Contain`/`None` stops propagation when the axis is already at a bound, killing scroll-chaining; `Auto` lets the residual bubble to an outer container. `None` additionally suppresses the OS overscroll affordance (no-op headless).
- **Reduced-motion.** Inertial/animated deltas are short-circuited under `prefers-reduced-motion` (read from the `UserPreferences` resource per `interaction.md:217`); discrete wheel ticks are unaffected.

#### A.2 Keyboard scroll

`ScrollArea` is `Focusable`; when focused, Arrow/PgUp/PgDn/Home/End mutate `ScrollOffset` through the same clamp (`accessibility.md:63` "Home/End, PgUp/PgDn for long lists. F"). PgUp/PgDn = one viewport minus a small overlap; Home/End = top/bottom; Arrow = one line. Runs in `BuiySet::Input`, gated on `FocusedEntity == this ScrollArea` so it does not steal keys from a focused child editor.

#### A.3 `ScrollExtent` — cached content size (no per-frame children union)

```rust
/// Render-prep cache of a scroll-container's content extent (logical px),
/// updated only when the container's ResolvedLayout or any descendant's
/// changes. Decouples clamp from an O(children) per-frame walk and keeps
/// the no-invalidate-on-scroll invariant (it is written by a layout-change
/// reaction, never by the scroll handler).
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct ScrollExtent {
    pub content: Vec2,
    pub viewport: Vec2,
    /// false on Default (spawn frame, before the first layout pass); set true
    /// the first time update_scroll_extent runs with a resolved layout. While
    /// false, clamp_to_extent treats the upper bound as unknown — see the
    /// initialization/ordering note below.
    pub valid: bool,
}
```

Updated by a system seeded on `Changed<ResolvedLayout>` within a scroll-container subtree (the same trigger shape `seed_scroll_dirty` uses). This is the resolution of the §3.2 open question (cached vs per-frame union): **cache**, because the clamp runs every wheel tick and an O(children) union per tick is wasteful, and because computing it in a layout-change reaction keeps it off the scroll write path (invariant-safe). `content = union(children ResolvedLayout boxes) relative to container origin`.

**Initialization / ordering — `ScrollExtent` must be current before the first `Pointer<Scroll>` (mirrors C1's clip-after-bridge edge).** The clamp (`clamp_to_extent`, §A.1) reads `ScrollExtent`; a `Pointer<Scroll>` that arrives before the extent is first populated would otherwise clamp against a zero extent (`content == viewport == 0`), pinning `ScrollOffset` to `0` and silently eating the first wheel ticks on a freshly-spawned `ScrollArea`. Two coordinated guarantees prevent this:

1. **Ordered producer.** `update_scroll_extent` reads `ResolvedLayout` and is scheduled `.after(BuiySet::Layout)` and `.before(BuiySet::Input)` — so within any frame where layout resolved, the extent is recomputed **before** `on_scroll` (which runs in `BuiySet::Input`) can consume a wheel event. This is the same ordering discipline C1 adds for `write_clip_rects.after(sync_simple_transforms).before(BuiySet::Picking)` (coordinate-space-correctness.md D3): a derived cache must be refreshed before its consumer reads it. The `Default` `ScrollExtent` (all-zero) is therefore only ever observed in the spawn frame *before* the first layout pass.
2. **Defensive clamp against an unknown extent (covers the spawn-frame race + multi-frame layout).** `ScrollExtent` carries a `valid: bool` (false on `Default`, set true the first time `update_scroll_extent` runs with a resolved layout). While `valid == false`, `clamp_to_extent` treats the extent as **unknown** and clamps only to the lower bound (`offset.max(0.0)` per axis) — it does **not** clamp the upper bound to a not-yet-known zero content size. This is the "unknown-extent sentinel, not a zero extent" rule: a wheel tick in the spawn frame accumulates into `ScrollOffset` and is re-clamped correctly the next frame once the real extent lands, rather than being clamped to `0` and lost. (`ScrollArea` spawns with `ScrollExtent::default()` via `#[require]`, §A.4; the first layout pass seeds the real extent and flips `valid`.)

#### A.4 `ScrollArea` widget (`buiy_widgets`)

```rust
#[derive(Component, Reflect, Default, Clone, Debug)]
#[require(
    Node,
    Overflow = scroll_area_overflow(),       // { y: Auto, .. } by default; axis configurable
    ScrollOffset,
    ScrollExtent,
    Focusable,                                // the CONTAINER owns focus + keyboard (prior-art)
    A11yRole = A11yRole::Group,               // scroll-region; a11y on the container
)]
pub struct ScrollArea;
```

Per the bevy-ui-widgets lesson (grounding priorArtLessons #1; widgets.md:112-115): the **container** owns a11y + keyboard focus; the **scrollbar is a pointer-only affordance with no `AccessibilityNode`, no focus, and emits no events** — it directly mutates `ScrollOffset` on the target. The scrollbar widget is **C-tier** (`media-and-widgets.md:90`) and is **deferred** out of this child (the implicit overflow scrollbars are themable but not a widget here). The a11y lowering sets `set_scroll_x/y` + `scroll_*_min/max` on the container node (accesskit 0.24 supports these) and handles the `Action::SetScrollOffset` request (AT-driven scroll) by writing `ScrollOffset`.

#### A.5 `scroll_to` (optional, `BuiySet::Animate`)

```rust
pub fn scroll_to(commands, target: Entity, to: Vec2, behavior: ScrollBehavior);
```

`Auto` writes `ScrollOffset` immediately; `Smooth` spawns a short tween component interpolated in `BuiySet::Animate` (honoring `ScrollBehavior::Smooth` from `Overflow`), short-circuited to instant under reduced-motion. Tween writes only `ScrollOffset` (invariant-safe).

#### A.6 Virtualization posture — **none-with-a-named-ceiling for v1**

v1 builds **no recycling virtual list**. It leans on the already-built `ContentVisibility::Auto` (off-screen layout+paint skip, 200px hysteresis) as the scaling relief valve. The **named ceiling**: one entity + one text buffer + one Taffy node per row; `ContentVisibility::Auto` removes off-screen rows from *layout cost and paint cost* but **not** from entity/buffer count. The documented ceiling is **~1000–2000 rows** before per-entity overhead (buffer allocation, change-detection scan) dominates; C8's 1000-row scale-game fixture (umbrella §5 Wave 5) settles the exact number. A recycling `VirtualList` widget is **deferred to a follow-up widget-catalog slice** (overflow-and-scrolling.md §6 already punts it to `buiy-widget-catalog-design`; freya/gameface confirm virtualization is a *widget* concern above the scroll-container primitive — grounding priorArtLessons #8). Do **not** fold recycling into the scroll-container primitive.

#### A.7 `apply_filter` / hide → `Hidden` marker (audit W15)

The gallery's filter (and TodoMVC's "active/completed" filter) hides rows via a `Hidden` marker, **not** a `Display` rewrite. Reuse the existing render-owned `CssVisibility::Hidden` (`render/components.rs:337-345`) which keeps the layout box and a11y presence — **wrong** for a filtered-out row that should leave layout *and* the a11y tree. So this child specifies the filter uses `Display::None` semantics via a dedicated **`Hidden` marker component** (C5-owned, *geometry*) that, when present, sets the *resolved* display to none **without touching the author's `Display`/`FlexParams`** (a style override at sync time, analogous to `PostTaffyPositionOverrides`), and causes the row to be pruned from the a11y tree. The a11y prune itself is **owned by the agent-interface campaign** (its `A11yHidden`/inert prune, semantic-tree.md §7.4): C5 does **not** edit `build_tree`'s a11y behavior and does **not** define a competing prune marker — it **supplies a `Hidden` (resolved-display-none) predicate to the agent-interface campaign's a11y prune**, which skips those entities in `build_tree` exactly as it skips inert ones. (The cleanest realization: a `Hidden` row also carries / lowers to the agent-interface `A11yHidden` marker so the existing prune does the work; C5 supplies only the *resolved-display geometry* + the predicate, never a parallel a11y marker.) This leaves the author's `Display: Flex(Row)` + direction intact so un-filtering restores exactly. Decision rationale in §3.3.

---

### SLICE B — Overlay / popover / menu / tooltip

#### B.1 Overlay picking correctness (consumes C3 §6.1, C1 §6.2)

Overlay hit-correctness is **not** built here. It is the C3 `emit_picks` rewrite (umbrella §6.1: pick-depth from `painters_z`, derivation = index within nearest ancestor `StackingContext` composed across nested contexts, ECS-tree-order tiebreak) on top of the C1 absolute-`GlobalTransform` coordinate fix (umbrella §6.2). **C1+C3 together deliver the stacking-aware `hit_test`/`emit_picks` that the agent-interface campaign explicitly *deferred* (its follow-up #3) and *depends on* for `HitTargetable` to mean "not obscured"** — see "## Coordination". This child **states the requirement and supplies the regression fixtures** (an overlay over content must intercept the click; §6). C5's only picking contribution is the **inert/hidden hit-test exclusion predicate** (§C.2) that C3's `emit_picks` calls — the marker being the agent-interface campaign's `A11yHidden`/inert, not a C5-defined one.

#### B.2 `Popover` — positioning primitive (not a widget)

Adopt the bevy-ui-widgets shape (grounding priorArtLessons #3; widgets.md:154): `Popover` holds candidate placements; menu/tooltip/anchored-popover compose it. It has **no a11y role of its own** — the wrapping widget supplies the role.

```rust
pub enum PopoverSide { Top, Bottom, Left, Right }
pub enum PopoverAlign { Start, Center, End }
pub struct PopoverPlacement { pub side: PopoverSide, pub align: PopoverAlign, pub gap: f32 }

#[derive(Component, Reflect, Default, Clone, PartialEq)]
pub struct Popover {
    pub anchor: Option<Entity>,        // anchor element; None = positioned by author
    pub positions: Vec<PopoverPlacement>,   // ordered candidates
    pub window_margin: f32,
}
```

`position_popover` runs in `BuiySet::Animate` **after** layout (mirrors bevy-ui-widgets' `PostUpdate`-after-`ui_layout_system`; in Buiy, layout is `BuiySet::Layout`, so this runs in `Animate` reading resolved `GlobalTransform` of the anchor) and picks the first placement that fits the window (minus `window_margin`); falls back to least-bad (Floating-UI-inspired). It writes a `PostTaffyPositionOverrides`-style position override or a `UiTransform`, never invalidating layout. Anchored-popover = `Popover { anchor: Some(e), .. }`. This realizes `visuals.md §3.2` anchored positioning + `media-and-widgets.md:84`.

#### B.3 `MenuPopup` / `MenuItem` / `MenuButton` — the C5 container layer

> **Coordination.** `Menu`/`MenuItem` roles, the `A11yContract` advertising/honoring, the APG `menu` keyboard contract, and roving's `aria-activedescendant` *lowering* are **owned by the agent-interface campaign** (it owns the canonical APG widget bundles). C5 owns the **positioning + stacking + focus-scope + lifecycle + light-dismiss + rendering** container the menu sits in. The bundle below shows **only C5's contribution** (`#[require]`-merged with the agent-interface bundle for the same widget — coordinate so the role/state/contract come from there, the geometry from here).

```rust
// C5's container contribution, composed with the agent-interface campaign's
// Menu bundle (which supplies the role + A11yContract + APG keyboard).
#[derive(Component, Default, Clone)]
#[require(
    Node,
    Stacking = popover_stacking(),         // top_layer: Popover  (C5-owned)
    Popover,                                // composes the positioning primitive (C5-owned)
    FocusScope = FocusScope::trap(),        // §C.1 — modal tab-group inside the open menu (C5-owned)
    // A11yRole::Menu, the Menu A11yContract, and the aria-activedescendant /
    // roving LOWERING come from the agent-interface campaign's Menu bundle —
    // NOT defined here. C5 supplies the FocusScope geometry they sit in.
)]
pub struct MenuPopup { pub layout: MenuLayout }   // Column | Row

// MenuItem's role + Focusable + APG activation come from the agent-interface
// bundle; C5 adds nothing to MenuItem beyond visible rendering.

#[derive(Component, Default, Clone)]      // pairs with Button
pub struct MenuButton;
```

- **Focus-coupled lifecycle** (grounding priorArtLessons #2; widgets.md:134): the menu stays open while a descendant has focus; click-outside is the §B.5 light-dismiss. `MenuFocusState{Opening, Open, Closed}` is the C5-owned lifecycle state machine (the *geometry/visibility* state, distinct from the agent-interface a11y state). `Opening` triggers a **deferred** focus set (because scene-fn spawn may be async — you cannot focus a not-yet-spawned child; the focus is set the frame after spawn) via the shared `PendingFocus` primitive defined in §B.3a. This deferred-focus gotcha is load-bearing for Buiy's scene-fn widgets.

##### B.3a `PendingFocus` — the deferred-focus primitive (shared by MenuPopup §B.3 and Dialog §C.5)

Scene-fn spawn is not synchronous: when a `MenuPopup`/`Dialog` is spawned, its focusable children are queued as commands and do **not** exist in the same frame, so an in-frame "focus the first focusable child" would find nothing. Rather than each overlay re-inventing a frame delay, both reuse **one** primitive:

```rust
/// Request to move focus into a subtree once its focusable content exists.
/// Inserted on the overlay root the frame it opens; drained by
/// resolve_pending_focus in BuiySet::Input, AFTER the spawn flush.
#[derive(Component, Reflect, Clone, Debug)]
pub struct PendingFocus {
    /// Where to put focus: the first focusable descendant, or, if the
    /// overlay itself is the target (e.g. an alert dialog with no focusable
    /// body), the root entity.
    pub target: PendingFocusTarget,   // FirstFocusableChild | SelfRoot(Entity)
    /// Frames remaining before we give up and fall back (spawn budget).
    pub budget: u8,                    // default 4 frames
}
```

- **When it runs.** `resolve_pending_focus` is a system in `BuiySet::Input` scheduled **`.after` the command/spawn flush** (i.e. after `apply_deferred`/the scene-fn spawn has been applied), so the first poll already sees any children spawned in the opening frame. (Equivalently: a one-shot system run `.after` the spawn flush, or a per-frame poll — both observe the same post-flush world; the per-frame poll is what carries the retry budget.)
- **How it detects the first focusable child.** It queries the overlay subtree for the first descendant in document order (the `Children`-walk order, matching §C.1's tab-order definition) that carries `Focusable` and is not inert (the agent-interface campaign's `A11yHidden`/inert marker, §C.2). The "first to appear" is found by querying `Focusable` within the subtree each frame until one exists — no reliance on `Added<Focusable>` edge-detection (which would be missed if the overlay's `PendingFocus` is inserted a frame after the child's spawn); the poll is level-triggered on subtree membership, not edge-triggered on the add.
- **Spawn that spans >1 frame.** If no focusable descendant exists yet (children still spawning, or an async scene-fn), `resolve_pending_focus` **decrements `budget` and retries next frame**. On success it writes `FocusedEntity`, sets `FocusVisible` per the keyboard-vs-pointer origin, and removes `PendingFocus`. If `budget` hits zero with still no focusable child, it falls back to focusing the overlay root (`SelfRoot`) so focus is never stranded outside the trap, removes `PendingFocus`, and logs once (a degenerate overlay with no focusable content).
- **Reuse.** `MenuPopup` (§B.3, `Opening` → `PendingFocus{FirstFocusableChild}`) and `Dialog`/`AlertDialog` (§C.5, open → `PendingFocus{FirstFocusableChild}` or `SelfRoot(dialog)` when the body has none) insert the **same** component and share the **one** `resolve_pending_focus` system. There is exactly one deferred-focus mechanism in C5.
- **Keyboard contract — owned by the agent-interface campaign (APG `menu`).** The Arrow/Home/End intra-navigation, the Enter/Space item activation, and the Escape close are the agent-interface Menu bundle's consumer-side APG keyboard contract; activation flows through the existing `OnPress` / the agent-interface action router (umbrella §2.7 — **no competing `Activate` event**). C5's only contribution to the menu close-on-activate / close-on-Escape behavior is the **focus-restoration on close** (§C.4) and the **light-dismiss** (§B.5); the *which-key-does-what* is not C5's. Submenus deferred (TODO, like prior-art).
- **`MenuButton`** opens the associated `MenuPopup` on activation (via `OnPress` / the agent-interface router). `aria-haspopup`/`aria-expanded` are the agent-interface campaign's `A11yHasPopup`/`A11yExpanded` lowering (P1a/P1d); C5 toggles the menu *visibility* + `TopLayer` membership and reads `A11yExpanded` for *visual* state, it does not own the expanded a11y state.

#### B.4 `Tooltip` — the C5 container layer

> **Coordination.** The `Tooltip` role, the Tooltip-trigger `A11yContract` (`{ShowTooltip, HideTooltip}` + `A11yRelations.described_by=[tooltip]`), and the `aria-describedby` lowering are **owned by the agent-interface campaign** (widget-contracts.md "Tooltip-trigger"). C5 owns the **positioning + stacking + hover/focus + dismiss-timing geometry** the tooltip sits in.

```rust
// C5's container contribution, composed with the agent-interface Tooltip bundle
// (which supplies A11yRole::Tooltip + the described_by relation lowering).
#[derive(Component, Default, Clone)]
#[require(Node, Stacking = tooltip_stacking() /* top_layer: Tooltip */, Popover)]
pub struct Tooltip;
```

Non-interactive, shown on hover **or** focus of its anchor (observes C3 `Pointer<Over>`/`Out` + focus events). WCAG 1.4.13 dismissable/hoverable/persistent (`accessibility.md:121`): Escape dismisses without moving pointer; the tooltip stays while the pointer is over *either* the anchor or the tooltip; it does not auto-hide on a timer. The `aria-describedby` anchor→tooltip link is lowered by the agent-interface campaign's `A11yRelations.described_by`; C5 drives only the *show/hide timing + positioning*.

#### B.5 Light-dismiss (consumes C3 `Pointer<E>` + Escape)

Two channels, both required (grounding priorArtLessons #6 — Pointer light-dismiss AND keyboard Escape):

- **Pointer-outside:** a global observer on `Pointer<Press>` (capture phase) checks whether the press target is inside any open `auto`-dismiss overlay; if not, it closes the top-most light-dismiss overlay and fires `beforetoggle`/`toggle` (`media-and-widgets.md:83`). Uses C3's `painters_z` pick-depth so the "inside/outside" test respects stacking, and C1 absolute coords so the hit is correct. This is the clean path the `Pointer<E>` model enables — see §3.5 for why the focus-change-only fallback is rejected.
- **Escape:** a keyboard handler in `BuiySet::Input` closes the top-most overlay (consulting `TopLayerActivation.order` for "top-most") and restores focus (§C.4).

Light-dismiss honors the `closedby` policy (`any`/`closerequest`/`none`, `media-and-widgets.md:81`): `manual` popovers ignore pointer-outside; `none` modals ignore both (must be explicitly closed).

---

### SLICE C — Modal container + focus-trap *traversal* + focus restoration

> **Coordination.** Slice C is reframed from the pre-coordination draft. **C5 owns** the focus-trap *traversal* (scoped `compute_next_focus` + `FocusScope`), the document-order tab-order fix (W16), focus restoration (`FocusReturn`), and the modal *container* (Stacking/TopLayer membership + the trap geometry). **The agent-interface campaign owns** the `A11yHidden`/inert prune marker (semantic-tree.md §7.4), the Dialog/AlertDialog roles + `A11yModal` + `A11yContract` + labelling/`owns` (widget-contracts.md "Dialog"), the roving `aria-activedescendant` *lowering* + `A11yRelations.active_descendant`, and the `A11yLive`/role-implied-live/announcer substrate (semantic-tree.md §5). C5 *consumes* those, supplies the inert/hidden *exclusion predicates*, and *populates* the `A11yModal`/`active_descendant`/`A11yLive` source for its containers.

This is the **highest-risk** edit (umbrella §9.4, §5 Wave 4): `focus.rs`'s flat-global `compute_next_focus` becomes scope-aware. The WCAG 2.1.2 no-keyboard-trap property test (`accessibility.md:123`) gates it.

#### C.1 Scoped `compute_next_focus` + `FocusScope`

```rust
/// A focus-traversal boundary. Tab/Shift+Tab cycle within the innermost
/// open trap scope; outside any trap, traversal is the global set (today's
/// behavior, preserved for the non-modal case).
#[derive(Component, Reflect, Clone, Debug)]
pub struct FocusScope {
    pub mode: FocusScopeMode,   // Contain (wrap inside, used for non-modal regions) | Trap (modal)
}
impl FocusScope { pub fn trap() -> Self { Self { mode: FocusScopeMode::Trap } } }
```

`compute_next_focus` (currently `focus.rs:85-110`) changes signature to take the **active scope**:

```rust
fn compute_next_focus(
    focusables: &[(Entity, Focusable, FocusContext)],  // FocusContext = which scope each belongs to
    active_scope: Option<Entity>,    // innermost open Trap scope, else None
    current: Option<Entity>,
    forward: bool,
) -> Option<Entity>;
```

- **`active_scope` derivation:** the innermost open `TopLayer::Modal` entity that carries `FocusScope::Trap` — read from `TopLayerActivation.order` (the back of the deque is the most-recently-activated modal). This is the §3.4 resolution: **derive the trap scope from the modal's `FocusScope` component, keyed by the `TopLayerActivation` deque** — not a free-floating registry.
- When `active_scope` is `Some`, the candidate set is filtered to focusables that are descendants of that scope entity (and not inert — the agent-interface campaign's `A11yHidden`/inert marker, §C.2); Tab wraps within it (the trap). When `None`, the global set is used (preserving today's non-modal behavior).
- **Tab order fix (audit W16, folded into this same pass per umbrella §9.4):** within a scope, sort by `(explicit tab_order, document/layout order)` instead of `entity.index()`. Document order = the `painters_z`/`Children`-walk order (stable across despawn/respawn), resolving the `focus.rs:9` deferral. The `FocusContext`/order resolution and the trap filtering happen in **one pass** so the entity-index→document-order fix lands with the scope generalization (umbrella §9.4).

#### C.2 Inert — CEDED to the agent-interface campaign; C5 supplies the focus + hit-test predicates (umbrella §6.4 + §2.7)

> **Reframed by the 2026-06-22 coordination decision.** The pre-coordination draft defined a C5-owned `Inert`/`ComputedInert` marker pair gating three walks. **The inert/`A11yHidden` marker + the AccessKit-tree prune are now owned by the agent-interface campaign** (semantic-tree.md §7.4: "`A11yHidden` + inert entities and descendants emit no node, excluded from parents' lists. Joint focus + AccessKit + picking concern."). C5 must **not** define a competing `Inert` marker. The behavior is the same — *one marker, three walks* — but the marker lives in the a11y substrate:

`A11yHidden`/inert (agent-interface campaign) gates **three independent walks** (umbrella §6.4 — the same three, redistributed):

1. **Focus traversal (C5 owns this consumer, `focus.rs`):** inert entities are filtered out of the candidate set in `compute_next_focus` (§C.1). **C5 supplies this predicate** — it reads the agent-interface inert marker.
2. **AccessKit prune (agent-interface campaign owns):** `build_tree` skips inert entities and descendants (semantic-tree.md §7.4). C5 does **not** edit `build_tree`; it relies on the agent-interface prune. (The "mark the rest of the tree inert when a modal opens" *action* is C5's modal lifecycle in §C.5 — it *sets* the agent-interface marker, it does not define it.)
3. **Hit-test (C3's `emit_picks`, C5 supplies the predicate):** `emit_picks` skips inert entities, so content behind a modal is non-interactive. **C5 supplies this predicate** to C3's rewrite (the same predicate-supply pattern as the `Hidden` filter, §A.7).

**What C5 owns here:** the *focus-traversal exclusion predicate* and the *hit-test exclusion predicate* over the agent-interface inert marker, plus the *modal lifecycle that sets/clears the marker* on the rest-of-tree when a dialog opens/closes (§C.5). **What C5 does NOT own:** the inert marker itself, its subtree propagation, or the AccessKit prune — all agent-interface. If a C5-local *computed* inert needs to propagate (e.g. to feed the picking predicate efficiently), it does so by *reading* the agent-interface marker, never by introducing a parallel `Inert`/`ComputedInert` type. Resolution of §3.1 (one marker, three walks, not folded into paint-skip) stands; only the *home* of the marker moves.

#### C.3 Roving tabindex + `aria-activedescendant` — CEDED to the agent-interface campaign

> **Reframed by the 2026-06-22 coordination decision.** Roving-tabindex intra-navigation, the `active`-descendant state, and the `aria-activedescendant` *lowering* are **owned by the agent-interface campaign** — `A11yRelations.active_descendant: Option<Entity>` (semantic-tree.md §3, lowered via `set_active_descendant`) plus the consumer-side APG keyboard for menus/listboxes (widget-contracts.md). C5 must **not** define a `Roving` component or an `active_descendant` wire-field. **C5's contribution is the focus-scope geometry the roving container sits in** (§C.1): the container is the single Tab-stop within the scope; intra-container Arrow/Home/End navigation is the agent-interface APG keyboard, not C5's. Where a C5 container needs to express its active descendant, it *populates* `A11yRelations.active_descendant` on the agent-interface component, never a competing field.

**Wire-format ownership (umbrella §6.5 + §2.7, consumer side).** The `A11yNodeView`/`A11yStates`/`A11yRelations` fields C5's containers touch — `active_descendant` (this section, agent-interface `A11yRelations`), the live politeness (announcer/live-region, §C.6, agent-interface `A11yLive`), the modal flag (Dialog, §C.5, agent-interface `A11yModal`) — are **all added to the schema by the agent-interface campaign (P1a)**, before C5 lands. The scroll-position fields `scroll_x`/`scroll_y`/`scroll_x_min`/`scroll_x_max`/`scroll_y_min`/`scroll_y_max` (ScrollArea, §A.4) are the one wire-format surface C5 still *needs added* that is not in the agent-interface P1a set — coordinate with the agent-interface campaign + C7 to land those in the **same** single wire-format change as P1a's field additions, so the a11y goldens re-bless exactly once. **C5 does not extend the a11y schema itself; it only *populates* these fields** from its containers. The fields must exist before C5's a11y fixtures assert their populated values (§6).

#### C.4 Focus restoration on close

When a modal/menu/popover opens, it saves the current `FocusedEntity` into a `FocusReturn(Entity)` component on the overlay; on close, focus is restored to that entity (or the next-best if it was despawned — fall back to the overlay's invoker, else clear). Stored on the overlay so nested overlays restore in LIFO order, keyed off `TopLayerActivation`.

#### C.5 `Dialog` / `AlertDialog` — the C5 container/trap layer

> **Coordination.** The `Dialog`/`AlertDialog` roles, `A11yModal` (+ `set_modal`), the Dialog `A11yContract`, `A11yRelations.labelled_by`/`described_by`/`owns`, the invoker `{Click}` + `controls=[dialog]` advertisement, and role-implied assertive/atomic live for `AlertDialog` are **owned by the agent-interface campaign** (widget-contracts.md "Dialog"; semantic-tree.md role-implied live). Crucially, the agent-interface campaign states *"No AccessKit dialog verb: focus-trap/Esc/restore are **Buiy's overlay state machine**"* — **that overlay state machine is C5.** C5 owns the modal *container*: `Stacking`/`TopLayer::Modal` membership, the `FocusScope::trap`, the focus-trap *traversal*, `FocusReturn` restoration, and the deferred focus-into. C5 *populates* `A11yModal` (sets the agent-interface marker on open), it does not define the modal a11y state.

```rust
// C5's container/trap contribution, composed with the agent-interface Dialog
// bundle (which supplies the role + A11yModal + A11yContract + labelling/owns).
#[derive(Component, Default, Clone)]
#[require(
    Node,
    Stacking = modal_stacking(),       // top_layer: Modal  (C5-owned)
    FocusScope = FocusScope::trap(),    // §C.1 trap traversal (C5-owned)
    FocusReturn,                        // restoration target captured on open (C5-owned)
    // A11yRole::Dialog / A11yRole::AlertDialog, A11yModal, the Dialog
    // A11yContract, and labelled_by/described_by/owns come from the
    // agent-interface Dialog bundle — NOT defined here.
)]
pub struct Dialog;
```

On open (C5's overlay state machine): push to `TopLayerActivation` (already built), **set the agent-interface inert marker (`A11yHidden`/inert, §C.2) on the rest of the tree** so it is pruned from a11y + excluded from focus/hit, capture `FocusReturn`, **set the agent-interface `A11yModal` marker**, and insert the shared `PendingFocus` primitive (§B.3a) — `FirstFocusableChild`, falling back to `SelfRoot(dialog)` when the body has no focusable content — to set focus the frame after the dialog's children spawn (Dialog and MenuPopup reuse the one `resolve_pending_focus` system). On close: clear the inert marker on the rest-of-tree, restore `FocusReturn`, fire `toggle`. `aria-modal=true` lowers from the agent-interface `A11yModal` (its `set_modal`, semantic-tree.md §5), not a C5 call. `::backdrop` styling is C6's paint concern; this child only establishes the modal *container* + trap.

#### C.6 Live-region / Announcer (WCAG 4.1.3) — CEDED to the agent-interface campaign

> **Reframed by the 2026-06-22 coordination decision.** The pre-coordination draft defined a C5-owned `Announcer` resource + `LiveRegion` component + persistent live-region nodes. **The live-region/announcer substrate is owned by the agent-interface campaign:** P1a adds `A11yLive { politeness: Live, atomic: bool }` (semantic-tree.md §2 → `set_live` + `set_live_atomic`) and the role-implied live derivation `resolve_live` (semantic-tree.md §5: `Role::Alert`⇒Assertive+atomic, `Role::Status`⇒Polite+atomic, `Role::Log`⇒Polite), which gate #4 verifies. C5 must **not** define a competing `Announcer`/`LiveRegion`/persistent-node mechanism.

C5's only relationship to live regions is as a **consumer/populator** for its containers:

- An `AlertDialog` (§C.5) announces by virtue of its agent-interface `A11yRole::AlertDialog` + role-implied assertive/atomic live — C5 sets no live state, it just opens the dialog.
- A toast/status overlay C5 positions carries the agent-interface `A11yLive`/`A11yRole::Status`; C5 owns the *stacking + positioning + lifecycle*, the agent-interface campaign owns the *announcement*.
- The gallery's "N items left" count node (the C8 exemplar) uses the agent-interface `A11yLive` directly; C5 supplies no announcer plumbing for it.

If C5 ever needs an *ad-hoc* announce (no anchored node), it routes through the agent-interface campaign's announcer mechanism rather than standing up a parallel queue. The WCAG 4.1.3 CI gate (`accessibility.md:158`) is the agent-interface gate #4; C5 does not re-implement it. (The `accessibility.md:44` "global announcer service" commitment is realized by the agent-interface campaign, not C5 — the pre-coordination claim that C5 owns it is superseded by §2.7.)

---

## 3. Decisions & rejected alternatives

### 3.1 Inert enforcement — one marker, three consumers, not folded into paint-skip; marker home CEDED to the agent-interface campaign

**Decision (post-coordination):** the *design* stands — one inert marker, three independent consumers (focus traversal, a11y prune, hit-test), **not** folded into paint-skip. The *home of the marker moved*: per the 2026-06-22 coordination decision, the `A11yHidden`/inert marker + its subtree prune live in the **agent-interface campaign's a11y substrate** (semantic-tree.md §7.4), not in a C5-owned `Inert`/`ComputedInert` pair. **C5 owns two of the three consumers as predicates** (focus-traversal exclusion in `focus.rs`; hit-test exclusion supplied to C3's `emit_picks`) and the modal lifecycle that *sets* the marker; the agent-interface campaign owns the marker itself + the AccessKit prune.
**Rejected — fold into the existing paint-skip subtree walk:** inert is an *interaction* concern (focus/a11y/hit), paint-skip is a *visual* concern; an inert subtree behind a modal **still paints** (dimmed). Conflating them would either stop painting inert content (wrong) or fail to suppress interaction on painted content (wrong). Separate marker, distinct semantics — unchanged by the home move.
**Rejected — a C5-defined competing `Inert` marker (the pre-coordination draft):** would duplicate the agent-interface campaign's `A11yHidden`/inert and create two sources of "what is interaction-suppressed" that would drift (the same single-source-of-truth argument as §3.4's rejected registry). Cede the marker; keep the predicates.
**accesskit 0.24 verification (still load-bearing for the agent-interface prune):** `Display::None` entities never reach extract (they are layout-pruned, `render/components.rs:440`) so they are already absent from the a11y `build_tree` query results only if `build_tree` filters them — it does **not** today. So `Display::None` does **not** reliably prune the accesskit tree; the agent-interface campaign's `A11yHidden`/inert prune (semantic-tree.md §7.4) is the explicit prune, and C5's `Hidden` filter (§A.7) lowers to it.

### 3.2 Scroll bounds source — cached `ScrollExtent`, not per-frame children union

**Decision:** cache content/viewport extent in a `ScrollExtent` component, recomputed only on layout change.
**Rejected — union children `ResolvedLayout` each wheel tick (O(children)):** the clamp runs on every wheel event (potentially many per frame on a trackpad); an O(children) walk per tick is wasteful on a 1000-row list, and computing it on the scroll write-path risks coupling to the no-invalidate invariant. The cache is updated by a *layout-change* reaction, keeping it off the scroll path entirely.

### 3.3 Filter/hide — `Hidden` marker overriding *resolved* display, leaving author `Display`/`FlexParams` intact (audit W15)

**Decision:** a `Hidden` marker that, at style-sync time, overrides the resolved Taffy display to `None` **without mutating** the author's `Display`/`FlexParams`, and prunes the row from the a11y tree (Inert-adjacent). Un-filtering = remove the marker; the author's `Display: Flex(Row)` + direction are untouched.
**Rejected — direct `Display` rewrite (the prototype):** desyncs `FlexParams.direction` (decoupled component, `style.rs:73-77`) and stomps author intent (audit report line 151).
**Rejected — `CssVisibility::Hidden`:** it keeps the *layout box and a11y presence* (`render/components.rs:337-345`), which is wrong for a filtered-out row — a hidden todo must collapse its box and leave the a11y tree, not occupy space invisibly.

### 3.4 Focus-scope representation — `FocusScope` component keyed by `TopLayerActivation`, not a free registry

**Decision:** a `FocusScope` component on the modal/region marks the trap boundary; the *active* scope is derived as the innermost open `TopLayer::Modal` carrying `FocusScope::Trap`, read from the already-built `TopLayerActivation.order` deque.
**Rejected — derive scope purely from `TopLayer::Modal` with no component:** not every trap is a modal (a non-modal `Contain` region — e.g. a focus-cycling toolbar — needs scope without top-layer membership); coupling scope to top-layer alone cannot express `Contain` vs `Trap`.
**Rejected — a free-floating `FocusTrapRegistry` resource:** duplicates the LIFO ordering `TopLayerActivation` already maintains; two sources of "what's the active modal" would drift. Keying off the existing deque is the single-source-of-truth choice (grounding §3.4 — couples to the built activation deque).

### 3.5 Light-dismiss — `Pointer<Press>`-outside observer (not focus-change-only)

**Decision:** pointer-outside light-dismiss via a `Pointer<Press>` capture-phase observer using C3's `painters_z` pick-depth, plus Escape.
**Rejected — focus-change-only detection (bevy-ui-widgets' menu approach):** detecting outside-clicks via focus changes (close when no descendant has focus) works for menus but **fails for non-focusable outside targets** — clicking inert decorative content or empty canvas does not change focus, so a popover would not dismiss. The audit's open question (grounding §"Light-dismiss without Pointer<E>") asked whether focus-change is sufficient; with C3 adopting `Pointer<E>` (umbrella §2.2, the decided input model), the clean `Pointer<Press>`-outside observer is available and is strictly more correct. Focus-change remains the *menu* lifecycle signal (a menu closes when focus leaves it), but light-dismiss of arbitrary overlays uses the pointer observer.

### 3.6 Announcer → accesskit — CEDED to the agent-interface campaign

**Decision (post-coordination):** the live-region/announcer substrate is **owned by the agent-interface campaign** — `A11yLive { politeness, atomic }` + role-implied live (`resolve_live`, semantic-tree.md §5), verified by its gate #4. C5 does **not** define an `Announcer` resource, a `LiveRegion` component, or persistent live-region nodes; it *consumes* the agent-interface live substrate for its toast/status/alert-dialog containers (§C.6).
**The pre-coordination decision (retained for the audit trail):** two persistent off-screen live-region nodes (polite + assertive) whose text is mutated to trigger AT, since accesskit 0.24 has no one-shot announce verb (only `Live` + `set_live`/`set_live_atomic`). That *mechanism* is correct and is what the agent-interface campaign realizes — the design conclusion was right; only the *owner* moved (§2.7).
**Rejected — a C5-owned parallel announcer (the pre-coordination draft):** duplicates the agent-interface live substrate and risks two announcement paths competing on the same AccessKit tree. Cede it.

### 3.7 Keep C5 as one document; stage *implementation* A→B→C

**Decision:** spec C5 as one document with A/B/C slices (umbrella §7); recommend **not** splitting into sub-children. The slices share the `Pointer<E>`/`painters_z`/`Inert`/focus contracts and reviewing them together keeps the cross-references honest. Implementation stages A (lowest risk, downstream wired) → B (after C3 picking-depth lands) → C (the `focus.rs` edit, last, behind the WCAG-2.1.2 gate).
**Rejected — split into C5a/C5b/C5c now:** premature; the slices are tightly coupled through the shared contracts and a single reviewer pass catches cross-slice drift. §7 holds the split open if implementation reveals one slice is too large for one unit.

### 3.8 Virtualization ceiling — lean on `ContentVisibility::Auto`, defer recycling (§7 open q, resolved for v1)

**Decision:** v1 = none-with-named-ceiling (A.6). The gallery's purpose is to *exercise* scrolling; `ContentVisibility::Auto` demonstrates off-screen relief at ~1000 rows, which is the capability the kickoff needs. A recycling `VirtualList` is a follow-up widget.
**Rejected — build a recycling virtual-list now:** it is a widget concern above the scroll-container primitive (overflow-and-scrolling.md §6 punts it to widget-catalog; freya/gameface confirm), and folding it into the scroll-container primitive would be the wrong layer. C8's scale-game fixture validates the ceiling empirically and tells us if/when recycling is needed.

---

## 4. Contracts & interfaces

### 4.1 Shared contracts referenced (umbrella §6 — do NOT redefine here)

- **§6.1 Pick-depth from `painters_z`** — C3 owns the `emit_picks`/`hit_test` rewrite (which **delivers the stacking-aware `hit_test` the agent-interface campaign deferred** as its follow-up #3 and depends on for `HitTargetable`); C5 *consumes* it for overlay hit-correctness (B.1) and light-dismiss inside/outside (B.5). C5 supplies the **inert/hidden hit-test exclusion predicate** that `emit_picks` calls (over the agent-interface inert marker).
- **§6.2 Coordinate space (C1)** — C5 depends on absolute `GlobalTransform` for correct overlay picking and popover anchor positioning. The bridge's `ScrollOffset` fold (`bridge.rs:135-163`) means scroll picking is **free** once C1 lands.
- **§6.3 `Pointer<Scroll>`** — C3 owns the event entry; C5 owns nearest-container routing + clamp + overscroll (A.1).
- **§6.4 Inert gates three walks** — focus traversal (C5/`focus.rs`, C5 supplies the predicate), AccessKit prune (**agent-interface campaign owns** the `A11yHidden`/inert prune, semantic-tree.md §7.4), hit-test (C3 `emit_picks`; C5 supplies predicate). **The inert marker + propagation + a11y prune are agent-interface-owned** (§2.7); C5 owns the focus + hit-test *predicates* and the modal lifecycle that sets the marker.
- **§6.5 A11y wire format** — **the agent-interface campaign (P1a) adds the a11y state/relation surface** (`A11yLive`, `A11yRelations.active_descendant`, `A11yModal`, the widened `A11yNodeView`); C7 lands the **`scroll_*`** wire fields C5 needs in the **same single coordinated change** so the goldens re-bless once. C5 *populates* `active_descendant` (its containers), `A11yLive` source (its toast/alert containers), `A11yModal` (Dialog), `scroll_*` (ScrollArea). C5 does **not** extend the a11y schema.
- **§6.9 Event vocabulary** — activation flows through the existing **`OnPress`** / the agent-interface action router (umbrella §2.7 — **no competing `Activate` event**); C5's menu items / dialog buttons *adopt* `OnPress`. The activation/light-dismiss/Escape duality (every overlay needs pointer AND keyboard) per grounding priorArtLessons #6 — C5 owns the *light-dismiss + Escape-close + focus-restore* half, the agent-interface APG keyboard owns the *activation* half.

### 4.2 Contracts this child owns

- **`ScrollInputPlugin`** (`buiy_core`): consumes `Pointer<Scroll>` → clamped `ScrollOffset`; unit normalization; overscroll-contain; reduced-motion. Keyboard scroll. `scroll_to`.
- **`ScrollExtent`** component (`buiy_core`): cached content/viewport extent, layout-change-driven.
- **`Hidden`** marker (`buiy_core`): resolved-display override at style-sync time (the *geometry*) + a prune *predicate* supplied to the agent-interface campaign's a11y prune (§A.7), author `Display`/`FlexParams` untouched. C5 owns the resolved-display geometry; the a11y prune marker (`A11yHidden`/inert) is agent-interface's.
- **`FocusScope`** component + scoped `compute_next_focus` (`buiy_core/focus.rs`): trap/contain modes; active scope from `TopLayerActivation`; document-order tab-order (W16 folded in). **The highest-value C5-owned a11y-adjacent piece** — the focus-trap *traversal* the agent-interface Dialog/Menu explicitly delegate to "Buiy's overlay state machine."
- **Inert focus + hit-test predicates** (`buiy_core`): the focus-traversal exclusion (in `compute_next_focus`) and the hit-test exclusion (supplied to C3's `emit_picks`) over the **agent-interface campaign's** `A11yHidden`/inert marker. C5 owns the *predicates*, NOT the marker/propagation/a11y-prune (those are agent-interface, §2.7).
- **`FocusReturn`** (`buiy_core`): focus restoration target + the LIFO restore on overlay close.
- **`ScrollArea`, `Popover`** + the **container/positioning/focus-scope/rendering layer** of `MenuPopup`/`MenuItem`/`MenuButton`, `Tooltip`, `Dialog`/`AlertDialog` (`buiy_widgets`): `#[require]`+scene-fn, **composed with the agent-interface campaign's P1d bundles** (which own each widget's role + `A11yContract` + APG keyboard + a11y state). Coordinate per-widget; neither campaign rebuilds the other's layer.

### 4.3 A11yRole + state consumed (owned by the agent-interface campaign)

C5's containers sit under `A11yRole` variants the **agent-interface campaign** owns (semantic-tree.md §4 adds `Region`/`Group` — landed in P0; `Menu`/`MenuItem`/`Status`/`Alert` are added *there* if a Phase-1 widget needs them). `Dialog`/`AlertDialog`/`Tooltip` already exist (`a11y/mod.rs:34-38`). The widget a11y state C5 reads for *visual* feedback — `A11yToggled`/`A11ySelected`/`A11yExpanded` (e.g. a menu-item's selected tick, a disclosure's expanded chevron) — is owned by the agent-interface campaign's `a11y/states.rs` (P1a). **C5 *names* the requirement and *consumes* the role/state; it does not extend the enum or define state.**

---

## 5. Migration / build steps (ordered; blast radius)

Per umbrella §8, no code lands until the implementation gate clears + rebase onto fresh `origin/main`; each plan step re-confirms file:line anchors. Within C5, the implementation order is A → B → C.

1. **Slice A.1–A.3 (scroll input + extent).** New `buiy_core::scroll` module: `ScrollInputPlugin`, `on_scroll` observer, `normalize_delta`/`clamp_to_extent`, `ScrollExtent` + its layout-change updater, keyboard scroll. **Blast radius:** new module; register in `BuiyPlugin`; no existing code changed (bridge/clip/visibility already consume `ScrollOffset`). New unit tests for clamp/overscroll/normalization.
2. **Slice A.4–A.5 (ScrollArea + scroll_to).** New `buiy_widgets` widget + scene-fn + a11y lowering of `set_scroll_*` / `Action::SetScrollOffset`. **Blast radius:** new widget file; prelude addition.
3. **Slice A.7 (`Hidden` marker).** New `Hidden` component + style-sync resolved-display override. **Blast radius:** touches `sync_styles` (add override read); a `Hidden` row lowers to / supplies the prune predicate for the **agent-interface campaign's** `A11yHidden`/inert prune (C5 does not edit `build_tree`; coordinate the predicate/marker-set with the agent-interface campaign); new tests for un-filter restoration of `FlexParams.direction`.
4. **Slice B.2 (`Popover` positioning).** New positioning primitive + `position_popover` in `BuiySet::Animate`. Depends on C1 absolute coords. **Blast radius:** new file; new fixtures.
5. **Slice B.3–B.4 (Menu/Tooltip container layer).** New container widgets composed with the agent-interface P1d Menu/Tooltip bundles; `MenuFocusState` lifecycle; deferred-focus. Depends on C3 `Pointer<E>` + C5 `FocusScope` + the agent-interface Menu/Tooltip bundle (role/contract/APG keyboard). **Blast radius:** new files; prelude. Coordinate per-widget so the role/state/keyboard come from the agent-interface bundle.
6. **Slice B.5 (light-dismiss).** Global `Pointer<Press>` observer + Escape handler. Depends on C3 `painters_z` (§6.1). **Blast radius:** new observer; consumes `TopLayerActivation`.
7. **Slice C.2 (inert predicates) — CEDED marker, C5 predicates only.** The `A11yHidden`/inert marker + propagation + a11y prune land in the **agent-interface campaign** (P1b); C5 adds the **focus-traversal exclusion** (`focus.rs`) and the **hit-test exclusion** (supplied to C3's `emit_picks`) over that marker, plus the modal lifecycle that sets it. **Blast radius:** `focus.rs` filter; C3's `emit_picks` skip (coordinate with C3); the modal lifecycle sets the agent-interface marker. **Sequenced after the agent-interface P1b inert prune lands.** a11y goldens that include behind-modal content re-bless in the agent-interface prune change, not here.
8. **Slice C.1 (scoped `compute_next_focus`) — the highest-risk edit, C5-owned.** Generalize `focus.rs:85-110` signature; add `FocusScope`; fold in document-order tab-order (W16). **Blast radius:** `focus.rs` rewrite; every focus test + the hand-set-`FocusedEntity` e2e tests (audit §6) touch it; gated by the WCAG-2.1.2 property test (must be RED-first, §6). The `entity.index()`→document-order change affects any test asserting Tab order.
9. **Slice C.4 (FocusReturn) — C5-owned. Roving/activedescendant CEDED.** Focus restoration (`FocusReturn`) + LIFO restore on close. The roving system + `aria-activedescendant` population are **owned by the agent-interface campaign** (its APG keyboard + `A11yRelations.active_descendant`); C5 populates `active_descendant` for its containers but does not build the roving system. **Blast radius:** new focus-restore system.
10. **Slice C.5 (Dialog/AlertDialog container/trap layer).** New container widgets composing `FocusScope`+`FocusReturn`+`TopLayer` + the modal-inert lifecycle, **composed with the agent-interface Dialog bundle** (role + `A11yModal` + contract + labelling/owns). **Blast radius:** new files; prelude. The `aria-modal` a11y golden lands in the agent-interface Dialog change; C5's golden asserts the *focus-trap + restoration + inert-on-open* behavior.
11. **Slice C.6 (live-region) — CEDED.** The `Announcer`/`A11yLive`/persistent-live-region substrate is **agent-interface-owned** (P1a + role-implied live). C5 has no build step here beyond *using* the agent-interface live substrate for its toast/status/alert-dialog containers. The WCAG-4.1.3 gate is the agent-interface gate #4.
12. **Docs flip.** Mark `overflow-and-scrolling.md §2` ("scroll system in buiy-input-events-design writes ScrollOffset") and `stacking-and-top-layer.md` deferrals **realized**, pointing to this child; update `docs/README.md` index. Resolve the `focus.rs:9-22` Phase-0 deferral notes (tab-order, FocusVisible reset, scope) — those land in C3 (FocusVisible decay) and C5 (scope/order). Update `ScrollOffset`/`Overscroll`/`ScrollBehavior` component doc comments to point at `buiy_core::scroll` instead of the nonexistent `buiy-input-events-design`. Note in the `accessibility.md` inert/live-region rows that those are **realized by the agent-interface campaign**, with C5 supplying the focus-trap traversal + predicates.

**Snapshots/goldens:** scroll does NOT invalidate `ResolvedLayout` (enforced invariant + existing test), so **layout snapshots are stable under scroll**. New overlay/modal/scroll fixtures add NEW goldens lowest-tier-first (layout snapshot → display-list → reftest → golden, per `using-buiy-verification`). Existing top-layer paint-order goldens (`render_paint_order.rs`) must **not** move. a11y goldens gain `active_descendant`/`A11yLive`/`A11yModal` fields in the **agent-interface P1a change**; the **`scroll_*`** wire fields C5 needs land in the **same coordinated change** (with C7) — C5 must not trigger a second re-bless.

---

## 6. Verification (how C7 gates this; RED-first)

C7's Tier-A `PointerHarness` (umbrella §4 C7, §5 Wave 1) is the synthetic-input gate this child's behaviors run on — headless, no GPU/winit, real layout→bridge→`GlobalTransform`, `InteractionPlugin` + backend, synthetic `PointerId`/`PointerInput`. C5 adds fixtures on top of it. Each new predicate is proven **RED-first** (umbrella §9.5 — the existing `picking_backend.rs` hand-writes `ResolvedLayout` and is structurally blind; do not trust it as the gate).

**Slice A:**
- **Scroll clamp** (RED: write a scroll delta past content extent; assert `ScrollOffset` clamps to `[0, content−viewport]`, does not overshoot). Headless, drives synthetic `Pointer<Scroll>`.
- **Overscroll-contain** (RED: nested scroll-containers; inner at bound with `Contain`; assert the outer does NOT scroll — no chaining).
- **Unit normalization** (RED: `Line` vs `Pixel` deltas produce expected px offset).
- **No-invalidate invariant** (extend existing `layout_scroll_offset_no_invalidate.rs`): after a scroll, assert `ResolvedLayout` unchanged.
- **`Hidden` restoration** (RED: filter a flex-row child via `Hidden`, then un-filter; assert author `Display`/`FlexParams.direction` survive and the box returns; assert a11y tree drops/restores the node).

**Slice B:**
- **Overlay intercepts click** (RED: an overlay (`TopLayer::Popover`) painted over a button; synthetic press at the overlap; assert the **overlay** is hit, not the button behind — this is the audit's demanded offset-overlay-over-content picking regression, gated by C3's `painters_z` + C1 coords). This fixture must be RED on `main`'s smallest-area `emit_picks`.
- **Popover fit-in-window** (layout-snapshot tier: anchor near a window edge; assert the placement flips to the first candidate that fits).
- **Light-dismiss** (RED: open an `auto` popover; synthetic press outside; assert it closes + fires `toggle`. Then Escape; assert close + focus restored).

**Slice C — the gates C5 owns (the inert/live/roving gates are the agent-interface campaign's #3/#4/#7/#12; C5 does not duplicate them):**
- **No-keyboard-trap (WCAG 2.1.2, `accessibility.md:123`) — the gate for the `focus.rs` edit, C5-owned.** RED-first property test: in *every* widget/overlay state, repeated Tab eventually exits (or, inside a Trap, cycles without escaping to inert content but Escape exits). Must pass before the scoped `compute_next_focus` is accepted.
- **Focus trap containment** (RED, C5-owned: open a modal; Tab N times; assert focus stays within the modal's `FocusScope`, never lands on behind-modal inert content — the agent-interface inert marker).
- **Inert focus + hit-test exclusion — the two C5 predicates** (RED, two asserts on one fixture): behind a modal, (1) Tab never focuses inert content (C5's `focus.rs` filter), (2) a synthetic press on inert content does not hit it (C5's predicate in `emit_picks`). **The third walk — the a11y-tree omit/`is_hidden` — is the agent-interface campaign's gate #12 (no orphans, focus reachable), not re-asserted here.**
- **Focus restoration** (RED, C5-owned: focus button A, open dialog, close; assert focus returns to A).
- **Tab order is document-order, not entity-index (W16)** (RED, C5-owned: spawn focusables, despawn+respawn one to churn entity indices; assert Tab order follows document order, stable).
- **Roving + activedescendant + announcer / live-region — agent-interface gates, NOT C5.** The arrow-within-roving + `aria-activedescendant` lowering is the agent-interface APG keyboard + `A11yRelations.active_descendant` (its #7/#3 fixtures); the announcer/live-region (WCAG 4.1.3, `accessibility.md:158`) is its gate #4. C5's fixtures only assert that its *containers* carry the right populated source (a Dialog carries `A11yModal`; a roving container's `active_descendant` is set) — verified through the agent-interface in-process driver, not a parallel C5 announcer test.

**a11y wire-format:** the state/relation surface (`A11yLive`/`active_descendant`/`A11yModal`) is the **agent-interface P1a change**; the `scroll_*` wire fields land in the same coordinated change (with C7). C5's fixtures assert the *populated* values (`active_descendant`, modal, `scroll_*`) once the fields exist, and must not introduce a second golden re-bless.

**Scale-game (C8, Wave 5):** the 1000-row long-list fixture (umbrella §5) settles A.6's named ceiling empirically — it is C8's fixture but validates C5's virtualization posture.

---

## 7. Open questions deferred + dependencies

### Resolved in this spec
- Scroll producer channel → **`Pointer<Scroll>`** (§A.1; gated on C3's decided §2.2 input model).
- Scroll bounds source → **cached `ScrollExtent`** (§3.2).
- Inert enforcement → **one marker, three consumers, not folded into paint-skip** (§3.1); the marker (`A11yHidden`/inert) is **agent-interface-owned**, C5 owns the focus + hit-test *predicates* (§2.7).
- Focus-scope representation → **`FocusScope` component keyed by `TopLayerActivation`** (§3.4) — C5-owned.
- Light-dismiss → **`Pointer<Press>`-outside observer + Escape** (§3.5) — C5-owned.
- Announcer → accesskit → **persistent live-region nodes mutated** (§3.6); the mechanism is correct but **CEDED to the agent-interface campaign** (`A11yLive` + role-implied live), C5 consumes it (§2.7).
- Filter/hide → **`Hidden` marker (geometry) lowering to the agent-interface a11y prune, author `Display`/`FlexParams` untouched** (§3.3).
- Virtualization ceiling → **none-with-named-ceiling, lean on `ContentVisibility::Auto`** (§3.8).
- Sub-spec home → **realize here, no empty stubs** (§2.1; umbrella §7).
- C5 split → **one document, stage implementation A→B→C** (§3.7); split held open only if a slice proves too large to implement as one unit.

### Deferred (genuinely depend on un-built work)
- **Exact virtualization ceiling row-count** — settled empirically by C8's 1000-row scale-game fixture (Wave 5); the spec names ~1000–2000 as the design target.
- **Scrollbar widget (C-tier)** — deferred out of this F-tier child (`media-and-widgets.md:90`); the implicit overflow scrollbar is themable but not a widget here.
- **Recycling `VirtualList` widget** — a follow-up widget-catalog slice (overflow-and-scrolling.md §6); not in this child.
- **Submenus** — deferred (matches prior-art's TODO); `MenuPopup` v1 is single-level.
- **`aria-expanded` source for `MenuButton`/disclosure** — owned by the **agent-interface campaign's** `A11yExpanded` (P1a) + Disclosure-trigger contract (P1d); C5's `MenuButton` reads it for *visual* state and toggles menu *visibility*, it does not own the expanded a11y state.

### Dependencies (hard unless noted)
- **C1** (coordinate space, §6.2) — HARD; gates overlay picking + popover anchoring. Strictly precedes Slice B.
- **C3** (`Pointer<E>` + `painters_z` pick-depth + the stacking-aware `emit_picks`/`hit_test`, §6.1/§6.3/§6.9) — HARD; Slices A.1, B, light-dismiss consume it. C5's inert/hidden hit-test predicate plugs into C3's `emit_picks` (coordinate the landing). C1+C3 deliver the stacking-aware `hit_test` the agent-interface campaign deferred (its follow-up #3).
- **Agent-interface campaign** (§2.7) — HARD for the a11y substrate C5 consumes/populates: **P1a** (`A11yLive`/`A11yModal`/`A11yRelations.active_descendant` + the widened `A11yNodeView` C5 populates), **P1b** (the `A11yHidden`/inert prune C5's focus/hit predicates read), **P1d** (the Dialog/Menu/Tooltip/Disclosure bundles + roles + `A11yContract` + APG keyboard C5's containers compose with). Coordinate per-widget; C5 is sequenced after the relevant agent-interface phase for each widget.
- **C4** (the widget *visual/picking* layer + the `scroll_*` wire-field coordination §6.5) — SOFT for the menu/option *visual* state read (`A11yToggled`/`A11ySelected`/`A11yExpanded`, agent-interface-owned); the `scroll_*` wire fields must land in the agent-interface P1a / C7-coordinated change before C5's scroll a11y fixtures.
- **C6** (shadow/border/outline/backdrop) — SOFT; overlays look correct with shadows but function without them.
- **C7** (Tier-A `PointerHarness`) — HARD; the synthetic-input gate all C5 behaviors run on. C7 *complements* the agent-interface in-process driver (it owns the picking/coordinate-geometry tier; the a11y semantic gates are the agent-interface campaign's).
- **ContentVisibility** (built, Phase 11) — leaned on for virtualization posture; no new work.

---

## 8. Coordination with the agent-interface campaign

Per umbrella §2.7 + §8 (the user's 2026-06-22 "coordinate, don't cede" decision), C5 **coordinates** with the agent-interface campaign (`docs/specs/2026-06-18-buiy-agent-interface-design/`, landing P0→P1a→P1b→P1c→P1d→P2), which is **LOCKED to own the a11y substrate**. C5 keeps its full scope but builds the **container / positioning / scroll / focus-traversal geometry** layer *on* that substrate. The agent-interface widgets (Dialog/AlertDialog/Tooltip/Menu/Disclosure) **sit inside** C5's containers; C5 must not define competing a11y-state components, roles, inert markers, live-regions, or the APG widget bundles.

### C5 OWNS (geometry / mechanics — the agent-interface campaign does not build these)
- **Scroll input + geometry:** `ScrollInputPlugin` (`Pointer<Scroll>` → clamped `ScrollOffset`), `ScrollExtent`, keyboard scroll, `scroll_to`, the `ScrollArea` container (§A). C5 *populates* the `scroll_*` a11y fields; it coordinates landing those wire fields in the agent-interface P1a / C7 change.
- **Overlay positioning:** the `Popover` placement / fit-in-window primitive (§B.2); `Stacking`/`TopLayer` membership for menus/tooltips/dialogs.
- **Light-dismiss + Escape-close + focus restoration** (§B.5, §C.4): the `Pointer<Press>`-outside observer, Escape handler, and `FocusReturn` LIFO restore.
- **Focus-trap *traversal*:** scoped `compute_next_focus` + `FocusScope` + the W16 document-order tab-order fix (§C.1). This is the "Buiy overlay state machine" the agent-interface Dialog contract explicitly delegates to.
- **The `Hidden` (resolved-display) override** (§A.7): the *geometry* of filter visibility — author `Display`/`FlexParams` untouched. C5 supplies the prune *predicate*; the a11y prune is the agent-interface marker.
- **The inert focus + hit-test exclusion *predicates*** (§C.2): C5 reads the agent-interface `A11yHidden`/inert marker to exclude inert content from focus traversal and from `emit_picks`.
- **The stacking-aware `hit_test` dependency:** C1+C3 (this campaign) deliver the stacking-aware `hit_test`/`emit_picks` that the agent-interface campaign **deferred** (its follow-up #3) and **depends on** for `HitTargetable` to mean "not obscured." C5 consumes it for overlay hit-correctness.
- **The visible rendering** of the meeting-point widgets (menu/tooltip/dialog chrome, focus ring via C6), composed with the agent-interface bundles.

### C5 CONSUMES (agent-interface-owned — referenced, never redefined)
- **The Dialog/AlertDialog/Menu/MenuItem/Tooltip/Disclosure a11y *contracts* + roles** (widget-contracts.md): `A11yRole::Dialog`/`AlertDialog`/`Menu`/`MenuItem`/`Tooltip`, the `A11yContract` advertise+honor, and the consumer-side APG keyboard. C5 composes its container `#[require]` with these P1d bundles per-widget.
- **The `A11yModal` marker** (semantic-tree.md §2 → `set_modal`): C5 *sets* it on a dialog open; it does not define it. `aria-modal` lowers from it.
- **The `A11yHidden`/inert marker + AccessKit prune** (semantic-tree.md §7.4): C5 *sets* it on the rest-of-tree when a modal opens and *reads* it for its focus/hit predicates; it does not define a competing `Inert`/`ComputedInert`.
- **The live-region / Announcer substrate** (`A11yLive` + role-implied live `resolve_live`, semantic-tree.md §5): C5's toast/status/alert-dialog containers *use* it; C5 defines no `Announcer`/`LiveRegion`.
- **Roving-tabindex + `aria-activedescendant` *lowering*** (`A11yRelations.active_descendant` + APG keyboard): C5 *populates* `active_descendant` for its containers; it does not build the roving system or the lowering.
- **Activation:** flows through the existing **`OnPress`** / the agent-interface action router (`route_action_requests`, `Action::Click` → `OnPress`/Focus). **No competing `Activate` event.**
- **The a11y wire-format schema** (P1a): C5 *populates* fields, never extends the schema (the one exception — the `scroll_*` fields — is coordinated into the same single change).

### Removed / deferred because the agent-interface campaign owns it
- The C5-defined **`Inert` + `ComputedInert`** marker pair (was §C.2) — replaced by predicates over the agent-interface `A11yHidden`/inert.
- The C5-defined **`Roving`** component + `aria-activedescendant` population system (was §C.3) — agent-interface APG keyboard + `A11yRelations.active_descendant`.
- The C5-defined **`Announcer`** resource + **`LiveRegion`** component + persistent live-region nodes (was §C.6) — agent-interface `A11yLive` + role-implied live.
- The C5-set **`A11yRole`/state on the widget bundles** (was inlined in the `#[require]` blocks of §B.3/§B.4/§C.5) — now come from the agent-interface P1d bundles; C5's blocks show only the container/geometry contribution.
- The C5-asserted **inert-a11y-prune gate** + **roving/announcer gates** (was §6) — agent-interface gates #3/#4/#7/#12; C5 asserts only the focus-trap-traversal + focus-restoration + inert focus/hit-predicate + document-order gates.

### Sequencing
C5 (Wave 4) lands after the agent-interface **P1a** (states/relations C5 populates), **P1b** (inert prune C5's predicates read), and **P1d** (the widget bundles C5's containers compose with) for the widget-touching slices. Slice A (scroll) and the `FocusScope` traversal of Slice C have the lightest agent-interface coupling and can proceed once P1a's wire surface (+ the coordinated `scroll_*` fields) lands; the Dialog/Menu/Tooltip container slices wait for the matching P1d bundle.
