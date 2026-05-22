**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — system-specific terminology glossary

# Glossary

System-specific terms used throughout this folder. AccessKit-specific terms appear first; assistive-technology and platform-API terms follow. Definitions are scoped to how the term is used in the AccessKit / Buiy context, not the broader accessibility literature.

## AccessKit core types

- **`Tree`** — The root-level metadata struct: `{ root: NodeId, toolkit_name: Option<String>, toolkit_version: Option<String> }`. Identifies which node is the tree root and surfaces the producer toolkit's name + version to the AT for diagnostic purposes. Buiy sets `toolkit_name: Some("Buiy")`. Lives inside the consolidated tree owned by the adapter; the producer constructs `Tree` values on each `TreeUpdate.tree = Some(...)` but does not retain them.
- **`TreeUpdate`** — `{ nodes: Vec<(NodeId, Node)>, tree: Option<Tree>, tree_id: TreeId, focus: NodeId }`. The diff payload the producer pushes to the adapter on every accessibility-relevant change. `nodes` carries only changed nodes after the initial activation. `tree: Some(_)` only on initial activation or when toolkit metadata changes; `None` for routine diff updates. `focus` is required on every update.
- **`Node`** — A single producer-side accessibility unit: role + label + value + state flags + geometry + relations + supported actions. Built via `Node::new(Role)` followed by setters (`set_label`, `set_value`, `set_bounds`, `add_action`, etc.). Plain `Send + Sync` data — safe to build on any thread.
- **`NodeId`** — Opaque `u64` wrapper (`pub struct NodeId(pub u64)`, `#[repr(transparent)]`). Producer-chosen value space. Buiy derives from `Entity::to_bits()`. Per-tree scoped — two windows can use overlapping numeric values without conflict.
- **`TreeId`** — Identifies which tree a `TreeUpdate` belongs to. Single-window apps use the default; multi-window apps allocate one `TreeId` per window.
- **`Role`** — Flat `#[repr(u8)]` enum with 182 variants at 0.24.0. Frequency-ordered for serialization efficiency (not alphabetical). Closed set; no custom roles. Fall back to `Role::Generic` + `set_role_description(str)` when no variant fits.
- **`Action`** — Closed enum, 22 variants at 0.24.0: `Click`, `Focus`, `Blur`, `Collapse`, `Expand`, `CustomAction`, `Decrement`, `Increment`, `HideTooltip`, `ShowTooltip`, `ReplaceSelectedText`, `ScrollDown`, `ScrollLeft`, `ScrollRight`, `ScrollUp`, `ScrollIntoView`, `ScrollToPoint`, `SetScrollOffset`, `SetTextSelection`, `SetSequentialFocusNavigationStartingPoint`, `SetValue`, `ShowContextMenu`. Producer marks supported actions via `Node::add_action(Action::...)`.
- **`ActionRequest`** — `{ action: Action, target: NodeId, data: Option<ActionData> }`. The struct an AT sends to the producer's `ActionHandler::do_action(...)` to request an operation on a specific node.
- **`ActionData`** — Variants carry per-action payloads: `Value(String)` for `SetValue` text controls, `NumericValue(f64)` for `SetValue` numeric controls, `SetTextSelection(TextSelection)` for caret placement, `ScrollToPoint(Point)`, `CustomActionIndex(u32)` for custom-action dispatch.
- **`Toggled`** — `enum Toggled { False, True, Mixed }`. AccessKit's unification of `aria-checked` and `aria-pressed`. `Mixed` is the tri-state value used by tri-state checkboxes and partially-applied "select all" toggles.
- **`Invalid`** — `enum Invalid { False, True, Grammar, Spelling }`. Maps `aria-invalid`. `Grammar` and `Spelling` get different AT verbalisations than generic `True`.
- **`Live`** — `enum Live { Off, Polite, Assertive }`. Live-region politeness. `aria-relevant` is NOT in AccessKit; the producer layers relevance filtering on its own side.
- **`AccessibilityRequested`** — The activation-state signal. The adapter notifies the producer when an AT attaches; until activation, building the tree is wasted work. Exposed in `accesskit_winit` as the gate inside `Adapter::update_if_active(...)` and surfaced in Buiy as a resource the `BuiySet::A11yUpdate` system reads.

## Adapter API

