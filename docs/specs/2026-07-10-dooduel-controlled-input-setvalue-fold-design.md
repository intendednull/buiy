# Dooduel — a controlled `text_input` clobbers an AT `SetValue` on a rebuilding screen

- **Date:** 2026-07-10
- **Status:** landed — fix (b) applied: a `PendingProgrammaticEdit` marker
  (`crates/buiy_core/src/text/edit/input.rs`) set by `honor_text_set_value`
  (`crates/buiy_core/src/a11y/contract.rs`), read by the controlled reconcile to skip its
  clobber (`crates/buiy_view/src/reconcile.rs`), cleared by `route_text_input`
  (`crates/buiy_view/src/router.rs`). Regression test
  `crates/buiy_view/tests/controlled_input_rebuild_clobber.rs`.
- **Area:** buiy_view + buiy_core a11y/text — framework
- **Found by:** the QA-seat driver (Track 3 of the playtest cycle) — the in-game chat
  field; a `set_value`'d guess never submits and `wait_value` times out on `[value=""]`.

## The bug

`buiy::probe::set_value` (any assistive-tech `Action::SetValue`) into a **controlled**
`text_input` on a screen that **rebuilds every frame** does not survive: the edit is
clobbered before it folds into the MVU model. The SAME driver's `set_value` folds fine on
the static Join screen. Correlated (but SEPARATE — see below): the in-game chat placeholder
renders stale.

## Proven root cause (an ordering hazard, empirically reproduced)

The probe writes the editor buffer and emits `TextChanged` **out of band** — a direct
`&mut World` `dispatch_action_request` between `app.update()`s (`qa_seat.rs:318-320`:
`set_value(world, node, text); app.update();`). `honor_text_set_value`
(`crates/buiy_core/src/a11y/contract.rs:497-531`) applies the edit and writes `TextChanged`
(the #98 fix) but does **not** touch the model. So a pending `TextChanged` sits across the
frame boundary.

Within the next frame the schedule is:

1. **`ViewSet::Reconcile`** (`app.rs:149-150`, `.before(BuiySet::Layout)` — front of frame).
   Gated on `Changed<M>` (`reconcile.rs:159-174`). For a controlled `text_input` it calls
   `set_editor_value(editor, model.value)` unconditionally, drift-only vs the **editor**
   (`reconcile.rs:271`, `1552-1572`).
2. …`BuiySet::Input` (the winit-channel `route_action_requests` — empty here, the probe used
   the direct seam)…
3. **`route_text_input`** (`router.rs:80-91`) in `MvuSet::Enqueue`
   (`app.rs:140-148`, `.after(BuiySet::A11yUpdate).before(BuiySet::Render)` — late): reads the
   pending `TextChanged`, reads `editor.value()`, `enqueue`s `on_input(value)`.
4. `MvuSet::Drain`: the reducer folds it into `model.value`.

The reconcile (step 1) runs **before** `route_text_input` (step 3). On a screen whose model
changed for an unrelated reason (the countdown → `Changed<M>` every frame), the reconcile
**runs** on the settle frame, reads the still-stale `model.value == ""`, and clobbers the
editor's `"balloon"` → `""` **before** step 3 reads the editor. Step 3 then folds the
clobbered `""`; the edit is permanently lost (nothing re-emits it, subsequent reconciles
no-op at `"" == ""`).

**Why Join folds but in-game does not — the `Changed<M>` gate.** The Join model is *static*:
on the settle frame `Changed<M>` is empty, so the reconcile **early-outs** and never clobbers;
`route_text_input` reads the intact editor and folds the real value, and the reconcile only
runs the FOLLOWING frame, after the fold, when editor and model already agree. The in-game
model changes every frame (countdown), so its reconcile runs on the settle frame and clobbers.
The keyboard path is immune on both: a keystroke edits the editor in `BuiySet::Input` (AFTER
the reconcile, same frame) and `route_text_input` folds it that same frame — there is no
cross-boundary pending edit for a front-of-frame reconcile to race.

The #98 test (`a11y_set_value_route.rs`) passes precisely because its model is static (one
reconcile *after* the fold, never a clobbering one *before*). RED repro:
`controlled_input_rebuild_clobber.rs` — a `Tick` enqueued every frame; `draft` stays `""`.

## Candidate fixes

