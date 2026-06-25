//! Dialog widget — the P1d a11y SHAPE (Wave 3) **plus** the C5-d (Wave 4)
//! open/close/focus-trap/Esc/restore + scoped focus-trap + inert-background
//! overlay state machine.
//!
//! A dialog is a **modal container** labelled by its title and described by its
//! body, plus an **invoker** (a button) that `controls` it. The P1d slice built
//! the **static a11y shape + the relations** (widget-contracts.md §5 "Dialog"):
//! the dialog container is `A11yRole::Dialog` + [`A11yModal`] with
//! `A11yRelations.labelled_by = [title]` / `described_by = [body]` (wired via
//! `Added<Children>`, the disclosure `controls` precedent), and the invoker is a
//! [`Button`](crate::Button) carrying `A11yRelations.controls = [dialog]` — its
//! `Click` rides the EXISTING Button contract → `OnPress` (no new contract).
//!
//! **C5-d (Wave 4) — the overlay state machine built here** (scroll-overlay-modal.md
//! §C.5). The Dialog `#[require]` now also carries the modal *container* layer:
//! `Stacking = modal_stacking()` ([`TopLayer::Modal`](buiy_core::layout::TopLayer)
//! membership), [`FocusScope::trap`](buiy_core::FocusScope) (the §C.1 trap
//! traversal `buiy_core`'s `handle_tab` scopes to), [`FocusReturn`] (the §C.4
//! restoration target), and `CssVisibility::Hidden` (closed at rest). The
//! lifecycle systems:
//!
//! - **Open** ([`open_dialog_on_invoker_press`]): an invoker's `Click → OnPress`
//!   shows the controlled dialog, captures `FocusReturn`, marks the rest of the
//!   tree [`A11yHidden`] (the inert background — §C.2;
//!   `build_tree` prunes it from the a11y tree, `handle_tab` excludes it from
//!   focus), and queues [`PendingFocus`] to move focus into the dialog the frame
//!   after its children spawn.
//! - **Close** ([`close_dialog_on_escape`] / [`close_dialog_on_button`]): Escape
//!   (WCAG 2.1.2 — Escape ALWAYS escapes, no keyboard trap) or a `DialogClose`
//!   button hides the dialog, clears the inert background, and restores
//!   `FocusReturn` (§C.4).
//! - **Focus-into** ([`resolve_pending_focus`]): moves focus to the dialog's first
//!   focusable descendant the frame after it spawns (the deferred-focus primitive,
//!   §B.3a — scene-fn spawn is not synchronous).
//!
//! The `AlertDialog` role + the `owns` re-parent remain deferred (plain `Dialog`
//! only).

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::interaction::OnPress;
use buiy_core::{
    FocusReturn, FocusScope, FocusVisible, Focusable, FocusedEntity,
    a11y::{A11yHidden, A11yLabel, A11yModal, A11yRelations, A11yRole},
    components::Node,
    layout::{BoxModel, Stacking, Style, TopLayer, TopLayerActivation},
    render::color::ColorToken,
    render::components::{Background, Border, Corners, CssVisibility, Radius, TextColor},
    text::{FontSize, Text},
};
use std::borrow::Cow;

use crate::popover::is_open;

/// The catalog font size for the dialog title glyph (logical px).
pub(crate) const DIALOG_TITLE_FONT_SIZE: f32 = 18.0;
/// The catalog font size for the dialog body glyph (logical px).
pub(crate) const DIALOG_BODY_FONT_SIZE: f32 = 14.0;

