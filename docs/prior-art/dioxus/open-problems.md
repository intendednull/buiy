**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — critiques and open problems: multi-target fragmentation, Blitz pre-alpha, a11y, bundle size, hot-reload reliability, SSR maturity, animation primitives, reactivity edge cases

# Critiques and open problems

Dioxus is the most-iterated React-shape Rust UI framework. Its strengths are real (signals, multi-target deployment, Subsecond hot-patching, fullstack story); its weaknesses are also real and structurally embedded. This file documents both.

## Critiques

### 1. Multi-target fragmentation cost

Dioxus's marketing slogan is *"one codebase, every platform."* The reality after five years is that the targets have very different maturity:

- **Web (DOM):** production.
- **Desktop (Webview), Mobile (Webview):** production but inherits webview pain.
- **Desktop (WGPU/Blitz):** pre-alpha by author admission.
- **Mobile (Native/Blitz):** experimental.
- **SSR / Fullstack:** production for the web target.

Same `rsx!` code does not produce identical output across targets. Examples (from upstream issues + release notes):
- `<canvas>`, `<svg>` filters, complex form-control native styling work in web/webview, partial-or-missing in Blitz.
- Touch event handling diverges between mobile webview and mobile native.
- CSS animations work in web/webview; Blitz's animation support is limited.
- Accessibility — DOM/webview gets AT-for-free; Blitz has no AT integration as of 0.7.9.
- Bundle size — Blitz binaries embed an entire CSS engine + renderer; significantly larger than webview equivalents.

The cost is structural: each target's renderer is a separate maintenance line. Each has its own bug list. The framework's authors have spent **over a year** on Blitz alone before declaring it pre-alpha-not-production-ready. This is the **multi-target tax**, and it is paid in maintenance burden per target — not absorbed by writing the framework once.

### 2. Blitz is pre-alpha — and that's the flagship feature of 0.7

The headline of Dioxus 0.7 (October 2025) was **Dioxus Native + Blitz** — the WGPU-rendered, no-webview, native-app path. The Blitz README explicitly states it is "pre-alpha" and "we do not recommend building production apps with it." This means the most-marketed feature of the year-defining 0.7 release is **not production-ready by its own authors' admission**.

