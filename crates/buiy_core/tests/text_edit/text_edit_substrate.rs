//! E1 — editor substrate. `TextEditState` over `Editor<'static>` /
//! `BufferRef::Owned`, the policy markers, and (Task 2+) the
//! `TextBufferAccess` seam. Headless: shaping uses the embedded Fira Sans
//! latin subset, no adapter anywhere. The facade boundary itself is pinned
//! by `tests/text_facade_boundary.rs`.

use bevy::prelude::*;
use buiy_core::text::BuiyTextPlugin;
use buiy_core::text::edit::{Disabled, Placeholder, ReadOnly, SingleLine, TextEditState};
use cosmic_text::Metrics;

/// `Editor::new` is FontSystem-free (struct construction over the owned
/// buffer), so `TextEditState::new` builds without a lock — the same
/// lock-free construction contract `TextBuffer::new` honors (architecture
/// § 1.2: TextSync is not a lock site).
#[test]
fn text_edit_state_constructs_without_a_font_system() {
    let state = TextEditState::new(Metrics::new(16.0, 19.2));
    // A fresh editor's buffer is empty and unshaped: zero layout runs.
    state.with_buffer(|buffer| {
        assert_eq!(
            buffer.layout_runs().count(),
            0,
            "fresh editor buffer is unshaped"
        );
    });
    assert_eq!(
        state.intrinsics(),
        None,
        "no intrinsics cached before measure"
    );
}

/// `for_font_size` is the core constructor that keeps `cosmic_text::Metrics`
/// out of downstream crates (`buiy_widgets::TextInput::new` calls it). A Buiy
/// `f32` size in ⇒ an editor whose buffer metrics are `(size, size * 1.2)`.
#[test]
fn for_font_size_constructs_an_editor_with_matching_metrics() {
    // for_font_size(16.0) ⇒ an editor whose buffer metrics are (16, 19.2).
    let state = buiy_core::text::edit::TextEditState::for_font_size(16.0);
    let (fs, lh) = state.metrics_for_test();
    assert_eq!(fs, 16.0);
    assert!((lh - 19.2).abs() < 1e-4, "line height = size * 1.2: {lh}");
    assert_eq!(state.value(), "", "a fresh editor is empty");
}

/// The four policy markers are plain zero-size / string components: they
/// construct, compare, and (Task 1.4) reflect-register. Behavior is E2–E6;
/// E1 only proves they exist and gate (a query can filter on them).
#[test]
fn policy_markers_construct_and_gate() {
    let mut world = World::new();
    let editable = world
        .spawn(TextEditState::new(Metrics::new(16.0, 19.2)))
        .id();
    let read_only = world
        .spawn((TextEditState::new(Metrics::new(16.0, 19.2)), ReadOnly))
        .id();
    let disabled = world
        .spawn((TextEditState::new(Metrics::new(16.0, 19.2)), Disabled))
        .id();
    let single = world
        .spawn((TextEditState::new(Metrics::new(16.0, 19.2)), SingleLine))
        .id();
    world.spawn((
        TextEditState::new(Metrics::new(16.0, 19.2)),
        Placeholder(String::from("type here")),
    ));

    // The markers gate: an editable, non-Disabled, non-ReadOnly query.
    let mut q = world
        .query_filtered::<Entity, (With<TextEditState>, Without<Disabled>, Without<ReadOnly>)>();
    let editable_ids: Vec<Entity> = q.iter(&world).collect();
    assert!(editable_ids.contains(&editable));
    assert!(
        editable_ids.contains(&single),
        "SingleLine is still editable"
    );
    assert!(
        !editable_ids.contains(&read_only),
        "ReadOnly is filtered out"
    );
    assert!(
        !editable_ids.contains(&disabled),
        "Disabled is filtered out"
    );

    // Placeholder carries its string.
    let mut pq = world.query::<&Placeholder>();
    assert_eq!(pq.iter(&world).next().unwrap().0, "type here");
}

