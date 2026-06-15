# Tiers 1–2 — structured snapshots

**Date:** 2026-06-15
**Status:** draft
**Spec:** specs/2026-06-15-buiy-verification-design/README.md

The two cheapest, most deterministic rungs of the pyramid: **Tier 1** snapshots resolved
layout numbers per fixture (gate #5), **Tier 2** snapshots the whole CPU display-list /
paint-order / instance handoff holistically. Both replace today's low-density,
field-by-field `assert_eq!` in `render_*.rs` / `layout.rs` with one `insta` snapshot per
fixture, plus a byte-exact `Pod` hex check on `PackedInstance`. Pure-CPU, headless, in the
standard `cargo test` gate — no GPU, no window, sub-millisecond, 100% deterministic.

## Why a Display dump, not serde `Debug`/JSON

The report (§ Tier 2) is explicit: **do not snapshot raw `Debug`, and do not snapshot a
serde dump of the structs.** A serde/`Debug` snapshot couples the artifact to private field
names and `Entity` allocation bits (which vary with spawn order), so every struct refactor
re-blesses every snapshot and every unrelated spawn churns the diff. We instead emit a
purpose-built `Display` dump: one paint command per line, entities rendered by `Name`, floats
rounded, with a format-version header. The dump is the durable contract; the structs underneath
are free to churn. This is the Flutter `toStringDeep` / WebRender RON-display-list pattern, and
it is the one tier `masonry_testing` skips (it jumps straight to PNG goldens — the gap Buiy must
not replicate). **Consequence: no new `serde` derives are added to render types.** `serde` /
`serde_json` are already workspace deps (`crates/buiy_verify/Cargo.toml`) and stay unused by this
tier; the only new dependency is `insta`.

## Dependency: `insta`

Add to the workspace (`Cargo.toml [workspace.dependencies]`):

```toml
insta = { version = "1", features = ["glob"] }   # "glob" drives fixture-dir enrollment
```

`buiy_verify` and `buiy_core` (dev-dependency) consume it via `insta.workspace = true`. `insta`
is MIT/Apache-2.0 (already in the allow-list) and pulls `similar`, `console`, `linked-hash-map`
— all permissive. **`cargo deny check` is a required gate before this lands**; if any transitive
license is new it fails CI by design (deny.toml `[licenses]` is allow-list-only) and must be added
explicitly with its SPDX id, never an exception hack. The `cargo-insta` CLI (the review tool) is a
developer tool installed via `cargo install cargo-insta`, **not** a dependency — CI never needs it
(`INSTA_UPDATE=no` is the CI default, so an unreviewed `.snap.new` fails the build).

## Tier 1 — layout-number snapshots (gate #5)

### Public API (`buiy_verify::snapshot`)

```rust
/// Run the layout pipeline on `app`, then snapshot every entity's resolved box
/// as a stable Display dump. Asserts via `insta::assert_snapshot!` under `name`.
/// Pure-CPU: MinimalPlugins + CorePlugin + LayoutPlugin, no RenderApp, one `update()`.
pub fn assert_layout_snapshot(app: &mut App, name: &str);

/// The format-versioned Display dump backing the macro — `(name, position, size)`
/// per entity, sorted by `Name` then `Entity` index, floats rounded to `ROUND_DP`.
pub fn layout_dump(world: &World) -> String;
```

Dump format (`layout_dump`), version-headered so a format change is a single visible line:

```
# buiy-layout-dump v1
root            pos=0,0      size=200,100
  row.item[0]   pos=0,0      size=50,50
  row.item[1]   pos=50,0     size=50,50
```

- Entities are named by their `Name` component (`bevy::prelude::Name`); fixtures **must** set
  one (`Name::new("root")`). An unnamed entity falls back to `entity#<index>` — flagged, because
  an unnamed fixture is non-diff-stable across refactors. (`Name` is not currently spawned in
  `buiy_core`; fixtures opt in. The dump never prints raw `Entity` bits.)
- Tree indentation follows `ChildOf`; siblings ordered by `Name` (document order is unstable
  under ECS archetype moves, so `Name` is the sort key).
- Floats rounded to `ROUND_DP = 2` decimals (`const ROUND_DP: usize`) via a shared
  `round(f32) -> String` helper (Tier 1 + Tier 2 share it) — kills last-ULP churn from the
  Taffy/clip-space math while staying diff-readable.

### What it replaces

`crates/buiy_core/tests/layout.rs:33` — the `assert!((layout.size.x - 50.0).abs() < 0.5)`
pair in `layout_resolves_a_simple_flex_row` becomes one `assert_layout_snapshot(&mut app,
"flex_row_basic")`. The two GC tests (`layout_tree_garbage_collects_*`) assert `LayoutTree`
*cardinality*, not geometry — they stay as plain `assert_eq!` (snapshotting a length is
lower-density than the assert). Tier 1 replaces only the *geometry* asserts.

Taffy's WPT-derived corpus (`docs/prior-art/taffy/lessons.md`) is importable as fixtures here
to exercise Buiy's Taffy bridge — the coverage matrix (`coverage.md`) auto-enrolls them.

## Tier 2 — display-list / paint-order / instance snapshots

### Public API (`buiy_verify::snapshot`)

```rust
/// Snapshot the CPU display-list handoff holistically: ExtractedNodes order +
/// InstanceBuckets draw order + per-instance paint params, as one Display dump.
/// Pure-CPU — runs the extract/pack path, never a GPU. `name` keys the `.snap`.
pub fn assert_display_list_snapshot(nodes: &ExtractedNodes, name: &str, names: &NameLookup);

/// Display dump of an ExtractedNodes set: nodes in `painters_z` order, then the
/// pack_view() InstanceBuckets in BTreeMap (draw) order. Entities by Name.
pub fn display_list_dump(nodes: &ExtractedNodes, names: &NameLookup) -> String;

/// Resolve Entity -> human name for the dump (Name component, else `entity#idx`).
/// Built from the world once; passed in so the dump fn stays World-free/pure.
pub struct NameLookup(/* HashMap<Entity, String> */);
impl NameLookup { pub fn from_world(world: &World) -> Self; }
```

Dump format (`display_list_dump`), version-headered:

```
# buiy-display-list-dump v1
[nodes painters_z]
0  modal      rect pos=10,20  size=100,40  color=token:Surface  clip=none    group=none
1  tooltip    rect pos=0,0    size=80,24   color=#ffffffff       clip=0,0..80,24  group=0
[buckets draw-order]
(Quad,layer=0) x2
(Glyph,layer=1) x5
```

- **One paint command per line.** `ExtractedNode.nodes` is emitted in stored order (it is
  *never* re-sorted by render — `extract.rs:141` — so the snapshot is the paint order, and a
  z-sort regression shows as a line reorder, the exact bug class pixels name poorly).
- **Color rendered as a token when resolvable, else `#rrggbbaa`.** `ExtractedNode.color` is
  already theme-resolved (`extract.rs:77`), so a literal hex in a snapshot that should show a
  token is itself a regression signal (the magenta `MISSING_TOKEN_FALLBACK` sentinel surfaces
  as `#ff00ffff`).
- **`InstanceBuckets` appended in `BTreeMap` key order** (`buckets.rs:113`, `(layer, primitive
  paint-order)`) — the natural iteration *is* the deterministic draw order, so the dump pins
  both the per-node set and the batched draw order in one artifact. Per-batch instance *counts*
  go in the dump; the exact `[f32;13]` payload is pinned by the byte-hex check below (counts in
  the readable dump, bytes in the strict one — complementary, per report § Tier 2).
- Floats rounded to `ROUND_DP` via the shared helper; `clip=none` for the `None` full-view
  sentinel (`extract.rs:83`), else `min..max`; `group=<idx>|none` for `ExtractedNode.group`.

### The byte-exact `PackedInstance` hex check

`PackedInstance` is `#[repr(C)] Pod/Zeroable` (`render/instance.rs:41`), 52 bytes = `[f32;13]`
(pos2/size2/rgba4/radius1/clip_min2/clip_max2 — confirmed `instance.rs:42`–`:58`,
`PACKED_INSTANCE_STRIDE_BYTES = 52`). It is byte-snapshottable *now* with no new derive — a
deterministic, stricter, formatter-free regression on the px→logical packing:

```rust
/// Hex-dump a packed instance as `bytemuck::bytes_of(p)` — a byte-exact snapshot
/// of the GPU upload payload, independent of the Display dump's format version.
pub fn instance_hex(p: &PackedInstance) -> String;          // 104 hex chars
pub fn assert_instance_hex_snapshot(p: &PackedInstance, name: &str);
```

This is the complement the report mandates: the `Display` dump is diff-readable but
format-versioned; the hex dump is opaque but byte-exact and format-free. A packing arithmetic
change (e.g. the half-size sign bug `render_instance.rs` already regression-tests) flips the
hex even if the rounded Display dump rounds it away. **Endianness note:** `bytes_of` is
host-endian; CI and dev are both little-endian x86-64, and the hex is a within-repo regression
artifact (not a cross-host wire format), so this is acceptable — documented in the fn so a
big-endian CI host would be a conscious change.

### What it replaces

The low-density per-field `assert_eq!` named in the report become holistic snapshots:

| Test file | Today | After |
|---|---|---|
| `tests/render_extract.rs` (459 L) | `assert_eq!(node.position, …)`, `node.size`, `node.color`, `node.clip`, the `assemble_context_tree` order `assert_eq!(got, vec![root,a,nested,c,d,b])` (`:423`) | `assert_display_list_snapshot` over the assembled `ExtractedNodes` |
| `tests/render_buckets.rs` (385 L) | `b.len(q0)`, `total_instances`, `batch[0] == expect`, the `PackedPartition` field asserts (`:239`) | display-list dump (counts + draw order) + `assert_instance_hex_snapshot` for the exact payload |
| `tests/render_paint_order.rs` (135 L) | `assert_eq!(tail, vec![fullscreen,tooltip,popover,modal])` (`:64`) | display-list dump of the assembled order (the tail ordering reads off the node lines) |
| `tests/render_instance.rs` (168 L) | per-field `PackedInstance` asserts incl. the half-size sign regression | `assert_instance_hex_snapshot` (byte-exact; the sign bug flips the hex) |
| `tests/top_layer.rs` | `partition_top_layer` order asserts | display-list dump of `partition_top_layer` output |
| `tests/layout.rs:33` | geometry `assert!` | `assert_layout_snapshot` (Tier 1) |

**Replace, don't duplicate.** Each migrated test keeps its *scene construction* and *intent
comment*; only the trailing assert block collapses into one snapshot line. Asserts that pin a
**single named invariant** (e.g. `render_buckets.rs:9` `Shadow.paint_order() < Quad…`, or the
GC cardinality checks) stay as `assert!`/`assert_eq!` — a snapshot of one boolean is *lower*
density, which the report's "lowest tier that covers the behavior" rule rejects. The migration
is "holistic state → snapshot; single named property → keep the assert."

### Per-timestamp animation snapshots (Tier 2, opt-in — Decision 8)

Animation timing lives one tier down from pixels: the easing/interpolation curve is
fully observable in the deterministic CPU display-list, so temporal coverage is a
**display-list snapshot sampled at stepped virtual timestamps**, not a pixel sequence
(open-questions Decision 8). It is **opt-in per fixture** — default is end-state only
(the static golden covers the resting frame) — and a fixture enrolls only when its
*timing curve* is the behavior under test (a custom easing, a staged reveal, the caret
blink). When it does, the default sampling is **three logical timestamps** (`t=0`, mid,
end); a curve that demands more names them per fixture.

The entry point drives the same manually-advanced `Time<Virtual>` clock the determinism
stack mandates (`determinism.md` § "Async-asset flush" — `fixed_clock` is "drive
`Time<Virtual>` at explicit virtual timestamps"), snapshotting the display-list dump at
each step:

```rust
/// Snapshot the display-list dump at each virtual timestamp in `steps`, advancing
/// `Time<Virtual>` to each absolute logical time (NOT wall-clock) between captures.
/// One `.snap` per step, keyed `<name>@<t_ms>` (e.g. `caret_blink@0`,
/// `caret_blink@250`, `caret_blink@500`), so a timing regression shows as a diff in
/// exactly the frame whose curve drifted. Pure-CPU, no GPU — the dump is a text
/// artifact (snapshots.md), so a 3-sample sequence costs ~3× a single dump, not a
/// pixel capture. Each step runs the extract path and emits `display_list_dump`.
pub fn assert_display_list_snapshot_at(
    app: &mut App,
    name: &str,
    steps: &[std::time::Duration],   // e.g. &[ZERO, mid, end] — three by default
);
```

The fixed clock makes the sequence deterministic: every step is an explicit
`Time::<Virtual>::advance_to`/`advance_by` (the landed manual-clock mechanism,
`tests/text_caret_selection.rs:178`), so the same timestamps reproduce byte-identical
dumps across hosts and runs — the determinism stack's clock guarantee is exactly what
makes per-timestamp snapshots cheap and stable. Pixel-level temporal coverage stays
reserved for the rare fixture whose *rasterized* output changes per-frame in a way the
display list cannot express (Decision 8 runner-up rejection); the default temporal
altitude is this structured per-timestamp dump.

## The `cargo insta review` accept loop

`cargo insta review` *is* the `--accept` UX the report requires (`a`/`r`/`s` per change,
rewrites the `.snap` on accept). No bespoke env flag is added — this is the native analogue of
the in-repo `BUIY_ACCEPT_SHAPING` curated flow (`tests/text_shaping_snapshots.rs`), and the
discipline is identical: **a snapshot change is a behavior change — review the diff before
accepting.** `INSTA_UPDATE` defaults to `no`, so in CI an unreviewed `.snap.new` fails the
build. `.snap` files live beside their tests (`crates/buiy_core/tests/snapshots/`,
`crates/buiy_verify/tests/snapshots/`) — text, diff-readable, in git, zero binary blobs.

## Contract deviations

- **`serde` additions explicitly NOT taken.** The contract's `snapshot` bullet lists "the serde
  additions needed (ResolvedLayout, ExtractedNode, DrawData/InstanceData) **or** a Display dump
  formatter approach (preferred per report)". This spec takes the Display-dump branch
  exclusively and adds **no** serde derives — the report (§ Tier 2) is explicit that raw
  Debug/serde snapshots are the anti-pattern. Flagged for the synthesizer only because it
  resolves the contract's "or" to one branch.
- **`assert_display_list_snapshot` signature** takes `&NameLookup` (a `World`-free entity→name
  map) rather than the contract's bare `(nodes, name)`. Required because the dump renders
  entities by `Name`, and `ExtractedNode` carries only an `Entity`, not its `Name`. Pure-fn
  hygiene: the dump stays `World`-free; the lookup is built once via `NameLookup::from_world`.

## Verification

How the harness verifies *itself* (the snapshot tooling is load-bearing, so it gets its own
non-snapshot tests):

1. **Determinism of the dump.** A unit test builds one fixture, calls `layout_dump` /
   `display_list_dump` twice on independent `App`s spawned in **different entity order**, and
   `assert_eq!`s the two strings — proving the dump is invariant to `Entity` allocation order
   (the property the `Name`-keyed sort exists to guarantee). This is a plain `assert_eq!`, not a
   snapshot, so the meta-test cannot pass vacuously.
2. **Float rounding.** `round(1.005) == "1.0"`-class table tests on the shared helper pin the
   `ROUND_DP` behavior, including negative and sub-ULP inputs.
3. **Hex round-trips bytes.** `instance_hex` then `hex → bytes → bytemuck::pod_read_unaligned`
   reconstructs the original `PackedInstance` (`assert_eq!`), proving the hex is lossless and
   matches the GPU upload payload.
4. **Format-version tripwire.** A test asserts the dump's first line equals the current
   `vN` header constant — so a formatter edit that should bump the version but didn't fails
   here (answering Open Q #5: format changes are a conscious, version-gated re-bless).
5. **Migration is behavior-preserving.** Each migrated test's first run blesses the snapshot;
   reviewers diff the new `.snap` against the *old* per-field asserts to confirm the snapshot
   encodes the same facts (the half-size sign regression in `render_instance.rs` must still
   fail when re-introduced — verified by a mutation check during the migration plan).
6. **Standard gate.** Everything runs under `xvfb-run -a cargo test --workspace` with **no**
   `--ignored` and no GPU adapter — the headless gate stays green on a CI host with no GPU.

## Sources

Code: `render/extract.rs:65`/`:139`/`:141`/`:77`/`:83` (ExtractedNode/ExtractedNodes, never
re-sorted), `render/instance.rs:41`/`:42` (PackedInstance Pod, `[f32;13]`/52 B),
`render/buckets.rs:86`/`:113`/`:196` (InstanceBuckets BTreeMap draw order, PackedPartition),
`components.rs:25`/`:82` (ResolvedLayout, StackingContext), `render/golden.rs` (GoldenConfig /
the `BUIY_ACCEPT_SHAPING` accept-flow analogue). Tests replaced:
`tests/{layout,render_extract,render_buckets,render_paint_order,render_instance,top_layer}.rs`,
`tests/text_shaping_snapshots.rs` (the in-repo structured-snapshot + curated-accept precedent).
Prior-art: `docs/prior-art/taffy/lessons.md` (WPT layout corpus),
`docs/prior-art/xilem-masonry/lessons.md` (`insta`, the skipped structured tier). Report:
`docs/reports/2026-06-14-visual-bug-detection-strategy.md` §§ Tier 1, Tier 2, Open Q #5.
