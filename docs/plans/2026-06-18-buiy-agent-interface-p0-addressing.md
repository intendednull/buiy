# Buiy Agent-Interface Phase 0 — Addressing + Serializer Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Date:** 2026-06-18
**Status:** active
**Spec:** docs/specs/2026-06-18-buiy-agent-interface-design/README.md
**Campaign:** docs/plans/2026-06-18-buiy-agent-interface-campaign.md

**Goal:** Land the version-stable groundwork for the agent-interface campaign:
a `NodeId → Entity` inverse, the canonical-ref fix to the snapshot serializer,
and the seven new `A11yRole` variants — with no change to the live tree's
behavior.

**Architecture:** Three small, independent changes across two crates. Each is a
pure-function or enum change with a unit test; none touches the runtime
`build_tree`/`push_tree_updates` path, so the AccessKit tree an AT sees is
byte-identical except for the snapshot serializer's `entity` field value (which
becomes the canonical `NodeId`).

**Tech Stack:** Rust, Bevy ECS (`Entity::to_bits`/`from_bits`/`from_raw_u32`),
`accesskit` (`NodeId`, `Role`), `serde_json`.

---

## Why Phase 0 is version-stable (read first)

Per the campaign's dependency gate, the spec targets **accesskit 0.24 / Bevy
0.19-rc.3** and is sequenced after the BSN/0.19 bump. **Phase 0 is the
exception:** all three changes work identically on the current
**accesskit 0.21 / Bevy 0.18** `main` and on the post-bump 0.24 surface —
`entity_for_node_id` is pure Rust, the serializer fix is pure, and the seven new
`Role` variants (`CheckBox`/`Switch`/`Slider`/`TextInput`/`MultilineTextInput`/
`Region`/`Group`) exist in both accesskit lines. So Phase 0 can land **before or
after** the bump — coordinator's call. The only API to confirm against the
pinned versions at implementation time: the exact spelling of each `accesskit::Role`
variant (e.g. `Role::CheckBox`, capital B) and whether `Entity::from_bits`
returns `Entity` (0.18) or needs `Entity::try_from_bits(..).ok()` — both concrete
forms are given inline below.

**Repo commit policy:** end every commit message with the repo's required
trailer (`Co-Authored-By: …`, per the project convention). Run the full check
command (`cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace`) before the final Phase-0 commit; the per-task commands below are the fast inner loop.

---

### Task 1: `entity_for_node_id` — the NodeId → Entity inverse

The inbound action router (Phase 1c) resolves an `ActionRequest`'s
`target: NodeId` back to a Bevy `Entity`. That inverse is pure and testable now,
and the `buiy_verify` serializer fix (Task 2) reuses `node_id_for`. Add the
inverse next to `node_id_for`.

**Files:**
- Modify: `crates/buiy_core/src/a11y/translate.rs` (add `entity_for_node_id` after `node_id_for`, ~line 21)
- Test: `crates/buiy_core/tests/a11y_translate.rs`

- [ ] **Step 1: Write the failing tests**

Append to `crates/buiy_core/tests/a11y_translate.rs` (the file already imports
`bevy::prelude::*`, so `Entity` is in scope):

```rust
#[test]
fn entity_for_node_id_inverts_node_id_for() {
    use buiy_core::a11y::translate::{entity_for_node_id, node_id_for};
    let e = Entity::from_raw_u32(42).expect("valid entity index");
    assert_eq!(entity_for_node_id(node_id_for(e)), Some(e));
}

#[test]
fn entity_for_node_id_maps_root_to_none() {
    use buiy_core::a11y::translate::{entity_for_node_id, ROOT_NODE_ID};
    assert_eq!(entity_for_node_id(ROOT_NODE_ID), None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p buiy_core --test a11y_translate entity_for_node_id`
Expected: FAIL — compile error, `cannot find function 'entity_for_node_id' in module 'translate'`.

- [ ] **Step 3: Implement `entity_for_node_id`**

In `crates/buiy_core/src/a11y/translate.rs`, immediately after the `node_id_for`
function (after line 21), add:

```rust
/// Inverse of [`node_id_for`]: recover the [`Entity`] an inbound [`NodeId`]
/// addresses. `NodeId(0)` is the synthetic root and maps to `None`; every other
/// id is `entity.to_bits() + 1`, so `id.0 - 1` is always a valid `to_bits()`
/// pattern (it was produced by `node_id_for`).
pub fn entity_for_node_id(id: NodeId) -> Option<Entity> {
    if id == ROOT_NODE_ID {
        return None;
    }
    Some(Entity::from_bits(id.0 - 1))
}
```

