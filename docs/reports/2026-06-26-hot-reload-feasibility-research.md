**Date:** 2026-06-26
**Status:** report (research input to a future `buiy-asset-pipeline-design` / hot-reload spec)

# Hot-reload feasibility for Buiy — research

> Can Buiy hot-reload its GUI on a *running* app — and along which axes? This
> report decomposes "hot reload" into five distinct dimensions (asset, style/theme,
> scene/`bsn`, Rust code, state-preservation), audits what Buiy already has for each
> (cited to `file:line`), surveys the external prior art that has solved each one,
> and recommends a *layered* path: the cheap, separable wins now; the upstream-blocked
> and big-lift work captured as design and gated. This is the **research** stage
> output — an internal capability inventory, an external prior-art survey, and a
> layered recommendation with rejected alternatives. It does **not** decide the
> spec — it feeds one.

Produced by a multi-agent research workflow (an 11-way internal-audit + external
prior-art sweep → synthesis → adversarial code-and-source verification). Both
review passes returned **sound-with-corrections**; every load-bearing internal
claim below was re-checked against the actual code and the corrections folded in,
and the time-sensitive external/ecosystem facts are flagged as such. See
*Provenance* at the end.

---

## TL;DR

1. **"Hot reload" is FIVE dimensions with opposite feasibility profiles — do not
   design or ship them as one feature.** Available-now: Rust-code body-patch (dev
   loop) and style/theme. Blocked-on-upstream: scene/`.bsn` (there is no `.bsn`
   text loader in Bevy 0.19.0-rc.3). Big-lift: cross-shape state migration. A
   single "hot reload" umbrella hides three different difficulty and
   state-preservation stories.

2. **Buiy is structurally *better* positioned for stateful reload than Flutter,
   React, or SwiftUI** — those three had to invent and reverse-engineer the
   description/identity/state triad (Widget/Element/State; JSX/Fiber/hook-cell;
   View/identity/`@State`-in-AttributeGraph). The ECS already has all three as
   first-class primitives: `bsn` = description, `Entity` = identity, a Component =
   the state cell. The state cell and the identity are already author-addressable.

3. **The cheapest near-free win is dimension-4 (Rust code) for the DEV LOOP.**
   Bevy's `hotpatching` feature is present in Buiy's *exact* `0.19.0-rc.3` pin
   (`bevy_internal-0.19.0-rc.3/Cargo.toml:278-280`), is OFF by default (so
   `subsecond`/`dioxus-devtools` never enter `Cargo.lock` — zero dependency/compile
   cost), and preserves the *entire* World for free because only function pointers
   swap. Flip it on a gallery/example binary. Hard caveats: binary/tip-crate-ONLY
   (not the `buiy_core`/`buiy_widgets` libraries), native-ONLY (compiled out on
   wasm), body-only (no signature or Component-layout change), and the underlying
   `subsecond` is explicitly *experimental* (opt-in `--hotpatch` flag).

4. **STYLE/THEME (dimension 2) is the cheapest *shippable* win and is fully
   separable.** The extract-time `theme.is_changed()` re-resolve gate already
   exists (`render/extract.rs:941`, `render/mod.rs:418-425`); a theme/token asset +
   an `AssetEvent::Modified` observer that rebuilds the `Theme` resource plugs
   straight into it with *zero* render-pipeline changes, preserving the whole live
   tree by construction (the tree is never rebuilt). Ship it FIRST; never gate it
   behind scene reload (the bevy_flair lesson).

