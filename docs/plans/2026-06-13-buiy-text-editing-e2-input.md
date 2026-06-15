# Buiy text-editing E2 — input translation + editing operations + the latency gate

**Date:** 2026-06-13
**Phase:** E2 (of the E1–E6 text-editing campaign)
**Branch:** `text-editing-e2` (off `main`, which now includes E1)
**Campaign plan:** [2026-06-13-buiy-text-editing-campaign.md](2026-06-13-buiy-text-editing-campaign.md) § "E2 — Input translation + editing operations + the latency gate"
**Spec realized:** [editing-and-ime.md](../specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md) §§ 3, 3.1, 3.2, 3.3, 11
**Readiness:** [2026-06-13-text-editing-design-readiness.md](../reports/2026-06-13-text-editing-design-readiness.md) (OQ#1 resolved: one-frame latency; the new Input-driven N→N+1 fixture is E2's gate)

---

## Goal

Turn keystrokes into buffer edits. E1 landed the `TextEditState` substrate (the
`cosmic_text::Editor<'static>` over `BufferRef::Owned`, the policy markers,
`TextBufferAccess`). E2 adds the input layer **above** it: a data-driven
per-platform keymap table that maps `(modifiers, logical Key) → EditCommand`, a
focus-gated system in `BuiySet::Input` that reads `KeyboardInput` Messages and
lowers each `EditCommand` to one or more cosmic `Action`s applied to the editor,
the `SingleLine` Enter→Submit / newline-strip policy, the logical-value read
(`value()`), and the `TextChanged` Message. The phase closes with the
**Input-driven N→N+1 latency fixture** (OQ#1's gate) — an edit applied in
`BuiySet::Input` publishes glyphs one frame later.

Everything that names a cosmic `Action`/`Editor`/`Edit`/`Change` type stays
**inside `crates/buiy_core/src/text/edit/`** (the facade-boundary tripwire
`tests/text_facade_boundary.rs` fails the build otherwise). `EditCommand` itself
names no cosmic type, so it is public facade API; the lowering to `Action` names
`Action`, so it lives in the facade.

## Architecture

```
KeyboardInput Message (bevy_input: key_code, logical_key: Key, state, text, repeat, window)
        │  [system: apply_keyboard_edits, BuiySet::Input, focus-gated on FocusedEntity → editable TextEditState]
        ▼
Keymap (Resource: KeymapTable, one per platform, built at plugin init by data swap)
        │  table.resolve(mods, &logical_key) ─────────────► Option<EditCommand>   (the § 3.1 NORMATIVE rows)
        │  Key::Character(text) with no command ──────────► EditCommand::Insert(String)  (text field, layout-resolved)
        ▼
EditCommand (public facade enum — names NO cosmic type)
        │  TextEditState::apply(&mut FontSystem, EditCommand, single_line, read_only) → EditOutcome
        │     (the ONLY place EditCommand lowers to cosmic Action — inside the facade)
        ▼
cosmic_text::Editor::action(&mut FontSystem, Action…)   (one SharedFontSystem lock hold per system run)
        │
        ▼
EditOutcome { value_changed: bool, submitted: bool }
        │  value_changed ⇒ ① MessageWriter<TextChanged>.write(TextChanged(entity))
        │                   ② state.invalidate_intrinsics()           (the intrinsics cache is now stale)
        │                   ③ tree.mark_dirty_for_entity(entity)      (Taffy must re-measure the node)
        │  submitted     ⇒ EditCommand::Submit consumed internally (host-facing EditSubmitted is E6)
        ▼
next frame: TextSync runs (its TRIGGERS do NOT fire — Text is unchanged), but the
            node is already Taffy-dirty (③), so measure → TextCommit → extract_buiy_glyphs
            republish the edited buffer  (N → N+1)
```

**The dirty-mark seam (M1 — the load-bearing integration fact).** The text
pipeline is driven by two things: `TextSyncTriggers =
Or<(Changed<Text>, Changed<style carriers>, Added<TextBuffer>, …)>`
(`sync.rs:68-90`) and Taffy node dirtiness. An `Action::Insert` into the
editor-**owned** buffer leaves the `Text` component UNCHANGED, so none of the
sync triggers fire, `sync_one` never runs for the edited editor, and the
`mark_dirty_for_entity` call that lives **only inside `sync_one`**
(`sync.rs:335`) never happens — Taffy serves a cached measurement and
`text_commit`'s guard (`if !align_changed && !offset_stale && !size_stale
continue`, `commit.rs:102-104`) skips the reshape. The edit would dead-end with
no new `ComputedTextLayout` and no glyph republish. **So `apply_keyboard_edits`
must do for an editor edit exactly what `sync_one` does for a `Text` edit:** on
any reshaping command (insert / delete / multi-line Enter — i.e. whenever
`EditOutcome.value_changed`), (②) `invalidate_intrinsics()` on the edited
entity's authoritative buffer and (③) `mark_dirty_for_entity(entity)` on the
`LayoutTree`, so the **existing** measure → commit → extract path runs next
frame. Pure motion (`value_changed == false`) reshapes nothing and dirties
nothing.

**Why dirty-mark-from-input, not a new editor-edit-tick trigger.** The most
direct fix mirrors the established seam: `sync_one` already pairs
`invalidate_intrinsics()` + `mark_dirty_for_entity()` as *the* "buffer content
changed, re-measure me" gesture (`sync.rs:330,335`), so an editor edit performs
the identical pair from the input system. The runner-up — adding a
`Changed<TextEditState>`-style trigger to `TextSyncTriggers` so `sync_one`
re-runs for edited editors — is rejected: it would re-run the whole authored-
style resolution (`AuthoredStyle::resolve` + `apply_authored_to_buffer`,
re-applying the `Text` component over the editor's already-edited buffer,
CLOBBERING the edit). The dirty-mark targets only the measure/commit/extract
tail, which is exactly what a buffer-content edit needs. This adds **no new
Taffy compute pass** — OQ#1's "one-frame latency, no new machinery" holds for
the *scheduling* (Input still runs after Layout, so publish is N→N+1); the
dirty-mark is what lets the edit ENTER the existing measure path, not a fourth
Taffy site.

**`apply_keyboard_edits` is a NonSend main-thread system.** `LayoutTree` is an
`init_non_send_resource` (`layout/mod.rs:46`), so reaching it requires
`NonSendMut<LayoutTree>` — which pins the system to the main thread, exactly as
`text_sync_buffers` is. It takes `Option<NonSendMut<LayoutTree>>` so a
standalone `BuiyTextPlugin` app (no `LayoutPlugin`, hence no `LayoutTree`) runs
the system inertly, mirroring `text_sync_buffers`'s `Option<NonSendMut<LayoutTree>>`
(`sync.rs:147`).

**Lock discipline (architecture § 1.2 — exactly three lock sites: measure,
TextCommit, glyph producer; "reviewers reject a fourth").** Applying an
`Action` that reshapes needs the `FontSystem`, which is the fourth site if done
naively. **Resolution:** E2 does **not** add a steady-state lock site. The
keymap system locks `SharedFontSystem` **only on a frame where at least one
editing command actually arrives** for the focused editor (guarded behind an
early-return when no `KeyboardInput` events targeted an editable entity), and
the lock hold is the system body, mirroring how `TextCommit` holds it. This is
the input equivalent of the measure/commit sites — an edit *is* a reshape
trigger. Recorded as E2 erratum 2 (the architecture § 1.2 "three sites" count
predates the editor input path; the input apply is a legitimate fourth shaping
site, gated to fire only on real edits). A reviewer check: the lock is acquired
inside the per-edit branch, never unconditionally per frame.

**Why `EditCommand::Insert(String)` not `Insert(SmolStr)` (deviation from the
spec § 3 sketch).** The spec sketches `Insert(SmolStr)`. `smol_str` is **not** a
Buiy dependency, and `KeyboardInput.text` is `Option<SmolStr>` only because
bevy_input enables the `smol_str` feature. Holding a `String` (copied from the
event's `text`) needs no new dependency, lowers identically (we iterate `chars()`
either way, because `Action::Insert` takes a `char`), and keeps `EditCommand`
dependency-light. Recorded as E2 erratum 1.

## Tech stack

- **Bevy 0.18.1** — `KeyboardInput` is a `Message` (registered by bevy's own
  `InputPlugin`); `Key` (logical) is a **flat** `#[non_exhaustive]` enum (no
  `NamedKey` wrapper); modifiers read from `Res<ButtonInput<KeyCode>>`
  (`ControlLeft/Right`, `SuperLeft/Right`, `AltLeft/Right`, `ShiftLeft/Right`).
  Messages: `#[derive(Message)]`, `app.add_message::<T>()`, `MessageReader<T>`,
  `MessageWriter<T>` with `.read()` / `.write()`.
- **cosmic-text 0.19** — `Edit::action(&mut self, &mut FontSystem, Action)`;
  `Action::{Motion(Motion), Insert(char), Backspace, Delete, Enter, Escape}`;
  `Motion::{Left, Right, Up, Down, Home, End, LeftWord, RightWord, BufferStart,
  BufferEnd, PageUp, PageDown, …}` (22 variants); `Edit::{cursor, set_cursor,
  selection, set_selection, copy_selection, delete_selection}`;
  `Selection::{None, Normal(Cursor), Line, Word}`. **`Action::Motion` does NOT
  manage the selection anchor** — extend-vs-collapse is the lowering's job
  (Task 4; the motion arm only updates the cursor — `editor.rs:520-528`).
  `Action::Insert('\n')` self-routes to `Action::Enter` (verified
  `editor.rs:543-544`), so the single-line newline filter happens in the
  lowering, not in cosmic-text.
- **Rust** — `#[cfg!(target_os = …)]` evaluated ONCE at plugin init to select a
  table (a data swap, NOT scattered `cfg` in the hot system, per § 3.2).

---

## Files E2 creates

| File | Contents |
|---|---|
| `crates/buiy_core/src/text/edit/command.rs` | `EditCommand` enum (public facade API; names no cosmic type) |
| `crates/buiy_core/src/text/edit/keymap.rs` | `Modifiers`, `KeymapTable`, `Keymap` (Resource), `linux_windows_keymap()`, `macos_keymap()`, `default_keymap_for_platform()` — data only, no cosmic type |
| `crates/buiy_core/src/text/edit/input.rs` | `TextChanged` Message, `EditOutcome`, `TextEditState::apply` (the lowering — names `Action`), `apply_keyboard_edits` system, `value()` |
| `crates/buiy_core/tests/text_keymap.rs` | keymap table tests (per-platform rows → `EditCommand`) |
| `crates/buiy_core/tests/text_editing_ops.rs` | char insert, grapheme delete, motion/extend, SingleLine policy, repeat, `value()`/`TextChanged`, focus-gating, ReadOnly gating |
| `crates/buiy_core/tests/text_input_latency.rs` | the Input-driven N→N+1 latency fixture (OQ#1 gate) |

## Files E2 modifies

| File | Change |
|---|---|
| `crates/buiy_core/src/text/edit/mod.rs` | declare `command`/`keymap`/`input` modules; re-export the new public items |
| `crates/buiy_core/src/text/edit/state.rs` | add `value()` (reads editor buffer text) + `invalidate_intrinsics()` (the M1 dirty-mark's cache-drop half) to `TextEditState`; `apply` is added in `input.rs` via `impl TextEditState` (split so the cosmic-`Action`-naming code is in `input.rs`) |
| `crates/buiy_core/src/text/mod.rs` | re-export `EditCommand`, `Keymap`, `TextChanged`; register `Keymap` resource + `TextChanged` message + `apply_keyboard_edits` system in `BuiyTextPlugin::build` |
| `crates/buiy_core/src/text/sync.rs` | thread the `SingleLine` marker through `apply_authored_to_buffer` (BOTH call sites — m1) so a single-line editor buffer is `Wrap::None` (§ 3.3) |

---

## Task 0 — Branch sanity check

- [ ] **Confirm branch + clean tree.**

  ```sh
  cd /mnt/storage/projects/buiy/.claude/worktrees/render-pipeline
  git branch --show-current   # expect: text-editing-e2
  git log --oneline -1        # expect: E1 — editor substrate ... at or under HEAD
  cargo test -p buiy_core --test text_edit_substrate --test text_facade_boundary
  ```

  Expected: branch is `text-editing-e2`; both E1 test binaries pass. This pins
  the substrate E2 builds on before any change.

---

## Task 1 — `EditCommand`, the verb enum

The Buiy-owned command vocabulary (spec § 3 shape). It names **no** cosmic type,
so it is public facade API — but it lives in the facade module because the
lowering that consumes it (Task 6) names `Action`.

### Step 1.1 — Failing test: the enum exists and carries its variants

- [ ] Create `crates/buiy_core/tests/text_keymap.rs`:

  ```rust
  //! E2 — the keymap table and `EditCommand` vocabulary (editing-and-ime
  //! §§ 3, 3.1, 3.2). Pure / headless: no World, no adapter, no FontSystem —
  //! the table is data and `resolve` is a lookup.

  use buiy_core::text::edit::{EditCommand, Keymap, Modifiers};
  use cosmic_text::Motion;

  /// `EditCommand` carries the § 3 verbs. This pins the public shape the
  /// keymap produces and the lowering consumes.
  #[test]
  fn edit_command_carries_its_variants() {
      // Motion with the extend flag (Shift+arrow ⇒ extend = true).
      let m = EditCommand::Motion(Motion::Left, true);
      assert_eq!(m, EditCommand::Motion(Motion::Left, true));
      assert_ne!(m, EditCommand::Motion(Motion::Left, false));

      // The discrete verbs construct and compare.
      assert_eq!(EditCommand::Backspace, EditCommand::Backspace);
      assert_eq!(EditCommand::Insert(String::from("a")), EditCommand::Insert("a".into()));
      assert_ne!(EditCommand::Cut, EditCommand::Copy);
      assert_eq!(EditCommand::Submit, EditCommand::Submit);
  }
  ```

- [ ] Run it — expect a COMPILE failure (`EditCommand`, `Keymap`, `Modifiers`
  do not exist yet):

  ```sh
  cargo test -p buiy_core --test text_keymap 2>&1 | tail -20
  ```

  Expected: `error[E0432]: unresolved import ... EditCommand` (and `Keymap`,
  `Modifiers`).

### Step 1.2 — Implement `EditCommand`

- [ ] Create `crates/buiy_core/src/text/edit/command.rs`:

  ```rust
  //! `EditCommand` — the Buiy-owned editing verb vocabulary (editing-and-ime
  //! § 3). It borrows the cosmic `Action` *shape* but is Buiy-owned because
  //! clipboard / undo / submit verbs do not exist in `Action`. It names ONE
  //! cosmic type — `Motion` — which is a pure-data cursor-movement enum (no
  //! `Editor`/`Edit`/`Change`), so the facade-boundary tripwire (`Editor`,
  //! `Edit`, `Action`, `Change`) does not flag it. The lowering to `Action`
  //! (input.rs) is what must stay in the facade.
  //!
  //! **`Insert(String)` not `SmolStr`** (E2 erratum 1): `smol_str` is not a
  //! Buiy dependency; a `String` copied from `KeyboardInput.text` lowers
  //! identically (`Action::Insert` takes a `char`, so we iterate `chars()`).

  use cosmic_text::Motion;

  /// A single editing command, the unit the keymap produces and the editor
  /// applies (editing-and-ime § 3). Clipboard verbs (`Cut`/`Copy`/`Paste`)
  /// and undo verbs (`Undo`/`Redo`) are recognized here so the keymap rows
  /// exist from E2, but their behavior lands in E4 (this phase routes them to
  /// a no-op with a documented TODO — they must not silently insert text).
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum EditCommand {
      /// Cursor movement (arrows, Home/End, word-nav, PgUp/PgDn, doc
      /// start/end). `extend = true` grows the selection (Shift held); the
      /// editor moves in **visual** order per UAX #9 — the keymap never
      /// computes BiDi (§ 4.1).
      Motion(Motion, /* extend: */ bool),
      /// Insert literal text (the layout-resolved, dead-key-composed event
      /// `text` field), iterated as chars into `Action::Insert` (§ 3).
      Insert(String),
      /// Grapheme-correct deletion before / at the caret (inherited from
      /// `Action::Backspace` / `Action::Delete`).
      Backspace,
      Delete,
      /// Newline (multi-line) — on a `SingleLine` editor this is intercepted
      /// to `Submit` before reaching the editor (§ 3.3).
      Enter,
      /// § 7 — behavior is E4. Recognized here so the keymap rows exist.
      Cut,
      Copy,
      Paste,
      /// § 8 — behavior is E4.
      Undo,
      Redo,
      /// Select the whole buffer (Ctrl/Cmd-A).
      SelectAll,
      /// Clear the selection / cancel composition (§ 6.2d).
      Escape,
      /// Single-line Enter (§ 3.3): the host-facing `EditSubmitted` Message is
      /// finalized in E6; E2 emits it internally as an `EditOutcome` flag.
      Submit,
  }
  ```

- [ ] Wire it into the facade `mod.rs`. Edit `crates/buiy_core/src/text/edit/mod.rs`:

  ```rust
  //! `buiy_core::text::edit` — the editing facade (editing-and-ime § 2.1
  //! "lock-in containment"). This module, and ONLY this module, names the
  //! cosmic `Editor`/`Edit`/`Action`/`Change` types; every other Buiy system
  //! talks to `TextEditState` and `TextBufferAccess`. A future substrate swap
  //! stays local here. The boundary is mechanically enforced by
  //! `tests/text_facade_boundary.rs`.
  //!
  //! E1 lands the substrate: `TextEditState`, the policy markers, and the
  //! `TextBufferAccess` accessor. E2 adds input: the `EditCommand` vocabulary,
  //! the per-platform `Keymap`, and the `apply_keyboard_edits` system that
  //! lowers commands to cosmic `Action`s. Caret/selection (E3), clipboard/undo
  //! (E4), IME (E5), and lifecycle/widget (E6) extend it.

  mod access;
  mod command;
  mod input;
  mod keymap;
  mod state;

  pub use access::{
      TextBufferAccess, TextBufferAccessItem, TextBufferAccessReadOnly, TextBufferAccessReadOnlyItem,
  };
  pub use command::EditCommand;
  pub use input::{TextChanged, apply_keyboard_edits};
  pub use keymap::{Keymap, KeymapTable, Modifiers, default_keymap_for_platform};
  pub use state::{Disabled, Placeholder, ReadOnly, SingleLine, TextEditState};
  ```

  > Note: this references `keymap` and `input` modules created in Tasks 2–3 and
  > 6. To keep the build green between tasks, add the `mod`/`pub use` lines for
  > each module **as that module's first implementation step lands** — do NOT
  > add all four `mod` lines now (the files don't exist yet). Add `mod command;`
  > + `pub use command::EditCommand;` now; add the others in their tasks. The
  > block above is the FINAL state of the file at end of E2.

- [ ] For THIS step, the minimal `mod.rs` edit is only:

  ```rust
  mod access;
  mod command;
  mod state;

  pub use access::{
      TextBufferAccess, TextBufferAccessItem, TextBufferAccessReadOnly, TextBufferAccessReadOnlyItem,
  };
  pub use command::EditCommand;
  pub use state::{Disabled, Placeholder, ReadOnly, SingleLine, TextEditState};
  ```

### Step 1.3 — Run it green

- [ ] The test still imports `Keymap` and `Modifiers` (Task 2), so it won't
  compile yet. Temporarily narrow the test's `use` to only what Step 1.1
  asserts, OR (preferred) **stop here and run after Task 2** so the imports
  resolve. Since the keymap test file imports all three, the clean checkpoint is:
  build the crate alone to prove `EditCommand` compiles:

  ```sh
  cargo build -p buiy_core 2>&1 | tail -5
  ```

  Expected: `Finished` (the library compiles with `EditCommand` added; the test
  binary is not built by `cargo build`).

- [ ] **Commit:**

  ```sh
  git add -A && git commit -m "feat(text-editing): E2 task 1 — EditCommand verb enum

The Buiy-owned editing vocabulary (editing-and-ime § 3). Names only the
pure-data cosmic Motion enum, so it is public facade API; the lowering to
Action stays in the facade. Insert holds String (no smol_str dep — E2
erratum 1).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 2 — `Modifiers` + `KeymapTable` (the data structure)

The lookup is `(Modifiers, logical Key) → EditCommand`. We key on a small
owned `KeyKind` discriminant rather than `bevy_input::Key` directly, because
`Key::Character` carries a `SmolStr` payload (not `Hash`-friendly as a map key
across owned vs borrowed) and because the table must be platform-data, not
bevy-typed. The system (Task 5) translates a live `Key` into `(Modifiers,
KeyKind)` then calls `table.resolve`.

### Step 2.1 — Failing test: empty-table resolve + modifier equality

- [ ] Append to `crates/buiy_core/tests/text_keymap.rs`:

  ```rust
  use buiy_core::text::edit::KeymapTable;

  /// `Modifiers` is a plain bitset-ish struct; equality and the `ctrl()`/
  /// `shift()` constructors behave.
  #[test]
  fn modifiers_construct_and_compare() {
      assert_eq!(Modifiers::NONE, Modifiers::default());
      assert_ne!(Modifiers::NONE, Modifiers::ctrl());
      assert_eq!(Modifiers::ctrl(), Modifiers { ctrl: true, ..Modifiers::NONE });
      let cs = Modifiers { ctrl: true, shift: true, ..Modifiers::NONE };
      assert!(cs.ctrl && cs.shift && !cs.alt && !cs.cmd);
  }

  /// An empty table resolves nothing; an inserted row resolves exactly.
  #[test]
  fn keymap_table_resolves_inserted_rows() {
      use buiy_core::text::edit::keymap::KeyKind;
      let mut table = KeymapTable::default();
      assert_eq!(table.resolve(Modifiers::NONE, KeyKind::ArrowLeft), None);
      table.insert(Modifiers::NONE, KeyKind::ArrowLeft, EditCommand::Motion(Motion::Left, false));
      assert_eq!(
          table.resolve(Modifiers::NONE, KeyKind::ArrowLeft),
          Some(EditCommand::Motion(Motion::Left, false)),
      );
      // A different modifier set does not collide.
      assert_eq!(table.resolve(Modifiers::shift(), KeyKind::ArrowLeft), None);
  }
  ```

  > `KeyKind` is referenced via the `keymap` submodule path
  > `buiy_core::text::edit::keymap::KeyKind` — make the submodule `pub` and
  > `KeyKind` `pub` so the test (and only the test) can name the kinds directly.
  > Production code reaches them through the table builder + the system's
  > `Key → KeyKind` mapping.

- [ ] Run — expect compile failure (`KeymapTable`, `Modifiers`, `KeyKind`
  missing). The `Keymap` import from Task 1's test file also still dangles; that
  resolves in Step 2.2.

  ```sh
  cargo test -p buiy_core --test text_keymap 2>&1 | tail -20
  ```

### Step 2.2 — Implement `keymap.rs` data structures (no tables yet)

- [ ] Create `crates/buiy_core/src/text/edit/keymap.rs`:

  ```rust
  //! The data-driven editing keymap (editing-and-ime §§ 3.1, 3.2). One table
  //! per platform, selected ONCE at plugin init by a data swap
  //! (`default_keymap_for_platform`) — never scattered `cfg` in the hot
  //! system. The table is keyed on `(Modifiers, KeyKind)`; `KeyKind` is a
  //! small owned discriminant the input system derives from a live
  //! `bevy_input::Key`, so the table stays platform-data and the lookup key is
  //! `Hash`-clean (a `Key::Character(SmolStr)` payload never enters a map key).
  //!
  //! This file names NO cosmic `Editor`/`Edit`/`Action`/`Change` type — only
  //! the pure-data `Motion` (via `EditCommand`). It is inside the facade
  //! regardless (the editing module), but it would pass the boundary scan even
  //! outside it.

  use std::collections::HashMap;

  use bevy::prelude::Resource;

  use super::command::EditCommand;

  /// The active editing keymap, selected per platform at plugin init
  /// (§ 3.2). A `Resource` so the input system reads it; the later
  /// user-rebinding hook (§ 3.2, v1 ships fixed tables) replaces this
  /// resource's table.
  #[derive(Resource)]
  pub struct Keymap(pub KeymapTable);

  impl Default for Keymap {
      fn default() -> Self {
          Self(default_keymap_for_platform())
      }
  }

  /// The keyboard modifier state at the moment a key was pressed. `cmd` is the
  /// macOS Command key (`Super`); `ctrl`/`alt`/`shift` are uniform. The
  /// per-platform tables key the doc/word/clipboard verbs on the platform's
  /// modifier (Ctrl on Linux/Windows, Cmd on macOS), so a Linux table never
  /// has a `cmd` row and a macOS table never has a `ctrl` clipboard row.
  #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
  pub struct Modifiers {
      pub ctrl: bool,
      pub alt: bool,
      pub shift: bool,
      pub cmd: bool,
  }

  impl Modifiers {
      pub const NONE: Modifiers = Modifiers { ctrl: false, alt: false, shift: false, cmd: false };
      pub fn ctrl() -> Self { Modifiers { ctrl: true, ..Self::NONE } }
      pub fn shift() -> Self { Modifiers { shift: true, ..Self::NONE } }
      pub fn cmd() -> Self { Modifiers { cmd: true, ..Self::NONE } }
      pub fn alt() -> Self { Modifiers { alt: true, ..Self::NONE } }
  }

  /// A platform-neutral key discriminant — the subset of logical keys the
  /// editor binds. The input system maps a live `bevy_input::Key` to one of
  /// these (and `Char` carries the inserted text for the `Insert` fall-through,
  /// handled in the system, not the table). `Char` is intentionally NOT a
  /// table key — character insertion is the default when no command matches.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum KeyKind {
      ArrowLeft,
      ArrowRight,
      ArrowUp,
      ArrowDown,
      Home,
      End,
      PageUp,
      PageDown,
      Backspace,
      Delete,
      Enter,
      Escape,
      /// A printable-character key — never a table row; the system inserts the
      /// event `text` when `resolve` returns `None` for it.
      Char,
  }

  /// The `(Modifiers, KeyKind) → EditCommand` lookup table (§ 3.1).
  #[derive(Default)]
  pub struct KeymapTable {
      rows: HashMap<(Modifiers, KeyKind), EditCommand>,
  }

  impl KeymapTable {
      /// Bind a row. Later inserts overwrite — the platform builders insert
      /// once each, so order is irrelevant in v1.
      pub fn insert(&mut self, mods: Modifiers, key: KeyKind, command: EditCommand) {
          self.rows.insert((mods, key), command);
      }

      /// Resolve a key event to its command, if bound. Returns a clone (the
      /// table outlives the lookup; `EditCommand` is cheap to clone — only
      /// `Insert` allocates, and `Insert` is never a table row).
      pub fn resolve(&self, mods: Modifiers, key: KeyKind) -> Option<EditCommand> {
          self.rows.get(&(mods, key)).cloned()
      }
  }

  /// Select the platform table at init (§ 3.2 — a data swap, evaluated ONCE
  /// here, never per-event `cfg`). `cfg!` is a const, so the non-target arms
  /// compile out.
  pub fn default_keymap_for_platform() -> KeymapTable {
      if cfg!(target_os = "macos") {
          macos_keymap()
      } else {
          linux_windows_keymap()
      }
  }

  // The per-platform tables land in Task 3.
  pub(crate) fn linux_windows_keymap() -> KeymapTable {
      KeymapTable::default()
  }

  pub(crate) fn macos_keymap() -> KeymapTable {
      KeymapTable::default()
  }
  ```

- [ ] Make the submodule + `KeyKind` visible to the test. Edit
  `crates/buiy_core/src/text/edit/mod.rs` to add the `keymap` module declaration
  and re-exports (additive to the Step 1.2 state):

  ```rust
  mod access;
  mod command;
  pub mod keymap;
  mod state;

  pub use access::{
      TextBufferAccess, TextBufferAccessItem, TextBufferAccessReadOnly, TextBufferAccessReadOnlyItem,
  };
  pub use command::EditCommand;
  pub use keymap::{Keymap, KeymapTable, Modifiers, default_keymap_for_platform};
  pub use state::{Disabled, Placeholder, ReadOnly, SingleLine, TextEditState};
  ```

  > `pub mod keymap` (not `mod keymap`) so the test can name
  > `keymap::KeyKind` — `KeyKind` is an internal discriminant, exposed only for
  > the table tests; production consumers use the re-exported `Keymap`/`Modifiers`.

### Step 2.3 — Run green

- [ ] Run the keymap test binary — Tasks 1 + 2 tests now compile and pass; the
  per-platform-row tests are added in Task 3:

  ```sh
  cargo test -p buiy_core --test text_keymap 2>&1 | tail -20
  ```

  Expected: `test result: ok. 4 passed` (the three from Task 1 still in the file
  — `edit_command_carries_its_variants` — plus `modifiers_construct_and_compare`
  and `keymap_table_resolves_inserted_rows`). Exact count: 3 tests
  (`edit_command_carries_its_variants`, `modifiers_construct_and_compare`,
  `keymap_table_resolves_inserted_rows`).

- [ ] **Commit:**

  ```sh
  git add -A && git commit -m "feat(text-editing): E2 task 2 — Modifiers + KeymapTable

The (Modifiers, KeyKind) -> EditCommand lookup and the platform-select
seam (editing-and-ime §§ 3.1, 3.2). KeyKind is an owned discriminant so the
map key never carries a SmolStr; default_keymap_for_platform swaps tables
once at init. Names no cosmic editor type.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 3 — The normative per-platform tables (§ 3.1 rows)

The § 3.1 standard-keys rows are **NORMATIVE**. This task encodes them as the
two platform tables and proves every row with table tests.

### Step 3.1 — Failing test: the normative rows resolve per platform

- [ ] Append to `crates/buiy_core/tests/text_keymap.rs`:

  ```rust
  use buiy_core::text::edit::keymap::{KeyKind, linux_windows_keymap, macos_keymap};

  /// § 3.1 (NORMATIVE), Linux/Windows table. Arrows, word-nav (Ctrl+arrow),
  /// Home/End, doc start/end (Ctrl+Home/End), PgUp/PgDn, Shift⇒extend,
  /// Ctrl-A/X/C/V/Z, redo (Ctrl-Y and Ctrl-Shift-Z), Backspace/Delete, Enter,
  /// Escape.
  #[test]
  fn linux_windows_table_encodes_the_normative_rows() {
      let t = linux_windows_keymap();
      let none = Modifiers::NONE;
      let ctrl = Modifiers::ctrl();
      let shift = Modifiers::shift();
      let ctrl_shift = Modifiers { ctrl: true, shift: true, ..Modifiers::NONE };

      // Plain arrows ⇒ Motion, no extend.
      assert_eq!(t.resolve(none, KeyKind::ArrowLeft), Some(EditCommand::Motion(Motion::Left, false)));
      assert_eq!(t.resolve(none, KeyKind::ArrowRight), Some(EditCommand::Motion(Motion::Right, false)));
      assert_eq!(t.resolve(none, KeyKind::ArrowUp), Some(EditCommand::Motion(Motion::Up, false)));
      assert_eq!(t.resolve(none, KeyKind::ArrowDown), Some(EditCommand::Motion(Motion::Down, false)));

      // Shift+arrow ⇒ extend = true.
      assert_eq!(t.resolve(shift, KeyKind::ArrowLeft), Some(EditCommand::Motion(Motion::Left, true)));

      // Ctrl+arrow ⇒ word-nav (visual LeftWord/RightWord, § 4.1).
      assert_eq!(t.resolve(ctrl, KeyKind::ArrowLeft), Some(EditCommand::Motion(Motion::LeftWord, false)));
      assert_eq!(t.resolve(ctrl, KeyKind::ArrowRight), Some(EditCommand::Motion(Motion::RightWord, false)));
      // Ctrl+Shift+arrow ⇒ word-nav extend.
      assert_eq!(t.resolve(ctrl_shift, KeyKind::ArrowLeft), Some(EditCommand::Motion(Motion::LeftWord, true)));

      // Home/End and doc start/end.
      assert_eq!(t.resolve(none, KeyKind::Home), Some(EditCommand::Motion(Motion::Home, false)));
      assert_eq!(t.resolve(none, KeyKind::End), Some(EditCommand::Motion(Motion::End, false)));
      assert_eq!(t.resolve(ctrl, KeyKind::Home), Some(EditCommand::Motion(Motion::BufferStart, false)));
      assert_eq!(t.resolve(ctrl, KeyKind::End), Some(EditCommand::Motion(Motion::BufferEnd, false)));
      assert_eq!(t.resolve(shift, KeyKind::Home), Some(EditCommand::Motion(Motion::Home, true)));

      // PgUp/PgDn.
      assert_eq!(t.resolve(none, KeyKind::PageUp), Some(EditCommand::Motion(Motion::PageUp, false)));
      assert_eq!(t.resolve(none, KeyKind::PageDown), Some(EditCommand::Motion(Motion::PageDown, false)));

      // Deletion + Enter + Escape.
      assert_eq!(t.resolve(none, KeyKind::Backspace), Some(EditCommand::Backspace));
      assert_eq!(t.resolve(none, KeyKind::Delete), Some(EditCommand::Delete));
      assert_eq!(t.resolve(none, KeyKind::Enter), Some(EditCommand::Enter));
      assert_eq!(t.resolve(none, KeyKind::Escape), Some(EditCommand::Escape));

      // Clipboard + undo + select-all on Ctrl (§§ 7, 8, 3.1).
      assert_eq!(t.resolve(ctrl, KeyKind::Char), None, "Char is never a table row");
      // Char-keyed verbs (A/X/C/V/Z/Y) are keyed on KeyKind::Char + a letter,
      // which the table does NOT model (Char carries no letter). They are
      // resolved by the SYSTEM from the logical key's character + modifiers
      // (Step 5). So the table holds only the non-character rows; the
      // letter-command rows are a separate lookup the system owns. This test
      // asserts the table's contract; the letter rows are tested in
      // text_editing_ops via the system.
  }

  /// § 3.2, macOS table: Cmd replaces Ctrl for doc-ends / clipboard /
  /// select-all / undo; Option replaces Ctrl for word-nav; Cmd+arrow is
  /// line-ends (Home/End).
  #[test]
  fn macos_table_uses_cmd_and_option() {
      let t = macos_keymap();
      let none = Modifiers::NONE;
      let cmd = Modifiers::cmd();
      let alt = Modifiers::alt();

      // Plain arrows unchanged.
      assert_eq!(t.resolve(none, KeyKind::ArrowLeft), Some(EditCommand::Motion(Motion::Left, false)));
      // Option+arrow ⇒ word-nav (macOS convention).
      assert_eq!(t.resolve(alt, KeyKind::ArrowLeft), Some(EditCommand::Motion(Motion::LeftWord, false)));
      assert_eq!(t.resolve(alt, KeyKind::ArrowRight), Some(EditCommand::Motion(Motion::RightWord, false)));
      // Cmd+arrow ⇒ line-ends (Home/End).
      assert_eq!(t.resolve(cmd, KeyKind::ArrowLeft), Some(EditCommand::Motion(Motion::Home, false)));
      assert_eq!(t.resolve(cmd, KeyKind::ArrowRight), Some(EditCommand::Motion(Motion::End, false)));
      // Cmd+Up/Down ⇒ buffer start/end (macOS doc nav).
      assert_eq!(t.resolve(cmd, KeyKind::ArrowUp), Some(EditCommand::Motion(Motion::BufferStart, false)));
      assert_eq!(t.resolve(cmd, KeyKind::ArrowDown), Some(EditCommand::Motion(Motion::BufferEnd, false)));
      // Linux Ctrl rows are NOT present on macOS (the data swap, not a runtime
      // branch — proves the tables genuinely differ).
      assert_eq!(t.resolve(Modifiers::ctrl(), KeyKind::ArrowLeft), None);
  }
  ```

- [ ] Run — expect FAIL (the two builders return empty tables):

  ```sh
  cargo test -p buiy_core --test text_keymap 2>&1 | tail -25
  ```

  Expected: `linux_windows_table_encodes_the_normative_rows` and
  `macos_table_uses_cmd_and_option` fail on the first `assert_eq!` (`None` vs
  `Some(...)`).

### Step 3.2 — Implement the two tables

- [ ] Replace the two stub builders at the bottom of
  `crates/buiy_core/src/text/edit/keymap.rs`:

  ```rust
  /// The Linux/Windows table (§§ 3.1, 3.2): Ctrl is the doc/word/clipboard
  /// modifier. Motion rows are emitted for both the plain and Shift (extend)
  /// modifier, so a single `motion` helper writes the pair.
  pub(crate) fn linux_windows_keymap() -> KeymapTable {
      use EditCommand::*;
      use KeyKind::*;
      let mut t = KeymapTable::default();

      // Plain + Shift(extend) for every arrow/Home/End/Page motion.
      let mut motion = |mods: Modifiers, key: KeyKind, m: Motion| {
          t.insert(mods, key, Motion(m, false));
          t.insert(Modifiers { shift: true, ..mods }, key, EditCommand::Motion(m, true));
      };
      motion(Modifiers::NONE, ArrowLeft, cosmic_text::Motion::Left);
      motion(Modifiers::NONE, ArrowRight, cosmic_text::Motion::Right);
      motion(Modifiers::NONE, ArrowUp, cosmic_text::Motion::Up);
      motion(Modifiers::NONE, ArrowDown, cosmic_text::Motion::Down);
      motion(Modifiers::NONE, Home, cosmic_text::Motion::Home);
      motion(Modifiers::NONE, End, cosmic_text::Motion::End);
      motion(Modifiers::NONE, PageUp, cosmic_text::Motion::PageUp);
      motion(Modifiers::NONE, PageDown, cosmic_text::Motion::PageDown);
      // Ctrl+arrow ⇒ visual word-nav; Ctrl+Home/End ⇒ doc start/end.
      motion(Modifiers::ctrl(), ArrowLeft, cosmic_text::Motion::LeftWord);
      motion(Modifiers::ctrl(), ArrowRight, cosmic_text::Motion::RightWord);
      motion(Modifiers::ctrl(), Home, cosmic_text::Motion::BufferStart);
      motion(Modifiers::ctrl(), End, cosmic_text::Motion::BufferEnd);

      // Discrete keys (no extend pairing).
      t.insert(Modifiers::NONE, Backspace, EditCommand::Backspace);
      t.insert(Modifiers::NONE, Delete, EditCommand::Delete);
      t.insert(Modifiers::NONE, Enter, EditCommand::Enter);
      t.insert(Modifiers::NONE, Escape, EditCommand::Escape);
      t
  }

  /// The macOS table (§ 3.2): Cmd is the doc/clipboard/undo/select-all
  /// modifier; Option is word-nav; Cmd+Left/Right are line-ends; Cmd+Up/Down
  /// are buffer start/end.
  pub(crate) fn macos_keymap() -> KeymapTable {
      use KeyKind::*;
      let mut t = KeymapTable::default();

      let mut motion = |mods: Modifiers, key: KeyKind, m: cosmic_text::Motion| {
          t.insert(mods, key, EditCommand::Motion(m, false));
          t.insert(Modifiers { shift: true, ..mods }, key, EditCommand::Motion(m, true));
      };
      // Plain arrows.
      motion(Modifiers::NONE, ArrowLeft, cosmic_text::Motion::Left);
      motion(Modifiers::NONE, ArrowRight, cosmic_text::Motion::Right);
      motion(Modifiers::NONE, ArrowUp, cosmic_text::Motion::Up);
      motion(Modifiers::NONE, ArrowDown, cosmic_text::Motion::Down);
      motion(Modifiers::NONE, Home, cosmic_text::Motion::Home);
      motion(Modifiers::NONE, End, cosmic_text::Motion::End);
      motion(Modifiers::NONE, PageUp, cosmic_text::Motion::PageUp);
      motion(Modifiers::NONE, PageDown, cosmic_text::Motion::PageDown);
      // Option(Alt)+Left/Right ⇒ word-nav.
      motion(Modifiers::alt(), ArrowLeft, cosmic_text::Motion::LeftWord);
      motion(Modifiers::alt(), ArrowRight, cosmic_text::Motion::RightWord);
      // Cmd+Left/Right ⇒ line-ends; Cmd+Up/Down ⇒ buffer ends.
      motion(Modifiers::cmd(), ArrowLeft, cosmic_text::Motion::Home);
      motion(Modifiers::cmd(), ArrowRight, cosmic_text::Motion::End);
      motion(Modifiers::cmd(), ArrowUp, cosmic_text::Motion::BufferStart);
      motion(Modifiers::cmd(), ArrowDown, cosmic_text::Motion::BufferEnd);

      t.insert(Modifiers::NONE, Backspace, EditCommand::Backspace);
      t.insert(Modifiers::NONE, Delete, EditCommand::Delete);
      t.insert(Modifiers::NONE, Enter, EditCommand::Enter);
      t.insert(Modifiers::NONE, Escape, EditCommand::Escape);
      t
  }
  ```

  > **The letter-command rows (A/X/C/V/Z/Y) are NOT in the table** and that is
  > deliberate. `KeyKind::Char` carries no letter, so a `(Ctrl, Char)` table row
  > cannot distinguish Ctrl-A from Ctrl-C. The system (Task 5) resolves these
  > from the logical key's **character** + the platform's command modifier,
  > because the spec keys them on the *letter*, not a positional key. This keeps
  > the table a pure positional-key map and puts the one character-sensitive
  > lookup in the system where the character is available. The letter rows are
  > proven through the system in `text_editing_ops` (Task 7).

- [ ] Add the `cosmic_text` import at the top of `keymap.rs` is NOT needed —
  the builders qualify `cosmic_text::Motion::…` inline, and `EditCommand`
  already re-exports through `super::command`. Confirm the `use` block at the
  top stays `use std::collections::HashMap; use bevy::prelude::Resource; use
  super::command::EditCommand;` (no cosmic import — the `Motion` references are
  fully qualified, keeping the file's import surface minimal).

  > Facade note: `keymap.rs` names `cosmic_text::Motion`. The boundary tripwire
  > only forbids `Editor`/`Edit`/`Action`/`Change` — `Motion` is allowed
  > anywhere (it is pure cursor-movement data). But `keymap.rs` is inside the
  > facade regardless, so this is doubly fine.

### Step 3.3 — Run green

- [ ] ```sh
  cargo test -p buiy_core --test text_keymap 2>&1 | tail -20
  ```

  Expected: `test result: ok. 5 passed` (the 3 from Tasks 1–2 plus the 2
  platform-table tests).

- [ ] **Commit:**

  ```sh
  git add -A && git commit -m "feat(text-editing): E2 task 3 — normative per-platform keymap tables

Encode the § 3.1 standard-keys rows (NORMATIVE) as the Linux/Windows and
macOS tables (§ 3.2 data swap). Motion rows emit the plain + Shift(extend)
pair via a helper; letter-commands (A/X/C/V/Z/Y) are system-resolved (Char
carries no letter), proven later via the system.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 4 — `TextEditState::apply` — the `EditCommand → Action` lowering

The single place an `EditCommand` becomes cosmic `Action`s, applied to the
editor. **This method names `Action`, so it MUST live in the facade.** It owns
the three things cosmic-text does not do for us:

1. **Selection extend/collapse** — `Action::Motion` does not touch the
   selection anchor (verified `editor.rs:520-528` — the motion arm only updates
   the cursor). For `extend = true` we drop an
   anchor (`set_selection(Selection::Normal(cursor))`) if none is active, then
   move. For `extend = false` we clear the selection (`set_selection(None)`)
   before moving — so a non-extending arrow collapses a selection.
2. **Single-line newline policy** — `Enter` on a `SingleLine` editor never
   reaches `Action::Enter`; it returns `submitted = true`. Inserted `text`
   containing `\n` has newlines stripped before insertion (§ 3.3).
3. **The value-changed signal** — `apply` reports whether the logical buffer
   text changed, so the system can emit `TextChanged` exactly once per change.

We test `apply` directly (it needs a real `FontSystem` for reshaping motions /
inserts; we build one via `SharedFontSystem::new()` and lock it — headless, no
adapter).

### Step 4.1 — Failing test: insert, delete, motion, single-line submit

- [ ] Create `crates/buiy_core/tests/text_editing_ops.rs`:

  ```rust
  //! E2 — editing operations applied to the editor (editing-and-ime §§ 3,
  //! 3.1, 3.3). The `apply` lowering and `value()` are tested directly against
  //! a real (headless) `FontSystem` — reshaping motions / inserts need it, but
  //! no adapter is involved (cosmic shaping is CPU). The system-level focus /
  //! gating / Message tests follow in later steps of this task.

  use buiy_core::text::SharedFontSystem;
  use buiy_core::text::edit::{EditCommand, TextEditState};
  use cosmic_text::{Metrics, Motion};

  /// Inserting characters grows the logical value; backspace shrinks it
  /// grapheme-correctly (inherited from `Action::Backspace`).
  #[test]
  fn insert_and_backspace_change_the_value() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      assert_eq!(state.value(), "");

      let mut fs = fonts.lock();
      let out = state.apply(&mut fs, EditCommand::Insert("hi".into()), false, false);
      assert!(out.value_changed);
      assert_eq!(state.value(), "hi");

      let out = state.apply(&mut fs, EditCommand::Backspace, false, false);
      assert!(out.value_changed);
      assert_eq!(state.value(), "h");
  }

  /// A non-extending motion does NOT change the value and reports
  /// `value_changed = false` (so it never emits `TextChanged`).
  #[test]
  fn motion_does_not_change_the_value() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      state.apply(&mut fs, EditCommand::Insert("abc".into()), false, false);

      let out = state.apply(&mut fs, EditCommand::Motion(Motion::Left, false), false, false);
      assert!(!out.value_changed, "moving the caret is not a value change");
      assert_eq!(state.value(), "abc");
  }

  /// On a `SingleLine` editor, `Enter` submits (never inserts a newline) and
  /// reports `submitted`; the value is unchanged.
  #[test]
  fn single_line_enter_submits_without_newline() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      state.apply(&mut fs, EditCommand::Insert("name".into()), false, false);

      let out = state.apply(&mut fs, EditCommand::Enter, /* single_line: */ true, false);
      assert!(out.submitted, "single-line Enter submits");
      assert!(!out.value_changed, "submit does not change the value");
      assert_eq!(state.value(), "name", "no newline inserted");
  }

  /// On a multi-line editor, `Enter` inserts a newline (the value gains it).
  #[test]
  fn multi_line_enter_inserts_a_newline() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);
      let out = state.apply(&mut fs, EditCommand::Enter, /* single_line: */ false, false);
      assert!(out.value_changed);
      assert!(!out.submitted);
      assert_eq!(state.value(), "ab\n");
  }

  /// A `SingleLine` insert strips newlines from pasted/typed text (§ 3.3).
  #[test]
  fn single_line_insert_strips_newlines() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      let out = state.apply(&mut fs, EditCommand::Insert("a\nb\nc".into()), /* single_line: */ true, false);
      assert!(out.value_changed);
      assert_eq!(state.value(), "abc", "newlines stripped on a single-line editor");
  }

  /// `ReadOnly` refuses mutation but allows motion: insert/backspace are
  /// no-ops (value unchanged, value_changed false); a motion still moves.
  #[test]
  fn read_only_blocks_mutation_allows_motion() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      // Seed text WITHOUT the read-only gate, then turn the gate on.
      state.apply(&mut fs, EditCommand::Insert("locked".into()), false, false);

      let out = state.apply(&mut fs, EditCommand::Insert("X".into()), false, /* read_only: */ true);
      assert!(!out.value_changed, "read-only refuses insertion");
      assert_eq!(state.value(), "locked");

      let out = state.apply(&mut fs, EditCommand::Backspace, false, true);
      assert!(!out.value_changed, "read-only refuses backspace");
      assert_eq!(state.value(), "locked");

      // Motion is allowed under read-only (caret/selection yes — § 2.2).
      let out = state.apply(&mut fs, EditCommand::Motion(Motion::Home, false), false, true);
      assert!(!out.value_changed, "motion never changes the value");
  }
  ```

- [ ] Run — expect COMPILE failure (`value`, `apply`, `EditOutcome` missing):

  ```sh
  cargo test -p buiy_core --test text_editing_ops 2>&1 | tail -20
  ```

### Step 4.2 — Implement `value()` on `TextEditState`

- [ ] Edit `crates/buiy_core/src/text/edit/state.rs` — add a `value()` reader to
  the `impl TextEditState` block (after `intrinsics()`):

  ```rust
      /// The logical value: the editor buffer's full text. Pre-IME this is the
      /// complete buffer content (editing-and-ime § 3); E5 refines this to
      /// subtract the live preedit byte range (invariant § 6.2b). Lines are
      /// joined with `\n` (the LF normalization the value contract uses; the
      /// editor stores per-line endings separately, `BufferLine::ending`).
      pub fn value(&self) -> String {
          use cosmic_text::Edit;
          self.editor.with_buffer(|buffer| {
              let mut out = String::new();
              for (i, line) in buffer.lines.iter().enumerate() {
                  if i > 0 {
                      out.push('\n');
                  }
                  out.push_str(line.text());
              }
              out
          })
      }

      /// Drop the cached intrinsic widths — the "buffer content changed,
      /// re-measure me" half of the dirty-mark seam (M1). The input system
      /// calls this on a reshaping edit, exactly as `sync_one` calls the
      /// accessor's `invalidate_intrinsics` after a `Text` change
      /// (`sync.rs:330`). Mutating `self.intrinsics` directly (not through the
      /// accessor) is correct here: the system already holds `&mut
      /// TextEditState` to apply the edit, and the `Changed<TextEditState>`
      /// tick the apply incurs is harmless (nothing keys off it; the edit IS a
      /// change). The other half — Taffy node dirtiness — is the system's
      /// `mark_dirty_for_entity` call (Task 5), because the node lives in
      /// `LayoutTree`, not on the component.
      pub fn invalidate_intrinsics(&mut self) {
          self.intrinsics = None;
      }
  ```

  > `buffer.lines` is a `pub Vec<BufferLine>` (buffer.rs:336); `line.text()`
  > returns `&str` WITHOUT the terminator (buffer_line.rs:68). Joining with
  > `\n` gives the canonical logical value. This is the pre-IME definition; E5
  > overrides it. `invalidate_intrinsics` writes the `pub(crate) intrinsics`
  > field directly (state.rs declares it `pub(crate)`); it is the input-side
  > mirror of the accessor's same-named method.

### Step 4.3 — Implement `apply` + `EditOutcome` in `input.rs`

- [ ] Create `crates/buiy_core/src/text/edit/input.rs`:

  ```rust
  //! E2 input application: the `EditCommand → cosmic Action` lowering
  //! (`TextEditState::apply`), the focus-gated `apply_keyboard_edits` system,
  //! and the `TextChanged` Message (editing-and-ime §§ 3, 3.3, 11). This file
  //! NAMES `Action` (the lowering), so it MUST stay inside the facade — the
  //! boundary tripwire (`tests/text_facade_boundary.rs`) enforces it.

  use bevy::prelude::*;
  use cosmic_text::{Action, Edit, FontSystem, Selection};

  use super::command::EditCommand;
  use super::state::TextEditState;

  /// Emitted when an editor's logical value changes (editing-and-ime § 11 row
  /// `TextChanged`). Never emitted for caret motion or for preedit (E5). The
  /// value is read from the component (`TextEditState::value`), so the payload
  /// is just the entity — the § 11 contract.
  #[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
  pub struct TextChanged(pub Entity);

  /// What one `apply` did, so the system can emit the right Messages. `value`
  /// changes drive `TextChanged`; `submitted` drives the internal Submit path
  /// (E6 turns it into the host-facing `EditSubmitted`).
  #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
  pub struct EditOutcome {
      pub value_changed: bool,
      pub submitted: bool,
  }

  impl TextEditState {
      /// Lower one `EditCommand` to cosmic `Action`s and apply it to the
      /// editor (editing-and-ime § 3). This is the ONE place `EditCommand`
      /// meets `Action`.
      ///
      /// - `single_line` (the `SingleLine` marker): `Enter ⇒ Submit`; inserted
      ///   text is newline-stripped (§ 3.3).
      /// - `read_only` (the `ReadOnly` marker): mutation is refused; motion /
      ///   selection / escape still apply (§ 2.2).
      ///
      /// Returns an [`EditOutcome`] describing what the caller must signal.
      pub fn apply(
          &mut self,
          font_system: &mut FontSystem,
          command: EditCommand,
          single_line: bool,
          read_only: bool,
      ) -> EditOutcome {
          // Value-changed is measured by comparing the buffer text across the
          // mutation (cheap: only mutating commands take this branch, and the
          // buffer is small for F-tier inputs). Motion/Escape never change it,
          // so they skip the snapshot.
          match command {
              // ── Mutations (gated on !read_only) ──────────────────────────
              EditCommand::Insert(text) => {
                  if read_only {
                      return EditOutcome::default();
                  }
                  let before = self.value();
                  // Single-line strips newlines (§ 3.3); the editor's
                  // Action::Insert self-routes '\n' to Enter, so stripping at
                  // the source is the policy.
                  for ch in text.chars() {
                      if single_line && (ch == '\n' || ch == '\r') {
                          continue;
                      }
                      self.editor.action(font_system, Action::Insert(ch));
                  }
                  EditOutcome { value_changed: self.value() != before, submitted: false }
              }
              EditCommand::Backspace => {
                  if read_only {
                      return EditOutcome::default();
                  }
                  let before = self.value();
                  self.editor.action(font_system, Action::Backspace);
                  EditOutcome { value_changed: self.value() != before, submitted: false }
              }
              EditCommand::Delete => {
                  if read_only {
                      return EditOutcome::default();
                  }
                  let before = self.value();
                  self.editor.action(font_system, Action::Delete);
                  EditOutcome { value_changed: self.value() != before, submitted: false }
              }
              EditCommand::Enter => {
                  if single_line {
                      // § 3.3: single-line Enter submits, never inserts a
                      // newline. No mutation, so read_only is irrelevant.
                      return EditOutcome { value_changed: false, submitted: true };
                  }
                  if read_only {
                      return EditOutcome::default();
                  }
                  let before = self.value();
                  self.editor.action(font_system, Action::Enter);
                  EditOutcome { value_changed: self.value() != before, submitted: false }
              }

              // ── Motion (allowed under read_only) ─────────────────────────
              EditCommand::Motion(motion, extend) => {
                  // Action::Motion does NOT manage the selection anchor
                  // (editor.rs:520-528 — the motion arm only updates the
                  // cursor). Extend ⇒ ensure an anchor at the current caret
                  // before moving; non-extend ⇒ collapse the selection first.
                  // The editor moves in VISUAL order (§ 4.1) — we never compute
                  // BiDi.
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
                  // Anchor at buffer start, active at buffer end — a single
                  // Normal selection spanning everything. Motion-only (no
                  // value change), allowed under read_only.
                  self.editor.set_selection(Selection::None);
                  self.editor.action(font_system, Action::Motion(cosmic_text::Motion::BufferStart));
                  let start = self.editor.cursor();
                  self.editor.set_selection(Selection::Normal(start));
                  self.editor.action(font_system, Action::Motion(cosmic_text::Motion::BufferEnd));
                  EditOutcome::default()
              }

              EditCommand::Escape => {
                  self.editor.action(font_system, Action::Escape);
                  EditOutcome::default()
              }

              // ── Submit (internal) ────────────────────────────────────────
              EditCommand::Submit => EditOutcome { value_changed: false, submitted: true },

              // ── E4 verbs: recognized, no behavior yet ────────────────────
              // Clipboard (§ 7) and undo (§ 8) land in E4. They must NOT fall
              // through to text insertion; routing them to a no-op here keeps
              // the keymap rows valid from E2 without faking behavior.
              EditCommand::Cut
              | EditCommand::Copy
              | EditCommand::Paste
              | EditCommand::Undo
              | EditCommand::Redo => {
                  // TODO(E4): clipboard + undo engine.
                  EditOutcome::default()
              }
          }
      }
  }
  ```

  > **`SelectAll` correctness note:** `set_selection(Normal(start))` then
  > `Action::Motion(BufferEnd)` moves the *active* cursor to the buffer end with
  > the anchor pinned at start — because the second `Motion` runs with a
  > selection present and cosmic moves the cursor while we keep the anchor. We
  > re-clear and re-anchor to avoid extending a stale selection. The E3 caret/
  > selection phase owns the richer selection model; E2's `SelectAll` is the
  > minimal correct span.

- [ ] Add the `input` module to the facade `mod.rs` (additive — full final
  state shown in Step 1.2; add the two lines):

  ```rust
  mod input;
  // ...
  pub use input::{EditOutcome, TextChanged, apply_keyboard_edits};
  ```

  > `apply_keyboard_edits` is implemented in Task 5; for THIS step it does not
  > exist yet. To keep the build green, re-export only what exists now:
  > `pub use input::{EditOutcome, TextChanged};`. Add `apply_keyboard_edits` to
  > the re-export in Task 5.

### Step 4.4 — Run green

- [ ] ```sh
  cargo test -p buiy_core --test text_editing_ops 2>&1 | tail -25
  ```

  Expected: `test result: ok. 6 passed` (insert/backspace, motion-no-change,
  single-line-submit, multi-line-newline, single-line-strip, read-only).

- [ ] **Commit:**

  ```sh
  git add -A && git commit -m "feat(text-editing): E2 task 4 — EditCommand->Action lowering + value()