/// Dialog container marker. The `#[require(...)]` contract is the single source of
/// the dialog SHAPE (the `Button`/`Checkbox`/… precedent): the bare marker —
/// `world.spawn(Dialog)` / `bsn! { Dialog }` — materializes the full
/// layout-visible + paintable + **modal accessible** container.
///
/// The require list:
/// - `Node` — the layout marker (pulls the full `Style` decomposition).
/// - `BoxModel = dialog_box_model()` — the canonical dialog panel box.
/// - `Background` / `Border` — the panel fill + rounded edge.
/// - `A11yRole = A11yRole::Dialog` — the modal-dialog role (plain `Dialog`;
///   `AlertDialog` is DEFERRED).
/// - `A11yModal` — the modal flag (`set_modal`), so an AT announces the rest of
///   the page as inert while the dialog is up.
/// - `Stacking = modal_stacking()` — `top_layer: TopLayer::Modal` (C5-d): the
///   dialog escapes its parent stacking context, paints above the page, and joins
///   the `TopLayerActivation` deque the Escape/focus-trap handlers consult.
/// - `FocusScope = FocusScope::trap()` — the C5-d modal trap (§C.1): while this is
///   the innermost open modal, `buiy_core`'s `handle_tab` cycles Tab only among
///   the dialog's focusable descendants.
/// - `FocusReturn` — the C5-d restoration target captured on open (§C.4).
/// - `CssVisibility::Hidden` — the dialog starts **closed**; the invoker opens it
///   ([`open_dialog_on_invoker_press`]).
///
/// The `labelled_by`/`described_by` relations are authored by [`Dialog::new`] /
/// the `dialog(...)` scene-fn (they reference the title/body child entities,
/// unknown to the `#[require]` which cannot name a sibling). Like the disclosure
/// `controls` edge, they are filled by [`wire_dialog_relations`] once the
/// children exist.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = dialog_box_model(),
    Background = dialog_background(),
    Border = dialog_border(),
    A11yRole = A11yRole::Dialog,
    A11yModal,
    Stacking = modal_stacking(),
    FocusScope = FocusScope::trap(),
    FocusReturn,
    CssVisibility = CssVisibility::Hidden,
)]
pub struct Dialog;

/// Marker for a **close button** inside a [`Dialog`] (scroll-overlay-modal.md
/// §C.5). A `DialogClose`-marked entity whose `Click → OnPress` closes its
/// enclosing dialog ([`close_dialog_on_button`]) — the pointer/keyboard
/// counterpart of the Escape close. Pair it with [`Button`](crate::Button) for
/// the activation contract.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct DialogClose;

/// C5-d-local marker recording that the dialog lifecycle **set** [`A11yHidden`] on
/// this entity (the inert background, §C.2). On close, the lifecycle removes
/// `A11yHidden` only from entities it marked — it never clobbers an author's own
/// `A11yHidden`, the same framework-owned-marker discipline the focus-ring
/// `FocusRingMarker` uses. Internal to the dialog open/close pair.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct DialogInertedByModal;

/// The dialog's **title** child — the node that *labels* the dialog
/// (`A11yRelations.labelled_by = [title]`). A real `A11yRole::Heading` node (the
/// accessible name source) carrying the visible title pixels. A
/// `buiy_widgets`-local marker so [`wire_dialog_relations`] can find it among the
/// dialog's children.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
#[require(Node, A11yRole = A11yRole::Heading)]
pub struct DialogTitle;

/// The dialog's **body** child — the node that *describes* the dialog
/// (`A11yRelations.described_by = [body]`). A real `A11yRole::Text` node carrying
/// the visible body pixels. A `buiy_widgets`-local marker so
/// [`wire_dialog_relations`] can find it among the dialog's children.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
#[require(Node, A11yRole = A11yRole::Text)]
pub struct DialogBody;

// The initializer fns are `pub(crate)` so the `scene` module's `dialog()`
// scene-fn can spell the SAME canonical values as the `#[require]` path.

/// The canonical dialog panel box: a 320×180 modal panel.
// TODO(buiy-widget-catalog-design): replace hardcoded sizes with size tokens.
pub(crate) fn dialog_box_model() -> BoxModel {
    Style::default().width_px(320.0).height_px(180.0).box_model
}

/// The default dialog panel fill (the `color.surface.primary` token).
pub(crate) fn dialog_background() -> Background {
    Background {
        color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
    }
}

/// The default dialog panel border: rounded corners (`radius.md`).
pub(crate) fn dialog_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(8.0)),
        ..Default::default()
    }
}

