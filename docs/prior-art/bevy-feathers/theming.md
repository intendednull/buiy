**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — Theming model: UiTheme resource, token taxonomy, dark-only default, OS-preference status

# Theming model

bevy_feathers's theming is a single-resource token map with observer-driven application to entities. The full surface area lives in three files: `src/theme.rs`, `src/tokens.rs`, `src/dark_theme.rs` (plus `src/palette.rs` for the base color constants). The model is simple, fully reactive, and **deliberately narrow** — no scales for spacing or motion, no light variant, no OS-preference binding.

## The `UiTheme` resource

```rust
// Sketch from src/theme.rs
pub struct ThemeToken(SmolStr);

pub struct ThemeProps {
    pub color: HashMap<ThemeToken, Color>,
}

#[derive(Resource)]
pub struct UiTheme(pub ThemeProps);

impl UiTheme {
    pub fn color(&self, token: &str) -> Color { /* lookup */ }
    pub fn set_color(&mut self, token: &str, color: Color) { /* insert */ }
}
```

The token map is **color-only**. There is no `space`, `radius`, `motion`, or `typography` scale on `UiTheme`. Spacings/sizes live in `src/constants.rs` as hardcoded `f32`s (`ROW_HEIGHT=24.0`, `CHECKBOX_SIZE=18.0`, `MEDIUM_FONT=14.0`, etc.); border radii live as a `RoundedCorners` enum + a separate `r: f32` passed in; font choice lives in `InheritableFont`. These are not customizable through `UiTheme`.

## Theme-component observers

Entities consume the theme by attaching one of:

- **`ThemeBackgroundColor(token)`** — observer writes `BackgroundColor` from theme.
- **`ThemeBorderColor(token)`** — observer writes `BorderColor` (all four edges).
- **`ThemeTextColor(token)`** — observer writes `TextColor` on a single text span (non-inherited).
- **`InheritableThemeTextColor(token)`** — observer propagates `TextColor` downward via `Propagate<TextColor>` to `ThemedText`-marked descendants.

Plus the `update_theme` system, which iterates **every** themed entity on `UiTheme` change. This is intentional — theme hot-swap rewrites all colors in one pass. Change-detection is the gating mechanism.

## Token taxonomy — what `src/tokens.rs` exports

Token names are plain `&'static str` constants. The full set (verified against `src/tokens.rs` HEAD):

**Surface / chrome**
- `WINDOW_BG` — top-level window background.
- `FOCUS_RING` — focus outline color.
- `TEXT_MAIN`, `TEXT_DIM` — primary text + de-emphasized text.

**Button** (three variants × four states × ~two text states)
- Normal: `BUTTON_BG`, `BUTTON_BG_HOVER`, `BUTTON_BG_PRESSED`, `BUTTON_BG_DISABLED`, `BUTTON_TEXT`, `BUTTON_TEXT_DISABLED`.
- Primary: `BUTTON_PRIMARY_*` with same suffixes.
- Plain: `BUTTON_PLAIN_BG{,_HOVER,_PRESSED,_DISABLED}` (no text variant — plain button inherits `TEXT_MAIN`).

**Slider** (background + bar + text)
- `SLIDER_BG{,_HOVER,_PRESSED,_DISABLED}`
- `SLIDER_BAR{,_HOVER,_PRESSED,_DISABLED}`
- `SLIDER_TEXT{,_DISABLED}`

**Scrollbar**, **Checkbox**, **Radio**, **Switch** — all follow the same state suffix pattern (`*_BG`, `*_BG_HOVER`, `*_BG_PRESSED`, `*_BG_DISABLED`, plus `*_CHECKED` variants for the binary widgets).

**Text input**
- `TEXT_INPUT_BG`, `TEXT_INPUT_TEXT`, `TEXT_INPUT_CURSOR`, `TEXT_INPUT_SELECTION`.

**Number input axis sigils** (for the 3D-editor-style X/Y/Z colored strip).

**Containers**
- `PANE_HEADER_BG`, `PANE_BODY_BG`, `PANE_BORDER`, `PANE_TEXT` — same pattern for `SUBPANE_*` and `GROUP_*`.

**Menu**
- `MENU_BG`, `MENU_ITEM_BG{,_HOVER,_PRESSED}`, `MENU_DIVIDER`, `MENU_TEXT`.

Total: ~100+ tokens. The scheme is **flat namespace strings** (no nested struct), with state encoded as a suffix. Cosmetic note: token names are `SCREAMING_SNAKE_CASE` and there is no language-level enforcement that an entity's `ThemeBackgroundColor(...)` token is *actually* a background token — typos silently miss and the observer falls back to a default color.

## Light vs dark variants

**There is no light theme in `bevy_feathers` source.** Only `src/dark_theme.rs` (`create_dark_theme()`) exists. The `UiTheme` resource defaults to the dark theme via `init_resource`. To use a different variant, an app must construct its own `UiTheme` (populate the token map manually) and `app.insert_resource(custom_theme)`.

A high-contrast variant likewise does not exist. A user-supplied light or high-contrast theme is just another token table — feathers does not ship one.

Buiy's foundation [architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to `light` / `dark` / `high-contrast` variants out of the box, with the default theme passing WCAG 2.2 AA contrast by construction. This is a deliberate point of departure.

## Type-scale and spacing tokens