TextEditState::apply lowers each command to cosmic Action inside the facade:
selection extend/collapse (Action::Motion does not anchor — editor.rs:520-528),
single-line Enter->Submit + newline strip (§ 3.3), read-only mutation gate,
and the value-changed signal. value() reads the editor buffer text (pre-IME
full buffer; E5 refines); invalidate_intrinsics() is the M1 dirty-mark's
cache-drop half. Clipboard/undo verbs recognized, no-op until E4.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 5 — `apply_keyboard_edits` — the focus-gated input system

The system reads `KeyboardInput` Messages, resolves each to an `EditCommand`
through the keymap + the letter-command lookup, applies it to the **focused,
editable** editor, and emits `TextChanged`. It runs in `BuiySet::Input` (the
`handle_tab` precedent) and locks `SharedFontSystem` **only** when a real edit
is applied.

### Step 5.1 — Failing test: a synthetic KeyboardInput types into the focused editor

- [ ] Append to `crates/buiy_core/tests/text_editing_ops.rs`:

  ```rust
  use bevy::input::ButtonState;
  use bevy::input::keyboard::{Key, KeyboardInput};
  use bevy::prelude::*;
  use buiy_core::text::edit::{ReadOnly, TextChanged};
  use buiy_core::text::{BuiyTextPlugin, Text};
  use buiy_core::{CorePlugin, FocusedEntity, Node};
  use buiy_core::layout::{LayoutPlugin, Style};

  /// Build a full headless editing app (Core + Focus + Layout + Text), with a
  /// synthetic primary window the KeyboardInput events target.
  fn editing_app() -> (App, Entity) {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins)
          .add_plugins(CorePlugin)
          // FocusPlugin owns `FocusedEntity` (CorePlugin does NOT — M2). The
          // editing system reads it via Option<Res<FocusedEntity>>, and the
          // tests set it to focus an editor, so the resource must exist.
          .add_plugins(buiy_core::focus::FocusPlugin)
          .add_plugins(LayoutPlugin)
          .add_plugins(BuiyTextPlugin::default());
      // MinimalPlugins omits bevy's InputPlugin, which is the sole owner of
      // both `add_message::<KeyboardInput>()` and `ButtonInput<KeyCode>`. The
      // editing system reads the modifier resource and the test enqueues the
      // message, so insert both directly (the headless idiom — no winit / no
      // InputPlugin needed for the resources themselves). Without the
      // add_message, `World::write_message::<KeyboardInput>` returns `None`
      // and the event is silently dropped. (FocusPlugin's `handle_tab` also
      // reads `Res<ButtonInput<KeyCode>>`, so this insert is doubly required.)
      app.add_message::<KeyboardInput>();
      app.insert_resource(ButtonInput::<KeyCode>::default());
      // A window entity so KeyboardInput.window points somewhere valid
      // (the system reads the focused editor, not the window, but events
      // carry it).
      let window = app.world_mut().spawn(()).id();
      (app, window)
  }

  /// Push a logical-character key press (text-bearing) and one frame.
  fn press_char(app: &mut App, window: Entity, ch: &str) {
      app.world_mut().write_message(KeyboardInput {
          key_code: KeyCode::KeyA, // physical code is irrelevant for text insert
          logical_key: Key::Character(ch.into()),
          state: ButtonState::Pressed,
          text: Some(ch.into()),
          repeat: false,
          window,
      });
      app.update();
  }

  /// A focused editable entity receives typed characters; an unfocused one
  /// does not. TextChanged fires once per typed char.
  #[test]
  fn typing_routes_to_the_focused_editor_only() {
      let (mut app, window) = editing_app();
      app.add_message::<TextChanged>(); // ensure reader is valid even if plugin order varies

      let editor = app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::new()),
                  buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2))))
          .id();
      let other = app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::new()),
                  buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2))))
          .id();
      app.update(); // settle spawn

      // Nothing focused ⇒ typing is dropped.
      press_char(&mut app, window, "x");
      assert_eq!(app.world().get::<buiy_core::text::edit::TextEditState>(editor).unwrap().value(), "");

      // Focus the editor ⇒ typing lands there, not on `other`.
      app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
      press_char(&mut app, window, "h");
      press_char(&mut app, window, "i");
      assert_eq!(app.world().get::<buiy_core::text::edit::TextEditState>(editor).unwrap().value(), "hi");
      assert_eq!(app.world().get::<buiy_core::text::edit::TextEditState>(other).unwrap().value(), "");
  }

  /// A focused `ReadOnly` editor ignores typed characters (mutation gate).
  #[test]
  fn read_only_focused_editor_ignores_typing() {
      let (mut app, window) = editing_app();
      let editor = app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::new()),
                  buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2)),
                  ReadOnly))
          .id();
      app.update();
      app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
      press_char(&mut app, window, "z");
      assert_eq!(app.world().get::<buiy_core::text::edit::TextEditState>(editor).unwrap().value(), "");
  }
  ```

  > `World::write_message::<M>(message)` is the verified Bevy 0.18 world method
  > to enqueue a Message (bevy_ecs-0.18.1 `src/world/mod.rs:2836`; the
  > buffered-event enqueue that replaced 0.17's `send_event`). It returns
  > `Option<MessageId>` (`None` if the message type is unregistered — but
  > `KeyboardInput` is registered by bevy's `InputPlugin`; under the
  > `MinimalPlugins` harness here it is registered because `BuiyTextPlugin` does
  > NOT add it, so the test must rely on `KeyboardInput` already being known).
  > **Verification:** `KeyboardInput` is registered via `add_message` by
  > `InputPlugin` only — `MinimalPlugins` omits `InputPlugin`, so the editing
  > tests must register it themselves: add
  > `app.add_message::<KeyboardInput>();` in `editing_app` (next to the
  > `ButtonInput<KeyCode>` insert in Step 6.2). Without it, `write_message`
  > returns `None` and the event is dropped. Add this line to `editing_app` now.

- [ ] Run — expect compile/behavior failure (the system does not exist; nothing
  consumes `KeyboardInput`):

  ```sh
  cargo test -p buiy_core --test text_editing_ops 2>&1 | tail -25
  ```

### Step 5.2 — Implement `apply_keyboard_edits`

- [ ] Append to `crates/buiy_core/src/text/edit/input.rs` (after the `apply`
  impl block):

  ```rust
  use bevy::input::keyboard::{Key, KeyboardInput};
  use bevy::input::{ButtonInput, ButtonState};

  use super::keymap::{KeyKind, Keymap, Modifiers};
  use super::state::{Disabled, ReadOnly, SingleLine};
  use crate::FocusedEntity;
  use crate::layout::LayoutTree;
  use crate::text::SharedFontSystem;

  /// Map a live logical `Key` to the table's `KeyKind`, plus the inserted text
  /// for a character key. Returns `None` for keys the editor does not bind
  /// (modifiers, function keys) — those are dropped.
  fn classify(key: &Key) -> Option<(KeyKind, Option<String>)> {
      Some(match key {
          Key::ArrowLeft => (KeyKind::ArrowLeft, None),
          Key::ArrowRight => (KeyKind::ArrowRight, None),
          Key::ArrowUp => (KeyKind::ArrowUp, None),
          Key::ArrowDown => (KeyKind::ArrowDown, None),
          Key::Home => (KeyKind::Home, None),
          Key::End => (KeyKind::End, None),
          Key::PageUp => (KeyKind::PageUp, None),
          Key::PageDown => (KeyKind::PageDown, None),
          Key::Backspace => (KeyKind::Backspace, None),
          Key::Delete => (KeyKind::Delete, None),
          Key::Enter => (KeyKind::Enter, None),
          Key::Escape => (KeyKind::Escape, None),
          Key::Space => (KeyKind::Char, Some(String::from(" "))),
          Key::Character(s) => (KeyKind::Char, Some(s.to_string())),
          _ => return None,
      })
  }

  /// Read modifier state from the physical-key input (logical `Key` modifiers
  /// are side-agnostic, but physical codes give the same booleans and are
  /// already maintained). `cmd` is `Super`.
  fn read_modifiers(keys: &ButtonInput<KeyCode>) -> Modifiers {
      Modifiers {
          ctrl: keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
          alt: keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight),
          shift: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
          cmd: keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight),
      }
  }

  /// The platform's command modifier (Ctrl on Linux/Windows, Cmd on macOS) —
  /// the one the letter-commands (A/X/C/V/Z/Y) key on. Evaluated as a const
  /// `cfg!`, not a per-event branch.
  fn command_modifier_held(mods: &Modifiers) -> bool {
      if cfg!(target_os = "macos") { mods.cmd } else { mods.ctrl }
  }

  /// The letter-command lookup the table cannot model (`KeyKind::Char` carries
  /// no letter). Resolves Ctrl/Cmd-{A,X,C,V,Z,Y} and Ctrl/Cmd-Shift-Z (redo)
  /// from the typed character (§ 3.1). Returns `None` when it is not a
  /// command — the caller then inserts the character.
  fn letter_command(text: &str, mods: &Modifiers) -> Option<EditCommand> {
      if !command_modifier_held(mods) {
          return None;
      }
      // Lowercase so Shift+letter (uppercase text) still matches.
      let ch = text.chars().next()?.to_ascii_lowercase();
      Some(match ch {
          'a' => EditCommand::SelectAll,
          'x' => EditCommand::Cut,
          'c' => EditCommand::Copy,
          'v' => EditCommand::Paste,
          'z' if mods.shift => EditCommand::Redo, // Ctrl/Cmd-Shift-Z
          'z' => EditCommand::Undo,
          'y' if !cfg!(target_os = "macos") => EditCommand::Redo, // Ctrl-Y (not macOS)
          _ => return None,
      })
  }

  /// The focus-gated editing input system (editing-and-ime §§ 3, 3.3). Runs in
  /// `BuiySet::Input` (the `handle_tab` precedent), main-thread (NonSend — it
  /// touches `LayoutTree`). Reads every `KeyboardInput` press for the focused,
  /// non-`Disabled` editor, resolves it to an `EditCommand`, applies it, and on
  /// a value-changing edit (M1): emits `TextChanged`, invalidates the editor's
  /// intrinsics cache, and Taffy-dirties the node so the **existing** measure →
  /// commit → extract path republishes next frame (`sync_one`'s pair —
  /// `sync.rs:330,335`). The `SharedFontSystem` lock is held ONLY when a
  /// command is actually applied (collect-then-lock keeps a no-key frame
  /// lock-free — architecture § 1.2 / E2 erratum 2).
  ///
  /// `focused` is `Option<Res<FocusedEntity>>` (M2): `FocusedEntity` is init by
  /// `FocusPlugin`, NOT `CorePlugin`, so a `BuiyTextPlugin` harness without
  /// `FocusPlugin` has no such resource — the system no-ops rather than
  /// panicking at param validation. `tree` is `Option<NonSendMut<LayoutTree>>`
  /// (init by `LayoutPlugin`) for the same reason, mirroring `text_sync_buffers`
  /// (`sync.rs:147`).
  #[allow(clippy::too_many_arguments)]
  pub fn apply_keyboard_edits(
      mut events: MessageReader<KeyboardInput>,
      focused: Option<Res<FocusedEntity>>,
      keymap: Res<Keymap>,
      keys: Res<ButtonInput<KeyCode>>,
      fonts: Res<SharedFontSystem>,
      mut tree: Option<NonSendMut<LayoutTree>>,
      mut editors: Query<(&mut TextEditState, Has<SingleLine>, Has<ReadOnly>), Without<Disabled>>,
      mut changed: MessageWriter<TextChanged>,
  ) {
      // No focus infrastructure (no FocusPlugin) ⇒ nothing to edit. Drain the
      // queue so events don't accumulate across frames.
      let Some(focused) = focused else {
          events.clear();
          return;
      };
      let Some(entity) = focused.0 else {
          events.clear(); // nothing focused this frame
          return;
      };
      let Ok((mut state, single_line, read_only)) = editors.get_mut(entity) else {
          events.clear(); // focus is on a non-editor / Disabled entity
          return;
      };

      let mods = read_modifiers(&keys);
      // Collect this frame's commands BEFORE locking, so the lock is held for
      // the apply burst only (and not at all when nothing resolves).
      let mut commands: Vec<EditCommand> = Vec::new();
      for ev in events.read() {
          if ev.state != ButtonState::Pressed {
              continue; // key-up never edits
          }
          let Some((kind, text)) = classify(&ev.logical_key) else {
              continue; // an unbound logical key (function keys, bare modifiers)
          };
          // Honor key-repeat for motions/deletes (§ 3); a repeated character
          // also re-inserts (the OS sends repeats with `repeat = true` and the
          // resolved text, so no special-casing is needed — we simply process
          // every Pressed event, repeats included).
          let _ = ev.repeat;

          let command = match kind {
              KeyKind::Char => {
                  let text = text.unwrap_or_default();
                  // A command modifier turns a letter into a verb; otherwise
                  // it is literal insertion.
                  letter_command(&text, &mods).unwrap_or(EditCommand::Insert(text))
              }
              // A positional key with no row for THIS modifier set is dropped
              // (m4 — drop unmapped keys, do not invent a fallback command). An
              // unbound combo like Ctrl+Backspace simply does nothing in v1;
              // the user-rebinding hook (§ 3.2) is where such bindings are added.
              other => {
                  let Some(command) = keymap.0.resolve(mods, other) else {
                      continue;
                  };
                  command
              }
          };
          commands.push(command);
      }

      if commands.is_empty() {
          return; // NO lock on an idle / non-editing frame.
      }

      // The one lock hold — the apply burst (E2 erratum 2). Mirrors TextCommit.
      let mut font_system = fonts.lock();
      let mut any_value_change = false;
      for command in commands {
          let outcome = state.apply(&mut font_system, command, single_line, read_only);
          any_value_change |= outcome.value_changed;
          // `outcome.submitted` is consumed internally in E2 (the host-facing
          // EditSubmitted is E6). A submit does not change the value.
      }
      drop(font_system);

      // M1 — the dirty-mark seam. A value-changing edit reshaped the editor's
      // OWNED buffer, but the `Text` component is unchanged, so NONE of
      // TextSyncTriggers fire and `sync_one` (the only `mark_dirty_for_entity`
      // caller) never runs for this entity. Do here exactly what `sync_one`
      // does after a Text change: drop the intrinsics cache and Taffy-dirty the
      // node, so next frame's measure → TextCommit → extract republish (N→N+1).
      // Pure motion (any_value_change == false) reshapes nothing — no dirty.
      if any_value_change {
          state.invalidate_intrinsics();
          if let Some(tree) = tree.as_deref_mut() {
              // Absent tree (standalone BuiyTextPlugin, no LayoutPlugin):
              // nothing measures, nothing to dirty — `sync_one`'s same guard.
              tree.mark_dirty_for_entity(entity);
          }
          changed.write(TextChanged(entity));
      }
  }
  ```

  > **Why collect-then-lock:** the lock is acquired only inside `if
  > !commands.is_empty()`, so a frame with no relevant keypress never touches
  > `SharedFontSystem` — preserving the "no fourth steady-state lock site"
  > invariant.
  >
  > **Why drop unmapped keys (m4):** a positional key with no row for its
  > modifier set is dropped (`continue`), not forced to a fallback command. An
  > earlier draft routed it to `Escape`, which would clear the selection on a
  > stray Ctrl+Backspace — surprising. Dropping is the least-astonishing v1
  > behavior; the rebinding hook (§ 3.2) is where users add the binding.
  >
  > **Borrow note:** `state.invalidate_intrinsics()` and the `tree` write both
  > happen AFTER `drop(font_system)` and outside the `editors` borrow conflict —
  > `state` is the `Mut<TextEditState>` from the query (still in scope), `tree`
  > is a distinct resource. No aliasing.

- [ ] Update the facade re-export to include the system. Edit
  `crates/buiy_core/src/text/edit/mod.rs`:

  ```rust
  pub use input::{EditOutcome, TextChanged, apply_keyboard_edits};
  ```

### Step 5.3 — Register the system + Keymap resource + TextChanged message

- [ ] Edit `crates/buiy_core/src/text/mod.rs`. Add the re-export (in the
  `pub use edit::{...}` line):

  ```rust
  pub use edit::{
      Disabled, EditCommand, Keymap, Placeholder, ReadOnly, SingleLine, TextBufferAccess,
      TextChanged, TextEditState, apply_keyboard_edits,
  };
  ```

- [ ] In `BuiyTextPlugin::build`, after the `write_caret_blink` registration
  block (before the system-font-scan block), add:

  ```rust
          // E2 (editing-and-ime §§ 3, 11): the per-platform keymap (selected
          // once at init by a data swap) and the focus-gated input system.
          // Runs in BuiySet::Input — the `handle_tab` precedent (focus.rs:56),
          // two sets after Layout, so an edit publishes N→N+1 (OQ#1: accepted
          // one-frame latency). The TextChanged Message is registered so
          // consumers (the a11y layer, the widget catalog) can subscribe.
          app.init_resource::<crate::text::edit::Keymap>();
          app.add_message::<crate::text::edit::TextChanged>();
          app.add_systems(
              Update,
              crate::text::edit::apply_keyboard_edits.in_set(crate::BuiySet::Input),
          );
  ```

  > **M2 — `FocusedEntity` is NOT a `CorePlugin` resource.** It is init ONLY by
  > `FocusPlugin::build` (`focus.rs:54`); `CorePlugin` does not add `FocusPlugin`
  > nor `init_resource::<FocusedEntity>()`. The existing focus/a11y tests add
  > `FocusPlugin` explicitly. So if `apply_keyboard_edits` took
  > `Res<FocusedEntity>`, registering it in `BuiyTextPlugin::build` would panic
  > at system-param validation on **every** existing `BuiyTextPlugin` harness
  > that lacks `FocusPlugin` — `text_engine.rs`, `text_sync.rs`,
  > `text_measure.rs`, `text_typing_latency.rs`, `TextExtractHarness`
  > (`support/extract_harness.rs`), and more — turning the whole headless gate
  > red. The system therefore takes **`Option<Res<FocusedEntity>>`** and no-ops
  > when the resource is absent (the text plugin does not own a focus resource —
  > least coupling; the runner-up of `BuiyTextPlugin` defensively
  > `init_resource::<FocusedEntity>()` is rejected because it would silently
  > coexist-with / shadow `FocusPlugin`'s and blur ownership). The editing tests
  > that DRIVE the system establish focus themselves (set `FocusedEntity` after
  > adding `CorePlugin` — `CorePlugin` does not provide it, so the test inserts
  > or the harness gains `FocusPlugin`; see `editing_app`). `BuiySet::Input` is
  > configured (chained after Layout/Style) by `CorePlugin`; standalone
  > `BuiyTextPlugin` apps run the system unordered but inert (no focus, no tree).

### Step 5.4 — Run green

- [ ] ```sh
  cargo test -p buiy_core --test text_editing_ops 2>&1 | tail -25
  ```

  Expected: all 8 tests pass (6 `apply`-direct + `typing_routes_to_the_focused_editor_only`
  + `read_only_focused_editor_ignores_typing`).

- [ ] **Commit:**

  ```sh
  git add -A && git commit -m "feat(text-editing): E2 task 5 — focus-gated apply_keyboard_edits

The BuiySet::Input system reads KeyboardInput Messages, resolves each via the
keymap + the letter-command lookup (Ctrl/Cmd-{A,X,C,V,Z,Y}, system-resolved
from the typed letter), applies to the focused non-Disabled editor, and on a
value-changing edit emits TextChanged AND dirty-marks the node into the
existing measure pipeline (invalidate_intrinsics + mark_dirty_for_entity —
sync_one's pair; M1), so the editor edit publishes N->N+1. Focus is gated via
Option<Res<FocusedEntity>> and the tree via Option<NonSendMut<LayoutTree>> so
the system no-ops (never panics) in harnesses without FocusPlugin/LayoutPlugin
(M2). Unmapped positional keys are dropped (m4). Locks SharedFontSystem only on
a real-edit frame (collect-then-lock — E2 erratum 2).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 6 — Letter-command + repeat coverage through the system

The table tests (Task 3) deliberately do not cover the letter-commands
(SelectAll/Cut/Copy/Paste/Undo/Redo) because those are system-resolved. This
task proves them end-to-end through `apply_keyboard_edits`, plus `repeat`
semantics and the `value()`/`TextChanged` round-trip.

### Step 6.1 — Failing test: Ctrl-A selects all; Enter on single-line emits nothing destructive; repeat re-applies

- [ ] Append to `crates/buiy_core/tests/text_editing_ops.rs`:

  ```rust
  use bevy::input::keyboard::Key as LogicalKey;

  /// Drive a key WITH a modifier physical key held. Presses the modifier code
  /// (so `ButtonInput<KeyCode>` reports it), then the logical key, in one
  /// frame.
  fn press_with_ctrl(app: &mut App, window: Entity, logical: LogicalKey, text: Option<&str>) {
      // Hold Ctrl at the physical layer for this frame.
      app.world_mut()
          .resource_mut::<ButtonInput<KeyCode>>()
          .press(KeyCode::ControlLeft);
      app.world_mut().write_message(KeyboardInput {
          key_code: KeyCode::KeyA,
          logical_key: logical,
          state: ButtonState::Pressed,
          text: text.map(Into::into),
          repeat: false,
          window,
      });
      app.update();
      app.world_mut()
          .resource_mut::<ButtonInput<KeyCode>>()
          .release(KeyCode::ControlLeft);
  }

  /// Ctrl-A on the focused editor selects the whole buffer; a subsequent
  /// non-extending typed char REPLACES the selection (cosmic deletes the
  /// selection before inserting). This proves the letter-command lookup AND
  /// that the selection is live.
  #[test]
  fn ctrl_a_selects_all_then_typing_replaces() {
      let (mut app, window) = editing_app();
      let editor = app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::new()),
                  buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2))))
          .id();
      app.update();
      app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

      press_char(&mut app, window, "h");
      press_char(&mut app, window, "i");
      assert_eq!(value(&app, editor), "hi");

      // Ctrl-A (logical 'a', text "a", Ctrl held) ⇒ SelectAll.
      press_with_ctrl(&mut app, window, LogicalKey::Character("a".into()), Some("a"));
      // Typing now replaces the whole selection.
      press_char(&mut app, window, "X");
      assert_eq!(value(&app, editor), "X", "Ctrl-A selected all; typing replaced it");
  }

  /// A repeated keypress (`repeat = true`) re-applies — two delete-repeats
  /// remove two graphemes.
  #[test]
  fn key_repeat_reapplies_the_command() {
      let (mut app, window) = editing_app();
      let editor = app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::new()),
                  buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2))))
          .id();
      app.update();
      app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
      for c in ["a", "b", "c"] { press_char(&mut app, window, c); }
      assert_eq!(value(&app, editor), "abc");

      // Two Backspace events in ONE frame, the second flagged repeat — both
      // apply (the system processes every Pressed event, repeats included).
      app.world_mut().write_message(KeyboardInput {
          key_code: KeyCode::Backspace, logical_key: LogicalKey::Backspace,
          state: ButtonState::Pressed, text: None, repeat: false, window,
      });
      app.world_mut().write_message(KeyboardInput {
          key_code: KeyCode::Backspace, logical_key: LogicalKey::Backspace,
          state: ButtonState::Pressed, text: None, repeat: true, window,
      });
      app.update();
      assert_eq!(value(&app, editor), "a", "two backspaces (one a repeat) removed two chars");
  }

  /// `TextChanged` fires once per value-changing frame, and not for a pure
  /// motion.
  #[test]
  fn text_changed_fires_on_value_change_only() {
      let (mut app, window) = editing_app();
      // A collector system that counts TextChanged per frame.
      #[derive(Resource, Default)]
      struct Seen(usize);
      app.init_resource::<Seen>();
      app.add_systems(Update, |mut r: MessageReader<TextChanged>, mut s: ResMut<Seen>| {
          s.0 += r.read().count();
      });

      let editor = app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::new()),
                  buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2))))
          .id();
      app.update();
      app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

      let before = app.world().resource::<Seen>().0;
      press_char(&mut app, window, "q"); // value change ⇒ one TextChanged
      let after_type = app.world().resource::<Seen>().0;
      assert_eq!(after_type, before + 1, "typing emits one TextChanged");

      // A bare ArrowLeft (motion) ⇒ no TextChanged.
      app.world_mut().write_message(KeyboardInput {
          key_code: KeyCode::ArrowLeft, logical_key: LogicalKey::ArrowLeft,
          state: ButtonState::Pressed, text: None, repeat: false, window,
      });
      app.update();
      assert_eq!(app.world().resource::<Seen>().0, after_type, "motion emits no TextChanged");
  }

  /// Small reader helper.
  fn value(app: &App, e: Entity) -> String {
      app.world().get::<buiy_core::text::edit::TextEditState>(e).unwrap().value()
  }
  ```

- [ ] Run — expect FAIL until the system handles the modifier-letter lookup and
  repeat correctly (the system is already implemented in Task 5; these tests
  PROVE the existing implementation — if Task 5 is right, they pass immediately;
  if a gap exists, they fail and you fix the system, not the test):

  ```sh
  cargo test -p buiy_core --test text_editing_ops 2>&1 | tail -25
  ```

  > These tests are written RED-first conceptually, but Task 5's system was
  > built to satisfy them. If they pass on first run, that is expected (the
  > system was designed against this surface). If any fail, the bug is in the
  > Task 5 system — root-cause it there. Do NOT weaken the test.

### Step 6.2 — Fix any gaps, run green

- [ ] The `editing_app` builder (Step 5.1) already inserts
  `ButtonInput<KeyCode>` and registers `KeyboardInput`, so `press_with_ctrl`
  (which presses `KeyCode::ControlLeft` on that resource) and the synthetic
  `write_message(KeyboardInput { … })` both work. If
  `ctrl_a_selects_all_then_typing_replaces` fails, the bug is in the Task 5
  system's `letter_command` / modifier-read path (root-cause it there — do NOT
  weaken the test). A common cause: `read_modifiers` reading the wrong resource,
  or `command_modifier_held` testing `cmd` on Linux — verify it tests `ctrl`
  under `cfg!(not(target_os = "macos"))`.

- [ ] Re-run:

  ```sh
  cargo test -p buiy_core --test text_editing_ops 2>&1 | tail -25
  ```

  Expected: all tests pass (10 total in the file now).

- [ ] **Commit:**

  ```sh
  git add -A && git commit -m "test(text-editing): E2 task 6 — letter-commands, repeat, TextChanged

