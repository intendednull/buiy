**Date:** 2026-05-22
**Status:** active
**Subject:** Xilem + Masonry — Linebender's next-generation Rust UI substrate (sibling crates: reactive layer + retained widget toolkit)

# Xilem + Masonry

This folder documents **two crates released as one workspace** by Linebender (the Raph-Levien-led collective behind Vello, Parley, Druid, et al.):

- **Masonry** — retained-mode widget toolkit. Owns the widget tree, runs event/update/layout/paint passes, integrates AccessKit.
- **Xilem** — reactive layer on top of Masonry. View-tree-diffing à la React / SwiftUI / Elm, with Rust-ownership-respecting message routing.

They are **sibling crates in `linebender/xilem`**, co-released, co-versioned (both at 0.4.0 as of 2025-10-29). Xilem is the recommended entry point for application authors; Masonry is the entry point for UI-framework authors who want to build a different reactive paradigm on the same widget substrate. Both are **experimental / pre-1.0** and the README says so explicitly.

This folder is a **compressed single-author deep-dive** covering both crates together because their architecture and lessons are inseparable; splitting them would force most files to cross-reference each other constantly. Sibling Linebender substrate crates (Vello, Parley, Skrifa, Fontique) get their own folders elsewhere (or should, when written) — this folder is the consumer-side dive on Xilem + Masonry specifically.

## Key facts

| Fact | Value | Source |
|---|---|---|
| Latest version (both crates) | 0.4.0 | docs.rs, crates.io |
| Release date | 2025-10-29 | git tag history (note: GitHub release-page rendering shows 2024 in some views; tag list shows 2025) |
| License | Apache-2.0 (single) | workspace `Cargo.toml` |
| Steward | Linebender (volunteer collective; Raph Levien informally leads) | https://linebender.org/about/ |
| Repo | `https://github.com/linebender/xilem` (workspace) | n/a |
| MSRV | Rust 1.92 (workspace HEAD); 1.88 at 0.4.0 release | workspace `Cargo.toml`, release notes |
| Maturity | Experimental, pre-1.0, alpha | README + release notes verbatim |
| xilem downloads | 7,596 lifetime (per pre-amble; verify on crates.io for live numbers) | pre-amble |
| masonry downloads | 17,690 lifetime (per pre-amble) | pre-amble |
| Render substrate | Vello + wgpu | workspace deps |
| Text substrate | Parley + Fontique (+ Skrifa under Parley) | workspace deps |
| Windowing | winit | workspace deps |
| Accessibility | AccessKit (via `accesskit_winit`) | masonry deps, source code |
| Platforms (verified) | Linux, macOS, Windows, BSD desktop; Android in-progress (examples ship as cdylib); web via separate `xilem_web` (DOM, not Masonry/Vello) | README |

**Verified corrections to the pre-amble:**

