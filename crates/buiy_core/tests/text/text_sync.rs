//! `BuiyLayoutStep::TextSync` trigger-set tests (architecture § 5.1 row 1;
//! measure-and-layout § 4.1).
//!
//! Headless — TextSync never locks `SharedFontSystem` (the 0.19 lazy
//! setters); nothing in this file shapes, measures, or rasterizes (T3/T4).

use bevy::prelude::*;
use buiy_core::layout::{
    BuiyLayoutStep, Direction, LayoutPlugin, LayoutTree, ScrollOffset, Style, WritingMode,
};
use buiy_core::text::{
    BuiyTextPlugin, DecorationLines, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight,
    FontsGeneration, GEIST_MONO_FAMILY, GEIST_SANS_FAMILY, LetterSpacing, LineHeight,
    SharedFontSystem, Text, TextAlign, TextBuffer, TextDecorations, TextSyncAppliedCount, TextWrap,
    WhiteSpace,
};
use buiy_core::{BuiySet, CorePlugin, Node};
use cosmic_text::{Metrics, Wrap};

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

fn spawn_text(app: &mut App, content: &str) -> Entity {
    app.world_mut()
        .spawn((Node, Style::default(), Text(String::from(content))))
        .id()
}

/// Run the spawn frame plus the one-shot `Added<TextBuffer>` re-apply frame
/// (the documented deferred-insert echo), landing in steady state.
fn settle(app: &mut App) {
    app.update();
    app.update();
}

fn applied(app: &App) -> usize {
    app.world().resource::<TextSyncAppliedCount>().0
}

fn buffer_lines(app: &App, entity: Entity) -> Vec<String> {
    app.world()
        .get::<TextBuffer>(entity)
        .expect("text entity has a TextBuffer")
        .buffer
        .lines
        .iter()
        .map(|line| line.text().to_owned())
        .collect()
}

#[test]
fn spawning_text_creates_a_buffer_with_collapsed_content() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "hello\n  world");
    app.update();

    assert_eq!(
        buffer_lines(&app, entity),
        vec!["hello world"],
        "the § 5.2 collapse pre-pass runs before set_text (white-space: normal initial)"
    );
    let buffer = app.world().get::<TextBuffer>(entity).unwrap();
    assert!(
        buffer.intrinsics().is_some(),
        "TextSync leaves intrinsics invalidated; the same frame's measure \
         closure computes and caches them (T3 Task 4)"
    );
    assert_eq!(
        buffer.buffer.wrap(),
        Wrap::Word,
        "§ 5.2 `normal` row pins Wrap::Word, not Buffer::new_empty's WordOrGlyph default"
    );
}

#[test]
fn unset_style_components_fall_back_to_plugin_defaults() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "default style");
    app.update();
    let metrics = app
        .world()
        .get::<TextBuffer>(entity)
        .unwrap()
        .buffer
        .metrics();
    assert_eq!(
        metrics,
        Metrics::relative(16.0, 1.2),
        "TextStyleDefaults.size with the line-height:normal 1.2 stand-in (the carrier is T3's)"
    );
}

#[test]
fn steady_state_applies_zero() {
    let mut app = text_app();
    spawn_text(&mut app, "static");
    settle(&mut app);
    app.update();
    assert_eq!(
        applied(&app),
        0,
        "no-change frame: TextSync must touch nothing"
    );
}

#[test]
fn text_change_resyncs_only_the_changed_entity() {
    let mut app = text_app();
    let changed = spawn_text(&mut app, "before");
    let _static_peer = spawn_text(&mut app, "peer");
    settle(&mut app);

    app.world_mut().get_mut::<Text>(changed).unwrap().0 = String::from("after  edit");
    app.update();

    assert_eq!(applied(&app), 1, "exactly the Changed<Text> entity");
    assert_eq!(buffer_lines(&app, changed), vec!["after edit"]);
}

#[test]
fn font_size_change_resyncs_and_updates_metrics() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "resize me");
    settle(&mut app);

    app.world_mut().entity_mut(entity).insert(FontSize(24.0));
    app.update();

    assert_eq!(applied(&app), 1, "Changed<FontSize> fires the union");
    let metrics = app
        .world()
        .get::<TextBuffer>(entity)
        .unwrap()
        .buffer
        .metrics();
    assert_eq!(metrics, Metrics::relative(24.0, 1.2));
}

