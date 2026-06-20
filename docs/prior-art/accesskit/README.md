**Date:** 2026-06-18
**Status:** active
**Subject:** AccessKit — cross-platform Rust accessibility infrastructure; the load-bearing bridge Buiy uses to reach NVDA / VoiceOver / Orca / TalkBack

# AccessKit

AccessKit is a cross-platform, cross-language abstraction over the platform accessibility APIs (UIA on Windows, NSAccessibility on macOS, AT-SPI on Linux, Android `AccessibilityNodeInfo`, iOS UIAccessibility). A producer (a UI toolkit) builds a single `Tree` of `Node`s using AccessKit's Rust schema and pushes `TreeUpdate` diffs through a per-window `accesskit_winit::Adapter`; the platform adapters translate that tree into the local OS accessibility vocabulary so screen readers and other assistive technologies read it. The schema is "based largely on Chromium's cross-platform accessibility abstraction" (the README's own phrasing) and the Rust shape is canonical — C and Python bindings consume the same types.

Buiy treats AccessKit as a load-bearing dependency: Buiy is **AccessKit-first**. Buiy builds and pushes `TreeUpdate`s directly with its own decomposed components (`A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, `A11yRelations`), owns the `accesskit_winit::Adapter` per window, computes ACCNAME 1.2 in `buiy_core`, and intentionally replaces `bevy_a11y` for windows where Buiy is present (see [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md), and [`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)). The same tree is also Buiy's most promising **agent-control + automation surface** — the read-and-act-by-reference seam an LLM agent or test driver would drive Buiy through (see [`agent-control.md`](agent-control.md)). This folder is the version-pinned reference future Buiy spec authors should consult when designing against AccessKit.

## Honest assessment

**Strong points.**

- **Production on Windows, macOS, Linux.** `accesskit_windows` has shipped since December 2021; `accesskit_macos` since November 2022; `accesskit_unix` since January 2023. NVDA, JAWS, Narrator, VoiceOver, and Orca consume AccessKit-driven trees in deployed egui, Slint, Bevy, and Freya applications today.
- **One schema, many platforms.** The producer-side `Node` shape is invariant; per-platform divergences live inside the adapter crates. This is the cross-platform pitch and it holds for the desktop trio.
- **Lazy activation gate.** `update_if_active` only runs the closure that builds the `TreeUpdate` if an AT is actually listening — idle windows pay nothing. The gate makes per-frame tree rebuild affordable for retained-mode and immediate-mode toolkits alike.
- **Producer-friendly API.** `Node::new(Role)` + setters; no builder pattern; ~200 accessor methods on `Node` cover the full ARIA-1.2 surface that maps onto AccessKit's `Role` / `Action` / state / relation vocabulary.
- **WAI-ARIA-aligned without being one-to-one.** AccessKit unifies `aria-checked` and `aria-pressed` into a single `Toggled` enum, splits ARIA `combobox` into `ComboBox` / `EditableComboBox`, and frequency-orders `Role` for serialization. The deliberate divergences are documented and load-bearing.

**Pre-1.0 cadence churn.** AccessKit ships on its own irregular cadence (six-month gap mid-2025, three releases in three weeks in early 2026, ten co-released crates on 2026-05-11). Minor bumps regularly carry breaking changes — the `accesskit` crate is at 0.24.0 with no public 1.0 timeline. Buiy's "AccessKit major release between Bevy minors triggers a Buiy patch release" policy ([`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md)) needs to read "minor or major" given the pre-1.0 versioning.

**iOS adapter is 11 days old.** `accesskit_ios` 0.1.0 shipped 2026-05-11 with release notes "Basic iOS adapter" and ~229 lifetime downloads at folder-writing time. The Buiy foundation spec calls iOS "in-progress upstream" — that wording is now slightly outdated but the **deferred-CI-gate posture is still right**. v0.1.0 is alpha; coverage of UIAccessibility protocols is incomplete; no production app is reported as shipping on it.

**Android adapter is shipping but small.** `accesskit_android` 0.7.3 (~26k lifetime downloads, vs macOS 7.7M and Windows 7.8M) is more mature than iOS but still pre-1.0 and under the README's caveat that adapters "don't yet support all types of UI elements."

**Web adapter is NOT shipped.** No `accesskit_web` crate on crates.io, no actively-WIP PR in recent issue search. The web case is architecturally different from desktop/mobile (DOM-aligned ARIA, not a parallel tree) and the work has not visibly started. Buiy's Bevy-on-WASM target is partial accessibility-wise until upstream ships.

