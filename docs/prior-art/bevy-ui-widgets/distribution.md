**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Distribution, license, governance, area SMEs

# Distribution & governance

## Crate metadata

| | |
|---|---|
| Crate name | `bevy_ui_widgets` |
| Repository | https://github.com/bevyengine/bevy (monorepo; folder `crates/bevy_ui_widgets/`) |
| License | **MIT OR Apache-2.0** (workspace-default, same as all other Bevy crates) |
| MSRV | Tracks Bevy's MSRV (no explicit `rust-version` field in `Cargo.toml`) |
| Edition | 2024 |
| Total downloads | 201,008 (crates.io, lifetime, as of 2026-05-22) |
| Recent downloads | 177,568 (90 days) — most of lifetime, reflecting the recent introduction |
| Cargo features | None as of 0.19.0-rc.2 (the previous `experimental` feature was removed in PR #22934) |
| Reverse dependencies | 3 on crates.io (mostly `bevy_feathers` + small Bevy ecosystem) — *low; the crate is too new for broad downstream adoption* |

## Release cadence

Lockstep with Bevy minor releases. Bevy minor cadence is ~quarterly (slipping toward ~3.5 months recently); each Bevy minor publishes a matching `bevy_ui_widgets 0.X.0` plus patch releases through the rest of the cycle.

| bevy_ui_widgets | Bevy | Date | Notes |
|---|---|---|---|
| 0.17.0-rc.1 | 0.17.0-rc.1 | 2025-09-12 | first publish (cart) |
| 0.17.0-rc.2 | 0.17.0-rc.2 | 2025-09-21 | mockersf |
| **0.17.0** | **0.17.0** | **2025-09-30** | first stable; ships 5 widgets (cart) |
| 0.17.1 | 0.17.1 | 2025-10-01 | mockersf |
| 0.17.2 | 0.17.2 | 2025-10-04 | mockersf |
| 0.17.3 | 0.17.3 | 2025-11-17 | scrollbar fix (mockersf) |
| 0.18.0-rc.1 | 0.18.0-rc.1 | 2025-12-17 | mockersf |
| 0.18.0-rc.2 | 0.18.0-rc.2 | 2025-12-30 | mockersf |
| **0.18.0** | **0.18.0** | **2026-01-13** | adds Menu + Popover (cart) |
| **0.18.1** | **0.18.1** | **2026-03-04** | current stable (alice-i-cecile) |
| 0.19.0-rc.1 | 0.19.0-rc.1 | 2026-05-13 | + text_input (mockersf) |
| 0.19.0-rc.2 | 0.19.0-rc.2 | 2026-05-22 | today (mockersf) |

**Note on dates:** the brief stated "Latest stable: 0.18.1 (2026-05-13)." Per the crates.io API: 0.18.1 was published **2026-03-04**, not 2026-05-13. 2026-05-13 is the publish date of 0.19.0-rc.1.

## Area SMEs

Subject-matter experts per recent commits + merged PRs touching `crates/bevy_ui_widgets/`:

| Name | GitHub | Role | Contributions |
|---|---|---|---|
| **viridia** (Mike "Talin" Schlossman) | [@viridia](https://github.com/viridia) | Widget area lead; original designer | Discussion #16900 author; PR #20944 (rename); designed Menu, Popover, Slider, Radio (per Bevy 0.17/0.18 release-note credits) |
| **alice-i-cecile** | [@alice-i-cecile](https://github.com/alice-i-cecile) | Bevy maintainer; widget reviewer | Discussion #16900 extensive feedback; PR #20972 (mark experimental); PR #22934 (remove experimental flag); published 0.18.1 |
| **ickshonpe** | [@ickshonpe](https://github.com/ickshonpe) | UI / text area co-maintainer | Co-credited on bevy_ui_widgets in 0.17 release notes; deep bevy_ui + bevy_text expertise |
| **PPakalns** | [@PPakalns](https://github.com/PPakalns) | Frequent widget-area contributor | Co-credited on Menu + Popover (0.18); PR #21835 (scrollbar fix) |
| **mockersf** (François Mockers) | [@mockersf](https://github.com/mockersf) | Release manager | Publishes most rc/patch releases (crates.io owner) |
| **cart** (Carter Anderson) | [@cart](https://github.com/cart) | Bevy BDFL | Publishes major releases (0.17.0, 0.18.0); crates.io owner |
| **DuckyBlender** | [@DuckyBlender](https://github.com/DuckyBlender) | Contributor | PR #21827 (vertical slider, 0.18) |
| **fallible-algebra** | [@fallible-algebra](https://github.com/fallible-algebra) | Contributor | PR #23924 (FromTemplate derives, 0.19) |
| **Atlas16A** | [@Atlas16A](https://github.com/Atlas16A) | Feathers contributor | Bevy 0.17 credit (on feathers side) |
| **amedoeyes** | [@amedoeyes](https://github.com/amedoeyes) | Feathers contributor | Bevy 0.17 credit (on feathers side) |

The widget area centroid is **viridia + alice-i-cecile + ickshonpe + PPakalns**. New widget proposals and major redesigns typically flow through viridia; merges go through alice-i-cecile.

## Crate ownership (crates.io)

Three owners with publish rights:

- `mockersf` — release manager
- `cart` — Bevy BDFL
- `github:bevyengine:publish` (team) — the org-level publish team

## Bevy Foundation context

`bevy_ui_widgets` is published under the broader Bevy project, which is supported by **Bevy Foundation** — the U.S. 501(c)(3) non-profit incorporated in 2024 with cart as president. Foundation governance is described at https://bevy.org/foundation/. Foundation-side directional funding has historically gone to: cart (engine), alice-i-cecile (ECS), viridia (UI work — including bevy_ui_widgets + bevy_feathers), pcwalton (rendering), and others through targeted contracts.

`bevy_ui_widgets` and `bevy_feathers` are part of the **editor effort** — viridia's stated motivation for both crates is the in-development Bevy editor (a Bevy app needing a styled GUI). The headless crate is the by-product needed by the editor, exposed to apps as a useful primitive.

## RFC / design process

Bevy uses a lightweight design process: GitHub Discussions for proposals (e.g. #16900), draft PRs for prototype merges, HackMD documents for specs, Discord channels for chatter. There is no formal RFC repository like Rust's or like the W3C process. The 22-month BSN saga (discussion #14437 → PR #20158 still draft as of 2026-05-22, per [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)) is the canonical demonstration of the cost.

For bevy_ui_widgets specifically:
- Discussion #16900 (2024-12-19) → first ship 2025-09-30 = ~9 months from proposal to release.
- The discussion remains open; ongoing widget proposals (Tabs, Dialog, Tooltip, Combobox) are added as comments + spin-off discussions.
- No version-locked design doc exists for the widget set; the source code + lib.rs prose are the closest thing.

## Implications for Buiy

- **MSRV / Bevy lockstep is non-negotiable.** Buiy's foundation policy ([architecture.md § 2.9](../../specs/2026-05-07-buiy-foundation/architecture.md)) is "rolling latest-stable Bevy, no back-compat across Bevy minors." This matches bevy_ui_widgets's posture; Buiy gets no advantage by pinning older Bevy. Buiy's release notes should pin the Bevy version, the AccessKit version, and the cosmic-text version per release explicitly.
- **The viridia + alice-i-cecile axis is also the bevy_ui review axis** ([per bevy-ui prior-art governance](../bevy-ui/governance.md)). Buiy's PRs into Bevy core (if any — most Buiy work is in `bevy-y/buiy/` not upstream) would route through the same reviewers; relationships across both crates' reviews compound.
- **The "no formal RFC, design in discussions + draft PRs" approach is Bevy-cultural and unavoidable.** Buiy's foundation guidelines ([CLAUDE.md](../../../CLAUDE.md)) explicitly take the opposite position: canonical doc log in `docs/specs/` + `docs/plans/`, no design state in Discord. This is a process bet Buiy is making against the upstream cultural default; the cost of doing so is making sure every Buiy spec is self-contained enough to land without Discord context.

## Sources

- crates.io API — https://crates.io/api/v1/crates/bevy_ui_widgets (fetched 2026-05-22)
- crates.io owners — https://crates.io/api/v1/crates/bevy_ui_widgets/owners
- Bevy 0.17 / 0.18 release announcements — https://bevy.org/news/
- Bevy Foundation — https://bevy.org/foundation/
- Discussion #16900 — https://github.com/bevyengine/bevy/discussions/16900
- Sibling: [`history.md`](history.md), [`open-problems.md`](open-problems.md), [`../bevy-ui/governance.md`](../bevy-ui/governance.md)