**Not present in `UiTheme`.** Font sizes are constants in `src/constants.rs` (`MEDIUM_FONT=14`, `COMPACT_FONT=13`, `SMALL_FONT=12`, `EXTRA_SMALL_FONT=11`). Container sizes are constants (`ROW_HEIGHT=24`, `HEADER_HEIGHT=30`, `CHECKBOX_SIZE=18`, etc.). Spacing between elements is encoded directly in widget scenes via Taffy properties on `Node`.

This is a real limitation. A consumer wanting a denser or sparser feathers (e.g. for a touch-driven editor) cannot retheme — they must fork the widget scenes. Buiy's foundation commits to `space.*`, `radius.*`, `motion.*`, `typography.*` token scales (foundation [architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md), [cross-cutting.md § 3.14](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).

## Motion tokens

**Not present.** Feathers has no animation primitives, no transition tokens, no reduced-motion gating. The only "motion" in the kit is the disclosure-toggle chevron's `Rot2::turn_fraction(0.25)` rotation, which is not a timed animation — it's an instant rewrite tied to the `Checked` state change.

## OS preference binding — `prefers-color-scheme`, `prefers-contrast`, `forced-colors`, `prefers-reduced-motion`

**Not bound.** Feathers does not consult winit / OS settings for any of these. The `prefers-reduced-motion` query is irrelevant because feathers has no motion to reduce. `prefers-color-scheme` is irrelevant because feathers ships only a dark theme. `forced-colors` (Windows High Contrast) and `prefers-contrast` have no integration.

Buiy's foundation [cross-cutting.md § 3.14](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) commits to all `prefers-*` queries surfaced as a `UserPreferences` resource, with forced-colors triggering a palette swap to system colors and reduced-motion short-circuiting animations.

## Custom theme override

The intended override path: construct a `UiTheme` with the tokens the app cares about and `app.insert_resource(my_theme)`. Tokens absent from the custom map fall through to whatever default the theme observer assumes (verify behavior — there's a real risk of fallback-to-magenta-or-similar for missing tokens).

There is no subtree-scoped theme — the `UiTheme` is a global resource. A pane that wants a darker / lighter sub-look has to override individual `ThemeBackgroundColor` components on its own entities. Buiy's foundation supports per-subtree `Theme` components ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md), [cross-cutting.md § 3.14](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).

There is no hot-reload of themes as assets. The `UiTheme` is a runtime resource; reloading on edit requires the app to rebuild it.

## Comparison to Buiy's planned token system

|                          | bevy_feathers                          | Buiy foundation spec                          |
|--------------------------|----------------------------------------|-----------------------------------------------|
| Scope                    | Color only                             | Color + spacing + radius + typography + motion + elevation |
| Variants shipped         | Dark only                              | Light + dark + high-contrast + user-defined  |
| Default contrast         | Not audited to WCAG 2.2 AA             | WCAG 2.2 AA by construction (foundation § 2.5)|
| OS preference binding    | None                                   | `prefers-color-scheme`, `prefers-contrast`, `forced-colors`, `prefers-reduced-motion`, `prefers-reduced-transparency`, `inverted-colors` — all bound to `UserPreferences` resource |
| Hot-reload               | No (resource, not asset)               | Yes (theme is an asset)                       |
| Subtree override         | No (single global resource)            | Yes (`Theme` component on a subtree)          |
| Contrast linter          | None                                   | CI-gated + dev-time live checker              |
| Token namespacing        | Flat `SCREAMING_SNAKE_CASE` strings    | Hierarchical (`color.surface.primary`, `space.4`, `radius.md`, `motion.fast`) |
| Missing-token behavior   | Silent fallback                        | Hard error at load (contrast linter + load-time validation) |

## Implications for Buiy

Feathers's theming is consistent with its scope (a dark editor look). It is **not** a model Buiy can adopt as-is — Buiy's WCAG 2.2 AA commitment, OS-preference binding, and asset-based hot reload all require structurally different machinery. What's worth borrowing:

1. **Token-as-string-key lookup with observer-driven reaction.** Simple, fully reactive, change-detection-friendly. Buiy's theme system can use the same shape but with hierarchical tokens and a typed-token API for compile-time safety.
2. **Component-as-binding pattern** (`ThemeBackgroundColor(token)` etc.). The decomposition of one component per styled property maps directly onto Buiy's decomposed-component philosophy ([foundation architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)).
3. **Resource + observer**, not stylesheet selectors. Feathers shows the resource-based path scales fine for ~100 tokens; Buiy stays in that lane (no CSS-flavored stylesheet in foundation; see [cross-cutting.md § 3.14](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).

What to **avoid**:

- Flat string namespace with no type guard.
- Color-only scope (lock spacing/typography/motion to constants).
- Dark-only default with no variants.
- Global-resource-only (no subtree override).
- No hot-reload, no contrast linter, no OS-pref binding.

## Sources

- `theme.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/theme.rs
- `tokens.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/tokens.rs
- `dark_theme.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/dark_theme.rs
- `palette.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/palette.rs
- `constants.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/constants.rs
- Buiy foundation theming — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.5
- Buiy foundation cross-cutting (theming + user preferences) — [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) § 3.14
- Cross-link: [architecture.md](architecture.md), [accessibility.md](accessibility.md)