**Stewardship is small and informal.** AccessKit is heavily Matt-Campbell-centric with Arnold Loubriat as the main co-maintainer on Linux/AT-SPI. Pneuma Solutions (Campbell's company) is the implicit but not contractual sponsor; there is no foundation, no Series-A, no Sovereign-Tech-Fund grant tagged to AccessKit by name (NLnet NGI0 funded the AT-SPI and iOS adapters, but not the core). Adopting AccessKit as load-bearing means inheriting this bus-factor concentration.

**Known AT-side bugs.** `accesskit_macos` issue #520 — ListBox selected state is not properly communicated to AT. AT-SPI's `aria-activedescendant` semantics are fuzzy on Orca. Wayland clients can't query absolute window position (winit returns `Err`), so AT-SPI bounds may be wrong on Wayland sessions. These are real, scoped, and documented in [`platform-adapters.md`](platform-adapters.md) and [`critiques.md`](critiques.md).

**Matt Campbell is NOT a former NVDA developer.** The brief that seeded earlier writeups got this wrong. NVDA was created by Michael Curran in 2006, co-led by James Teh — separate lineage. Campbell's screen-reader résumé is Serotek Corporation's "System Access to Go" + the Microsoft Windows Accessibility Team, then Pneuma Solutions (co-founded with Mike Calvo in 2020) where AccessKit started in 2021. The corpus consistently uses the Pneuma about page's biography.

## Key facts

| Fact | Value | Source |
|---|---|---|
| Core crate | `accesskit` 0.24.0 (2026-02-01) | [crates.io](https://crates.io/crates/accesskit) |
| `accesskit_winit` | 0.33.0 (2026-05-11) | [crates.io](https://crates.io/crates/accesskit_winit) |
| `accesskit_windows` | 0.33.0 (2026-05-11) — production, since 2021-12-21 | [crates.io](https://crates.io/crates/accesskit_windows) |
| `accesskit_macos` | 0.26.1 — production, since 2022-11-23 | [crates.io](https://crates.io/crates/accesskit_macos) |
| `accesskit_unix` | 0.21.1 — production, since 2023-01-05; AT-SPI / D-Bus | [crates.io](https://crates.io/crates/accesskit_unix) |
| `accesskit_android` | 0.7.3 (2026-05-11) — pre-1.0, shipping; first published 2025-03-06; ~26k downloads | [crates.io](https://crates.io/crates/accesskit_android) |
| `accesskit_ios` | **0.1.0 shipped 2026-05-11** — basic, alpha; ~229 lifetime downloads | [GitHub releases](https://github.com/AccessKit/accesskit/releases) |
| Web adapter | **Not shipped.** No crate, no visible active WIP PR. | — |
| `accesskit_consumer` | 0.36.0 (2026-05-11) — **adapter-side** library; not for application integration | [consumer/README](https://github.com/AccessKit/accesskit/blob/main/consumer/README.md) |
| Repo | https://github.com/AccessKit/accesskit | — |
| License | MIT OR Apache-2.0 (Chromium-derived portions carry a BSD-style license) | repo LICENSE files |
| MSRV / edition | rust-version 1.85, edition 2024 | [workspace Cargo.toml](https://raw.githubusercontent.com/AccessKit/accesskit/main/Cargo.toml) |
| `accesskit` total downloads | 17,933,800 lifetime; 4,081,803 recent (90d) | crates.io |
| Founder | **Matt Campbell** — co-founder + CTO of Pneuma Solutions (founded 2020 with Mike Calvo); previously Serotek + Microsoft Windows Accessibility Team; **NOT** ex-NVDA | [Pneuma about](https://pneumasolutions.com/about/) |
| iOS adapter author | Arnold Loubriat, funded by NLnet NGI0 Commons Fund | platforms/ios README |
| Other copyright holders | Arnold Loubriat, Google LLC (Chromium-derived schema code), Leonard de Ruijter | [AUTHORS](https://raw.githubusercontent.com/AccessKit/accesskit/main/AUTHORS) |
| Schema lineage | Chromium's `ui::AXNode` cross-platform accessibility abstraction | upstream README |
| Production adopters (verified) | egui (since 2022-12-04, enabled by default in eframe), Slint (since v0.2.5 2022-07-06; AccessKit explicit at v1.7.0), Bevy (via `bevy_a11y` since 0.10.0 2023-03-01), Freya (accesskit 0.24 + accesskit_winit 0.32), Xilem/Masonry (Linebender) | [ecosystem.md](ecosystem.md), [history.md](history.md) |
| Not adopters (verified false) | Iced (drafts only — PR #3111 open since 2025-11-11; 0.14.0 shipped without AccessKit), Tauri (uses WebView a11y), GPUI/Zed (no integration visible), Druid (discontinued, never integrated) | [ecosystem.md](ecosystem.md) |
| `Role` enum | 182 `#[repr(u8)]` variants, frequency-ordered for serialization | [Role docs](https://docs.rs/accesskit/0.24.0/accesskit/enum.Role.html) |
| `Action` enum | 22 variants (`Click`, `Focus`, `SetValue`, `ScrollIntoView`, `CustomAction`, …); **identical set + order in the in-tree 0.21 pin and the 0.24 BSN-bump target** | [Action docs](https://docs.rs/accesskit/0.24.0/accesskit/enum.Action.html), [agent-control.md § 6](agent-control.md) |
| `Toggled` enum | `False`, `True`, `Mixed` (unifies `aria-checked` + `aria-pressed`) | [tree-model.md](tree-model.md) |
| `Invalid` enum | `False`, `True`, `Grammar`, `Spelling` | [tree-model.md](tree-model.md) |
| ARIA field spelling | `labelled_by` (British double-l), not `labeled_by` | [tree-model.md](tree-model.md) |
| Focus model | `TreeUpdate.focus: NodeId` — one focused node per tree per update; **not** a node state flag | [api.md](api.md) |
| Tri-state fields | `is_selected -> Option<bool>`, `is_expanded -> Option<bool>` (None = not applicable) | [tree-model.md](tree-model.md) |
| One adapter per window | Structural — `accesskit_winit::Adapter` owns the per-window a11y bridge | [platform-adapters.md](platform-adapters.md) |
| Coordinate space | `Node::set_bounds` is window-relative logical pixels; the adapter applies window position + DPI scale to produce screen coords for the AT | [tree-model.md](tree-model.md) |
| Threading | Producer calls `update_if_active` from the main / event-loop thread; AccessKit data types are `Send + Sync` | [api.md](api.md) |
| Release cadence | Irregular; co-release pattern (all adapter crates publish on the same day) | [governance.md](governance.md) |
| Funding (named) | NLnet NGI0 Commons Fund (AT-SPI + iOS adapters), Pneuma Solutions in-kind, GitHub Sponsors (`github.com/sponsors/mwcampbell`) | NLnet, [governance.md](governance.md) |

## Producer / Consumer terminology — read this first

AccessKit's word choices invert what most readers expect. Get this straight before reading any other file:

- **Producer** = the UI toolkit that creates `Tree` + `Node` data and calls `adapter.update(...)`. **Buiy is the producer.** So are egui, Slint, Freya, Xilem/Masonry, and Bevy's `bevy_a11y`. The producer is the *source of truth* — it's where accessibility information originates.
- **Consumer** = the **adapter-side** code that reads the producer's tree, walks/diffs/caches it, and pushes it through the local OS accessibility API to the assistive technology. The crate is literally named `accesskit_consumer`. It is **not for direct application integration**; it sits inside the adapter crates (`accesskit_windows`, `accesskit_macos`, `accesskit_unix`, `accesskit_android`, `accesskit_ios`).
- The platform AT (NVDA, JAWS, Narrator, VoiceOver, Orca, TalkBack) is the *end consumer*, reached only via the OS accessibility API — not via `accesskit_consumer` directly.

If you find yourself thinking "Buiy is the consumer because Buiy consumes the AccessKit crate" — that is wrong vocabulary. Buiy is the *producer*. The AT is the consumer of accessibility *information*. `accesskit_consumer` consumes the *tree*.

## Contents

Each file is independently skimmable. Sources are listed per file.

**Technical subsystems**

- [**architecture.md**](architecture.md) — The producer/adapter split, the push-based `TreeUpdate` protocol (full initial + diffs thereafter), the activation gate, the handler triple (`ActivationHandler` / `ActionHandler` / `DeactivationHandler`), the per-frame producer loop, one-tree-per-window-per-process.
- [**tree-model.md**](tree-model.md) — The `Node` data model: 182-variant `Role` enum, 22-variant `Action` enum, boolean and tri-state flags, `Toggled` / `Invalid` enums, relations (`labelled_by`, `described_by`, `controls`, `flow_to`, …), `Live` enum, coordinate spaces, the root identification rule, text content limitations, ACCNAME 1.2 mapping.
- [**platform-adapters.md**](platform-adapters.md) — Per-platform adapter status, capabilities, limitations, screen-reader interop: Windows (UIA + NVDA/JAWS/Narrator), macOS (NSAccessibility + VoiceOver; bug #520), Linux (AT-SPI + Orca; Wayland vs X11; async-runtime requirement), Android (TalkBack; the two adapter shapes; `embedded-dex` feature), iOS (alpha; UIAccessibility + VoiceOver), web (not shipped), `accesskit_winit`'s three constructors.
- [**api.md**](api.md) — Producer-side API surface: top-level item inventory at 0.24.0 (~200 accessor methods on `Node`), `Node::new(Role)` setter style (NO more `NodeBuilder` / `NodeClass`), `NodeId` semantics, `TreeUpdate` construction, the three handler traits, the Unix async-runtime requirement, threading model, lifetime/ownership, versioning posture.
- [**integration.md**](integration.md) — The canonical winit-based integration pattern; how egui, Slint, Iced (it doesn't), Bevy via `bevy_a11y` (the megacomponent Buiy replaces), Freya, Xilem, and Buiy each wire AccessKit; coordinate-space gotcha; threading; the issue-17644 lesson Buiy's decomposed-component split addresses. (For the **inbound** `ActionHandler::do_action` half — which `bevy_winit` already owns and Buiy must *not* re-register — see [`agent-control.md § 4`](agent-control.md).)
- [**capabilities.md**](capabilities.md) — What AccessKit's schema CAN express (ARIA roles, states, relations, live regions, text), and what it deliberately CANNOT (APG keyboard contracts, custom widget patterns outside 182 roles, forced-colors / reduced-motion, programmatic focus skipping for inert, APCA / WCAG contrast). Where each gap is addressed in Buiy.
- [**agent-control.md**](agent-control.md) — The **agent / automation lens** on the same tree: the bidirectional `ActionRequest` → `do_action` contract, platform AX as an automation substrate (UIA control patterns / AT-SPI `do_action` / `AXUIElementPerformAction`), the snapshot-then-act-by-ref agent loop, **the exact Buiy seam** to go bidirectional via the existing `bevy_winit` `MessageWriter<ActionRequestWrapper>` channel (no new `ActionHandler`), the per-widget action vocabulary to advertise, the inverted-lossiness insight, and the honest limits (closed 22-verb set, per-frame-rebuild instability, GPU-drawn content, multi-window scoping).

**Project lens**

- [**history.md**](history.md) — 2021 genesis through 2026-05-11 iOS 0.1.0 release; the core-crate timeline 0.1 → 0.24; per-adapter landing dates; adoption milestones; language bindings; Pneuma formation; Matt Campbell's actual lineage (Serotek + Microsoft, NOT NVDA); the Google LLC AUTHORS entry (Chromium-derived code).
- [**governance.md**](governance.md) — Stewardship (independent project; Pneuma-adjacent but not contractually owned), funding (no AccessKit-specific announcement; NLnet NGI0 for AT-SPI + iOS; GitHub Sponsors); release cadence (~quarterly with irregular bursts); CONTRIBUTING (Conventional Commits, release-please, MSRV 1.85, no formal RFC process); W3C ARIA / ACCNAME 1.2 alignment (AccessKit doesn't compute names — consumers do); bus factor.
- [**ecosystem.md**](ecosystem.md) — Major adopters verified (egui, Bevy, Slint, Freya, Xilem/Masonry); verification corrections (Iced not yet, Tauri WebView, GPUI/Zed no, Druid discontinued); production-app reach (mostly egui by volume); adjacent a11y tooling; comparison to native platform a11y APIs; the "why not just use platform APIs directly" answer; ACCNAME 1.2 alignment story.
- [**critiques.md**](critiques.md) — Combines critiques + open problems per the brief. Critiques: iOS adapter alpha; Android coverage incomplete; no web adapter; per-frame TreeUpdate cost in large trees; one-tree-per-window constraint; cadence mismatch with Bevy + winit; bus-factor / stewardship informality; "rough feature parity" caveat not version-tagged; rich text / hypertext unsupported. Open problems: 11 inherited unknowns Buiy will carry by adopting AccessKit.

**Reference**

- [**lessons.md**](lessons.md) — **The consult-this-when-designing decision file.** Validates / Avoid / Borrow.
- [**glossary.md**](glossary.md) — System-specific terms: `Tree`, `TreeUpdate`, `Node`, `NodeId`, `Role`, `Action`, `Adapter`, `ActivationHandler`, producer vs consumer, the platform-adapter crates, the AT names (NVDA, VoiceOver, Orca, TalkBack, JAWS, Narrator), `AccessibilityRequested`, `bevy_a11y`, `NLnet NGI0`, `Pneuma Solutions`.

## How to use this prior-art doc

When designing a Buiy feature that touches accessibility:

1. Start in [**lessons.md**](lessons.md). It enumerates which Buiy design decisions AccessKit already validates, which AccessKit pitfalls Buiy has explicit mitigations for, and which AccessKit primitives are worth borrowing.
2. If lessons references a subsystem, read the matching file (`architecture.md`, `tree-model.md`, `platform-adapters.md`, `api.md`, `integration.md`, `capabilities.md`).
3. If you're investigating whether AccessKit can express a specific ARIA / WCAG concept, [capabilities.md](capabilities.md) and [tree-model.md](tree-model.md) have the per-feature mapping; [critiques.md](critiques.md) covers what it CANNOT express.
4. If you're designing an agent / automation / test-driver surface on top of the tree, read [agent-control.md](agent-control.md) — it locates the exact inbound seam and the per-widget action vocabulary.
5. If you're worried about maintenance/governance (license, bus factor, funding, release cadence, fork strategy), read [governance.md](governance.md) and [history.md](history.md).
6. Promote any decision that affects Buiy into a Buiy spec under `docs/specs/`. This folder captures what we learn from AccessKit; it does not encode Buiy's own decisions.

Doc lives, not snapshot — bump the date in this file's header on every meaningful update. The most date-sensitive facts are crate versions, adapter landing dates, and the iOS / Android adoption story; refresh those when next iterating.

**Framing disclosure.** This corpus is written from a **"Buiy is the AccessKit producer; `bevy_a11y` is the BSN-hostile producer Buiy replaces per-window"** stance. Most "Implications for Buiy" lines treat AccessKit's API surface as a fixed contract Buiy adapts to. Future readers auditing whether *AccessKit itself* is the right primitive — vs writing per-platform native a11y directly, vs forking AccessKit, vs waiting for a different cross-platform abstraction — should weigh the corpus accordingly: it's a learn-from-AccessKit-into-Buiy artifact, not a neutral catalog of accessibility-bridge choices. Two biases worth naming explicitly:

- **Pre-1.0 cadence churn is soft-pedalled.** Buiy will inherit AccessKit's irregular release cadence, frequent breaking minors, and "all adapter crates bump together" coupling as a real cost across every Bevy migration. The corpus describes it; it doesn't dwell on it. A reader weighing "should we even commit to AccessKit-first" should treat that cost as material, not incidental.
- **Producer-API ergonomics are over-emphasised.** Because Buiy's foundation spec built the decomposed-component split partly to escape `bevy_a11y`'s megacomponent shape, the corpus treats AccessKit's clean `Node::new(Role)` + setters API as a high-value affordance. That affordance is real, but it would also be real if Buiy talked to UIA / NSAccessibility / AT-SPI directly; the cross-platform pitch (one schema, many adapters) is the structurally load-bearing benefit, not the API shape.

## Sources

- AccessKit repo: https://github.com/AccessKit/accesskit
- AccessKit website: https://accesskit.dev/
- AccessKit on crates.io: https://crates.io/crates/accesskit
- AccessKit AUTHORS file: https://raw.githubusercontent.com/AccessKit/accesskit/main/AUTHORS
- AccessKit releases (per-crate, co-release pattern): https://github.com/AccessKit/accesskit/releases
- `accesskit` 0.24.0 docs.rs: https://docs.rs/accesskit/0.24.0/accesskit/
- `accesskit_winit` 0.33.0 docs.rs: https://docs.rs/accesskit_winit/0.33.0/accesskit_winit/
- Pneuma Solutions about: https://pneumasolutions.com/about/
- NV Access about (for the Curran/Teh-not-Campbell lineage): https://nvaccess.org/about-nv-access/
- NLnet NGI0 funding: https://nlnet.nl/project/
- Buiy foundation — accessibility: [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation — architecture §2.6 + §2.9: [`docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation — cross-cutting §3.18: [`docs/specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- Bevy issue #17644 (`bevy_a11y` BSN-incompatibility, the producer-side lesson AccessKit's API reveals): https://github.com/bevyengine/bevy/issues/17644
- Per-file `## Sources` sections cite the specific URLs each file relies on.
