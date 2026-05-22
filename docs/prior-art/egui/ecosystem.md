**Date:** 2026-05-22
**Status:** active
**Subject:** egui — production users, integrations, and place in the Rust GUI landscape

# Ecosystem

egui's adoption is concentrated in three workloads: **developer tools**, **internal company tooling**, and **single-product apps where the egui aesthetic is acceptable**. It is largely absent from the "ship a polished consumer app" workload — which is honest, not a failure.

## Production users

### Rerun (the canonical at-scale user)

Rerun is the **flagship egui app**. Streaming-data visualization for ML / robotics teams; desktop + WASM. The Rerun Viewer renders multi-megabyte time-aligned tensor streams (cameras, point clouds, transforms, scalars) inside an egui shell — dockable panels, multi-viewport floating windows, custom 3D viewports embedded as egui textures.

- **Scale claims (verified via Rerun docs):** sustained 60fps streaming of 10k+ items per frame on consumer hardware.
- **Why it works:** Rerun's widget count is bounded — most pixels are custom 3D viewports, not egui widgets. The egui layer is the chrome around the 3D content, not the bulk of the rendering.
- **What it doesn't prove:** Rerun is not evidence that a 10k-egui-widget UI is performant. It's evidence that egui-as-chrome-around-custom-content works.

### Embark Studios (internal tooling)

Embark uses egui internally for world editors, asset browsers, debug overlays, and developer dashboards. This is the original dogfooding context — Emil worked at Embark when egui matured. Public evidence: egui's GitHub stars + early commit attribution, plus Embark engineering blog mentions. Cross-link: this is the workload pattern bevy_egui inherits.

### bevy_egui (Bevy bridge)

bevy_egui is the dominant route by which Bevy users consume egui (~2M downloads). Fully documented in `prior-art/bevy-egui/`. Position in the egui ecosystem: bevy_egui is **the** dev-tool / debug-overlay choice for Bevy users, and the only practical way to get egui into a Bevy app without writing a custom integration.

### Other game studios (tools, not in-game UI)