#[test]
fn font_weight_and_family_changes_resync() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "restyle me");
    settle(&mut app);

    app.world_mut().entity_mut(entity).insert(FontWeight(700));
    app.update();
    assert_eq!(applied(&app), 1, "Changed<FontWeight> fires the union");

    app.world_mut()
        .entity_mut(entity)
        .insert(FontFamily(FontStack(vec![FamilyEntry::Named(
            String::from("Fira Sans"),
        )])));
    app.update();
    assert_eq!(applied(&app), 1, "Changed<FontFamily> fires the union");
}

/// The letter-spacing the entity's buffer carries on its first line's DEFAULT
/// attrs — the surface `text_sync_buffers` lowers `LetterSpacing` into
/// (`Attrs.letter_spacing` → the per-glyph advance at shaping). This is the
/// cosmic-text value, which is **em** (`px / font_size`), not the authored px
/// — see [`letter_spacing_syncs_as_px_not_em`]. `None` means `normal` (the
/// unset/zero case never builds the opt).
fn buffer_letter_spacing(app: &App, entity: Entity) -> Option<f32> {
    app.world()
        .get::<TextBuffer>(entity)
        .expect("text entity has a TextBuffer")
        .buffer
        .lines
        .first()
        .expect("non-empty buffer has a first line")
        .attrs_list()
        .defaults()
        .letter_spacing_opt
        .map(|spacing| spacing.0)
}

/// parity-prototype A1: `LetterSpacing` lowers onto the buffer's `Attrs`
/// (the shaping path) — a `-0.75 px` tracking authored as the design's
/// `-.025em @ 30px` headline reaches cosmic-text's advance accumulator. The
/// authored value is logical **px**, but cosmic-text 0.19's `letter_spacing`
/// is **em** (`shape.rs` adds it to the em-unit advance, then multiplies by
/// font-size at width time), so the lowering divides by the font-size:
/// `-0.75 px @ 30 px → -0.025 em` (exactly the design's `-.025em`).
#[test]
fn letter_spacing_syncs_to_the_shaping_attrs() {
    let mut app = text_app();
    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("todos")),
            FontSize(30.0),
            LetterSpacing(-0.75),
        ))
        .id();
    settle(&mut app);

    // px → em: -0.75 / 30 = -0.025 em (the design's authored `-.025em`).
    let em = buffer_letter_spacing(&app, entity).expect("tracking lowered");
    assert!(
        (em - (-0.025)).abs() < 1e-6,
        "LetterSpacing(-0.75 px) @ 30 px lowers to -0.025 em on the shaped \
         buffer (px / font_size), got {em}"
    );
}

/// Unset / `normal` carries NO letter_spacing opt — a default run shapes
/// exactly as before this knob existed (no spurious Attrs inequality, no
/// reshape churn).
#[test]
fn unset_letter_spacing_leaves_no_attrs_override() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "normal tracking");
    settle(&mut app);

    assert_eq!(
        buffer_letter_spacing(&app, entity),
        None,
        "no LetterSpacing component → Attrs.letter_spacing stays unset (CSS normal)"
    );
}

/// A `LetterSpacing` change is a member of the trigger union (it rides
/// `Attrs`, so the run must reshape) and re-lowers the new value.
#[test]
fn letter_spacing_change_resyncs_and_relowers() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "retrack me");
    settle(&mut app);

    app.world_mut()
        .entity_mut(entity)
        .insert(LetterSpacing(1.4));
    app.update();
    assert_eq!(applied(&app), 1, "Changed<LetterSpacing> fires the union");
    // 1.4 px @ the default 16 px → 0.0875 em (px / font_size).
    let em = buffer_letter_spacing(&app, entity).expect("tracking lowered");
    assert!(
        (em - (1.4 / 16.0)).abs() < 1e-6,
        "the new tracking re-lowers onto the buffer as em (px / font_size), got {em}"
    );
}

