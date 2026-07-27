**Date:** 2026-07-11
**Status:** active

# Native mobile (iOS + Android) feasibility for Buiy

One-shot feasibility investigation: *could Buiy ever target native iOS/Android?* Maps
the current stack against what a native mobile port requires, then validates the
load-bearing claims against current upstream sources (crate versions, spec minimums,
open issues) as of 2026-07-11.

**Investigation base:** `main @ 4010753` (WebGPU/browser v1). **Re-verified** against
`main @ 6e07954` — every in-repo anchor below still holds on current `main`. Framework
`file:symbol` citations are line-drift-robust; external facts carry source URLs (see
§10 Sources). This is a *desk study* — nothing was built or run on a device.

**Method:** two multi-agent workflows — a 6-dimension subsystem probe (render/GPU,
windowing+input+lifecycle, text+IME, a11y, platform-services+build, ecosystem) with
accuracy + completeness critics, then a 6-agent validation pass (5 web fact-checks +
1 prior-art corpus scan). Full agent ledger lives in the session transcript.

---

## 1. Verdict

**Native mobile is feasible, and Buiy's substrate is unusually well-positioned for
it. The work is *unbuilt*, not *blocked*.** Nothing in the architecture forecloses
iOS/Android; the render/text/layout core is already target-agnostic in the ways that
matter, and the WebGPU/WebGL2 browser port already de-risked much of the same
portability work. The hardest remaining items — robust soft-keyboard/IME and OS
accessibility — are **upstream (winit/Bevy) maturity problems**, not Buiy code. One Rust
peer toolkit (Slint) already ships this exact class of UI to both app stores, and
Buiy sits *above* iced/floem/xilem on mobile-readiness because it inherits Bevy's
mobile targets.

Effort split:
- **"Runs and draws on a device"** — *minor-to-moderate*: mechanical GLES fixes,
  clipboard cfg gate, mobile build scaffold, first-tap touch activation.
- **"A shippable, accessible, text-capable, battery-respecting app"** — *moderate-to-large*,
  and its two hardest items are gated on upstream Bevy/winit rather than on us.

The **biggest sleeper is not on the "does it launch" axis at all**: Buiy inherits
Bevy's default continuous render loop and would peg CPU+GPU on a foreground-idle
phone. That is a one-line fix that maps cleanly onto the MVU dirty-state model — but
it must be done, and the static-portability analysis nearly missed it.

---

## 2. What the validation pass changed

The validation overturned two claims from the first-pass investigation and refined
several. Recorded here in the interest of honesty (full ledger in §9):

