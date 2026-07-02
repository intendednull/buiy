//! `buiy_gallery::inspector` — the **Inspector pane** content + live accent
//! theming (parity Wave C4). Fills the C1 inspector stub (the gear + "INSPECTOR"
//! header) the shell builds with the design's four sections:
//!
//! 1. **name + desc** of the active screen (Geist 14/600 name + Geist Mono
//!    11.5/400 desc — values.md § 4 "Inspector — widget name/desc").
//! 2. **"Composed of"** — a flex-wrap of [`chip`](crate::composites::chip)
//!    composites (a §1.1 type-dot + the primitive name) for the active screen's
//!    widgets (the design's `META.widgets`).
//! 3. **"Live state"** — key/value [`stat_row`](crate::composites::stat_row)s that
//!    UPDATE every frame from the active screen's authoritative ECS state (the
//!    design's per-screen `inspState`).
//! 4. **"Accent"** — the 4 accent swatch buttons (Blue / Green / Violet / Coral,
//!    values.md § 1.1). Pressing one writes [`SetAccent`](buiy_core::theme::SetAccent),
//!    which re-themes the WHOLE app live (the `theme.is_changed()` re-extract).
//!    The currently-selected swatch shows the 2px white border + ring.
//!
//! ## Switch mechanism — rebuild on switch + value-update every frame
//!
//! The **chip set** and the **live-state row set** differ per screen (todo's 4
//! rows are `total/remaining/completed/filter`; the showcase's 5 are different
//! keys entirely). So the name/desc/chips/live-state **skeleton** is *rebuilt*
//! when the [`ScreenRouter`](crate::shell::ScreenRouter) changes
//! ([`rebuild_inspector_on_switch`](crate::inspector::rebuild_inspector_on_switch))
//! — cheaper and clearer than spawning 5 parallel blocks and toggling
//! `Display::None` for a panel whose row *keys* vary. The live-state **values**
//! then refresh every frame
//! ([`update_inspector_live_state`](crate::inspector::update_inspector_live_state),
//! touching `Text`/`TextColor` only on change) so toggling a todo or selecting a
//! row updates the inspector immediately.
//!
//! The static frame (the 4 section containers, their uppercase headings, and the
//! 4 accent swatches) is built ONCE by
//! [`build_inspector_content`](crate::inspector::build_inspector_content) (called
//! right after the shell's `build_inspector` stub); only the per-screen content
//! inside the name/desc + chips + live-state slots is rebuilt on switch.
//!
//! Design: `docs/specs/2026-06-25-widget-catalog-parity-design.md`; exact values:
//! `docs/specs/2026-06-25-widget-catalog-values.md` § 4 (typography), § 6 (chip
//! dot / gear), § 7.1 (the `<aside>` sections), § 1.1 (the accent palette +
//! type-dot colors), § 2 (`shadow.swatch-selected`).

use bevy::prelude::{
    App, Children, Color, Component, Entity, IntoScheduleConfigs, MessageReader, MessageWriter,
    Name, Plugin, Query, Update, With, World,
};
use buiy::prelude::*;
use buiy_core::BuiySet;
use buiy_core::a11y::{A11yLabel, A11yToggled, Toggled};
use buiy_core::interaction::OnPress;
use buiy_core::render::color::ThemeContract;
use buiy_core::render::components::{BoxShadow, LineStyle, Shadow, TextColor};
use buiy_core::text::{FamilyEntry, FontStack, LetterSpacing, LineHeight, Text};
use buiy_core::theme::Theme;

use crate::composites::{chip, stat_row};
use crate::shell::{Screen, ScreenRouter, SetAccent};
use crate::{
    Filter, FilterMode, MenuActivations, ScrollList, ScrollNode, SelectedRow, ShowcaseBuild,
    ShowcaseStepper, TodoRow, scroll_window_size,
};

// ===========================================================================
// Shared authoring helpers (module-local, mirroring `shell.rs` / `composites.rs`
// so the inspector module stays independently buildable).
// ===========================================================================

/// The Geist sans font stack (authored by name — the sans generic resolves to
/// Fira, Wave A note).
fn geist() -> FontFamily {
    FontFamily(FontStack(vec![FamilyEntry::Named("Geist".into())]))
}

/// The Geist Mono font stack.
fn geist_mono() -> FontFamily {
    FontFamily(FontStack(vec![FamilyEntry::Named("Geist Mono".into())]))
}

/// A solid 1px `BorderSide` of a [`ColorToken`] color.
fn solid_side(token: ColorToken) -> BorderSide {
    BorderSide {
        color: token,
        style: LineStyle::Solid,
    }
}

/// A leaf text node (`Pickable::IGNORE` — inspector labels are decorative pixels).
#[allow(clippy::too_many_arguments)]
fn text_leaf(
    world: &mut World,
    name: &str,
    s: &str,
    family: FontFamily,
    size: f32,
    weight: u16,
    color: ColorToken,
    letter_spacing: Option<f32>,
) -> Entity {
    let mut e = world.spawn((
        Node,
        Name::new(name.to_string()),
        Text(s.to_string()),
        FontSize(size),
        family,
        FontWeight(weight),
        TextColor(color),
        Pickable::IGNORE,
    ));
    if let Some(ls) = letter_spacing {
        e.insert(LetterSpacing(ls));
    }
    e.id()
}

// ===========================================================================
// The per-screen inspector metadata (the design's `META`, JS 540–546). The
// widget chips are `(primitive name, §1.1 type-dot color token)` pairs; the
// dot colors are the design's hexes mapped to the dark-theme tokens.
// ===========================================================================

