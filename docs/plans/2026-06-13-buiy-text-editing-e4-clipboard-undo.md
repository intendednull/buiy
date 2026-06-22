# Buiy text-editing E4 — clipboard + undo/redo with composition-aware grouping

**Date:** 2026-06-13
**Status:** landed
**Phase:** E4 (of the E1–E6 text-editing campaign)
**Branch:** `text-editing-e4` (off `main`, which now includes E1 + E2 + E3)
**Campaign plan:** [2026-06-13-buiy-text-editing-campaign.md](2026-06-13-buiy-text-editing-campaign.md) § "E4 — Clipboard + undo/redo with composition-aware grouping"
**Spec:** [editing-and-ime.md](../specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md) §§ 7 (clipboard), 8 (undo/redo), 11 (`EditUndone`/`EditRedone` Messages)
**Readiness:** [2026-06-13-text-editing-design-readiness.md](../reports/2026-06-13-text-editing-design-readiness.md)

---

## Goal

Give the editor a **clipboard** and an **undo history**. E2 left the
`Cut`/`Copy`/`Paste`/`Undo`/`Redo` verbs *recognized but no-op* (a documented
`TODO(E4)` arm in `TextEditState::apply`, `input.rs:165-172`); E4 hooks the real
behavior in at exactly that arm.

E4 delivers two mechanisms, both **on by default in core** (the bevy-cosmic-edit
"undo cannot be an optional plugin" warning, spec § 8 rationale):

1. **Clipboard facade** — `arboard` 3.6.x behind a `ClipboardProvider`
   Resource trait-object so tests inject an in-memory fake and the dep stays
   swappable. Plain text only (HTML/image deferred — pre-campaign decision 4).
   `Cut`/`Copy` = `copy_selection()` → `provider.set_text`; `Paste` =
   `provider.get_text` → insert (newline-stripped on `SingleLine`, spec § 3.3).
2. **Undo engine** — a Buiy-owned two-stack model over the verified cosmic
   `Change` substrate (`start_change`/`finish_change`/`Change::reverse`/
   `apply_change`). Every mutating edit is wrapped in a `start_change`/
   `finish_change` pair; the resulting non-empty `Change` becomes an `UndoUnit`
   carrying caret + selection before/after; consecutive units coalesce by
   `GroupKind` (typing run, delete run) under a time-window + caret-adjacency
   rule; any motion/click/discrete command seals the open group. Undo restores
   the `_before` pair, redo the `_after`; redo clears on any new edit; the stack
   is depth-bounded (config, default 1000).

Everything that names a cosmic `Editor`/`Edit`/`Action`/`Change` type stays
**inside `crates/buiy_core/src/text/edit/`** — the facade-boundary tripwire
`tests/text_facade_boundary.rs` fails the build otherwise. The undo engine names
`cosmic_text::Change`, so `UndoStack`/`UndoUnit`/`GroupKind` MUST live in
`text::edit`. The `EditUndone`/`EditRedone` Messages name no cosmic type, so they
are public facade API (re-exported through `text::edit`).

## Architecture

```
                          apply_keyboard_edits  (BuiySet::Input, focus-gated — E2's system, EXTENDED in E4)
                                   │
   collects EditCommands this frame, then ONE SharedFontSystem lock hold:
                                   │
                                   ▼
   for each command:  TextEditState::apply_tracked(&mut FontSystem, command, ctx) → EditOutcome
                                   │                    ▲
                                   │                    └─ ctx carries: single_line, read_only,
                                   │                       now: Duration (Time<Virtual>),
                                   │                       clipboard: &mut dyn ClipboardProvider
                                   ▼
   ┌──────────────────────────── apply_tracked dispatch ─────────────────────────────┐
   │ Insert/Backspace/Delete/Enter (mutations):                                       │
   │     editor.start_change(); editor.action(fs, …); change = editor.finish_change() │
   │     if change non-empty → undo.record(UndoUnit{change, caret/sel before/after,   │
   │                                                  group: classify(command, now)}) │
   │ Motion/SelectAll/Escape (non-mutating): undo.seal(); editor.action(fs, …)        │
   │ Cut:  copy_selection()→clipboard.set_text; then DELETE-selection as a tracked    │
   │       change (one DeleteRun-sealed Discrete unit)                                │
   │ Copy: copy_selection()→clipboard.set_text   (no buffer change, no undo unit)     │
   │ Paste: clipboard.get_text → (single_line ? strip newlines) → tracked Insert      │
   │ Undo: unit = undo.pop_undo(); editor.apply_change(reverse(unit.change));          │
   │       restore caret_before + selection_before; → EditUndone(entity, unit.group)  │
   │ Redo: unit = undo.pop_redo(); editor.apply_change(unit.change);                   │
   │       restore caret_after + selection_after;  → EditRedone(entity, unit.group)   │
   └──────────────────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
   any value-changing command (incl. Undo/Redo/Cut/Paste) ⇒ E2's M1 dirty-mark seam:
   state.invalidate_intrinsics(); tree.mark_dirty_for_entity(entity); TextChanged(entity)
                                   │
                                   ▼
   next frame: TextSync (triggers don't fire — Text unchanged) but node is Taffy-dirty,
               so measure → TextCommit → extract republish the edited buffer (N→N+1)
```

**Why `apply_change` needs no `FontSystem` (verified, cosmic-text 0.19).**
`Edit::apply_change(&mut self, change: &Change) -> bool`
(`edit/mod.rs:322`, impl `edit/editor.rs:483-505`) mutates the buffer lines via
`insert_at`/`delete_range` and sets redraw — it takes **no** `FontSystem`, exactly
like the editor's own `Change` recording. Reshape is deferred to next frame's
measure → `TextCommit`, the SAME one-frame path E2 already established for an
`Action::Insert`. The undo/redo replay is therefore identical in shape to the
cosmic `vi` reference (`edit/vi.rs:13-28`): `apply_change(change)` for redo,
`apply_change(&{ let mut r = change.clone(); r.reverse(); r })` for undo. We still
hold the `SharedFontSystem` lock across the apply burst because the SAME burst
also runs `editor.action(fs, …)` for non-undo commands — one lock hold for the
whole frame's commands (E2's collect-then-lock discipline, `input.rs:344-357`).

**Why empty changes must be skipped (verified).** `finish_change()` is
`self.change.take()` (`editor.rs:512-513`) — it returns `Some(Change { items:
vec![] })` whenever `start_change` was called but the action recorded nothing
(e.g. `Backspace` at offset 0, `Delete` at end). The cosmic `vi` reference guards
exactly this: `if !change.items.is_empty() { commands.push(…) }`
(`edit/vi.rs:38-42`). `UndoStack::record` MUST drop empty changes — pushing one
would make Undo a no-op the user has to press twice.

**Mirror-direction invariant (inherited from E3).** `TextEditState.selection` is
a PROJECTION the E3 `write_caret_and_selection` render-prep writer recomputes from
the editor each frame; it is never a second source of truth. So `apply_tracked`
captures `selection_before`/`selection_after` by calling `state.mirror_selection()`
(the live read-out, `state.rs:142-151`) at the two boundaries — it does NOT read
the stale `state.selection` field. After an Undo/Redo restores the editor's
selection, E3's writer mirrors it back out next render-prep pass; E4 does not
touch the `selection` field.

**Composition grouping is shaped, not built.** `GroupKind::Composition` exists in
E4 and `UndoStack::record` accepts it as a "one unit, never coalesces with a
neighbor" group, but NO E4 code emits it — IME is E5 (spec § 6.2c). E5 will wrap
a commit (delete preedit span + insert committed text) in a single
`start_change`/`finish_change` pair and call `undo.record(…, GroupKind::Composition)`.
E4's job is to not paint E5 into a corner: `record` is the seam, and `Composition`
is a first-class non-coalescing variant from day one.

## Tech stack

- **Rust / Bevy 0.18.1**, `buiy_core` crate. Editing facade
  `crates/buiy_core/src/text/edit/`.
- **cosmic-text 0.19** (already pinned): `Edit::{start_change, finish_change,
  apply_change, copy_selection}`, `Change { items: Vec<ChangeItem> }`,
  `Change::reverse`. All verified in vendored source
  (`~/.cargo/.../cosmic-text-0.19.0/src/edit/{mod,editor,vi}.rs`).
- **arboard 3.6** (NEW direct dep): `Clipboard::new()`, `get_text()`,
  `set_text()`. `cargo deny check` at adoption (Task 1).
- **proptest 1** (already a `[workspace.dependencies]` entry, NEW to
  `buiy_core` dev-deps): the undo property test (Task 6).
- **`Time<Virtual>`** for deterministic time-window coalescing — `advance_by`,
  the E3 blink-test pattern (`text_caret_selection.rs:178`).

## Placement decisions (resolved)

- **`ClipboardProvider`** is a trait in a NEW file
  `crates/buiy_core/src/text/edit/clipboard.rs`. The real impl `ArboardClipboard`
  (wrapping `arboard::Clipboard`) lives there too. The **fake** is `MemClipboard`
  (an in-memory `RefCell<Option<String>>`), **public** (not `#[cfg(test)]`):
  the clipboard tests live in the `tests/text_clipboard_undo.rs` integration
  crate, which cannot see `#[cfg(test)]` items. The active provider is a Resource
  wrapping the boxed trait object: `#[derive(Resource)] pub struct
  Clipboard(pub Box<dyn ClipboardProvider>)` — Bevy resources cannot be bare
  `dyn`, so the newtype is the idiom. `BuiyTextPlugin` inserts
  `Clipboard(Box::new(ArboardClipboard::new()))` on a real build; tests insert
  `Clipboard(Box::new(MemClipboard::default()))`. (Why public fake: integration
  crates need it, and a publicly-swappable provider is the whole point of the
  facade.)
- **`UndoStack` / `UndoUnit` / `GroupKind`** live in a NEW file
  `crates/buiy_core/src/text/edit/undo.rs` (the file NAMES `cosmic_text::Change`,
  so it MUST be inside the facade). The `UndoStack` is a FIELD on `TextEditState`
  (`undo: UndoStack`) — exactly where the spec § 2.2 sketch puts it, and where E3
  put `selection`/`blink`. Per-entity undo history is correct (each editor has its
  own); a global resource would conflate two focused-in-sequence editors.
- **The `EditUndone`/`EditRedone` Messages** live in `undo.rs` (named there with
  the engine) and re-export through `text::edit` and `text::mod`. They carry
  `(Entity, GroupKind)` — the spec § 11 payload.
