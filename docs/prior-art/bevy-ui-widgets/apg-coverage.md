**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — WAI-ARIA APG coverage map

# APG coverage

[WAI-ARIA Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/) defines the canonical keyboard contracts and ARIA roles for ~30 widget patterns. This file maps each APG pattern to bevy_ui_widgets's coverage, gap by gap.

## Patterns covered (with caveats)

| APG pattern | bevy_ui_widgets | Notes |
|---|---|---|
| [Button](https://www.w3.org/WAI/ARIA/apg/patterns/button/) | ✅ `Button` | Toggle-button (`aria-pressed`) variant missing — workaround is Checkbox+Switch role override. |
| [Checkbox](https://www.w3.org/WAI/ARIA/apg/patterns/checkbox/) | ✅ `Checkbox` (binary) | **Tri-state (`aria-checked="mixed"`) is not modeled** — `Checked` is a marker component, not an enum. APG dual-state variant is uncovered. |
| [Radio Group](https://www.w3.org/WAI/ARIA/apg/patterns/radio/) | ✅ `RadioGroup` + `RadioButton` | Full keyboard contract (arrows wrap, Home/End). Group focusable, buttons not — per APG. |
| [Slider (single-thumb)](https://www.w3.org/WAI/ARIA/apg/patterns/slider/) | ✅ `Slider` (since 0.17; vertical since 0.18) | PageUp/PageDown larger-step **not** explicitly implemented in observers. |
| [Slider (multi-thumb)](https://www.w3.org/WAI/ARIA/apg/patterns/slider-multithumb/) | ❌ | Not shipped. App must compose two `Slider`s and reconcile, with no APG-compliant interleaved keyboard. |
| [Menu / Menubar](https://www.w3.org/WAI/ARIA/apg/patterns/menubar/) | ⚠️ `MenuPopup` + `MenuItem` + `MenuButton` (since 0.18) | Column + Row layouts have arrow-key navigation; **Grid layout requires app-supplied observer**. **No submenu support** (in-source TODO). No type-ahead first-letter focus. No menubar-specific role (uses `MenuListPopup`). |
| [Textbox](https://www.w3.org/WAI/ARIA/apg/patterns/textbox/) (single-line) | ⚠️ `EditableText` driven by `text_input.rs` (since 0.19) | Full editing keymap, IME, double/triple-click selection. No `aria-multiline` / single-vs-multi distinction beyond `allow_newlines`. |
| Scrollbar | ⚠️ `Scrollbar` | **Not exposed in a11y tree** by design (see `widgets.md`). Not focusable, no keyboard. The scroll container owns these affordances. |

## Patterns explicitly NOT covered

These are common APG patterns the brief asked Buiy to track. None ship in `bevy_ui_widgets`. For Buiy these are **foundation-tier (F) or core-tier (C)** widgets in [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md).

| APG pattern | Status | Buiy tier |
|---|---|---|
| [Toggle Button](https://www.w3.org/WAI/ARIA/apg/patterns/button/) (aria-pressed) | ❌ — use Button + manual `aria-pressed` | F |
| [Switch](https://www.w3.org/WAI/ARIA/apg/patterns/switch/) | ❌ — *"override `AccessibilityNode` with Switch role"* on a Checkbox (per checkbox.rs docs) | F |
| [Link](https://www.w3.org/WAI/ARIA/apg/patterns/link/) | ❌ | F |
| [Disclosure](https://www.w3.org/WAI/ARIA/apg/patterns/disclosure/) | ❌ | F |
| [Accordion](https://www.w3.org/WAI/ARIA/apg/patterns/accordion/) | ❌ | F |
| [Tabs](https://www.w3.org/WAI/ARIA/apg/patterns/tabs/) (automatic + manual activation) | ❌ | F |
| [Tooltip](https://www.w3.org/WAI/ARIA/apg/patterns/tooltip/) | ❌ — Popover positioning exists, but no Tooltip role/lifecycle wrapper | F |
| [Dialog (Modal + Non-modal)](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/) | ❌ | F |
| [Alert Dialog](https://www.w3.org/WAI/ARIA/apg/patterns/alertdialog/) | ❌ | F |
| [Alert / Status / Log / Timer](https://www.w3.org/WAI/ARIA/apg/patterns/alert/) (live regions) | ❌ | F (Alert/Status), C (Log/Timer) |
| [Combobox](https://www.w3.org/WAI/ARIA/apg/patterns/combobox/) | ❌ | F |
| [Listbox (single + multi-select)](https://www.w3.org/WAI/ARIA/apg/patterns/listbox/) | ❌ | F |
| [Spinbutton](https://www.w3.org/WAI/ARIA/apg/patterns/spinbutton/) | ❌ | F |
| [Searchbox](https://www.w3.org/WAI/ARIA/apg/patterns/combobox/) | ❌ | F |
| [Toolbar](https://www.w3.org/WAI/ARIA/apg/patterns/toolbar/) | ❌ | C |
| [Breadcrumb](https://www.w3.org/WAI/ARIA/apg/patterns/breadcrumb/) | ❌ | C |
| [Tree View](https://www.w3.org/WAI/ARIA/apg/patterns/treeview/) | ❌ | C |
| [Treegrid](https://www.w3.org/WAI/ARIA/apg/patterns/treegrid/) | ❌ | C |
| [Grid](https://www.w3.org/WAI/ARIA/apg/patterns/grid/) | ❌ | C |
| [Table (semantic)](https://www.w3.org/WAI/ARIA/apg/patterns/table/) | ❌ | C |
| [Window Splitter](https://www.w3.org/WAI/ARIA/apg/patterns/windowsplitter/) | ❌ | C |
| [Carousel](https://www.w3.org/WAI/ARIA/apg/patterns/carousel/) | ❌ | C |
| [Feed](https://www.w3.org/WAI/ARIA/apg/patterns/feed/) | ❌ | C |
| [Meter](https://www.w3.org/WAI/ARIA/apg/patterns/meter/) | ❌ | C |
| [Progressbar](https://www.w3.org/WAI/ARIA/apg/patterns/progressbar/) (determinate + indeterminate) | ❌ | F |
| [Date Picker (Dialog)](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/examples/datepicker-dialog/) | ❌ — Feathers ships `ColorPlane` (color picker) but no date picker | C |

## Counting

bevy_ui_widgets covers **~5 of the ~30 APG widget patterns** with full keyboard contracts (Button, Checkbox, Radio, Slider, Menu). Two more are partial (Textbox single-line as a separate input plugin; Scrollbar is deliberately not exposed in a11y).

Buiy's planned coverage (per [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) is **~50 widgets across Foundation + Core tiers**, covering essentially the entire APG plus web extras (Popover with light-dismiss state machine, HTML `<dialog>` `closedby` policy, `command` / `commandfor` invocation, etc.). The gap is intentional: bevy_ui_widgets is a starter set; Buiy is committing to web-platform-parity APG coverage from day one.

## Keyboard contract conformance (per shipped widget)

The implementations match the APG keyboard contract on the operations they implement, but with several documented gaps:

- **Button:** ✅ Enter / Space on key-up. ❌ no spec deviation noted.
- **Checkbox:** ✅ Space (the APG primary). ⚠️ Enter is *also* mapped (APG calls Enter optional/non-standard). ❌ no mixed-state shortcut.
- **Radio:** ✅ Arrow wrap, Home, End. ❌ no first-letter type-ahead. ⚠️ Buttons are non-focusable (APG-conformant); apps that want a focusable button (rare) must override.
- **Slider:** ✅ Arrow stepping. ⚠️ Home/End to set min/max — confirm whether implementation matches; quick read of slider.rs doc doesn't show explicit Home/End in the keyboard observer (mostly pointer-driven). ❌ PageUp / PageDown larger-step (APG-recommended).
- **Menu (Column):** ✅ Arrow, Home, End, Enter / Space, Escape. ❌ first-letter type-ahead. ❌ submenu open/close (TODO in source).
- **Menu (Row):** ✅ Left / Right arrow nav.
- **Menu (Grid):** ⚠️ Arrow keys *unmapped* — *"you'll need to write your own observer"*. Notable for any data-grid-style menu.
- **EditableText:** ✅ Comprehensive editing keymap, platform-aware modifiers, IME. ❌ rich-text input modes; ❌ multi-cursor / multi-caret; ❌ undo/redo (Cmd+Z / Cmd+Shift+Z); ❌ find / replace.

## ARIA role coverage

Roles set via `AccessibilityNode(accesskit::Node::new(Role::<X>))` per widget:

| Widget | accesskit Role |
|---|---|
| Button | `Role::Button` |
| Checkbox | `Role::CheckBox` |
| RadioGroup | `Role::RadioGroup` |
| RadioButton | `Role::RadioButton` |
| Slider | `Role::Slider` (with `Orientation::{Horizontal,Vertical}` hint) |
| Scrollbar | (none — not in a11y tree) |
| MenuPopup | `Role::MenuListPopup` |
| MenuItem | `Role::MenuItem` |
| MenuButton | `Role::Button` (inherits from bevy_ui::widget::Button) |

Notable absences in the role surface: `Role::Switch`, `Role::Tab`, `Role::TabList`, `Role::TabPanel`, `Role::Tree`, `Role::TreeItem`, `Role::Dialog`, `Role::AlertDialog`, `Role::Tooltip`, `Role::Listbox`, `Role::ComboBox`, `Role::Grid`, `Role::GridCell`, `Role::Toolbar`, `Role::Progressbar`, `Role::Meter`, `Role::Alert`, `Role::Status`, `Role::Log`, `Role::Timer`. AccessKit *supports* all these roles — bevy_ui_widgets just doesn't ship widgets that use them.

## Implications for Buiy

Buiy's [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) inventory is **~5× the breadth** of bevy_ui_widgets. The catalog ships all foundation widgets (Button incl. toggle-pressed, Switch as its own widget, Link, Text, Image, Heading, Label, Group, Region, all landmarks, Checkbox incl. tri-state, Switch, RadioGroup, Listbox, Combobox, Slider single+multi-thumb, Spinbutton, Textbox single+multi-line, Searchbox, all Menu shapes, Tabs both flavors, Dialog modal+non-modal, AlertDialog, Popover with full HTML state machine, anchored Popover, Tooltip, Disclosure, Accordion incl. exclusive, Progressbar determinate+indeterminate, Alert, Status, Toast w/ WCAG 2.2.3) at the F tier alone.

The lesson is **scope honesty**: bevy_ui_widgets covers the easiest 20% of the APG. Buiy's spec is committed to ~all of it, with WCAG 2.2 SC coverage gates ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md) gates 3 + 4 + 7), which means the **verification harness** is the load-bearing artifact that makes a 50-widget catalog tractable. Without per-widget CI gates, the work of authoring 50 APG-conformant widgets is the work that has historically *not* gotten done in any Rust UI library.

Buiy's foundation also commits each widget to: AccessKit role + name source + states, theme-token consumption, `:focus-visible`, forced-colors fallback, reduced-motion fallback, RTL mirroring, ≥24×24 hit target (WCAG 2.5.8). bevy_ui_widgets ships none of these by default — they're either app-side polling, Feathers-side defaults, or absent.

## Sources

- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- AccessKit roles enum — https://docs.rs/accesskit/0.24/accesskit/enum.Role.html
- Per-widget sources — `crates/bevy_ui_widgets/src/*.rs` (@ main, 2026-05-22)
- Buiy widget catalog — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Buiy verification gates — [`../../specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)
- Sibling: [`widgets.md`](widgets.md), [`open-problems.md`](open-problems.md)
