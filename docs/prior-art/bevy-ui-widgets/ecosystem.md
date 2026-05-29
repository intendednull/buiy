**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Ecosystem, comparisons, production usage

# Ecosystem & comparisons

## Production usage observations

`bevy_ui_widgets` first shipped 2025-09-30. Eight months old as of 2026-05-22. **Production adoption is nascent.** Honest read:

- **Crates.io reverse-dependencies: 3** (publishers depending on `bevy_ui_widgets` directly via crates.io). This is low; most apps using it route through the `bevy` meta-crate's `ui_widgets` re-export, so the reverse-deps count under-states actual usage. But the count does measure "third-party libraries that build on bevy_ui_widgets as a substrate," and that's small.
- **Total downloads: 201,008 lifetime, 177,568 in the last 90 days.** ~88% of lifetime downloads are recent — consistent with the crate being new. The 90-day figure is comparable to bevy_feathers (~145K-range for the same window) and to other `bevy_*` sub-crates that ship with `bevy`'s default features.
- **Flagship shipping app: the in-development Bevy editor itself.** Not yet released. Per Bevy's roadmap, the editor is the canonical bevy_ui_widgets + bevy_feathers consumer.
- **No third-party flagship app** verifies as a public bevy_ui_widgets consumer at scale as of 2026-05-22. This mirrors [`../bevy-ui/ecosystem.md`](../bevy-ui/ecosystem.md)'s finding for bevy_ui itself: *"bevy_ui has not yet been used at scale in a flagship commercial title."*
- **Bevy ecosystem games (Tiny Glade, Foresight Spar, Roboquest)** mostly built their own UI renderers before bevy_ui_widgets existed; migration is unlikely without a clear "Feathers on top of bevy_ui_widgets" win.

## Comparisons

### vs `bevy_feathers` (sibling, the styled layer)

| | bevy_ui_widgets | bevy_feathers |
|---|---|---|
| Role | Headless interaction logic | Styled-widget kit (visuals on top) |
| Direct downstream user | bevy_feathers, custom-styled apps | Bevy editor + apps that want editor-flavored UI |
| Ships theme tokens? | No | Yes |
| Ships visuals? | No | Yes (dark theme, atlas icons, fixed sizing) |
| Ships its own widgets? | Yes (5 core + Menu + Popover + text input) | Yes — composes the headless widgets + adds Feathers-specific (`ColorPlane`, counter, button styles) |
| Bevy version | 0.17 → 0.19-rc.2 | 0.17 → 0.19-rc.2 (lockstep) |
| Crates.io downloads (lifetime) | 201,008 | 191,700 |

The split is clean and well-motivated. Apps that want the editor look: use Feathers. Apps that want their own visuals: use bevy_ui_widgets directly.

### vs third-party Bevy widget kits

| Kit | License | Last release | Widget count (approx) | Substrate | Headless-aware? |
|---|---|---|---|---|---|
| `sickle_ui` | MIT | active (2026) | ~25 widgets incl. ColorPicker, Dropdown, FloatingPanel | bevy_ui | No (built before bevy_ui_widgets) |
| `woodpecker_ui` | Apache-2.0 | active (2026) | React-style framework; widget catalog varies | bevy_ui | No |
| `bevy_lunex` | MIT/Apache-2.0 | active (2026) | 2D + 3D UI with own widget set | own layout engine | No |
| `bevy_egui` | MIT/Apache-2.0 | very active | egui's full widget catalog (very broad) | egui (separate paradigm) | No |
| `bevy_ui_dsl` | MIT/Apache-2.0 | sporadic | DSL over bevy_ui; widget-shaped helpers | bevy_ui | No |

**None of these were designed against `bevy_ui_widgets`** — they pre-date it. Migration paths are theoretically possible (replace internal state machines with `bevy_ui_widgets` markers + observers) but no public migration has shipped. `sickle_ui` in particular ships much broader APG coverage (Dropdown, FloatingPanel, ColorPicker, Tabs) than bevy_ui_widgets does — moving its visuals on top of `bevy_ui_widgets` would *remove* widgets unless the headless layer fills the gap first.

### vs JS/TS headless prior art

`bevy_ui_widgets`'s design lineage is explicit (per viridia in discussion #16900): JS headless widget libraries.

| | bevy_ui_widgets | Headless UI (Tailwind) | Radix Primitives | React Aria |
|---|---|---|---|---|
| Substrate | Bevy ECS + bevy_ui | React | React | React + others |
| Widget count | 5–8 core + menu + popover + text input | ~15 (Combobox, Dialog, Disclosure, Listbox, Menu, Popover, Radio, Switch, Tabs, etc.) | ~30+ | ~50+ |
| APG coverage | ~5/30 patterns | ~15/30 | ~25/30 | ~all of APG |
| Accessibility integration | AccessKit (system-level) | ARIA in DOM | ARIA in DOM | ARIA in DOM + iOS/Android adapters |
| Maturity | <1 year; experimental | ~4 years | ~3 years | ~5 years |
| State management | External (observers) | External (controlled props) | External (controlled props) | External (hooks) |

The headless pattern is well-established in the JS world. Bevy is the first major game engine to ship it as a primitive. **The 1:1 comparison shows the scope gap clearly — even mature headless libraries took years to fill out APG coverage, so bevy_ui_widgets's 5-widget starting point is normal for a new entrant.** What's unusual is the ambition (game engine + accessibility + headless) at all.

### vs Buiy's planned catalog

