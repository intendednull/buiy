**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — distribution, governance, license, platform support, stewardship

This file combines `distribution` and `governance` as the per-amble allowed (compressed-folder mode).

## License

**MIT** (single license, not the Apache-2.0 dual-license common in Rust UI). This is **a correction** to the pre-amble (which already said MIT) — confirmed via direct inspection of `LICENSE` on `main`. The single-MIT choice matches Lapce itself (also MIT) but differs from the Linebender norm (Apache-2.0 OR MIT dual). For Buiy users, the practical implication is: Buiy's intended Apache-2.0 OR MIT dual licensing is compatible with depending on Floem, but a Floem dependency would prevent licensing Buiy as Apache-2.0-only if that were ever desired.

## Crates.io distribution

| Field | Value |
|---|---|
| Crate | `floem` (and sister crates `floem_reactive`, `floem_vello_renderer`, `floem_vger_renderer`, `floem_skia_renderer`, `floem_winit`) |
| Latest published | 0.2.0 (2024-11-15) |
| Total versions | 3 |
| Total crates.io downloads | ~15,352 (low for the age — see context below) |
| Owners | Lapce-team members |

For context: 15,352 downloads across 4 years and three versions is in the same range as small experimental Rust UI crates (well below egui's millions, below Iced's hundreds of thousands). The download count is consistent with the "primarily consumed by Lapce as a git dependency" pathway — crates.io is not Floem's primary distribution channel.

## Git-dependency consumption

The standard Floem-on-`main` consumption pattern is:

```toml
[dependencies]
floem = { git = "https://github.com/lapce/floem", rev = "..." }
```

This is the path Lapce takes, the path the Floem examples take, and the path the few external Floem-using projects (e.g., `floem-ui-kit`, `understory`) take. The crates.io 0.2.0 release is effectively a snapshot for users who want a stable pin but accept the 17-month staleness.

## Workspace layout

Floem ships as a Cargo workspace with multiple member crates:

- `floem` — the public meta-crate.
- `floem_reactive` — the signal runtime, independently usable.
- `floem_vger_renderer` — vger (Floem's own GPU renderer) backend.
- `floem_vello_renderer` — Vello backend (optional).
- `floem_skia_renderer` — Skia backend (optional).
- `floem_winit` — the custom winit fork's published mirror.
- `understory_*` — sister crates (focus tree, box-tree) shared with Lapce.

The split mirrors what Buiy plans for `buiy_core` / `buiy_text` / `buiy_widgets` etc. (foundation `architecture.md` §2.8). Floem's split is renderer-driven (one crate per backend); Buiy's plan is subsystem-driven (one crate per layer). Both are valid.

## Platform support

| Platform | Status |
|---|---|
| Windows | Supported |
| macOS | Supported |
| Linux (X11 + Wayland) | Supported. Wayland surface recovery added 2026-05 via PR #1074. |
| iOS | Not supported |
| Android | Not supported |
| Web (WASM) | **Experimental** as of 0.2.0; an `examples/webgpu` exists. |

The mobile gap is significant. No iOS, no Android. For Buiy's "game and app, both" foundation goal (foundation `README.md` §1 goal #6), Floem is not a precedent for mobile-class Rust UI.

## Governance

| Dimension | Reality |
|---|---|
| Foundation membership | None |
| Corporate steward | None directly funding the project |
| Maintainers | Lapce-team members (volunteer / part-time) |
| Full-time devs | **Zero**, per Dongdong Zhou's HN comment Feb 2024 |
| Bus factor | Effectively 1–2 (Dongdong Zhou + a small cluster of regular contributors) |
| Funding model | Unknown — Lapce has had donation channels historically; no clear sustaining funding |
| Decision-making | De facto BDFL with PR-merge gate by Lapce-team |
| Release authority | Lapce-team |

The "no full-time devs" data point (HN, Feb 2024) is the single most important governance fact about Floem. It explains the release cadence, the AccessKit gap (#8 unstaffed three years), the documentation gaps, and the single-flagship dogfooding pattern.

For Buiy: this is the **scariest** pattern in the prior-art folder. Floem ships in a real editor with users; the code works; but the *process maturity* (release cadence, accessibility roadmap, documentation, ecosystem cultivation) is not at production-library level. Buiy's foundation must avoid this trap if Buiy is to serve users beyond its own first flagship.

## Roadmap visibility

The Floem repo has:

- 71 open issues, 22 open PRs (as of verification).
- No `ROADMAP.md`.
- No milestones beyond release tags.
- 0.2.0 release notes describe historical work but do not commit to a 0.3.0 scope or timeline.

A Buiy designer trying to plan against Floem cannot. The roadmap is implicit in Lapce's needs.

## Community

- GitHub discussions: active enough for routine questions to get answers.
- Discord/Matrix: Lapce's chat channels carry Floem discussion; no separate Floem-only chat.
- Documentation: docs.rs API docs only; no book, no extended tutorials beyond the 27 examples.

The 27 examples are the de facto documentation. For a library this complex, that's thin.

## Sources

- Floem `LICENSE` — https://github.com/lapce/floem/blob/main/LICENSE
- Floem on crates.io — https://crates.io/crates/floem
- Floem releases — https://github.com/lapce/floem/releases
- HN comment "no full-time devs" — https://news.ycombinator.com/item?id=39423493
- Cargo workspace layout — https://github.com/lapce/floem/blob/main/Cargo.toml
- PR #1074 Wayland — https://github.com/lapce/floem/pull/1074
