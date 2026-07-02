//! WCAG 2 contrast linter. See: docs/specs/2026-05-07-buiy-foundation/verification.md
//! (CI gate #9). APCA is on the same code path but advisory; it ships in v0.x
//! per `buiy-theme-tokens-design`.

use bevy::prelude::Color;
use buiy_core::render::color::{ColorToken, ThemeContract, contrast_ratio};
use buiy_core::theme::Theme;

pub const WCAG_AA_NORMAL: f64 = 4.5;
pub const WCAG_AA_LARGE: f64 = 3.0;
pub const WCAG_AA_NON_TEXT: f64 = 3.0;
pub const WCAG_AAA_NORMAL: f64 = 7.0;
pub const WCAG_AAA_LARGE: f64 = 4.5;

#[derive(Debug, Clone, PartialEq)]
pub enum ContrastSeverity {
    Pass,
    /// Reserved for advisory tiers (e.g., near-AA, APCA Lc). Phase 0 doesn't
    /// emit `Warn`; v0.x will use it for ratios in the borderline band when
    /// APCA support lands.
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct ContrastViolation {
    pub bg_token: String,
    pub fg_token: String,
    pub ratio: f64,
    pub required: f64,
    pub severity: ContrastSeverity,
}

/// WCAG 2.1 §1.4.3 contrast ratio: `(L_lighter + 0.05) / (L_darker + 0.05)`.
/// Returns a value in [1, 21]; identical colors yield 1.0, black-on-white yields 21.0.
///
/// Thin f64 wrapper over the single WCAG source of truth,
/// [`buiy_core::render::color::contrast_ratio`] (computed in f32). Keeping the
/// math in one place means this gate (#9) and the render-side focus-ring /
/// token-pair checks can never drift. The argument order is `(fg, bg)` here for
/// linter ergonomics; the underlying ratio is symmetric.
pub fn wcag2_ratio(fg: Color, bg: Color) -> f64 {
    contrast_ratio(fg, bg) as f64
}

pub fn contrast_violations(
    theme: &Theme,
    pairs: &[(ColorToken, ColorToken)],
    required: f64,
) -> Vec<ContrastViolation> {
    let mut out = Vec::new();
    for &(bg_token, fg_token) in pairs {
        // Every typed `ColorToken` resolves to a concrete color (exhaustive
        // match), so there is no missing-token skip anymore — the old
        // `theme.color(&str)` HashMap lookup that could miss is gone.
        let bg = theme.resolve(bg_token);
        let fg = theme.resolve(fg_token);
        let ratio = wcag2_ratio(fg, bg);
        let severity = if ratio < required {
            ContrastSeverity::Fail
        } else {
            ContrastSeverity::Pass
        };
        out.push(ContrastViolation {
            bg_token: bg_token.debug_name(),
            fg_token: fg_token.debug_name(),
            ratio,
            required,
            severity,
        });
    }
    out.into_iter()
        .filter(|v| v.severity == ContrastSeverity::Fail)
        .collect()
}

/// Lint the canonical text-on-surface pairs in any theme at WCAG 2 AA.
/// Returns Ok if all pass.
///
/// Phase 0 walks 3 hand-picked canonical pairs only.
/// TODO(buiy-theme-tokens-design): expand to an exhaustive `Surface*` ×
/// `Text*` cartesian walk now that the typed token enum has replaced the
/// string-keyed `Theme` HashMap.
pub fn lint_theme(theme: &Theme) -> Result<(), Vec<ContrastViolation>> {
    let pairs = [
        (ColorToken::SurfacePrimary, ColorToken::TextPrimary),
        (ColorToken::SurfacePrimary, ColorToken::TextSecondary),
        (ColorToken::SurfaceSecondary, ColorToken::TextPrimary),
    ];
    let v = contrast_violations(theme, &pairs, WCAG_AA_NORMAL);
    if v.is_empty() { Ok(()) } else { Err(v) }
}
