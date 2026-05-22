**Date:** 2026-05-22
**Status:** active
**Subject:** Godot Control — critiques and open problems: anchor+margin friction, 11-year a11y gap, BiDi added late (4.0), RichTextLabel limitations, performance at scale, missing CSS primitives

# Critiques and open problems

Godot Control is a real, shipping, indie-dominant UI stack with twelve years of production polish. It is also a stack with substantial known limitations that Buiy's foundation explicitly chooses to *not* inherit. This file enumerates the critiques honestly — most are recognized by the Godot core team and tracked as `godot-proposals` issues; the question is whether they are designed-around or planned-to-fix.

## Layout: anchor + offset is non-intuitive vs CSS

The most frequent first-week-with-Godot critique on forums, Reddit, and Stack Overflow: the anchor + offset layout model is **non-obvious for developers coming from CSS / web**. Specific reports:

- **Anchor semantics don't match intuition.** `anchor_left = 1.0` means "left edge is at parent's right side, so this Control's *left* is on the *right*" — readable once you've internalized it; counter-intuitive at first reading.
- **Containers overwrite anchors silently.** A novice sets anchors on a Control inside an HBoxContainer, expecting them to take effect; the container ignores them and the novice can't figure out why their layout isn't responding. The Godot 4 LayoutMode enum + the inspector switching between "ANCHORS" and "CONTAINER" modes is an attempt to surface this, but the underlying confusion remains.
- **No flexbox / grid mental model.** Developers used to `display: flex; justify-content: space-between` reach for HBoxContainer + size_flags and find the equivalent more verbose. Composing nested layouts is more rigid (one container per algorithm, no `flex-direction: column-reverse`, no `flex-wrap` flag).
- **Mixed RTL / vertical-writing-mode support.** Godot's `mirror_layout` flag flips horizontally for RTL but doesn't update the property naming (still `offset_left/right`, not `offset_inline-start/end`). No vertical-writing-mode at the layout level.

**Designed-around or planned-to-fix?** Designed-around. Godot has not announced (as of 4.6) a layout-model rewrite. The 4.0 LayoutMode enum and the 4.x editor presets are accommodations, not replacements.

## Accessibility: added 11 years late, still "experimental"

The single largest historical gap. See [`accessibility.md`](accessibility.md) for the deep-dive. Critique surface:

- **No a11y in 1.0 → 4.4** (eleven years, January 2014 to March 2025). Blind users were effectively locked out of Godot games and the editor.
- **AccessKit landed in 4.5 (September 2025) as experimental.** Coverage is partial — Project Manager + standard widgets work; Inspector partial; full editor incomplete.
- **No drag-and-drop alternative contract** (WCAG 2.5.7). The `_get_drag_data` / `_can_drop_data` / `_drop_data` API has no built-in keyboard-equivalent requirement.
- **No ACCNAME 1.2-conforming name computation.** `accessibility_name` is consulted but the fallback chain (`aria-labelledby` chains, visible text, `title`) isn't formalized.
- **No live-region implementation across the catalog yet.** `accessibility_live` exists as a property; per-widget integration is in progress.
- **WCAG 2.5.8 target size not enforced.** Default theme hit-targets sometimes fall below 24×24.
- **No forced-colors / prefers-contrast / prefers-reduced-motion** plumbing.

**Designed-around or planned-to-fix?** Actively being fixed. The 4.5 → 4.6 → 4.7+ trajectory is improving coverage. But the retrofit cost is real — adding a11y to a 12-year-old widget catalog with custom `_draw()` per Control is significantly harder than designing it in from v1.

## Text: BiDi + complex scripts added 9 years late (4.0)

Until Godot 4.0 (March 2023), the engine had **no BiDi**, **no complex graphemes** (Devanagari, Tamil, Arabic shaping, etc.), **no ligatures**, **no multi-level font fallback**, **no color emoji**, **no variable fonts**. The TextServer overhaul in 4.0 fixed all of these in one large multi-year effort.

