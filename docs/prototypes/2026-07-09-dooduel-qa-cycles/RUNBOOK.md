# Dooduel QA cycles — orchestrator runbook

**Date:** 2026-07-09 · **Branch:** `feat/dooduel-multiplayer-m1` · Worktree
`/mnt/storage/projects/buiy/.claude/worktrees/dooduel-app2`.

How to run one QA cycle: spin up a real 4-agent Dooduel match on the GUI seat
driver, play it to the podium, collect structured findings, triage them, fix the
real ones via `/staged-development`, update the known-issues ledger, and rotate into
the next cycle. Repeat until a cycle is clean or you have run ≥ 5 (charter §6).

> **Prerequisites (build these before cycle 1).** The GUI seat driver
> (`apps/dooduel/examples/qa_seat.rs`, spec §res-Q6) and its `#[ignore]` GPU-lane
> smoke (`apps/dooduel/tests/qa_seat_smoke.rs`, spec §6) do **not** exist yet — they
> are separate deliverables of this campaign's harness-setup step. This runbook
> assumes both are built and the smoke passes on the GPU host before any live cycle.

---

## 1. Pre-flight

### 1.1 Build (needs the stack bump — spec §C2, archaeology §6.6)

```bash
RUST_MIN_STACK=33554432 cargo build -p dooduel_server
RUST_MIN_STACK=33554432 cargo build -p dooduel --example qa_seat
# Optional: add --release to both for smoother real-time under 4 concurrent render
# worlds on one GPU (§C4); then use target/release/… below instead of target/debug/….
```

### 1.2 Prove the driver first (spec §6)

Before trusting a live cycle, run the smoke on this GPU host (AMD RX 6700 XT / RADV;
no display needed). It self-spawns a real `dooduel_server` and exercises boot →
readback → snapshot → resolving click → create → join → pick → stroke → guess, with
**≥ 3 concurrent seats** to validate the multi-render-world regime (spec §6):

```bash
RUST_MIN_STACK=33554432 cargo test -p dooduel --test qa_seat_smoke -- --ignored --test-threads=1
```

Green smoke = the driver is trustworthy for a live cycle. Do not run a cycle on a red
or unbuilt driver.

### 1.3 Server config — WIDEN the timers for agent play (spec §6 operational dependency)

Write the QA config to the server's live config file (repo-root `dooduel_server.toml`,
gitignored — precedence `--config` > `DOODUEL_CONFIG` > `./dooduel_server.toml`):

```toml
# dooduel_server.toml — QA cycle config (agent play)
[room]
rounds = 1          # one drawer rotation; bump to 2 in a later cycle to exercise rollover
draw_seconds = 240  # WIDE — the GUI driver needs many acked toolbar clicks per drawing
pick_seconds = 120  # WIDE — agent reads the pick overlay + screen.png before choosing
reveal_seconds = 60 # WIDE — agent reads the result + clicks Continue
hints = 2
bots = false        # every seat is an agent
```