/// One inspector "Composed of" chip: a primitive name + its §1.1 type-dot color
/// token (the design's `META.widgets` `[name, hex]` pairs).
struct WidgetChip {
    name: &'static str,
    dot: ColorToken,
}

/// The inspector description for a screen (the design's `META.desc`, JS 541–545).
fn inspector_desc(screen: Screen) -> &'static str {
    match screen {
        Screen::Todo => "The canonical list app — add, complete, filter, clear.",
        Screen::Scroll => "1,000 nodes, windowed render — only visible rows mount.",
        Screen::Menu => "Anchored popover with roving focus + outside-click scrim.",
        Screen::Modal => "Focus-trapped dialog, backdrop dim, Esc to dismiss.",
        Screen::Showcase => "Switch, slider, segmented, stepper, meter, disclosure.",
    }
}

/// The "Composed of" widget chips for a screen (the design's `META.widgets`, with
/// the hex dot colors mapped to the §1.1 dark-theme type-dot tokens).
fn inspector_widgets(screen: Screen) -> &'static [WidgetChip] {
    // The §1.1 type-dot palette tokens (values.md § 1.1 + the inspector chip rows):
    // #5b86f5 → accent.blue, #d7a23f → status.warn, #45c07d → status.ok,
    // #f0655b → status.error, #b98aff → accent.violet, #868d99 → text.muted,
    // #555c67 → text.dim.
    match screen {
        Screen::Todo => &[
            WidgetChip {
                name: "Stack",
                dot: ColorToken::AccentBlue,
            },
            WidgetChip {
                name: "TextInput",
                dot: ColorToken::StatusWarn,
            },
            WidgetChip {
                name: "Checkbox",
                dot: ColorToken::StatusOk,
            },
            WidgetChip {
                name: "Button",
                dot: ColorToken::StatusOk,
            },
            WidgetChip {
                name: "List",
                dot: ColorToken::AccentBlue,
            },
        ],
        Screen::Scroll => &[
            WidgetChip {
                name: "ScrollView",
                dot: ColorToken::StatusError,
            },
            WidgetChip {
                name: "Row",
                dot: ColorToken::AccentBlue,
            },
            WidgetChip {
                name: "Text",
                dot: ColorToken::TextMuted,
            },
            WidgetChip {
                name: "Badge",
                dot: ColorToken::StatusOk,
            },
        ],
        Screen::Menu => &[
            WidgetChip {
                name: "Popover",
                dot: ColorToken::AccentViolet,
            },
            WidgetChip {
                name: "MenuItem",
                dot: ColorToken::AccentBlue,
            },
            WidgetChip {
                name: "Divider",
                dot: ColorToken::TextDim,
            },
            WidgetChip {
                name: "Icon",
                dot: ColorToken::StatusOk,
            },
        ],
        Screen::Modal => &[
            WidgetChip {
                name: "Dialog",
                dot: ColorToken::AccentViolet,
            },
            WidgetChip {
                name: "Backdrop",
                dot: ColorToken::TextDim,
            },
            WidgetChip {
                name: "Segmented",
                dot: ColorToken::AccentBlue,
            },
            WidgetChip {
                name: "Switch",
                dot: ColorToken::StatusOk,
            },
        ],
        Screen::Showcase => &[
            WidgetChip {
                name: "Switch",
                dot: ColorToken::StatusOk,
            },
            WidgetChip {
                name: "Slider",
                dot: ColorToken::AccentBlue,
            },
            WidgetChip {
                name: "Segmented",
                dot: ColorToken::AccentBlue,
            },
            WidgetChip {
                name: "Stepper",
                dot: ColorToken::StatusWarn,
            },
            WidgetChip {
                name: "Disclosure",
                dot: ColorToken::AccentViolet,
            },
        ],
    }
}

// ===========================================================================
// The accent swatches (the design's `ACC`, JS 666). Each swatch is one of the
// four selectable accents (values.md § 1.1 `accent.*`).
// ===========================================================================

/// One accent swatch's `(token, accessible name, hex)`. The hex is the literal
/// `Color` a press writes via `SetAccent` (the swap re-seeds the ramp); the token
/// resolves the bg fill so the swatch matches the live theme.
struct AccentOption {
    token: ColorToken,
    name: &'static str,
    color: Color,
}

/// The four selectable accents in design order (the design's `ACC`, JS 666):
/// Blue / Green / Violet / Coral (values.md § 1.1 `accent.*`).
const ACCENTS: &[AccentOption] = &[
    AccentOption {
        token: ColorToken::AccentBlue,
        name: "Blue",
        color: Color::srgb_u8(0x5b, 0x86, 0xf5),
    },
    AccentOption {
        token: ColorToken::AccentGreen,
        name: "Green",
        color: Color::srgb_u8(0x45, 0xc0, 0x7d),
    },
    AccentOption {
        token: ColorToken::AccentViolet,
        name: "Violet",
        color: Color::srgb_u8(0xb9, 0x8a, 0xff),
    },
    AccentOption {
        token: ColorToken::AccentCoral,
        name: "Coral",
        color: Color::srgb_u8(0xf0, 0x65, 0x5b),
    },
];

/// Marks one accent swatch button, carrying the accent it selects (the press
/// writes `SetAccent(color)`; [`reflect_accent_selection`] sets its selected ring
/// by comparing `color` to the live theme accent).
#[derive(Component, Clone, Copy)]
pub struct AccentSwatch(pub Color);

// ===========================================================================
// Inspector content slots + live-state row markers (the rebuild handles).
// ===========================================================================

