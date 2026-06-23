//! `buiy_gallery` — the widget-catalog campaign's runnable exemplar app and the
//! `buiy_verify` screen-fixture source. **C8-a delivers S1 (TodoMVC); C8-b adds
//! S2 (scroll / long-list) + S3 (overlay / menu); C8-c adds S4 (modal + focus-
//! trap) + S5 (F-tier look showcase).**
//!
//! S1 composes the landed P1d widget bundles (single-line [`TextInput`], the
//! tri-state [`Checkbox`], [`Button`]) + the A11yLive `Status` live region into
//! the literal TodoMVC exemplar: type the "What needs to be done?" field + Enter
//! to add a row, toggle a row's checkbox to complete it, destroy a row, clear
//! completed, filter All/Active/Completed, and a "N items left" count that lives
//! in an `A11yRole::Status` aria-live region. Double-clicking a row's label edits
//! it in place (C3b `MultiClick`).
//!
//! **S2 ([`screen_scroll_list`]) — the scale game.** A [`ScrollArea`] (C5-a)
//! holding [`SCROLL_LIST_ROWS`] (~1000) rows — the 1000× TodoMVC scale-game that
//! settles C5's virtualization ceiling (off-screen rows ride the landed
//! `ContentVisibility::Auto` skip). The container owns keyboard scroll
//! (PageDown/End) and the SC-4 `A11yScroll` source the a11y fold projects, so a
//! wheel (via the C7 `PointerHarness`) or a key advances the clamped
//! `ScrollOffset` and the driver snapshot's scroll fields track it.
//!
//! **S3 ([`screen_overlay_menu`]) — overlays.** A [`MenuButton`] opening a
//! [`Menu`] of [`MenuItem`]s (roving / activedescendant), a [`TooltipTrigger`]
//! (show/hide its tip), and a standalone anchored [`Popover`] (light-dismiss). The
//! menu item activation flows through the shared `OnPress` sink into an observable
//! effect ([`MenuActivations`]), so the driver can drive open → arrow-nav →
//! Enter-activate → Esc/outside-close and observe each step through the a11y tree.
//!
//! **S4 ([`spawn_modal`]) — modal + focus-trap.** A trigger Button (the invoker)
//! that `controls` a C5-d `Dialog` (title + body + a focusable `Switch` + a
//! `DialogClose` button) and a focusable background button OUTSIDE the dialog.
//! Activating the invoker opens the dialog (the `A11yModal` is in the snapshot +
//! focus moves inside), Tab traps + wraps inside the modal (never the background),
//! Escape closes + restores focus to the invoker, and the background is pruned from
//! the a11y tree (`A11yHidden`) while open + restored on close. The whole lifecycle
//! is the C5-d `WidgetsPlugin` overlay state machine; S4 is **pure composition**
//! (no app systems). The dialog is spawned imperatively (the invoker references the
//! dialog entity, which a scene cannot name), like S3's standalone popover.
//!
//! **S5 ([`screen_showcase`]) — the F-tier look.** A styled `#ShowcaseCard` with
//! the C6 channels (a multi-term `BoxShadow` elevation, a per-side `Border` band, a
//! rounded radius) holding a [`Switch`], a [`Slider`], and a [`Disclosure`], each
//! focusable so a keyboard focus shows the C6-a focus-ring `Outline`. The driver
//! drives each widget (Switch toggles, Slider increments, Disclosure expands) and
//! the display-list acceptance asserts the card emits shadow + border bands + a
//! keyboard-focused widget emits the focus-ring Outline (`scroll_overlay_c8b.rs`'s
//! successor `modal_showcase_c8c.rs`).
//!
//! **Pure composition (C8 contract).** The crate defines no widget bundle, no
//! a11y-state component, and no primitive. [`screen_todomvc`] authors the static
//! tree; [`TodoMvcPlugin`] registers the retained-mode app systems that run
//! `.after(BuiySet::Input)` (C8 §3.1). The binary boots the screen under
//! `BuiyPlugin + TodoMvcPlugin`; the `buiy_verify` fixture spawns the same
//! [`screen_todomvc`] tree (the "example IS the fixture" discipline), and the
//! inspection-driver acceptance test adds [`TodoMvcPlugin`] to drive behavior.
//!
//! ## App-logic shape (the retained-mode pattern, C8 §3.4 KEEP)
//!
//! Activation/submit are `Message`s emitted in `BuiySet::Input`. The intent
//! systems (`collect_*`) are ordinary `MessageReader` systems that run
//! `.after(BuiySet::Input)` and stage their reads into [`TodoIntents`]; the
//! `apply_intents` exclusive system then performs the structural mutations
//! (append/despawn rows, clear/seed editors) over `&mut World` and clears the
//! staging. Splitting "read messages once" (each `Message` has exactly one
//! reader, so no double-buffer re-read) from "mutate the tree" keeps the
//! exclusive system free of `MessageReader` cursor pitfalls. The pure
//! change-detection systems (`apply_filter`, `update_count`, `restyle_completed`)
//! run last and need no exclusive access.
//!
//! Design: `docs/specs/2026-06-23-c8a-todomvc-s1-design.md`.

use bevy::prelude::{
    App, Camera2d, ChildOf, Children, Commands, Component, Entity, Has, IntoScheduleConfigs,
    MessageReader, Name, On, Plugin, Query, Res, ResMut, Resource, Update, With, World, children,
};
use buiy::prelude::*;
use buiy_core::BuiySet;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::a11y::{A11yHidden, A11yLabel, A11yRole, A11yToggled, Toggled, set_value};
use buiy_core::focus::FocusedEntity;
use buiy_core::interaction::OnPress;
use buiy_core::render::components::{CssVisibility, TextColor};
use buiy_core::text::Text;
use buiy_core::text::edit::{EditSubmitted, TextEditState};
use buiy_widgets::checkbox::CheckboxMark;

// ===========================================================================
// App-state markers + resources (C8: composition-level, NOT a widget primitive)
// ===========================================================================

// The markers authored as bare `bsn!` idents (or via `template_value`) derive
// `Clone + Default` — the trait surface `bsn!` requires of an authorable
// component (mirrors `buiy_widgets`' `CheckboxMark`).

