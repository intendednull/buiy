# Track C / C4 — Builder→Bundle design review + scope decision

**Date:** 2026-07-02. **Status:** design gate for C4 (the largest Track C slice). Produced by a multi-agent design workflow (4 per-widget-group design proposals → coherent synthesis with scope recommendation → 3 adversarial review lenses: correctness, call-site ripple, LLM-ergonomics). Feeds the C4/C5 sections of `docs/plans/2026-07-02-track-c-coherent-surface.md`; realizes/tempers spec `docs/specs/2026-07-01-first-class-llm-dev-support-design.md` §3.1.

## TL;DR

C4-as-planned ("a Builder→Bundle for every one of the 11 widgets, children attached by an `On<Add>` observer, scene-fns deprecated in C5") is **over-scoped, and the observer mechanism is the wrong default**. C1 (prelude), C2 (accessors), C3 (typed events) already banked the top LLM-ergonomics wins the prototype *measured*. C4's genuine new capability is exactly one thing: **a named builder with chainable setters that store the real component** — `Checkbox::new("Dark mode").checked(true)` → real `A11yToggled` (the spec §3.1/§6 headline). That applies only to the **setter-bearing** widgets (Checkbox `.checked`/`.indeterminate`, Switch `.on`, Disclosure `.expanded`; Slider's range is already carried by its 5-arg `new`). For every other widget the author writes the identical `Widget::new(args)` line before and after — the builder buys nothing.

## What the review found (blockers / majors)

1. **The `On<Add>` observer delivers zero author-facing value** (llm + ripple, major). Today's `impl Bundle` constructors already hide `children![…]`, so `spawn(Checkbox::new("x"))` already auto-attaches children with no author `.with_children`. The observer merely *relocates* where fixed children attach (internal architecture), while adding real cost.
2. **Observer is not idempotent on respawn** (correctness, major). The builder-only trigger component is `register_type`'d, so a `DynamicScene` load, hot-reload respawn, or **MVU whole-UI replay** reconstructs the entity with *both* the trigger *and* the deserialized `Children` → `On<Add>` fires and spawns a **second** mark+label / track+thumb. This collides with the active hot-reload + MVU-replay campaigns (the very ones §3.1 says it co-designs with). Fix if kept: an idempotency guard (early-return if the signature child already exists) or skip-serialize the trigger.
3. **Duplicate-component runtime panic** (ripple, BLOCKER). If a builder carries `A11yToggled`, shipped tuples like `world.spawn((Checkbox::new(""), A11yToggled(want), ControlledLeaf, …))` (`buiy_view` `reconcile.rs`, `todomvc_view`) and the `switch.rs:285` test have `A11yToggled` twice → bevy panics ("Bundle has duplicate components"). These are **required** call-site migrations, not no-ops.
4. **`bsn!` merge-patch has no builder replacement** (ripple, major). A builder is a spawn-time `impl Bundle`; it *cannot* be field-merged in `bsn!{ button("Save") BoxModel{…} }` like the scene-fns' `impl Scene`. `hello_bsn:67`, `gallery:2032` break with no replacement ⇒ **scene-fns must stay** (C5's "deprecate scene-fns / move to builder" is a category error for `bsn!` authors).
5. **The §4.1c trap is NOT dissolved** (llm, BLOCKER). Markers must stay preluded — they are the accessor namespace (`Checkbox::checked`) and the `With<Checkbox>` query filter. So the trap-prone spelling (`spawn((Checkbox, …))` / `bsn!{ Checkbox { … } }` single-field-patch of a `#[require]`'d component) stays on the agent's *default* surface. A builder is additive, not a replacement. Don't claim the trap is closed; and do **not** delete `position_popover`'s defensive per-frame `Stacking` re-assert while the trap-prone path is still reachable.
6. **C5 ripple map is wrong** (ripple, major): scene-fns do **not** back `counter_view`/`todomvc_view` (those use `buiy_view`'s own Element builders). The only real coupling is the resolved glob-collision. The genuine dependents are `bsn!` authors.
7. **Descope creates a per-widget asymmetry** (llm, major): an LLM can't predict which widgets chain (`Checkbox::new(l).checked(true)` vs `Dialog::new(t,b)`). Mitigate via the Track D cookbook (enumerate constructor form + setters per widget); consider `Slider::new(label).range(min,max).value(now).step(s)` for consistency.

## What is genuinely worth building (if C4 proceeds)

- **Named builders + setters** for the setter-bearing widgets: **Checkbox** (`.checked`/`.indeterminate`), **Switch** (`.on`), **Disclosure** (`.expanded`), + **Button** (label-only, to cement the pattern + the §6 acceptance). Setters store the real component (`self.toggled = A11yToggled(…)`).
- **Popover** §4.1c correctness fix (a real human-DX win, independent).
- Keep everything else (`Dialog`/`Tooltip`/`Menu`/`MenuButton`/`MenuItem`/`TextInput`/`ScrollArea`/`Slider`) as `impl Bundle`.
- **Keep the `On<Add>` observer only if** it carries an idempotency guard + a pinned synchronous-flush test; the reviews' cheaper alternative is a `children!`-Bundle-**field** on the builder (identical author line, no observer/keying/flush/snapshot risk — its only downside is an ugly field type).
- **Keep scene-fns**; do not deprecate them in C5 (bsn! needs them). C5 shrinks to near-nothing.

## Decision

**Deferred to the user** (a material scope change from the approved plan). See the campaign thread. The mechanism/keying details, the synchronous-flush timing resolution (from bevy_ecs source: `world.spawn().id()` flushes; `commands.spawn` defers), and the full per-widget proposals are in the workflow transcript (run `wf_28bee3d8-029`, 2026-07-02).