End-to-end coverage through apply_keyboard_edits: Ctrl-A selects all (typing
replaces the selection), key-repeat re-applies, and TextChanged fires once per
value-changing frame (never for motion). editing_app inserts ButtonInput
<KeyCode> (MinimalPlugins omits InputPlugin).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 7 — `SingleLine` ⇒ `Wrap::None` in the sync seam (§ 3.3)

§ 3.3: a single-line editor's buffer is configured `Wrap::None` so the layout
never wraps it. The wrap is resolved in `text_sync_buffers` at the
`buffer.set_wrap(resolve_wrap(...))` call. E2 threads the `SingleLine` marker
into it.

### Step 7.1 — Failing test: a single-line editor buffer never wraps

- [ ] Append to `crates/buiy_core/tests/text_editing_ops.rs`:

  ```rust
  use buiy_core::text::edit::SingleLine;

  /// A `SingleLine` editor lays its content on ONE visual line even when the
  /// content exceeds the node width — `Wrap::None` (§ 3.3). A multi-line
  /// editor with the same long content wraps to >1 line.
  #[test]
  fn single_line_editor_buffer_does_not_wrap() {
      let mut single = editing_app().0;
      let mut multi = editing_app().0;

      let long = "wrapping word wrapping word wrapping word wrapping word";
      let make = |app: &mut App, single_line: bool| -> Entity {
          let mut e = app.world_mut().spawn((
              Node, Style::default(),
              Text(String::from(long)),
              buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2)),
          ));
          if single_line { e.insert(SingleLine); }
          let id = e.id();
          // A narrow sized parent forces wrapping for the multi-line case.
          app.world_mut().spawn((
              Node, Style::default().flex_column().width_px(80.0).height_px(200.0),
          )).add_child(id);
          id
      };
      let s = make(&mut single, true);
      let m = make(&mut multi, false);
      single.update(); single.update();
      multi.update(); multi.update();

      let line_count = |app: &App, e: Entity| -> usize {
          app.world().get::<buiy_core::text::edit::TextEditState>(e).unwrap()
              .with_buffer(|b| b.layout_runs().count())
      };
      assert_eq!(line_count(&single, s), 1, "single-line editor never wraps (Wrap::None)");
      assert!(line_count(&multi, m) > 1, "multi-line editor wraps the long content");
  }
  ```

