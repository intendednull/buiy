# Proto-3 Research — the `TextEditState` crux: widget-internal text-edit state vs. a core MVU log

> **Stage:** RESEARCH (read-only) for prototype-3 "MVU as the CORE". Area:
> **text-edit-state-crux** (charter signal #3, named identically by the hot-reload
> research). Code claims are against the CURRENT `origin/main` worktree at
> `/mnt/storage/projects/buiy/.claude/worktrees/mvu-core`; all `crates/…` paths
> resolve there. Seed docs (charter, retrospectives, draft spec, proto-1/2 code)
> live in the separate `state-mgmt-elm-prototype` worktree.

## TL;DR (the load-bearing conclusions)

1. **The cosmic substrate cannot be Reflected, and it should not be.** `TextEditState`
   (`crates/buiy_core/src/text/edit/state.rs:92`) wraps a `cosmic_text::Editor<'static>`
   plus three foreign-typed sub-states (`undo` Changes, `compose_delete` Change, the
   editor cursor/selection). The proto-2 `Model` trait *requires* `Reflect`
   (`examples/mvu_native/src/runtime.rs:30`). The editor can never satisfy it without
   reflecting `cosmic_text::Buffer`/`Change` — a huge, derived-cache-laden foreign
   surface. **So "make `TextEditState` Reflect" is the wrong framing.**

2. **The recordable substrate does NOT require Reflecting the editor — it requires
   routing the INPUT through the funnel.** The editor is already a *de-facto reducer*:
   one verb vocabulary (`EditCommand`, `command.rs:21`) and one mutation site
   (`TextEditState::apply_tracked`, `input.rs:94`). If the funnel records the
   `EditCommand`/IME message stream into the editor's mailbox, **replay reconstructs the
   editor by re-folding from init** (Elm semantics) — the cosmic Editor never needs to
   serialize. This is the clean way to "complete the log".

3. **Record/replay and hot-reload are DIFFERENT problems with different mechanisms.**
   Replay = re-fold input Msgs from init (no editor snapshot). Hot-reload = preserve
   *live* state across a world rebuild *without* replay-from-init ⇒ needs a small
   serializable **logical projection** (`value:String` + caret + selection + preedit +
   undo-as-`ChangeItem`s), reconstructing the cosmic Editor via `set_text`/`set_cursor`.
   Neither needs the Editor itself to be `Reflect`.

4. **The editor's update is intrinsically IMPURE — it needs `&mut FontSystem`.** This is
   the single biggest finding against "every widget is a pure actor". Motion/Click verbs
   shape against the `FontSystem` (`input.rs:164`, `pointer.rs:105`), and `Paste` reads
   the OS clipboard inside the reducer (`input.rs:250`). A sealed `PureEnv`
   (`runtime.rs:188`) cannot bless `FontSystem`. **The editor must be an imperative leaf
   that ROUTES (records its Msgs, emits `TextChanged` upward) — not a pure `Model`.**

5. **A core-MVU drain is a NEW post-`TextCommit` buffer mutator, so it re-arms the
   proto-1 extract crash.** The drain that folds editor Msgs runs after Layout (Input is
   after Layout, `lib.rs:107`), unshapes the buffer, and `extract_buiy_glyphs` asserts
   `layout_runs().count() == ComputedTextLayout.lines.len()` (`extract.rs:720`). The
   spec MUST pin the drain ordering so `reshape_edited_editors` (`commit.rs:181`) runs
   after the drain and before extract — the `mod.rs:202` warning ("Any FUTURE
   post-Input editor-buffer mutator MUST also be ordered before this system") is
   addressed to exactly this drain.

6. **Performance is FAVORABLE for text, not a threat — if you record Msgs, not state.**
   Keystrokes/IME are human-paced (tens/sec), and only the *one focused* editor mutates
   per frame (`input.rs:592`). Per-keystroke Reflect-serialize is trivially inside the
   16 ms/60 Hz budget. The charter's "Reflect-on-the-hot-path" risk does **not** bite
   text *unless* someone serializes editor state per frame — which the design must
   forbid.

---

## (a) Exactly what text-edit state exists, where it lives, and what is NOT Reflect/serializable

### The one component: `TextEditState` (`state.rs:92-126`)

`#[derive(Component)]` only — **deliberately NOT `Reflect`-registered**; the doc is
explicit (`state.rs:88-91`): *"Machinery state — NOT reflect-registered (it carries a
`cosmic_text::Editor`, and this module is the cosmic boundary; the `TextBuffer`
precedent)."* It is also **optional** on a text entity (display-only entities never pay
for it, `state.rs:74-77`).

