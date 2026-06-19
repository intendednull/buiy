//! T7 selection + caret + placeholder painting — the paint-input state
//! components, token resolution, the blink writer (Task 2), selection
//! emission (Task 3), caret emission + damage (Task 4), placeholder
//! (Task 5). Spec: decoration-and-paint §§ 5–8; glyph-pipeline §§ 6.2–6.3.

use bevy::math::Rect;
use bevy::prelude::*;
use buiy_core::render::color::{
    CARET_COLOR_TOKEN, ColorToken, PLACEHOLDER_COLOR_TOKEN, SELECTION_BG_TOKEN, SELECTION_FG_TOKEN,
    resolve_caret_color, resolve_selection_bg, resolve_selection_fg,
};
use buiy_core::render::components::{CaretColor, TextColor};
use buiy_core::text::{CaretVisual, SelectionVisual};
use buiy_core::theme::{default_light_theme, forced_colors_theme};
use cosmic_text::Cursor;

// --- Task 1: components, tokens, resolution --------------------------------

#[test]
fn default_theme_carries_the_t7_tokens_but_not_color_caret() {
    let t = default_light_theme();
    assert!(t.color(SELECTION_BG_TOKEN).is_some(), "selection bg token");
    assert!(t.color(SELECTION_FG_TOKEN).is_some(), "selection fg token");
    assert!(
        t.color(PLACEHOLDER_COLOR_TOKEN).is_some(),
        "placeholder token"
    );
    // Decision 7: caret-color `auto` parity — NO default caret entry, the
    // chain falls through to currentColor.
    assert!(
        t.color(CARET_COLOR_TOKEN).is_none(),
        "no default caret token"
    );
}

#[test]
fn selection_colors_resolve_named_tokens_in_a_normal_theme() {
    let t = default_light_theme();
    assert_eq!(
        resolve_selection_bg(&t),
        t.color(SELECTION_BG_TOKEN).unwrap()
    );
    assert_eq!(
        resolve_selection_fg(&t),
        t.color(SELECTION_FG_TOKEN).unwrap()
    );
}

#[test]
fn selection_colors_prefer_system_keys_under_forced_colors() {
    // Decision 6: the wholesale forced-colors swap leaves no named tokens;
    // Highlight/HighlightText are the CSS ::selection system pair — the
    // resolve_token CurrentColor idiom, extended.
    use buiy_core::render::color::SystemColorKeyword;
    let t = forced_colors_theme();
    assert_eq!(
        resolve_selection_bg(&t),
        t.color(SystemColorKeyword::Highlight.token()).unwrap()
    );
    assert_eq!(
        resolve_selection_fg(&t),
        t.color(SystemColorKeyword::HighlightText.token()).unwrap()
    );
}

#[test]
fn caret_color_chain_explicit_then_theme_key_then_current() {
    let mut t = default_light_theme();
    let current = Color::srgb(0.1, 0.2, 0.3);

    // Tier 3: no explicit token, no theme key → currentColor.
    assert_eq!(resolve_caret_color(None, &t, current), current);

    // Tier 2: the theme caret key, when a theme opts in (presence check,
    // never a magenta miss).
    t.colors
        .insert(CARET_COLOR_TOKEN.into(), Color::srgb(0.9, 0.0, 0.0));
    assert_eq!(
        resolve_caret_color(None, &t, current),
        Color::srgb(0.9, 0.0, 0.0)
    );

    // Tier 1: an explicit CaretColor token wins over both.
    t.colors
        .insert("my.caret".into(), Color::srgb(0.0, 0.9, 0.0));
    let explicit = ColorToken::Token("my.caret".into());
    assert_eq!(
        resolve_caret_color(Some(&explicit), &t, current),
        Color::srgb(0.0, 0.9, 0.0)
    );
}

#[test]
fn caret_visual_defaults_visible_with_zero_rect() {
    // The § 6.3 shape verbatim; insert-visible matches "caret becomes
    // visible" on focus gain (editing § 10) and the t=0 blink phase.
    let cv = CaretVisual::default();
    assert!(cv.visible);
    assert_eq!(cv.rect, Rect::default());
}

