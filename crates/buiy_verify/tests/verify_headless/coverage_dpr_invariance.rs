//! DPR-invariance property for the CPU snapshot tiers (audit 2026-06-18,
//! "Investigated & dismissed" → T3.6). Pure-CPU, headless.
//!
//! **DPR is inert at the CPU tiers.** `ResolvedLayout` and the display list are
//! in LOGICAL px — DPR lives only in the GPU render-view uniform — so a CPU
//! layout / display-list dump is byte-identical at every DPR. The snapshot tiers
//! therefore drive a SINGLE DPR ([`Matrix::cpu_snapshots`]); previously they
//! drove both `X1` and `X2`, doubling the `.snap` count with identical content
//! (24 of 48 button CPU snapshots were exact dpr1==dpr2 duplicates).
//! `cpu_snapshots_are_dpr_invariant` asserts that real invariant DIRECTLY and
//! once — the property the duplicate baselines were implicitly (and wastefully)
//! encoding — so collapsing the DPR axis loses no coverage. (The GPU golden tier
//! keeps both DPRs: DPR genuinely changes the rasterized output there.)
//!
//! (The former Part-2 "no CPU snapshot baselines the missing-token magenta
//! sentinel" guard was retired by Track B: `ColorToken` is now a closed enum
//! resolved through an exhaustive match, so a missing/typo'd token is a compile
//! error — there is no runtime magenta sentinel left to baseline.)

use bevy::prelude::*;
use buiy_core::render::golden::Dpr;
use buiy_verify::coverage::{Cell, Matrix, build_app, sorted_catalog};
use buiy_verify::snapshot::{
    NameLookup, display_list_dump, extract_nodes_from_world, layout_dump,
};

/// Build one (fixture, cell), run a single `update()`, and return the two CPU
/// snapshot artifacts the tiers baseline: the Tier-1 layout dump and the Tier-2
/// display-list dump. This is the exact pair `coverage_layout.rs` /
/// `coverage_display_list.rs` record, so comparing them across DPR observes the
/// real baselined artifact (not a parallel re-implementation).
fn cpu_dumps(fx: &buiy_verify::coverage::Fixture, cell: &Cell) -> (String, String) {
    let mut app = build_app(fx, cell);
    app.update();
    let layout = layout_dump(app.world());
    let names = NameLookup::from_world(app.world());
    let nodes = extract_nodes_from_world(app.world());
    let display_list = display_list_dump(&nodes, &names);
    (layout, display_list)
}

/// The DPR-invariance property. For every snapshot-enrolled cell, the CPU layout
/// dump AND the CPU display-list dump are BYTE-IDENTICAL at `Dpr::X1` and
/// `Dpr::X2`. This is the single, explicit assertion that replaces the 24
/// redundant dpr2 button baselines the collapse removed: it proves DPR does not
/// affect CPU output, so a single-DPR snapshot tier loses nothing.
///
/// A regression that let DPR leak into the logical-px layout / display list
/// (e.g. a scale-factor multiply in the wrong place) would make the two dumps
/// diverge here — caught once, centrally, instead of via 24 duplicate `.snap`s.
#[test]
fn cpu_snapshots_are_dpr_invariant() {
    let mut compared = 0usize;
    // `cpu_snapshots()` carries a single DPR; pair each of its cells with the
    // SECOND integer DPR to form the (X1, X2) comparison. Skip a cell the fixture
    // cannot paint (same skip the snapshot tiers honor) so we never compare two
    // empty/degenerate dumps and call DPR "invariant" vacuously.
    for fx in sorted_catalog() {
        for cell in Matrix::cpu_snapshots().cells() {
            if !fx.snapshots_cell(&cell) {
                continue;
            }
            let lo = Cell {
                dpr: Dpr::X1,
                ..cell
            };
            let hi = Cell {
                dpr: Dpr::X2,
                ..cell
            };
            assert_ne!(lo.dpr, hi.dpr, "the two cells must differ in DPR");

            let (layout_lo, dl_lo) = cpu_dumps(fx, &lo);
            let (layout_hi, dl_hi) = cpu_dumps(fx, &hi);

            assert_eq!(
                layout_lo,
                layout_hi,
                "CPU layout dump must be DPR-invariant for {}.{} (theme={}, viewport={}, fc={}): \
                 ResolvedLayout is logical-px, so DPR must not change it",
                fx.name,
                fx.state,
                cell.theme.key(),
                cell.viewport.key,
                cell.forced_colors,
            );
            assert_eq!(
                dl_lo, dl_hi,
                "CPU display-list dump must be DPR-invariant for {}.{} (viewport={}, fc={}): \
                 the display list is logical-px, so DPR must not change it",
                fx.name, fx.state, cell.viewport.key, cell.forced_colors,
            );
            compared += 1;
        }
    }
    assert!(
        compared > 0,
        "the DPR-invariance property must compare at least one cell (else it is vacuous)"
    );
}
