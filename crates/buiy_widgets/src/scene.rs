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

use bevy::ecs::hierarchy::Children;
use bevy::picking::Pickable;
use bevy::scene::{Scene, bsn, template_value};
use buiy_core::a11y::{A11yLabel, A11yOrientation, A11yRole, A11yValue, Orientation};
use buiy_core::components::Node;
use buiy_core::layout::Translate;
use buiy_core::render::components::{Background, Border, CssVisibility, TextColor};
use buiy_core::text::edit::Placeholder;
use buiy_core::text::{FontSize, Text};

use crate::button::{Button, button_background, button_border, button_box_model};
use crate::checkbox::{
    CHECK_GLYPH, CHECKBOX_MARK_FONT_SIZE, Checkbox, CheckboxMark, checkbox_background,
    checkbox_border, checkbox_box_model,
};
use crate::dialog::{
    DIALOG_BODY_FONT_SIZE, DIALOG_TITLE_FONT_SIZE, Dialog, DialogBody, DialogTitle,
    dialog_background, dialog_border, dialog_box_model,
};
use crate::disclosure::{
    CARET_GLYPH, DISCLOSURE_FONT_SIZE, Disclosure, DisclosureCaret, DisclosurePanel,
    caret_rotation_collapsed, disclosure_background, disclosure_border, disclosure_box_model,
    disclosure_panel_background, disclosure_panel_box_model,
};
use crate::slider::{
    SLIDER_LABEL_FONT_SIZE, Slider, SliderThumb, SliderTrack, slider_background, slider_border,
    slider_box_model, slider_thumb_background, slider_thumb_border, slider_thumb_box_model,
    slider_track_background, slider_track_box_model,
};
use crate::switch::{
    SWITCH_LABEL_FONT_SIZE, Switch, SwitchThumb, switch_background, switch_border,
    switch_box_model, switch_thumb_background, switch_thumb_border, switch_thumb_box_model,
};
use crate::text_input::{
    TextInput, text_input_background, text_input_border, text_input_box_model, text_input_overflow,
};
use crate::tooltip::{
    TOOLTIP_FONT_SIZE, TooltipNode, TooltipTrigger, tooltip_background, tooltip_box_model,
    tooltip_trigger_background, tooltip_trigger_border, tooltip_trigger_box_model,
};
use buiy_core::layout::{BoxModel, Length, Overflow};
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
///
/// Layers the single-line **role** override (`A11yRole::TextInput`) on top of the
/// shared base — the bare marker / `text_input_multi_line` path defaults to
/// `A11yRole::MultilineTextInput` (the role split IS the multiline distinction,
/// widget-contracts.md §5). `A11yRole` is a fieldless enum variant, which the bsn
/// field-patch path does not author, so it is inserted as a whole value
/// (`template_value`).
pub fn text_input_single_line(placeholder: impl Into<String>) -> impl Scene {
    bsn! {
        { text_input_base(placeholder) }
        SingleLine
        template_value(A11yRole::TextInput)
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

/// A labelled checkbox as a composable BSN scene (Wave-3 slice-1). Mergeable: the
/// `Checkbox` marker triggers the full `#[require]` contract (role + tri-state
/// `A11yToggled` + focus + a11y + box), and the spelled field-patches layer the
/// canonical box style on top. The `Children [ … ]` subtree authors the check/
/// dash **mark** glyph and the visible **label** `Text`, both `Pickable::IGNORE`
/// so a hit resolves to the widget root the router addresses (pick-through,
/// co-drive SC-3). The mark starts `CssVisibility::Hidden` (the default toggle is
/// `False`); `update_checkbox_visual` reveals it on the first state flip.
///
/// ```ignore
/// use buiy::prelude::*;
/// world.spawn_scene(bsn! { checkbox("Done") });
/// ```
pub fn checkbox(label: impl Into<String>) -> impl Scene {
    let bm = checkbox_box_model();
    let bg = checkbox_background();
    let border = checkbox_border();
    let label = label.into();
    bsn! {
        Checkbox
        BoxModel {
            width: { bm.width },
            height: { bm.height },
        }
        Background { color: { bg.color } }
        Border { radius: { border.radius } }
        A11yLabel({ label.clone() })
        Children [
            (
                CheckboxMark
                Text({ CHECK_GLYPH.to_string() })
                FontSize({ CHECKBOX_MARK_FONT_SIZE })
                template_value(TextColor::default())
                // The mark starts hidden (default toggle is `False`);
                // `update_checkbox_visual` reveals it on the first flip.
                // `CssVisibility` / `Pickable` are inserted as whole values
                // (`template_value`) — bsn's field-patch path does not author a
                // fieldless enum variant / an associated `const`.
                template_value(CssVisibility::Hidden)
                template_value(Pickable::IGNORE)
            ),
            (
                Text({ label })
                FontSize({ CHECKBOX_MARK_FONT_SIZE })
                template_value(TextColor::default())
                template_value(Pickable::IGNORE)
            ),
        ]
    }
}

/// A labelled switch as a composable BSN scene (Wave-3 slice-1). Mergeable: the
/// `Switch` marker triggers the full `#[require]` contract (role + binary
/// `A11yToggled` + focus + a11y + track), and the field-patches layer the
/// canonical pill style. The `Children [ … ]` subtree authors the sliding **thumb**
/// and the visible **label** `Text`, both `Pickable::IGNORE` (pick-through). The
/// thumb starts at the off position (`Translate` x = 0, the default toggle is
/// `False`); `update_switch_visual` slides it on the first state flip.
///
/// ```ignore
/// use buiy::prelude::*;
/// world.spawn_scene(bsn! { switch("Wi-Fi") });
/// ```
pub fn switch(label: impl Into<String>) -> impl Scene {
    let bm = switch_box_model();
    let bg = switch_background();
    let border = switch_border();
    let thumb_bm = switch_thumb_box_model();
    let thumb_bg = switch_thumb_background();
    let thumb_border = switch_thumb_border();
    let label = label.into();
    bsn! {
        Switch
        BoxModel {
            width: { bm.width },
            height: { bm.height },
        }
        Background { color: { bg.color } }
        Border { radius: { border.radius } }
        A11yLabel({ label.clone() })
        Children [
            (
                SwitchThumb
                Node
                BoxModel {
                    width: { thumb_bm.width },
                    height: { thumb_bm.height },
                }
                Background { color: { thumb_bg.color } }
                Border { radius: { thumb_border.radius } }
                // The thumb starts at the off position (x = 0); inserted as a
                // whole value because `Translate` is a tuple struct.
                template_value(Translate(Length::px(0.0), Length::px(0.0), Length::px(0.0)))
                template_value(Pickable::IGNORE)
            ),
            (
                Text({ label })
                FontSize({ SWITCH_LABEL_FONT_SIZE })
                template_value(TextColor::default())
                template_value(Pickable::IGNORE)
            ),
        ]
    }
}

/// A labelled slider over `[min, max]` (starting at `now`, stepping by `step`) as
/// a composable BSN scene (Wave-3 slice-2). Mergeable: the `Slider` marker
/// triggers the full `#[require]` contract (role, valued range, orientation,
/// focus, a11y, and the track box), the field-patches layer the canonical rail
/// style, and the `A11yValue`/`A11yOrientation` whole-value patches author the
/// live range and horizontal orientation. The `Children [ … ]` subtree authors
/// the **track** rail, the sliding **thumb**, and the visible **label** `Text`,
/// each `Pickable::IGNORE` (pick-through). The thumb starts at x = 0, and
/// `update_slider_visual` positions it from `A11yValue` on the first state change.
///
/// ```ignore
/// use buiy::prelude::*;
/// world.spawn_scene(bsn! { slider("Volume", 50.0, 0.0, 100.0, 1.0) });
/// ```
pub fn slider(label: impl Into<String>, now: f64, min: f64, max: f64, step: f64) -> impl Scene {
    let bm = slider_box_model();
    let bg = slider_background();
    let border = slider_border();
    let track_bm = slider_track_box_model();
    let track_bg = slider_track_background();
    let thumb_bm = slider_thumb_box_model();
    let thumb_bg = slider_thumb_background();
    let thumb_border = slider_thumb_border();
    let label = label.into();
    bsn! {
        Slider
        BoxModel {
            width: { bm.width },
            height: { bm.height },
        }
        Background { color: { bg.color } }
        Border { radius: { border.radius } }
        A11yLabel({ label.clone() })
        // The valued range + orientation are inserted as whole values: `A11yValue`
        // carries `Option` fields the bsn field-patch path does not author, and
        // `A11yOrientation` wraps a fieldless foreign enum.
        template_value(A11yValue {
            now,
            min,
            max,
            step: Some(step),
            jump: None,
            text: None,
        })
        template_value(A11yOrientation(Orientation::Horizontal))
        Children [
            (
                SliderTrack
                Node
                BoxModel {
                    width: { track_bm.width },
                    height: { track_bm.height },
                }
                Background { color: { track_bg.color } }
                template_value(Pickable::IGNORE)
            ),
            (
                SliderThumb
                Node
                BoxModel {
                    width: { thumb_bm.width },
                    height: { thumb_bm.height },
                }
                Background { color: { thumb_bg.color } }
                Border { radius: { thumb_border.radius } }
                // The thumb starts at the min end (x = 0); inserted as a whole
                // value because `Translate` is a tuple struct.
                template_value(Translate(Length::px(0.0), Length::px(0.0), Length::px(0.0)))
                template_value(Pickable::IGNORE)
            ),
            (
                Text({ label })
                FontSize({ SLIDER_LABEL_FONT_SIZE })
                template_value(TextColor::default())
                template_value(Pickable::IGNORE)
            ),
        ]
    }
}

/// A labelled disclosure as a composable BSN scene (Wave-3 slice-3). Mergeable: the
/// `Disclosure` marker triggers the full `#[require]` contract (role `Button` + the
/// `A11yExpanded` state + focus + a11y + the trigger box), and the field-patches
/// layer the canonical row style. The `Children [ … ]` subtree authors the
/// decorative **caret** glyph + the visible **label** `Text` (both
/// `Pickable::IGNORE` — pick-through) and the controlled **panel**
/// (`A11yRole::Region`). The caret starts collapsed (`Rotate` identity ⇒ pointing
/// right) and the panel starts `CssVisibility::Hidden` (the default `A11yExpanded`
/// is `false`); `update_disclosure_visual` rotates the caret + reveals the panel on
/// the first state flip.
///
/// The trigger's `A11yRelations.controls = [panel]` is wired by
/// `wire_disclosure_controls` once the children exist (the panel entity is unknown
/// at authoring time, so neither the bundle nor the scene can spell it).
///
/// ```ignore
/// use buiy::prelude::*;
/// world.spawn_scene(bsn! { disclosure("Details") });
/// ```
pub fn disclosure(label: impl Into<String>) -> impl Scene {
    let bm = disclosure_box_model();
    let bg = disclosure_background();
    let border = disclosure_border();
    let panel_bm = disclosure_panel_box_model();
    let panel_bg = disclosure_panel_background();
    let label = label.into();
    bsn! {
        Disclosure
        BoxModel {
            width: { bm.width },
            height: { bm.height },
        }
        Background { color: { bg.color } }
        Border { radius: { border.radius } }
        A11yLabel({ label.clone() })
        Children [
            (
                DisclosureCaret
                Text({ CARET_GLYPH.to_string() })
                FontSize({ DISCLOSURE_FONT_SIZE })
                template_value(TextColor::default())
                // The caret starts collapsed (Rotate identity ⇒ pointing right);
                // inserted as a whole value because `Rotate` is a tuple struct
                // wrapping a foreign `Quat`. `update_disclosure_visual` rotates it.
                template_value(caret_rotation_collapsed())
                template_value(Pickable::IGNORE)
            ),
            (
                Text({ label })
                FontSize({ DISCLOSURE_FONT_SIZE })
                template_value(TextColor::default())
                template_value(Pickable::IGNORE)
            ),
            (
                DisclosurePanel
                Node
                BoxModel {
                    width: { panel_bm.width },
                    height: { panel_bm.height },
                }
                Background { color: { panel_bg.color } }
                // The panel starts hidden (default expanded is `false`);
                // `update_disclosure_visual` reveals it on the first flip.
                template_value(CssVisibility::Hidden)
            ),
        ]
    }
}

/// A titled + described dialog as a composable BSN scene (Wave-3 slice-5).
/// Mergeable: the `Dialog` marker triggers the full `#[require]` contract (role
/// `Dialog` + `A11yModal` + the panel box), and the field-patches layer the
/// canonical panel style. The `Children [ … ]` subtree authors the **title**
/// (`A11yRole::Heading`, the label source) and the **body** (`A11yRole::Text`, the
/// description source), both `Pickable::IGNORE` (pick-through — decorative text).
///
/// The dialog's `A11yRelations.labelled_by = [title]` / `described_by = [body]`
/// are wired by `wire_dialog_relations` once the children exist (the title/body
/// entities are unknown at authoring time, so neither the bundle nor the scene
/// can spell them).
///
/// **No open/close/focus-trap** — the live show/hide + trap is C5 (Wave 4); this
/// is the static a11y shape only.
///
/// ```ignore
/// use buiy::prelude::*;
/// world.spawn_scene(bsn! { dialog("Delete?", "This cannot be undone.") });
/// ```
pub fn dialog(title: impl Into<String>, body: impl Into<String>) -> impl Scene {
    let bm = dialog_box_model();
    let bg = dialog_background();
    let border = dialog_border();
    let title = title.into();
    let body = body.into();
    bsn! {
        Dialog
        BoxModel {
            width: { bm.width },
            height: { bm.height },
        }
        Background { color: { bg.color } }
        Border { radius: { border.radius } }
        Children [
            (
                DialogTitle
                Text({ title.clone() })
                FontSize({ DIALOG_TITLE_FONT_SIZE })
                A11yLabel({ title })
                template_value(TextColor::default())
                template_value(Pickable::IGNORE)
            ),
            (
                DialogBody
                Text({ body.clone() })
                FontSize({ DIALOG_BODY_FONT_SIZE })
                A11yLabel({ body })
                template_value(TextColor::default())
                template_value(Pickable::IGNORE)
            ),
        ]
    }
}

/// A labelled tooltip trigger + its tooltip as a composable BSN scene (Wave-3
/// slice-5). Mergeable: the `TooltipTrigger` marker triggers the full `#[require]`
/// contract (the `A11yTooltipHost` capability + a neutral `A11yRole::Generic` +
/// focus + a11y + the trigger box), and the field-patches layer the canonical
/// trigger style. The `Children [ … ]` subtree authors the controlled **tooltip**
/// node (`A11yRole::Tooltip`, `Pickable::IGNORE`), which starts
/// `CssVisibility::Hidden`; the router's generic `ShowTooltip`/`HideTooltip` honor
/// flips its `CssVisibility`.
///
/// The trigger's `A11yRelations.described_by = [tooltip]` is wired by
/// `wire_tooltip_described_by` once the children exist (the tooltip entity is
/// unknown at authoring time). The trigger advertises `{ShowTooltip, HideTooltip,
/// Focus, Blur}` — NO `Click` (the neutral role contributes no activation verb).
///
/// **No placement / auto-show timing** — that is C5 (Wave 4); this is the static
/// a11y shape + the minimal `CssVisibility` show/hide only.
///
/// ```ignore
/// use buiy::prelude::*;
/// world.spawn_scene(bsn! { tooltip_trigger("?", "More info here") });
/// ```
pub fn tooltip_trigger(label: impl Into<String>, tip: impl Into<String>) -> impl Scene {
    let bm = tooltip_trigger_box_model();
    let bg = tooltip_trigger_background();
    let border = tooltip_trigger_border();
    let tip_bm = tooltip_box_model();
    let tip_bg = tooltip_background();
    let label = label.into();
    let tip = tip.into();
    bsn! {
        TooltipTrigger
        BoxModel {
            width: { bm.width },
            height: { bm.height },
        }
        Background { color: { bg.color } }
        Border { radius: { border.radius } }
        A11yLabel({ label })
        Children [
            (
                TooltipNode
                Text({ tip.clone() })
                FontSize({ TOOLTIP_FONT_SIZE })
                A11yLabel({ tip })
                BoxModel {
                    width: { tip_bm.width },
                    height: { tip_bm.height },
                }
                Background { color: { tip_bg.color } }
                template_value(TextColor::default())
                // The tooltip starts hidden; the router's ShowTooltip/HideTooltip
                // honor flips this `CssVisibility`.
                template_value(CssVisibility::Hidden)
                template_value(Pickable::IGNORE)
            ),
        ]
    }
}