#[test]
fn selection_visual_normalizes_on_construction() {
    // The selection_bounds() invariant (start ≤ end), enforced by the
    // ordered constructor so producer-side code never re-sorts.
    let a = Cursor::new(2, 5);
    let b = Cursor::new(1, 9);
    let sv = SelectionVisual::new(a, b);
    assert_eq!((sv.start.line, sv.start.index), (1, 9));
    assert_eq!((sv.end.line, sv.end.index), (2, 5));
    // Same line, indices swapped.
    let sv = SelectionVisual::new(Cursor::new(1, 7), Cursor::new(1, 3));
    assert_eq!((sv.start.index, sv.end.index), (3, 7));
    assert!(SelectionVisual::new(a, a).is_collapsed());
}

#[test]
fn text_color_placeholder_is_the_token_constructor() {
    let TextColor(token) = TextColor::placeholder();
    assert_eq!(token, ColorToken::Token(PLACEHOLDER_COLOR_TOKEN.into()));
}

use buiy_core::text::{CaretBlinkInterval, blink_phase};
use buiy_core::theme::UserPreferences;
use std::time::Duration;

// --- Task 2: blink_phase, the pure square wave ------------------------------

#[test]
fn blink_phase_is_a_square_wave_starting_visible() {
    let half = Duration::from_millis(500);
    assert!(blink_phase(Duration::ZERO, half), "t=0 visible");
    assert!(blink_phase(Duration::from_millis(499), half));
    assert!(
        !blink_phase(Duration::from_millis(500), half),
        "edge: hidden"
    );
    assert!(!blink_phase(Duration::from_millis(999), half));
    assert!(
        blink_phase(Duration::from_millis(1000), half),
        "full period"
    );
    // Zero interval = steady visible (defensive, documented).
    assert!(blink_phase(Duration::from_secs(7), Duration::ZERO));
    // Sub-microsecond half-period: !is_zero() but as_micros()==0 — must
    // not divide by zero (the nanos division; plan-snippet erratum).
    assert!(blink_phase(Duration::ZERO, Duration::from_nanos(500)));
}

// --- Task 2: the writer — edge-only, reduced-motion steady ------------------

/// Minimal headless app with the text plugin (the writer registers there)
/// and a change-tick probe counting `Changed<CaretVisual>` per frame.
fn blink_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::theme::ThemePlugin)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(buiy_core::layout::LayoutPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.init_resource::<CaretEdges>();
    // After Picking ⇒ after the writer — sees the settled value + tick.
    app.add_systems(Update, count_caret_edges.after(buiy_core::BuiySet::Picking));
    app
}

#[derive(Resource, Default)]
struct CaretEdges(usize);

fn count_caret_edges(q: Query<(), Changed<CaretVisual>>, mut edges: ResMut<CaretEdges>) {
    edges.0 += q.iter().count();
}

fn advance(app: &mut App, ms: u64) {
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(ms));
    app.update();
}

#[test]
fn blink_writes_only_on_phase_edges() {
    let mut app = blink_app();
    // The plugin-initialized default half-period — the edge math below
    // (500 ms flips) depends on it.
    assert_eq!(
        app.world().resource::<CaretBlinkInterval>().0,
        Duration::from_millis(500)
    );
    let e = app.world_mut().spawn(CaretVisual::default()).id();
    app.update(); // insertion frame: Added counts as one edge
    let after_spawn = app.world().resource::<CaretEdges>().0;
    assert!(after_spawn >= 1);

    // Mid-phase steps: the square wave does not flip → ZERO writes.
    advance(&mut app, 100);
    advance(&mut app, 100);
    assert_eq!(app.world().resource::<CaretEdges>().0, after_spawn);
    assert!(app.world().get::<CaretVisual>(e).unwrap().visible);

    // Cross the 500 ms edge → exactly one write, visible flips false.
    advance(&mut app, 400);
    assert_eq!(app.world().resource::<CaretEdges>().0, after_spawn + 1);
    assert!(!app.world().get::<CaretVisual>(e).unwrap().visible);

    // Steady in the hidden phase → zero writes again.
    advance(&mut app, 100);
    assert_eq!(app.world().resource::<CaretEdges>().0, after_spawn + 1);

    // The next edge flips back.
    advance(&mut app, 400);
    assert_eq!(app.world().resource::<CaretEdges>().0, after_spawn + 2);
    assert!(app.world().get::<CaretVisual>(e).unwrap().visible);
}

