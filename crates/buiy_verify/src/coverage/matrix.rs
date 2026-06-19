//! The global axes and their Cartesian product (coverage.md § "The Matrix").
//!
//! A [`Matrix`] declares the four global axes — theme × viewport ×
//! forced-colors × dpr — and [`cells`](Matrix::cells) takes their Cartesian
//! product into one [`Cell`] per combination. The full corpus is
//! `Matrix × Fixture`; a [`Cell`] is half of a
//! [`CoverageKey`](super::key::CoverageKey).
//!
//! Iteration order is **stable** (axis-declaration order: theme, then viewport,
//! then forced-colors, then dpr) so snapshot/golden stems are deterministic
//! across runs.

use buiy_core::render::golden::Dpr;
use buiy_core::theme::{Theme, default_light_theme, forced_colors_theme};

/// CI ceiling on cells **per fixture**. Tripping it is a planned
/// storage-migration trigger (report Open Q #6), forced through the
/// `verify_cell_count_under_ceiling` self-test — never a silent surprise. The
/// `ci_default` product is 24 (2 themes × 3 viewports × 2 fc × 2 dpr); the
/// ceiling leaves deliberate headroom for one more axis value without a budget
/// review, but widening past it must be a conscious, documented decision (the
/// metric's fuzz-budget discipline, applied to combinatorics).
pub const CELL_CEILING_PER_FIXTURE: usize = 32;

/// The theme axis: which [`Theme`] a cell installs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeAxis {
    /// The default light theme ([`default_light_theme`]).
    Light,
    /// The forced-colors (system-color) theme ([`forced_colors_theme`]).
    ForcedColors,
}

impl ThemeAxis {
    /// Construct the [`Theme`] this axis selects.
    pub fn build(self) -> Theme {
        match self {
            Self::Light => default_light_theme(),
            Self::ForcedColors => forced_colors_theme(),
        }
    }

    /// The stable lower-kebab key — the `theme` field of a
    /// [`CoverageKey`](super::key::CoverageKey) stem.
    pub fn key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::ForcedColors => "forced",
        }
    }
}

/// A named logical viewport `(w, h)`. The `key` is the stem component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    /// Logical width in CSS px.
    pub w: u32,
    /// Logical height in CSS px.
    pub h: u32,
    /// Stable lower-kebab key (`phone` | `tablet` | `desktop`).
    pub key: &'static str,
}

/// The four global axes. The product `Matrix × Fixture` is the full corpus.
#[derive(Clone, Debug)]
pub struct Matrix {
    /// Theme axis — light + forced-colors (dark when it lands).
    pub themes: Vec<ThemeAxis>,
    /// Logical viewports — phone, tablet, desktop.
    pub viewports: Vec<Viewport>,
    /// Forced-colors **mode** axis: `false`, `true`. Each value gets its own
    /// baseline (Chromatic modes).
    pub forced_colors: Vec<bool>,
    /// DPR **mode** axis as canonical milliscale: `Dpr::X1`, `Dpr::X2`.
    pub dprs: Vec<Dpr>,
}

impl Matrix {
    /// The CI default: a conservative product (2 themes × 3 viewports × 2 fc ×
    /// 2 dpr = 24 cells/fixture). Widen any axis only with a documented reason,
    /// never silently — the `verify_cell_count_under_ceiling` self-test enforces
    /// [`CELL_CEILING_PER_FIXTURE`].
    pub fn ci_default() -> Self {
        Self {
            themes: vec![ThemeAxis::Light, ThemeAxis::ForcedColors],
            viewports: vec![
                Viewport {
                    w: 360,
                    h: 640,
                    key: "phone",
                },
                Viewport {
                    w: 768,
                    h: 1024,
                    key: "tablet",
                },
                Viewport {
                    w: 1280,
                    h: 800,
                    key: "desktop",
                },
            ],
            forced_colors: vec![false, true],
            dprs: vec![Dpr::X1, Dpr::X2],
        }
    }

