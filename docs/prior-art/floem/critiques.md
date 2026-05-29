**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — third-party critiques, structural problems, open issues

This file combines critiques with open-problems-not-yet-solved per the compressed-folder template.

## Structural critiques

### 1. Release cadence

**The critique:** Three published versions in ~4 years; 17+ months since 0.2.0. While `main` is alive, crates.io users are stranded on an increasingly-stale snapshot.

**The evidence:** crates.io version list shows 0.1.0, 0.1.1, 0.2.0 (2024-11-15). Direct commit log shows continuous activity through 2026-05.

**The mitigation Floem hasn't done:** The Lapce team has not chosen to cut `0.3.0` despite ~18 months of work accumulated on `main`. There's no public commitment to a 0.3.0 timeline.

**For Buiy:** Floem demonstrates that **a UI library cannot serve users beyond its dogfood-flagship without release discipline.** Buiy must define release cadence policy explicitly (foundation `architecture.md` §2.9 commits to "rolling latest-stable Bevy"; Buiy's *own* release cadence is implicit). Owner: a future `buiy-release-policy-design` sub-spec or a section in the verification design.

### 2. Single-flagship dogfooding

**The critique:** Floem is dogfooded by Lapce. That's it. The "Floem outside Lapce" experience is unverified at scale.

**The evidence:** [`ecosystem.md`](ecosystem.md) — production-app list is Lapce + Lapdev (also Lapce-team). Third-party widget crates: one (`floem-ui-kit`). Documentation: thin beyond the 27 examples.

**Why it matters:** Single-flagship dogfooding catches the bugs and missing features the flagship hits, but misses entire surface areas the flagship doesn't exercise. Examples Floem-not-tested-by-Lapce:

- Form-heavy data-entry UI (Lapce doesn't have forms; it has editor + tree views).
- Multi-window app patterns beyond editor/inspector.
- Touch / pen input (Lapce is desktop-only).
- Right-to-left and complex-script content (Lapce is code, mostly ASCII).
- Screen-reader interaction (Lapce doesn't ship a11y).

**For Buiy:** Buiy's "game and app, both" foundation goal #6 implies multiple flagship surfaces. Foundation goal #2 (WCAG 2.2 AA) implies AT-using flagship users. **At least one early Buiy adopter must require accessibility, or the foundation goal drifts toward Floem's reality.**

### 3. The AccessKit gap

**The critique:** Issue #8 ("Support Accessibility via AccessKit") has been open since **2023-04-14** with no progress in 3+ years. Floem ships without accessibility infrastructure.

**The evidence:** [`accessibility.md`](accessibility.md) — direct verification of Cargo.toml, docs.rs modules, README.

**Why it matters:** Adding accessibility as an afterthought is much more expensive than designing for it from day one. Floem's component trait (`View`) does not have an accessibility-node method; widgets don't carry roles; there's no ACCNAME path. Retrofitting this would be a major refactor.

**For Buiy:** This is the single most important *negative* lesson. Foundation `accessibility.md` and `architecture.md` §2.6 commit Buiy to AccessKit-first. Stay the course — Floem is the warning.

### 4. Custom winit fork

**The critique:** Floem holds `lapce/winit` as a git fork. The Cargo.toml line:

```toml
winit = { git = "https://github.com/lapce/winit", rev = "133268de...", package = "floem-winit" }
```

**Why it matters:**

- Security fixes in upstream winit must be backported manually.
- External crates that depend on winit cannot share the event loop without coordination.
- The fork drifts; absorbing upstream is increasingly expensive.
- Crates.io has `floem-winit` as a parallel name; ecosystem fragmentation.

**The Lapce-team rationale (inferred):** winit-upstream releases on its own cadence; Lapce/Floem need fixes faster than upstream releases them. This is true but the cost compounds.

**For Buiy:** Buiy depends on winit through Bevy. Bevy owns the winit relationship. **Do not** fork winit at the Buiy layer; if Buiy needs winit changes, contribute upstream or push through Bevy. Floem's fork is a structural debt Buiy must avoid.

### 5. Documentation thinness

**The critique:** docs.rs API docs + 27 examples + README is the totality of official documentation. No book, no long-form guide, no migration docs.

**The evidence:** [`ecosystem.md`](ecosystem.md) — direct enumeration.

**Why it matters:** A UI library with a non-trivial reactive runtime, four renderer backends, and a custom view trait *needs* a book. Solid.js has one. Leptos has one. Dioxus has one. Floem does not.

**For Buiy:** Documentation is not a separable concern; it's part of "the foundation." Buiy must commit to documentation tier in the foundation, not after. Owner: foundation `verification.md` or a follow-up sub-spec.

### 6. Multi-renderer surface

**The critique:** vger / vello / skia / tiny-skia is **four** rendering backends. Each must be tested, kept in sync, and supported.

**The mitigation Floem partially does:** The renderers are behind Cargo features, so users pick one. But the API surface that abstracts over them is shared; bugs in one renderer can leak into others; CI must cover all paths.

**Why it matters for Floem:** With no full-time devs, four renderers is a maintenance burden the project cannot sustain at quality.

**For Buiy:** **Don't multi-back-end render.** Buiy has exactly one renderer (Bevy's render graph through wgpu). Foundation `architecture.md` §2.2 commits to this. Floem's multi-renderer experiment is a cautionary tale about scope creep.

### 7. Reactivity-runtime ergonomics

**The critique:** Rust's `'static` closure bounds, generic `T: Copy` constraints on signals, and the `with(|t| ...)` callback pattern for non-`Copy` reads add ergonomic tax that doesn't exist in Solid.js.

**The evidence:** Routine Floem code peppered with `move` keywords, `.clone()` calls on `Arc` references, and `signal.with(|t| ...)` for `String` / `Vec` reads.

**Why it matters:** A signal API that's ergonomic in JS may not be ergonomic in Rust. Floem accepts the tax; users adapt.

**For Buiy:** If §2.7 is reopened, expect this tax. The mitigation is *not* macros (Floem chose direct API over macros; the surface is cleaner). Mitigation is **shrink the API to what Bevy users actually need.** A small signal surface (Get/Set/Subscribe + ECS interop) is more likely to land than a full Solid.js port.

## Open problems (not solved upstream)

### Mobile targets

iOS, Android — no support, no roadmap. For Buiy this is informational; Bevy handles mobile at the engine layer, and Buiy will inherit.

### WASM target

Experimental in 0.2.0; production-readiness unknown. For Buiy, WASM is an open question (foundation `README.md` §5).

### COLRv1 emoji

Inherited from Swash. Same gap as cosmic-text.

### Vertical writing modes

Inherited from Parley. Same gap as cosmic-text.

### Screen-reader testing

Not applicable to Floem (no a11y). For Buiy, see foundation `verification.md` manual release gates.

### Stable 1.0 timeline

The README says "we will make occasional breaking changes and add missing features on our way to v1." No timeline.

### Ecosystem cultivation

How does Floem grow beyond Lapce? No published strategy. For Buiy, this is the **most important open lesson**: a library cannot grow an ecosystem without explicit ecosystem cultivation, and ecosystem cultivation needs staffing the Lapce team doesn't have.

## Sources

- All sibling files in this folder.
- Floem issue #8 — https://github.com/lapce/floem/issues/8
- Floem Cargo.toml — https://github.com/lapce/floem/blob/main/Cargo.toml
- Floem releases page — https://github.com/lapce/floem/releases
- Cross-link: [`open-problems.md`](open-problems.md) (pointer file).