- **`Adapter`** — `accesskit_winit::Adapter`. The per-window adapter handle that owns the platform-specific adapter (`accesskit_windows::Adapter`, `accesskit_macos::Adapter`, etc.) and the consumer-side tree cache. One per `winit::Window`. Constructed before the window is visible — all three constructors panic otherwise.
- **`ActivationHandler`** — Trait the producer implements; the adapter calls `request_initial_tree() -> Option<TreeUpdate>` the first time an AT attaches. Producer returns a complete `TreeUpdate` with `tree: Some(...)` and every node in the world payloaded into `nodes`.
- **`ActionHandler`** — Trait the producer implements; the adapter calls `do_action(ActionRequest)` every time the AT invokes an action. Producer routes the request to whatever widget owns the target `NodeId`.
- **`DeactivationHandler`** — Trait the producer implements; the adapter calls `deactivate_accessibility()` when the AT detaches. Producer can drop cached tree state and stop pushing updates until reactivation.
- **`accesskit_consumer`** — Internal library (crate name) that adapters use to walk / diff / cache the tree. Despite the name, it sits on the **adapter side** of the boundary. **Not for direct application integration** — application code never depends on `accesskit_consumer`. The consumer/README is explicit: "not meant for direct application integration."

## Producer / consumer (the terminology trap)

- **Producer** — The UI toolkit that creates the `Tree` + `Node` data and calls `adapter.update_if_active(...)`. The *source of truth* side. Buiy is the producer. So are egui, Slint, Freya, Xilem/Masonry, and Bevy via `bevy_a11y`. The producer pushes `TreeUpdate`s.
- **Consumer** — The adapter-side code (i.e. the `accesskit_consumer` crate inside `accesskit_windows`, `accesskit_macos`, `accesskit_unix`, `accesskit_android`, `accesskit_ios`) that reads the producer's tree, diffs / caches it, and exposes it through the platform AT API. The consumer consumes *the tree*. **It is NOT the assistive technology** (NVDA / VoiceOver / Orca / TalkBack reach the producer indirectly through the OS, not through `accesskit_consumer`).

## Platform adapter crates

- **`accesskit_windows`** — UI Automation (UIA) adapter. Production since 2021-12-21. Implements `IRawElementProviderSimple`, `IRawElementProviderFragment`, and the COM control-pattern interfaces.
- **`accesskit_macos`** — NSAccessibility informal-protocol adapter. Production since 2022-11-23. Attaches `NSObject` subclasses to the winit `NSView`. Known bug: issue #520 — ListBox selected state not properly communicated.
- **`accesskit_unix`** — AT-SPI 2 over D-Bus adapter. Production since 2023-01-05. Async-runtime-backed (`tokio` or `async-io` Cargo feature required). Uses the pure-Rust `zbus` D-Bus implementation. NLnet NGI0-funded.
- **`accesskit_atspi_common`** — Internal AT-SPI helper crate shared by `accesskit_unix`.
- **`accesskit_android`** — Android `AccessibilityNodeInfo` JNI adapter. Pre-1.0 (0.7.3 as of 2026-05-11; first published 2025-03-06). Two adapter shapes: `Adapter` (low-level JNI; caller supplies Java glue) and `InjectingAdapter` (embeds a precompiled `.dex` for "drop in and it works"; `embedded-dex` Cargo feature).
- **`accesskit_ios`** — UIKit UIAccessibility adapter. Brand new — 0.1.0 shipped 2026-05-11; "Basic iOS adapter." Authored by Arnold Loubriat; NLnet NGI0-funded.
- **`accesskit_winit`** — Multiplexing helper. One `Adapter` per `winit::Window`. Three constructors: `with_event_loop_proxy`, `with_direct_handlers`, `with_mixed_handlers`. Provides cross-platform `Event` / `WindowEvent` types. The recommended integration path for any winit-based toolkit, Buiy included.

## Assistive technologies AccessKit ultimately reaches

- **NVDA** — Open-source Windows screen reader. Created by Michael Curran (2006); co-led by James Teh; produced by NV Access. Reads UIA output. **Not Matt Campbell's project** — confusion between AccessKit's founder and NVDA's lineage is a common error worth correcting wherever it appears.
- **VoiceOver** — Apple's first-party screen reader (macOS and iOS). Reads NSAccessibility (macOS) or UIAccessibility (iOS) output. iOS VoiceOver has rotor-based navigation that motivates careful `aria-roledescription` tagging.
- **Orca** — GNOME / GTK Linux screen reader. Also used on KDE, Cinnamon, XFCE, Sway, Hyprland. Reads AT-SPI output via D-Bus.
- **TalkBack** — Google's Android screen reader. Reads `AccessibilityNodeInfo` output. Samsung's TalkBack-derivative and BRLTTY consume the same graph.
- **JAWS** — Commercial Windows screen reader from Freedom Scientific. Reads UIA output (and legacy MSAA via the OS shim).
- **Narrator** — Microsoft's built-in Windows screen reader. Reads UIA output.

## Platform accessibility APIs (the targets AccessKit's adapters speak)

