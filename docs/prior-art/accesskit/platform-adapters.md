**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — per-platform adapter status, capabilities, limitations, and screen-reader interop

AccessKit's value proposition is **one tree shape, many platform adapters**. The producer (Buiy) builds the tree once; each adapter translates it to the local OS accessibility API. This file walks each adapter as of 2026-05-22, with version landings and known gaps. Numbers come from crates.io and the AccessKit release stream of 2026-05-11.

## Adapter inventory at a glance

| Crate                  | Latest    | First published | Status                            | Platform API        |
|------------------------|-----------|-----------------|-----------------------------------|---------------------|
| `accesskit_windows`    | 0.33.0    | 2021-12-21      | Production                        | UI Automation (UIA) |
| `accesskit_macos`      | 0.26.1    | 2022-11-23      | Production                        | NSAccessibility     |
| `accesskit_unix`       | 0.21.1    | 2023-01-05      | Production                        | AT-SPI (D-Bus)      |
| `accesskit_atspi_common` | 0.18.1  | (internal)      | Production (shared by unix)       | —                   |
| `accesskit_android`    | 0.7.3     | 2025-03-06      | Pre-1.0, shipping                 | Android a11y / TalkBack |
| `accesskit_ios`        | **0.1.0** | **2026-05-11**  | Brand-new, "basic"                | UIAccessibility     |
| `accesskit_winit`      | 0.33.0    | (early)         | Multiplex helper                  | (per-platform)      |
| Web / DOM adapter      | —         | —               | **Not shipped**                   | —                   |

All adapter releases land on the same publish day cycle (2026-05-11 batch above; prior batches 2026-03-04, 2026-02-25, 2026-02-01, 2026-01-18, 2026-01-15, 2026-01-03, 2025-10-25). The version numbers diverge because the crates have independent semver tracks, but they always co-release.

**Critical caveat for Buiy's "platform-staged" v1 commitment.** Buiy's `architecture.md` §2.9 calls iOS "in-progress upstream in AccessKit" and web "not yet shipped." As of 2026-05-22, `accesskit_ios` 0.1.0 shipped 11 days ago — technically released, but at "basic" feature level with 229 total downloads, no battle-testing. Treating it as "production" would be premature; Buiy's "deferred until each platform's AccessKit adapter exposes a headless harness usable in CI" gate is still the right posture for v1.

---

## Windows — `accesskit_windows`

**API:** UI Automation (UIA), the modern Windows accessibility API. The legacy MSAA path is not exposed by AccessKit — UIA bridges to MSAA via the OS shim, so MSAA-only ATs still work via the bridge but AccessKit produces UIA natively.

**Integration boundary.** The adapter implements `IRawElementProviderSimple`, `IRawElementProviderFragment`, `IRawElementProviderFragmentRoot`, and the control-pattern interfaces (`IInvokeProvider`, `IValueProvider`, `IRangeValueProvider`, `ISelectionProvider`, `IGridProvider`, etc.) as COM objects backed by the cached tree. `accesskit_winit` calls the adapter's `process_event` on every `WM_GETOBJECT` to route UIA queries; producer-side updates feed `accesskit_consumer` to keep the cache fresh.

