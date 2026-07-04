# Dooduel — Web acceptance evidence (2026-07-04)

App source: `apps/dooduel` + `apps/dooduel/dooduel_web` on branch `docs/dooduel-acceptance`
(off main `24fa997`). Verified in this worktree; NO app/framework code changed.
Environment: local GPU host (WebGPU adapter present) + headless Chromium (Playwright chromium-1228).

## Verdicts

| # | Milestone | Verdict | Evidence |
|---|-----------|---------|----------|
| M1 | wasm builds on BOTH backends (webgpu + webgl2) | **PASS** | `webacc_build_webgpu.log`, `webacc_build_webgl2.log` (both EXIT=0) |
| M2 | app boots + renders Home in headless browser | **PASS (both backends)** | `webgl2/home.png`, `webgpu/home.png`, `*/interact_summary.json` |
| M3 | F9 HiDPI gate (dsf 2 + 3, both knobs) | **PASS** | `webgl2/hidpi_dsf2.png`, `webgl2/hidpi_dsf3.png`, `webacc_hidpi.log` |
| M4 | dynamic-resize transient probe | **REPRODUCED** (persistent, resize-only) | `webgl2/resize_*.png`, `webgl2/resize560_t*.png`, `webgl2/freshload_560x900.png` |
| M5 | basic web interaction folds | **PASS (both backends)** | `*/after_theme.png`, `*/after_play.png` |

## Build (M1)
- `trunk build apps/dooduel/dooduel_web/index.html --features webgpu` -> EXIT 0. wasm 116 MB (dev/unoptimized).
- `trunk build apps/dooduel/dooduel_web/index.html --features webgl2` -> EXIT 0. wasm 121 MB (dev/unoptimized).
- ~1m23-27s each (post-dep). `RELEASE=1 tools/build-web.sh` (wasm-release + wasm-opt -Oz) shrinks these for shipping.
- No `RUSTFLAGS` override needed: the wasm `--cfg=web_sys_unstable_apis` comes from `.cargo/config.toml`.

## Boot + render (M2)
Both backends render the full Home screen (title "Dooduel", violet "Play" CTA, name field,
Create/Join, avatars, theme pill). 820-828 distinct colors; card surface white; zero console errors.
WebGPU additionally cleared the strict **Tint** shader-conformance gate (`run.mjs`: adapter present,
0 shader/pipeline errors, canvas painted — `webacc_webgpu_smoke.log`).

## HiDPI (M3) — both-knobs consistent-dpr, phone 390x844
- dsf=2: backing **780x1688** == CSS 390x844 x 2; logical == CSS; overflowX 0. PASS.
- dsf=3: backing **1170x2532** == CSS 390x844 x 3; logical == CSS; overflowX 0. PASS.
Confirms the F9 finding for the Dooduel build: crisp HiDPI, no dpr-x mis-scale, content fits.

## Dynamic resize (M4) — REPRODUCED
Resizing the viewport AFTER boot persistently confines the app render to a sub-region: the canvas
backing store tracks the new viewport exactly, but the app content fills only part of it, leaving a
hard DARK CHROME BAND at the grown edge(s).
- 900x640 -> 560x900: right edge **100% dark** (band ~170px wide); does NOT clear — held at 100% across
  t = 2s, 4s, 6s, 12s, 20s (`resize560_t*.png`). A further 560->820 resize leaves BOTH edges dark.
- 900x640 -> 1300x840: card mis-centered/clipped right (`resize_up.png`).
- **Fresh load directly at 560x900 is CORRECT** (right edge 1.7% dark, centered card — `freshload_560x900.png`),
  so the fault is exclusively the dynamic-resize path, not the size. dpr=1 here, so it is NOT a dpr artifact.
- Caveat: the F9 report already flagged this dynamic-resize sub-case as unverified and headless resize
  emulation as not fully faithful; it must be confirmed/root-caused on the real-device gate. It does NOT
  affect the load-at-a-fixed-size criterion (M2/M3 pass). Narrowly-scoped follow-up for the app/framework team.

## Interaction (M5) — both backends
- **Theme toggle** click at the fixed bottom-right pill (CSS 844,603): folds `SetTheme` through the MVU
  funnel; card surface flips white(255,255,255) -> dark(38,42,51), 97.5-97.6% of the frame changes,
  pill label "Light" -> "Dark" (`after_theme.png`). Zero console errors.
- **Play** click (violet CTA located by color): navigates Home -> in-game word-pick ("Round 1/2",
  scoreboard, WINDMILL/RAINBOW/UMBRELLA); ~39% frame change, violet CTA drops 35018 -> 1711 px (`after_play.png`).

## Repro
Drivers (in `$CLAUDE_JOB_DIR/tmp`): `dooduel_interact.mjs` (M2+M5), `dooduel_resize.mjs` +
`dooduel_resize_settle.mjs` (M4), `png.mjs` (zlib PNG decoder), stock `tools/web-smoke/{run,run-webgl2,hidpi-check}.mjs`.
Serve a dist dir with `python3 -m http.server`; drive with Playwright chromium (WebGL2 = SwiftShader args;
WebGPU = Vulkan/ANGLE args). Never `getContext` on `#buiy` for a mismatched backend; the drivers only
`el.screenshot()` + `page.mouse`.