- [ ] Run — expect FAIL (`line_count(single) == 1` fails: without the hook a
  single-line editor wraps like any other):

  ```sh
  cargo test -p buiy_core --test text_editing_ops 2>&1 | grep -A3 single_line_editor
  ```

### Step 7.2 — Thread `SingleLine` into the sync wrap resolution

The `set_wrap(resolve_wrap(...))` call (line 503) lives **inside
`apply_authored_to_buffer`** (defined line 488), which has **TWO call sites**
(verified): line 221 (the `unsynced` / `Added<TextBuffer>` insert path — a
brand-new display buffer, provably WITHOUT an editor, because that query is
`Without<TextBuffer>` and never binds `TextEditState`) and line 326 (the
`sync_one` accessor path — where an editor entity's single-line marker
matters). **Threading a `single_line` param therefore forces updating BOTH call
sites** (a surprise compile error otherwise). Line 221 passes `false` (no editor
there); the meaningful value flows at line 326.

- [ ] **Step A — add the param to `apply_authored_to_buffer`.** Change its
  signature (line 488) to accept `single_line: bool` and apply the override at
  the `set_wrap` call (line 503):

  ```rust
  fn apply_authored_to_buffer(
      buffer: &mut Buffer,
      text: &Text,
      style: &AuthoredStyle<'_>,
      registry: &FontRegistry,
      index: &mut FontMatchIndex,
      now: f64,
      single_line: bool,
  ) -> bool {
      // … existing body up to the set_wrap line …

      // § 3.3: a SingleLine editor never wraps, regardless of white-space /
      // text-wrap. The marker wins over the resolved wrap.
      let wrap = if single_line {
          cosmic_text::Wrap::None
      } else {
          resolve_wrap(style.white_space, style.text_wrap)
      };
      buffer.set_wrap(wrap);
      // … rest of the body unchanged …
  }
  ```