- **Where the edit is wrapped:** `TextEditState::apply` (E2's lowering, `input.rs`)
  is renamed/extended to `apply_tracked`, which takes an `EditContext` carrying
  `now` + `&mut dyn ClipboardProvider` in addition to the E2 `single_line`/
  `read_only` flags. This keeps ALL `Change`-wrapping in the one facade method that
  already owns the `EditCommand → Action` lowering — no second mutation site, and
  E5's IME commit reuses the same `record` seam.

---

## Task 1 — Adopt `arboard`; the `ClipboardProvider` facade + the fake

Add the dependency, run the supply-chain gate, and build the provider trait with
its real arboard impl and the public in-memory fake. No editor wiring yet.

- [ ] **Add `arboard` to the workspace and `buiy_core`.** Edit
  `Cargo.toml` (workspace root) `[workspace.dependencies]` — add after the
  `proptest = "1"` line:

  ```toml
  # Clipboard for the text editor (editing-and-ime § 7). arboard is the
  # ecosystem standard (egui/bevy_egui lineage), 1Password-stewarded,
  # MIT/Apache. v1 ships PLAIN TEXT only — HTML/image flavors are the named
  # follow-up slice (spec § 13, pre-campaign decision 4) — so default features
  # are OFF: `image-data` (the default) drags in the `image` crate +
  # `objc2-core-graphics` on macOS for image clipboard, which the text-only
  # adoption never uses (m2). The text backends — x11rb (Linux/X11),
  # clipboard-win (Windows), objc2-app-kit (macOS) — are NOT behind
  # `image-data`, so text cut/copy/paste works with default features disabled.
  # Linux defaults to X11 (`x11rb`); native Wayland needs the non-default
  # `wayland-data-control` feature, deferred — v1 reaches Wayland via XWayland,
  # and `ArboardClipboard::new()` degrades gracefully if no backend is present
  # (m3). Behind the ClipboardProvider facade so tests inject a fake and the
  # dep stays swappable.
  arboard = { version = "3.6", default-features = false }
  ```

  Edit `crates/buiy_core/Cargo.toml` `[dependencies]` — add after the
  `smallvec = { workspace = true }` line (the workspace entry already pins
  `default-features = false`, so the crate line inherits text-only):

  ```toml
  arboard = { workspace = true }
  ```

- [ ] **Supply-chain gate (CLAUDE.md rule; spec § 7 "cargo deny check at
  adoption").** arboard is not yet in `Cargo.lock`. Run:

  ```sh
  cargo deny check
  ```

  Expected: `advisories ok`, `bans ok`, `licenses ok`, `sources ok` — the review
  confirmed arboard 3.6.1's full tree passes the existing `deny.toml` with NO new
  SPDX id required. With `default-features = false` the text-only tree is `x11rb`
  (Linux/X11), `clipboard-win` (Windows), and the `objc2-*` app-kit crates
  (macOS) — no `image`, no `wl-clipboard-rs`, no `objc2-core-graphics`. If
  `licenses` nonetheless fails on a license not in `deny.toml`'s `allow` list,
  STOP and surface it — adding an SPDX id to `allow` is a deliberate call (the
  deny.toml header: "Adding a new dep with an unlisted license fails CI; that is
  the intended forcing function").

- [ ] **RED — write the failing facade test.** Create
  `crates/buiy_core/tests/text_clipboard_undo.rs`:

  ```rust
  //! E4 — clipboard facade + undo/redo engine (editing-and-ime §§ 7, 8, 11).
  //! Clipboard is driven through the FAKE `MemClipboard` provider (no OS
  //! clipboard touched) — platform-independent, avoiding the macOS/Windows CI
  //! issues E2/E3 hit. The undo engine is tested both as a unit (the property
  //! test, grouping fixtures) and through the real `apply_keyboard_edits`
  //! system (Task 7). Headless throughout — no adapter.

  use buiy_core::text::edit::{ClipboardProvider, MemClipboard};

  #[test]
  fn mem_clipboard_round_trips_text() {
      let mut c = MemClipboard::default();
      assert_eq!(c.get_text(), None, "empty clipboard reads None");
      c.set_text("hello".to_string());
      assert_eq!(c.get_text(), Some("hello".to_string()));
      c.set_text("world".to_string());
      assert_eq!(c.get_text(), Some("world".to_string()), "set overwrites");
  }

  #[test]
  fn mem_clipboard_is_usable_as_a_trait_object() {
      let mut boxed: Box<dyn ClipboardProvider> = Box::new(MemClipboard::default());
      boxed.set_text("via dyn".to_string());
      assert_eq!(boxed.get_text(), Some("via dyn".to_string()));
  }
  ```

- [ ] **Run it — expect a COMPILE failure** (the symbols do not exist yet):

  ```sh
  cargo test -p buiy_core --test text_clipboard_undo
  ```

  Expected: `error[E0432]: unresolved import buiy_core::text::edit::ClipboardProvider`.

- [ ] **GREEN — implement the facade.** Create
  `crates/buiy_core/src/text/edit/clipboard.rs`:

  ```rust
  //! The clipboard facade (editing-and-ime § 7). `arboard` behind a
  //! `ClipboardProvider` Resource trait-object so tests inject a fake and the
  //! dependency stays swappable. v1 is PLAIN TEXT only (HTML/image deferred —
  //! pre-campaign decision 4). This file names NO cosmic type, so the
  //! facade-boundary tripwire does not constrain it — it lives in `text::edit`
  //! for cohesion (it is editing mechanism), not because it must.

  use bevy::prelude::Resource;

  /// The swappable clipboard backend. Plain text only in v1. Both methods take
  /// `&mut self`: a real clipboard owns OS handles that mutate on read on some
  /// platforms, and the fake owns interior state — `&mut` keeps the trait
  /// honest for both. Errors are swallowed to `None` / no-op (a clipboard that
  /// is unavailable must never crash an editor; spec § 7 "must not be optional"
  /// is about presence, not infallibility).
  pub trait ClipboardProvider: Send + Sync + 'static {
      /// The current clipboard text, or `None` if empty / unavailable.
      fn get_text(&mut self) -> Option<String>;
      /// Replace the clipboard text. A failure is a silent no-op.
      fn set_text(&mut self, text: String);
  }

  /// The active provider, a Resource newtype over the boxed trait object
  /// (Bevy resources cannot be bare `dyn`). `BuiyTextPlugin` inserts the
  /// arboard-backed one on a real build; tests insert a `MemClipboard`.
  #[derive(Resource)]
  pub struct Clipboard(pub Box<dyn ClipboardProvider>);

  /// The real backend: a lazily-constructed `arboard::Clipboard`. Construction
  /// can fail (no display server, Wayland without a clipboard manager); we hold
  /// an `Option` and retry on each call, so a headless or transiently-broken
  /// clipboard degrades to "empty" rather than panicking at startup.
  #[derive(Default)]
  pub struct ArboardClipboard {
      inner: Option<arboard::Clipboard>,
  }

  impl ArboardClipboard {
      pub fn new() -> Self {
          Self::default()
      }

      /// Get (or lazily build) the arboard handle. `None` if construction fails.
      fn handle(&mut self) -> Option<&mut arboard::Clipboard> {
          if self.inner.is_none() {
              self.inner = arboard::Clipboard::new().ok();
          }
          self.inner.as_mut()
      }
  }

  impl ClipboardProvider for ArboardClipboard {
      fn get_text(&mut self) -> Option<String> {
          self.handle()?.get_text().ok()
      }

      fn set_text(&mut self, text: String) {
          if let Some(h) = self.handle() {
              let _ = h.set_text(text);
          }
      }
  }

  /// An in-memory clipboard for tests (PUBLIC so integration-crate tests can
  /// use it — `#[cfg(test)]` items are invisible across the crate boundary).
  /// Also the right default for a headless app that wants copy/paste WITHIN the
  /// app without touching the OS clipboard.
  #[derive(Default)]
  pub struct MemClipboard {
      text: Option<String>,
  }

  impl ClipboardProvider for MemClipboard {
      fn get_text(&mut self) -> Option<String> {
          self.text.clone()
      }

      fn set_text(&mut self, text: String) {
          self.text = Some(text);
      }
  }
  ```

  Wire the module + re-exports. In
  `crates/buiy_core/src/text/edit/mod.rs`, add `mod clipboard;` to the module
  block (alphabetical, after `mod caret;`) and add to the `pub use` block:

  ```rust
  pub use clipboard::{ArboardClipboard, Clipboard, ClipboardProvider, MemClipboard};
  ```

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --test text_clipboard_undo
  ```

  Expected: `test result: ok. 2 passed`.

- [ ] **Commit:** `feat(text-editing): E4.1 — arboard + ClipboardProvider facade (cargo deny green)`

---

## Task 2 — The undo engine: `GroupKind`, `UndoUnit`, `UndoStack` (record + bound)

The two-stack model with depth-bounding and empty-change rejection — as a pure
data structure, no editor wiring. Grouping/coalescing is Task 3.

- [ ] **RED — append the failing unit tests** to
  `crates/buiy_core/tests/text_clipboard_undo.rs`:

  ```rust
  use buiy_core::text::edit::{GroupKind, UndoStack, UndoUnit};
  use cosmic_text::{Change, ChangeItem, Cursor};

  /// A one-item insert `Change` at `(0, idx)` of `text` — a test helper that
  /// mirrors what `finish_change` produces for an `Action::Insert`.
  fn insert_change(idx: usize, text: &str) -> Change {
      Change {
          items: vec![ChangeItem {
              start: Cursor::new(0, idx),
              end: Cursor::new(0, idx + text.len()),
              text: text.to_string(),
              insert: true,
          }],
      }
  }

  fn unit(change: Change, group: GroupKind, before: usize, after: usize) -> UndoUnit {
      use buiy_core::text::edit::TextSelection;
      UndoUnit {
          change,
          caret_before: Cursor::new(0, before),
          caret_after: Cursor::new(0, after),
          selection_before: TextSelection::collapsed(Cursor::new(0, before)),
          selection_after: TextSelection::collapsed(Cursor::new(0, after)),
          group,
      }
  }

  #[test]
  fn record_pushes_a_nonempty_unit_and_clears_redo() {
      let mut stack = UndoStack::default();
      // Seed a redo entry to prove a new record clears it (§ 8).
      stack.push_redo_for_test(unit(insert_change(0, "x"), GroupKind::Discrete, 0, 1));
      assert_eq!(stack.redo_len(), 1);

      stack.record(unit(insert_change(0, "a"), GroupKind::Discrete, 0, 1));
      assert_eq!(stack.undo_len(), 1);
      assert_eq!(stack.redo_len(), 0, "a new edit clears the redo stack");
  }

  #[test]
  fn record_drops_an_empty_change() {
      // finish_change returns Some(Change{items: []}) when nothing was recorded
      // (Backspace at offset 0). The stack must NOT push it.
      let mut stack = UndoStack::default();
      stack.record(unit(Change::default(), GroupKind::DeleteRun, 0, 0));
      assert_eq!(stack.undo_len(), 0, "empty change is never a unit");
  }

  #[test]
  fn pop_undo_moves_the_unit_to_redo_and_back() {
      let mut stack = UndoStack::default();
      stack.record(unit(insert_change(0, "a"), GroupKind::Discrete, 0, 1));

      let popped = stack.pop_undo().expect("one unit to undo");
      assert_eq!(popped.caret_after, Cursor::new(0, 1));
      assert_eq!(stack.undo_len(), 0);
      assert_eq!(stack.redo_len(), 1, "undone unit is now redoable");

      let redone = stack.pop_redo().expect("one unit to redo");
      assert_eq!(redone.caret_before, Cursor::new(0, 0));
      assert_eq!(stack.redo_len(), 0);
      assert_eq!(stack.undo_len(), 1, "redone unit is undoable again");
  }

  #[test]
  fn depth_bound_drops_the_oldest_unit() {
      let mut stack = UndoStack::with_depth(3);
      for i in 0..5u32 {
          // Distinct groups so they never coalesce (Task 3) — here Discrete.
          stack.record(unit(insert_change(i as usize, "x"), GroupKind::Discrete, i as usize, i as usize + 1));
      }
      assert_eq!(stack.undo_len(), 3, "bounded to the 3 most recent units");
      // The oldest survivor is the 3rd recorded (caret_before index 2).
      let oldest = stack.undo.first().expect("non-empty");
      assert_eq!(oldest.caret_before, Cursor::new(0, 2));
  }
  ```

- [ ] **Run it — expect a COMPILE failure** (`UndoStack`, `UndoUnit`, `GroupKind`
  unresolved):

  ```sh
  cargo test -p buiy_core --test text_clipboard_undo
  ```

- [ ] **GREEN — implement the engine.** Create
  `crates/buiy_core/src/text/edit/undo.rs`:

  ```rust
  //! The undo/redo engine (editing-and-ime § 8). A Buiy-owned two-stack model
  //! over the verified cosmic `Change` substrate — `Change::reverse()` +
  //! `Edit::apply_change()` are the exact replay pair (the `vi` reference,
  //! `cosmic-text-0.19.0/src/edit/vi.rs:13-28`). This file NAMES
  //! `cosmic_text::Change`, so it MUST stay inside the `text::edit` facade
  //! (the boundary tripwire `tests/text_facade_boundary.rs`).
  //!
  //! Grouping (§ 8): an IME composition is ONE unit (Composition — shaped here,
  //! emitted by E5); consecutive typing coalesces by time window + caret
  //! adjacency into a TypingRun; consecutive same-direction deletes into a
  //! DeleteRun; any motion/click/discrete command seals the open group. The
  //! seam between "this is one unit" and "this extends the open run" is
  //! `record` + `seal`; the application of changes to the editor is the
  //! caller's job (input.rs), so this file names no `Editor`/`Edit`/`Action`.

  use bevy::prelude::{Entity, Message};
  use cosmic_text::{Change, Cursor};

  use super::selection::TextSelection;

  /// The default undo depth (spec § 8: "v1 default 1000 units").
  pub const DEFAULT_UNDO_DEPTH: usize = 1000;

  /// How a unit groups with its neighbors when coalescing (§ 8).
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum GroupKind {
      /// An IME composition — exactly one unit, never coalesces (§ 6.2c).
      /// Shaped in E4; emitted by E5.
      Composition,
      /// A run of inserted characters that coalesces by time + adjacency.
      TypingRun,
      /// A run of same-direction deletions that coalesces likewise.
      DeleteRun,
      /// A standalone unit (paste, cut, a single deliberate edit) — never
      /// coalesces with a neighbor.
      Discrete,
  }

  /// One undoable edit: the cosmic `Change` plus the caret + selection on
  /// either side, so undo/redo restore the full cursor state, not just the text
  /// (spec § 8). `selection_*` are captured via `mirror_selection()` at the
  /// edit boundaries (the E3 mirror-direction invariant — never the stale
  /// `state.selection` field).
  #[derive(Debug, Clone)]
  pub struct UndoUnit {
      pub change: Change,
      pub caret_before: Cursor,
      pub caret_after: Cursor,
      pub selection_before: TextSelection,
      pub selection_after: TextSelection,
      pub group: GroupKind,
  }

  /// The two-stack undo history, one per editor (a `TextEditState` field).
  /// Depth-bounded: when `undo` exceeds `depth`, the OLDEST unit is dropped
  /// (`Vec::remove(0)` — the history is short and bounded, so the O(n) shift is
  /// irrelevant; correctness over micro-optimization).
  #[derive(Debug)]
  pub struct UndoStack {
      pub undo: Vec<UndoUnit>,
      pub redo: Vec<UndoUnit>,
      depth: usize,
  }

  impl Default for UndoStack {
      fn default() -> Self {
          Self::with_depth(DEFAULT_UNDO_DEPTH)
      }
  }

  impl UndoStack {
      pub fn with_depth(depth: usize) -> Self {
          Self {
              undo: Vec::new(),
              redo: Vec::new(),
              depth: depth.max(1),
          }
      }

      pub fn undo_len(&self) -> usize {
          self.undo.len()
      }

      pub fn redo_len(&self) -> usize {
          self.redo.len()
      }

      /// `true` while a coalescing run is open (the last unit is a TypingRun or
      /// DeleteRun). Used by the grouping logic (Task 3) and by `seal`.
      pub fn has_open_group(&self) -> bool {
          matches!(
              self.undo.last().map(|u| u.group),
              Some(GroupKind::TypingRun | GroupKind::DeleteRun)
          )
      }

      /// Record a new edit. Drops an empty `Change` (a no-op edit — Backspace at
      /// offset 0; `finish_change` returns `Some(Change{items: []})`,
      /// `editor.rs:512`). A new edit ALWAYS clears the redo stack (§ 8).
      /// Grouping/coalescing is applied by `record_grouped` (Task 3); the bare
      /// `record` pushes a standalone unit (and is what the grouping path calls
      /// when it decides NOT to coalesce).
      pub fn record(&mut self, unit: UndoUnit) {
          if unit.change.items.is_empty() {
              return;
          }
          self.redo.clear();
          self.undo.push(unit);
          self.enforce_depth();
      }

      /// Pop the most recent undo unit onto the redo stack and return it (so the
      /// caller can `apply_change(reverse)` + restore `_before`). `None` if
      /// there is nothing to undo.
      pub fn pop_undo(&mut self) -> Option<UndoUnit> {
          let unit = self.undo.pop()?;
          self.redo.push(unit.clone());
          Some(unit)
      }

      /// Pop the most recent redo unit back onto the undo stack and return it
      /// (so the caller can `apply_change(change)` + restore `_after`). `None`
      /// if there is nothing to redo.
      pub fn pop_redo(&mut self) -> Option<UndoUnit> {
          let unit = self.redo.pop()?;
          self.undo.push(unit.clone());
          Some(unit)
      }

      /// Seal any open coalescing run: the next `record_grouped` starts a fresh
      /// unit even if it would otherwise coalesce. Called on any motion / click
      /// / discrete command and on focus loss (E6). Re-tags the open run as
      /// `Discrete` so `has_open_group` goes false (the text is unchanged — only
      /// the coalescing eligibility ends).
      pub fn seal(&mut self) {
          if let Some(last) = self.undo.last_mut() {
              if matches!(last.group, GroupKind::TypingRun | GroupKind::DeleteRun) {
                  last.group = GroupKind::Discrete;
              }
          }
      }

      fn enforce_depth(&mut self) {
          while self.undo.len() > self.depth {
              self.undo.remove(0);
          }
      }

      /// Test-only seam: seed a redo entry (so `record_clears_redo` can prove the
      /// clear). Not used in production — production redo entries only ever
      /// arrive via `pop_undo`.
      pub fn push_redo_for_test(&mut self, unit: UndoUnit) {
          self.redo.push(unit);
      }
  }

  /// Emitted when an edit is undone (editing-and-ime § 11 row `EditUndone`).
  /// Payload: the entity + the undone unit's `GroupKind`.
  #[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
  pub struct EditUndone(pub Entity, pub GroupKind);

  /// Emitted when an edit is redone (editing-and-ime § 11 row `EditRedone`).
  #[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
  pub struct EditRedone(pub Entity, pub GroupKind);
  ```

  Wire the module + re-exports in `crates/buiy_core/src/text/edit/mod.rs`: add
  `mod undo;` (after `mod state;`) and to the `pub use` block:

  ```rust
  pub use undo::{
      DEFAULT_UNDO_DEPTH, EditRedone, EditUndone, GroupKind, UndoStack, UndoUnit,
  };
  ```

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --test text_clipboard_undo
  ```

  Expected: the 2 Task-1 tests + the 4 Task-2 tests pass (`6 passed`).

- [ ] **Commit:** `feat(text-editing): E4.2 — undo two-stack (record/pop/bound/seal) + GroupKind + Messages`

---

## Task 3 — Grouping: time-window + caret-adjacency coalescing

`record_grouped` decides whether a new typing/delete unit EXTENDS the open run or
starts a fresh one. The time window is deterministic via the caller-supplied
`now: Duration`.

- [ ] **RED — append the failing grouping tests** to
  `crates/buiy_core/tests/text_clipboard_undo.rs`:

  ```rust
  use std::time::Duration;

  /// A delete `Change`: one delete item covering `[idx, idx+len)` of `text`.
  fn delete_change(idx: usize, text: &str) -> Change {
      Change {
          items: vec![ChangeItem {
              start: Cursor::new(0, idx),
              end: Cursor::new(0, idx + text.len()),
              text: text.to_string(),
              insert: false,
          }],
      }
  }

  fn ms(n: u64) -> Duration {
      Duration::from_millis(n)
  }

  #[test]
  fn adjacent_typing_within_the_window_coalesces_into_one_unit() {
      let mut stack = UndoStack::default();
      // Type "a" at 0→1, then "b" at 1→2, 100ms later, caret adjacent.
      stack.record_grouped(
          unit(insert_change(0, "a"), GroupKind::TypingRun, 0, 1),
          ms(0),
      );
      stack.record_grouped(
          unit(insert_change(1, "b"), GroupKind::TypingRun, 1, 2),
          ms(100),
      );
      assert_eq!(stack.undo_len(), 1, "adjacent in-window typing is ONE unit");
      let merged = &stack.undo[0];
      assert_eq!(merged.change.items.len(), 2, "both items kept for replay");
      assert_eq!(merged.caret_before, Cursor::new(0, 0), "before = first");
      assert_eq!(merged.caret_after, Cursor::new(0, 2), "after = last");
  }

  #[test]
  fn typing_past_the_time_window_starts_a_new_unit() {
      let mut stack = UndoStack::default();
      stack.record_grouped(unit(insert_change(0, "a"), GroupKind::TypingRun, 0, 1), ms(0));
      // 2 seconds later — well past the 1s window.
      stack.record_grouped(unit(insert_change(1, "b"), GroupKind::TypingRun, 1, 2), ms(2000));
      assert_eq!(stack.undo_len(), 2, "a long pause seals the run");
  }

  #[test]
  fn typing_with_a_caret_jump_starts_a_new_unit() {
      let mut stack = UndoStack::default();
      stack.record_grouped(unit(insert_change(5, "a"), GroupKind::TypingRun, 5, 6), ms(0));
      // In-window, but the caret is NOT adjacent (clicked elsewhere then typed).
      stack.record_grouped(unit(insert_change(0, "b"), GroupKind::TypingRun, 0, 1), ms(100));
      assert_eq!(stack.undo_len(), 2, "non-adjacent caret seals the run");
  }

  #[test]
  fn a_seal_breaks_the_run_even_within_the_window() {
      let mut stack = UndoStack::default();
      stack.record_grouped(unit(insert_change(0, "a"), GroupKind::TypingRun, 0, 1), ms(0));
      stack.seal(); // an arrow key / click happened between the two types
      stack.record_grouped(unit(insert_change(1, "b"), GroupKind::TypingRun, 1, 2), ms(50));
      assert_eq!(stack.undo_len(), 2, "a sealed run never re-opens");
  }

  #[test]
  fn same_direction_deletes_coalesce_typing_and_delete_do_not_mix() {
      let mut stack = UndoStack::default();
      // Two backspaces: delete "b" at 1, then "a" at 0 — same direction, adjacent.
      stack.record_grouped(unit(delete_change(1, "b"), GroupKind::DeleteRun, 2, 1), ms(0));
      stack.record_grouped(unit(delete_change(0, "a"), GroupKind::DeleteRun, 1, 0), ms(50));
      assert_eq!(stack.undo_len(), 1, "adjacent deletes coalesce");

      // A typing unit must NOT join a delete run (different GroupKind).
      stack.record_grouped(unit(insert_change(0, "x"), GroupKind::TypingRun, 0, 1), ms(60));
      assert_eq!(stack.undo_len(), 2, "typing never joins a delete run");
  }

  #[test]
  fn discrete_and_composition_never_coalesce() {
      let mut stack = UndoStack::default();
      stack.record_grouped(unit(insert_change(0, "pasted"), GroupKind::Discrete, 0, 6), ms(0));
      stack.record_grouped(unit(insert_change(6, "more"), GroupKind::Discrete, 6, 10), ms(10));
      assert_eq!(stack.undo_len(), 2, "Discrete units stand alone");
      stack.record_grouped(unit(insert_change(10, "字"), GroupKind::Composition, 10, 11), ms(20));
      stack.record_grouped(unit(insert_change(11, "体"), GroupKind::Composition, 11, 12), ms(25));
      assert_eq!(stack.undo_len(), 4, "each composition is its own unit");
  }
  ```

- [ ] **Run it — expect a COMPILE failure** (`record_grouped` / `last_active_index`
  unresolved):

  ```sh
  cargo test -p buiy_core --test text_clipboard_undo
  ```

- [ ] **GREEN — add the coalescing logic** to
  `crates/buiy_core/src/text/edit/undo.rs`. Add the window constant near
  `DEFAULT_UNDO_DEPTH`:

  ```rust
  /// The typing/delete coalescing window (spec § 8 "by time window"). Edits this
  /// close in (virtual) time, with an adjacent caret and the same group kind,
  /// fold into one undo unit. 1s matches the common editor convention (the user
  /// perceives a continuous typing burst as one undoable action).
  pub const COALESCE_WINDOW: std::time::Duration = std::time::Duration::from_millis(1000);
  ```

  Add the methods to `impl UndoStack` (after `record`), plus a field to track the
  open run's timestamp. Replace the struct + `with_depth` + the existing `record`
  to thread the timestamp:

  ```rust
  // --- add this field to `struct UndoStack` (after `depth: usize,`) ---
  //     /// When the open coalescing run was last extended (virtual time). Only
  //     /// meaningful while `has_open_group()`. The grouping window compares the
  //     /// NEW edit's `now` against this.
  //     last_edit_at: std::time::Duration,
  // --- and to `with_depth`'s initializer: `last_edit_at: Duration::ZERO,` ---
  ```

  ```rust
  impl UndoStack {
      /// Record an edit, coalescing it into the open run when eligible (§ 8).
      /// `now` is the current virtual-clock instant (deterministic in tests via
      /// `Time<Virtual>::advance_by`). Coalesces iff ALL hold:
      ///   - the open last unit has the SAME `group` (TypingRun or DeleteRun),
      ///   - `now - last_edit_at <= COALESCE_WINDOW`,
      ///   - the new edit's `caret_before` is adjacent to the run's `caret_after`
      ///     (continuing the same caret position — typed/deleted contiguously).
      /// Otherwise it starts a fresh unit (`record`). `Discrete`/`Composition`
      /// never coalesce. Empty changes are dropped by `record` (Backspace at 0).
      pub fn record_grouped(&mut self, unit: UndoUnit, now: std::time::Duration) {
          if unit.change.items.is_empty() {
              return;
          }
          let coalesces = matches!(unit.group, GroupKind::TypingRun | GroupKind::DeleteRun)
              && self
                  .undo
                  .last()
                  .is_some_and(|open| {
                      open.group == unit.group
                          && now.saturating_sub(self.last_edit_at) <= COALESCE_WINDOW
                          && at(open.caret_after) == at(unit.caret_before)
                  });

          if coalesces {
              let open = self.undo.last_mut().expect("checked by `coalesces`");
              // Extend: append the change items, carry the caret/selection AFTER
              // forward (BEFORE stays the run's original — undo restores to the
              // start of the whole burst).
              open.change.items.extend(unit.change.items);
              open.caret_after = unit.caret_after;
              open.selection_after = unit.selection_after;
              self.last_edit_at = now;
              // A coalesced edit is still a new edit ⇒ redo is stale.
              self.redo.clear();
          } else {
              self.last_edit_at = now;
              self.record(unit);
          }
      }
  }

  /// Position key for caret comparison (line, byte index) — `Cursor` is not
  /// `Ord`, so adjacency compares the positional pair.
  fn at(c: Cursor) -> (usize, usize) {
      (c.line, c.index)
  }
  ```

  > **Adjacency note.** "Adjacent" here is exact-position continuity
  > (`caret_after == caret_before`): the new edit picks up where the run's caret
  > left off. This is the precise, BiDi-safe definition — it uses the editor's own
  > post-edit cursor, never a Buiy-side index arithmetic. A backspace run's carets
  > step backward (2→1, 1→0) and each `caret_after` equals the next
  > `caret_before`, so same-direction deletes coalesce; a forward Delete run's
  > carets stay put (the text after the caret shrinks), which also satisfies
  > `caret_after == caret_before`. A click/arrow between edits changes the caret
  > and the `seal()` the caller issues breaks the run regardless.

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --test text_clipboard_undo
  ```

  Expected: all Task 1–3 tests pass (`12 passed`).

- [ ] **Commit:** `feat(text-editing): E4.3 — typing/delete coalescing by time window + caret adjacency`

---

## Task 4 — `EditContext` + `apply_tracked`: wrap mutations as undo units

Add `apply_tracked` (the richer seam that wraps every mutating command in
`start_change`/`finish_change`, records the unit grouped, and seals on
non-mutating commands), and keep a thin **`apply` compatibility shim** so the E2
and E3 test files that call the 4-arg `apply` keep compiling verbatim. Clipboard
+ Undo/Redo verbs are Task 5; this task handles Insert/Backspace/Delete/Enter/
Motion/SelectAll/Escape/Submit and the `undo` field on `TextEditState`.

> **Why a shim, not a rename (M1).** FIVE files call `state.apply(&mut fs, cmd,
> single_line, read_only)`: `tests/text_editing_ops.rs`, `tests/text_input_latency.rs`,
> `tests/text_caret_geometry.rs:45,51,67`, `tests/text_mouse_selection.rs:31`, and
> `tests/text_caret_selection_e3_gpu.rs:170` (the GPU file is `#[ignore]` but is
> STILL COMPILED by `cargo test --workspace`). Renaming `apply` away changes the
> public signature and reddens the Task 8 gate — and because the per-file
> `cargo test --test text_undo_*` loops in Tasks 4–7 never compile those three E3
> files, the break would stay invisible until the final gate (the CI-only-failure
> class E2/E3 already shipped). Keeping `apply` as a `Duration::ZERO` /
> ephemeral-clipboard delegate to `apply_tracked` preserves the E2/E3 API
> verbatim: their motion/insert/select-all calls now record-or-seal undo units
> harmlessly — none of those tests inspect the undo stack. Only the SYSTEM
> `apply_keyboard_edits` switches to `apply_tracked` with the real
> `EditContext`/clipboard (its body already changes in this task).

- [ ] **Add the `undo` field to `TextEditState`.** In
  `crates/buiy_core/src/text/edit/state.rs`, add the import and field. After the
  `use super::selection::TextSelection;` line:

  ```rust
  use super::undo::UndoStack;
  ```

  In `struct TextEditState`, after the `blink: CaretBlink,` field:

  ```rust
      /// The per-entity undo/redo history (§ 8). E4 lands it; before E4 the
      /// editor had no history (E2's edits were unrecorded). Read/written only
      /// by `apply_tracked` (input.rs) — the one mutation site.
      pub(crate) undo: UndoStack,
  ```

  In `TextEditState::new`, after `blink: CaretBlink::default(),`:

  ```rust
              undo: UndoStack::default(),
  ```

  Add a read accessor (used by Task 7's system test) to `impl TextEditState`,
  after `blink_origin`:

  ```rust
      /// The undo-stack depth (units available to undo). Test/inspection; stays
      /// inside the facade.
      pub fn undo_depth(&self) -> usize {
          self.undo.undo_len()
      }

      /// The redo-stack depth. Test/inspection.
      pub fn redo_depth(&self) -> usize {
          self.undo.redo_len()
      }
  ```

- [ ] **RED — write the failing `apply_tracked` test.** Create
  `crates/buiy_core/tests/text_undo_ops.rs`:

  ```rust
  //! E4 — `apply_tracked` wraps each editing op as an undo unit, and Undo/Redo
  //! replay through `apply_change` (editing-and-ime § 8). Driven directly
  //! against a headless `FontSystem` (cosmic shaping is CPU — no adapter), the
  //! E2 `text_editing_ops.rs` pattern. Clipboard ops use the FAKE provider.

  use buiy_core::text::SharedFontSystem;
  use buiy_core::text::edit::{EditCommand, EditContext, MemClipboard, TextEditState};
  use cosmic_text::{Metrics, Motion};
  use std::time::Duration;

  /// A context for a multi-line, mutable editor at virtual time `now`, with a
  /// fresh fake clipboard the caller can inspect.
  fn ctx(now_ms: u64, clipboard: &mut MemClipboard) -> EditContext<'_> {
      EditContext {
          single_line: false,
          read_only: false,
          now: Duration::from_millis(now_ms),
          clipboard,
      }
  }

  #[test]
  fn typing_then_undo_restores_the_previous_value_and_caret() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      let mut clip = MemClipboard::default();

      state.apply_tracked(&mut fs, EditCommand::Insert("hi".into()), &mut ctx(0, &mut clip));
      assert_eq!(state.value(), "hi");
      assert_eq!(state.undo_depth(), 1, "one typing run recorded");

      let out = state.apply_tracked(&mut fs, EditCommand::Undo, &mut ctx(10, &mut clip));
      assert_eq!(state.value(), "", "undo removes the typed run");
      assert!(out.value_changed, "undo is a value change (republish)");
      assert_eq!(state.undo_depth(), 0);
      assert_eq!(state.redo_depth(), 1, "the undone run is redoable");
  }

  #[test]
  fn redo_reapplies_the_change_and_restores_the_after_caret() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      let mut clip = MemClipboard::default();

      state.apply_tracked(&mut fs, EditCommand::Insert("ab".into()), &mut ctx(0, &mut clip));
      state.apply_tracked(&mut fs, EditCommand::Undo, &mut ctx(10, &mut clip));
      assert_eq!(state.value(), "");

      let out = state.apply_tracked(&mut fs, EditCommand::Redo, &mut ctx(20, &mut clip));
      assert_eq!(state.value(), "ab", "redo reapplies the change");
      assert!(out.value_changed);
      assert_eq!(state.redo_depth(), 0);
      assert_eq!(state.undo_depth(), 1);
  }

  #[test]
  fn a_motion_between_two_types_seals_the_run_so_undo_is_two_steps() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      let mut clip = MemClipboard::default();

      state.apply_tracked(&mut fs, EditCommand::Insert("a".into()), &mut ctx(0, &mut clip));
      // An arrow key seals the open typing run.
      state.apply_tracked(
          &mut fs,
          EditCommand::Motion(Motion::Left, false),
          &mut ctx(5, &mut clip),
      );
      state.apply_tracked(&mut fs, EditCommand::Insert("b".into()), &mut ctx(10, &mut clip));
      assert_eq!(state.value(), "ba");
      assert_eq!(state.undo_depth(), 2, "the motion split the run");

      state.apply_tracked(&mut fs, EditCommand::Undo, &mut ctx(20, &mut clip));
      assert_eq!(state.value(), "a", "first undo removes the second insert");
  }

  #[test]
  fn backspace_at_offset_zero_records_no_unit() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      let mut clip = MemClipboard::default();
      // Empty buffer: Backspace changes nothing ⇒ empty Change ⇒ no unit.
      let out = state.apply_tracked(&mut fs, EditCommand::Backspace, &mut ctx(0, &mut clip));
      assert!(!out.value_changed);
      assert_eq!(state.undo_depth(), 0, "a no-op edit is never an undo unit");
  }
  ```

- [ ] **Run it — expect a COMPILE failure** (`EditContext`, `apply_tracked`
  unresolved):

  ```sh
  cargo test -p buiy_core --test text_undo_ops
  ```

- [ ] **GREEN — add `apply_tracked` + the `apply` shim in `input.rs`.** In
  `crates/buiy_core/src/text/edit/input.rs`, update the imports at the top:

  ```rust
  use std::time::Duration;

  use bevy::prelude::*;
  use cosmic_text::{Action, Cursor, Edit, FontSystem, Selection};

  use super::clipboard::{ClipboardProvider, MemClipboard};
  use super::command::EditCommand;
  use super::state::TextEditState;
  use super::undo::{GroupKind, UndoUnit};
  ```

  Replace the entire `impl TextEditState { pub fn apply(...) -> EditOutcome {
  ... } }` block (the E2 method, `input.rs:29-175`) with the `EditContext`, the
  `apply` compatibility shim, and `apply_tracked` below. (Clipboard verbs are
  filled in Task 5 — here they keep the E2 no-op so this task compiles and the
  undo tests pass; Task 5 replaces the `Cut|Copy|Paste` arm.)

  ```rust
  /// Everything `apply_tracked` needs beyond the command itself. Carries the
  /// policy flags (E2's `single_line`/`read_only`), the virtual-clock instant
  /// for undo coalescing (deterministic in tests), and the clipboard provider
  /// for cut/copy/paste. A struct (not 4 positional args) keeps the call sites
  /// readable and leaves room for E5 to add IME context without churning every
  /// caller.
  pub struct EditContext<'a> {
      pub single_line: bool,
      pub read_only: bool,
      pub now: Duration,
      pub clipboard: &'a mut dyn ClipboardProvider,
  }

  impl TextEditState {
      /// E2/E3-compatible apply (the 4-arg signature those test files and any
      /// non-system caller use). Delegates to `apply_tracked` with a zero clock
      /// and an EPHEMERAL in-memory clipboard — so motion/insert/select-all
      /// record-or-seal undo units harmlessly, and a stray Copy/Paste through
      /// this path touches a throwaway clipboard, never the OS. The SYSTEM
      /// (`apply_keyboard_edits`) does NOT use this shim — it builds a real
      /// `EditContext` with the shared `Time<Virtual>` clock + the `Clipboard`
      /// resource. Kept (not renamed) so E4 changes ZERO E2/E3 call sites (M1).
      pub fn apply(
          &mut self,
          font_system: &mut FontSystem,
          command: EditCommand,
          single_line: bool,
          read_only: bool,
      ) -> EditOutcome {
          let mut fallback = MemClipboard::default();
          let mut ctx = EditContext {
              single_line,
              read_only,
              now: Duration::ZERO,
              clipboard: &mut fallback,
          };
          self.apply_tracked(font_system, command, &mut ctx)
      }
  }

  impl TextEditState {
      /// Lower one `EditCommand` to cosmic `Action`s, apply it, and maintain the
      /// undo history (editing-and-ime §§ 3, 7, 8). This is the ONE place
      /// `EditCommand` meets `Action` AND the one place edits are recorded as
      /// undo units — so E5's IME commit reuses the same `record` seam.
      ///
      /// Mutating commands are wrapped in a `start_change`/`finish_change` pair;
      /// the resulting non-empty `Change` is recorded (grouped by `GroupKind`).
      /// Non-mutating commands (Motion/SelectAll/Escape) SEAL the open run.
      pub fn apply_tracked(
          &mut self,
          font_system: &mut FontSystem,
          command: EditCommand,
          ctx: &mut EditContext<'_>,
      ) -> EditOutcome {
          match command {
              // ── Mutations (gated on !read_only), recorded as undo units ──
              EditCommand::Insert(text) => {
                  if ctx.read_only {
                      return EditOutcome::default();
                  }
                  // Hoist the policy flag so the closure captures a `bool`, not
                  // the `&mut EditContext` (n3 — same capture style as Paste).
                  let single_line = ctx.single_line;
                  self.tracked_edit(font_system, GroupKind::TypingRun, ctx.now, |ed, fs| {
                      for ch in text.chars() {
                          if single_line && (ch == '\n' || ch == '\r') {
                              continue; // § 3.3 single-line newline strip
                          }
                          ed.action(fs, Action::Insert(ch));
                      }
                  })
              }
              EditCommand::Backspace => {
                  if ctx.read_only {
                      return EditOutcome::default();
                  }
                  self.tracked_edit(font_system, GroupKind::DeleteRun, ctx.now, |ed, fs| {
                      ed.action(fs, Action::Backspace);
                  })
              }
              EditCommand::Delete => {
                  if ctx.read_only {
                      return EditOutcome::default();
                  }
                  self.tracked_edit(font_system, GroupKind::DeleteRun, ctx.now, |ed, fs| {
                      ed.action(fs, Action::Delete);
                  })
              }
              EditCommand::Enter => {
                  if ctx.single_line {
                      return EditOutcome {
                          value_changed: false,
                          submitted: true,
                      };
                  }
                  if ctx.read_only {
                      return EditOutcome::default();
                  }
                  // A newline is a discrete edit (web parity: undo removes the
                  // whole line break as one step, not coalesced with prior typing).
                  self.undo.seal();
                  self.tracked_edit(font_system, GroupKind::Discrete, ctx.now, |ed, fs| {
                      ed.action(fs, Action::Enter);
                  })
              }

              // ── Motion / selection / escape: seal the run, never record ──
              EditCommand::Motion(motion, extend) => {
                  self.undo.seal();
                  if extend {
                      if self.editor.selection() == Selection::None {
                          let anchor = self.editor.cursor();
                          self.editor.set_selection(Selection::Normal(anchor));
                      }
                  } else {
                      self.editor.set_selection(Selection::None);
                  }
                  self.editor.action(font_system, Action::Motion(motion));
                  EditOutcome::default()
              }
              EditCommand::SelectAll => {
                  self.undo.seal();
                  self.editor.set_selection(Selection::None);
                  self.editor
                      .action(font_system, Action::Motion(cosmic_text::Motion::BufferStart));
                  let start = self.editor.cursor();
                  self.editor.set_selection(Selection::Normal(start));
                  self.editor
                      .action(font_system, Action::Motion(cosmic_text::Motion::BufferEnd));
                  EditOutcome::default()
              }
              EditCommand::Escape => {
                  self.undo.seal();
                  self.editor.action(font_system, Action::Escape);
                  EditOutcome::default()
              }
              EditCommand::Submit => EditOutcome {
                  value_changed: false,
                  submitted: true,
              },

              // ── Undo / Redo (§ 8) ────────────────────────────────────────
              EditCommand::Undo => self.apply_undo(),
              EditCommand::Redo => self.apply_redo(),

              // ── Clipboard (§ 7) — filled in Task 5; E2 no-op until then ──
              EditCommand::Cut | EditCommand::Copy | EditCommand::Paste => {
                  let _ = &ctx.clipboard; // silence unused until Task 5
                  EditOutcome::default()
              }
          }
      }

      /// Run a mutating `edit` closure wrapped in a `start_change`/`finish_change`
      /// pair and record the resulting NON-EMPTY change as an undo unit grouped
      /// by `group`. Captures caret + selection on both sides via the live
      /// `mirror_selection()` read-out (the E3 mirror-direction invariant — never
      /// the stale `selection` field). Returns whether the value changed.
      fn tracked_edit(
          &mut self,
          font_system: &mut FontSystem,
          group: GroupKind,
          now: Duration,
          edit: impl FnOnce(&mut cosmic_text::Editor<'static>, &mut FontSystem),
      ) -> EditOutcome {
          let before = self.value();
          let caret_before = self.editor.cursor();
          let selection_before = self.mirror_selection();

          self.editor.start_change();
          edit(&mut self.editor, font_system);
          let change = self.editor.finish_change().unwrap_or_default();

          let caret_after = self.editor.cursor();
          let selection_after = self.mirror_selection();

          // Empty change ⇒ no-op edit (e.g. Backspace at 0); record_grouped drops
          // it, so the undo stack stays clean and value_changed stays false.
          self.undo.record_grouped(
              UndoUnit {
                  change,
                  caret_before,
                  caret_after,
                  selection_before,
                  selection_after,
                  group,
              },
              now,
          );

          EditOutcome {
              value_changed: self.value() != before,
              submitted: false,
          }
      }

      /// Undo the most recent unit: replay the reversed change, restore the
      /// `_before` caret + selection (§ 8). The reversed change deletes the
      /// inserted text / re-inserts the deleted text. No `FontSystem` needed —
      /// `apply_change` mutates buffer lines + sets redraw; reshape is next
      /// frame's `TextCommit` (the one-frame path). `value_changed` is whether
      /// the text actually moved (always true for a non-empty unit).
      fn apply_undo(&mut self) -> EditOutcome {
          let Some(unit) = self.undo.pop_undo() else {
              return EditOutcome::default();
          };
          let mut reversed = unit.change.clone();
          reversed.reverse();
          let changed = self.editor.apply_change(&reversed);
          self.restore_cursor(unit.caret_before, unit.selection_before);
          EditOutcome {
              value_changed: changed,
              submitted: false,
          }
      }

      /// Redo the most recent undone unit: replay the change forward, restore the
      /// `_after` caret + selection (§ 8).
      fn apply_redo(&mut self) -> EditOutcome {
          let Some(unit) = self.undo.pop_redo() else {
              return EditOutcome::default();
          };
          let changed = self.editor.apply_change(&unit.change);
          self.restore_cursor(unit.caret_after, unit.selection_after);
          EditOutcome {
              value_changed: changed,
              submitted: false,
          }
      }

      /// Restore the caret + the editor's selection after an undo/redo. The
      /// editor's `Selection` is the authoritative one E3 mirrors OUT next pass;
      /// we set both the cursor and (for a non-collapsed range) the anchor.
      fn restore_cursor(&mut self, caret: Cursor, selection: super::selection::TextSelection) {
          self.editor.set_cursor(caret);
          if selection.is_collapsed() {
              self.editor.set_selection(Selection::None);
          } else {
              self.editor
                  .set_selection(Selection::Normal(selection.primary.anchor));
              self.editor.set_cursor(selection.primary.active);
          }
      }
  }
  ```

  > **`set_cursor` (verified present).** `Edit::set_cursor(&mut self, cursor:
  > Cursor)` is on the 0.19 trait (`edit/mod.rs:207`, impl `editor.rs:186`) —
  > the cursor setter paired with `cursor()`. No `FontSystem` needed; it sets the
  > field directly.

  > **Word/Line-selection undo caret fidelity (n2 — accepted for v1).** When the
  > undone unit's `selection_before` came from a double/triple-click (a
  > `Selection::Word`/`Line`), `from_bounds` (E3, `selection.rs:80-95`) anchored
  > at `lo` with `active` at `hi` because the editor's live cursor was interior to
  > the expanded span. `restore_cursor` therefore lands the caret at `hi` and the
  > anchor at `lo` — the SELECTED SPAN is restored faithfully (the painted
  > highlight is identical), but the exact pre-undo caret-within-span position is
  > not recovered (it lands at the span end, not the click point). This is a
  > cosmetic caret-position nicety on an already-rare path (undo while a
  > word/line selection was active); span fidelity — what the user sees — is
  > exact. Acceptable for v1; recovering the interior caret would need storing the
  > editor's raw `Selection` variant in the `UndoUnit`, deferred.

- [ ] **Switch the SYSTEM `apply_keyboard_edits` to `apply_tracked`** (same file,
  `input.rs`) — the ONLY caller that changes (the `apply` shim covers every test
  caller). It builds a real `EditContext` with the shared `Time<Virtual>` clock +
  the `Clipboard` resource. Update its signature + the apply loop. Add to the
  imports block already at the bottom of the file (after `use crate::FocusedEntity;`):

  ```rust
  use super::clipboard::Clipboard;
  ```

  Change the system signature to add `time` and `clipboard`:

  ```rust
  #[allow(clippy::type_complexity, clippy::too_many_arguments)]
  pub fn apply_keyboard_edits(
      events: Option<MessageReader<KeyboardInput>>,
      focused: Option<Res<FocusedEntity>>,
      keymap: Res<Keymap>,
      keys: Option<Res<ButtonInput<KeyCode>>>,
      time: Res<Time>,
      fonts: Res<SharedFontSystem>,
      mut clipboard: Option<ResMut<Clipboard>>,
      mut tree: Option<NonSendMut<LayoutTree>>,
      mut editors: Query<(&mut TextEditState, Has<SingleLine>, Has<ReadOnly>), Without<Disabled>>,
      mut changed: MessageWriter<TextChanged>,
      mut undone: MessageWriter<super::undo::EditUndone>,
      mut redone: MessageWriter<super::undo::EditRedone>,
  ) {
  ```

  Replace the apply burst (the block from `let mut font_system = fonts.lock();`
  through `drop(font_system);`, `input.rs:348-357`) with:

  ```rust
      // A clipboard is required to lower Cut/Copy/Paste; without the resource
      // (a bare BuiyTextPlugin harness that didn't insert one) fall back to an
      // ephemeral in-memory clipboard so the system still runs (the Option-param
      // inert-harness discipline). Clipboard verbs then no-op across frames,
      // which is correct for a harness with no clipboard configured.
      let mut fallback = super::clipboard::MemClipboard::default();
      let clip: &mut dyn ClipboardProvider = match clipboard.as_deref_mut() {
          Some(Clipboard(boxed)) => boxed.as_mut(),
          None => &mut fallback,
      };
      let now = time.elapsed();

      let mut font_system = fonts.lock();
      let mut any_value_change = false;
      for command in commands {
          // Capture the group BEFORE applying, for the undo/redo Messages.
          let was_undo = command == EditCommand::Undo;
          let was_redo = command == EditCommand::Redo;
          let group_before_undo = state.undo_top_group();
          let group_before_redo = state.redo_top_group();
          let mut ctx = EditContext {
              single_line,
              read_only,
              now,
              // Reborrow each iteration: `&mut` is not `Copy`, so `clipboard: clip`
              // would MOVE it on iteration 1 and fail to compile on iteration 2
              // ("use of moved value"). `&mut *clip` reborrows for this ctx only.
              clipboard: &mut *clip,
          };
          let outcome = state.apply_tracked(&mut font_system, command, &mut ctx);
          any_value_change |= outcome.value_changed;
          if was_undo && outcome.value_changed {
              if let Some(g) = group_before_undo {
                  undone.write(super::undo::EditUndone(entity, g));
              }
          }
          if was_redo && outcome.value_changed {
              if let Some(g) = group_before_redo {
                  redone.write(super::undo::EditRedone(entity, g));
              }
          }
      }
      drop(font_system);
  ```

  > **The group-before capture.** `apply_undo` pops the unit before we can read
  > its group from the system, so we snapshot the top group BEFORE applying. Add
  > the two tiny accessors to `impl TextEditState` in `input.rs` (next to
  > `apply_tracked`, inside the facade):
  >
  > ```rust
  > impl TextEditState {
  >     /// The `GroupKind` of the unit Undo would pop next (the top of the undo
  >     /// stack), for the `EditUndone` Message payload.
  >     pub(crate) fn undo_top_group(&self) -> Option<GroupKind> {
  >         self.undo.undo.last().map(|u| u.group)
  >     }
  >     /// The `GroupKind` Redo would pop next (top of the redo stack).
  >     pub(crate) fn redo_top_group(&self) -> Option<GroupKind> {
  >         self.undo.redo.last().map(|u| u.group)
  >     }
  > }
  > ```

- [ ] **Register the two new Messages** in `crates/buiy_core/src/text/mod.rs`.
  In the E2 block that registers `TextChanged` (`mod.rs:206-211`), after
  `app.add_message::<crate::text::edit::TextChanged>();`:

  ```rust
          // E4 (editing-and-ime § 8, 11): the undo/redo transition Messages.
          app.add_message::<crate::text::edit::EditUndone>();
          app.add_message::<crate::text::edit::EditRedone>();
  ```

  And insert the default `Clipboard` resource (E4 § 7 — undo + clipboard are core,
  on by default). After `app.init_resource::<crate::text::edit::Keymap>();`:

  ```rust
          // E4 (editing-and-ime § 7): the OS clipboard, behind the facade. On a
          // headless build with no display arboard construction fails and the
          // provider degrades to "empty" (ArboardClipboard::handle returns None) —
          // never a panic. Tests override this resource with a MemClipboard.
          app.insert_resource(crate::text::edit::Clipboard(Box::new(
              crate::text::edit::ArboardClipboard::new(),
          )));
  ```

  Update the `pub use edit::{...}` re-export block in `mod.rs` (the E2/E3 block,
  `mod.rs:53-58`) to add the new public symbols:

  ```rust
  pub use edit::{
      ArboardClipboard, CaretBlink, CaretMoved, ClickTracker, Clipboard, ClipboardProvider,
      Disabled, EditCommand, EditContext, EditRedone, EditUndone, GroupKind, Keymap, MemClipboard,
      Placeholder, PointerGesture, ReadOnly, SelectionChanged, SelectionRange, SingleLine,
      TextBufferAccess, TextChanged, TextEditState, TextSelection, UndoStack, UndoUnit,
      apply_keyboard_edits, pointer_selection, pointer_to_cursor, write_caret_and_selection,
  };
  ```

  > **`Time` resource note.** `Res<Time>` in the `Update` schedule is the
  > `Time<Virtual>`-backed generic clock (the E3 `write_caret_and_selection`
  > reader uses exactly `time.elapsed()`, `caret.rs:152`). `advance_by` on
  > `Time<Virtual>` advances it deterministically — the same hook the E3
  > blink tests use.

- [ ] **Run the unit test — expect PASS:**

  ```sh
  cargo test -p buiy_core --test text_undo_ops
  ```

  Expected: `4 passed`.

- [ ] **Run the E2/E3 regression suite — confirm the shim kept every caller
  compiling.** The 4-arg `apply` shim means ZERO test files change: E2's
  `text_editing_ops.rs` and the three E3 callers (`text_caret_geometry.rs`,
  `text_mouse_selection.rs`, `text_caret_selection_e3_gpu.rs`) keep calling
  `state.apply(&mut fs, cmd, single_line, read_only)` verbatim. Compile + run
  every affected file (including the `#[ignore]` GPU file, which must still
  COMPILE under `--workspace`):

  ```sh
  cargo test -p buiy_core --test text_editing_ops --test text_input_latency \
    --test text_caret_geometry --test text_mouse_selection \
    --test text_caret_selection_e3_gpu
  ```

  Expected: all pass (the GPU file's `#[ignore]` tests compile + are skipped).
  None of these inspect the undo stack, so the harmless record-or-seal that the
  shim now performs does not change any assertion. If `text_input_latency`'s
  harness constructs the registered system, it now needs the `Clipboard`
  resource — the plugin inserts it, so a full-plugin harness has it; a bare
  hand-built harness uses the system's ephemeral-clipboard fallback.

- [ ] **Commit:** `feat(text-editing): E4.4 — apply_tracked wraps edits as undo units; Undo/Redo replay`

---

## Task 5 — Clipboard verbs: Cut / Copy / Paste through the provider

Fill in the `Cut | Copy | Paste` arm of `apply_tracked` with real behavior, and
test it through the fake provider (round-trip + single-line newline strip).

- [ ] **RED — append the failing clipboard tests** to
  `crates/buiy_core/tests/text_undo_ops.rs`:

  ```rust
  use buiy_core::text::edit::ClipboardProvider;

  /// Select the whole buffer, then run the command. Helper to set up a
  /// non-empty selection for Cut/Copy.
  fn select_all(state: &mut TextEditState, fs: &mut cosmic_text::FontSystem, clip: &mut MemClipboard) {
      state.apply_tracked(fs, EditCommand::SelectAll, &mut ctx(0, clip));
  }

  #[test]
  fn copy_puts_the_selection_on_the_clipboard_without_changing_the_value() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      let mut clip = MemClipboard::default();

      state.apply_tracked(&mut fs, EditCommand::Insert("hello".into()), &mut ctx(0, &mut clip));
      select_all(&mut state, &mut fs, &mut clip);
      let out = state.apply_tracked(&mut fs, EditCommand::Copy, &mut ctx(10, &mut clip));

      assert!(!out.value_changed, "copy never changes the value");
      assert_eq!(clip.get_text(), Some("hello".to_string()));
      assert_eq!(state.value(), "hello", "buffer intact");
  }

  #[test]
  fn cut_copies_then_deletes_the_selection_as_one_undoable_unit() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      let mut clip = MemClipboard::default();

      state.apply_tracked(&mut fs, EditCommand::Insert("hello".into()), &mut ctx(0, &mut clip));
      select_all(&mut state, &mut fs, &mut clip);
      let out = state.apply_tracked(&mut fs, EditCommand::Cut, &mut ctx(10, &mut clip));

      assert!(out.value_changed, "cut removes the selection");
      assert_eq!(clip.get_text(), Some("hello".to_string()));
      assert_eq!(state.value(), "", "selection deleted");
      assert_eq!(state.undo_depth(), 2, "the insert run + the cut");

      // Undo the cut restores the text.
      state.apply_tracked(&mut fs, EditCommand::Undo, &mut ctx(20, &mut clip));
      assert_eq!(state.value(), "hello", "undo brings the cut text back");
  }

  #[test]
  fn paste_inserts_the_clipboard_text() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      let mut clip = MemClipboard::default();
      clip.set_text("pasted".to_string());

      let out = state.apply_tracked(&mut fs, EditCommand::Paste, &mut ctx(0, &mut clip));
      assert!(out.value_changed);
      assert_eq!(state.value(), "pasted");
      assert_eq!(state.undo_depth(), 1, "paste is one undoable unit");
  }

  #[test]
  fn single_line_paste_strips_newlines() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      let mut clip = MemClipboard::default();
      clip.set_text("a\nb\r\nc".to_string());

      // single_line: true in the context.
      let mut single_ctx = EditContext {
          single_line: true,
          read_only: false,
          now: Duration::from_millis(0),
          clipboard: &mut clip,
      };
      let out = state.apply_tracked(&mut fs, EditCommand::Paste, &mut single_ctx);
      assert!(out.value_changed);
      assert_eq!(state.value(), "abc", "newlines stripped on a single-line editor (§ 3.3)");
  }

  #[test]
  fn paste_with_an_empty_clipboard_is_a_no_op() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      let mut clip = MemClipboard::default();
      let out = state.apply_tracked(&mut fs, EditCommand::Paste, &mut ctx(0, &mut clip));
      assert!(!out.value_changed);
      assert_eq!(state.undo_depth(), 0, "nothing to paste, nothing recorded");
  }
  ```

- [ ] **Run it — expect FAILURE** (Copy/Cut/Paste are still the no-op arm):

  ```sh
  cargo test -p buiy_core --test text_undo_ops
  ```

  Expected: the 4 Task-4 tests pass; the 5 new clipboard tests FAIL (clipboard
  empty after Copy, value unchanged after Cut/Paste).

- [ ] **GREEN — implement the clipboard arm.** In
  `crates/buiy_core/src/text/edit/input.rs`, replace the placeholder
  `Cut | Copy | Paste` arm of `apply_tracked` with:

  ```rust
              // ── Clipboard (§ 7) — plain text only (decision 4) ───────────
              EditCommand::Copy => {
                  // copy_selection() is None when there is no selection; a bare
                  // caret Copy is a no-op (web parity).
                  if let Some(text) = self.editor.copy_selection() {
                      ctx.clipboard.set_text(text);
                  }
                  EditOutcome::default()
              }
              EditCommand::Cut => {
                  if ctx.read_only {
                      return EditOutcome::default(); // ReadOnly: copy yes, cut no
                  }
                  let Some(text) = self.editor.copy_selection() else {
                      return EditOutcome::default(); // nothing selected
                  };
                  ctx.clipboard.set_text(text);
                  // Delete the selection as one DISCRETE undoable unit (a cut is
                  // a deliberate single action — never coalesced with neighbors).
                  self.undo.seal();
                  self.tracked_edit(font_system, GroupKind::Discrete, ctx.now, |ed, fs| {
                      let _ = fs; // delete_selection needs no FontSystem
                      ed.delete_selection();
                  })
              }
              EditCommand::Paste => {
                  if ctx.read_only {
                      return EditOutcome::default();
                  }
                  let Some(text) = ctx.clipboard.get_text() else {
                      return EditOutcome::default(); // empty clipboard
                  };
                  // A paste is one DISCRETE unit, newline-stripped on single-line
                  // (§ 3.3 — the same policy as a single-line Insert).
                  self.undo.seal();
                  let single_line = ctx.single_line;
                  self.tracked_edit(font_system, GroupKind::Discrete, ctx.now, |ed, fs| {
                      for ch in text.chars() {
                          if single_line && (ch == '\n' || ch == '\r') {
                              continue;
                          }
                          ed.action(fs, Action::Insert(ch));
                      }
                  })
              }
  ```

  Remove the now-dead `let _ = &ctx.clipboard;` and the combined no-op arm.

  > **`delete_selection` change recording.** `Edit::delete_selection` calls
  > `delete_range` (`editor.rs:464-481`), which records a delete `ChangeItem` into
  > the open `change` (`editor.rs:303-306`) — so wrapping it in `tracked_edit`'s
  > `start_change`/`finish_change` captures the deletion correctly. Verified in
  > vendored source.

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --test text_undo_ops
  ```

  Expected: `9 passed`.

- [ ] **Commit:** `feat(text-editing): E4.5 — Cut/Copy/Paste through ClipboardProvider (plain text, single-line strip)`

---

## Task 6 — The undo property test (spec § 12 / § 8 normative)

For arbitrary edit scripts, undo-all restores the initial value + caret, and
`apply_change(reverse(c))` after `c` is identity on the buffer text.

- [ ] **Add `proptest` to `buiy_core` dev-deps.** In
  `crates/buiy_core/Cargo.toml` `[dev-dependencies]`, after `naga = "27"`:

  ```toml
  proptest = { workspace = true }
  ```

  (`proptest = "1"` is already a `[workspace.dependencies]` entry, `Cargo.toml:51`.)

- [ ] **RED — write the failing property test.** Create
  `crates/buiy_core/tests/text_undo_property.rs`:

  ```rust
  //! E4 — the undo property test (editing-and-ime §§ 8, 12). For an arbitrary
  //! script of edits, undoing every recorded unit restores the editor's initial
  //! value and caret. Headless (cosmic shaping is CPU — no adapter). The script
  //! is generated by proptest; each case builds a fresh editor, applies the
  //! script, then undoes to exhaustion and asserts the round-trip.

  use buiy_core::text::SharedFontSystem;
  use buiy_core::text::edit::{EditCommand, EditContext, MemClipboard, TextEditState};
  use cosmic_text::{Metrics, Motion};
  use proptest::prelude::*;
  use std::time::Duration;

  /// One scripted edit. We restrict to the mutating + motion verbs the property
  /// is about (insert/backspace/delete/enter/left/right) — clipboard/undo are
  /// not part of the "undo-all is identity" invariant (Undo inside the script
  /// would be testing the test).
  #[derive(Debug, Clone)]
  enum ScriptOp {
      Type(char),
      Backspace,
      Delete,
      Enter,
      Left,
      Right,
  }

  fn op_strategy() -> impl Strategy<Value = ScriptOp> {
      prop_oneof![
          // Bias toward typing so the buffer actually grows.
          4 => prop::char::range('a', 'z').prop_map(ScriptOp::Type),
          1 => Just(ScriptOp::Backspace),
          1 => Just(ScriptOp::Delete),
          1 => Just(ScriptOp::Enter),
          1 => Just(ScriptOp::Left),
          1 => Just(ScriptOp::Right),
      ]
  }

  fn apply_op(
      state: &mut TextEditState,
      fs: &mut cosmic_text::FontSystem,
      clip: &mut MemClipboard,
      op: &ScriptOp,
      now_ms: u64,
  ) {
      let mut ctx = EditContext {
          single_line: false,
          read_only: false,
          // Spread edits past the coalescing window so each op is its own unit
          // OR coalesces — both are valid; undo-all must restore regardless.
          now: Duration::from_millis(now_ms),
          clipboard: clip,
      };
      let cmd = match op {
          ScriptOp::Type(c) => EditCommand::Insert(c.to_string()),
          ScriptOp::Backspace => EditCommand::Backspace,
          ScriptOp::Delete => EditCommand::Delete,
          ScriptOp::Enter => EditCommand::Enter,
          ScriptOp::Left => EditCommand::Motion(Motion::Left, false),
          ScriptOp::Right => EditCommand::Motion(Motion::Right, false),
      };
      state.apply_tracked(fs, cmd, &mut ctx);
  }

  proptest! {
      #![proptest_config(ProptestConfig::with_cases(128))]

      /// Undo-all restores the initial (empty) value and caret, for any script.
      #[test]
      fn undo_all_restores_the_initial_state(script in prop::collection::vec(op_strategy(), 0..40)) {
          let fonts = SharedFontSystem::new();
          let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
          let mut fs = fonts.lock();
          let mut clip = MemClipboard::default();

          let initial_value = state.value();
          let initial_caret = state.caret();

          // Apply the script, time advancing ~50ms/op (some coalesce, some don't).
          for (i, op) in script.iter().enumerate() {
              apply_op(&mut state, &mut fs, &mut clip, op, (i as u64) * 50);
          }

          // Undo to exhaustion (each Undo pops one unit; advance time so the
          // Undo never coalesces with anything — it is non-mutating-of-stack).
          let mut guard = 0;
          while state.undo_depth() > 0 {
              let mut ctx = EditContext {
                  single_line: false,
                  read_only: false,
                  now: Duration::from_millis(100_000 + guard * 50),
                  clipboard: &mut clip,
              };
              state.apply_tracked(&mut fs, EditCommand::Undo, &mut ctx);
              guard += 1;
              prop_assert!(guard < 1000, "undo did not terminate");
          }

          prop_assert_eq!(state.value(), initial_value, "value restored");
          let c = state.caret();
          prop_assert_eq!(
              (c.line, c.index),
              (initial_caret.line, initial_caret.index),
              "caret restored"
          );
      }

      /// Redo-after-undo-all returns to the post-script value (the redo stack
      /// replays forward). Proves the `_after` restoration is sound.
      #[test]
      fn redo_after_full_undo_restores_the_final_value(
          script in prop::collection::vec(op_strategy(), 1..40)
      ) {
          let fonts = SharedFontSystem::new();
          let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
          let mut fs = fonts.lock();
          let mut clip = MemClipboard::default();

          for (i, op) in script.iter().enumerate() {
              apply_op(&mut state, &mut fs, &mut clip, op, (i as u64) * 50);
          }
          let final_value = state.value();
          let units = state.undo_depth();

          let mut t = 100_000u64;
          for _ in 0..units {
              let mut ctx = EditContext { single_line: false, read_only: false, now: Duration::from_millis(t), clipboard: &mut clip };
              state.apply_tracked(&mut fs, EditCommand::Undo, &mut ctx);
              t += 50;
          }
          for _ in 0..units {
              let mut ctx = EditContext { single_line: false, read_only: false, now: Duration::from_millis(t), clipboard: &mut clip };
              state.apply_tracked(&mut fs, EditCommand::Redo, &mut ctx);
              t += 50;
          }
          prop_assert_eq!(state.value(), final_value, "redo-all returns to the final value");
      }
  }
  ```

- [ ] **Run it — expect PASS** (proptest now available; the engine is correct):

  ```sh
  cargo test -p buiy_core --test text_undo_property
  ```

  Expected: `2 passed`. If proptest finds a shrinking counterexample, it has
  found a REAL undo bug — root-cause it (the likely culprit is `restore_cursor`
  or the empty-change handling), do not weaken the property.

- [ ] **Commit:** `test(text-editing): E4.6 — undo round-trip property test (undo-all = identity)`

---

## Task 7 — System-level wiring: the time-window coalescing test through `Time<Virtual>`

Prove the whole path through the real `apply_keyboard_edits` system: a focused
editor, synthetic `KeyboardInput`, `Time<Virtual>` advanced deterministically,
typing coalesces into one undo unit within the window and splits across it, and
the `EditUndone` Message fires.

- [ ] **RED — write the failing system test.** Create
  `crates/buiy_core/tests/text_undo_system.rs`:

  ```rust
  //! E4 — the undo engine through the real `apply_keyboard_edits` system
  //! (editing-and-ime §§ 8, 11). A focused editor, synthetic `KeyboardInput`,
  //! and a `Time<Virtual>` clock advanced deterministically (the E3 blink-test
  //! pattern, `text_caret_selection.rs:178`) — so the time-window coalescing is
  //! reproducible, never wall-clock. Headless: no adapter, the FAKE clipboard.

  use bevy::input::ButtonState;
  use bevy::input::keyboard::{Key, KeyboardInput};
  use bevy::prelude::*;
  use buiy_core::layout::Style;
  use buiy_core::text::Text;
  use buiy_core::text::edit::{Clipboard, EditUndone, MemClipboard, TextEditState};
  use buiy_core::{FocusedEntity, Node};
  use cosmic_text::Metrics;
  use std::time::Duration;

  /// Build a minimal app: BuiyTextPlugin (keymap + system + Clipboard resource)
  /// + FocusPlugin + the KeyboardInput / ButtonInput infra, with a focused
  /// editable entity. The clipboard is overridden to the fake.
  fn app_with_focused_editor() -> (App, Entity) {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(buiy_core::CorePlugin);
      app.add_plugins(buiy_core::layout::LayoutPlugin);
      app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
      app.add_plugins(buiy_core::focus::FocusPlugin);
      app.add_message::<KeyboardInput>();
      app.insert_resource(ButtonInput::<KeyCode>::default());
      // Override the OS clipboard with the in-memory fake (no display needed).
      app.insert_resource(Clipboard(Box::new(MemClipboard::default())));

      let editor = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(300.0).height_px(60.0),
              Text(String::new()),
              TextEditState::new(Metrics::new(16.0, 19.2)),
          ))
          .id();
      app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
      // Pause the virtual clock so the ONLY time progression is our explicit
      // `advance_by` — `app.update()` no longer adds a real per-frame delta, so
      // the coalescing-window timing is fully deterministic, not "wide enough"
      // (n1). `advance_by` still advances a paused clock.
      app.world_mut().resource_mut::<Time<Virtual>>().pause();
      app.update(); // settle layout
      (app, editor)
  }

  /// Send a character keypress for the next `app.update()`.
  fn type_char(app: &mut App, c: char) {
      let window = Entity::PLACEHOLDER;
      app.world_mut().write_message(KeyboardInput {
          key_code: KeyCode::KeyA, // physical code is irrelevant to char insertion
          logical_key: Key::Character(c.to_string().into()),
          state: ButtonState::Pressed,
          text: Some(c.to_string().into()),
          repeat: false,
          window,
      });
  }

  fn advance(app: &mut App, ms: u64) {
      app.world_mut()
          .resource_mut::<Time<Virtual>>()
          .advance_by(Duration::from_millis(ms));
  }

  fn undo_depth(app: &App, e: Entity) -> usize {
      app.world().get::<TextEditState>(e).unwrap().undo_depth()
  }

  #[test]
  fn typing_within_the_window_coalesces_into_one_undo_unit() {
      let (mut app, editor) = app_with_focused_editor();

      type_char(&mut app, 'a');
      app.update();
      advance(&mut app, 100); // well within the 1s window
      type_char(&mut app, 'b');
      app.update();
      advance(&mut app, 100);
      type_char(&mut app, 'c');
      app.update();

      assert_eq!(
          app.world().get::<TextEditState>(editor).unwrap().value(),
          "abc"
      );
      assert_eq!(undo_depth(&app, editor), 1, "in-window typing is ONE unit");
  }

  #[test]
  fn typing_across_the_window_splits_into_separate_units() {
      let (mut app, editor) = app_with_focused_editor();

      type_char(&mut app, 'a');
      app.update();
      advance(&mut app, 2000); // past the 1s window — seals the run
      type_char(&mut app, 'b');
      app.update();

      assert_eq!(undo_depth(&app, editor), 2, "a long pause splits the run");
  }

  #[test]
  fn undo_emits_edit_undone_with_the_group_kind() {
      let (mut app, editor) = app_with_focused_editor();
      type_char(&mut app, 'x');
      app.update();

      // Send Ctrl/Cmd-Z. On Linux/Windows that's Ctrl-Z; press the modifier.
      #[cfg(not(target_os = "macos"))]
      app.world_mut()
          .resource_mut::<ButtonInput<KeyCode>>()
          .press(KeyCode::ControlLeft);
      #[cfg(target_os = "macos")]
      app.world_mut()
          .resource_mut::<ButtonInput<KeyCode>>()
          .press(KeyCode::SuperLeft);

      let window = Entity::PLACEHOLDER;
      app.world_mut().write_message(KeyboardInput {
          key_code: KeyCode::KeyZ,
          logical_key: Key::Character("z".into()),
          state: ButtonState::Pressed,
          text: Some("z".into()),
          repeat: false,
          window,
      });
      app.update();

      assert_eq!(
          app.world().get::<TextEditState>(editor).unwrap().value(),
          "",
          "Ctrl/Cmd-Z undid the typed char"
      );
      // The EditUndone Message fired this frame with the TypingRun group.
      let messages = app.world().resource::<Messages<EditUndone>>();
      let mut reader = messages.get_cursor();
      let got: Vec<_> = reader.read(messages).copied().collect();
      assert_eq!(got.len(), 1, "exactly one EditUndone");
      assert_eq!(got[0].0, editor);
  }
  ```

  > **Message-read idiom (verified).** `Messages::<T>::get_cursor()` + `cursor
  > .read(messages)` is the manual-drain pattern already used in
  > `tests/picking_backend.rs:71-73` and `tests/text_caret_geometry.rs:233-241`
  > (0.18.1). The alternative — a system-injected `MessageReader<EditUndone>`
  > draining into a `Local`/`Resource` — is the `tests/text_editing_ops.rs:407`
  > pattern; either works. The assertion only needs "exactly one `EditUndone` for
  > `editor` this frame".

- [ ] **Run it — expect PASS** (the system is fully wired from Task 4):

  ```sh
  cargo test -p buiy_core --test text_undo_system
  ```

  Expected: `3 passed`. If `app_with_focused_editor` panics on a missing
  plugin/resource, mirror the EXACT plugin set `text_input_latency.rs` uses (it is
  the proven editor-system harness) — `MinimalPlugins` may need `bevy::time::TimePlugin`
  explicitly for `Time<Virtual>` to exist; add `app.add_plugins(bevy::time::TimePlugin)`
  if `resource_mut::<Time<Virtual>>()` panics.

- [ ] **Commit:** `test(text-editing): E4.7 — system-level coalescing via Time<Virtual> + EditUndone Message`

---

## Task 8 — Facade boundary + full gate

Confirm no cosmic type leaked outside `text::edit`, then run the whole headless
gate.

- [ ] **Facade boundary — confirm clean.** The boundary tripwire greps the diff
  for `Editor`/`Edit`/`Action`/`Change` outside `text/edit/`. E4's new files
  (`clipboard.rs`, `undo.rs`) and the edited `input.rs`/`state.rs` are ALL inside
  `text/edit/`. The new Messages (`EditUndone`/`EditRedone`) and the `Clipboard`
  resource registration in `text/mod.rs` name NO cosmic type. Run:

  ```sh
  cargo test -p buiy_core --test text_facade_boundary
  ```

  Expected: pass. (`undo.rs` names `cosmic_text::Change` but is inside the facade
  — the tripwire only flags cosmic types OUTSIDE `text/edit/`.)

- [ ] **The full headless gate** (CLAUDE.md § Build & Test):

  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    xvfb-run -a cargo test --workspace
  ```

  Expected: all green. Resolve every clippy/doc warning before committing
  (mechanical-rigor rule). The new public items (`ClipboardProvider`,
  `Clipboard`, `UndoStack`, `UndoUnit`, `GroupKind`, `EditContext`, `EditUndone`,
  `EditRedone`, `MemClipboard`, `ArboardClipboard`) need doc comments — they have
  them in the code above; verify `cargo doc` is clean.

- [ ] **Supply-chain re-confirm** (the dep is now locked):

  ```sh
  cargo deny check
  ```

  Expected: green.

- [ ] **Commit (if the gate surfaced any fmt/clippy fixups):**
  `chore(text-editing): E4 gate — fmt/clippy/doc clean`

---

## Self-review against spec §§ 7 / 8 / 11

**§ 7 Clipboard — coverage.**
- arboard 3.6.x as a direct dep behind a `ClipboardProvider` Resource
  trait-object — Task 1 (`Clipboard(Box<dyn ClipboardProvider>)`, `ArboardClipboard`
  real, `MemClipboard` fake). ✓
- `cargo deny check` at adoption — Task 1 + Task 8. ✓
- `Cut`/`Copy` via `copy_selection()`, `Paste` through the § 3.3 newline policy —
  Task 5 (`single_line` strip in the `Paste` arm). ✓
- Plain text only (HTML/image deferred — decision 4) — the trait is text-only;
  no MIME surface. ✓
- ReadOnly: copy yes, cut/paste no — Task 5 (`Cut`/`Paste` arms early-return on
  `ctx.read_only`; `Copy` does not). Matches § 2.2 "ReadOnly keeps copy". ✓

**§ 8 Undo/redo — coverage.**
- Two-stack model over `Change::reverse` + `apply_change` — Task 2 (`UndoStack`)
  + Task 4 (`apply_undo`/`apply_redo` replay, the `vi` reference shape). ✓
- `UndoUnit { change, caret_before/after, selection_before/after, group }` — Task 2,
  exact spec field set. ✓
- `GroupKind: Composition | TypingRun | DeleteRun | Discrete` — Task 2. Composition
  is shaped (non-coalescing), emitted by E5 — does not preclude § 6.2c. ✓
- Grouping: typing coalesces by time window + caret adjacency; same-direction
  deletes coalesce; motion/click/discrete seals — Task 3 (`record_grouped`) +
  Task 4 (`seal` on Motion/SelectAll/Escape/Enter; Cut/Paste seal then record
  Discrete). ✓
- Undo restores `_before`, redo `_after`; redo clears on new edit — Task 2/4
  (`pop_undo`/`pop_redo`/`restore_cursor`; `record`/`record_grouped` clear redo). ✓
- Depth-bounded (config, default 1000) — Task 2 (`with_depth`, `DEFAULT_UNDO_DEPTH`,
  `enforce_depth` drops oldest). ✓
- Empty changes never pushed — Task 2 (`record` guard) + verified against
  `finish_change` returning `Some(empty)` (`editor.rs:512`). Covered by
  `record_drops_an_empty_change` + `backspace_at_offset_zero_records_no_unit`. ✓
- Property test (undo-all = identity on value + caret) — Task 6. ✓
- Undo lives in core, on by default — `undo: UndoStack` field on `TextEditState`,
  always constructed; `Clipboard` resource inserted by `BuiyTextPlugin`. ✓

**§ 11 Messages — coverage.**
- `EditUndone` / `EditRedone` carrying `(Entity, GroupKind)` — Task 2 (defined) +
  Task 4 (emitted from the system, group snapshotted before the pop) + registered
  in `text/mod.rs`. ✓
- Emitted on transition only (a no-op Undo with nothing to undo writes nothing —
  the system guards on `outcome.value_changed`). ✓
- E4 owns ONLY `EditUndone`/`EditRedone` (campaign Message-taxonomy note);
  `TextChanged` (E2) still fires for the value change an undo/redo causes — correct
  (an undo changes the logical value, so consumers re-read). No half-built
  E5/E6 Messages. ✓

**Type consistency.**
- `Change`/`ChangeItem`/`Cursor` field names match vendored cosmic-text 0.19
  (`items: Vec<ChangeItem>`, `ChangeItem { start, end, text, insert }`,
  `Cursor::new(line, index)`). ✓
- `TextSelection`/`SelectionRange` reused from E3 (`collapsed`, `is_collapsed`,
  `primary.anchor`/`primary.active`) — `UndoUnit.selection_*` uses the E3 type
  unchanged. ✓
- `EditContext` threads `now: Duration` (matching `Time::elapsed()` and the E3
  blink reset's `Duration`) and `&mut dyn ClipboardProvider`. ✓

**No placeholders.** Every code block is complete and compilable — no `// add X`,
no `todo!()`. The one E2→E4 transitional no-op (Task 4's `Cut|Copy|Paste` arm) is
explicitly replaced with real code in Task 5, RED-first. Every cosmic / Bevy API
named in the plan was verified against vendored source during grounding:
`set_cursor` (`edit/mod.rs:207`), `apply_change`/`finish_change`/`copy_selection`/
`delete_selection` (`edit/{mod,editor}.rs`), `Change`/`ChangeItem` fields,
`Messages::get_cursor` (`tests/picking_backend.rs:71`), `Time<Virtual>::advance_by`
(`tests/text_caret_selection.rs:178`).

**DRY / YAGNI / TDD.** `tracked_edit` is the single change-wrapping helper (Insert/
Backspace/Delete/Enter/Cut/Paste all route through it — DRY); the clipboard is
text-only (no speculative MIME — YAGNI, decision 4); every task is failing-test-
first (TDD). Composition grouping is shaped but not built (YAGNI for E4 — E5 owns
it) while leaving `record`/`GroupKind::Composition` as the non-precluding seam.

---

## E4 erratum (spec inaccuracy found while grounding)

**`finish_change` returns `Some(empty Change)`, not `None`, on a no-op edit.**
Spec § 8 describes wrapping each edit in `start_change`/`finish_change` and
pushing "the resulting `Change`" but does not state the empty-change case. The
vendored source (`cosmic-text-0.19.0/src/edit/editor.rs:512-513`) shows
`finish_change` is `self.change.take()` — it returns `Some(Change { items: [] })`
whenever `start_change` ran but the action recorded nothing (Backspace at offset
0, Delete at end). The grouping engine therefore MUST skip empty changes
(`UndoStack::record`'s `if unit.change.items.is_empty() { return }` guard), exactly
as cosmic's own `vi` reference does (`edit/vi.rs:38-42`). Fold a one-line note into
spec § 8 at campaign closure: "`finish_change` may return an empty `Change`; the
stack drops it." Zero design impact — the engine handles it — but the spec should
record the fact so a future reader does not re-derive it.
