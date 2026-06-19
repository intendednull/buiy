# Buiy text-editing E1 — Editor substrate + Buffer ownership

**Date:** 2026-06-13
**Status:** landed
**Campaign:** [2026-06-13-buiy-text-editing-campaign.md](2026-06-13-buiy-text-editing-campaign.md) § "E1 — Editor substrate + Buffer ownership"
**Spec:** [editing-and-ime.md](../specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md) §§ 2.1, 2.2, 2.2a, 2.3; [measure-and-layout.md](../specs/2026-06-09-buiy-text-rendering-design/measure-and-layout.md) § 2.3

---

## Goal

Land the **substrate** the editing campaign builds on, and nothing more: a
`TextEditState` component wrapping `cosmic_text::Editor<'static>` over a
`BufferRef::Owned(Buffer)`; the four policy markers (`ReadOnly`, `Disabled`,
`SingleLine`, `Placeholder(String)`); the `TextBufferAccess` accessor that makes
the editor's owned buffer authoritative for **measure + glyph + sync** while
display-only entities pay nothing; and the thin `text::edit` module facade that
contains the cosmic `Editor`/`Edit` lock-in. **No input, no keymap, no IME, no
caret behavior, no widget** — those are E2–E6.

The flagship invariant E1 must hold: **an entity carrying `TextEditState` shapes,
measures, and emits glyphs identically to the equivalent display-only entity** —
because both routes funnel through `TextBufferAccess`, which prefers the editor's
buffer when present and falls back to `TextBuffer.buffer` otherwise. The seam is
*transparent*: same `ComputedTextLayout`, same measure call count, same glyphs.

A second invariant, structural: **the facade boundary holds** — no symbol outside
`crates/buiy_core/src/text/edit/` names `Editor`, `Edit`, `Action`, or `Change`.
E1 establishes the boundary and the grep tripwire that guards it for the whole
campaign.

## Architecture

Today (T1–T9, merged) every text system binds `&mut TextBuffer` / `&TextBuffer`
**directly** because the editor does not exist yet (the T3 erratum deferred
`TextBufferAccess` to this campaign — `measure-and-layout.md` § 2.3 "As landed",
`components.rs:500-509`). The four direct binders are:

1. **`text_sync_buffers`** (`text/sync.rs`) — writes the buffer via
   `apply_authored(&mut TextBuffer, …)` → `set_metrics`/`set_wrap`/`set_text`.
   Owns the `IntrinsicWidths` cache invalidation.
2. **the measure closure** (`text/measure.rs` `measure_text_node`,
   `cached_intrinsics`) — `set_size`/`shape_until_scroll`/`layout_runs`; reads &
   fills the intrinsics cache.
3. **`text_commit`** (`text/commit.rs`) — final-width reshape; reads `layout_runs`
   for `ComputedTextLayout` + `ResolvedBaseline`.
4. **`extract_buiy_glyphs`** (`text/extract.rs`) — read-only `layout_runs` +
   `buffer.size()` at extract.

E1 introduces one new submodule, `text::edit`, that owns:

- **`TextEditState`** — `Editor<'static>` over `BufferRef::Owned(Buffer)`, plus
  the `intrinsics` cache **moved off `TextBuffer` onto the authoritative side**
  (decision 3 below). Constructed FontSystem-free (`Editor::new` is pure struct
  construction, verified — `cosmic-text-0.19.0/src/edit/editor.rs:37`).
- the four markers (`ReadOnly`, `Disabled`, `SingleLine`, `Placeholder`).
- **`TextBufferAccess`** — a `#[derive(QueryData)]` with a `#[query_data(mutable)]`
  on the mutable form, binding `&mut TextBuffer` + `Option<&mut TextEditState>`,
  exposing `with_buffer`/`with_buffer_mut`/`intrinsics`/`cache_intrinsics`/
  `invalidate_intrinsics` that **dispatch editor-first**. The read-only form (for
  extract) reaches the editor's buffer via `Edit::with_buffer` (`&self`).

The accessor is the *only* place that names `Editor`/`Edit`: its `with_buffer*`
closures hand callers a bare `&mut Buffer` / `&Buffer`, so sync/measure/commit/
extract keep working in terms of `Buffer` and never name a cosmic editor type.

**Why route sync through the accessor too (not just measure+glyph).** The
campaign plan's E1 deliverable says the editor's owned buffer is authoritative for
*measure + glyph*. But text only *reaches* a buffer through `text_sync_buffers`'
`set_text` — and the headless test demands an editor entity "shapes through the
editor-owned Buffer identically." If sync still wrote `TextBuffer.buffer` while
measure/glyph read the editor buffer, the editor buffer would stay empty and the
editor entity would render nothing. So sync MUST write through the accessor too.
This is still substrate, not behavior: `set_text` from authored `Text` is the
existing display path, merely retargeted to the authoritative buffer. (Runner-up
rejected: seed the editor buffer in the test by hand and leave sync display-only —
it makes the test lie about the production path and strands editor entities with
no text source the moment E2 stops hand-seeding.)

### Decisions (resolve before coding)

**Decision 1 — which `TextEditState` fields exist in E1.** Only `editor` (the
`Editor<'static>`) and `intrinsics` (moved off `TextBuffer`, see decision 3). The
spec § 2.2 sketch lists five fields (`editor`, `selection`, `preedit`, `undo`,
`blink`) but those four behavior fields have **no reader and no writer** in E1 —
`selection` is E3, `preedit` is E5, `undo` is E4, `blink` is E3. Including them as
unit-typed placeholders would be dead fields that clippy's `dead_code` flags and
that mislead the next implementer into thinking the machinery exists. **Each later
phase adds its field when it adds the system that reads it** — the field and its
first consumer land together, no orphan state.

> *Runner-up rejected:* land all five fields now as `()`-typed or
> `Option<Never>`-shaped placeholders for "shape stability." Rejected: the public
> field set is not load-bearing (the campaign's other phases add fields freely —
> there is no serialized layout or external API pinned to it), and dead fields
> violate the project's no-speculative-scaffolding rule. The markers ARE all four
> now (they cost nothing, gate cleanly, and E2/E6 reference them by name).

**Decision 2 — markers are all four, decomposed, in E1.** `ReadOnly`, `Disabled`,
`SingleLine`, `Placeholder(String)` are zero-behavior in E1 but cost nothing,
compile, reflect-register, and are referenced by E2 (`SingleLine`, `Disabled`),
E5/E6 (`ReadOnly`, `Disabled`), and E6 (`Placeholder`). Landing them now matches
the spec § 2.2 "decomposed, not aggregated" pin and lets the markers' gate test
(campaign E1 test surface "the markers compile and gate") exist. They are
authoring-surface components, so they reflect-register like `Text` does.

**Decision 3 — the intrinsics cache moves to the authoritative side.** Today
`IntrinsicWidths` is cached on `TextBuffer` (`components.rs:518`), keyed to
`TextBuffer.buffer`'s content version. If the editor owns the authoritative
buffer, the cache must key to *that* buffer or it goes stale silently (TextSync
invalidates the display cache while measure shapes the editor buffer). So the
cache moves behind the accessor: `TextBufferAccess::intrinsics()` /
`cache_intrinsics()` / `invalidate_intrinsics()` read/write the cache on
`TextEditState` when present, else on `TextBuffer`. `TextEditState` carries its
own `intrinsics: Option<IntrinsicWidths>` field; `TextBuffer` keeps its existing
one for display-only entities. Both arms route through the same accessor methods,
so sync/measure call sites stay one-line.

> *Runner-up rejected:* keep the cache only on `TextBuffer` and let it key to the
> editor buffer indirectly. Rejected: TextSync's `invalidate_intrinsics` and
> measure's `cache_intrinsics` would touch the display component's cache while the
> shaped buffer is the editor's — the cache and its buffer diverge, and the first
> editor re-measure reads a cache computed against the wrong (empty) buffer. The
> cache must live with the buffer it describes.

**Decision 4 — the accessor's two forms.** Bevy's `#[derive(QueryData)]` generates
a read-only companion automatically; we add inherent methods to **both** generated
item types. The mutable form (`TextBufferAccessItem`) exposes
`with_buffer_mut(&mut self, f: FnOnce(&mut Buffer) -> T) -> T`,
`with_buffer(&self, …)`, and the three cache methods. The read-only form
(`TextBufferAccessReadOnlyItem`) exposes only `with_buffer(&self, …)` (extract
never mutates). Both dispatch editor-first. Mutable buffer access bypasses change
detection in both arms (measure § 7 — a width probe is not a damage signal), so
the `&mut TextBuffer` / `&mut TextEditState` query members are accessed through
`bypass_change_detection()` inside the methods.

