**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — critiques (Zed-only dogfooding, Apache-only license, macOS-first parity, doc maturity, pre-1.0 churn) and open problems (a11y, mobile, web, third-party adoption, monorepo divergence)

# Critiques and open problems

This file is the **honest assessment** — the parts of GPUI that don't work, the costs the design imposes, and the questions that remain unanswered as of May 2026. Each section names the gap, then maps it to a Buiy decision (commit, avoid, or open question).

## Critique 1: Single-product dogfooding limits generality

GPUI was built to serve Zed. Every API exists because Zed needed it; every API _doesn't_ exist because Zed didn't need it. The consequences:

- **No widget library.** GPUI ships zero widgets. Buttons, checkboxes, dropdowns, dialogs, tooltips — all of these exist only in user code (Zed's, or `longbridge/gpui-component`'s). For a "UI framework" this is a major gap.
- **No theming model.** Theme structs live in user code (Zed has one; users define their own). There is no built-in theme token system, no asset format, no contrast linter, no OS-preference binding.
- **No animation library.** GPUI has timer primitives but no spring physics, no keyframe API, no transition system. Zed implements its animations manually per case.
- **No form/validation system.** No form state machine, no constraint validation, no error-message model.
- **Editor-specific features overweight.** The `UniformList`, `List`, and per-line custom layout primitives are exactly what an editor needs. A typical app needs more emphasis on dialogs, modals, and data-entry forms than on virtualized infinite-scroll lists.

**Buiy mapping:** This validates Buiy's foundation §2.3 list of subsystems Buiy owns (widget catalog, theming, animation, forms). Buiy commits to shipping all of these as first-class. GPUI's gap-list is effectively Buiy's required-feature checklist.

## Critique 2: Apache-2.0-only license is unusual

The single Apache-2.0 license (vs Rust's standard MIT/Apache dual) restricts reusability:

- Downstream MIT-only projects must explicitly handle the Apache attribution.
- The clean-room reimplementation path is the safer route for any project wanting MIT-or-Apache flexibility.
- The patent grant in Apache is valuable but not unique (MIT plus an explicit Apache patent grant via a CONTRIBUTING file is another common pattern).

**Buiy mapping:** Buiy commits to dual MIT/Apache. Any GPUI-sourced inspiration must be clean-room reimplemented, not copied. The good news: GPUI's value to Buiy is at the **design level** (the four-stage pipeline, SDF shaders, primitive decomposition), not the **code level** (Buiy's render pipeline lives in Bevy's render graph, not in GPUI's platform abstraction).

## Critique 3: macOS-first cross-platform parity

Despite production releases on Linux (2024) and Windows (2025), GPUI's quality ceiling is set by macOS:

- macOS rendering (Metal + Core Text + Cocoa) is reference quality.
- Linux quality trails — text rendering complaints, NVIDIA-PRIME issues, Wayland-vs-X11 inconsistencies.
- Windows quality is new and shaking out — performance characteristics still being benchmarked, accessibility gap acute.

The **per-platform native-API strategy creates a parity gap by construction**: each backend is independently good, but bringing them to feature parity is N×O(work-per-backend) instead of 1×O(cross-platform-work).

**Buiy mapping:** Buiy commits to Bevy's wgpu (one render path). The trade is "lower per-platform polish ceiling" for "consistent cross-platform behavior + less code." If Buiy ever needs DirectWrite-quality text on Windows or Core Text-quality text on macOS, the GPUI three-backend strategy is the reference for what a remediation would cost. Foundation [`architecture.md § 2.2`](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to wgpu; this is the explicit cross-platform-uniformity bet.

## Critique 4: Documentation maturity

GPUI's docs are visibly incomplete:

- The README is a stub (~30 lines) directing users to "read the Zed source code or ask in Discord."
- docs.rs coverage is shallow — types are documented but examples are sparse.
- The [`gpui.rs`](https://www.gpui.rs/) marketing site exists; the actual reference docs are thin.
- The official example list ([`crates/gpui/examples/`](https://github.com/zed-industries/zed/tree/main/crates/gpui/examples)) is reportedly outdated against the current API (issue [#46183](https://github.com/zed-industries/zed/issues/46183)).
- The most-cited learning resource is Antonio Scandurra's 2023 blog post — three years old, predating GPUI 2's major architectural changes.

This is **predictable for a pre-1.0 dogfooded framework**: docs lag the source by definition. But it raises the cost-of-entry for any third-party adopter substantially.

**Buiy mapping:** Buiy's documentation commitment lives in foundation `verification.md` — every widget specs its keyboard contract, every WCAG SC has a fixture, every theme token has docs. The discipline of "every claim is testable" doubles as a documentation forcing function. The opposite of GPUI's "read the source" model.

## Critique 5: Pre-1.0 API churn

The README explicitly says: "GPUI is still in active development as it works on the Zed code editor, and is still pre-1.0, with breaking changes often occurring between versions."

This is honest. It's also disqualifying for many uses:

- Downstream apps must pin to specific versions and absorb migration cost on each upgrade.
- Third-party widget libraries (like `gpui-component`) must continuously adapt to breaking changes.
- The crates.io publish cadence (three 0.2.x publishes in 18 months) means upstream changes batch into infrequent, large migration events.

**Buiy mapping:** Buiy's foundation §2.9 commitment is "rolling latest-stable Bevy. No multi-version compatibility promise. Each Bevy minor release is a migration event." This is structurally similar to GPUI's "breaking changes between versions" — but Buiy's migration cadence is set by Bevy's release schedule, which is a known, public quarterly-ish rhythm. GPUI's cadence is internal-only.

## Open problem 1: Accessibility (largest)

Covered in [`accessibility.md`](accessibility.md). The summary: no AccessKit, no platform a11y API integration, 2.5+ years of open discussion with no shipped solution. Windows users with screen readers cannot use Zed. The retrofit cost is substantial — requires changes to every element type to carry semantic data.

**Buiy mapping:** Foundation §2.6 (AccessKit-first) is the inverse commitment. Validated as the right bet by GPUI's negative example.

## Open problem 2: Mobile targets

Tracking issues [#43206 (iOS)](https://github.com/zed-industries/zed/issues/43206) and [#43207 (Android)](https://github.com/zed-industries/zed/issues/43207) are open with no committed work. GPUI's platform abstraction is deeply tied to macOS/Linux/Windows native APIs; reaching mobile would require additional backends (Metal for iOS is shared, but UIKit windowing and iOS touch input are different from macOS Cocoa).

**Buiy mapping:** Bevy supports iOS and Android already. Buiy inherits this. Foundation README open question 8 ("Platform support staging — all platforms at v1, or staged?") is the right place to make this commitment explicit. The Bevy path makes "ship Buiy on Android" much cheaper than reaching there from GPUI.

## Open problem 3: Web (WASM) target

GPUI has no WASM target and no public plans. Zed's design (native windowing, OS-specific text shaping, no DOM dependency) makes WASM particularly hard — there's no Cocoa, no DirectWrite, no FreeType on the web; the entire platform abstraction would need rewriting.

**Buiy mapping:** Bevy supports WASM. Buiy could in principle inherit. Foundation README open question 16 ("Bevy WASM target policy") is the explicit deferral. Web a11y waits for AccessKit's web adapter (also deferred upstream); visual/input/layout work today on WASM through Bevy.

## Open problem 4: Third-party adoption ceiling

The empirical state: Zed + Longbridge are the only two named production apps. The ~101k crates.io downloads suggest experimental adopters in the hundreds, not thousands. The community-fork (`gpui-ce`) has single-maintainer-scale activity.

The structural reasons GPUI struggles to adopt:

- No widgets (`gpui-component` is the workaround, but it's a separate dependency outside Zed's blessing)
- No accessibility (rules out enterprise / government / education)
- Pre-1.0 (rules out risk-averse adopters)
- macOS-first parity (rules out Linux-or-Windows-primary adopters)
- Apache-only license (creates friction for MIT-or-permissive projects)
- February 2026 community-deprioritization (signals "not coming")

**Buiy mapping:** This is a useful counter-checklist. Buiy must avoid each of these adoption barriers if it wants ecosystem traction. Foundation commits to: widget catalog (§2.3), accessibility-first (§2.6), Bevy's quarterly migration cadence (§2.9), wgpu-uniform cross-platform (§2.2), dual MIT/Apache, and explicit ecosystem-friendliness (the BSN-friendly component constraints are pro-ecosystem-tooling).

## Open problem 5: Monorepo vs crates.io divergence

Three crates.io publishes in 18 months. The crate trails `main` substantially. No maintenance branch. Examples outdated.

**Buiy mapping:** If Buiy publishes to crates.io as a separate crate, the same problem applies. The foundation §2.8 sub-spec on crate organization needs to address publish strategy: either (a) Buiy ships as published crates with real release discipline, or (b) Buiy is consumed by vendoring from the workspace, not via crates.io. Either is workable; the failure mode is doing neither cleanly (which is GPUI's current state).

## Open problem 6: The `gpui-ce` fork sustainability

The community-fork pattern works in principle but requires sustained maintainer attention. `gpui-ce` has single-digit merged PRs and a founder who's contemplating starting fresh instead. The fork could:

- (a) Find a sustainable maintainer rhythm and become the de facto community version
- (b) Fade with low activity and be effectively abandoned
- (c) Be supplanted by a "from-scratch" replacement (the founder's own hint)

Outcome (b) is the most likely without external coordination. Outcome (c) would mean the community starts over from primitives — Bevy's UI ecosystem, Iced, Xilem/Masonry, or something new. This is **the gap Buiy could partially fill** if Buiy's renderer pipeline reaches GPUI-quality polish.

**Buiy mapping:** Not a direct decision input, but a strategic data point. If Buiy succeeds at "GPU-accelerated retained-mode app UI in Rust + accessibility-first + Bevy ecosystem," it occupies a slot the broader community currently has no good answer for.

## Open problem 7: Documentation and learning curve

GPUI's "read the Zed source" model is sustainable only because Zed is open source and very actively read. For broader adoption, this doesn't scale — third-party adopters can't reverse-engineer architecture decisions by reading editor code.

**Buiy mapping:** Foundation `verification.md` commits to per-widget keyboard contract docs, per-WCAG-SC fixtures, per-token theme docs. The discipline of "every claim is testable" produces documentation as a side effect of testing infrastructure.

## Open problem 8: GPU memory and integrated graphics

GPUI's atlas + batched-instance model is GPU-memory-intensive. On integrated graphics (Intel UHD, AMD Vega, NVIDIA Optimus laptops), Zed has had issues with `NoSupportedDeviceFound` ([discussion 28265](https://github.com/zed-industries/zed/discussions/28265)) and PRIME-related workarounds ([PR 23438](https://github.com/zed-industries/zed/pull/23438)).

**Buiy mapping:** Bevy + wgpu has its own integrated-graphics story (similar issues at the wgpu adapter level). Foundation `verification.md` calls out the platform matrix; integrated-graphics testing should be on the CI platform list. GPUI's negative experience is the reminder to test on weak hardware.

## Sources

- GPUI README (API stability disclaimer): https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md
- Examples outdated issue #46183: https://github.com/zed-industries/zed/issues/46183
- Accessibility discussion #6576: https://github.com/zed-industries/zed/discussions/6576
- iOS tracking #43206: https://github.com/zed-industries/zed/issues/43206
- Android tracking #43207: https://github.com/zed-industries/zed/issues/43207
- Integrated graphics discussion #28265: https://github.com/zed-industries/zed/discussions/28265
- NVIDIA PRIME PR #23438: https://github.com/zed-industries/zed/pull/23438
- HN: deprioritization announcement: https://news.ycombinator.com/item?id=47003569
- HN: gpui-ce founder reflection: https://news.ycombinator.com/item?id=47005761
- Cross-link: Buiy foundation: [`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/)