/// Tag on the `#TodoList` container — where rows are appended and walked.
#[derive(Component, Clone, Default)]
pub struct TodoList;

/// Tag on the add-field `TextInput` ("What needs to be done?").
#[derive(Component, Clone, Default)]
pub struct AddField;

/// One todo row (the checkbox/destroy/label children are found by a `Children`
/// walk + marker query — C8 §3.4 drops the denormalized child cache).
#[derive(Component, Clone, Default)]
pub struct TodoRow;

/// Marks the checkbox inside a row.
#[derive(Component, Clone, Default)]
pub struct RowCheckbox;

/// Marks the destroy button inside a row.
#[derive(Component, Clone, Default)]
pub struct RowDestroy;

/// Marks the visible label child inside a row (the double-click edit target).
#[derive(Component, Clone, Default)]
pub struct RowLabel;

/// The "N items left" Status live-region root (its `A11yLabel` is the announced
/// utterance; the child `Text` carries the visible pixels).
#[derive(Component, Clone, Default)]
pub struct ItemsLeft;

/// The visible `Text` inside the [`ItemsLeft`] region.
#[derive(Component, Clone, Default)]
pub struct ItemsLeftText;

/// One filter button (All / Active / Completed).
#[derive(Component, Clone, Copy, Default)]
pub struct FilterButton(pub FilterMode);

/// The clear-completed button.
#[derive(Component, Clone, Default)]
pub struct ClearCompleted;

/// Marks an in-place editor; carries the row whose label it edits.
#[derive(Component, Clone, Copy)]
pub struct EditingInPlace {
    /// The row whose label is being edited.
    pub row: Entity,
}

/// Which rows the filter shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilterMode {
    #[default]
    All,
    Active,
    Completed,
}

/// The active filter (the single source of truth `apply_filter` reads).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filter(pub FilterMode);

impl Default for Filter {
    fn default() -> Self {
        Self(FilterMode::All)
    }
}

/// Staged interaction intents, read once per frame by the `collect_*` systems
/// and consumed by [`apply_intents`]. Empty between frames.
#[derive(Resource, Default)]
pub struct TodoIntents {
    /// A submitted add-field value (the trimmed text), if Enter fired this frame.
    pub add: Option<String>,
    /// Rows whose destroy button fired.
    pub destroy_rows: Vec<Entity>,
    /// Whether clear-completed fired.
    pub clear_completed: bool,
    /// Labels requesting edit-in-place (double-clicked this frame).
    pub begin_edit: Vec<Entity>,
    /// In-place editors that were submitted (commit the edit).
    pub commit_edit: Vec<Entity>,
}

/// The demo rows the binary + fixture seed: `(label, completed)`.
pub const DEMO_SEEDS: &[(&str, bool)] = &[
    ("Taste BSN authoring", true),
    ("Compose the P1d widgets", false),
    ("Inspect through the a11y driver", false),
];

// ===========================================================================
// The screen scene (the static tree — the "example IS the fixture")
// ===========================================================================