#[test]
fn reduced_motion_pins_steady_visible() {
    let mut app = blink_app();
    app.world_mut()
        .resource_mut::<UserPreferences>()
        .prefers_reduced_motion = true;
    let e = app.world_mut().spawn(CaretVisual::default()).id();
    app.update();
    let baseline = app.world().resource::<CaretEdges>().0;
    // Cross what would be three blink edges: no flips, no writes.
    for _ in 0..3 {
        advance(&mut app, 600);
        assert!(app.world().get::<CaretVisual>(e).unwrap().visible);
    }
    assert_eq!(app.world().resource::<CaretEdges>().0, baseline);
}

#[test]
fn reduced_motion_flip_during_hidden_phase_is_one_edge_to_visible() {
    let mut app = blink_app();
    let e = app.world_mut().spawn(CaretVisual::default()).id();
    app.update();
    advance(&mut app, 700); // into the hidden phase
    assert!(!app.world().get::<CaretVisual>(e).unwrap().visible);
    app.world_mut()
        .resource_mut::<UserPreferences>()
        .prefers_reduced_motion = true;
    app.update(); // the steady state takes over: one edge to true
    assert!(app.world().get::<CaretVisual>(e).unwrap().visible);
}

use crate::support::extract_harness::TextExtractHarness;
use buiy_core::render::extract::TextQuad;
use buiy_core::text::{ComputedTextLayout, FontSize, Text, TextBuffer, TextDecorations};
use buiy_core::theme::Theme;

const SEL_BG: Color = Color::srgb(1.0, 0.0, 0.0);
const SEL_FG: Color = Color::srgb(0.0, 0.0, 1.0);

fn set_selection_tokens(app: &mut App) {
    let mut theme = app.world_mut().resource_mut::<Theme>();
    theme.colors.insert(SELECTION_BG_TOKEN.into(), SEL_BG);
    theme.colors.insert(SELECTION_FG_TOKEN.into(), SEL_FG);
}

fn selection_quads(h: &TextExtractHarness) -> Vec<TextQuad> {
    h.text_quads()
        .quads
        .iter()
        .copied()
        .filter(|q| q.color == SEL_BG)
        .collect()
}

// --- Task 3: rect derivation — exact numbers against the same buffer -------

#[test]
fn selection_rects_match_highlight_spans_folded_by_origin() {
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    let e = h
        .app
        .world_mut()
        .spawn((
            buiy_core::Node,
            // Distinct nonzero padding per axis → content origin (7, 3):
            // the expected-vs-got compare below then actually constrains
            // the producer's per-axis origin fold. (With a (0, 0) origin
            // the fold terms are identities and the test is vacuous.)
            buiy_core::layout::Style::default()
                .padding_edges(buiy_core::layout::Edges::axis(7.0, 3.0)),
            Text("Hi there".into()),
            SelectionVisual::new(Cursor::new(0, 1), Cursor::new(0, 5)),
        ))
        .id();
    h.settle();

    // Expected: the SAME buffer, the SAME upstream API — proves the
    // producer's origin fold and seat plumbing (the span MATH is
    // upstream's; the line GATE is pinned by the multiline test below).
    let world = h.app.world();
    let buffer = world.get::<TextBuffer>(e).unwrap();
    let computed = world.get::<ComputedTextLayout>(e).unwrap();
    let origin = world
        .get::<GlobalTransform>(e)
        .unwrap()
        .translation()
        .truncate()
        + computed.content_offset;
    assert!(
        origin.x > 0.0 && origin.y > 0.0 && origin.x != origin.y,
        "fixture guard: a zero or axis-symmetric origin would make the \
         fold comparison vacuous, got {origin}"
    );
    let sel = world.get::<SelectionVisual>(e).unwrap();
    let mut expected = Vec::new();
    for run in buffer.buffer.layout_runs() {
        if run.line_i < sel.start.line || run.line_i > sel.end.line {
            continue;
        }
        for (x, w) in run.highlight(sel.start, sel.end) {
            expected.push((
                Vec2::new(origin.x + x, origin.y + run.line_top),
                Vec2::new(w, run.line_height),
            ));
        }
    }
    assert!(!expected.is_empty(), "the fixture selects something");
    let got: Vec<_> = selection_quads(&h)
        .iter()
        .map(|q| (q.position, q.size))
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn mixed_bidi_selection_yields_multiple_disjoint_rects() {
    // THE campaign contract (text.md:89): a logical range straddling the
    // BiDi boundary maps to ≥ 2 visually disjoint rects — upstream's
    // highlight() does the math, the producer must not flatten it.
    // "hello עולם world": bytes 0..6 "hello ", 6..14 עולם, 14.. " world".
    // Select mid-Hebrew → mid-"world": logical [10, 18).
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    crate::support::register_fixture_font(
        &mut h.app,
        "Noto Sans Hebrew",
        "NotoSansHebrew-hebrew.ttf",
    );
    h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default()
            .width_px(400.0)
            .height_px(100.0),
        Text("hello עולם world".into()),
        buiy_core::text::FontFamily(buiy_core::text::FontStack(vec![
            buiy_core::text::FamilyEntry::Named("Fira Sans".into()),
            buiy_core::text::FamilyEntry::Named("Noto Sans Hebrew".into()),
        ])),
        FontSize(20.0),
        SelectionVisual::new(Cursor::new(0, 10), Cursor::new(0, 18)),
    ));
    h.settle();

    let mut quads = selection_quads(&h);
    assert!(
        quads.len() >= 2,
        "mixed-BiDi selection must paint disjoint rects, got {}",
        quads.len()
    );
    // All on one line band, sorted by x, strictly disjoint with a gap.
    quads.sort_by(|a, b| a.position.x.total_cmp(&b.position.x));
    for pair in quads.windows(2) {
        assert_eq!(pair[0].position.y, pair[1].position.y, "one line");
        assert!(
            pair[0].position.x + pair[0].size.x < pair[1].position.x,
            "visually disjoint (the unselected Hebrew remainder sits between)"
        );
    }
}

