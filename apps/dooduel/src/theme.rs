//! Dooduel's visual theme (W3 + W6): the protokit color ladder, the hand-drawn
//! font pairing, the purple accent, and — new in W6 — the **light/dark palette**
//! + the sketchy drop-shadow tokens, bundled as [`DooduelThemePlugin`].
//!
//! **Colors (W6 dark palette).** Buiy's `Color` facade names only five roles and
//! the default theme's non-accent tokens are completeness stubs (both journaled),
//! so every non-accent design color is pinned here as an exact literal. The tokens
//! that DIFFER between light and dark (the protokit `SURFACES_DARK` ladder) live on
//! a [`Palette`] the view threads (`Palette::LIGHT` / `Palette::DARK`, selected by
//! the model's [`ThemePref`]); the theme-invariant ones (white, the always-dark
//! top bar, the scrim) stay module consts. The **accent** stays semantic
//! (`Color::Accent`, resolved through the `Theme` resource) so it re-themes: light
//! `#7C4FE0` ↔ dark `#A78BFA`, swapped by [`sync_theme_resource`] from the model.
//!
//! **Fonts.** Caveat (display) + Shantell Sans (body), registered by bytes.

use std::sync::Arc;

use bevy::prelude::*;
use buiy_core::text::{FontFaceDescriptors, FontRegistry};
use buiy_core::theme::{Theme, default_dark_theme, default_light_theme};
use buiy_view::Color;

/// Which palette the app wears. Model-owned + replayable (a `SetTheme` folds
/// through the funnel); persisted (`storage`). The design's `theme` state
/// (`'light'` / `'dark'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum ThemePref {
    #[default]
    Light,
    Dark,
}

impl ThemePref {
    /// The opposite theme (the toggle target).
    pub fn toggled(self) -> Self {
        match self {
            ThemePref::Light => ThemePref::Dark,
            ThemePref::Dark => ThemePref::Light,
        }
    }

    /// This theme's color ladder.
    pub fn palette(self) -> Palette {
        match self {
            ThemePref::Light => Palette::LIGHT,
            ThemePref::Dark => Palette::DARK,
        }
    }

    /// The stored string form (design's localStorage `dooduel-proto-theme`).
    pub fn as_str(self) -> &'static str {
        match self {
            ThemePref::Light => "light",
            ThemePref::Dark => "dark",
        }
    }

    /// Parse the stored string form (unknown ⇒ light).
    pub fn from_stored(s: &str) -> Self {
        match s {
            "dark" => ThemePref::Dark,
            _ => ThemePref::Light,
        }
    }
}

/// The theme-VARYING color ladder (the protokit `:root` light values ↔ the
/// `SURFACES_DARK` dark values). Threaded through the view so every surface swaps
/// on a `SetTheme`. Copy + `const`, so `Palette::LIGHT` / `Palette::DARK` are the
/// pinned design ladders (the W3 way, now with a dark twin). The accent + its tint
/// come from the semantic `Color::Accent` path (re-themed via the `Theme`
/// resource); everything else is exact sRGB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// App background behind the cards (`--canvas`).
    pub canvas: Color,
    /// Card / panel fill (`--surface`).
    pub surface: Color,
    /// Subtle inset fill (`--surface-2`) — the room-code box, chat bubbles.
    pub surface_2: Color,
    /// Primary ink (`--ink`) — headings AND the sketchy borders. In dark this is
    /// near-white, so ink outlines become light-on-dark (design-faithful).
    pub ink: Color,
    /// Secondary ink (`--ink-2`) — body copy.
    pub ink_2: Color,
    /// Tertiary ink (`--muted`) — captions, eyebrow-secondary, quiet labels.
    pub muted: Color,
    /// Border hairline (`--hair`) — a faint divider / neutral badge fill.
    pub hair: Color,
    /// Positive tint (`--pos-tint`) — the "Joined"/"Guessed" badge fill.
    pub pos_tint: Color,
    /// Accent tint (`--accent-tint`) — soft accent fills (selected pills/tabs).
    pub accent_tint: Color,
    /// Danger tint (`--danger-tint`) — the urgent timer pill fill.
    pub danger_tint: Color,
    /// The `--sh-*` FIRST term: a hard (blur-0) low-alpha offset. Ink-toned in
    /// light, black in dark (the design's shadow ladders).
    pub shadow_hard: Color,
    /// The `--sh-*` SECOND term: a soft ambient blur.
    pub shadow_soft: Color,
}