5. **SCENE/`bsn` (dimension 3) is blocked on the upstream `.bsn` text loader.** The
   re-apply *primitive* already exists upstream (`apply_scene` writes "directly on
   top of any existing components", `ScenePatch` is an Asset, `resolve_scene_patches`
   re-resolves on `AssetEvent`), and `buiy_bsn` already routes through
   `Assets<ScenePatch>`. But (a) the text loader is not shipped (deferred from
   upstream PR #23413; `docs/plans/follow-ups.md:1433`), and (b) `apply_scene`
   *spawns* related children rather than reconciling them — so a whole-tree re-apply
   would duplicate children. The keyed child reconciler keyed on `bsn` `#Name` refs
   is the genuine novel work.

6. **Every prior system converges on one rule:** patch CODE bodies + identity-
   preserving in-place COMPONENT patch are live; FORCE RESTART on data-layout /
   signature change; never auto-migrate layout in place. A reload is an explicit,
   versioned, *fallible* `migrate(old) → new` (Erlang `code_change` / CLOS
   `update-instance-for-redefined-class`), not a magic respawn. Unreal's legacy
   "Hot Reload" reinstancing is the canonical corruption tale.

7. **The state crux is the un-reflected island.** `TextEditState`
   (`text/edit/state.rs:92-126`) and `TextBuffer` (`text/components.rs:572`) are
   `#[derive(Component)]`-only (a deliberate facade boundary — they carry
   `cosmic_text` types), so a serialize-migrate-respawn reload silently DROPS live
   text/caret/selection/undo. In-place patch (no respawn, no Entity-id churn) is
   therefore the correct primitive; serialize-respawn needs these reflected or
   carved out with a hand migration.

---

## Part 1 — What Buiy has today (internal capability inventory)

Buiy is a Bevy **0.19.0-rc.3** ECS UI framework parallel to `bevy_ui`, with a
retained widget tree, decomposed CSS-subset style components, Taffy layout,
cosmic-text 0.19, a custom wgpu render pipeline, an authored AccessKit semantic
tree, `bevy_picking` input, and `bsn!` authoring via `buiy_bsn`. Every claim below
was code-verified by the review (line-number corrections folded in).

### 1a. Asset reload — the font beachhead (the only landed reload surface)

Font asset reload is Buiy's one fully-implemented reload dimension, and it
establishes the pattern everything else can reuse: `AssetEvent::Modified` handled
as remove + re-add under one lock hold + one generation bump, with survival tests
proving no ID leak or db growth across cycles.

| Capability | Where | Notes |
|---|---|---|
| `BuiyFont` asset + `BuiyFontLoader` | `text/font_asset.rs:17-90` | sfnt-bytes asset; loader-output-is-always-sfnt invariant. |
| `FontRegistry` strong-handle registry + `apply_font_registry` | `text/registry.rs:133-545` | Processes staged ops + the `AssetEvent` stream under **one** lock + **one** `FontsGeneration` bump per batch. |
| `Modified` = remove + re-add composition | `text/registry.rs:361-375` | Old faces removed (`:372`), re-addition staged (`:373`) — composed under a single lock + single bump. |
| `FontSystem` rebuild via `into_locale_and_db` | `text/registry.rs:476-489` | The only rebuild method (cosmic-text's `font_cache` has no purge API); carries the same `Database` so surviving IDs stay valid. |
| Generation bump exactly once per batch | `text/registry.rs:543` | `generation.0 += 1; // exactly once per batch`. |
| `reregister_bytes` programmatic reload | `text/registry.rs:181-186` | Same mechanics as the `AssetEvent` path; the app-driven injection seam. |
| Content/preedit/style survival across reload | `tests/text_edit/text_font_reload_survival.rs:95-257` | A `FontsGeneration` bump reshapes but **preserves** buffer content + metrics. |
| Leak/growth guardrails (N=8 cycles) | `tests/text/text_registry.rs:140-179` | Fresh fontdb ID each cycle; old IDs never resolve again; face count returns to baseline. |

Honest scope: fonts are the only asset type with a reload path today. Whether
`AssetPlugin`'s `watch_for_changes` is wired was **not** exhaustively verified this
pass; the survival tests and `reregister_bytes` make clear that reloads are at
least driven **programmatically** (app-injected) — treat OS file-watch as *not yet
enabled*, not as proven absent. This matters for the web target (see §4): Bevy's
filesystem watch uses the `notify`-based `file_watcher` feature, which does not run
on wasm, so any web reload must be app-driven regardless.

### 1b. Style / theme — a Resource, Rust-authored, with a live re-resolve gate

The theme machinery already has the *consumer* half of hot-reload (an extract-time
re-resolve on change); it lacks only the *producer* half (a file format + loader).

| Capability | Where | Notes |
|---|---|---|
| `Theme` as a Bevy **Resource** (not an Asset) | `theme.rs:18-25` | Flat `HashMap`s: `colors`/`spaces`/`radii`. No `Asset<Theme>`, no file format, no loader. |
| `Theme` + `UserPreferences` Reflect-registered | `theme.rs:18-19, :48-49` | Both `#[reflect(Resource)]`; registered in `ThemePlugin`. Prepared for a future loader. |
| Extract-time `!theme.is_changed()` damage gate | `render/extract.rs:941` + `render/mod.rs:418-425` | A `Theme` change rebuilds all painted nodes — the seam any reload rides. |
| `resolve_token(token, theme)` live re-read | `render/color.rs:127` | Re-reads the active `Theme` every frame; no theme-stamped cached buffers. |
| Decomposed paint components carry `ColorToken` | `render/components.rs:17-200` | `Background`/`Border`/`Outline`/`BoxShadow`/`TextColor`/`CaretColor` resolve via tokens; tokens never reach the GPU. |
| The one live `Theme` mutation today | `render/forced_colors.rs:30-57` | The forced-colors swap marks `Theme` changed exactly once → single re-extract. |

Gaps: no `Asset<Theme>`/loader/file format (CSS is **explicitly out of scope** —
Buiy has a typed decomposed-token model, not a cascade); no variant binding
(`prefers_dark` etc. are stored but unconsumed); and the per-entity `Theme`
component override is *mentioned* in the architecture but **unwired** — a reload
system must not clobber it or programmatic `Theme` mutations.

### 1c. `bsn` / scene — compile-time authoring + an upstream re-apply primitive

`bsn!` is a compile-time macro (`→ Scene`) and can *never* hot-reload by itself.
The runtime re-apply primitive exists upstream but only spawns children:

| Capability | Where | Notes |
|---|---|---|
| `apply_scene` writes onto existing components | `bevy_scene-0.19.0-rc.3/src/spawn.rs:459/465/499/591` | Doc verbatim: "write directly on top of any existing components on the entity." |
| `ScenePatch` is a Bevy Asset; re-resolves on `AssetEvent` | `bevy_scene .../spawn.rs:603` (`resolve_scene_patches`) | The asset-driven re-resolve seam already exists. |
| `ResolvedSceneRoot::apply` **spawns** related children | `bevy_scene .../resolved_scene.rs:62-65` | Not a reconcile — a naive whole-tree re-apply DUPLICATES children. |
| `buiy_bsn` routes through `Assets<ScenePatch>` | `crates/buiy_bsn/src/lib.rs:~44-50` | `spawn_scene` resolves via the registry + `AssetServer`; "that loader is deferred upstream." |
| `.bsn` text loader + component hot-reload deferred | `docs/specs/2026-06-18-buiy-bsn-integration-design.md:152-153`; `docs/plans/follow-ups.md:1433` | Owned by the (unwritten) `buiy-asset-pipeline-design`; "Component hot-reload. Depends on the `.bsn` loader; deferred with it." |

So the leaf/component re-apply is upstream-present; the **loader** and the **keyed
child reconciler** are the deferred work. (External corroboration the loader is
unlanded after 0.19: PR #23413 states the BSN loader "will be added in a future
PR"; the Bevy 0.19 release notes defer the asset-driven workflow to a future
release; open issue #24309 asks to remove the premature `.bsn` asset-format docs.
These are *time-sensitive ecosystem facts* — re-check before the spec freezes.)

### 1d. ECS state-preservation — the reflected structure vs the un-reflected island

A sharp divide. Authored/structural state is broadly Reflect-registered (**176**
`register_type::<>` call sites across `buiy_core` (151) + `buiy_widgets` (25) — Focus,
all style/layout/render components). Machinery state is deliberately *not* Reflect
and would be destroyed on a respawn:

| State | Where | Reload behavior |
|---|---|---|
| `TextEditState` (cosmic `Editor` + caret blink clock + undo + preedit) | `text/edit/state.rs:92-126` | `#[derive(Component)]`-ONLY, doc says "Machinery state — NOT reflect-registered". DESTROYED on respawn. |
| `TextBuffer` (per-line shape caches) | `text/components.rs:572` | `#[derive(Component)]`-only. DESTROYED on respawn (loses the typing-latency win). |
| `CaretVisual`/`SelectionVisual`/`PreeditVisual` | `text/components.rs:397-489` | Carry `cosmic_text::Cursor`; not Reflect. DESTROYED. |
| `ScrollOffset` (value struct, Entity-independent) | `layout/components.rs:521-523` | `#[reflect(Component)]`; survives **iff** the Entity survives. |
| `FocusedEntity(Option<Entity>)` | `focus.rs:109-111` | Reflect, but holds an `Entity` — stale after a respawn (needs remap). |
| Render caches: `ResidentTextKeys`/`ExtractedGlyphs`/`GlyphMetaCache` | `text/extract.rs:120-140`, `render/prepare.rs:52-77` | Rebuilt on the `Added`/`Changed` cascade that a respawn forces anyway. |

The decisive design fact: a despawn+respawn mints new `Entity` ids, which (by
reasoning, not a measured fact) invalidates every `Entity`-keyed structure —
`LayoutTree.by_entity`, the anchor maps, `FocusedEntity.0`, `TopLayerActivation`'s
modal stack, `GlyphEntityRun.entity`. The font-survival tests prove preservation
works *only for the in-place (no-respawn) path*. That is the empirical case for
making in-place patch the primitive.

---

## Part 2 — External prior art

Six clusters were surveyed. The throughline: **a reload is "re-apply the
description onto live instances, keyed by identity," and state survives iff it
lives outside the re-applied description.** Every system either patches in place
(state-preserving) or rebuilds (state-losing), and all of them draw the same
"this change forces a restart" line at data-layout/signature changes.

For each system: *mechanism / granularity / state-preservation / Buiy-applicability*
+ sources. External anecdotes (issue numbers, archive dates, corruption histories)
are cited to their trackers/docs and flagged as *unverified-from-repo* —
authoritative sourcing belongs in the prior-art folders (§13).

### Cluster A — Stateful-UI reload (the description/identity/state gold standard)

- **Flutter hot reload** — *Mechanism:* the Dart VM JIT-recompiles changed
  classes in place; `reassemble()` walks the persistent Element tree marking every
  Element dirty so all `build()` re-run against new code. *Granularity:* whole
  element tree at Element granularity. *State:* STRONG/automatic — `State` objects
  persist on the Element tree (identity), the immutable `Widget`s (description) are
  rebuilt. Forces a full restart on `main()`/`initState()` body, global/static
  *initializers*, and `enum`↔class conversions. *Applicability:* the triad maps 1:1
  onto Buiy — `bsn`/scene-fn = Widget, `Entity` = Element, runtime-state Components
  = State. `reassemble()` = a dev-only system that re-runs scene-fns and patches
  description-derived components while leaving runtime-state untouched. *Does not
  transfer:* Flutter ships the keyed child reconciler (`Element.updateChild`); Buiy
  `bsn!` only spawns — that reconciler is the missing bulk.
  Sources: `docs.flutter.dev/tools/hot-reload`; `api.flutter.dev/.../BindingBase/reassembleApplication`.

- **React Fast Refresh** — *Mechanism:* a Babel transform injects `$RefreshReg$`
  (component-family identity) + `$RefreshSig$` (hook signature); the runtime
  re-renders mounted instances of edited families in place. *Granularity:*
  component-module. *State:* function-component hook state survives **iff the hook
  signature is stable**; otherwise that subtree remounts; class components never
  preserve. *Applicability:* the signature-stability rule is the precise
  per-entity preserve-vs-reset heuristic; the component-family is a stable author
  key *decoupled from the live instance* — exactly the author-stable key Buiy needs
  (Entity bits are not stable across respawn). `// @refresh reset` is worth copying
  as a per-widget force-remount opt-out. *Does not transfer:* boundary detection is
  JS-module-graph-specific; ECS has no `module.hot.accept`.
  Sources: `reactnative.dev/docs/fast-refresh`; `github.com/pmmmwh/react-refresh-webpack-plugin`.

- **SwiftUI Previews / dynamic replacement** — *Mechanism:* `@_dynamicReplacement(for:)`
  swaps function bodies in a thunk dylib without a full module rebuild; structural
  edits force a rebuild ("Automatic Preview Updating Paused"). *State:* `@State`
  survives a body swap because it lives in AttributeGraph keyed by structural
  identity while the View struct is ephemeral — the same split as Flutter/React.
  *Applicability:* strongest *conceptual* match for the ECS model (state on a stable
  Entity vs re-patched description); validates ECS as arguably *cleaner* because
  identity and state are first-class and author-addressable. *Cautionary:* it rests
  on an underscored, unstable attribute and is chronically fragile — the lesson is
  "do not promise seamless code reload; declaration/type/signature edits drop to a
  recompile."
  Sources: `fatbobman.com/en/posts/how-swiftui-preview-works`; `developer.apple.com/documentation/swiftui/previews-in-xcode`.

### Cluster B — Rust code reload (the directly-available bridge)

- **Bevy upstream `hotpatching` (built on Dioxus subsecond)** — THE load-bearing
  option, verified present in Buiy's exact pin. *Mechanism:* `HotPatchPlugin`
  (`bevy_app-0.19.0-rc.3/src/hotpatch.rs:22`) calls `connect_subsecond()`
  (non-wasm only) and, on a new jump table, pushes a `HotPatched` message + flips
  `HotPatchChanges` (`bevy_ecs lib.rs:138-155`); every `FunctionSystem` runs its
  body through `subsecond::HotFn::current(...)` and `refresh_hotpatch()` only
  re-fetches the pointer when `HotPatchChanges` changed (`function_system.rs:509,537,704-705`)
  — no per-frame cost. *Granularity:* per-system/observer **body**. *State:*
  EXCELLENT and FREE — only function pointers swap, the whole World survives;
  dimension-5 solved by construction *for body edits*. *Applicability:* HIGHEST —
  flip `bevy/hotpatching` behind a dev feature, run a `hello_*`/gallery binary
  under `dx serve --hot-patch`. *Hard limits:* binary/tip-crate-ONLY (DioxusLabs
  #4160 — `buiy_core`/`buiy_widgets` library systems do NOT patch), native-ONLY
  (collides with the landed WASM design — though the workshop notes wasm is "not a
  technical limitation, just not yet implemented", so it is a gap, not a permanent
  wall), and no signature/Component-layout change. Feature is OFF by default
  (zero cost; `subsecond`/`dioxus-devtools` absent from `Cargo.lock`).
  Sources: vendored `bevy_app`/`bevy_ecs` 0.19.0-rc.3; PR #19309 (landed Bevy 0.17);
  `bevy.org/news/bevy-0-17`.

- **subsecond (DioxusLabs)** — the jump-table engine under the wrapper.
  *Mechanism:* incremental-link + pointer swap via `dx`; `HotFn`/`call`,
  `HotFnPanic` unwinds to the nearest `call` boundary. *State:* NONE intrinsic —
  patches code, not data; struct-layout/alignment change against an old instance
  *crashes* (documented — strengthening "refuse-and-restart on layout change" from a
  restriction to a crash hazard). *Maturity (corrected):* explicitly EXPERIMENTAL
  in Dioxus 0.7, opt-in `--hotpatch` flag — frame it as experimental, not merely
  "edge cases." *Applicability:* do NOT add a direct dep; Bevy vendors and wraps it
  — reach it only via `bevy::app::hotpatch`.
  Sources: `docs.rs/subsecond`; `dioxuslabs.com/learn/0.7/essentials/ui/hotreload`;
  `github.com/DioxusLabs/dioxus/issues/4160`.

- **dexterous_developer / hot-lib-reloader** — rejected as tools, mined for one
  concept. dexterous_developer does true state-migration across a dylib swap via
  `rmp_serde` (read old bytes → deserialize into the new shape) — the concept to
  keep — but it is **archived 2026-02-08** with a Bevy **0.14** ceiling (5 majors
  behind), so porting is from-scratch against dead upstream. hot-lib-reloader is
  ECS-hostile (a dylib reload changes `TypeId`s, breaking Component/Resource
  identity; `tracing` crashes on reload). Both superseded by the upstream
  integration. (Archive date + ceiling externally confirmed.)
  Sources: `github.com/lee-orr/dexterous_developer`; `github.com/rksm/hot-lib-reloader-rs`.

### Cluster C — Native game-engine live-coding (production-scale discipline)

- **Live++** — *Mechanism:* in-process machine-code patching of a running native
  process; recompile only changed `.obj`s, atomically patch function prologues.
  *State:* maximal by construction (heap/globals/threads/stack persist); the wall
  is data-LAYOUT change, where the developer supplies pre/post-patch hooks to
  serialize→delete→recreate→deserialize. *Applicability:* the literal model
  subsecond imitates; transfer the *discipline*, not the (C++/Windows) tool: patch
  system/widget bodies; on any Component/Resource struct change, force restart or an
  explicit migration step — never reuse old-layout storage.
- **Unreal Live Coding + the legacy "Hot Reload" cautionary tale** — Live Coding
  patches `.cpp` bodies in-process; reflected-type/header/constructor changes force
  an editor rebuild. The predecessor "Hot Reload" instead swapped a module DLL and
  *reinstanced* UObjects via reflection — "unreliable and frequently causes
  blueprint corruption" (values reset to defaults, dangling pointers). This is the
  canonical refuse-and-restart cautionary tale and the closest precedent for Buiy's
  `#[require]`/Component restart line.
- **Unity domain/scene reload + Enter Play Mode Options** — the state-RESET axis:
  *disabling* domain reload preserves statics but is a footgun (stale counters,
  duplicate static-event subscriptions) requiring manual reset seams
  (`RuntimeInitializeOnLoadMethod`). The two-mode model — reload-on vs reload-off —
  IS Buiy's needed "hot restart (rebuild, lose runtime state)" vs "hot reload (keep
  World)" toggle; ship both, named and distinct.
  Sources: `liveplusplus.tech/docs`; `dev.epicgames.com/.../using-live-coding`;
  `docs.unity3d.com/.../domain-reloading.html`.

### Cluster D — Declarative-DSL reload (the directly-relevant `.bsn` model)

- **Slint** — *Mechanism:* `slint-interpreter` re-interprets the `.slint` at
  runtime; the live-preview makes a *compiled* app transparently interpreted (the
  compiler "replaces native code generation with stubs that watch the filesystem").
  *State:* "properties, models, and callbacks you've set are preserved" — snapshot
  host-set state → re-interpret → re-apply. *Honest limit (#8369):* it re-applies
  *all* data, so source edits to those properties are masked and callbacks can get
  "stuck"; a "Reset state" command is the requested escape hatch. *Applicability:*
  the compiled-vs-interpreted **duality** names Buiy's exact constraint — `bsn!`
  macro = compiled (never reloads), a future `.bsn` asset = interpreted (where
  reload lives). #8369 is exactly what `apply_scene` ("writes directly on top of
  existing components") will hit: typed text/caret/scroll reset every reload unless
  components are split author-owned vs runtime-owned.
- **Makepad** — the strongest state-preservation model: an explicit author-declared
  `#[live]` (re-applied from the DSL) vs `#[rust]` (runtime state, untouched) field
  split; `ApplyFrom::{NewFromDoc,UpdateFromDoc,Over}`, `apply_over(cx, &[LiveNode])`,
  `LiveHook::after_update_from_doc`, websocket `StudioToApp::LiveChange` delivery,
  and inline MPSL `reset_for_live_reload`. The ECS has the analog for free
  (`bsn`-authored components ~ `#[live]`; system/runtime components ~ `#[rust]`);
  `apply_over` ~ `apply_scene`; `after_update_from_doc` ~ an on-reload observer that
  re-derives caches. This is the keystone analog for Buiy's component-ownership split.
- **bevy_flair** — CSS asset-watch → re-cascade onto existing components; state is
  preserved *by construction because the tree is never rebuilt*. The template for
  Buiy's dimension-2: ship STYLE reload FIRST, independently, never gated behind
  scene reload. (Buiy re-applies *tokens*, not selector matches — lower complexity.)
- **belly** — the `.eml`(tree)/`.ess`(style) hot-reload SPLIT mirrors Buiy's
  SCENE-vs-STYLE dimensions (design them independently); cautionary point: tree
  reload trends toward rebuild unless state lives outside the description. Buiy has
  no reactive binding layer (an explicit non-goal), so it must rely on the
  component-ownership split.
- **NoesisGUI** — the BAR for state-preserving SCENE reload and the only explicit
  state-preservation contract in the corpus: parse the edited XAML to see *exactly*
  what changed, patch only affected controls, preserve `DataContext`/instance state.
  This structural differ is precisely what `apply_scene` lacks; `bsn` `#Name` refs
  are Buiy's identity key. (Buiy's "data context" is the ECS itself — cleaner than a
  re-bound view-model.)
  Sources: `docs.slint.dev/.../slint_interpreter`; `slint-ui/slint#8369`;
  `makepad.rs`; `github.com/eckz/bevy_flair`; `noesisengine.com/docs/Gui.Core.HotReloadTutorial.html`.

### Cluster E — Live-code lineage / state-migration canon

The deep canon the modern Rust/Bevy tools rediscover. Unifying thesis: **a reload
is an explicit, versioned, identity-preserving, FALLIBLE `migrate(old) → new`.**

- **Erlang/OTP** — two-version code coexistence + the explicit fallible
  `code_change(OldVsn, State, Extra)` callback (transform the running state record;
  on `{error,_}` the whole upgrade *rolls back*), orchestrated by `appup`/`relup`.
  The deepest precedent for migrating live runtime state across a code version — the
  pattern (version-keyed migrate hook + rollback + suspend→migrate→resume window)
  transfers; the BEAM two-version code table does not (Rust has no VM equivalent).
- **Smalltalk `become:`** — atomic identity-preserving reshape of live objects; the
  transferable invariant is *identity preservation across reload* — the exact line
  between hot-reload and hot-restart. In an ECS, identity = the Entity id;
  `bsn` patch-apply (keep the Entity, patch components) is the bounded, safe
  `become:` analog. A global heap rewrite is unsafe and unnecessary in Rust.
- **CLOS `update-instance-for-redefined-class`** — the cleanest formal model: a
  three-way structural diff (added/discarded/retained slots) with a default plus a
  customization hook, *discarded values surfaced* rather than dropped. Auto-derive
  the trivial part from Reflect; require a hand hook only for non-trivial remaps.
  (CLOS migrates lazily via a metaobject protocol; Buiy must migrate *eagerly* over
  a reflected query — only `#[derive(Reflect)]` components can participate.)
- **Elm / Redux** — model-is-data: reload = re-run pure `update` over the preserved
  model; the load-bearing *caution* — serialization is necessary but NOT sufficient,
  and a model shape change forces a fallback to restart (elm-watch detects it).
  Direct Buiy hazard: the un-reflected `TextEditState`/`TextBuffer` islands are
  exactly the "un-serializable island" Elm's limitation predicts.
  Sources: `erlang.org/doc/.../gen_server`; `gbracha.blogspot.com/2009/07/miracle-of-become.html`;
  `clhs.lisp.se/Body/f_upda_1.htm`; `lydell.github.io/elm-watch/hot-reloading`.

### Cluster F — Bevy ecosystem + `.bsn` roadmap

Covered inline above (`apply_scene`/`ScenePatch`/`resolve_scene_patches` as the
upstream re-apply seam; the deferred `.bsn` loader; the upstream `hotpatching`
integration). The single highest-leverage *correction* this research surfaces:
the existing `docs/prior-art/bevy-ui/open-problems.md` framing of component
hot-reload as "fully blocked" is wrong — the re-apply *primitive* already exists;
the gap narrows to **loader + keyed reconciler + state policy**.

---

## Part 3 — Per-dimension feasibility matrix + layered recommendation

### Feasibility matrix

| Dimension | Current state | Feasibility | Approach | Effort / Risk |
|---|---|---|---|---|
| **1. Asset** (fonts/images/themes from disk) | Font reload fully landed as the beachhead (`text/registry.rs`, `font_asset.rs`); the only asset type with reload; OS file-watch not confirmed-enabled (app-injected today). | **already-partly-there** | Generalize the font lock-discipline + generation-bump to image assets; enable `watch_for_changes` behind a dev feature (native only). | **low** / **low** — the font path empirically proves no leak/growth + full survival. |
| **2. Style / theme** | `Theme` is a Rust-authored Resource (`theme.rs:18-25`); the extract-time `is_changed()` re-resolve gate already exists (`extract.rs:941`). | **high** | Theme/token asset (TOML/JSON/RON — *not* CSS) + `AssetEvent::Modified` observer rebuilding `Theme`, riding the existing gate. State-preserving by construction. | **medium** (format + loader + observer; no render changes) / **low-medium** — must preserve per-entity overrides + programmatic mutations; watch opt-in. |
| **3. Scene / `bsn`** | `bsn!` compile-time only; `apply_scene`/`ScenePatch`/`resolve_scene_patches` exist upstream but `apply` *spawns* children; `.bsn` text loader deferred (PR #23413, `follow-ups.md:1433`). | **blocked-on-upstream** | Capture the DESIGN now (triad classification; keyed `#Name` reconciler; per-component ownership policy); gate the build on the upstream loader OR write an interim Buiy text→`Scene` loader. | **high** (interim loader + keyed reconciler + state policy) / **high** — blind `apply_scene` clobbers runtime state (#8369); naive whole-tree apply duplicates children; respawn churns Entity ids. |
| **4. Rust code** | Bevy `hotpatching` verified in the `0.19.0-rc.3` pin (`hotpatch.rs:22`, `function_system.rs:509,537,704-705`, `Cargo.toml:278-280`); OFF by default. | **high** | Dev-only `buiy/dev-hotpatch` feature → `bevy/hotpatching`; designate a gallery/`hello_*` smoke target; add a `.run_if(on_message::<HotPatched>)` re-sync to invalidate cached shaping/layout/instances. No direct `subsecond` dep. | **low** to flip; **medium** for re-sync + an honest coverage matrix / **medium** — binary/tip-crate-only, native-only, body-only, experimental upstream; must no-op on release+wasm. |
| **5. State-preservation** | Structural state Reflect-registered (176 `register_type`); machinery state (`TextEditState` `state.rs:92-126`, `TextBuffer` `components.rs:572`, visuals, render caches) deliberately un-reflected → destroyed on respawn; Entity-id churn invalidates every Entity-keyed map. | **medium** | Make **in-place patch** the primitive (preserve Entity identity, re-apply only description-derived components, leave runtime-state untouched) — HIGH feasibility. Serialize-migrate-respawn is LOW (un-reflected islands, no id remap). For shape changes: a Reflect-driven, versioned, fallible `migrate(old)→new` with rollback-to-restart. Ship two named modes: hot-reload vs hot-restart. | **high** (id-remap layer; migration framework; reflect/carve-out the text islands) / **high** — silent drops, no remap layer, in-flight IME/momentum/clocks reset unless checkpointed; auto-migrating layout in place is UB territory. |

### Layered recommendation (cheapest / highest-value first)

Each layer names the **Buiy substrate** it builds on and the **gap** to close.

- **L0 — Font asset reload beachhead (LANDED).** The proven asset-reload pattern
  and pattern-*source*: `AssetEvent::Modified` remove+re-add under one lock + one
  generation bump, with leak/growth survival tests. *Builds on:* on main
  (`text/registry.rs`, `font_asset.rs`). *Gap:* scoped to fonts; watch not
  confirmed-enabled (programmatic today).

- **L1 — Rust-code hot-reload for the DEV LOOP.** Cheapest near-free stateful win:
  flip `bevy/hotpatching` on a gallery/example binary; change widget/system behavior
  live while keeping all caret/scroll/focus/text state (the World is never
  re-spawned). *Builds on:* the vendored `hotpatching` feature (OFF by default,
  zero cost). *Gap:* binary/tip-crate-only, native-only, body-only; add a
  `HotPatched` re-sync invalidation pass.

- **L2 — STYLE/THEME asset hot-reload.** Cheapest *shippable*, state-preserving-by-
  construction win: theme/token asset + `AssetEvent::Modified` observer rebuilding
  `Theme`, riding the existing `is_changed()` gate. **Ship FIRST and independently.**
  *Builds on:* L0 lock-discipline + the `extract.rs:941` re-resolve gate +
  decomposed `ColorToken` paint components. *Gap:* a file format (not CSS),
  variant-binding (`prefers_dark`), the unwired per-entity override; watch opt-in.

- **L3 — Generalized asset watch.** Enable `watch_for_changes` behind a dev feature
  (native) and extend the font pattern to image assets. *Builds on:* L0 + L2.
  *Gap:* programmatic-only today; cascading multi-file invalidation semantics
  undefined.

- **L4 — Component-ownership CONTRACT + author-stable key (design keystone).**
  Classify every author-facing component as description-derived (re-applied) vs
  runtime-state (preserved); define an author-stable `ValueKey`/`testTag` decoupled
  from Entity bits. Makes patch-not-replace decidable. *One key serves the
  hot-reload reconciler + a11y identity + agent locators.* *Builds on:* the 176
  `register_type` inventory + `bsn` `#Name` refs + the agent-interface report's
  open-Q2 key need. *Gap:* not formalized; the key mechanism is unbuilt.

- **L5 — SCENE/`bsn` hot-reload (the big lift).** Re-apply a changed `.bsn` as a
  PATCH onto existing entities via `apply_scene`, plus a keyed child reconciler
  keyed on `#Name`, plus the L4 state policy. *Builds on:* L4 + the upstream
  `apply_scene`/`ScenePatch`/`resolve_scene_patches` seam. *Gap:* blocked on the
  upstream `.bsn` text loader (needs an interim Buiy loader); the keyed reconciler
  is novel (`apply_scene` only spawns children).

- **L6 — State-migration framework for shape changes.** Reflect-driven, versioned,
  fallible `migrate(old)→new` with rollback-to-restart, for changed Component
  layouts; reflect or carve out the text islands so live text/caret/selection/undo
  can round-trip. *Builds on:* L4 + L5 + the Erlang `code_change` / CLOS / rmp_serde
  precedents. *Gap:* `TextEditState`/`TextBuffer` are un-reflected; no Entity-id
  remap layer; the data-layout wall is the universal hard boundary.

---

## Use-case fork (and the library-boundary scoping)

Two use cases with different feasibility, state stories, and library placement —
keep them separate in the spec.

- **(A) Dev-time DX** — a fast iteration loop for Buiy/app authors: Rust-code
  hotpatch on example/gallery binaries (L1) + style/asset file-watch (L2/L3). This
  is inherently **dev-only and native-only** (`connect_subsecond` is compiled out on
  wasm; *all four* reference systems — Flutter `reassemble`, subsecond, SwiftUI
  dynamic replacement, Live++/UE — are debug-only) and must compile to a **no-op on
  release AND on wasm**. Code-patch (dim 4) can never be a shipped or web feature.

- **(B) Shipped feature** — a state-preserving SCENE/THEME reload an app could ship
  at runtime (live-editing tools, design-system swap, agent-driven UI mutation):
  L2 (theme) now and L5/L6 (scene + migration) later. This path **is web-reachable**
  — but only via APP-DRIVEN reloads (the font beachhead's `reregister_bytes` /
  pushed-bytes model), **NOT** via `AssetPlugin.watch_for_changes` on web (Bevy's
  `notify`-based `file_watcher` does not run on wasm). The spec must not promise
  filesystem-watch reload on the web target.

**Library-boundary scoping** (mirroring the agent-interface precedent's "ship the
substrate vs the app owns transport/policy" split): Buiy the foundation library
ships the **substrate** — the re-apply primitive (`apply_scene` wrapper), the
description-vs-runtime ownership contract, the keyed reconciler, the
generation-bump invalidation, the Reflect-migration seam, and the dev-only feature
flag. The **app** owns the **policy/transport** — whether reloads come from
file-watch vs websocket vs an agent trigger, when to reload, and the dev-vs-prod
gating. Buiy must not bake a file-watcher or a websocket studio protocol into the
library; it exposes the seams and lets the app drive them, exactly as the font
beachhead today exposes `reregister_bytes` and lets the app inject the reload.

---

## Design tensions

- **Despawn+respawn (easy) vs keyed identity-preserving in-place patch (hard).**
  Respawn is hot-RESTART masquerading as hot-reload — it mints new Entity ids and
  breaks every Entity-keyed map (`LayoutTree.by_entity`, `FocusedEntity`,
  `TopLayerActivation`, `GlyphEntityRun`). In-place patch is the only thing that
  preserves caret/scroll/focus/blink.
- **Re-apply ALL authored components (simple) vs preserve runtime-mutated values**
  (the Slint #8369 / `apply_scene` "writes directly on top of existing components"
  clobber hazard). Typed text, caret, selection, scroll reset every reload unless
  components are split author-owned vs runtime-owned *per component*.
- **In-place patch (works only when state SHAPE is unchanged) vs serialize-migrate-
  respawn** (handles shape change but is blocked by the un-reflected
  `TextEditState`/`TextBuffer` islands and has no Entity-id remap layer).
- **Dev-only convenience vs shipped capability.** Code-patch preserves World state
  for free but is dev/native-only; scene-respawn rebuilds state and is web-reachable.
  Conflating them hides two feasibility/state profiles and risks leaking machinery
  into release/wasm.
- **Reflecting `TextEditState`/`TextBuffer`** (enables snapshot+migrate) **vs the
  deliberate facade boundary** that keeps them un-reflected (they carry
  `cosmic_text` types that should not cross the reflection surface).
- **Library substrate vs app transport/policy** — where do the file-watcher, the
  dev-vs-prod gate, and the reload trigger live? The agent-interface precedent says
  substrate in the library, transport/policy in the app.
- **Gate on the upstream `.bsn` loader (wait, no ETA) vs write an interim Buiy
  text→`Scene`/`ScenePatch` loader now** (unblocks dim-3 but is throwaway if
  upstream diverges).
- **Author-stable key as a NEW concept vs reuse** — one key must serve the
  hot-reload reconciler, a11y stable identity, AND agent locators (a triple-consumer
  investment, not three separate ones).

---

## Rejected alternatives

- **Despawn+respawn the whole `bsn!` tree and call it "hot reload."** It is a hot
  RESTART; mints new Entities, drops caret/selection/scroll/focus/blink, and dangles
  every Entity-keyed reference. The single biggest trap.
- **A direct `subsecond` dependency.** Bevy already vendors and wraps it behind
  `hotpatching`; a direct dep duplicates the jump-table runtime and risks version
  conflict. Reach it only via `bevy::app::hotpatch`.
- **dexterous_developer as the code-reload tool.** Archived 2026-02-08, Bevy 0.14
  ceiling (5 majors behind); porting is from-scratch against dead upstream. Borrow
  its `rmp_serde` serialize-across-reload migration CONCEPT only.
- **hot-lib-reloader.** ECS-hostile: dylib reload changes `TypeId`s (breaking
  Component/Resource identity), `tracing` crashes on reload, and its "no state in
  the hot lib" model fights the ECS World. Superseded by the upstream integration.
- **A CSS selector/cascade engine for theming (bevy_flair-style).** Buiy has a typed
  decomposed-token model, not a CSS cascade; CSS is explicitly out of scope.
  Re-apply tokens, don't run selector matching.
- **A reactive binding / `DataContext` layer to carry state across reloads** (the
  Noesis/belly route). An explicit Buiy non-goal; the ECS (resources + non-authored
  components) is the cleaner, already-present state home.
- **Serialize-the-whole-World-and-respawn as the PRIMARY reload model.** The
  un-reflected `TextEditState`/`TextBuffer` islands are silently dropped; Elm proves
  serialization is necessary-but-not-sufficient and shape changes must fall back to
  restart.
- **Smalltalk `become:`-style global heap rewrite.** Unsafe and unnecessary in
  Rust; the ECS already gives the bounded, safe version (query + in-place `&mut`
  over a stable Entity id).
- **Promising Rust-code hot-reload on wasm/web or as a shipped feature.**
  `connect_subsecond` is compiled out on wasm; all reference systems are debug-only.
  Web gets asset/style/scene reload only.
- **Building the component-hot-reload machinery ahead of the upstream `.bsn`
  loader.** The implementation is gated; only the DESIGN (triad, reconciler,
  migration) can and should precede it.
- **Auto-migrating live Component/Resource layout in place.** UE's legacy Hot Reload
  reinstancing is the canonical corruption failure (blueprint corruption, values
  reset to defaults, dangling pointers). Refuse-and-restart on layout/signature
  change is industry consensus, not a compromise — and for subsecond it is a
  documented *crash* hazard, not merely a restriction.

---

## Open questions (for the spec stage)

1. What is the author-stable key mechanism (`ValueKey`/`testTag`), and can ONE key
   genuinely serve the hot-reload reconciler + a11y stable identity + agent locators
   (agent-interface open-Q2)? The linchpin shared investment.
2. Will Bevy upstream ship the `.bsn` text loader, and on what timeline? Build an
   interim Buiy text→`Scene`/`ScenePatch` loader now, or wait?
3. Should `TextEditState`/`TextBuffer` be made Reflect (round-trip a snapshot),
   carved out with a hand-written migration, or designated never-touch
   (in-place-only)? This decides whether serialize-respawn is ever viable for the
   text-rich widgets.
4. Per-component ownership classification: exactly which components are
   description-derived (re-applied) vs runtime-state (preserved)? Edge cases — the
   per-entity `Theme` override, interaction/hover/pressed state, animation clocks?
5. Animation-clock policy on reload: reset or continue? (`CaretBlink.origin` today;
   future CSS/layout transitions.) Unity/Live++ show in-flight machines need explicit
   checkpoint-or-reset.
6. Cascading multi-file/asset reload semantics: does a changed token file trigger
   full or partial re-evaluation of dependents? Cycle detection? Orphan cleanup?
   (No prior-art folder systematizes this.)
7. Entity-id remap strategy for the respawn path: path-based re-identification
   (`ChildOf` walk + position) vs author-key. Cost of a remap pass over
   `LayoutTree.by_entity`, anchors, the modal stack?
8. How does the keyed child reconciler handle add/remove/reorder of children while
   preserving identity (and per-child state) for unchanged subtrees? The Noesis-grade
   hard part `apply_scene` lacks.
9. Do in-flight IME composition / scroll momentum survive or reset across a
   code-body patch (subsecond) vs a scene patch? They must be checkpointed or
   documented as resetting.
10. Is "asset/style/scene reload only, no code reload" an acceptable WASM/web
    hot-reload scope for the landed WASM design?

---

## Recommended next steps

1. **Charter the named owner spec `buiy-asset-pipeline-design` plus a sibling
   hot-reload design;** capture the description/identity/state triad, the keyed
   child reconciler (keyed on `bsn` `#Name`), and the Reflect-driven versioned
   fallible migration DESIGN now — design precedes the upstream loader.
2. **Ship STYLE/THEME asset hot-reload FIRST** as an independent, low-risk
   capability (theme/token asset + `AssetEvent::Modified` observer riding the
   existing `theme.is_changed()` gate); keep it explicitly separate from scene
   reload in the spec.
3. **Land a dev-only `buiy/dev-hotpatch` feature** wiring `bevy/hotpatching` on the
   gallery/a `hello_*` binary; document the `BEVY_ASSET_ROOT='.' dx serve
   --hot-patch` loop; add a `.run_if(on_message::<HotPatched>)` re-sync pass
   invalidating cached shaping/layout/instances; write an honest reloadable-coverage
   matrix (what reloads vs requires restart).
4. **Enable `AssetPlugin` `watch_for_changes` behind a dev feature** (native);
   generalize the font beachhead's lock-discipline + generation-bump to image
   assets.
5. **Formalize the description-derived vs runtime-state component contract** as a
   documented invariant; define the author-stable key and coordinate it with the
   agent-interface report's open-Q2 (one key, three consumers).
6. **File the candidate prior-art folders** (§13); in particular CORRECT
   `bevy-ui/open-problems.md`'s "hot-reload fully blocked" framing to "the re-apply
   primitive (`apply_scene`/`ScenePatch`) already exists; the gap narrows to loader +
   keyed reconciler + state policy", and cross-link `follow-ups.md:1433` to the new
   design.
7. **Decide the `TextEditState`/`TextBuffer` reflect-vs-carve-out question;** spec
   the versioned fallible migrate seam with rollback-to-full-restart and a
   Slint-style "reset state" escape hatch.
8. **Match this report's rigor against the precedent** at
   `docs/reports/2026-06-18-agent-interaction-surface-research.md`.

---

## Prior-art folders to spawn / extend (per `using-prior-art`)

The corpus has 44 folders; none is focused on hot-reload as a cluster, though the
cross-framework hot-reload facets are scattered across existing UI folders.
Verified: the four CREATE candidates below are all absent (no duplication), and the
seven EXTEND targets all exist under the cited names. Systems already covered get a
hot-reload *facet*; only genuinely-uncovered systems get a NEW folder. Ranked.

**Create (genuinely uncovered):**

- **`subsecond-hotpatching`** (high) — the native-Rust code-reload cluster directly
  available to Buiy in one folder: Dioxus subsecond's jump-table mechanism
  (incremental-link + pointer swap, `HotFn`/`call`, `HotFnPanic`, struct-layout/
  statics/generics caveats, tip-crate-only, *experimental*) AND Bevy's upstream
  integration Buiy can flip on today (`HotPatchPlugin`, `HotPatched`/
  `HotPatchChanges`, per-system `HotFn::current`/`refresh_hotpatch`, the
  `hotpatching` feature present in `0.19.0-rc.3`, wasm exclusion). Plus a short
  comparison covering dexterous_developer (archived; keep its `rmp_serde`
  serialize-across-reload pattern) and hot-lib-reloader (ECS-hostile via `TypeId`).
  *Category:* Bevy/Rust ecosystem + devtools (siblings: `bevy-remote-protocol`,
  `engine-devtools-protocols`, `dioxus`); cross-links `dioxus`.
- **`stateful-ui-hot-reload`** (high) — the cross-framework stateful-reload gold
  standard as one coherent body (mirroring how the agent-interface report folded
  Flutter+Compose into one folder): Flutter `reassemble()` + the Widget/Element/State
  triad; React Fast Refresh's `$RefreshReg$`/`$RefreshSig$` families +
  hook-signature-stability + the only-export-components boundary; SwiftUI Previews'
  `@_dynamicReplacement` + `@State`-in-AttributeGraph keying. The unifying
  description/identity/state-cell model, the "what forces a restart" boundary lists,
  and the explicit mapping onto Buiy's ECS retained tree. Distinct from the existing
  `flutter-golden-testing` folder (golden testing, not hot-reload) and
  `retained-mode-semantics-automation` (automation, not reload). *Category:*
  UI-framework reload-and-iteration semantics (siblings: `slint`, `makepad`,
  `noesisgui`, `unreal-slate-umg`, `unity-ui`).
- **`native-engine-live-coding`** (medium) — the production-scale native live-coding
  discipline in one folder: Live++ in-process machine-code patching + hotpatchable-
  build requirements + pre/post-patch data-layout-migration hooks + hard limits
  (stack frames, TLS, LTCG); Unreal Live Coding AND the legacy Hot Reload
  reinstancing corruption history (the canonical refuse-and-restart cautionary
  tale); Unity domain/scene reload + Enter Play Mode Options (the two-mode reload-on/
  off + static-reset footgun). Distinct from `unreal-slate-umg` and `unity-ui` (UI,
  not live coding). *Category:* game-engine live-coding / reset-seam discipline.
- **`live-update-state-migration-lineage`** (medium) — the deep canon the modern
  tools rediscover, consolidated: Erlang/OTP two-version code loading + the fallible
  versioned `code_change` + `appup`/`relup` rollback; Smalltalk `become:`
  identity-preserving reshape; CLOS `update-instance-for-redefined-class` three-way
  structural slot diff; Elm/Redux model-is-data reload + the serialization-necessary-
  but-not-sufficient limitation. Thesis: a reload is an explicit, versioned,
  identity-preserving, FALLIBLE `migrate(old)→new` — the anchor for Buiy's
  dimension-5 migration seam. *Category:* a new live-programming / state-migration
  lineage (no existing sibling).

**Extend (already covered — add a hot-reload facet):**

- **`bevy-ui`** (high) — the highest-leverage CORRECTION: extend `open-problems.md`'s
  "Hot-reload of components" with the bevy_scene re-apply SEAM cited to file:line
  (`apply_scene` "writes directly on top of existing components" `spawn.rs:459`;
  `ScenePatch` is an Asset; `resolve_scene_patches` re-resolves on `AssetEvent`
  `spawn.rs:603`; `ResolvedSceneRoot::apply` SPAWNS children `resolved_scene.rs:62`).
  Corrects "fully blocked" → "the primitive exists; the gap is loader + keyed
  reconciler + state policy." Cross-reference the two new create folders.
- **`makepad`** (high) — extend `live-language.md` with the precise APPLY mechanism:
  the `#[live]` vs `#[rust]` split (the direct ECS-component-ownership analog),
  `ApplyFrom::{NewFromDoc,UpdateFromDoc,Over}`, `apply_over(cx,&[LiveNode])`,
  `LiveHook::after_update_from_doc`, the `StudioToApp::LiveChange` websocket delivery
  + `handle_live_edit`/`process_file_changes`, inline MPSL `reset_for_live_reload`.
  The cluster's strongest state-preservation model.
- **`slint`** (high) — add a focused `live-preview-and-interpreter.md`: the
  `slint-interpreter`, the compiled-vs-interpreted DUALITY (the compiler swaps native
  codegen for filesystem-watching stubs) that names Buiy's exact `bsn!`-macro-vs-
  `.bsn`-asset constraint, the snapshot→re-interpret→re-apply model, and the honest
  #8369 limit (re-apply-all stomps source edits; needs a Reset-state command). The
  single most directly relevant system for `.bsn` reload.
- **`bevy-flair`** (medium) — add a `hot-reload.md`: the asset-watch → re-cascade
  mechanism (`AssetEvent<StyleSheet>::Modified`, `NodeStyleSheet`/`NodeStyleData`/
  `ComponentPropertyRef`, minimum-affected-nodes) and the lesson that STYLE
  hot-reload is SEPARABLE and preserves state by construction — the template for
  Buiy's dimension-2 to ship first.
- **`noesisgui`** (medium) — add the structural-diff hot-reload mechanism as the BAR
  for state-preserving SCENE reload (the only explicit state-preservation contract in
  the corpus): parse-XAML-to-see-what-changed, patch only affected controls, preserve
  `DataContext`/instance state. Frames what Buiy's keyed reconciler must achieve and
  what `apply_scene` lacks, with `#Name` refs as the identity key.
- **`dioxus`** (low) — cross-link: note that subsecond was upstreamed into Bevy (PR
  #19309, Bevy 0.17) and is reachable directly from Buiy's 0.19 pin, and point to the
  new `subsecond-hotpatching` folder for the depth.
- **`belly`** (low) — one-line note in `lessons.md`: the `eml`(tree)/`ess`(style)
  hot-reload SPLIT mirrors Buiy's SCENE-vs-STYLE dimensions; tree reload trends
  toward rebuild unless state lives outside the description — reinforcing the ECS
  ownership-split conclusion (Buiy has no reactive binding layer).

---

## Provenance

Multi-agent research workflow: an 11-way internal-capability audit (cited to
`file:line`) + external prior-art sweep (web-researched, sourced) → synthesis →
two adversarial verification passes (one repo-scoped code verifier, one external
source verifier). Both verdicts **sound-with-corrections**; the corrections are
folded into this report: the off-by-one/range line-number fixes (`resolve_scene_patches`
at `spawn.rs:603`, `HotPatchPlugin` struct at `hotpatch.rs:22`, `TextEditState` at
`state.rs:92-126`, `resolve_token` at `color.rs:127`); `ScrollOffset` located in
`layout/components.rs:521-523` (not `scroll.rs`); the exact `register_type` call-site
count (176; 151 `buiy_core` + 25 `buiy_widgets`); the softening of "font is the only reload surface / watch disabled" to "the
only *confirmed* reload surface, app-injected today" pending a direct grep; the
sharpening of subsecond's maturity to "experimental, opt-in `--hotpatch` flag" and
its struct-layout limit to a documented crash hazard; "native-only" reframed from a
permanent wall to an unimplemented gap; the web-asset-reload caveat (no OS
file-watch on wasm — app-driven only); the hardened "blocked-on-upstream" verdict
for the `.bsn` loader (PR #23413 + Bevy 0.19 release notes + issue #24309 + tracking
issue #23637); and the Entity-id-churn invalidation presented as design inference,
not a measured code fact. Time-sensitive ecosystem facts (the upstream `.bsn` loader
status, the subsecond/Dioxus maturity, the dexterous_developer archive) are flagged
inline and should be re-verified before the spec freezes.
