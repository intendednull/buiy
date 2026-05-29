**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium) — the style engine: `ComputedStyle` (the large generated shared struct + the "megastruct" maintenance critique), the cascade, style recalc, invalidation sets, and custom properties; contrasted with Buiy's decomposed-component authoring model

# Blink: the style engine

Blink is the canonical implementation of the CSS cascade. When a browser turns `color: red` plus a hundred inherited and reset defaults into a concrete value an element paints with, the data structure that holds the answer is `ComputedStyle`, and the algorithm that fills it is the cascade plus style recalc. Buiy implements a typed-Rust *subset* of the same CSS semantics, so Blink's style engine is load-bearing prior art for what Buiy deliberately does *not* build: Buiy has no cascade, no selector matching, and no single mega-struct. This file maps Blink's style subsystem and draws the contrast that the [lessons.md](lessons.md) synthesis turns into a Validates/Avoid pair.

See [architecture.md](architecture.md) for where Style sits in the RenderingNG lifecycle, [layout.md](layout.md) for how `ComputedStyle` feeds LayoutNG, and [stacking-and-paint.md](stacking-and-paint.md) / [containment-and-queries.md](containment-and-queries.md) for the style-derived triggers (opacity, transform, `contain`) those passes read.

## 1. `ComputedStyle` — one struct per element

Every element that participates in layout has a `ComputedStyle`: the fully-resolved, post-cascade value of every CSS property the element supports. It is read by layout, paint, hit-testing, the compositor input phase, and the accessibility tree. It is the single source of truth that downstream phases consult — the equivalent role to Buiy's resolved-layout outputs, but Blink packs all of style into one object rather than many components.

`ComputedStyle` is created during the **Style** lifecycle phase and, post-BlinkNG, is **immutable** thereafter. That immutability is recent and hard-won (see §5): the `DocumentLifecycle` class now enforces that `ComputedStyle` is only mutated while in the `kInStyleRecalc` state, and that nothing dirty remains once the lifecycle reaches `kStyleClean`.

## 2. The "megastruct" and how Blink fights its own size

CSS has hundreds of longhand properties. A naïve `ComputedStyle` with one field per property, instantiated once per element, would consume large amounts of memory on a heavy page. Blink mitigates this with three techniques, none of which removes the underlying fact that `ComputedStyle` is a very large generated type:

- **Code generation.** The bulk of `ComputedStyle` is a generated base class, `ComputedStyleBase`, produced by `make_computed_style_base.py` from the property database `css_properties.json5`. Fields, getters, setters, the equality/diff comparison, and the field packing are emitted, not hand-written. Adding a property is editing JSON5, not editing a hand-maintained struct.
- **Rare-data groups.** Fields are partitioned into frequently-used inline fields and "rare" groups (historically `StyleRareNonInheritedData`, `StyleRareInheritedData`, and finer-grained generated groups). Rare groups are heap-allocated lazily and only when a non-default value is set, so the common element pays for the common fields only.
- **Data sharing / copy-on-write.** Rare-data groups are reference-counted and shared between `ComputedStyle` objects that have identical values for that group; a write triggers a copy. Many sibling elements end up sharing the same backing data.

Even with all this, `ComputedStyle` is the textbook example of a struct that grew with the platform. The **maintenance critique** is real and acknowledged in Blink's own engineering: the diffing logic (deciding whether a style change needs layout, paint, or just compositing) must account for every field; the generated groups exist precisely because hand-maintaining the packing became untenable; and a change to one property's classification ripples through generated diff code. The struct is correct and fast, but it is a monument to the cost of a single growing type owning every property — which is the exact shape Buiy refuses (§7).

## 3. The cascade

`ComputedStyle` is the *output*; the **cascade** is the algorithm that produces it. Blink's `StyleResolver` and `StyleCascade` (`core/css/resolver/`) implement the CSS Cascade module: collect every declaration that applies to an element, then resolve conflicts by, in order, origin and importance, cascade layers (`@layer`), specificity, and source order. Inputs are the matched rules from selector matching plus inline style, animations, and presentation hints.