**Decision 5 — `Editor<'static>` field access for the accessor.** `TextEditState`
lives in `text::edit`; its `editor` field is private. The accessor methods
(`with_buffer*`) also live in `text::edit` (defined in the same `edit/access.rs`),
so they reach `state.editor` directly — no public getter that would leak `Editor`.
The `Edit` trait's `with_buffer`/`with_buffer_mut` (default methods, verified at
`cosmic-text-0.19.0/src/edit/mod.rs:176,185`) are the dispatch.

### Tech Stack

- **Bevy 0.18.1** — `#[derive(QueryData)]`, `#[query_data(mutable)]`, component
  registration, `Mut::bypass_change_detection`.
- **cosmic-text 0.19.0** (pinned, `Cargo.lock:1664`) — `Editor<'static>`,
  `BufferRef::Owned(Buffer)` (`From<Buffer>` at `src/edit/mod.rs:86`),
  `Editor::new(impl Into<BufferRef>)` (`src/edit/editor.rs:37`, FontSystem-free),
  the `Edit` trait's `with_buffer`/`with_buffer_mut` defaults
  (`src/edit/mod.rs:176,185`). `Editor` is `Send + Sync` (verified, spec § 2.2),
  so `TextEditState` is a plain `Component`.
- **Test harness** — the existing `text_app()` pattern (`MinimalPlugins` +
  `CorePlugin` + `LayoutPlugin` + `BuiyTextPlugin`) and the `settle()` two-frame
  discipline (`tests/text_measure.rs:22-37`). No GPU, no adapter.

### Files E1 creates

- `crates/buiy_core/src/text/edit/mod.rs` — the facade submodule root; re-exports.
- `crates/buiy_core/src/text/edit/state.rs` — `TextEditState` + the four markers.
- `crates/buiy_core/src/text/edit/access.rs` — `TextBufferAccess` + its methods.
- `crates/buiy_core/tests/text_edit_substrate.rs` — the headless E1 test surface.
- `crates/buiy_core/tests/text_facade_boundary.rs` — the grep tripwire.

### Files E1 modifies

- `crates/buiy_core/src/text/mod.rs` — `mod edit;`, re-exports, marker registration.
- `crates/buiy_core/src/text/components.rs` — keep `TextBuffer.intrinsics` (now the
  display-only arm); no field removed.
- `crates/buiy_core/src/text/sync.rs` — `apply_authored` writes through the accessor.
- `crates/buiy_core/src/text/measure.rs` — measure closure reads through the accessor.
- `crates/buiy_core/src/text/commit.rs` — commit reshapes through the accessor.
- `crates/buiy_core/src/text/extract.rs` — producer reads through the read-only accessor.

---

## Task 1 — the `text::edit` facade module + `TextEditState` + markers

Establishes the submodule, the editor-wrapping component (editor + intrinsics
fields only — decision 1/3), and the four policy markers (decision 2). This task
is pure construction: no system reads `TextEditState` yet, so the only test is
that the component constructs FontSystem-free and the markers compile/gate.

- [ ] **Step 1.1 — write the failing test.** Create
  `crates/buiy_core/tests/text_edit_substrate.rs` with the construction + marker
  tests (the accessor tests come in Task 2; the parity tests in Task 4):

  ```rust
  //! E1 — editor substrate. `TextEditState` over `Editor<'static>` /
  //! `BufferRef::Owned`, the policy markers, and (Task 2+) the
  //! `TextBufferAccess` seam. Headless: shaping uses the embedded Fira Sans
  //! latin subset, no adapter anywhere. The facade boundary itself is pinned
  //! by `tests/text_facade_boundary.rs`.

  use bevy::prelude::*;
  use buiy_core::text::edit::{Disabled, Placeholder, ReadOnly, SingleLine, TextEditState};
  use cosmic_text::Metrics;

  /// `Editor::new` is FontSystem-free (struct construction over the owned
  /// buffer), so `TextEditState::new` builds without a lock — the same
  /// lock-free construction contract `TextBuffer::new` honors (architecture
  /// § 1.2: TextSync is not a lock site).
  #[test]
  fn text_edit_state_constructs_without_a_font_system() {
      let state = TextEditState::new(Metrics::new(16.0, 19.2));
      // A fresh editor's buffer is empty and unshaped: zero layout runs.
      state.with_buffer(|buffer| {
          assert_eq!(buffer.layout_runs().count(), 0, "fresh editor buffer is unshaped");
      });
      assert_eq!(state.intrinsics(), None, "no intrinsics cached before measure");
  }

  /// The four policy markers are plain zero-size / string components: they
  /// construct, compare, and (Task 1.4) reflect-register. Behavior is E2–E6;
  /// E1 only proves they exist and gate (a query can filter on them).
  #[test]
  fn policy_markers_construct_and_gate() {
      let mut world = World::new();
      let editable = world.spawn(TextEditState::new(Metrics::new(16.0, 19.2))).id();
      let read_only = world
          .spawn((TextEditState::new(Metrics::new(16.0, 19.2)), ReadOnly))
          .id();
      let disabled = world
          .spawn((TextEditState::new(Metrics::new(16.0, 19.2)), Disabled))
          .id();
      let single = world
          .spawn((TextEditState::new(Metrics::new(16.0, 19.2)), SingleLine))
          .id();
      world
          .spawn((
              TextEditState::new(Metrics::new(16.0, 19.2)),
              Placeholder(String::from("type here")),
          ))
          .id();

      // The markers gate: an editable, non-Disabled, non-ReadOnly query.
      let mut q = world.query_filtered::<Entity, (With<TextEditState>, Without<Disabled>, Without<ReadOnly>)>();
      let editable_ids: Vec<Entity> = q.iter(&world).collect();
      assert!(editable_ids.contains(&editable));
      assert!(editable_ids.contains(&single), "SingleLine is still editable");
      assert!(!editable_ids.contains(&read_only), "ReadOnly is filtered out");
      assert!(!editable_ids.contains(&disabled), "Disabled is filtered out");

      // Placeholder carries its string.
      let mut pq = world.query::<&Placeholder>();
      assert_eq!(pq.iter(&world).next().unwrap().0, "type here");
  }
  ```

- [ ] **Step 1.2 — run it, watch it fail.**

  ```sh
  cargo test -p buiy_core --test text_edit_substrate
  ```

  Expected: a **compile error** — `unresolved import buiy_core::text::edit` /
  `TextEditState`, `ReadOnly`, etc. do not exist yet. (A compile failure IS the
  red state for a new-module test.)

- [ ] **Step 1.3 — write `edit/state.rs`.** Create
  `crates/buiy_core/src/text/edit/state.rs`:

  ```rust
  //! `TextEditState` — the editor state machine over `cosmic_text::Editor`
  //! (editing-and-ime § 2.1: wrap `Editor`, do not rebuild it), and the four
  //! decomposed policy markers (§ 2.2). This module is INSIDE the
  //! `text::edit` facade: it is one of the two files allowed to name a cosmic
  //! `Editor`/`Edit` type (the other is `access.rs`); every other Buiy module
  //! reaches the editor's buffer only through `TextBufferAccess`
  //! (`tests/text_facade_boundary.rs` is the tripwire).
  //!
  //! **E1 field set (E1 plan decision 1):** only `editor` and `intrinsics`.
  //! The spec § 2.2 sketch lists `selection`/`preedit`/`undo`/`blink` too, but
  //! each is dead state until its phase reads it — E3 adds `selection` +
  //! `blink`, E4 `undo`, E5 `preedit`, together with the system that consumes
  //! it. No orphan placeholder fields.

  use bevy::prelude::*;
  use cosmic_text::{Buffer, Editor, Metrics};

  use crate::text::IntrinsicWidths;

  /// The editor state machine for an editable text entity (editing-and-ime
  /// § 2.2). Optional on a text entity: entities with only a display
  /// `TextBuffer` never pay for it (editor-optional / buffer-required — the
  /// `TextBufferAccess` dispatch reaches whichever exists).
  ///
  /// **Buffer ownership (§ 2.2a):** the editor wraps `BufferRef::Owned(Buffer)`
  /// — the only `BufferRef` shape that allows mutation (`Borrowed`
  /// self-borrows, which a component cannot do; `Arc` forbids mutation). When
  /// `TextEditState` is present its owned buffer is **authoritative**: the
  /// measure seam, `TextCommit`, the glyph producer, and `TextSync` all reach
  /// it through `TextBufferAccess` (this campaign's `access.rs`), preferring it
  /// over the display-only `TextBuffer.buffer`.
  ///
  /// `Editor` is `Send + Sync` in 0.19 (verified — docs.rs auto-traits), so
  /// this is a plain `Component`, no `NonSend` contortion. Machinery state —
  /// NOT reflect-registered (it carries a `cosmic_text::Editor`, and this
  /// module is the cosmic boundary; the `TextBuffer` precedent,
  /// `components.rs`).
  #[derive(Component)]
  pub struct TextEditState {
      /// The wrapped editor over `BufferRef::Owned`. Private: the only way to
      /// reach its buffer from outside `text::edit` is `TextBufferAccess`.
      pub(crate) editor: Editor<'static>,
      /// Cached intrinsic widths for the AUTHORITATIVE (editor-owned) buffer
      /// (E1 plan decision 3 — moved off `TextBuffer` so the cache keys to the
      /// buffer it describes). `None` until measure computes them, and after
      /// every `TextSync` invalidation. Read/written only through
      /// `TextBufferAccess`'s cache methods.
      pub(crate) intrinsics: Option<IntrinsicWidths>,
  }

  impl TextEditState {
      /// A new editor over an empty, unshaped owned buffer at `metrics`.
      /// FontSystem-free: `Buffer::new_empty` takes no `FontSystem`, and
      /// `Editor::new` is pure struct construction (verified,
      /// `cosmic-text-0.19.0/src/edit/editor.rs:37`) — so construction is NOT
      /// a lock site (architecture § 1.2), mirroring `TextBuffer::new`.
      pub fn new(metrics: Metrics) -> Self {
          Self {
              editor: Editor::new(Buffer::new_empty(metrics)),
              intrinsics: None,
          }
      }

      /// Read the editor's owned buffer. Test/inspection convenience that
      /// stays INSIDE the facade (it lives in `text::edit`); production
      /// readers go through `TextBufferAccess`. Mirrors `Edit::with_buffer`.
      pub fn with_buffer<T>(&self, f: impl FnOnce(&Buffer) -> T) -> T {
          use cosmic_text::Edit;
          self.editor.with_buffer(f)
      }

      /// The cached intrinsics, if valid for the current content version.
      pub fn intrinsics(&self) -> Option<IntrinsicWidths> {
          self.intrinsics
      }
  }

  /// Marker: editable but not mutable — caret + selection + copy yes, mutation
  /// no (editing-and-ime § 2.2). IME stays disabled on a `ReadOnly` editor.
  /// Behavior is E2/E5/E6; E1 only lands the marker.
  #[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
  #[reflect(Component, Default)]
  pub struct ReadOnly;

  /// Marker: no focus, no caret, no IME (editing-and-ime § 2.2). The strongest
  /// suppression: editing systems gate on `not Disabled` (E2+). E1 lands the
  /// marker.
  #[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
  #[reflect(Component, Default)]
  pub struct Disabled;

  /// Marker: Enter ⇒ Submit, `Wrap::None`, newline-stripped paste
  /// (editing-and-ime §§ 2.2, 3.3). Behavior is E2; E1 lands the marker.
  #[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
  #[reflect(Component, Default)]
  pub struct SingleLine;

  /// The placeholder string, shown when the logical value is empty
  /// (editing-and-ime § 10). Rendering is E6; E1 lands the carrier. The string
  /// never enters the editor buffer — it is a display-only Buffer at paint.
  #[derive(Component, Reflect, Default, Clone, PartialEq, Eq, Debug)]
  #[reflect(Component, Default)]
  pub struct Placeholder(pub String);
  ```

- [ ] **Step 1.4 — write `edit/mod.rs` and wire it into `text/mod.rs`.** Create
  `crates/buiy_core/src/text/edit/mod.rs`:

  ```rust
  //! `buiy_core::text::edit` — the editing facade (editing-and-ime § 2.1
  //! "lock-in containment"). This module, and ONLY this module, names the
  //! cosmic `Editor`/`Edit`/`Action`/`Change` types; every other Buiy system
  //! talks to `TextEditState` and `TextBufferAccess`. A future substrate swap
  //! stays local here. The boundary is mechanically enforced by
  //! `tests/text_facade_boundary.rs`.
  //!
  //! E1 lands the substrate: `TextEditState`, the policy markers, and the
  //! `TextBufferAccess` accessor. Input/keymap (E2), caret/selection (E3),
  //! clipboard/undo (E4), IME (E5), and lifecycle/widget (E6) extend it.

  mod access;
  mod state;

  pub use access::{TextBufferAccess, TextBufferAccessItem, TextBufferAccessReadOnlyItem};
  pub use state::{Disabled, Placeholder, ReadOnly, SingleLine, TextEditState};
  ```

  > NOTE for the executor: `access.rs` lands in Task 2. To keep Task 1's commit
  > compiling, create `access.rs` now as a one-line stub —
  > `// TextBufferAccess lands in E1 Task 2.` — and have `mod.rs` re-export only
  > `state` symbols this task (`pub use state::{…};`), adding the `access`
  > re-exports in Task 2. Do NOT reference `access` symbols from `mod.rs` until
  > they exist.

  Then in `crates/buiy_core/src/text/mod.rs`, add the submodule and re-export.
  After the existing `mod direction;` line (alphabetical neighbor), add:

  ```rust
  mod edit;
  ```

  and add a re-export block near the other `pub use` lines (after the
  `pub use direction::…` line):

  ```rust
  pub use edit::{Disabled, Placeholder, ReadOnly, SingleLine, TextEditState};
  ```

  Register the four markers for reflection in `BuiyTextPlugin::build` — extend the
  existing `register_type` chain (after `.register_type::<TextDirection>()`):

  ```rust
              .register_type::<crate::text::edit::ReadOnly>()
              .register_type::<crate::text::edit::Disabled>()
              .register_type::<crate::text::edit::SingleLine>()
              .register_type::<crate::text::edit::Placeholder>()
  ```

