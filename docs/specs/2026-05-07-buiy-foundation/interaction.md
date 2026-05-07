# Feature inventory — interaction (forms, events, animation)

**Parent:** [README.md](README.md)

Tier legend: **F** = foundation, **C** = core, **E** = extended, **O** = out (excluded, with reason). See [README.md § Tier legend](README.md#tier-legend).

## 3.6 Forms and input

**Input types**
- Text, search, tel, url, email, password. **F**
- Numeric: `number`, `range` (slider). **F**
- Date / time: `date`, `month`, `week`, `time`, `datetime-local` — Buiy ships full pickers per APG. **C**
- Special: `color` (color picker), `file` (file picker). **C**
- Hidden form-state holder (no UI surface; carries form value only). **F**
- Button-like: `submit`, `reset`, `button`, `image`. **F**
- Selection: `checkbox` (incl. tri-state via indeterminate), `radio`. **F**

**Other form controls**
- Select (single + multi). **F**
- Combobox (textbox + popup). **F**
- Datalist — autocomplete suggestion source for textboxes / comboboxes. **F**
- Textarea. **F**
- Button. **F**
- Output (computed-result element). **C**
- Progress, meter (incl. low / high / optimum). **C**
- Fieldset, legend. **C**
- Label (with-for or wrapping; ACCNAME 1.2 source). **F**
- Form (in-process submit/reset semantics; not HTTP). **C**
- HTML invoker analogue (`command` / `commandfor` attributes — declarative bind from a button to a target's command, e.g. `show-popover`, `close`). **C**
- `field-sizing: content` — auto-size text inputs to content. **E**

**Constraint validation**
- Attributes: `required`, `pattern`, `min`, `max`, `step`, `minlength`, `maxlength`, `multiple`. **F**
- ValidityState analogue: `valueMissing`, `typeMismatch`, `patternMismatch`, `tooLong`, `tooShort`, `rangeUnderflow`, `rangeOverflow`, `stepMismatch`, `badInput`, `customError`. **F**
- `setCustomValidity` / `reportValidity` / `checkValidity`. **F**
- Pseudo-class state: `:required`, `:optional`, `:valid`, `:invalid`, `:user-valid`, `:user-invalid`, `:in-range`, `:out-of-range`, `:placeholder-shown`, `:read-only`, `:read-write`, `:default`, `:checked`, `:indeterminate`, `:disabled`, `:enabled`. **F**
- Form-associated custom components (analogue of `ElementInternals`). **C**

**Error message model** (WCAG 3.3.1 / 3.3.3 / 3.3.4)
- Each form control carries an error-message slot routed via the `aria-errormessage` analogue. **F**
- Error messages are live-region-aware: announced on validation failure via the global announcer. **F**
- Per-form error summary primitive (lists invalid fields, links to each). **C**
- Suggestion / fix proposal (3.3.3) is an authoring concern; Buiy provides the slot + aria wiring. **C**

**WCAG 2.2 form-specific SCs**
- WCAG 3.3.7 Redundant Entry (**Level A**) — form-state machine retains values across navigation; per-field "remember me" hooks. **C**
- WCAG 3.3.8 Accessible Authentication (Minimum, **Level AA**) — Buiy provides paste-friendly password fields, optional copy from clipboard, and avoids cognitive-only verification UIs by default. Paste-not-blocked is a CI gate. **F**
- WCAG 3.3.9 Accessible Authentication (Enhanced, **Level AAA**) — strict no-cognitive-test mode opt-in. **E**
- WCAG 3.2.6 Consistent Help (**Level A**) — apps own help placement; Buiy widget catalog ensures Help / Tooltip / Disclosure widgets render consistently. **C**

**State**
- `disabled`, `readonly`, `autofocus`, `name`, `value`, `placeholder`. **F**
- `autocomplete` token list (WCAG 1.3.5 input purpose). **C**
- Form state machine (pristine / dirty / touched / visited / valid). **F**
- Validation state propagation up forms / fieldsets. **F**

**File handling**
- File picker (single + multiple). **C**
- `accept` filter. **C**
- File drag-and-drop into a target. **C**
- Camera / mic capture. **E**
- Directory upload. **E**

**Out:** HTTP form submission, browser autofill credential store integration. **O**

## 3.7 Events and input handling

**Mouse events**
- `mousedown`, `mouseup`, `click`, `dblclick`, `auxclick`, `contextmenu`, `mouseenter`, `mouseleave`, `mouseover`, `mouseout`, `mousemove`. **F**
- Coordinates: client / page / screen / offset / movement. **F**
- Buttons + modifiers. **F**

**Pointer events** (unified, primary input model)
- `pointerdown` / `pointerup` / `pointermove` / `pointercancel` / `pointerover` / `pointerout` / `pointerenter` / `pointerleave` / `pointerrawupdate`. **F**
- `gotpointercapture` / `lostpointercapture`. **C**
- `pointerType` (mouse / touch / pen). **F**
- `pressure`, `tangentialPressure`, `tiltX/Y`, `twist`, `width`, `height` (pen / stylus fidelity). **C**
- `isPrimary`. **F**
- Pointer capture. **F**

**Touch events**
- `touchstart` / `touchmove` / `touchend` / `touchcancel`. **C**
- Multi-touch via stable identifiers. **C**
- Gesture primitives: pinch, rotate, swipe, long-press, double-tap. **C**

**Keyboard events**
- `keydown` / `keyup` / `beforeinput` / `input`. **F**
- Logical key (`KeyboardEvent.key`), physical code (`code`), repeat, location, modifiers, `isComposing`. **F**
- IME composition events. **F**
- Keyboard shortcut binding (`aria-keyshortcuts` analogue). **F** — every menu / button-with-shortcut widget needs it for APG conformance and WCAG 2.1.4.
- Global keyboard shortcut activation (`accesskey` analogue + window-level chord registration; OS-conflict policy: shortcuts that collide with OS / IME modifiers are rejected at registration time). **F**
- Single-key shortcut remap policy (per WCAG 2.1.4): every single-key shortcut is opt-in or remappable, suppressible while a textbox has focus. **F**
- `InputEvent.inputType` taxonomy (`insertText`, `deleteContentBackward`, `historyUndo`, `formatBold`, etc.) for editing semantics. **C**
- Keyboard layout map (logical-to-physical, locale-aware). **E**

**Gamepad** — first-class
- Standard mappings (DPad / sticks / face buttons / triggers / shoulder / start / select). **F**
- Logical actions (navigate / activate / back / context-menu), remappable. **F**
- Spatial focus navigation (DPad / left stick → geometric focus movement). **F**
- Analog inputs for sliders, scrollers, draggables. **C**

**Wheel / scroll**
- `wheel` event with `deltaX/Y/Z` and `deltaMode`. **F**
- `scroll` event. **F**
- `scrollend` event. **C**
- Smooth scrolling, scroll snap, momentum. **C**

**Drag and drop**
- Lifecycle: `dragstart`, `drag`, `dragend`, `dragenter`, `dragover`, `dragleave`, `drop`. **C**
- DataTransfer analogue. **C**
- OS drag-source / drag-target interop. **C**
- Every drag-driven Buiy widget ships a keyboard alternative (WCAG 2.5.7). **F**

**Focus events**
- `focus` / `blur` (non-bubbling). **F**
- `focusin` / `focusout` (bubbling). **F**
- `:focus-visible` heuristic. **F**
- `relatedTarget`. **C**

**Form events**
- `input`, `change`, `submit`, `reset`, `invalid`, `formdata`, `beforeinput`. **F**
- `selectionchange`, `select`. **C**

**Clipboard**
- `copy`, `cut`, `paste` events. **F**
- Programmatic clipboard read/write (text + HTML + image MIME). **C**
- OS clipboard format negotiation. **C**

**Event flow**
- Capture → target → bubble. **F**
- `stopPropagation`, `stopImmediatePropagation`, `preventDefault`. **F**
- Listener options: `passive`, `once`, `signal`, `capture`. **C**
- Synthetic / dispatched events. **C**

**Pseudo-class state surface (interactive)**
- `:hover`, `:active`, `:focus`, `:focus-visible`, `:focus-within`, `:target`. **F**
- `:has()` (dependent-state selector). **C**
- `:is()`, `:where()`, `:not()`. **C**
- `:dir(ltr | rtl)`, `:lang(<code>)`. **F** — required given RTL is a foundation goal.
- `:state(<custom>)` — Custom State Pseudo-class API for form-associated custom widgets. **C**
- `:fullscreen` — true when subtree is the active fullscreen surface. **C**
- `:modal` — true when subtree is an open modal `Dialog` or `AlertDialog`. **C**
- `:popover-open` — true when popover element is open (auto / manual / hint). **C**

**Pseudo-class state surface (structural)**
- `:nth-child(<an+b>)`, `:nth-of-type(<an+b>)`, `:nth-last-child`, `:nth-last-of-type`. **C**
- `:first-child`, `:last-child`, `:only-child`, `:first-of-type`, `:last-of-type`, `:only-of-type`. **C**
- `:empty`. **C**
- `:root`, `:scope`. **C**

**Pseudo-elements**
- `::before`, `::after` — generated content / decorative inserts. **C**
- `::backdrop` — modal / dialog / fullscreen backdrop styling. **F**
- `::selection`. **F**
- `::placeholder`. **F**
- `::marker` (list bullets). **C**
- `::highlight(<name>)` — Custom Highlight API for collaborative highlights, find-in-page, custom underline ranges. **E**
- `::file-selector-button`. **C**
- `::part(<name>)`, `::slotted(<selector>)` — Shadow-DOM-style component-encapsulation pseudo-elements. **E**
- `::details-content` — disclosure / `<details>` open state content. **C**
- `::view-transition`, `::view-transition-group`, `::view-transition-image-pair`, `::view-transition-old`, `::view-transition-new` — view transition pseudo-elements (paired with § 3.8 below). **C**
- `::spelling-error`, `::grammar-error`. **E**
- `::first-letter`, `::first-line`. **E**
- `::target-text` (text fragments). **E**

**Observers** (programmatic observation primitives, analogous to web Observer APIs)
- `IntersectionObserver` analogue — observe when a node enters / leaves viewport or another node. **C** — required for lazy-load, virtualization, scroll-based reveal.
- `ResizeObserver` analogue — observe size changes on a node. **C** — required for container-query authors and responsive components.
- `MutationObserver` analogue — observe subtree mutations beyond Bevy's per-component change-detection. **C**
- `PerformanceObserver` analogue — observe per-frame layout / render / a11y-update timings. **E**

**At-rules / cascade primitives**
- Token cascade is Buiy-native; CSS at-rules (`@media`, `@supports`, `@layer`, `@scope`, `@import`) are not the primary expression. The features they expose are reified differently:
  - User-preference media-query equivalents (`prefers-color-scheme`, `prefers-contrast`, `forced-colors`, `prefers-reduced-motion`, `prefers-reduced-transparency`, `inverted-colors`) live in the `UserPreferences` resource. **F**
  - Capability media-query equivalents (`pointer: none|coarse|fine`, `hover: none|hover`). **C**
  - Display media-query equivalents (`color-gamut: srgb|p3|rec2020`, `dynamic-range: standard|high`). **C**
  - Container-query units (`cqw / cqh / cqi / cqb / cqmin / cqmax`). **C** — see [visuals.md § 3.2](visuals.md).
  - Feature-detection (`@supports`) reifies as runtime capability resources (e.g., `RenderCapabilities`). **C**
  - Cascade layering (`@layer`) is unnecessary because Buiy doesn't ship a stylesheet language; theme override priority is explicit (subtree `Theme` component). **O**
  - `@scope` is unnecessary for the same reason. **O**
  - CSS nesting (`& selector`) — irrelevant without a stylesheet. **O**

**Out:** deprecated DOM mutation events, web's `Event.isTrusted` security flag (Buiy events are all in-process; the verification harness in [verification.md](verification.md) *does* synthesize input events for testing — this is unrelated to the web's trusted-vs-synthetic distinction), hashchange / popstate. **O**

## 3.8 Animation and motion

**Property transitions** (CSS Transitions analogue)
- Transition any animatable property on state change. **F**
- `transition-property` / `-duration` / `-timing-function` / `-delay` / `-behavior` (allow-discrete). **F**
- Timing functions: `linear()` (multi-stop), `ease`, `ease-in/out/in-out`, `cubic-bezier()`, `steps()`, `step-start/end`. **C**
- Discrete property transitions (e.g., display) via `@starting-style` analogue. **C**
- `interpolate-size: allow-keywords` analogue — animate to/from intrinsic-size keywords (`auto`, `min-content`, `max-content`, `fit-content`). **C**
- Transition lifecycle events. **C**

**Keyframe animations** (CSS Animations analogue)
- Keyframes (from / to / percentages, named timelines). **F**
- Animation properties: name, duration, timing, delay, iteration-count, direction, fill-mode, play-state, composition, timeline, range. **C**
- Animation lifecycle events. **C**

**Programmatic animation API** (Web Animations API analogue)
- Per-element programmatic control: play, pause, reverse, finish, cancel, playback rate. **C**
- Composite operations (replace / add / accumulate). **E**

**Layout transitions** (View Transitions analogue)
- Animate layout changes (size, position) automatically when state changes. **C**
- Cross-state snapshots. **C**
- Per-element view-transition names. **C**

**Scroll-driven animations**
- Scroll timeline, view timeline. **E**

**Game-flavored animation**
- Spring physics primitives. **C**
- Timeline composition (sequence, parallel). **C**

**Reduced motion**
- All animations short-circuit under `prefers-reduced-motion: reduce`. **F**
- WCAG 2.3.1 — no flashes >3/sec; flash detection in CI. **F**
- WCAG 2.3.3 — animation from interactions respects reduced-motion. **F**
