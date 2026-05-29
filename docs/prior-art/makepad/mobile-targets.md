**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — mobile target maturity (iOS / Android / tvOS / OpenHarmony); cargo-makepad toolchain; touch handling

# Mobile targets

The feature Makepad pushes hardest as a differentiator. The `cargo-makepad` cargo subcommand installs and drives cross-target toolchains for iOS, Android, tvOS, and OpenHarmony in addition to the desktop and WASM targets. Robrix (the Robius community's flagship Matrix client built on Makepad) ships on macOS / Linux / Windows / Android / iOS / iPadOS — the most concrete demonstration that Makepad's mobile story is real.

## `cargo-makepad` toolchain installer

The README documents installation steps roughly:

```sh
cargo install cargo-makepad --locked
cargo makepad android install-toolchain
cargo makepad ios install-toolchain
cargo makepad apple-tv install-toolchain   # tvOS
cargo makepad openharmony install-toolchain
```

What `install-toolchain` does (per repo `tools/cargo_makepad/`):

- **Android.** Installs the Android NDK, sets `ANDROID_HOME` / `ANDROID_NDK_ROOT`, configures rustup targets (`aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`), prepares a Gradle skeleton, optionally pre-builds the Android JNI bridge crate.
- **iOS.** Configures rustup targets (`aarch64-apple-ios`, `aarch64-apple-ios-sim`, `x86_64-apple-ios`), drives `xcrun` for signing and `xcodebuild`-like behaviour without requiring a full Xcode project. Signing certificate handling is in-tool.
- **tvOS.** Same as iOS but targeting `aarch64-apple-tvos` / `aarch64-apple-tvos-sim`.
- **OpenHarmony.** Configures HarmonyOS SDK paths. Currently "builds but doesn't run yet" per Robrix README.

Building a Makepad app for mobile is then `cargo makepad android run -p <crate>` / `cargo makepad ios run -p <crate>`.

**Comparison with the Bevy mobile story.** Bevy supports iOS and Android but the toolchain story is more manual — `cargo-mobile2` is the community-standard tool. Makepad's `cargo-makepad` is more integrated but less general (Makepad-specific). Buiy's mobile staging (foundation README § 5 open question, accessibility.md § "Platform support — staged") inherits Bevy's substrate, not Makepad's.

## Touch input handling

Makepad's event model includes `FingerDown` / `FingerMove` / `FingerUp` / `FingerHoverIn` / `FingerHoverOut` / `FingerScroll` / `FingerLongPress` events on the `Cx` event queue. Widgets handle them in `Widget::handle_event`. The model is per-finger (not gesture-recognized): the application writes gesture detection on top of finger events. Examples like `arracing` and `splash` demonstrate touch-driven interaction.

What works (per examples + Robrix shipping):

- **Single-finger tap / press / drag** — first-class. The common case.
- **Multi-finger touch** — exposed via per-finger IDs in the event queue; gesture recognition is application-level.
- **Scrolling momentum** — `ScrollXYView`'s built-in momentum scrolling is mobile-grade.
- **Pinch-zoom** — application-level via multi-finger events (e.g., `map` example).
- **Soft keyboard** — `TextInput` triggers the OS soft keyboard; layout adjusts for keyboard occlusion (per Robrix).

What's documented as work-in-progress:

- **Geolocation permissions on Android** (per Robrix's known-issues list).
- **OpenHarmony runtime** — builds, doesn't run.
- **iOS background / lifecycle** — minor friction in Robrix's WAN message handling.

## Mobile widget primitives

The `makepad-widgets` 1.0.0 catalog includes:

- **`<Window>`** — windows on desktop, full-screen surface on mobile.
- **`<StackNavigation>`** — push/pop view stack with mobile-style transitions. Mobile-pattern-aware.
- **`<ScrollXYView>` / `<PortalList>`** — virtualized scrolling, touch-momentum-aware.
- **`<TextInput>`** — IME / soft-keyboard-integrated.
- **`<Slider>` / `<SwipeAction>`** — touch-driven controls.
- **`<DesktopButton>` / `<MobileButton>`** — separate variants where touch ergonomics matter.

The catalog covers common mobile patterns but is **smaller than the WAI-ARIA APG** — no `Combobox` (no listbox-popup pattern documented), no `TreeView`, no `Menubar`, no `Tabs` with full APG keyboard contract, no `DatePicker` / `Calendar`, no `MenuButton` with the APG actions enumerated. Buiy's APG widget commitment ([media-and-widgets.md](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) is a much larger surface than Makepad ships.

## Mobile accessibility — entirely absent

This is the cleanest place to surface the [`open-problems.md`](open-problems.md) and [`critiques.md`](critiques.md) accessibility critique. Mobile platforms have first-class accessibility APIs:

- **iOS** — UIAccessibility (UIA on Apple platforms). The OS reads accessibility tree from the active app and drives VoiceOver / Switch Control / Voice Control. Makepad does not produce a UIA-compatible tree.
- **Android** — TalkBack reads via the Android AccessibilityNodeInfo tree. Makepad does not produce one.
- **iPadOS** — same as iOS.

A blind iPhone user opening Robrix would find an entirely unlabeled, unfocusable, unreadable application. There is no producer-side fix Makepad provides; this is a framework-level gap.

For comparison, `accesskit_ios` 0.1.0 (recently shipped in the AccessKit ecosystem, see [`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md)) and `accesskit_android` are the canonical Rust-ecosystem producer-side adapters. Makepad uses neither.

## Tablet / iPad

Robrix README says "iPadOS" is supported. The same Makepad code path serving iOS handles iPad; the layout adapts via `<StackNavigation>` orientation handling. No iPad-specific split-view (sidebar pattern) widget is documented; Buiy would need to produce one if it wanted iPad split-view ergonomics.

## tvOS / set-top boxes / focus model

Makepad supports tvOS via the iOS-derived Metal backend. The remote-control / focus-via-D-pad input model exists in the event queue (per the examples in `arracing`). But there's no documented `tvOS focus engine` integration equivalent to UIKit's `UIFocusGuide` system; Buiy spatial gamepad navigation ([architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md)) is a richer model.

## Implications for Buiy

- **Validates mobile-targeting as a real differentiator.** Robrix shipping on five mobile-class targets via Makepad is the cleanest data point that **a Rust UI toolkit can serve mobile production today** without a JS-bridge / React-Native-style indirection. Buiy's mobile-platform-staging ([accessibility.md § "Platform support — staged"](../../specs/2026-05-07-buiy-foundation/accessibility.md)) becomes more credible with this data point.
- **Borrow: integrated cross-target build tooling.** `cargo-makepad` is the kind of integration Buiy's tooling should emulate — a single tool to install rustup targets + Android NDK + iOS signing + run on device. `cargo-mobile2` is the substrate; a `cargo-buiy` wrapper that drives platform-specific behaviour is the polish target.
- **Borrow: mobile input primitives.** Makepad's per-finger `FingerDown/Move/Up` event model is a clean baseline for Buiy's `buiy-input-events-design` mobile coverage. Gesture-recognizer-layered-on-finger-events is the right design (vs. fixed gesture recognizers at the framework level).
- **Avoid: ship mobile without accessibility.** Robrix is unusable for VoiceOver / TalkBack users today. This is a *Buiy red line* — mobile platforms have accessibility APIs; Buiy must wire AccessKit (or platform-direct in the iOS / Android cases where AccessKit adapters land late) before claiming mobile support. See [`lessons.md`](lessons.md).
- **Borrow: `<StackNavigation>` as the mobile-pattern primitive.** Mobile-app users expect push/pop view stacks with platform-idiomatic transitions. Buiy's widget catalog ([media-and-widgets.md](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) should include this pattern.

## Sources

- Makepad README mobile setup: https://github.com/makepad/makepad
- `cargo-makepad`: https://github.com/makepad/makepad/tree/dev/tools/cargo_makepad
- Robrix platform-support table: https://github.com/project-robius/robrix
- Mobile-targeting examples: `arracing`, `splash`, `map`, `camera`, `text_input`, `text_selection`
- Sibling files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`live-language.md`](live-language.md), [`open-problems.md`](open-problems.md), [`lessons.md`](lessons.md)
- Buiy foundation accessibility staging: [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy input events sub-spec scope: [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) (`buiy-input-events-design`)
- AccessKit mobile adapters: [`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md)