| Field (`state.rs`) | Type | Nature | Reflect/serde today | Replay role | Hot-reload snapshot role |
|---|---|---|---|---|---|
| `editor` (`:96`) | `cosmic_text::Editor<'static>` | **Authoritative** content + cursor + selection + shape caches + in-flight `Change` | **No** (foreign; private — only reachable via `TextBufferAccess`) | reconstructed by fold | project to `value:String` + cursor (line,index,**affinity**) + selection; rebuild via `set_text`/`set_cursor` |
| `intrinsics` (`:102`) | `Option<IntrinsicWidths>` | Derived width cache | No (plain) | recomputed by measure | **drop** (derived) |
| `selection` (`:107`) | `TextSelection` | **Projection** (re-mirrored each changed frame, never a 2nd truth — `state.rs:104-107`) | No (`SmallVec<SelectionRange>` of pure-data `Cursor`) | recomputed | derivable; or snapshot the pair |
| `blink` (`:110`) | `CaretBlink { origin: Duration }` | Ephemeral animation phase, reset on edit/focus | No (trivially serializable) | irrelevant | **drop** (resets on focus) |
| `undo` (`:114`) | `UndoStack` → `Vec<UndoUnit>` each holding a `cosmic_text::Change` (`undo.rs:50-73`) | Real user history | **No** (`Change` is foreign) | reconstructed by fold *or* lost | needs serializable mirror of `ChangeItem{start,end,text,insert}` |
| `preedit` (`:119`) | `Option<PreeditSpan>` (`ime.rs:43`) | Live IME composition span (line,start,len,cursor) | No (pure data) | reconstructed by re-folding `Ime::Preedit` | snapshot, OR seal composition on snapshot |
| `compose_delete` (`:125`) | `Option<ComposeDelete>` (`state.rs:34-43`) | Stashed reversible delete-of-selection (holds a `cosmic_text::Change`) | **No** (`Change` is foreign; note: it lives here *because* `PreeditSpan` derives `Eq` and `Change` is not `Eq`, `state.rs:31-33`) | reconstructed | snapshot only mid-composition (edge) |

The **decomposed policy markers** ARE Reflect, and are the only edit-side types
registered today (`state.rs:298,305,311,318`; registered in `mod.rs:154-157`):
`ReadOnly`, `Disabled`, `SingleLine`, `Placeholder(String)`. These are authoring-surface
config, not runtime state — they already round-trip.

The **paint-seat outputs** (`CaretVisual`, `SelectionVisual`, `PreeditVisual` in
`components.rs:416,463,502`; `ComputedTextLayout` `:658`) are *also* deliberately not
Reflect — "Computed output / machinery state — not reflect-registered" (they carry
`cosmic_text::Cursor`). They are **derived from `TextEditState` every frame** by
`write_caret_and_selection` (`caret.rs:151`), so they never need to be in any snapshot —
they regenerate.

### What is genuinely un-serializable, distilled

