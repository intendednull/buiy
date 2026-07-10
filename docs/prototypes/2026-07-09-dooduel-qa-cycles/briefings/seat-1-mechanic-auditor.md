# Seat supplement — Mechanic Auditor

> Read `COMMON.md` first. This adds your archetype's job on top of it.

You verify the **numbers**. You recompute scoring, time the hint schedule, probe
guess normalization, and build chat volume for the KI-02 regression probe. Otherwise
you guess normally and draw your own turn honestly.

The orchestrator gives you the configured **`draw_seconds`** (the standard QA config
is **240**). Use *that* value everywhere below — do **not** assume 150.

## 1. Scoring — recompute, don't eyeball (charter §1.B)

- **Guesser award** `= max(20, round((50 + 450·fracTimeLeft) · 0.82^order))`, where
  `order` is 0-based (0 = first correct this turn) and `fracTimeLeft =
  drawSecondsLeft / totalDraw`.
- **Drawer award** `= round(100 · correctCount / guesserCount)`, 0 if nobody guessed.
- **Procedure:** each time you guess correctly, log the `ui.md` countdown at that
  instant (`drawSecondsLeft`) and your `order` (count the `"X guessed the word!"`
  system lines that preceded yours this turn). Read your `— N pts` before and after,
  recompute, compare. A delta beyond rounding is **S2 / mechanics**. See the worked
  example in `finding-template.md` (it is exactly this probe).

## 2. Hint schedule — time it against the formula (charter §1.B)

- Revealed count `= clamp(hints, 0, letters−1)`. Thresholds at
  `floor(totalDraw·(0.6 − i·0.18))` **seconds-left**, for i = 0, 1, …
- **Compute for the ACTUAL draw_seconds.** With draw **240** + 2 hints: reveal 0 at
  `floor(240·0.6) = 144` s-left; reveal 1 at `floor(240·0.42) = 100` s-left. (The
  charter's "~90 / ~63 s" numbers assume draw 150 — recompute for your config.)
- Log the countdown when each hint letter appears; compare. Off-schedule or a missing
  hint is **S2**.

## 3. Guess normalization + near-miss (charter §1.B, §4)

Normalization is `trim().toLowerCase().replace(/[^a-z0-9]/g,'')`, so `" RAIN-BOW! "`
must match `rainbow`. On a turn where you can guess **and are confident of the word**,
run this **ordered** probe as your guesses:

1. **Near-miss first** (edit distance 1, e.g. `"robof"` for `robot`) → expect a
   **private** `"So close! 👀"` nudge visible **only in your** `ui.md`/`screen.png`,
   not broadcast to chat. If it broadcasts, or never fires, that's a finding.
2. **Messy-but-correct** (`" RoBoT! "`) → expect it to normalize, **lock**, score,
   and broadcast `"X guessed the word!"` (never the raw text). If the messy form is
   rejected, **S2**.
3. **One more guess after locking** → expect a **graceful** rejection: no crash, no
   double-score (no free-chat in M1). A crash/double-score is **S1/S2**.

Do the near-miss **before** you lock — you cannot guess after a correct lock.

## 4. Canvas convergence — your own view only (honesty-scoped)

You can only see **your** canvas. As a guesser, confirm strokes appear
**incrementally** (live streaming), not all-at-once. On your own draw turn, confirm
your strokes render, **Undo** removes exactly your last op, and **Clear** empties
your canvas — all in **your** `screen.png`. The **cross-seat byte-identical canvas
diff is the orchestrator's check** (it holds every seat dir); you must not read other
seats, so just report what your own view shows.

## 5. Drive chat volume for KI-02

Make **many genuine guess attempts across every turn you are a guesser** — wrong
guesses broadcast literally and add chat rows. This deliberately grows the chat past
the point where the working set crosses an atlas page, which is the setup for Seat 0
to confirm late chat rows still render text (KI-02). Keep it honest (real attempts),
but keep the volume up.
