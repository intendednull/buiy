# Dooduel QA seat-driver — a per-seat GUI agent driver for visual playtest

**Date:** 2026-07-09 · **Status:** landed · **Revision:** rev-2.2 (rev-2 spec review folded;
rev-2.1 = a one-line §2.3 `consumed: K` clarification requested by the plan review; rev-2.2 =
§res-Q5 corrected — the framework now emits `TextChanged` on a value-changing `SetValue` and the
driver workaround is removed — see the change log) · **Branch:** `feat/dooduel-multiplayer-m1`

> Companion to the M1 acceptance flow (`docs/specs/2026-07-04-dooduel-multiplayer-m1-design.md`
> §1.4 / §7, plan `docs/plans/2026-07-04-dooduel-multiplayer-m1.md` W6.2/W6.3). Prior
> playtests are archived in `docs/reports/2026-07-04-dooduel-acceptance-playtest.md`
> and `docs/prototypes/2026-07-02-dooduel-PROTO1-*`. This spec adds the one capability
> those runs lacked: seat agents that **see the rendered GUI** and **act through real
> UI interactions**, not just the protocol-tier `dooduel_mcp` view.
>
> *Doc-index note:* on landing, add this spec to `docs/README.md` and reconcile with the
> still-open plan items W6.2/W6.3 (out of scope for this draft — it edits only this file).

---

## 1. Purpose & requirements

The QA playtest campaign puts **N LLM agents into one real networked Dooduel match**,
each holding a different player seat, and asks each to hunt visual / mechanical / UX
bugs. The bar the prior runs could not meet
(`qa-research-playtest-archaeology.md` §2): *the seat agents never saw the rendered
GUI* — they read the protocol-tier `get_state` markdown + the `get_canvas` PNG, so
every GUI-only bug (empty chat pills, the stuck scoreboard) was caught only by ad-hoc
**human** desktop screenshots, not by the agents.

This driver closes that gap. Requirements for v1 (exactly what cycle-1 needs — YAGNI):

- **R1 — Real app, real match.** Each seat runs the **actual Dooduel client** — the real
  `view` code, the real MVU funnel, and `WsClientPlugin` connecting to a real
  `dooduel_server` over `ws://` — the same `install_runtime` plugin set the shipped
  native binary uses (`apps/dooduel/src/main.rs:12-13`, `apps/dooduel/src/lib.rs:1038-1063`).
  It **ticks in real time** (wall-clock `Msg::Tick` via `ClockPlugin`,
  `crates/buiy_core/src/mvu/clock.rs:123`), so phase timers, the net pump, and countdowns
  advance exactly as for a human — **not** frame-stepped like `capture.rs`.
- **R2 — Pixel eyes.** Every seat writes a periodic PNG of what a player would see:
  the UI camera renders offscreen to a `RenderTarget::Image` at the app's window size,
  read back on the GPU (the `capture.rs` pattern, `apps/dooduel/src/bin/capture.rs`).
  This host has a real wgpu adapter (AMD RX 6700 XT / RADV, no display needed).
- **R3 — Semantic eyes.** Alongside the PNG, each seat writes the **raw** semantic
  snapshot — the Playwright-style role tree + the text/layout dump — from
  `buiy_core::a11y::report::snapshot_report` (`crates/buiy_core/src/a11y/report.rs:59`).
  *Raw is deliberate:* the prior runs' denoised "You can now" summaries are exactly what
  hid the empty-chat-pill / stuck-scoreboard bugs (they summarized away the visual truth).
  The driver adds **no** synthesized affordance layer; agent orientation guidance lives in
  the seat briefings, not the tool output (§3.4).
- **R4 — Hands through supported interactions only.** Agents act via a file-command
  loop (append-only `commands.jsonl`, the `seat_driver.py` idiom): `click` a widget by
  role+name (real `bevy_picking` synthetic pointer), `set_value` into a text field,
  `stroke` across the canvas (real drag), plus `shot` / `quit`. **No** game-semantic
  short-circuits — the agent must click the real toolbar swatches, the real Send button,
  the real Create/Join buttons.
- **R5 — All seats identical, host included.** There is no separate launcher. The host
  agent clicks **Create a room** (`Msg::CreateRoom`, `home.rs:98`) and reads the
  server-issued code from its own `ui.md`/`screen.png`; the other agents navigate
  **Join a room** → enter the code → **Join room** (the corrected flow, §res-Q5 / §3.3).
  This exercises the real lobby end-to-end.

**Why the real UI path matters (beyond R1):** `dooduel_mcp`'s verbs address the *game*
(`pick_word`, `draw_stroke{color,size}`, `guess`), bypassing the widgets entirely
(`qa-research-interaction-surface.md` §2a). This driver drives the *widgets* — the
brush/fill/eraser segments, the 16 swatches, the brush-size dots, Undo/Clear, the Send
button, the pick-overlay choices (all real `A11yRole::Button`s,
`apps/dooduel/src/view/in_game.rs:521-652,684-700,811-827`). It is therefore the first
agent surface that can catch a *toolbar* or *hit-testing* bug (the pick-set ≠ paint-set
class the parity campaign found live). That is the point.

---

## 2. Architecture

### 2.1 Components (one process per seat)

```
        dooduel_server (ws://HOST:PORT)  ← one, shared by all seats + any human GUI
              ▲   ▲   ▲
   ┌──────────┘   │   └───────────┐         each seat = one qa_seat process:
   │              │               │
┌──┴───────────┐  …            ┌──┴───────────┐   ┌─ Real app (install_runtime) ───────────┐
│ qa_seat #1   │               │ qa_seat #N   │   │ view + MVU + NetPlugin + WsClientPlugin │
│  <seat_dir>/ │               │  <seat_dir>/ │   │ + ClockPlugin (real-time Tick)          │
│   screen.png │◀── readback ──│   …          │   │ + ViewportPlugin + CanvasPlugin         │
│   ui.md      │◀── snapshot ──│              │   ├─ Picking camera → RenderTarget::Window  │ (§res-Q1)
│   commands.  │──▶ hands ─────│              │   ├─ Readback camera → RenderTarget::Image  │
│   jsonl      │               │              │   ├─ synthetic Window + PointerId::Mouse    │
│   driver.log │               │              │   └─ headless RenderPlugin (offscreen GPU)  │
└──────────────┘               └──────────────┘   └────────────────────────────────────────┘
                                                    DOODUEL_STATE_DIR=<seat_dir>/state
                                                    DOODUEL_SERVER_URL=ws://HOST:PORT
```