- [ ] **Step B — update the two call sites.**
  - **Line 221 (the insert path)** passes `false` — that path builds a display
    `TextBuffer` for an entity that has no editor (the `unsynced` query is
    `Without<TextBuffer>` and never binds `TextEditState`), so single-line wrap
    is moot there. An editor entity that ALSO carries `SingleLine` re-syncs next
    frame through the `Added<TextBuffer>` arm → `sync_one` (line 326), which
    passes the real value:

    ```rust
        let blocked = apply_authored_to_buffer(
            &mut buffer.buffer,
            text,
            &style,
            ctx.registry,
            ctx.index,
            ctx.now,
            false, // insert path: no editor here, single-line wrap is moot
        );
    ```

  - **Line 326 (`sync_one` accessor path)** passes the queried marker. Add
    `Has<super::edit::SingleLine>` as the LAST member of both the `SyncedText`
    and `SyncedTextItem` type aliases (Bevy 0.18 `Has<T>` is a `bool`-valued
    read-only `QueryData` item — verified `bevy_ecs query/fetch.rs:2583`; no
    `&'static`/lifetime needed), destructure it in `sync_one`'s `item` binding
    (line ~306, the tuple ending `…, pending) = item;` becomes `…, pending,
    single_line) = item;`), and pass it:

    ```rust
        apply_authored_to_buffer(
            buffer, text, &style, ctx.registry, ctx.index, ctx.now, single_line,
        )
    ```

    > `Has<SingleLine>` is added to the QUERY shape (`SyncedText`), so it binds
    > the marker without filtering — display-only and multi-line editors get
    > `false` and are unaffected.

  > The `SyncedText` union does NOT need a `Changed<SingleLine>` trigger member:
  > toggling single-line at runtime is rare and a value edit re-syncs anyway.
  > Add `Changed<SingleLine>` to `TextSyncTriggers` ONLY if a test shows a stale
  > wrap after a runtime marker toggle (YAGNI — not in scope; note the decision
  > in the commit body).

  > Facade note: `sync.rs` is OUTSIDE the facade. It already imports
  > `super::edit::{TextBufferAccess, TextBufferAccessItem}` (line 39); naming
  > `cosmic_text::Wrap` is fine (Wrap is not a forbidden type) and so is
  > `super::edit::SingleLine` (a marker, not an editor type). Do NOT name
  > `Editor`/`Edit`/`Action`/`Change` here.