**Implication:** for nine years, Godot was **not viable for international games** without significant external tooling. Studios shipping in Arabic, Hebrew, Thai, Devanagari languages either pre-rendered text to textures, used a third-party library to bypass Godot's text rendering, or accepted broken output. The "shipping" community workarounds were inadequate; the 4.0 fix was overdue.

**Designed-around or planned-to-fix?** Fixed in 4.0. But the 9-year delay is the cautionary tale Buiy explicitly learns from.

## RichTextLabel: BBCode is divergent and read-only

- **BBCode is not HTML or Markdown.** Forum-software syntax from the 1990s; supports a small fixed tag vocabulary. No semantic elements, no CSS-style stylesheet, no content security model.
- **No tag composability.** Custom BBCode tags via `RichTextEffect` exist but the system is narrower than HTML's `<custom-element>` or Markdown's plugin ecosystem.
- **Not editable.** `RichTextLabel` is display-only. Rich-text editing in Godot is user-built on top of `TextEdit` (plain text) plus a sidecar parser, or accepts the limitation.
- **A11y unclear.** Whether `[url]` links announce as links in 4.5's AccessKit integration is undocumented as of writing.
- **No real `<table>` semantics.** `[table]` exists but renders without ARIA grid semantics; headers / row / column are not exposed to AT.

**Designed-around or planned-to-fix?** Designed-around. RichTextLabel + BBCode is mature and well-loved by Godot users; a redesign would be a significant compatibility break. The lack of a rich-text editor is a known proposal target but not actively in-flight.

## Performance at scale

- **Layout: per-container-class C++ algorithm.** No caching across frames; each container subclass implements its own `_notification(NOTIFICATION_SORT_CHILDREN)`. For deep / wide hierarchies, layout cost scales linearly with the visible subtree.
- **`_draw()` is called per frame on every visible Control.** The canvas server caches draw commands, but the visual state has to be reconstituted in `_draw()` each time the Control is marked dirty. Heavy use of custom `_draw()` (e.g., a node-graph with 1000+ nodes) does measurably slow down at high counts.
- **No virtualization in ItemList / Tree.** Render all items eagerly. Large lists (10,000+ rows) noticeably stutter; user-built virtualization is necessary.
- **Inspector at scale.** The Godot editor's Inspector for complex resources can render 500+ property rows; performance is acceptable but not snappy.

**Designed-around or planned-to-fix?** Mixed. The team has shipped editor-specific optimizations (Inspector property caching in 4.3, scene-tree dock virtualization improvements) but the underlying Control system doesn't have a built-in virtualization story.

## Missing primitives vs the modern web

Foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) catalogs the modern-web feature set Buiy commits to. Godot Control lacks:

- **No CSS Grid** with span / template-areas / fr / auto-placement / subgrid. GridContainer is fixed-columns row-flow only.
- **No container queries.** Components can't react to their containing-block size beyond anchor math.
- **No CSS anchor positioning** (different concept from Godot anchors despite shared name).
- **No `position: sticky`.** Sticky headers are user-built via `_process()`.
- **No multi-column** (`column-count`).
- **No logical properties** (`inline-size`, `padding-inline-*`).
- **No writing modes** (vertical-rl, vertical-lr).
- **No `clip-path`** beyond rectangular clips.
- **No `backdrop-filter`.**
- **No `mix-blend-mode`** at the Control level (only via custom shader material).
- **No view transitions / scroll-driven animations** at the layout level.
- **No `:focus-visible` semantics.** Focus styling is theme-driven and uniform; no "show ring on keyboard, hide on click" automatic split.
- **No content security model for rich text** (user-supplied BBCode could trigger arbitrary resource loads).

**Designed-around or planned-to-fix?** All designed-around. Godot doesn't claim web-platform parity; the model is "game engine UI for game UI" with the modern web treated as a different target.

## Distribution: GDExtension ABI churn

- **GDExtension ABI is not yet fully stable across minor versions.** Rust plugins (via `gdext`) typically need to recompile per Godot minor.
- **Crates.io coverage of Godot is via `gdext`** (third-party Rust binding) which itself is pre-1.0.
- **The C++ source API drifts within 4.x minors.** Editor plugin authors who hook into C++ classes face migration cost.

