**Date:** 2026-07-01
**Status:** decision-input (prototype seed)

> Fresh-context panel (n=6, order-debiased, retry-covered) on which VIEW-declaration style is easier for an **LLM** to author/follow/debug: **T** = macro tree-with-attributes (paradigm ①) vs **V** = `view(&Model) -> Element<Msg>` function (paradigm ②). **Vote: V 6 — T 0 — tie 0 (unanimous for V).** This + the maintainer's lean selects **V ("safer V")** as the prototype's authoring surface.

## The verdict

**Unanimous for V, 6–0** (n=6, no ties). Every lens — authoring from scratch, modify/extend, debugging, reading/comprehension, scaling, and holistic — picked the plain `view(&Model) -> Element<Msg>` function over the reactive `view!` macro tree. Confidence was **high on 4 lenses** (author-scratch, debug-errors, scale-bigger, holistic) and **medium on 2** (modify-extend, read-comprehend). T won **zero** lenses.

The answer to "which is easier for an LLM to follow and author": **V, decisively.** The single load-bearing reason repeated by all six panelists is that **V is near-isomorphic to Iced** (`column![]`/`row![]`, `button(..).on_press(Msg)`, `text_input(..).on_input(..)`, `.iter().map(..)`, `(cond).then_some(msg)`, terminal `.into()`), which is heavily represented in training data — so the model reproduces it from strong priors rather than reconstructing a bespoke grammar. T's `view!` is "a third bespoke reactive-macro grammar that is neither Leptos `view!` nor Dioxus `rsx!`," so the model has weak priors and high cross-contamination risk.

The one honest hedge: the holistic and author-scratch lenses both note the gap is **narrow on the tiny Counter** and **widens with app size**, because bigger apps pull in conditionals, filtering, derived state, and sub-view composition — all ordinary Rust in V, all "macro-provided-or-nothing" in T.

## Why — the case that won

Four arguments recurred across nearly every opinion:

1. **"It's just Rust" = maximal prior + full fallback.** Control flow and derivation in V are host-language Rust: lists via `.iter().map(...)`, derived counts via `.filter(...).count()`, derived values via a plain `let left = ...`, conditionals via `if`/`match`/`.then_some()`. The scale-bigger lens is blunt: conditionals, child models, and derived state "are exactly the three things V expresses as ORDINARY RUST and T must express through bespoke macro grammar." Critically, when the macro doesn't provide a construct, T leaves the model "guessing a feature that may not exist — a first-try failure with no ordinary-Rust fallback."

2. **Uniform API surface vs. T's split grammar.** In V every knob is a dot-method (`.gap`, `.padding`, `.on_press`, `.on_input`, `.key`, `.into`) — "there is no classification decision to get wrong." T splits knobs into two look-alike categories: dotted builder methods (`col().gap(8.0)`, `text(...).font_size(20.0)`) vs. bare space-separated attributes (`button("−") on_press(...)`, `disabled(...)`). "Nothing in the names tells me `font_size` is a `.method()` but `placeholder` is a bare attribute." The holistic lens calls this an "internally inconsistent modifier grammar" the model "would constantly misplace."

3. **Errors land as ordinary, precisely-spanned Rust diagnostics.** The debug lens is emphatic: a wrong V view is a wrong ordinary-Rust expression, so you get "`no method named on_click, did you mean on_press`" pinned to the call, `expected Option<Msg>, found bool`, or a standard borrow error — "diagnostics I map to a fix reliably." T's mistakes "surface as proc-macro expansion errors referencing generated tokens I never wrote and cannot see — the exact regime where I debug poorly." Because `column!`/`text!` are thin `vec!`/`format!`-shaped collectors, even V's macro cases "degrade to familiar expression/format diagnostics rather than DSL-parser errors."

4. **Explicit data flow vs. magic `m`/`t`.** V takes `state: &Counter` and reads `state.count` — explicit parameter, plain field access. T injects a "magic `m`" (and `t` inside row closures) that must be known to be in scope, plus a `bind!` wrapper required only when reading the model. "Deciding when `bind!` is required vs forbidden, and which magic identifier is live in a nested `view!`, is pure recall with no prior to anchor it."

## The honest case against it / for the other

No lens flipped to T, but the panel was consistent about T's real advantages:

- **T steers toward correctness on two axes V leaves as footguns.** `for_each(..., key: |t| t.id, ...)` makes the list key a **required argument you can't forget**, whereas V's `.key(id)` is an optional chained method whose omission is a *silent* reconciliation bug (see below). And T's declarative `disabled(bind!(..))` is "more discoverable" than V's non-obvious `on_press_maybe((cond).then_some(Msg))`. The scale and holistic lenses both concede mandatory keys are "a meaningful T advantage" / "T's one genuine scaling edge."