### Step 7.3 — Run green + facade re-check

- [ ] ```sh
  cargo test -p buiy_core --test text_editing_ops --test text_facade_boundary 2>&1 | tail -15
  ```

  Expected: `single_line_editor_buffer_does_not_wrap` passes; the facade
  boundary test still passes (no forbidden type leaked into `sync.rs`).

- [ ] **Commit:**

  ```sh
  git add -A && git commit -m "feat(text-editing): E2 task 7 — SingleLine ⇒ Wrap::None in sync (§ 3.3)

Thread the SingleLine marker into text_sync_buffers' wrap resolution: a
single-line editor buffer is Wrap::None regardless of white-space/text-wrap,
so it lays on one visual line. The marker is a plain query member (no
forbidden cosmic type in sync.rs — facade boundary holds).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 8 — The Input-driven N→N+1 latency fixture (OQ#1 gate)

This is E2's distinctive deliverable: the editor-input latency fixture the
readiness report requires (the T8 `text_typing_latency` fixture proves only the
sync-side path and **must not** be cited as editor-path proof). An edit applied
in `BuiySet::Input` (via a synthetic `KeyboardInput`) publishes glyphs **one
frame later** (N → N+1), because Input runs two sets after Layout.

### Step 8.1 — Failing test: edit in Input publishes glyphs at N+1

- [ ] Create `crates/buiy_core/tests/text_input_latency.rs`:

  ```rust
  //! E2 — the EDITOR-INPUT latency gate (OQ#1, editing-and-ime § 12 /
  //! readiness report). A keystroke applied in `BuiySet::Input` (the editor
  //! path) reaches a freshly-published `ExtractedGlyphs` in exactly ONE more
  //! frame (N → N+1), because Input runs two sets AFTER Layout, so the edit's
  //! reshape is picked up by NEXT frame's TextSync → measure → TextCommit →
  //! extract. This is DISTINCT from T8's `text_typing_latency` fixture, which
  //! mutates `Text` BEFORE Layout (the sync-side path) — that fixture must not
  //! be cited as editor-path proof (readiness § gate caveat).
  //!
  //! Headless on the adapterless extract harness; the edit is driven through
  //! the real `apply_keyboard_edits` system (a synthetic `KeyboardInput` +
  //! `FocusedEntity`), so the WHOLE editor pipeline is exercised, not a `Text`
  //! poke.

  mod support;

  use bevy::input::ButtonState;
  use bevy::input::keyboard::{Key, KeyboardInput};
  use bevy::prelude::*;
  use buiy_core::text::edit::TextEditState;
  use buiy_core::text::Text;
  use buiy_core::{FocusedEntity, Node};
  use buiy_core::layout::Style;
  use cosmic_text::Metrics;
  use support::extract_harness::TextExtractHarness;

  /// Spawn a focused editable "Hi" under a sized column root — two glyphs, so
  /// one typed char appends exactly one new instance (the T8 fixture shape,
  /// but EDITABLE and focused).
  fn spawn_focused_editor(h: &mut TextExtractHarness) -> Entity {
      let editor = h
          .app
          .world_mut()
          .spawn((
              Node,
              Style::default(),
              Text(String::from("Hi")),
              TextEditState::new(Metrics::new(16.0, 19.2)),
          ))
          .id();
      h.app
          .world_mut()
          .spawn((Node, Style::default().flex_column().width_px(300.0).height_px(100.0)))
          .add_child(editor);
      // The harness (CorePlugin + LayoutPlugin + BuiyTextPlugin) provides NO
      // FocusedEntity (FocusPlugin owns it — M2) and no InputPlugin. Add
      // FocusPlugin so `FocusedEntity` exists (and `handle_tab` is harmless —
      // no Tab is sent here), register `KeyboardInput` so `write_message` is
      // not dropped, and insert `ButtonInput<KeyCode>` for the modifier read /
      // `handle_tab`. Then focus the editor so `apply_keyboard_edits` targets
      // it. Plugins/resources may be added before the first update; the
      // harness has not updated yet (settle() runs after this).
      h.app.add_plugins(buiy_core::focus::FocusPlugin);
      h.app.add_message::<KeyboardInput>();
      h.app.world_mut().insert_resource(ButtonInput::<KeyCode>::default());
      h.app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
      editor
  }

  #[test]
  fn one_frame_from_input_edit_to_glyph_publish() {
      let mut h = TextExtractHarness::new();
      let editor = spawn_focused_editor(&mut h);
      h.settle();

      let count0 = h.glyph_count();
      let publishes0 = h.changed_frames();
      let window = h.app.world_mut().spawn(()).id();

      // THE keystroke: a synthetic KeyboardInput '!' enqueued so that the
      // edit is applied by apply_keyboard_edits in BuiySet::Input THIS frame
      // (frame N) — AFTER Layout already ran. The reshape is therefore picked
      // up by frame N+1's TextSync.
      h.app.world_mut().write_message(KeyboardInput {
          key_code: KeyCode::Digit1,
          logical_key: Key::Character("!".into()),
          state: ButtonState::Pressed,
          text: Some("!".into()),
          repeat: false,
          window,
      });

      // Frame N: Update applies the edit in Input (post-Layout) — so this
      // frame's extract sees the OLD glyph set (the edit missed this frame's
      // TextSync/measure/commit).
      h.frame();
      // The edit DID land in the editor buffer this frame (apply_keyboard_edits
      // ran in Input) — this is the M1 proof half: the buffer changed, the
      // node was dirty-marked, but the glyphs have NOT flowed yet.
      assert_eq!(
          h.app.world().get::<TextEditState>(editor).unwrap().value(),
          "Hi!",
          "the edit applied to the editor buffer on frame N (in BuiySet::Input)"
      );
      assert_eq!(
          h.changed_frames(),
          publishes0,
          "frame N (edit applied post-Layout) does NOT republish — the edit \
           missed this frame's TextSync"
      );
      assert_eq!(h.glyph_count(), count0, "frame N still shows the pre-edit glyphs");

      // Frame N+1: the node was Taffy-dirtied by apply_keyboard_edits (M1), so
      // even though TextSyncTriggers do NOT fire (Text is unchanged), this
      // frame's measure → TextCommit → extract reshape and publish the new
      // glyph. WITHOUT the dirty-mark this assertion fails (the cache holds and
      // nothing republishes) — it is the M1 regression guard.
      h.frame();
      assert_eq!(
          h.changed_frames(),
          publishes0 + 1,
          "frame N+1 publishes the edit (one-frame editor-input latency — OQ#1; \
           proves the M1 dirty-mark entered the measure path)"
      );
      assert_eq!(
          h.glyph_count(),
          count0 + 1,
          "the '!' glyph is in the N+1 published set"
      );
  }
  ```

- [ ] Run — expect FAIL initially **only if** the harness's plugin set does not
  include the editing system. The `TextExtractHarness` builds with
  `CorePlugin + LayoutPlugin + BuiyTextPlugin`, and E2 registered
  `apply_keyboard_edits` in `BuiyTextPlugin`, so the system IS present. The test
  should pass once the system is wired. Run it:

  ```sh
  cargo test -p buiy_core --test text_input_latency 2>&1 | tail -25
  ```

### Step 8.2 — Diagnose the two failure modes

- [ ] **If the frame-N republish assertion fails (republished too early):** the
  edit reached TextSync the SAME frame — meaning the system runs BEFORE Layout,
  contradicting OQ#1. Verify `apply_keyboard_edits` is registered
  `.in_set(crate::BuiySet::Input)` (after Layout) in `text/mod.rs`. The expected
  pass proves the OQ#1 ordering empirically. (The permanent `value() == "Hi!"`
  assertion after frame N is the witness: the edit applied in Input on frame N,
  while the glyphs still show `count0`.)

- [ ] **If the frame-N+1 publish assertion fails (never republishes):** this is
  the M1 failure — the edit landed in the buffer (`value()` is `"Hi!"`) but the
  node was NOT dirty-marked, so Taffy served a cached measurement and
  `text_commit` short-circuited. Confirm `apply_keyboard_edits` calls
  `state.invalidate_intrinsics()` + `tree.mark_dirty_for_entity(entity)` on the
  `any_value_change` branch, and that `tree` is actually `Some` (the harness has
  `LayoutPlugin`, so `LayoutTree` exists). Without both, the edit dead-ends —
  exactly the bug this gate exists to catch.

- [ ] **If `write_message` seems dropped** (both glyph counts stay at baseline
  AND `value()` stays `"Hi"`): the `KeyboardInput` message was not registered
  (returns `None`) or `FocusedEntity` was not set. Confirm `spawn_focused_editor`
  added `FocusPlugin`, `add_message::<KeyboardInput>()`, and set `FocusedEntity`.

### Step 8.3 — Run green

- [ ] ```sh
  cargo test -p buiy_core --test text_input_latency 2>&1 | tail -15
  ```

  Expected: `test result: ok. 1 passed`.

- [ ] **Commit:**

  ```sh
  git add -A && git commit -m "test(text-editing): E2 task 8 — Input-driven N→N+1 latency gate (OQ#1)

