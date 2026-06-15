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
//! **Why the light-theme display-list snapshot shows `#ff00ffff` (magenta).**
//! Buiy's forced-colors model is a *wholesale theme swap*: the light theme holds
//! only brand tokens, the forced theme only the 16 system-color tokens — no
//! single token resolves in BOTH (theme.rs). A system-color token therefore
//! misses under the light theme and renders the magenta sentinel; under the
//! forced theme it resolves (e.g. `ButtonText` → white). The committed
//! `*.light.*` display-list baselines record that magenta faithfully — it is the
//! expected artifact of system-color tokens being forced-colors-only, NOT a
//! harness bug. Reconciling the two-theme split (so one widget resolves cleanly
//! in both) is the same `buiy-widget-catalog-design` / theme-tokens concern.

use bevy::prelude::*;
use buiy_core::render::color::{ColorToken, SystemColorKeyword};
use buiy_core::render::components::{Background, Border, BorderSide, LineStyle};
use buiy_widgets::Button;

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
}

/// A solid border side painted with a system-color token.
fn solid(kw: SystemColorKeyword) -> BorderSide {
    BorderSide {
        color: ColorToken::SystemColor(kw),
        style: LineStyle::Solid,
    }
}