The driver is a headless swap of the shipped binary: it keeps **all** of
`install_runtime`'s plugins and replaces only `DefaultPlugins`' **winit window +
on-screen surface** with a headless `RenderPlugin` + an offscreen `Image` render target +
a synthetic `PointerId::Mouse`.

**Per-seat state isolation (mandatory).** `install_runtime` adds `storage::StoragePlugin`
(`lib.rs:1057`), whose native path is `state_path()` — it writes/reads
`$DOODUEL_STATE_DIR/state.json`, falling back to `~/.config/dooduel/state.json`
(`apps/dooduel/src/storage.rs:84-102`). Without an override, **every seat races the one
shared file AND clobbers the developer's real profile.** The driver **must** set
`DOODUEL_STATE_DIR=<seat_dir>/state` per process (the env override storage.rs already
honors — the same isolation `capture`/tests use).

**Exact plugin composition (no existing target combines these — the §res-Q1 novelty).**
Neither `BuiyProbePlugin` (the probe tests — GPU-free, no render) nor `BuiyHeadlessPlugin`
(the `capture` bin — render, but no picking) combines a headless `RenderPlugin` **with**
picking. This driver is the first, and that combination is exactly what smoke checkpoint 1
retires (§6). Compose:

| Layer | Plugin(s) | Why (what it provides) |
|---|---|---|
| App loop | `MinimalPlugins` | the schedule runner. |
| Transform | `bevy::transform::TransformPlugin` | canonical `GlobalTransform` late propagation. Buiy's own `CorePlugin` bridge already computes `GlobalTransform` in `Update` (`report.rs:113-114`; `BuiyPlugin` needs no `TransformPlugin`, `lib.rs:562-576`), but the two proven picking harnesses add it (`gui_networked.rs:210`, `canvas_e2e.rs:64`) and `DefaultPlugins` (the real app) includes it — so match them. |
| Headless Bevy render | `WindowPlugin{ primary: 1280×800 }`, `AssetPlugin`, `ScenePlugin`, `RenderPlugin`, `ImagePlugin`, `CameraPlugin`, `CorePipelinePlugin`, then `init_asset::<Mesh>()` + `init_asset::<SkinnedMeshInverseBindposes>()` | offscreen GPU render + readback (the `capture.rs:191-217` stack verbatim). |
| Input | `bevy::input::InputPlugin` | focus/keymap + the editor keymap read `Res<ButtonInput<KeyCode>>`. |
| Picking core | `bevy::picking::PickingPlugin` | `PointerInput::receive` + hit scheduling + `Messages<PointerHits>` (the `gui_networked.rs:213` shape; NOT `DefaultPickingPlugins`, so **no** winit `PointerInputPlugin` → no duplicate `PointerId::Mouse`). |
| Buiy (headless) | `buiy::BuiyHeadlessPlugin` | core (+ the `GlobalTransform` bridge) · theme · **`A11yPlugin` — the in-process semantic TREE, `lib.rs:726`** · focus · layout · text · widgets · render. It **omits** `AccessKitAdapterPlugin` (the OS bridge — "needs a real window", `lib.rs:638,680-682`) and the winit `PointerInputPlugin` (`lib.rs:629-633`). This is what makes `snapshot_report` / `get_by_role` / `absolute_pos` work with **no** OS adapter. |
| Buiy picking | `buiy_core::picking::PickingPlugin` + `BuiyPickingBackendPlugin` | re-added because `BuiyHeadlessPlugin` omits picking (`lib.rs:683-685`) — the `InteractionPlugin` hover stage + the Buiy hit-test backend. |
| Fidelity (added beyond the reviewers' list — see change log B) | `buiy_core::scroll::ScrollInputPlugin` + `buiy_core::animation::AnimationPlugin` | the live `BuiyPlugin` app runs both (`lib.rs:646,654`); the chat `.stick_to_bottom()` + scoreboard `.scroll_x()` (`in_game.rs:669,198`) and the button press-dip tweens need them for **faithful** screenshots. Both are query-gated and safe on top of picking. |
| Dooduel | `dooduel::install_runtime` | the real-app bundle: `DooduelThemePlugin` (fonts + theme) · `ClockPlugin` (real-time Tick) · `ViewportPlugin` · `NetPlugin` · `LocalAuthorityPlugin` · **`WsClientPlugin`** (opens the real socket to `server_url()`) · `CanvasPlugin` · `ConfettiPlugin` · `StoragePlugin` (`lib.rs:1038-1063`). Do **not** re-add these individually — `install_runtime` owns them. |

Spawn on top: the `WindowPlugin` auto-creates the sized primary `Window`; the driver adds
the **picking camera** (`Camera2d` + `RenderTarget::Window(Primary)`), the **readback
camera** (`Camera2d` + `RenderTarget::from(image)`), and the synthetic `PointerId::Mouse`
targeting the primary window (§res-Q1). `WsClientPlugin` opens the socket on a staged
`pending_connect` (`net.rs:530-531`, URL from `DOODUEL_SERVER_URL` / default
`ws://127.0.0.1:7878`, `net.rs:548-553`).

### 2.2 The main loop (real-time, throttled I/O)

```
loop:
    app.update()                      # one frame: ClockPlugin folds wall-clock Tick,
                                      # NetPlugin pumps the socket, view re-renders.
    pace to ~real time (e.g. target 30–60 Hz; sleep the remainder of the frame budget)

    drain new \n-terminated lines from commands.jsonl (byte-cursor tail):
        for each command: apply it (§3.2), then append its outcome + a monotonic
        "consumed: K" marker to driver.log (§2.3), then force an I/O refresh so the
        agent sees the effect.

    every ~1 s (throttle) OR on a `shot` command:
        refresh screen.png  (spawn Readback on the Image, pump a few frames until
                             ReadbackComplete fires, strip row padding, save PNG)
        refresh ui.md       (snapshot_report(world))
        (change-detection: skip the write if the bytes are byte-identical to the last)
```

`app.update()` runs continuously so timers and the net round-trip never stall (R1). The
expensive readback + snapshot are **throttled to ~1 Hz** (matching `seat_driver.py`'s
`POLL_SECS = 1.0`, `apps/dooduel_mcp/examples/seat_driver.py:37`) plus an immediate
refresh after every applied command. The GPU readback is the `capture.rs` burst: spawn
`Readback::texture(image)` with an observer, pump up to ~60 frames until `ReadbackComplete`
delivers the bytes, despawn (`capture.rs:377-402`).

**Readback pacing (do not "fix" the stall).** The real-time `Msg::Tick` folds only the
*actual* wall-clock elapsed between frames (`mvu/clock.rs` advances `Time<Virtual>` by the
real delta), so the tens-of-ms readback burst does **not** distort game time — it just
makes one frame's `dt` larger, which the countdown/timers absorb correctly. The ~1 Hz
stall is acceptable and expected; an implementer must not chase it with a background render
thread (that reintroduces the multi-adapter-context hazard §res-Q6/§C4).

### 2.3 File protocol (per seat dir; atomic tmp+rename; append-only for the two logs)

All output files are written atomically (write `<file>.tmp`, `rename` over the target —
`seat_driver.py:40-44`) so an agent can read them at any instant without a torn file.
`commands.jsonl` and `driver.log` are append-only; the driver tails `commands.jsonl` by a
byte cursor and consumes only `\n`-terminated lines (mid-write appends are safe). A
**malformed** JSON line is logged-and-skipped, never fatal (the `seat_driver.py:199`
precedent).

| File | Dir | Writer | Content |
|---|---|---|---|
| `screen.png` | `<seat_dir>/` | driver | RGBA PNG of the offscreen readback camera at window size (§res-Q2). |
| `ui.md` | `<seat_dir>/` | driver | raw `snapshot_report(world)` — role tree + `--- text & layout ---` dump. |
| `commands.jsonl` | `<seat_dir>/` | **agent** | one JSON object per line (§3.1). |
| `driver.log` | `<seat_dir>/` | driver | timestamped `command → outcome`, each carrying a monotonic **`consumed: K`** index. |

**`consumed: K` acknowledgment.** Each consumed `commands.jsonl` line's `driver.log` entry
echoes its 0-based line index `K` (`… consumed: 12 → Ok` / `… consumed: 13 → NotFound{…}`),
so an agent can tell consumed-from-pending by comparing `K` against the number of lines it
has appended — cheaply, without parsing the mixed log. (This is the *only* structured
addition; the eyes stay raw per R3.) **`K` counts EVERY `\n`-terminated line** — a blank or
malformed line still consumes its `K` (logged `… consumed: K → skipped …`), so `K` always
equals the agent's appended-line count and never desyncs; only a trailing partial line with no
`\n` yet is uncounted (it stays buffered until completed).

### 3. Command schema

#### 3.1 Verbs (JSON, one per line)

```jsonc
{"cmd":"click","role":"Button","name":"Create a room"}
{"cmd":"set_value","role":"TextInput","text":"7XQ2KP"}     // name optional if role is unique
{"cmd":"stroke","points":[[120,90],[300,110],[480,300]]}   // Game CANVAS coords 0..720 × 0..450
{"cmd":"shot"}                                             // force an immediate screen.png+ui.md refresh
{"cmd":"quit"}
```

- **`click {role, name}`** — locate the node via
  `buiy::probe::get_by_role(world, A11yRole::<role>, Some(name), None)` (STRICT single
  match: 0 or >1 ⇒ `NotFound`, `crates/buiy_core/src/a11y/inprocess.rs:463-487`), resolve
  its laid-out center from `GlobalTransform` + `ResolvedLayout`, and drive a real
  synthetic-pointer micro-stroke through `bevy_picking` (the `click_button` recipe,
  `apps/dooduel/tests/gui_networked.rs:277-301`). See §res-Q6 for why the pointer path,
  not the a11y typed click.
- **`set_value {role, text, name?}`** — `buiy::probe::set_value` on the addressed node
  (§res-Q5).
- **`stroke {points}`** — **Game-canvas** coords (v1 targets `CanvasKind::Game` only,
  §3.3), mapped to the live canvas rect and driven as a real drag (§res-Q4). A 1-point
  `points` is promoted to a 2-point micro-stroke so a single-tap (paint-bucket seed) still
  lands a press.
- **`shot`** / **`quit`** — force refresh / exit the loop.

Every command's outcome (`Ok`, or the typed `ActionError` —
`NotFound`/`Unsupported`/`NotActionable`/`BadData`, `crates/buiy_core/src/a11y/contract.rs:41-82`)
is appended to `driver.log`. A `NotFound`/miss is a genuine QA signal (the widget the
agent expected is not on screen / not hittable), never swallowed.

