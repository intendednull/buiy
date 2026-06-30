# RD3 — Replay-guarantee scope: structural-ops-on-log + keyed Subscription

**Decision: both structural-ops-on-log and the keyed Subscription are correctly
ROADMAP (neither built; neither needed for the proven v1 state-replay killer use
case). But the FINAL spec should promise a narrower, architecturally-grounded
structural guarantee — structure DERIVED as a deterministic keyed-reconcile of
on-log Model state DOES replay — which needs one concrete fix the prototype lacks:
rebuild the LogicalId resolver after structural change instead of computing it once
up-front.**

Confidence: **high**.

---

## 1. Structural ops = ROADMAP, split into two cases; promise only the first

- **(a) DERIVED structure** — entities spawned by a keyed-reconcile system reading
  an on-log Model's collection field — IS replay-covered **in principle**. Replaying
  the Model's folds reconstructs the collection field, and the same reconcile system
  re-runs deterministically in the replay app (`replay_into` already calls
  `app.update()` after each fold — prototype `replay.rs:166`). This is the
  relm4/Druid-validated shape (keyed-reconcile by stable DOMAIN id, never position).
- **(b) IMPERATIVE off-Model spawn/despawn** (the W4 test's raw `world.spawn`) is
  OFF-LOG and explicitly outside the boundary.

Pick the **DERIVE** path over recording raw spawn/despawn entries: recording raw
Entity-bearing structural ops reintroduces the Entity-vs-LogicalId portability
problem the LogicalId log exists to avoid, and DERIVE composes with the Machine
tier already being built.

### CRITICAL caveat the spec must own — case (a) does NOT work in the ported prototype as-is

`replay.rs:149` computes the `LogicalId → Entity` resolver **ONCE** before the loop;
`logical_id_index` (`replay.rs:177`) queries only entities already present in the
fresh seed app. So a fold targeting a replay-spawned child **dead-letters** (silent
`continue` skip, `replay.rs:152-153`) because the resolver is stale. **The FINAL's
replay loop must refresh the resolver after each structural change** (or resolve
live per-entry via a `Query`).

The failing-in-the-flesh structural-gap test confirms the boundary: an off-funnel
`world.spawn((Checkbox, LogicalId(999)))` produces ZERO log entries and is NOT
recreated by replay, while the on-log toggle fold replays — prototype
`examples/buiy_gallery/tests/mvu_whole_ui_replay.rs:459-528`.

---

## 2. Subscription = DEFERRABLE for v1; spec the minimal shape now

NOT needed for the W4 killer use case (clicks + keyboard + IME + paste all already
enter the funnel / the editor apply-tap; IME captured at the apply site with its
payload + `now: Duration`, `record.rs:213-222,287`). It becomes REQUIRED only when a
timer/OS/async source drives **MODEL** state rather than pure visuals — caret-blink
is purely render-prep (mutates no `TextEditState` field), so its being off-log does
not break byte-identical STATE replay, which is why v1 can ship without it.

**Minimal replay-safe shape (Iced-validated):** a keyed Subscription identified by a
stable hash/key; the runtime diffs the active sub-set each frame from the owning
Model, starts new keys, drops vanished keys (drop = cancel, the existing
despawn/InFlight discipline); every emission flows through the SAME enqueue→drain
funnel and is logged like any Msg with an **origin tag** (D6 Envelope-origin design).

**The one load-bearing invariant that makes it replay-safe by construction:** replay
NEVER starts a subscription and NEVER re-runs an effect — it only re-feeds the
logged Msgs the subscription/effect already produced (the same rule that makes
`Cmd::task` safe), and nondeterministic inputs (time, clipboard, async results) are
reproduced solely from the Msg payload captured in the log (the `RecordedEdit.now`
precedent). So Subscription needs almost no NEW replay machinery — the substrate
(enqueue + ordered drain + record tap + LogicalId) already does the work; it needs
only (i) lifecycle keying and (ii) the payload-carries-nondeterminism discipline.

Bundle minimal `Cmd::task` + keyed Subscription as ONE follow-up phase AFTER
substrate+leaf+editor+first-machine land, but **bake the Envelope origin tag +
the replay-re-feed rule into the v1 log format** so they are not an expensive
retrofit (`LoggedEntry` at `mvu/mod.rs:150-156` currently has no origin field).

---

## 3. The honest one-paragraph guarantee (verbatim, ready to paste into the spec)

> Buiy records and replays the MVU-governed subtree, not the whole world. With
> recording on, every message folded through the single ordered drain — widget
> activation/value/expand folds and the editor's resolved EditCommand/IME stream —
> is logged against its stable LogicalId in one global sequence. Replaying that log
> into a fresh app built from the SAME seed scene reproduces every funneled
> widget-internal state (toggle/value/expand, focus transitions, and the editor's
> buffer + caret + selection) BYTE-IDENTICALLY. The guarantee is scoped and
> conditional, not unconditional whole-UI: (a) it covers entities present in the
> seed plus structure that is a deterministic keyed-reconcile of on-log Model state;
> imperative spawn/despawn performed outside the funnel is off-log and is NOT
> reconstructed; (b) state written by escape-hatched raw-ECS systems (entities with
> no Model, direct component writes) is outside the boundary and is reconstructed
> only to its seed value; (c) replay re-feeds logged effect/subscription results and
> never re-runs effects or re-subscribes, so nondeterministic input (time, OS
> clipboard, async results) is reproduced only insofar as it was captured as a
> logged Msg payload. A debug-build write-outside-the-funnel auditor makes the
> boundary detectable rather than silent.

This matches H6/D9/H8 and over-claims nothing the prototype did not prove.

---

## Residual open-for-spec / risks

- **DERIVED-structure replay is asserted-not-proven.** The prototype built no
  data-driven list machine, so no test exercises reconcile-from-Model replay. Either
  ship a minimal keyed-list fixture (Model collection → reconcile spawns children
  with deterministic LogicalIds → replay reconstructs them) **plus the
  resolver-rebuild fix + make the dead-letter loud/typed**, OR downgrade the spec
  wording to "derived structure is targeted, not yet proven." Do not let clause (a)
  market an untested claim.
- **Subscription replay-safety hinges entirely on payload-carries-nondeterminism.**
  If any subscription emits a bare tick and the reducer reads time/clipboard from
  live env during replay, replay diverges silently. The editor is the documented
  PureEnv-exempt hole, so the payload-carries-time rule must be a **tested
  invariant**, not a doc note.
- **State the v1 trigger condition explicitly:** a Subscription is REQUIRED the
  moment a timer/OS/async source drives Model state (not pure visuals). Audit the
  planned Dialog/Popover machines for timer/OS inputs (auto-dismiss timer,
  reposition-on-resize) before sequencing the phases — they could pull Subscription
  earlier than "after first machine."
- **Cross-process UnifiedLog** (REFINE #5): W4's two-log in-process merge is NOT a
  serialized format; the honest guarantee's clause (c) assumes a single serialized
  log that does not yet exist. Cross-process replay is itself partially roadmap.
- **Resolver strategy:** per-entry live `Query` (simple, O(entries×scene), robust)
  vs incremental resolver maintenance (faster, more state) — a one-line iai
  judgement once derived structure exists.

## Key evidence

- prototype `replay.rs:42-47` (the two H8 gaps verbatim), `:149` (resolver computed
  once), `:152-153` (silent dead-letter `continue`), `:166` (`app.update()` per
  applier), `:177` (`logical_id_index` seed-only).
- prototype `mvu_whole_ui_replay.rs:459-528` (off-funnel spawn = zero log, not
  recreated), `:377-453` (killer use case: clicks+keyboard+IME+paste byte-identical,
  no Subscription/structural ops).
- prototype `mvu/mod.rs:113-124` (Cmd = None/Emit/Batch; task/Subscription deferred);
  `record.rs:213-222,287,321-326` (IME tapped at apply site, carries `now`).
- `SYNTHESIS.md:29` (H8), `:200-201` (open-Q1/Q2), `:133-135` (D9).
- prior-art: `elm-redux-time-travel/lessons-for-buiy-mvu.md:46,48` (AVOID-5/-6);
  `relm4/lessons-for-buiy-mvu.md:43-44,66`; `iced/glossary.md:25`,
  `architecture.md:109`.