Multiple studios use egui for **internal tools**: level editors, asset pipelines, build dashboards. Public attribution is rare (studios don't advertise their internal tooling stacks), but Embark's example + the GitHub stargazer list + community Discord show real adoption. The pattern is consistent: tools, not shipped in-game UI.

### Indie game UI (rare, honestly documented)

A handful of indie titles use egui for menus / HUD. This is the **honest weak link** of egui's ecosystem: the visual homogeneity ([critiques.md § homogeneity](critiques.md)) makes it hard to ship a distinctive-looking game on egui, and the immediate-mode rebuild-per-frame cost is uncomfortable for HUDs that need animations or many widgets. Most production game UIs ship on engine-native systems (Unity UI, Unreal UMG) or on retained-mode Rust UIs (Iced, Slint), not egui.

### Bitcoin / crypto / sysadmin niche

A real niche: egui is heavily used in cryptocurrency wallets (Sparrow Wallet's competitors), Bitcoin node visualizers, infrastructure dashboards, and homelab tools. The Rust-language overlap with crypto + the dev-tool aesthetic fit make this a sticky niche.

### Web apps (eframe + WASM)

eframe-on-WASM is a small but real workload. Use cases: Rerun Web Viewer, the egui demo itself (`egui.rs/#demo`), and a long tail of "Rust dev shipping a tiny web tool." Bundle sizes are large (multi-MB) — see [open-problems.md § WASM bundle size](open-problems.md).

## What egui is NOT used for (common misconceptions)

| Project | Reality | Misconception |
|---|---|---|
| **Tauri** | Uses **web view** for UI; egui is irrelevant. | "Tauri uses egui." |
| **Zed** | Uses **GPUI** (custom retained-mode framework). | "Zed uses egui." (Common false belief.) |
| **Lapce** | Uses **Floem** (signal-based retained). | "Lapce uses egui." (False.) |
| **System76 COSMIC** | Uses **Iced**. | "COSMIC uses egui." (False.) |
| **Bevy editor (when shipped)** | Will use **Bevy's own** UI stack (`bevy_ui` + `bevy_feathers` + `bevy_ui_widgets`), not egui. | "Bevy's editor will use egui." |

These corrections matter because Zed/Lapce/COSMIC are the loudest-shipping Rust desktop apps, and getting them wrong misrepresents egui's actual reach. egui's reach is **dev tools + Rerun + dashboards + crypto + indie utilities**, not the headline Rust apps.

## eframe — the egui-native runtime

eframe is egui's official "write an app and ship it" runtime. It wraps `winit` (windowing) + `egui-wgpu` or `egui_glow` (rendering) + `accesskit_winit` (a11y) + clipboard + persistence + a web/WASM target. Full feature list in [distribution.md](distribution.md).

- **Same code, web + native.** The eframe value proposition: write `impl App for MyApp { fn ui(&mut self, ui: &mut Ui) {} }`, get desktop + WASM binaries with the same source.
- **Render backend choice.** wgpu (default) for broad GPU support; glow (OpenGL) for smaller WASM bundles.
- **Persistence.** Optional `persistence` feature stashes window positions + `Memory` state via `ron`.

eframe is what most non-game egui consumers use. Game engines (Bevy via bevy_egui, custom engines) skip eframe and embed egui via `egui-winit` + their own renderer.

## egui's place in the Rust GUI landscape

The Rust GUI landscape circa 2026-05 (separate prior-art folders to come for each):

| Project | Mode | Backed by | Niche |
|---|---|---|---|
| **egui** | Immediate | Rerun | Dev tools, dashboards, Rerun, internal apps |
| **Iced** | Retained (Elm-like) | System76 (COSMIC) | Polished apps, desktop, web |
| **Slint** | Retained (DSL) | SixtyFPS GmbH | Embedded + commercial apps |
| **Dioxus** | Retained (React-like) | DioxusLabs | Web-first, full-stack |
| **Druid / Xilem** | Retained | Linebender | Research → production retained-mode |
| **Floem** | Retained (signal-based) | Lapce community | Reactive desktop apps |
| **GPUI** | Retained (custom) | Zed Industries | Zed only |
| **Bevy UI stack (bevy_ui / bevy_feathers / bevy_ui_widgets)** | Retained (ECS) | Bevy Foundation | Bevy-internal |
| **Buiy** | Retained (ECS + BSN) | (in development) | Bevy-native, web-platform-parity, AccessKit-first |
| **Dear ImGui (C++) / imgui-rs** | Immediate | Omar Cornut (community) | Dev tools, AAA in-game debug |

egui is the immediate-mode option in Rust. The retained-mode options outnumber it; the retained-mode options also collectively ship more polished consumer apps. But egui dominates one workload (Rust dev tools) so completely that "an egui-shaped UI" is the **default Rust dev-tool aesthetic** — see [critiques.md § homogeneity](critiques.md).

See [comparisons.md](comparisons.md) for design-axis comparisons against each alternative.

## The "dev tool default" status

If you are a Rust developer who needs to ship a tool with a UI in a weekend, egui is the default choice. This is not marketing — it's an empirical claim:

- The community routinely answers "what UI library should I use?" with "egui, unless you have a specific reason not to."
- The egui demo `egui.rs/#demo` is the entry point most newcomers see.
- The integration story is the broadest in Rust (eframe, bevy_egui, custom-engine integration, web).

The default-status comes with downsides — the homogeneity critique, the "looks like an egui app" recognition pattern. But the default-status is real, and Buiy should plan for it: any Bevy developer reaching for dev tooling will reach for `bevy_egui` first. Buiy's positioning is "the production UI when you want web-platform-parity + AccessKit-first," not "the dev-tool default" — those are different workloads. bevy_egui and Buiy can coexist in the same Bevy app.

## Sources

- Rerun — https://rerun.io ; viewer demo — https://app.rerun.io/
- Embark Studios engineering blog — https://www.embark.dev (occasional egui mentions; specific blog posts not pinned to date)
- bevy_egui — https://github.com/vladbat00/bevy_egui ; full corpus at `prior-art/bevy-egui/`
- Iced — https://iced.rs ; corpus at `prior-art/iced/`
- Slint — https://slint.dev
- Dioxus — https://dioxuslabs.com
- Floem — https://github.com/lapce/floem
- GPUI — https://www.gpui.rs (Zed Industries)
- COSMIC Desktop (Iced-based) — https://system76.com/cosmic
- Dear ImGui — https://github.com/ocornut/imgui
- egui 3rd-party crates wiki — https://github.com/emilk/egui/wiki/3rd-party-egui-crates
