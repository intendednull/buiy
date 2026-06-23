# Editor Text-Integrity (Bugs 2 + 3) — child C2 of the widget-catalog campaign

`2026-06-22` · `[draft]` · Wave 1 · realizes foundation `text.md` (the editor measure/commit seam) · depends on C0; co-delivers Tier-B with C7

> Scope (umbrella §4 C2 / decision §2.6): the `FontsGeneration` bump must not
> clobber an editor-owned buffer's typed content (content survival), must not
> leave a buffer un-reshaped (the shape guard), and must preserve a live IME
> preedit/composition. Bug 2 (`text_commit` shape-guard) and Bug 3 (`TextSync`
> editor-content clobber) are **one** change because the guard *masks* the
> clobber on the editor class. This child owns the fix; the isolating headless
> tests are co-delivered with C7 Tier-B (umbrella §5 Wave 1).

---

## 1. Problem & current state

### 1.1 Bug 3 — `TextSync` clobbers the editor's typed content (the data-loss bug)

A `TextInput` is `(Text(""), TextEditState, …)` — `Text` is *required* and initialized
to the empty string (`crates/buiy_widgets/src/text_input.rs:56-60`), because the entity
needs a `Text` so `text_sync_buffers` runs and the node measures (the editor-optional /
buffer-required contract, `state.rs:74-85`). **Typed content never reaches the `Text`
component.** The edit path (`apply_tracked` → cosmic `Action`, `crates/buiy_core/src/text/edit/input.rs:94`)
mutates the editor-owned `Buffer` directly; the `Text` component stays `""` forever.

The `TextBufferAccess` accessor is **editor-first**: when `TextEditState` is present its
owned buffer is authoritative (`crates/buiy_core/src/text/edit/access.rs:43-79`). So when
`text_sync_buffers` sweeps, it writes the *authored* `Text` into the *editor-owned* buffer:

- The `FontsGeneration` bump sweeps **every** buffer (`crates/buiy_core/src/text/sync.rs:251`,
  `synced.p1()` over the full set), calling `sync_one` (`sync.rs:298`).
- `sync_one` → `access.with_buffer_mut(|buffer| apply_authored_to_buffer(buffer, text, …))`
  (`sync.rs:332-342`). On an editor entity `buffer` is the **editor-owned** buffer and `text`
  is `Text("")`.
- `apply_authored_to_buffer` → `buffer.set_text(&directed, …)` (`sync.rs:530`) where
  `directed` is the collapsed+strong-marked `""`. **The editor-owned buffer is overwritten
  with the empty string — the user's typed content is gone.**

