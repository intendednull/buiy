**Date:** 2026-05-22
**Status:** active
**Subject:** iced — production users, community widgets, Rust-GUI landscape position

# Ecosystem

This file catalogues who actually ships iced in production, what community libraries surround it, and where it sits in the Rust GUI landscape relative to its peers. Companion to [`comparisons.md`](comparisons.md) (head-to-head) and [`governance.md`](governance.md) (who funds what).

## The flagship at-scale user: COSMIC desktop

[System76](https://system76.com) chose iced as the UI substrate for the [COSMIC desktop environment](https://github.com/pop-os/cosmic-epoch), announced in 2022 and shipped as part of Pop!_OS COSMIC starting in 2024. This is by far iced's largest production deployment.

Quasi-comprehensive list of COSMIC components that build on iced (per the [awesome-iced](https://github.com/iced-rs/awesome-iced) curation as of 2026-05-22):

- **`cosmic-comp`** — the Wayland compositor.
- **`cosmic-applets`** — panel/tray applets.
- **`cosmic-launcher`** — the pop-launcher frontend.
- **`cosmic-settings`** — the system settings app.
- **`cosmic-text-editor`** — the default text editor.
- **`cosmic-files`** — the file manager (not in awesome-iced but on `pop-os/cosmic-files`).
- **`cosmic-terminal`** — the default terminal.

The COSMIC project is the single largest non-Héctor source of iced contributions, especially around theming (the COSMIC team built [`libcosmic`](https://github.com/pop-os/libcosmic) as a higher-level styling layer over iced), Wayland integration, multi-window correctness, and accessibility (which iced upstream still doesn't have — see [`open-problems.md`](open-problems.md)).

System76's bet on iced is the strongest single signal of long-term project viability: their commercial OS depends on it.

## Other production users (verified)

The [`awesome-iced`](https://github.com/iced-rs/awesome-iced) curated list is the canonical inventory. Highlights — apps that have shipped publicly and have non-trivial user bases — are partitioned below. Each is **verified to be in `awesome-iced`** as of 2026-05-22.

**Communication / social:**
- **[Halloy](https://github.com/squidowl/halloy)** — open-source IRC client. Featured in the iced README's two-screenshot showcase. Active development.

**Games & game tools:**
- **[Veloren](https://veloren.net)** — multiplayer voxel RPG written in Rust. Uses iced for in-game menus and HUD overlays. Has shipped to thousands of players.
- **[Airshipper](https://gitlab.com/veloren/airshipper)** — Veloren's official launcher.
- **[ESLauncher2](https://github.com/EndlessSkyCommunity/ESLauncher2)** — Endless Sky launcher.
- **[ajour](https://github.com/casperstorm/ajour)** — World of Warcraft addon manager. Has had ~tens-of-thousands of downloads at peak.
- **[ludusavi](https://github.com/mtkennerly/ludusavi)** — PC game save-data backup tool. Active, multi-platform.
- **[Neothesia](https://github.com/PolyMeilex/Neothesia)** — Synthesia-like piano-roll visualizer.

**Audio / media:**
- **[OctaSine](https://github.com/greatest-ape/OctaSine)** — cross-platform FM synth as a VST2 + CLAP plugin. iced runs inside the DAW's plugin window.

**System utilities:**
- **[Sniffnet](https://github.com/GyulyVGC/sniffnet)** — network-traffic monitoring tool. ~10k+ GitHub stars.
- **[Raspirus](https://github.com/Raspirus/Raspirus)** — rules-based malware scanner.
- **[Furtherance](https://github.com/lakoliu/Furtherance)** — privacy-focused time-tracker.

**Finance / commercial:**
- **[Cryptowatch desktop](https://docs.cryptowat.ch/desktop)** — Kraken's flagship crypto charting application. The Cryptowatch team funds iced development (see [`governance.md`](governance.md)). This is iced's most commercially-significant non-COSMIC user.
- **[liana](https://github.com/wizardsardine/liana)** — Bitcoin wallet with timelocked recovery.
- **[revault-gui](https://github.com/revault/revault-gui)** — Bitcoin vault GUI.

## Brief corrections (against the folder's source brief)

The brief that produced this folder named two production users that turn out to be wrong on direct verification:

- **modrinth-app** — listed in the brief as "modrinth's desktop launcher." **Wrong.** [modrinth/code](https://github.com/modrinth/code) is built with Tauri + Vue: *"It is built with Tauri and Vue."* No iced. Removed from this file.
- **ZebraD2** — listed in the brief as "Zebra (Zcash node) GUI." **No such project exists.** [`github.com/ZcashFoundation/zebra`](https://github.com/ZcashFoundation/zebra) is a CLI-only full-node implementation; a GitHub search for "ZebraD2" returns zero results. Removed.

Honest record: the canonical inventory of iced production users is the `awesome-iced` README. Cite from there, not from secondhand claims.

## Custom widget / extension libraries

The most-active community libraries (verified in `awesome-iced` and on crates.io):

- **[`iced_aw`](https://github.com/iced-rs/iced_aw)** — official-adjacent (iced-rs org) widget-extras crate. Badges, color pickers, date pickers, drop-downs, menu bars, modals, segmented buttons, tab bar. The de-facto "stdlib extension" for iced apps. 415k recent downloads per crates.io reverse-deps.
- **[`iced_audio`](https://github.com/iced-rs/iced_audio)** — VST/LV2-oriented widget set (rotary knobs, sliders, X/Y pads).
- **[`iced_video_player`](https://github.com/jazzfool/iced_video_player)** — GStreamer-backed video widget.
- **[`iced_term`](https://github.com/Harzu/iced_term)** — terminal-emulator widget.
- **[`iced_fonts`](https://github.com/iced-rs/iced_fonts)** — icon-font helpers (Bootstrap Icons, Nerd Fonts). 336k recent downloads.
- **[`plotters-iced`](https://github.com/Joylei/plotters-iced)** — `plotters` rendering backend for iced. 98k recent downloads.
- **[`iced_layershell`](https://github.com/waycrate/iced_layershell)** — wlr-layer-shell binding (Wayland panel/lockscreen apps).
- **[`iced_code_editor`](https://github.com/airstrike/iced_code_editor)** — code-editor widget with syntax highlighting.
- **[`Cosmic Time`](https://github.com/pop-os/cosmic-time)** — animation toolkit by System76 for COSMIC (pre-dates the 0.14 built-in animation API; still actively used).
- **[`bevy_iced`](https://github.com/tasgon/bevy_iced)** — embed iced apps inside Bevy. This is the closest existing cross-link between iced and Buiy's host engine; covered separately in [`comparisons.md`](comparisons.md).

## Place in the Rust GUI landscape

iced is the largest-by-download retained-mode Rust GUI library (1,885,134 lifetime downloads). Compared to the broader landscape:

- **vs `egui`** — egui is *immediate-mode* and the *most-adopted-by-downloads* Rust GUI library overall (Rerun, eframe-based dev tools, many dev/internal-tool screens). egui prioritizes ease and is intentionally minimalist on styling. iced trades that for type-safety and theme richness. See [`comparisons.md`](comparisons.md) for the full row.
- **vs `gpui` (Zed)** — gpui is retained, GPU-first, and not published to crates.io for general use (it lives inside the Zed monorepo). iced is the published-and-supported alternative for "I want what Zed has" but with a public API.
- **vs Linebender (`xilem`, `vello`, `parley`)** — Linebender is the *other* major retained-mode line, with a Microsoft-funded research/production track. Xilem is younger and signal-based. The Linebender stack's text engine (Parley + vello) and iced's (cosmic-text + cryoglyph + wgpu) are *parallel*, both production-grade, neither converging.
- **vs Slint** — Slint targets embedded systems and uses a DSL `.slint` file. iced is Rust-source-only ("no DSL" is in the philosophy chapter).
- **vs Dioxus** — Dioxus is React-flavored signal-reactive and targets desktop + web + mobile via WebView. iced is Elm-flavored and renders natively via wgpu.
- **vs Floem (Lapce)** — Floem is signal-based reactive; uses Parley for text. Different mental model.
- **vs GTK-rs** — GTK is the OS-native option; iced is the GPU-first cross-platform option. They occupy non-overlapping niches.

## Place vs Buiy

iced and Buiy occupy different niches:

- **iced** is a cross-platform desktop GUI library for **standalone-app** development. It owns its window (via winit) and event loop.
- **Buiy** is a UI layer for the **Bevy game engine**. Bevy owns the window and event loop; Buiy plugs into Bevy's render graph and ECS.

The shared substrate is cosmic-text + wgpu + winit (Buiy will use winit transitively via Bevy, not directly). The architectural commitments diverge sharply: iced is Elm-architecture single-Model; Buiy is ECS + BSN.

The closest cross-link is `bevy_iced`, which embeds iced inside Bevy — proof that *both* libraries can coexist in one binary, but the bridge is heavy (two event loops, two layout engines, two text caches). Buiy's foundation [README § Goal 4](../../specs/2026-05-07-buiy-foundation/README.md) explicitly chose the parallel-to-bevy_ui path over a port-iced-to-Bevy path; the cost-benefit was that Buiy needs the ECS + BSN authoring model and the AccessKit-first stance, neither of which iced provides.

See [`comparisons.md`](comparisons.md) for the side-by-side table.

## Sources

- awesome-iced (community list) — https://github.com/iced-rs/awesome-iced
- COSMIC desktop — https://github.com/pop-os/cosmic-epoch
- libcosmic — https://github.com/pop-os/libcosmic
- Cryptowatch desktop — https://docs.cryptowat.ch/desktop
- crates.io reverse-dependencies for iced — https://crates.io/crates/iced/reverse_dependencies
- modrinth/code (NOT an iced user) — https://github.com/modrinth/code
- ZcashFoundation/zebra (NOT a GUI project) — https://github.com/ZcashFoundation/zebra
