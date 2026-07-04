//! Dooduel `DoodleAvatar` — 22 hand-drawn doodle icons on a tinted circular
//! badge, deterministically chosen from a player's name.
//!
//! Ported from `docs/reference-designs/dooduel/DoodleAvatar.dc.html`: each doodle
//! is a set of stroked SVG primitives (paths + lines + circles + ellipses + dots),
//! white on a tint hashed from the name. The design authors on a **40×40** viewBox,
//! and the F3 `icon()` element takes the viewBox as an argument — so the doodle
//! coordinates and the `2.6` stroke width pass through in their **native 40-box
//! units** (the prototype had to pre-scale every coordinate by `24/40` because the
//! icon viewBox was pinned to 24; the F3 viewbox arg retires that scale).
//!
//! **The whole doodle is ONE stroked `Icon`.** `Icon` takes a single multi-subpath
//! `d` string + one stroke width + one paint. We fold every primitive into one
//! stroked `d`:
//!   - `<path>` d-strings pass through verbatim (M/L/Q/C/Z only — no arcs);
//!   - `<line>` → `M x1 y1 L x2 y2`;
//!   - `<circle>`/`<ellipse>`/`<dot>` → 4 cubic-bezier quadrants (the kappa
//!     approximation — deliberately NOT SVG arcs, whose flag values must not scale).
//!
//! The design's tiny FILLED dots become small STROKED rings here (a stroke ≈ the
//! dot's diameter reads as a filled dot at avatar sizes) — the one fidelity
//! compromise of the single-stroked-icon approach.
//!
//! The badge (tint fill + circular clip + the design's `1.5px solid rgba(0,0,0,.22)`
//! ring) is the SAME `icon(...)` node: `.background(tint).radius(Full).border(…)` —
//! the F4b bordered-rounded fill fix means the tinted fill rounds cleanly under the
//! ring band (no square "ears"), so the whole badge is one element.

use bevy::asset::Handle;
use bevy::image::Image;
use buiy_view::{Color, Element, LineStyle, Radius, icon, raster};

/// The design's white doodle stroke width, in the native 40-box viewBox units the
/// F3 `icon()` viewbox arg expects (`stroke-width: 2.6`).
const STROKE_W: f32 = 2.6;
/// The design's authoring viewBox (`DoodleAvatar` is `viewBox="0 0 40 40"`).
const VIEWBOX: f32 = 40.0;
/// Bezier circle-quadrant control handle length as a fraction of the radius.
const KAPPA: f32 = 0.552_285;

/// The design badge ring: `border: 1.5px solid rgba(0,0,0,.22)`.
const RING: Color = Color::Custom(0, 0, 0, 56); // .22 · 255 ≈ 56
const RING_W: f32 = 1.5;

/// The 10 muted avatar tints (design `TINTS`, in order), as exact sRGB.
const TINTS: [Color; 10] = [
    Color::rgb(0xc2, 0x41, 0x0c),
    Color::rgb(0x1f, 0x6f, 0x54),
    Color::rgb(0x9a, 0x5b, 0x2b),
    Color::rgb(0x3a, 0x6e, 0xa5),
    Color::rgb(0x8a, 0x5c, 0xb0),
    Color::rgb(0xb0, 0x3a, 0x4e),
    Color::rgb(0x5a, 0x7d, 0x3a),
    Color::rgb(0x2f, 0x7d, 0x5b),
    Color::rgb(0xa8, 0x76, 0x1c),
    Color::rgb(0x7a, 0x6c, 0xc4),
];

/// The white doodle stroke color.
const STROKE_COLOR: Color = Color::rgb(0xff, 0xff, 0xff);

/// One doodle's primitives (design coordinates on the 40×40 viewBox).
struct Doodle {
    paths: &'static [&'static str],
    lines: &'static [(f32, f32, f32, f32)],
    circles: &'static [(f32, f32, f32)],
    ellipses: &'static [(f32, f32, f32, f32)],
    dots: &'static [(f32, f32, f32)],
}

impl Doodle {
    const fn new() -> Self {
        Doodle {
            paths: &[],
            lines: &[],
            circles: &[],
            ellipses: &[],
            dots: &[],
        }
    }
}

