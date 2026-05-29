**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — honest critiques: DSL learning curve, small adoption, mobile-first feature shaping, integration friction, AI-replaces-a11y as a Buiy red line

# Critiques

The honest "what's wrong, what to flag, what would block adoption" view. Sourced from public discussion, issue tracker, and structural reasoning. Where critiques are speculative or single-sourced, the source is named.

## §1 — DSL learning curve

Live is a new language. The syntax is not JSON, not Rust, not QML, not CSS. The `Foo = {{FooStruct}} { ... }` double-brace, the `<View>` instantiation syntax, the inline GLSL `fn pixel(self) -> vec4 { ... }` blocks, the `animator` declarative state machine, the `instance` keyword for shader attributes — all of these are Makepad-specific conventions a new developer must internalize.

The cost is not the language itself (it's well-designed in isolation) but the **interaction with Rust**:

- Type errors in expanded Live blocks surface as macro-expanded compile errors against generated Rust. Stack traces refer to lines of generated code, not the Live source.
- Renames cross the language boundary manually (rename `text` on a Rust `#[live]` field, search-and-replace every `text:` in `.live` syntax).
- IDE features (autocomplete, go-to-definition, refactor) work in Makepad Studio. Outside Makepad Studio, the developer gets syntax highlighting at best.

Compare with Slint: the same issue exists (Slint's `slint!` macro has the same compile-error indirection), but Slint's editor-agnostic LSP (`slint-lsp`) is a significantly better consolation. Makepad's Studio lock-in is worse.

## §2 — Small adoption despite 1.0

16,974 lifetime downloads. 6.4k GitHub stars. ~6 years of development. Two production-grade applications (Makepad Studio + Robrix-alpha). The numbers don't justify a "broadly adopted Rust UI library" framing.

Possible causes (in rough order of likelihood):

1. **Documentation gap** (5.92% / 0% / 0% per major crate, no static site, no reference manual, no examples-and-explanation tutorials).
2. **Accessibility-absence-blocking-procurement** (any organization with even soft a11y requirements cannot adopt).
3. **Editor lock-in** (Makepad Studio dependency drives developers to Slint / Dioxus / egui alternatives).
4. **DSL learning curve** (combined with documentation gap, the on-ramp is steep).
5. **Standalone-framework positioning** (not Bevy-native, not familiar React-equivalent, not Qt-port — niche).

This is **not** a fundamental criticism of the technology. Makepad's GPU rendering, hot-reload, mobile shipping, and shader-as-first-class-authoring are technically impressive. But the surface that's exposed to new developers (docs, tooling, editor support, accessibility procurement) is the bottleneck, not the runtime.

## §3 — Mobile-first feature scope shapes desktop concerns

Makepad's strongest narrative is mobile (Robrix on iOS / Android). The corollary: desktop-grade UX expectations that don't matter on mobile are de-prioritized.

What lags:

- **System tray / status-bar integration** — not first-class.
- **Drag-and-drop across windows** — limited.
- **Real OS-modal dialogs** — popups render in-tree, not as separate OS windows.
- **Desktop-keyboard-shortcut conventions** — global mnemonics, accelerator keys, OS-conformant menu-bar behavior.
- **Multi-monitor / HiDPI per-display** — present but example coverage is thin.
- **Spell-check OS integration** — not modeled.
- **Native clipboard with rich formats (HTML, RTF, image)** — `Cx`'s clipboard API is plaintext-focused per scanning.

For comparison, Slint's "Making Slint Desktop-Ready" blog post explicitly enumerates rich-text, modal-windows, system-tray, drag-and-drop, real-window-popups, keyboard-shortcuts, two-way bindings, and a clipboard model as work-in-progress. Makepad is in roughly the same place but without the same self-aware enumeration.

Buiy's web-platform-parity goal puts all of the above in scope as F (foundation) or C (core) — Buiy must solve them where Makepad has not. See [interaction.md](../../specs/2026-05-07-buiy-foundation/interaction.md) and [cross-cutting.md](../../specs/2026-05-07-buiy-foundation/cross-cutting.md).

## §4 — `.live`-vs-Rust integration friction

Specific friction modes:

- **Two-language stack traces.** A property type mismatch in Live syntax causes a macro-expansion error referencing generated Rust. The developer must mentally map back to the offending Live source line.
- **Refactor across the boundary is manual.** No bridging LSP refactors a Rust field rename through the matching `.live` property names.
- **Build-system surprises.** `.live` source changes trigger a recompile through the `live_design!` macro's input dependence. Build-cache invalidation rules are not obvious; large `.live` files can cause long incremental builds.
- **Hot-reload limitations.** Live-syntax-only changes hot-reload; Rust changes require a recompile-and-restart. The boundary between the two is sometimes opaque (does this `Color` constant change require a rebuild? what about a new `#[live]` field?). The runtime's hot-reload errors are pragmatic but not exhaustive.

Buiy's BSN-as-Rust-syntax choice avoids most of this — BSN compiles via standard `rust-analyzer` paths; refactors propagate through normal Rust mechanisms. See [`live-language.md`](live-language.md).

## §5 — Live is yet-another-DSL

Already covered in [`open-problems.md`](open-problems.md). The marginal cost of "one more DSL the ecosystem has to learn" is real, even for individually-well-designed DSLs. Slint adds `.slint`; Makepad adds `.live`; Dioxus adds RSX (Rust-syntax-adjacent, less costly); SwiftUI adds its own; web has HTML+CSS+JS. Each DSL has its own learning curve, editor support story, documentation requirements, refactor tooling needs.

Buiy's choice (stay in Rust syntax via BSN macro / `.bsn` Rust-compatible files) is **a pragmatic ecosystem choice as much as a technical one**: don't ask Rust developers to learn another language.

## §6 — AI-replaces-a11y is a Buiy red line

The single most-quoted maintainer position in this corpus is the issue-#196 paraphrase:

> Rik mentioned that for Accessibility most likely AI would soon do the heavy lifting for us.

This is **not a critique of Makepad's technology** (the runtime / DSL / GPU work is unaffected). It is a critique of a **product-philosophy position** that Buiy must explicitly reject. Reasons:

- **AI cannot retrofit a missing accessibility tree.** Screen readers consume `Tree` + `Node` data from a producer. There's no inference step that materializes a tree from pixel-level GPU output.
- **WCAG / EN 301 549 / ADA / EAA compliance requires producer-side semantics**, not post-hoc inference. Auditors check the producer's reported structure, not what an AI thinks the structure means.
- **The accessibility-tree-as-source-of-truth pattern is shipped at Apple, Google, Microsoft, Mozilla, KDE, GNOME** — every major UI platform. The position that this is "going to be solved by AI later" runs counter to the entire industry's working consensus.
- **AT users themselves have rejected the framing.** The community member who opened #196 cites visually impaired Fediverse users worried about this exact posture.

Buiy must say, in any positioning statement, that **accessibility is producer-side infrastructure shipped at the framework level**. Not delegated to AI. Not delegated to authors. Not deferred. This is the cleanest place where Buiy's accessibility-first commitment is *responsive to a specific named position in the prior-art ecosystem* — not a generic "we care about a11y" claim. See [`lessons.md`](lessons.md) Avoid #1.

## §7 — Standalone-framework positioning forecloses Bevy integration

Makepad is not a Bevy UI library. It cannot be used as a UI layer for a Bevy app — they would each own a window, an event loop, a rendering surface. The standalone-framework choice was deliberate (see [`history.md`](history.md)) but it means Buiy designers cannot evaluate Makepad as a "could we use this as part of our Bevy stack?" candidate.

This is not a critique of Makepad — it's a critique of the **scope of comparability** for Buiy. The lessons are about *architectural choices* (DSL above runtime, hot-reload, mobile-first, GPU pipeline), not about *integration opportunities*. See [`lessons.md`](lessons.md).

## §8 — Bus factor and funding opacity

Three core architects. No foundation. Unclear funding sources (self-funded? consulting? Futurewei via Robius?). The project has shipped at 1.0 but its long-term sustainability depends on continued founder commitment. If Rik Arends or either of the co-architects stepped back tomorrow, the impact on Makepad's pace would be material. See [`distribution-and-governance.md`](distribution-and-governance.md).

Robius's Futurewei funding adds a separate concentration: if Futurewei policy on Robius changes, the most-visible Makepad app (Robrix) loses its primary maintainer's paid time.

For Buiy, this is a *cautionary* observation — Bevy-Foundation-style governance with multiple paid maintainers is a structurally lower-risk shape. See [`lessons.md`](lessons.md) Avoid #4.

## §9 — Documentation as procurement gate

Reiterating from [`open-problems.md`](open-problems.md) because it's a critique, not just a gap. The 5.92% docs-coverage figure on `makepad-widgets` 1.0.0 — the *flagship* crate at a 1.0 release — is **below the threshold for serious adoption by enterprise / public-sector buyers**. Many procurement processes ask "does the library have a reference manual?" as a checkbox. Makepad fails this checkbox.

Bevy's `bevyengine.org/learn/`, Slint's `docs.slint.dev`, even Iced's hosted book — these are not just nice-to-have surfaces. They are how a developer evaluating "should I bet our product on this UI library?" forms confidence. Makepad's lack of equivalent material is a structural adoption blocker independent of the technical quality.

Buiy must commit to documentation as a deliverable, not an afterthought. The Buiy foundation spec implicitly assumes this; the implementation phases should explicit-budget docs work.

## Implications for Buiy

Each critique above is a Buiy planning input:

- **§1 (DSL learning curve), §4 (integration friction), §5 (yet-another-DSL).** → BSN-as-Rust-syntax. ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md))
- **§2 (small adoption), §9 (docs as procurement gate).** → Buiy implementation phasing must include docs as deliverables, not afterthoughts.
- **§3 (mobile-first shapes desktop).** → Buiy's web-platform-parity target keeps desktop and mobile in equal scope.
- **§6 (AI-replaces-a11y).** → Buiy must publicly state accessibility is producer-side infrastructure, not deferred AI work. The clearest single-sentence positioning Buiy can make against the prior-art ecosystem.
- **§7 (standalone-framework forecloses Bevy integration).** → Buiy is parallel-to-bevy_ui, Bevy-native; this is structural.
- **§8 (bus factor and funding).** → Bevy-Foundation-adjacent governance. Multiple paid maintainers over time.

## Sources

- Issue #196: https://github.com/makepad/makepad/issues/196 (the quoted maintainer position)
- Slint "Making Slint Desktop-Ready" blog (comparable desktop-gap analysis): https://slint.dev/blog/making-slint-desktop-ready
- docs.rs coverage: https://docs.rs/makepad-widgets/1.0.0/
- Crates.io download counts: https://crates.io/api/v1/crates/makepad-widgets
- Robius Futurewei funding: https://github.com/project-robius
- Sibling files: [`open-problems.md`](open-problems.md), [`live-language.md`](live-language.md), [`gpu-rendering.md`](gpu-rendering.md), [`distribution-and-governance.md`](distribution-and-governance.md), [`lessons.md`](lessons.md)
- Slint comparison: [`../slint/open-problems.md`](../slint/open-problems.md), [`../slint/lessons.md`](../slint/lessons.md)
- AccessKit context: [`../accesskit/lessons.md`](../accesskit/lessons.md)