- [ ] **Step 1.5 — run it, watch it pass.**

  ```sh
  cargo test -p buiy_core --test text_edit_substrate
  ```

  Expected: `test_edit_state_constructs_without_a_font_system` and
  `policy_markers_construct_and_gate` both pass (`test result: ok. 2 passed`).

- [ ] **Step 1.6 — add the marker-registration assertion** to
  `tests/text_edit_substrate.rs` (mirrors `text_components.rs`'s
  `authoring_types_are_registered_for_reflection`):

  ```rust
  use buiy_core::text::BuiyTextPlugin;

  /// The policy markers reflect-register (BSN / inspectors — the
  /// authoring-surface convention `Text` follows).
  #[test]
  fn policy_markers_are_registered_for_reflection() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(BuiyTextPlugin::default());
      app.update();
      let registry = app.world().resource::<AppTypeRegistry>().read();
      for name in [
          "buiy_core::text::edit::state::ReadOnly",
          "buiy_core::text::edit::state::Disabled",
          "buiy_core::text::edit::state::SingleLine",
          "buiy_core::text::edit::state::Placeholder",
      ] {
          assert!(
              registry.get_with_type_path(name).is_some(),
              "marker not registered: {name}",
          );
      }
  }
  ```

  Run `cargo test -p buiy_core --test text_edit_substrate` — expect 3 passed.
  (If a type path differs, fix the string to the path the panic prints — the
  module is `text::edit::state`.)

- [ ] **Step 1.7 — commit.**

  ```sh
  git add -A && git commit -m "feat(text): E1.1 — text::edit facade, TextEditState, policy markers"
  ```

---

## Task 2 — the `TextBufferAccess` accessor (the seam)

The accessor is the heart of E1: one `QueryData` that dispatches buffer access
**editor-first**, so every call site stays buffer-shaped and the editor never
leaks. Both forms (mutable + read-only) and the intrinsics-cache dispatch
(decision 3/4) land here, unit-tested directly against a `World`.