- **UIA** — UI Automation. The modern Windows accessibility API; COM-based. Replaces legacy MSAA (MSAA still works via OS bridge but AccessKit produces UIA natively).
- **NSAccessibility** — Cocoa's macOS accessibility protocol. ObjC selectors (`accessibilityLabel`, `accessibilityValue`, `accessibilityChildren`, `accessibilityPerformPress`) on `NSAccessibilityElement` subclasses.
- **AT-SPI** — Assistive Technology Service Provider Interface (AT-SPI 2). Freedesktop.org D-Bus accessibility protocol. Used on Linux desktops. Different behaviours on X11 vs Wayland sessions (Wayland hides absolute window position).

## Spec references AccessKit aligns with

- **ARIA** — W3C Accessible Rich Internet Applications. AccessKit's schema is "WAI-ARIA-aligned" but not one-to-one; AccessKit unifies `aria-checked` + `aria-pressed` into `Toggled`, splits combobox flavours, frequency-orders `Role`.
- **APG** — ARIA Authoring Practices Guide. The keyboard-contract document for ARIA widget patterns. **NOT in AccessKit** — AccessKit models the tree, not the keyboard contracts. Implementing APG is consumer-toolkit responsibility (Buiy's `buiy-widget-catalog-design`).
- **ACCNAME 1.2** — W3C Accessible Name and Description Computation 1.2. The algorithm for computing an accessible name from labels / descriptions / content. AccessKit does **NOT** compute accessible names — it holds references via `set_labelled_by([NodeId])` + `set_described_by([NodeId])`; the consuming toolkit walks them. Buiy implements ACCNAME 1.2 in `buiy_core`.

## Bevy / Buiy-side terms relevant to the AccessKit boundary

- **`bevy_a11y`** — Bevy's first-party AccessKit producer. Created in Bevy 0.10 (PR #6874, merged 2023-03-01). Exposes a single megacomponent (`AccessibilityNode(NodeBuilder)` historically, `AccessNode(accesskit::Node)` later) wrapping the entire AccessKit Node. **BSN-hostile by Bevy's own analysis** (issue #17644). Buiy replaces `bevy_a11y` for windows Buiy owns; same-window coexistence with `bevy_a11y` is impossible (AccessKit allows only one adapter per window).
- **`BuiySet::A11yUpdate`** — The Buiy system set that walks change-tracked entities, builds `Node`s from Buiy's decomposed components, and calls `Adapter::update_if_active(...)`. Ordered after picking, before render. No-op when no AT is attached.
- **`A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations`** — Buiy's decomposed accessibility components. Small, public-fielded, observable. The BSN-friendly answer to `bevy_a11y`'s megacomponent shape; the prescribed fix per issue #17644.

## Stewardship / funding

- **Pneuma Solutions** — The cloud-accessibility company Matt Campbell co-founded in 2020 with Mike Calvo. Pneuma ships commercial products (Remote Incident Manager, Scribe, Sero, DocuScan Plus); AccessKit is Campbell's open-source work alongside Pneuma. There is no formal contractual relationship between Pneuma and the AccessKit project — relationship is "load-bearing-but-informal."
- **NLnet NGI0** — The Next Generation Internet Zero Commons Fund administered by NLnet Foundation. Funded the AccessKit AT-SPI adapter and the iOS adapter (the latter authored by Arnold Loubriat). Does not fund the core `accesskit` crate.
- **Conventional Commits + release-please** — The release automation pattern AccessKit uses. PR titles follow Conventional Commits; CHANGELOG is auto-generated; the workspace co-releases all member crates on the same day (the 2026-05-11 batch is the most recent example, with 10 simultaneous release notes).

## Sources

- `accesskit` 0.24.0 docs.rs: https://docs.rs/accesskit/0.24.0/accesskit/
- `accesskit_winit` 0.33.0 docs.rs: https://docs.rs/accesskit_winit/0.33.0/accesskit_winit/
- `common/src/lib.rs` (canonical schema): https://github.com/AccessKit/accesskit/tree/main/common/src
- `consumer/README.md`: https://github.com/AccessKit/accesskit/blob/main/consumer/README.md
- AccessKit README (Chromium lineage statement): https://github.com/AccessKit/accesskit/blob/main/README.md
- Pneuma Solutions about: https://pneumasolutions.com/about/
- NV Access about (for NVDA lineage clarity): https://nvaccess.org/about-nv-access/
- NLnet NGI0 funding: https://nlnet.nl/project/
- Bevy issue #17644: https://github.com/bevyengine/bevy/issues/17644
- Sibling files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`tree-model.md`](tree-model.md), [`platform-adapters.md`](platform-adapters.md), [`api.md`](api.md), [`integration.md`](integration.md), [`capabilities.md`](capabilities.md), [`history.md`](history.md), [`governance.md`](governance.md), [`ecosystem.md`](ecosystem.md), [`critiques.md`](critiques.md), [`lessons.md`](lessons.md)
