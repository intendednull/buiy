//! Widget **scene-fns** — the mergeable styled-authoring path for BSN.
//!
//! `Button::new()` / `TextInput::single_line()` (the `impl Bundle` constructors)
//! are for `commands.spawn`. The bare markers carry a `#[require(...)]` contract
//! so `bsn! { Button }` materializes the full widget. But authoring a single
//! field-patch on the bare marker — `bsn! { Button BoxModel { width: … } }` —
//! hits a required-component gotcha: an explicit `BoxModel` patch *suppresses*
//! the `#[require(BoxModel = …)]` initializer entirely, so the patch layers onto
//! the plain component `Default` and the widget's other canonical fields (here,
//! padding) are dropped.
//!
//! These **scene-fns** fix that. Each returns an `impl Scene` whose body spells
//! the widget's styling as explicit `bsn!` FIELD-patches. When a user composes
//! the scene-fn and patches on top — `bsn! { button("Save") BoxModel { width: … } }`
//! — the two field-patches **merge field-wise**: the user's `width` wins while
//! the scene-fn's `height`/`padding` survive. Upstream guarantees this for both
//! the `Clone + Default` blanket path and `FromTemplate` (bevy_scene
//! 0.19.0-rc.3 lib.rs:284-288, 313-352: "unmentioned fields keep their values
//! from earlier patches or the type's defaults, and multiple patches merge
//! rather than overwrite").
//!
//! The bodies reuse the same `pub(crate)` initializer fns the `#[require]`
//! contracts use (`button_box_model()`, `button_background()`, …), so the
//! canonical default values live in exactly one place. Only the styled
//! components a user is likely to patch (`BoxModel`, `Background`, `Border`) and
//! the threaded label/placeholder are spelled out; the rest of the contract
//! (`Node`, the Style decomposition, `Focusable`, `A11yRole`, the editor
//! mechanism) rides the markers' `#[require]`.

use bevy::scene::{Scene, bsn};
use buiy_core::a11y::A11yLabel;
use buiy_core::render::components::{Background, Border};
use buiy_core::text::edit::Placeholder;

use crate::button::{Button, button_background, button_border, button_box_model};
use crate::text_input::{
    TextInput, text_input_background, text_input_border, text_input_box_model, text_input_overflow,
};
use buiy_core::layout::{BoxModel, Overflow};
use buiy_core::text::edit::SingleLine;

/// A labelled button as a composable BSN scene. Mergeable: patch any spelled
/// field on top and the rest of the canonical button survives.
///
/// ```ignore
/// use buiy::prelude::*;
/// // 240px-wide button; height + padding keep the canonical button defaults.
/// world.spawn_scene(bsn! {
///     button("Save")
///     BoxModel { width: { Sizing::Length(Length::Px(240.0)) } }
/// });
/// ```
pub fn button(label: impl Into<String>) -> impl Scene {
    let bm = button_box_model();
    let bg = button_background();
    let border = button_border();
    let label = label.into();
    // Field-patches (not full-value inserts) so a user's outer patch merges.
    // `Button` triggers the rest of the `#[require]` contract.
    bsn! {
        Button
        BoxModel {
            width: { bm.width },
            height: { bm.height },
            padding: { bm.padding },
        }
        Background { color: { bg.color } }
        Border { radius: { border.radius } }
        A11yLabel({ label })
    }
}

/// The shared text-input scene body (editor + display carrier + box + paint +
/// placeholder), spelled as mergeable field-patches. `single_line` layers the
/// `SingleLine` policy on top.
fn text_input_base(placeholder: impl Into<String>) -> impl Scene {
    let bm = text_input_box_model();
    let overflow = text_input_overflow();
    let bg = text_input_background();
    let border = text_input_border();
    let placeholder = placeholder.into();
    bsn! {
        TextInput
        BoxModel {
            width: { bm.width },
            height: { bm.height },
            padding: { bm.padding },
        }
        Overflow { x: { overflow.x }, y: { overflow.y } }
        Background { color: { bg.color } }
        Border { radius: { border.radius } }
        Placeholder({ placeholder })
    }
}

/// A single-line text input as a composable BSN scene (Enter ⇒ Submit, the
/// `SingleLine` policy). Mirrors `TextInput::single_line`. Mergeable.
pub fn text_input_single_line(placeholder: impl Into<String>) -> impl Scene {
    bsn! {
        { text_input_base(placeholder) }
        SingleLine
    }
}

/// A multi-line text input as a composable BSN scene (Enter inserts a newline).
/// Mirrors `TextInput::multi_line`. Mergeable.
///
/// The font size + editor mechanism (`FontSize`, `TextEditState`) are supplied
/// by the `TextInput` `#[require(... = TEXT_INPUT_FONT_SIZE)]` initializers (the
/// editor needs metrics at construction), so the scene-fns do not re-spell
/// them — `#[require]` is the shared source there.
pub fn text_input_multi_line(placeholder: impl Into<String>) -> impl Scene {
    text_input_base(placeholder)
}