Blink tracks Cascade-and-related modules as the reference implementation: `@layer` (cascade layers) shipped via the Blink launch process (intent-to-prototype → intent-to-ship on `blink-dev`; see [governance.md](governance.md)). The cascade is where `var()`, `calc()`, `revert`, `revert-layer`, and `!important` are resolved into the single value written to `ComputedStyle`.

Buiy has **no cascade**. A Buiy `Style` builder writes component fields directly; there is no origin/specificity/layer conflict resolution because there is no selector-matched rule soup to resolve. This is a deliberate scope cut, not an omission — see [comparisons.md](comparisons.md) and `bevy_flair`, which *does* lease a real cascade (the Servo `selectors` crate) for a CSS-over-Bevy stylesheet layer ([../bevy-flair/architecture.md](../bevy-flair/architecture.md)).

## 4. Style recalc

Style recalc is the per-frame work of bringing `ComputedStyle` up to date after the DOM or stylesheets change. The pre-NG version interleaved with layout and was hard to reason about; post-BlinkNG it is a bounded phase that walks the elements flagged as dirty, re-runs the cascade for each, produces fresh immutable `ComputedStyle` objects, and stops. The naïve alternative — recalculating the whole document on any change — is what invalidation sets exist to avoid.

## 5. Invalidation sets

When a class is toggled or an attribute changes, Blink must answer "which elements' `ComputedStyle` could this have changed?" without re-matching every selector against every element. **Invalidation sets** are the answer. The `RuleFeatureSet` compiles the loaded stylesheets once into indexed `InvalidationSet`s: for a given change (e.g. adding class `c1` to an ancestor), the set names which descendants/siblings could now match differently, so only those are scheduled for recalc.

The design doc is explicit that the mechanism is conservative — it **over-invalidates** rather than risk missing an element: invalidation sets "err on the side of correctness, so we invalidate elements that do not need recalculation but these are significantly better than recalculating everything." There are immediate invalidations (applied at the mutation) and pending invalidations (accumulated and flushed when style is next read). When a change is too broad to localize, Blink falls back to a subtree or whole-document recalc.

Buiy's analogue is Bevy ECS change detection: `Changed<T>` filters mark which entities' components moved, and a system only processes those. The selector-free design means Buiy never needs Blink's `RuleFeatureSet` machinery — there is no rule database whose match-set could shift. ECS change-detection is coarser (per-component, not per-rule-feature) but vastly simpler, and the layout pipeline's first step (`RemovedNodesGc`) plus per-step `Changed<T>` queries are the whole story. See [architecture.md](architecture.md) and the Buiy foundation pipeline ([../../specs/2026-05-07-buiy-foundation/architecture.md](../../specs/2026-05-07-buiy-foundation/architecture.md)).

## 6. Custom properties (CSS variables)

Custom properties (`--foo`) and registered custom properties (`@property` / `registerProperty()`) are stored on `ComputedStyle` in dedicated variable maps, separated by whether the property inherits. Registered properties carry a `syntax`, an `inherits` flag, and an optional `initial-value`; per Chrome's documentation they are **validated when computed, not when parsed**, and declaring `inherits: false` lets Blink skip re-parsing descendants when the value changes, narrowing recalc scope. This is one more place where the cascade's complexity — substitution, cycle detection, type checking — lands inside the one `ComputedStyle` struct.

Buiy has no `var()` substitution layer. The equivalent of a design token in Buiy is an ordinary Rust binding or a Bevy resource read at authoring time, not a cascaded runtime variable. The tradeoff: Buiy loses runtime theming-by-cascade, and gains the absence of an entire substitution/cycle-detection subsystem.

## 7. The contrast: decomposed components vs. the megastruct

This is the central Validates/Avoid pair for Buiy.