/// Marks the inspector's per-screen content **slots** the switch rebuild rehomes
/// children under (the name/desc leaves, the chip flex-wrap, the live-state
/// column). Held by [`rebuild_inspector_on_switch`] to clear + repopulate.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum InspectorSlot {
    /// The name leaf (`#InspectorName`) — rewritten in place (always present).
    Name,
    /// The desc leaf (`#InspectorDesc`) — rewritten in place (always present).
    Desc,
    /// The "Composed of" chip flex-wrap — its children are despawned + rebuilt.
    Chips,
    /// The "Live state" stat-row column — its children are despawned + rebuilt.
    LiveState,
}

/// Marks one live-state row's value `Text` leaf with its key, so
/// [`update_inspector_live_state`] can find + rewrite the value/color per frame
/// without re-walking the tree by position.
#[derive(Component, Clone)]
pub struct LiveStateValue(pub String);

// ===========================================================================
// Building the static inspector frame (once, after the shell stub).
// ===========================================================================

/// Fill the shell's C1 inspector stub (the `#Inspector` panel — the gear +
/// "INSPECTOR" header already built by `shell::build_inspector`) with the four
/// content sections. Called ONCE at boot, after `build_shell`. The name/desc +
/// chips + live-state content is then populated for the initial screen by
/// [`rebuild_inspector_content`]; the accent swatches are static.
pub fn build_inspector_content(world: &mut World) {
    let Some(panel) = find_named::<crate::shell::Inspector>(world) else {
        return;
    };

    let initial = world.resource::<ScreenRouter>().0;

    let name_desc = build_name_desc_section(world);
    let composed = build_composed_section(world);
    let live = build_live_state_section(world);
    let accent = build_accent_section(world);
    world
        .entity_mut(panel)
        .add_children(&[name_desc, composed, live, accent]);

    // Populate the per-screen content for the boot screen.
    rebuild_inspector_content(world, initial);
}

/// One section container: `padding:14px 16px`, an optional bottom 1px
/// `border.subtle` divider (the Accent section has none — design HTML 436), and a
/// `flex-column`. The section heading + body are added by the caller.
fn section(world: &mut World, name: &str, divider: bool) -> Entity {
    let mut style = Style::default()
        .flex_column()
        .padding_edges(Edges::axis(16.0, 14.0));
    if divider {
        style = style.border_edges(Edges {
            top: Length::px(0.0),
            right: Length::px(0.0),
            bottom: Length::px(1.0),
            left: Length::px(0.0),
        });
    }
    let mut e = world.spawn((Node, Name::new(name.to_string()), style));
    if divider {
        e.insert(Border {
            bottom: solid_side(ColorToken::BorderSubtle),
            ..Default::default()
        });
    }
    e.id()
}

/// One uppercase mono section heading (Geist Mono 10 / 500 / .12em → 1.20px LS,
/// `text.dim`, margin-bottom 11px — values.md § 4 "Inspector — section labels";
/// HTML 414/424/436). The margin-bottom is a fixed `BoxModel` patch.
fn section_heading(world: &mut World, name: &str, s: &str) -> Entity {
    let e = text_leaf(
        world,
        name,
        s,
        geist_mono(),
        10.0,
        500,
        ColorToken::TextDim,
        Some(1.20),
    );
    world.entity_mut(e).insert(BoxModel {
        margin: Edges {
            bottom: Length::px(11.0),
            ..Default::default()
        },
        ..Default::default()
    });
    e
}

/// The name + desc section (HTML 408): `padding:8px 16px 14px`, bottom divider, a
/// Geist 14/600 `text.primary` name (margin-bottom 3px) over a Geist Mono 11.5/400
/// `text.faint` desc (line-height 1.5). Both leaves carry an [`InspectorSlot`] so
/// the switch rebuild rewrites them in place.
fn build_name_desc_section(world: &mut World) -> Entity {
    let name = text_leaf(
        world,
        "#InspectorName",
        "",
        geist(),
        14.0,
        600,
        ColorToken::TextPrimary,
        None,
    );
    world.entity_mut(name).insert((
        InspectorSlot::Name,
        BoxModel {
            margin: Edges {
                bottom: Length::px(3.0),
                ..Default::default()
            },
            ..Default::default()
        },
    ));

    let desc = text_leaf(
        world,
        "#InspectorDesc",
        "",
        geist_mono(),
        11.5,
        400,
        ColorToken::TextFaint,
        None,
    );
    world
        .entity_mut(desc)
        .insert((InspectorSlot::Desc, LineHeight::Scale(1.5)));

    world
        .spawn((
            Node,
            Name::new("#InspectorNameDesc"),
            Style::default()
                .flex_column()
                .padding_edges(Edges {
                    top: Length::px(8.0),
                    right: Length::px(16.0),
                    bottom: Length::px(14.0),
                    left: Length::px(16.0),
                })
                .border_edges(Edges {
                    top: Length::px(0.0),
                    right: Length::px(0.0),
                    bottom: Length::px(1.0),
                    left: Length::px(0.0),
                }),
            Border {
                bottom: solid_side(ColorToken::BorderSubtle),
                ..Default::default()
            },
        ))
        .add_children(&[name, desc])
        .id()
}

/// The "Composed of" section (HTML 413): the heading over a `gap:6px` flex-wrap of
/// chip composites (the wrap carries [`InspectorSlot::Chips`]; its children are
/// rebuilt on switch).
fn build_composed_section(world: &mut World) -> Entity {
    let heading = section_heading(world, "#InspectorComposedLabel", "COMPOSED OF");
    let wrap = world
        .spawn((
            Node,
            InspectorSlot::Chips,
            Name::new("#InspectorChips"),
            Style::default()
                .flex_row()
                .flex_wrap(FlexWrap::Wrap)
                .align_items(AlignItems::FlexStart)
                .gap_px(6.0),
        ))
        .id();

    let section_e = section(world, "#InspectorComposed", true);
    world.entity_mut(section_e).add_children(&[heading, wrap]);
    section_e
}