// --- Task 3: the seat + the reference-render behaviors ----------------------

#[test]
fn selection_rects_precede_decoration_quads_for_the_entity() {
    // § 4.4 seat order: 2 (selection) < 3 (underline) — for the WHOLE
    // entity (decision 5's pre-pass), so the carrier segment is
    // [sel…, deco…].
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi there".into()),
        TextDecorations {
            line: buiy_core::text::DecorationLines::UNDERLINE,
            ..Default::default()
        },
        SelectionVisual::new(Cursor::new(0, 0), Cursor::new(0, 8)),
    ));
    h.settle();
    let quads = &h.text_quads().quads;
    let first_deco = quads
        .iter()
        .position(|q| q.color != SEL_BG)
        .expect("underline");
    let last_sel = quads
        .iter()
        .rposition(|q| q.color == SEL_BG)
        .expect("selection");
    assert!(
        last_sel < first_deco,
        "all selection quads before all decoration quads"
    );
}

#[test]
fn multiline_selection_extends_to_the_line_edge_and_fills_empty_lines() {
    // Upstream's reference behaviors (Orientation § 2): non-final selected
    // lines extend their last rect to the buffer width (LTR); a fully
    // selected INTERNAL empty line paints one full-width rect.
    //
    // Lines 0 ("xx") and 4 ("ef") sit OUTSIDE [start.line, end.line] on
    // each side — they exercise the producer's line gate (Orientation § 1:
    // upstream's highlight() predicate degenerates to all-selected on such
    // lines, so an ungated producer would paint extra full-line rects,
    // caught by the exact len() == 3 below) and the re-tint predicate's
    // two line terms (caught by the tint pattern at the end).
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    let e = h
        .app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default()
                .width_px(200.0)
                .height_px(120.0),
            // Pre-line keeps the \n\n: line 0 "xx", line 1 "ab",
            // line 2 "" (internal), line 3 "cd", line 4 "ef".
            Text("xx\nab\n\ncd\nef".into()),
            buiy_core::text::WhiteSpace::PreLine,
            SelectionVisual::new(Cursor::new(1, 1), Cursor::new(3, 1)),
        ))
        .id();
    h.settle();

    let world = h.app.world();
    let buffer = world.get::<TextBuffer>(e).unwrap();
    let width = buffer.buffer.size().0.expect("committed width");
    let computed = world.get::<ComputedTextLayout>(e).unwrap();
    let origin = world
        .get::<GlobalTransform>(e)
        .unwrap()
        .translation()
        .truncate()
        + computed.content_offset;
    let quads = selection_quads(&h);
    assert_eq!(quads.len(), 3, "one rect per SELECTED line, gated");
    // Line 1 (non-final): right edge == origin.x + buffer width.
    assert_eq!(quads[0].position.x + quads[0].size.x, origin.x + width);
    // Line 2 (internal empty): full width from x = origin.x.
    assert_eq!(quads[1].position.x, origin.x);
    assert_eq!(quads[1].size.x, width);
    // Line 3 (final): ends at the grapheme edge, NOT the line edge.
    assert!(quads[2].position.x + quads[2].size.x < origin.x + width);

    // Re-tint line terms: only glyphs on lines inside [start.line,
    // end.line] AND inside the byte range re-tint — 8 painted glyphs
    // x x | a b | (empty) | c d | e f, of which only 'b' and 'c' fall
    // inside the (1,1)-(3,1) selection.
    let fg = LinearRgba::from(SEL_FG);
    let fg = [fg.red, fg.green, fg.blue, fg.alpha];
    let tinted: Vec<bool> = h.glyphs().glyphs.iter().map(|g| g.color == fg).collect();
    assert_eq!(
        tinted,
        [false, false, false, true, true, false, false, false]
    );
}

