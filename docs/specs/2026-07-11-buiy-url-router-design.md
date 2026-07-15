# Buiy URL router — web view-navigation seam (design)

**Date:** 2026-07-11
**Status:** draft
**Amends:** [`2026-05-07-buiy-foundation/cross-cutting.md` § 3.13](2026-05-07-buiy-foundation/cross-cutting.md) + [`README.md` non-goals](2026-05-07-buiy-foundation/README.md) — a **targeted scope carve-out** (§ 1 below), not a full reversal.

> **This is issue #143 Item 7 ("URL router with shareable routes").** The audit
> (`reports/2026-07-11-web-first-class-audit.md`) found it 0% landed and gated on a scope decision:
> the foundation currently declares URL routing out of scope. This spec makes the minimal carve-out
> and designs a thin, additive, web-only Router seam that **drives the app's existing MVU nav model**
> — it does not invent a new navigation system.

## Purpose

Let a Buiy web app expose **shareable, deep-linkable, back/forward-navigable URLs** that reflect
which view is showing, mapped to MVU `Model` state. On native the same routing *logic* runs but no
URL is touched (in-memory history). The app author supplies the route grammar; the framework supplies
the browser-History plumbing they cannot reach through Buiy today.

## 1. Foundation scope carve-out (targeted, not a reversal)

The exclusion is bundled with *persistence*:
- `cross-cutting.md:32` (§ 3.13): *"**Out:** History API / URL routing, `localStorage` /
  `sessionStorage` / `IndexedDB`. UI does not own persistence or routing. **O**"*
- `README.md:49` non-goal: *"Networking, persistence, routing/URL navigation … data and transport are
  the consuming app's concern."*

The rationale is a *layering* argument (UI ≠ persistence/transport owner), and the non-goal names
routing **independently** (*"UI does not own persistence **or** routing"*) — so this is not merely a
persistence rider to peel off. The carve-out argues on the **merits** instead: **URL-as-view-navigation
touches no storage, no transport, and no data model.** A URL that names which subtree is rendered is
pure UI navigation reflecting `Model` state the framework already owns and already lets the app switch
in-app (the gallery's `NavModel`); the *app* still authors the route grammar and owns which ids are
shareable. What it cannot do without framework help is reach `window.history` — the one seam only the
framework's wasm layer can provide. The tier is `O` = "out *with reason*", not "never" — and its sibling
§ 3.13 entry (the Signal/computed layer, `cross-cutting.md:31`) is also `O` yet explicitly says *"may
return as a follow-up sub-spec if usage demands it."* The same door is open here; #143 is the demand.

**The carve-out (minimal):** split the bundle. Persistence (`localStorage`/`sessionStorage`/
`IndexedDB`) and transport stay `O`. Carve back in *only* **"URL ↔ which-view navigation"** as a
**web-only, additive, `router`-feature-gated seam that no-ops (in-memory) on native.** Concretely, the
plan edits `cross-cutting.md:32` to keep storage `O` and move "URL routing (History-API view-navigation
seam)" to a `C`-tier entry pointing at this spec, and adds one clarifying README line. Everything above
the seam (the route grammar, which ids are shareable) stays app-authored.

*Rejected:* a broad "routing framework" (nested route trees, layouts, typed-param macros, route
guards). Rejected as over-reach — #143 needs shareable view URLs, not a Leptos/Dioxus-scale router.
Nesting/params are deferred until a real app needs them (§ 4).

## 2. What already exists (do not rebuild or confuse)

Two artifacts are named "router" but are **not** URL routers:
- `crates/buiy_view/src/router.rs` (#106/#111/#127) — an **event→Msg router** (`OnPress` →
  `PressAction`, `TextChanged` → `InputAction`, etc., stored as replay-safe Msg *values*). No URLs.
- The gallery **`ScreenRouter`** (`examples/buiy_gallery/src/shell.rs:163`) — an in-app screen
  switcher, now an MVU projection of `NavModel(Screen)` (`shell.rs:189`), driven by
  `NavMsg::Switch(Screen)` (`:198`) through the pure `nav_reducer` (`:205`, `set_if_neq`-idempotent),
  applied by `apply_screen_router` via `fold_one_inline::<NavModel>` (`:~1516`).