The editor-path latency fixture the readiness report requires: a synthetic
KeyboardInput applied by apply_keyboard_edits in BuiySet::Input publishes
glyphs ONE frame later (N→N+1), because Input runs two sets after Layout.
Distinct from T8's sync-side fixture (which must not be cited as editor-path
proof). Drives the WHOLE editor pipeline headless, no adapter.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 9 — Full gate + facade re-check + plan self-review

### Step 9.0 — M2 regression sweep: the FULL `buiy_core` suite, not just new binaries

Registering `apply_keyboard_edits` in `BuiyTextPlugin::build` makes it run in
**every** `BuiyTextPlugin` harness. The system params must be tolerant
(`Option<Res<FocusedEntity>>`, `Option<NonSendMut<LayoutTree>>`) so a harness
WITHOUT `FocusPlugin` / `LayoutPlugin` does not panic at param validation. The
only way to prove no existing harness regressed is to run the WHOLE crate suite,
not just the three new binaries.

- [ ] Run the full `buiy_core` suite and confirm the pre-existing
  `BuiyTextPlugin`-only harnesses still pass (these have NO `FocusPlugin`):

  ```sh
  cargo test -p buiy_core --test text_engine --test text_sync --test text_measure \
    --test text_typing_latency 2>&1 | tail -20
  ```

  Expected: all pass. A panic here ("Resource ... FocusedEntity ... validation"
  or "NonSend ... LayoutTree") means a param is `Res`/`NonSendMut` instead of
  `Option<…>` — fix the system signature, not the harness. (The
  `TextExtractHarness`-based `text_typing_latency` is the canary: it has
  `LayoutPlugin` but NO `FocusPlugin`, so it exercises the
  `Option<Res<FocusedEntity>>` None-arm.)