Only three things are foreign and load-bearing: **(i) the editor's buffer/cursor/selection**
(but its *logical* content is recoverable as a `String` + `Cursor` via the existing
`value()`/`caret()`/`mirror_selection()` accessors — `state.rs:200,239,227`), **(ii) the
undo/redo `Change`s**, **(iii) the `compose_delete` `Change`**. (ii) and (iii) are the
genuinely awkward ones: a `cosmic_text::Change` is a `Vec<ChangeItem>` where each item is
`{start: Cursor, end: Cursor, text: String, insert: bool}` — *individually* serializable
fields wrapped in a foreign type. There is **no `serde` anywhere in `text/edit/`** today
(verified: grep returns nothing).

---

## (b) The input → edit-state → shaping → render-extract pipeline

```
 winit ─► Bevy Messages:  KeyboardInput / Ime / Pointer<Press|Drag>
                              │
   BuiySet::Input  ──────────►│  (runs AFTER BuiySet::Layout — lib.rs:107)
   ┌──────────────────────────┴───────────────────────────────────────────┐
   │ apply_keyboard_edits (input.rs:562)  — focus-gated on FocusedEntity    │
   │   classify(logical_key) ► keymap.resolve / letter_command              │
   │     ► EditCommand (command.rs:21)                                      │
   │   collect-then-lock (input.rs:604,656): FontSystem lock held ONLY for  │
   │     the apply burst; no-key frame is lock-free                         │
   │   TextEditState::apply_tracked (input.rs:94) — THE ONE mutation site:  │
   │     start_change / ed.action(fs, Action::…) / finish_change            │
   │     ► records UndoUnit{cosmic Change} (undo.rs:156)                    │
   │     ► edit LAZILY UN-SHAPES the buffer (reshape deferred — input.rs:317)│
   │   M1 dirty-mark: invalidate_intrinsics + tree.mark_dirty (input.rs:704)│
   │   emit TextChanged / EditSubmitted / EditUndone (input.rs:711)         │
   │ apply_ime (ime.rs:429) — splice/commit/remove via DIRECT BufferLine    │
   │   surgery, NO Change (invariant a, ime.rs:99) ► also unshapes          │
   │ editor_pointer_press/drag (pointer.rs:146,182) — Buffer::hit needs the │
   │   PREVIOUS frame's shaped layout; cosmic Action::Click{x,y}            │
   └──────────────────────────┬───────────────────────────────────────────┘
                              │  buffer now UNSHAPED (edit happened post-TextCommit)
   reshape_edited_editors (commit.rs:181)  .after(Input).after(focus_lifecycle)
   ┌──────────────────────────┴───────────────────────────────────────────┐
   │ lock-free guard: layout_runs().count() != computed.lines.len()        │
   │   ► if stale: shape_until_scroll(fs) + rewrite ComputedTextLayout      │
   │   (the COHERENCE REPAIR that prevents the proto-1 extract crash)       │
   └──────────────────────────┬───────────────────────────────────────────┘
                              │  buffer SHAPED + ComputedTextLayout coherent
   write_caret_and_selection (caret.rs:151) .before(write_caret_blink)      │
   ┌──────────────────────────┴───────────────────────────────────────────┐
   │ mirror_selection ► CaretVisual / SelectionVisual / PreeditVisual       │
   │ if buffer transiently unshaped: `continue` — defer to N+1 (caret.rs:202)│
   │ reset blink, emit CaretMoved / SelectionChanged                        │
   └──────────────────────────┬───────────────────────────────────────────┘
   write_ime_window / auto_scroll_caret / focus_lifecycle (same window)
   #[cfg(debug)] debug_assert_shape_coherence (Last, commit.rs:243)
                              │
   ── RENDER WORLD (ExtractSchedule) ──
   extract_buiy_glyphs (extract.rs:165)  .after(maintain_atlas)
   ┌──────────────────────────┴───────────────────────────────────────────┐
   │ reads editor-owned buffer via TextBufferAccessReadOnly (access.rs:122) │
   │ walks layout_runs() ► emit GlyphAlphaInstance + decoration quads       │
   │ TRIPWIRE: debug_assert layout_runs == lines.len() (extract.rs:720)     │
   └────────────────────────────────────────────────────────────────────────┘
```

