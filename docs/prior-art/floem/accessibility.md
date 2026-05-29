**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — accessibility status, the AccessKit gap, contrast with Rust UI peers

## The summary: Floem has no accessibility integration

As of the verified state of `main` (2026-05):

- `accesskit` is **not** a dependency in `Cargo.toml`.
- `accesskit_winit` is **not** a dependency in `Cargo.toml`.
- There is no `floem::accessibility` module on docs.rs.
- The README does not mention accessibility.
- Issue [#8](https://github.com/lapce/floem/issues/8) "Support Accessibility via AccessKit" was opened **2023-04-14** by user `Zizico2` and is **still open with no progress in three years**. Body is a one-line link to the AccessKit repo. No assignees, no labels, no milestone.

For Buiy's purposes this is decisive: Floem is **not** a reference for accessibility implementation. It is, at best, a *negative* reference — an example of what happens when accessibility is filed as future work and never staffed.

## What Floem ships instead of accessibility

The `understory_focus` dependency (a Lapce-team sister crate) provides focus tracking. Focus is a *prerequisite* for accessibility (AT needs to know what has focus), but focus alone does not constitute accessibility. The missing layers:

- **Accessible role assignment.** No `role="button"` / `role="checkbox"` equivalent. Without role, screen readers cannot announce the type of widget.
- **Accessible name / description computation.** No ACCNAME-1.2-equivalent path from text content + ARIA attributes to a screen-reader utterance.
- **AccessKit `Tree` / `Node` construction.** No tree-to-AccessKit translation; no `accesskit_winit::Adapter` instance.
- **State announcements.** No `aria-pressed`, `aria-expanded`, `aria-checked` equivalents to push state changes through the AT bus.
- **Live regions.** No `aria-live` equivalent for status updates.

## Contrast with peers

| Project | AccessKit integrated? | Adapter ownership |
|---|---|---|
| **egui** | Yes (since 2022, PR #2294) | `eframe` owns the adapter |
| **fltk-rs** | Yes (via `fltk-accesskit`) | Third-party crate |
| **Slint** | Yes | Slint runtime owns |
| **Iced** | Yes (since iced 0.13) | Iced owns |
| **Xilem / Masonry** | Yes — explicit AccessKit-first design | Masonry owns |
| **Dioxus desktop** | Partial via Blitz integration | Blitz owns |
| **Floem** | **No** | n/a |
| **bevy_ui** | Partial via `bevy_a11y` | `bevy_a11y` owns |
| **Buiy** (planned) | **Yes, AccessKit-first** | Buiy owns `accesskit_winit::Adapter` per window |

Floem is the **outlier** among living Rust UI projects on this dimension. egui shipped AccessKit in 2022; Xilem was designed AccessKit-first from day one. The fact that issue #8 has sat untouched for three years signals that accessibility is not on Lapce's roadmap, and Floem inherits that priority order.

## What Lapce-the-editor does for accessibility

Empirical observation (informed by HN threads, not direct Lapce inspection): Lapce ships without screen-reader support. For an editor this is a significant accessibility gap; the editor pattern (vim mode, modal editing, custom keybindings) is already hostile to baseline AT, and the absence of an AccessKit tree means even basic announcements don't work.

For Buiy: this is a real lesson. A UI library that ships in an editor-with-no-accessibility produces no pressure on the library to add accessibility. **Single-flagship dogfooding without an accessibility requirement results in a library with no accessibility.** Buiy's foundation goal #2 (WCAG 2.2 AA as a floor) only holds if Buiy's flagship apps require it — or if Buiy adds accessibility *before* its first flagship.

## The technical surface that's missing (if anyone tried to add it)

For a future implementer of Floem AccessKit support, the rough plan would be:

1. Add `accesskit` + `accesskit_winit` dependencies.
2. Construct an `accesskit_winit::Adapter` per Floem window in the event loop.
3. Define a `View::accessibility_node(&self, ctx) -> Option<accesskit::Node>` trait method on `View`.
4. Walk the view tree on each frame (or on accessibility-tree-update request) to produce an `accesskit::Tree` rooted at the window.
5. Route `accesskit::ActionRequest` events back through the Floem event system (translate "click" / "focus" / "set_value" requests into Floem action dispatch).
6. Per-widget: ACCNAME-1.2-compliant name computation, role assignment, state binding for each built-in view.

None of this exists in Floem today. The effort is substantial — comparable to the egui PR #2294 which was a multi-month effort. Without staffing, issue #8 will likely remain open.

## Buiy implication

Floem's accessibility status confirms three things for Buiy:

1. **AccessKit-first is correct.** Retrofitting AccessKit onto a finished UI library is hard enough that mature projects (Floem) defer it indefinitely. Buiy's foundation §2.6 commits to AccessKit ownership from day one. Stay the course.
2. **The accessibility-tree-construction trait** (`View::accessibility_node` shape above) should be a *required* method on Buiy's component / widget trait, not an optional one. Optional methods get skipped; required methods force every widget to confront its accessibility surface.
3. **The flagship-with-accessibility-requirement** matters. Buiy needs at least one early dogfooder whose users require AT support — without that pressure, accessibility quality drifts.

See [`../accesskit/`](../accesskit/) for the AccessKit substrate's own folder, [`../accesskit/lessons.md`](../accesskit/lessons.md) for the Validates/Avoid/Borrow on AccessKit specifically, and Buiy foundation `accessibility.md` for Buiy's accessibility commitments.

## Sources

- Floem issue #8 "Support Accessibility via AccessKit" — https://github.com/lapce/floem/issues/8
- Floem Cargo.toml (no accesskit) — https://github.com/lapce/floem/blob/main/Cargo.toml
- egui PR #2294 (AccessKit integration) — https://github.com/emilk/egui/commit/e1f348e4b24c2fa83d25c6a7ddfd9b38b85de161
- fltk-accesskit — https://github.com/fltk-rs/fltk-accesskit
- Buiy foundation `accessibility.md` — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Cross-link: [`../accesskit/lessons.md`](../accesskit/lessons.md)
