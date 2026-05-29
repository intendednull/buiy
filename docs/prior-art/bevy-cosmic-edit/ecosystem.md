**Date:** 2026-05-22
**Status:** archived
**Subject:** bevy_cosmic_edit — peak production usage, downstream consumers, fork landscape, comparisons with alternatives during its active years.

# Ecosystem and comparisons

## Peak production usage

bevy_cosmic_edit reached ~40,832 lifetime crates.io downloads and 110 GitHub stars. Modest by upstream standards but meaningful inside the Bevy ecosystem, where most third-party UI crates sit under 10k downloads. Known production users (verifiable):

- **`velo`** — Dimchikkk's own brainstorming app (Rust, 339 stars at archive, also archived). The original dogfood substrate. velo was where bevy_cosmic_edit's design pressure came from; many features (placeholder, password, multi-line edit) trace to velo's needs. Both archived in 2025.
- **`revel`** — Dimchikkk's note-taking app (C — *not* Rust; likely uses something else now). Mentioned in [`history.md`](history.md) as a successor project; the move to C suggests the maintainer's interest shifted away from the Rust/Bevy stack.
- **Various hobby Bevy projects** — itch.io game jams, exploratory tools. No flagship commercial Bevy title is verified as a bevy_cosmic_edit user.

The bus-factor story is grim: when the dogfood app (velo) and the maintainer's primary investment moved off Bevy, the crate's pull-driver disappeared. This is the same pattern as cosmic-text's COSMIC Desktop dogfood relationship (see [`../cosmic-text/governance.md`](../cosmic-text/governance.md)), except in *reverse* — cosmic-text *gained* a stable dogfood substrate; bevy_cosmic_edit *lost* its.

## Fork landscape

At archive (2025-03-21) there were 14 forks. As of May 2026:

- No fork has emerged as the canonical successor.
- Several forks appear in `bevy_cosmic_edit`'s GitHub network graph from 2024-2025 — most are personal experiments or fix-this-one-thing branches, not maintenance forks.
- No fork is publishing to crates.io under an alternative name.
- No fork is keeping pace with Bevy 0.16+.

The community-organization signal is absent. If a fork were to become canonical, it would need: (1) a maintainer with bandwidth for the two-upstream load described in [`why-archived.md`](why-archived.md); (2) a dogfood substrate to drive pressure; (3) ideally, a funding source. None of those existed at archive and none have emerged since.

## Comparisons during its active years (2023–2025)

These are the alternatives a Bevy app developer was choosing between when bevy_cosmic_edit was alive. Comparing them clarifies what bevy_cosmic_edit was filling — and what filled the gap after it left.

### vs bevy_ui's own `Text` widget

| Capability | bevy_cosmic_edit (0.26) | bevy_ui `Text` (Bevy 0.15) |
|---|---|---|
| Display text | yes | yes |
| Multi-run styling | yes (`set_rich_text`) | partial (`TextSpan`) |
| Editable | **yes** | no (display-only) |
| Caret + selection | yes | no |
| IME pass-through | yes (no preedit render) | no |
| Clipboard | yes (`arboard`) | no |
| Glyph atlas integration | no (separate pipeline) | yes |
| Multi-line | yes | yes |
| BiDi | yes (via cosmic-text) | yes (via cosmic-text-via-bevy_text) |

The killer differentiator was **"editable, yes."** bevy_ui's `Text` was display-only through every release bevy_cosmic_edit targeted; bevy_cosmic_edit existed to be the editing layer.

The trade was the parallel render pipeline — cleaner glyph-atlas semantics on bevy_ui's side, but no editing.

### vs cosmic-text directly (no Bevy integration)

| Capability | bevy_cosmic_edit | cosmic-text directly |
|---|---|---|
| Bevy component API | yes | no (consumer wires up) |
| Render-to-Bevy-texture | yes | no (consumer rasterizes + uploads) |
| Input routing | yes | no (consumer translates winit events) |
| Focus model | yes (`FocusedWidget`) | no |
| Clipboard | yes | no |
| IME | partial | partial (cosmic-text leaves IME to embedder) |