/// parity-prototype A1: the embedded Geist (UI sans) + Geist Mono families
/// resolve in the shared `FontSystem` — both by NAME (the gallery authors
/// `FontFamily(["Geist" / "Geist Mono"])`) and Geist Mono via the monospace
/// generic pin (`GenericFamily::Monospace`, now a TRUE monospace).
#[test]
fn geist_and_geist_mono_resolve_in_the_font_system() {
    use cosmic_text::fontdb;
    let app = text_app();
    let fonts = app.world().resource::<SharedFontSystem>();
    let guard = fonts.lock();
    let db = guard.db();

    let geist = db.query(&fontdb::Query {
        families: &[fontdb::Family::Name(GEIST_SANS_FAMILY)],
        weight: fontdb::Weight(450),
        ..fontdb::Query::default()
    });
    assert!(
        geist.is_some(),
        "Geist resolves by name at the design's 450 weight (variable wght axis)"
    );

    let geist_mono_named = db.query(&fontdb::Query {
        families: &[fontdb::Family::Name(GEIST_MONO_FAMILY)],
        ..fontdb::Query::default()
    });
    assert!(
        geist_mono_named.is_some(),
        "Geist Mono resolves by name (Family::Name)"
    );

    let geist_mono_generic = db.query(&fontdb::Query {
        families: &[fontdb::Family::Monospace],
        ..fontdb::Query::default()
    });
    assert_eq!(
        geist_mono_generic, geist_mono_named,
        "the monospace generic pins to Geist Mono — GenericFamily::Monospace is first-class"
    );
}

#[test]
fn writing_mode_resolved_change_resyncs() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "direction-sensitive");
    settle(&mut app);

    app.world_mut().entity_mut(entity).insert(WritingMode {
        direction: Direction::Rtl,
        ..Default::default()
    });
    app.update();

    assert_eq!(
        applied(&app),
        1,
        "WritingModeInherit rewrites the resolved cache (guarded) before TextSync, \
         and the § 5.1 union consumes Changed<WritingModeResolved> the same frame"
    );
}

#[test]
fn t3_carrier_changes_fire_the_union() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "carrier triggers");
    settle(&mut app);

    app.world_mut()
        .entity_mut(entity)
        .insert(LineHeight::Px(30.0));
    app.update();
    assert_eq!(applied(&app), 1, "Changed<LineHeight> fires the union");
    let metrics = app
        .world()
        .get::<TextBuffer>(entity)
        .unwrap()
        .buffer
        .metrics();
    assert_eq!(
        metrics,
        Metrics::new(16.0, 30.0),
        "line-height → Metrics (§ 5.1)"
    );

    app.world_mut()
        .entity_mut(entity)
        .insert(WhiteSpace::Nowrap);
    app.update();
    assert_eq!(applied(&app), 1, "Changed<WhiteSpace> fires the union");
    assert_eq!(
        app.world().get::<TextBuffer>(entity).unwrap().buffer.wrap(),
        Wrap::None,
        "§ 5.2 nowrap row"
    );

    app.world_mut().entity_mut(entity).insert(TextWrap::Balance);
    app.update();
    assert_eq!(applied(&app), 1, "Changed<TextWrap> fires the union");
    assert_eq!(
        app.world().get::<TextBuffer>(entity).unwrap().buffer.wrap(),
        Wrap::None,
        "balance degrades to the table value; nowrap's None wins here"
    );

    app.world_mut().entity_mut(entity).insert(TextAlign::Center);
    app.update();
    assert_eq!(
        applied(&app),
        1,
        "Changed<TextAlign> fires the union (§ 5.1 carrier pin) — \
         the VALUE is applied at TextCommit, not here"
    );
}

#[test]
fn text_decorations_change_triggers_exactly_one_resync() {
    // § 5.1 union growth (T6): the line bits live in Attrs and
    // has_decoration() gates span creation upstream, so a TextDecorations
    // edit must resync like any other text-style change.
    let mut app = text_app();
    let entity = spawn_text(&mut app, "decorate me");
    settle(&mut app);
    app.update();
    assert_eq!(applied(&app), 0, "steady before the decoration edit");

    app.world_mut().entity_mut(entity).insert(TextDecorations {
        line: DecorationLines::UNDERLINE,
        ..Default::default()
    });
    app.update();
    assert_eq!(
        applied(&app),
        1,
        "Changed<TextDecorations> (== Added) fires the union once"
    );

    // Mutating the existing component re-fires.
    app.world_mut()
        .get_mut::<TextDecorations>(entity)
        .unwrap()
        .line
        .insert(DecorationLines::LINE_THROUGH);
    app.update();
    assert_eq!(
        applied(&app),
        1,
        "in-place line-bit edit re-fires the union"
    );
    app.update();
    assert_eq!(applied(&app), 0, "back to steady");
}

