**Date:** 2026-07-01
**Status:** active — Phase-B execution plan for **PR1** (the first mergeable `buiy_view` PR).
**Spec:** [buiy_view authoring surface — FINAL design](../specs/2026-07-01-buiy-view-authoring-design.md) (§4 defines PR1).
**Base:** `worktree-interface-final` off `origin/main` @ `164f347`.
**Audited port from:** the throwaway prototype `worktree-interface-proto` @ `11b94fb` (`examples/safer_v_proto/src/lib.rs` = the validated surface to restructure + refine).

# Plan — `buiy_view` PR1 (the first mergeable surface)

**Strategy:** an *audited port*, not a rebuild. The prototype validated the Element + reconciler + router + keyed_column + when + map logic; PR1 restructures it into the production `crates/buiy_view` crate and re-implements the REFINE items the prototype deferred (decomposed-style patching #9, reconcile-before-layout #10, the internal `ViewSlot` contract #12, drift-only writes #11, typed tokens end-to-end #8, `ui()` M-inference #7, view work-counters + the W4 gate #14). Interdependent → **sequential in the warm final worktree**, RUN + gate each wave. Merge-gate on human review (do NOT self-merge). Commits allowed; push held for explicit ask.

## Waves

### FW1 — crate + core surface + Counter (port + the structural refines)
- Create `crates/buiy_view` (workspace member): deps `buiy_core` + `buiy_widgets` + `bevy` (no cycle — §1).
- Port `Element<Msg>`/`Kind`, builders `text`/`button` + `column!`/`row!`/`text!`, uniform dot-modifiers, typed tokens `Space`/`Color`/`Radius` resolved against `Theme` (#8), `ui(init, update, view)` with **M inferred from all three** (#7).
- The **reconciler** (positional): **emit decomposed style components** (`FlexGap`/`BoxModel`/`FlexParams`/`Background`) so style **patches in place** (#9); run **`.before(BuiySet::Layout)`** (#10); **drift-only controlled writes** (#11); the internal **`ViewSlot`** stamp so the label patch doesn't re-walk widget children (#12).
- The **press router** (`route_presses<M>` → real `enqueue`), replay-safe typed `PressAction<M>`.
- Re-author **Counter** as `examples/` (a `buiy_view` example: windowed bin + headless capture bin).
- **Tests:** reconcile/DX-2 (patch-in-place, id-stable), router/DX-3 (no app route system), styling/#9 (token-driven style patches in place, same id).
- **RUN + verify:** capture PNG (Count:0 Reset-dimmed → Count:3 Reset-bright); recompile + tests green.

### FW2 — keyed lists + editor bridge + TodoMVC
- `keyed_column(iter, key_fn, view_fn)` (required key) + **keyed reconcile** (spawn-new/despawn-gone/reorder-in-place, row identity preserved).
- Builders `checkbox`/`text_input`; handlers `on_toggle`(bare fn)/`on_input`(bare fn)/`on_submit`; the two editor routers (`route_text_input` reads `TextChanged`, `route_text_submit` reads `EditSubmitted`); controlled draft via the low-level `apply()` seam; `clear` = `SelectAll`+`Delete`.
- Re-author **TodoMVC** as `examples/`.
- **Tests:** keyed (id-stable add/remove/reorder + real `A11yToggled`), editor bridge, **replay** (record add+toggle+remove → replay into fresh app → `replayed_model == recorded_model`, tree re-derives, only harmless off-log leaf/editor dead-letters).
- **RUN + verify:** capture PNG (card + rows + "N items left"); tests green.

### FW3 — conditional + map + view work-counters + the W4 go/no-go gate
- `when(cond, el)` + `Kind::Empty` (+ `Option`→`Empty` auto-wrap) (#5); `Element::map(fn(Msg)->Parent)` (value + bare-fn handlers) (#6).
- `ViewWorkCounters { reconciles, nodes_spawned, nodes_despawned, nodes_patched }` (reset pre-reconcile; `nodes_patched` counts only value-changing writes).
- **The W4 can-fail gate** (§5): idle ⇒ `reconciles==0`; idempotent fold ⇒ no cascade; `Inc` ⇒ reconcile once, `nodes_spawned==0`, `nodes_patched==1`, layout dirty-set bounded to the changed subtree (does NOT re-dirty the whole tree). **Green to bless the surface.**
- **Tests:** conditional (slot alternates Empty↔content, siblings id-stable), child-lift (embed Counter twice via `map`, isolated), the W4 gate assertions.
- (map/when's demo showcase = the scaling demo, which is PR2; PR1 validates them via logic tests + optionally a minimal two-Counter example.)

### FW4 — prelude wiring + docs + the full mechanical gate → merge-ready
- The **`buiy::view` sub-prelude** module re-exporting the surface through `buiy` (§1 — a distinct path, because the view builders `button`/`checkbox` collide with the existing bsn! scene-fns in the main prelude).
- Rustdoc on the public surface; the docs/README catalog entry; a short "authoring an app" doc.
- **Full mechanical gate** (CLAUDE.md): `cargo fmt --all --check` + `clippy --workspace --all-targets --locked -D warnings` + `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --locked` + headless `nextest --workspace --locked`, all GREEN; the GPU `--ignored` capture lane on a real adapter.
- **Stop → merge gate.** Open the PR (do not merge); report merge-ready + the W4 gate result. Await human go.

## Verification discipline
- RUN the artifact (capture PNG + view it) every wave — don't trust green (the prototype + widget-catalog history both show headless-green ≠ works).
- Force a real recompile before believing any "broken" LSP signal (the prototype hit stale `E0583`/proc-macro false alarms every wave).
- Fresh-context review gate after FW3 (pre-polish) and again at merge-ready.

## Out of scope (honest — PR2/PR3 per spec §4)
- Async `Cmd::task` from `update` (PR2) · the scaling demo showcase (PR2) · capturing `on_input` (PR3) · `ControlledLeaf` checkbox double-fold suppression (PR3) · the public `ViewWidget` third-party slot trait (PR3). Each documented at its site; none block authoring Counter/TodoMVC.