### Step 9.1 — Run the headless gate

- [ ] Format, lint, doc, and the full headless test suite:

  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    cargo test -p buiy_core 2>&1 | tail -30
  ```

  Expected: clean fmt, zero clippy warnings, doc builds, all `buiy_core` tests
  pass (including the new `text_keymap`, `text_editing_ops`, `text_input_latency`
  binaries AND the unchanged E1 `text_edit_substrate` / `text_facade_boundary`),
  AND every pre-existing `BuiyTextPlugin` harness (the M2 regression surface).

- [ ] Run the full workspace suite (xvfb where needed):

  ```sh
  xvfb-run -a cargo test --workspace 2>&1 | tail -20
  ```

  Expected: green. (E2 adds no GPU `#[ignore]` tests — all of E2 is headless;
  the GPU lane is unchanged from E1.)

### Step 9.2 — Facade boundary audit (the per-phase reviewer check)

- [ ] Confirm no `Editor`/`Edit`/`Action`/`Change` leaked outside `text/edit/`:

  ```sh
  cargo test -p buiy_core --test text_facade_boundary
  git diff main --name-only | grep -v '^crates/buiy_core/src/text/edit/' | grep -v '^crates/buiy_core/tests/' | grep '\.rs$'
  # For each non-facade .rs in that list, eyeball it for the forbidden types:
  git diff main -- crates/buiy_core/src/text/sync.rs crates/buiy_core/src/text/mod.rs | grep -nE '\b(Editor|Action|Change)\b|cosmic_text::Edit\b' || echo "clean"
  ```

  Expected: the boundary test passes; the only non-facade modified files are
  `sync.rs` (names `Wrap`/`SingleLine` — both allowed) and `mod.rs` (names
  `apply_keyboard_edits`/`Keymap`/`TextChanged`/`EditCommand` — none forbidden).
  The grep prints `clean`.

### Step 9.3 — Spec/campaign self-review

Walk the E2 deliverable list (campaign § E2 + spec §§ 3/3.1/3.2/3.3/11) and
confirm coverage:

- [ ] **`EditCommand` enum (spec § 3 shape)** — `command.rs`. ✓ All verbs:
  `Motion(Motion, bool)`, `Insert(String)`, `Backspace`, `Delete`, `Enter`,
  `Cut`/`Copy`/`Paste`, `Undo`/`Redo`, `SelectAll`, `Escape`, `Submit`.
  Deviation: `Insert(String)` not `Insert(SmolStr)` (erratum 1, justified).
- [ ] **Data-driven per-platform keymap, data swap (§§ 3.1, 3.2)** —
  `keymap.rs` `KeymapTable` + `default_keymap_for_platform()` (`cfg!` once at
  init). Both platform tables encode the NORMATIVE § 3.1 rows; tested per
  platform. Letter-commands system-resolved (justified — `KeyKind::Char` carries
  no letter). ✓
- [ ] **`KeyboardInput` → `EditCommand` → `Action` lowering in `BuiySet::Input`,
  focus-gated (§ 3)** — `apply_keyboard_edits` + `TextEditState::apply`. ✓
  Focus-gated on `FocusedEntity` → non-`Disabled` `TextEditState`; `ReadOnly`
  blocks mutation, allows motion.
- [ ] **Character insertion via layout-resolved `text` (§ 3)** — `classify`
  pulls `Key::Character(s)`/`Key::Space`; iterated as chars into
  `Action::Insert`. ✓
- [ ] **`repeat` honored (§ 3)** — every `Pressed` event processed, repeats
  included; tested. ✓
- [ ] **Grapheme-correct Backspace/Delete (inherited, § 3.1)** — routed to
  `Action::Backspace`/`Delete`; cosmic owns grapheme correctness.
  > **Coverage gap flagged:** the plan does NOT add an explicit emoji-ZWJ /
  > combining-mark delete fixture (campaign § E2 test surface names it). The
  > inherited correctness is real (cosmic-text's job), but the campaign asks for
  > the fixture. **Add a `grapheme_delete` test** in `text_editing_ops.rs`:
  > insert `"a👨‍👩‍👧b"` (a ZWJ family emoji), backspace once, assert the whole
  > cluster is gone (`value() == "a"` if the cluster was before `b` and caret at
  > end after deleting `b`… design the fixture so one Backspace removes one
  > grapheme cluster and assert the byte-length drop equals the cluster, not one
  > `char`). This was omitted from the task body above; the executor MUST add it
  > as Step 6.3 before the gate. It needs the real `FontSystem` (shaping the
  > emoji), already available via `SharedFontSystem::new()`.
- [ ] **`SingleLine` policy (§ 3.3)** — Enter ⇒ `Submit` (never `Action::Enter`);
  newline-stripped insert; `Wrap::None`. ✓ (Tasks 4 + 7.)
- [ ] **`value()` (born here)** — `state.rs`; reads the editor buffer text
  (pre-IME full buffer; E5 refines). ✓
- [ ] **`TextChanged` Message (§ 11, born here)** — `input.rs`; emitted once per
  value-changing frame, never for motion; registered in the plugin. ✓
- [ ] **OQ#1 one-frame path realized** — the latency fixture proves N→N+1; no
  new Taffy compute PASS is added (Input still runs after Layout, scheduling is
  unchanged). The one piece of machinery the editor edit DOES need is the **M1
  dirty-mark** (`invalidate_intrinsics` + `mark_dirty_for_entity` from
  `apply_keyboard_edits`), because an editor-buffer edit trips none of the
  `TextSyncTriggers` and so must dirty-mark to ENTER the existing measure path —
  the same gesture `sync_one` makes for a `Text` edit. This is a seam into the
  existing path, not a new pass; OQ#1's "one-frame latency, no extra Taffy site"
  holds. ✓
- [ ] **Internal `EditCommand::Submit` (E2 owns)** — `EditOutcome.submitted`;
  host-facing `EditSubmitted` deferred to E6 (correct — not half-built). ✓
- [ ] **Lock discipline** — the lock fires only on a real-edit frame
  (collect-then-lock); erratum 2 records the input shaping site. ✓

- [ ] **Type-consistency sweep:** `cosmic_text::Motion` is the one cosmic type
  `EditCommand` names — confirm it appears in `command.rs` and `keymap.rs` only
  (both facade-internal). `Action` appears in `input.rs` only. `FontSystem`
  appears in `input.rs` (lock) only. Grep:

  ```sh
  grep -rln 'cosmic_text::Action\|::action(' crates/buiy_core/src/text/edit/
  # expect ONLY input.rs
  ```

- [ ] **No placeholders sweep:** every code block in this plan is complete Rust
  (no `// ...`, no "similar to above", no `todo!()` in shipped paths — the only
  `TODO(E4)` is a documented no-op for the clipboard/undo verbs, which is the
  correct E2 state, not a stub). Confirm the shipped crate has no `todo!()`:

  ```sh
  grep -rn 'todo!\|unimplemented!' crates/buiy_core/src/text/edit/ || echo "no stubs"
  ```

### Step 9.4 — Update docs index

- [ ] Add the plan to `docs/README.md` under the text-editing campaign plans
  (mirror the E1 line if one exists; otherwise add under the campaign plan
  entry):

  ```
  - [Buiy text-editing E2 — input translation](plans/2026-06-13-buiy-text-editing-e2-input.md) — the keymap table (data-driven, per-platform), KeyboardInput→EditCommand→Action lowering in BuiySet::Input (focus-gated), SingleLine policy, value()/TextChanged, and the Input-driven N→N+1 latency gate (OQ#1). Realizes editing-and-ime §§ 3, 3.1–3.3, 11. `[active]`
  ```

- [ ] **Final commit:**

  ```sh
  git add -A && git commit -m "docs(text-editing): E2 plan + index entry; gate green

E2 closes: keymap table, focus-gated input→Action lowering in BuiySet::Input,
SingleLine Wrap::None + Enter-submit + newline strip, value()/TextChanged, and
the Input-driven N→N+1 latency gate (OQ#1). Headless gate + facade boundary
green; no GPU work (E2 is fully headless). Errata: Insert(String) not SmolStr
(no new dep); the input apply is a gated shaping lock site (fires only on real
edits).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## E2 errata (fold into the spec at campaign closure)

1. **`EditCommand::Insert` holds `String`, not `SmolStr`** (spec § 3 sketches
   `Insert(SmolStr)`). `smol_str` is not a Buiy dependency; a `String` copied
   from `KeyboardInput.text` lowers identically (`Action::Insert` takes a
   `char`). Avoids a new dependency for zero behavioral difference.
2. **The input apply is a fourth, gated, `SharedFontSystem` lock site**
   (architecture § 1.2 names "exactly three": measure, TextCommit, glyph
   producer). The editor-input path legitimately reshapes, so applying an
   `Action` needs the lock. It is **gated to fire only on a frame with a real
   editing command** (collect-then-lock), so it adds no steady-state lock cost —
   an edit *is* a reshape trigger, the same class as the three existing sites.
   The § 1.2 count predates the editor input path and should be updated to "four
   sites, the fourth gated on input edits."
3. **The § 3.1 letter-commands (A/X/C/V/Z/Y) are system-resolved, not table
   rows.** The table is a pure positional-key map keyed on `(Modifiers,
   KeyKind)`; `KeyKind::Char` carries no letter, so a `(Ctrl, Char)` row cannot
   distinguish Ctrl-A from Ctrl-C. The system resolves these from the typed
   character + the platform command modifier. The spec's "Ctrl/Cmd-{A,…} per
   §§ 7/8" rows are honored — at the system layer, where the character is
   available — not in the static table. (A clarifying note, not a contradiction.)
4. **The editor-input path needs a dirty-mark to enter the measure pipeline
   (M1).** The campaign frames OQ#1 as "one-frame latency, no new machinery,"
   and the *scheduling* needs none. But an `Action` into the editor-OWNED buffer
   leaves the `Text` component unchanged, so it trips none of `TextSyncTriggers`
   and `sync_one`'s `mark_dirty_for_entity` never fires — the edit would
   dead-end. So `apply_keyboard_edits` must, on a value-changing edit,
   `invalidate_intrinsics()` + `mark_dirty_for_entity(entity)` (exactly
   `sync_one`'s pair) to enter the EXISTING measure → commit → extract path.
   This is a seam into the existing pipeline, NOT a new Taffy pass; the
   campaign's "(no new machinery)" should be read as "no new compute pass" — the
   dirty-mark gesture is the editor-side equivalent of the sync-side one. (This
   also makes `apply_keyboard_edits` a NonSend main-thread system, since
   `LayoutTree` is a non-send resource — the same as `text_sync_buffers`.)
5. **`FocusedEntity` is a `FocusPlugin` resource, not `CorePlugin` (M2).** The
   focus-gated editing system therefore reads `Option<Res<FocusedEntity>>` (and
   `Option<NonSendMut<LayoutTree>>`) so registering it in `BuiyTextPlugin` does
   not panic harnesses that lack `FocusPlugin`/`LayoutPlugin`. (A wiring fact,
   not a spec contradiction — noted so E3+ phases keep the same Option-tolerant
   gating when they add focus-gated systems.)

## Deferred to later phases (NOT E2)

- Mouse `Click`/`DoubleClick`/`TripleClick`/`Drag`, the selection geometry, the
  caret model + painting, the BiDi split caret → **E3**.
- `Cut`/`Copy`/`Paste` behavior (clipboard), `Undo`/`Redo` behavior (the undo
  engine) — recognized as commands in E2, no-op until → **E4**.
- IME preedit, `value()` preedit-exclusion refinement → **E5**.
- Focus gain/loss lifecycle, `Placeholder` rendering, auto-scroll, the
  host-facing `EditSubmitted`, the `TextInput` widget → **E6**.