**Why so wide (and why this supersedes the charter's "reuse 150/30/12").** Agents are
much slower than humans, and this GUI driver is slower still than the prior
`dooduel_mcp` text seats that ran 150/30/12: a single drawing is now **many** acked
round-trips (`Brush` → `Color N` → `Brush size K` → `stroke`, each an append-and-ack),
the eyes throttle at ~1 Hz (spec §2.2), and each seat also *reasons over a
screenshot*. A tight-timer server starves the seats regardless of a correct driver
(spec §6). These values are a starting point — **calibrate from cycle-1 experience**
(if drawers routinely finish early, tighten draw; if picks auto-fire, widen pick).
Log any change in the journal. (`draw 240 / pick 120 / reveal 60` is wider than run-3's
150/30/12 and roughly run-2's 420/180/120 scaled down; the archaeology's rule is
"every run widened them", §6.5.)

### 1.4 Match directory layout

Pick a per-cycle scratch match dir outside the repo (evidence is copied into the repo
at cycle close — §6). Create the per-seat dirs + the shared handoff dir:

```
$MATCH/                         # e.g. $CLAUDE_JOB_DIR/tmp/qa-cycle-1
├── server-transcript.log       # dooduel_server stderr — the per-turn evidence stream
├── shared/
│   └── room-code.txt           # host writes it; the ONLY sanctioned cross-seat file
├── seat-0/  seat-1/  seat-2/  seat-3/    # one per seat; the driver writes:
│     screen.png  ui.md  commands.jsonl  driver.log  report.md  findings/  state/
```

```bash
MATCH="$CLAUDE_JOB_DIR/tmp/qa-cycle-1"
mkdir -p "$MATCH/shared" "$MATCH"/seat-{0,1,2,3}
```

### 1.5 Launch the server (tee stderr to the transcript)

```bash
./target/debug/dooduel_server 2>| tee "$MATCH/server-transcript.log"   # prints "LISTENING port=7878" to stdout
```

Run it in the background (a long-lived process). The stderr transcript
(`[room CODE] phase=… / seat N guessed correctly (+pts) / turn ended — word was '…'`)
is the orchestrator's ground-truth timeline for metronoming and triage.

### 1.6 Launch one driver process per seat (background, long-lived)

Each `qa_seat` process runs the real client, renders offscreen, and bridges its seat
dir. It sets `DOODUEL_STATE_DIR=<seat_dir>/state` itself (per-seat isolation, spec
§2.1) — do not share state dirs. Give each a distinct player name:

```bash
for N in 0 1 2 3; do
  RUST_MIN_STACK=33554432 ./target/debug/examples/qa_seat \
    --dir "$MATCH/seat-$N" --url ws://127.0.0.1:7878 --name "<Name-$N>" &
done
```

Names: use distinct human names (Priya / Theo / Sam / Ada). To test the deterministic
`DoodleAvatar` (charter §1.A), a later cycle can give two seats the **same** name and
assert identical avatars — not cycle 1.

---

## 2. Seat-agent dispatch

The `qa_seat` **processes** are the hands + eyes; the seat **agents** are the brains.
Dispatch one independent agent per seat (foreground agents you monitor), each given
**only**:

- `briefings/COMMON.md` (the shared briefing),
- its archetype supplement for this cycle (§3 rotation),
- its absolute **seat dir** path and the absolute **`shared/room-code.txt`** path,
- the **build sha under test** (`git rev-parse --short HEAD`) for the `build:` field.

**Naive-seat firewall.** Whichever seat runs the **naive-first-timer** archetype this
cycle gets **only** `COMMON.md` + `seat-3-naive-first-timer.md` — never the other
supplements, the charter, the reference design, or `known-issues.md` (charter §4;
the supplement's header repeats this). Keep its dispatch prompt clean.

**Liveness / metronome policy (the #1 historical failure — archaeology §6.1/§6.3,
journal 2026-07-10).** Seat agents idle after single actions and lose background
wakes. Their briefing mandates a persistent foreground poll loop, but you must
back-stop it:

- Tail `server-transcript.log` for phase transitions and each `seat-N/driver.log` for
  new commands. If the phase is **Drawing** and the drawer seat has appended no
  `stroke` by ~half the draw timer (or **Picking** with no pick by ~half the pick
  timer), **`SendMessage`-nudge that seat** ("it's your turn to draw/pick — act now").
- At **Reveal**, the timer auto-advances, but nudge a seat to click `Continue` to keep
  pace.
- **Expect idle-without-report.** After the podium, `SendMessage` **every** seat to
  write/finish `report.md` and `quit` — seats reliably go quiet without delivering;
  the nudge recovers the deliverable (journal 2026-07-10 skill note).

---

## 3. Archetype rotation across cycles

`archetype(seat, cycle) = (seat + cycle − 1) mod 4`, where `0 = host+visual`,
`1 = mechanic`, `2 = chaos`, `3 = naive` (charter §4). Cycle 1 is the identity map;
each later cycle shifts by one so **every fix is re-verified by a different seat than
reported it**, and each seat eventually draws, hosts, and plays naive.

| cycle | seat 0 | seat 1 | seat 2 | seat 3 |
|---|---|---|---|---|
| **1** | host+visual | mechanic | chaos | naive |
| **2** | mechanic | chaos | naive | host+visual |
| **3** | chaos | naive | host+visual | mechanic |
| **4** | naive | host+visual | mechanic | chaos |

The seat running **host+visual** each cycle is the one that clicks "Create a room"
and publishes `shared/room-code.txt`; the others join off that file.

---

## 4. Cycle flow

1. **Launch** — §1 pre-flight, §1.5–1.6 processes, §2 agents. Host creates + publishes
   the code; the others join via the corrected flow (click "Join a room" → `set_value`
   the code → click "Join room", spec §res-Q5); host starts once the roster is full.
2. **Play to podium** — agents run their loops; you metronome (§2). Watch the
   transcript for the podium / final standings.
3. **Collect** — gather each `seat-N/report.md` (findings + session report + harness
   notes) and each `seat-N/findings/*.png`.
4. **Triage (orchestrator)** — you hold every seat dir + the transcript, so you do the
   cross-seat work the honesty contract forbids the seats:
   - **Dedupe** across seats (the same bug seen by 2+ seats = one finding, higher
     confidence).
   - **Known-issues check** against `known-issues.md` — drop re-reports of §2/§3/§4
     items; a §1 regression-watch recurrence is a real regression (S2 minimum).
   - **Canvas byte-diff** — the cross-seat check the mechanic seat can't do: diff the
     turn-end `screen.png` canvas regions (or the paint buffers, if the driver dumps
     them) across seat dirs; a mismatch is canvas desync (S2, charter §1.B).
   - **Classify** severity + `suspected_layer`; when torn, take the higher severity at
     `confidence: low` (finding-template rules).
   - Write `cycle-N/triage.md`: the deduped findings ledger + the fix plan.
5. **Fix** — each real issue (S1–S3; a *pile* of new S4 on one screen may triage up to
   one S3 "screen X diverges") goes through a **full `/staged-development` cycle**,
   executed by subagents and gated (journal standing constraint). S4/S5 feed the
   polish backlog; they do not dirty a cycle.
6. **Update the ledger** — in `known-issues.md`: move each **newly-fixed** item into
   **§1 regression-watch** (so the next cycle re-verifies it by a different seat);
   record newly-accepted quirks in §3; note the cycle's disposition. This keeps the
   suppression list honest cycle-over-cycle.
7. **Next cycle** — bump the cycle number, rotate archetypes (§3), fresh match dir,
   fresh seat agents. Re-verify the fixes shipped since the last cycle.

---

## 5. "Cycle clean" — the exit criterion (charter §6)

A cycle is **clean** when, on a full run to the podium:

1. **Zero new S1–S3 findings** (new = not on `known-issues.md` at cycle start). S4/S5
   are logged but do not dirty the cycle.
2. **Zero regressions** — nothing previously fixed (this campaign or a §1 entry)
   recurs, and every fix since the last cycle is re-verified by a **different** seat.
3. **All scripted probes pass** — every §1.D robustness probe hit its spec-pinned
   outcome; the mechanic seat's scoring arithmetic, hint timing, and the cross-seat
   canvas byte-diff all matched.
4. **Standing match invariants hold** (FINAL §4.3 / M1 §1.4): the match completes end
   to end, **every seat draws once**, every word is guessed or carried by hints
   without wedging, per-seat word honesty holds throughout (no pre-reveal leak),
   podium ordering correct.

**Campaign exit:** the **first clean cycle, or after ≥ 5 cycles**, whichever comes
first — hand over the residual S4/S5 backlog and any consciously-deferred S3s as a
triaged ledger in the retrospective (journal §Charter).

---

## 6. Evidence retention

Per cycle, copy the durable evidence into the repo under
`docs/prototypes/2026-07-09-dooduel-qa-cycles/cycle-N/`:

```
cycle-N/
├── triage.md                 # the deduped findings ledger + severity/layer + fix plan
├── seat-0-report.md … seat-3-report.md
├── server-transcript.log     # the per-turn timeline
└── evidence/                 # ONLY findings-referenced screenshots (findings/<id>.png)
```

**Prune** the working artifacts — the raw `screen.png` streams, the throttled `ui.md`
snapshots, `driver.log`, `commands.jsonl`, and `state/` — are large and disposable;
keep only the screenshots a finding actually cites, the transcript, the reports, and
the triage. This mirrors the `2026-07-04-dooduel-acceptance-playtest-assets/`
precedent (archaeology §5).

**Commit policy (campaign-wide, journal standing constraint).** Local commits of the
cycle evidence are fine; **nothing is pushed / PR'd / merged without the user's
explicit go**. Cycle 1's evidence can double as the never-written M1 acceptance report
(plan W6.2/W6.3) — flag that to the user at close-out (charter §7, journal 2026-07-09).

---

## 7. Per-cycle checklist (copy per run)

- [ ] Driver + smoke built; smoke green on the GPU host (§1.2).
- [ ] `dooduel_server.toml` written with the wide QA timers (§1.3).
- [ ] Match dir + `shared/` + `seat-{0..3}/` created (§1.4).
- [ ] Server launched, stderr tee'd to `server-transcript.log` (§1.5).
- [ ] 4 `qa_seat` processes launched with per-seat dirs + names (§1.6).
- [ ] 4 seat agents dispatched with the rotated briefings (§2, §3); naive-seat firewall held.
- [ ] Host created the room + published `shared/room-code.txt`; all joined; match started.
- [ ] Metronomed stalled seats; match reached the podium; every seat drew once.
- [ ] All `report.md` collected (nudged idle seats); findings + session reports in hand.
- [ ] Triaged: deduped, known-issues-checked, canvas byte-diffed, classified → `triage.md`.
- [ ] Real issues fixed via `/staged-development`; `known-issues.md` updated (fixed → §1).
- [ ] Evidence copied to `cycle-N/`, pruned; cycle marked clean or dirty.