/// The S1 TodoMVC screen as a composable [`Scene`]: a `#TodoCard` flex-column
/// card holding the add-field, the (initially empty) todo list, and the footer
/// (the "N items left" Status region + filter group + clear-completed).
///
/// `seeds` is reserved for parameterizing the seed set; rows are spawned
/// imperatively in [`setup`] (they are dynamic — appended/removed at runtime —
/// so they are not part of the static scene). Every entity is `#Name`-tagged so
/// layout/a11y dumps are content-keyed.
pub fn screen_todomvc(seeds: &[(&str, bool)]) -> impl Scene {
    let _ = seeds;
    bsn! {
        #TodoCard
        Node
        template_value(Display::flex_column())
        FlexParams {
            direction: FlexAxis::Column,
            gap: { FlexGap { row: Length::px(12.0), column: Length::px(12.0) } },
        }
        BoxModel {
            width: { Sizing::Length(Length::Px(408.0)) },
            padding: { Edges::all(24.0) },
        }
        Background { color: { ColorToken::Token("color.surface.primary".into()) } }
        Border { radius: { Corners::all(Radius::circular(6.0)) } }
        Children [
            // The `#Name` tags content-key the dumps; the bare marker idents
            // (`AddField`/`TodoList`/…) are the app-logic handles the systems
            // query (the `#Name` syntax adds a `Name`, NOT a same-named
            // component, so the markers are spelled explicitly).
            (#AddField AddField text_input_single_line("What needs to be done?")),
            (
                #TodoList
                TodoList
                Node
                template_value(Display::flex_column())
                FlexParams {
                    direction: FlexAxis::Column,
                    gap: { FlexGap { row: Length::px(6.0), column: Length::px(6.0) } },
                }
            ),
            (
                #Footer
                Node
                template_value(Display::flex_row())
                FlexParams {
                    direction: FlexAxis::Row,
                    gap: { FlexGap { row: Length::px(8.0), column: Length::px(8.0) } },
                }
                Children [
                    (
                        #ItemsLeft
                        ItemsLeft
                        Node
                        template_value(A11yRole::Status)
                        A11yLabel({ items_left_text(0) })
                        Children [
                            (#ItemsLeftCount ItemsLeftText Text({ items_left_text(0) }) FontSize({ 16.0 })),
                        ]
                    ),
                    (#FilterAll button("All") template_value(FilterButton(FilterMode::All))),
                    (#FilterActive button("Active") template_value(FilterButton(FilterMode::Active))),
                    (#FilterCompleted button("Completed") template_value(FilterButton(FilterMode::Completed))),
                    (#ClearButton ClearCompleted button("Clear completed")),
                ]
            ),
        ]
    }
}

/// The "N items left" announcement string (singular/plural).
pub fn items_left_text(n: usize) -> String {
    if n == 1 {
        "1 item left".to_string()
    } else {
        format!("{n} items left")
    }
}

/// One todo-row scene: a flex-row of `[Checkbox(label), destroy Button("×")]`.
fn todo_row(label: &str) -> impl Scene {
    let label = label.to_string();
    bsn! {
        #TodoRow
        TodoRow
        Node
        template_value(Display::flex_row())
        FlexParams {
            direction: FlexAxis::Row,
            gap: { FlexGap { row: Length::px(8.0), column: Length::px(8.0) } },
        }
        Children [
            (#RowCheckbox RowCheckbox checkbox(label)),
            (#RowDestroy RowDestroy button("×")),
        ]
    }
}

// ===========================================================================
// Setup (binary + fixture both call this to seed the demo rows)
// ===========================================================================

/// Startup system: spawn a camera, the screen, then the [`DEMO_SEEDS`] rows.
pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn_scene(screen_todomvc(DEMO_SEEDS));
    commands.queue(|world: &mut World| {
        for &(label, completed) in DEMO_SEEDS {
            append_row(world, label, completed);
        }
    });
}

// ===========================================================================
// The TodoMvcPlugin — the retained-mode app logic (C8 §3.1)
// ===========================================================================

/// The TodoMVC app logic. Registers the intent collectors + the exclusive
/// applier + the pure change-detection systems, all `.after(BuiySet::Input)`
/// (C8 §2.5(1)), plus the [`MultiClick`] double-click observer.
pub struct TodoMvcPlugin;

impl Plugin for TodoMvcPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Filter>()
            .init_resource::<TodoIntents>()
            .add_observer(on_label_double_click)
            .add_systems(
                Update,
                (
                    collect_add_submit,
                    collect_button_press,
                    collect_edit_submit,
                    apply_intents,
                    apply_filter,
                    update_count,
                    restyle_completed,
                )
                    .chain()
                    // `.after(Input)`: the activation/submit Messages are emitted
                    // (and `advance_toggle_on_press` flips `A11yToggled`) in
                    // `BuiySet::Input`, so the logic reads them the same frame
                    // (C8 §2.5(1)). `.before(A11yUpdate)`: the count/filter
                    // mutations to `A11yLabel`/`A11yHidden` must land BEFORE
                    // `build_tree` (which runs in `A11yUpdate`) so the driver's
                    // very-next snapshot reflects them — without this the tree is
                    // one frame stale.
                    .after(BuiySet::Input)
                    .before(BuiySet::A11yUpdate),
            );
    }
}

/// Append a new row to `#TodoList`, seeding its checkbox state. Returns the row.
/// Shared by [`setup`] and the runtime add-flow.
pub fn append_row(world: &mut World, label: &str, completed: bool) -> Entity {
    let Some(list) = find_single::<TodoList>(world) else {
        return Entity::PLACEHOLDER;
    };
    let row = world
        .spawn_scene(todo_row(label))
        .expect("spawn todo row scene")
        .id();
    world.entity_mut(list).add_child(row);

    if let Some(checkbox) = child_with::<RowCheckbox>(world, row) {
        if completed && let Some(mut t) = world.get_mut::<A11yToggled>(checkbox) {
            t.0 = Toggled::True;
        }
        if let Some(label_child) = checkbox_label_child(world, checkbox) {
            world.entity_mut(label_child).insert(RowLabel);
        }
    }
    row
}

// --- Intent collectors (ordinary MessageReader systems) --------------------

/// Stage the add-field submission (single-line Enter → `EditSubmitted`).
pub fn collect_add_submit(
    mut reader: MessageReader<EditSubmitted>,
    fields: Query<&TextEditState, With<AddField>>,
    mut intents: ResMut<TodoIntents>,
) {
    for EditSubmitted(e) in reader.read() {
        if let Ok(state) = fields.get(*e) {
            let text = state.value().trim().to_string();
            if !text.is_empty() {
                intents.add = Some(text);
            }
        }
    }
}

/// Stage destroy / clear-completed / filter presses. `kinds` reads the pressed
/// entity's button role (destroy/clear/filter) in one query; `hierarchy` walks
/// the row ancestry for a destroy.
pub fn collect_button_press(
    mut reader: MessageReader<OnPress>,
    kinds: Query<(Has<RowDestroy>, Has<ClearCompleted>, Option<&FilterButton>)>,
    hierarchy: Query<(Has<TodoRow>, Option<&ChildOf>)>,
    mut intents: ResMut<TodoIntents>,
    mut filter: ResMut<Filter>,
) {
    for OnPress(e) in reader.read() {
        let Ok((is_destroy, is_clear, fb)) = kinds.get(*e) else {
            continue;
        };
        if is_destroy {
            if let Some(row) = ancestor_row(*e, &hierarchy) {
                intents.destroy_rows.push(row);
            }
        } else if is_clear {
            intents.clear_completed = true;
        } else if let Some(fb) = fb {
            filter.0 = fb.0;
        }
    }
}

/// Stage in-place editor submissions (commit the edit).
pub fn collect_edit_submit(
    mut reader: MessageReader<EditSubmitted>,
    editors: Query<(), With<EditingInPlace>>,
    mut intents: ResMut<TodoIntents>,
) {
    for EditSubmitted(e) in reader.read() {
        if editors.get(*e).is_ok() {
            intents.commit_edit.push(*e);
        }
    }
}

/// Observer: a double-click (`MultiClick.count >= 2`) on a row label stages an
/// in-place edit. `MultiClick` auto-propagates, so a click on the label's own
/// pixels (or a descendant) bubbles to the `RowLabel` entity.
pub fn on_label_double_click(
    ev: On<MultiClick>,
    labels: Query<(), With<RowLabel>>,
    mut intents: ResMut<TodoIntents>,
) {
    if ev.count >= 2 && labels.get(ev.entity).is_ok() {
        intents.begin_edit.push(ev.entity);
    }
}

// --- The exclusive applier (structural mutations over &mut World) ----------

/// Consume the staged [`TodoIntents`]: append the new todo, despawn destroyed /
/// completed rows (restoring focus), begin/commit in-place edits. Clears the
/// staging at the end so each intent fires exactly once.
pub fn apply_intents(world: &mut World) {
    let intents = std::mem::take(&mut *world.resource_mut::<TodoIntents>());

    // 1. Add a todo + clear the add-field through the driver's text channel.
    if let Some(text) = intents.add {
        append_row(world, &text, false);
        if let Some(field) = find_single::<AddField>(world) {
            let _ = set_value(world, node_id_for(field), "");
        }
    }

    // 2. Destroy rows (explicit ×) + clear-completed.
    let mut to_despawn = intents.destroy_rows;
    if intents.clear_completed {
        for row in completed_rows(world) {
            if !to_despawn.contains(&row) {
                to_despawn.push(row);
            }
        }
    }
    for row in to_despawn {
        despawn_row_restoring_focus(world, row);
    }

    // 3. Begin in-place edits: hide the static label, spawn + seed + focus an
    //    editor sibling tagged with the row.
    for label in intents.begin_edit {
        begin_one_edit(world, label);
    }

    // 4. Commit in-place edits: write the new text back to the label, restore
    //    its visibility, despawn the editor.
    for editor in intents.commit_edit {
        commit_one_edit(world, editor);
    }
}

fn begin_one_edit(world: &mut World, label: Entity) {
    let Some(text) = world.get::<Text>(label).map(|t| t.0.clone()) else {
        return;
    };
    let Some(row) = ancestor_row_world(world, label) else {
        return;
    };
    world.entity_mut(label).insert(CssVisibility::Hidden);
    let editor = world
        .spawn_scene(text_input_single_line(""))
        .expect("spawn edit-in-place editor")
        .id();
    world.entity_mut(editor).insert(EditingInPlace { row });
    world.entity_mut(row).add_child(editor);
    let _ = set_value(world, node_id_for(editor), &text);
    world.resource_mut::<FocusedEntity>().0 = Some(editor);
}

fn commit_one_edit(world: &mut World, editor: Entity) {
    let Some(EditingInPlace { row }) = world.get::<EditingInPlace>(editor).copied() else {
        return;
    };
    let new_text = world
        .get::<TextEditState>(editor)
        .map(|s| s.value())
        .unwrap_or_default();
    if let Some(checkbox) = child_with::<RowCheckbox>(world, row)
        && let Some(label) = checkbox_label_child(world, checkbox)
    {
        if let Some(mut t) = world.get_mut::<Text>(label) {
            t.0 = new_text.clone();
        }
        if let Some(mut name) = world.get_mut::<A11yLabel>(checkbox) {
            name.0 = new_text.clone();
        }
        world.entity_mut(label).insert(CssVisibility::Visible);
    }
    if world.resource::<FocusedEntity>().0 == Some(editor) {
        world.resource_mut::<FocusedEntity>().0 = None;
    }
    world.entity_mut(editor).despawn();
}

// --- Pure change-detection systems -----------------------------------------

/// Show only rows matching the active filter: add/remove `A11yHidden` (a11y
/// prune, C8 §3.4) + `CssVisibility` (visual collapse). One cheap walk/frame.
pub fn apply_filter(
    filter: Res<Filter>,
    mut commands: Commands,
    rows: Query<(Entity, &Children), With<TodoRow>>,
    checkboxes: Query<&A11yToggled, With<RowCheckbox>>,
) {
    for (row, children) in &rows {
        let completed = row_completed(children, &checkboxes);
        let show = match filter.0 {
            FilterMode::All => true,
            FilterMode::Active => !completed,
            FilterMode::Completed => completed,
        };
        if show {
            commands
                .entity(row)
                .remove::<A11yHidden>()
                .insert(CssVisibility::Visible);
        } else {
            commands
                .entity(row)
                .insert(A11yHidden)
                .insert(CssVisibility::Hidden);
        }
    }
}

/// Recount incomplete rows → the `ItemsLeft` Status region's `A11yLabel` (the
/// announced utterance) + its visible `Text` child.
pub fn update_count(
    rows: Query<&Children, With<TodoRow>>,
    checkboxes: Query<&A11yToggled, With<RowCheckbox>>,
    mut region: Query<(&Children, &mut A11yLabel), With<ItemsLeft>>,
    mut texts: Query<&mut Text, With<ItemsLeftText>>,
) {
    let remaining = rows
        .iter()
        .filter(|children| !row_completed(children, &checkboxes))
        .count();
    let phrase = items_left_text(remaining);
    for (children, mut label) in &mut region {
        if label.0 != phrase {
            label.0 = phrase.clone();
        }
        for &child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child)
                && text.0 != phrase
            {
                text.0 = phrase.clone();
            }
        }
    }
}

/// Dim a completed row's label (the completed visual). One walk/frame.
pub fn restyle_completed(
    rows: Query<&Children, With<TodoRow>>,
    checkboxes: Query<(&A11yToggled, &Children), With<RowCheckbox>>,
    mut labels: Query<&mut TextColor, With<RowLabel>>,
) {
    for row_children in &rows {
        for &child in row_children.iter() {
            let Ok((toggled, cb_children)) = checkboxes.get(child) else {
                continue;
            };
            let completed = toggled.0 == Toggled::True;
            for &cb_child in cb_children.iter() {
                if let Ok(mut color) = labels.get_mut(cb_child) {
                    color.0 = if completed {
                        ColorToken::Token("color.text.disabled".into())
                    } else {
                        ColorToken::Token("color.text.primary".into())
                    };
                }
            }
        }
    }
}

// ===========================================================================
// Helpers (Children walks + marker queries — C8 §3.4 drops the denorm cache)
// ===========================================================================

/// The single entity carrying `T`, or `None` (0 or >1).
fn find_single<T: Component>(world: &mut World) -> Option<Entity> {
    let mut q = world.query_filtered::<Entity, With<T>>();
    let mut it = q.iter(world);
    let first = it.next()?;
    if it.next().is_some() {
        return None;
    }
    Some(first)
}

/// The first child of `parent` carrying `T`.
fn child_with<T: Component>(world: &World, parent: Entity) -> Option<Entity> {
    let children = world.get::<Children>(parent)?;
    children
        .iter()
        .copied()
        .find(|&c| world.get::<T>(c).is_some())
}

/// The checkbox's visible label child (the non-mark `Text` child).
fn checkbox_label_child(world: &World, checkbox: Entity) -> Option<Entity> {
    let children = world.get::<Children>(checkbox)?;
    children
        .iter()
        .copied()
        .find(|&c| world.get::<Text>(c).is_some() && world.get::<CheckboxMark>(c).is_none())
}

/// Walk up from `e` to its owning `TodoRow` (query form, for the collector
/// system). `hierarchy` yields `(is_row, parent)` per entity.
fn ancestor_row(e: Entity, hierarchy: &Query<(Has<TodoRow>, Option<&ChildOf>)>) -> Option<Entity> {
    let mut cur = e;
    for _ in 0..8 {
        let (is_row, parent) = hierarchy.get(cur).ok()?;
        if is_row {
            return Some(cur);
        }
        cur = parent?.parent();
    }
    None
}

/// Walk up from `e` to its owning `TodoRow` ancestor (`&World` form).
fn ancestor_row_world(world: &World, e: Entity) -> Option<Entity> {
    let mut cur = e;
    for _ in 0..8 {
        if world.get::<TodoRow>(cur).is_some() {
            return Some(cur);
        }
        cur = world.get::<ChildOf>(cur)?.parent();
    }
    None
}

/// Whether a row (given its `Children`) is completed — its checkbox is `True`.
fn row_completed(children: &Children, checkboxes: &Query<&A11yToggled, With<RowCheckbox>>) -> bool {
    children
        .iter()
        .copied()
        .filter_map(|c| checkboxes.get(c).ok())
        .any(|t| t.0 == Toggled::True)
}

/// Every completed row (a `&mut World` walk for the exclusive clear-completed).
fn completed_rows(world: &mut World) -> Vec<Entity> {
    let rows: Vec<(Entity, Vec<Entity>)> = {
        let mut q = world.query_filtered::<(Entity, &Children), With<TodoRow>>();
        q.iter(world)
            .map(|(row, children)| (row, children.iter().copied().collect()))
            .collect()
    };
    rows.into_iter()
        .filter(|(_, children)| {
            children.iter().any(|&c| {
                world.get::<RowCheckbox>(c).is_some()
                    && world.get::<A11yToggled>(c).map(|t| t.0) == Some(Toggled::True)
            })
        })
        .map(|(row, _)| row)
        .collect()
}

/// Despawn a row + subtree, clearing focus if the subtree held it (C8 §2.5(4)).
fn despawn_row_restoring_focus(world: &mut World, row: Entity) {
    let focused = world.resource::<FocusedEntity>().0;
    let held = focused.is_some_and(|f| is_descendant(world, f, row));
    world.entity_mut(row).despawn();
    if held {
        world.resource_mut::<FocusedEntity>().0 = None;
    }
}

/// Whether `e` is `ancestor` or a descendant of it.
fn is_descendant(world: &World, e: Entity, ancestor: Entity) -> bool {
    let mut cur = e;
    for _ in 0..16 {
        if cur == ancestor {
            return true;
        }
        match world.get::<ChildOf>(cur) {
            Some(c) => cur = c.parent(),
            None => return false,
        }
    }
    false
}

// ###########################################################################
// S2 — scroll / long-list (the scale-game). A `ScrollArea` over ~1000 rows.
// ###########################################################################

/// The long-list row count — the 1000× TodoMVC scale-game (C8 §3.2). One entity
/// per row (the retained-mode model); the off-screen rows ride the landed
/// `ContentVisibility::Auto` skip, so the 1000-row build costs 1000 entities but
/// only the on-screen window costs paint + shaping.
pub const SCROLL_LIST_ROWS: usize = 1000;

/// The S2 viewport height (logical px). Small relative to the content so the
/// list overflows and is scrollable from the first frame.
const SCROLL_VIEWPORT_H: f32 = 300.0;

/// One long-list row's fixed height (logical px). `content = ROWS × ROW_H` is the
/// scroll extent the clamp reads.
const SCROLL_ROW_H: f32 = 28.0;

/// Tag on the S2 `#ScrollList` container (the `ScrollArea`) — where the rows are
/// appended and the scroll/keyboard handlers target.
#[derive(Component, Clone, Default)]
pub struct ScrollList;

/// One long-list row (a labelled `Text` line). A bare app-state marker; the
/// visible pixels are the row's `Text` child.
#[derive(Component, Clone, Default)]
pub struct ScrollRow;

/// The S2 screen as a composable [`Scene`]: a `#ScrollCard` flex-column card
/// holding a heading and the `#ScrollList` [`ScrollArea`] viewport. The rows are
/// appended imperatively (they are dynamic — 1000 of them — so they are seeded by
/// [`fill_scroll_list`], not authored statically), the way S1 seeds its todo rows.
///
/// `n` is the row count the binary/fixture seeds (the default is
/// [`SCROLL_LIST_ROWS`]); it is threaded so a test can shrink it. Every entity is
/// `#Name`-tagged so layout/a11y dumps are content-keyed.
pub fn screen_scroll_list(n: usize) -> impl Scene {
    let _ = n; // rows are seeded imperatively (see `fill_scroll_list`).
    bsn! {
        #ScrollCard
        Node
        template_value(Display::flex_column())
        FlexParams {
            direction: FlexAxis::Column,
            gap: { FlexGap { row: Length::px(12.0), column: Length::px(12.0) } },
        }
        BoxModel {
            width: { Sizing::Length(Length::Px(360.0)) },
            padding: { Edges::all(16.0) },
        }
        Background { color: { ColorToken::Token("color.surface.primary".into()) } }
        Border { radius: { Corners::all(Radius::circular(6.0)) } }
        Children [
            (#ScrollHeading Text({ format!("{SCROLL_LIST_ROWS} items") }) FontSize({ 18.0 })),
            (
                // The scroll viewport: the `ScrollArea` marker triggers the full
                // C5-a `#[require]` contract (a scrollable `Overflow`,
                // `ScrollOffset`, `ScrollExtent`, `Focusable`, `A11yRole::Group`,
                // and the SC-4 `A11yScroll` source). Sized to a short viewport so
                // the 1000-row content overflows + scrolls. The `ScrollList`
                // marker is the app-logic handle the seeding queries.
                #ScrollList
                ScrollList
                scroll_area("Items")
                BoxModel {
                    width: { Sizing::Length(Length::Px(328.0)) },
                    height: { Sizing::Length(Length::Px(SCROLL_VIEWPORT_H)) },
                }
                FlexParams {
                    direction: FlexAxis::Column,
                    gap: { FlexGap { row: Length::px(2.0), column: Length::px(2.0) } },
                }
            ),
        ]
    }
}

/// One long-list row scene: a fixed-height `Text` line. Pinned `min_height` so the
/// flex column cannot shrink the row back (the same "keep it tall so the container
/// overflows" discipline the C5-a scroll tests use).
fn scroll_row(label: &str) -> impl Scene {
    let label = label.to_string();
    bsn! {
        #ScrollRow
        ScrollRow
        Node
        BoxModel {
            width: { Sizing::Length(Length::Px(300.0)) },
            height: { Sizing::Length(Length::Px(SCROLL_ROW_H)) },
            min_height: { Sizing::Length(Length::Px(SCROLL_ROW_H)) },
        }
        Children [
            (#ScrollRowText Text({ label }) FontSize({ 14.0 })),
        ]
    }
}

/// Seed `n` rows into the `#ScrollList` (the binary + fixture both call this; rows
/// are dynamic, like S1's todo rows). Returns the row count actually appended.
pub fn fill_scroll_list(world: &mut World, n: usize) -> usize {
    let Some(list) = find_single::<ScrollList>(world) else {
        return 0;
    };
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        let row = world
            .spawn_scene(scroll_row(&format!("Item {}", i + 1)))
            .expect("spawn scroll row scene")
            .id();
        rows.push(row);
    }
    world.entity_mut(list).add_children(&rows);
    n
}

/// Startup system for the S2 binary: a camera, the screen, then the rows.
pub fn setup_scroll_list(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn_scene(screen_scroll_list(SCROLL_LIST_ROWS));
    commands.queue(|world: &mut World| {
        fill_scroll_list(world, SCROLL_LIST_ROWS);
    });
}

// ###########################################################################
// S3 — overlay / menu. A MenuButton→Menu, a TooltipTrigger, and a Popover.
// ###########################################################################

/// Tag on each S3 menu item — carries the item's index so an activation records a
/// content-keyed effect ([`MenuActivations`]) the driver can observe.
#[derive(Component, Clone, Copy, Default)]
pub struct MenuAction(pub usize);

/// The observable effect of a menu-item activation (the S3 grounding loop): the
/// labels of the items whose shared `OnPress` sink fired, in order. The driver
/// asserts an Enter on the active item appends to this — proving activation
/// reaches an app-level effect, not just the a11y state.
#[derive(Resource, Default)]
pub struct MenuActivations(pub Vec<String>);

/// The labels of the S3 menu's items (Cut / Copy / Paste — the canonical menu).
pub const MENU_ITEM_LABELS: &[&str] = &["Cut", "Copy", "Paste"];

/// The S3 screen as a composable [`Scene`]: a `#OverlayCard` flex-row of the three
/// overlay triggers — a [`MenuButton`] ("Edit") controlling a [`Menu`] of
/// [`MenuItem`]s, a [`TooltipTrigger`] ("?"), and a standalone anchored
/// [`Popover`]. The `Popover`'s anchor (and the menu↔button + tooltip
/// `described_by` edges) are wired by the `WidgetsPlugin` wiring systems once the
/// children exist; the popover here is authored open-on-demand (it starts hidden
/// and is shown by the driver/binary). Every entity is `#Name`-tagged.
///
/// The menu items carry a [`MenuAction`] index so [`OverlayMenuPlugin`] records an
/// observable effect on activation. `popover_anchor` is the entity the standalone
/// popover anchors to (the tooltip trigger, so it has a real on-screen anchor);
/// it is passed in because a `Popover` references an `Entity` the scene cannot
/// name until it is spawned — the binary/fixture spawns the trigger first.
pub fn screen_overlay_menu() -> impl Scene {
    bsn! {
        #OverlayCard
        Node
        template_value(Display::flex_row())
        FlexParams {
            direction: FlexAxis::Row,
            gap: { FlexGap { row: Length::px(16.0), column: Length::px(16.0) } },
        }
        BoxModel {
            width: { Sizing::Length(Length::Px(420.0)) },
            padding: { Edges::all(24.0) },
        }
        Background { color: { ColorToken::Token("color.surface.primary".into()) } }
        Border { radius: { Corners::all(Radius::circular(6.0)) } }
        Children [
            // The MenuButton + its controlled Menu (Cut/Copy/Paste). The
            // `menu_button` scene-fn triggers the full trigger contract
            // (A11yHasPopup(Menu) + A11yExpanded + the Button keymap); the menu +
            // items are authored as the button's children, and `wire_menu_button`
            // wires the controls/anchor edges. Each item carries a `MenuAction`.
            (
                #EditMenuButton
                menu_button("Edit")
                Children [
                    (Text("Edit") FontSize({ 16.0 }) template_value(bevy::picking::Pickable::IGNORE)),
                    (
                        #EditMenu
                        menu()
                        Children [
                            (#MenuCut menu_item("Cut") template_value(MenuAction(0))),
                            (#MenuCopy menu_item("Copy") template_value(MenuAction(1))),
                            (#MenuPaste menu_item("Paste") template_value(MenuAction(2))),
                        ]
                    ),
                ]
            ),
            // The tooltip trigger ("?", tip "More info here"). The `tooltip_trigger`
            // scene-fn triggers the A11yTooltipHost capability + the controlled
            // Tooltip node (starts hidden); the router's ShowTooltip/HideTooltip
            // honor flips its visibility, and `position_tooltip` places it.
            (#InfoTip tooltip_trigger("?", "More info here")),
        ]
    }
}

/// Spawn the S3 scene into `world` + a standalone anchored [`Popover`] anchored to
/// the tooltip trigger (the popover references an `Entity`, so it is spawned after
/// the scene). Returns the standalone popover entity. Shared by the binary
/// ([`setup_overlay_menu`]) and the fixtures so both render the same tree.
pub fn spawn_overlay_menu(world: &mut World) -> Entity {
    world
        .spawn_scene(screen_overlay_menu())
        .expect("spawn the overlay-menu screen");
    // The standalone anchored popover (light-dismiss): a small panel anchored to
    // the tooltip trigger. It starts hidden; the driver/binary opens it. Authored
    // imperatively because `Popover.anchor` names the trigger's entity.
    let anchor = find_single::<buiy_widgets::tooltip::TooltipTrigger>(world);
    world
        .spawn((
            buiy_widgets::Popover {
                anchor,
                ..Default::default()
            },
            Name::new("InfoPopover"),
            buiy_core::render::components::CssVisibility::Hidden,
            buiy_core::layout::Style::default()
                .width_px(160.0)
                .height_px(80.0),
            children![(
                Name::new("InfoPopoverText"),
                Text("Anchored panel".to_string()),
                FontSize(14.0),
                bevy::picking::Pickable::IGNORE,
            )],
        ))
        .id()
}

/// Startup system for the S3 binary: a camera, then the overlay screen + popover.
pub fn setup_overlay_menu(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.queue(|world: &mut World| {
        spawn_overlay_menu(world);
    });
}

/// The S3 app logic: record an observable effect when a menu item is activated.
/// A menu item's activation rides the shared [`OnPress`] sink (written by the
/// menu's Enter/Space keyboard nav, or a future per-item pointer handler); this
/// system reads it `.after(BuiySet::Input)` (the C8 §2.5(1) ordering) and appends
/// the activated item's label to [`MenuActivations`] — the grounding-loop effect
/// the driver asserts.
pub struct OverlayMenuPlugin;

impl Plugin for OverlayMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuActivations>()
            .add_systems(Update, record_menu_activation.after(BuiySet::Input));
    }
}

/// Append each activated menu item's label to [`MenuActivations`]. Reads the
/// shared `OnPress` sink (the same sink the menu keyboard nav writes for the
/// active item) and resolves the pressed entity's [`MenuAction`] → its label.
pub fn record_menu_activation(
    mut reader: MessageReader<OnPress>,
    items: Query<&MenuAction>,
    mut log: ResMut<MenuActivations>,
) {
    for OnPress(e) in reader.read() {
        if let Ok(action) = items.get(*e)
            && let Some(label) = MENU_ITEM_LABELS.get(action.0)
        {
            log.0.push((*label).to_string());
        }
    }
}

// ###########################################################################
// S4 — modal + focus-trap. A trigger Button invokes a C5-d Dialog.
// ###########################################################################

/// The S4 dialog **title** + **body** text (the label/description sources).
pub const MODAL_TITLE: &str = "Delete file?";
/// The S4 dialog body.
pub const MODAL_BODY: &str = "This action cannot be undone.";
/// The S4 background button's accessible name (a focusable OUTSIDE the dialog —
/// the driver proves the modal prune drops it + the trap never reaches it).
pub const MODAL_BG_BUTTON: &str = "Background action";
/// The S4 invoker button's accessible name.
pub const MODAL_INVOKER: &str = "Open dialog";

/// Tag on the S4 invoker button (the trigger the driver activates to open the
/// dialog). A bare app-state marker; the dialog-open wiring is the C5-d
/// `WidgetsPlugin` lifecycle, keyed on the invoker's `A11yRelations.controls`.
#[derive(Component, Clone, Default)]
pub struct ModalInvoker;

/// Tag on the S4 dialog (so the binary/fixture/driver can find it among roots).
#[derive(Component, Clone, Default)]
pub struct ModalDialog;

/// Spawn the S4 modal screen into `world`: a background button (a focusable root
/// OUTSIDE the dialog — proves the inert prune + trap exclusion), a closed C5-d
/// [`Dialog`] holding a title + body + two focusable controls (a [`Switch`] and a
/// [`DialogClose`](buiy_widgets::dialog::DialogClose) "Close" button), and the
/// **invoker** that `controls` it, all
/// under a window-sized root. Returns `(invoker, dialog, background)`.
///
/// The dialog is spawned imperatively (not in a `screen_*` scene-fn) because the
/// invoker references the dialog **entity**, which a scene cannot name until it is
/// spawned — the same constraint S3's standalone popover hits (`spawn_overlay_menu`).
/// `WidgetsPlugin` owns the whole open/close/focus-trap/Esc/restore + inert-
/// background lifecycle (C5-d `dialog.rs`); S4 is **pure composition** — it adds
/// no app systems.
pub fn spawn_modal(world: &mut World) -> (Entity, Entity, Entity) {
    use buiy_widgets::dialog::{Dialog, DialogBody, DialogClose, DialogTitle, dialog_invoker};

    // A background focusable root, OUTSIDE the dialog: proves the modal prunes the
    // rest-of-tree from the a11y tree + the trap never reaches it.
    let bg = world
        .spawn((
            buiy_widgets::Button,
            A11yLabel(MODAL_BG_BUTTON.to_string()),
            Name::new("ModalBackgroundButton"),
        ))
        .id();

    // The closed dialog: a titled + described modal panel holding two focusable
    // controls (a Switch the driver can toggle inside the trap, and a DialogClose
    // button). The Dialog `#[require]` carries the modal a11y + TopLayer::Modal +
    // FocusScope::trap + FocusReturn + CssVisibility::Hidden (closed at rest).
    let dialog = world
        .spawn((
            Dialog,
            ModalDialog,
            Name::new("ModalDialog"),
            children![
                (
                    DialogTitle,
                    Text(MODAL_TITLE.to_string()),
                    FontSize(18.0),
                    A11yLabel(MODAL_TITLE.to_string()),
                    Name::new("ModalTitle"),
                    bevy::picking::Pickable::IGNORE,
                ),
                (
                    DialogBody,
                    Text(MODAL_BODY.to_string()),
                    FontSize(14.0),
                    A11yLabel(MODAL_BODY.to_string()),
                    Name::new("ModalBody"),
                    bevy::picking::Pickable::IGNORE,
                ),
                // A focusable control INSIDE the trap (the driver toggles it to
                // prove a control inside the modal still functions while trapped).
                (Switch::new("Confirm"), Name::new("ModalSwitch")),
                // The close button — its Click → OnPress rides the C5-d
                // `close_dialog_on_button` path (closes + restores focus).
                (
                    buiy_widgets::Button,
                    DialogClose,
                    A11yLabel("Close".to_string()),
                    Name::new("ModalClose"),
                ),
            ],
        ))
        .id();

    // The invoker: a Button whose `A11yRelations.controls = [dialog]`. Its
    // activation rides the C5-d `open_dialog_on_invoker_press` consumer.
    let invoker = world
        .spawn((
            dialog_invoker(MODAL_INVOKER, dialog),
            ModalInvoker,
            Name::new("ModalInvoker"),
        ))
        .id();

    // A window-sized root so layout/picking has a context to walk; the invoker +
    // background are its children (the dialog is its own top-layer root).
    let root = world
        .spawn((
            Node,
            buiy_core::layout::Style::default()
                .width_px(800.0)
                .height_px(600.0),
            Name::new("ModalRoot"),
        ))
        .id();
    world.entity_mut(root).add_children(&[invoker, bg]);
    (invoker, dialog, bg)
}

/// Startup system for the S4 binary: a camera, then the modal screen.
pub fn setup_modal(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.queue(|world: &mut World| {
        spawn_modal(world);
    });
}

// ###########################################################################
// S5 — F-tier look showcase. Switch + Slider + Disclosure on a styled card.
// ###########################################################################

/// The S5 slider's accessible name (the driver addresses it by role+name).
pub const SHOWCASE_SLIDER: &str = "Volume";
/// The S5 switch's accessible name.
pub const SHOWCASE_SWITCH: &str = "Wi-Fi";
/// The S5 disclosure's accessible name.
pub const SHOWCASE_DISCLOSURE: &str = "Advanced";

/// The S5 slider's initial value / range / step (the driver increments it by
/// `step` and observes `now` advance through the a11y tree).
pub const SHOWCASE_SLIDER_NOW: f64 = 50.0;
/// The S5 slider's minimum.
pub const SHOWCASE_SLIDER_MIN: f64 = 0.0;
/// The S5 slider's maximum.
pub const SHOWCASE_SLIDER_MAX: f64 = 100.0;
/// The S5 slider's step.
pub const SHOWCASE_SLIDER_STEP: f64 = 5.0;

/// Tag on the S5 styled card — the F-tier "elevation" element. It carries the
/// C6 channels the display-list acceptance asserts: a multi-term
/// [`BoxShadow`](buiy_core::render::components::BoxShadow), a
/// per-side [`Border`] band (styled sides + a `BoxModel.border` width), and a
/// rounded `Border.radius`. The widgets sit inside it.
#[derive(Component, Clone, Default)]
pub struct ShowcaseCard;

/// The F-tier card's shadow: a two-term
/// [`BoxShadow`](buiy_core::render::components::BoxShadow) (a soft ambient + a tighter
/// key shadow) — the "elevation" cue. Authored as the single source so the
/// screen-fn and the acceptance test agree on the term count. Both terms are
/// outset (v1 ships outset only) and resolve against real theme tokens.
pub fn showcase_card_shadow() -> buiy_core::render::components::BoxShadow {
    use buiy_core::render::components::{BoxShadow, Shadow};
    BoxShadow(vec![
        // The ambient, wider/softer term.
        Shadow {
            color: ColorToken::Token("color.shadow.card".into()),
            offset_x: Length::px(0.0),
            offset_y: Length::px(2.0),
            blur: Length::px(8.0),
            spread: Length::px(0.0),
            inset: false,
        },
        // The key, tighter term.
        Shadow {
            color: ColorToken::Token("color.shadow.card".into()),
            offset_x: Length::px(0.0),
            offset_y: Length::px(1.0),
            blur: Length::px(3.0),
            spread: Length::px(0.0),
            inset: false,
        },
    ])
}

/// The F-tier card's per-side border PAINT: four styled (solid, real-token) sides.
/// The band only extracts when the layout-owned `BoxModel.border` width is > 0 too
/// (authored on the card's `BoxModel.border` in [`screen_showcase`]); this supplies
/// the per-side colors. The single source the screen-fn + acceptance test share.
pub fn showcase_card_border() -> Border {
    let side = || BorderSide {
        color: ColorToken::Token("color.accent".into()),
        style: buiy_core::render::components::LineStyle::Solid,
    };
    Border {
        top: side(),
        right: side(),
        bottom: side(),
        left: side(),
        radius: Corners::all(Radius::circular(8.0)),
    }
}

/// The S5 F-tier showcase as a composable [`Scene`]: a `#ShowcaseCard` flex-column
/// card with the C6 F-tier channels — a multi-term
/// [`BoxShadow`](buiy_core::render::components::BoxShadow) (elevation), a
/// per-side [`Border`] band (a 2px `BoxModel.border` width + styled sides), and a
/// rounded radius — holding a [`Switch`], a [`Slider`], and a [`Disclosure`], each
/// styled by its scene-fn and **focusable** (so a keyboard focus shows the C6-a
/// focus-ring `Outline`).
///
/// The widgets ride their scene-fns (`switch`/`slider`/`disclosure`), which trigger
/// the full P1d `#[require]` contracts + the canonical pixel styling; S5 makes them
/// **look like** F-tier widgets by sitting them on the elevated, bordered card. The
/// card is the "styled element" whose shadow + border bands the display-list
/// acceptance asserts; the widgets are the focus-ring + function targets.
pub fn screen_showcase() -> impl Scene {
    let shadow = showcase_card_shadow();
    let border = showcase_card_border();
    bsn! {
        #ShowcaseCard
        ShowcaseCard
        Node
        template_value(Display::flex_column())
        FlexParams {
            direction: FlexAxis::Column,
            gap: { FlexGap { row: Length::px(16.0), column: Length::px(16.0) } },
        }
        BoxModel {
            width: { Sizing::Length(Length::Px(280.0)) },
            padding: { Edges::all(20.0) },
            // The layout-owned border WIDTH: the band extracts only when this is
            // > 0 (the per-side PAINT is the `Border` below).
            border: { Edges::all(2.0) },
        }
        Background { color: { ColorToken::Token("color.surface.primary".into()) } }
        // The F-tier channels (C6): the elevation shadow + the per-side border band
        // (styled sides + the rounded radius). Inserted as whole values — `BoxShadow`
        // wraps a `Vec` and `Border` carries `BorderSide`/`Corners` the bsn
        // field-patch path does not author.
        template_value(shadow)
        template_value(border)
        Children [
            (#ShowcaseSwitch switch(SHOWCASE_SWITCH)),
            (
                #ShowcaseSlider
                slider(
                    SHOWCASE_SLIDER,
                    SHOWCASE_SLIDER_NOW,
                    SHOWCASE_SLIDER_MIN,
                    SHOWCASE_SLIDER_MAX,
                    SHOWCASE_SLIDER_STEP
                )
            ),
            (#ShowcaseDisclosure disclosure(SHOWCASE_DISCLOSURE)),
        ]
    }
}

/// Startup system for the S5 binary: a camera, then the showcase screen.
pub fn setup_showcase(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn_scene(screen_showcase());
}
