//! Dialog widget — Wave-3 slice-5 (the LAST P1d widget bundle: the a11y SHAPE +
//! the labelling/controls relations, NOT the open/close/focus-trap behavior).
//!
//! A dialog is a **modal container** labelled by its title and described by its
//! body, plus an **invoker** (a button) that `controls` it. This slice builds
//! only the **static a11y shape + the relations** (widget-contracts.md §5
//! "Dialog"): the dialog container is `A11yRole::Dialog` + [`A11yModal`] with
//! `A11yRelations.labelled_by = [title]` / `described_by = [body]` (wired via
//! `Added<Children>`, the disclosure `controls` precedent), and the invoker is a
//! [`Button`](crate::Button) carrying `A11yRelations.controls = [dialog]` — its
//! `Click` rides the EXISTING Button contract → `OnPress` (no new contract).
//!
//! **Deferred to C5 (Wave 4) — NOT built here** (co-drive §3 demand-pull): the
//! dialog **open/close/focus-trap/Esc/restore** overlay state machine (that is
//! Buiy's overlay machine, *not* an AccessKit verb), the dialog **placement**,
//! the `AlertDialog` role (plain `Dialog` only this slice), and the `owns`
//! re-parent (S4's dialog is in-place, so the tree already reflects ownership —
//! no portalling). This slice is a STATIC bundle: a `dialog(...)` builds the
//! labelled modal container, a `dialog_invoker(...)` builds the controlling
//! button; the live show/hide + trap is C5's job over this shape.

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yModal, A11yRelations, A11yRole},
    components::Node,
    layout::{BoxModel, Style},
    render::color::ColorToken,
    render::components::{Background, Border, Corners, Radius, TextColor},
    text::{FontSize, Text},
};
use std::borrow::Cow;

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
///   `AlertDialog` is DEFERRED to C5).
/// - `A11yModal` — the modal flag (`set_modal`), so an AT announces the rest of
///   the page as inert while the dialog is up.
///
/// The `labelled_by`/`described_by` relations are authored by [`Dialog::new`] /
/// the `dialog(...)` scene-fn (they reference the title/body child entities,
/// unknown to the `#[require]` which cannot name a sibling). Like the disclosure
/// `controls` edge, they are filled by [`wire_dialog_relations`] once the
/// children exist.
///
/// **No open/close/focus-trap** — that is C5's overlay state machine (Wave 4).
/// This bundle is the static a11y shape only.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = dialog_box_model(),
    Background = dialog_background(),
    Border = dialog_border(),
    A11yRole = A11yRole::Dialog,
    A11yModal,
)]
pub struct Dialog;

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
    (
        crate::Button,
        A11yLabel(label.into()),
        A11yRelations {
            controls: vec![dialog],
            ..Default::default()
        },
    )
}