#[test]
fn collapsed_selection_paints_nothing() {
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi".into()),
        SelectionVisual::new(Cursor::new(0, 1), Cursor::new(0, 1)),
    ));
    h.settle();
    assert!(selection_quads(&h).is_empty());
}

// --- Task 3: per-cluster re-tint --------------------------------------------

#[test]
fn selected_glyphs_re_tint_to_the_selection_fg() {
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    let e = h
        .app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Hi there".into()),
            // Select "i th" → bytes [1, 5): glyphs for 'i','t','h' re-tint
            // (the space emits no instance).
            SelectionVisual::new(Cursor::new(0, 1), Cursor::new(0, 5)),
        ))
        .id();
    h.settle();

    let fg = LinearRgba::from(SEL_FG);
    let fg = [fg.red, fg.green, fg.blue, fg.alpha];
    let tinted: Vec<bool> = h.glyphs().glyphs.iter().map(|g| g.color == fg).collect();
    // 7 painted glyphs ("Hithere"): H i t h e r e → i,t,h selected.
    assert_eq!(tinted, [false, true, true, true, false, false, false]);
    // The selection state never touches the atlas (re-tint is per-instance):
    // every uv in the rebuilt set is identical to a no-selection rebuild —
    // cheap proxy: remove the selection, re-settle, compare uv sequences.
    let uvs_selected: Vec<[f32; 4]> = h.glyphs().glyphs.iter().map(|g| g.uv).collect();
    h.app.world_mut().entity_mut(e).remove::<SelectionVisual>();
    h.frame();
    let uvs_plain: Vec<[f32; 4]> = h.glyphs().glyphs.iter().map(|g| g.uv).collect();
    assert_eq!(
        uvs_selected, uvs_plain,
        "the atlas/uv set is selection-invariant"
    );
}

// --- Task 3: damage --------------------------------------------------------

#[test]
fn selection_changes_fire_the_union_and_removal_clears() {
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    let e = h
        .app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Hi there".into()),
        ))
        .id();
    h.settle();
    let g0 = h.changed_frames();

    // Steady: no rebuild.
    h.frame();
    assert_eq!(h.changed_frames(), g0);

    // Insert → rebuild (Changed includes Added).
    h.app
        .world_mut()
        .entity_mut(e)
        .insert(SelectionVisual::new(Cursor::new(0, 0), Cursor::new(0, 2)));
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 1);
    assert!(!selection_quads(&h).is_empty());

    // Mutate endpoints → rebuild.
    h.app.world_mut().get_mut::<SelectionVisual>(e).unwrap().end = Cursor::new(0, 5);
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 2);

    // Remove → the removal stream fires a rebuild that clears the rects.
    h.app.world_mut().entity_mut(e).remove::<SelectionVisual>();
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 3);
    assert!(selection_quads(&h).is_empty());
    // And back to steady.
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 3);
}

use buiy_core::render::atlas::GlyphAlphaInstance;
use buiy_core::text::{caret_stamp_rect, solid_stamp_key};

/// The caret instance: a stamp (uv min == max — the midpoint-replicated
/// uv_rect) emitted LAST for the entity (seat 6).
fn caret_instance(h: &TextExtractHarness) -> Option<GlyphAlphaInstance> {
    h.glyphs()
        .glyphs
        .last()
        .copied()
        .filter(|g| g.uv[0] == g.uv[2] && g.uv[1] == g.uv[3])
}

// --- Task 4: geometry, color chain, seat ------------------------------------

