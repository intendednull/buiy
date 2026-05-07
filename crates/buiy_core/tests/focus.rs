use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    focus::{FocusPlugin, Focusable, FocusedEntity, advance_focus_for_test},
};

#[test]
fn tab_cycles_focus_through_focusables() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(FocusPlugin);

    let a = app.world_mut().spawn(Focusable::default()).id();
    let b = app.world_mut().spawn(Focusable::default()).id();
    let c = app.world_mut().spawn(Focusable::default()).id();

    advance_focus_for_test(&mut app, true);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(a));
    advance_focus_for_test(&mut app, true);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(b));
    advance_focus_for_test(&mut app, true);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(c));
    advance_focus_for_test(&mut app, true);
    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(a),
        "wraps to first focusable"
    );

    advance_focus_for_test(&mut app, false);
    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(c),
        "Shift+Tab moves backward"
    );
}