**Designed-around or planned-to-fix?** Planned. The team has stated ABI stabilization is a goal for 5.x; current 4.x churn is acknowledged.

## Three-way scripting fragmentation

- **GDScript** (primary) — Python-like, easy to learn, integrated into the editor.
- **C# / .NET** — wider ecosystem reach, but has had stop-start support history (Mono dependency, then .NET 6+, then .NET 8+); some breakage per major release.
- **GDExtension** (C++, Rust, Swift, Zig, etc.) — ABI-driven, more flexible, more setup cost.

Each has different ergonomics for Control authoring:

- GDScript: `extends Control` + `func _draw():` is one-liner ergonomic.
- C#: `public partial class MyControl : Control` is verbose but typed and tools-friendly.
- GDExtension: typed but requires per-Godot-minor recompile and ABI awareness.

The fragmentation means **Godot's Control documentation has to address three audiences**, and library / addon authors choose one and lose the other two. Buiy is Rust-only, which removes this fragmentation entirely.

**Designed-around or planned-to-fix?** Designed-in (the multi-language story is a deliberate goal). The fragmentation is a feature for the user community even if it's a friction for plugin authors.

## Other observed friction

- **No first-class `prefers-reduced-motion`** enforcement. The OS pref is readable; no engine-wide motion gate.
- **No first-class internationalization-tested text expansion** at the layout-engine level (pseudolocalization tool helps in 4.0+, but English layouts that pack 30% tighter than German still break at runtime).
- **No automated visual regression testing harness** built in. Third-party (e.g., `gut`) covers some scenarios.
- **No editor inspector-level a11y** until 4.5+, and incomplete even there.
- **Lack of high-CPU-count multi-windowing performance gains.** Godot's UI thread model is single-threaded for layout + draw queueing.
- **Custom drawing in `_draw()` doesn't compose with declarative props.** If a user-built control draws via `draw_rect`, they have to imperatively manage what they draw — there's no "set my background color, the engine handles it" path equivalent to BackgroundColor + Border + BoxShadow ECS components.

## Implications for Buiy

This is a long list. The TL;DR for Buiy's foundation:

- **Validates Buiy's CSS-via-Taffy bet.** The anchor + offset model is the dominant alternative; Godot's friction reports are the evidence that the alternative isn't free.
- **Validates Buiy's AccessKit-first bet.** Godot's 11-year a11y gap and 4.5's experimental status are the empirical cost of late retrofit.
- **Validates Buiy's BiDi-from-v1 bet.** Godot's 9-year BiDi gap demonstrates the retrofit cost; the TextServer rewrite in 4.0 took multi-year coordinated effort.
- **Validates Buiy's WCAG-AA-floor commitment.** Godot doesn't claim WCAG compliance; the gaps in target size, focus contrast, drag alternatives are not enforced.
- **Borrow:** the *additive* engineering posture. Godot 1.0's Control architecture has shipped through 4.6 unchanged. Stability is a feature; Buiy should commit to additive evolution under the spec discipline already established.
- **Avoid:** stuffing visual properties on Control and rendering via per-widget `_draw()`. Decomposed components + a render pipeline that knows BackgroundColor / Border / BoxShadow as first-class data is the corrective. (Foundation [`architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md).)
- **Avoid:** BBCode as the rich-text markup. Pick HTML-or-Markdown-shaped; a11y bridges already understand it.

## Sources

- Godot Forum + Reddit (r/godot) — recurring layout-friction threads.
- godot-proposals repository — https://github.com/godotengine/godot-proposals (search for "anchor", "accessibility", "wcag", "container query").
- Godot 4.0 release notes — https://godotengine.org/article/godot-4-0-sets-sail/
- Godot 4.5 release notes — https://godotengine.org/releases/4.5/
- `scene/gui/*.cpp` source — https://github.com/godotengine/godot/tree/master/scene/gui/
- Buiy foundation visuals spec — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
- bevy-ui critiques (sibling) — [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md)
