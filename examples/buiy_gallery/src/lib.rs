//! `buiy_gallery` — the widget-catalog campaign's runnable exemplar app and the
//! `buiy_verify` screen-fixture source. **C8-a delivers S1 (TodoMVC).**
//!
//! S1 composes the landed P1d widget bundles (single-line [`TextInput`], the
//! tri-state [`Checkbox`], [`Button`]) + the A11yLive `Status` live region into
//! the literal TodoMVC exemplar: type the "What needs to be done?" field + Enter
//! to add a row, toggle a row's checkbox to complete it, destroy a row, clear
//! completed, filter All/Active/Completed, and a "N items left" count that lives
//! in an `A11yRole::Status` aria-live region. Double-clicking a row's label edits
//! it in place (C3b `MultiClick`).
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
    MessageReader, On, Plugin, Query, Res, ResMut, Resource, Update, With, World,
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
