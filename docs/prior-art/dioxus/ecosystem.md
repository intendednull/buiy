**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — production users; comparisons vs Yew / Leptos / Sycamore / Iced / egui / React / Solid / Buiy

# Ecosystem and comparisons

## Production users

Dioxus is the most-downloaded React-shaped Rust UI framework (1.5M+ lifetime downloads, 424K in the last 90 days, per crates.io API 2026-05-22). The publicly named consumers, in rough order of citation frequency:

| Org | Use | Source / verification |
|---|---|---|
| **Airbus + European Space Agency (ESA)** | Collision-avoidance system UI | YC company page; sponsor relationship via FutureWei |
| **Huawei / FutureWei** | Internal apps (specifics not public) | Sponsorship relationship; YC launch post |
| **Satellite.im** | P2P Discord-like messenger | Sponsor; named in Dioxus's website "trusted by" section |
| **Cognition / Devin** (per dioxuslabs.com "trusted by" badge crawl) | Unspecified | Logo display only |
| **Various YC-batch sister companies** | Internal tooling | YC alumni community |
| **Community apps** (awesome-dioxus list) | Diverse | https://github.com/DioxusLabs/awesome-dioxus |

The trust-badges section on dioxuslabs.com displays **FutureWei, Airbus, ESA, Cognition, and Y Combinator** as logos. The Airbus/ESA use is the most-cited concrete production story; the others are less specified.

**Honest scale assessment.** Compared to React (millions of production deployments) or Yew (one of the most-installed Rust web frameworks pre-Dioxus), Dioxus's production-app count is small in absolute terms — likely **low-hundreds of production apps**, not thousands. The framework is best characterized as **"adopted at scale by serious organizations for internal tools, not yet behind a consumer-facing flagship that approaches the scale of, e.g., a Discord-class JS-app deployment."** Satellite.im is the closest thing to a consumer-facing Dioxus flagship and is itself research-grade in user count. Honest framing: Dioxus is **production-viable for the web target and webview targets; not yet behind a mass-consumer-facing app**.

## Comparisons

### vs Yew (Rust web framework)

**Yew** (yewstack, 2016–) is the prior-generation React-in-Rust-WASM framework. Mature, ~30K stars, Virtual DOM, hooks via `function_component` macro.

| | Dioxus | Yew |
|---|---|---|
| Targets | Web + Desktop + Mobile + SSR + Fullstack | Web only |
| Reactivity | Signals (0.5+) + Stores (0.7+) | `use_state` / `use_reducer` (React-shape) |
| Authoring DSL | `rsx!` | `html!` |
| VDOM | Yes (fiber-like) | Yes |
| Tooling | `dx` CLI | trunk + manual |
| Bundle size | Comparable | Comparable |
| Community | Larger and faster-growing | Long-established, slower-growing |

Yew is **web-only** by deliberate scope. Dioxus's multi-target ambition is the major differentiator; for pure-web projects, the choice between Dioxus and Yew is mostly preference (`rsx!` vs `html!`, signals vs hooks).

### vs Leptos (Rust web framework, signals-first)

**Leptos** (leptos-rs, 2022–) is the closest Dioxus competitor architecturally. Signals-first from day one (no VDOM), fine-grained reactivity à la Solid.