- **winit soft-keyboard was NOT "unimplemented on mobile" (REFUTED).** winit 0.30.13's
  `Window::set_ime_allowed(true)` *is* documented to show/hide the soft keyboard on
  iOS and Android (it is a no-op only on Web/Orbital). The real risk is narrower and
  still real: it is *app-driven* (winit has no focus model, so nothing raises the
  keyboard automatically) and **unreliable on Android** — open bug
  [winit #4126](https://github.com/rust-windowing/winit/issues/4126) (`showSoftInput`
  ignored on Pixels), with the umbrella [#1823](https://github.com/rust-windowing/winit/issues/1823)
  still open. The peers that ship mobile (Slint, Makepad) still wrote their own
  JNI/UIKit text layer rather than trust it. And winit 0.31 *deprecates*
  `set_ime_allowed` for `request_ime_update` — a forward migration. So: **flaky
  starting point, not absent.**
- **accesskit version attributions were scrambled (REFUTED).** Android multiplexing
  has been in `accesskit_winit` since **0.24.0** (2025-03-06) — it ships in the 0.32.x
  Bevy pins, but as an **opt-in, non-default feature** (`accesskit_android`) that Bevy
  does not enable. iOS multiplexing arrived in **0.33.0** (2026-05-11). So the two
  mobile unblocks differ: **iOS** needs Bevy to ride `accesskit_winit ≥ 0.33`
  (**already done on Bevy `main`** → expected ~Bevy 0.20); **Android** just needs
  Bevy to flip on the already-present `accesskit_android` feature. Still zero mobile
  OS-AT reach *today* on Bevy 0.19 — but nearer than "hard blocker" implied.
- **CONFIRMED, load-bearing:** the continuous-redraw battery cost, the total absence of
  audio, wgpu Metal-iOS/Vulkan-Android as first-class, the 16-vertex-attribute floor,
  the arboard hard build error, and Slint as the one App-Store-grade Rust precedent.
- **Refined:** "TBDR" is imprecise — say **tile-based**; only Apple is strictly TBDR
  among Apple/Adreno/Mali (Adreno is tile-based binning with a direct-mode fallback,
  Mali's classification is disputed). The off-screen-then-composite tile-flush cost
  argument holds for all three.

---

## 3. Dimension map

| Dimension | Verdict | The crux |
|---|---|---|
| Render / GPU | **minor-work** | wgpu auto-selects Metal/Vulkan; shaders already uniformity-clean. Only the 17-attr `band.wgsl` (GLES/min-spec-Vulkan) + `Rgba16Float` effect target (old GLES) need fixing — both already designed as deferred "WebGL2 reach" work. |
| Windowing / input / lifecycle | **moderate-work** | bevy_picking already gives per-finger touch for free; surface-loss is architecturally handled. First-tap activation, safe-area insets, and on-device suspend/resume are unbuilt/unproven. |
| Text / IME / soft-keyboard | **moderate-work** | Engine (cosmic-text, embedded fonts, IME composition) is portable as-is. The mobile *input surface* — raising a reliable soft keyboard, `inputmode`, touch selection handles — is the unbuilt part, partly gated on winit maturity. |
| Accessibility | **blocked-today, upstream-gated** | Buiy's a11y producer is winit-free and needs ~zero change; the OS sink is pinned to `accesskit_winit 0.32` via Bevy → no mobile AT today. Unblocks on a Bevy version bump (iOS) / feature flip (Android). |
| Platform services / build | **moderate-work** | Fonts + shaders embedded (no asset-bundling pain). arboard is a hard build error on mobile (trivial cfg fix). Zero mobile scaffold (no android-activity feature, entry point, NDK/Xcode, CI lane). |
| Operational (power/thermal/audio) | **real, under-weighted** | Continuous redraw drains battery; GPU-side 60Hz floor unmodeled on tiled mobile GPUs; no audio, haptics, memory-pressure hooks, or background-kill state restoration. |

### 3.1 Render / GPU — `minor-work`

Buiy requests **no** wgpu backend/features/limits/present-mode anywhere
(`crates/buiy/src/lib.rs` `BuiyRenderPlugin`; format/MSAA read dynamically per view in
`render/pipeline.rs`), so wgpu auto-selects Metal (iOS) / Vulkan-or-GLES (Android).
Zero compute/storage shaders; atlas pages 1024px (inside every mobile floor); all four
SDF shaders already made derivative-uniformity-clean by the WebGPU/Tint work — the
*same* rule Metal (MSL) and GLES (GLSL ES) enforce. iOS Metal is close to
portable-as-is.

Two concrete GLES-path fixes, both already designed as deferred "WebGL2-reach" work:
1. **`band.wgsl` declares 17 vertex attributes** (through `@location(16)`), exceeding
   the **16 floor** on Android GLES3 (`GL_MAX_VERTEX_ATTRIBS`), min-spec Android Vulkan
   (`maxVertexInputAttributes`), and default WebGPU — Metal's 31 is fine. *(Correction
   from first pass: this does **not** blank the screen — every pipeline is
   None-guarded, so it degrades to missing borders/focus-rings.)* Fix: fold clip/affine
   `vec2`s into `vec4`s → ≤16. Note real Adreno/Mali often report >16, so failure is
   only guaranteed on exactly-16 adapters.
2. **`Rgba16Float` effect-compositor target** with no fallback — fine on Metal/Vulkan,
   needs `EXT_color_buffer_float` on old Android GLES. Only bites apps using
   opacity<1 / isolation / blur groups.

### 3.2 Windowing / input / lifecycle — `moderate-work`

Apps bootstrap with the vanilla `App::new().add_plugins(DefaultPlugins).add_plugins(BuiyPlugin).run()`;
bevy_winit 0.30.13 / wgpu 29 already ship full iOS+Android backends. Three facts make
basic touch UI plausible with little new code: (1) bevy_picking already lowers winit
`TouchInput` to a per-finger `PointerId::Touch`, and Buiy's hit-test is pointer-agnostic,
so a tap already produces `OnPress` — **multi-touch is inherited, not net-new**
(`prior-art/bevy-picking/capabilities.md` L15); (2) layout is 100% logical-px, DPI flows
through; (3) extract early-returns + marks `NodeDamage::Full` when the window vanishes —
that *is* the Android suspend→resume rebuild shape, present by construction. Gaps:
a documented one-frame hover-lag makes **first-tap activation** unreliable (a tap has no
prior hover); **safe-area / notch insets** are unplumbed; on-device suspend/resume is
inferred but never run; hover-only affordances (tooltips) degrade on touch.

### 3.3 Text / IME / soft-keyboard — `moderate-work`

The engine transfers cleanly: cosmic-text behind a facade, fonts embedded via
`include_bytes!` (no desktop path), full IME composition already built and correct. The
*mobile input surface* is the unbuilt part: raising a **reliable** soft keyboard (winit's
`set_ime_allowed` works but is Android-flaky per §2), no `inputmode`/`enterkeyhint` (so
every field raises the generic alphabetic keyboard), no touch selection handles / context
menu (copy-paste is keyboard-shortcut-only today), no autocorrect/autocapitalize hinting.

### 3.4 Accessibility — `blocked-today, upstream-gated`

Buiy's a11y is cleanly two-layered: a winit-free semantic-tree **producer** (portable
as-is) plus pluggable **sinks**. The OS sink pushes into bevy_winit's
`ACCESS_KIT_ADAPTERS`, and Bevy 0.19 pins `accesskit_winit = "0.32"` (`Cargo.toml`), so
today iOS/Android get zero screen-reader reach. Buiy's *own* code needs near-zero change.
The in-process consumer already gives mobile a correct, inspectable a11y tree for
tests/agents. Unblock paths (both near-term, both upstream): **iOS** ← Bevy rides
`accesskit_winit ≥ 0.33` (**done on Bevy `main`**, ~0.20); **Android** ← Bevy enables the
existing opt-in `accesskit_android` feature. `accesskit_ios` is 0.1.1 (basic),
`accesskit_android` is 0.7.4 (pre-1.0, but battle-tested-ish: shipping since 2025-03).