Key timing facts:
- **Edits run AFTER the layout step.** `apply_keyboard_edits`/`apply_ime` are in
  `BuiySet::Input` (`mod.rs:311,326`), and `BuiySet::Input` is chained *after*
  `BuiySet::Layout` (`lib.rs:107`), which is where `text_commit` lives
  (`mod.rs:188`). Hence the accepted **one-frame (N→N+1) latency** and the need for the
  repair system.
- **The accessor is editor-first.** `TextBufferAccess` (`access.rs:41`) binds display
  `TextBuffer` + `Option<TextEditState>` and dispatches buffer reads "editor-owned if
  present, else display" (`access.rs:59`). Measure, commit, the caret writer, and extract
  all reach the editor's buffer through this one seam — *they never name a cosmic Editor*
  (facade boundary, `tests/text_facade_boundary.rs`).
- **Only the focused editor mutates from keyboard/IME** (`input.rs:592`, `ime.rs:446`).
  At thousands of editors, keystroke cost stays O(1); only `reshape_edited_editors` walks
  all editors for its cheap guard (a real but pre-existing scale note — see Risks).

---

## (c) What whole-UI record/replay + hot-reload REQUIRE of this state

### Record / replay (the MVU thesis): route input, do NOT snapshot the editor

The proto funnel records `(LogicalId, Msg, seq)` and re-folds from init
(`runtime.rs:101-134`, `MsgLog`). For text, the **Msg is the input** — the editor is the
fold. Requirements:

1. **Route keystrokes/IME/pointer through the funnel inbox, not direct `MessageReader`.**
   Today `apply_keyboard_edits`/`apply_ime` read `MessageReader<KeyboardInput>`/`<Ime>`
   directly (`input.rs:563`, `ime.rs:430`) — *outside* any log. Core-MVU must make the
   editor consume from a funnel-controlled mailbox so replay can inject synthetic Msgs.
2. **The recorded Msg must be `Reflect`.** Record at the **resolved `EditCommand`** level
   (post-keymap) so replay is keymap-independent — but `EditCommand` names
   `cosmic_text::Motion` (`command.rs:13,26`), a foreign type. **Migration cost: Buiy
   must own a `Motion` enum (≈20 pure-data variants) or `reflect_opaque` it.** `Ime`
   (`bevy::window::Ime`) and `KeyboardInput` are also foreign — if recording raw input,
   they need Reflect wrappers; recording `EditCommand` + a small `ImeCommand` mirror is
   cleaner.
3. **Determinism of the fold.** `apply_tracked` is deterministic *given the same
   `FontSystem` and box geometry*. Shaping determinism already depends on
   `FontsGeneration`/`FontDbLineage` and the embedded default font; **the system-font
   scan (non-deterministic) must stay OFF or be recorded as an env/init condition**
   (`mod.rs:98-103` — it is off by default). The virtual clock (`now: Duration`,
   `input.rs:653`) feeds undo coalescing + blink and **must be recorded** (already a
   `Time<Virtual>` the proto seeds deterministically).
4. **Impure reads inside the reducer become logged EFFECTS.** `Paste` reads the OS
   clipboard *inside* `apply_tracked` (`input.rs:250`); `Copy`/`Cut` *write* it
   (`input.rs:224,236`). On replay there is no OS clipboard — the resolved pasted text
   must be in the log (the proto's "an effect's result is itself a later log entry",
   draft spec §7). So the clipboard read must be hoisted out of the reducer into a
   recorded effect, or the `Paste` Msg must carry its resolved text.

**Net: replay needs the editor to be a recordable reducer, but NOT `Reflect`.** Nothing
about the editor's *state* is serialized for replay; the entire `TextEditState` (buffer,
undo Changes, preedit, compose_delete) is rebuilt by re-applying the logged Msg stream
from a known init.

