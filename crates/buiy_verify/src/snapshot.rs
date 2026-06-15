//! Tiers 1–2 — structured snapshots (snapshots.md).
//!
//! The two cheapest, most deterministic rungs of the verification pyramid:
//!
//! - **Tier 1** ([`assert_layout_snapshot`]) snapshots every entity's resolved
//!   box (`ResolvedLayout.position`/`.size`) as a stable, `Name`-keyed Display
//!   dump (gate #5).
//! - **Tier 2** ([`assert_display_list_snapshot`]) snapshots the whole CPU
//!   display-list handoff holistically: the [`ExtractedNodes`] paint order plus
//!   the packed [`InstanceBuckets`] draw order, in one Display dump — plus a
//!   byte-exact [`assert_instance_hex_snapshot`] on the [`PackedInstance`]
//!   px→logical packing.
//!
//! Both emit a **purpose-built Display dump**, never raw `Debug`/serde, so the
//! artifact is decoupled from private field names and `Entity` allocation bits
//! (which vary with spawn order). Entities render by [`Name`]; floats round via
//! the shared [`round`]; each dump carries a format-version header so a format
//! change is a single visible line (snapshots.md § "Why a Display dump").
//!
//! Pure-CPU, headless, sub-millisecond, 100% deterministic: no GPU, no window.
//! The `assert_*` helpers are `#[track_caller]` so insta writes each `.snap`
//! beside the *calling* test file (`crates/<crate>/tests/snapshots/`), even
//! though the helper bodies live here in `buiy_verify`.

use std::collections::HashMap;
use std::fmt::Write as _;

use bevy::prelude::*;

use buiy_core::components::ResolvedLayout;
#[cfg(doc)]
use buiy_core::render::buckets::InstanceBuckets;
use buiy_core::render::buckets::pack_view;
use buiy_core::render::extract::{ExtractedNode, ExtractedNodes};
use buiy_core::render::instance::PackedInstance;

// ---------------------------------------------------------------------------
// Shared dump primitives (Task 2.1) — used by both Tier 1 and Tier 2.
// ---------------------------------------------------------------------------

/// Decimal places floats are rounded to in every dump. Two decimals kills the
/// last-ULP churn from the Taffy / clip-space math while staying diff-readable
/// (snapshots.md § Tier 1).
pub const ROUND_DP: usize = 2;

/// Format-version header for the Tier-1 layout dump. A formatter change bumps
/// the `vN` and re-blesses every layout `.snap` as one conscious, visible diff
/// (snapshots.md § Verification #4).
pub const LAYOUT_DUMP_VERSION: &str = "# buiy-layout-dump v1";

/// Format-version header for the Tier-2 display-list dump. See
/// [`LAYOUT_DUMP_VERSION`].
pub const DISPLAY_LIST_DUMP_VERSION: &str = "# buiy-display-list-dump v1";

/// Round a float to [`ROUND_DP`] decimals and render it diff-stably: trailing
/// zeros and a bare trailing `.` are stripped (`50.0 → "50"`), and `-0.0`
/// normalizes to `"0"` so a sub-ULP negative never prints a spurious `-0`. The
/// shared rounding helper for Tier 1 + Tier 2 (snapshots.md § Tier 1, §
/// Verification #2).
pub fn round(v: f32) -> String {
    // Round to ROUND_DP decimals. `{:.*}` does round-half-away; the result is
    // a fixed-decimal string we then trim.
    let mut s = format!("{v:.*}", ROUND_DP);
    // Normalize "-0", "-0.00", etc. to a single "0" before trimming so the
    // sign never leaks for a value that rounded to zero.
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s = s[1..].to_string();
    }
    // Strip trailing zeros, then a trailing dot, only when a dot is present.
    if s.contains('.') {
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        s = trimmed.to_string();
    }
    s
}

// ---------------------------------------------------------------------------
// `#[track_caller]` insta bridge — write `.snap` beside the CALLING test file.
// ---------------------------------------------------------------------------

