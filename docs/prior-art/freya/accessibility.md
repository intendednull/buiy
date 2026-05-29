**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — AccessKit integration status

# Accessibility in Freya

Freya is an **AccessKit-producing framework**. Its workspace declares:

```
accesskit = "0.24.0"
accesskit_winit = "0.32.0"
```

This places Freya alongside Slint, egui, Bevy (via `bevy_a11y`), Floem, Druid (legacy), Makepad, and Zed (via GPUI's tree) in the named adopters listed in [`../accesskit/ecosystem.md`](../accesskit/ecosystem.md). The depth of Freya's AccessKit tree-building has not been independently audited as part of this corpus; only its *presence* is verified at workspace level.

## What's verified

- **AccessKit + AccessKit-winit dependencies present in workspace.** Freya is wired to push `TreeUpdate`s to the platform AT bridge (UIA on Windows, NSAccessibility on macOS, AT-SPI on Linux).
- **Focus management exists.** `freya-hooks::use_focus()` returns a `FocusManager`. Focus state is a Freya-side concept, distinct from AccessKit focus (Freya's focus manager *feeds* AccessKit focus).
- **Tab navigation works** out of the box for focusable widgets (`Button`, `Input`-like components).

## What's NOT independently verified in this corpus

- **APG conformance per widget.** Whether Freya's `Slider`, `Calendar`, `VirtualScrollView`, etc. follow [WAI-ARIA APG](https://www.w3.org/WAI/ARIA/apg/) keyboard contracts (`Slider`: ←/→/Home/End/PgUp/PgDn; `Calendar`: grid pattern with arrow nav + PgUp/PgDn for month/year; etc.).
- **ACCNAME 1.2 compliance.** Whether Freya widgets compute accessible names following the [Accessible Name and Description Computation 1.2](https://www.w3.org/TR/accname-1.2/) algorithm, or use ad-hoc name extraction.
- **Live region support.** Whether `aria-live`-equivalent announcements are exposed (Buiy [foundation § 3.11](../../specs/2026-05-07-buiy-foundation/accessibility.md) commits to a global announcer resource).
- **Forced-colors / high-contrast OS pref binding.** Whether Freya theme reacts to `prefers-contrast` / forced-colors mode (Buiy commits to this auto-binding).
- **Screen-reader real-utterance verification.** Has any screen-reader (NVDA / JAWS / VoiceOver / Orca) actually exercised a Freya app end-to-end? Unknown from public records.
- **AccessKit version cadence policy.** When AccessKit majors between Freya releases, how Freya absorbs the migration. Unknown.

These gaps are not unique to Freya — they apply to nearly every AccessKit adopter outside the major ones tracked in [`../accesskit/ecosystem.md`](../accesskit/ecosystem.md). The presence of the AccessKit dep is a *significantly above baseline* signal for Rust GUI frameworks; the depth claim requires independent audit.

## Adapter ownership

Freya is a *native desktop GUI*, so it always owns the winit window and the AccessKit `accesskit_winit::Adapter`. There is no coexistence story (unlike Buiy's potential coexistence with `bevy_ui` per foundation [cross-cutting § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) — Freya is the only UI inhabiting its window.

This single-owner model is structurally simpler than Buiy's. Buiy's [foundation § 2.6 Adapter ownership](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first) keys adapters by winit `WindowId` precisely to allow per-window coexistence with `bevy_a11y` / `bevy_ui`; Freya doesn't have that constraint.

## What Buiy can learn

- **Validates AccessKit-first as a working Rust GUI policy.** Freya + Slint + Bevy + Floem + Zed (GPUI) all making AccessKit a workspace-level dep is the existence proof that AccessKit is the *de facto* Rust AT-bridge standard. Foundation [§ 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first) is well-precedented.
- **Hook-based focus access works.** `use_focus()` as a per-component reactive accessor for focus state is a clean pattern. Buiy's foundation does not commit to hooks (no signal layer in v1), but the *shape* — focus state as something a UI element can subscribe to — is reusable.
- **The depth gap is universal.** Freya, like most AccessKit adopters, ships *adapter presence* without per-widget APG verification. Buiy's [verification spec](../../specs/2026-05-07-buiy-foundation/verification.md) commits to APG-conformance tests in CI for every Buiy widget — this is a deliberate above-baseline commitment, and the prior-art shows why it matters: presence-without-verification has been the Rust GUI norm.

## What Buiy should not borrow

- **Theme-as-Rust-struct without forced-colors / prefers-contrast binding.** Freya's theme is pure code; OS preferences do not auto-propagate. Buiy's `UserPreferences` resource ([foundation § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)) is the more accessibility-correct choice.
- **Single-window assumption.** Buiy supports multi-window from the start (foundation sub-spec `buiy-window-and-surface-design`); the per-`WindowId`-adapter pattern is the correct generalization.

## Open questions

- **Verify by reading source.** A second pass that reads `freya-core`'s AccessKit tree-builder code would resolve the depth-of-integration questions above. Out of scope for this compressed corpus.
- **Screen-reader test gallery.** Would benefit from a community-maintained "Freya apps tested with NVDA/JAWS/VoiceOver/Orca" gallery; doesn't currently exist.

## Sources

- Freya workspace `Cargo.toml` (`accesskit = "0.24.0"`, `accesskit_winit = "0.32.0"`) — https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml
- Freya docs.rs (hooks: `use_focus`) — https://docs.rs/freya/latest/freya/hooks/
- Cross-references: [`../accesskit/lessons.md`](../accesskit/lessons.md), [`../accesskit/ecosystem.md`](../accesskit/ecosystem.md), [`../slint/accessibility.md`](../slint/accessibility.md).
- Buiy foundation — [`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first), [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md).
- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- ACCNAME 1.2 — https://www.w3.org/TR/accname-1.2/