| | Dioxus | Leptos |
|---|---|---|
| Reactivity | Signals + VDOM-diff | Signals + direct DOM mutation (no VDOM) |
| Update granularity | Component-level rerender | DOM-node-level rerender |
| Bundle size | Comparable per-feature | Often smaller (no VDOM machinery) |
| SSR / fullstack | Yes (fullstack with Axum) | Yes (cargo-leptos) |
| Targets | Web/Desktop/Mobile/Native | Web (primarily) |
| `Copy`-signal arena | `generational-box` | `RwSignal` (Leptos's own arena) |
| Authoring DSL | `rsx!` | `view!` |
| Maturity | Both production for web |

The **no-VDOM-just-signals** approach is Leptos's distinguishing bet; **VDOM-plus-signals** is Dioxus's. Performance benchmarks (per the LogRocket / Reintech surveys) put them in the same ballpark on the JS Frameworks Benchmark, both faster than Yew, both competitive with native JS frameworks. The substantive difference is **mental model**, not performance.

For Buiy, the Leptos comparison matters more than the Dioxus one in one specific axis: Leptos is the cleaner example of "signals all the way down, no VDOM," which is *closer* to a hypothetical Bevy-ECS-signal-layer than Dioxus's VDOM-on-top-of-signals shape.

### vs Sycamore (older Rust signals framework)

**Sycamore** (2020–) is the original signals-first Rust UI framework, predating both Dioxus and Leptos. Signals via `Rc<RefCell<T>>` (not arena-based, not `Copy`). Web-only.

Sycamore was influential — Solid's model came to Rust through Sycamore first — but the `Rc/RefCell` signal shape is ergonomically inferior to the Copy-arena approach (Dioxus's `Signal<T>` and Leptos's `RwSignal<T>`). Adoption stalled. Worth reading as evidence that **the Copy-arena form is what made signals usable in Rust** — the conceptual model alone is not enough.

### vs egui / bevy_egui (immediate-mode)

**egui** (Emil Ernerfeldt, 2020–) is the dominant Rust immediate-mode UI library, used pervasively in dev tooling, inspectors, debug overlays. `bevy_egui` is the Bevy wrapper ([cross-reference `prior-art/bevy-egui/`](../bevy-egui/)).

The paradigm gap is total. egui has **no component model, no VDOM, no state-across-frames; the entire UI is re-emitted every frame**. Dioxus has a VDOM, scopes, hooks, signals — all of which exist to preserve state across frames. These are different architectures suited to different workloads:

- **egui wins** for dev tooling, in-game debug overlays, inspectors, anything where the UI is mostly stateless and the dev wants to write a function and have it run.
- **Dioxus wins** for application UI with non-trivial state (forms, lists with selections, multi-step wizards, anything React-shaped).

For Buiy's purposes, both egui and Dioxus are **paradigm contrasts**. Buiy is retained-mode + ECS-shape — neither matches Dioxus's VDOM nor egui's per-frame redraw. See [`prior-art/bevy-egui/lessons.md`](../bevy-egui/lessons.md) for the immediate-mode contrast; this folder is the React-shape contrast.

### vs Iced (Rust ELM-shape)

**Iced** (Héctor Ramón, 2019–) is an Elm-architecture Rust UI library: `Message -> Update -> View` with explicit state and no hooks. Cross-platform (native + WASM); uses wgpu directly. ~26K stars.

Compared to Dioxus, Iced is:
- **Explicit-state** (no hooks; state lives in your `Application` struct)
- **No VDOM** (each view is a tree of widgets re-built each update; closer to immediate-shape but with retained state externally)
- **No signals** (manual subscriptions via the Elm-architecture message bus)
- **Native rendering** (wgpu via tiny-skia and lyon; no DOM)

Iced is the closest pre-existing "native rendering Rust UI library" reference for Buiy in terms of *not* using webview/DOM. Its layout uses an in-house solver (not Taffy — [cross-reference `prior-art/taffy/lessons.md` § "Avoid" — README inaccuracy](../taffy/lessons.md)). Its authoring DSL is plain Rust function calls, not a macro DSL.

For Buiy, Iced is a useful neighbor — same target stack (native + wgpu), different reactivity paradigm.

### vs React (JS reference point)

Dioxus's design is more React-shaped than any other reference here — it's the cleanest "React in Rust" available. The differences from React:
- Components are functions returning `Element` (React: functions returning `JSX.Element` or `ReactNode`).
- State is `Signal<T>` (subscribe-in-render-path) rather than `useState` (re-render whole tree).
- VDOM diffing is similar; Dioxus has a custom "block-diff" optimization that batches mutations more aggressively.
- Hot-reload (Subsecond) is more invasive than Fast Refresh.

If your team's mental model is React's, Dioxus is the smallest cognitive jump in Rust.

### vs Solid.js (JS reference point)

Dioxus's signals are explicitly Solid-derived. The differences:
- Dioxus keeps a VDOM; Solid is no-VDOM (compiler emits direct DOM mutations).
- Dioxus's component model is React-shape (functions returning Element); Solid's is closer (but with subtle differences around how often the function body runs — Solid runs once per scope, Dioxus runs every render).
- Both use Copy-shape signals (Solid's signals are JS values; Dioxus's are arena-Copy).

Solid is the **conceptual ancestor of Dioxus's reactivity model**; reading Ryan Carniato's writeups is the right way to understand the model before reading Dioxus source.

### vs Buiy (Bevy game-engine UI context)

| | Dioxus | Buiy |
|---|---|---|
| Substrate | Own runtime (VDOM + scheduler + renderer per target) | Bevy ECS + render graph + scheduler |
| Authoring | `rsx!` macro | BSN (planned) + ECS spawn |
| State | Signals + Stores | ECS components + `Changed<T>` (v1); signals out of scope for v1 |
| Targets | Web/Desktop/Mobile/Native (own renderers) | Bevy targets (inherits Bevy's WASM/desktop/mobile) |
| Layout | Taffy (via Blitz on Native target only) | Taffy (directly, on every target) |
| Text | Browser text shaping (web/webview); Parley (Blitz) | cosmic-text (foundation commit) |
| Accessibility | DOM/AT (webview); none yet on Blitz | AccessKit-first across all Bevy targets |
| Hot-reload | Subsecond (binary-patching) | BSN-asset reload + component reload (planned) |
| Renderer | per-target | single (Buiy-owned, on Bevy's render graph) |
| Use case | Cross-platform app framework | Bevy app & game UI |

**Buiy and Dioxus are not competitors.** They target different substrates (Bevy ECS vs own runtime), different deployment matrices (Bevy app vs cross-platform-bring-your-own-runtime), and different paradigms (ECS-retained-mode vs VDOM-React-shape). A Bevy app may use Dioxus *via Tauri-style embedding* in a separate window — but that's a coexistence, not a substitution.

The relevant lessons from Dioxus are **DSL shape** (`rsx!`-style authoring informs BSN), **reactivity model** (signals + Stores is the eventual shape if Buiy adds reactivity), and **multi-target taxation** (don't do it — see [`open-problems.md`](open-problems.md)).

## Implications for Buiy

- **Leptos is closer to a Buiy-signal-layer's ideal shape than Dioxus.** No-VDOM, signals-all-the-way-down, direct mutation. If Buiy ever adds signals, the Leptos source is more relevant reading than Dioxus's. See [`signals-and-state.md`](signals-and-state.md).
- **Dioxus is the canonical "React in Rust" comparison point** for Buiy decisions. When a designer asks "should Buiy adopt VDOM diffing?", the answer is grounded in why Bevy's ECS substrate makes that the wrong primitive — not in disagreeing with Dioxus's choice for its own substrate.
- **Iced is the closest "native Rust UI" neighbor** in terms of substrate (wgpu + non-webview). Worth a folder of its own at some point; the current `prior-art/iced/` directory is empty.
- **No flagship consumer-facing Dioxus app exists.** The same critique that [`prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) applies to Bevy UI ("no flagship game") applies in milder form to Dioxus: enterprise / dev-tool deployment is real, mass-consumer deployment is not yet demonstrated. Buiy should expect the same characterization in its early years and not over-promise "production-ready for consumer apps" prematurely.

## Sources

- crates.io API for `dioxus`: https://crates.io/api/v1/crates/dioxus
- dioxuslabs.com (trust badges crawl 2026-05-22)
- Y Combinator company page: https://www.ycombinator.com/companies/dioxus-labs
- Yew: https://yew.rs/
- Leptos: https://leptos.dev/
- Leptos vs Yew vs Dioxus comparison (Reintech 2026): https://reintech.io/blog/leptos-vs-yew-vs-dioxus-rust-frontend-framework-comparison-2026
- Sycamore (historical reference): https://sycamore-rs.netlify.app/
- Iced: https://iced.rs/
- egui: https://www.egui.rs/
- Solid.js (reactivity model upstream): https://www.solidjs.com/
- Awesome-Dioxus user list: https://github.com/DioxusLabs/awesome-dioxus
- Sibling: [`../bevy-egui/`](../bevy-egui/), [`../taffy/`](../taffy/)
