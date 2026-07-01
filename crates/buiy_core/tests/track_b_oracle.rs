//! TRACK B WAVE 1 — resolved-color oracle (TEMPORARY; delete before PR).
//! Dumps every (theme, token) -> Color through the typed `ThemeContract`, the
//! successor to the pre-migration HashMap dump. DARK is the pixel-critical
//! (gallery) palette and is asserted byte-identical elsewhere (color.rs /
//! theme.rs unit tests); LIGHT's family-derived defaults + FORCED's role map are
//! expected-to-change (documented in the plan).

use buiy_core::render::color::{ColorToken, ThemeContract};
use buiy_core::theme::{Theme, default_dark_theme, default_light_theme, forced_colors_theme};

/// The full closed semantic vocabulary, paired with the dotted name it
/// succeeds (for a legible dump). The four non-semantic kinds
/// (`Transparent`/`CurrentColor`/`SystemColor`/`Custom`) are exercised
/// separately below.
const SEMANTIC_TOKENS: &[(ColorToken, &str)] = &[
    (ColorToken::SurfaceApp, "color.surface.app"),
    (ColorToken::SurfacePrimary, "color.surface.primary"),
    (ColorToken::SurfaceSecondary, "color.surface.secondary"),
    (ColorToken::SurfaceCard, "color.surface.card"),
    (ColorToken::SurfaceRaised, "color.surface.raised"),
    (ColorToken::SurfaceRaisedAlt, "color.surface.raised-alt"),
    (ColorToken::SurfaceInset, "color.surface.inset"),
    (ColorToken::SurfaceChrome, "color.surface.chrome"),
    (
        ColorToken::SurfaceChromeTranslucent,
        "color.surface.chrome-translucent",
    ),
    (ColorToken::SurfaceDanger, "color.surface.danger"),
    (ColorToken::SurfaceDangerSoft, "color.surface.danger-soft"),
    (
        ColorToken::SurfaceDangerStrong,
        "color.surface.danger-strong",
    ),
    (ColorToken::TextPrimary, "color.text.primary"),
    (ColorToken::TextSecondary, "color.text.secondary"),
    (ColorToken::TextMuted, "color.text.muted"),
    (ColorToken::TextDim, "color.text.dim"),
    (ColorToken::TextDimmer, "color.text.dimmer"),
    (ColorToken::TextFaint, "color.text.faint"),
    (ColorToken::TextBright, "color.text.bright"),
    (ColorToken::TextPlaceholder, "color.text.placeholder"),
    (ColorToken::TextDanger, "color.text.danger"),
    (ColorToken::TextDangerDim, "color.text.danger-dim"),
    (ColorToken::TextOnAccent, "color.text.on-accent"),
    (ColorToken::BorderDefault, "color.border.default"),
    (ColorToken::BorderSubtle, "color.border.subtle"),
    (ColorToken::BorderSubtle2, "color.border.subtle-2"),
    (ColorToken::BorderMuted, "color.border.muted"),
    (ColorToken::BorderStrong, "color.border.strong"),
    (ColorToken::BorderStrong2, "color.border.strong-2"),
    (ColorToken::BorderDanger, "color.border.danger"),
    (ColorToken::Accent, "color.accent"),
    (ColorToken::AccentLighter, "color.accent.lighter"),
    (ColorToken::AccentSoft, "color.accent.soft"),
    (ColorToken::AccentGlow, "color.accent.glow"),
    (ColorToken::AccentBlue, "color.accent.blue"),
    (ColorToken::AccentGreen, "color.accent.green"),
    (ColorToken::AccentViolet, "color.accent.violet"),
    (ColorToken::AccentCoral, "color.accent.coral"),
    (ColorToken::StatusOk, "color.status.ok"),
    (ColorToken::StatusWarn, "color.status.warn"),
    (ColorToken::StatusError, "color.status.error"),
    (ColorToken::ShadowCard, "color.shadow.card"),
    (ColorToken::ShadowMenu, "color.shadow.menu"),
    (ColorToken::ShadowModal, "color.shadow.modal"),
    (ColorToken::ShadowSliderThumb, "color.shadow.slider-thumb"),
    (ColorToken::ShadowSwitchThumb, "color.shadow.switch-thumb"),
    (ColorToken::ShadowDangerButton, "color.shadow.danger-button"),
    (ColorToken::SelectionBg, "color.selection.bg"),
    (ColorToken::SelectionFg, "color.selection.fg"),
    (ColorToken::ScrollbarThumb, "color.scrollbar.thumb"),
    (
        ColorToken::ScrollbarThumbHover,
        "color.scrollbar.thumb-hover",
    ),
    (ColorToken::ScrollbarTrack, "color.scrollbar.track"),
    (ColorToken::FocusRing, "color.focus.ring"),
    (ColorToken::Scrim, "color.scrim"),
    (ColorToken::White, "color.misc.white"),
    (ColorToken::DotBg, "color.misc.dot-bg"),
];

fn dump(name: &str, theme: &Theme) {
    println!(
        "--- {name} theme: {} semantic tokens ---",
        SEMANTIC_TOKENS.len()
    );
    let mut lines: Vec<String> = SEMANTIC_TOKENS
        .iter()
        .map(|(token, dotted)| {
            let s = bevy::color::Srgba::from(theme.resolve(*token));
            format!(
                "{name}\t{dotted}\tsrgba({:.4},{:.4},{:.4},{:.4})",
                s.red, s.green, s.blue, s.alpha
            )
        })
        .collect();
    lines.sort();
    for l in lines {
        println!("{l}");
    }
}

#[test]
fn track_b_color_oracle() {
    dump("light", &default_light_theme());
    dump("dark", &default_dark_theme());
    dump("forced", &forced_colors_theme());
    // The dark palette is the byte-identical parity target: its 56 semantic
    // tokens must all resolve to a concrete color (they do by exhaustiveness).
    assert_eq!(SEMANTIC_TOKENS.len(), 56);
}