/// The canonical dialog `Stacking`: `top_layer: TopLayer::Modal` (C5-d, §C.5). A
/// modal dialog escapes its parent stacking context, paints above the page, and
/// joins the `TopLayerActivation` deque the Escape/focus-trap handlers consult.
/// `pub(crate)` so the scene-fn spells the SAME value as the `#[require]`.
pub(crate) fn modal_stacking() -> Stacking {
    Stacking {
        top_layer: TopLayer::Modal,
        ..Default::default()
    }
}

impl Dialog {
    /// Spawn-ready bundle for a titled + described dialog. Returns `impl Bundle`
    /// carrying the full container SHAPE (role `Dialog` + `A11yModal` + the panel
    /// box) plus two children: the **title** (`A11yRole::Heading`, the label
    /// source) and the **body** (`A11yRole::Text`, the description source), each a
    /// real semantic node carrying its visible pixels.
    ///
    /// The dialog's `A11yRelations.labelled_by = [title]` / `described_by = [body]`
    /// edges are wired by [`wire_dialog_relations`] once the `children!` exist (a
    /// `#[require]`/closure cannot reference a sibling entity at construction —
    /// the disclosure `controls` precedent).
    ///
    /// **No open/close/focus-trap** — the live show/hide + trap is C5 (Wave 4);
    /// this is the static a11y shape only.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> impl Bundle {
        let title = title.into();
        let body = body.into();
        (
            Dialog,
            children![
                // The title — labels the dialog (resolved into `labelled_by`).
                // Decorative text ⇒ `Pickable::IGNORE` (pick-through, co-drive
                // SC-3): a hit on the title is not a separate target.
                (
                    DialogTitle,
                    Text(title.clone()),
                    FontSize(DIALOG_TITLE_FONT_SIZE),
                    TextColor::default(),
                    A11yLabel(title),
                    Pickable::IGNORE,
                ),
                // The body — describes the dialog (resolved into `described_by`).
                (
                    DialogBody,
                    Text(body.clone()),
                    FontSize(DIALOG_BODY_FONT_SIZE),
                    TextColor::default(),
                    A11yLabel(body),
                    Pickable::IGNORE,
                ),
            ],
        )
    }
}

/// The query data [`wire_dialog_relations`] reads per dialog: the dialog entity,
/// its `Children` (to find the title/body), and its current `A11yRelations` (to
/// preserve author-set edges + check idempotency). Aliased so the system
/// signature stays under clippy's `type_complexity` bar.
type DialogRelationsData = (Entity, &'static Children, Option<&'static A11yRelations>);

/// The change-detection filter for [`wire_dialog_relations`]: a `Dialog` that just
/// gained its `Children` this frame (the `children!` macro inserts `Children`
/// after the root spawns), so the labelling wiring runs once per dialog. Aliased
/// to keep the system signature simple.
type NewlyChildedDialog = (With<Dialog>, Added<Children>);

/// Wire each dialog's `A11yRelations.labelled_by`/`described_by` to its
/// [`DialogTitle`] / [`DialogBody`] children (Wave-3 slice-5). The
/// edges reference the title/body **entities**, which do not exist until the
/// dialog's `children!` are spawned, so they cannot be set in `Dialog::new`'s
/// bundle / the `#[require]` contract; this system fills them once, on the frame
/// the dialog gains its children — the disclosure `wire_disclosure_controls`
/// precedent.
///
/// Gated on `Added<Children>` so it runs once per newly-childed dialog. It fills
/// the labelling edges if the dialog does not already carry them (the scene-fn
/// path authors them directly, so this is idempotent — it never overwrites an
/// author-set edge). Registered in `WidgetsPlugin`.
pub fn wire_dialog_relations(
    mut commands: Commands,
    dialogs: Query<DialogRelationsData, NewlyChildedDialog>,
    titles: Query<(), With<DialogTitle>>,
    bodies: Query<(), With<DialogBody>>,
) {
    for (dialog, children, relations) in &dialogs {
        // The labelling edges are already authored (scene-fn path) ⇒ leave them.
        if relations.is_some_and(|r| !r.labelled_by.is_empty() || !r.described_by.is_empty()) {
            continue;
        }
        let title = children.iter().find(|&c| titles.get(c).is_ok());
        let body = children.iter().find(|&c| bodies.get(c).is_ok());
        // A malformed dialog with neither child — nothing to wire.
        if title.is_none() && body.is_none() {
            continue;
        }
        // Preserve any other author-set relations; only fill labelling.
        let mut next = relations.cloned().unwrap_or_default();
        if let Some(title) = title {
            next.labelled_by = vec![title];
        }
        if let Some(body) = body {
            next.described_by = vec![body];
        }
        commands.entity(dialog).insert(next);
    }
}