- **T avoids closure/borrow ceremony.** T's event handlers take plain message **values** (`on_toggle(TodoMsg::Toggle(t.id))`), sidestepping V's `let id = todo.id; move |_| Msg::Toggle(id)` capture dance and the lifetime/borrow errors that come with borrowing `&state`/fields inside a returned `Element`.

- **T's tree shape aids reading and insertion location.** Multiple lenses grant that T's indented `Children [ ]` is "arguably more scannable" for pure structural "what does the tree look like" reading, and makes the insertion *location* "visually unambiguous when adding a widget to a deep tree." T is also more concise with less `.into()`/generic noise, and its in-place mutation is a runtime win (though that's a performance, not authoring, concern).

- **Where it's close:** the tiny Counter example. author-scratch: "Close only on the tiny Counter." holistic: T's correctness-steering means "V's edge is not a blowout on correctness-steering, only on debuggability and training-distribution fit."

## LLM failure modes, per style

| How an LLM gets **T** wrong | How an LLM gets **V** wrong |
|---|---|
| **Hallucinates unshown grammar** — invents a conditional-child construct (`if bind!(m.x){}`, `when(..)`, Leptos `<tag/>`, Dioxus `div {}`) because none is shown. Highest-risk exactly where scaling lives. | **Omits `.key(id)`** on mapped rows — *compiles cleanly*, silent reconciliation bug. Named the "single most dangerous V failure" / "highest-severity miss." |
| Mixes method-vs-attribute placement: `.on_press(...)`/`.disabled(...)` (dotting a bare attr) or `font_size(20.0)` as a bare attr. | Closure `move`/capture slips — forgetting `let id = todo.id;` before `move`, or letting a borrow of the loop item escape. |
| Forgets `bind!` around a reactive read (`text("Count: {}", m.count)` silently never updates) **or** over-wraps a static literal in `bind!`. | Forgets the trailing `.into()` coercion to `Element<Msg>`. |
| Wrong `Children [..]` shape/separators (`children:`, `.children(..)`, comma-vs-space confusion). | Confuses `column![a,b]` (macro, static) with `column(iter)` (fn, dynamic) — picks the wrong form. |
| Hallucinates `for_each` arg names/order (`item:`/`view:` instead of `row:`; `id:` instead of `key:`). | Doesn't know `on_press_maybe`/`.then_some` exists → hand-rolls disable, or writes an `if` returning a *structurally different* element that defeats keying/diff. |
| Wrong magic identifier (`m` where only `t` is live; `model.count` instead of `m.count`). | Message-lifting boilerplate at scale — omitting `.map(ParentMsg::Child)` or wrong wrapper variant. |
| **Detonation:** opaque macro-expansion errors, coarse spans, references to generated code — poor debug loop. | **Detonation:** mostly clean rustc "did you mean"/type/borrow errors at the real span — except the silent `.key()` miss. |

The asymmetry the panel keeps returning to: **V's failures are recoverable (compiler-pinned or localized), T's are opaque (macro-expansion) or silent (missing `bind!`).** The one V failure that breaks this pattern — the missing `.key()` — is the panel's loudest warning about V.

## Caveats & anything worth stealing

- **This is LLM-authorability only.** The panel explicitly brackets human readability and performance. T's in-place mutation is "a runtime, not authoring, concern," and T's conciseness/clean tree shape may well favor human readers. Don't read a 6–0 LLM verdict as a verdict on the style overall.

- **The verdict is partly about the *examples shown*, not just the styles.** Several lenses hinge on T's docs showing **no conditional and no child-model pattern**, forcing the model to fabricate one. The scale lens's explicit mitigation: "the highest-leverage mitigation is documenting the conditional and child-model macro forms explicitly, since those absences are what an LLM will most confidently fabricate." Much of T's disadvantage may be a documentation gap, not an intrinsic one — worth testing before concluding.

- **Two concrete fixes that would narrow the gap if you keep T:** (1) "drop the dotted-vs-bare attribute inconsistency (make ALL modifiers method-chained)"; (2) make macro errors span-preserving back to source. The holistic lens: "the gap would narrow a lot."

- **Steal T's key-safety for V.** The clearest cross-pollination: V's `.key()` landmine should be closed by a **make-illegal-states-unrepresentable API** — the panel suggests `keyed_column(iter, |x| x.id, ...)` making the key a *required* argument, mirroring T's mandatory `for_each(key: ..)`. Absent that, at minimum a lint. This is the one place T is strictly safer and V should copy it.

- **The hybrid worth prototyping:** keep V's plain-Rust authoring surface and error legibility (the win) while recovering T's single-retained-tree/in-place-mutation performance under the hood via the reconciler that already diffs V's rebuilt `Element` onto the retained tree — plus adopt T's *required-key* and *declarative-disable* ergonomics as V API affordances (`keyed_column`, keep/promote `on_press_maybe`). That captures T's two genuine correctness-steering wins without importing its bespoke grammar.