### Hot-reload: a serializable LOGICAL projection (NOT a Reflected Editor)

Hot-reload preserves *live* state across a code/asset swap **without** replay-from-init
(the hot-reload research's "state crux"). Here you DO need a snapshot — but of the
*logical projection*, not the cosmic Editor:

- **Capture:** `value(): String` (`state.rs:200` — already excludes preedit and LF-joins
  lines), caret `(line, index, affinity)` (`caret()`, `state.rs:239`), selection
  (`mirror_selection()`, `state.rs:227`), `preedit` span, and the undo/redo history as
  `Vec<SerializableChangeItem>`.
- **Restore:** rebuild the cosmic Editor with `set_text` + `set_cursor` + re-apply
  selection + re-splice preedit + rebuild the `UndoStack` from the serialized items. One
  reshape follows (the normal `reshape_edited_editors` path).
- **A new type, e.g. `TextEditSnapshot`, that IS `Reflect`/`Serialize`** and a
  `to_snapshot`/`from_snapshot` pair on `TextEditState`. This is a **bounded** addition,
  not "reflect the whole engine".

**What breaks / what must be handled in a snapshot:**
- **`Cursor.affinity`** (Before/After) is load-bearing for BiDi carets (the whole
  `secondary_caret_rect_for` machinery, `caret.rs:111`) — the snapshot must round-trip it
  or BiDi caret position drifts after reload.
- **Shape caches** are dropped → one reshape on restore (acceptable; it is the steady
  path).
- **Mid-composition IME across a reload** is the ugly edge: the preedit is *spliced into
  the buffer* (direct surgery, `ime.rs:245`) while `value()` excludes it; `compose_delete`
  may hold a stashed delete. Recommend **sealing composition on snapshot** (mirror the
  focus-loss seal, `lifecycle.rs:77`) so the snapshot is taken at a clean boundary.
- **Undo fidelity is the cost decision:** dropping undo on reload is cheap but
  user-visible; preserving it requires the `ChangeItem` mirror.

### Does `TextEditState` have to become `Reflect`? — **No.**

Both needs are met without reflecting the cosmic Editor: replay re-folds; hot-reload uses
a logical projection. Reflecting `Buffer`/`Change` would be a large, fragile surface whose
serialized bytes are mostly *derived caches* — strictly worse than the projection. The
charter's framing ("the un-reflected `TextEditState`") is real as a *symptom*, but the fix
is **a projection + funnel-routing**, not a `#[derive(Reflect)]` on the engine.

---

## (d) The proto-1 unshaped-text extract-crash lesson, and how core-MVU interacts with extract timing

### The lesson

Proto-1 surfaced an "unshaped-text-at-extract crash" (proto-1 retrospective; the campaign
"already drew blood here: *reshape ordering vs focus_lifecycle*", RETRO line 95). The
root mechanism, now hardened in main:

- `extract_buiy_glyphs` walks `buffer.layout_runs()`, which **terminates at the first
  unshaped line**, and asserts the run count equals the committed line count
  (`extract.rs:717-724`): *"TextBuffer dirty-unshaped at extract (mutated after
  TextCommit?)"*.
- Any system that **mutates a text buffer after `TextCommit` and before extract without
  reshaping** trips it. Editor edits are exactly that: they run in `BuiySet::Input` (after
  Layout/TextCommit) and *lazily unshape* the buffer (`commit.rs:153-168` doc;
  `input.rs:317`).
- The fix is **`reshape_edited_editors`** (`commit.rs:181`): a post-Input,
  pre-caret-writer system that re-detects the *same* invariant lock-free
  (`layout_runs().count() != lines.len()`, `commit.rs:203`) and reshapes, plus a
  debug-only `Last`-schedule mirror `debug_assert_shape_coherence` (`commit.rs:243`) so
  the headless gate (which runs no render world) still catches it. The ordering is pinned
  in `mod.rs:207-213`: `.after(BuiySet::Input).after(focus_lifecycle)
  .before(write_caret_and_selection)`.