| | bevy_ui_widgets | Buiy `buiy_widgets` |
|---|---|---|
| APG coverage | ~5/30 patterns at v1.x | ~all of APG at F+C tier ([media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) |
| Foundation-tier widgets | partial | committed: Button (incl. toggle), Link, Text, Image, Heading, Label, Group, Region, all landmarks, Checkbox (incl. tri-state), Switch, RadioGroup, Listbox, Combobox, Slider (single + multi-thumb), Spinbutton, Textbox (single + multi), Searchbox, Menu, Menu Button, Tabs (auto + manual), Dialog modal + non-modal, AlertDialog, Popover (auto/manual/hint), anchored Popover, Tooltip (WCAG 1.4.13), Disclosure, Accordion, Progressbar, Alert, Status, Toast (WCAG 2.2.3) |
| WCAG 2.2 SC coverage | not enforced | gated in CI (verification gates 3, 4, 7) |
| Touch / gamepad first-class | partial (via bevy_picking) | gamepad first-class, touch in core ([interaction.md § 3.7](../../specs/2026-05-07-buiy-foundation/interaction.md)) |
| RTL widget mirroring | absent | required ([media-and-widgets.md](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) |
| Hit target ≥24×24 (WCAG 2.5.8) | not enforced | required by default |
| Theme tokens | none (apps own theming, or use Feathers) | first-class ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md)) |
| BSN-friendly | partial (PR #23924 added FromTemplate to most components 2026-04-22) | required by construction ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)) |
| Per-widget verification fixture | none in CI | gated ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) |

Buiy is committing to an order-of-magnitude broader catalog with substantially stricter accessibility + verification gates. The bevy_ui_widgets reference is **the contemporary substrate Buiy is parallel-stacking against** — Buiy must do better on coverage breadth, not by reusing the crate but by shipping its own catalog at its own component model.

## Bevy editor as the canonical consumer

The in-development Bevy editor is the load-bearing consumer of `bevy_ui_widgets + bevy_feathers`. viridia's stated motivation for both crates is editor tooling. The editor is the natural fit for headless widgets:
- Editor UIs need accessibility (NVDA / VoiceOver / Orca screen reader use is normal for editor users).
- Editor visuals are opinionated and fixed (Feathers ships the canonical look).
- Editor UI shape is well-bounded (panels, property grids, menubars, dialogs) — closer to the 5-widget starter set than a general-purpose app would be.

When the editor ships, it'll be the first significant production demonstration. Until then, production usage is hypothetical.

## Third-party adopters (observations)

As of 2026-05-22:

- The most plausible early adopters are **Bevy ecosystem developers building productivity-app-shaped tools** (asset inspectors, level editors, debug overlays). Several Bevy users have posted demos / experiments to Discord and X using `bevy_ui_widgets` + custom styling.
- **No public shipping app** verifies as a meaningful production user. Searching crates.io for reverse dependencies returns 3 — small libraries / experiments.
- The cargo-feature-gate removal in 0.18 (PR #22934) means many Bevy users may be running bevy_ui_widgets transitively without realizing it (it's in `bevy::ui_widgets`); usage measurement via direct dependency is unreliable.

## Implications for Buiy

- **Don't expect bevy_ui_widgets to grow into a parallel-stack competitor any time soon.** The cadence (3 widget categories added across 0.17 → 0.19 = 8 months) is consistent with steady, multi-year growth. By the time bevy_ui_widgets covers half the APG, Buiy can ship its full catalog if the verification harness pulls its weight.
- **The third-party kits are the more interesting competitive landscape.** `sickle_ui` in particular has broader widget coverage than bevy_ui_widgets and a usable styled API. If `sickle_ui` migrates onto `bevy_ui_widgets` as substrate (still bevy_ui-bound), it becomes the de-facto "broad styled kit on Bevy." Buiy's differentiation against that future is the *parallel-stack* (no bevy_ui-renderer coupling, own pipeline) + the broader WCAG verification + the BSN-friendly-from-day-one component model.
- **"No flagship shipping app yet"** is true for both bevy_ui_widgets and bevy_ui itself. Buiy's foundation README goal 6 ("Game and app, both") is the bet that Buiy's verification harness + game + productivity-app fixtures will close the no-flagship gap that bevy_ui has carried since 2020.
- **For Buiy app authors who need a bevy_ui-flavored window in the same app:** add `UiWidgetsPlugins` + `FeathersPlugin` to that window. Per-window coexistence ([integration.md](integration.md) #5) is the supported integration.

## Sources

- crates.io API — https://crates.io/api/v1/crates/bevy_ui_widgets, https://crates.io/api/v1/crates/bevy_feathers (fetched 2026-05-22)
- `sickle_ui` repo — https://github.com/UmbraLuminosa/sickle_ui
- `woodpecker_ui` repo — https://github.com/StarArawn/woodpecker_ui
- `bevy_lunex` repo — https://github.com/IDEDARY/bevy_lunex
- `bevy_egui` repo — https://github.com/vladbat00/bevy_egui
- Headless UI — https://headlessui.com
- Radix Primitives — https://www.radix-ui.com/primitives
- React Aria — https://react-spectrum.adobe.com/react-aria/
- Discussion #16900 (JS prior-art lineage) — https://github.com/bevyengine/bevy/discussions/16900
- Buiy foundation goals — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling: [`../bevy-ui/ecosystem.md`](../bevy-ui/ecosystem.md), [`../bevy-feathers/`](../bevy-feathers/)