    /// The Cartesian product → one [`Cell`] per combination, in stable
    /// axis-declaration order (theme, viewport, forced-colors, dpr). Stable
    /// order is what makes the derived stems deterministic across runs.
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        self.themes.iter().flat_map(move |&theme| {
            self.viewports.iter().flat_map(move |&viewport| {
                self.forced_colors.iter().flat_map(move |&forced_colors| {
                    self.dprs.iter().map(move |&dpr| Cell {
                        theme,
                        viewport,
                        forced_colors,
                        dpr,
                    })
                })
            })
        })
    }

    /// The number of cells one fixture enrolls into — the product of the axis
    /// lengths. Adding a fixture grows the total corpus by exactly this many
    /// (the `auto-enroll by construction` property the self-test proves).
    pub fn cells_per_fixture(&self) -> usize {
        self.themes.len() * self.viewports.len() * self.forced_colors.len() * self.dprs.len()
    }

    /// The CPU **snapshot** view of [`ci_default`](Self::ci_default): the same
    /// matrix with the **DPR axis collapsed to a single value** (`Dpr::X1`).
    ///
    /// DPR is inert at the CPU tiers. `ResolvedLayout` and the display list are
    /// in **logical** px (DPR lives only in the GPU render-view uniform —
    /// `render/extract.rs` `ExtractedNodes.scale_factor`), so the layout /
    /// display-list dump a CPU snapshot tier baselines is byte-identical at every
    /// DPR. Driving both `Dpr::X1` and `Dpr::X2` at the snapshot tiers only
    /// doubled the `.snap` count with identical content (audit 2026-06-18,
    /// "Investigated & dismissed"). The real invariant the duplicate baselines
    /// were implicitly encoding — *DPR does not affect CPU output* — is asserted
    /// directly and once by the `cpu_snapshots_are_dpr_invariant` property test;
    /// this collapse removes the redundant baselines.
    ///
    /// The **GPU golden tier keeps [`ci_default`](Self::ci_default)** (both DPR
    /// values) — DPR genuinely changes the rasterized output there. So does the
    /// CPU **invariant** tier, whose finiteness/extent predicates are cheap to
    /// run per DPR and guard a degenerate box at an extreme DPR (it baselines no
    /// `.snap`, so it has no orphan-snapshot cost).
    pub fn cpu_snapshots() -> Self {
        Self {
            dprs: vec![Dpr::X1],
            ..Self::ci_default()
        }
    }
}

/// One enrolled combination — half of a
/// [`CoverageKey`](super::key::CoverageKey) (the other half is the
/// [`Fixture`](super::fixture::Fixture)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub theme: ThemeAxis,
    pub viewport: Viewport,
    pub forced_colors: bool,
    pub dpr: Dpr,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_default_product_is_twenty_four() {
        let m = Matrix::ci_default();
        assert_eq!(m.cells_per_fixture(), 24);
        assert_eq!(m.cells().count(), 24);
    }

    #[test]
    fn cells_per_fixture_under_ceiling() {
        assert!(Matrix::ci_default().cells_per_fixture() <= CELL_CEILING_PER_FIXTURE);
    }

    #[test]
    fn cpu_snapshots_collapses_dpr_to_one() {
        // The CPU snapshot view drops the DPR axis to a single value, halving the
        // per-fixture cell count (24 → 12). Every other axis is unchanged, so the
        // sole survivor is `Dpr::X1`.
        let cpu = Matrix::cpu_snapshots();
        assert_eq!(cpu.dprs, vec![Dpr::X1], "DPR axis collapsed to X1");
        assert_eq!(cpu.themes, Matrix::ci_default().themes);
        assert_eq!(cpu.viewports, Matrix::ci_default().viewports);
        assert_eq!(cpu.forced_colors, Matrix::ci_default().forced_colors);
        assert_eq!(cpu.cells_per_fixture(), 12);
        assert_eq!(cpu.cells().count(), 12);
        // No cell carries a DPR other than X1.
        assert!(cpu.cells().all(|c| c.dpr == Dpr::X1));
    }

    #[test]
    fn cells_iterate_in_stable_axis_order() {
        // First two cells differ only in the innermost axis (dpr), proving the
        // declaration-order nesting (theme outer … dpr inner).
        let m = Matrix::ci_default();
        let cells: Vec<Cell> = m.cells().take(2).collect();
        assert_eq!(cells[0].theme, cells[1].theme);
        assert_eq!(cells[0].viewport, cells[1].viewport);
        assert_eq!(cells[0].forced_colors, cells[1].forced_colors);
        assert_ne!(cells[0].dpr, cells[1].dpr);
    }

    #[test]
    fn theme_axis_builds_distinct_themes() {
        // Light has the brand surface token; forced-colors has the system map.
        assert!(
            ThemeAxis::Light
                .build()
                .color("color.surface.primary")
                .is_some()
        );
        assert!(ThemeAxis::ForcedColors.build().color("Canvas").is_some());
        assert_eq!(ThemeAxis::Light.key(), "light");
        assert_eq!(ThemeAxis::ForcedColors.key(), "forced");
    }
}
