//! WCAG 2 contrast linter. See: docs/specs/2026-05-07-buiy-foundation/verification.md
//! (CI gate #9). APCA is on the same code path but advisory; it ships in v0.x
//! per `buiy-theme-tokens-design`.

use bevy::prelude::Color;
use buiy_core::theme::Theme;

pub const WCAG_AA_NORMAL: f64 = 4.5;
pub const WCAG_AA_LARGE: f64 = 3.0;
pub const WCAG_AA_NON_TEXT: f64 = 3.0;
pub const WCAG_AAA_NORMAL: f64 = 7.0;
pub const WCAG_AAA_LARGE: f64 = 4.5;

#[derive(Debug, Clone, PartialEq)]
pub enum ContrastSeverity {
    Pass,
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

pub fn wcag2_ratio(fg: Color, bg: Color) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(c: Color) -> f64 {
    let lin = c.to_linear();
    let lin_r = lin.red as f64;
    let lin_g = lin.green as f64;
    let lin_b = lin.blue as f64;
    0.2126 * lin_r + 0.7152 * lin_g + 0.0722 * lin_b
}

pub fn contrast_violations(
    theme: &Theme,
    pairs: &[(&str, &str)],
    required: f64,
) -> Vec<ContrastViolation> {
    let mut out = Vec::new();
    for (bg_token, fg_token) in pairs {
        let bg = match theme.color(bg_token) {
            Some(c) => c,
            None => continue,
        };
        let fg = match theme.color(fg_token) {
            Some(c) => c,
            None => continue,
        };
        let ratio = wcag2_ratio(fg, bg);
        let severity = if ratio < required {
            ContrastSeverity::Fail
        } else {
            ContrastSeverity::Pass
        };
        out.push(ContrastViolation {
            bg_token: bg_token.to_string(),
            fg_token: fg_token.to_string(),
            ratio,
            required,
            severity,
        });
    }
    out.into_iter()
        .filter(|v| v.severity == ContrastSeverity::Fail)
        .collect()
}

/// Lint the canonical text-on-surface pairs in any theme. Returns Ok if all pass.
pub fn lint_theme(theme: &Theme) -> Result<(), Vec<ContrastViolation>> {
    let pairs = [
        ("color.surface.primary", "color.text.primary"),
        ("color.surface.primary", "color.text.secondary"),
        ("color.surface.secondary", "color.text.primary"),
    ];
    let v = contrast_violations(theme, &pairs, WCAG_AA_NORMAL);
    if v.is_empty() { Ok(()) } else { Err(v) }
}