**What Blink validates:** a fully-resolved style snapshot, computed once per frame in a bounded phase, immutable thereafter, that every downstream phase reads without recomputing. Buiy adopts the same contract — *layout writes, render reads* — and the same hard lesson BlinkNG learned: downstream phases must never mutate the resolved data (§5; [architecture.md](architecture.md)).

**What Buiy avoids:** the single growing struct that owns every property. Buiy's authoring model is ECS + **decomposed, public-fielded components** — `Style`, `Position`, `Stacking`, `UiTransform`, `Containment`, and so on — with an explicit **no-megacomponents** rule. Where Blink centralizes every property into `ComputedStyleBase` and pays for it with code-generation, rare-data groups, copy-on-write sharing, and field-aware diff logic, Buiy spreads properties across small components and lets:

- **Memory** be paid per-component: an entity without a transform simply has no `UiTransform`, the same win Blink's rare-data groups chase, but achieved structurally (component presence) rather than via a lazy heap group inside one struct.
- **Change detection** be per-component (`Changed<UiTransform>`), so a transform tweak doesn't touch the machinery that owns color — no field-by-field diff classification needed.
- **Maintenance** be additive: a new feature is a new component (Phase 8's `UiTransform` + `Containment`, Phase 9's `Stacking`), not a new field threaded through one struct's getters, setters, packing, and diff.

The risk Buiy accepts is the inverse: many small components mean more types to register/reflect and the burden of getting the resolution *order* right across systems (the `PostTaffyOverrides` sub-pass chain 6a–6f exists precisely to sequence the cross-component dependencies that Blink resolves inside a single recalc). Blink trades one giant struct's maintenance cost for a simple data-flow; Buiy trades many-components' ordering cost for additive, change-detected, structurally-sparse style. Both are defensible; Buiy's bet is that decomposition ages better in an ECS than a megastruct does in C++. See [open-problems.md](open-problems.md) for where decomposition's ordering cost bites, and [critiques.md](critiques.md) for the megastruct critique in full.

## Sources

- Blink announcement (WebCore fork, 2013-04-03; Chrome 28+) — https://blog.chromium.org/2013/04/blink-rendering-engine-for-chromium.html
- RenderingNG deep-dive: BlinkNG (`DocumentLifecycle`, `ComputedStyle` immutability, pre-NG "leaky abstraction barriers", `ComputedStyle` mutated by later stages) — https://developer.chrome.com/docs/chromium/blinkng
- CSS Style Invalidation in Blink (invalidation sets, `RuleFeatureSet`, "err on the side of correctness") — https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/blink/renderer/core/css/style-invalidation.md
- CSS Style Calculation in Blink (style recalc, cascade flow) — https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/blink/renderer/core/css/style-calculation.md
- `StyleCascade` source (`core/css/resolver/style_cascade.h`) — https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/blink/renderer/core/css/resolver/style_cascade.h
- `css_properties.json5` (property database driving `ComputedStyleBase` generation) — https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/core/css/css_properties.json5
- `computed_style.h` (the `ComputedStyle` struct, generated base) — https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/style/computed_style.h
- Registered custom properties / `@property` (validated when computed; `inherits: false` narrows recalc) — https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/At-rules/@property
- `@property` performance (recalc-scope benefits) — https://web.dev/blog/at-property-performance
- Sibling prior-art: [architecture.md](architecture.md), [layout.md](layout.md), [stacking-and-paint.md](stacking-and-paint.md), [containment-and-queries.md](containment-and-queries.md), [critiques.md](critiques.md), [open-problems.md](open-problems.md), [comparisons.md](comparisons.md), [governance.md](governance.md), [lessons.md](lessons.md)
- Related corpus: [../bevy-flair/architecture.md](../bevy-flair/architecture.md) (a real CSS cascade over Bevy, via Servo `selectors`)
- Buiy specs: [../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md), [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md), [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)