#[test]
fn caret_emits_one_snapped_stamp_after_all_glyphs() {
    let mut h = TextExtractHarness::new();
    let e = h
        .app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Hi".into()),
            CaretVisual {
                visible: true,
                rect: Rect::new(12.3, 0.0, 13.3, 19.2),
            },
        ))
        .id();
    h.settle();

    let world = h.app.world();
    let computed = world.get::<ComputedTextLayout>(e).unwrap();
    let origin = world
        .get::<GlobalTransform>(e)
        .unwrap()
        .translation()
        .truncate()
        + computed.content_offset;
    let caret = caret_instance(&h).expect("a caret stamp");
    // § 6.1 + § 3.3 via the pure helper (its own snap math is unit-pinned
    // in visual.rs) — this asserts the producer composes it correctly.
    assert_eq!(
        caret.rect,
        caret_stamp_rect(origin, Rect::new(12.3, 0.0, 13.3, 19.2), 1.0)
    );
    // Seat 6: the caret is the LAST instance for the entity.
    assert_eq!(
        h.glyphs().glyphs.iter().position(|g| g.rect == caret.rect),
        Some(h.glyph_count() - 1)
    );
    // § 6.3 residency: the stamp key joined the touch-pass set.
    assert!(h.resident_keys().contains(&solid_stamp_key()));
    // caret-color: auto ⇒ the entity's resolved foreground
    // (color.text.primary in the default theme).
    let auto = LinearRgba::from(
        h.app
            .world()
            .resource::<Theme>()
            .color("color.text.primary")
            .unwrap(),
    );
    assert_eq!(caret.color, [auto.red, auto.green, auto.blue, auto.alpha]);
}

#[test]
fn caret_color_chain_resolves_at_emission() {
    let mut h = TextExtractHarness::new();
    let e = h
        .app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Hi".into()),
            CaretVisual {
                visible: true,
                rect: Rect::new(0.0, 0.0, 1.0, 19.2),
            },
        ))
        .id();
    h.settle();

    // Tier 2: a theme that opts into color.caret re-tints on theme change
    // (theme.is_changed() is already in the union).
    h.app
        .world_mut()
        .resource_mut::<Theme>()
        .colors
        .insert(CARET_COLOR_TOKEN.into(), Color::srgb(0.9, 0.0, 0.0));
    h.frame();
    let red = LinearRgba::from(Color::srgb(0.9, 0.0, 0.0));
    assert_eq!(caret_instance(&h).unwrap().color[0], red.red);

    // Tier 1: an explicit CaretColor wins.
    h.app
        .world_mut()
        .resource_mut::<Theme>()
        .colors
        .insert("my.caret".into(), Color::srgb(0.0, 0.9, 0.0));
    h.app
        .world_mut()
        .entity_mut(e)
        .insert(CaretColor(ColorToken::Token("my.caret".into())));
    h.frame();
    let green = LinearRgba::from(Color::srgb(0.0, 0.9, 0.0));
    assert_eq!(caret_instance(&h).unwrap().color[1], green.green);
}

#[test]
fn invisible_or_removed_caret_emits_nothing() {
    // The plan snippet authored `visible` directly (spawn false at t≈0,
    // write true mid-hidden-phase) — but the Task-2 writer OWNS `visible`
    // and flips both writes back to the blink phase in the same Update,
    // before extract ever reads them (a plan erratum, noted in the commit
    // body). Drive visibility through the writer's own clock instead: the
    // hidden phase IS the invisible-caret fixture, and the writer's flip
    // IS the Changed<CaretVisual> edge the producer must consume.
    let mut h = TextExtractHarness::new();
    let e = h
        .app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Hi".into()),
            CaretVisual {
                visible: true,
                rect: Rect::new(0.0, 0.0, 1.0, 19.2),
            },
        ))
        .id();
    h.settle();
    assert!(caret_instance(&h).is_some());

    // Into the hidden phase: the writer flips `visible` off → the
    // producer emits no stamp.
    h.app
        .world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(600));
    h.frame();
    assert!(!h.app.world().get::<CaretVisual>(e).unwrap().visible);
    assert!(caret_instance(&h).is_none());

    // Back to the visible phase: the Changed member fires, the stamp
    // reappears.
    h.app
        .world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(500));
    h.frame();
    assert!(caret_instance(&h).is_some());

    // REMOVAL hides (focus loss) — the removal stream fires the rebuild.
    h.app.world_mut().entity_mut(e).remove::<CaretVisual>();
    h.frame();
    assert!(caret_instance(&h).is_none());
}