- [ ] **Step 2.1 — write the failing accessor test.** Append to
  `tests/text_edit_substrate.rs`:

  ```rust
  use buiy_core::text::edit::TextBufferAccess;
  use buiy_core::text::{IntrinsicWidths, TextBuffer};

  /// On a DISPLAY-ONLY entity (`TextBuffer`, no `TextEditState`) the accessor
  /// routes to `TextBuffer.buffer` and `TextBuffer`'s intrinsics slot.
  #[test]
  fn accessor_routes_to_display_buffer_when_no_editor() {
      let mut world = World::new();
      let e = world.spawn(TextBuffer::new(Metrics::new(16.0, 19.2))).id();

      let mut q = world.query::<TextBufferAccess>();
      let mut item = q.get_mut(&mut world, e).unwrap();
      // Cache round-trips through the accessor onto the display component.
      assert_eq!(item.intrinsics(), None);
      item.cache_intrinsics(IntrinsicWidths { min_content: 3.0, max_content: 9.0 });
      assert_eq!(item.intrinsics(), Some(IntrinsicWidths { min_content: 3.0, max_content: 9.0 }));
      item.with_buffer_mut(|buffer| buffer.set_size(Some(120.0), None));
      item.with_buffer(|buffer| assert_eq!(buffer.size().0, Some(120.0)));

      // The write landed on the display component (proof it routed there).
      let tb = world.get::<TextBuffer>(e).unwrap();
      assert_eq!(tb.buffer.size().0, Some(120.0));
      assert_eq!(tb.intrinsics(), Some(IntrinsicWidths { min_content: 3.0, max_content: 9.0 }));
  }

  /// On an EDITABLE entity (both components present) the accessor PREFERS the
  /// editor-owned buffer and the editor's intrinsics slot; the display
  /// component is untouched.
  #[test]
  fn accessor_prefers_editor_buffer_when_present() {
      let mut world = World::new();
      let e = world
          .spawn((
              TextBuffer::new(Metrics::new(16.0, 19.2)),
              TextEditState::new(Metrics::new(16.0, 19.2)),
          ))
          .id();

      let mut q = world.query::<TextBufferAccess>();
      let mut item = q.get_mut(&mut world, e).unwrap();
      item.cache_intrinsics(IntrinsicWidths { min_content: 7.0, max_content: 11.0 });
      item.with_buffer_mut(|buffer| buffer.set_size(Some(250.0), None));
      item.with_buffer(|buffer| assert_eq!(buffer.size().0, Some(250.0)));

      // The editor buffer got the write + cache; the DISPLAY buffer did not.
      let tb = world.get::<TextBuffer>(e).unwrap();
      assert_eq!(tb.buffer.size().0, None, "display buffer untouched (editor is authoritative)");
      assert_eq!(tb.intrinsics(), None, "display cache untouched");
      let state = world.get::<TextEditState>(e).unwrap();
      state.with_buffer(|buffer| assert_eq!(buffer.size().0, Some(250.0)));
      assert_eq!(state.intrinsics(), Some(IntrinsicWidths { min_content: 7.0, max_content: 11.0 }));
  }

  /// `invalidate_intrinsics` clears whichever side is authoritative.
  #[test]
  fn accessor_invalidate_clears_the_authoritative_cache() {
      let mut world = World::new();
      let e = world
          .spawn((
              TextBuffer::new(Metrics::new(16.0, 19.2)),
              TextEditState::new(Metrics::new(16.0, 19.2)),
          ))
          .id();
      let mut q = world.query::<TextBufferAccess>();
      let mut item = q.get_mut(&mut world, e).unwrap();
      item.cache_intrinsics(IntrinsicWidths { min_content: 1.0, max_content: 2.0 });
      assert!(item.intrinsics().is_some());
      item.invalidate_intrinsics();
      assert_eq!(item.intrinsics(), None);
  }

  /// The bypass-change-detection contract on the EDITOR arm of
  /// `with_buffer_mut` (measure § 7): a width probe is not a damage signal, so
  /// a mutable buffer write through the accessor must NOT tick
  /// `Changed<TextEditState>`. Guards the editor arm DIRECTLY — the steady-frame
  /// parity test (Task 4) cannot, because nothing reads `Changed<TextEditState>`
  /// in E1 and Taffy's layout cache keeps the measure closure from running on a
  /// no-change frame (verified by mutation: dropping the editor-arm bypass
  /// leaves the steady-frame test green). Mirrors `render_clip_rects.rs:420-425`.
  #[test]
  fn with_buffer_mut_bypasses_change_detection_on_the_editor_arm() {
      let mut world = World::new();
      let e = world
          .spawn((
              TextBuffer::new(Metrics::new(16.0, 19.2)),
              TextEditState::new(Metrics::new(16.0, 19.2)),
          ))
          .id();
      // Advance the change-detection baseline past the spawn tick.
      world.clear_trackers();

      let mut q = world.query::<TextBufferAccess>();
      let mut item = q.get_mut(&mut world, e).unwrap();
      item.with_buffer_mut(|buffer| buffer.set_size(Some(180.0), None));
      let state = world.get::<TextEditState>(e).unwrap();
      state.with_buffer(|buffer| assert_eq!(buffer.size().0, Some(180.0)));

      // The write did NOT tick `Changed<TextEditState>` (a `DerefMut` would).
      let mut rq = world.query::<Ref<TextEditState>>();
      let state_ref = rq.get(&world, e).expect("editor entity has TextEditState");
      assert!(
          !state_ref.is_changed(),
          "with_buffer_mut on the editor arm must bypass change detection",
      );
  }
  ```

  > The mutable-access bypass-change-detection contract is verified DIRECTLY by
  > `with_buffer_mut_bypasses_change_detection_on_the_editor_arm` (a
  > `Ref::is_changed()` probe across a `clear_trackers()` baseline, mirroring
  > `tests/render_clip_rects.rs:420-425`). The steady-frame
  > `TextMeasureCallCount == 0` parity test (Task 4) does NOT cover it: nothing
  > in the crate reads `Changed<TextEditState>` in E1, and on a no-change frame
  > Taffy's layout cache keeps the measure closure (the only accessor caller)
  > from running — so a broken editor-arm bypass leaves that test green
  > (confirmed by mutation). The steady-frame test proves convergence (O(0)
  > re-measure), not the bypass; the direct unit test guards the bypass.

- [ ] **Step 2.2 — run it, watch it fail.**

  ```sh
  cargo test -p buiy_core --test text_edit_substrate
  ```

  Expected: compile error — `TextBufferAccess` unresolved, `cache_intrinsics`/
  `with_buffer_mut`/`invalidate_intrinsics` not found.

- [ ] **Step 2.3 — make `IntrinsicWidths` + `TextBuffer.buffer` cache reachable.**
  The accessor needs to write `TextBuffer`'s private `intrinsics` field. Add
  `pub(crate)` methods on `TextBuffer` if not already present — `intrinsics()`,
  `cache_intrinsics`, and `invalidate_intrinsics` already exist
  (`components.rs:536-549`); `cache_intrinsics`/`invalidate_intrinsics` are
  `pub(crate)`, so `text::edit` (same crate) can call them. No change needed to
  `components.rs` here. (`TextBuffer.buffer` is already `pub`.) `IntrinsicWidths`
  is already `pub` (`components.rs:557`) and re-exported (`mod.rs:39`).