/// Spawn-ready bundle for a dialog **invoker** — the button that opens (and so
/// `controls`) a dialog (widget-contracts.md §5 "Dialog": "the INVOKER advertises
/// `{Click}` + `controls=[dialog]`"). Returns `impl Bundle`: a labelled
/// [`Button`](crate::Button) (the full Button `#[require]` contract — role
/// `Button`, focus, the APG Enter+Space keymap, `Click → OnPress`) plus
/// `A11yRelations.controls = [dialog]`.
///
/// The invoker references the dialog **entity**, so unlike the dialog's own
/// labelling (a sibling-child edge wired post-spawn) the `controls` edge is set
/// **at construction** — the caller already holds the dialog entity (it spawned
/// the dialog first). The invoker's `Click` rides the EXISTING Button contract →
/// `OnPress`; this slice adds **no** open behavior (clicking does not open the
/// dialog yet — that wiring is C5's overlay state machine).
pub fn dialog_invoker(label: impl Into<String>, dialog: Entity) -> impl Bundle {
    // `Button::new` carries the full visible button (incl. the centered label
    // `Text` child); layer the invoker's `controls` relation on top.
    (
        crate::Button::new(label),
        A11yRelations {
            controls: vec![dialog],
            ..Default::default()
        },
    )
}

// ---------------------------------------------------------------------------
// C5-d — the open/close/focus-trap/Esc/restore + inert-background overlay state
// machine (scroll-overlay-modal.md §C.5).
// ---------------------------------------------------------------------------

/// Request to move focus into a dialog once its focusable content exists
/// (scroll-overlay-modal.md §B.3a — the deferred-focus primitive). Inserted on the
/// dialog the frame it opens; drained by [`resolve_pending_focus`] in
/// `BuiySet::Input` *after* the spawn flush, so the first poll already sees any
/// children spawned in the opening frame.
///
/// Scene-fn / `children!` spawn is not synchronous: a freshly-spawned dialog's
/// focusable descendants are queued commands that do not exist in the same frame,
/// so an in-frame "focus the first focusable child" would find nothing. This
/// component carries a retry `budget` (frames) so a multi-frame spawn still lands
/// focus; if the budget expires with no focusable descendant, focus falls back to
/// the dialog root so focus is never stranded outside the trap.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct PendingFocus {
    /// Frames remaining before falling back to focusing the dialog root.
    pub budget: u8,
}

impl Default for PendingFocus {
    fn default() -> Self {
        Self { budget: 4 }
    }
}