/// The "Live state" section (HTML 424): the heading over a `gap:9px` column of
/// stat rows (the column carries [`InspectorSlot::LiveState`]; its rows are rebuilt
/// on switch + their values refreshed every frame).
fn build_live_state_section(world: &mut World) -> Entity {
    let heading = section_heading(world, "#InspectorLiveLabel", "LIVE STATE");
    let column = world
        .spawn((
            Node,
            InspectorSlot::LiveState,
            Name::new("#InspectorLiveState"),
            Style::default().flex_column().gap_px(9.0),
        ))
        .id();

    let section_e = section(world, "#InspectorLive", true);
    world.entity_mut(section_e).add_children(&[heading, column]);
    section_e
}

/// The "Accent" section (HTML 436 — no bottom divider): the heading over a
/// `gap:8px` row of the 4 static accent swatch buttons.
fn build_accent_section(world: &mut World) -> Entity {
    let heading = section_heading(world, "#InspectorAccentLabel", "ACCENT");
    let swatches: Vec<Entity> = ACCENTS.iter().map(|a| build_swatch(world, a)).collect();
    let row = world
        .spawn((
            Node,
            Name::new("#InspectorAccentRow"),
            Style::default().flex_row().gap_px(8.0),
        ))
        .add_children(&swatches)
        .id();

    let section_e = section(world, "#InspectorAccent", false);
    world.entity_mut(section_e).add_children(&[heading, row]);
    section_e
}

/// One 30×30 accent swatch button (radius 8 — values.md § 7.1 "accent swatches",
/// JS 668): a bare `Button` (OnPress sink + keymap + a11y role, no auto-label),
/// the accent-colored bg, and a 2px border. Unselected = `border.default`; the
/// selected ring (2px white border + the `shadow.swatch-selected` glow) is set by
/// [`reflect_accent_selection`]. Carries [`AccentSwatch`] (the press handle) + an
/// `A11yLabel` (the accent name).
fn build_swatch(world: &mut World, accent: &AccentOption) -> Entity {
    world
        .spawn((
            buiy::prelude::Button,
            AccentSwatch(accent.color),
            A11yLabel(accent.name.to_string()),
            Name::new(format!("#AccentSwatch-{}", accent.name)),
            Style::default().width_px(30.0).height_px(30.0).border(2.0),
            Background {
                color: accent.token,
            },
            Border {
                top: solid_side(ColorToken::BorderDefault),
                right: solid_side(ColorToken::BorderDefault),
                bottom: solid_side(ColorToken::BorderDefault),
                left: solid_side(ColorToken::BorderDefault),
                radius: Corners::all(Radius::circular(8.0)),
            },
        ))
        .id()
}

// ===========================================================================
// Rebuilding the per-screen content (name/desc + chips + live-state skeleton)
// when the screen switches.
// ===========================================================================

/// Repopulate the inspector's per-screen content for `screen`: rewrite the
/// name/desc leaves in place, despawn + rebuild the chip wrap's children, and
/// despawn + rebuild the live-state column's rows (seeded with the screen's keys;
/// the values are refreshed every frame by [`update_inspector_live_state`]).
pub fn rebuild_inspector_content(world: &mut World, screen: Screen) {
    // Name + desc: rewrite the slot leaves in place.
    set_slot_text(world, InspectorSlot::Name, screen.name());
    set_slot_text(world, InspectorSlot::Desc, inspector_desc(screen));

    // Chips: clear the wrap + rebuild from the screen's widget list.
    if let Some(wrap) = find_slot(world, InspectorSlot::Chips) {
        despawn_children(world, wrap);
        let chips: Vec<Entity> = inspector_widgets(screen)
            .iter()
            .map(|w| chip(world, w.name, w.dot))
            .collect();
        world.entity_mut(wrap).add_children(&chips);
    }

    // Live state: clear the column + rebuild the screen's key rows (seeded with
    // placeholder values; `update_inspector_live_state` fills them this frame).
    if let Some(column) = find_slot(world, InspectorSlot::LiveState) {
        despawn_children(world, column);
        let rows: Vec<Entity> = live_state_keys(screen)
            .iter()
            .map(|&key| build_live_row(world, key))
            .collect();
        world.entity_mut(column).add_children(&rows);
    }

    // Fill the freshly-built rows' values this frame.
    update_live_state_values(world, screen);
}

/// One live-state row: a `stat_row` whose value leaf is tagged with its key
/// ([`LiveStateValue`]) so the per-frame updater can rewrite it. The value/color
/// are seeded empty + refreshed immediately by [`update_live_state_values`].
fn build_live_row(world: &mut World, key: &str) -> Entity {
    let row = stat_row(world, key, "");
    // Tag the value leaf (the second child — the mono value, `stat_row` order
    // `[key, value]`) so the updater finds it by key, not position.
    if let Some(value_leaf) = nth_text_child(world, row, 1) {
        world
            .entity_mut(value_leaf)
            .insert(LiveStateValue(key.to_string()));
    }
    row
}

// ===========================================================================
// The per-screen live-state mapping (the design's `inspState`, JS 659–664).
// ===========================================================================

/// The ordered live-state keys for a screen (the design's `inspState` `k` fields).
/// The row skeleton is built from these on switch; the values are filled per frame.
fn live_state_keys(screen: Screen) -> &'static [&'static str] {
    match screen {
        Screen::Todo => &["total", "remaining", "completed", "filter"],
        Screen::Scroll => &["nodes", "mounted", "window", "selected"],
        Screen::Menu => &["open", "items", "last action"],
        Screen::Modal => &["open", "mode", "focus trap", "name"],
        Screen::Showcase => &["wireframe", "radius", "density", "count", "build"],
    }
}