/// The policy markers reflect-register (BSN / inspectors — the
/// authoring-surface convention `Text` follows).
#[test]
fn policy_markers_are_registered_for_reflection() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BuiyTextPlugin::default());
    app.update();
    let registry = app.world().resource::<AppTypeRegistry>().read();
    for name in [
        "buiy_core::text::edit::state::ReadOnly",
        "buiy_core::text::edit::state::Disabled",
        "buiy_core::text::edit::state::SingleLine",
        "buiy_core::text::edit::state::Placeholder",
    ] {
        assert!(
            registry.get_with_type_path(name).is_some(),
            "marker not registered: {name}",
        );
    }
}

use buiy_core::text::edit::TextBufferAccess;
use buiy_core::text::{IntrinsicWidths, TextBuffer};

/// On a DISPLAY-ONLY entity (`TextBuffer`, no `TextEditState`) the accessor
/// routes to `TextBuffer.buffer` and `TextBuffer`'s intrinsics slot.
#[test]
fn accessor_routes_to_display_buffer_when_no_editor() {
    let mut world = World::new();
    let e = world.spawn(TextBuffer::new(Metrics::new(16.0, 19.2))).id();

    let mut q = world.query::<TextBufferAccess>();
    let mut item = q.get_mut(&mut world, e).unwrap();
    // Cache round-trips through the accessor onto the display component.
    assert_eq!(item.intrinsics(), None);
    item.cache_intrinsics(IntrinsicWidths {
        min_content: 3.0,
        max_content: 9.0,
    });
    assert_eq!(
        item.intrinsics(),
        Some(IntrinsicWidths {
            min_content: 3.0,
            max_content: 9.0
        })
    );
    item.with_buffer_mut(|buffer| buffer.set_size(Some(120.0), None));
    item.with_buffer(|buffer| assert_eq!(buffer.size().0, Some(120.0)));

    // The write landed on the display component (proof it routed there).
    let tb = world.get::<TextBuffer>(e).unwrap();
    assert_eq!(tb.buffer.size().0, Some(120.0));
    assert_eq!(
        tb.intrinsics(),
        Some(IntrinsicWidths {
            min_content: 3.0,
            max_content: 9.0
        })
    );
}

