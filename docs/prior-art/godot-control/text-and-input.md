**Date:** 2026-05-22
**Status:** active
**Subject:** Godot Control — text rendering (TextServer, HarfBuzz, ICU, BiDi from 4.0), RichTextLabel + BBCode, input routing, IME

# Text and input

Godot's text and input story bifurcates around the **Godot 4.0 release (March 2023)**. Pre-4.0 (Godot 1.x, 2.x, 3.x — i.e., January 2014 through Q1 2023), text rendering was a thin wrapper around FreeType with no BiDi, no complex-script support, no proper font fallback, no IME. Post-4.0, Godot has its own `TextServer` abstraction with HarfBuzz shaping, ICU BiDi, decoupled font sizes, multi-level fallback, and a pluggable backend interface. The nine-year gap is real and Buiy should understand both sides.

## The TextServer abstraction (4.0+)

Godot 4.0 introduced [`TextServer`](https://docs.godotengine.org/en/stable/classes/class_textserver.html) — an abstract interface over text shaping, layout, and rasterization. Two implementations ship:

- **`TextServerAdvanced`** — the full implementation. Uses **HarfBuzz** for shaping (ligatures, kerning, contextual alternates, OpenType features), **ICU** for BiDi (UAX #9) + word/sentence/grapheme break iteration, **FreeType** for rasterization. Adds ~5 MB to the binary. This is the default.
- **`TextServerFallback`** — minimal implementation. No HarfBuzz, no ICU. Latin-only, no BiDi, no complex scripts. Used to shrink mobile / web exports when the project doesn't need international text.

The TextServer choice is project-level configuration (`Project Settings > General > Text > Text Server`). Switching is a recompile or a config-flag change; per-platform overrides are supported.

## What landed in Godot 4.0

The TextServer work was led by [Pāvels Nadtočajevs (@bruvzg)](https://github.com/bruvzg) — the same contributor who later landed the AccessKit integration in 4.5. From the [4.0 release announcement](https://godotengine.org/article/godot-4-0-sets-sail/):

- **BiDi support** — right-to-left text (Arabic, Hebrew) renders correctly with proper bidirectional reordering.
- **Complex graphemes** — Devanagari, Tamil, Bengali, Khmer, Thai, Lao scripts with proper glyph clustering.
- **Ligatures** — OpenType `liga`, `dlig`, `clig`, `hlig` features supported.
- **Multi-level font fallback** — primary font → fallback chain → emoji font → tofu, all transparent to the developer.
- **Decoupled font sizes** — Font is the typeface; FontSize is the size; they're separate theme items now (see [`theme-and-styling.md`](theme-and-styling.md)).
- **Pseudolocalization** — a built-in transform for testing UI against longer-translated text without writing translations.
- **Color emoji** — embedded color emoji rendering via the `EMOJI` font fallback.
- **Variable fonts** — OpenType variations (`wght`, `wdth`, `ital`) supported via FontVariation resource.

The contributor's [GodotCon talk](https://godotengine.org/article/godotcon-talk-text-rendering/) is the canonical deep-dive reference; the engine community considers this work a major Godot 4 milestone.

## RichTextLabel and BBCode

Rich text in Godot is `RichTextLabel` + **BBCode markup**. BBCode is a 1990s forum-software syntax (`[b]bold[/b]`, `[color=red]text[/color]`, `[url=...]link[/url]`) — Godot inherits the syntax because it pre-dated when modern markdown / HTML alternatives stabilized. The choice is divergent from web norms but battle-tested at this point.

Supported tags (subset — full list in [`bbcode_in_richtextlabel`](https://docs.godotengine.org/en/stable/tutorials/ui/bbcode_in_richtextlabel.html)):

- **Text styling:** `[b]` bold, `[i]` italic, `[u]` underline, `[s]` strikethrough, `[code]`, `[lb]` / `[rb]` to escape brackets.
- **Color:** `[color=red]`, `[bgcolor=blue]`, `[outline_color=...]`.
- **Font:** `[font=res://...]`, `[font_size=24]`.
- **Layout:** `[center]`, `[right]`, `[fill]`, `[p]`, `[indent]`.
- **Lists:** `[ul]`, `[ol]`, `[ul bullet=disc]`.
- **Tables:** `[table=3]` with `[cell]` children.
- **Images:** `[img]res://icon.png[/img]`, with width/height/region/color attributes.
- **Hyperlinks:** `[url=https://...]link text[/url]` — fires `meta_clicked` signal.
- **Effects:** `[rainbow]`, `[wave]`, `[shake]`, `[tornado]`, `[fade]`, `[pulse]`, and custom effects via subclassing `RichTextEffect`.

**RichTextLabel is display-only — not editable.** A rich-text *editor* in Godot does not exist; user code that needs rich-text editing rolls its own on top of TextEdit (plain text) plus a sidecar markup state, or uses `RichTextLabel` for preview only.

### What BBCode lacks vs HTML / Markdown

- No semantic tags (`<article>`, `<aside>`, `<nav>`, `<h1>`-`<h6>`). All structure is presentational.
- No CSS-style stylesheet for BBCode. Each tag is parsed and rendered immediately; no separation of structure and style.
- No accessibility metadata. Until 4.5's AccessKit work, `[url]` links did not announce as links to screen readers (and screen readers were absent from Godot anyway).
- No content security model for user-supplied BBCode. `[img]` with arbitrary `res://` paths means user-supplied BBCode in a chat application could exfiltrate texture loads; user input must escape `[` / `]` via `[lb]` / `[rb]`.
- No entangled (overlapping) tags. `[b]bold[i]bold-italic[/b]italic[/i]` is illegal; tags must nest cleanly.

## Plain text editing: LineEdit, TextEdit, CodeEdit

- **LineEdit** is the single-line input. IME-aware (4.0+), undo / redo, selection, clipboard, placeholder, secret mode, max-length.
- **TextEdit** is the multi-line plain-text editor. Caret management (single + multi-caret), undo / redo with grouping, search and replace, word-wrap, gutters (line numbers, fold markers), drag-and-drop selection.
- **CodeEdit** extends TextEdit with syntax-highlighting hooks, code completion popups, line folding, bookmarks, breakpoints, and the `indent_size` / `indent_use_spaces` / `auto_brace_completion_enabled` settings.

The script editor and shader editor in Godot are built directly on CodeEdit — production dogfooding at editor scale. CodeEdit's `SyntaxHighlighter` resource is a pluggable interface (`CodeHighlighter` for code-style highlighting, `EditorSyntaxHighlighter` for editor-aware coloring). Third-party language plugins implement this.

## IME (Input Method Editor)

IME is supported from 4.0+ in LineEdit, TextEdit, CodeEdit. The OS-level IME (Windows IMM/TSF, macOS NSTextInputContext, Linux IBus / Fcitx, Wayland text-input-v3) routes through Godot's platform layer to the focused text widget; preedit text is rendered inline with the configured marker style. Pre-4.0 IME was severely limited.

## Input routing through Control

Input flows OS → `DisplayServer` → `Input` singleton → `Viewport` → GUI manager (see [`architecture.md`](architecture.md)). Each Control's `_gui_input(event)` is called for events that hit-test inside its rect, in reverse Z-order. `accept_event()` stops propagation; `mouse_filter` controls whether events bubble.

Keyboard input flows to the focused Control via the focus pointer. Focus is set by:

- Click on a Control with `focus_mode = FOCUS_CLICK` or `FOCUS_ALL`.
- Tab / Shift+Tab (sequential navigation via `focus_next` / `focus_previous` properties).
- Arrow keys (spatial navigation via `focus_neighbor_left/top/right/bottom`).
- Programmatically via `grab_focus()`.

The keyboard contracts for built-in widgets (e.g., arrow keys in PopupMenu, Enter to activate Button, Escape to close Popup) are baked into each Control's `_gui_input()` handler. No central APG-style keyboard contract registry; each widget enforces its own.

## Gamepad navigation

Godot 4.x supports gamepad-driven UI navigation through the Input singleton's `ui_left`, `ui_right`, `ui_up`, `ui_down`, `ui_accept`, `ui_cancel` actions, configured in `Project Settings > Input Map`. The GUI manager translates these into focus moves via the focus-neighbor chain. Auto-neighbor inference is supported but the manual wires remain canonical.

## Drag and drop

`Control._get_drag_data(at_position)` returns a Variant that becomes the drag payload. `Control._can_drop_data(at_position, data)` returns whether a drop is acceptable. `Control._drop_data(at_position, data)` handles the drop. Visual drag preview is set via `set_drag_preview(preview_control)`. The system is functional but has no WCAG 2.5.7 keyboard-alternative contract — a known gap (see [`accessibility.md`](accessibility.md)).

## Implications for Buiy

- **Validates Buiy's commit to cosmic-text.** Godot's TextServerAdvanced (HarfBuzz + ICU + FreeType) is the same recipe cosmic-text uses (harfrust + ICU bidi + swash + skrifa); both produce comparable output. The fact that Godot ships this stack across desktop / mobile / web / consoles validates the underlying primitives.
- **The 9-year BiDi gap is the cautionary tale.** Godot 1.0 → 3.x shipped Latin-only-ish text for **9 years**. Adding BiDi + complex scripts in 4.0 was a major engineering effort that broke API and required a full TextServer redesign. Buiy's foundation [`text.md`](../../specs/2026-05-07-buiy-foundation/text.md) commits to BiDi + complex scripts from v1 — Godot's history is the reason to.
- **BBCode is divergent from web norms.** Buiy's foundation `text.md` doesn't commit to a markup language for rich text; if Buiy ships a rich-text widget, **don't pick BBCode** — pick something HTML-or-Markdown-shaped that a11y bridges already understand. RichTextLabel-as-display-only-no-editor is also a real limitation Buiy should not inherit.
- **Plain-text-editor parity at the CodeEdit level is the high bar.** Buiy's `buiy-text-editing-design` sub-spec should treat CodeEdit's feature set (syntax-highlighter resource, code completion, multi-caret, folding) as the "complete editor" reference. Buiy doesn't need to ship a code editor at v1, but the text-editing primitive should not box itself out of that target.
- **Focus-neighbor manual wires are clean but tedious.** Godot's `focus_neighbor_left/top/right/bottom` properties give precise control but require per-widget setup. Buiy's foundation `interaction.md` commits to roving-tabindex and `aria-activedescendant`; the focus-neighbor primitive is worth keeping as an explicit escape hatch for game HUDs where DOM-order doesn't match visual order.
- **Drag-and-drop without WCAG 2.5.7 alternatives is a hard miss.** Buiy's foundation `accessibility.md` § 3.11 names the drag-replacement contract explicitly; Godot 4.5's AccessKit work doesn't (yet) address drag-and-drop a11y. Don't replicate the gap.

## Sources

- TextServer class reference — https://docs.godotengine.org/en/stable/classes/class_textserver.html
- Internationalizing games (BiDi, RTL, complex script) — https://docs.godotengine.org/en/stable/tutorials/i18n/internationalizing_games.html
- Godot 4.0 release announcement (text rendering details) — https://godotengine.org/article/godot-4-0-sets-sail/
- BBCode in RichTextLabel — https://docs.godotengine.org/en/stable/tutorials/ui/bbcode_in_richtextlabel.html
- RichTextLabel class reference — https://docs.godotengine.org/en/stable/classes/class_richtextlabel.html
- LineEdit, TextEdit, CodeEdit class refs — https://docs.godotengine.org/en/stable/classes/class_lineedit.html etc.
- `scene/resources/text_server.cpp` — https://github.com/godotengine/godot/blob/master/servers/text_server.cpp
- Buiy text spec — [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)