API note (verified against `bevy_ecs-0.18.1/src/entity/mod.rs:576`):
`Entity::from_bits(bits: u64) -> Entity` returns `Entity` directly on Bevy 0.18,
so the form above is correct as written. If the pinned Bevy ever makes it
fallible, drop the `Some(...)` wrapper and return `Entity::try_from_bits(id.0 - 1)`
directly (`try_from_bits` returns `Option<Entity>` — mod.rs:590).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p buiy_core --test a11y_translate entity_for_node_id`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_core/src/a11y/translate.rs crates/buiy_core/tests/a11y_translate.rs
git commit -m "feat(a11y): add entity_for_node_id, the NodeId->Entity inverse"
```

---

### Task 2: fix the snapshot serializer to emit the canonical ref

`buiy_verify::a11y::snapshot_tree` currently serializes the **raw** entity bits
(`n.entity.to_bits()`), which is **off by one** from the `NodeId` an AT or agent
addresses (`node_id_for(entity) = to_bits() + 1`). Fix the value to the canonical
ref. No golden files capture entity bits (only the unit test in
`crates/buiy_verify/tests/a11y.rs`, which asserts on role/name/focusable), so
this is a value fix plus a new positive assertion — nothing to re-bless.

**Files:**
- Modify: `crates/buiy_verify/src/a11y.rs` (the `snapshot_tree` map, line 47; add an import)
- Test: `crates/buiy_verify/tests/a11y.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/buiy_verify/tests/a11y.rs` (it already has the `entity(index)`
helper and imports `A11yNodeView`/`A11yRole`):

```rust
#[test]
fn snapshot_entity_field_is_the_canonical_node_id() {
    let e = entity(1);
    let nodes = vec![A11yNodeView {
        entity: e,
        role: A11yRole::Button,
        name: "Save".into(),
        description: "".into(),
        focusable: true,
    }];
    let json = snapshot_tree(&nodes);
    let expected = buiy_core::a11y::translate::node_id_for(e).0;
    assert!(
        json.contains(&format!("\"entity\":{expected}")),
        "snapshot must emit the canonical NodeId ref (to_bits()+1), got: {json}",
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p buiy_verify --test a11y snapshot_entity_field_is_the_canonical_node_id`
Expected: FAIL — assertion fails; the JSON contains the raw `to_bits()` value
(one less than `expected`).

- [ ] **Step 3: Fix the serializer**

In `crates/buiy_verify/src/a11y.rs`, add the import beneath the existing `use`
(line 5 area):

```rust
use buiy_core::a11y::translate::node_id_for;
```

Then change the `entity` field in the `snapshot_tree` map (line 47) from:

```rust
            entity: n.entity.to_bits(),
```

to:

```rust
            // Canonical AccessKit ref: node_id_for(entity) = to_bits() + 1.
            // This is the id an inbound ActionRequest's `target` carries, so the
            // snapshot's `entity` field round-trips with `entity_for_node_id`.
            entity: node_id_for(n.entity).0,
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p buiy_verify --test a11y`
Expected: PASS — the new test plus the three existing tests
(`snapshot_tree_serializes_to_stable_json`, `diff_returns_none_for_identical_snapshots`,
`diff_returns_some_for_different_snapshots`) all green (the existing ones assert
on role/name/focusable, unaffected by the `entity` value change).

- [ ] **Step 5: Commit**

```bash
git add crates/buiy_verify/src/a11y.rs crates/buiy_verify/tests/a11y.rs
git commit -m "fix(a11y): snapshot serializer emits the canonical NodeId ref, not raw entity bits"
```

---

### Task 3: extend `A11yRole` with the seven new variants

The widget catalog (Phase 1d) needs `Checkbox`/`Switch`/`Slider`/`TextInput`/
`MultilineTextInput`/`Region`/`Group` roles, and TextInput retires the
`A11yRole::Text` stopgap later. Add the variants now (no widget emits them yet,
so the live tree is unchanged), updating **both** stringifiers in the same change
so snapshots never show `"Unknown"`. `role_to_accesskit` (in `buiy_core`) has an
**exhaustive** match — the compiler forces the new arms; `role_to_str` (in
`buiy_verify`) has a `_` wildcard, so the `KNOWN_ROLES` test is the forcing
function there.

**Files:**
- Modify: `crates/buiy_core/src/a11y/mod.rs` (the `A11yRole` enum, ~lines 28-40)
- Modify: `crates/buiy_core/src/a11y/translate.rs` (`role_to_accesskit`, lines 44-56)
- Modify: `crates/buiy_verify/src/a11y.rs` (`role_to_str` lines 28-41; `KNOWN_ROLES` lines 90-100)

- [ ] **Step 1: Write the failing test — extend `KNOWN_ROLES`**

In `crates/buiy_verify/src/a11y.rs`, extend the `KNOWN_ROLES` array (lines 90-100)
to include the seven new variants:

```rust
    const KNOWN_ROLES: &[A11yRole] = &[
        A11yRole::Generic,
        A11yRole::Button,
        A11yRole::Link,
        A11yRole::Image,
        A11yRole::Text,
        A11yRole::Heading,
        A11yRole::Dialog,
        A11yRole::AlertDialog,
        A11yRole::Tooltip,
        A11yRole::Checkbox,
        A11yRole::Switch,
        A11yRole::Slider,
        A11yRole::TextInput,
        A11yRole::MultilineTextInput,
        A11yRole::Region,
        A11yRole::Group,
    ];
```

- [ ] **Step 2: Run to verify it fails (compile error — variants undefined)**

Run: `cargo test -p buiy_verify --test a11y role_to_str_handles_every_known_variant`
Expected: FAIL — compile error, `no variant named 'Checkbox' found for enum 'A11yRole'` (and the others).

- [ ] **Step 3: Add the seven variants to the `A11yRole` enum**

In `crates/buiy_core/src/a11y/mod.rs`, extend the enum (insert before the
`// Phase 0 stops here` comment, after `Tooltip,`):

```rust
    Tooltip,
    Checkbox,
    Switch,
    Slider,
    TextInput,
    MultilineTextInput,
    Region,
    Group,
    // Phase 0 stops here; full taxonomy is in the foundation spec accessibility.md.
```

- [ ] **Step 4: Run to verify the next failure (exhaustive match in `buiy_core`)**

Run: `cargo test -p buiy_core`
Expected: FAIL — compile error in `translate.rs`, `non-exhaustive patterns:
'A11yRole::Checkbox', … not covered` in `role_to_accesskit` (the match has no
wildcard, which is the forcing function).

- [ ] **Step 5: Add the seven arms to `role_to_accesskit`**

In `crates/buiy_core/src/a11y/translate.rs`, extend `role_to_accesskit` (lines
44-56), inserting before the closing brace of the `match`:

```rust
        A11yRole::Tooltip => Role::Tooltip,
        A11yRole::Checkbox => Role::CheckBox,
        A11yRole::Switch => Role::Switch,
        A11yRole::Slider => Role::Slider,
        A11yRole::TextInput => Role::TextInput,
        A11yRole::MultilineTextInput => Role::MultilineTextInput,
        A11yRole::Region => Role::Region,
        A11yRole::Group => Role::Group,
    }
```

API note (verify against the pinned accesskit): the variant is `Role::CheckBox`
(capital `B`). Confirm each of the seven `accesskit::Role` variant spellings
against `Cargo.lock`'s pinned accesskit at implementation time; all seven exist
in both 0.21 and 0.24.

- [ ] **Step 6: Run to verify the next failure (`role_to_str` returns "Unknown")**

Run: `cargo test -p buiy_verify --test a11y role_to_str_handles_every_known_variant`
Expected: FAIL — `buiy_core` now compiles, but the test asserts each
`KNOWN_ROLES` entry stringifies to non-`"Unknown"`; the seven new variants hit
the `_ => "Unknown"` wildcard.

- [ ] **Step 7: Add the seven arms to `role_to_str`**

In `crates/buiy_verify/src/a11y.rs`, extend `role_to_str` (lines 28-41),
inserting before the `_ => "Unknown",` wildcard:

```rust
        A11yRole::Tooltip => "Tooltip",
        A11yRole::Checkbox => "Checkbox",
        A11yRole::Switch => "Switch",
        A11yRole::Slider => "Slider",
        A11yRole::TextInput => "TextInput",
        A11yRole::MultilineTextInput => "MultilineTextInput",
        A11yRole::Region => "Region",
        A11yRole::Group => "Group",
        _ => "Unknown",
```

- [ ] **Step 8: Run to verify everything passes**

Run: `cargo test -p buiy_core --test a11y_translate && cargo test -p buiy_verify --test a11y`
Expected: PASS — `role_to_str_handles_every_known_variant` covers all 16
variants; Task 1 + Task 2 tests still green.

- [ ] **Step 9: Commit**

```bash
git add crates/buiy_core/src/a11y/mod.rs crates/buiy_core/src/a11y/translate.rs crates/buiy_verify/src/a11y.rs
git commit -m "feat(a11y): add Checkbox/Switch/Slider/TextInput/MultilineTextInput/Region/Group roles"
```

---

## Phase 0 done — final gate

- [ ] **Run the full project check command** (mirrors CI):

```bash
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace
```

Expected: all green. (On macOS/Windows drop `xvfb-run -a`.)

- [ ] **Confirm no behavior change to the live tree:** `build_tree`,
  `build_tree_update`, and `push_tree_updates` are untouched; only the
  `buiy_verify` snapshot `entity` field value changed (raw bits → canonical
  NodeId), and the seven new roles are inert until widgets emit them in Phase 1d.

**Next:** write the Phase 1a detailed plan (decomposed component surface) by
re-invoking `superpowers:writing-plans` against `semantic-tree.md` §1–§6 — after
the BSN/0.19 bump lands and the accesskit 0.24 setter signatures are confirmed
against `Cargo.lock`.