impl Palette {
    /// The protokit LIGHT ladder (`tokens/colors.css :root`), exact sRGB.
    pub const LIGHT: Palette = Palette {
        canvas: Color::rgb(0xf4, 0xf5, 0xf8),
        surface: Color::rgb(0xff, 0xff, 0xff),
        surface_2: Color::rgb(0xef, 0xf1, 0xf5),
        ink: Color::rgb(0x14, 0x16, 0x1b),
        ink_2: Color::rgb(0x44, 0x4a, 0x55),
        muted: Color::rgb(0x6f, 0x77, 0x83),
        hair: Color::rgb(0xe3, 0xe6, 0xec),
        pos_tint: Color::rgb(0xdc, 0xef, 0xe3),
        accent_tint: Color::rgb(0xec, 0xe3, 0xfb),
        danger_tint: Color::rgb(0xfb, 0xe4, 0xe2),
        // --sh-md light: 0 4px 0 rgba(20,22,27,.12), 0 3px 12px rgba(20,22,27,.09).
        shadow_hard: Color::Custom(0x14, 0x16, 0x1b, 31), // .12
        shadow_soft: Color::Custom(0x14, 0x16, 0x1b, 23), // .09
    };

    /// The protokit `SURFACES_DARK` ladder (a softer charcoal — the design's own
    /// note: the DS default dark crushes to near-black; this stays clearly "dark
    /// mode"). The pos/accent/danger TINTS are darkened here (the design leaves
    /// them light, which reads wrong on a dark card — the one faithful deviation,
    /// journaled) so the badges sit on dark surfaces.
    pub const DARK: Palette = Palette {
        canvas: Color::rgb(0x1b, 0x1e, 0x25),
        surface: Color::rgb(0x26, 0x2a, 0x33),
        surface_2: Color::rgb(0x20, 0x23, 0x2b),
        ink: Color::rgb(0xee, 0xf0, 0xf4),
        ink_2: Color::rgb(0xc6, 0xca, 0xd4),
        muted: Color::rgb(0x95, 0x9a, 0xa5),
        hair: Color::rgb(0x3c, 0x41, 0x4c),
        pos_tint: Color::rgb(0x1c, 0x3a, 0x2e),
        accent_tint: Color::rgb(0x2f, 0x2a, 0x48),
        danger_tint: Color::rgb(0x3a, 0x24, 0x22),
        // --sh-md dark: 0 4px 0 rgba(0,0,0,.4), 0 3px 12px rgba(0,0,0,.34).
        shadow_hard: Color::Custom(0, 0, 0, 102), // .40
        shadow_soft: Color::Custom(0, 0, 0, 87),  // .34
    };
}

// --- Theme-INVARIANT design colors (same in light + dark) --------------------

/// On-accent text / white chip fills (`#fff`) — invariant.
pub const WHITE: Color = Color::rgb(0xff, 0xff, 0xff);
/// Positive status (`--pos`) — correct-guess ticks, badge text. Invariant.
pub const POS: Color = Color::rgb(0x1c, 0x8a, 0x52);
/// Fully transparent — a chromeless (ghost) button fill. Invariant.
pub const CLEAR: Color = Color::Custom(0, 0, 0, 0);

// --- The always-dark in-game chrome (dark in BOTH themes) --------------------
/// The always-dark hero bar (`--ink-panel`) — the in-game top bar + the floating
/// theme toggle track.
pub const INK_PANEL: Color = Color::rgb(0x16, 0x18, 0x1e);
/// Near-white text/icons on the dark top bar (`--ink-panel-on`).
pub const INK_PANEL_ON: Color = Color::rgb(0xf2, 0xf4, 0xf7);
/// A faint white divider/label tone on the dark top bar.
pub const PANEL_MUTED: Color = Color::rgb(0x9a, 0xa2, 0xb0);
/// Danger red (`--danger`) — the urgent (<10s) timer ring + countdown. Invariant.
pub const DANGER: Color = Color::rgb(0xcf, 0x3a, 0x36);
/// A scrim behind a centered dialog overlay (dark, translucent). Invariant.
pub const SCRIM: Color = Color::Custom(0x14, 0x16, 0x1b, 0x9c);

