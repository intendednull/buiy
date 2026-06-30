# RD4 — Hybrid port map + base reconciliation (4010753/WASM #85) + supply-chain + WASM-cleanliness

**Decision: the 4 new MVU files port verbatim and the substrate is wasm-clean; the
real work is reconciling 5 wasm-touched files (preserve the base's clipboard
cfg-gating), REDESIGNING `menu.rs` + the gallery onto the `GalleryPlugin`
restructure, and a `deny.toml` exception that must cover TWO iai-callgrind
advisories, not the one the brief named.**

Confidence: **high** (cargo-deny verdicts run against the prototype graph;
wasm-cleanliness greps confirmed).

---

## 1. Port map (prototype file → verdict + base-reconciliation note)

| # | File | Verdict | Note |
|---|------|---------|------|
| 1 | `crates/buiy_core/src/mvu/mod.rs` | **PORT-AS-IS** | New file. Only new dep is `ron` (already 0.12.1 in base lock via bevy_asset). `enqueue`'s command closure is Send; fine on wasm single-thread. |
| 2 | `crates/buiy_core/src/mvu/leaf.rs` | **PORT-AS-IS** | New file; reuses `a11y::A11yToggled` as Model. |
| 3 | `crates/buiy_core/src/replay.rs` | **PORT-AS-IS** | Uses `std::time::Duration` only (value type, wasm-safe; NOT `Instant`). |
| 4 | `crates/buiy_core/src/text/edit/record.rs` | **PORT-AS-IS** | Paste-replay uses `MemClipboard` + `ClipboardProvider` (`clipboard.rs:143`, NOT wasm-gated) — wasm-clean by construction. |
| 5 | `text/edit/input.rs` (record tap) | **PORT-AS-IS** (re-apply hunk) | Base already abstracts clipboard as `clip: &mut dyn ClipboardProvider` w/ `MemClipboard` fallback (`input.rs:648-652`); on wasm the resource IS `MemClipboard` (`text/mod.rs:308-314`). |
| 6 | `text/edit/ime.rs` (record tap) | **PORT-AS-IS** (4 hunks) | IME sub-events self-contained; no OS dep on replay. |
| 7 | `text/edit/mod.rs` | **RECONCILE** | Add `mod record;` + re-exports but KEEP base's `#[cfg(not(target_arch="wasm32"))] pub use clipboard::ArboardClipboard;` (`mod.rs:34-35`). Prototype (pre-wasm) re-exported it unconditionally — must NOT regress. |
| 8 | `text/mod.rs` | **RECONCILE** | Re-add `pub use commit::reshape_edited_editors;` (W6 prereq; base uses it internally at :197/:211 but does NOT pub-export) + EditLog/RecordSession/RecordedEdit inits, placed AFTER and WITHOUT disturbing the base's wasm clipboard-default split (`text/mod.rs:304-314`). |
| 9 | `crates/buiy_widgets/src/menu.rs` | **REDESIGN** | 607-line W5 machine rewrite replacing the base open-state path (advance_expanded_on_press / sync_menu_open / close_menu). Base here is pre-wasm-unaffected — pure logic delta. |
| 10 | `crates/buiy_widgets/src/dismiss.rs` | **REDESIGN-LITE** | Add the `dismiss_overlay` menu branch (carries the layering-coupling note — see RD5/dismiss-uninvert). |
| 11 | `crates/buiy_widgets/src/lib.rs` | **REDESIGN-LITE** | Re-apply WidgetsPlugin MVU wiring + `advance_toggle_on_press` reroute. |
| 12 | `examples/buiy_gallery/src/lib.rs` | **REDESIGN** | The W6 todomvc chain-split must re-apply INSIDE the base's new `GalleryPlugin`/`TodoMvcPlugin` (base `lib.rs:130-152` — the WASM PR DRY'd native+gallery_web; prototype never saw it). |
| 13 | `benches/mvu.rs`, `benches/mvu_iai.rs`, `mvu_scenes.rs` (+ `lib.rs:24`) | **PORT-AS-IS** | Dev-only. |
| 14 | `Cargo.toml` (workspace) | **RECONCILE** | Add `ron = "0.12"` + `iai-callgrind = "0.16"` WITHOUT dropping wasm members (`examples/buiy_web`, `examples/gallery_web`) or `[profile.wasm-release]`. |
| 15 | Test files | **PORT-AS-IS** except `mvu_whole_ui_replay.rs` → **RECONCILE** onto `GalleryPlugin`. |

---

## 2. deny.toml exception — VERIFIED BY RUNNING cargo-deny 0.19.4

iai-callgrind 0.16.1 pulls **TWO** unmaintained crates, not the one the brief named.
Add two entries to the existing `[advisories].ignore` (which already holds
RUSTSEC-2024-0436/paste):

