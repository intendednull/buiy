# Scribbl gap-audit — raw evidence

Raw structured outputs of the two multi-agent audit workflows behind
[`../2026-07-01-scribbl-app-capability-gap-audit.md`](../2026-07-01-scribbl-app-capability-gap-audit.md).
Each is an array of per-dimension results: `{dimension, title, audit: {summary, capabilities[]
(have/partial/missing + file:line evidence), gaps[]}, gaps[] (with adversarial `verdict`)}`.

- `audit-1-full-11-dimensions-STALE-BASE-4010753.json` — 66 agents, all 11 dimensions.
  **Caveat:** ran on `main @ 4010753`, 10 commits behind `origin/main`, *pre-MVU-as-core* — its
  MVU/theming/overlays/scaffolding/widget findings are superseded by audit 2; its render / vector /
  input / layout / text / animation findings are base-insensitive and stand.
- `audit-2-mvu-rerun-correct-base-6c4ff22.json` — 37 agents, the 5 MVU-sensitive dimensions re-run
  on `origin/main @ 6c4ff22` (MVU present). The `widget-inventory-2` slot is null (structured-output
  retry cap); the widget facts from audit 1 stand.

File:line evidence cites the base each audit ran against; re-verify against current main before
building (the report's resume plan mandates a re-baseline anyway).