**This `NavModel`/`NavMsg`/`nav_reducer` triad IS the route model a URL router drives** — the router
does not replace it. The MVU hooks it uses (`crates/buiy_core/src/mvu/mod.rs`): `enqueue::<M>` (`:630`,
push a nav Msg in), `fold_one_inline` (`:1048`), `add_model`/`add_reducer` (`:1060`; every `Msg` is
`Reflect`, so route serialization is already available), and `LogicalId` (`:288`, the session-stable
id the record log keys on).

## 3. Decisions

### D1 — A `History` provider trait (mirror `ClipboardProvider`)
`push(path)` / `replace(path)` / `current() -> String` + a pop subscription. Two cfg-selected impls,
exactly the seam discipline Buiy already uses for clipboard (native `arboard` vs wasm `WebClipboard`):
- **wasm `BrowserHistory`** — wraps `web_sys::window().history()`: `pushState`/`replaceState` +
  `location.pathname`; a `popstate` listener. Adds web-sys features `History`, `Location`,
  `PopStateEvent` to `crates/buiy_core/Cargo.toml:103` — **no new crates, no new lock entries** (the
  wasm web-sys/wasm-bindgen substrate is already present for the IME/clipboard/a11y seams).
- **native `MemoryHistory`** — an in-memory stack so in-app back/forward still works on desktop
  (the Dioxus-desktop pattern); **no URL is ever touched on native.**

*Rejected:* hash-routing (`/#/view`) as the default. Rejected — uglier, collides with in-page anchors;
path-routing is cleaner for sharing. (Hash kept as a fallback option where a host can't do SPA
path-rewrite — see D5.)

