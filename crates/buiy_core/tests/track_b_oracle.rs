//! TRACK B WAVE 1.0 — ground-truth color oracle (TEMPORARY; delete before PR).
//! Dumps every seeded (theme, token) -> Color so the typed-token migration has a
//! byte-identical parity target. DARK is the pixel-critical (gallery) palette;
//! LIGHT's sparse set + FORCED are expected-to-change (documented in the plan).

use buiy_core::theme::{default_dark_theme, default_light_theme, forced_colors_theme};

fn dump(name: &str, theme: buiy_core::theme::Theme) {
    let mut keys: Vec<_> = theme.colors.keys().cloned().collect();
    keys.sort();
    println!("--- {name} theme: {} color tokens ---", keys.len());
    for k in keys {
        let c = theme.colors[&k];
        let s = bevy::color::Srgba::from(c);
        println!(
            "{name}\t{k}\tsrgba({:.4},{:.4},{:.4},{:.4})",
            s.red, s.green, s.blue, s.alpha
        );
    }
}

#[test]
fn track_b_color_oracle() {
    dump("light", default_light_theme());
    dump("dark", default_dark_theme());
    dump("forced", forced_colors_theme());
}