/// The per-invoker query data [`open_dialog_on_invoker_press`] resolves the
/// controlled dialog from. Aliased to keep the system signature under clippy's
/// `type_complexity` bar (the menu `MenuButtonWireData` precedent).
type InvokerControls = (Entity, &'static A11yRelations);

/// The per-dialog query data [`apply_dialog_modal_state`] reads on an open/close
/// transition: the dialog entity, its `CssVisibility` (open state), and its
/// current `FocusReturn` (the restore target captured on open). Aliased to keep
/// the system signature under clippy's `type_complexity` bar.
type DialogTransitionData = (Entity, &'static CssVisibility, Option<&'static FocusReturn>);

/// The change-detection filter for [`apply_dialog_modal_state`]: a `Dialog` whose
/// visibility just changed this frame (the open/close transition).
type ChangedDialogVisibility = (With<Dialog>, Changed<CssVisibility>);

/// Open a dialog when its **invoker** is activated (scroll-overlay-modal.md §C.5).
/// The invoker's `Click` rides the EXISTING Button contract → the shared
/// [`OnPress`] sink (pointer / keyboard Enter+Space / AT-`Click` all converge);
/// this consumer reads each `OnPress(invoker)`, resolves the invoker's
/// `A11yRelations.controls` first entry (the dialog), and **opens** it:
///
/// 1. show the dialog (`CssVisibility::Visible`),
/// 2. capture `FocusReturn` = the entity focused at open time (the invoker, after
///    its own `focus_on_click`) so close can restore it (§C.4),
/// 3. queue [`PendingFocus`] to move focus into the dialog the frame after its
///    children spawn (§B.3a).
///
/// The inert-background marking is a separate reactive system
/// (`apply_dialog_modal_state`) keyed on the dialog's visibility change, so a
/// dialog opened by ANY path (invoker, an author flipping `CssVisibility`, an AT
/// verb) gets the inert background — not just this invoker path.
///
/// Runs in `BuiySet::Input` so a same-frame activation opens the dialog the same
/// frame.
pub fn open_dialog_on_invoker_press(
    mut reader: MessageReader<OnPress>,
    invokers: Query<InvokerControls>,
    mut dialogs: Query<&mut CssVisibility, With<Dialog>>,
    mut commands: Commands,
) {
    for OnPress(invoker) in reader.read() {
        let Ok((_, relations)) = invokers.get(*invoker) else {
            continue; // the pressed entity advertises no controls — not an invoker.
        };
        let Some(&dialog) = relations.controls.first() else {
            continue;
        };
        let Ok(mut vis) = dialogs.get_mut(dialog) else {
            continue; // controls something that is not a dialog — ignore.
        };
        if *vis == CssVisibility::Visible {
            continue; // already open — idempotent.
        }
        *vis = CssVisibility::Visible;
        // Queue the deferred focus-into; the apply_dialog_modal_state reaction
        // (on the visibility change) captures FocusReturn + marks the background.
        commands.entity(dialog).insert(PendingFocus::default());
    }
}

/// Drive the modal **inert background** + `FocusReturn` capture from each dialog's
/// open/close transition (scroll-overlay-modal.md §C.2 + §C.5). Reacts to
/// `Changed<CssVisibility>` on a `Dialog` so it fires once per open/close
/// regardless of WHAT flipped the visibility (the invoker, an author, an AT verb):
///
/// - **Open** (`Visible`): capture `FocusReturn` = the currently-focused entity
///   (typically the invoker), then mark every other top-level root
///   [`A11yHidden`](buiy_core::a11y::A11yHidden) (tagged [`DialogInertedByModal`]
///   so close removes exactly those). `build_tree` prunes the inert subtrees from
///   the a11y tree (semantic-tree.md §7.4) and `handle_tab` excludes them from
///   focus — the dialog's subtree (a top-level root itself) is left interactive.
/// - **Close** (`Hidden`/`Collapse`): clear the `A11yHidden` the lifecycle set
///   (only `DialogInertedByModal`-tagged), then restore focus to `FocusReturn`
///   (§C.4) — but only when focus is still inside the now-closed dialog (never
///   steal focus from an unrelated entity).
///
/// Runs in `BuiySet::Input` `.after(open_dialog_on_invoker_press)` so a same-frame
/// open is reacted to the same frame.
///
/// `pub(crate)` (not `pub`) because its signature names the crate-private
/// [`DialogInertedByModal`] bookkeeping marker; it is registration-only — tests
/// drive the lifecycle through `OnPress`/Escape, never by calling this directly.
pub(crate) fn apply_dialog_modal_state(
    changed: Query<DialogTransitionData, ChangedDialogVisibility>,
    roots: Query<Entity, Without<ChildOf>>,
    parents: Query<&ChildOf>,
    inerted: Query<Entity, With<DialogInertedByModal>>,
    mut focused: Option<ResMut<FocusedEntity>>,
    mut focus_visible: Option<ResMut<FocusVisible>>,
    mut commands: Commands,
) {
    for (dialog, vis, focus_return) in &changed {
        if is_open(Some(vis)) {
            // OPEN: capture the restore target (the focused entity at open time).
            let restore = focused.as_ref().and_then(|f| f.0);
            commands.entity(dialog).insert(FocusReturn(restore));

            // Mark every top-level root OUTSIDE the dialog subtree inert. The
            // dialog is itself a top-layer root, so marking the *other* roots
            // (and thus their whole subtrees, via the build_tree/handle_tab
            // ancestor walks) leaves only the dialog interactive.
            for root in &roots {
                if root == dialog || is_self_or_ancestor(dialog, root, &parents) {
                    continue; // never inert the dialog or an ancestor of it.
                }
                commands
                    .entity(root)
                    .insert((A11yHidden, DialogInertedByModal));
            }
        } else {
            // CLOSE: clear the inert background the lifecycle set.
            for e in &inerted {
                commands
                    .entity(e)
                    .remove::<A11yHidden>()
                    .remove::<DialogInertedByModal>();
            }
            // Restore focus to the captured target — but only when focus is still
            // inside the now-closed dialog (or nothing is focused), never stealing
            // it from an unrelated entity (§C.4).
            let target = focus_return.and_then(|r| r.0);
            let focus_inside = focused
                .as_ref()
                .and_then(|f| f.0)
                .is_none_or(|f| is_self_or_ancestor(f, dialog, &parents));
            if focus_inside {
                if let Some(f) = focused.as_mut() {
                    f.0 = target;
                }
                if let Some(v) = focus_visible.as_mut() {
                    v.0 = true; // keyboard/programmatic restore ⇒ focus-visible.
                }
            }
        }
    }
}

/// Move focus into a dialog once its focusable content exists (scroll-overlay-modal.md
/// §B.3a). Drains each [`PendingFocus`] in `BuiySet::Input`: finds the dialog's
/// first focusable descendant in document order and focuses it; if none exists yet
/// (children still spawning), decrements the retry budget and waits a frame; on
/// budget exhaustion it focuses the dialog root so focus is never stranded outside
/// the trap.
///
/// Level-triggered on subtree membership (a per-frame query), not edge-triggered
/// on `Added<Focusable>` — the `PendingFocus` may be inserted a frame after the
/// child spawned, which an edge detector would miss.
pub fn resolve_pending_focus(
    mut pending: Query<(Entity, &mut PendingFocus)>,
    children: Query<&Children>,
    focusables: Query<(), With<Focusable>>,
    hidden: Query<(), With<A11yHidden>>,
    mut focused: Option<ResMut<FocusedEntity>>,
    mut focus_visible: Option<ResMut<FocusVisible>>,
    mut commands: Commands,
) {
    for (dialog, mut req) in pending.iter_mut() {
        let first = first_focusable_descendant(dialog, &children, &focusables, &hidden);
        let target = match first {
            Some(child) => Some(child),
            None if req.budget == 0 => Some(dialog), // give up — focus the root.
            None => {
                req.budget = req.budget.saturating_sub(1);
                continue; // retry next frame.
            }
        };
        if let Some(target) = target {
            if let Some(f) = focused.as_mut() {
                f.0 = Some(target);
            }
            if let Some(v) = focus_visible.as_mut() {
                v.0 = true; // programmatic focus-into ⇒ focus-visible.
            }
        }
        commands.entity(dialog).remove::<PendingFocus>();
    }
}

/// Close the top-most open modal dialog on **Escape** (scroll-overlay-modal.md
/// §C.5; WCAG 2.1.2 — Escape ALWAYS escapes, so a modal is never a keyboard trap).
/// "Top-most" is the back of `TopLayerActivation.order` (most-recently-activated)
/// filtered to open `Dialog`s. Flips the dialog's `CssVisibility` to `Hidden`;
/// `apply_dialog_modal_state` then clears the inert background + restores focus.
///
/// `keys` is an `Option<Res<…>>` (the `escape_dismiss` precedent): a headless
/// harness without an input stack has no keyboard, so the system is a no-op there.
/// Runs in `BuiySet::Input`.
pub fn close_dialog_on_escape(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    activation: Option<Res<TopLayerActivation>>,
    mut dialogs: Query<&mut CssVisibility, With<Dialog>>,
) {
    let Some(keys) = keys else { return };
    let Some(activation) = activation else { return };
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    // The top-most open dialog (back of the deque first).
    let topmost = activation
        .order
        .iter()
        .rev()
        .copied()
        .find(|&e| dialogs.get(e).is_ok_and(|v| is_open(Some(v))));
    if let Some(dialog) = topmost
        && let Ok(mut vis) = dialogs.get_mut(dialog)
    {
        *vis = CssVisibility::Hidden;
    }
}

/// Close a dialog when a [`DialogClose`] button inside it is activated
/// (scroll-overlay-modal.md §C.5 — the pointer/keyboard close-button counterpart
/// of Escape). Reads the shared [`OnPress`] sink; for each pressed `DialogClose`,
/// walks up the `ChildOf` chain to the enclosing `Dialog` and hides it. Runs in
/// `BuiySet::Input`.
pub fn close_dialog_on_button(
    mut reader: MessageReader<OnPress>,
    closers: Query<(), With<DialogClose>>,
    parents: Query<&ChildOf>,
    is_dialog: Query<(), With<Dialog>>,
    mut dialogs: Query<&mut CssVisibility, With<Dialog>>,
) {
    for OnPress(pressed) in reader.read() {
        if closers.get(*pressed).is_err() {
            continue; // not a close button.
        }
        let Some(dialog) = enclosing_dialog(*pressed, &parents, &is_dialog) else {
            continue;
        };
        if let Ok(mut vis) = dialogs.get_mut(dialog) {
            *vis = CssVisibility::Hidden;
        }
    }
}

/// Whether `a` is `ancestor` or any `ChildOf` ancestor of `a` is `ancestor` (the
/// "is `a` inside `ancestor`'s subtree" test). `ancestor` counts as inside itself.
fn is_self_or_ancestor(a: Entity, ancestor: Entity, parents: &Query<&ChildOf>) -> bool {
    let mut cur = a;
    loop {
        if cur == ancestor {
            return true;
        }
        match parents.get(cur) {
            Ok(p) => cur = p.parent(),
            Err(_) => return false,
        }
    }
}

/// The nearest enclosing [`Dialog`] of `e` (including `e` itself), walking up
/// `ChildOf`. `None` if no ancestor is a dialog.
fn enclosing_dialog(
    e: Entity,
    parents: &Query<&ChildOf>,
    is_dialog: &Query<(), With<Dialog>>,
) -> Option<Entity> {
    let mut cur = e;
    loop {
        if is_dialog.contains(cur) {
            return Some(cur);
        }
        cur = parents.get(cur).ok()?.parent();
    }
}

/// The first focusable descendant of `dialog` in document order (the `Children`
/// DFS order, matching `handle_tab`'s tab-order definition) that is not inert
/// (`A11yHidden` self/ancestor). `None` if the dialog has no focusable content yet
/// (children still spawning) — the [`resolve_pending_focus`] retry case.
fn first_focusable_descendant(
    dialog: Entity,
    children: &Query<&Children>,
    focusables: &Query<(), With<Focusable>>,
    hidden: &Query<(), With<A11yHidden>>,
) -> Option<Entity> {
    fn dfs(
        e: Entity,
        children: &Query<&Children>,
        focusables: &Query<(), With<Focusable>>,
        hidden: &Query<(), With<A11yHidden>>,
    ) -> Option<Entity> {
        if hidden.contains(e) {
            return None; // skip an inert subtree wholesale.
        }
        if focusables.contains(e) {
            return Some(e);
        }
        if let Ok(kids) = children.get(e) {
            for &child in kids {
                if let Some(found) = dfs(child, children, focusables, hidden) {
                    return Some(found);
                }
            }
        }
        None
    }
    // The dialog root itself is not a focus target (it is a container); descend
    // into its children for the first focusable.
    let kids = children.get(dialog).ok()?;
    kids.iter()
        .find_map(|child| dfs(child, children, focusables, hidden))
}