// --- The sketchy character: hand-drawn wobble corner radii (W6) --------------
// Per-corner circular rx `(top_left, top_right, bottom_right, bottom_left)`. The
// design uses per-axis wobble (e.g. `26 32 24 30 / 30 22 32 24`); the render band
// rounds per corner with `rx` (per-axis elliptical is not rendered — journaled),
// so these are the design's `rx` values.

/// The pre-game card wobble (design Home/Lobby `border-radius:26 32 24 30 / …`).
pub const WOBBLE_CARD: [f32; 4] = [26.0, 32.0, 24.0, 30.0];
/// The in-game panel wobble (design `border-radius:18 22 18 22`).
pub const WOBBLE_PANEL: [f32; 4] = [18.0, 22.0, 18.0, 22.0];

/// The display face (headlines / scores / timers / buttons) — design `--font-display`.
pub const FONT_DISPLAY: &str = "Caveat";
/// The body face — design `--font-sans`.
pub const FONT_BODY: &str = "Shantell Sans";

/// The Dooduel accent for a given theme (design `ACCENT_LIGHT.accent = #7C4FE0`,
/// `ACCENT_DARK.accent = #A78BFA`). Set into `Theme::accent` so `Color::Accent`
/// resolves to it (and the live accent ramp derives from it).
fn accent_for(theme: ThemePref) -> bevy::color::Color {
    match theme {
        ThemePref::Light => bevy::color::Color::srgb_u8(0x7c, 0x4f, 0xe0),
        ThemePref::Dark => bevy::color::Color::srgb_u8(0xa7, 0x8b, 0xfa),
    }
}

/// Mirror the model's [`ThemePref`] onto the `Theme` RESOURCE each frame it
/// changes — swaps the base light/dark ladder (so the widget-default surfaces the
/// app does NOT pin, e.g. the `text_input`, re-theme) and pins the protokit accent
/// on top. A model-observing side effect (like the paint / confetti syncs): it
/// only READS the model, never enqueues, and a `Local` latch reinserts the
/// resource only on a real change (no per-frame churn). `Option` so it no-ops in a
/// harness without a `Theme` (the GPU-free probe tests).
fn sync_theme_resource(
    model: Option<Single<&crate::Dooduel>>,
    theme: Option<ResMut<Theme>>,
    mut last: Local<Option<ThemePref>>,
) {
    let (Some(model), Some(mut theme)) = (model, theme) else {
        return;
    };
    let want = model.theme;
    if *last == Some(want) {
        return;
    }
    let mut base = match want {
        ThemePref::Light => default_light_theme(),
        ThemePref::Dark => default_dark_theme(),
    };
    base.accent = accent_for(want);
    *theme = base;
    *last = Some(want);
}

/// Register Caveat + Shantell Sans by bytes (baked into the binary).
fn register_fonts(mut registry: ResMut<FontRegistry>) {
    static CAVEAT: &[u8] = include_bytes!("../assets/fonts/Caveat.ttf");
    static SHANTELL: &[u8] = include_bytes!("../assets/fonts/ShantellSans.ttf");
    registry.register_bytes(
        FONT_DISPLAY,
        Arc::new(CAVEAT.to_vec()),
        FontFaceDescriptors::default(),
    );
    registry.register_bytes(
        FONT_BODY,
        Arc::new(SHANTELL.to_vec()),
        FontFaceDescriptors::default(),
    );
}

/// Installs Dooduel's theme: the font pairing + the model-driven light/dark
/// `Theme`-resource sync (accent + base ladder). Kept OUT of `dooduel::install`
/// (the pure MVU wiring) so the GPU-free probe tests — which carry neither a
/// `Theme` nor a `FontRegistry` — don't need it; the windowed bin + the GPU
/// capture bins add it.
pub struct DooduelThemePlugin;

impl Plugin for DooduelThemePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, register_fonts);
        app.add_systems(Update, sync_theme_resource);
    }
}