/// The 22 doodles, in the design's `ICON_KEYS` order (name-hash indexes this).
/// cat, star, rocket, sun, cactus, dino, penguin, octopus, icecream, balloon,
/// robot, ghost, cloud, mushroom, butterfly, fish, umbrella, snowman, owl,
/// flower, turtle, heart.
const ICONS: [Doodle; 22] = [
    // cat
    Doodle {
        paths: &[
            "M12,17 L15,8 L18,16 Z",
            "M22,16 L25,8 L28,17 Z",
            "M17,26 Q20,29 23,26",
        ],
        lines: &[
            (6.0, 22.0, 13.0, 21.0),
            (6.0, 26.0, 13.0, 25.0),
            (34.0, 22.0, 27.0, 21.0),
            (34.0, 26.0, 27.0, 25.0),
        ],
        circles: &[(20.0, 23.0, 9.0)],
        dots: &[(17.0, 21.0, 1.5), (23.0, 21.0, 1.5)],
        ..Doodle::new()
    },
    // star
    Doodle {
        paths: &[
            "M20,5 L23.2,13.6 L32.4,14 L25.2,19.7 L27.6,28.5 L20,23.5 L12.4,28.5 L14.8,19.7 L7.6,14 L16.8,13.6 Z",
        ],
        ..Doodle::new()
    },
    // rocket
    Doodle {
        paths: &[
            "M20,6 C25,11 26,19 24,27 L16,27 C14,19 15,11 20,6 Z",
            "M16,24 L9,32 L16,29 Z",
            "M24,24 L31,32 L24,29 Z",
            "M17,29 Q20,35 23,29",
        ],
        circles: &[(20.0, 16.0, 3.0)],
        ..Doodle::new()
    },
    // sun
    Doodle {
        circles: &[(20.0, 20.0, 7.0)],
        lines: &[
            (30.0, 20.0, 35.0, 20.0),
            (10.0, 20.0, 5.0, 20.0),
            (20.0, 30.0, 20.0, 35.0),
            (20.0, 10.0, 20.0, 5.0),
            (27.1, 27.1, 30.6, 30.6),
            (12.9, 12.9, 9.4, 9.4),
            (12.9, 27.1, 9.4, 30.6),
            (27.1, 12.9, 30.6, 9.4),
        ],
        ..Doodle::new()
    },
    // cactus
    Doodle {
        paths: &[
            "M17,33 L17,14 Q17,9 20,9 Q23,9 23,14 L23,33 Z",
            "M17,22 Q10,22 10,17 Q10,13 14,13",
            "M23,18 Q30,18 30,13 Q30,9 26,9",
        ],
        lines: &[
            (9.0, 33.0, 31.0, 33.0),
            (15.0, 18.0, 13.0, 18.0),
            (27.0, 24.0, 29.0, 24.0),
            (15.0, 27.0, 13.0, 27.0),
        ],
        ..Doodle::new()
    },
    // dino
    Doodle {
        paths: &[
            "M6,29 C6,20 11,15 18,15 C19,12 23,11 25,14 C30,14 33,18 32,23 C34,25 34,28 31,29 Z",
            "M15,15 L17,10 L19,15 Z",
            "M20,13 L22,8 L24,13 Z",
            "M25,14 L27,9 L29,14 Z",
            "M6,27 Q2,26 3,22",
        ],
        lines: &[(12.0, 29.0, 12.0, 33.0), (27.0, 29.0, 27.0, 33.0)],
        dots: &[(11.0, 21.0, 1.4)],
        ..Doodle::new()
    },
    // penguin
    Doodle {
        ellipses: &[(20.0, 20.0, 9.0, 12.0)],
        paths: &[
            "M17,21 L13,23 L17,25 Z",
            "M11,18 Q7,22 10,27",
            "M29,18 Q33,22 30,27",
        ],
        lines: &[(16.0, 32.0, 14.0, 35.0), (24.0, 32.0, 26.0, 35.0)],
        dots: &[(16.0, 16.0, 1.4), (22.0, 16.0, 1.4)],
        ..Doodle::new()
    },
    // octopus
    Doodle {
        paths: &[
            "M9,20 Q9,7 20,7 Q31,7 31,20 Z",
            "M12,20 Q10,26 13,30 Q15,33 12,36",
            "M17,20 Q17,28 15,32 Q14,35 17,37",
            "M23,20 Q23,28 25,32 Q26,35 23,37",
            "M28,20 Q30,26 27,30 Q25,33 28,36",
        ],
        dots: &[(15.0, 15.0, 1.6), (25.0, 15.0, 1.6)],
        ..Doodle::new()
    },
    // icecream
    Doodle {
        circles: &[(20.0, 15.0, 8.0)],
        paths: &["M14,20 L20,35 L26,20 Z", "M15,9 Q20,4 25,9"],
        ..Doodle::new()
    },
    // balloon
    Doodle {
        ellipses: &[(20.0, 16.0, 9.0, 11.0)],
        paths: &["M17,26 L20,29 L23,26 Z", "M20,29 Q17,32 20,34 Q23,36 20,38"],
        ..Doodle::new()
    },
    // robot
    Doodle {
        paths: &[
            "M11,14 Q11,10 15,10 L25,10 Q29,10 29,14 L29,25 Q29,29 25,29 L15,29 Q11,29 11,25 Z",
            "M15,24 Q20,27 25,24",
        ],
        lines: &[(20.0, 10.0, 20.0, 5.0)],
        circles: &[(20.0, 4.0, 1.8)],
        dots: &[(16.0, 18.0, 1.8), (24.0, 18.0, 1.8)],
        ..Doodle::new()
    },
    // ghost
    Doodle {
        paths: &[
            "M11,32 L11,18 Q11,8 20,8 Q29,8 29,18 L29,32 Q26,29 23,32 Q20,35 17,32 Q14,29 11,32 Z",
        ],
        circles: &[(20.0, 23.0, 1.6)],
        dots: &[(16.0, 18.0, 1.7), (24.0, 18.0, 1.7)],
        ..Doodle::new()
    },
    // cloud
    Doodle {
        paths: &[
            "M11,25 Q7,25 7,21 Q7,17 11,17 Q11,12 17,12 Q22,12 23,16 Q29,15 29,21 Q29,25 24,25 Z",
        ],
        lines: &[
            (14.0, 28.0, 13.0, 31.0),
            (20.0, 29.0, 19.0, 32.0),
            (26.0, 28.0, 25.0, 31.0),
        ],
        ..Doodle::new()
    },
    // mushroom
    Doodle {
        paths: &[
            "M8,20 Q8,9 20,9 Q32,9 32,20 Z",
            "M15,20 L15,28 Q15,32 20,32 Q25,32 25,28 L25,20 Z",
        ],
        dots: &[(14.0, 15.0, 1.5), (22.0, 13.0, 1.7), (27.0, 17.0, 1.3)],
        ..Doodle::new()
    },
    // butterfly
    Doodle {
        paths: &[
            "M20,20 Q8,10 6,18 Q6,24 20,22 Z",
            "M20,20 Q10,24 10,30 Q12,33 20,24 Z",
            "M20,20 Q32,10 34,18 Q34,24 20,22 Z",
            "M20,20 Q30,24 30,30 Q28,33 20,24 Z",
        ],
        lines: &[
            (20.0, 14.0, 20.0, 28.0),
            (20.0, 14.0, 17.0, 9.0),
            (20.0, 14.0, 23.0, 9.0),
        ],
        ..Doodle::new()
    },
    // fish
    Doodle {
        ellipses: &[(16.0, 20.0, 10.0, 7.0)],
        paths: &["M28,20 L35,14 L35,26 Z", "M16,13 L20,8 L22,14 Z"],
        lines: &[(11.0, 23.0, 14.0, 23.0)],
        dots: &[(12.0, 18.0, 1.5)],
        ..Doodle::new()
    },
    // umbrella
    Doodle {
        paths: &[
            "M6,20 Q6,7 20,7 Q34,7 34,20 Q29,16 24,20 Q20,16 16,20 Q11,16 6,20 Z",
            "M20,33 Q14,33 14,28",
        ],
        lines: &[(20.0, 20.0, 20.0, 33.0), (20.0, 7.0, 20.0, 4.0)],
        ..Doodle::new()
    },
    // snowman
    Doodle {
        circles: &[(20.0, 28.0, 8.0), (20.0, 14.0, 6.0)],
        paths: &["M20,15 L26,16 L20,17 Z"],
        lines: &[(12.0, 27.0, 5.0, 22.0), (28.0, 27.0, 35.0, 22.0)],
        dots: &[
            (18.0, 12.0, 1.0),
            (22.0, 12.0, 1.0),
            (20.0, 26.0, 1.2),
            (20.0, 31.0, 1.2),
        ],
        ..Doodle::new()
    },
    // owl
    Doodle {
        ellipses: &[(20.0, 22.0, 11.0, 11.0)],
        paths: &[
            "M11,12 L9,5 L15,10 Z",
            "M29,12 L31,5 L25,10 Z",
            "M18,24 L20,28 L22,24 Z",
            "M10,24 Q8,30 12,34",
            "M30,24 Q32,30 28,34",
        ],
        circles: &[(15.0, 19.0, 4.0), (25.0, 19.0, 4.0)],
        dots: &[(15.0, 19.0, 1.3), (25.0, 19.0, 1.3)],
        ..Doodle::new()
    },
    // flower
    Doodle {
        circles: &[
            (20.0, 11.0, 5.0),
            (28.0, 16.0, 5.0),
            (25.0, 25.0, 5.0),
            (15.0, 25.0, 5.0),
            (12.0, 16.0, 5.0),
            (20.0, 18.0, 4.0),
        ],
        paths: &["M20,30 Q26,28 26,33 Q21,34 20,30 Z"],
        lines: &[(20.0, 23.0, 20.0, 36.0)],
        ..Doodle::new()
    },
    // turtle
    Doodle {
        ellipses: &[(18.0, 19.0, 11.0, 9.0)],
        paths: &["M12,19 Q20,24 28,19"],
        circles: &[(33.0, 19.0, 4.0)],
        lines: &[
            (20.0, 11.0, 20.0, 27.0),
            (11.0, 26.0, 7.0, 31.0),
            (27.0, 26.0, 31.0, 31.0),
            (11.0, 12.0, 7.0, 7.0),
            (27.0, 12.0, 31.0, 7.0),
            (8.0, 19.0, 4.0, 19.0),
        ],
        dots: &[(34.0, 17.0, 1.0)],
    },
    // heart
    Doodle {
        paths: &["M20,32 C8,24 6,15 12,10 C16,7 20,9 20,14 C20,9 24,7 28,10 C34,15 32,24 20,32 Z"],
        ..Doodle::new()
    },
];