### 3.5 Platform services / build — `moderate-work`

Biggest free win: fonts (`include_bytes!`) + shaders (`load_internal_asset!`) are
compiled in, so the framework carries **no runtime asset-path dependency** — the
asset-bundling step that dominates mobile packaging is solved for Buiy's own resources.
Narrow platform-awareness today: the only target-conditional services are clipboard
(wasm cfg) and keyboard-modifier logic (macos cfg). Gaps: **arboard is a hard *build*
error** on iOS/Android (arboard 3.6.1 has no android/ios module; on Android the cfg
matches nothing → missing symbol, on iOS it mis-matches the linux cfg → tries to build
X11/Wayland; PR #50's dummy fallback is still unmerged) — trivial fix: widen the existing
`cfg(not(target_arch = "wasm32"))` clipboard gate to also exclude android/ios and reuse
the wired `MemClipboard`. And **zero mobile build scaffold**: no android-activity feature,
`#[bevy_main]`/android_main entry, cargo-ndk/Xcode/gradle, or CI lane (the web analogue —
trunk + web-smoke — is the template to copy).

### 3.6 Operational reality — the under-weighted axis

"Launches" ≠ "good mobile citizen." The static analysis over-indexed on "does it draw /
does the tree build / does it compile" and under-weighted:

- **Idle battery drain (the sleeper).** Buiy sets **no** `WinitSettings` (verified: zero
  matches in `crates/`), so it inherits Bevy's default `UpdateMode::Continuous` and
  repaints every frame while *focused* even when nothing changes (a static text screen
  measured 10.9 ms/frame in the perf audit). Invisible on a plugged-in desktop; on a
  foreground-idle phone it pegs CPU+GPU (corroborated by open Bevy
  [#16734](https://github.com/bevyengine/bevy/issues/16734), "empty iOS app ~50% CPU").
  **Fix is a one-liner** (`WinitSettings { focused_mode: UpdateMode::reactive(...), .. }`
  or `desktop_app()`) — and it maps naturally onto Buiy's MVU dirty-state invalidation
  (a reactive redraw driven by MVU damage is the right end state). Note: only the
  *focused* mode is Continuous; backgrounded drops to `reactive_low_power(60Hz)`, so this
  is specifically a foreground-idle problem.
- **GPU-side 60Hz floor unproven on mobile.** Buiy's hard 60Hz floor was validated only as
  a *desktop CPU-instruction* budget (iai-callgrind on an RX 6700 XT + lavapipe). Whether
  4× MSAA + `Rgba16Float` off-screen targets at 3× DPR hold 60Hz on a thermally-throttled
  tiled Mali/Adreno — including the **tile-flush cost** of the off-screen effect groups
  (an intermediate sampled by a later pass can't be memoryless, so it forces a
  store+reload of tile memory — disproportionately costly on mobile's low bandwidth) — is
  unmodeled.
- **No audio** (bevy_audio not in the feature set; verified zero `bevy_audio`/rodio/oboe/kira
  in tree) — matters directly for the Dooduel game campaign. **No haptics.** No OS
  memory-pressure hooks. No **background-kill state restoration** (MVU state is in-memory
  only). Hot-reload/MVU-replay unexamined on device. Plus min touch-target sizes
  (WCAG 2.5.5 24px / Material 48dp), kinetic scrolling/overscroll, app/binary size, and
  120Hz/ProMotion frame pacing.

---

## 4. Blockers, re-tiered after validation

1. **Robust soft-keyboard/IME on mobile** — *large, partly upstream*. A starting point
   exists (`set_ime_allowed`), but Android is flaky (winit #4126/#1823) and a
   production-grade port would likely need its own android-activity/JNI text layer, as
   Slint and Makepad did. This is the single biggest UX-critical item for any text app.
2. **OS accessibility** — *near-zero Buiy work, upstream-gated*. Blocked on a Bevy
   version bump (iOS, already on Bevy `main`) / feature flip (Android). Buiy's own spec
   already defers mobile a11y to a "manual-release-gate."
3. **Battery/power model** — *small, framework-wide*. Reactive redraw. Not mobile-specific,
   but decisive for whether a mobile app is usable at all.
4. **Mechanical** — *small*: arboard cfg gate (trivial), band 16-attr fold, `Rgba16Float`
   gate, first-tap activation, mobile build scaffold.

---

## 5. Precedent (validated)

| Toolkit | Mobile status (as of 2026-07) | Store-grade? |
|---|---|---|
| **Slint** | Android **production** since 1.5 (2024-03); iOS since **1.12** (2025-06), still "tech-preview" at 1.17; safe-area + virtual-keyboard added 1.15. Own viewer app live on **both** App Store + Play. | **Yes** — the one Rust GUI toolkit with App-Store/Play-Store-grade shipping. Android bypasses winit (own android-activity backend + JNI `InputConnection` bridge). |
| **Makepad / Robrix** | iOS + iPadOS + Android from one codebase, zero platform-specific code, via `cargo makepad`. Bypasses winit entirely. | **No** — pre-alpha, GitHub-release binaries only (TestFlight aspirational). |
| **iced** | Explicit non-goal (issue #302 open); community examples only, no CI. | No |
| **floem / freya** | Desktop-only; iOS/Android not supported. | No |
| **xilem / masonry** | Alpha; Android examples-only; **unofficial + buggy iOS** (PRs #418/#421, scale bug #419). | No |
| **GPUI (Zed)** | None; platform abstraction too tied to native APIs (tracking #43206/#43207, no commitment). Explicitly maps "Bevy path makes shipping Buiy on mobile much cheaper than reaching there from GPUI." | No |

Takeaway: exactly one Rust GUI toolkit (Slint) has store-grade mobile today, and the
ones that ship mobile at all **wrote their own native text-input layer** rather than
rely on winit's IME — the clearest signal for what a serious Buiy port would need.

---

## 6. Prior-art map — folders to consult

The corpus has strong mobile coverage. Consult in roughly this order for a mobile spec:

**Primary (native-mobile-specific):**
- **`docs/prior-art/makepad/mobile-targets.md`** — the single richest native-mobile
  reference. Per-finger touch primitives (L34), `<TextInput>` → OS soft keyboard +
  keyboard-occlusion layout (L42), mobile widget patterns (`<StackNavigation>`,
  `<SwipeAction>`, touch-momentum lists, L55-59), and the **"mobile a11y entirely
  absent → a Buiy red line"** lesson (L63-73, L85-88). Notes Buiy inherits Bevy's
  substrate + cargo-mobile2, not Makepad's (L30).
- **`docs/prior-art/accesskit/platform-adapters.md`** — the load-bearing a11y reference:
  `accesskit_android` (§L70-83, InjectingAdapter/embedded-dex "drop in and it works"),
  `accesskit_ios` (§L87-95, UIAccessibility, "basic"), the version/date table (L9-18),
  and the `accesskit_winit` wiring (L109-138). Note: this folder's version narrative is
  what §2/§9 corrects — Android multiplexing predates 0.33.
- **`docs/prior-art/slint/`** — the best worked example of one Rust-UI codebase shipping
  to both stores. `open-problems.md` §"Mobile target completeness" (L31-40, honest
  touch/soft-keyboard/IME/CI gaps), `governance-and-distribution.md` platform table
  (L100-113) + **mobile = commercial-license gate** (L42-59), `history.md` L41 (1.15
  safe-area props — the *only* shipped safe-area precedent in the corpus).

**Substrate (what Buiy inherits from Bevy):**
- **`docs/prior-art/bevy-picking/`** — `capabilities.md` L15: per-finger multi-pointer is
  first-class at the substrate Buiy consumes → touch is inherited. `api.md` L85-86
  (pinch scroll, touch-lost cancel).
- **`docs/prior-art/bevy-a11y/distribution.md`** L56-60 — confirms Buiy gives up **no
  platform reach** by replacing bevy_a11y per-window; mobile a11y is unwired in Bevy today.
- **`docs/prior-art/bevy-ui/distribution.md`** L64-67 — the sibling stack's mobile matrix
  = Buiy's inherited baseline ("UI renders; a11y lags").

**Comparative / calibration:**
- **`docs/prior-art/gpui/`** — negative precedent with an explicit Buiy mapping
  (`critiques-and-open-problems.md` L75-79); also the continuous-per-frame-paint power
  model (`architecture.md` L17) and ProMotion refresh sync (`gpu-rendering.md` L69).
- **`docs/prior-art/dioxus/targets.md`** — clearest articulation of native-mobile-GPU
  constraints (battery/thermal/driver variance, L64) + "Buiy's choice to inherit Bevy's
  one mobile path is validated" (L95).
- **`docs/prior-art/iced/open-problems.md`** L21-31, L95-99 — a clean checklist of the
  hard, unbuilt native-mobile subsystems (Activity/UIApplication lifecycle, touch-first
  widgets, soft-keyboard, refresh-rate-aware rendering).
- **`docs/prior-art/rmlui/text-and-input.md`** L74-81 — native touch + inertial scroll are
  a *very recent* (6.2, 2026-01) addition even in a mature engine UI; IME is "the
  embedder's problem."
- **`docs/prior-art/unity-ui/accessibility.md`** L7, L20-22, L54 — the "add mobile a11y
  late" cautionary tale + evidence that TalkBack/VoiceOver is the highest-yield a11y
  target (supports Buiy's AccessKit-first-on-mobile stance).
- **`docs/prior-art/egui/open-problems.md`** L116-117 + **`bevy-egui`** L51 — the two
  mobile-*ergonomics* concerns almost nothing else names: safe-area-aware layout
  primitives and touch hit-target sizing (WCAG 2.5.5 / Material 48dp).
- **`docs/prior-art/floem` + `freya/distribution.md`** — negative markers (mature Rust
  reactive UIs with no mobile at all).
- **`docs/prior-art/web-rendering/`** — *web* mobile path (wasm on the mobile-browser
  canvas), not native, but holds the mobile-GPU texture-limit + high-DPR startup-crash
  finding (`open-problems.md` L37) and the canvas-raises-no-soft-keyboard problem that
  mirrors the native winit-IME question.

---

## 7. If we pursue it — sequencing

1. **Reactive redraw + a mobile perf model first.** Framework-wide, not mobile-specific,
   and the difference between usable and unusable on battery. Wire it to MVU dirty-state.
2. **Mechanical GLES fixes + mobile scaffold** to get pixels on a device: arboard cfg
   gate, band 16-attr fold, `Rgba16Float` gate, android-activity feature + entry point +
   cargo-ndk/Xcode + a CI lane.
3. **Touch + soft-keyboard UX**: first-tap activation, `inputmode`, selection handles;
   plan for a likely android-activity/JNI text layer (Slint/Makepad pattern) if winit's
   IME proves too flaky.
4. **Ride the Bevy `accesskit_winit ≥ 0.33` bump for a11y** when Buiy next upgrades Bevy
   (iOS), and enable `accesskit_android` (Android).

This maps onto Buiy's existing "additive, cfg-gated new target" playbook — the same one
the wasm port followed.

---

## 8. Corpus gaps (candidate `researching-prior-art` triggers)

No prior-art folder covers, and a serious mobile effort would want: Bevy's concrete
native-mobile **build/deploy toolchain** (GameActivity/cargo-ndk/Gradle, Info.plist,
signing) and its **suspend/resume + process-death lifecycle**; **safe-area insets** as a
Buiy-applicable subsystem; **mobile-GPU render-pipeline behavior** (tile memory,
load/store cost, MSAA bandwidth, precision, thermal); **reactive/refresh-rate-aware
power** on native mobile; native **soft-keyboard/IME invocation** (as opposed to the web
canvas gap); touch **ergonomics** (tap-target sizing, long-press, haptics); mobile
**navigation/gesture** patterns + Android predictive-back; **app-store distribution**
mechanics + runtime permissions; orientation/foldables/split-view; OS **dynamic-type /
font-scaling** binding; and a real native-mobile **CI/test** harness (which is exactly
Buiy's own gate for un-deferring mobile).

### Prior-art promotion suggestions (flagged, not spawned)

- **`slint-mobile`** — the only Rust GUI toolkit with store-grade mobile shipping
  (Android backend-android-activity + iOS Xcode/TestFlight pipeline + the
  `SlintAndroidJavaHelper` `InputConnection` bridge). Most load-bearing external
  precedent if Buiy pursues mobile. Sources: slint.dev blog 1.5/1.12/1.17, docs.slint.dev
  Android guide.
- **`bevy-mobile`** — Bevy's own mobile build flow (GameActivity + cargo-ndk vs deprecated
  cargo-apk/NativeActivity, API-31 floor, the `tracing-oslog` iOS-simulator blocker +
  Xcode-template-churn workaround) and the open lifecycle-bug ledger (#12693 audio, #16604
  rotation, #16734 iOS CPU, #17122 Android touch). `jinleili/bevy-in-app` is the leading
  embed-into-native-shell pattern.
- **`makepad-platform` / `android-activity`** — the "bypass winit entirely" reference
  (makepad-android-state JNI; android-activity GameTextInput, issues #18/#44) — the
  NDK-level substrate any Android text path sits on.

Per `using-prior-art`, these are flagged for the user to triage — not spawned.

---

## 9. Validation ledger

| Claim | Verdict | Corrected state (2026-07-11) |
|---|---|---|
| winit does not implement mobile soft keyboard | **REFUTED** | `set_ime_allowed(true)` shows/hides it on iOS/Android (0.30.13); no-op only on Web. App-driven; Android unreliable (#4126); umbrella #1823 open; 0.31 deprecates for `request_ime_update`. |
| `set_ime_allowed` is a no-op on mobile | **REFUTED** | No-op on **Web**, not mobile; behaves like a no-op only in the Android "view not served" failure mode. |
| Slint/Makepad bypass winit with own JNI/UIKit text layer | **CONFIRMED** | Makepad bypasses winit on all platforms; Slint bypasses on its Android backend; both do mobile text outside winit's IME. |
| accesskit_winit 0.32 has no mobile adapter (macos/unix/windows only) | **PARTIAL** | Android adapter present since **0.24.0** (opt-in feature Bevy doesn't enable); only iOS truly absent from 0.32.x. "Zero mobile AT on Bevy 0.19" conclusion still correct. |
| Android added in 0.33.0, iOS in 0.33.1 | **REFUTED** | Android since 0.24.0 (2025-03-06); iOS in **0.33.0** (2026-05-11); 0.33.1 is a patch bump. |
| accesskit_ios 0.1.0 basic; accesskit_android ~0.7.x pre-1.0 | **CONFIRMED** | ios 0.1.1, android 0.7.4 (both 2026-06-12). |
| Bevy pins accesskit_winit ^0.32 → mobile-a11y is a Bevy-upstream gate | **CONFIRMED** | Bevy `main` already bumped to 0.33 → iOS unblocks ~Bevy 0.20; Android needs the `accesskit_android` feature enable. |
| Bevy officially supports iOS/Android, rough toolchain (iOS-sim broken @0.16, cargo-apk→cargo-ndk, GameActivity API 31+) | **CONFIRMED** | All sub-claims verified; iOS-sim-broken is version-pinned to 0.16 practitioner reports. |
| wgpu Metal-iOS / Vulkan-Android production-ready, GLES fallback | **CONFIRMED** | wgpu matrix: both ✅ First Class; GLES 🆗 downlevel. |
| Bevy defaults to Continuous redraw; Buiy sets no WinitSettings → foreground-idle battery drain | **CONFIRMED** | Default `WinitSettings::game()` → focused `Continuous`; zero WinitSettings in Buiy; corroborated by Bevy #16734. Foreground-idle-specific. |
| Known Bevy mobile lifecycle bugs (iOS audio #12693) | **CONFIRMED** | #12693 open; also #16604 rotation, #16734 CPU, #17122 Android touch. |
| Slint Android prod ~1.5 / iOS ~1.10-1.12 with store tooling | **CONFIRMED** | iOS landed **1.12** (not 1.10), still "tech-preview" at 1.17; viewer app live on both stores. |
| Makepad/Robrix one-codebase mobile, GitHub-release only | **CONFIRMED** | Pre-alpha, sideload-only, TestFlight aspirational. |
| iced/floem/xilem don't ship mobile | **PARTIAL** | Holds overall; but xilem iOS is **unofficial+buggy**, not "out of scope." None store-present. |
| 16-attr floor (GLES3/Vulkan/WebGPU=16, Metal=31); band.wgsl 17 fails on 16-floor adapters | **CONFIRMED** | All four numbers match specs; failure guaranteed only on exactly-16 adapters; WebGPU default is 16 even on desktop unless higher requested. |
| arboard 3.6.x = hard build error on android/ios, no fallback | **CONFIRMED** | 3.6.1; platform dir linux/osx/windows only; PR #50 unmerged. iOS mis-matches the linux cfg; Android has no branch. |
| Off-screen→composite forces tile flush, far costlier on TBDR mobile | **PARTIAL** | Cost mechanism well-supported (Apple memoryless can't help a later-sampled target). "TBDR" label fits only Apple; say **tile-based**. |

---

## 10. Sources

winit: [window.rs 0.30.13](https://raw.githubusercontent.com/rust-windowing/winit/v0.30.13/src/window.rs),
[#1823](https://github.com/rust-windowing/winit/issues/1823),
[#4126](https://github.com/rust-windowing/winit/issues/4126) ·
android-activity [#18](https://github.com/rust-mobile/android-activity/issues/18) ·
accesskit_winit [CHANGELOG](https://raw.githubusercontent.com/AccessKit/accesskit/main/platforms/winit/CHANGELOG.md),
[versions](https://crates.io/api/v1/crates/accesskit_winit/versions) ·
bevy_winit [0.19-rc.3 Cargo.toml](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.0-rc.3/crates/bevy_winit/Cargo.toml),
[main Cargo.toml](https://raw.githubusercontent.com/bevyengine/bevy/main/crates/bevy_winit/Cargo.toml) ·
Bevy [winit_config.rs](https://raw.githubusercontent.com/bevyengine/bevy/main/crates/bevy_winit/src/winit_config.rs),
[#16734](https://github.com/bevyengine/bevy/issues/16734),
[#12693](https://github.com/bevyengine/bevy/issues/12693),
[examples/mobile](https://github.com/bevyengine/bevy/blob/main/examples/mobile/android_basic/readme.md),
[cheatbook platforms](https://bevy-cheatbook.github.io/platforms.html) ·
wgpu [README matrix](https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/README.md) ·
Slint [1.12](https://slint.dev/blog/slint-1.12-released),
[App Store viewer](https://apps.apple.com/app/slint-viewer/id6773729654),
[Android guide](https://docs.slint.dev/latest/docs/slint/guide/platforms/android/) ·
Makepad/Robrix [robrix](https://github.com/project-robius/robrix) ·
xilem [#419](https://github.com/linebender/xilem/issues/419) ·
GPU limits: [Vulkan required limits](https://docs.vulkan.org/refpages/latest/refpages/source/Required_Limits.html),
[GLES3 glGet](https://docs.gl/es3/glGet),
[WebGPU limits](https://developer.mozilla.org/en-US/docs/Web/API/GPUSupportedLimits),
[Metal feature tables](https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf) ·
arboard [platform/mod.rs](https://raw.githubusercontent.com/1Password/arboard/master/src/platform/mod.rs),
[PR #50](https://github.com/1Password/arboard/pull/50) ·
TBDR: [Apple WWDC20 memoryless](https://developer.apple.com/videos/play/wwdc2020/10632/),
[Imagination TBDR](https://docs.imgtec.com/starter-guides/powervr-architecture/html/topics/tile-based-deferred-rendering-index.html).
