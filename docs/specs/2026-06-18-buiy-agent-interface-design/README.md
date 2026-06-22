# Buiy agent-interface design — semantic substrate for screen readers, in-process tests, and LLM agents

**Date:** 2026-06-18
**Status:** draft

## Purpose

The **agent-interaction surface** for Buiy: the one semantic tree a screen reader, Buiy's own headless test driver, and (later) an external LLM agent all read and drive. Multi-file spec; this README is the index, holds the locked decisions, rejected alternatives, scope/non-goals, and load-bearing risks, and points at the children in reading order.

Headline shape: **one canonical AccessKit semantic tree, N consumers.** The same decomposed component model an AT reads outbound is the addressing space an agent drives inbound — through **one** action ingress, **one** generic `perform` primitive, and **one** in-process inspect+control contract a networked transport later wraps unchanged.

## Accessibility leads (LOCKED #5)

This spec **owns the accessibility substrate** — decomposed `A11yStates`/`A11yRelations`, real ECS-tree nesting, ACCNAME 1.2, per-widget WAI-ARIA APG action contracts. It **folds in and supersedes the planned `buiy-accessibility-design` roadmap slot** (forward-referenced in `docs/specs/2026-05-07-buiy-foundation/accessibility.md` and in code: `a11y/translate.rs`, `buiy_widgets/src/text_input.rs:83-88`, `buiy_widgets/src/button.rs` TODOs). One a11y design — this one. No megacomponents. (See risk #6 for the docs-index supersede obligation.)

## Children (reading order)

1. [semantic-tree.md](./semantic-tree.md) — decomposed component model, accesskit 0.24 derive fold, ACCNAME 1.2, real ECS nesting + `owns` overlay + hidden prune, merged/unmerged projection. **Read first.**
2. [action-router.md](./action-router.md) — single inbound `Action` ingress (existing `bevy_winit` `MessageReader<ActionRequestWrapper>`), `dispatch_action_request` headless fn, liveness + live capability guard, per-`Action` table, new `EditCommand::SetSelection`.
3. [widget-contracts.md](./widget-contracts.md) — `A11yContract` (one declaration drives advertise + honor) + per-widget APG contracts.
4. [inprocess-api.md](./inprocess-api.md) — transport-agnostic `snapshot`/`perform`, `get_by_role`, no-sleep actionability loop, new lowest `buiy_verify` semantic-tree tier, off-by-one ref fix.
5. [verification.md](./verification.md) — headless gates #3/#4/#6/#7/#12, green with no winit adapter and no GPU.
6. [mcp-companion.md](./mcp-companion.md) — **Phase 2, opt-in** `buiy_mcp` transport envelope over the unchanged in-process contract. Ships zero of itself in Phase 1.
7. [phasing.md](./phasing.md) — review-gated phases (0 → 1a–1d → 2), follow-ups, open questions, risks.

## Scope (LOCKED #1) — full stack, phased

- **Substrate in `buiy_core`** — decomposed semantic tree + in-process inspect/control. The foundation deliverable.
- **`buiy_mcp` is an OPT-IN companion crate (Phase 2)** — networked transport only, gated on user go-ahead. Networking/persistence/security is foundation **non-goal #1**, wholly outside `buiy_core`.

## Locked decisions

1. Scope = full stack, phased. Substrate in `buiy_core`; `buiy_mcp` opt-in (Phase 2). Networked transport is an opt-in/app concern (foundation non-goal #1).
2. Phase 1 = the in-process substrate + Buiy's OWN headless test driver. No transport/security yet. Lights gates #3/#4/#6/#7 (+ #12) headless — no winit, no GPU.
3. Action model = AccessKit Actions as the single inbound channel + `Action::CustomAction(i32)` registry for app verbs. ONE ingress; one generic `perform()`; wrappers are thin sugar.
4. Addressing = AccessKit `NodeId` (`entity.to_bits()+1`) is the canonical ref. FIX the off-by-one: `buiy_verify::a11y::snapshot_tree` emits raw `to_bits()` today. Author test-ids are a NAMED Phase-2 follow-up.
5. ACCESSIBILITY LEADS. Owns decomposed `A11yStates`/`A11yRelations` + real nesting + per-widget APG contracts; folds in and supersedes the `buiy-accessibility-design` slot. No megacomponents (#17644).
6. Inbound seam = the EXISTING `bevy_winit` `MessageReader<ActionRequestWrapper>`; NO competing `ActionHandler`. Headless via `dispatch_action_request(&mut World, &ActionRequest)`.
7. Base = accesskit 0.24 / Bevy 0.19-rc.3 — the BSN/0.19 bump LANDED (PR #70, main @ `3b3b0ba`); this is the current base, no version gate remains. Verify 0.24 signatures against the resolved deps (`cargo tree` / `cargo doc`), not docs.rs.

## Rejected alternatives {#rejected-alternatives}

Linked by the children as `#rejected-alternatives` and `#rejected`.

<span id="rejected"></span>

- **A competing AccessKit `ActionHandler`.** The `accesskit_winit::Adapter` is structurally single-occupant; `bevy_winit` already installs the one handler and forwards every `ActionRequest` onto a `MessageReader<ActionRequestWrapper>`. A second handler is impossible/wrong. Rejected for one reader system draining that channel, plus `dispatch_action_request` for headless (LOCKED #6). See [action-router.md](./action-router.md).
- **`bevy_a11y`-style megacomponents.** The #17644 anti-pattern — flipping `checked` dirties everything, BSN can't patch one field. Rejected for one tiny `Reflect` component per concept (the two surgical exceptions are justified in [semantic-tree.md](./semantic-tree.md) §1).
- **Two stored trees (merged + unmerged).** Double build + divergence risk. Rejected for ONE canonical unmerged tree with merged as a read-time projection ([semantic-tree.md](./semantic-tree.md) §8).
- **A purely role-static `actions()` set as the whole capability model.** Can't express a now-read-only/disabled instance dropping a verb. Rejected as-the-whole-story; kept as the advertisement layer with the router's live per-instance filter on top ([action-router.md](./action-router.md) §3).
- **`CustomAction` as the structured app-verb channel in Phase 1.** 0.24 `CustomAction` is an `i32` index only. Rejected: ship `i32 → verb` honestly; structured name+args defer to the `buiy_mcp` RPC lane ([mcp-companion.md](./mcp-companion.md) §5.5).
- **A parallel test driver beside the production protocol.** The flutter_driver/integration_test/MCP fragmentation. Rejected for one contract over an in-process channel the socket later wraps unchanged (React DevTools Bridge/Wall). See [inprocess-api.md](./inprocess-api.md) §7.
- **Routing agents through the Bevy Remote Protocol.** BRP pokes raw ECS by reflection — no semantic model, no APG contract, no live filter. Rejected as the agent plane (kept as a debug hatch). See [mcp-companion.md](./mcp-companion.md) §7.

## Risks {#risks}

Full register in [phasing.md](./phasing.md#risks); load-bearing summary: (1) wide `build_tree` query tuple (~22 components) vs query-arity limits; (2) one-frame winit inbound latency, sidestepped by the headless seam; (3) same-frame despawn races — typed `ActionError` must propagate loudly; (4) NodeId is session-stable, not human-stable until test-ids; (5) the 0.24/BSN bump has landed (PR #70) — this coupling risk is retired; keep the derive fold isolated and verify resolved 0.24 signatures via `cargo tree`/`cargo doc`; (6) supersede-don't-contradict — update `docs/README.md` so readers don't see two parallel a11y entries. The slot was a forward reference only (never a catalog entry), so the index change is a fresh **add** that claims the a11y territory.

## Prior-art consulted

Real `docs/prior-art/` folders this spec leans on:

- [../../prior-art/bevy-a11y/component-model-incident.md](../../prior-art/bevy-a11y/component-model-incident.md) — the `AccessibilityNode` megacomponent anti-pattern (#17644) this spec inverts.
- [../../prior-art/accesskit/tree-model.md](../../prior-art/accesskit/tree-model.md), [../../prior-art/accesskit/api.md](../../prior-art/accesskit/api.md), [../../prior-art/accesskit/capabilities.md](../../prior-art/accesskit/capabilities.md), [../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md) — NodeId/TreeUpdate/single-root, the Node setter surface, what the schema can and can't express (Actions as the verb channel; no key bindings), agent-control/decomposed-component lessons.
- [../../prior-art/wai-aria-apg/](../../prior-art/wai-aria-apg/) — pattern catalog, keyboard contracts, roles/states/properties, name-computation, live-regions, focus-management, WCAG 2.2 AA mapping.
- Foundation a11y target state [../2026-05-07-buiy-foundation/accessibility.md](../2026-05-07-buiy-foundation/accessibility.md) (this spec's slot) and harness [../2026-06-15-buiy-verification-design/README.md](../2026-06-15-buiy-verification-design/README.md).

**Research debt (named, not assumed):** the **transport** half (MCP envelope, browser-automation actionability, engine-devtools Bridge/Wall, BRP) is **not yet backed by `docs/prior-art/` folders** — no `llm-agent-interface/`, `browser-automation/`, `engine-devtools-protocols/`, or `bevy-remote-protocol/` corpus exists today. Those designs (in [mcp-companion.md](./mcp-companion.md) and the actionability loop in [inprocess-api.md](./inprocess-api.md)) name their external references in prose and are marked forward-looking. **Run `researching-prior-art` to create those folders before Phase 2 transport lands.** The substrate half (Phase 0–1) is fully grounded in the real folders above.