/// Design `hashStr`: `h = (h*31 + charCode) >>> 0` folded to `u32` (wrapping).
/// ASCII names match JS `charCodeAt` exactly.
fn hash_str(s: &str) -> u32 {
    let mut h: u32 = 0;
    for c in s.chars() {
        h = h.wrapping_mul(31).wrapping_add(c as u32);
    }
    h
}

/// The doodle index + tint for `name` (design: `icon = ICONS[hash(name) % 22]`,
/// `tint = TINTS[hash(name + "-tint") % 10]`). Same name ⇒ same avatar, no state.
fn avatar_for(name: &str) -> (usize, Color) {
    let icon_idx = (hash_str(name) % ICONS.len() as u32) as usize;
    let tint_idx = (hash_str(&format!("{name}-tint")) % TINTS.len() as u32) as usize;
    (icon_idx, TINTS[tint_idx])
}

/// A full circle (native 40-box coords) as 4 cubic-bezier quadrants (no arcs).
fn circle_d(cx: f32, cy: f32, r: f32) -> String {
    ellipse_d(cx, cy, r, r)
}

/// A full ellipse (native coords) as 4 cubic-bezier quadrants — kappa handles.
fn ellipse_d(cx: f32, cy: f32, rx: f32, ry: f32) -> String {
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    format!(
        "M{r} {cy} C{r} {a} {b} {d} {cx} {d} C{e} {d} {l} {a} {l} {cy} C{l} {g} {e} {h} {cx} {h} C{b} {h} {r} {g} {r} {cy} Z",
        r = cx + rx,
        l = cx - rx,
        d = cy + ry,
        h = cy - ry,
        a = cy + ky,
        g = cy - ky,
        b = cx + kx,
        e = cx - kx,
        cx = cx,
        cy = cy,
    )
}