```toml
ignore = [
    { id = "RUSTSEC-2024-0436", reason = "transitive via Bevy/wgpu-hal and image/rav1e; no upstream fix available" },
    # iai-callgrind (0.16.1) is a DEV-ONLY bench pricer (buiy_core [dev-dependencies]
    # driving the `mvu_iai` [[bench]] under Valgrind). Dev-deps are excluded from the
    # production AND wasm build graphs, so neither crate below ever ships. No safe
    # upgrade exists (both upstreams ceased). Re-evaluate when iai-callgrind drops
    # bincode 1.x / proc-macro-error2.
    { id = "RUSTSEC-2025-0141", reason = "bincode 1.3.3 unmaintained; dev-only via iai-callgrind, never in prod/wasm graph" },
    { id = "RUSTSEC-2026-0173", reason = "proc-macro-error2 2.0.1 unmaintained; dev-only via iai-callgrind-macros, never in prod/wasm graph" },
]
```

Confirmed green after the add: `cargo deny check licenses` = ok (all 7 new subtree
crates MIT/Apache-2.0), `bans` = ok, `sources` = ok. The wasm32 target the WASM PR
added to `[graph].targets` (`deny.toml:31-33`) does not change this — dev-deps are
audited on every listed target but never compiled into a shipped artifact.

**OPTIONAL belt-and-suspenders:** also gate the iai-callgrind dev-dep + `mvu_iai`
bench behind `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]` (it is
x86/Valgrind tooling with no wasm backend). This does NOT remove the advisory
ignores — the crates remain in the native dev graph. Recommendation: **do both.**

### Separate, pre-existing base failure (own commit)

The FINAL base @4010753 **already** fails `cargo deny check advisories` on
**ttf-parser (RUSTSEC-2026-0192, via bevy_winit)** — advisory-DB drift since #85
landed, independent of the MVU port. Per scope discipline this belongs in its OWN
commit (a triaged ignore or a bevy_winit bump), NOT bundled with the iai-callgrind
exception — but it MUST be resolved or the FINAL's deny gate stays red. **No finding
owns this fix; the plan must assign it.**

---

## 3. WASM-cleanliness — CONFIRMED CLEAN

Grep of all 4 ported MVU files for `std::thread`/`Instant`/`SystemTime`/`rayon`/
`park`/`spawn_blocking` returns **NONE**. Substrate primitives are all wasm-safe:
`Reflect` + RON (pure-Rust serde, already in base lock), Bevy `Messages`
(`Envelope<M>`), `Mut::set_if_neq`, `LogicalId(u64)`. `now: Duration` originates
from bevy `Time::elapsed()` (`input.rs:666`) — bevy's wasm-driven clock, not
`std::Instant`. Nothing in the MVU/replay/record path needs cfg-gating; the ONLY
wasm-sensitive surface is the pre-existing clipboard (arboard), which the base
already cfg-gates and the record tap consumes through the trait object — so the
port introduces **ZERO new wasm obstacle**.

---

## Residual open-for-spec / risks

- **deny under-scoping** is the trap the brief itself fell into: shipping only the
  proc-macro-error2 ignore leaves CI's deny gate RED on bincode. Both ignores
  required.
- **PRE-EXISTING ttf-parser base failure** blocks the green gate independent of the
  port — own commit, must be assigned.
- **Gallery REDESIGN risk:** the W6 chain-split was authored on the pre-wasm gallery;
  the base restructured into a shared `GalleryPlugin` (native+web). A naive
  patch-apply fails; re-derive inside `TodoMvcPlugin` and re-verify `gallery_web`
  (wasm) still builds+runs with the MVU chain.
- **menu.rs is a 607-line REDESIGN that deletes base systems** (`sync_menu_open`/
  `close_menu`/`menu_of_item`). Mis-merge could resurrect a second writer of
  `A11yExpanded` — the exact multi-writer defect W5 kills.
- **text/mod.rs + text/edit/mod.rs reconciliation must PRESERVE the base wasm
  cfg-gates** while inserting record exports/inits; a careless port re-introducing
  the prototype's unconditional `ArboardClipboard` re-export breaks the wasm build.
- **iai-callgrind in the native dev graph adds 7 transitive crates** (incl.
  serde_json) — keep it strictly dev-only.

## Key evidence

- prototype `mvu/mod.rs:628-630`, `:365-371`; `leaf.rs:90-106`, `:126-147`;
  `replay.rs:52`, `:142-173`; `record.rs:35`, `:230-246`, `:392-402`.
- FINAL base `text/edit/clipboard.rs:62,68,83,143`; `input.rs:648-652,666`;
  `text/edit/mod.rs:34-35`; `text/mod.rs:304-314,197,211`.
- FINAL base `deny.toml:31-33,45-52`; cargo-deny 0.19.4 on prototype:
  RUSTSEC-2026-0173 (proc-macro-error2, Lock:326), RUSTSEC-2025-0141 (bincode 1.3.3,
  Lock:94); on FINAL base: RUSTSEC-2026-0192 (ttf-parser via bevy_winit).
- FINAL base `examples/buiy_gallery/src/lib.rs:130-152` (GalleryPlugin);
  prototype gallery diff `:936-960` (chain-split); `buiy_bench_support/lib.rs:24`.
