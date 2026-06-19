**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — adopter survey (egui, Slint, Bevy, Freya, Xilem/Masonry, …), production-app reach, comparison to native platform a11y APIs, the ACCNAME 1.2 alignment story

## Major adopters (verified)

| Adopter | Status | First integration | Current AccessKit version |
|---|---|---|---|
| **egui** | Production, enabled by default in eframe | [PR #2294](https://github.com/emilk/egui/pull/2294), merged 2022-12-04 (Windows-only at first), by Matt Campbell himself | accesskit 0.21.0 in egui 0.32.0 (2025-07-10); current bumps tracked in egui's CHANGELOG. |
| **Bevy** (via `bevy_a11y`) | Production, ships with default plugins | [PR #6874](https://github.com/bevyengine/bevy/pull/6874), merged 2023-03-01, in Bevy 0.10.0 | accesskit 0.24 on `main` ([bevy_a11y/Cargo.toml](https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/Cargo.toml)) |
| **Slint** | Production | Accessibility primitives in v0.2.5 (2022-07-06); AccessKit explicit at v1.7.0 (2024-07-18) | accesskit 0.16 was the named version at 1.7.0; ongoing upgrades (e.g. Slint PR #9919, 2025-11-03). |
| **Freya** | Production | Tracks AccessKit closely as a workspace dependency | accesskit 0.24.0 + accesskit_winit 0.32.0 ([Freya Cargo.toml](https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml)) |
| **Xilem / Masonry** (Linebender) | Production | Listed as foundational dependency in the [Xilem README](https://github.com/linebender/xilem): "AccessKit for plugging into accessibility APIs." | Tracked in workspace. Verify exact pin via current Cargo.toml. |
| **Druid** | **Discontinued** — never integrated AccessKit | n/a — succeeded by Xilem | n/a |

## Non-adopters / verification correction list

| Project | Brief preamble claim | Verified status |
|---|---|---|
| **Iced** | "Notable downstream" | **Not an adopter as of 2026-05-22.** Active draft PR #3111 (opened 2025-11-11), earlier WIP PR #1849 (2023-05-11) still draft, PR #3281 closed unmerged 2026-03-14. Iced 0.14.0 (2025-12-07) shipped without AccessKit. |
| **Tauri** | "Notable downstream" | **Not an adopter.** Tauri renders via the system WebView (`tao` + `wry`); the WebView provides accessibility via the OS's native a11y for the embedded browser engine. AccessKit not in Tauri's stack. |
| **GPUI (Zed editor)** | "verify" hedge in the preamble | **No AccessKit integration visible** in [Zed](https://github.com/zed-industries/zed) as of 2026-05-22. The verify hedge resolves negative. Zed/GPUI's accessibility story is separate and largely undocumented publicly. |
| **Vello** | preamble "?" | Vello is a render layer (vector graphics on wgpu), not a UI toolkit — it does not directly use AccessKit. AccessKit consumers like Xilem use Vello as their renderer separately. |

## Production apps shipping AccessKit (via downstream)

Confirming "is this in user-facing software" — the answer is yes, mostly via egui:

- **egui-based apps** — production egui applications (Rerun, the egui playground in `eframe`, various game-modding tools, etc.) ship AccessKit. The reach of egui is the dominant AccessKit-by-numbers data point.
- **Slint-based apps** — Slint targets embedded, desktop, and mobile; AccessKit deployment on each Slint app depends on the target. Concrete production deployments verifiable on the Slint customers page (not pinned in this folder; refresh on next pass).
- **Bevy games** — `bevy_a11y` ships with default plugins, so any Bevy game on a desktop adapter target has the AccessKit pump available, even if `bevy_ui` does not yet expose every widget through it.
- **Zed editor** — **not shipping AccessKit** based on the verification above. Zed's accessibility coverage is reported externally as limited; the corpus does not document a path through AccessKit.
- **Helix editor** — terminal-based; uses its operating environment's terminal accessibility (e.g. a terminal that already has screen-reader integration). AccessKit not in the stack.

The total `accesskit` crates.io downloads (17.9M lifetime, 4.08M recent per [crates.io](https://crates.io/crates/accesskit) on 2026-05-22) is **mostly egui**. Treat AccessKit's user-facing reach as dominated by the egui distribution; everything else is additive.

## Adjacent a11y tooling

AccessKit is the bridge to OS-native a11y, not a replacement for the screen readers / assistive technologies themselves. The consuming stack on each platform:

- **Windows:** UI Automation (UIA) is the platform API; **NVDA**, **JAWS**, **Narrator** are the AT consumers. AccessKit's `accesskit_windows` calls UIA, which NVDA/JAWS/Narrator read. Matt Campbell's Windows-first development path reflects UIA being the most-tested target.
- **macOS:** NSAccessibility is the platform API; **VoiceOver** is the AT. `accesskit_macos` bridges NSAccessibility.
- **Linux:** AT-SPI 2 over D-Bus is the platform API; **Orca** is the AT (plus various braille displays via BRLTTY). `accesskit_unix` uses the pure-Rust `zbus` D-Bus implementation, avoiding the C `at-spi2-atk` dependency chain.
- **Android:** Java accessibility API is the platform interface; **TalkBack** is the AT. `accesskit_android` is the JNI bridge.
- **iOS:** UIAccessibility protocols (UIKit) are the platform API; **VoiceOver iOS** is the AT. `accesskit_ios` v0.1.0 (2026-05-11) is the first shipping crate.
- **Web:** ARIA in the DOM is the platform API — fundamentally different model from the parallel-tree approach AccessKit uses everywhere else; see [`critiques.md`](critiques.md). **No shipping `accesskit_web` crate** as of 2026-05-22; listed as "planned".

**Caveat about NVDA / VoiceOver / TalkBack utterances:** specific phrases ("Button, label, disabled") that a screen reader speaks for a given AccessKit Node are NOT in AccessKit's contract. Utterance depends on the AT's verbosity settings, language pack, and platform-specific behaviour. The Buiy verification harness ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) must snapshot the AccessKit *tree* and *tree property names*, never the screen-reader-spoken transcript — those drift across AT versions and locales.

## Comparison to traditional platform a11y APIs

AccessKit **abstracts** the platform APIs — it does not **hide** them. The four platform adapters are thin wrappers that map AccessKit's Node + Action vocabulary to the platform vocabulary:

| AccessKit concept | UIA | NSAccessibility | AT-SPI |
|---|---|---|---|
| `Node` + `Role` | UIA Element + ControlType | NSAccessibilityElement + AXRole | AT-SPI Accessible + Role |
| `Action::Click` | `IInvokeProvider::Invoke` | `AXPress` action | `Action::Activate` |
| `Action::SetValue` | `IValueProvider::SetValue` | `accessibilitySetValue:` | `EditableText::insertText` |
| `set_labelled_by` | LabeledBy / LegacyIAccessible | `AXTitleUIElement` | `Relation::LabelledBy` |
| live regions | UIA notification events | `NSAccessibilityAnnouncementRequested` | `Object::TextChanged` + `org.a11y.atspi.Event.Object.AnnouncementChanged` |
| Bounds | UIA BoundingRectangle (screen coords) | `accessibilityFrame` (screen coords) | `Component::GetExtents` |

This abstraction layer is **why** AccessKit costs less to integrate than per-platform native a11y — Buiy writes Node + Action + Relation once and the adapters distribute it. The trade-off is that AccessKit-only abstractions (the `Toggled` enum unification, frequency-ordered Role enum, etc.) sometimes paper over real platform differences (e.g. ARIA's `mixed` checkbox state has different AT verbalisation on Windows vs macOS vs Linux). See [`critiques.md`](critiques.md) for the divergence concerns.

## "Why not just use platform APIs directly?"

The cross-platform pitch — implementing accessibility once instead of 5+ times — is the obvious answer, but it understates the technical case. Native a11y APIs are:

- **Different shapes** — UIA is COM-based, NSAccessibility is ObjC protocols, AT-SPI is D-Bus, Android is JNI, iOS is UIKit protocols. Five language-bridge sets.
- **Different lifetimes** — UIA elements live in the COM process; NSAccessibility elements have ARC lifetime; AT-SPI has D-Bus reference counting; Android has JVM GC.
- **Different threading models** — UIA marshals across apartments; NSAccessibility expects main thread; AT-SPI has its own event-loop integration; Android requires JNI thread attachment.

AccessKit absorbs all of this. The consumer talks Node + Action; the platform adapters absorb the lifetime + threading + marshalling differences.

## ACCNAME 1.2 alignment

The [W3C ACCNAME 1.2 spec](https://www.w3.org/TR/accname-1.2/) is the algorithm for computing an accessible name from a UI element's labels, descriptions, and content. AccessKit's role here is **infrastructure, not algorithm**: it provides the references (`set_labelled_by([NodeId])`, `set_described_by([NodeId])`, `set_label([str])`, `set_description([str])`) but does not compute the final name. The consuming toolkit walks the references per ACCNAME 1.2.

The Buiy spec moves the ACCNAME 1.2 implementation into `buiy_core` ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)), with the precedence:

```
Name:  aria-labelledby  >  aria-label  >  host-language label  >  content  >  title
Desc:  aria-describedby >  aria-description  >  host-language  >  title
```

Hidden-subtree exclusion rules are part of the algorithm and live in `buiy_core`. The result is pushed to AccessKit via `Node::set_label([str])` and `Node::set_description([str])`.

The fact that AccessKit *deliberately doesn't ship an ACCNAME 1.2 implementation* is a load-bearing design choice — different host language conventions, different label sources, different inheritance rules, all argue for keeping name computation host-side. The trade-off is that every AccessKit consumer reimplements ACCNAME 1.2; whether two consumers' implementations agree is up to them.

## Cross-links

- Per-platform adapter mechanics: [`platform-adapters.md`](platform-adapters.md).
- Integration mechanics in adopters: [`integration.md`](integration.md).
- The Iced / GPUI / Tauri non-adoption story is also relevant context for [`critiques.md`](critiques.md) on AccessKit's reach.
- ACCNAME 1.2 is reachable via [`capabilities.md`](capabilities.md).

## Sources

- https://github.com/AccessKit/accesskit/blob/main/README.md
- https://github.com/emilk/egui/pull/2294
- https://github.com/emilk/egui/blob/main/CHANGELOG.md
- https://github.com/bevyengine/bevy/pull/6874
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/Cargo.toml
- https://github.com/slint-ui/slint/blob/master/CHANGELOG.md
- https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml
- https://github.com/linebender/xilem
- https://github.com/linebender/druid
- https://github.com/iced-rs/iced/pulls?q=accesskit
- https://github.com/tauri-apps/tauri
- https://github.com/zed-industries/zed
- https://crates.io/crates/accesskit
- https://www.w3.org/TR/accname-1.2/
- docs/specs/2026-05-07-buiy-foundation/accessibility.md
- docs/specs/2026-05-07-buiy-foundation/verification.md