/// § 5.2 preserve rows: `pre` keeps runs of spaces + hard breaks and
/// maps to Wrap::None.
#[test]
fn white_space_pre_preserves_content_verbatim() {
    let mut app = text_app();
    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("a  b\tc\nsecond  line")),
            WhiteSpace::Pre,
        ))
        .id();
    app.update();
    assert_eq!(
        buffer_lines(&app, entity),
        vec!["a  b\tc", "second  line"],
        "preserve mode: nothing collapses; segment breaks become buffer lines"
    );
    assert_eq!(
        app.world().get::<TextBuffer>(entity).unwrap().buffer.wrap(),
        Wrap::None
    );
}

/// Authored zero metrics degrade instead of hitting cosmic's
/// `set_metrics` assert (the METRICS_FLOOR clamp).
#[test]
fn zero_font_size_and_line_height_do_not_panic() {
    let mut app = text_app();
    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("degenerate")),
            FontSize(0.0),
            LineHeight::Px(0.0),
        ))
        .id();
    app.update(); // would panic inside set_metrics without the floor
    let metrics = app
        .world()
        .get::<TextBuffer>(entity)
        .unwrap()
        .buffer
        .metrics();
    assert!(metrics.font_size > 0.0 && metrics.line_height > 0.0);
}

#[test]
fn fonts_generation_bump_sweeps_every_buffer() {
    let mut app = text_app();
    spawn_text(&mut app, "one");
    spawn_text(&mut app, "two");
    spawn_text(&mut app, "three");
    settle(&mut app);

    app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
    app.update();
    assert_eq!(
        applied(&app),
        3,
        "a font-set change reshapes EVERY TextBuffer once — late fonts never \
         leave stale tofu (architecture § 2.2)"
    );

    app.update();
    assert_eq!(
        applied(&app),
        0,
        "the sweep is edge-triggered, never latched"
    );
}

#[derive(Resource, Default)]
struct DirtyProbe(Option<bool>);

/// Reads the text entity's Taffy dirtiness BETWEEN TextSync and SyncStyles
/// (the only window where the mark is observable — TaffyCompute clears it
/// by computing later the same frame).
fn probe_text_node_dirtiness(
    tree: Option<NonSend<LayoutTree>>,
    texts: Query<Entity, With<TextBuffer>>,
    mut out: ResMut<DirtyProbe>,
) {
    out.0 = None;
    let Some(tree) = tree else { return };
    let Ok(entity) = texts.single() else { return };
    let Some(&node) = tree.by_entity().get(&entity) else {
        return;
    };
    out.0 = Some(tree.tree_ref().dirty(node).expect("node is live"));
}

#[test]
fn text_change_marks_the_taffy_node_dirty_before_styles_sync() {
    let mut app = text_app();
    app.init_resource::<DirtyProbe>();
    app.add_systems(
        Update,
        probe_text_node_dirtiness
            .after(BuiyLayoutStep::TextSync)
            .before(BuiyLayoutStep::SyncStyles),
    );
    let entity = spawn_text(&mut app, "measure me");
    settle(&mut app);

    app.update();
    assert_eq!(
        app.world().resource::<DirtyProbe>().0,
        Some(false),
        "steady frame: the node serves Taffy's cache"
    );

    app.world_mut().get_mut::<Text>(entity).unwrap().0 = String::from("longer content now");
    app.update();
    assert_eq!(
        app.world().resource::<DirtyProbe>().0,
        Some(true),
        "content change must invalidate Taffy's leaf cache — the only lever \
         is mark_dirty; set_style is never called for a pure text change \
         (architecture § 4.1)"
    );
}

#[test]
fn removing_text_drops_the_buffer() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "ephemeral");
    settle(&mut app);
    assert!(app.world().get::<TextBuffer>(entity).is_some());

    app.world_mut().entity_mut(entity).remove::<Text>();
    app.update();
    assert!(
        app.world().get::<TextBuffer>(entity).is_none(),
        "a Text-less entity stops being a text leaf; the buffer dies on the edge"
    );
}