`sync.rs` is **byte-identical** between current `main` and the prototype tree (audit §2 Bug 3):
the prototype never fixed this; its only mitigation was operational ("settle 5 frames before
typing") — timing-luck that does not generalize. The trigger is **broader** than the periodic
~9 s system-font scan: `apply_font_registry` bumps `FontsGeneration` on *every* runtime
`add_font` batch (`crates/buiy_core/src/text/registry.rs:543`, `generation.0 += 1` after a
`db_changed` batch), so the clobber can fire **mid-typing**, not just once at startup.

There is **no whole-value-set verb** in `EditCommand` (`crates/buiy_core/src/text/edit/command.rs:21-51`).
The display-`Text`→editor seam is therefore the editor's **only** content-seed and **only**
de-facto programmatic-set path today. Once §2.1 suppresses content-lowering for editors, the
seed/programmatic-set must move to the **existing** `EditCommand` verbs — empty editor →
`Insert(initial)`, programmatic set → `SelectAll` + `Insert` — exactly the lowering the
**agent-interface campaign** already uses for `Action::SetValue`-text (it adds **no** new
value-set variant; see "Coordination with the agent-interface campaign" below). C2 does **not**
add an `EditCommand::SetValue`.

TextSync is also the **sole writer** of the editor buffer's `wrap` / `tab_width` / `attrs`
(`apply_authored_to_buffer` calls `set_wrap` (`sync.rs:526`), `set_tab_width` (`sync.rs:527`),
and lowers `Attrs`; `text_commit` only sets `set_size` + per-line `set_align`,
`commit.rs:89-119`). So a blanket "editors skip TextSync" would silence the clobber **and**
sever the editor's only style channel.

### 1.2 Bug 2 — `text_commit` has no shape guard (and the guard would *mask* Bug 3)

`text_commit`'s steady-state short-circuit gates on `align_changed || offset_stale || size_stale`
only (`crates/buiy_core/src/text/commit.rs:98-104`) — there is **no `shape_stale` term**
(`grep -c shape_stale crates/buiy_core/src = 0`). A buffer that has been unshaped *without*
its content-box size changing reaches extract still unshaped. Extract's tripwire then fires:

```
debug_assert_eq!(runs, computed.lines.len(),
    "TextBuffer dirty-unshaped at extract (mutated after TextCommit?)");
```
(`crates/buiy_core/src/text/extract.rs:709-716`) — a panic in debug, a **silent no-paint in
release** (`layout_runs()` terminates at the first unshaped line, so a both-axes-definite
editor box reaches extract with `runs=0` while `computed.lines.len()>0`).

The audit's CRITICAL finding (§2 Bug 2, Appendix-A.3): on the **editor class** the
`FontsGeneration` sweep is not merely an unshape, it is the **content clobber** of §1.1.
For the *observed* empty-editor crash the clobber was a no-op (the buffer was already empty),
which is exactly why "unshape only" looked complete. **Adopting the commit guard alone would
ship a framework that silently eats editor content on any async font load** while a TextInput
is non-empty: the guard reshapes the now-empty buffer (`size_stale` is false, but the
re-`set_text` left it unshaped, so the new `shape_stale` term fires), the assert is silenced,
and the user's text is gone with **zero** developer signal. This is why the two are one fix.

### 1.3 What the existing tests do NOT cover (the verification gap)

- `text_sync.rs::fonts_generation_bump_sweeps_every_buffer` (`crates/buiy_core/tests/text_sync.rs:326`)
  sweeps a **display-only** `Text` node — it never spawns a `TextEditState`, so it is
  structurally blind to the editor clobber.
- `text_commit.rs::fonts_generation_bump_remeasures_and_recommits` (`crates/buiy_core/tests/text_commit.rs:438`)
  likewise uses a plain display node; it auto-heals via `mark_dirty` and passes pre-fix.
- The prototype's `text_commit_font_reload.rs` (not on `main`) auto-heals and "isolates
  nothing" (audit §1 WRONG, Appendix-A.5).
- There is **no** content-survival, **no** preedit-survival, and **no** editor-class
  shape-guard test at any tier. This is the C7 Tier-B deliverable, co-delivered here.

---

## 2. Target design

The fix is **two coordinated changes**; the content-skip's replacement content channel is the
**existing** `EditCommand` verb pair (`Insert` for the empty-editor seed, `SelectAll` + `Insert`
for a programmatic set) — **not** a new variant (§3 resolves the open question to alt (a),
re-based onto the existing verbs to match the agent-interface campaign's owned `EditCommand`
surface; see "Coordination with the agent-interface campaign").

### 2.1 TextSync: editor entities skip content-lowering, keep style-lowering (Bug 3)

Split `apply_authored_to_buffer` into a **content+style** path (display entities, unchanged)
and a **style-only** path (editor entities). The editor's content is owned by the buffer; the
sweep must touch `metrics`/`wrap`/`tab_width`/`attrs` but **never `set_text`**.

The split is keyed on the accessor knowing whether `TextEditState` is present — add a
`has_edit()` discriminant to `TextBufferAccessItem` (it already binds
`edit: Option<&mut TextEditState>`, `access.rs:47`):

```rust
// access.rs — TextBufferAccessItem
/// `true` when an editor owns this entity's authoritative buffer (the
/// content-vs-style split point: editors own content, TextSync owns style).
pub fn has_edit(&self) -> bool { self.edit.is_some() }
```

`sync_one` branches on it (`sync.rs:332`):

```rust
let is_editor = access.has_edit();
let blocked = access.with_buffer_mut(|buffer| {
    if is_editor {
        // Style-only: never set_text (the editor owns content). Re-apply
        // metrics/wrap/tab-width and RE-SHAPE-AWARE attr refresh on the
        // existing lines, then return the font-display block flag.
        apply_authored_style_to_editor_buffer(buffer, &style, ctx.registry, ctx.index, ctx.now, single_line)
    } else {
        apply_authored_to_buffer(buffer, text, &style, ctx.registry, ctx.index, ctx.now, single_line)
    }
});
```

`apply_authored_style_to_editor_buffer` (new, `sync.rs`):

```rust
/// Style-only re-lower onto an editor-owned buffer (Bug 3 fix). Applies the
/// SAME metrics/wrap/tab-width as the content path but PRESERVES the buffer's
/// existing line text — `set_text` is the clobber, so it is never called. The
/// font-display block flag still derives from `resolve_spans` over the buffer's
/// CURRENT first-line text (a Block family must still gate the editor).
fn apply_authored_style_to_editor_buffer(
    buffer: &mut Buffer,
    style: &AuthoredStyle<'_>,
    registry: &FontRegistry,
    index: &mut FontMatchIndex,
    now: f64,
    single_line: bool,
) -> bool {
    buffer.set_metrics(style.metrics());
    let wrap = if single_line { Wrap::None } else { resolve_wrap(style.white_space, style.text_wrap) };
    buffer.set_wrap(wrap);
    buffer.set_tab_width(DEFAULT_TAB_WIDTH);
    // Refresh the per-line default attrs (weight/family/decoration bits) WITHOUT
    // dropping line text: rewrite each BufferLine's AttrsList defaults to the
    // resolved base attrs and reset the line's shape cache so TextCommit reshapes
    // it at the next lock site. (This is the cosmic-has-no-"set-attrs-without-text"
    // gap the audit names — solved by per-line default-attrs surgery, the same
    // technique ime.rs already uses to preserve attrs across a splice, ime.rs:233-244.)
    // resolution.blocked is computed over the buffer's current first line.
    refresh_line_default_attrs(buffer, style);
    style_block_flag(buffer, style, registry, index, now)
}
```

The attr-refresh reuses the per-line `AttrsList::defaults()` surgery already proven in
`ime.rs` (`splice_text_into_line` preserves resolved attrs by carrying `defaults()`,
`ime.rs:233-249`). It resets each line's shape so `text_commit` reshapes it (M1 re-measure),
keeping the editor's font/weight/decoration/wrap in sync after a runtime `add_font` **without
touching content**.

**Insert/seed path is unchanged.** The `unsynced` creation loop (`sync.rs:191-246`) is
`Without<TextBuffer>`; it never binds an editor (an editor entity already has a `TextBuffer`
required carrier). The editor's content seed comes from `EditCommand::Insert(initial)` applied
once on an empty editor at construction (§2.3) — NOT from `Text`. The display `TextBuffer.buffer`
of an editor entity is inert (the accessor never reads it for an editor), so leaving it empty is
correct.

### 2.2 `text_commit`: add the `shape_stale` guard (Bug 2)

Add a fourth term to the steady-state short-circuit (`commit.rs:98-104`) that compares the
exact two quantities extract compares — so it can never drift from extract's truth and never
false-positives on height-cropped text (audit §1 RIGHT):

```rust
// commit.rs — inside the per-entity loop, before the short-circuit.
// The reshape guard (Bug 2): extract asserts layout_runs().count() == computed.lines.len().
// A buffer unshaped after commit (a FontsGeneration sweep's set_metrics/attr-reset, a
// Display::None compute_hidden_layout escape) leaves layout_runs() short of the committed
// line count. Re-detect it with the SAME comparison extract makes, so the two cannot diverge.
let shape_stale = existing_layout.is_some_and(|computed| {
    access.with_buffer(|buffer| buffer.layout_runs().count() != computed.lines.len())
});
if !align_changed && !offset_stale && !size_stale && !shape_stale {
    continue;
}
```

`existing_layout.is_some_and` keeps the guard inert on the **first** commit of a buffer
(`existing_layout` is `None` until the first commit writes `ComputedTextLayout`), so the
guard adds **zero** work to the never-committed path and only an O(lines) `layout_runs().count()`
walk to **already-committed** entities — bounded, the same walk `computed_outputs` already does
(`commit.rs:156`). Steady-state cost is addressed in §3.4.

This is the cheap, general **last line of defense**: it catches *any* future unshape source
(not just the sweep), and with the Bug-3 fix in §2.1 the editor's content is intact when the
guard reshapes — so the guard reshapes the **real** content, the assert stays a true tripwire,
and silent-no-paint cannot occur.

### 2.3 The editor's content channel — the **existing** `Insert` / `SelectAll` + `Insert` (Bug 3 enabler)

Once content-lowering is suppressed for editors (§2.1), the editor needs an explicit
seed/programmatic-set channel (the display-`Text` seam is gone). **C2 does not add a new
`EditCommand` variant.** It uses the verbs that already exist, exactly as the **agent-interface
campaign** lowers `Action::SetValue`-text (which "lowers fine now via the EXISTING `SelectAll` +
`Insert`" — `action-router.md` §4, `phasing.md` P1c). The agent-interface campaign **owns** the
`EditCommand` surface and deliberately adds **no** value-set variant, so C2 matches that path:

- **Seed (construction-time).** A fresh editor is empty (`for_font_size` → `new()` seeds one
  empty line, so `value() == ""`). Seed initial content with **`EditCommand::Insert(initial)`**
  on the empty editor — one insert into an empty buffer makes the whole value.
- **Programmatic set.** Replace the whole value with **`EditCommand::SelectAll`** then
  **`EditCommand::Insert(new)`** (select-all + type-over). Both verbs exist today (`SelectAll` is
  the select-all arm; `Insert` is the literal-insert arm) and both flow through `apply_tracked`
  (`input.rs:94`), so each is recorded and IME-aware on its existing path.

This is the same select-all-then-insert lowering the agent-interface router applies for an
`Action::SetValue` on a text field, so the editor's programmatic-set behavior is identical
whether driven by an app, by C4's `TextField` lifecycle, or by an assistive-tech/agent
`Action::SetValue` — one path, no parallel verb.

Preedit note: `SelectAll` and `Insert` go through the existing edit path, which already handles
a live composition (the agent-interface router relies on this for `Action::SetValue`). C2 adds
no special composition handling — it inherits whatever the existing `SelectAll`/`Insert` arms do.
The §2.1 style-only path's preedit safety (§2.4) is independent of and unaffected by this content
channel: it holds because the **sweep** never `set_text`s, not because of any seed/set verb.

### 2.4 Preedit safety (the joint-fix's third leg)

A live IME preedit is spliced into the editor buffer as a metadata-marked span
(`ime.rs:36`, `PREEDIT_METADATA`; the span recorded in `TextEditState.preedit`,
`state.rs:119`). The §2.1 style-only path is **already preedit-safe by construction**: it never
calls `set_text`, so the spliced preedit bytes and the `PreeditSpan` record both survive a
`FontsGeneration` bump. The attr-refresh resets line shape (so the composing line reshapes at
the new font) but preserves the line *text* including the preedit run.

The pre-fix behavior was a **double** corruption on the editor class during composition: the
clobber `set_text("")` destroyed both the committed text **and** the live preedit span (a
mid-composition `set_text` destroys composition, not just committed text — audit §3,
Appendix-B.3). The §2.1 fix removes both. The seed/set channel (§2.3) reuses the **existing**
`Insert` / `SelectAll` + `Insert` verbs, whose composition handling already lives in the edit
path (the same path the agent-interface `Action::SetValue` lowering drives) — C2 adds no new
preedit-handling code there. C2's preedit guarantee is wholly the §2.1 style-only path: the
sweep never `set_text`s, so the spliced preedit and the `PreeditSpan` record survive a bump.

### 2.5 Surface (what ships)

| Change | File | Kind |
|---|---|---|
| `has_edit()` discriminant | `text/edit/access.rs` | new method on `TextBufferAccessItem` |
| `apply_authored_style_to_editor_buffer` + `refresh_line_default_attrs` + `style_block_flag` | `text/sync.rs` | new private fns; `sync_one` branches |
| `shape_stale` term | `text/commit.rs` | one added guard term |
| editor seed/set via existing `Insert` / `SelectAll` + `Insert` | `text/edit/input.rs` (C4 lifecycle calls) | **no new variant** — reuses existing verbs (agent-interface campaign owns the `EditCommand` surface) |
| `EditCommand` re-export (if not already prelude) | `text/edit/mod.rs`, `buiy/lib.rs` prelude | export (umbrella prelude gap, audit §4) |

No new `EditCommand` variant, no new components, no new resources, no schedule changes, no
stride/byte-format changes. (The whole-value-set verb is **not** added here: the agent-interface
campaign owns the `EditCommand` surface and lowers `Action::SetValue`-text via the existing
`SelectAll` + `Insert`; C2 seeds/sets the editor the same way — see "Coordination with the
agent-interface campaign".)

---

## 3. Decisions & rejected alternatives

### 3.1 Fix shape (the umbrella §2.6 open question) → **alt (a): content-skip + seed via existing `Insert` / `SelectAll` + `Insert`**, enriched with the style-only re-lower

The three candidates (audit §2 Bug 3, §8 item 6):

- **(a)** Skip content-lowering for `TextEditState` entities; still apply style; seed/programmatic-set
  via the existing `EditCommand` verbs (`Insert` for the empty-editor seed, `SelectAll` + `Insert`
  for a programmatic set).
- **(b)** Full content-sync vs style-sync split, editors opt out of content-sync.
- **(c)** Edge-triggered value-diff: cache the last-lowered content; re-`set_text` only when the
  display `Text` actually changed (the React controlled-`value` pattern).

**Chosen: (a)**, with the explicit observation that (a) and (b) *converge* in this codebase —
"skip content for editors" (a) **is** "editors opt out of content-sync" (b); the only real
difference is whether the seed/programmatic-set channel is a dedicated value-set verb or the
existing edit verbs. **The original C2 draft proposed a new `EditCommand::SetValue` here; that is
superseded** — the agent-interface campaign **owns** the `EditCommand` surface and deliberately
lowers `Action::SetValue`-text via the **existing** `SelectAll` + `Insert` (`action-router.md` §4,
`phasing.md` P1c), adding no value-set variant. C2 seeds (`Insert` into the empty editor) and sets
(`SelectAll` + `Insert`) the same way — the minimal form, and the one that keeps the editor's
programmatic-set behavior identical across app / C4 / assistive-tech callers.

**Decisive rationale — (c) has nothing to diff against.** The edge-triggered value-diff (c)
caches the last-lowered `Text` and re-`set_text`s only on `Text` change. But for an editor,
**`Text` is permanently `""`** (§1.1): typed content goes into the editor buffer via
`apply_tracked` (`input.rs:94`), never into `Text`. So (c)'s cache would forever read
`""`, the diff would forever be "unchanged after the first frame," and — critically — (c)
still requires the cache to *update on user edits* or the next genuine `Text` change re-clobbers.
But user edits don't touch `Text`, so the cache cannot observe them: **(c) is structurally
unable to track the editor's real content** under this architecture. (c) is a clean pattern for
a *controlled* widget where the app owns `value` and writes it through `Text`/a prop — which is
not how Buiy editors work (the editor owns content; `Text` is a measurement carrier). Adopting
(c) would require *also* making `Text` the editor's content authority, which re-introduces the
two-source-of-truth the editor-owned-buffer design (`state.rs:79-85`) deliberately avoids.
**Rejected: (c) solves a different architecture's problem.**

**Why not (b) as stated** — "full content-sync vs style-sync split, editors opt out": (b) is
the right *shape* but under-specifies the seed. (a) is (b) plus a **named seed channel** — the
existing `Insert` / `SelectAll` + `Insert` verbs (§2.3), not a new one. The audit itself notes
(b)'s open hole: "cosmic has no 'set attrs without text' on a whole buffer — style-only attr
changes may need a content-preserving reshape reading the editor value" (audit §3 alt (b)).
§2.1's `refresh_line_default_attrs` resolves exactly that hole via per-line
`AttrsList::defaults()` surgery (the `ime.rs:233-249` technique), which is why (a)-with-style-
re-lower is strictly more complete than bare (b).

**Why (a) preserves the four constraints the §2.6 framing names:**

1. *Preserves the editor's ONLY style/seed channel.* TextSync remains the sole writer of
   wrap/tab-width/attrs via the style-only path (§2.1); the editor's content seed is the existing
   `Insert` / `SelectAll` + `Insert` (§2.3). Commit still owns only size/align
   (`commit.rs:89-119`) — unchanged.
2. *IME-preedit safety.* The style-only path never `set_text`s, so the spliced preedit + the
   `PreeditSpan` record survive (§2.4) — independent of any seed/set verb. The seed/set channel
   reuses the existing `Insert`/`SelectAll` arms, whose composition handling already lives in the
   edit path (the agent-interface `Action::SetValue` lowering drives the same path).
3. *Blast radius.* Smallest of the durable options: one `sync_one` branch, one new private fn
   pair, one guard term — and **no new `EditCommand` variant or arm** (the seed/set reuses
   existing verbs). No component/resource/schedule churn. (§5.)
4. *Test isolation.* The content-skip is directly assertable headlessly: bump `FontsGeneration`
   over an editor holding typed content (seeded via `Insert`), assert `value()` survives AND the
   buffer reshaped (§6). (c) and a snapshot/restore hack are not as crisply isolable.

### 3.2 `text_commit` guard locus → **commit-side `shape_stale` re-detect (kept), NOT a TextSync damage flag**

The audit offers commit-side re-detect vs a TextSync damage flag set at the `set_text` site
(audit §2 Bug 2, §8 item 7, §9 open-Q 5).

**Chosen: commit-side re-detect** (§2.2). It is *general* — it catches **any** unshape source,
not just the sweep (the `Display::None`/`compute_hidden_layout` escape the audit flags as the
genuinely-correct addition, audit §2 Bug 2; a future overflow/virtualization pass; a third-party
mutator). A damage flag set only at TextSync's `set_text` would miss every non-TextSync unshape.
The guard compares the **exact** pair extract compares (`layout_runs().count()` vs
`computed.lines.len()`), so it is sound by construction and cannot diverge from extract's truth
(audit §1 RIGHT). **Runner-up — TextSync damage flag:** O(1) per frame and points at one root
cause, but it is *narrow* (one writer) and adds a third shape-decision site (a lock-discipline
cost the commit.rs module doc warns against, `commit.rs:1-11`). Rejected as both less safe and
not actually cheaper once you account for the misses it would require additional guards to cover.

The §2.1 Bug-3 fix is the *root-cause* fix for the **sweep** specifically; §2.2 is the
defense-in-depth net for the class. They are complementary, not redundant — which is the whole
"fix both together" thesis (§2.6).

### 3.3 Release-only self-heal in extract → **deferred (NO), keep the `debug_assert` tripwire**

The audit raises a release-only reshape-on-mismatch in extract (audit §2 Bug 2, §8 item 7).
**Rejected for v1.** Extract is the main-world read-only producer (`extract.rs` runs in the
`Extract` schedule; the `with_buffer` accessor is `&self`, `access.rs:114-124`); reshaping there
violates the read-only contract (architecture §4.4) and needs a `FontSystem` lock at extract,
which the producer deliberately avoids. With §2.1 (no clobber) + §2.2 (re-detect → reshape at
the commit lock site, the correct phase), a buffer **cannot** reach extract unshaped in steady
operation, so the self-heal guards an unreachable state. Keep the `debug_assert` (`extract.rs:712`)
as the developer tripwire; if a future unshape source slips past commit, the assert catches it in
debug and the dev adds the commit-side term — the right escalation path. **Deferred, not
rejected forever:** if a real release-time unshape class is ever found that commit cannot
re-detect (none known), revisit.

### 3.4 Steady-state cost of `shape_stale` → **bounded, gated on `existing_layout.is_some`, pinned by C2 (Task 7)**

The audit flags the `layout_runs().count()` walk as unmeasured per-frame O(lines) cost (audit
§1 counterpoints, §7). Mitigations baked into §2.2: the term is `existing_layout.is_some_and(…)`
so it is **skipped entirely** until the first commit; it is the *same* walk `computed_outputs`
already runs on any reshape (`commit.rs:156`); and on a settled multi-line editor it is bounded
by line count, not glyph count. The steady-state-cost pin is **C2-owned** (Task 7 of the C2
plan, NOT C7): C2's plan extends `steady_state_zero_measure_calls_and_zero_reshapes`
(`text_commit.rs:230`) — already asserting zero reshapes on a no-change frame — to document that
`shape_stale` *walks* `layout_runs().count()` but does not *trigger* a reshape in steady state
(the walk runs, but `layout_runs().count() == lines.len()` holds, so the short-circuit still
fires and no lock is taken). No `TextCommitReshapeCount` regression on a steady frame is the
gate, and it lives in C2's domain (it tests `commit.rs`, needs no harness). C7 owns the Tier-B
*font-reload survival* suite (§6) but does **not** own this steady-state-cost test — earlier
drafts mis-attributed it to C7; C7's plan has no such benchmark, and C2 Task 7 is the home for
it. **Decision: accept the bounded per-already-committed-entity walk**; if a text-churn
benchmark later shows it material, fold detection into a per-entity dirty bit — not v1.

### 3.5 Seed/set undo behavior → **the existing `Insert` / `SelectAll` + `Insert` recording (no new verb)**

A programmatic value set should be undoable (controlled-input parity; the audit's "controlled
input" framing, §3 alt (c)). Because the seed/set channel is the **existing** `Insert` /
`SelectAll` + `Insert` verbs (§2.3) — not a new `SetValue` — the undo behavior is whatever those
verbs already record on the `apply_tracked` path: a `SelectAll` + `Insert` programmatic set is the
ordinary select-then-type undo behavior the editor already provides, and the same the
agent-interface `Action::SetValue` lowering relies on. C2 adds **no new undo grouping** and **no
new verb** to make this happen. (The original C2 draft proposed a dedicated, recorded
`EditCommand::SetValue` here; that is superseded — adding the verb is the agent-interface
campaign's call on the surface it owns, and that campaign chose not to.) The construction-time
*seed* is a single `Insert` into an empty editor, applied before any user interaction, so its
undo entry is harmless. **Runner-up — a dedicated recorded `SetValue` verb:** one uniform call
site, but it adds a variant to an `EditCommand` surface the agent-interface campaign owns and has
deliberately kept value-set-free. Rejected to avoid a competing verb; the existing
`SelectAll` + `Insert` is sufficient and shared.

---

## 4. Contracts & interfaces

### 4.1 Shared contracts referenced (umbrella §6 — not redefined here)

- **Event vocabulary (umbrella §6.9):** C2 defines no `EditCommand` verb and no cross-widget
  event. The editor's seed/set uses the existing `Insert` / `SelectAll` + `Insert` (editor
  commands, not cross-widget events); they do not touch `Activate`/`ValueChange<T>`/`Set<X>`
  (those are C3/C4). C4's controlled-`TextField` `ValueChange<String>` emission *consumes* the
  `TextChanged` message this fix preserves — C2 keeps the existing `TextChanged` (`input.rs:22`)
  honest by ensuring a sweep no longer silently changes the value.
- **`EditCommand` surface (agent-interface-owned, umbrella §2.7):** the `EditCommand` enum is
  owned by the agent-interface campaign (it adds `SetSelection` in P1c and lowers
  `Action::SetValue`-text via the existing `SelectAll` + `Insert`). C2 neither adds nor renames a
  variant; it consumes the existing `Insert`/`SelectAll` verbs for the editor seed/set.
- **Verification discipline (umbrella §5 Wave 1, §6 implied):** the Tier-B tests land **RED-first**
  (umbrella §5: "Tier B is C2's content-survival gate … Lands RED-first"). C7 owns the harness
  conventions; C2 owns the test *content* (the assertions in §6).
- **Coordinate space / picking / styling (umbrella §6.1, §6.2, §6.7):** untouched — C2 is purely
  in the text measure/commit/edit path; it shares no contract with C1/C3/C6.

### 4.2 Own contracts (defined here)

- **C2-CONTRACT-1 (content survival):** a `FontsGeneration` bump (from the periodic system-font
  scan OR a runtime `apply_font_registry` `add_font` batch, `registry.rs:543`) MUST NOT change
  the logical `value()` of any `TextEditState` entity. TextSync MUST NOT `set_text` an
  editor-owned buffer.
- **C2-CONTRACT-2 (style survival):** the SAME bump MUST re-apply the editor buffer's
  `metrics`/`wrap`/`tab_width`/default-attrs from the authored style components (TextSync remains
  the sole writer of these on the editor buffer).
- **C2-CONTRACT-3 (reshape guard):** no buffer with an `existing` `ComputedTextLayout` may reach
  extract with `layout_runs().count() != computed.lines.len()`; `text_commit` re-detects and
  reshapes the mismatch at its lock site.
- **C2-CONTRACT-4 (preedit survival):** a live IME preedit (`TextEditState.preedit` + its
  spliced span) MUST survive a `FontsGeneration` bump unchanged. This holds because the §2.1
  style-only path never `set_text`s the editor buffer — it is independent of the seed/set channel.
- **C2-CONTRACT-5 (seed/set channel):** the editor's content seed and programmatic-set channel is
  the **existing** `EditCommand` verb pair — `Insert(initial)` into an empty editor for the seed,
  `SelectAll` + `Insert(new)` for a programmatic set. C2 adds **no** new `EditCommand` variant
  (the `EditCommand` surface is agent-interface-owned, umbrella §2.7); these verbs go through the
  existing recorded, IME-aware `apply_tracked` path.

---

## 5. Migration / build steps (ordered; blast radius)

1. **Confirm the editor seed/set channel is the existing `Insert` / `SelectAll` + `Insert`** — no
   new `EditCommand` variant is added (the `EditCommand` surface is agent-interface-owned, umbrella
   §2.7; `Action::SetValue`-text lowers via the existing `SelectAll` + `Insert`). *Blast:* none in
   the `EditCommand` enum. (Re-export `EditCommand`/`TextChanged` through the `buiy` prelude per
   the audit §4 prelude gap remains worthwhile — it surfaces the existing verbs apps and C4 call.)
2. **Add `has_edit()` to `TextBufferAccessItem`** (`access.rs`). *Blast:* one method; no callers
   change except `sync_one`.
3. **Split `sync_one` content/style** (`sync.rs`): add `apply_authored_style_to_editor_buffer`,
   `refresh_line_default_attrs`, `style_block_flag`; branch on `access.has_edit()`. *Blast:*
   `sync.rs` only; `apply_authored_to_buffer` (display path) unchanged.
4. **Add the `shape_stale` guard** (`commit.rs:98-104`). *Blast:* one term in the short-circuit;
   `computed_outputs` unchanged.
5. **Update the editor lifecycle to seed via `EditCommand::Insert(initial)`** (and programmatic set
   via `SelectAll` + `Insert`) if/where a construction-time initial value is supported (coordinate
   with C4's `TextField` controlled-value seed; the bare `TextInput` seeds `""`, so no behavior
   change for existing `hello_text`/widgets).

**Tests/snapshots touched (blast radius):**
- **C2-owned (the `shape_stale` isolating proof):** `crates/buiy_core/tests/text_commit.rs`
  gains `shape_stale_reshapes_a_committed_but_unshaped_buffer` — the directed RED-first unit test
  that constructs a committed-but-unshaped buffer (`reset_shaping()`, no bump, no `Text` edit) and
  asserts `text_commit` reshapes it via `shape_stale` alone (§6). This is C2's domain (tests
  `commit.rs`, no harness), and it is the **non-vacuous** proof of the guard.
- **C7-owned (Tier-B regression, belt-and-suspenders):** `crates/buiy_core/tests/text_font_reload_survival.rs`
  — content-survival, label-reshape/glyph-count, empty-editor 0-vs-1, preedit-survival,
  style-survival. C7 creates the file + the `bump_fonts_generation` harness method and lands the
  survival/preedit arms `#[ignore]`-RED; C2 deletes the `#[ignore]` (un-ignore) and asserts GREEN.
  RED-first. These prove the production `FontsGeneration`-bump path keeps painting content after
  the fix, but they **auto-heal** via the sweep's `mark_dirty` → Taffy re-measure, so they are NOT
  the isolating proof of `shape_stale` (the C2 directed test above is) — they are regression guards.
- **Extended (C2-owned, Task 7):** `text_commit.rs::steady_state_zero_measure_calls_and_zero_reshapes`
  (assert `shape_stale` does not trigger a reshape in steady state); `text_sync.rs` may gain an
  editor-arm sweep assertion (content survives) mirroring `fonts_generation_bump_sweeps_every_buffer`.
- **No golden re-bless.** C2 touches no render output format, no a11y wire format, no layout
  numbers in steady state. (The editor's content already painted from the editor buffer; the
  fix *prevents* its disappearance — existing goldens are unaffected, and a zero-glyph editor
  golden was never blessable; C7's bless-guard formalizes that.)
- `text_clipboard_undo.rs` may gain a `SelectAll` + `Insert` programmatic-set undo assertion
  (the existing-verb seed/set behavior, not a new `SetValue` verb).

**Sequencing note (umbrella §8 gate):** the first plan step rebases onto the then-current
`origin/main` and re-confirms every `file:line` anchor here (the prototype diffs are stale-base
and re-derived, never cherry-picked).

---

## 6. Verification (how C7 gates this; RED-first)

C7 Tier-B (umbrella §4 C7, §5 Wave 1) is **co-delivered with this child**. It builds on the
**adapterless** substrate that already exists — there is no winit, no GPU, no real async loader:

- `crates/buiy_core/tests/support/extract_harness.rs` (`TextExtractHarness`) drives the
  production `extract_buiy_glyphs` with no adapter and exposes `glyph_count()` — the substrate
  for the reshape/glyph-count assertions.
- `crates/buiy_core/tests/text_commit.rs` / `text_sync.rs` use `MinimalPlugins + CorePlugin +
  LayoutPlugin + BuiyTextPlugin` and bump `FontsGeneration` directly
  (`app.world_mut().resource_mut::<FontsGeneration>().0 += 1`) — the deterministic injection of
  the Bug-2/3 trigger **with zero winit** (resolves audit open-Q #7: the bump *can* be injected
  headlessly).

The C7-owned suite `text_font_reload_survival.rs` spawns a **real editor** (`TextEditState`
on a measuring `Node`/`Style`; the editor-owned buffer is authoritative), seeds typed content via
`EditCommand::Insert` / `apply` (the production edit path), settles, then asserts each
contract. Every predicate is proven **RED before GREEN** (the harness mandate; a vacuous green
is the worst defect in a verifier):

- **Content survival (C2-CONTRACT-1) — proves Bug 3 fixed.** Editor holds `"hello"` (typed via
  the edit path, so it is in the editor buffer, NOT in `Text`). Bump `FontsGeneration`. Assert
  `state.value() == "hello"`. **RED-proof:** revert §2.1 (let `sync_one` `set_text` the editor
  buffer) → `value()` becomes `""` → test fails. *This is the test the audit says is entirely
  missing.*
- **Reshape guard (C2-CONTRACT-3) — proves Bug 2 fixed, ISOLATED.** The isolating proof is a
  **C2-owned directed unit test** in `crates/buiy_core/tests/text_commit.rs`
  (`shape_stale_reshapes_a_committed_but_unshaped_buffer`), NOT the C7 font-reload suite. It
  constructs the committed-but-unshaped state **directly** — settle a real text node, then
  `buffer.lines[0].reset_shaping()` (cosmic `buffer_line.rs:203`, with `buffer.size()`/align/
  content-offset unchanged) — with **no** `FontsGeneration` bump and **no** `Text`/style edit, so
  the `text_sync_buffers` sweep never runs (its triggers are `fonts_generation.is_changed()` or
  the `Or<(Changed<Text>, …)>` set, `sync.rs:69,251`; `Changed<TextBuffer>` is not a trigger) and
  `tree.mark_dirty_for_entity` (`sync.rs:350`) is never called — so Taffy never re-measures and
  the auto-heal is removed. On the next `app.update()` the only system that can reshape the buffer
  is `text_commit`, and the only term that can fire is `shape_stale` (size/align/offset equal).
  Assert `layout_runs().count() == computed.lines.len()` AND `TextCommitReshapeCount == 1`.
  **RED-proof:** without `shape_stale`, `text_commit` short-circuits → the buffer stays unshaped →
  `layout_runs().count() == 0` (and `TextCommitReshapeCount == 0`) → the test fails. The flip RED→
  GREEN is attributable to `shape_stale` alone (the other three terms are equal across both runs).
  *Why this is the proof and not the end-to-end path:* the `FontsGeneration`-bump survival tests
  (C7 Tier-B, below) **auto-heal** via the sweep's `mark_dirty` → Taffy re-measure, so they are
  GREEN with or without the guard term — they cannot isolate it (audit §2 Bug 2, Appendix-A.5).
  The C7 end-to-end `label_reshapes_…`/`editor_style_stays_live_…` arms stay as **belt-and-
  suspenders** regression guards that the production bump path keeps painting content after the fix.
- **Empty-editor 0-vs-1 (C2-CONTRACT-3 edge).** A *fresh empty* editor: bump, commit, extract;
  assert `glyph_count()==0` AND no `debug_assert` panic (the synthetic empty `LayoutLine` yields
  `runs==computed.lines.len()==1`, `commit.rs:139-148`). This is the arm that *looked* complete
  pre-fix (the clobber was a no-op on empty) — it must stay green and prove the guard does not
  false-positive on the empty-line synthetic-run case.
- **Preedit survival (C2-CONTRACT-4) — proves preedit-aware.** Editor holds `"abc"`; splice a
  live preedit `"み"` (`splice_preedit`, `ime.rs:115`); bump `FontsGeneration`; assert
  `state.has_preedit()`, `state.preedit_span()` unchanged, `state.value() == "abc"` (preedit
  excluded), and `buffer_text_for_test()` still contains the preedit run. **RED-proof:** with the
  pre-fix clobber, `set_text("")` destroys both the preedit span and `"abc"` → test fails on
  every assertion.
- **Style survival (C2-CONTRACT-2).** Editor holds content; change `FontSize` (or register a
  font, bumping the generation); assert the editor buffer's `metrics`/`wrap` updated AND
  `value()` unchanged. **RED-proof:** a naive blanket "editors skip TextSync entirely" → metrics
  do not update → test fails (guards against over-correcting Bug 3).

**Content-presence + bless-guard (C7's W19, referenced not owned here):** the editor's label
fixture must emit `glyph_count() > 0` for non-empty content and the bless-guard must refuse a
zero-glyph golden of a non-empty editor — the exact silent-no-paint hole Bug 2 opened. C2's
content-survival + reshape tests are the unit-level proof; C7's invariant is the
coverage-by-construction enrollment. They are complementary; C2 does not redefine the bless-guard.

**Steady-state regression (C2-CONTRACT-3 cost) — C2-owned (Task 7):** extend
`text_commit.rs::steady_state_zero_measure_calls_and_zero_reshapes` (`text_commit.rs:230`) to
assert `TextCommitReshapeCount == 0` on a no-change frame *with the `shape_stale` term present*
(the walk runs but never triggers a reshape). Guards §3.4's cost claim. This pin lives in C2's
plan Task 7 (it tests `commit.rs`, needs no harness) — **not** C7; C7 owns only the Tier-B
font-reload survival suite above.

---

## 7. Open questions deferred + dependencies

**Resolved here (do not re-litigate):** fix shape = (a) content-skip + seed via the existing
`Insert` / `SelectAll` + `Insert` + style-only re-lower (§3.1, **no new `EditCommand::SetValue`** —
the `EditCommand` surface is agent-interface-owned, umbrella §2.7); guard locus = commit-side
`shape_stale` re-detect (§3.2); release-only self-heal = deferred/no (§3.3); seed/set undo behavior
= the existing-verb recording (§3.5).

**Deferred (genuinely depend on un-built work):**

- **`refresh_line_default_attrs` exact cosmic surgery.** The per-line `AttrsList::defaults()`
  rewrite + shape-reset is sketched against the `ime.rs:233-249` precedent, but the exact cosmic
  0.19 call sequence to reset a `BufferLine`'s shape cache *without* changing its text is an
  implementation detail to confirm against the cosmic source at plan time (the first plan step's
  anchor re-confirm). If cosmic offers no clean "reset shape, keep text," the fallback is a
  content-preserving `set_text(line.text(), new_attrs)` that reads the *editor* value (audit §3
  alt (b)'s noted hole) — still no clobber, since it writes the buffer's own current text back.
- **Whether the style-only path must also handle a per-span (rich-text) editor.** v1 editors are
  single default-attrs runs (the `set_text` path, `sync.rs:530-538`); a future rich-text editor
  with mixed spans would need per-span attr refresh. Out of scope for the catalog (TextField is
  uniform style); flagged for the editor's rich-text follow-up. Does not block C2.
- **C4 coordination on the construction-time seed.** Whether `TextField`'s controlled-`value`
  prop seeds via `EditCommand::Insert` at spawn (and programmatic-set via `SelectAll` + `Insert`)
  or via a C4-owned lifecycle system is C4's call; C2 only guarantees those existing verbs are the
  correct seed/set channel (matching the agent-interface `Action::SetValue` lowering). (umbrella
  §6.9 event vocabulary is C4's.)

**Dependencies:**
- **C0** (umbrella) — anchors §2.6 (joint, preedit-aware framing) and the Wave-1 RED-first gate.
- **C7 Tier-B** — co-delivered; owns the harness conventions, the content-presence invariant,
  and the bless-guard. C2 owns the test *content* (§6). The two land in the same wave.
- **No dependency on C1/C3/C6** — C2 is isolated in the text path; it shares no coordinate,
  picking, or styling contract.
- **C4 consumes** C2 (the editor seed via the existing `Insert` / `SelectAll` + `Insert` + the
  preserved `TextChanged`) but does not block it.
- **Agent-interface campaign** (umbrella §2.7) **owns** the `EditCommand` surface (adds
  `SetSelection`; lowers `Action::SetValue`-text via the existing `SelectAll` + `Insert`). C2
  consumes the existing `EditCommand` verbs and adds none — see the Coordination section below.

---

## 8. Coordination with the agent-interface campaign

Per umbrella §2.7, the **agent-interface campaign**
(`docs/specs/2026-06-18-buiy-agent-interface-design/`; landed P0 on `main`, phasing P1a→P1d)
**owns the `EditCommand` surface** and the inbound action router. This child (C2) reconciles to
that ownership and was edited (2026-06-22) to drop its originally-proposed `EditCommand::SetValue`.

**C2 owns (unaffected by the agent-interface campaign — these are text-pipeline bugs):**
- The **Bug 3** TextSync editor-content-clobber fix: `sync_one` branches on
  `TextBufferAccessItem::has_edit()`; editor entities take a **style-only** re-lower
  (`apply_authored_style_to_editor_buffer`) that re-applies metrics/wrap/tab-width/default-attrs
  but **never `set_text`s** (§2.1).
- The **Bug 2** `text_commit` **`shape_stale`** guard term (§2.2).
- **Preedit survival** across a `FontsGeneration` bump (a consequence of the §2.1 style-only path,
  §2.4), and the **directed `shape_stale` isolation test** + the co-delivered C7 Tier-B
  survival/preedit/style arms (§6). None of this substance changes.

**C2 consumes (owned/extended by the agent-interface campaign — C2 does not redefine):**
- The **`EditCommand` enum**: the agent-interface campaign adds `EditCommand::SetSelection` (P1c)
  and lowers `Action::SetValue`-text via the **existing** `SelectAll` + `Insert` (it adds **no**
  value-set variant — `action-router.md` §4, `phasing.md` P1c). C2 therefore **does not add
  `EditCommand::SetValue`**: it seeds the empty editor with the existing `Insert(initial)` and does
  a programmatic set with `SelectAll` + `Insert(new)` (§2.3) — the same path the router uses, so
  the editor's set behavior is identical across app / C4 / assistive-tech callers.
- The **inbound action router** + the `EditCommand::SetSelection` variant are **not** defined here.

**Net change from the original C2 draft:** removed the `EditCommand::SetValue(String)` variant and
its `apply_tracked` arm; replaced the seed/programmatic-set channel with the existing
`Insert` / `SelectAll` + `Insert` verbs. The Bug-2/Bug-3 fix substance, the contracts (C2-CONTRACT-1
through C2-CONTRACT-5, with -5 restated as the existing-verb channel), and the verification §6 are
otherwise intact.
