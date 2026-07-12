# Buiy First-Class Web (WASM) — Epic #143 Reconciled Against `origin/main` 6e07954

**Date:** 2026-07-11
**Status:** active

> **Method & provenance.** Produced by a 10-dimension multi-agent reconciliation of GitHub issue
> #143 against the code at `origin/main` `6e07954` (run in a fresh worktree cut from `origin/main`,
> *not* the stale shared checkout), with adversarial "does it actually work end-to-end?" verifier
> passes on the two highest-overclaim-risk dimensions (a11y sink, IME/OSK), plus orchestrator
> spot-checks of the load-bearing claims (band 16-attr guard, `web_sink.rs` test/CI coverage,
> networking/router absence in `Cargo.lock`, the governing spec's residual). Every verdict is
> anchored to `file:line` + landing PR.

## 1. Headline meta-finding

Issue #143 ("Epic: Buiy first-class web (WASM)") was authored against baseline **4010753 (PR #85)** and was **never reconciled against the Dooduel web-reach waves W1–W5 (#94–#102) and the F4b render fold** that have since landed on `origin/main` 6e07954. At audit time the *shared* checkout was still pinned at the stale 4010753, ~55 commits behind `origin/main` — this audit was therefore run in a fresh worktree cut from `origin/main`. **The staleness trap is recurring: the repo already contains a `...audit-1-full-11-dimensions-STALE-BASE-4010753.json` artifact from a prior audit that hit the same baseline. Any future auditor must re-baseline against `origin/main` before reading, or it drifts stale within weeks.**

### 1a. There is already a governing spec — and it already tracks the residual

The most important reconciliation the dimension agents *missed* (they read the code, not this spec): **[`docs/specs/2026-06-30-buiy-browser-reach-widening-design.md`](../specs/2026-06-30-buiy-browser-reach-widening-design.md) already governs #143's Items 1/3/4/5/9.** It is a prototype-validated spec of 9 decisions (D1–D9) → 5 waves (W1–W5), and its own § 3 status reads: *"4 waves LANDED + verified on `main`; **the only remaining item is D3 (`Rgba16Float` float-less fallback), DEFERRED**"* because no dev/CI WebGL2 adapter can even reproduce the float-less path (all expose the float extensions), so it only hardens old/low-end mobile. Its D7 is self-labeled **"LANDED (W4, read-only v1)"** — precisely this audit's a11y downgrade.

So the true reconciliation is three-way, not two-way:

- **#143 Items 1/3/4/5/9/10** → owned by the 2026-06-30 spec, **largely LANDED**. Residual = **D3 Rgba16Float** (the spec's one acknowledged deferral) **plus the verification/operability debt this audit surfaces** (a11y CI guard + real-AT reach; automated IME test + real-device OSK; a11y inbound actions + live focus — the spec *named* these as W4/W5 follow-ups but did **not** schedule them; web golden; runtime macOS-modifier; Msaa::Off).
- **#143 Items 6/7** (networking, URL router) → **outside the 2026-06-30 spec's scope entirely**, genuinely 0% landed — two net-new subsystems.
- **#143 Item 8** (OG/SSR) → correctly out of scope (app backend).

**Bottom line: this is not a "build web support" campaign. It is (a) close the 2026-06-30 spec's residual + pay down its verification debt, and (b) two net-new subsystems (networking, router) that need their own specs. Build the campaign ON the 2026-06-30 spec, not from #143's stale checklist.**

Net status across the epic's 10 audited dimensions:

- **Substantially LANDED or PARTIAL (7):** Item 1 (WebGL2 reach), Item 2 (CI proof-of-paint), Item 3 (a11y sink), Item 4 (IME/OSK bridge), Item 5 (clipboard), Item 9 (browser matrix/touch), Item 10 (web-correctness follow-ups). The epic asserts most of these are un-started; they are not.
- **Genuinely OPEN (2):** Item 6 (wasm networking / WebSocket-into-ECS) and Item 7 (URL router). Both are 0% landed — the epic is factually correct here.
- **Correctly OUT OF SCOPE (1):** Item 8 (OpenGraph/SSR). The epic's own disposition ("not a Buiy problem") is confirmed.

**The single most consequential correction is the epic's "long pole."** The epic names the **browser a11y sink** as the *XL biggest remaining blocker* and mandates disclosing "reaches ZERO AT" in every web milestone. That is now **stale**: PR #100 (164f347) landed the exact Route-A hidden-ARIA-overlay the epic predicted, so an outbound browser AX tree exists over the canvas. The blocker is **materially de-risked from XL to ~M** — but not closed (real-screen-reader reach is unproven, it is read-only, and it has zero CI coverage).

Three further corrections invert the epic's premises outright:
- **B2 (17→16 vertex-attr fold): DONE.** The epic says the band pipeline still declares 17 attributes; `origin/main` declares 16 with two CI guard tests. (F4b, task-map #137 408b6c2, on #94.)
- **WebGL2 second artifact: LANDED and CI-enforced**, not "still needed" (#94 0a1ca73 + navigator.gpu loader).
- **CI proof-of-paint: real in-browser paint IS enforced on every PR** for the WebGL2 leg via SwiftShader (#94), so the epic's blanket "web stays a manual visual gate" is now true only for the WebGPU/Tint leg.

The epic, in short, **underclaims done-ness** on Items 1/2/3/4/5/9, and the genuine residual is narrower and better-characterized than its estimates.

## 2. True-state table (D1–D10)

| Dim | Epic item | Epic's stated status | ACTUAL status on `origin/main` | Key evidence (file:line + PR) | Verification depth / caveat |
|---|---|---|---|---|---|
| **D1** | Item 1 — WebGL2 fallback (B2 repack, W1 float compositor, navigator.gpu loader, 5-shader naga validation, DPR clamp) | Untouched / all missing | **PARTIAL — mostly landed** | Band fold 16-attr `render/primitive.rs:280-420`, guard tests `:835`,`:857`; `band.wgsl:22-45`; loader `tools/build-web.sh:77-95` + `pages.yml:90`; smoke `ci.yml:377-427` (#94 0a1ca73, F4b 408b6c2) | B2 & loader STALE_DONE. Genuine residue: W1 Rgba16Float compositor has NO WebGL2 fallback (`compositor.rs:439`, unconditional); gradient + effect shaders never instantiated by the button-only smoke; no DPR `max_texture_dimension_2d` clamp. |
| **D2** | Item 2 — CI proof-of-paint (compile gate, tint-CLI lane, pixel-paint, Pages deploy) | Web stays a manual visual gate; tint-CLI = PRIORITIZE follow-up | **PARTIAL** | Compile gate `ci.yml:402-405` + `deny.toml:33`; WebGL2 paint gate `run-webgl2.mjs:58-116` (no skip path); Pages `pages.yml` (#94 0a1ca73, #105 05031f3) | WebGL2 leg: real non-blank paint + 0 GLSL-ES link errors enforced every PR — but coarse variance floor, NOT a golden. WebGPU leg SKIPS (no adapter, `run.mjs:96-101`). **tint-CLI lane still absent** (`ci.yml:369` defers it) — the Tint-strict uniformity class is CI-unverified. Source-read only; no live green CI run observed. |
| **D3** | Item 3 — Browser a11y sink (**XL, the biggest blocker**) | accesskit ships no web adapter → reaches ZERO AT silently | **PARTIAL (high overclaim risk) — outbound sink landed** | `a11y/web_sink.rs` (216 lines) `rebuild()` `:125-216`, registered `mod.rs:320-321` cfg(wasm32) (#100 164f347) | **Downgrade applied.** Outbound hidden `#buiy-a11y-tree` ARIA subtree landed (Route A, one source→two sinks off `A11yNodeView` snapshot, NOT `build_tree_update`). BUT real-AT reach proven only by a **one-time manual CDP `getFullAXTree` dump**; **zero automated tests, zero CI coverage** (cfg-wasm, not in web-smoke → can silently rot); **inbound actions + live focus unbuilt** (`data-buiy-entity`/`data-buiy-focused` are unconsumed stubs). Read-only, not operable. |
| **D4** | Item 4 — Browser IME + mobile OSK | Only Latin keydown survives; CJK/OSK do NOT work; needs a shim | **PARTIAL (downgraded from LANDED)** | `text/edit/web_ime.rs` (368 lines), registered `text/mod.rs:116-117` cfg(wasm32) UNCONDITIONAL (#102 f37c6fa) | The prescribed hidden-`<input>` shim shipped and is always-on for wasm; desktop keyboard + **synthetic** composition observed once. BUT **mobile OSK NEVER tested on a real device** (only headless *desktop* Chrome, which cannot raise an OSK); **CJK proven with synthetic events only** (no real Pinyin/JP engine); `ime_position` untracked; **zero tests, no CI lane** exercises it. |
| **D5** | Item 5 — Cross-app clipboard | In-app-only (MemClipboard); needs async-trait rework OR cached-read bridge | **PARTIAL — copy landed, paste best-effort** | `WebClipboard` `text/edit/clipboard.rs:190-271`, wired `text/mod.rs:338-341`; async latch `:198-231` (#99 93f370e) | Cross-app COPY reaches OS via `navigator.clipboard.writeText` (STALE_DONE; the epic's own "cached-read bridge" is exactly what shipped, no trait rework). Cross-app PASTE is explicitly **best-effort** (async `OS_TEXT_LATCH`, one-value-stale, degrades to in-app copy where denied). Guaranteed paste (DOM `paste`-event bridge) unbuilt. No automated test of the real path. |
| **D6** | Item 6 — Wasm networking / WebSocket-into-ECS | Entirely future work | **OPEN — genuinely unbuilt** | grep for `websocket\|ewebsock\|tungstenite` over crates+examples+Cargo.lock = ZERO; `buiy_core/Cargo.toml:103-116` has no WebSocket web-sys features | Epic is factually correct. No transport, no ECS poll bridge, no reconnect logic. Networking explicitly out of the 2026-06-25 wasm design. Only prior art is the **unmerged** Dooduel M1 branch (correctly excluded). |
| **D7** | Item 7 — URL router (History API, shareable routes) | Future work, S–M | **OPEN — 0% landed + scope decision precedes code** | grep `pushstate\|popstate\|window().history` = ZERO; no "History" web-sys feature. Naming traps: `buiy_view/src/router.rs:1-111` = event→Msg router (#106/#111/#127); gallery `ScreenRouter` `shell.rs:1696-1727` = in-app switcher | Two same-named artifacts are NOT URL routers. Foundation spec `2026-05-07-buiy-foundation/cross-cutting.md:32` currently declares "History API / URL routing" **OUT of scope** — a spec reversal must precede any code. MVU `add_model`/`enqueue`/`replay` + NavModel give a clean hook. |
| **D8** | Item 8 — OpenGraph / SSR-for-canvas | NOT a Buiy problem; keep OUT | **OUT OF SCOPE — confirmed** | Repo-wide grep `opengraph\|og:\|SSR\|prerender\|axum` = zero Buiy-owned code (only prior-art refs); `2026-05-07-buiy-foundation/README.md:53` "no SSR"; wasm spec `2026-06-25-...:33` restates | Epic's disposition is correct. The three web hosts (`gallery_web/index.html`, `buiy_web`, `dooduel_web`) are bare single-canvas pages — blank-unfurl by construction. Belongs to the **app backend**, not the framework. Recommend closing this sub-item explicitly. |
| **D9** | Item 9 — Broaden browser matrix + reach quantification | Broad reach REQUIRES WebGL2 (missing); touch-tap deferred; quantify audience | **MIXED — precondition satisfied** | WebGL2 build #94 0a1ca73 (`build-web.sh:25-77`, both example features); touch-tap `picking/backend.rs:52` + `activation.rs:96,124` with real-path tests `pointer_events_c3b.rs:108,127` (#98 0ca3716) | The "REQUIRES WebGL2" precondition is STALE_DONE — it shipped + is CI-enforced. Touch-tap landed with real headless regression tests + real-browser root-cause. Remaining: **D3 Rgba16Float float-less fallback** (infra-blocked, mobile-tail-only) + **audience quantification** (a product decision, not code; the code precondition is moot). |
| **D10** | Web-correctness follow-ups (Msaa::Off, runtime macOS-modifier, sRGB confirm, D5 trim+#107 size) | Restore all four to worklist | **MIXED** | Msaa: bare `Camera2d` → default `Msaa::Sample4` (`shell.rs:1686`, `render/mod.rs:390`), never evaluated for web. Modifier: still compile-time `cfg!(target_os="macos")` `input.rs:517,541`, `keymap.rs:128` → wasm always gets Ctrl. sRGB: self-adapting `main_texture_format()` (`compositor.rs:716`), correct but **no web-vs-native test**. Size: #107 d1c388a → 15.7/16.6 MB, bevy_pbr NOT in resolved graph | **Msaa::Off and runtime-modifier are genuinely OPEN** (browser macOS users get Ctrl not Cmd — real breakage). sRGB functionally correct, unguarded. **D5 size/trim = DONE** (`cargo tree -i bevy_pbr` → nothing). |

## 3. Genuine residual-gap list (deduped, for first-class web)

### A. Real code gaps

| Gap | Dim(s) | Effort | Prototype-first | Blockers |
|---|---|---|---|---|
| **W1 effect-compositor Rgba16Float WebGL2 fallback** — `EXT_color_buffer_float` feature-gate + Rgba8Unorm (or scaled-int) substitute; wire an opacity<1 group fixture into the WebGL2 smoke. Any app using group opacity/blur is unproven, likely broken, on WebGL2. | D1, D9(D3) | **M** | **Yes** | Must empirically determine float-RT renderability on real WebGL2 adapters AND on CI SwiftShader (both currently expose the extensions, so the float-*less* path can't be reproduced without new infra). |
| **Complete the WebGL2 shader-conformance coverage** — smoke only compiles the button's shaders; `gradient.wgsl` + effect-compositor pipelines are never instantiated, so their WGSL→GLSL-ES translation is unvalidated. Spawn a gradient bg + effect group in the smoke fixture (or smoke `gallery_web`) and couple to the existing compile/link-error assertion. | D1, D2 | **S** | No | None — extend `run-webgl2.mjs` fixture. |
| **tint-CLI conformance lane** — run WGSL through Dawn's standalone `tint` in CI (no GPU) to gate the Tint-strict uniformity class that neither native-naga (lenient) nor SwiftShader/GLSL-ES catches. | D2 | **M** | No | CI tooling only: vendor a pinned `tint` binary (like the binaryen install). No upstream blocker. |
| **CI regression guard for the a11y sink** — headless CDP `getFullAXTree` assertion (Chrome already launched by web-smoke) or a `wasm_bindgen_test` over `rebuild()`. Today any refactor of `A11yNodeView`/`aria_role`/`rebuild()` silently breaks the whole sink. | D3 | **S** | No | None — cheap extension of the existing chromium harness. |
| **Inbound AT action bridge** — DOM click/focus/keydown on a `#buiy-a11y-tree` node routed back via the existing `route_action_requests` path, keyed off the already-emitted `data-buiy-entity`. Without it the web AX experience is read-only (WCAG operability gap). | D3 | **M** | No | None architectural — outbound handle + native inbound router already exist. Reuse the W5 double-activation policy. |
| **Live focus bridge** — `aria-activedescendant` / real `.focus()` on the focused node (currently a passive `data-buiy-focused` marker). | D3 | **S** | No | Must reconcile focus ownership between a11y overlay, canvas, and the W5 IME `<input>` to avoid focus fights. |
| **Guaranteed cross-app PASTE** — DOM `paste`-event bridge reading `clipboardData` inside the gesture; current async latch is best-effort/one-value-stale. | D5 | **M** | No | Seam: paste fires in the DOM gesture but Bevy `Update` runs on the rAF tick. Standard web API, no upstream blocker. |
| **Runtime macOS-modifier detection** — thread a runtime platform flag (navigator.platform/userAgentData on wasm) into `default_keymap_for_platform()` + `command_modifier_held()`, replacing the `cfg!` consts. Browser macOS users currently get Ctrl not Cmd. | D10 | **M** | No | None technical; needs a wasm-target test (host `cfg!` can't exercise unknown-OS). |
| **wasm WebSocket transport behind a cfg-selected provider seam** — web-sys WebSocket (+ MessageEvent/BinaryType/CloseEvent) on wasm, tokio-tungstenite/ewebsock native. ewebsock is the pragmatic unified choice. | D6 | **M** | **Yes** | None upstream-gated. Needs an echo-server fixture; CI likely can't exercise the real socket without a server harness. |
| **WebSocket→ECS non-blocking poll bridge** — Resource holding the transport handle + per-frame system draining inbound to `EventWriter` / flushing outbound, zero await in systems (async confined to `spawn_local`→flume/crossbeam channel). | D6 | **M** | **Yes** | Depends on the transport seam. Channel must be wasm-single-thread-compatible. |
| **Reconnect/backoff + honest connection-state enum** as an ECS resource. | D6 | **S** | No | Depends on transport + bridge; straightforward once they land. |
| **History-API Router seam** — web-sys History (push/replaceState) + popstate listener; Router resource mapping URL↔MVU model; back/forward. Building blocks exist (MVU + gallery NavModel). | D7 | **M** | No | **Scope decision precedes code:** foundation spec declares URL routing OUT — needs a spec to reverse/carve an exception + design the seam + native fallback. |
| **Shareable per-entity route grammar** (serializable per-entity ids in URL). | D7, (Item 8 app-side) | **M** | No | Co-design with the id scheme; no upstream blocker. |
| **Automated tests for the web IME bridge** — wasm-bindgen-test/Playwright injecting synthetic keydown/composition into `#buiy-ime-input`, asserting editor state, wired into web-smoke. | D4 | **M** | No | Needs a wasm test harness in the web-smoke job. |
| **`ime_position` tracking** — position the hidden `<input>` at the caret so the candidate window follows it. | D4 | **S** | No | Read `Window.ime_position` (already an output seam). |
| **Web paint golden (upgrade from variance floor)** — reuse `buiy_verify` golden infra against the SwiftShader screenshot to prove the web build paints *correctly*, not merely non-blank. Also closes the sRGB "confirm vs native" task. | D2, D10 | **M** | **Yes** | SwiftShader determinism across runners → needs a pinned software stack (analogous to the pinned-lavapipe recipe). |
| **navigator.gpu loader selection test** — headless test loading `dist-web/` with `navigator.gpu` absent, asserting the webgl2 bundle is selected (the `?force=` hook exists). | D1, D2 | **S** | No | None. |
| **Post-deploy Pages smoke** — load the published URL and assert it paints, so a base-path/loader regression fails the deploy. | D2 | **S** | No | None — `run-webgl2.mjs` is reusable against the deployed URL. |
| **Msaa::Off evaluation for the web main pass** — measure 4x vs Off on a weak/tiled-mobile GPU, then set or document. | D10 | **S** | No | Needs a real weak-GPU/mobile measurement; code change is trivial. |
| **DPR `max_texture_dimension_2d` clamp** — clamp surface/scale_factor to the adapter's texture cap at launch. F9 resolved the sizing artifact but left surface-reconfigure-at-large-sizes open. | D1 | **M** | **Yes** | Needs empirical real-device testing (actual reported cap vs physical surface at high DPR) before deciding to clamp; risk of over-clamping crispness. |
| Partial DOM `code`→KeyCode table completion; HTML/image clipboard OS flavors; brotli precompression | D4, D5, D10 | **S** each | No | Mechanical / optimization-only; low priority. |

### B. Product decisions (not code)

- **Audience quantification of the no-WebGPU slice** (Firefox-Linux, older Safari, older Android) against the target app's user profile, recorded in a decision log. **S, no blocker.** The code precondition (ship WebGL2) is already moot — this is now documentation/prioritization, not gating work. (D9)

### C. Explicitly out of scope (do NOT absorb into the framework)

- **OpenGraph / SSR read-layer** — belongs to the consuming app backend (Bad Apple Brawl / Dooduel). Absorbing axum/SSG/OG would breach the documented foundation non-goals (networking/transport/persistence = app concern). Recommend closing Item 8 with a one-line pointer. (D8)

## 4. Verification-depth caveats — where "landed" ≠ "reaches a real user"

These are the claims a campaign **must not repeat as done** without producing the named evidence. This is precisely the "a11y/mobile/IME can silently look complete" trap the epic itself warns about.

1. **A11y real-screen-reader reach (D3).** Proven only by a **one-time manual CDP `Accessibility.getFullAXTree` dump** — a proxy for what an AT *computes*, not what a real AT *announces*. No NVDA/VoiceOver/JAWS/TalkBack run exists anywhere. A present AX tree is necessary but not sufficient (announcement, focus tracking, live-region politeness, virtual-cursor nav all unverified). **Honest claim requires:** a real-AT pass on each of NVDA (Windows) / VoiceOver (macOS+iOS) / TalkBack (Android), driven manually against `gallery_web`, findings recorded. Prototype-first (drive, observe, then decide fixes).

2. **Mobile soft-keyboard (D4).** The OSK-raise rests entirely on the browser standard that focusing a text `<input>` raises the OSK — verified only in headless **desktop** Chrome, which **cannot surface a mobile OSK**. The touch-only focus policy is unimplemented. **Honest claim requires:** on-device (or genuine touch-emulation) confirmation that focus raises the OSK and typing flows through the bridge.

3. **CJK / real IME composition (D4).** Proven only with **synthetic** `compositionstart/update/end("你好")` — no real Pinyin/JP/dead-key engine ever driven; `ime_position` candidate-window alignment unimplemented. **Honest claim requires:** a rig with an actual OS IME (or dead-key layout) exercising the `compositionupdate/end` path end-to-end.

4. **Cross-app clipboard (D5).** Copy verified once manually in headless Chrome; paste is architecturally best-effort (rAF-tick can't guarantee gesture timing). **Honest claim requires:** a gesture-driven copy/paste assertion in the Playwright harness with clipboard permissions granted, plus the DOM paste-event bridge for guaranteed paste.

5. **WebGL2 visual correctness (D2/D10).** CI proves the WebGL2 build renders **non-blank with 0 shader errors** — NOT that it renders *correctly* (coarse variance floor, no golden). WebGPU paint is **not proven on CI at all** (smoke skips without an adapter). sRGB correctness rests on human eyeballing. **Honest claim requires:** a web golden (pinned SwiftShader) or a documented one-time native-vs-browser side-by-side.

6. **No CI observed live.** All D1/D2 verdicts are **source-read only** — no live green CI run was observed. A campaign should confirm the enforced gates actually run green on a real PR.

## 5. Proposed campaign shape (skeleton)

The residual is **not** a fresh "build web support" campaign — it is a **hardening + finishing + two net-new subsystems** campaign. Suggested shape:

**Phase 0 — Reconcile the record (docs, no code).** Rewrite/split #143 against `origin/main` (see §7). Mark the W1–W5 landings DONE. Produce a `docs/reports/2026-07-11-web-first-class-audit.md` (this report). Log the audience-quantification product decision. Close Item 8 out-of-scope.

**Phase 1 — Verification debt (fast, high-value, mostly S).** This is the cheapest way to convert "landed" into "honestly first-class." Non-prototype:
- a11y CI regression guard (S) + navigator.gpu loader-selection test (S) + post-deploy Pages smoke (S) + complete WebGL2 shader-conformance coverage (S) + web IME test harness (M).
- Standalone: the **real-AT / mobile-OSK / real-IME manual verification passes** (§4.1–4.3) — **prototype-first** (drive real ATs/devices, record, then decide fixes). These are the highest-risk unproven claims; do them early so the campaign knows what it's actually fixing.

**Phase 2 — WebGL2 robustness (prototype-first where infra-blocked).**
- W1 Rgba16Float float-less compositor fallback (**M, prototype-first**) — the single largest genuine code gap; empirically determine float-RT availability first.
- DPR texture-cap clamp (**M, prototype-first**, needs real-device data).
- tint-CLI lane (M) + web golden (M, prototype-first for SwiftShader determinism).

**Phase 3 — A11y operability.** Inbound action bridge (M) + live focus bridge (S) — turns the read-only sink into an operable app. Sequence after the real-AT pass tells you whether the outbound tree announces correctly.

**Phase 4 — Interaction finishing.** Guaranteed cross-app paste (M), runtime macOS-modifier (M), `ime_position` (S), Msaa::Off eval (S). Independent, parallelizable.

**Phase 5 — Net-new subsystems (each a self-contained prototype-first cycle).**
- **Networking (Item 6):** one prototype-first cycle covering transport seam + ECS poll bridge + reconnect (the three form one subsystem; the "no await in ECS" design is load-bearing). Ideally reconcile with the **unmerged Dooduel M1** networking prototype rather than building blind.
- **URL router (Item 7):** **spec-first** — a scope-reversal spec (foundation currently excludes routing) MUST precede code. Then a small implementation on the MVU/NavModel hook. Co-design the per-entity route grammar with any Item-8 app-side permalink work.

**Track-don't-build (upstream/infra-blocked):**
- CI-enforced **WebGPU** paint (needs a self-hosted GPU runner or Dawn exposing a headless software adapter — every SwiftShader/lavapipe combo currently yields `requestAdapter()→null`). **L, infra.**
- `accesskit_web` upstream adapter (Buiy correctly routed around it; track only if upstream revives it).
- winit#4424 / bevy#13168 (both worked around in userspace; track for eventual removal of the shims).

## 6. Cross-references — #142 (native widget catalog) and #141 (native-mobile feasibility)

**Convergence with #142 (native widget catalog):**
- **Image primitive.** #142 lists an "Image primitive" as missing, but a **RasterImage / ImageNode** path already exists on the Dooduel line (the drawing-canvas subsystem per the campaign memory) — the catalog's "missing Image" is likely already satisfiable; audit #142 against the same `origin/main` baseline before building it.
- **WebSocket bridge is shared infra.** The Item-6 WebSocket→ECS poll bridge is exactly what a multiplayer widget-catalog demo (and Dooduel) needs — build it once, in `buiy_core` behind the provider seam, not per-app.
- **A11y sink feeds both.** The `A11yNodeView` snapshot that drives the web sink is the same source the native winit sink and the widget-catalog test-tree consume — any widget added to the catalog automatically populates the web AX tree, so #142 and D3 share the role-map maintenance burden.

**Convergence with #141 (native-mobile feasibility):** web and mobile share a large upstream-blocker surface — building for web pre-pays much of mobile:
- **Band 16-attr fold** (D1/B2) is required by *both* WebGL2 and mobile GLES — already landed, benefits both.
- **Rgba16Float float-less fallback** (D1/W1) is the *same* gap on low-end mobile GPUs lacking `EXT_color_buffer_float` — solve once for both.
- **arboard cfg-gating** (clipboard, B1) is shared — the wasm `WebClipboard` and a future mobile clipboard both sit behind the same `ClipboardProvider` seam.
- **Touch activation** (D9, #98) is shared code (`sync_pointer_location_on_button` + `touch_tap_activates`) — landed for web, directly reusable on native mobile.
- **IME/OSK + a11y** (D4/D3) — the hidden-`<input>` OSK model and the ARIA-overlay/AccessKit split are the web instances of problems mobile also faces; the real-device verification passes (§4) should be run on mobile browsers to double-count toward #141.

**Recommendation:** treat W1 float-less fallback, the touch path, the clipboard/IME/a11y seams, and the WebSocket bridge as **shared web+mobile+catalog work items**, not web-only — sequence them so one implementation discharges obligations across all three epics.

## 7. Recommendation on #143 itself

**Do not close it, and do not leave it as-is. Rewrite it against `origin/main` and split it.** The epic is ~40% stale-wrong (it asserts un-started what has landed) and its effort estimates (notably the XL a11y "long pole") no longer hold. Leaving it invites re-building landed work and repeating unverified claims. Specifically:

1. **Rewrite the body against `origin/main` 6e07954.** Mark the W1–W5 landings DONE with commit anchors (#94, #98, #99, #100, #102, #105, #107), reframe each item's residual to match §3, and **correct the two factual errors**: B2 is 16 attrs not 17, and the a11y sink reuses the `A11yNodeView` snapshot, not `build_tree_update`.

2. **Split into four child issues** along natural seams:
   - **#143a "Web hardening & verification debt"** — Phases 1–4 above (Items 1/2/3/4/5/9/10 residuals). Mostly S/M, some prototype-first. This is where "first-class" is actually earned.
   - **#143b "Wasm networking (WebSocket-into-ECS)"** — Item 6, one prototype-first subsystem, reconciled with the Dooduel M1 prototype.
   - **#143c "URL router"** — Item 7, **spec-first** (scope-reversal decision gates it).
   - **Close Item 8 out-of-scope** with a pointer to app-backend ownership (don't carry it as an open child).

3. **Demote the "long pole" framing.** The a11y sink is de-risked from XL-blocker to ~M-finishing; the epic's mandatory "reaches ZERO AT" disclosure is stale and should be replaced with the honest narrower one: *"an AX tree is exposed outbound/read-only and observed once via CDP; real-AT reach unverified; inbound actions and live focus not built; no CI guard."*

4. **Record the process finding** in the rewrite: the stale-worktree-baseline trap (§1) — future auditors must read via `origin/main`, and the epic must be re-baselined whenever a web wave lands, or it drifts stale again within weeks.