- [ ] **Step 2.4 — write `edit/access.rs`.** Replace the stub:

  ```rust
  //! `TextBufferAccess` — the one accessor every system uses to reach "the
  //! entity's buffer" (measure-and-layout § 2.3; editing-and-ime § 2.2a). It
  //! binds the display `TextBuffer` and the optional `TextEditState`, and
  //! dispatches buffer reads/writes and the intrinsics cache **editor-first**:
  //! when `TextEditState` is present its owned `Buffer` is authoritative
  //! (`BufferRef::Owned`), else the display `TextBuffer.buffer`. Display-only
  //! and editable entities take the same code path; compatibility with
  //! `BufferRef::Owned` holds by construction.
  //!
  //! This file is INSIDE the `text::edit` facade — one of the two allowed to
  //! name `Edit`. The `with_buffer*` methods hand callers a bare
  //! `&Buffer`/`&mut Buffer`, so sync/measure/commit/extract stay
  //! buffer-shaped and never name a cosmic editor type
  //! (`tests/text_facade_boundary.rs` is the tripwire).
  //!
  //! **Change-detection (measure § 7):** a width probe is not a damage signal.
  //! Mutable buffer access bypasses change detection on BOTH the `TextBuffer`
  //! and `TextEditState` members, so the measure/commit/sync writes never tick
  //! `Changed<TextBuffer>` / `Changed<TextEditState>` — damage keys on the
  //! commit OUTPUT components (`ComputedTextLayout`), the existing contract.

  use bevy::ecs::query::QueryData;
  use cosmic_text::{Buffer, Edit};

  use super::state::TextEditState;
  use crate::text::{IntrinsicWidths, TextBuffer};

  /// The shared buffer accessor (measure-and-layout § 2.3). `#[query_data(
  /// mutable)]` generates the read-only companion (`TextBufferAccessReadOnly`)
  /// automatically — extract binds that form.
  #[derive(QueryData)]
  #[query_data(mutable)]
  pub struct TextBufferAccess {
      /// The display-only buffer — authoritative iff `edit` is `None`.
      display: &'static mut TextBuffer,
      /// The editor — authoritative when present (§ 2.2a).
      edit: Option<&'static mut TextEditState>,
  }

  // NOTE: in Bevy 0.18.1 `#[derive(QueryData)]` generates the item struct
  // with TWO lifetimes — `Item<'__w, '__s>` (world + state)
  // (`bevy_ecs_macros-0.18.1/src/query_data.rs:72-81,309`). So the item type
  // is `TextBufferAccessItem<'_, '_>` everywhere it is NAMED (the `impl` line
  // here, the `SyncedTextItem` member in Step 3.1). Method params like
  // `&mut TextBufferAccessItem` elide fine.
  impl TextBufferAccessItem<'_, '_> {
      /// Read the authoritative buffer (editor-owned if present, else the
      /// display buffer). `&self`: read-only, no tick.
      pub fn with_buffer<T>(&self, f: impl FnOnce(&Buffer) -> T) -> T {
          match self.edit.as_ref() {
              Some(state) => state.editor.with_buffer(f),
              None => f(&self.display.buffer),
          }
      }

      /// Mutate the authoritative buffer. Bypasses change detection on
      /// whichever side is authoritative (measure § 7).
      pub fn with_buffer_mut<T>(&mut self, f: impl FnOnce(&mut Buffer) -> T) -> T {
          match self.edit.as_mut() {
              Some(state) => {
                  let state = state.bypass_change_detection();
                  state.editor.with_buffer_mut(f)
              }
              None => {
                  let display = self.display.bypass_change_detection();
                  f(&mut display.buffer)
              }
          }
      }

      /// The cached intrinsics for the authoritative buffer (decision 3 — the
      /// cache lives with the buffer it describes).
      pub fn intrinsics(&self) -> Option<IntrinsicWidths> {
          match self.edit.as_ref() {
              Some(state) => state.intrinsics,
              None => self.display.intrinsics(),
          }
      }

      /// Fill the authoritative cache (the measure closure is the only
      /// writer). Bypasses change detection (a probe is not damage).
      pub fn cache_intrinsics(&mut self, widths: IntrinsicWidths) {
          match self.edit.as_mut() {
              Some(state) => state.bypass_change_detection().intrinsics = Some(widths),
              None => self.display.bypass_change_detection().cache_intrinsics(widths),
          }
      }

      /// Invalidate the authoritative cache (every content change — `TextSync`).
      pub fn invalidate_intrinsics(&mut self) {
          match self.edit.as_mut() {
              Some(state) => state.bypass_change_detection().intrinsics = None,
              None => self.display.bypass_change_detection().invalidate_intrinsics(),
          }
      }
  }

  impl TextBufferAccessReadOnlyItem<'_, '_> {
      /// Read the authoritative buffer (the extract producer's form). The
      /// editor's `Edit::with_buffer` is `&self`, so this stays read-only —
      /// the `Extract` main-world read-only contract (architecture § 4.4).
      pub fn with_buffer<T>(&self, f: impl FnOnce(&Buffer) -> T) -> T {
          match self.edit.as_ref() {
              Some(state) => state.editor.with_buffer(f),
              None => f(&self.display.buffer),
          }
      }
  }
  ```

  > Bevy names the generated item types `<Name>Item` and `<Name>ReadOnlyItem`,
  > and the read-only companion struct `<Name>ReadOnly`. Confirm the exact
  > names the macro generates (the compiler error on a wrong name prints the
  > right one); the lifetime on the item types may be `<'w>` / `<'_>` depending
  > on the Bevy version — match what compiles.

  Then add to `edit/mod.rs`:

  ```rust
  pub use access::{TextBufferAccess, TextBufferAccessItem, TextBufferAccessReadOnlyItem};
  ```

  and to `text/mod.rs`'s edit re-export:

  ```rust
  pub use edit::{
      Disabled, Placeholder, ReadOnly, SingleLine, TextBufferAccess, TextEditState,
  };
  ```

- [ ] **Step 2.5 — run it, watch it pass.**

  ```sh
  cargo test -p buiy_core --test text_edit_substrate
  ```

  Expected: all accessor tests pass (7 passed total with Task 1's tests — the
  four accessor tests including the editor-arm bypass guard, plus Task 1's three).

- [ ] **Step 2.6 — commit.**

  ```sh
  git add -A && git commit -m "feat(text): E1.2 — TextBufferAccess editor-first buffer + intrinsics seam"
  ```

---

## Task 3 — route the four binders through `TextBufferAccess`

The mechanical swap: `text_sync_buffers`, the measure closure, `text_commit`, and
`extract_buiy_glyphs` stop binding `&mut TextBuffer` / `&TextBuffer` directly and
bind `TextBufferAccess` (read-only form for extract). Display-only entities are
unchanged (the accessor falls back); editor entities now route to the
authoritative buffer. This is the load-bearing refactor — Task 4 proves it's
transparent.

> **TDD note.** This task has no new behavioral test of its own — its correctness
> IS the Task 4 parity suite (an editor entity matching a display entity end to
> end). Task 3 is "make the existing green tests stay green through the rebinding,
> then Task 4's new tests prove the editor path." After EACH sub-step, run the
> existing suite for that file and confirm it stays green:
>
> ```sh
> cargo test -p buiy_core --test text_sync --test text_measure --test text_commit --test text_extract
> ```

- [ ] **Step 3.1 — route `apply_authored` (sync.rs) through the accessor.** The
  cleanest minimal change: keep `apply_authored`'s `&mut Buffer` shape but call it
  via the accessor so the editor buffer is the target when present.

  In `text/sync.rs`, change `apply_authored`'s signature from
  `buffer: &mut TextBuffer` to take a closure-friendlier shape. Concretely, split
  the buffer mutation from the intrinsics invalidation so both go through the
  accessor. Replace the `apply_authored(&mut TextBuffer, …)` calls in `sync_one`
  and the creation loop with accessor-routed calls.

  The creation loop (`sync.rs:178-215`) builds a fresh `TextBuffer` and calls
  `apply_authored(&mut buffer, …)` BEFORE inserting it — there is no
  `TextEditState` to route to at that point (creation only ever makes a
  display-`TextBuffer` entity; editor entities are spawned with `TextEditState`
  already present by E2+ / tests). For creation, KEEP the direct
  `apply_authored(&mut buffer, …)` on the freshly-built `TextBuffer` — the new
  entity has no editor yet, so the display buffer IS authoritative. Add a doc line
  noting this is the only direct binder left and it is correct (the entity is
  display-only at insert time).

  Change `sync_one` (`sync.rs:267-306`) to bind `TextBufferAccess` instead of
  `&mut TextBuffer`. Note there are TWO paired aliases to update in lockstep
  (`sync.rs:92-130`): `SyncedText` (the `QueryData`, member 2 is
  `&'static mut TextBuffer`) and `SyncedTextItem<'w>` (the item type, member 2 is
  `Mut<'w, TextBuffer>`). Replace member 2 in `SyncedText` with `TextBufferAccess`
  and member 2 in `SyncedTextItem<'w>` with `TextBufferAccessItem<'w, '_>` — the
  generated item carries TWO lifetimes in 0.18 (`Item<'__w, '__s>`, world + state;
  see the M1 note in Step 2.4), so a single-lifetime spelling won't compile. The
  `synced` `ParamSet`
  (`sync.rs:141`) is built from `SyncedText`, so it follows automatically. Because
  `apply_authored` writes `set_metrics`/`set_wrap`/`set_text` then
  `invalidate_intrinsics`, restructure it to take `&mut Buffer` for the writes and
  call `access.invalidate_intrinsics()` separately:

  ```rust
  // in sync_one, replacing the `buffer.bypass_change_detection()` block:
  let mut access = item_access; // the TextBufferAccess query member
  let style = AuthoredStyle::resolve(/* … unchanged … */);
  let blocked = access.with_buffer_mut(|buffer| {
      apply_authored_to_buffer(buffer, text, &style, ctx.registry, ctx.index, ctx.now)
  });
  access.invalidate_intrinsics();
  ```

  Rename `apply_authored(buffer: &mut TextBuffer, …)` to
  `apply_authored_to_buffer(buffer: &mut Buffer, …)`, dropping the
  `buffer.invalidate_intrinsics()` line from its body (the accessor does it now)
  and changing every `buffer.buffer.set_*` to `buffer.set_*` (it's now a bare
  `&mut Buffer`). Keep the creation-loop caller building a `TextBuffer` and call
  `apply_authored_to_buffer(&mut buffer.buffer, …)` + `buffer.invalidate_intrinsics()`
  on it directly.

  > **Why `SyncedText` must bind the accessor, not bare `&mut TextBuffer`:** an
  > editor entity has BOTH `Text` and `TextEditState`; its `set_text` must land on
  > the editor buffer or the editor renders nothing. The accessor's `Option<&mut
  > TextEditState>` makes the query still match display-only entities (the option
  > is `None`), so no entity is dropped from the sync set.

- [ ] **Step 3.2 — run the sync suite, confirm green.**

  ```sh
  cargo test -p buiy_core --test text_sync
  ```

  Expected: all existing `text_sync` tests pass (the display path is unchanged —
  `Option<&mut TextEditState>` is `None` for every entity these tests spawn).

- [ ] **Step 3.3 — route the measure closure (measure.rs) through the accessor.**
  In `text/measure.rs`, change `TextMeasureParam.buffers` from
  `Query<(&'static mut TextBuffer, Option<&'static BoxModel>)>` to
  `Query<(TextBufferAccess, Option<&'static BoxModel>)>`. Update `measure_text_node`
  to take the accessor item: replace the `text.bypass_change_detection()` +
  `text.buffer.set_size(…)` + `cached_intrinsics(text, …)` calls with accessor
  routing:

  ```rust
  let Ok((mut access, box_model)) = buffers.get_mut(entity) else {
      return TaffySize::ZERO;
  };
  let intrinsics = cached_intrinsics(&mut access, font_system);
  let keyword_width = box_model.and_then(|bm| match bm.width {
      Sizing::MinContent => Some(intrinsics.min_content),
      Sizing::MaxContent => Some(intrinsics.max_content),
      _ => None,
  });
  let width = known_dimensions.width.or(keyword_width).unwrap_or(match available_space.width {
      AvailableSpace::MinContent => intrinsics.min_content,
      AvailableSpace::MaxContent => intrinsics.max_content,
      AvailableSpace::Definite(w) => w,
  });
  let (max_w, total_h) = access.with_buffer_mut(|buffer| {
      buffer.set_size(Some(width), None);
      buffer.shape_until_scroll(font_system, false);
      fold_runs(buffer)
  });
  TaffySize { width: max_w.ceil(), height: total_h.ceil() }
  ```

  Rewrite `cached_intrinsics` to take `&mut TextBufferAccessItem` and route both
  the cache and the buffer through it:

  ```rust
  fn cached_intrinsics(access: &mut TextBufferAccessItem, font_system: &mut FontSystem) -> IntrinsicWidths {
      if let Some(cached) = access.intrinsics() {
          return cached;
      }
      let widths = access.with_buffer_mut(|buffer| {
          buffer.set_size(Some(0.0), None);
          buffer.shape_until_scroll(font_system, false);
          let min_content = fold_runs(buffer).0;
          buffer.set_size(None, None);
          buffer.shape_until_scroll(font_system, false);
          let max_content = fold_runs(buffer).0;
          IntrinsicWidths { min_content, max_content }
      });
      access.cache_intrinsics(widths);
      widths
  }
  ```

  Update the `use` lines: add `use super::edit::TextBufferAccess;` and the item
  type, drop the now-unused direct `TextBuffer` import if nothing else needs it
  (`IntrinsicWidths` import stays).

- [ ] **Step 3.4 — run the measure suite, confirm green.**

  ```sh
  cargo test -p buiy_core --test text_measure
  ```

  Expected: all `text_measure` tests pass (display entities route through the
  `None` arm — identical shaping, identical cache behavior).

- [ ] **Step 3.5 — route `text_commit` (commit.rs) through the accessor.** In
  `text/commit.rs`, change the `texts` query's `&mut TextBuffer` member to
  `TextBufferAccess`. Replace `text.bypass_change_detection()` +
  `text.buffer.lines` / `text.buffer.set_size` / `text.buffer.shape_until_scroll`
  / `text.buffer.size()` with accessor routing. The per-line `set_align` loop and
  the final-width reshape both go inside one `with_buffer_mut`; the steady-state
  short-circuit reads `buffer.size()` inside a `with_buffer` first:

  ```rust
  for (entity, mut access, align, existing_layout, existing_baseline) in texts.iter_mut() {
      let Some(&node) = tree.by_entity.get(&entity) else { continue; };
      let Ok(layout) = tree.tree.layout(node) else { continue; };
      let content = layout.content_box_size();
      let target = (Some(content.width.max(0.0)), Some(content.height.max(0.0)));
      let content_offset = Vec2::new(
          layout.border.left + layout.padding.left,
          layout.border.top + layout.padding.top,
      );
      let align = align.copied().unwrap_or_default().to_cosmic();
      let align_changed = access.with_buffer_mut(|buffer| {
          let mut changed = false;
          for line in buffer.lines.iter_mut() {
              changed |= line.set_align(align);
          }
          changed
      });
      let offset_stale = existing_layout.is_none_or(|c| c.content_offset != content_offset);
      let size_stale = access.with_buffer(|buffer| buffer.size() != target);
      if !align_changed && !offset_stale && !size_stale {
          continue;
      }
      let font_system = font_system.get_or_insert_with(|| fonts.lock());
      let (computed, baseline) = access.with_buffer_mut(|buffer| {
          buffer.set_size(target.0, target.1);
          buffer.shape_until_scroll(font_system, false);
          computed_outputs(buffer, content_offset)
      });
      reshaped.0 += 1;
      // … the idempotent-insert blocks for `computed` / `baseline`, unchanged …
  }
  ```

  `computed_outputs(buffer: &cosmic_text::Buffer, …)` already takes a bare
  `&Buffer` — no change. The `bypass_change_detection` is now inside the accessor.

- [ ] **Step 3.6 — run the commit suite, confirm green.**

  ```sh
  cargo test -p buiy_core --test text_commit
  ```

  Expected: all `text_commit` tests pass (including the steady-frame
  `TextCommitReshapeCount == 0` and `TextMeasureCallCount == 0` assertions —
  the accessor's bypass keeps the no-change frame O(0)).

- [ ] **Step 3.7 — route `extract_buiy_glyphs` (extract.rs) through the read-only
  accessor.** In `text/extract.rs`, change the `&TextBuffer` member of the `texts`
  `Extract<Query<…>>` to the read-only accessor form
  (`TextBufferAccessReadOnly`). The two read sites
  (`buffer.buffer.size()` at line ~419 and `buffer.buffer.layout_runs()` at lines
  ~420, ~473) become `access.with_buffer(|buffer| …)`:

  ```rust
  // the destructured tuple member `buffer` becomes `access`
  // full_w computation:
  let full_w = access.with_buffer(|buffer| buffer.size().0).unwrap_or(computed.size.x);
  // the run loops:
  access.with_buffer(|buffer| {
      for run in buffer.layout_runs() {
          // … existing per-run emission, unchanged …
      }
  });
  ```

  The `changed` and removal queries that filter on `With<TextBuffer>` (lines
  ~206-247) STAY as-is — they gate on the display component's presence as a "this
  is a text entity" filter. An editor entity also carries `TextBuffer` (decision:
  editor entities are spawned with BOTH a display `TextBuffer` and `TextEditState`,
  per the accessor's display+optional-edit shape), so the `With<TextBuffer>` gate
  still matches them. (Confirm in Task 4 that the editor test entity carries
  `TextBuffer` — it does, because `TextSync`'s creation loop inserts one whenever
  `Text` is present, and the editor test spawns `Text` + `TextEditState`.)

  > **Subtle but critical:** the extract `changed` trigger union keys on
  > `Changed<ComputedTextLayout>` etc., NOT on `Changed<TextEditState>`. Because
  > `TextCommit` writes `ComputedTextLayout` for editor entities exactly as for
  > display entities (it routes through the same accessor), the editor entity's
  > glyph damage fires through the identical signal. No new trigger member is
  > needed in E1 — confirmed by Task 4's editor-glyph parity test.

- [ ] **Step 3.8 — run the extract suite + the GPU-lane build, confirm green.**

  ```sh
  cargo test -p buiy_core --test text_extract
  ```

  Expected: all `text_extract` tests pass (display entities unchanged). Then
  confirm the GPU-lane tests still **compile** (they're `#[ignore]`d, so this is a
  build check, not a run):

  ```sh
  cargo test -p buiy_core --no-run
  ```

  Expected: clean build, no warnings.

- [ ] **Step 3.9 — commit.**

  ```sh
  git add -A && git commit -m "feat(text): E1.3 — route sync/measure/commit/extract through TextBufferAccess"
  ```

---

## Task 4 — the transparency parity suite (the flagship invariant)

The campaign's E1 test surface, made concrete: an editor entity shapes, measures,
and emits glyphs **identically** to the equivalent display entity, and display-only
entities are unaffected. This is what proves the seam is transparent.

- [ ] **Step 4.1 — write the failing parity tests.** Append to
  `tests/text_edit_substrate.rs` (these need the full layout+text app, so add the
  `text_app`/`settle` helpers mirroring `text_measure.rs:22-37`):

  ```rust
  use buiy_core::layout::{LayoutPlugin, Style};
  use buiy_core::text::{ComputedTextLayout, Text, TextMeasureCallCount};
  use buiy_core::{CorePlugin, Node, ResolvedLayout};

  fn text_app() -> App {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(LayoutPlugin);
      app.add_plugins(BuiyTextPlugin::default());
      app
  }

  fn settle(app: &mut App) {
      app.update();
      app.update();
  }

  /// THE flagship invariant: an entity with `TextEditState` (+ `Text`) shapes,
  /// measures, and lays out IDENTICALLY to the equivalent display-only entity
  /// — because both route through `TextBufferAccess`, editor-preferred. The
  /// editor's owned buffer is the one that gets the text and produces the
  /// layout.
  #[test]
  fn editor_entity_lays_out_identically_to_display_entity() {
      let mut app = text_app();
      let display = app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::from("hello editor world"))))
          .id();
      let editor = app
          .world_mut()
          .spawn((
              Node,
              Style::default(),
              Text(String::from("hello editor world")),
              TextEditState::new(Metrics::new(16.0, 19.2)),
          ))
          .id();
      // Two FlexStart rows so cross-axis stretch doesn't mask measured height.
      for child in [display, editor] {
          app.world_mut()
              .spawn((
                  Node,
                  Style::default()
                      .flex_row()
                      .align_items(buiy_core::layout::AlignItems::FlexStart)
                      .width_px(600.0)
                      .height_px(100.0),
              ))
              .add_child(child);
      }
      settle(&mut app);

      let d_layout = app.world().get::<ResolvedLayout>(display).unwrap().size;
      let e_layout = app.world().get::<ResolvedLayout>(editor).unwrap().size;
      assert_eq!(d_layout, e_layout, "editor entity sizes identically to display");

      let d_computed = app.world().get::<ComputedTextLayout>(display).unwrap().clone();
      let e_computed = app.world().get::<ComputedTextLayout>(editor).unwrap().clone();
      assert_eq!(d_computed, e_computed, "identical settled line geometry");

      // The editor's OWNED buffer is the one that holds the text (proof the
      // accessor routed sync + measure + commit to it, not the display buffer).
      let state = app.world().get::<TextEditState>(editor).unwrap();
      state.with_buffer(|buffer| {
          assert!(buffer.layout_runs().next().is_some(), "editor buffer is shaped");
          assert!(buffer.size().0.is_some(), "editor buffer committed at a final width");
      });
  }

  /// Display-only entities are unaffected by the editor seam: a frame with
  /// BOTH an editor and a display entity still measures each exactly once on
  /// change, and the steady frame measures ZERO — the seam CONVERGES (no
  /// perpetual re-shape). This proves convergence, not the change-detection
  /// bypass: nothing reads `Changed<TextEditState>` in E1 and Taffy's layout
  /// cache keeps the measure closure from running on a no-change frame, so the
  /// editor-arm bypass is unobservable here — it is guarded directly by
  /// `with_buffer_mut_bypasses_change_detection_on_the_editor_arm` (Task 2).
  #[test]
  fn the_seam_preserves_the_zero_measure_steady_frame() {
      let mut app = text_app();
      let editor = app
          .world_mut()
          .spawn((
              Node,
              Style::default(),
              Text(String::from("steady")),
              TextEditState::new(Metrics::new(16.0, 19.2)),
          ))
          .id();
      app.world_mut()
          .spawn((Node, Style::default(), Text(String::from("steady"))));
      // a parent so they lay out
      app.world_mut()
          .spawn((Node, Style::default().flex_column().width_px(400.0).height_px(200.0)))
          .add_child(editor);
      settle(&mut app);
      // Two-flush discipline (the `text_commit.rs:250-253`
      // steady_state_zero_measure_calls_and_zero_reshapes pattern): the editor
      // entity's `Added<TextBuffer>` echo fires on frame 2 (editor-first
      // re-sync fills the editor buffer + invalidates intrinsics → one more
      // re-measure), so the editor path converges ONE frame later than a
      // display entity. Flush the cascade remnant, THEN assert on the truly
      // steady frame — a single `update()` here would flake frame-3-vs-frame-4.
      app.update(); // flush the creation-echo remnant
      app.update(); // THE steady frame
      assert_eq!(
          app.world().resource::<TextMeasureCallCount>().0,
          0,
          "no-change frame measures zero — the editor seam converges (no perpetual re-shape)",
      );
  }
  ```

  > The load-bearing assertion in `editor_entity_lays_out_identically_to_display_entity`
  > is the `ComputedTextLayout` equality — keep that exact. The editor-buffer
  > checks below it (`layout_runs().next().is_some()`, `size().0.is_some()`) only
  > prove the accessor routed sync+measure+commit to the editor buffer, not the
  > display one; they are deliberately width-value-agnostic to avoid a brittle
  > float compare.

- [ ] **Step 4.2 — run it, watch it pass.**

  ```sh
  cargo test -p buiy_core --test text_edit_substrate
  ```

  Expected: every test passes — the parity tests confirm the editor entity is
  transparent through the whole layout pipeline, and the steady-frame test
  confirms O(0) survives. If the editor entity's `ComputedTextLayout` differs from
  the display entity's, the seam is NOT routing one of the four binders correctly
  — re-check Task 3 (most likely sync did not write the editor buffer, leaving it
  empty → zero runs → zero size).

- [ ] **Step 4.3 — add the editor-glyph parity check.** Editor entities must emit
  glyphs through the same producer. Add a render-world extract test mirroring an
  existing `text_extract` fixture — confirm the editor entity contributes glyph
  instances to `ExtractedGlyphs` identically to a display entity. Look at
  `tests/text_extract.rs` for the established render-app harness pattern (it drives
  `ExtractSchedule` and reads `ExtractedGlyphs`); add a test there OR in
  `text_edit_substrate.rs` if the harness is reusable, asserting the editor
  entity's `entity_runs` entry has the same glyph count as the display entity's.

  > If the render-app harness in `text_extract.rs` is not trivially reusable from
  > a second test file, add the test INSIDE `text_extract.rs` (it already has the
  > harness + imports) rather than duplicating ~40 lines of render-app setup. The
  > assertion: spawn a display entity and an editor entity with identical `Text`,
  > settle, run extract, and assert their `GlyphEntityRun` glyph spans have equal
  > length.

- [ ] **Step 4.4 — run the glyph parity test.**

  ```sh
  cargo test -p buiy_core --test text_extract --test text_edit_substrate
  ```

  Expected: pass — the editor entity emits the same glyph run as the display
  entity.

- [ ] **Step 4.5 — commit.**

  ```sh
  git add -A && git commit -m "feat(text): E1.4 — editor/display layout+glyph transparency parity suite"
  ```

---

## Task 5 — the facade-boundary tripwire

The campaign's structural invariant: no symbol outside `text::edit` names
`Editor`/`Edit`/`Action`/`Change`. A grep-based test makes it mechanical and
catches the next phase's accidental leak.

- [ ] **Step 5.1 — write the failing boundary test.** Create
  `crates/buiy_core/tests/text_facade_boundary.rs`:

  ```rust
  //! The lock-in containment tripwire (editing-and-ime § 2.1): the cosmic
  //! `Editor`/`Edit`/`Action`/`Change` types are named ONLY inside
  //! `crates/buiy_core/src/text/edit/`. A leak anywhere else widens the
  //! bridge surface the bevy-cosmic-edit post-mortem warns against — this test
  //! fails the build the moment it happens. Greps SOURCE, not the running app:
  //! the boundary is a compile-time architectural fact, so a source scan is
  //! the right tier (it needs no World, no plugins, no adapter).

  use std::path::{Path, PathBuf};

  /// The four cosmic editor type identifiers the facade contains. Matched as
  /// WHOLE WORDS (word-boundary), NOT substrings — because this codebase uses
  /// grouped imports pervasively (`use cosmic_text::{Buffer, Editor};`,
  /// e.g. `sync.rs:29`, `extract.rs:19`, `components.rs:10`). A substring
  /// match on `"cosmic_text::Editor"` would MISS `cosmic_text::{Buffer,
  /// Editor}` entirely — a real leak passing silently. The scan normalizes
  /// grouped-import braces first (below), then checks bare identifiers.
  ///
  /// `Edit` is special-cased: as a bare word it collides with the prefix of
  /// `Editor`/`EditState`/`TextEditState` and with `edit` in paths, so it is
  /// ONLY flagged in a `cosmic_text::`-qualified position (the normalized form
  /// makes that exact). The facade subtree (`text/edit/`) is exempt entirely.
  const FORBIDDEN: &[&str] = &["Editor", "Edit", "Action", "Change"];

  fn src_dir() -> PathBuf {
      Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
  }

  /// Recursively collect `.rs` files under `dir`, skipping the `text/edit/`
  /// facade subtree (the one place these types are allowed).
  fn rust_files_outside_facade(dir: &Path, facade: &Path, out: &mut Vec<PathBuf>) {
      for entry in std::fs::read_dir(dir).unwrap() {
          let path = entry.unwrap().path();
          if path.is_dir() {
              if path == facade {
                  continue; // the facade is exempt by definition
              }
              rust_files_outside_facade(&path, facade, out);
          } else if path.extension().is_some_and(|e| e == "rs") {
              out.push(path);
          }
      }
  }

  /// Expand grouped `cosmic_text::{A, B, C}` imports into the flat
  /// `cosmic_text::A, cosmic_text::B, cosmic_text::C` form, so a single
  /// substring rule (`cosmic_text::<Ident>` as a whole word) catches BOTH the
  /// single-import (`cosmic_text::Editor`) and grouped-import
  /// (`cosmic_text::{Buffer, Editor}`) shapes. Conservative: only rewrites the
  /// first `cosmic_text::{ … }` on a line (the codebase never nests two on one
  /// line); anything else passes through unchanged.
  fn normalize_grouped_imports(line: &str) -> String {
      let Some(brace_start) = line.find("cosmic_text::{") else {
          return line.to_string();
      };
      let inner_start = brace_start + "cosmic_text::{".len();
      let Some(rel_end) = line[inner_start..].find('}') else {
          return line.to_string(); // unterminated (multi-line group) — leave it
      };
      let inner = &line[inner_start..inner_start + rel_end];
      // Each comma-separated entry, re-qualified. Strips `as Alias` and
      // whitespace; an entry like `Edit` becomes `cosmic_text::Edit`.
      let expanded: Vec<String> = inner
          .split(',')
          .map(|e| e.split_whitespace().next().unwrap_or("").trim())
          .filter(|e| !e.is_empty())
          .map(|e| format!("cosmic_text::{e}"))
          .collect();
      // Rebuild: prefix + expanded list + suffix-after-`}`.
      let suffix = &line[inner_start + rel_end + 1..];
      format!("{}{} {}", &line[..brace_start], expanded.join(", "), suffix)
  }

  /// True if `line` names `cosmic_text::<ident>` where `<ident>` is `needle`
  /// as a WHOLE word — the next char after `needle` must not be an
  /// identifier char (so `cosmic_text::Editor` does not match needle `Edit`,
  /// and `cosmic_text::Edit` does match it; `cosmic_text::Editor` matches
  /// needle `Editor`).
  fn names_cosmic_type(line: &str, needle: &str) -> bool {
      let pat = format!("cosmic_text::{needle}");
      let mut from = 0;
      while let Some(rel) = line[from..].find(&pat) {
          let after = from + rel + pat.len();
          let boundary = line[after..]
              .chars()
              .next()
              .is_none_or(|c| !c.is_alphanumeric() && c != '_');
          if boundary {
              return true;
          }
          from = after;
      }
      false
  }

  #[test]
  fn no_cosmic_editor_types_outside_the_facade() {
      let src = src_dir();
      let facade = src.join("text").join("edit");
      assert!(facade.is_dir(), "the text::edit facade must exist");
      let mut files = Vec::new();
      rust_files_outside_facade(&src, &facade, &mut files);
      assert!(!files.is_empty(), "scanned at least one source file");

      let mut leaks = Vec::new();
      for file in &files {
          let body = std::fs::read_to_string(file).unwrap();
          for (lineno, line) in body.lines().enumerate() {
              // Ignore comments and doc lines: the boundary is about CODE
              // naming the type, and the codebase documents `TextEditState`
              // wrapping `Editor` in prose all over (e.g. components.rs).
              let trimmed = line.trim_start();
              if trimmed.starts_with("//") || trimmed.starts_with("*") {
                  continue;
              }
              let normalized = normalize_grouped_imports(line);
              for needle in FORBIDDEN {
                  if names_cosmic_type(&normalized, needle) {
                      leaks.push(format!("{}:{}: {}", file.display(), lineno + 1, line.trim()));
                  }
              }
          }
      }
      assert!(
          leaks.is_empty(),
          "cosmic editor types leaked outside text::edit (facade boundary, \
           editing-and-ime § 2.1):\n{}",
          leaks.join("\n"),
      );
  }
  ```

- [ ] **Step 5.2 — run it, watch it pass (it should be green immediately).**

  ```sh
  cargo test -p buiy_core --test text_facade_boundary
  ```

  Expected: PASS on the first run — Task 1–3 already confined every `Editor`/`Edit`
  name to `text/edit/state.rs` and `text/edit/access.rs`. (This is the rare test
  that is born green because the production discipline preceded it.)

  > **Verify the tripwire bites — use a GROUPED-import leak (do this, then
  > revert).** The grouped form is the gap a naive substring matcher misses, so
  > the bite-check MUST exercise it (not the single-import form): `sync.rs:29`
  > already has a `use cosmic_text::{ … };` group, so append a real group leak.
  > ```sh
  > # a grouped-import leak — the form a substring matcher would MISS
  > printf '\nuse cosmic_text::{Buffer as _B, Editor as _Leak};\n' >> crates/buiy_core/src/text/sync.rs
  > cargo test -p buiy_core --test text_facade_boundary 2>&1 | grep -q 'sync.rs' && echo "TRIPWIRE BITES"
  > git checkout crates/buiy_core/src/text/sync.rs
  > ```
  > Expected: prints `TRIPWIRE BITES`, then the file is restored. (If it does NOT
  > print, `normalize_grouped_imports` is not expanding the group — fix it before
  > committing; a tripwire that misses the grouped form is the M2 hole reopening.)

- [ ] **Step 5.3 — commit.**

  ```sh
  git add -A && git commit -m "feat(text): E1.5 — facade-boundary grep tripwire (no Editor/Edit outside text::edit)"
  ```

---

## Task 6 — full gate + self-review

- [ ] **Step 6.1 — run the full headless gate** (CLAUDE.md § Build & Test, no
  `--ignored`):

  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    xvfb-run -a cargo test --workspace
  ```

  Expected: clean. Resolve every clippy warning (especially `dead_code` — if it
  fires, a marker or field has no consumer; that is decision 1 working as
  intended, so `#[allow]` is WRONG — instead confirm the marker/field IS used by a
  test or remove it). If the test step link-OOMs, add `-j 2` to the `cargo test`
  step.

- [ ] **Step 6.2 — confirm the GPU lane still builds** (it has no E1-specific test,
  but the rebinding of `extract_buiy_glyphs` must not break the `#[ignore]`d GPU
  tests' compilation):

  ```sh
  cargo test -p buiy_core --no-run
  ```

  Expected: clean build. (Running the GPU lane is the orchestrator's gate step on
  a GPU host; E1 adds no GPU assertion.)

- [ ] **Step 6.3 — self-review against the spec.** Confirm each, in the commit body
  or PR description:

  - **editing-and-ime § 2.1** (wrap `Editor`, facade): `TextEditState` wraps
    `Editor<'static>`; the facade boundary holds (Task 5 grep green). ✓
  - **§ 2.2** (the components): `TextEditState` + the four markers exist;
    decomposed, not aggregated. Field set is `editor` + `intrinsics` (decision 1 —
    behavior fields deferred to their phases, justified). ✓
  - **§ 2.2a** (Buffer ownership): `BufferRef::Owned(Buffer)` is the only shape;
    `Editor::new(Buffer::new_empty(metrics))` constructs it; the accessor prefers
    it when present (Task 2/4 tests). ✓
  - **§ 2.3** (crate split): all mechanism in `buiy_core`; no widget, no input. ✓
  - **measure-and-layout § 2.3** (`TextBufferAccess`): the QueryData shape matches
    the pin (`&mut TextBuffer` + `Option<&mut TextEditState>`,
    `with_buffer`/`with_buffer_mut`, editor-preferred, bypass on mutation); the
    read-only form feeds extract. ✓
  - **No placeholders, no dead code:** every symbol has a reader (markers in tests
    + named by E2-E6; the intrinsics field read by the accessor). Confirm
    `cargo clippy -D warnings` green proves it.
  - **Type consistency:** `IntrinsicWidths` is the one cache type, now reachable on
    both arms through the accessor; `Metrics` flows identically to `TextBuffer::new`.

- [ ] **Step 6.4 — final commit / PR.** Push the branch, open the E1 PR
  (one PR per phase, campaign § "Execution loop" step 5). The PR description
  records the field-set decision (decision 1), the cache-relocation (decision 3),
  and the E1 erratum below.

---

## E1 erratum (fold into the spec at campaign closure)

**Erratum E1-1 — the intrinsics cache relocation is unstated in the spec.**
`measure-and-layout.md` § 2.3 pins `TextBufferAccess` as `&mut TextBuffer` +
`Option<&mut TextEditState>` with `with_buffer`/`with_buffer_mut`, and
`components.rs` caches `IntrinsicWidths` on `TextBuffer`. Neither file states where
the cache lives once the editor owns the authoritative buffer. This plan resolves
it (decision 3): the cache moves to the authoritative side
(`TextEditState.intrinsics` when present, `TextBuffer.intrinsics` otherwise), both
reached through the accessor's `intrinsics`/`cache_intrinsics`/
`invalidate_intrinsics` methods — because a cache keyed to the wrong buffer goes
silently stale. The spec's § 2.3 accessor sketch should gain the three cache
methods alongside `with_buffer*` at closure.

**Erratum E1-2 — the spec § 2.2 field sketch overstates E1.** The five-field
`TextEditState` (`editor`/`selection`/`preedit`/`undo`/`blink`) is the *campaign
end state*, not E1. E1 lands `editor` + `intrinsics` only; the four behavior fields
arrive with their consuming phase (decision 1). The spec's § 2.2 code block is fine
as a target sketch but should annotate which field each phase adds (the campaign
plan's "Message taxonomy emits incrementally" note already establishes the
phase-by-phase pattern).