- proto-2 confirmed plain `Text` labels no longer crash (the `#[require(Node)]` fix held,
  PROTO2 RETRO 107-109) — but that is the *display* path; the *editor* path is guarded by
  `reshape_edited_editors`, which proto-2 never exercised (no editor in the counter demo).

### How a core-MVU model interacts with this timing

A core-MVU drain that folds editor Msgs **is a new post-`TextCommit` buffer mutator** —
precisely the hazard `mod.rs:202-204` warns about: *"Any FUTURE post-Input editor-buffer
mutator MUST also be ordered before this [reshape] system."* Consequences the spec must
nail:

1. **The drain ordering is load-bearing, not emergent.** Both retrospectives already flag
   this: proto-1 REDESIGN #2 ("run-to-completion drain ordering is a DESIGNED concern…
   pin the `MvuSet` ordering precisely") and proto-2 REFINE #2 ("pin the flush points…
   latency is one designed frame, not emergent"). The editor fold must be ordered so that
   `reshape_edited_editors` runs **after** it and **before** `write_caret_and_selection`
   and extract.

2. **Two viable placements:**
   - **(Recommended) Keep the editor fold in the post-Layout window** (where Input is
     today) and let `reshape_edited_editors` keep repairing. Minimal disruption; the
     one-frame latency is already accepted; the repair is idempotent.
   - **(Runner-up) Move the editor fold BEFORE `TextCommit`** so the layout step reshapes
     naturally and the repair system could be retired. Tempting (removes a system) but
     dangerous: pointer hit-testing and caret rects need the *current* shaped geometry,
     which comes from the previous frame's commit; folding before commit risks the editor
     reading geometry that this-frame's commit then changes, and it fights the established
     "edits are post-Layout" invariant the whole text campaign is built on. High blast
     radius for a marginal win.

3. **Run-to-completion must finish within the frame.** The proto's `Cmd::Emit`
   re-entrancy (`runtime.rs:68-86`) can mutate the buffer several times in one drain pass
   — fine, the repair runs once after. But proto-2 REFINE #2 noted flush points can *span
   frames*. **If a drain leaves an editor mutated-but-unshaped across a frame boundary,
   extract sees it unshaped.** The mitigation already exists and must be preserved:
   `reshape_edited_editors` runs **unconditionally every frame** with a cheap guard, so
   any unshaped editor at that point is repaired regardless of which system dirtied it.
   The spec must require the drain to complete before that repair each frame.

4. **The caret writer's "skip when unshaped" defense composes cleanly.**
   `write_caret_and_selection` already `continue`s rather than synthesizing a bogus rect
   on a transiently-unshaped frame (`caret.rs:202`, with a long comment about *not*
   tripping the extract tripwire). A core-MVU fold does not invalidate this; it relies on
   the same repair-then-read ordering.

**Bottom line for (d):** core-MVU does **not** eliminate the extract-timing hazard and
does **not** let us delete `reshape_edited_editors`; it *relocates the mutation into the
drain*, which makes the drain ordering vs. the repair system a **hard, must-pin
constraint** in the spec. The existing repair + debug mirror are the right shape and
should be kept; the campaign's prior "reshape ordering vs focus_lifecycle" blood is the
precedent for getting this wrong.

---

## Decisions (recommendation + rationale + runner-up)

### D1 — How does text-edit become part of the recordable substrate?
**Recommendation: the editor is an IMPERATIVE LEAF that ROUTES — record its
`EditCommand`/IME Msg stream into the funnel; reconstruct on replay by re-folding; keep
`TextEditState` non-`Reflect`.**
*Rationale:* it is already command-shaped (one verb enum, one mutation site), so routing
is small; replay rebuilds the editor for free; it sidesteps reflecting the cosmic engine;
and it matches the charter's own "leaf widgets stay imperative and only route" branch.
*Runner-up:* make `TextEditState` a first-class `Model` (Reflect) — **rejected**: the
proto `Model: Reflect` bound (`runtime.rs:30`) is unsatisfiable for a `cosmic_text::Editor`
without reflecting `Buffer`/`Change`; the serialized bytes would be mostly derived caches.