**Exact matchable names (case-sensitive, STRICT single-match; reviewers verified no
per-screen label collisions):**

- Brush-size dots are **`Brush size 3` / `6` / `11` / `18`** — `BRUSH_SIZES = [3,6,11,18]`
  (`apps/dooduel_core/src/canvas.rs:43`, labelled `in_game.rs:619`). There is **no**
  "Brush size 2".
- Word-choice buttons are the **UPPERCASED** word — `click {name:"ROBOT"}`
  (`button(w.to_uppercase())`, `in_game.rs:818`).
- The floating theme toggle's name is **state-dependent** — `"Dark"` while dark, `"Light"`
  while light — and **flips on click** (`widgets.rs:214-219`); re-read `ui.md` after
  toggling rather than clicking the same name twice.
- Swatches are `Color 0..15`, tool segments `Brush`/`Fill`/`Eraser`, plus `Undo`/`Clear`,
  `Send`, `Copy`, `Leave` — all real `A11yRole::Button`s (§res-Q6).

#### 3.2 The drawing workflow is real toolbar interaction

There is **no** per-stroke color/size (unlike `dooduel_mcp`'s `draw_stroke{color,size}`).
The GUI paints with the *currently selected* tool/color/size (`apps/dooduel/src/paint.rs`
reads `s.tools`), so an agent selects them by **clicking the real toolbar** first:

```jsonc
{"cmd":"click","role":"Button","name":"Brush"}          // or "Fill" / "Eraser" (in_game.rs:569-584)
{"cmd":"click","role":"Button","name":"Color 3"}        // swatch (in_game.rs:638)
{"cmd":"click","role":"Button","name":"Brush size 6"}   // dot   (in_game.rs:619; sizes 3/6/11/18)
{"cmd":"stroke","points":[[120,90],[300,110]]}          // paints with the selection
{"cmd":"click","role":"Button","name":"Fill"}
{"cmd":"stroke","points":[[600,400]]}                   // bucket seed (promoted to micro-stroke)
{"cmd":"click","role":"Button","name":"Undo"}           // (in_game.rs:541) / "Clear"
```

Guessing:

```jsonc
{"cmd":"set_value","role":"TextInput","text":"robot"}
{"cmd":"click","role":"Button","name":"Send"}           // fires Msg::SubmitGuess (in_game.rs:690)
```

#### 3.3 CLI & scope

**CLI** (mirror `seat_driver.py`'s flags where they still apply, `seat_driver.py:151-156`):
`--dir <seat_dir>` (required), `--url <ws://…>` (default `DOODUEL_SERVER_URL` /
`ws://127.0.0.1:7878`; the driver **exports** `DOODUEL_SERVER_URL` so `server_url()` picks
it up, `net.rs:548-553`), `--name <Name>` (optional convenience — pre-seed the Home name
field). The driver also **sets `DOODUEL_STATE_DIR=<seat_dir>/state`** for per-seat
persistence isolation (§2.1). Drop `--bin` (no MCP subprocess — the driver *is* the client)
and `--room` (room create/join is done through the UI per R5). Optional: `--size WxH`
(default `1280x800`, §res-Q2), `--interval <secs>` (default `1.0`). Build/run needs
`RUST_MIN_STACK=33554432` (§C2).

**Canvas scope (v1).** `stroke` targets `CanvasKind::Game` **only**. Custom-avatar
*drawing* (the `CanvasKind::Avatar` scratch surface in the avatar editor) is **out of
cycle-1 scope** — agents may open the avatar editor and `click` **preset** avatars, but
not draw one; and custom avatars do not cross the wire in M1 anyway, so other seats would
never see a drawn avatar (KI-20; the roster shows the name-hashed doodle for non-preset
wire avatars, `apps/dooduel/src/view/lobby.rs:131-139`). The documented future extension
is an optional `canvas` field on `stroke` (`{"cmd":"stroke","canvas":"avatar",…}`,
default `"game"`).

### 3.4 What the driver does NOT add

No synthesized "You can now" / affordance summary layer (R3): the raw `snapshot_report` +
`screen.png` are the whole eye. Agent orientation ("if it's your turn to draw, the pick
overlay shows word buttons…") lives in the **seat briefings** the campaign orchestrator
writes, not in the tool output — so a summarizer can never hide a visual bug again.

---

## 4. Resolved questions (with code evidence)

### res-Q1 — Does synthetic picking work while rendering offscreen? (highest risk)

**Answer: yes, via a two-camera setup — a Window-target camera for picking + an
Image-target camera for readback — on one window-sized layout.** The no-panic path is
traced in source (below); smoke checkpoint 1 confirms it end-to-end (§6, §C1).

The Buiy picking backend hit-tests **entirely in window-logical space**: `emit_picks`
reads the pointer's `PointerLocation.position` as the cursor
(`crates/buiy_core/src/picking/backend.rs:139`) and tests it against each node's
`GlobalTransform.translation()` (absolute, top-left origin, `backend.rs:152-153`). The
camera is used only for hit **provenance + ordering**, not the hit math. Layout's viewport
is read from `Query<&Window, With<PrimaryWindow>>` (independent of any camera; the
`ViewportPlugin` feeds `viewport_w` from that window — `crates/buiy_verify/src/pointer.rs:176`).

But `emit_picks` applies two hard gates (`backend.rs:129-138`):

1. the pointer's target **must** be `NormalizedRenderTarget::Window(_)` — a pointer whose
   target is an **`Image`/`TextureView` is skipped** (the comment names exactly "the
   deferred render-to-texture case", `backend.rs:130-135`);
2. there **must** be an **active** camera whose `RenderTarget` normalizes to that window
   (`camera_for_target` requires `cam.is_active`, `backend.rs:194-202`).

Offscreen readback, meanwhile, needs a `Camera2d` whose `RenderTarget::from(image)`
targets an **Image**, then `Readback::texture(image)` (`capture.rs:221-243,377-402`).
Picking wants a **Window** target; readback wants an **Image** target — different targets,
so the driver spawns **two cameras** at the same resolution as the primary `Window`:

- **Picking camera** — `Camera2d` + `RenderTarget::Window(WindowRef::Primary)`, active;
  satisfies `camera_for_target` for the synthetic pointer (which targets that window,
  exactly as `pointer.rs:197-208`, `gui_networked.rs:229-241`, `canvas_e2e.rs:96-108`).
- **Readback camera** — `Camera2d` + `RenderTarget::from(image)`, active; renders the same
  tree to the `Image` the screenshot reads back.

Because hit-testing and layout both live in the primary-Window's logical space, and the
screenshot Image is the same size, the pointer, the picked geometry, and the pixels all
agree.

**Why the surfaceless Window camera does not panic (the no-panic trace).** The QA driver
is the *first* app to combine a real headless `RenderPlugin` with **both** a Window-target
and an Image-target camera (in `gui_networked.rs`/`canvas_e2e.rs`/`PointerHarness` the
Window camera is inert — GPU-free probe preset, **no** `RenderPlugin`; in `capture.rs`
there is a `RenderPlugin` but **only** an Image camera). The path is nonetheless
well-supported by source:

- **In-repo (verified):** Buiy's render node `buiy_pass` takes a
  `ViewQuery<(&'static ViewTarget, …)>` (`crates/buiy_core/src/render/node.rs:75-83`) — a
  view with **no** `ViewTarget` is simply not iterated, so the surfaceless Window camera is
  never drawn; and even a reached view early-returns on missing pipelines
  (`node.rs:94-99`). No panic.
- **bevy_render 0.19 (reviewer-traced registry paths):** a window with no winit surface is
  never extracted — `extract_windows` requires a `&RawHandleWrapper`, which only
  `WinitPlugin` adds (`bevy_render-0.19.0/.../view/window/mod.rs:128`); and a Window-target
  camera with no output attachment is stripped of its `ViewTarget` and skipped
  (`bevy_render-0.19.0/.../view/mod.rs:1224-1234`). So the Window camera never reaches a
  swapchain acquire.

C1 is therefore reframed from "load-bearing unproven assumption" to **"well-supported by
source; smoke confirms"** (§8). **Fallback if the smoke still surprises us:** the
Image-target rejection at `backend.rs:130-135` is *deliberate* ("the deferred
render-to-texture case"), so any relaxation must be **scoped** — resolve an Image-target
pointer to *its* Image camera **only for the QA driver's own image**, without re-enabling
picking for a genuine off-screen render-to-texture group. That is a real (if small) design
change, not a blind 5-liner; low priority, unlikely to fire. Do **not** pre-build it.

### res-Q2 — Window size

**Answer: `1280×800`.** `is_mobile()` is true only below `MOBILE_BREAKPOINT = 430.0`
(`apps/dooduel/src/lib.rs:71,185-186`), so any desktop-scale width renders the desktop
3-pane (scoreboard | canvas + toolbar | chat, `in_game.rs:429-439`) — the composition a
desktop player sees. `1280×800` is the exact size the existing desktop GUI end-to-end
test uses and is proven to lay out the full 3-pane (`canvas_e2e.rs:86`); the extra height
over the shipped app's default clears the 556-px desktop chat pane + top bar + header
without clipping (the same reason `capture.rs:30` bumps to 760). The real binary opens at
bevy's `1280×720` default (`main.rs:12`, `DefaultPlugins`, no override) — same width, so
`1280` is faithful; `+80` px height is headroom. **Rejected — `1200×760`** (capture /
gui_networked): `1200` sits at/under the 3-pane's natural minimum
(scoreboard `240` + center `606` (`in_game.rs:31`) + chat `300` + inter-pane gaps + body
`Space::Lg` padding), so the `.grow()` center compresses slightly — fine for a scripted
parity shot, but for QA we want the uncompressed desktop layout.

### res-Q3 — Is the room code (and the guesser's hints) readable from the semantic tree?

**Answer: yes, from `ui.md`'s text section — no exposure fix needed.** The lobby renders
the code as a bare, role-less `text(s.replica.room_code.as_str())`
(`apps/dooduel/src/view/lobby.rs:36-38`). The a11y **role** tree drops role-less `Text`,
but `snapshot_report`'s `--- text & layout ---` section exists precisely to surface it:
it lists "plain, role-less `Text` the tree drops" as `text="…"` glyph content in reading
order (`crates/buiy_core/src/a11y/report.rs:207-262`, `write_text_section`). So the host
agent reads the code from `ui.md` (and confirms against `screen.png`) with **no OCR** and
**no** a11y change. The room-code exposure exception from the task brief is therefore
**not triggered** (YAGNI).

**The same mechanism gives guessers their hints (assurance J-b).** The word-slot row and
the countdown are both rendered as plain `Text` and so land in the text section: each
revealed letter is `text(ch_str)` (`in_game.rs:353-364`) and the countdown is
`text!("{}", secs)` (`in_game.rs:392,394`). A guesser reads the **revealed letters** as
glyphs and the **seconds** as a number from `ui.md`. Nuance: a *blank* slot renders as a
space (`'_' → " "`, `in_game.rs:355-359`), so the agent infers word *length* + blank
*pattern* from the slot count + the `screen.png` underlines rather than a clean `_ _ B _ _`
string — the briefings should say to cross-read the screenshot for the blank pattern.

### res-Q4 — Stroke coordinate mapping

**Answer: the driver accepts canvas coords `0..720 × 0..450` (identical to
`dooduel_mcp`/`get_canvas`, so agents reuse prior drawing knowledge) and maps them to
window px via the live canvas rect** — the exact inverse of the app's own `to_pixel`.
`to_pixel` maps a window point back to a texel as
`floor((win − tl) / size * CANVAS_dim)` (`apps/dooduel/src/paint.rs:191-208`), where
`size` is the canvas node's *displayed* rect, **not** the image size. On desktop the
canvas displays at `600×375` (`CANVAS_DISP_W/H`, `in_game.rs:27-28`) — the raster is
scaled to fit the center pane (`in_game.rs:24-26`) — so the mapping is **not 1:1** and a
per-axis ratio is essential. Inverse (driver):

```
rect = (tl, size) of the CanvasKind::Game node  (GlobalTransform + ResolvedLayout,
                                                  exactly canvas_e2e.rs:146-155)
win.x = tl.x + ((cx + 0.5) / CANVAS_W) * size.x      // CANVAS_W = 720, CANVAS_H = 450
win.y = tl.y + ((cy + 0.5) / CANVAS_H) * size.y      // +0.5 → texel center (to_pixel floors)
```

The driver feeds the mapped points to `buiy_verify::pointer::drive_stroke(app, window,
pointer, &pts)` — the real press → drag → release the drawing canvas already consumes
headlessly (`apps/dooduel/tests/canvas_e2e.rs:206`; `drive_stroke` is documented for
exactly "a long-running playtest host that drives agent strokes",
`crates/buiy_verify/src/pointer.rs:645-647`). Because it derives from the live rect per
call, the same mapping is correct on mobile (`content_w × 240`, `in_game.rs:209-218`) too.

### res-Q5 — Text entry + the corrected Create/Join flow

**Text entry: `set_value` on an `A11yRole::TextInput`, then click the screen's CTA
button.** A `text_input` view element realizes a `buiy_widgets::TextInput::single_line`
carrying `A11yRole::TextInput` (`crates/buiy_view/src/reconcile.rs:506`,
`crates/buiy_core/src/a11y/translate.rs:229`). `buiy::probe::set_value` dispatches
`Action::SetValue` lowered through the editor's `SelectAll` + `Insert` channel
(`crates/buiy_core/src/a11y/inprocess.rs:429-441`) — the same channel `canvas_e2e.rs` and
the widget-catalog gallery use — which fires the field's `on_input` binding, folding the
text into the MVU model. Each of these screens has exactly **one** `TextInput`, so address
by role alone (`get_by_role(TextInput, None, None)`; the strict single-match resolves it,
and the placeholder is phase-dependent so role-alone beats name).

> **rev-2.2 correction.** The "fires the field's `on_input` binding" claim above was **not**
> true pre-fix: `Action::SetValue` (`honor_text_set_value`) updated the editor + a11y tree but
> emitted no `TextChanged`, so `route_text_input` never fired `on_input` and the model never
> folded — the framework gap the W1 gate found (a `set_value`'d Join code that `SubmitJoin`
> read as `""`). The framework now emits `TextChanged` on a value-changing `SetValue` (mirroring
> the keyboard path, `crates/buiy_core/src/a11y/contract.rs`), so the claim holds and the
> driver's `TextChanged` re-emit workaround is removed.

- **Guess (chat):** the field is `text_input(s.chat_input.clone()).on_input(Msg::SetChatInput)
  .on_submit(Msg::SubmitGuess)` (`in_game.rs:684-689`); submit by **clicking** `Button`
  "Send" (`in_game.rs:690` → `Msg::SubmitGuess`).
- **Join (corrected flow — this was wrong in rev-1).** "Join a room" on **Home** is
  `Msg::GoJoin` — pure **navigation** to the Join screen (`apps/dooduel/src/view/home.rs:99`),
  **not** the join action. The real CTA is on the Join screen:
  `primary_button("Join room", Msg::SubmitJoin, p)` (`apps/dooduel/src/view/join.rs:39`),
  and that screen's `TextInput` also has `.on_submit(Msg::SubmitJoin)` (`join.rs:22`). So
  the sequence is: `click "Join a room"` (Home → Join screen) → `set_value` the code into
  the Join screen's `TextInput` → `click "Join room"`.

### res-Q6 — Binary placement + shared helpers

**Answer: place it as an *example* target — `apps/dooduel/examples/qa_seat.rs` — reusing
public surfaces; no production-code promotion required.** Cargo `[[bin]]` targets can use
only `[dependencies]`, but **example** (and test/bench) targets can use
`[dev-dependencies]` (`buiy_verify` is already one, `apps/dooduel/Cargo.toml:77-78`) *and*
the package's own lib. So an example freely combines `dooduel::*`,
`buiy::probe::{get_by_role, set_value, click}`, `buiy_core::a11y::translate::entity_for_node_id`,
and `buiy_verify::pointer::drive_stroke` — the exact public building blocks
`gui_networked.rs`'s `click_button` (lines 277-301) and `canvas_e2e.rs` already compose —
**without** touching production paths. The offscreen render-target + readback come from
`capture.rs`'s `render_target` (`capture.rs:221-243`) + `capture` (`377-424`) functions,
adapted into the example (~40 lines; `capture.rs` is a bin, so its fns aren't importable —
the example carries its own copy).

**Why the synthetic pointer, not the a11y typed click.** `buiy::probe::click` dispatches
`Action::Click` through the a11y router (screen-reader activation) — also "supported", but
it bypasses `bevy_picking` and so cannot catch the pick-set ≠ paint-set / occluded-hit
class the parity campaign found live. The driver instead *addresses* by role
(`get_by_role`, strict single-match) and *acts* through the real synthetic pointer at the
node's resolved center (the `click_button` recipe). A button that resolves by role but
whose center is not hittable then **fails** — a genuine QA signal, not a driver bug.

**Optional DRY (not required for v1):** factor `gui_networked.rs::click_button` into
`buiy_verify::pointer::click_role_center(app, window, pointer, role, name)` (next to
`drive_stroke`) so the test and the example share one implementation. `buiy_verify` is the
dev-only verification/driver crate that already owns `drive_stroke`, so this is squarely
its purpose and **not** a production change — but it is deferrable past cycle 1.

**Not a `playtest_host` revival.** The retired `playtest_host`
(deleted commit `0e1904d`, `apps/dooduel/Cargo.toml:36-40`,
`docs/plans/2026-07-04-dooduel-multiplayer-m1.md:321`) was a *pre-replica, no-network,
no-GPU* file host driving the old local `Game` model directly. This driver is a
**different tier**: the *rendered UI* (pixels + widget hits) of the *real replica-based
client* over the *real network*. `dooduel_mcp` remains the protocol-tier seat unchanged;
`qa_seat` is the UI-tier eyes/hands. They coexist.

### res-Q7 — Dependency hygiene

**Answer: the example target keeps `buiy_verify` a `[dev-dependencies]` — it never enters
`dooduel`'s production graph.** `buiy_verify` is the verification harness; it must not ship
in the game (the `buiy_bench_support` "dev-only, never in the production graph" precedent).
As an example (not a `[[bin]]`), `qa_seat` adds **zero** new edges to
`apps/dooduel/Cargo.toml`'s `[dependencies]` — it consumes the *existing* dev-dependency.
**Rejected — add `buiy_verify` to `[dependencies]`** so a `src/bin/qa_seat.rs` could use
it: that drags the verification/golden crate into every shipped `dooduel` build.
**Fallback (only if a first-class installable bin is ever required, which cycle 1 does
not):** a dedicated `apps/dooduel_qa` crate with `buiy_verify` as a normal dependency
(publish = false, depended on by nothing that ships — the `buiy_bench_support` shape).

### res-Q8 — Reconnect

**Answer: out of scope for v1 — keep the process alive; a `CONNECTION LOST` banner means
the seat is lost.** `seat_driver.py` cannot rejoin mid-match: its `join_room` always sends
`reconnect: None`, and the reconnect token dies with the process
(`qa-research-interaction-surface.md` §5.2; `dooduel_mcp/src/mcp.rs:138`); a fresh
mid-match join is server-rejected `MatchInProgress`
(`apps/dooduel_server/tests/e2e.rs:458-499`). Same here: a supervised `qa_seat` stays alive
for the whole match. If its socket drops, the reducer routes to a "connection lost" state
(`apps/dooduel/src/lib.rs:520-526`, `WsClientPlugin`-enqueued) and the seat is gone.
Auto-reconnect is explicitly deferred (§7).

---

## 5. Rejected alternatives

- **`dooduel_mcp`-only eyes (text `get_state` + `get_canvas` PNG).** The proven M1 harness,
  but it *is the gap*: it never renders the GUI, so it cannot see layout / theming /
  overlap / invisible-text bugs (`qa-research-interaction-surface.md` §5.3). Kept as the
  orthogonal protocol-tier seat; not a substitute for pixel eyes.
- **OS-window capture + `xdotool`/synthetic OS input.** Requires a real display + winit
  window per seat; concurrent seats fight for keyboard/pointer focus; clicks become fragile
  absolute-pixel guesses; and it injects at the winit layer Buiy's own synthetic-pointer
  path deliberately replaces (`pointer.rs:305-307`). Offscreen render + in-process
  `bevy_picking` avoids all three: no display, no focus contention (each seat is its own
  process/World), role-addressed hits.
- **Web build (`dooduel_web`) + browser automation (`agent-browser`).** Genuinely gives
  live pixels + real clicks, and is the *only* current live-pixel path
  (`qa-research-interaction-surface.md` §1e). But it is heavier (build the wasm bundle,
  serve it, drive N browser tabs), carries web-specific traps
  (`qa-research-playtest-archaeology.md` §6.12), and tests the *web* client, not the native
  one. Deferred as a possible later **web lane** (§7), not v1.
- **Extend `dooduel_mcp` itself with a render stack.** Would bolt a GPU/render pipeline onto
  the headless protocol client, conflating the protocol-tier seat (whose whole value is
  being GPU-free and pixel-free) with a UI-tier tool, and re-introduce the production-hygiene
  problem (the MCP client would need render deps). A separate example running the *real GUI
  client* keeps each tool's concern clean.

---

## 6. Verification design — prove the driver before cycle 1

**Form (decided):** a committed **`#[ignore]` GPU-lane integration test** (e.g.
`apps/dooduel/tests/qa_seat_smoke.rs`) that **self-spawns the real `dooduel_server`**
(the `apps/dooduel_server/tests/e2e.rs` "spawn the real binary" pattern) and drives the
`qa_seat` instances through the file protocol. It compiles under `--all-targets`, **never
runs in the headless CI gate** (no adapter there), and is on-demand on a GPU host:

```sh
RUST_MIN_STACK=33554432 cargo test -p dooduel --test qa_seat_smoke -- --ignored --test-threads=1
```

*Recommended shape:* spawn **one process per seat** (the built `qa_seat` example binary),
not N render `App`s in one process — that mirrors the real deployment and gives each seat
its own wgpu adapter context (the GPU lane already serializes with `--test-threads=1`
precisely because it's "one adapter context at a time", CLAUDE.md). The test writes
`commands.jsonl` and reads `screen.png`/`ui.md` — black-box driving the real shape.

Checkpoints (a real `dooduel_server` running throughout):

1. **Checkpoint 1 — the composed-stack boot (the §res-Q1 / §C1 gate).** One `qa_seat`
   boots the full composition (§2.1) **headless without panic**; **one** GPU readback lands
   in `screen.png`; **one** `snapshot_report` writes a **non-empty** `ui.md`; and **one
   `click` on a known button resolves to its `Msg`** (proving the picking camera + synthetic
   pointer path works end-to-end, not just that the app boots).
2. **Host creates via UI:** `click "Create a room"`; assert the lobby renders and the room
   code appears in `ui.md`'s text section (res-Q3) — read it back programmatically.
3. **Second seat joins via UI (corrected flow):** a second `qa_seat` against the same
   server — `click "Join a room"` (→ Join screen) → `set_value` the code → `click "Join
   room"`; assert both seats show a 2-player roster.
4. **Start + pick:** host `click "▶ Start game"` → reaches the board; drawer `click`s a
   word-choice button (`name` = the UPPERCASED word) → Drawing phase
   (`gui_networked.rs:467-536`).
5. **A stroke lands, exercising label-only buttons:** drawer `click "Brush"` (a text
   button), **then `click "Color 3"` and `click "Brush size 6"`** — the `.label()`-derived
   accessible-name path (`buiy_core::a11y::mod.rs:30` `compute_accessible_name`, name from
   `label > value > placeholder`) that swatches/dots depend on and that a text-only "Brush"
   click never exercises — then `stroke` across the canvas; assert ink in the paint buffer
   (`canvas_e2e.rs:169-224`) **and** that `screen.png` changed.
6. **A guess lands:** guesser `set_value` the word + `click "Send"`; assert a chat line
   appears in the guesser's `ui.md`/`screen.png`.

**Concurrency validation (extend the smoke to ≥3 concurrent seats).** The campaign's
working size is **4 concurrent seats** (`MAX_SEATS = 8`, `apps/dooduel_core/src/session.rs:65`),
each its own process with its own wgpu render world on the one AMD RX 6700 XT / RADV host.
Run the smoke with **≥3** seats to validate the multi-render-world regime before trusting
4 in a live cycle.

**Already covered by existing tests (reuse, don't duplicate):** the real-click
create→start→board→pick lifecycle over a real `Session` (`gui_networked.rs`); the
synthetic-drag → canvas-ink path (`canvas_e2e.rs`); the offscreen render→PNG readback
(`capture.rs`); the two-process networked match to podium against the real binaries
(`e2e.rs`). The smoke's novelty is only their *combination* in one live process
(checkpoint 1).

**Operational dependency (hard).** Agents are much slower than humans, so the
`dooduel_server.toml` for a cycle **must widen the phase timers** — the archaeology lesson
(runs used draw 240→420→150 s, pick 60→180→30 s; the interactive 10 s pick auto-picks
before an agent responds, `qa-research-playtest-archaeology.md` §6.5). A tight-timer server
config will starve the seats regardless of a correct driver.

---

## 7. Out of scope (v1)

- **Auto-reconnect** (res-Q8) — keep seats supervised; a dropped socket loses the seat.
- **Custom-avatar drawing** (res-Q4/§3.3) — preset avatars via `click` only; the
  `CanvasKind::Avatar` scratch surface and the `stroke {canvas:"avatar"}` extension wait.
- **Event-push "your move" notifications** — agents poll `ui.md`/`screen.png` (~1 Hz); the
  batched-background-wake hazard is an *orchestration* concern (foreground poll loops,
  §C3), not a driver feature.
- **A web lane** (browser-driven `dooduel_web`) — a possible later addition; native
  offscreen is v1.
- **Compound/game-semantic verbs** — v1 keeps UI primitives so the real toolbar is
  exercised.

---

## 8. Concerns / blockers & assurances

- **C1 (well-supported by source; smoke confirms — not a blocker).** Whether an
  `is_active` Window-target (picking) camera coexists without error alongside an
  Image-target (readback) camera under a headless `RenderPlugin` with no winit surface. The
  no-panic path is traced in res-Q1 (in-repo: `buiy_pass`'s `ViewQuery<&ViewTarget>` never
  iterates a targetless view, `node.rs:75-99`; bevy_render 0.19: a surfaceless window is
  never extracted / has its `ViewTarget` stripped). No existing target combines exactly this
  composition, so it is smoke checkpoint 1. If it still surprises us, the fallback is a
  **scoped** picking-backend change (resolve the QA image's own pointer to its image camera,
  without re-enabling picking for genuine RTT groups) — real but small, low-probability, not
  pre-built. **No other framework or production app code change is anticipated** (the
  room-code and guesser-hint paths need none, res-Q3).
- **C2 (build ergonomics, known).** The large bevy binaries need
  `RUST_MIN_STACK=33554432` to build/run (rustc SIGSEGV during monomorphization otherwise).
  The example inherits this; the run command must set it.
- **C3 (orchestration, inherited).** Seat agents idle after single actions and lose batched
  background-task wakes — each seat needs one persistent **foreground** poll-act loop for
  the whole match (`qa-research-playtest-archaeology.md` §6.1/§6.3). A briefing/harness
  concern, not the driver, but it must be honored or seats go AFK.
- **C4 (concurrency budget).** 4 concurrent seats = 4 processes each with its own wgpu
  render world on the single AMD RX 6700 XT / RADV host; `MAX_SEATS = 8` is the ceiling but
  4 is the campaign's working size. The smoke's ≥3-seat run (§6) validates the regime; the
  wide-timer server config (§6 operational dependency) is a hard co-requirement.
- **Assurance J-a (honesty is inherited, not driver-enforced).** A guesser never sees the
  word pre-reveal because the **server** filters per-seat: its replica is populated only by
  the events the authority addresses to that seat (`Recipient::Seat`/`All`, the redaction is
  structural — a guesser's replica has no field for the secret, proven
  `apps/dooduel_mcp/src/lib.rs:904`). The driver renders whatever the replica holds, so the
  seat-dir honesty contract rides the same authority guarantee the `dooduel_mcp` seats do —
  the driver adds no redaction of its own and needs none.

---

## Change log

**rev-2.2 (2026-07-10)** — §res-Q5 correction: pre-fix, `set_value` did NOT fire the field's
`on_input` (the framework gap the W1 gate found — `Action::SetValue` mutated the editor + a11y
tree but emitted no `TextChanged`, so `route_text_input` never folded and the next reconcile
clobbered the edit). The framework now emits `TextChanged` on a value-changing `SetValue`
(`buiy_core::a11y::contract::honor_text_set_value`, mirroring the keyboard path), so the driver's
`set_value_role` `TextChanged` re-emit workaround is removed.

**rev-2.1 (2026-07-09)** — one-line §2.3 clarification requested by the plan review (plan
minor #5): the `consumed: K` index counts EVERY `\n`-terminated line (blank/malformed lines
consume their `K`, logged as skipped), so `K` never desyncs from the agent's appended-line
count. No other change; the driver's `tail_commands` is aligned to this in the plan.

**rev-2 (2026-07-09)** — folded both reviewers' APPROVE-WITH-FIXES (no blockers). Each item
re-verified against code as folded.

- **A [state isolation]** — §2.1/§3.3: the driver MUST set `DOODUEL_STATE_DIR=<seat_dir>/state`
  per process; without it N seats race one `state.json` and clobber the dev's real profile
  (`storage.rs:84-102`; `install_runtime` adds `StoragePlugin`, `lib.rs:1057`).
- **B [exact composition]** — §2.1: added the precise plugin table — `BuiyHeadlessPlugin`
  (keeps `A11yPlugin`, drops the winit `AccessKitAdapterPlugin` + `PointerInputPlugin`,
  `lib.rs:629-638,680-685,726`) + re-added `bevy::picking::PickingPlugin` +
  `buiy_core::picking::{PickingPlugin, BuiyPickingBackendPlugin}` + `InputPlugin` + the
  `capture.rs:191-217` render stack + `install_runtime`; named the `GlobalTransform` bridge
  provider (`CorePlugin`, `report.rs:113-114`) and `TransformPlugin`. **Added beyond the
  reviewers' list:** `ScrollInputPlugin` + `AnimationPlugin` for visual fidelity (the live
  `BuiyPlugin` app runs both) — flagged as a deliberate call. Stated plainly that no
  existing target combines this composition.
- **C [consumed ack]** — §2.3: `driver.log` entries carry a monotonic `consumed: K` index;
  explicitly kept `snapshot_report` **raw** (no synthesized affordance layer), with a new
  §3.4 saying orientation lives in briefings (R3 updated).
- **D [Join flow]** — §1 R5, §3.2, §res-Q5, §6 step 3: corrected — "Join a room" on Home is
  `Msg::GoJoin` navigation (`home.rs:99`); the CTA is "Join room" `Msg::SubmitJoin` on the
  Join screen (`join.rs:39`).
- **E [avatar scope]** — §3.1/§3.3/§7: `stroke` targets `CanvasKind::Game` only; custom-avatar
  drawing out of scope (KI-20; custom avatars don't wire in M1, `lobby.rs:131-139`); named the
  future `stroke {canvas:"avatar"}` extension.
- **F [concurrency]** — R2/§6/§C4: named the host adapter (AMD RX 6700 XT / RADV, **not**
  lavapipe); stated the 4-seat working size / `MAX_SEATS=8` ceiling (`session.rs:65`); extended
  the smoke to ≥3 concurrent seats; cross-referenced the "widen phase timers" operational
  dependency.
- **G [name examples]** — §3.1/§3.2: fixed to `Brush size 3/6/11/18` (`canvas.rs:43`; no
  "size 2"), UPPERCASED word-choice names (`in_game.rs:818`), state-dependent theme toggle
  `"Dark"/"Light"` (`widgets.rs:214`); added the case-sensitive STRICT-single-match note.
- **H [smoke form]** — §6: decided — a committed `#[ignore]` GPU-lane integration test that
  self-spawns `dooduel_server`; added the manual run command; recommended subprocess-per-seat.
- **I [checkpoint + C1]** — §6 checkpoint 1 broadened (boot + readback + non-empty snapshot +
  a resolving click); C1 reframed to "well-supported; smoke confirms" with the in-repo
  (`node.rs:75-99`) + bevy_render-traced no-panic path; fallback note made honest (a scoped
  change, not a blind 5-liner).
- **J [assurances]** — §8 J-a (honesty inherited from the server's per-seat replica
  filtering, `lib.rs:904`) + §res-Q3 J-b (guessers read revealed letters + countdown from the
  text section, `in_game.rs:353-364,392`; blanks render as spaces → cross-read the screenshot).
- **K [edge behaviors]** — §2.3 malformed lines logged-and-skipped (`seat_driver.py:199`);
  §2.2 readback-pacing note (real-time Tick folds only real elapsed; the ~1 Hz stall is
  acceptable — don't "fix" it).
- **L [label-only click]** — §6 checkpoint 5 also clicks `Color 3` + `Brush size 6` to
  exercise the `.label()`-derived accessible-name path (`a11y/mod.rs:30`,
  `compute_accessible_name`).
