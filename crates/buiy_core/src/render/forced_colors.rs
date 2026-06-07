//! Forced-colors (OS high-contrast) selection — a **main-world** Theme swap,
//! run before extract. When `UserPreferences.forced_colors` flips, the active
//! `Theme` becomes the system-color variant (the stub `forced_colors_theme()`);
//! when it clears, the theme captured before the flip is restored. Because the
//! swap mutates `Res<Theme>`, it rides the existing `Theme::is_changed()`
//! re-resolve edge (color-and-forced-colors.md § 2.3 / § 3.1) — there is **no**
//! separate forced-colors color path in extract. The one direct
//! `UserPreferences.forced_colors` read in extract is the BoxShadow draw-skip
//! (§ 3.3), owned by the component-model / compositor phase, not here.

use crate::theme::{Theme, UserPreferences, forced_colors_theme};
use bevy::prelude::*;

/// Remembers the theme that was active **before** forced-colors was applied,
/// so clearing `forced_colors` restores it. `None` while forced-colors is off
/// and no swap has happened. Render-relevant main-world resource.
#[derive(Resource, Default)]
pub struct PrePreferenceTheme(pub Option<Theme>);

/// Main-world system: keep the active `Theme` in sync with
/// `UserPreferences.forced_colors`. Idempotent — only touches `Theme` on the
/// frame the preference actually transitions, so it does not spuriously mark
/// `Theme` changed every frame (which would force a full re-resolve in extract
/// — § 2.3). Scheduled in `BuiySet::Style`, before `BuiySet::Render` (Task 5).
///
/// `UserPreferences` and `Theme` are taken as `Option` so `BuiyRenderPlugin`
/// stays self-sufficient: an app that adds the render plugin without the theme
/// stack (no `CorePlugin`/`ThemePlugin`) has nothing to swap and this system
/// no-ops rather than panicking on a missing resource.
pub fn apply_forced_colors_theme(
    prefs: Option<Res<UserPreferences>>,
    theme: Option<ResMut<Theme>>,
    mut saved: ResMut<PrePreferenceTheme>,
) {
    let (Some(prefs), Some(mut theme)) = (prefs, theme) else {
        return;
    };
    if !prefs.is_changed() {
        return;
    }
    match (prefs.forced_colors, saved.0.is_some()) {
        // Entering forced-colors: save current, swap in the system-color theme.
        (true, false) => {
            saved.0 = Some(theme.clone());
            *theme = forced_colors_theme();
        }
        // Leaving forced-colors: restore the saved theme.
        (false, true) => {
            if let Some(prev) = saved.0.take() {
                *theme = prev;
            }
        }
        // Already in the requested state (e.g. a different preference changed):
        // leave Theme untouched so it is not re-marked changed.
        _ => {}
    }
}