/// One live-state cell: the value string + its color token (the design's
/// `inspState` `{v, color}`). The colors map the design's hexes to dark tokens:
/// `#c2c8d2` → text.secondary, `#45c07d` → status.ok, `#555c67` → text.dim,
/// `ac` (the live accent) → color.accent.
struct LiveCell {
    value: String,
    color: ColorToken,
}

impl LiveCell {
    fn new(value: impl Into<String>, color: ColorToken) -> Self {
        Self {
            value: value.into(),
            color,
        }
    }
}

/// Compute the `(key → cell)` live-state for `screen` from the authoritative ECS
/// state, in the same order as [`live_state_keys`]. Reads the SAME sources the
/// screens themselves read (the `Filter` / `MenuActivations` resources, the
/// `TodoRow` / `ScrollNode` / `SelectedRow` markers, the showcase state
/// components), so the inspector never duplicates state — it reflects it.
fn compute_live_state(world: &mut World, screen: Screen) -> Vec<(&'static str, LiveCell)> {
    match screen {
        Screen::Todo => {
            let (total, completed) = todo_counts(world);
            let remaining = total - completed;
            // `get_resource` (not `resource`) so the inspector never panics when a
            // screen's plugin is absent (e.g. the layout-snapshot harness builds
            // the tree without `TodoMvcPlugin`); fall back to the default `All`.
            let filter = match world.get_resource::<Filter>().map(|f| f.0) {
                Some(FilterMode::Active) => "active",
                Some(FilterMode::Completed) => "completed",
                _ => "all",
            };
            vec![
                (
                    "total",
                    LiveCell::new(total.to_string(), ColorToken::TextSecondary),
                ),
                (
                    "remaining",
                    // `remaining ? ac : '#45c07d'` — accent while items remain, ok
                    // (all-clear green) at zero (the design's `inspState` todo row).
                    LiveCell::new(
                        remaining.to_string(),
                        if remaining > 0 {
                            ColorToken::Accent
                        } else {
                            ColorToken::StatusOk
                        },
                    ),
                ),
                (
                    "completed",
                    LiveCell::new(completed.to_string(), ColorToken::StatusOk),
                ),
                ("filter", LiveCell::new(filter, ColorToken::TextSecondary)),
            ]
        }
        Screen::Scroll => {
            // "nodes" = the filtered total (every matching row mounts in Buiy's
            // real-overflow scroll). "mounted" = the design's `visRows.length` —
            // the WINDOWED visible-row count, the same span the footer's
            // "rows X–Y mounted" reports (shared `scroll_window_size` math). The
            // two were both wired to the filtered total, so "mounted" read 1000
            // while the footer read 11 — this aligns "mounted" with the footer.
            let filtered = scroll_visible_count(world);
            let windowed = scroll_window_size(scroll_offset_y(world), filtered);
            let selected = selected_scroll_index(world);
            vec![
                (
                    "nodes",
                    LiveCell::new(format_thousands(filtered), ColorToken::TextSecondary),
                ),
                (
                    "mounted",
                    LiveCell::new(windowed.to_string(), ColorToken::Accent),
                ),
                // Our scroll is real-overflow (every matching row mounts, paint/
                // layout-skipped off-screen — NOT DOM windowing), so the visible
                // "window" is the whole filtered set: report "all".
                ("window", LiveCell::new("all", ColorToken::TextSecondary)),
                match selected {
                    Some(i) => (
                        "selected",
                        LiveCell::new(format!("#{i:04}"), ColorToken::Accent),
                    ),
                    None => ("selected", LiveCell::new("none", ColorToken::TextDim)),
                },
            ]
        }
        Screen::Menu => {
            let open = menu_open(world);
            let last = world
                .get_resource::<MenuActivations>()
                .and_then(|m| m.0.last().cloned());
            vec![
                // The menu's open/closed state is owned by the machine-tier `MenuModel`
                // (the W6 MVU migration); the inspector REFLECTS its live `open` field —
                // accent while open, dim while closed (spec §14, the desync fix).
                (
                    "open",
                    LiveCell::new(
                        if open { "true" } else { "false" },
                        if open {
                            ColorToken::Accent
                        } else {
                            ColorToken::TextDim
                        },
                    ),
                ),
                ("items", LiveCell::new("5", ColorToken::TextSecondary)),
                match last {
                    Some(action) => ("last action", LiveCell::new(action, ColorToken::Accent)),
                    None => ("last action", LiveCell::new("—", ColorToken::TextDim)),
                },
            ]
        }
        Screen::Modal => {
            // The dialog open/mode is owned by the C5-d overlay lifecycle; at rest
            // the inspector reports the resting (closed / create) state.
            vec![
                ("open", LiveCell::new("false", ColorToken::TextDim)),
                ("mode", LiveCell::new("create", ColorToken::TextSecondary)),
                ("focus trap", LiveCell::new("idle", ColorToken::TextDim)),
                // The design uses `∅` (U+2205) for the empty-name cell and leans
                // on the browser's font fallback; Buiy's registered-only font
                // system (Geist / Geist Mono / Fira) carries no `∅`, so the
                // literal tofus (finding M4). Use this inspector's OWN established
                // empty-value glyph `—` (em-dash, dim — same as the "last action"
                // None cell above), which Geist renders, preserving the meaning.
                ("name", LiveCell::new("—", ColorToken::TextDim)),
            ]
        }
        Screen::Showcase => {
            let wireframe = showcase_switch_on(world, "Wireframe mode");
            let radius = showcase_radius(world);
            let density = showcase_density(world);
            let count = showcase_count(world);
            let build = showcase_build_pct(world);
            vec![
                (
                    "wireframe",
                    LiveCell::new(
                        if wireframe { "on" } else { "off" },
                        if wireframe {
                            ColorToken::Accent
                        } else {
                            ColorToken::TextDim
                        },
                    ),
                ),
                (
                    "radius",
                    LiveCell::new(format!("{radius}px"), ColorToken::TextSecondary),
                ),
                ("density", LiveCell::new(density, ColorToken::TextSecondary)),
                (
                    "count",
                    LiveCell::new(count.to_string(), ColorToken::TextSecondary),
                ),
                (
                    "build",
                    LiveCell::new(format!("{build}%"), ColorToken::TextSecondary),
                ),
            ]
        }
    }
}