- The release date 2025-10-29 is correct (verified from git tag `v0.4.0`). GitHub's release-summary rendering elsewhere showed "October 29, 2024" — that is a display quirk; the tag list and crates.io publish date both confirm 2025.
- The pre-amble says "Substrate: Parley (text) + Vello (GPU) + own layout (NOT Taffy)." **Confirmed** — Masonry uses Parley/Vello and rolls its own layout (no Taffy dep in `masonry_core/Cargo.toml`). Each widget's layout is computed inside its `Widget::layout` method against parent-imposed `BoxConstraints`, the Flutter/Druid style. This is a sharp contrast with bevy_ui, woodpecker_ui, Iced, Dioxus, all of which use Taffy.
- The pre-amble says "Linebender stack: Xilem, Masonry, Parley (text), Vello (GPU rendering), Druid (legacy)." **Confirmed and extended** — also: Kurbo (curves), Peniko (paint primitives), Color (color spaces), Skrifa (font data via read-fonts), Fontique (font enumeration), `tree_arena` (Masonry's tree storage), Velato (Lottie animation), Vello SVG, Norad (UFO fonts), Interpoli, Kompari, Piet (legacy abstraction), Runebender (font editor), Skribo (legacy text shaper).
- The pre-amble says "Druid is officially superseded by Xilem." **Confirmed** — Druid README says verbatim *"UNMAINTAINED - The Druid project has been discontinued."* Last release 0.8.3, 2023-02-28. Repository not formally `archived` flag on GitHub but development is dead.
- The pre-amble says "Bevy 0.19-dev migrated to Parley." **Confirmed** by cross-link to [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Top-of-file finding #2 (issue #21765, labeled Blessed). Buiy explicitly diverges and stays on cosmic-text.
- The pre-amble says AccessKit integration needs verification. **Verified** — `masonry_core/Cargo.toml` direct-depends on `accesskit`; widgets implement an `accessibility(&mut self, ctx, node)` method that builds an `accesskit::Node` per widget. Masonry uses `accesskit_winit` for the platform adapter. The accesskit pin in the released 0.4.0 crate is **0.21.1** (per docs.rs metadata); workspace HEAD has bumped to 0.24.0 post-release. This pin lag is the version-cadence-decoupled-from-Bevy story playing out in real time. See [`accessibility.md`](accessibility.md) for the full read.
- The pre-amble's "Lead architects: Raph Levien, others." **Confirmed** — Linebender's about-page calls out Raph as the informal leader; Daniel McNab and Olivier Faure are co-leads on Xilem/Masonry per the "This Month in Xilem" post bylines. Matt Campbell (AccessKit) is also Linebender-adjacent.

## Reading order

If you're consulting this folder for a Buiy design decision:

1. **Start here:** [`lessons.md`](lessons.md) — the consult-this-when-designing file (validates / avoid / borrow).
2. **For text + GPU substrate questions:** [`text-and-rendering.md`](text-and-rendering.md) and [`linebender-stack.md`](linebender-stack.md).
3. **For a11y questions:** [`accessibility.md`](accessibility.md) (cross-link to `../accesskit/lessons.md`).
4. **For reactivity / authoring questions:** [`xilem-architecture.md`](xilem-architecture.md) then [`masonry-toolkit.md`](masonry-toolkit.md).
5. **For history / "why this and not that":** [`history.md`](history.md) (Druid → Masonry → Xilem timeline).
6. **For ecosystem / production status:** [`ecosystem.md` + `comparisons.md`](ecosystem-comparisons.md).
7. **For critiques / open problems:** [`critiques-and-open-problems.md`](critiques-and-open-problems.md).
8. **For governance / licensing:** [`distribution-governance.md`](distribution-governance.md).

## Table of contents

- [`xilem-architecture.md`](xilem-architecture.md) — Xilem's reactive paradigm: views as functions, diffing, view-state, message passing, id paths, Adapt nodes.
- [`masonry-toolkit.md`](masonry-toolkit.md) — Masonry as the underlying widget toolkit: `Widget` trait, retained tree, layout pass, paint pass, the tree-arena storage model.
- [`linebender-stack.md`](linebender-stack.md) — The full Linebender stack: Vello, Parley, Skrifa, Fontique, Kurbo, Peniko, Color, Masonry, Xilem, Druid (legacy). How they compose. Why Linebender unbundled.
- [`text-and-rendering.md`](text-and-rendering.md) — Parley + Skrifa + Fontique + Vello: how the Linebender text+render stack works; comparison to cosmic-text + harfrust + swash (Buiy's choice); the Bevy 0.19-dev migration to Parley.
- [`history.md`](history.md) — Druid (2018+) → "Druid is dead, long live Xilem"; the Xilem paper (May 2022); Masonry split from Druid (2023+); the 2025-10-29 0.4.0 alignment.
- [`accessibility.md`](accessibility.md) — Xilem + Masonry's AccessKit integration: the `Widget::accessibility` method, the `accesskit_winit` adapter wiring, version-pin lag, what Buiy can borrow.
- [`distribution-governance.md`](distribution-governance.md) — Linebender as a volunteer collective; Raph Levien as informal lead; Apache-2.0 single-license (no MIT-OR-Apache dual); release cadence (irregular); funding (informal sponsorship + day jobs); RFC process (Zulip + GitHub).
- [`ecosystem-comparisons.md`](ecosystem-comparisons.md) — Production users (essentially none yet for Xilem/Masonry directly; Vello + Parley have wider adoption); comparison vs Iced, Slint, Dioxus, GPUI, egui, Druid (legacy), Buiy.
- [`critiques-and-open-problems.md`](critiques-and-open-problems.md) — Pre-1.0 maturity, small adoption, Linebender bandwidth split across many projects, "yet another Rust UI" reception; open problems: mobile/WASM, theme system, APG coverage, Vello stability, Parley parity with cosmic-text.
- [`lessons.md`](lessons.md) — **The decision file.** Validates / Avoid / Borrow for Buiy.
- [`glossary.md`](glossary.md) — Xilem, Masonry, Vello, Parley, Skrifa, Fontique, View, ViewSequence, ViewMarker, Pod, Memoize, BoxConstraints, ViewCtx, etc.

## Framing disclosure

This folder was authored in a single agent pass on 2026-05-22, compressed from the seven-stage parallel-agent workflow that produced larger folders ([`bevy-ui/`](../bevy-ui/), [`accesskit/`](../accesskit/)). Compression rationale: Xilem + Masonry, while load-bearing as the closest *substrate-shape* prior art for Buiy's foundation, is *not* a load-bearing dependency for Buiy — Buiy will not depend on either crate. We need the framing, not the version-pinned exhaustive coverage. The compressed shape favors `lessons.md`'s decision content over deep-dives on individual subsystems.

Author perspective: written from Buiy's foundation-spec stance, which is "own renderer + own layout integration + cosmic-text + AccessKit + Bevy ECS as authoring substrate." Buiy and Xilem are sibling experiments in the design space "how do you build a next-generation Rust UI?", with different substrate + ECS commitments. This folder reads Xilem/Masonry as *the closest existing-art reference point* for that design space, not as a competitor to avoid or an upstream to depend on.

When this folder needs refresh: bump the Date header at the top of any file you touch; if any crate ships a 1.0 release, or if Linebender's governance changes (e.g. formal foundation incorporation), promote a full re-review. If Buiy's foundation spec ever adds a signal-style reactivity layer (currently out per [`architecture.md § 2.7`](../../specs/2026-05-07-buiy-foundation/architecture.md)), re-read [`xilem-architecture.md`](xilem-architecture.md) and [`lessons.md`](lessons.md) Borrow #1 first.

## Sources

- Xilem repo: https://github.com/linebender/xilem
- Xilem docs.rs (0.4.0): https://docs.rs/xilem/0.4.0/xilem/
- Masonry docs.rs (0.4.0): https://docs.rs/masonry/0.4.0/masonry/
- Linebender about: https://linebender.org/about/
- Raph Levien's blog: https://raphlinus.github.io/ (Xilem paper May 2022; "Advice for the next dozen Rust GUIs" July 2022; reactive UI series 2019–2020)
- Druid repo (legacy, unmaintained): https://github.com/linebender/druid
- AccessKit prior-art (cross-link): [`../accesskit/lessons.md`](../accesskit/lessons.md)
- woodpecker_ui prior-art (cross-link, fellow Vello user): [`../woodpecker-ui/lessons.md`](../woodpecker-ui/lessons.md)
- bevy_ui prior-art (cross-link, Parley migration): [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- cosmic-text prior-art (cross-link, Buiy's text shaper): [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md)
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) §2.2, §2.7
