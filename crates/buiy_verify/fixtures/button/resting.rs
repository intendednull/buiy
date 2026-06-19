//! Catalog fixture: `button` × `resting` (coverage.md § "The fixture as single
//! source of truth").
//!
//! Spawns the live [`Button::new`](buiy_widgets::Button::new) bundle the
//! `hello_button` example uses — the catalog row, named once — into a
//! deterministic app. The matrix enrolls it across every tier
//! (layout / display-list / invariant / golden) and the forced-colors scan.
//!
//! **Forced-colors-safe paint (a deliberate boundary).** The live default
//! `Button::new` paints `Background { color: Token("color.surface.secondary") }`
//! — a *brand* token absent from the forced-colors system-color map, which
//! under `forced_colors: active` resolves to the magenta sentinel (a genuine
//! gate-#11 `NonSystemColor` violation; color-and-forced-colors.md § 3.1). The
//! default widget being forced-colors-safe is owned by
//! `buiy-widget-catalog-design`, not this verification campaign. So this
//! catalog row overrides the paint with **system-color tokens** — the paint the
//! default catalog must converge to — and the forced-colors producer
//! ([`live_catalog_paint`](crate::coverage::live_catalog_paint)) reads these
//! LIVE spawned components, not a hand-built descriptor. The override is the
//! single line of "what the catalog should be"; everything else is the real
//! button bundle.
//!
//! **Why the snapshot tiers skip the *light* cells (no magenta baseline).**
//! Buiy's forced-colors model is a *wholesale theme swap*: the light theme holds
//! only brand tokens, the forced theme only the 16 system-color tokens — no
//! single token resolves in BOTH (theme.rs). A system-color token therefore
//! misses under the light theme and would render the magenta missing-token
//! sentinel (`#ff00ffff`); under the forced theme it resolves (e.g.
//! `ButtonText` → white). This fixture is thus **system-color-only**, so it
//! declares `paints_cell = |cell| cell.theme == ThemeAxis::ForcedColors`: the
//! CPU snapshot tiers (layout / display-list) **skip its light cells** instead
//! of baselining the sentinel as if it were the expected color. Baselining a
//! known-wrong magenta pixel would cement it and hide a real regression to/from
//! magenta at that cell (audit 2026-06-18). The forced-colors cells — where the
//! tokens DO resolve — keep full snapshot coverage. Reconciling the two-theme
//! split (so the default widget resolves cleanly in both) remains a
//! `buiy-widget-catalog-design` / theme-tokens concern.

use bevy::prelude::*;
use buiy_core::render::color::{ColorToken, SystemColorKeyword};
use buiy_core::render::components::{Background, Border, BorderSide, LineStyle};
use buiy_widgets::Button;

// This file is `#[path]`-included as a module *inside* the `buiy_verify` crate
// (src/coverage/mod.rs), so coverage types are reached via `crate::`, not the
// external `buiy_verify::` path.
use crate::coverage::ThemeAxis;

crate::fixture! {
    name = "button",
    state = "resting",
    spawn = |app: &mut App| {
        app.world_mut().spawn(Camera2d);
        // Spawn the live widget bundle (marker + node + style + focusable +
        // a11y + its default brand-token `Background`/`Border`), then INSERT
        // the forced-colors-safe paint to replace those two components. We
        // cannot override inside the spawn tuple — `Button::new` already
        // carries `Background`/`Border`, so a second copy in the same bundle is
        // a duplicate-component panic. The insert-after-spawn is the override.
        app.world_mut()
            // The catalog row's stable identity — every dump keys on this Name.
            .spawn((Name::new("button"), Button::new("Save")))
            .insert((
                // Forced-colors-safe paint: system-color tokens that resolve in
                // the forced map (ButtonText fill, ButtonBorder stroke). The
                // producer reads these LIVE components off the Name-tagged root.
                Background {
                    color: ColorToken::SystemColor(SystemColorKeyword::ButtonText),
                },
                Border {
                    left: solid(SystemColorKeyword::ButtonBorder),
                    right: solid(SystemColorKeyword::ButtonBorder),
                    top: solid(SystemColorKeyword::ButtonBorder),
                    bottom: solid(SystemColorKeyword::ButtonBorder),
                    ..Default::default()
                },
            ));
    },
    // System-color-only paint (above): its `SystemColor` tokens resolve ONLY
    // under the forced-colors theme swap and would render the magenta sentinel
    // under the brand-token light theme. So the snapshot tiers skip the light
    // cells rather than baseline the sentinel; the forced-colors cells (where the
    // tokens resolve) keep full coverage.
    paints_cell = |cell| cell.theme == ThemeAxis::ForcedColors,
}

/// A solid border side painted with a system-color token.
fn solid(kw: SystemColorKeyword) -> BorderSide {
    BorderSide {
        color: ColorToken::SystemColor(kw),
        style: LineStyle::Solid,
    }
}