**(a) Ordering — fold before the reconcile.** Run `route_text_input` + the drain before
`set_editor_value` so the model is current when the reconcile re-asserts. *Rejected:* the
reconcile is deliberately `.before(Layout)` (no unlaid-out-flash, design #10) and the fold is
in the late `MvuSet` chain (`.after(A11yUpdate)`); making the fold precede the reconcile means
moving the whole MVU Enqueue→Drain chain before Layout — a framework-wide schedule change that
ripples through the early-leaf model and the a11y-before-drain contract. Too broad for this
bug (it is also a same-frame cycle: reconcile wants the model *after* the fold, the fold wants
the editor written *before* it).

**(b, CHOSEN) Clobber-guard — a "pending programmatic edit" marker.**
`honor_text_set_value` sets a `PendingProgrammaticEdit` marker on the editor when it emits
`TextChanged` (the out-of-band AT/probe path only — the keyboard path never sets it). The
reconcile **skips** `set_editor_value` while the marker is present; `route_text_input` **clears**
it for every `TextChanged` it drains (folding when an `InputAction` is present, clearing
regardless otherwise so no input can leave the marker stuck). This suppresses exactly the one
clobber: on the settle frame the reconcile leaves the un-folded editor intact, `route_text_input`
reads it and folds the real value, and the next frame (marker cleared) the ordinary drift-only
re-assert resumes.

**(c) Fold-at-source — `honor` drives the model synchronously.** *Rejected:* `honor` lives in
`buiy_core::a11y`, is model-agnostic, and has no access to `buiy_view`'s `InputAction<M>`
binding or the reducer `M`. It physically cannot fold into the model without a layering
inversion (core reaching up into the view surface). Not clean.

**(d) Prop-change push — re-assert only when the controlled value changed.** Gate the
model→editor push on `el.value` differing from the last-pushed value (a per-entity
`LastControlledValue`), the Elm/React "push on prop change" semantic. *Rejected — it fails a
real, tested pattern.* `on_submit_with` (`on_submit_with.rs`) builds `text_input(String::new())`
with **no** `on_input`: its controlled value is a **constant** `""`, and it deliberately relies
on the reconcile re-asserting that empty value **after a submit** to clear the editor for the
next guess. Under (d) `el.value` never "changes", so the clear never fires and the field keeps
the just-submitted text. (d) conflates "the prop is unchanged" with "leave the editor alone",
but the discriminator must instead be "is there an **in-flight un-folded** edit" — an
already-consumed editor (post-submit) must still be cleared. Only (b) draws that line.

### Why (b)

It fixes the *cause* precisely: the reconcile must not re-assert over an edit that is en route to
the model but has not yet folded. The marker is the exact signal for that (set at the
programmatic write, cleared at the fold), so keyboard typing, the post-submit clear, external
model pushes, and a transforming `on_input` all keep working — only the one racing clobber is
suppressed. The **silent seam stays**: `set_editor_value` still must NOT emit `TextChanged`
(its low-level `apply` is deliberately invisible to the bridges to avoid a fold feedback-loop /
log flood); (b) does not change that — it only skips *calling* it for the pending frame. #98
(`a11y_set_value_route.rs`) and the emit-count semantics (`a11y_action.rs`) are untouched.

**Cost / tradeoff recorded.** The marker spans crates (set in `buiy_core::a11y`, read in the
`buiy_view` reconcile, cleared in the `buiy_view` router). This is justified: it is the
editor↔bridge handshake, and `buiy_view` already builds directly on `buiy_core`'s editor
internals (`TextEditState`, `TextChanged`). The stuck-marker edge (an AT set on an input with no
`on_input`) is handled by clearing on *every* drained `TextChanged`, fold or not.

## Scope

- **In scope:** the `PendingProgrammaticEdit` marker (set in `honor_text_set_value`, read by the
  reconcile's `Kind::TextInput` push, cleared in `route_text_input`). RED→GREEN:
  `controlled_input_rebuild_clobber.rs`.
- **Separate root cause (fixed in a companion commit):** the stale in-game placeholder is NOT
  this timing bug. The reconcile's `Kind::TextInput` **patch** branch patched only the value +
  handlers; it never re-patched the placeholder, which was seeded once at spawn. So a phase
  change (`Phase::Picking` → `Drawing`) never updated the placeholder text. Fixed with a
  drift-only `set_placeholder` patch (`Placeholder` `set_if_neq`, so the existing
  `sync_placeholder` reshape + `A11yPlaceholder` mirror pick it up) in that branch. RED→GREEN:
  `controlled_placeholder_patch.rs`. Distinct commit so the two root causes stay reviewable
  apart.