Consumers who didn't want bevy_cosmic_edit had to re-implement the input-event-to-`Action` translation, the rasterize-to-texture pipeline, and the focus model themselves. For ~80% of use cases, bevy_cosmic_edit's defaults were what consumers would have written.

This is the load-bearing observation for Buiy: the **non-trivial value of the bridge was the input + render + focus glue**, not anything about cosmic-text. When Buiy owns text-edit, it re-implements that glue once, against Buiy's component model, and avoids the bridge-crate trap.

### vs egui's text editing

| Capability | bevy_cosmic_edit | egui (in bevy_egui) |
|---|---|---|
| Component model | Bevy ECS | egui immediate-mode |
| Multi-line edit | yes | yes |
| Caret + selection | yes | yes |
| Multi-script shaping | yes (cosmic-text Advanced) | limited (egui's own shaper, no Arabic / Indic) |
| IME | partial | yes (egui handles preedit) |
| BiDi | yes | partial |
| Theming integration | Bevy-native | egui-only |
| Editor extensibility | "yes" (re-add via plugins) | "yes" (TextEdit memory) |

egui's text editor is more **complete** for Latin / European scripts and ships preedit rendering. It's weaker on complex-script shaping (no harfbuzz/harfrust). Different bet: egui's text edit is a single-paradigm thing, where bevy_cosmic_edit was infrastructure plus styling primitives. Neither survived as the canonical Bevy text-edit answer — egui-in-Bevy survives, but it's not Bevy-ECS-native.

### vs Iced's text editor

Iced uses cosmic-text directly (since Iced 0.13). Iced is a separate runtime; comparing it head-to-head with bevy_cosmic_edit is comparing two different application architectures. Worth noting only because Iced is the **largest surviving cosmic-text production consumer in the Rust UI space**, and its choice to integrate cosmic-text directly without a bridge crate is the architectural shape Buiy is mirroring. See [`../cosmic-text/integration.md`](../cosmic-text/integration.md).

## Why bevy_cosmic_edit filled a real gap (2023–2024)

When bevy_cosmic_edit shipped in mid-2023:

- bevy_ui's `Text` was display-only and had been for ~3 years.
- cosmic-text was new and had no Bevy adopter.
- The Bevy community had multiple open issues asking for "real text input" (e.g. the eternal "I want a username field" example).
- The 10 Challenges for Bevy UI Frameworks ([issue #11100](https://github.com/bevyengine/bevy/issues/11100), opened 2023-12) included a text-editing challenge no Bevy widget framework had cleanly demonstrated.

bevy_cosmic_edit was the first credible Bevy-native answer. It deserves credit for unblocking ~21 months of Bevy app development. The fact that the architectural shape didn't scale to a 4-year maintenance horizon doesn't retroactively negate the ~21 months of value it delivered.

The Buiy lesson is not "bridge crates are bad ideas." It's **"bridge crates between two fast-moving Rust UI ecosystems don't last; if you're going to depend on the editing surface for years, you need to own it."** See [`lessons.md`](lessons.md).

## Sources

- crates.io download stats — https://crates.io/crates/bevy_cosmic_edit
- velo (Dimchikkk's dogfood app, also archived) — https://github.com/Dimchikkk/velo
- revel (Dimchikkk's successor project in C) — https://github.com/Dimchikkk/revel
- Bevy 10 Challenges (issue #11100) — https://github.com/bevyengine/bevy/issues/11100
- Iced cosmic-text integration — [`../cosmic-text/integration.md`](../cosmic-text/integration.md)
- cosmic-text governance / dogfood — [`../cosmic-text/governance.md`](../cosmic-text/governance.md)
- bevy_egui — https://github.com/vladbat00/bevy_egui
