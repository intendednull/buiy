//! `hello_bsn` — the headline proof that **BSN authoring works** in Buiy.
//!
//! A real Buiy UI tree authored declaratively with the `bsn!` macro through
//! `use buiy::prelude::*;` — a styled container whose `Children [ … ]` are the
//! widget **scene-fns** (`button(label)`, `text_input_single_line(...)`), the
//! mergeable styled-authoring path: patching a field on top of a scene-fn keeps
//! the widget's other canonical defaults (spec § 4.1c).
//!
//! The tree is factored into [`hello_bsn_scene`] so the binary's startup system
//! (`commands.spawn_scene`) and the headless layout-snapshot gate
//! (`world.spawn_scene`) author the **same** scene — the example IS the test
//! fixture.

use bevy::prelude::*;
use buiy::prelude::*;

/// The Buiy-via-BSN demo tree, as a composable [`Scene`].
///
/// A `#Toolbar` flex-column container (padded, filled) holding, top to bottom:
/// - a `#Search` single-line text input (the editor widget),
/// - a `#Actions` flex-row holding a `#Save` and a `#Cancel` button.
///
/// The `#Save` button overrides its width via a field-patch on the `button()`
/// scene-fn — `bsn! { button("Save") BoxModel { width: … } }` — to demonstrate
/// the **merge** path: the wider box keeps the button's canonical 8px padding
/// (it is NOT dropped, the way a single-field patch on the bare `#[require]`d
/// marker would drop it). `#Cancel` is the unpatched canonical button.
///
/// Every entity is `#Name`-tagged so the tree is observable in a stable,
/// content-keyed layout snapshot (Tier 1 of `buiy_verify`).
pub fn hello_bsn_scene() -> impl Scene {
    bsn! {
        #Toolbar
        // `Node` is the layout marker; it `#[require]`s the full Style
        // decomposition, so the container is layout-valid with only the
        // components we override spelled out below.
        Node
        // `template_value` inserts a whole component value (the
        // constructor result) — the right form for a non-default enum
        // component, since `bsn!`'s enum-variant patching needs per-variant
        // `default_*` methods the blanket `Clone + Default` path doesn't emit.
        template_value(Display::flex_column())
        FlexParams {
            direction: FlexAxis::Column,
            gap: { FlexGap { row: Length::px(8.0), column: Length::px(8.0) } },
        }
        BoxModel {
            width: { Sizing::Length(Length::Px(340.0)) },
            padding: { Edges::all(12.0) },
        }
        Background { color: { ColorToken::SurfacePrimary } }
        Children [
            (#Search text_input_single_line("Search…")),
            (
                #Actions
                Node
                template_value(Display::flex_row())
                FlexParams {
                    direction: FlexAxis::Row,
                    gap: { FlexGap { row: Length::px(8.0), column: Length::px(8.0) } },
                }
                // Wide enough that the two buttons keep their natural border-box
                // sizes (no flex-shrink), so the snapshot shows the merge
                // cleanly: Save = 140 + 16 padding, Cancel = 120 + 16 padding.
                BoxModel { width: { Sizing::Length(Length::Px(320.0)) } }
                Children [
                    (#Save button("Save") BoxModel { width: { Sizing::Length(Length::Px(140.0)) } }),
                    (#Cancel button("Cancel")),
                ]
            ),
        ]
    }
}

/// Startup system: spawn the BSN-authored tree (and a 2D camera so it renders).
pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn_scene(hello_bsn_scene());
}