/// Build the whole doodle as ONE stroked `d` in the native 40-box coordinates: the
/// raw paths verbatim, lines as `M L`, circles/ellipses/dots as bezier rings. No
/// scaling — the F3 `icon()` viewbox arg (`VIEWBOX`) carries the coordinate space.
fn build_doodle_d(idx: usize) -> String {
    let doodle = &ICONS[idx];
    let mut d = String::new();
    for p in doodle.paths {
        d.push_str(p);
        d.push(' ');
    }
    for &(x1, y1, x2, y2) in doodle.lines {
        d.push_str(&format!("M{x1} {y1} L{x2} {y2} "));
    }
    for &(cx, cy, r) in doodle.circles {
        d.push_str(&circle_d(cx, cy, r));
        d.push(' ');
    }
    for &(cx, cy, rx, ry) in doodle.ellipses {
        d.push_str(&ellipse_d(cx, cy, rx, ry));
        d.push(' ');
    }
    for &(cx, cy, r) in doodle.dots {
        d.push_str(&circle_d(cx, cy, r));
        d.push(' ');
    }
    d
}

/// The number of stock doodle icons (the design's `ICON_KEYS.length` = 22). The
/// avatar-editor gallery renders one forced badge per index.
pub const ICON_COUNT: usize = ICONS.len();
/// The number of avatar tints (the design's `TINTS.length` = 10).
pub const TINT_COUNT: usize = TINTS.len();