#[test]
fn empty_text_still_carries_a_caret() {
    // Text("") has a synthetic glyph-less run (T3 erratum 5) and no glyph
    // instances — the caret is rect-authored, so it paints regardless.
    let mut h = TextExtractHarness::new();
    h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text(String::new()),
        CaretVisual {
            visible: true,
            rect: Rect::new(0.0, 0.0, 1.0, 19.2),
        },
    ));
    h.settle();
    assert_eq!(h.glyph_count(), 1, "exactly the caret stamp");
    assert!(caret_instance(&h).is_some());
}

// --- Task 4: THE damage contract (§ 6.3 / verification § 1.2) ---------------

#[test]
fn blink_edges_rebuild_glyphs_only_and_steady_phases_rebuild_nothing() {
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    // A fixture with BOTH carriers live: an underline (quad tier) + a
    // caret (glyph tier) — so the test can see one move without the other.
    h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi there".into()),
        TextDecorations {
            line: buiy_core::text::DecorationLines::UNDERLINE,
            ..Default::default()
        },
        CaretVisual {
            visible: true,
            rect: Rect::new(2.0, 0.0, 3.0, 19.2),
        },
    ));
    h.settle();
    let g0 = h.changed_frames();
    let q0 = h.quad_changed_frames();
    let quads_before = h.text_quads().quads.clone();

    // "No blink edge → zero producer reruns": mid-phase virtual-clock
    // steps flip nothing, the writer writes nothing, the union stays
    // cold, NEITHER carrier is touched.
    for _ in 0..3 {
        h.app
            .world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(Duration::from_millis(100));
        h.frame();
    }
    assert_eq!(h.changed_frames(), g0, "no edge → no glyph rebuild");
    assert_eq!(h.quad_changed_frames(), q0, "no edge → no quad rebuild");

    // Cross the edge: the writer flips visible → Changed<CaretVisual>
    // fires → ONE glyph rebuild (the caret stamp drops out)… and the
    // value-compared publish leaves the quad carrier UNTOUCHED (its
    // content is identical) — the § 6.3 damage property, CPU half.
    h.app
        .world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(300));
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 1, "blink edge → glyph rebuild");
    assert_eq!(h.quad_changed_frames(), q0, "blink edge → quads RETAINED");
    assert_eq!(h.text_quads().quads, quads_before, "quad content identical");
    assert!(caret_instance(&h).is_none(), "hidden phase: no stamp");

    // And back: the next edge re-emits the stamp, quads still retained.
    h.app
        .world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(500));
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 2);
    assert_eq!(h.quad_changed_frames(), q0);
    assert!(caret_instance(&h).is_some());
}

// --- Task 5: ::placeholder — same pipeline, one tint ------------------------

#[test]
fn placeholder_is_identical_to_normal_text_except_color() {
    // § 7: same Buffer machinery, same producer, same seats — the ONLY
    // difference is the foreground token. Two identical fixtures, one
    // with TextColor::placeholder(): instance streams must differ in
    // `color` alone (rect/uv/clip/page identical), proving no second
    // paint path exists to keep correct.
    let spawn = |h: &mut TextExtractHarness, color: TextColor| {
        h.app
            .world_mut()
            .spawn((
                buiy_core::Node,
                buiy_core::layout::Style::default(),
                Text("Search…".into()),
                color,
            ))
            .id()
    };

    let mut normal = TextExtractHarness::new();
    spawn(&mut normal, TextColor::default());
    normal.settle();

    let mut placeholder = TextExtractHarness::new();
    spawn(&mut placeholder, TextColor::placeholder());
    placeholder.settle();

    let a = &normal.glyphs().glyphs;
    let b = &placeholder.glyphs().glyphs;
    assert_eq!(a.len(), b.len());
    let expected = {
        let theme = placeholder.app.world().resource::<Theme>();
        let lin = LinearRgba::from(theme.color(PLACEHOLDER_COLOR_TOKEN).unwrap());
        [lin.red, lin.green, lin.blue, lin.alpha]
    };
    for (x, y) in a.iter().zip(b) {
        assert_eq!(x.rect, y.rect, "same geometry");
        assert_eq!(x.uv, y.uv, "same atlas cells — the atlas is tint-blind");
        assert_eq!(x.clip, y.clip);
        assert_eq!(x.page, y.page);
        assert_eq!(y.color, expected, "the one difference: the tint");
    }
}