**COM threading.** UIA queries arrive on the UI thread (the window's WndProc thread). The adapter does not require background threads — all work is synchronous on the WndProc-owning thread. winit users get this for free because winit pins window events to the event-loop thread.

**Performance.** Push-based + cached: AT queries are answered from the in-process cache without producer involvement, so query latency is sub-millisecond. The cost moves to `TreeUpdate` application time, which scales linearly with the diff size.

**Screen reader interop.** AccessKit-tested toolkits work with **NVDA**, **Narrator**, and (per the Buiy verification roster — [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) **JAWS**. The README does not enumerate per-AT compatibility statements — concrete verbatim utterances ("Button. Submit. Press SPACE to activate.") vary per AT and AT version and should be confirmed via Buiy's verification harness rather than cited from upstream marketing.

**0.33.0 (2026-05-11) added "Support tree views on Windows"** — earlier versions had partial `Role::Tree` / `Role::TreeItem` support; the dev-cycle 0.33 release made tree-view UIA expansion / collapse complete. Buiy's Tree widget can rely on this.

---

## macOS — `accesskit_macos`

**API:** Cocoa's `NSAccessibility` informal protocol (responding to selectors like `accessibilityLabel`, `accessibilityValue`, `accessibilityChildren`, `accessibilityPerformPress` on `NSAccessibilityElement` subclasses).

**Integration boundary.** The adapter creates `NSObject` subclasses representing the tree, attached to the winit-owned `NSView`. `accesskit_winit::Adapter::process_event` forwards `WindowEvent::Focused` and `WindowEvent::Resized` to the adapter so it can maintain `accessibilityFrame` and focus state. NSAccessibility is a *pull* API natively (the AT sends selector messages and the OS dispatches them to the producer), but AccessKit caches the tree so producer-side work happens once per update rather than on every selector arrival.

**Screen reader interop.** Primary AT is **VoiceOver**. The macOS README documents one current bug: **issue #520** — "the selected state of ListBox items is not properly communicated to assistive technologies." Buiy's ListBox widget will inherit this gap until #520 is fixed upstream. Workaround options: emit ListBox items as `Role::MenuItem` (loses semantic precision) or wait. Buiy's verification harness should include a `ListBox` selection fixture on macOS that currently expects-failure, flipping to expects-pass when upstream resolves.

**Bounds coordinate handling.** macOS coordinates are bottom-left origin; AccessKit's `Rect` is top-left origin in window coordinates. The adapter performs the y-axis flip and adds the window's screen position at query time — producers do not handle this.

---

## Linux — `accesskit_unix` + `accesskit_atspi_common`

**API:** AT-SPI 2 (Assistive Technology Service Provider Interface), the freedesktop.org D-Bus accessibility protocol. The adapter publishes the tree as D-Bus objects on the session bus (or, when AT-SPI runs over a dedicated bus, on the AT-SPI bus).

**Integration boundary.** The adapter is async — D-Bus calls go through a runtime. The `accesskit_unix` crate requires either the `tokio` or the `async-io` Cargo feature; `accesskit_winit` exposes the same gate. Bevy currently uses neither runtime for its main loop, so Buiy's `accesskit_winit` integration on Linux will pull in one of them — almost certainly `async-io` to avoid the heavier tokio dependency closure. (This is an open question worth pinning in a Buiy sub-spec.)

**Screen reader interop.** Primary AT is **Orca** (the GNOME/MATE screen reader). KDE's screen reader is also Orca in practice. Cinnamon, XFCE, etc. all share Orca.

**GNOME vs KDE.** AT-SPI is desktop-environment-agnostic, but GNOME's at-spi-registryd is the de facto registry. KDE ships Qt's QAccessible bridge that also publishes to AT-SPI. AccessKit producers do not need to differentiate.

**Wayland vs X11.** AT-SPI runs over D-Bus, which is transport-independent of the display server, so AccessKit producers work on both Wayland and X11 in principle. **However**: some legacy ATs depend on X11 grabbing / event-injection paths for *input* simulation that have no Wayland equivalent yet. This affects ATs that drive UI rather than just narrate it (Orca's mouse-mode features, screen-keyboard apps). The accessibility *information channel* (AT-SPI) is fine on Wayland; the *control* channel may not be. Buiy's spec calls this out as one of its open questions ([architecture.md §2.9](../../specs/2026-05-07-buiy-foundation/architecture.md#29-compatibility--policy)); the AccessKit project itself does not currently differentiate X11 vs Wayland in adapter docs.

**Funding.** The AT-SPI adapter received NLnet NGI0 Commons Fund support, per the upstream funding-acknowledgement pattern visible across recent releases. See [governance.md](governance.md) (sibling B).

---

## Android — `accesskit_android`

**Status: pre-1.0 shipping, two-shape API.** First published 2025-03-06 at 0.1.0; current is 0.7.3 (2026-05-11). Approximately 26k total downloads (vs. macOS 7.7M and Windows 7.8M — a real production-use gap, but the adapter is real).

**Two adapter shapes:**

1. **`Adapter`** — the low-level core. Maximum flexibility; the producer / wrapping framework writes the JNI glue and Android-side `View.AccessibilityDelegate` implementation.
2. **`InjectingAdapter`** — higher-level. Embeds a precompiled `.dex` file with the necessary Java class and "injects accessibility into an arbitrary Android view without requiring the view class to be modified." Activated via the `embedded-dex` Cargo feature. This is the "drop in and it works" shape — at the cost of bundling a small `.dex` blob.

**Screen reader interop.** Primary AT is **TalkBack** (Google's Android screen reader). Samsung's TalkBack-derivative and BRLTTY are downstream beneficiaries — anything that consumes the Android `AccessibilityNodeInfo` graph reads AccessKit-published nodes.

**Known gaps.** The pre-1.0 version vector (0.x not 1.x) signals the project authors are not yet ready to commit to API stability. Producer-side semver hygiene: assume an `accesskit_android` minor bump can force a producer-side migration. Buiy's "AccessKit major release between Bevy minors triggers a Buiy patch release" policy ([architecture.md §2.9](../../specs/2026-05-07-buiy-foundation/architecture.md#29-compatibility--policy)) needs an "or AccessKit-platform-adapter minor release on a not-yet-1.0 adapter" extension.

**winit interop.** `accesskit_winit` 0.33.0 added Android support; before that release, Android required direct integration without the winit adapter as a multiplexer.

---

## iOS — `accesskit_ios`

**Status: 0.1.0 shipped 2026-05-11, "basic iOS adapter."** This is the most recent platform landing in AccessKit. Author: Arnold Loubriat. Funding: NGI0 Commons Fund / NLnet (per the platforms/ios README). Total downloads at 2026-05-22: ~229.

**API:** UIKit's `UIAccessibility` informal protocol — selectors on `UIView` and the `UIAccessibilityContainer` protocol for non-view content. The adapter exposes the tree through these selectors backed by the cached tree.

**Screen reader interop.** Primary AT is **VoiceOver (iOS)**. Switch Control reads the same `UIAccessibility` graph. The 0.1.0 "basic" qualifier strongly suggests not every `Role` / `Action` is wired through yet — verbatim verification of which roles map to which iOS traits requires reading the adapter source or running fixtures, not citing release notes.

**Implication for Buiy's iOS posture.** The Buiy spec correctly identifies iOS as "in-progress upstream in AccessKit"; that's still true even with 0.1.0 released. Buiy's "manual-release-gate platform" treatment until a headless CI harness exists is the right call. A `buiy-platform-ios-design` sub-spec should not assume any specific iOS-AT interop unless verified against `accesskit_ios` 0.1.0's actual `Role` coverage.

---

## Web — DOM accessibility tree generation

**Status: not shipped.** No `accesskit_web` crate exists on crates.io as of 2026-05-22. The README mentions "A web adapter is planned" without dates. Recent issue-tracker activity does not show an actively-WIP web adapter PR.

The intended design (per long-standing AccessKit project discussion) is for the producer to emit DOM elements with appropriate ARIA attributes rather than to emit a parallel JS tree — i.e. the web adapter would render an HTML shadow tree alongside the producer's canvas-rendered UI, so that web ATs (NVDA + Chrome, VoiceOver + Safari) read the shadow tree. The shadow-tree approach is what Bevy's nascent web a11y work assumes; it is also what Slint's web target uses.

**Implication for Buiy.** Buiy's "Web (AccessKit web adapter — not yet shipped)" deferred-platform stance is accurate and load-bearing for the "v1 desktop-only with full a11y" commitment. Web ships when the upstream adapter ships, not before.

---

## `accesskit_winit` — the canonical winit integration

`accesskit_winit` 0.33.0 (2026-05-11) is the multiplexing helper that gives a producer **one `Adapter` per window, working on every supported platform** via the same Rust API. It wraps the per-platform adapter inside an `Adapter` struct and exposes:

```rust
pub struct Adapter { inner: platform_impl::Adapter }

pub enum WindowEvent {
    InitialTreeRequested,
    ActionRequested(ActionRequest),
    AccessibilityDeactivated,
}

pub struct Event { window_id: WindowId, window_event: WindowEvent }
```

Constructors:

- `Adapter::with_event_loop_proxy(window, event_loop_proxy)` — routes activation, action, and deactivation events through winit's `EventLoopProxy<T>` (a custom user-event channel). Best fit when the producer drives via winit's event loop.
- `Adapter::with_direct_handlers(window, activation_handler, action_handler, deactivation_handler)` — caller provides the three trait implementations directly. Best fit when the producer's main loop is *not* winit's event loop (Bevy's `MainSchedule`, Buiy's `BuiySet::A11yUpdate`). The handlers are owned by the adapter and called from whatever thread the platform dispatches events on.
- `Adapter::with_mixed_handlers<T>(window, activation_handler, event_loop_proxy)` — hybrid; activation handler is direct (for initial-tree-on-activation latency), action and deactivation route through the proxy.

Instance methods:

- `process_event(&mut self, window: &Window, event: &WinitWindowEvent)` — wire this into the winit `WindowEvent` handler so the adapter sees focus / resize / close events. Mandatory.
- `update_if_active(&mut self, updater: impl FnOnce() -> TreeUpdate)` — the producer's per-frame call. Closure runs *only if an AT is currently attached* — the activation-gate cost-avoidance from [architecture.md](architecture.md).

All constructors **panic if the window is already visible** — this is the AccessKit author's chosen way to enforce "register the adapter before the window appears" so the AT detects the producer's accessibility info on first show.

**Single tree per window per adapter.** This is enforced structurally: `Adapter` owns `Window`'s a11y bridge; constructing a second `Adapter` on the same `Window` is not supported. Buiy's per-window adapter ownership ([cross-cutting.md §3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md#318-compatibility-and-coexistence)) aligns directly with this rule.

## Sources

- crates.io: `accesskit_windows`, `accesskit_macos`, `accesskit_unix`, `accesskit_android`, `accesskit_ios`, `accesskit_winit`, `accesskit_atspi_common`, `accesskit_consumer` (versions and publish dates verified 2026-05-22)
- `platforms/windows/README.md`, `platforms/macos/README.md`, `platforms/unix/README.md`, `platforms/android/README.md`, `platforms/ios/README.md`: https://github.com/AccessKit/accesskit/tree/main/platforms
- macOS adapter issue #520 (ListBox selected state): https://github.com/AccessKit/accesskit/issues/520
- `accesskit_winit` 0.33.0 docs and release notes: https://docs.rs/accesskit_winit/0.33.0/accesskit_winit/
- NLnet NGI0 Commons Fund acknowledgement (iOS adapter and AT-SPI funding): https://nlnet.nl/project/
- Buiy spec — platform-staged v1 commitment: `/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md` §2.9
- Sibling: [architecture.md](architecture.md), [tree-model.md](tree-model.md), [api.md](api.md)