/// A tinted circular badge with `ICONS[icon_idx]` stroked white on `tint`, at
/// `badge_px`, with the design's `1.5px solid rgba(0,0,0,.22)` dark ring. One
/// `icon(...)` node — the badge quad + the ring band + the icon coverage.
fn doodle_badge<Msg>(icon_idx: usize, tint: Color, badge_px: f32) -> Element<Msg> {
    let d = build_doodle_d(icon_idx.min(ICONS.len() - 1));
    // The design draws the doodle at 68% of the badge, centered.
    let size_px = (badge_px * 0.68).round().max(1.0) as u16;
    icon(d, size_px, STROKE_W, VIEWBOX)
        .width(badge_px)
        .height(badge_px)
        .background(tint)
        .radius(Radius::Full)
        .border(RING_W, RING, LineStyle::Solid)
        .color(STROKE_COLOR)
}

/// A `DoodleAvatar` element: the name-hashed doodle on its name-hashed tint.
pub fn doodle_avatar<Msg>(name: &str, badge_px: f32) -> Element<Msg> {
    let (idx, tint) = avatar_for(name);
    doodle_badge(idx, tint, badge_px)
}

/// A `DoodleAvatar` with an EXPLICIT icon + tint (the avatar-editor gallery pick
/// and the human's chosen preset). `tint_idx` indexes `TINTS`.
pub fn doodle_avatar_forced<Msg>(icon_idx: usize, tint_idx: usize, badge_px: f32) -> Element<Msg> {
    doodle_badge(icon_idx, TINTS[tint_idx % TINTS.len()], badge_px)
}

/// The human's *drawn* custom avatar: the committed 220×220 image sampled onto a
/// `badge_px` square. The circular clip is stamped onto the `RasterImage` entity by
/// `paint::round_avatar_rasters` (F4b raster rounded clip needs a `Border` on the
/// node, which the view `raster()` element does not lower — see that system), so a
/// drawn avatar reads round like the stock doodles.
pub fn custom_avatar<Msg>(handle: Handle<Image>, badge_px: f32) -> Element<Msg> {
    raster(handle, badge_px, badge_px)
}

/// A tiny pencil "edit" glyph as an `Icon` path (the design's ✏️ badge — the font
/// stack can't render color emoji, so we stroke a pencil doodle instead). On the
/// 24×24 icon viewBox: a pencil body from lower-left to upper-right + the tip.
pub const PENCIL_D: &str = "M5 19 L7 13 L15 5 L19 9 L11 17 L5 19 Z M13 7 L17 11";
/// The pencil glyph's authoring viewBox (24-box — distinct from the 40-box doodles).
pub const PENCIL_VIEWBOX: f32 = 24.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_matches_design_js() {
        // `h = (h*31 + charCode) >>> 0`. "Mara" = ((((0*31+77)*31+97)*31+114)*31+97).
        assert_eq!(
            hash_str("Mara"),
            ((((77u32 * 31 + 97) * 31 + 114) * 31 + 97),).0
        );
    }

    #[test]
    fn avatar_is_deterministic_and_in_range() {
        let (i1, t1) = avatar_for("Priya");
        let (i2, t2) = avatar_for("Priya");
        assert_eq!(i1, i2);
        assert_eq!(t1, t2);
        assert!(i1 < ICONS.len());
        assert!(avatar_for("Theo").0 < ICONS.len());
    }

    #[test]
    fn all_22_doodles_build_nonempty_native_paths() {
        for idx in 0..ICONS.len() {
            let d = build_doodle_d(idx);
            assert!(!d.trim().is_empty(), "doodle {idx} built an empty path");
            // Native 40-box: no coordinate should exceed ~40.
            for tok in d.split_whitespace() {
                if let Ok(v) = tok.parse::<f32>() {
                    assert!(v.abs() <= 41.0, "doodle {idx} coord {v} out of 40-box");
                }
            }
        }
    }
}