/// The TaffyTree<Entity> migration carried the registered context to the
/// measure closure (T3 Task 4): a text leaf measures its content through
/// `compute_roots_with_text_measure`, while non-text nodes carry no
/// context and keep the zero-measure dispatch. (Until Task 4 this was the
/// migration's does-not-change-behavior snapshot, asserting height zero.)
#[test]
fn text_leaf_measures_its_content_through_the_node_context() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "not yet measured");
    let plain = app.world_mut().spawn((Node, Style::default())).id();
    settle(&mut app);

    let layout = app
        .world()
        .get::<buiy_core::ResolvedLayout>(entity)
        .expect("text leaf has a layout");
    assert!(layout.size.y > 0.0, "the measure closure is live (Task 4)");

    let tree = app.world().non_send::<LayoutTree>();
    let node = *tree.by_entity().get(&entity).unwrap();
    assert_eq!(
        tree.tree_ref().get_node_context(node),
        Some(&entity),
        "text leaf registered its Entity as node context (measure § 2.1)"
    );
    let plain_node = *tree.by_entity().get(&plain).unwrap();
    assert_eq!(
        tree.tree_ref().get_node_context(plain_node),
        None,
        "non-text nodes carry no context"
    );
}

/// measure § 2.2 — unregistration on the Text-removal edge.
#[test]
fn removing_text_clears_the_node_context() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "ephemeral context");
    settle(&mut app);

    app.world_mut().entity_mut(entity).remove::<Text>();
    app.update();

    let tree = app.world().non_send::<LayoutTree>();
    let node = *tree.by_entity().get(&entity).unwrap();
    assert_eq!(
        tree.tree_ref().get_node_context(node),
        None,
        "clear_text_context on the RemovedComponents<Text> edge"
    );
}

/// measure § 2.2 — Text ADDED to an entity that already has a Taffy node
/// (the existing-node half of the registration split, decision 1).
#[test]
fn adding_text_to_an_existing_node_registers_the_context() {
    let mut app = text_app();
    let entity = app.world_mut().spawn((Node, Style::default())).id();
    settle(&mut app);

    app.world_mut()
        .entity_mut(entity)
        .insert(Text(String::from("late text")));
    app.update();

    let tree = app.world().non_send::<LayoutTree>();
    let node = *tree.by_entity().get(&entity).unwrap();
    assert_eq!(tree.tree_ref().get_node_context(node), Some(&entity));
}

/// The deliberate § 5.1 exclusion: scroll moves glyph rects via transforms;
/// layout and shaping are unchanged.
#[test]
fn scroll_offset_change_is_excluded() {
    let mut app = text_app();
    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("scrolled")),
            ScrollOffset::default(),
        ))
        .id();
    settle(&mut app);

    app.world_mut().get_mut::<ScrollOffset>(entity).unwrap().y = 42.0;
    app.update();
    assert_eq!(
        applied(&app),
        0,
        "Changed<ScrollOffset> is not a reshape trigger"
    );
}

#[derive(Resource, Default)]
struct BufferTickCount(usize);

fn count_buffer_ticks(mut count: ResMut<BufferTickCount>, changed: Query<(), Changed<TextBuffer>>) {
    count.0 += changed.iter().count();
}

/// measure-and-layout § 7: author intent rides `Changed<Text>` + the style
/// carriers; `Changed<TextBuffer>` is reserved for NOTHING. The only tick
/// ever observed is the insertion tick (the `Added<TextBuffer>` edge the
/// § 5.1 union itself consumes) — every later in-place mutation routes
/// through `Mut::bypass_change_detection`.
#[test]
fn changed_text_buffer_never_fires_after_insertion() {
    let mut app = text_app();
    app.init_resource::<BufferTickCount>();
    app.add_systems(Update, count_buffer_ticks.after(BuiySet::Layout));

    let entity = spawn_text(&mut app, "tick discipline");
    settle(&mut app);
    assert_eq!(
        app.world().resource::<BufferTickCount>().0,
        1,
        "exactly the insertion tick — the frame-2 Added re-apply is bypassed"
    );

    app.world_mut().get_mut::<Text>(entity).unwrap().0 = String::from("rewritten in place");
    app.update();
    app.world_mut().entity_mut(entity).insert(FontSize(32.0));
    app.update();
    app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
    app.update();

    assert_eq!(
        app.world().resource::<BufferTickCount>().0,
        1,
        "content edits, carrier changes, and the generation sweep all mutate \
         the buffer WITHOUT ticking it — the O(0) steady-state contract \
         would die by a thousand downstream filters otherwise (measure § 7)"
    );
}