/// Assert `value` against the named text snapshot, writing the `.snap` beside
/// the **caller's** source file (`<caller-dir>/snapshots/<name>.snap`) rather
/// than beside this `buiy_verify` module. This is the seam that lets the dump
/// helpers live in `buiy_verify` while their `.snap`s live next to the
/// `buiy_core` tests that call them.
///
/// Mechanics: insta keys a `.snap` off the *macro call site* (`file!()`,
/// `module_path!()`). Because the helper is a plain `fn`, the macro would key
/// off `buiy_verify`'s source and collide every caller's snapshot. We instead
/// call `insta::_macro_support::assert_snapshot` directly with the caller's
/// `Location` (via `#[track_caller]`), an empty `module_path`, and
/// `prepend_module_to_snapshot(false)`, so the file is exactly
/// `<caller-dir>/snapshots/<name>.snap`. The workspace root is resolved by
/// insta from `CARGO_MANIFEST_DIR` (same workspace ⇒ same root).
#[track_caller]
fn assert_named_snapshot(name: &str, value: String) {
    let loc = std::panic::Location::caller();
    // insta joins `workspace_root / dirname(assertion_file) / snapshot_path /
    // <name>.snap`. `Location::file()` is workspace-relative, matching what
    // `file!()` yields at the call site.
    let workspace = insta::_macro_support::get_cargo_workspace(
        insta::_macro_support::Workspace::DetectWithCargo(env!("CARGO_MANIFEST_DIR")),
    );

    let mut settings = insta::Settings::clone_current();
    // Filename is exactly `<name>.snap` (no `module__` prefix) — matches the
    // dump-format examples in snapshots.md (e.g. `flex_row_basic.snap`).
    settings.set_prepend_module_to_snapshot(false);
    settings.set_snapshot_path("snapshots");
    let _guard = settings.bind_to_scope();

    insta::_macro_support::assert_snapshot(
        (Some(name.to_string()), value.as_str()).into(),
        workspace.as_path(),
        // function_name only disambiguates auto-named snapshots; we always pass
        // an explicit `name`, so an empty string is fine.
        "",
        // Empty module_path + `prepend_module_to_snapshot(false)` ⇒ no prefix.
        "",
        loc.file(),
        loc.line(),
        // The "expression" shown in the failure diff header.
        name,
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// Tier 1 — layout-number snapshots (gate #5).
// ---------------------------------------------------------------------------

/// Run one `update()` on `app`, then snapshot every entity's resolved box as a
/// stable [`layout_dump`], keyed by `name`. Pure-CPU: the caller wires
/// `MinimalPlugins + CorePlugin + LayoutPlugin` (no RenderApp). The `.snap`
/// lands beside the calling test (`<caller-dir>/snapshots/<name>.snap`).
#[track_caller]
pub fn assert_layout_snapshot(app: &mut App, name: &str) {
    app.update();
    let dump = layout_dump(app.world());
    assert_named_snapshot(name, dump);
}

/// The format-versioned Display dump backing [`assert_layout_snapshot`]:
/// `(name, position, size)` per [`ResolvedLayout`] entity, one per line,
/// indented by `ChildOf` depth, siblings ordered by `Name` then rendered box
/// (position, size) as a content tiebreak — never by `Entity` index. Floats
/// round via [`round`]; an unnamed entity falls back to `entity#<index>` (a
/// flagged, non-diff-stable fixture). The dump never prints raw `Entity` bits
/// (snapshots.md § Tier 1).
pub fn layout_dump(world: &World) -> String {
    let entries = collect_layout_entries(world);

    let mut out = String::new();
    out.push_str(LAYOUT_DUMP_VERSION);
    out.push('\n');
    for e in &entries {
        let indent = "  ".repeat(e.depth);
        let _ = writeln!(
            out,
            "{indent}{name} pos={px},{py} size={sx},{sy}",
            name = e.name,
            px = round(e.position.x),
            py = round(e.position.y),
            sx = round(e.size.x),
            sy = round(e.size.y),
        );
    }
    out
}

/// Total order on a laid-out node's `(name, position, size)` — `Name`, then the
/// rendered box compared component-wise via `f32::total_cmp` (a total order over
/// all floats incl. NaN/±0). Content-derived, so it is a deterministic function
/// of the layout, never of ECS allocation order.
fn cmp_layout_content(a: &(String, Vec2, Vec2), b: &(String, Vec2, Vec2)) -> std::cmp::Ordering {
    a.0.cmp(&b.0)
        .then_with(|| a.1.x.total_cmp(&b.1.x))
        .then_with(|| a.1.y.total_cmp(&b.1.y))
        .then_with(|| a.2.x.total_cmp(&b.2.x))
        .then_with(|| a.2.y.total_cmp(&b.2.y))
}

/// Sort `sibs` into the deterministic content order, then assert no two are
/// indistinguishable. Two siblings identical in `Name` AND box have no
/// content-derived order — their relative order, and their subtrees' order in
/// the dump, would fall back to spawn order. Rather than silently emit a flaky
/// snapshot, refuse: the fixture must give them distinct `Name`s or positions.
fn sort_siblings_by_content(sibs: &mut [Entity], boxes: &HashMap<Entity, (String, Vec2, Vec2)>) {
    sibs.sort_by(|x, y| cmp_layout_content(&boxes[x], &boxes[y]));
    for pair in sibs.windows(2) {
        if cmp_layout_content(&boxes[&pair[0]], &boxes[&pair[1]]) == std::cmp::Ordering::Equal {
            let (name, pos, size) = &boxes[&pair[0]];
            panic!(
                "ambiguous siblings: two entities share Name `{name}`, position {pos:?} and \
                 size {size:?} — the layout dump cannot be made spawn-order-independent. \
                 Give them distinct Names or positions."
            );
        }
    }
}

/// One resolved-layout row, pre-sorted into a stable, content-keyed pre-order
/// tree walk (depth carries the `ChildOf` indentation).
struct LayoutEntry {
    name: String,
    depth: usize,
    position: Vec2,
    size: Vec2,
}

/// Gather every `ResolvedLayout` entity into a stable pre-order list: roots
/// (entities with no `ChildOf`) first, then a depth-first descent through
/// `Children`, siblings ordered by `Name` then rendered box (position, size).
/// The content key is what makes the dump invariant to ECS spawn/archetype
/// order — even when siblings share a `Name`.
fn collect_layout_entries(world: &World) -> Vec<LayoutEntry> {
    // entity -> (name, position, size) for every laid-out entity. `Name` is
    // looked up per-entity via `world.get` (not in the query) because `Name`
    // may be UNREGISTERED in a fixture that tags no entity — `try_query` over
    // an unregistered component returns `None`. `ResolvedLayout` is always
    // registered by `LayoutPlugin`, so its query never fails.
    let mut boxes: HashMap<Entity, (String, Vec2, Vec2)> = HashMap::new();
    let mut q = world
        .try_query::<(Entity, &ResolvedLayout)>()
        .expect("ResolvedLayout is registered by LayoutPlugin");
    for (e, layout) in q.iter(world) {
        let label = entity_label(world.get::<Name>(e), e);
        boxes.insert(e, (label, layout.position, layout.size));
    }

    // Adjacency: parent -> children (only over laid-out entities). `ChildOf`
    // may be unregistered (a flat fixture with no children) — then every
    // entity is a root.
    let mut children: HashMap<Entity, Vec<Entity>> = HashMap::new();
    let mut has_parent: HashMap<Entity, bool> = HashMap::new();
    for &e in boxes.keys() {
        has_parent.entry(e).or_insert(false);
    }
    if let Some(mut cq) = world.try_query::<(Entity, &ChildOf)>() {
        for (e, child_of) in cq.iter(world) {
            if !boxes.contains_key(&e) {
                continue;
            }
            let parent = child_of.parent();
            if boxes.contains_key(&parent) {
                children.entry(parent).or_default().push(e);
                has_parent.insert(e, true);
            }
        }
    }

    // Stable sibling order keyed by CONTENT, not Entity index: by Name, then by
    // the rendered box (position, then size). `Entity::index()` is allocation /
    // spawn-order dependent, so using it as the tiebreak made same-Name siblings
    // (e.g. list rows all `Name::new("row")`) dump in spawn order — a flaky,
    // non-reproducible snapshot. The box is a deterministic function of the
    // layout, so the dump is now genuinely invariant to spawn/archetype order;
    // two siblings identical in name AND box fail loudly (see `sort_siblings_by_content`).
    for siblings in children.values_mut() {
        sort_siblings_by_content(siblings, &boxes);
    }
    let mut roots: Vec<Entity> = boxes.keys().copied().filter(|e| !has_parent[e]).collect();
    sort_siblings_by_content(&mut roots, &boxes);

    let mut out = Vec::with_capacity(boxes.len());
    let mut stack: Vec<(Entity, usize)> = roots.into_iter().rev().map(|e| (e, 0)).collect();
    while let Some((e, depth)) = stack.pop() {
        let (name, position, size) = boxes[&e].clone();
        out.push(LayoutEntry {
            name,
            depth,
            position,
            size,
        });
        if let Some(kids) = children.get(&e) {
            // Push reversed so the lowest sort_key is popped first.
            for &child in kids.iter().rev() {
                stack.push((child, depth + 1));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tier 2 — display-list / paint-order / instance snapshots.
// ---------------------------------------------------------------------------

/// Resolve an [`Entity`] to its human name for a dump: the [`Name`] component
/// when present, else `entity#<index>`. Built from the `World` ONCE and passed
/// into [`display_list_dump`] so that dump fn stays `World`-free and pure
/// (snapshots.md § Tier 2 / README § Resolved #5).
#[derive(Debug, Clone, Default)]
pub struct NameLookup(HashMap<Entity, String>);

impl NameLookup {
    /// Build the entity→name map from every named entity in `world`. An entity
    /// absent from the map renders as `entity#<index>` (the unnamed fallback).
    pub fn from_world(world: &World) -> Self {
        let mut map = HashMap::new();
        // `Name` may be unregistered (no entity is named) — then the map is
        // empty and every entity falls back to `entity#<index>`.
        if let Some(mut q) = world.try_query::<(Entity, &Name)>() {
            for (e, name) in q.iter(world) {
                map.insert(e, name.as_str().to_string());
            }
        }
        Self(map)
    }

    /// Build the lookup from explicit `(entity, name)` pairs — the World-free
    /// constructor for pure-CPU tests that assemble synthetic `ExtractedNode`s
    /// (no spawned `Name` component). Mirrors [`from_world`](Self::from_world);
    /// an entity absent from the pairs renders as `entity#<index>`.
    pub fn from_pairs<I, S>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (Entity, S)>,
        S: Into<String>,
    {
        Self(pairs.into_iter().map(|(e, n)| (e, n.into())).collect())
    }

    /// The label for `e`: its stored `Name`, else `entity#<index>`.
    fn label(&self, e: Entity) -> String {
        self.0
            .get(&e)
            .cloned()
            .unwrap_or_else(|| format!("entity#{}", e.index().index()))
    }
}

/// The label for an entity given its (optional) [`Name`] — the shared
/// unnamed-fallback rule, so Tier 1 and Tier 2 agree.
fn entity_label(name: Option<&Name>, e: Entity) -> String {
    match name {
        Some(n) => n.as_str().to_string(),
        None => format!("entity#{}", e.index().index()),
    }
}

/// `#rrggbbaa` for a color, in sRGB (the authoring space): the `ExtractedNode`
/// color is already theme-resolved, so the magenta `MISSING_TOKEN_FALLBACK`
/// sentinel surfaces here as `#ff00ffff` — a literal that flags an unresolved
/// token (snapshots.md § Tier 2).
fn color_hex(color: Color) -> String {
    let s = Srgba::from(color);
    let to_u8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        to_u8(s.red),
        to_u8(s.green),
        to_u8(s.blue),
        to_u8(s.alpha),
    )
}

/// Render one node's clip field: `none` for the full-view sentinel, else
/// `minx,miny..maxx,maxy` (rounded).
fn clip_str(node: &ExtractedNode) -> String {
    match node.clip {
        None => "none".to_string(),
        Some(c) => format!(
            "{},{}..{},{}",
            round(c.min.x),
            round(c.min.y),
            round(c.max.x),
            round(c.max.y),
        ),
    }
}

/// Snapshot the CPU display-list handoff holistically (nodes in paint order +
/// packed buckets in draw order), keyed by `name`, beside the calling test.
/// See [`display_list_dump`].
#[track_caller]
pub fn assert_display_list_snapshot(nodes: &ExtractedNodes, name: &str, names: &NameLookup) {
    let dump = display_list_dump(nodes, names);
    assert_named_snapshot(name, dump);
}

/// Display dump of an [`ExtractedNodes`] set: every node in `painters_z` stored
/// order (NEVER re-sorted by render — `extract.rs:141` — so a z-sort regression
/// shows as a line reorder), then the [`pack_view`] [`InstanceBuckets`] in
/// `BTreeMap` (draw) order with per-batch `xN` counts. Entities by `Name`;
/// floats via [`round`]; format-version-headered (snapshots.md § Tier 2).
///
/// Color renders as `#rrggbbaa` (sRGB). Token-name rendering (`token:<Name>`)
/// is intentionally NOT done here: the pinned signature carries no `Theme`, and
/// `ExtractedNode.color` is already resolved — so the hex IS the artifact, and
/// the magenta sentinel surfaces as `#ff00ffff` (the unresolved-token signal).
pub fn display_list_dump(nodes: &ExtractedNodes, names: &NameLookup) -> String {
    let mut out = String::new();
    out.push_str(DISPLAY_LIST_DUMP_VERSION);
    out.push('\n');

    out.push_str("[nodes painters_z]\n");
    for (i, node) in nodes.nodes.iter().enumerate() {
        let group = match node.group {
            Some(g) => g.to_string(),
            None => "none".to_string(),
        };
        let _ = writeln!(
            out,
            "{i} {name} rect pos={px},{py} size={sx},{sy} color={color} clip={clip} group={group}",
            name = names.label(node.entity),
            px = round(node.position.x),
            py = round(node.position.y),
            sx = round(node.size.x),
            sy = round(node.size.y),
            color = color_hex(node.color),
            clip = clip_str(node),
        );
    }

    out.push_str("[buckets draw-order]\n");
    let buckets = pack_view(&nodes.nodes);
    for (key, batch) in buckets.batches() {
        let _ = writeln!(
            out,
            "({:?},layer={}) x{}",
            key.primitive,
            key.layer,
            batch.len(),
        );
    }
    out
}

// ---------------------------------------------------------------------------
// The byte-exact `PackedInstance` hex check.
// ---------------------------------------------------------------------------

/// Hex-dump a [`PackedInstance`] as `bytemuck::bytes_of(p)` — a byte-exact
/// snapshot of the GPU upload payload (52 B → 104 hex chars), independent of
/// the Display dump's format version. A packing arithmetic change (e.g. the
/// half-size sign bug `render_instance.rs` regression-tests) flips the hex even
/// when the rounded Display dump rounds it away (snapshots.md § byte-exact).
///
/// **Endianness:** `bytes_of` is host-endian. CI and dev are both
/// little-endian x86-64, and the hex is a within-repo regression artifact (not
/// a cross-host wire format), so this is acceptable. A big-endian CI host would
/// be a conscious change.
pub fn instance_hex(p: &PackedInstance) -> String {
    let bytes = bytemuck::bytes_of(p);
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Assert one [`PackedInstance`]'s [`instance_hex`] against the named snapshot,
/// beside the calling test. The byte-exact complement to the Display dump.
#[track_caller]
pub fn assert_instance_hex_snapshot(p: &PackedInstance, name: &str) {
    assert_named_snapshot(name, instance_hex(p));
}

// ---------------------------------------------------------------------------
// Per-timestamp animation snapshots (Tier 2, opt-in — Decision 8).
// ---------------------------------------------------------------------------

/// Snapshot the display-list dump at each virtual timestamp in `steps`,
/// advancing `Time<Virtual>` to each **absolute** logical time (not wall-clock)
/// between captures. One `.snap` per step, keyed `<name>@<t_ms>` (e.g.
/// `caret_blink@0`, `caret_blink@250`), so a timing regression shows as a diff
/// in exactly the frame whose curve drifted. Pure-CPU — the dump is a text
/// artifact, so a 3-sample sequence costs ~3× a single dump, not a pixel
/// capture (snapshots.md § Per-timestamp).
///
/// Opt-in per fixture: enroll a fixture only when its *timing curve* is the
/// behavior under test (a custom easing, a staged reveal, the caret blink).
/// Default sampling is three logical timestamps named by the caller.
#[track_caller]
pub fn assert_display_list_snapshot_at(app: &mut App, name: &str, steps: &[std::time::Duration]) {
    // Pin the virtual clock to manual stepping FIRST: under the default
    // `TimeUpdateStrategy::Automatic`, every `app.update()` advances
    // `Time<Virtual>` by the wall-clock delta since the previous update, so the
    // captured frame's logical time would be `t + (accumulated wall-clock)` —
    // non-reproducible, and once the wall-clock drift exceeds a step gap
    // `advance_virtual_to`'s `checked_sub` underflows to ZERO and silently stops
    // advancing. Pinning makes `advance_virtual_to` the SOLE clock driver, which
    // is the byte-for-byte determinism this function's contract promises.
    // (Regression: `wall_clock_does_not_leak_into_the_per_timestamp_clock`.)
    pin_manual_virtual_clock(app);
    for &t in steps {
        // Drive the manual virtual clock to the ABSOLUTE logical time `t` (the
        // landed `Time<Virtual>::advance_by` mechanism, text_caret_selection.rs),
        // then run one update so the animation systems observe the new clock —
        // Bevy's `TimePlugin` syncs `Time<Virtual>` into the generic `Time` at
        // the head of each update, so no manual clock mirroring is needed.
        advance_virtual_to(app, t);
        app.update();

        let names = NameLookup::from_world(app.world());
        let nodes = extract_nodes_from_world(app.world());
        let dump = display_list_dump(&nodes, &names);
        let keyed = format!("{name}@{}", t.as_millis());
        assert_named_snapshot(&keyed, dump);
    }
}

/// Pin `Time<Virtual>` to manual stepping by installing
/// [`TimeUpdateStrategy::ManualDuration(ZERO)`], so each `app.update()` advances
/// the virtual clock by zero and [`advance_virtual_to`] is the only thing that
/// moves it. Idempotent — overwriting the resource each call is harmless. This
/// is the substrate of the per-timestamp determinism guarantee; without it the
/// `TimePlugin`'s automatic wall-clock advance contaminates the logical time.
fn pin_manual_virtual_clock(app: &mut App) {
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::ZERO,
    ));
}

/// Advance `Time<Virtual>` to an absolute logical time `t` (since clock start)
/// by stepping the remaining delta. Steps are expected monotonic; a backwards
/// `t` is a no-op (`advance_by` cannot rewind). Combined with
/// [`pin_manual_virtual_clock`] (the manual-clock pin) this makes per-timestamp
/// snapshots reproducible byte-for-byte regardless of wall-clock.
fn advance_virtual_to(app: &mut App, t: std::time::Duration) {
    let mut virt = app.world_mut().resource_mut::<Time<Virtual>>();
    let elapsed = virt.elapsed();
    let delta = t.checked_sub(elapsed).unwrap_or(std::time::Duration::ZERO);
    virt.advance_by(delta);
}

/// Build an `ExtractedNodes` from a laid-out world by reading each entity's
/// resolved box + background through the production `extracted_node_for`,
/// ordered by `Name` then rendered box (position, size) for determinism — never
/// by `Entity` index (spawn-order dependent; same fix as the Tier-1 layout
/// sort). Pure-CPU: the same single record builder the RenderApp's extract uses.
fn extract_nodes_from_world(world: &World) -> ExtractedNodes {
    use buiy_core::render::components::Background;
    use buiy_core::render::extract::extracted_node_for;
    use buiy_core::theme::Theme;

    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();

    let mut rows: Vec<(String, ExtractedNode)> = Vec::new();
    // Query only the always-registered `ResolvedLayout`; the optional paint
    // inputs (`GlobalTransform`/`Background`/`Name`) are looked up per-entity
    // via `world.get`, which tolerates an unregistered component (a fixture
    // that tags none) where `try_query` would return `None`.
    let mut q = world
        .try_query::<(Entity, &ResolvedLayout)>()
        .expect("ResolvedLayout is registered by LayoutPlugin");
    for (e, layout) in q.iter(world) {
        let gt = world
            .get::<GlobalTransform>(e)
            .copied()
            .unwrap_or(GlobalTransform::IDENTITY);
        let bg = world.get::<Background>(e);
        let node = extracted_node_for(e, &gt, layout, bg, None, &theme);
        rows.push((entity_label(world.get::<Name>(e), e), node));
    }
    // Content tiebreak (position, then size) via `total_cmp`, NOT `Entity::index`
    // — so same-`Name` nodes (e.g. list rows) order deterministically by their
    // rendered box rather than by spawn order.
    rows.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.position.x.total_cmp(&b.1.position.x))
            .then_with(|| a.1.position.y.total_cmp(&b.1.position.y))
            .then_with(|| a.1.size.x.total_cmp(&b.1.size.x))
            .then_with(|| a.1.size.y.total_cmp(&b.1.size.y))
    });

    ExtractedNodes {
        nodes: rows.into_iter().map(|(_, n)| n).collect(),
        ..Default::default()
    }
}

#[cfg(test)]
mod time_determinism {
    use super::*;
    use std::time::Duration;

    /// The per-timestamp snapshot determinism guarantee: `advance_virtual_to`
    /// must be the SOLE driver of `Time<Virtual>`, so wall-clock between updates
    /// never leaks into the captured logical time.
    ///
    /// Phase (a) proves the bug is real — on the default `Automatic` clock a
    /// `sleep` between updates DOES advance the virtual clock past the requested
    /// time. Phase (b) proves [`pin_manual_virtual_clock`] fixes it: the same
    /// sequence lands EXACTLY on the logical time regardless of wall-clock.
    #[test]
    fn wall_clock_does_not_leak_into_the_per_timestamp_clock() {
        // (a) precondition — the Automatic clock leaks wall-clock.
        {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            advance_virtual_to(&mut app, Duration::from_millis(100));
            app.update();
            std::thread::sleep(Duration::from_millis(20));
            app.update(); // no advance — yet the Automatic clock moves anyway
            let leaked = app.world().resource::<Time<Virtual>>().elapsed();
            assert!(
                leaked > Duration::from_millis(100),
                "precondition: the default Automatic clock must leak wall-clock \
                 (got {leaked:?}); if this fails the bug model changed"
            );
        }

        // (b) fix — pinning the manual clock makes advance_virtual_to the sole
        // driver, so the identical sequence lands exactly on 100ms.
        {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            pin_manual_virtual_clock(&mut app);
            advance_virtual_to(&mut app, Duration::from_millis(100));
            app.update();
            std::thread::sleep(Duration::from_millis(20));
            app.update(); // no advance — the pinned clock must NOT move
            let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
            assert_eq!(
                elapsed,
                Duration::from_millis(100),
                "pinned clock: wall-clock must not leak into the virtual clock"
            );
        }
    }
}