The architectural bet is plausible — Stylo (Mozilla's CSS engine) + Taffy + Parley + Vello are all serious components. But the integration is years from finished, and the maintenance scope (a real HTML/CSS engine!) is heavy.

### 3. Accessibility gap on native targets

The DOM-target story is good — semantic HTML elements (`button`, `nav`, `dialog`, `input`) get AT integration from the browser. The webview targets inherit the same.

The native (Blitz) targets have **no AT integration as of 0.7.9**. AccessKit integration is roadmapped but not shipped. For a UI library shipping a native-rendering path, this is a serious gap — see [Buiy foundation accessibility § AccessKit](../../specs/2026-05-07-buiy-foundation/accessibility.md) for the bar Buiy holds itself to.

Mobile targets (webview-based) have partial a11y via the platform webview's AT bridge, which is uneven (iOS WKWebView better than Android System WebView than Linux WebKitGTK).

### 4. Bundle size and startup time on web

Dioxus 0.7's release notes advertise "web apps under 50kb" for minimal apps. Real-world Dioxus apps (with router + signals + a component library + a few signal-using pages) tend to land in the 300-800 KB compressed range, comparable to React + Redux + Router. WASM startup time on slow networks is real; the framework does not solve this and arguably cannot — WASM startup is a substrate property.

### 5. Hot-reload reliability

Subsecond is novel and ambitious. It is also fragile in practice — community feedback indicates that hot-patching breaks on certain code patterns (lifetimes-heavy generics, certain async closures, type-state-machine patterns). The official guidance is to handle these via Subsecond's explicit `subsecond::call()` integration points and "framework cleanup" sync points, which is more invasive than React's Fast Refresh / Svelte HMR.

The 0.7 patch series (0.7.1 — 0.7.9) has shipped a steady stream of Subsecond bug fixes, suggesting that production users are hitting reliability edge cases regularly.

### 6. SSR / fullstack story is web-target-only

`dioxus-ssr` renders to a string for the web target. There is no SSR-shape for native/desktop/mobile (which makes conceptual sense — there's no server-rendering of a webview-app). This means "the fullstack story" is really "the web-target's fullstack story"; desktop/mobile/native apps don't benefit from suspense, streaming, or server functions in the same way.

### 7. Animation primitives are basic

Dioxus does not own a first-class animation system. Animation on the web target is via CSS animations (browser-rendered). On webview targets, same. On Blitz/Native, animation is limited — CSS animation support in Blitz is incomplete. There is no equivalent to React Spring, Framer Motion, or Svelte's `tweened`/`spring` primitives.

For game-engine UI (Buiy's domain), animation primitives are foundational ([foundation interaction § Animation](../../specs/2026-05-07-buiy-foundation/interaction.md)). Dioxus's gap here is informative — even on a non-game-engine substrate, mature animation requires significant per-platform work.

### 8. Reactivity diamond dependencies

Signals have a known weakness in topological-order updates. If A depends on B and C, and both B and C depend on D, then writing D may cause A to recompute twice (once via B's update, once via C's). Solid and Leptos handle this via topological batching; Dioxus 0.7 has improved but not fully solved it. The practical impact is rare in typical app code but appears in complex reactive graphs (e.g., dashboards with many derived stores).

### 9. Stylo embedding is heavyweight

Blitz embeds Stylo (Mozilla's CSS engine) for full CSS-cascade support. Stylo is a real Firefox component — measured in megabytes, not kilobytes. The resulting Blitz binary size is significantly larger than what a "Rust UI library" suggests. This is the cost of CSS-spec compliance; the alternative (a Buiy-shape token system without full cascade) is what Buiy chose ([foundation cross-cutting § Theming](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).

### 10. Informal design process

Dioxus has no public RFC repo. Major design decisions emerge in GitHub discussions / Discord / per-release blog posts. The Blitz architecture was discussed across multiple issues and Discord channels before culminating in 0.7's release post; reading the upstream design rationale from outside the project requires significant archaeology. See [`governance.md`](governance.md) § "RFC / design process."

## Open problems

### Native-target maturity

Blitz needs years more work to reach production. Specific gaps known from upstream issues:
- CSS coverage incomplete (many properties unimplemented or buggy).
- AccessKit integration not shipped.
- Animation incomplete.
- Form control native-styling incomplete.
- `<canvas>`/`<svg>` partial.
- `<video>` not supported.
- Text input / IME behavior less mature than browser-text.

### Mobile UX parity

Mobile-webview ergonomics (CLI, debugging, IPC) improved markedly in 0.6 but remain less polished than the web/desktop story. Mobile-native (Blitz) is experimental, inherits all Blitz gaps, and adds mobile-GPU constraints.

### Hot-reload completeness

Subsecond is novel and works in many cases. The completeness story — "any Rust code change hot-reloads, no exceptions" — is not yet achievable; framework-cleanup-point invariants and certain code patterns require user awareness. Long-term it's unclear whether Subsecond can ever be fully transparent.

### Server-side rendering completeness

SSR is web-target-only. Streaming HTML works; suspense works. Universal rendering (server-side for non-web targets) is not a goal.

### Animation primitives

A first-class spring / tween / declarative animation API is missing. Whether Dioxus ever ships one (vs. delegating to user libraries or CSS) is unclear from public roadmap.

### Drag-and-drop primitives

No first-class DnD API. Web target uses HTML5 DnD; native targets have no DnD primitive.

### File system access

`@web/wasi-filesystem` exists in some experimental paths; not first-class.

### `Send` requirements for fullstack

Server functions impose `Send` bounds on returned futures; this conflicts with some non-`Send` patterns common in async-Rust ecosystems. Workarounds exist but are awkward.

### Reactivity scheduler aligned with Bevy ECS

(Buiy-specific.) Dioxus's signal scheduler is single-threaded and topologically ordered. Bevy's system scheduler is parallel and access-pattern ordered. A hypothetical Buiy signal layer would need to bridge these. The shape of that bridge is not obvious from any existing reference. See [`signals-and-state.md`](signals-and-state.md).

## Implications for Buiy

- **Multi-target ambition is the single most expensive choice Dioxus made.** Buiy's commitment to **single-substrate (Bevy)** and **inheriting Bevy's target story** is validated by every Dioxus target-maturity story. Foundation [non-goal § 1.3 — "non-Bevy frontends"](../../specs/2026-05-07-buiy-foundation/README.md) is correct.
- **A "flagship 0.7 feature is pre-alpha by its authors' own admission" is a real outcome to avoid.** When Buiy 0.x ships a flagship-level feature, the feature should not be pre-alpha by Buiy's own authors' admission. Foundation [verification spec](../../specs/2026-05-07-buiy-foundation/verification.md) enforces this via CI gates + manual release gates per Buiy version.
- **AccessKit-first across all targets** is the right policy. Dioxus's webview-AT-for-free shortcut doesn't apply to Bevy targets; Buiy commits to AccessKit explicitly. Foundation [accessibility § AccessKit](../../specs/2026-05-07-buiy-foundation/accessibility.md).
- **Animation primitives are not "free."** Even a mature framework like Dioxus has gaps. Buiy's foundation [animation in interaction.md](../../specs/2026-05-07-buiy-foundation/interaction.md) and the `buiy-animation-design` sub-spec ([foundation sub-spec roadmap](../../specs/2026-05-07-buiy-foundation/README.md)) need to commit to actual primitives, not "we'll integrate with bevy_animation."
- **Hot-reload completeness is a years-long arc.** Subsecond shipped in 0.7, hits real reliability edge cases, and DioxusLabs is patching them release-by-release. Buiy's BSN-hot-reload story should be designed for an explicit "hot-reload coverage matrix" (which constructs are hot-reloadable, which require restart) rather than aspirational "everything reloads."
- **Don't trust marketing slogans about scope.** "One codebase, every platform" turns out to mean "one codebase, every platform with significantly different quality per target." Buiy's tier-F/C/E/O foundation language ([foundation README § "Tier legend"](../../specs/2026-05-07-buiy-foundation/README.md)) is the correct corrective — make scope visible per feature, not behind generic terms.

## Sources

- Dioxus 0.7 release notes (Blitz pre-alpha, Subsecond intro, Stores): https://dioxuslabs.com/blog/release-070
- Blitz repo README (pre-alpha status): https://github.com/DioxusLabs/blitz
- Dioxus patch-release cadence (0.7.1 → 0.7.9 over 7 months): https://crates.io/crates/dioxus
- Dioxus GitHub issues (Subsecond reliability reports): https://github.com/DioxusLabs/dioxus/issues
- Reintech Rust framework comparison 2026 (bundle-size context): https://reintech.io/blog/leptos-vs-yew-vs-dioxus-rust-frontend-framework-comparison-2026
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/)
- Sibling: [`targets.md`](targets.md), [`signals-and-state.md`](signals-and-state.md)
