//! Gate-#11 static analyzers over the default widget catalog (build/test-time,
//! no GPU). Check (a): under the forced-colors theme every paint color is a
//! **system-color token reference** that resolves inside the system-color key
//! set (§ 3.1). Check (b): every state-bearing widget has a non-`BoxShadow`
//! inter-state cue (§ 3.2, Task 8).
//!
//! The analyzer is possible only because paint color is uniformly a token edge
//! in the component model — there is no second, literal-color path to miss.
//!
//! `CatalogPaint` is the cross-phase seam: until `Background`/`Border`/
//! `Outline`/`BoxShadow` (component-model phase) exist, the catalog is
//! enumerated as plain descriptors; when those components land, the descriptor
//! is built from them with no change to the analyzer or its tests.

use crate::render::color::{ColorToken, MISSING_TOKEN_FALLBACK, SystemColorKeyword, resolve_token};
use crate::theme::Theme;

/// One widget × state's emitted paint, as token references. Built from the
/// default catalog. `has_shadow_only_state_delta` records whether this state
/// differs from the resting state *only* in `BoxShadow` (check (b), Task 8).
#[derive(Clone, Debug)]
pub struct CatalogPaint {
    pub widget: &'static str,
    pub state: &'static str,
    pub background: ColorToken,
    pub border: ColorToken,
    pub outline: ColorToken,
    pub has_shadow_only_state_delta: bool,
}

/// A gate-#11 violation.
#[derive(Clone, Debug, PartialEq)]
pub enum ForcedColorsViolation {
    /// A paint token resolved outside the system-color set under forced-colors
    /// (it hit the magenta sentinel — an absent/brand token).
    NonSystemColor {
        widget: &'static str,
        state: &'static str,
        field: &'static str,
    },
    /// (Check (b), Task 8) the only inter-state difference is `BoxShadow`.
    ShadowOnlyAffordance {
        widget: &'static str,
        state: &'static str,
    },
}

/// Check (a): under `theme` (the forced-colors variant), assert every non-
/// `Transparent` paint token resolves to a real system color — i.e. does not
/// fall through to the magenta sentinel. Returns the violations (empty == pass).
pub fn analyze_forced_colors(
    catalog: &[CatalogPaint],
    theme: &Theme,
) -> Vec<ForcedColorsViolation> {
    let mut out = Vec::new();
    for paint in catalog {
        for (field, token) in [
            ("background", &paint.background),
            ("border", &paint.border),
            ("outline", &paint.outline),
        ] {
            if matches!(token, ColorToken::Transparent) {
                continue;
            }
            if resolve_token(token, theme) == MISSING_TOKEN_FALLBACK {
                out.push(ForcedColorsViolation::NonSystemColor {
                    widget: paint.widget,
                    state: paint.state,
                    field,
                });
            }
        }
    }
    out
}

/// The system-color key set, for callers that want the allow-list directly.
pub fn system_color_tokens() -> [&'static str; 16] {
    SystemColorKeyword::ALL.map(|kw| kw.token())
}

/// Check (b): assert no widget state conveys its affordance with a shadow
/// alone. A `CatalogPaint` whose `has_shadow_only_state_delta` is set fails —
/// once `BoxShadow` is suppressed under forced-colors (§ 3.3) such a state is
/// indistinguishable from resting. Because `Background`/`Border`/`Outline` are
/// four distinct components from `BoxShadow`, "has a non-shadow cue?" is a
/// structural query answerable without rendering (§ 3.2). The visual half is
/// the forced-colors golden (gate #11(b), GPU — golden.rs).
pub fn analyze_shadow_only(catalog: &[CatalogPaint]) -> Vec<ForcedColorsViolation> {
    catalog
        .iter()
        .filter(|p| p.has_shadow_only_state_delta)
        .map(|p| ForcedColorsViolation::ShadowOnlyAffordance {
            widget: p.widget,
            state: p.state,
        })
        .collect()
}
