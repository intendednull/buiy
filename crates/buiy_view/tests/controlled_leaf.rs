//! #16 regression: a `buiy_view`-authored checkbox is stamped `ControlledLeaf`.
//!
//! `ControlledLeaf` (buiy_core::mvu) opts a checkbox OUT of the built-in press-to-toggle leaf
//! (`buiy_widgets::advance_toggle_on_press` filters `Without<ControlledLeaf>`), so the view's
//! model route — not the leaf — is the sole SOURCE of the `A11yToggled` fold (no double-fold,
//! design §3 #16). The buiy_widgets side (the filter) is covered by
//! `buiy_widgets/tests/checkbox.rs`; THIS asserts the other half — that `buiy_view` actually
//! stamps the marker on the checkbox it spawns (a regression would silently reinstate the
//! double-fold).

mod common;

use bevy::prelude::*;
use buiy_core::mvu::{Cmd, ControlledLeaf, Model};
use buiy_view::{BuiyViewAppExt, Element, Kind, checkbox, column, find_kind};

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct CbApp {
    on: bool,
}
impl Model for CbApp {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Set(bool),
}

fn update(s: &mut CbApp, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Set(v) => s.on = v,
    }
    Cmd::none()
}

fn view(s: &CbApp) -> Element<Msg> {
    column![checkbox(s.on).on_toggle(Msg::Set)]
}

#[test]
fn view_checkbox_is_stamped_controlled_leaf() {
    let mut app = common::logic_app();
    app.ui(CbApp::default(), update, view);
    common::settle(&mut app);

    let cb = find_kind(app.world_mut(), Kind::Checkbox).expect("checkbox realized");
    assert!(
        app.world().get::<ControlledLeaf>(cb).is_some(),
        "LOAD-BEARING (#16): a buiy_view checkbox is stamped ControlledLeaf, so its model route \
         (not the built-in press-to-toggle leaf) owns the A11yToggled fold — no double-fold"
    );
}