### D2 — What does the recorded Msg look like?
**Recommendation: record the resolved `EditCommand` (+ a small `ImeCommand` mirror),
after keymap resolution; Buiy owns a `Motion` enum to make it `Reflect`.**
*Rationale:* keymap-independent replay; `Motion` is pure-data and already lowered in the
facade; avoids reflecting `bevy::window::Ime`/`KeyboardInput`.
*Runner-up:* record raw `KeyboardInput`/`Ime` — **rejected**: drags the platform keymap
+ modifier state into the replay env and needs Reflect wrappers for two foreign input
types.

### D3 — Hot-reload state preservation
**Recommendation: add a `Reflect`/`Serialize` `TextEditSnapshot` (logical projection:
value + caret incl. affinity + selection + preedit + undo-as-`ChangeItem`s) with
`to_snapshot`/`from_snapshot`; seal composition on snapshot.**
*Rationale:* survives a world rebuild without replay-from-init; bounded surface; rebuilds
the Editor via existing `set_text`/`set_cursor` seams; keeps derived caches out of the
serialized form.
*Runner-up:* reflect the engine, or drop edit state on reload — **rejected** (former is a
huge fragile surface; latter is a user-visible data-loss regression).

### D4 — Editor reducer purity under `PureEnv`
**Recommendation: explicitly EXEMPT the editor from reducer-purity — it is the documented
case where "every widget is a pure actor" breaks (needs `&mut FontSystem`, reads the OS
clipboard).** Determinism is guaranteed at the boundary (same FontSystem + box ⇒ same
fold), not via `PureEnv`. Clipboard reads become logged effects (D2/§c.4).
*Rationale:* shaping is intrinsic to applying motion/click edits (`input.rs:164`,
`pointer.rs:105`); `FontSystem` is a non-Reflect mutable shared resource a sealed
`PureEnv` cannot bless.
*Runner-up:* refactor the editor so the reducer mutates only a pure text rope and shaping
is a `Cmd` effect — **rejected for v1**: cosmic's `Editor` couples text mutation to
cursor/visual-motion semantics that *need* shaped runs; splitting it is a deep rewrite of
the verified editor, out of proportion to the benefit.

### D5 — Drain vs. extract timing
**Recommendation: keep the editor fold in the post-Layout window and keep
`reshape_edited_editors` + `debug_assert_shape_coherence`; pin the drain so the repair
runs after it and before the caret writer / extract.**
*Rationale:* preserves the hard-won shape-coherence invariant and the accepted one-frame
latency; the repair is idempotent and already unconditional.
*Runner-up:* fold before `TextCommit` to retire the repair — **rejected** (high blast
radius; fights the "edits are post-Layout" invariant; pointer/caret geometry needs
current shape).

---

## Risks