// ===========================================================================
// State readers (the authoritative ECS sources the inspector reflects).
// ===========================================================================

/// `(total, completed)` todo rows: counts every `TodoRow`, and the subset whose
/// `RowCheckbox` child is `A11yToggled::True` (the design's `done` count).
fn todo_counts(world: &mut World) -> (i32, i32) {
    let row_children: Vec<Vec<Entity>> = {
        let mut q = world.query_filtered::<&Children, With<TodoRow>>();
        q.iter(world)
            .map(|c| c.iter().copied().collect::<Vec<_>>())
            .collect()
    };
    let mut total = 0;
    let mut completed = 0;
    for children in row_children {
        total += 1;
        // The row's checkbox child carries the toggle state (the same path
        // `restyle_completed` reads).
        for child in children {
            if let Some(toggled) = world.get::<A11yToggled>(child) {
                if toggled.0 == Toggled::True {
                    completed += 1;
                }
                break;
            }
        }
    }
    (total, completed)
}

/// The count of currently-mounted (non-pruned) scroll rows. Our scroll mounts
/// every *matching* row (the search filter prunes non-matches via `Display::None`),
/// so this is the filtered node total the footer reports.
fn scroll_visible_count(world: &mut World) -> usize {
    let mut mounted = 0;
    let mut q = world.query::<(&ScrollNode, Option<&Display>)>();
    for (_, display) in q.iter(world) {
        if !matches!(display, Some(Display::None)) {
            mounted += 1;
        }
    }
    mounted
}

/// The live vertical scroll offset of the `#ScrollList` (`ScrollOffset.y`), or
/// `0.0` when absent — the windowing input the inspector's "mounted" cell shares
/// with the footer's "rows X–Y mounted" label.
fn scroll_offset_y(world: &mut World) -> f32 {
    use buiy_core::layout::ScrollOffset;
    let mut q = world.query_filtered::<&ScrollOffset, With<ScrollList>>();
    q.iter(world).next().map(|o| o.y).unwrap_or(0.0)
}

/// The selected scroll node's index, if any (`SelectedRow` → `ScrollNode.index`).
fn selected_scroll_index(world: &mut World) -> Option<usize> {
    let mut q = world.query_filtered::<&ScrollNode, With<SelectedRow>>();
    q.iter(world).next().map(|n| n.index)
}

/// Whether the overlay-menu screen's [`Menu`](buiy_widgets::Menu) is currently open —
/// read LIVE from its `MenuModel.open` (the machine-tier single source of truth the W6
/// MVU migration moved the open state into). Replaces the pre-MVU hardcoded `"false"`
/// (a headless-invisible desync, spec §14): the inspector must REFLECT state, never
/// duplicate or guess it. `false` when no menu is mounted (a partial harness).
fn menu_open(world: &mut World) -> bool {
    use buiy_widgets::menu::MenuModel;
    let mut q = world.query::<&MenuModel>();
    q.iter(world).next().map(|m| m.open).unwrap_or(false)
}

/// Whether the showcase switch with accessible name `label` is on
/// (`A11yToggled::True`).
fn showcase_switch_on(world: &mut World, label: &str) -> bool {
    let mut q = world.query::<(&A11yLabel, &A11yToggled)>();
    q.iter(world)
        .any(|(l, t)| l.0 == label && t.0 == Toggled::True)
}

/// The showcase slider's current radius value (rounded; from its `A11yValue.now`).
fn showcase_radius(world: &mut World) -> i32 {
    use buiy_core::a11y::A11yValue;
    use buiy_widgets::Slider;
    let mut q = world.query_filtered::<&A11yValue, With<Slider>>();
    q.iter(world)
        .next()
        .map(|v| v.now.round() as i32)
        .unwrap_or(0)
}

/// The showcase segmented density label (the selected option's text, lower-cased
/// — the design's `seg`). Scoped to the [`ShowcaseDensitySegmented`] track: its
/// children are the density `SegmentedOption`s (Cozy/Compact/Dense). The selected
/// one is filled with the `color.accent` token (the C2 `set_segmented` selected
/// paint); reads its label leaf. WITHOUT this scoping the query would also see
/// the modal screen's KIND segmented (Button/Layout/Input — same `SegmentedOption`
/// marker) and could return its selected "Button". Falls back to `compact`.
fn showcase_density(world: &mut World) -> String {
    use crate::ShowcaseDensitySegmented;
    use crate::composites::SegmentedOption;

    // The density track's option entities (its direct children).
    let options: Vec<Entity> = {
        let mut q = world.query_filtered::<&Children, With<ShowcaseDensitySegmented>>();
        q.iter(world)
            .next()
            .map(|c| c.iter().copied().collect())
            .unwrap_or_default()
    };

    // The accent-filled option's first child is its label leaf (the selected one).
    let label_leaf: Option<Entity> = options.into_iter().find_map(|opt| {
        let is_option = world.get::<SegmentedOption>(opt).is_some();
        let is_accent = world
            .get::<Background>(opt)
            .is_some_and(|bg| matches!(bg.color, ColorToken::Accent));
        if is_option && is_accent {
            world
                .get::<Children>(opt)
                .and_then(|c| c.iter().next().copied())
        } else {
            None
        }
    });

    label_leaf
        .and_then(|leaf| world.get::<Text>(leaf).map(|t| t.0.to_lowercase()))
        .unwrap_or_else(|| "compact".to_string())
}