/// On an EDITABLE entity (both components present) the accessor PREFERS the
/// editor-owned buffer and the editor's intrinsics slot; the display
/// component is untouched.
#[test]
fn accessor_prefers_editor_buffer_when_present() {
    let mut world = World::new();
    let e = world
        .spawn((
            TextBuffer::new(Metrics::new(16.0, 19.2)),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();

    let mut q = world.query::<TextBufferAccess>();
    let mut item = q.get_mut(&mut world, e).unwrap();
    item.cache_intrinsics(IntrinsicWidths {
        min_content: 7.0,
        max_content: 11.0,
    });
    item.with_buffer_mut(|buffer| buffer.set_size(Some(250.0), None));
    item.with_buffer(|buffer| assert_eq!(buffer.size().0, Some(250.0)));

    // The editor buffer got the write + cache; the DISPLAY buffer did not.
    let tb = world.get::<TextBuffer>(e).unwrap();
    assert_eq!(
        tb.buffer.size().0,
        None,
        "display buffer untouched (editor is authoritative)"
    );
    assert_eq!(tb.intrinsics(), None, "display cache untouched");
    let state = world.get::<TextEditState>(e).unwrap();
    state.with_buffer(|buffer| assert_eq!(buffer.size().0, Some(250.0)));
    assert_eq!(
        state.intrinsics(),
        Some(IntrinsicWidths {
            min_content: 7.0,
            max_content: 11.0
        })
    );
}

/// `invalidate_intrinsics` clears whichever side is authoritative.
#[test]
fn accessor_invalidate_clears_the_authoritative_cache() {
    let mut world = World::new();
    let e = world
        .spawn((
            TextBuffer::new(Metrics::new(16.0, 19.2)),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    let mut q = world.query::<TextBufferAccess>();
    let mut item = q.get_mut(&mut world, e).unwrap();
    item.cache_intrinsics(IntrinsicWidths {
        min_content: 1.0,
        max_content: 2.0,
    });
    assert!(item.intrinsics().is_some());
    item.invalidate_intrinsics();
    assert_eq!(item.intrinsics(), None);
}

/// The C2 content-vs-style split point: `has_edit()` distinguishes an editor
/// entity (owns its content; `TextSync` re-applies STYLE only, never `set_text`)
/// from a display-only entity (content+style path). The read-only companion is
/// what `query.get` yields here.
#[test]
fn has_edit_distinguishes_editor_from_display_entities() {
    use buiy_core::text::TextBuffer;
    use buiy_core::text::edit::{TextBufferAccess, TextEditState};
    use cosmic_text::Metrics;

    let metrics = Metrics::new(16.0, 19.2); // for the display TextBuffer
    let mut world = World::new();
    let display = world.spawn(TextBuffer::new(metrics)).id();
    // Editor uses the facade constructor (for_font_size == Metrics::new(16.0,
    // 19.2), state.rs:170) — the ONE form across all plans.
    let editor = world
        .spawn((TextBuffer::new(metrics), TextEditState::for_font_size(16.0)))
        .id();

    let mut q = world.query::<TextBufferAccess>();
    assert!(
        !q.get(&world, display).unwrap().has_edit(),
        "a display-only entity has no editor"
    );
    assert!(
        q.get(&world, editor).unwrap().has_edit(),
        "an entity with TextEditState owns its buffer"
    );
}

/// The bypass-change-detection contract on the EDITOR arm of `with_buffer_mut`
/// (measure § 7, access.rs decision 4): a width probe is not a damage signal,
/// so a mutable buffer write through the accessor must NOT tick
/// `Changed<TextEditState>`. This guards the editor arm DIRECTLY — the
/// steady-frame parity test cannot, because nothing in the crate reads
/// `Changed<TextEditState>` yet and Taffy's layout cache keeps the measure
/// closure (the only accessor caller) from running on a no-change frame, so a
/// broken editor-arm bypass would leave that test green (verified by mutation:
/// dropping `bypass_change_detection()` from the editor arm does not fail it).
/// Mirrors the `tests/render_clip_rects.rs:420-425` `Ref::is_changed()` probe.
#[test]
fn with_buffer_mut_bypasses_change_detection_on_the_editor_arm() {
    let mut world = World::new();
    let e = world
        .spawn((
            TextBuffer::new(Metrics::new(16.0, 19.2)),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    // Advance the change-detection baseline past the spawn tick, so a later
    // `Ref::is_changed()` reports only writes made AFTER this point (the spawn
    // itself no longer reads as "changed").
    world.clear_trackers();

    // A mutable buffer write through the accessor — the measure/commit/sync
    // shape path. The editor is authoritative here (both components present).
    let mut q = world.query::<TextBufferAccess>();
    let mut item = q.get_mut(&mut world, e).unwrap();
    item.with_buffer_mut(|buffer| buffer.set_size(Some(180.0), None));
    // The write landed on the editor buffer (it is authoritative).
    let state = world.get::<TextEditState>(e).unwrap();
    state.with_buffer(|buffer| assert_eq!(buffer.size().0, Some(180.0)));

    // The decisive guard: the write did NOT tick `Changed<TextEditState>`.
    // A `DerefMut` on the `Mut<TextEditState>` (the broken arm) would have.
    let mut rq = world.query::<Ref<TextEditState>>();
    let state_ref = rq.get(&world, e).expect("editor entity has TextEditState");
    assert!(
        !state_ref.is_changed(),
        "with_buffer_mut on the editor arm must bypass change detection \
         (a width probe is not a damage signal — measure § 7)",
    );
}

use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::{ComputedTextLayout, Text, TextMeasureCallCount};
use buiy_core::{CorePlugin, Node, ResolvedLayout};

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

fn settle(app: &mut App) {
    app.update();
    app.update();
}

/// Seed an editor entity's OWNED content via the explicit `EditCommand::Insert`
/// verb (C2 § 2.3) — the editor owns its content; the display `Text`→editor
/// content seam is gone (C2 § 2.1). Locks the shared `FontSystem` the way the
/// edit-path tests do.
fn seed_editor(app: &mut App, editor: Entity, content: &str) {
    use buiy_core::text::SharedFontSystem;
    use buiy_core::text::edit::EditCommand;
    let fonts = app.world().resource::<SharedFontSystem>().clone();
    let mut fs = fonts.lock();
    let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
    state.apply(&mut fs, EditCommand::Insert(content.into()), false, false);
}

/// THE flagship invariant: an entity with `TextEditState` shapes, measures, and
/// lays out IDENTICALLY to the equivalent display-only entity — because both
/// route through `TextBufferAccess`, editor-preferred. The editor's owned buffer
/// is the one that gets the text (seeded via `EditCommand::Insert` — the editor
/// owns its content; the display `Text` seam is gone, C2 § 2.1) and produces the
/// layout.
#[test]
fn editor_entity_lays_out_identically_to_display_entity() {
    let mut app = text_app();
    let display = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("hello editor world")),
        ))
        .id();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::new()), // inert display carrier (editor owns its content)
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    // Seed the editor's OWNED content via the explicit verb (C2 § 2.3): the
    // display `Text`→editor seam is gone, so equivalent content reaches the
    // editor through `Insert`, not by TextSync lowering the display `Text`.
    seed_editor(&mut app, editor, "hello editor world");
    // Two FlexStart rows so cross-axis stretch doesn't mask measured height.
    for child in [display, editor] {
        app.world_mut()
            .spawn((
                Node,
                Style::default()
                    .flex_row()
                    .align_items(buiy_core::layout::AlignItems::FlexStart)
                    .width_px(600.0)
                    .height_px(100.0),
            ))
            .add_child(child);
    }
    settle(&mut app);

    let d_layout = app.world().get::<ResolvedLayout>(display).unwrap().size;
    let e_layout = app.world().get::<ResolvedLayout>(editor).unwrap().size;
    assert_eq!(
        d_layout, e_layout,
        "editor entity sizes identically to display"
    );

    let d_computed = app
        .world()
        .get::<ComputedTextLayout>(display)
        .unwrap()
        .clone();
    let e_computed = app
        .world()
        .get::<ComputedTextLayout>(editor)
        .unwrap()
        .clone();
    assert_eq!(d_computed, e_computed, "identical settled line geometry");

    // The editor's OWNED buffer is the one that holds the text (proof the
    // accessor routed sync + measure + commit to it, not the display buffer).
    let state = app.world().get::<TextEditState>(editor).unwrap();
    state.with_buffer(|buffer| {
        assert!(
            buffer.layout_runs().next().is_some(),
            "editor buffer is shaped"
        );
        assert!(
            buffer.size().0.is_some(),
            "editor buffer committed at a final width"
        );
    });
}

/// Display-only entities are unaffected by the editor seam: a frame with
/// BOTH an editor and a display entity still measures each exactly once on
/// change, and the steady frame measures ZERO — the seam CONVERGES (no
/// perpetual re-shape). This proves convergence, not the change-detection
/// bypass: nothing reads `Changed<TextEditState>` in E1 and Taffy's layout
/// cache keeps the measure closure from running on a no-change frame, so the
/// editor-arm bypass is unobservable here — it is guarded directly by
/// `with_buffer_mut_bypasses_change_detection_on_the_editor_arm`.
#[test]
fn the_seam_preserves_the_zero_measure_steady_frame() {
    let mut app = text_app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("steady")),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default(), Text(String::from("steady"))));
    // a parent so they lay out
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(400.0)
                .height_px(200.0),
        ))
        .add_child(editor);
    settle(&mut app);
    // Two-flush discipline (the `text_commit.rs:250-253`
    // steady_state_zero_measure_calls_and_zero_reshapes pattern): the editor
    // entity's `Added<TextBuffer>` echo fires on frame 2 (editor-first
    // re-sync fills the editor buffer + invalidates intrinsics → one more
    // re-measure), so the editor path converges ONE frame later than a
    // display entity. Flush the cascade remnant, THEN assert on the truly
    // steady frame — a single `update()` here would flake frame-3-vs-frame-4.
    app.update(); // flush the creation-echo remnant
    app.update(); // THE steady frame
    assert_eq!(
        app.world().resource::<TextMeasureCallCount>().0,
        0,
        "no-change frame measures zero — the editor seam converges (no perpetual re-shape)",
    );
}