| Risk | Severity | Evidence | Candidate mitigation |
|---|---|---|---|
| **A core-MVU drain re-arms the proto-1 extract crash** (post-`TextCommit` buffer mutation reaching extract unshaped) | **High** | `extract.rs:720` tripwire; `commit.rs:153-168`; `mod.rs:202-204` warning addressed to "future post-Input mutators" | Keep `reshape_edited_editors` (unconditional, cheap guard) + `debug_assert_shape_coherence`; pin drain `.before` the repair; require run-to-completion within the frame |
| **Editor reducer is impure (`&mut FontSystem`, OS clipboard) — cannot be a `PureEnv` reducer** | **High** | `input.rs:164` (motion shapes), `pointer.rs:105` (click shapes), `input.rs:250` (Paste reads clipboard); `runtime.rs:188` sealed `PureEnv` | Exempt the editor as an imperative routing leaf (D1/D4); hoist clipboard read to a logged effect |
| **`EditCommand` (and Ime/KeyboardInput) are not `Reflect`** — the recorded Msg can't round-trip | Medium | `command.rs:13,26` names `cosmic_text::Motion`; no serde in `text/edit/` (verified) | Buiy-owned `Motion` enum + `ImeCommand` mirror; record resolved `EditCommand` (D2) |
| **Undo/`compose_delete` hold foreign `cosmic_text::Change`** — hot-reload snapshot can't capture history | Medium | `undo.rs:50-58`, `state.rs:34-43` | Serializable `ChangeItem{start,end,text,insert}` mirror; or accept undo-loss-on-reload as a documented v1 limit |
| **IME edge cases under record/replay** (compose-over-selection delete+stash, cancel reverse-apply, empty-commit-over-selection; preedit spliced into buffer while `value()` excludes it) | Medium | `ime.rs:115-179` (splice+stash), `:194-214` (cancel), `:323-379` (commit fold); `state.rs:200-221` (value excludes preedit) | Route every `Ime` sub-event through the funnel; replay reproduces deterministically given buffer state; seal composition at snapshot boundaries |
| **`Cursor.affinity` lost in a naive snapshot** ⇒ BiDi caret drift after reload | Medium | `caret.rs:111` secondary-caret machinery keys on glyph direction at `caret.index` | Snapshot stores affinity explicitly; round-trip test against a BiDi fixture |
| **Reflect-on-the-hot-path** IF state is serialized per frame | Low (for text) | keystrokes are human-paced; only the focused editor mutates (`input.rs:592`) | Record Msgs (human-paced), never serialize editor state per frame; hard rule in the spec |
| **`reshape_edited_editors` walks ALL editors every frame for its guard** | Low (pre-existing) | `commit.rs:199-204` iterates `Query<…, With<TextEditState>>` each frame | Pre-existing; only the dirtied editor reshapes. If thousands of editors materialize, gate the guard with a dirty-set/`Changed` (out of scope for this crux) |
| **Migration cost of MVU-ifying the editor** (~3.5k LOC across `text/edit/`, facade-boundary-protected) | Medium | `edit/` is 8 systems + state machine; `tests/text_facade_boundary.rs` tripwire constrains where cosmic types may be named | Do NOT rewrite the editor; add a thin funnel adapter (route input, emit upward) + the snapshot projection — the editor's internals stay behind the facade unchanged |

---

## Open questions for the SPEC stage

1. **At what granularity is the editor Msg recorded** — resolved `EditCommand` (recommended)
   vs. raw key events? Confirms the keymap's place (env vs. replayed).
2. **Clipboard as a logged effect** — does `Paste` carry resolved text in its Msg, or is
   the clipboard read a separate `Cmd`/effect whose Result is logged? (Cut/Copy are pure
   *writes* to an external sink — are they recorded at all?)
3. **Undo on hot-reload** — preserve via `ChangeItem` mirror, or accept loss-on-reload for
   v1? (Cost vs. fidelity.)
4. **Does the focused-editor mailbox sit on the editor entity** (per-`LogicalId`) or on a
   global input router that addresses the focused entity? Interacts with the
   agent-interface action-lowering (charter advantage #3) and `set_focus`/`set_value`
   becoming Msg-addressed (proto-1 REDESIGN #1).
5. **Mid-composition snapshot policy** — seal-on-snapshot (recommended) vs. faithfully
   capture `preedit` + `compose_delete`. Seal is simpler; faithful is more correct for a
   reload that lands mid-IME.
6. **Does `FocusedEntity` itself become Msg-addressed/recorded?** Focus drives *which*
   editor folds; replay must reproduce focus transitions (`lifecycle.rs:53`), which today
   are driven by pointer/Tab outside any log.