/// The showcase stepper's current count (the design's `count`).
fn showcase_count(world: &mut World) -> i32 {
    let mut q = world.query::<&ShowcaseStepper>();
    q.iter(world).next().map(|s| s.count).unwrap_or(0)
}

/// The showcase build progress as a whole-number percent (the design's
/// `progress`). `get_resource` so the inspector never panics when `ShowcasePlugin`
/// is absent (the snapshot harness); falls back to 0%.
fn showcase_build_pct(world: &mut World) -> i32 {
    world
        .get_resource::<ShowcaseBuild>()
        .map(|b| (b.progress * 100.0).round() as i32)
        .unwrap_or(0)
}

/// Format a count with thousands separators (the design's `toLocaleString`).
fn format_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

// ===========================================================================
// The per-frame live-state value updater.
// ===========================================================================

/// Refresh the active screen's live-state row values (the design's per-frame
/// `inspState` recompute): compute the cells, then rewrite each row's tagged value
/// `Text` + `TextColor` — only when the value or color actually changed (so it is
/// free at rest). Exclusive (it reads disparate screen state across many queries).
pub fn update_inspector_live_state(world: &mut World) {
    let screen = world.resource::<ScreenRouter>().0;
    update_live_state_values(world, screen);
}

/// Write the computed live-state cells into the tagged value leaves for `screen`.
fn update_live_state_values(world: &mut World, screen: Screen) {
    let cells = compute_live_state(world, screen);
    // Collect the tagged value leaves keyed by their live-state key.
    let leaves: Vec<(Entity, String)> = {
        let mut q = world.query::<(Entity, &LiveStateValue)>();
        q.iter(world).map(|(e, k)| (e, k.0.clone())).collect()
    };
    for (key, cell) in cells {
        let Some(&(leaf, _)) = leaves.iter().find(|(_, k)| k == key) else {
            continue;
        };
        if let Some(mut text) = world.get_mut::<Text>(leaf)
            && text.0 != cell.value
        {
            text.0 = cell.value.clone();
        }
        let want = cell.color;
        if let Some(mut color) = world.get_mut::<TextColor>(leaf)
            && color.0 != want
        {
            color.0 = want;
        }
    }
}

// ===========================================================================
// The switch-rebuild system + the accent-swatch wiring + the reflect systems.
// ===========================================================================

/// On a [`ScreenRouter`] change, rebuild the inspector's per-screen content
/// (name/desc + chips + live-state skeleton). A change-detection exclusive system
/// (free at rest; the rebuild is a handful of despawns + spawns on each switch).
pub fn rebuild_inspector_on_switch(world: &mut World) {
    if !world.is_resource_changed::<ScreenRouter>() {
        return;
    }
    let screen = world.resource::<ScreenRouter>().0;
    rebuild_inspector_content(world, screen);
}

/// Map an accent swatch's `OnPress` to a [`SetAccent`] request: read the pressed
/// swatch's [`AccentSwatch`] color + write the swap (which re-themes the whole app
/// via `Theme`'s `is_changed()` re-extract). Ordinary `.after(Input)` collector.
pub fn route_accent_press(
    mut reader: MessageReader<OnPress>,
    swatches: Query<&AccentSwatch>,
    mut writer: MessageWriter<SetAccent>,
) {
    for OnPress(e) in reader.read() {
        if let Ok(AccentSwatch(color)) = swatches.get(*e) {
            writer.write(SetAccent(*color));
        }
    }
}

/// Reflect the live theme accent into the swatch selection ring: the swatch whose
/// `AccentSwatch` color matches the current `color.accent` gets the 2px white
/// border + the `shadow.swatch-selected` glow; the rest get the 2px
/// `border.default` and no glow (the design's `on` swatch style, JS 668). A
/// change-detection exclusive system (runs only when the theme changed — exactly
/// the `SetAccent` swap edge).
pub fn reflect_accent_selection(world: &mut World) {
    if !world.is_resource_changed::<Theme>() {
        return;
    }
    let Some(theme) = world.get_resource::<Theme>() else {
        return;
    };
    // The live accent base resolves through the typed `ThemeContract` (the
    // string-keyed `theme.color(ColorToken::Accent)` HashMap lookup is gone); every
    // token resolves, so there is no missing-color early-return anymore.
    let active = theme.resolve(ColorToken::Accent);
    let swatches: Vec<(Entity, Color)> = {
        let mut q = world.query::<(Entity, &AccentSwatch)>();
        q.iter(world).map(|(e, s)| (e, s.0)).collect()
    };
    for (swatch, color) in swatches {
        let selected = colors_match(color, active);
        set_swatch_selected(world, swatch, selected);
    }
}

/// Two accent colors match if their srgb u8 channels are equal (the design
/// compares `S.accent.toLowerCase()===hex`). Compares the swatch's literal color
/// to the theme's resolved `color.accent`.
fn colors_match(a: Color, b: Color) -> bool {
    let to_u8 = |c: Color| {
        let s = bevy::prelude::Srgba::from(c);
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        (q(s.red), q(s.green), q(s.blue))
    };
    to_u8(a) == to_u8(b)
}