### D2 — Generic over the app's route model; app supplies `to_path`/`from_path`
The framework is generic over the app route type `R` (e.g. the gallery's `NavModel`). The app supplies
two pure fns: `to_path(&R) -> String`, `from_path(&str) -> Option<R::Msg>`. The framework provides:
- **model→URL:** an observer on `Changed<R>` (Bevy change-detection — the gallery already navigates off
  change-detection, via `router.is_changed()` (`shell.rs:1562`) / `is_resource_changed::<ScreenRouter>()`
  (`:1587`), though on the `ScreenRouter` *resource*; the router applies the same idea one level up, on
  the route-model *component* `R`) → `to_path` → `history.push`.
- **URL→model:** a `popstate` (wasm) / memory-pop (native) callback → `from_path` →
  `enqueue::<R>(.., msg)` (`mvu/mod.rs:630`).

*Rejected:* a `#[derive(Routable)]`-style macro (Dioxus/Leptos). Deferred — for v1, two hand-written
fns per app are cheaper and `Reflect` already gives serialization; a derive can come later if apps
accrete many routes.

### D3 — Bidirectional-by-construction + echo-loop guard
`pushState`/`replaceState` do **not** fire `popstate` (it fires only on user back/forward), so the seam
must both write on model change *and* read on pop — never rely on pop to catch your own writes. The
feedback loop (model→URL→model→…) is broken two ways, both already available: `nav_reducer`'s
`set_if_neq` idempotence (`shell.rs:205`) folds a redundant `Switch` to no change, and a
`popstate`-originated fold uses **`replace` (not `push`)** / a re-entrancy flag so back/forward doesn't
mint duplicate history entries.

### D4 — Shareable grammar reuses `LogicalId`; v1 is flat single-segment
Grammar: `/{view}` for v1 (flat, one segment — matches the gallery's 5 sibling screens). Deep-linking
to an entity is `/{view}/{stable-id}` where the id is the MVU **`LogicalId`** (`mvu/mod.rs:288`) — so
the URL id-space, the replay/record log, and the agent-interface test-id space share **one** scheme
rather than inventing a third. The `/{id}` segment is **deferred** until an app needs per-entity deep
links. Framework owns parse/format; the app owns which ids are shareable. (Co-design this grammar with
any app-backend OG/permalink work — Item 8, out of framework scope — so a shared URL round-trips
through both.)
> **Precondition for un-deferring `/{id}`:** `LogicalId` is documented as *session-stable*
> (`mvu/mod.rs:288`). A *shareable* link needs an id durable **across sessions and visitors on the same
> build** (seed-scene-stable), not merely stable within one session — otherwise a pasted
> `/{view}/{id}` won't resolve for a different viewer. v1 (flat `/{view}`) is unaffected; but the
> deferred per-entity segment must first confirm/establish seed-scene-stable ids (aligns with audit
> § 3's "co-design with the id scheme"). Do not ship `/{id}` on session-only-stable ids.

### D5 — Path-based; note the host rewrite requirement
Path routes (`/scroll`) need the web host to serve the SPA shell on any path (History-fallback /
404-rewrite). The GitHub Pages deploy (`pages.yml`) must be configured for this (a `404.html` copy of
the shell, or hash-fallback where not possible). The plan owns wiring this; the spec flags it as a
launch prerequisite for path routing.

### D6 — Home: a `buiy_view` app-ext behind a `router` feature
The route model is an MVU model and `buiy_view` already owns the app-facing `ui::<M>()` sugar
(`crates/buiy_view/src/app.rs:87`). A `router::<R>(to_path, from_path)` ext registers the observer +
the pop listener + the provider in one call. Gate it behind a `router` cargo feature (Dioxus-style
carve-out; `prior-art/dioxus/lessons.md:70`) so non-web / non-routing apps pay nothing.

## 4. Deferred (named, not built)
Nested routes/layouts/outlets; typed-param derive macro; route guards; per-entity `/{id}` segment;
nav-driven a11y (focus move + `aria-live` announce on navigation) — **coupled to the still-unbuilt
a11y live-focus bridge** (audit § 3 D3; `a11y/web_sink.rs` is read-only). v1 router does **not**
announce nav; when the live-focus bridge lands, a route change becomes a natural place to drive focus +
announcement. Recorded here so the omission is deliberate.

## 5. Verification
- **Headless (native `MemoryHistory`):** unit-test `to_path`/`from_path` round-trip; push/pop drives
  `NavModel` via `enqueue`; back/forward pops the memory stack; echo-loop guard prevents duplicate
  entries / infinite folds. No browser needed — the lowest tier that observes routing.
- **Wasm (`BrowserHistory`):** a browser smoke asserting `pushState` updates `location.pathname` and a
  synthetic `popstate` re-drives the model — folded into the existing web-smoke harness
  (`tools/web-smoke/`), or a manual release-gate check per foundation § 2.9.
- **Shareability:** load a `/scroll`-style URL cold and assert the correct screen renders (the
  `?force=` loader hook + Pages deploy make this testable).

## 6. Open questions (for plan / review)
1. Native: `MemoryHistory` (keep in-app back/forward) vs a pure no-op? Recommend memory.
2. Path vs hash default given the Pages rewrite cost (D5)? Recommend path + `404.html` shell.
3. Does v1 announce nav a11y, or strictly defer to the live-focus bridge (D4/§4)? Recommend defer.
4. Route grammar co-design owner for the app-backend permalink case (Item 8).

## 7. References
- Audit: [`reports/2026-07-11-web-first-class-audit.md`](../reports/2026-07-11-web-first-class-audit.md) (Item 7).
- Prior-art: Yew `gloo-history` `History` trait (Browser/Hash/Memory impls) — the borrowed template;
  Dioxus Router (`dioxus-history` provider, desktop memory-history); Leptos Router (URL-as-truth, but
  DOM-anchor progressive-enhancement is N/A to a canvas). See `prior-art/dioxus/`.
- Code anchors: `crates/buiy_core/src/mvu/mod.rs` (`enqueue` :630, `LogicalId` :288, `add_model` :1060);
  `examples/buiy_gallery/src/shell.rs` (`NavModel` :189, `nav_reducer` :205, `apply_screen_router`);
  `crates/buiy_view/src/app.rs:87`; `crates/buiy_core/Cargo.toml:100-119` (wasm web-sys block).
- Sibling campaign track: [MVU Subscription ingest](2026-07-11-mvu-subscription-ingest-design.md); campaign roadmap: `plans/2026-07-11-web-first-class-campaign.md`.
