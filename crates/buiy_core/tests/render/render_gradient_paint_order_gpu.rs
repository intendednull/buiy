//! GPU lane (`#[ignore]` — needs a real wgpu adapter / lavapipe): the
//! background-gradient PAINT-ORDER regression. A node's background gradient must
//! paint at ITS node's paint-order position — after that node's own quad and
//! BEFORE its descendants' quads — so an ANCESTOR's gradient layer never
//! overpaints a descendant's opaque fill.
//!
//! This is the focused guard for the parity gradient-bleed bug: the gallery's
//! viewport `<main>` carries a `RadialGradient::dot_grid` `BackgroundLayer` and
//! the component cards are its paint-order DESCENDANTS, but the pre-fix flat pass
//! drew the WHOLE quad blob and THEN the WHOLE gradient blob, so every gradient
//! painted over every quad — the viewport dot-grid bled on top of the opaque
//! cards. (The structural cause: `node.rs` issued one `draw(0..gradient_count)`
//! after the quad `flat_ranges`, so an ancestor gradient sat above descendant
//! quads.)
//!
//! Fixture: a 64×64 PARENT box carrying a `linear-gradient(90deg, RED→GREEN)`
//! `BackgroundLayer` (no solid fill — the gradient IS the parent's only paint)
//! and a 32×32 DESCENDANT box at (16,16) with an OPAQUE solid blue
//! `Background.color`. The child paints AFTER the parent (descendant ⇒ higher
//! paint order). We assert:
//!
//!   - the CHILD's fill region (its center) is the opaque solid BLUE — the parent
//!     gradient did NOT overpaint it (blue channel high, blue-dominant); on the
//!     pre-fix code this pixel is the red→green gradient (blue ≈ 0), so the test
//!     FAILS, reproducing the bug, and
//!   - a PARENT-ONLY region (outside the child) IS the painted gradient (red
//!     present, blue ≈ 0) — proving the gradient pipeline actually ran, so the
//!     child assertion is not trivially green from the gradient being absent.
//!
//! Adapter-tolerant (relative channel separation on the blue channel, ±tol),
//! passes on this RX 6700 XT host AND CI's lavapipe. NOT a blessed golden.
//!
//! Run:  cargo test -p buiy_core --test render gradient_paint_order -- --ignored --test-threads=1

use bevy::prelude::*;
use buiy_core::Length;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Sizing, Style};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{
    Background, BackgroundLayer, BackgroundLayers, ColorStop, LinearGradient,
};
use buiy_core::render::golden::{GoldenConfig, capture_app, capture_to_image};
use std::borrow::Cow;

use crate::support::px;

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn ancestor_gradient_does_not_overpaint_descendant_solid_fill() {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut app = capture_app(W, H);
    {
        // Maximally-distinct stops/fill so the bug's signature is unambiguous on
        // the BLUE channel: the gradient (red→green) has blue ≈ 0 everywhere; the
        // child solid is blue-dominant. If the ancestor gradient bleeds over the
        // child, the child center reads ~no blue (FAIL); if paint order is
        // correct, it reads the solid blue (PASS).
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert("test.grad.start".into(), Color::srgb_u8(0xff, 0x00, 0x00)); // RED
        theme
            .colors
            .insert("test.grad.end".into(), Color::srgb_u8(0x00, 0xff, 0x00)); // GREEN
        theme
            .colors
            .insert("test.child.fill".into(), Color::srgb_u8(0x40, 0x60, 0xff)); // opaque blue
    }

    // PARENT: a 64×64 box filling the view, painted ONLY by a 90deg (left→right)
    // RED→GREEN linear gradient (no solid `Background` — the gradient is its only
    // paint). It is the paint-order ANCESTOR of the child below.
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(0.0)),
                    left: Sizing::Length(Length::px(0.0)),
                    ..default()
                })
                .width_px(64.0)
                .height_px(64.0),
            BackgroundLayers(vec![BackgroundLayer::Linear(LinearGradient {
                angle_deg: 90.0,
                stops: vec![
                    ColorStop {
                        color: ColorToken::Token(Cow::Borrowed("test.grad.start")),
                        position: 0.0,
                    },
                    ColorStop {
                        color: ColorToken::Token(Cow::Borrowed("test.grad.end")),
                        position: 1.0,
                    },
                ],
            })]),
        ))
        .id();

    // CHILD (descendant ⇒ paints AFTER the parent): a 32×32 box at (16,16) with an
    // OPAQUE solid blue fill and NO gradient — the region that must survive intact
    // above the ancestor gradient.
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(16.0)),
                    left: Sizing::Length(Length::px(16.0)),
                    ..default()
                })
                .width_px(32.0)
                .height_px(32.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed("test.child.fill")),
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);
    // A root container holding the parent (mirrors the other gradient GPU fixtures).
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[parent]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    assert_eq!(img.dimensions(), (W, H));

    // Write the PNG proof artifact (NOT a blessed golden).
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/reports/parity-final-assets/gradient-paint-order-gpu.png");
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    img.save(&out)
        .unwrap_or_else(|e| panic!("write {}: {e}", out.display()));

    let pixels = img.clone().into_raw();

    // Child spans [16,48); sample its center (32,32). A PARENT-ONLY pixel: (6,6),
    // inside the parent [0,64) but outside the child, near the RED start stop.
    let child_center = px(&pixels, W, 32, 32);
    let parent_only = px(&pixels, W, 6, 6);

    // (1) The parent gradient ACTUALLY PAINTED where it should: the parent-only
    // region near the start stop is red-present, blue-absent — so a green child
    // assertion below cannot be satisfied by the gradient simply being missing.
    assert!(
        parent_only[0] > 100 && parent_only[2] < 60,
        "parent-only region must be the painted RED→GREEN gradient (red present, \
         blue absent), got {parent_only:?}"
    );

    // (2) THE REGRESSION: the child's opaque blue fill survives intact — the
    // ancestor gradient did NOT overpaint it. Blue is HIGH and DOMINANT (the solid
    // fill); the pre-fix bug would show the red/green gradient here (blue ≈ 0).
    assert!(
        child_center[2] > 180,
        "child fill region must be the opaque solid BLUE (ancestor gradient must \
         not overpaint a descendant's fill); pre-fix this is the red→green \
         gradient (blue ≈ 0). got {child_center:?}"
    );
    assert!(
        child_center[2] > child_center[0] && child_center[2] > child_center[1],
        "child fill region must be BLUE-DOMINANT (the solid fill, not the \
         red/green ancestor gradient bleeding through), got {child_center:?}"
    );
}