/// Set a swatch's selected/unselected ring: selected = 2px white (`text.primary`)
/// border + the `shadow.swatch-selected` glow (`0 0 0 3px rgba(0,0,0,.4),
/// 0 4px 12px -4px <hex>` — values.md § 2; JS 668); unselected = 2px
/// `border.default`, no shadow.
///
/// The glow's colored layer is keyed to the swatch's own hex. Only the SELECTED
/// swatch shows the glow, and the selected swatch IS the live accent — so its
/// colored drop resolves to `color.accent`, exactly the swatch's own hex, with no
/// per-accent token needed. The black ring reuses `color.shadow.switch-thumb`
/// (`rgba(0,0,0,.4)` — the same alpha the design's ring uses).
fn set_swatch_selected(world: &mut World, swatch: Entity, selected: bool) {
    let side = if selected {
        solid_side(ColorToken::TextPrimary)
    } else {
        solid_side(ColorToken::BorderDefault)
    };
    if let Some(mut border) = world.get_mut::<Border>(swatch) {
        border.top = side.clone();
        border.right = side.clone();
        border.bottom = side.clone();
        border.left = side;
    }
    if selected {
        // The two-layer `shadow.swatch-selected`: a 3px `rgba(0,0,0,.4)` ring +
        // a colored drop using the (selected = current accent) hex.
        world.entity_mut(swatch).insert(BoxShadow(vec![
            Shadow {
                color: ColorToken::ShadowSwitchThumb,
                offset_x: Length::px(0.0),
                offset_y: Length::px(0.0),
                blur: Length::px(0.0),
                spread: Length::px(3.0),
                inset: false,
            },
            Shadow {
                color: ColorToken::Accent,
                offset_x: Length::px(0.0),
                offset_y: Length::px(4.0),
                blur: Length::px(12.0),
                spread: Length::px(-4.0),
                inset: false,
            },
        ]));
    } else {
        world.entity_mut(swatch).remove::<BoxShadow>();
    }
}

// ===========================================================================
// Small `&mut World` tree helpers (the inspector's marker lookups).
// ===========================================================================

/// The single entity carrying marker component `T` (the shell panel / a slot).
fn find_named<T: Component>(world: &mut World) -> Option<Entity> {
    let mut q = world.query_filtered::<Entity, With<T>>();
    q.iter(world).next()
}

/// The inspector slot entity matching `slot`.
fn find_slot(world: &mut World, slot: InspectorSlot) -> Option<Entity> {
    let mut q = world.query::<(Entity, &InspectorSlot)>();
    q.iter(world).find(|(_, s)| **s == slot).map(|(e, _)| e)
}

/// Despawn every direct child of `parent` (each `despawn()` recursively despawns
/// its subtree — Bevy 0.19 default — and removes it from `parent`'s `Children`).
/// The clear half of the switch rebuild.
fn despawn_children(world: &mut World, parent: Entity) {
    let children: Vec<Entity> = world
        .get::<Children>(parent)
        .into_iter()
        .flat_map(|c| c.iter().copied().collect::<Vec<Entity>>())
        .collect();
    for child in children {
        world.entity_mut(child).despawn();
    }
}

/// Rewrite an in-place slot leaf's `Text` (the name/desc leaves).
fn set_slot_text(world: &mut World, slot: InspectorSlot, s: &str) {
    if let Some(e) = find_slot(world, slot)
        && let Some(mut t) = world.get_mut::<Text>(e)
        && t.0 != s
    {
        t.0 = s.to_string();
    }
}

/// The `n`-th direct `Text`-bearing child of `parent` (0-based; the `stat_row`
/// value leaf is index 1 — `[key, value]`).
fn nth_text_child(world: &World, parent: Entity, n: usize) -> Option<Entity> {
    let children = world.get::<Children>(parent)?;
    children
        .iter()
        .copied()
        .filter(|&c| world.get::<Text>(c).is_some())
        .nth(n)
}

// ===========================================================================
// The InspectorPlugin.
// ===========================================================================

/// The inspector app logic, ordered so a switch / accent swap lands the SAME
/// frame it is requested:
///
/// - `rebuild_inspector_on_switch` runs **after** the shell's
///   [`apply_screen_router`](crate::shell::apply_screen_router) (which sets the
///   `ScreenRouter`), so the rebuild observes the router change this frame (not a
///   frame late) — then `update_inspector_live_state` fills the fresh rows
///   (chained after the rebuild).
/// - `route_accent_press` (the swatch `OnPress` → `SetAccent` collector) runs
///   `.after(BuiySet::Input).before(apply_set_accent)` so the swap it requests is
///   consumed THIS frame; `reflect_accent_selection` runs **after**
///   [`apply_set_accent`](buiy_core::theme::apply_set_accent) (the `ThemePlugin`
///   system that consumes `SetAccent`), so the swatch ring reflects the new
///   accent the same frame the swap lands.
///
/// The static frame is built by [`build_inspector_content`] (the
/// binary/capture/test call it after `build_shell`).
pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (rebuild_inspector_on_switch, update_inspector_live_state)
                .chain()
                .after(BuiySet::Input)
                .after(crate::shell::apply_screen_router),
        )
        .add_systems(
            Update,
            route_accent_press
                .after(BuiySet::Input)
                .before(buiy_core::theme::apply_set_accent),
        )
        .add_systems(
            Update,
            reflect_accent_selection.after(buiy_core::theme::apply_set_accent),
        );
    }
}
